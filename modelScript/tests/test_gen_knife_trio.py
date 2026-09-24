#!/usr/bin/env python3
"""小刀三件套 + held_item_common 的回归锁。

重点钉两类东西：

1. **那些"看着像错、其实是刻意的"决定**——最典型是长宽比。参考图是 5.9~6.9:1
   的实物照，这里压到 3.9~4.7:1。任何人拿参考图来对，都会觉得这是没做到位，
   顺手"修回去"——修回去的结果 round 1 实测过：三把分别读成火把、针、电视塔。
   所以给它一条带理由的断言。
2. **OBJ / MTL / bbmodel 三份产物的一致性**。它们从同一张 box 表出，一致性是本
   模块存在的全部意义；断了就退回成"两套各写一份"的旧状态。

每条校验都配变异用例：造坏输入喂进去，断言它真报错。只测"好输入不报错"等于
没测——把校验函数整个删掉那种测试照样绿。
"""

from __future__ import annotations

import re
import sys
import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
for _d in ("generators",):
    sys.path.insert(0, str(LIB_DIR / _d))

import gen_knife_trio as knives  # noqa: E402
from bbmodel_maker.render.held_item_common import (  # noqa: E402
    TILE,
    Box,
    HeldItem,
    Material,
    assert_conventions,
    assert_host_is_claimable,
    assert_no_coplanar_faces,
    emit_offset,
    build_atlas,
    build_bbmodel,
    build_model_json,
    build_mtl,
    build_obj,
)


def _swatch(rgb=(128, 128, 128)) -> Image.Image:
    return Image.new("RGBA", (TILE, TILE), (*rgb, 255))


def _probe(boxes, materials=None, grip=0.15) -> HeldItem:
    materials = materials or (Material("m", (0.5, 0.5, 0.5), _swatch()),)
    return HeldItem("probe", "PROBE", "stick", tuple(boxes), tuple(materials), {}, grip)


def _mean_rgb(image: Image.Image) -> tuple[float, float, float]:
    data = image.convert("RGB").tobytes()
    n = len(data) // 3
    return tuple(sum(data[i::3]) / n for i in range(3))


class KnifeSetTest(unittest.TestCase):
    def test_exposes_the_three_template_ids(self) -> None:
        self.assertEqual(
            ["stone_knife", "iron_dagger", "bone_spike"],
            [i.key for i in knives.items()],
        )

    def test_host_items_are_distinct(self) -> None:
        """三件各劫持一个不同的 vanilla item model。撞车 = 后写的把先写的覆盖掉，
        两件在游戏里长成同一个样，而且不会有任何报错。"""
        hosts = [i.host_item for i in knives.items()]
        self.assertEqual(len(hosts), len(set(hosts)), f"宿主 item 撞车：{hosts}")

    def test_every_item_passes_its_own_guards(self) -> None:
        for item in knives.items():
            assert_conventions(item)
            assert_no_coplanar_faces(item)

    def test_aspect_ratio_is_deliberately_chunkier_than_the_reference(self) -> None:
        """长宽比必须在 3.5~5.2，**不是**参考图的 5.9~6.9。

        参考是实物照。照抄进 MC 是错的：手持物在屏幕上就三十来像素，6.9:1 渲出来
        是一根针——round 1 实测三把分别读成火把、针、电视塔。原版剑连护手约 4:1，
        这里对齐它。这条断言存在的唯一目的，就是拦住"照参考图修回细长"。
        """
        for item in knives.items():
            length = max(b.high[1] for b in item.boxes)
            width = (max(b.high[0] for b in item.boxes)
                     - min(b.low[0] for b in item.boxes))
            ratio = length / width
            self.assertTrue(
                3.5 <= ratio <= 5.2,
                f"{item.key} 长宽比 {ratio:.2f} 出界。参考图是 5.9~6.9，但那是实物照；"
                f"MC 手持物照抄会渲成一根针。别按参考改回去。",
            )

    def test_blades_are_thick_enough_to_catch_side_light(self) -> None:
        """刃厚不得低于 0.015。更薄的片在 item 光照下侧面几乎不受光，
        转到侧视整片刃会"消失"。"""
        for item in knives.items():
            for box in item.boxes:
                self.assertGreaterEqual(
                    box.half[2] * 2, 0.015,
                    f"{item.key}/{box.name} 厚 {box.half[2] * 2:.4f} < 0.015，侧视会消失",
                )

    def test_taper_is_not_evenly_segmented(self) -> None:
        """收刃必须"中段一长条 + 尖部快收"，不是等分多段。

        等长多段渲出来是一道楼梯。判据：刃族里**最长的那段**要显著长于中位数——
        中段那条长的才是眼睛读到的"刃"。
        """
        for item, prefix in ((knives.STONE_KNIFE, "blade_"),
                             (knives.IRON_DAGGER, "blade_"),
                             (knives.BONE_SPIKE, "point_")):
            heights = sorted(b.half[1] * 2 for b in item.boxes if b.name.startswith(prefix))
            self.assertGreaterEqual(len(heights), 3, f"{item.key} 刃段太少")
            median = heights[len(heights) // 2]
            self.assertGreater(
                heights[-1], median * 1.25,
                f"{item.key} 刃段高度 {heights} 太平均，会渲成楼梯",
            )

    def test_the_three_read_as_different_materials(self) -> None:
        """三把的主材质平均色必须两两拉开。

        这是本组资产的**玩法判据**：玩家要能在手里一眼分出拿的是石刃、铁匕还是
        骨刺。round 1 铁刃去掉暖偏做成纯灰，和石刃就分不开了。
        """
        primary = {i.key: _mean_rgb(i.materials[0].texture) for i in knives.items()}
        keys = list(primary)
        for a in range(len(keys)):
            for b in range(a + 1, len(keys)):
                ka, kb = keys[a], keys[b]
                dist = sum((primary[ka][i] - primary[kb][i]) ** 2 for i in range(3)) ** 0.5
                self.assertGreater(
                    dist, 30.0,
                    f"{ka} 与 {kb} 的主材质平均色只差 {dist:.1f}，手里分不出是哪把",
                )

    def test_iron_is_warm_not_neutral_grey(self) -> None:
        """铁必须带暖偏（锈）。纯灰的铁和石头分不开——round 1 的原样。"""
        r, _, b = _mean_rgb(knives.IRON_DAGGER.materials[0].texture)
        self.assertGreater(r - b, 8.0, f"铁刃 R-B={r - b:.1f}，太中性，读成石头")

    def test_textures_are_deterministic(self) -> None:
        first = {i.key: [m.texture.tobytes() for m in i.materials] for i in knives.items()}
        import importlib

        importlib.reload(knives)
        second = {i.key: [m.texture.tobytes() for m in i.materials] for i in knives.items()}
        self.assertEqual(first, second)


class HeldItemGuardTest(unittest.TestCase):
    """两道校验的变异测试。"""

    def test_conventions_reject_butt_off_the_origin(self) -> None:
        """y=0 必须落在握把末端。偏了的话这件的 display 变换和 axe_bone 那套基线
        对不上，手持时插进手掌或飘在外面——而三视图里看不出来。"""
        bad = _probe([Box("b", "m", (0.0, 0.4, 0.0), (0.04, 0.3, 0.03))])
        with self.assertRaisesRegex(ValueError, "握把末端"):
            assert_conventions(bad)

    def test_conventions_accept_butt_exactly_at_origin(self) -> None:
        assert_conventions(_probe([Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))]))

    def test_conventions_reject_absurd_length(self) -> None:
        for half_y, why in ((0.10, "太短"), (0.70, "太长")):
            bad = _probe([Box("b", "m", (0.0, half_y, 0.0), (0.04, half_y, 0.03))])
            with self.assertRaisesRegex(ValueError, "合理区间"):
                assert_conventions(bad)

    def test_conventions_reject_unused_material(self) -> None:
        """定义了没人用的 material 会白占一张 16² 贴图，还在 MTL 里挂个死条目。"""
        bad = _probe(
            [Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))],
            materials=(Material("m", (0.5,) * 3, _swatch()),
                       Material("ghost", (0.5,) * 3, _swatch())),
        )
        with self.assertRaisesRegex(ValueError, "没有 box 用"):
            assert_conventions(bad)

    def test_conventions_reject_unknown_material(self) -> None:
        bad = _probe([Box("b", "nope", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))])
        with self.assertRaisesRegex(ValueError, "未知 material"):
            assert_conventions(bad)

    def test_conventions_reject_wrong_texture_size(self) -> None:
        bad = _probe(
            [Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))],
            materials=(Material("m", (0.5,) * 3, Image.new("RGBA", (32, 32))),),
        )
        with self.assertRaisesRegex(ValueError, "不是"):
            assert_conventions(bad)

    def test_conventions_reject_duplicate_box_names(self) -> None:
        bad = _probe([Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03)),
                      Box("b", "m", (0.0, 0.60, 0.2), (0.04, 0.20, 0.03))])
        with self.assertRaisesRegex(ValueError, "重名"):
            assert_conventions(bad)

    def test_coplanar_guard_catches_two_blade_segments_at_the_same_half_width(self) -> None:
        """刃分段最容易犯的错：两段图省事写成同一个 x 半宽 → 侧面共面 z-fighting。"""
        bad = _probe([Box("s1", "m", (0.0, 0.20, 0.0), (0.05, 0.20, 0.02)),
                      Box("s2", "m", (0.0, 0.50, 0.0), (0.05, 0.12, 0.02))])
        with self.assertRaisesRegex(ValueError, "共面"):
            assert_no_coplanar_faces(bad)

    def test_coplanar_guard_allows_stacked_segments_that_merely_abut(self) -> None:
        """上下相接（a 的 y-max == b 的 y-min）是正常拼接，不是共面。"""
        assert_no_coplanar_faces(_probe([
            Box("s1", "m", (0.0, 0.20, 0.0), (0.05, 0.20, 0.02)),
            Box("s2", "m", (0.0, 0.52, 0.0), (0.04, 0.12, 0.018)),
        ]))


class ObjMtlBbmodelConsistencyTest(unittest.TestCase):
    """三份产物同源，一致性断了就退回成"两套各写一份"的旧状态。"""

    def test_obj_geometry_counts_and_index_range(self) -> None:
        for item in knives.items():
            obj = build_obj(item)
            n = len(item.boxes)
            verts = [l for l in obj.splitlines() if l.startswith("v ")]
            faces = [l for l in obj.splitlines() if l.startswith("f ")]
            self.assertEqual(8 * n, len(verts), f"{item.key} 顶点数不是 8×box")
            self.assertEqual(6 * n, len(faces), f"{item.key} 面数不是 6×box")
            for line in faces:
                for token in line.split()[1:]:
                    vi, vti, vni = (int(x) for x in token.split("/"))
                    self.assertTrue(1 <= vi <= len(verts), f"{item.key} 顶点索引 {vi} 越界")
                    self.assertTrue(1 <= vti <= 4, f"{item.key} vt 索引 {vti} 越界")
                    self.assertTrue(1 <= vni <= 6, f"{item.key} vn 索引 {vni} 越界")

    def test_every_usemtl_is_declared_in_the_mtl(self) -> None:
        for item in knives.items():
            used = set(re.findall(r"^usemtl (\S+)$", build_obj(item), re.M))
            declared = set(re.findall(r"^newmtl (\S+)$", build_mtl(item), re.M))
            self.assertEqual(used, declared, f"{item.key} OBJ 与 MTL 的 material 对不上")

    def test_map_kd_index_matches_material_order(self) -> None:
        """MTL 里第 n 个 material 的 map_Kd 必须指向 textures/item/<key>/n.png，
        因为落盘时贴图就是按 enumerate 顺序存的。错位 = 木柄贴成铁色。"""
        for item in knives.items():
            maps = re.findall(r"^newmtl (\S+)$|^map_Kd bong:item/(\S+)/(\d+)$",
                              build_mtl(item), re.M)
            pairs = []
            pending = None
            for name, key, index in maps:
                if name:
                    pending = name
                else:
                    pairs.append((pending, key, int(index)))
            self.assertEqual(len(item.materials), len(pairs))
            for order, (name, key, index) in enumerate(pairs):
                self.assertEqual(item.materials[order].name, name)
                self.assertEqual(item.key, key)
                self.assertEqual(order, index, f"{item.key}/{name} map_Kd 序号错位")

    def test_obj_vertices_match_the_box_table_shifted_by_emit_offset(self) -> None:
        """OBJ 坐标 = box 表 + `emit_offset`，一格不多一格不少。

        两者一旦能各自漂移这个公共层就白建了；而 offset 必须**真的加上去**，
        否则模型挂在方块角上，display 变换绕方块中心转 → 手持时刀飘在拳头外。
        """
        for item in knives.items():
            off = emit_offset(item)
            verts = [tuple(round(float(v), 4) for v in l.split()[1:4])
                     for l in build_obj(item).splitlines() if l.startswith("v ")]
            for index, box in enumerate(item.boxes):
                chunk = verts[index * 8:(index + 1) * 8]
                for axis in range(3):
                    self.assertAlmostEqual(round(box.low[axis] + off[axis], 4),
                                           min(v[axis] for v in chunk), places=4)
                    self.assertAlmostEqual(round(box.high[axis] + off[axis], 4),
                                           max(v[axis] for v in chunk), places=4)

    def test_emitted_grip_point_sits_at_the_block_centre(self) -> None:
        """出料系里握把点必须正好在 (8,8,8)px —— 那是 MC display 变换的枢轴
        (`ItemRenderer` 在 display 之后 translate(-0.5,-0.5,-0.5))。差一点点，
        整件在手里就偏一点点；差半个方块，就是"手没握住把柄"。"""
        for item in knives.items():
            off = emit_offset(item)
            self.assertAlmostEqual(0.5, off[0], places=6)
            self.assertAlmostEqual(0.5, off[2], places=6)
            self.assertAlmostEqual(0.5, item.grip + off[1], places=6,
                                   msg=f"{item.key} 握把点没落在方块中心")

    def test_conventions_reject_grip_off_the_model(self) -> None:
        for grip, why in ((0.0, "落在柄尾之下"), (0.9, "越过尖端")):
            bad = _probe([Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))], grip=grip)
            with self.assertRaisesRegex(ValueError, "grip"):
                assert_conventions(bad)

    def test_bbmodel_mirrors_the_same_boxes_at_16x(self) -> None:
        """bbmodel 写的是**出料系** ×16（px），和 OBJ 逐点同源。

        两边差一个平移 = "预览里握得住、进游戏握不住"，而这正是预览工具唯一
        用来判手持姿态的依据，差了就再也发现不了。
        """
        for item in knives.items():
            model = build_bbmodel(item)
            off = emit_offset(item)
            self.assertEqual(len(item.boxes), len(model["elements"]))
            by_name = {e["name"]: e for e in model["elements"]}
            for box in item.boxes:
                element = by_name[box.name]
                for axis in range(3):
                    self.assertAlmostEqual((box.low[axis] + off[axis]) * 16.0,
                                           element["from"][axis], places=3)
                    self.assertAlmostEqual((box.high[axis] + off[axis]) * 16.0,
                                           element["to"][axis], places=3)

    def test_bbmodel_uuids_are_stable_across_runs(self) -> None:
        """uuid5 不 uuid4：uuid4 会让每次重跑都产出一份 diff，git 上分不清
        "改了造型"和"只是重跑了一遍"（棺材那批生成器踩过）。"""
        for item in knives.items():
            first = [e["uuid"] for e in build_bbmodel(item)["elements"]]
            second = [e["uuid"] for e in build_bbmodel(item)["elements"]]
            self.assertEqual(first, second)
            self.assertEqual(len(set(first)), len(first), f"{item.key} uuid 撞车")

    def test_bbmodel_atlas_width_covers_every_material_tile(self) -> None:
        for item in knives.items():
            model = build_bbmodel(item)
            expected = TILE * len(item.materials)
            self.assertEqual(expected, model["resolution"]["width"])
            self.assertEqual((expected, TILE), build_atlas(item).size)
            for element in model["elements"]:
                for face in element["faces"].values():
                    u0, _, u1, _ = face["uv"]
                    self.assertTrue(0 <= u0 < u1 <= expected, f"{item.key} 面 uv 越界")

    def test_model_json_points_at_the_obj_and_declares_every_display_slot(self) -> None:
        import json

        required = {"thirdperson_righthand", "thirdperson_lefthand",
                    "firstperson_righthand", "firstperson_lefthand",
                    "ground", "gui", "fixed"}
        for item in knives.items():
            data = json.loads(build_model_json(item))
            self.assertEqual("sml:builtin/obj", data["parent"])
            self.assertEqual(f"bong:models/item/{item.key}/{item.key}.obj", data["model"])
            self.assertTrue(required <= set(data["display"]),
                            f"{item.key} 缺 display 槽 {required - set(data['display'])}")

    def test_digest_style_change_detection(self) -> None:
        """挪一个 box，OBJ 必须跟着变——否则"改了造型但产物没变"会静默通过。"""
        item = knives.STONE_KNIFE
        moved = replace(item, boxes=(replace(item.boxes[-1], center=(0.02, 0.70, 0.0)),)
                        + item.boxes[:-1])
        self.assertNotEqual(build_obj(item), build_obj(moved))


if __name__ == "__main__":
    unittest.main()


class HostHijackGuardTest(unittest.TestCase):
    """宿主劫持的 fail-fast。粒度是「一个 vanilla item → 一份 model JSON」，
    写进去全局生效，所以撞车必须炸而不是静默覆盖。"""

    def _host_json(self, tmp: Path, model: str) -> Path:
        path = tmp / "host.json"
        path.write_text(json.dumps({"parent": "sml:builtin/obj", "model": model}),
                        encoding="utf-8")
        return path

    def test_rejects_two_items_sharing_a_host_in_one_run(self):
        item = _probe([Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))])
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError, "共用宿主"):
                assert_host_is_claimable(item, Path(tmp) / "x.json", {"stick": "别的件"})

    def test_rejects_overwriting_another_templates_host(self):
        """现实案例：`bone.json` 指向 bone_dagger，而 bone_spike 也宿在 bone 上。"""
        item = _probe([Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))])
        with tempfile.TemporaryDirectory() as tmp:
            host = self._host_json(Path(tmp), "bong:models/item/bone_dagger/bone_dagger.obj")
            with self.assertRaisesRegex(ValueError, "已经被占用"):
                assert_host_is_claimable(item, host, {})

    def test_allows_reclaiming_its_own_host(self):
        """重跑生成器覆盖自己上次写的那份，是正常操作，不该炸。"""
        item = _probe([Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))])
        with tempfile.TemporaryDirectory() as tmp:
            host = self._host_json(Path(tmp), "bong:models/item/probe/probe.obj")
            assert_host_is_claimable(item, host, {})

    def test_allows_a_host_file_that_does_not_exist_yet(self):
        item = _probe([Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))])
        with tempfile.TemporaryDirectory() as tmp:
            assert_host_is_claimable(item, Path(tmp) / "nope.json", {})

    def test_unreadable_host_is_not_treated_as_a_collision(self):
        """读不动就放行，交给后面的写入报真错——别用一个假的"撞车"掩盖 IO 问题。"""
        item = _probe([Box("b", "m", (0.0, 0.30, 0.0), (0.04, 0.30, 0.03))])
        with tempfile.TemporaryDirectory() as tmp:
            host = Path(tmp) / "broken.json"
            host.write_text("{ 这不是 JSON", encoding="utf-8")
            assert_host_is_claimable(item, host, {})

    def test_the_three_knives_still_collide_today(self):
        """三把刀现在一件也装不了，这是**当前事实**，不是疏忽。

        哪天 plan-held-item-registration-v1 落地、宿主机制被废掉，这条会红——
        那时候该做的是删掉它和 `--install` 里那段 SystemExit，而不是放宽。
        """
        hosts = {item.key: item.host_item for item in knives.items()}
        self.assertEqual(
            {"stone_knife": "stone_sword", "iron_dagger": "iron_ingot",
             "bone_spike": "bone"},
            hosts,
            "宿主分配变了，请重新核对 --install 那段拒绝理由还成不成立")
