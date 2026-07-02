from __future__ import annotations

import base64
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gen_loot_crates as lc


class PackerZoneClampTest(unittest.TestCase):
    def test_cube_faces_uv_never_exceeds_material_zone(self) -> None:
        # 回归锁：超区大面曾写出 v>64 的 UV，光栅化越界撕裂（LootCrateVineChest cavity 实证）
        packer = lc.Packer(0, 57, lc.RES, 64)  # 高度仅 7 的窄分区
        faces = lc.cube_faces_uv([-6.1, 1.0, -5.1], [6.1, 9.8, 5.1], packer)
        for name, face in faces.items():
            u0, v0, u1, v1 = face["uv"]
            self.assertGreaterEqual(v0, 57, f"{name} v0 越出分区上界")
            self.assertLessEqual(v1, 64, f"{name} v1={v1} 越出分区下界（应被钳制）")
            self.assertLessEqual(u1, lc.RES)


class VariantSpecContractTest(unittest.TestCase):
    def test_five_variants_registered_with_expected_keys(self) -> None:
        self.assertEqual(
            {"bone_lash", "talisman", "rust_trunk", "vine_chest", "ash_urn"},
            set(lc.VARIANTS),
        )

    def test_every_variant_cubes_are_well_formed(self) -> None:
        for key, spec in lc.VARIANTS.items():
            cubes = spec.build_fn()
            self.assertGreater(len(cubes), 10, f"{key} 立方体数量过少")
            for bone, material, name, frm, to in cubes:
                self.assertIn(bone, spec.bone_order, f"{key}/{name} 骨骼未登记")
                self.assertIn(material, spec.material_zones, f"{key}/{name} 材质未登记")
                for axis in range(3):
                    self.assertLess(
                        frm[axis], to[axis],
                        f"{key}/{name} 轴 {axis} from>=to（倒置立方体会翻法线）",
                    )

    def test_every_variant_has_openable_bone_with_pivot(self) -> None:
        # 开箱动画契约：每变种必须有 lid 或 seal 独立骨骼且 pivot 已定义
        for key, spec in lc.VARIANTS.items():
            openable = {"lid", "seal"} & set(spec.bone_order)
            self.assertTrue(openable, f"{key} 缺 lid/seal 骨骼")
            for bone in openable:
                self.assertIn(bone, spec.bone_pivots)
                self.assertEqual(3, len(spec.bone_pivots[bone]))

    def test_bone_order_pivots_colors_zones_consistent(self) -> None:
        for key, spec in lc.VARIANTS.items():
            self.assertEqual(set(spec.bone_order), set(spec.bone_pivots), key)
            self.assertEqual(set(spec.bone_order), set(spec.bone_colors), key)
            for zone in spec.material_zones.values():
                x0, y0, x1, y1 = zone
                self.assertTrue(0 <= x0 < x1 <= lc.RES and 0 <= y0 < y1 <= lc.RES, key)


class BbmodelOutputContractTest(unittest.TestCase):
    def _build(self, key: str):
        import numpy as np

        spec = lc.VARIANTS[key]
        rng = np.random.default_rng(1)
        cubes = spec.build_fn()
        tex = spec.texture_fn(rng)
        return spec, cubes, lc.build_bbmodel(spec, cubes, tex), tex

    def test_bbmodel_structure_for_all_variants(self) -> None:
        for key in lc.VARIANTS:
            spec, cubes, model, _ = self._build(key)
            self.assertEqual(f"loot_crate_{key}", model["name"])
            self.assertEqual(len(cubes), len(model["elements"]))
            self.assertEqual(spec.bone_order,
                             [o["name"] for o in model["outliner"]])
            child_ids = {c for o in model["outliner"] for c in o["children"]}
            self.assertEqual({e["uuid"] for e in model["elements"]}, child_ids,
                             f"{key} 有元素未挂进任何骨骼（孤儿 cube）")
            for element in model["elements"]:
                faces = element["faces"]
                self.assertEqual(
                    {"north", "south", "east", "west", "up", "down"}, set(faces))
                for face in faces.values():
                    u0, v0, u1, v1 = face["uv"]
                    self.assertTrue(0 <= u0 <= u1 <= lc.RES)
                    self.assertTrue(0 <= v0 <= v1 <= lc.RES)

    def test_embedded_texture_is_opaque_rgba_png_at_res(self) -> None:
        for key in lc.VARIANTS:
            _, _, model, _ = self._build(key)
            src = model["textures"][0]["source"]
            self.assertTrue(src.startswith("data:image/png;base64,"))
            img = Image.open(io.BytesIO(base64.b64decode(src.split(",", 1)[1])))
            self.assertEqual("RGBA", img.mode)
            self.assertEqual((lc.RES, lc.RES), img.size)
            alpha = img.getchannel("A").getextrema()
            self.assertEqual((255, 255), alpha, f"{key} 贴图含透明像素")


class CliTest(unittest.TestCase):
    def _run(self, argv: list[str], tmp: Path) -> None:
        with mock.patch.object(lc, "LOCAL_MODELS", tmp / "local_models"), \
                mock.patch.object(lc, "PREVIEW_DIR", tmp / "previews"), \
                mock.patch.object(sys, "argv", ["gen_loot_crates.py", *argv]):
            (tmp / "previews").mkdir(parents=True, exist_ok=True)
            lc.main()

    def test_single_variant_writes_one_bbmodel_and_preview_no_combined(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            self._run(["--variant", "ash_urn"], tmp)
            models = sorted(p.name for p in (tmp / "local_models").glob("*.bbmodel"))
            self.assertEqual(["LootCrateAshUrn.bbmodel"], models)
            json.loads((tmp / "local_models" / "LootCrateAshUrn.bbmodel").read_text())
            self.assertTrue((tmp / "previews" / "loot_crate_ash_urn_preview.png").exists())
            self.assertFalse((tmp / "previews" / "loot_crates_preview_all.png").exists())

    def test_preview_only_writes_no_bbmodel(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            self._run(["--variant", "rust_trunk", "--preview-only"], tmp)
            self.assertFalse((tmp / "local_models").exists())
            self.assertTrue(
                (tmp / "previews" / "loot_crate_rust_trunk_preview.png").exists())

    def test_full_run_writes_five_bbmodels_and_combined_preview(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            self._run([], tmp)
            models = sorted(p.name for p in (tmp / "local_models").glob("*.bbmodel"))
            self.assertEqual(
                ["LootCrateAshUrn.bbmodel", "LootCrateBoneLash.bbmodel",
                 "LootCrateRustTrunk.bbmodel", "LootCrateTalisman.bbmodel",
                 "LootCrateVineChest.bbmodel"],
                models,
            )
            self.assertTrue((tmp / "previews" / "loot_crates_preview_all.png").exists())

    def test_unknown_variant_rejected_by_argparse(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            with self.assertRaises(SystemExit) as ctx:
                self._run(["--variant", "not_a_crate"], Path(td))
            self.assertEqual(2, ctx.exception.code)


if __name__ == "__main__":
    unittest.main()
