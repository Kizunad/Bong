#!/usr/bin/env python3

from __future__ import annotations

import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in ("generators", "exporters", "tools"):
    sys.path.insert(0, str(LIB_DIR / _d))
sys.path.insert(0, str(REPO / "client" / "tools"))   # gen_lower_body_gait 属客户端动画工具

import gen_iron_armor as iron
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, build_bbmodel, validate_part, write_material_assets
from bbmodel_maker.render.render_bbmodel import render_mode_summary


class IronArmorGeneratorTest(unittest.TestCase):
    def test_four_part_functions_have_distinct_dense_silhouettes(self) -> None:
        parts = iron.parts()
        self.assertEqual(
            ["iron_helmet", "iron_chestplate", "iron_leggings", "iron_boots"],
            [part.key for part in parts],
        )
        self.assertEqual([15, 23, 18, 14], [len(part.cubes) for part in parts])
        for part in parts:
            validate_part(part)

    def test_texture_is_deterministic_64_square_and_not_flat(self) -> None:
        first = iron.make_texture()
        second = iron.make_texture()
        self.assertEqual((64, 64), first.size)
        self.assertEqual(first.tobytes(), second.tobytes())
        self.assertGreater(len(set(first.getdata())), 100, "锤纹/锈蚀贴图不应退化为纯色")

    def test_bbmodel_preserves_every_cube_and_mount_group(self) -> None:
        part = iron.part_leggings()
        model = build_bbmodel("iron", part, iron.make_texture())
        self.assertEqual(len(part.cubes), len(model["elements"]))
        self.assertEqual({"left_leg", "right_leg"}, {group["name"] for group in model["outliner"]})
        self.assertTrue(str(model["textures"][0]["source"]).startswith("data:image/png;base64,"))

    def test_bottom_uv_uses_cube_x_width_and_z_depth(self) -> None:
        cube = Cube("HEAD", "non_cubic", (0.0, 24.0, 0.0), (5.0, 2.0, 1.0), (3, 4))
        model = build_bbmodel(
            "iron",
            ArmorPart("uv_probe", "UV PROBE", (cube,)),
            iron.make_texture(),
        )
        u1, v1, u2, v2 = model["elements"][0]["faces"]["down"]["uv"]
        self.assertEqual(5.0, u2 - u1, "down 面 UV 宽度必须使用 cube sx，否则非立方体会横向压缩")
        self.assertEqual(1.0, v2 - v1, "down 面 UV 纵向范围必须保持 cube sz")

    def test_render_mode_summary_matches_the_angles_actually_rendered(self) -> None:
        self.assertEqual(
            "yaw=-35.0 pitch=22.0",
            render_mode_summary(False, -35.0, 22.0),
            "单视图日志必须保留 CLI yaw/pitch",
        )
        # SIDE → SIDE_R 是 framing 引入的**诚实化**改名，不是笔误：yaw=90 照的是 −x 面，
        # 也就是 FRONT(yaw=180) 视里观者右手边那一侧。叫「SIDE」等于把左右两侧混成一个
        # 名字，manifest 点名器再按 must_show_in=["SIDE_R"] 去核对就没有着落。角度一个
        # 没动 —— 这条断言的本职（防偷偷改取景）原样保留。
        self.assertEqual(
            "three-view [FRONT yaw=180.0 pitch=0.0; SIDE_R yaw=90.0 pitch=0.0; "
            "3/4 yaw=145.0 pitch=15.0]",
            render_mode_summary(True, -35.0, 22.0),
            "三视图日志必须报告固定实际角度，不得误报未使用的 CLI 参数",
        )

    def test_invalid_duplicate_and_nonpositive_cubes_fail_loud(self) -> None:
        valid = iron.part_helmet()
        duplicate = replace(valid, cubes=valid.cubes + (valid.cubes[0],))
        with self.assertRaisesRegex(ValueError, "duplicate cube name"):
            validate_part(duplicate)

        bad_cube = replace(valid.cubes[0], size=(0.0, 1.0, 1.0))
        invalid = ArmorPart("bad", "bad", (bad_cube,))
        with self.assertRaisesRegex(ValueError, "size must be positive"):
            validate_part(invalid)

    def test_writer_emits_four_models_four_runtime_textures_and_five_previews(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            outputs = write_material_assets(
                "iron",
                iron.parts(),
                iron.make_texture(),
                root / "models",
                root / "textures",
                root / "previews",
                render_previews=True,
            )
            self.assertEqual(13, len(outputs), "4 model + 4 texture + 4 three-view + 1 combined 应全部产出")
            self.assertEqual(4, len(list((root / "models/armor/iron").glob("*.bbmodel"))))
            textures = list((root / "textures").glob("iron_*/0.png"))
            self.assertEqual(4, len(textures))
            for path in textures:
                with Image.open(path) as texture:
                    self.assertEqual((64, 64), texture.size)

            previews = [outputs[f"preview:{part.key}"] for part in iron.parts()]
            previews.append(outputs["preview:all"])
            self.assertEqual(5, len(previews), "铁甲必须有四件三视图与一张总览图")
            for path in previews:
                self.assertTrue(path.is_file(), f"铁甲预览输出缺失: {path}")
                with Image.open(path) as preview:
                    preview.load()
                    self.assertGreater(preview.width, 0, f"铁甲预览宽度不得为 0: {path}")
                    self.assertGreater(preview.height, 0, f"铁甲预览高度不得为 0: {path}")

    def test_writer_rejects_duplicate_part_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            part = iron.part_helmet()
            with self.assertRaisesRegex(ValueError, "duplicate armor part key"):
                write_material_assets(
                    "iron",
                    (part, part),
                    iron.make_texture(),
                    root / "models",
                    root / "textures",
                    root / "previews",
                    render_previews=False,
                )


if __name__ == "__main__":
    unittest.main()
