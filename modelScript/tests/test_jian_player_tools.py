#!/usr/bin/env python3

from __future__ import annotations

import base64
import io
import json
import math
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import numpy as np
from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in ("generators", "exporters", "tools"):
    sys.path.insert(0, str(LIB_DIR / _d))
sys.path.insert(0, str(REPO / "client" / "tools"))   # gen_lower_body_gait 属客户端动画工具

import gen_bamboo_jian as B
import gen_jian_player as J
import gen_jian_player_anim as A
import gen_lower_body_gait as G
from bbmodel_maker.render import held_item_render as H
from bbmodel_maker.render import render_player_pose as P



def _module_script(dotted: str) -> Path:
    """已迁进 bbmodel-maker 的 CLI，取它在安装位置的真实文件路径。

    这些入口原先是 `modelScript/core/*.py` / `modelScript/tools/*.py`，现在住在库里。
    测试仍按「起子进程跑脚本」验参数校验，所以要的是文件路径而不是模块名。
    """
    import importlib.util

    spec = importlib.util.find_spec(dotted)
    assert spec is not None and spec.origin, f"找不到已安装的模块 {dotted}"
    return Path(spec.origin)


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
            with self.assertRaisesRegex(ValueError, "must contain at least one keyframe"):
                P.render_anim(empty, size=1)

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
            with self.assertRaisesRegex(ValueError, rf"1 <= size <= {P.MAX_RENDER_SIZE}"):
                P._validate_size(size)
        with self.assertRaisesRegex(ValueError, rf"render_pose size.*1 <= size <= {P.MAX_RENDER_SIZE}"):
            P.render_pose([], np.zeros((1, 1, 3)), size=0)
        with self.assertRaisesRegex(ValueError, rf"render_anim size.*1 <= size <= {P.MAX_RENDER_SIZE}"):
            P.render_anim(Path("unused.json"), size=0)

    def test_render_player_pose_cli_rejects_invalid_size_and_accepts_maximum(self) -> None:
        script = _module_script("bbmodel_maker.render.render_player_pose")
        invalid = subprocess.run(
            [sys.executable, str(script), "--size", "0"],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(2, invalid.returncode)
        self.assertIn(f"1 <= size <= {P.MAX_RENDER_SIZE}", invalid.stderr)

        negative = subprocess.run(
            [sys.executable, str(script), "--size", "-1"],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(2, negative.returncode)
        self.assertIn(f"1 <= size <= {P.MAX_RENDER_SIZE}", negative.stderr)

        maximum = subprocess.run(
            [sys.executable, str(script), "--pose", "stand", "--size", str(P.MAX_RENDER_SIZE)],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(0, maximum.returncode, maximum.stdout + maximum.stderr)

        over_limit = subprocess.run(
            [sys.executable, str(script), "--size", str(P.MAX_RENDER_SIZE + 1)],
            cwd=REPO,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(2, over_limit.returncode)
        self.assertIn(f"1 <= size <= {P.MAX_RENDER_SIZE}", over_limit.stderr)

    def test_render_player_pose_with_jian_reaches_each_render_mode(self) -> None:
        with patch.object(P, "skin_atlas", return_value=np.zeros((1, 1, 3))) as atlas:
            with patch.object(P, "jian_tris", return_value=[]) as jian:
                with patch.object(P, "render_pose", return_value=object()):
                    with patch.object(P, "grid", return_value=Path("preview.png")):
                        with tempfile.TemporaryDirectory() as tmp:
                            animation = Path(tmp) / "one.json"
                            animation.write_text(json.dumps({
                                "name": "one",
                                "emote": {"degrees": True, "endTick": 1, "moves": [
                                    {"tick": 0, "body": {"pitch": 0.0}},
                                ]},
                            }), encoding="utf-8")
                            P.render_anim(animation, size=1, with_jian=True)
                        P.bend_matrix(size=1, with_jian=True)
                        atlas.assert_called_with(True)
                        self.assertGreaterEqual(jian.call_count, 19)

                        jian.reset_mock()
                        with patch.object(P, "grid", return_value=P.OUT_POSE):
                            with patch.object(sys, "argv", ["render_player_pose", "--pose", "stand", "--with-jian", "--size", "1"]):
                                P.main()
                        atlas.assert_called_with(True)
                        self.assertEqual(1, jian.call_count)
                        jian.assert_called_once_with(P.POSES["stand"], H.WEAPON_V_OFF)

    def test_body_animation_rotates_around_model_root(self) -> None:
        pose = {
            "_body": {"pitch": 90.0},
            "head": {},
        }
        transformed = P.part_tris("head", pose["head"])
        root = P.BODY_ROOT
        body = pose["_body"]
        matrix = P.part_matrix(body["pitch"], body.get("yaw", 0.0), body.get("roll", 0.0))
        rotated = np.array([matrix @ (v - root) + root for v in transformed[0][0]])
        expected = np.array([
            root + np.array([v[0] - root[0], v[2] - root[2], -(v[1] - root[1])])
            for v in transformed[0][0]
        ])
        np.testing.assert_allclose(rotated, expected, atol=1e-9)

    def test_load_grouped_resolves_fmt5_group_children_and_leaf_to_root_order(self) -> None:
        source = Image.new("RGBA", (2, 2), (90, 100, 110, 255))
        buffer = io.BytesIO()
        source.save(buffer, format="PNG")
        root_uuid = "root-group"
        child_uuid = "child-group"
        element_uuid = "cube-element"
        document = {
            "resolution": {"width": 2, "height": 2},
            "textures": [{"source": "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode()}],
            "elements": [{
                "uuid": element_uuid,
                "from": [0, 0, 0], "to": [1, 1, 1],
                "faces": {
                    "north": {"uv": [0, 0, 1, 1]},
                    "up": {"uv": [0, 0, 1, 1]},
                },
            }],
            "outliner": [{"uuid": root_uuid}],
            "groups": [
                {"uuid": root_uuid, "origin": [0, 0, 0], "rotation": [0, 0, 90], "children": [child_uuid]},
                {"uuid": child_uuid, "origin": [0, 0, 0], "rotation": [0, 90, 0], "children": [element_uuid]},
            ],
        }
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fmt5.bbmodel"
            path.write_text(json.dumps(document), encoding="utf-8")
            tris, _texture, _resolution, _name = J.load_grouped(path)
        vertices = np.concatenate([vs for vs, _uvs, _normal in tris])
        self.assertTrue(
            any(np.allclose(vertex, [-1.0, 0.0, -1.0]) for vertex in vertices),
            "UUID-resolved child and parent rotations must apply in leaf-to-root order",
        )

    def test_default_jian_model_fallback_is_reachable_without_writing_source_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "missing" / "BambooJianSingle.bbmodel"
            with patch.object(H, "DEFAULT_MODEL", missing), patch.object(J, "SRC_JIAN", missing):
                model, _label = J.build("stand")
            self.assertGreater(len(model["elements"]), len(H.PLAYER_CUBES))
            self.assertEqual("jian_player", model["name"])
            self.assertFalse(missing.exists(), "默认 fallback 应在内存中生成，不应写回源模型")

    def test_default_bamboo_jian_build_is_a_pair_with_distinct_sides(self) -> None:
        model, cubes, _texture = B.build_bbmodel()
        self.assertEqual(2, len(model["outliner"]))
        self.assertEqual(2, len({cube[2].rsplit("_", 1)[1] for cube in cubes}))
        self.assertEqual(len(B.build_cubes()) * 2, len(cubes))
        self.assertTrue(all(name.endswith(("_r", "_l")) for _bone, _material, name, *_rest in cubes))

    def test_jian_player_animation_converter_pins_axes_body_and_lower_filler(self) -> None:
        elements, _outliner, gmap, _atlas = A.build_geometry()
        self.assertGreater(len(elements), 0)
        # 这三个符号是**写侧**（生成器写给 Blockbench 看的那一侧，要预取反 X/Y 去抵消
        # 它读入时的取反）。2026-08-26 一度按"读写同一套"把它们翻掉，结果生成的资产在
        # Blockbench 里四肢镜像、身体朝后；依据与读侧口径见 `core/bb_anim_axes` 的
        # docstring，往返锁在 `test_bb_anim_roundtrip.py`。
        self.assertEqual(3, len(A.AXIS_LAYERS))
        self.assertEqual(("pitch", 0, 1.0), A.AXIS_LAYERS[0])
        self.assertEqual(("yaw", 1, -1.0), A.AXIS_LAYERS[1])
        self.assertEqual(("roll", 2, -1.0), A.AXIS_LAYERS[2])
        self.assertIs(A.AXIS_LAYERS, A.AX.WRITE_LAYERS, "必须用公共层的写侧，别再抄")
        animation = A.convert_animation(A.ANIM_DIR / "lower_walk.json", gmap)
        self.assertEqual("loop", animation["loop"])
        self.assertAlmostEqual(1.0, animation["length"])
        tracks = {track["name"]: track for track in animation["animators"].values()}
        self.assertIn("root_pos", tracks)
        self.assertIn("root_pitch", tracks)
        self.assertIn("arm_right_pitch", tracks)
        self.assertIn("arm_left_pitch", tracks)
        self.assertEqual(5, len(tracks["root_pos"]["keyframes"]))
        self.assertEqual(5, len(tracks["arm_right_pitch"]["keyframes"]))

    def test_lower_body_generators_match_committed_assets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output_dir = Path(tmp)
            original_resolve = G.AC.resolve_output_path
            G.AC.resolve_output_path = lambda name: output_dir / f"{name}.json"
            try:
                G.main()
            finally:
                G.AC.resolve_output_path = original_resolve
            for name in G.GAITS:
                generated = json.loads((output_dir / f"{name}.json").read_text())
                committed = json.loads((REPO / "client/src/main/resources/assets/bong/player_animation" / f"{name}.json").read_text())
                self.assertEqual(generated["emote"], committed["emote"], name)
            generated_dash = json.loads((output_dir / "lower_dash.json").read_text())
            committed_dash = json.loads((REPO / "client/src/main/resources/assets/bong/player_animation/lower_dash.json").read_text())
            self.assertEqual(generated_dash["emote"], committed_dash["emote"])

    def test_render_size_bounds_cover_zero_negative_max_and_over(self) -> None:
        for module in (J, H):
            for size in (0, -1, module.MAX_RENDER_SIZE + 1):
                with self.subTest(module=module.__name__, size=size):
                    with self.assertRaisesRegex(ValueError, rf"between 1 and {module.MAX_RENDER_SIZE}"):
                        module.validate_render_size(size)
            self.assertEqual(module.MAX_RENDER_SIZE, module.validate_render_size(module.MAX_RENDER_SIZE))
            self.assertEqual(1, module.validate_render_size(1))

            for size in (0, -1, module.MAX_RENDER_SIZE + 1):
                with self.subTest(module=module.__name__, size=size):
                    with self.assertRaisesRegex(ValueError, rf"between 1 and {module.MAX_RENDER_SIZE}"):
                        module.validate_render_size(size)
            self.assertEqual(module.MAX_RENDER_SIZE, module.validate_render_size(module.MAX_RENDER_SIZE))
            self.assertEqual(1, module.validate_render_size(1))
    def test_gen_jian_player_size_rejects_before_writing_bbmodel(self) -> None:
        script = LIB_DIR / "generators" / "gen_jian_player.py"
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
                self.assertIn(f"between 1 and {J.MAX_RENDER_SIZE}", result.stderr)

    def test_render_jian_in_hand_size_rejects_before_loading_model(self) -> None:
        script = _module_script("bbmodel_maker.render.held_item_render")
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
                self.assertIn(f"between 1 and {H.MAX_RENDER_SIZE}", result.stderr)
                self.assertNotIn("FileNotFoundError", result.stderr)

    def test_render_jian_in_hand_rejects_over_budget_composite(self) -> None:
        with self.assertRaisesRegex(ValueError, "render working set"):
            H.composite_canvas_dimensions(H.MAX_RENDER_SIZE, 16)
        width, height = H.composite_canvas_dimensions(1, 1)
        self.assertGreater(width * height, 0)

    def test_render_jian_in_hand_validates_scale_inputs_at_cli_boundary(self) -> None:
        self.assertEqual([1.0, 0.75, 2.0], H.validate_scales(" 1.0, 0.75,2 "))
        for raw in ("", ",,,", "1,broken", "nan", "inf", "-inf", "0", "-0.5"):
            with self.subTest(raw=raw):
                with self.assertRaisesRegex(ValueError, "scales"):
                    H.validate_scales(raw)

    def test_weapon_uvs_scale_from_non_square_source_texture_to_skin_atlas(self) -> None:
        source = Image.new("RGBA", (128, 32), (80, 90, 100, 255))
        buffer = io.BytesIO()
        source.save(buffer, format="PNG")
        model = {
            "textures": [{
                "source": "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode(),
            }],
            "elements": [{
                "from": [0, 0, 0], "to": [2, 4, 2],
                "faces": {"up": {"uv": [32, 8, 96, 24]}},
            }],
        }
        with patch.object(H, "load_model_document", return_value=model):
            tris, _texture = H.weapon_tris(Path("synthetic.bbmodel"))
        uv = tris[0][1]
        np.testing.assert_allclose(uv[:, 0], [16.0, 48.0, 48.0])
        np.testing.assert_allclose(uv[:, 1], [80.0, 80.0, 112.0])

    def test_gen_jian_player_overwrite_guard_rejects_each_hand_edit_signal(self) -> None:
        script = LIB_DIR / "generators" / "gen_jian_player.py"
        for document in (
            {"meta": {"format_version": "5.0"}},
            {"meta": {"format_version": "4.10"}, "groups": []},
        ):
            with self.subTest(document=document), tempfile.TemporaryDirectory() as tmp:
                output = Path(tmp) / "JianPlayer.bbmodel"
                output.write_text(json.dumps(document), encoding="utf-8")
                result = subprocess.run(
                    [sys.executable, str(script), "--out", str(output), "--no-render"],
                    cwd=REPO,
                    text=True,
                    capture_output=True,
                    check=False,
                )
                self.assertNotEqual(0, result.returncode)
                self.assertNotIn("Traceback", result.stdout + result.stderr)

    def test_gen_jian_player_rejects_output_changed_during_build(self) -> None:
        original = J.build
        script = LIB_DIR / "generators" / "gen_jian_player.py"
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "JianPlayer.bbmodel"
            output.write_text(json.dumps({"meta": {"format_version": "4.10"}}), encoding="utf-8")

            def build_and_modify(pose_key):
                model, label = original(pose_key)
                output.write_text("external Blockbench save", encoding="utf-8")
                return model, label

            with patch.object(J, "build", side_effect=build_and_modify):
                with patch.object(sys, "argv", [str(script), "--out", str(output), "--no-render"]):
                    with self.assertRaisesRegex(SystemExit, "生成期间发生变化"):
                        J.main()

    def test_gen_jian_player_creates_nested_output_directory(self) -> None:
        script = LIB_DIR / "generators" / "gen_jian_player.py"
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
        script = LIB_DIR / "generators" / "gen_jian_player.py"
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
