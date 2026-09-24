"""core 的 namespace 参数化。

拆库前这几处把 `"bong"` 直接写进 bbmodel 元数据和资源路径里。库不该知道调用方的
资源命名空间叫什么——写死了别的项目就用不了，而且错的命名空间在 MC 里是**静默**
失效（贴图找不到就是一片紫黑，不报错）。
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

CORE = Path(__file__).resolve().parents[1] / "core"
sys.path.insert(0, str(CORE))

from bbmodel_maker.model import armor_model_common as AMC  # noqa: E402
from bbmodel_maker import workspace  # noqa: E402
from PIL import Image  # noqa: E402


def _texture() -> Image.Image:
    return Image.new("RGBA", (AMC.TEXTURE_SIZE, AMC.TEXTURE_SIZE), (0, 0, 0, 0))


def _part() -> AMC.ArmorPart:
    return AMC.ArmorPart(
        key="helmet",
        display_name="头盔",
        cubes=(
            AMC.Cube(
                mount="HEAD", name="c",
                origin=(0.0, 0.0, 0.0), size=(1.0, 1.0, 1.0), uv=(0, 0),
            ),
        ),
    )


class ArmorNamespaceTest(unittest.TestCase):
    def setUp(self) -> None:
        self._saved = workspace._DEFAULT
        self.addCleanup(lambda: workspace.set_default(self._saved))

    def _namespace_of(self, model: dict) -> str:
        return model["textures"][0]["namespace"]

    def test_explicit_namespace_is_used(self) -> None:
        model = AMC.build_bbmodel("iron", _part(), _texture(), namespace="acme")
        self.assertEqual(
            self._namespace_of(model), "acme",
            "期望：显式传入的 namespace 写进 bbmodel 贴图元数据；实际取了别的值",
        )

    def test_falls_back_to_workspace_default(self) -> None:
        workspace.set_default(
            workspace.Workspace(root=Path("/p"), lib=Path("/p/lib"), namespace="from-ws")
        )
        model = AMC.build_bbmodel("iron", _part(), _texture())
        self.assertEqual(
            self._namespace_of(model), "from-ws",
            "期望：不传 namespace 时取 workspace 的配置值（bbmodel.toml 说了算）；"
            "实际没走 workspace",
        )

    def test_bong_is_not_hardcoded(self) -> None:
        """把 workspace 换成别的命名空间后，产出里不该再出现 bong。"""
        workspace.set_default(
            workspace.Workspace(root=Path("/p"), lib=Path("/p/lib"), namespace="other")
        )
        model = AMC.build_bbmodel("iron", _part(), _texture())
        self.assertNotIn(
            "bong", str(model),
            "期望：换了命名空间后产出里不含 bong；实际仍有硬编——"
            "MC 里命名空间错了是静默失效（贴图变紫黑），不会报错",
        )

    def test_explicit_beats_workspace(self) -> None:
        workspace.set_default(
            workspace.Workspace(root=Path("/p"), lib=Path("/p/lib"), namespace="from-ws")
        )
        model = AMC.build_bbmodel("iron", _part(), _texture(), namespace="explicit")
        self.assertEqual(self._namespace_of(model), "explicit")

    def test_repository_default_is_bong(self) -> None:
        """本仓库的 bbmodel.toml 声明 namespace = "bong"，产出不该变。"""
        workspace.set_default(None)
        ws = workspace.Workspace.discover(start=CORE)
        self.assertEqual(
            ws.namespace, "bong",
            f"期望：Bong 仓库根的 bbmodel.toml 把 namespace 定为 bong，"
            f"参数化不改变既有产出；实际 {ws.namespace}（根 {ws.root}，来源 {ws.source}）",
        )


class HeldItemNamespaceTest(unittest.TestCase):
    """held_item_common 把 namespace 拼进 MTL / model JSON / bbmodel 元数据。"""

    def setUp(self) -> None:
        self._saved = workspace._DEFAULT
        self.addCleanup(lambda: workspace.set_default(self._saved))
        workspace.set_default(
            workspace.Workspace(root=Path("/p"), lib=Path("/p/lib"), namespace="other")
        )

    @staticmethod
    def _item():
        from bbmodel_maker.render import held_item_common as HIC

        return HIC.HeldItem(
            key="test_blade",
            display_name="测试刃",
            host_item="stick",
            boxes=(),
            materials=(
                HIC.Material(
                    name="steel", kd=(0.5, 0.5, 0.5),
                    texture=Image.new("RGBA", (16, 16), (0, 0, 0, 0)),
                ),
            ),
            display={},
            grip=0.0,
        )

    def test_mtl_uses_workspace_namespace(self) -> None:
        from bbmodel_maker.render import held_item_common as HIC

        mtl = HIC.build_mtl(self._item())
        self.assertIn("map_Kd other:item/test_blade/0", mtl)
        self.assertNotIn(
            "bong", mtl,
            "期望：MTL 里的贴图引用跟随 workspace 命名空间；实际仍写死 bong——"
            "MC 找不到贴图不会报错，只会渲成紫黑",
        )

    def test_model_json_uses_workspace_namespace(self) -> None:
        from bbmodel_maker.render import held_item_common as HIC

        payload = HIC.build_model_json(self._item())
        self.assertIn('"other:models/item/test_blade/test_blade.obj"', payload)
        self.assertNotIn("bong", payload)

    def test_explicit_namespace_beats_workspace(self) -> None:
        from bbmodel_maker.render import held_item_common as HIC

        self.assertIn("acme:", HIC.build_model_json(self._item(), namespace="acme"))

    def test_bbmodel_model_identifier_follows_namespace(self) -> None:
        """`model_identifier` 是 f"geometry.<ns>.<key>"，最早漏掉的就是它。"""
        from bbmodel_maker.render import held_item_common as HIC

        model = HIC.build_bbmodel(self._item())
        self.assertEqual(
            model["model_identifier"], "geometry.other.test_blade",
            f"期望：model_identifier 跟随命名空间；实际 {model['model_identifier']}——"
            f"这处藏在 f-string 里，grep '\"bong\"' 抓不到",
        )

    def test_signature_exposes_namespace(self) -> None:
        import inspect

        from bbmodel_maker.render import held_item_common as HIC

        for fn in (HIC.build_bbmodel, HIC.write_assets):
            with self.subTest(fn=fn.__name__):
                params = inspect.signature(fn).parameters
                self.assertIn(
                    "namespace", params,
                    f"期望：{fn.__name__} 暴露 namespace 参数，让调用方决定资源命名空间；"
                    f"实际签名是 {list(params)}",
                )
                self.assertIsNone(
                    params["namespace"].default,
                    "期望：默认 None（延迟到 workspace 解析），不要把某个具体命名空间"
                    "焊死在签名里",
                )


class RigNamespaceTest(unittest.TestCase):
    def test_rig_builders_expose_namespace(self) -> None:
        import inspect

        from bbmodel_maker.rig import rigkit
        from bbmodel_maker.rig import voxel_rig

        for mod in (rigkit, voxel_rig):
            cls = next(
                v for v in vars(mod).values()
                if isinstance(v, type) and hasattr(v, "bbmodel")
            )
            with self.subTest(module=mod.__name__):
                params = inspect.signature(cls.bbmodel).parameters
                self.assertIn(
                    "namespace", params,
                    f"期望：{mod.__name__}.{cls.__name__}.bbmodel 暴露 namespace；"
                    f"实际 {list(params)}",
                )


if __name__ == "__main__":
    unittest.main()
