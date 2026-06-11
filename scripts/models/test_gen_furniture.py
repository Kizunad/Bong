from __future__ import annotations

import base64
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gen_furniture as furniture


class PackerTest(unittest.TestCase):
    def test_place_wraps_rows_and_resets_after_vertical_overflow(self) -> None:
        packer = furniture.Packer(0.0, 0.0, 10.0, 10.0)

        self.assertEqual((0.0, 0.0), packer.place(6.0, 3.0))
        self.assertEqual((0.0, 3.0), packer.place(6.0, 3.0))
        self.assertEqual((0.0, 0.0), packer.place(10.0, 8.0))

    def test_place_clamps_oversized_face_to_zone_width(self) -> None:
        packer = furniture.Packer(2.0, 3.0, 8.0, 9.0)

        self.assertEqual((2.0, 3.0), packer.place(99.0, 2.0))
        self.assertEqual(8.0, packer.current_x)


class FurnitureBuildTest(unittest.TestCase):
    def test_build_bbmodel_emits_six_faces_with_uvs_for_each_cube(self) -> None:
        model = furniture.build_bbmodel(furniture.SPECS[0])
        elements = model["elements"]

        self.assertGreaterEqual(len(elements), 4)
        for element in elements:
            faces = element["faces"]
            self.assertEqual({"north", "south", "east", "west", "up", "down"}, set(faces))
            for face in faces.values():
                self.assertEqual(4, len(face["uv"]))
                self.assertEqual(0, face["texture"])
                for value in face["uv"]:
                    self.assertGreaterEqual(value, 0.0)
                    self.assertLessEqual(value, furniture.TEXTURE_RES)

    def test_make_texture_and_data_url_are_png_rgba(self) -> None:
        image = furniture.make_texture(furniture.SPECS[0])
        data_url = furniture.png_data_url(image)

        self.assertEqual("RGBA", image.mode)
        self.assertEqual((furniture.TEXTURE_RES, furniture.TEXTURE_RES), image.size)
        self.assertTrue(data_url.startswith("data:image/png;base64,"))
        decoded = base64.b64decode(data_url.removeprefix("data:image/png;base64,"))
        self.assertEqual(b"\x89PNG\r\n\x1a\n", decoded[:8])

    def test_build_block_model_uses_explicit_elements_without_parent(self) -> None:
        model = furniture.build_block_model(furniture.SPECS[1])

        self.assertNotIn("parent", model)
        self.assertGreaterEqual(len(model["elements"]), 4)
        self.assertEqual("bong:block/meditation_mat", model["textures"]["all"])


class VerifySpecTest(unittest.TestCase):
    def spec_with(self, cubes: list[furniture.Cube], *, bones: tuple[str, ...] = ("base",)) -> furniture.FurnitureSpec:
        return furniture.FurnitureSpec(
            "broken_furniture",
            "BrokenFurniture",
            "坏家具",
            bones,
            {"wood": (100, 80, 60)},
            lambda: cubes,
        )

    def test_verify_all_specs_accepts_tracked_furniture(self) -> None:
        furniture.verify_all_specs()

    def test_verify_rejects_duplicate_cube_names(self) -> None:
        cubes = [
            furniture.cube("base", "wood", "duplicate", (0.0, 0.0, 0.0), (2.0, 2.0, 2.0)),
            furniture.cube("base", "wood", "duplicate", (2.0, 0.0, 0.0), (4.0, 2.0, 2.0)),
            furniture.cube("base", "wood", "third", (4.0, 0.0, 0.0), (6.0, 2.0, 2.0)),
            furniture.cube("base", "wood", "fourth", (6.0, 0.0, 0.0), (8.0, 2.0, 2.0)),
        ]

        with self.assertRaisesRegex(ValueError, "cube 名重复"):
            furniture.verify_spec(self.spec_with(cubes))

    def test_verify_rejects_unknown_bone_and_material(self) -> None:
        unknown_bone = [furniture.cube("ghost", "wood", f"cube_{index}", (0.0, index, 0.0), (1.0, index + 0.5, 1.0)) for index in range(4)]
        unknown_material = [furniture.cube("base", "ghost", f"cube_{index}", (0.0, index, 0.0), (1.0, index + 0.5, 1.0)) for index in range(4)]

        with self.assertRaisesRegex(ValueError, "未知 bone"):
            furniture.verify_spec(self.spec_with(unknown_bone))
        with self.assertRaisesRegex(ValueError, "未知 material"):
            furniture.verify_spec(self.spec_with(unknown_material))

    def test_verify_rejects_degenerate_or_out_of_range_coords(self) -> None:
        cubes = [
            furniture.cube("base", "wood", "bad", (0.0, 0.0, 0.0), (0.0, 1.0, 1.0)),
            furniture.cube("base", "wood", "two", (1.0, 0.0, 0.0), (2.0, 1.0, 1.0)),
            furniture.cube("base", "wood", "three", (2.0, 0.0, 0.0), (3.0, 1.0, 1.0)),
            furniture.cube("base", "wood", "four", (3.0, 0.0, 0.0), (4.0, 1.0, 1.0)),
        ]

        with self.assertRaisesRegex(ValueError, "坐标越界或退化"):
            furniture.verify_spec(self.spec_with(cubes))

    def test_verify_rejects_unused_bone(self) -> None:
        cubes = [furniture.cube("base", "wood", f"cube_{index}", (0.0, index, 0.0), (1.0, index + 0.5, 1.0)) for index in range(4)]

        with self.assertRaisesRegex(ValueError, "未使用 bone"):
            furniture.verify_spec(self.spec_with(cubes, bones=("base", "unused")))


class WriteAndCliTest(unittest.TestCase):
    def with_temp_outputs(self):
        temp_dir = tempfile.TemporaryDirectory()
        root = Path(temp_dir.name)
        old_values = (
            furniture.REPO,
            furniture.LOCAL_MODELS,
            furniture.PREVIEW_DIR,
            furniture.BLOCK_MODEL_DIR,
        )
        furniture.REPO = root
        furniture.LOCAL_MODELS = root / "local_models"
        furniture.PREVIEW_DIR = root / "previews"
        furniture.BLOCK_MODEL_DIR = root / "block_models"
        return temp_dir, old_values

    def restore_outputs(self, temp_dir, old_values) -> None:
        furniture.REPO, furniture.LOCAL_MODELS, furniture.PREVIEW_DIR, furniture.BLOCK_MODEL_DIR = old_values
        temp_dir.cleanup()

    def test_write_spec_validates_then_writes_all_outputs(self) -> None:
        temp_dir, old_values = self.with_temp_outputs()
        try:
            spec = furniture.SPECS[0]
            furniture.write_spec(spec)

            self.assertTrue((furniture.LOCAL_MODELS / "SimpleBed.bbmodel").exists())
            self.assertTrue((furniture.BLOCK_MODEL_DIR / "simple_bed.json").exists())
            self.assertTrue((furniture.PREVIEW_DIR / "simple_bed_preview.png").exists())
        finally:
            self.restore_outputs(temp_dir, old_values)

    def test_cli_verify_does_not_write_outputs(self) -> None:
        temp_dir, old_values = self.with_temp_outputs()
        old_argv = sys.argv
        try:
            sys.argv = ["gen_furniture.py", "--verify"]
            furniture.main()

            self.assertFalse(furniture.LOCAL_MODELS.exists())
            self.assertFalse(furniture.BLOCK_MODEL_DIR.exists())
            self.assertFalse(furniture.PREVIEW_DIR.exists())
        finally:
            sys.argv = old_argv
            self.restore_outputs(temp_dir, old_values)

    def test_cli_only_writes_selected_spec(self) -> None:
        temp_dir, old_values = self.with_temp_outputs()
        old_argv = sys.argv
        try:
            sys.argv = ["gen_furniture.py", "--only", "simple_bed"]
            furniture.main()

            self.assertTrue((furniture.LOCAL_MODELS / "SimpleBed.bbmodel").exists())
            self.assertFalse((furniture.LOCAL_MODELS / "MeditationMat.bbmodel").exists())
        finally:
            sys.argv = old_argv
            self.restore_outputs(temp_dir, old_values)


if __name__ == "__main__":
    unittest.main()
