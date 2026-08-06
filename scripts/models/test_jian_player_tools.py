#!/usr/bin/env python3

from __future__ import annotations

import json
import math
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import numpy as np

MODEL_DIR = Path(__file__).resolve().parent
REPO = MODEL_DIR.parents[2]
sys.path.insert(0, str(MODEL_DIR))

import gen_jian_player as J
import render_jian_in_hand as H
import render_player_pose as P


class JianPlayerToolsTest(unittest.TestCase):
    def test_weapon_placement_uses_grip_anchor_arm_then_wrist_and_rotates_normals(self) -> None:
        pose = {
            "right": {
                "arm": (17.0, -23.0, 11.0),
                "wrist": (-31.0, 13.0, 7.0),
            }
        }
        vertices = np.array([
            H.GRIP_ANCHOR,
            H.GRIP_ANCHOR + np.array([1.0, 0.0, 0.0]),
            H.GRIP_ANCHOR + np.array([0.0, 1.0, 0.0]),
        ])
        normal = np.array([0.0, 0.0, 1.0])
        placed = H.place([(vertices, np.zeros((3, 2)), normal)], pose, "right", scale=2.0)[0]

        arm, hand = H.arm_transform(pose, "right")
        wrist = H.euler_mat(pose["right"]["wrist"])
        expected = np.array([
            arm @ (wrist @ ((vertex - H.GRIP_ANCHOR) * 2.0) + H.HAND_REST["right"] - H.SHOULDER["right"])
            + H.SHOULDER["right"]
            for vertex in vertices
        ])
        np.testing.assert_allclose(placed[0], expected, atol=1e-9)
        np.testing.assert_allclose(placed[2], arm @ wrist @ normal, atol=1e-9)
        np.testing.assert_allclose(placed[0][0], hand, atol=1e-9,
                                   err_msg="握把锚点必须落在旋转后的手心，缩放不能移动手心")

    def test_player_pose_matrix_point_and_geometry_share_transform_contract(self) -> None:
        np.testing.assert_allclose(P.part_matrix(), np.eye(3), atol=1e-9)
        point, matrix = P.part_point("torso", {"bend": 90.0}, (0.0, 12.0, 0.0))
        np.testing.assert_allclose(point, [0.0, 18.0, 6.0], atol=1e-9)
        np.testing.assert_allclose(matrix, np.eye(3), atol=1e-9)
        self.assertGreater(len(P.part_tris("torso", {"bend": 30.0})), 0)
        self.assertGreater(len(P.part_tris("rightArm", {"pitch": -20.0, "bend": 45.0})), 0)

    def test_player_pose_body_extraction_and_empty_or_malformed_animation_inputs(self) -> None:
        body_doc = {
            "version": 3,
            "name": "body_probe",
            "emote": {
                "degrees": False,
                "endTick": 1,
                "moves": [{
                    "tick": 0,
                    "body": {"x": 0.25, "y": -0.5, "z": 0.125, "pitch": 0.1},
                }],
            },
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "body_probe.json"
            path.write_text(json.dumps(body_doc), encoding="utf-8")
            name, _emote, table = P.anim_pose_table(path)
            self.assertEqual("body_probe", name)
            body = table[0][1]["_body"]
            self.assertEqual(0.25, body["x"])
            self.assertEqual(-0.5, body["y"])
            self.assertEqual(0.125, body["z"])
            self.assertAlmostEqual(math.degrees(0.1), body["pitch"])

            degrees_path = Path(tmp) / "degrees.json"
            degrees_path.write_text(json.dumps({
                "name": "degrees",
                "emote": {
                    "degrees": True,
                    "moves": [{"tick": 0, "body": {"pitch": 45.0}}],
                },
            }), encoding="utf-8")
            degrees_body = P.anim_pose_table(degrees_path)[2][0][1]["_body"]
            self.assertEqual(45.0, degrees_body["pitch"])

            for invalid in (None, 0, 1, "false", []):
                invalid_path = Path(tmp) / f"invalid_{repr(invalid)}.json"
                invalid_path.write_text(json.dumps({
                    "emote": {
                        "degrees": invalid,
                        "moves": [{"tick": 0, "body": {"pitch": 0.1}}],
                    },
                }), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, r"emote\.degrees.*false means radians.*true means degrees"):
                    P.anim_pose_table(invalid_path)

            empty = Path(tmp) / "empty.json"
            empty.write_text(json.dumps({"name": "empty", "emote": {"degrees": False, "moves": []}}), encoding="utf-8")
            self.assertEqual([], P.anim_pose_table(empty)[2])

            malformed = Path(tmp) / "malformed.json"
            malformed.write_text("{}", encoding="utf-8")
            with self.assertRaises(KeyError):
                P.anim_pose_table(malformed)

    def test_up_alignment_is_a_finite_proper_rotation_for_all_direction_regimes(self) -> None:
        up = np.array([0.0, 1.0, 0.0])
        for direction in (
            np.array([0.0, -1.0, 0.0]),
            np.array([1e-10, -1.0, 1e-10]),
            np.array([1.0, 2.0, -3.0]),
        ):
            matrix = P._align_up_to_direction(direction)
            target = direction / np.linalg.norm(direction)
            np.testing.assert_allclose(matrix @ up, target, atol=1e-9)
            np.testing.assert_allclose(matrix.T @ matrix, np.eye(3), atol=1e-9)
            self.assertAlmostEqual(1.0, np.linalg.det(matrix), places=9)
            self.assertTrue(np.isfinite(matrix).all())

        with self.assertRaisesRegex(ValueError, "nonzero"):
            P._align_up_to_direction(np.zeros(3))

    def test_render_size_validation_covers_boundary_and_error_values(self) -> None:
        self.assertEqual(1, P._validate_size(1))
        self.assertEqual(P.MAX_RENDER_SIZE, P._validate_size(P.MAX_RENDER_SIZE))
        for size in (0, -1, P.MAX_RENDER_SIZE + 1, True, 1.5):
            with self.assertRaisesRegex(ValueError, r"1 <= size <= 4096"):
                P._validate_size(size)
        with self.assertRaisesRegex(ValueError, r"render_pose size.*1 <= size <= 4096"):
            P.render_pose([], np.zeros((1, 1, 3)), size=0)
        with self.assertRaisesRegex(ValueError, r"render_anim size.*1 <= size <= 4096"):
            P.render_anim(Path("unused.json"), size=0)

    def test_render_player_pose_cli_rejects_invalid_size_and_accepts_maximum(self) -> None:
        script = MODEL_DIR / "render_player_pose.py"
        invalid = subprocess.run(
            [sys.executable, str(script), "--size", "0"],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(2, invalid.returncode)
        self.assertIn("1 <= size <= 4096", invalid.stderr)

        negative = subprocess.run(
            [sys.executable, str(script), "--size", "-1"],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(2, negative.returncode)
        self.assertIn("1 <= size <= 4096", negative.stderr)

        maximum = subprocess.run(
            [sys.executable, str(script), "--size", "4096", "--help"],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(0, maximum.returncode, maximum.stdout + maximum.stderr)
        self.assertNotIn("size must", maximum.stderr)

        over_limit = subprocess.run(
            [sys.executable, str(script), "--size", "4097"],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(2, over_limit.returncode)
        self.assertIn("1 <= size <= 4096", over_limit.stderr)

    def test_default_jian_model_fallback_is_reachable_without_writing_source_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "missing" / "BambooJianSingle.bbmodel"
            with patch.object(H, "DEFAULT_MODEL", missing), patch.object(J, "SRC_JIAN", missing):
                model, _label = J.build("stand")
            self.assertGreater(len(model["elements"]), len(H.PLAYER_CUBES))
            self.assertEqual("jian_player", model["name"])
            self.assertFalse(missing.exists(), "默认 fallback 应在内存中生成，不应写回源模型")

    def test_render_size_bounds_cover_zero_negative_max_and_over(self) -> None:
        for module in (J, H):
            for size in (0, -1, module.MAX_RENDER_SIZE + 1):
                with self.subTest(module=module.__name__, size=size):
                    with self.assertRaisesRegex(ValueError, "between 1 and 4096"):
                        module.validate_render_size(size)
            self.assertEqual(module.MAX_RENDER_SIZE, module.validate_render_size(module.MAX_RENDER_SIZE))
            self.assertEqual(1, module.validate_render_size(1))

    def test_gen_jian_player_size_rejects_before_writing_bbmodel(self) -> None:
        script = MODEL_DIR / "gen_jian_player.py"
        for size in (0, -1, J.MAX_RENDER_SIZE + 1):
            with self.subTest(size=size), tempfile.TemporaryDirectory() as tmp:
                output = Path(tmp) / "JianPlayer.bbmodel"
                result = subprocess.run(
                    [sys.executable, str(script), "--size", str(size), "--out", str(output)],
                    cwd=REPO,
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertNotEqual(0, result.returncode)
                self.assertFalse(output.exists(), "非法尺寸必须在生成 bbmodel 前失败")
                self.assertIn("between 1 and 4096", result.stderr)

    def test_render_jian_in_hand_size_rejects_before_loading_model(self) -> None:
        script = MODEL_DIR / "render_jian_in_hand.py"
        missing_model = Path("does-not-exist.bbmodel")
        for size in (0, -1, H.MAX_RENDER_SIZE + 1):
            with self.subTest(size=size):
                result = subprocess.run(
                    [sys.executable, str(script), "--size", str(size), "--model", str(missing_model)],
                    cwd=REPO,
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertNotEqual(0, result.returncode)
                self.assertIn("between 1 and 4096", result.stderr)
                self.assertNotIn("FileNotFoundError", result.stderr)

    def test_render_jian_in_hand_rejects_over_budget_composite(self) -> None:
        with self.assertRaisesRegex(ValueError, "canvas area"):
            H.composite_canvas_dimensions(H.MAX_RENDER_SIZE, 4)
        width, height = H.composite_canvas_dimensions(1, 1)
        self.assertGreater(width * height, 0)

    def test_gen_jian_player_creates_nested_output_directory(self) -> None:
        script = MODEL_DIR / "gen_jian_player.py"
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "new" / "nested" / "JianPlayer.bbmodel"
            result = subprocess.run(
                [sys.executable, str(script), "--out", str(output), "--no-render"],
                cwd=REPO,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            self.assertTrue(output.is_file())

    def test_gen_jian_player_refuses_external_hand_edited_output_without_traceback(self) -> None:
        script = MODEL_DIR / "gen_jian_player.py"
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "hand-edited" / "JianPlayer.bbmodel"
            output.parent.mkdir(parents=True)
            output.write_text(json.dumps({"meta": {"format_version": "5.0"}, "groups": []}), encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(script), "--out", str(output), "--no-render"],
                cwd=REPO,
                text=True,
                capture_output=True,
                check=False,
            )
            combined = result.stdout + result.stderr
            self.assertNotEqual(0, result.returncode)
            self.assertIn(str(output), combined)
            self.assertNotIn("Traceback", combined)


if __name__ == "__main__":
    unittest.main()
