"""export_coffin_assets 黑盒测试 —— 断言落在生成的 geo/animation/贴图输出上，不绑内部调用。

happy path 用已 tracked 的 local_models/CoffinCommon.bbmodel（与已提交 coffin_common.geo.json
对拍）；边界/错误分支用合成 bbmodel dict，免依赖 user-local 的 4 档定稿 bbmodel。
"""

from __future__ import annotations

import base64
import io
import unittest

from PIL import Image

import export_coffin_assets as export


def _png_data_uri(w: int = 64, h: int = 64, color=(180, 90, 40, 255)) -> str:
    """造一张纯色 PNG，编码成 bbmodel 内嵌贴图的 data: URI。"""
    buf = io.BytesIO()
    Image.new("RGBA", (w, h), color).save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def _bb(elements, outliner, *, res=(64, 64), tex=True) -> dict:
    """最小合成 bbmodel。"""
    d = {
        "resolution": {"width": res[0], "height": res[1]},
        "elements": elements,
        "outliner": outliner,
    }
    if tex:
        d["textures"] = [{"name": "t", "source": _png_data_uri(*res)}]
    else:
        d["textures"] = []
    return d


def _cube(uuid, name, frm, to, faces=None, **extra) -> dict:
    e = {"uuid": uuid, "name": name, "from": frm, "to": to, "faces": faces or {}}
    e.update(extra)
    return e


def _lid_pivot_x(geo: dict) -> float:
    for b in geo["minecraft:geometry"][0]["bones"]:
        if b["name"] == "lid":
            return b["pivot"][0]
    raise AssertionError("无 lid 骨骼")


class CubeConversionTest(unittest.TestCase):
    def test_origin_size_uv_direct_mapping(self) -> None:
        e = _cube("u", "x", [-7, 0, -12], [7, 2, 12], {"north": {"uv": [0, 0, 14, 2]}})
        c = export._cube_from_element(e)
        self.assertEqual([-7, 0, -12], c["origin"], "origin 应等于 bbmodel from")
        self.assertEqual([14, 2, 24], c["size"], "size 应为 to-from")
        self.assertEqual([0, 0], c["uv"]["north"]["uv"])
        self.assertEqual([14, 2], c["uv"]["north"]["uv_size"], "uv_size=[u2-u1,v2-v1]")

    def test_mirrored_uv_size_keeps_negative(self) -> None:
        # u2 < u1 / v2 < v1 的镜像面，uv_size 应保留负值，不可裁成 0/正
        e = _cube("u", "x", [0, 0, 0], [2, 2, 2], {"south": {"uv": [10, 8, 2, 0]}})
        c = export._cube_from_element(e)
        self.assertEqual([-8, -8], c["uv"]["south"]["uv_size"])

    def test_element_rotation_emits_rotation_and_pivot(self) -> None:
        e = _cube("u", "x", [0, 0, 0], [2, 2, 2], rotation=[0, 45, 0], origin=[1, 2, 3])
        c = export._cube_from_element(e)
        self.assertEqual([0, 45, 0], c["rotation"])
        self.assertEqual([1, 2, 3], c["pivot"], "pivot 应取 element origin")

    def test_face_rotation_rejected(self) -> None:
        e = _cube("u", "x", [0, 0, 0], [2, 2, 2], {"up": {"uv": [0, 0, 2, 2], "rotation": 90}})
        with self.assertRaisesRegex(ValueError, "texture rotation"):
            export._cube_from_element(e)

    def test_mesh_element_rejected(self) -> None:
        bb = _bb([{"uuid": "m", "name": "blob", "type": "mesh"}], [{"uuid": "g", "children": ["m"]}])
        with self.assertRaisesRegex(ValueError, "mesh"):
            export._elements_by_uuid(bb)


class BoneAssignmentTest(unittest.TestCase):
    def _three_group_bb(self, lid_group_extra=None):
        # base 低 / body 中 / lid 高（按平均 Y 排序应判成 base/body/lid）
        els = [
            _cube("b0", "base", [-7, 0, -12], [7, 2, 12], {"north": {"uv": [0, 0, 14, 2]}}),
            _cube("y0", "body", [-7, 2, -12], [7, 8, 12], {"north": {"uv": [0, 4, 14, 10]}}),
            _cube("l0", "lid", [-5, 8, -10], [5, 11, 10], {"north": {"uv": [0, 16, 10, 19]}}),
        ]
        lid_group = {"uuid": "gl", "children": ["l0"]}
        if lid_group_extra:
            lid_group.update(lid_group_extra)
        outliner = [
            {"uuid": "gb", "children": ["b0"]},
            {"uuid": "gy", "children": ["y0"]},
            lid_group,
        ]
        return _bb(els, outliner)

    def test_three_unnamed_groups_named_base_body_lid_by_meanY(self) -> None:
        geo = export.build_geo(self._three_group_bb(), "geometry.bong.t_coffin")
        names = [b["name"] for b in geo["minecraft:geometry"][0]["bones"]]
        self.assertEqual(["root", "base", "body", "lid"], names)
        for b in geo["minecraft:geometry"][0]["bones"][1:]:
            self.assertEqual("root", b["parent"], "每根 bone 应挂在 root 下")

    def test_lid_pivot_centered_ignores_dirty_group_origin(self) -> None:
        # 静态 lid group 带非居中 origin（生成器脏值）：pivot 仍应取 cube X 中心(0.0)
        geo = export.build_geo(
            self._three_group_bb({"origin": [-5.0, 8.5, 0.0]}), "geometry.bong.t_coffin"
        )
        self.assertEqual(0.0, _lid_pivot_x(geo), "lid pivot X 须居中，不可继承脏 group origin")

    def test_bad_lid_pivot_assertion_fires_on_rotated_offcenter_group(self) -> None:
        # 带旋转的 lid group 沿用其 origin；若 origin 偏轴，后验断言须撞红
        with self.assertRaisesRegex(ValueError, "lid pivot"):
            export.build_geo(
                self._three_group_bb({"rotation": [0, 0, 5], "origin": [-5.0, 8.5, 0.0]}),
                "geometry.bong.t_coffin",
            )

    def test_orphan_cube_collected_into_loose_bone(self) -> None:
        # 有 element 未挂进任何 group → 应兜底进 loose 骨骼，不漏渲染
        els = [
            _cube("a", "grouped", [0, 0, 0], [2, 2, 2], {"north": {"uv": [0, 0, 2, 2]}}),
            _cube("orphan", "stray", [4, 4, 4], [6, 6, 6], {"north": {"uv": [0, 0, 2, 2]}}),
        ]
        geo = export.build_geo(_bb(els, [{"uuid": "g", "children": ["a"]}]), "geometry.bong.t_coffin")
        bones = {b["name"]: b for b in geo["minecraft:geometry"][0]["bones"]}
        self.assertIn("loose", bones, "未分组 element 应进 loose 骨骼")
        loose_origins = [c["origin"] for c in bones["loose"]["cubes"]]
        self.assertIn([4, 4, 4], loose_origins)

    def test_nested_group_with_transform_rejected(self) -> None:
        # 嵌套子 group 带 rotation/origin 会被展平丢失变换 → 必须 fail-loud，不可静默出错几何
        els = [_cube("a", "x", [0, 0, 0], [2, 2, 2], {"north": {"uv": [0, 0, 2, 2]}})]
        outliner = [
            {
                "uuid": "g",
                "children": [
                    {"uuid": "sub", "rotation": [0, 30, 0], "origin": [1, 1, 1], "children": ["a"]}
                ],
            }
        ]
        with self.assertRaisesRegex(ValueError, "嵌套 group"):
            export.build_geo(_bb(els, outliner), "geometry.bong.t_coffin")

    def test_nested_group_without_transform_flattens_ok(self) -> None:
        # 嵌套子 group 无变换：展平收集其 cube 不报错
        els = [_cube("a", "x", [0, 0, 0], [2, 2, 2], {"north": {"uv": [0, 0, 2, 2]}})]
        outliner = [{"uuid": "g", "children": [{"uuid": "sub", "children": ["a"]}]}]
        geo = export.build_geo(_bb(els, outliner), "geometry.bong.t_coffin")
        n_cubes = sum(len(b.get("cubes", [])) for b in geo["minecraft:geometry"][0]["bones"])
        self.assertEqual(1, n_cubes, "无变换嵌套 group 的 cube 应被正常收集")

    def test_non_three_groups_highest_is_lid(self) -> None:
        # 非典型组数（2 组）：最高 Y 那组仍叫 lid，供动画定位
        els = [
            _cube("lo", "low", [0, 0, 0], [2, 2, 2], {"north": {"uv": [0, 0, 2, 2]}}),
            _cube("hi", "high", [0, 9, 0], [2, 11, 2], {"north": {"uv": [0, 0, 2, 2]}}),
        ]
        outliner = [{"uuid": "g0", "children": ["lo"]}, {"uuid": "g1", "children": ["hi"]}]
        geo = export.build_geo(_bb(els, outliner), "geometry.bong.t_coffin")
        names = [b["name"] for b in geo["minecraft:geometry"][0]["bones"]]
        self.assertIn("lid", names, "非 3 组时最高 Y 组仍须命名 lid")


class GeoDescriptionTest(unittest.TestCase):
    def test_texture_dims_track_resolution(self) -> None:
        geo = export.build_geo(
            _bb(
                [_cube("a", "x", [0, 0, 0], [2, 2, 2], {"north": {"uv": [0, 0, 2, 2]}})],
                [{"uuid": "g", "children": ["a"]}],
                res=(32, 16),
            ),
            "geometry.bong.t_coffin",
        )
        desc = geo["minecraft:geometry"][0]["description"]
        self.assertEqual("geometry.bong.t_coffin", desc["identifier"])
        self.assertEqual(32, desc["texture_width"])
        self.assertEqual(16, desc["texture_height"])
        self.assertEqual("1.12.0", geo["format_version"])


class TextureExtractTest(unittest.TestCase):
    def test_extract_returns_png_bytes_and_actual_dims(self) -> None:
        bb = _bb([], [], res=(48, 24))
        png, w, h = export.extract_texture(bb)
        self.assertEqual((48, 24), (w, h), "返回尺寸须为实际 PNG 像素")
        self.assertEqual(b"\x89PNG\r\n\x1a\n", png[:8], "应为合法 PNG 字节")
        # 二次解码确认尺寸权威
        self.assertEqual((48, 24), Image.open(io.BytesIO(png)).size)

    def test_missing_texture_raises(self) -> None:
        bb = _bb([], [], tex=False)  # textures: []
        with self.assertRaises((IndexError, KeyError)):
            export.extract_texture(bb)


class AnimationTest(unittest.TestCase):
    def test_idle_targets_lid_with_per_tier_gradient(self) -> None:
        seen = []
        for tier in ("mundane", "jade", "stone", "bronze"):
            a = export.build_animation(tier)
            key = f"animation.bong.{tier}_coffin.idle"
            self.assertIn(key, a["animations"], "动画 key 命名须含档位")
            anim = a["animations"][key]
            self.assertTrue(anim["loop"])
            self.assertIn("lid", anim["bones"], "idle 须对 lid 骨骼")
            z_max = max(v[2] for v in anim["bones"]["lid"]["rotation"].values())
            seen.append((anim["animation_length"], z_max))
        # 四档摆幅/时长应严格递增（越高级摆动越显），不得同质
        self.assertEqual(seen, sorted(seen), "四档 idle 梯度应单调递增")
        self.assertEqual(len(set(seen)), 4, "四档 idle 不得完全同质")


class OracleTest(unittest.TestCase):
    def test_verify_oracle_reproduces_committed_coffin_common_geo(self) -> None:
        # 用已 tracked 的 CoffinCommon.bbmodel：转换 cube 须复现 coffin_common.geo.json
        self.assertTrue(
            export.verify_oracle(),
            "bbmodel→geo 转换关系应与 Blockbench 官方导出（coffin_common.geo.json）一致",
        )


if __name__ == "__main__":
    unittest.main()
