#!/usr/bin/env python3
"""animcore —— animkit / anim_rig 合并出来的公共底座。

除了逐函数的行为测试，这里还有两组**防复发**断言：
  · 两个模块暴露的公共名必须**就是** animcore 里的同一个对象（不是复制回去的副本）；
  · 关键帧 uuid 的两种历史种子拼法必须原样保住 —— 统一它会让所有既有产物的 uuid 全变。
"""

from __future__ import annotations

import math
import sys
import unittest
import uuid as uuidlib
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]

from bbmodel_maker.rig import anim_rig  # noqa: E402
from bbmodel_maker.rig import animcore  # noqa: E402
from bbmodel_maker.rig import animkit  # noqa: E402


class RotationTest(unittest.TestCase):
    def test_rotmat_axes(self) -> None:
        for axis in range(3):
            self.assertTrue(np.allclose(np.eye(3), animcore.rotmat(0.0, axis)),
                            f"轴 {axis} 转 0° 必须是单位阵")
        self.assertTrue(np.allclose((0, 0, 1), animcore.rotmat(90, 0) @ (0, 1, 0)),
                        "绕 x 转 90°：+y → +z")
        self.assertTrue(np.allclose((1, 0, 0), animcore.rotmat(90, 1) @ (0, 0, 1)),
                        "绕 y 转 90°：+z → +x")
        self.assertTrue(np.allclose((0, 1, 0), animcore.rotmat(90, 2) @ (1, 0, 0)),
                        "绕 z 转 90°：+x → +y")

    def test_rotmat_is_orthonormal_and_right_handed(self) -> None:
        for axis in range(3):
            R = animcore.rotmat(37.0, axis)
            self.assertTrue(np.allclose(np.eye(3), R @ R.T, atol=1e-12))
            self.assertAlmostEqual(1.0, float(np.linalg.det(R)), places=12)

    def test_euler_is_rz_ry_rx_in_that_order(self) -> None:
        rot = (11.0, 22.0, 33.0)
        expect = (animcore.rotmat(33, 2) @ animcore.rotmat(22, 1) @ animcore.rotmat(11, 0))
        self.assertTrue(np.allclose(expect, animcore.euler(rot)))
        self.assertTrue(np.allclose(np.eye(3), animcore.euler((0, 0, 0))))

    def test_euler_xyz_round_trips(self) -> None:
        rng = np.random.default_rng(7)
        for _ in range(200):
            rot = rng.uniform(-89.0, 89.0, 3)      # 避开万向锁
            got = animcore.euler_xyz(animcore.euler(rot))
            self.assertTrue(np.allclose(rot, got, atol=1e-9),
                            f"期望解回 {rot}，实际 {got}")

    def test_euler_xyz_pins_z_to_zero_in_the_gimbal_band(self) -> None:
        """万向锁附近 x 与 z 本就简并，硬解出来的那一对角度会在相邻帧之间乱跳。"""
        for pitch in (90.0, -90.0):
            x, y, z = animcore.euler_xyz(animcore.euler((17.0, pitch, 41.0)))
            self.assertAlmostEqual(pitch, y, places=6)
            self.assertEqual(0.0, z, "退化分支必须把 z 钉成 0")

    def test_gimbal_threshold_is_the_documented_one(self) -> None:
        self.assertEqual(1e-6, animcore.GIMBAL_EPS,
                         "两份历史实现分别用 1e-6 / 1e-7 判同一个量，合并取 1e-6")

    def test_affine_packs_rotation_and_translation(self) -> None:
        M = animcore.affine(animcore.rotmat(90, 1), np.array([1.0, 2.0, 3.0]))
        self.assertEqual((4, 4), M.shape)
        self.assertTrue(np.allclose([0, 0, 0, 1], M[3]))
        self.assertTrue(np.allclose([1, 2, 3], M[:3, 3]))


class AlignSlerpTest(unittest.TestCase):
    def test_align_maps_u_onto_v(self) -> None:
        rng = np.random.default_rng(3)
        for _ in range(50):
            u, v = rng.normal(size=3), rng.normal(size=3)
            R = animcore.align(u, v)
            got = R @ (u / np.linalg.norm(u))
            self.assertTrue(np.allclose(got, v / np.linalg.norm(v), atol=1e-9))

    def test_align_is_identity_for_parallel_vectors(self) -> None:
        self.assertTrue(np.allclose(np.eye(3), animcore.align((0, 0, 2), (0, 0, 5))))

    def test_align_handles_the_antiparallel_singularity(self) -> None:
        R = animcore.align((0, 0, 1), (0, 0, -1))
        self.assertTrue(np.allclose((0, 0, -1), R @ (0, 0, 1), atol=1e-9),
                        "反向时得挑一个垂直轴转 180°，不能退化成 NaN")
        self.assertAlmostEqual(1.0, float(np.linalg.det(R)), places=9)

    def test_quaternion_round_trip(self) -> None:
        rng = np.random.default_rng(11)
        for _ in range(50):
            R = animcore.euler(rng.uniform(-170, 170, 3))
            self.assertTrue(np.allclose(R, animcore.from_quat(animcore.to_quat(R)), atol=1e-9))

    def test_quaternion_handles_negative_trace_branch(self) -> None:
        R = animcore.euler((180.0, 0.0, 0.0))
        self.assertLess(float(np.trace(R)), 0.0, "这个姿态确实落在负迹分支上")
        self.assertTrue(np.allclose(R, animcore.from_quat(animcore.to_quat(R)), atol=1e-9))

    def test_slerp_endpoints_and_shortest_arc(self) -> None:
        R0 = np.eye(3)
        R1 = animcore.rotmat(90.0, 1)
        self.assertTrue(np.allclose(R0, animcore.slerp(R0, R1, 0.0), atol=1e-9))
        self.assertTrue(np.allclose(R1, animcore.slerp(R0, R1, 1.0), atol=1e-9))
        self.assertTrue(np.allclose(animcore.rotmat(45.0, 1), animcore.slerp(R0, R1, 0.5),
                                    atol=1e-9), "中点必须落在最短弧上")

    def test_slerp_takes_the_near_end_not_the_long_way_round(self) -> None:
        """不翻四元数符号会绕远路转 360° 减去夹角。"""
        R0 = animcore.rotmat(170.0, 1)
        R1 = animcore.rotmat(-170.0, 1)
        mid = animcore.slerp(R0, R1, 0.5)
        self.assertTrue(np.allclose(animcore.rotmat(180.0, 1), mid, atol=1e-9),
                        "170° → −170° 的中点是 180°，不是 0°")

    def test_slerp_near_identity_uses_the_linear_branch(self) -> None:
        R0 = np.eye(3)
        R1 = animcore.rotmat(1e-4, 1)
        mid = animcore.slerp(R0, R1, 0.5)
        self.assertTrue(np.allclose(animcore.rotmat(5e-5, 1), mid, atol=1e-9),
                        "夹角极小时走线性分支，结果仍须正确（不能除以 sin≈0）")


class CurveTest(unittest.TestCase):
    def test_wrap_folds_into_zero_one(self) -> None:
        for u, expect in ((0.0, 0.0), (0.25, 0.25), (1.0, 0.0), (1.25, 0.25), (-0.25, 0.75)):
            self.assertAlmostEqual(expect, animcore.wrap(u), places=12)

    def test_clamp01_and_smooth_boundaries(self) -> None:
        self.assertEqual(0.0, animcore.clamp01(-5))
        self.assertEqual(1.0, animcore.clamp01(5))
        self.assertEqual(0.0, animcore.smooth(-1.0))
        self.assertEqual(1.0, animcore.smooth(2.0))
        self.assertAlmostEqual(0.5, animcore.smooth(0.5), places=12)
        self.assertGreater(animcore.smooth(0.75), 0.75, "smoothstep 在上半段比线性更快逼近 1")

    def test_ease_out_boundaries_and_shape(self) -> None:
        self.assertEqual(0.0, animcore.ease_out(0.0))
        self.assertEqual(1.0, animcore.ease_out(1.0))
        self.assertEqual(1.0, animcore.ease_out(3.0), "越界要夹住，不是外推")
        self.assertGreater(animcore.ease_out(0.5), 0.5, "先快后慢")

    def test_pulse_peaks_at_its_centre_and_wraps_around(self) -> None:
        self.assertAlmostEqual(1.0, animcore.pulse(0.3, 0.3, 0.1), places=12)
        self.assertAlmostEqual(animcore.pulse(0.98, 0.02, 0.1), animcore.pulse(0.06, 0.02, 0.1),
                               places=12, msg="脉冲是环形的，跨过 0/1 两侧对称")
        self.assertLess(animcore.pulse(0.5, 0.0, 0.05), 1e-6)

    def test_keyed_covers_before_after_exact_and_zero_width(self) -> None:
        keys = [(0.0, 10.0), (1.0, 20.0), (1.0, 99.0), (2.0, 30.0)]
        self.assertEqual(10.0, animcore.keyed(-1.0, keys), "首点之前取首值")
        self.assertEqual(30.0, animcore.keyed(9.0, keys), "末点之后取末值")
        self.assertEqual(10.0, animcore.keyed(0.0, keys))
        self.assertAlmostEqual(15.0, animcore.keyed(0.5, [(0.0, 10.0), (1.0, 20.0)]), places=12)
        self.assertEqual(20.0, animcore.keyed(1.0, keys),
                         "时间重复时先匹配到的那一段胜出，不是后写的覆盖前面的")
        self.assertEqual(7.0, animcore.keyed(0.5, [(0.0, 5.0), (0.0, 7.0)]),
                         "整列零宽也不许除以零 —— 落到末值")

    def test_soft_clamp_is_transparent_inside_and_never_touches_the_bound(self) -> None:
        self.assertEqual(0.0, animcore.soft_clamp(0.0, -10.0, 10.0, 2.0),
                         "远离边界时原样返回，导数为 1，与未夹区无缝接上")
        for over in (0.0, 1.0, 5.0):
            v = animcore.soft_clamp(10.0 + over, -10.0, 10.0, 2.0)
            self.assertLess(v, 10.0, "永远取不到上界 —— 硬夹的代价是「冻住」而不是越界")
            self.assertGreater(v, 8.0, "收口只发生在边界前 knee 的宽度里")
        # 越界极大时 exp(−x/knee) 在 float64 下下溢到 0，数值上会贴到上界 ——
        # 数学上仍取不到，但断言写成严格小于就是在赌浮点。
        self.assertLessEqual(animcore.soft_clamp(110.0, -10.0, 10.0, 2.0), 10.0)
        self.assertGreater(animcore.soft_clamp(-100.0, -10.0, 10.0, 2.0), -10.0 - 1e-12)

    def test_soft_clamp_with_zero_knee_is_a_hard_clamp(self) -> None:
        self.assertEqual(10.0, animcore.soft_clamp(99.0, -10.0, 10.0, 0.0))
        self.assertEqual(-10.0, animcore.soft_clamp(-99.0, -10.0, 10.0, 0.0))

    def test_jitter_is_stable_across_processes_and_bounded(self) -> None:
        """crc32 不用内置 hash —— 后者每进程加盐，两次跑出的动画会不一样。"""
        self.assertAlmostEqual(-0.9237536656891495, animcore.jitter("x", 1), places=12)
        for i in range(200):
            v = animcore.jitter("bone", i)
            self.assertGreaterEqual(v, -1.0)
            self.assertLessEqual(v, 1.0)

    def test_decay_shake_starts_at_zero_and_decays(self) -> None:
        self.assertAlmostEqual(0.0, animcore.decay_shake(0.0, 5.0, 2.0), places=12)
        peaks = [abs(animcore.decay_shake(k / 5.0 + 0.05, 5.0, 2.0)) for k in range(4)]
        self.assertEqual(peaks, sorted(peaks, reverse=True), "余震必须单调衰减，不是等幅正弦")


class SamplingTest(unittest.TestCase):
    class _Ch:
        def __init__(self, rot):
            self.rot = list(rot)
            self.pos = [0.0, 0.0, 0.0]
            self.scale = [1.0, 1.0, 1.0]

    def _sampler(self, t):
        return {"a": self._Ch([t * 10.0, 0.0, 0.0])}

    def test_loop_reuses_the_first_pose_object_for_the_last_frame(self) -> None:
        """采样器在 t=0 和 t=1 上未必给出逐位相同的浮点数，那点差就是循环时的一跳。"""
        frames = animcore.sample_frames(self._sampler, 2.0, True, [0.0, 0.5, 1.0])
        self.assertEqual(3, len(frames))
        self.assertEqual(2.0, frames[-1][0], "末帧的时间是整条动作的时长")
        self.assertIs(frames[0][1], frames[-1][1], "末帧必须复用首帧那个对象，不是重采一次")

    def test_non_loop_samples_the_last_frame_normally(self) -> None:
        frames = animcore.sample_frames(self._sampler, 2.0, False, [0.0, 0.5, 1.0])
        self.assertIsNot(frames[0][1], frames[-1][1])
        self.assertAlmostEqual(10.0, frames[-1][1]["a"].rot[0], places=12)

    def test_channel_values_defaults_for_bones_the_pose_omits(self) -> None:
        frames = animcore.sample_frames(self._sampler, 1.0, False, [0.0, 1.0])
        self.assertEqual([(0.0, [0.0, 0.0, 0.0]), (1.0, [10.0, 0.0, 0.0])],
                         animcore.channel_values("a", "rot", 0.0, frames))
        self.assertEqual([(0.0, [1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0])],
                         animcore.channel_values("missing", "scale", 1.0, frames),
                         "姿态里没有的骨取该通道的默认值，不是 KeyError")

    def test_is_constant_default_boundary(self) -> None:
        self.assertTrue(animcore.is_constant_default([(0.0, [0.0, 0.0, 0.0])], 0.0))
        self.assertTrue(animcore.is_constant_default([(0.0, [0.0, 9e-5, 0.0])], 0.0),
                        "9e-5 < 1e-4 容差内，仍算恒定默认值")
        self.assertFalse(animcore.is_constant_default([(0.0, [0.0, 2e-4, 0.0])], 0.0))
        self.assertTrue(animcore.is_constant_default([(0.0, [1.0, 1.0, 1.0])], 1.0))

    def test_unwrap_degrees_kills_the_full_turn_artifact(self) -> None:
        """+179 → −179 播出来是整整转一圈；解缠之后是 +179 → +181。"""
        vals = [(0.0, [179.0, 0.0, 0.0]), (0.1, [-179.0, 0.0, 0.0]), (0.2, [-170.0, 0.0, 0.0])]
        animcore.unwrap_degrees(vals)
        self.assertAlmostEqual(181.0, vals[1][1][0], places=9)
        self.assertAlmostEqual(190.0, vals[2][1][0], places=9)

    def test_unwrap_leaves_ordinary_motion_alone(self) -> None:
        vals = [(0.0, [10.0, 0.0, 0.0]), (0.1, [20.0, 0.0, 0.0])]
        animcore.unwrap_degrees(vals)
        self.assertEqual(20.0, vals[1][1][0])


class ExportTest(unittest.TestCase):
    def test_stable_uuid_is_deterministic_and_a_valid_v4(self) -> None:
        a = animcore.stable_uuid("walkfoot_lrotation0")
        self.assertEqual(a, animcore.stable_uuid("walkfoot_lrotation0"))
        self.assertNotEqual(a, animcore.stable_uuid("walkfoot_lrotation1"))
        self.assertEqual(4, uuidlib.UUID(a).version,
                         "别用 crc32 拼：熵只有 32 位，版本/变体位也不合法")

    def test_keyframe_shape_matches_what_blockbench_reads(self) -> None:
        kf = animcore.keyframe("rotation", 0.123456, (1.0, -2.5, 3.0), "seed")
        self.assertEqual("rotation", kf["channel"])
        self.assertEqual(0.1235, kf["time"], "时间四舍五入到 4 位")
        self.assertEqual([{"x": "1.0000", "y": "-2.5000", "z": "3.0000"}], kf["data_points"],
                         "data_points 必须是**字符串**，照能正常打开的工程逐项对齐")
        for key in ("bezier_linked", "bezier_left_time", "bezier_left_value",
                    "bezier_right_time", "bezier_right_value", "interpolation", "color"):
            self.assertIn(key, kf, f"缺 {key}：默认值随 Blockbench 版本变，不值得赌")

    def test_animators_of_uses_the_supplied_uuid_and_seed_hooks(self) -> None:
        tracks = {"leg": {"rotation": [(0.0, [1, 2, 3]), (1.0, [4, 5, 6])]}}
        seen = []

        def seed_of(bone, chan, i):
            seen.append((bone, chan, i))
            return f"S{bone}{chan}{i}"

        out = animcore.animators_of(tracks, lambda b: f"uuid-{b}", seed_of)
        self.assertEqual(["uuid-leg"], list(out))
        self.assertEqual("leg", out["uuid-leg"]["name"])
        self.assertEqual(2, len(out["uuid-leg"]["keyframes"]))
        self.assertEqual([("leg", "rotation", 0), ("leg", "rotation", 1)], seen)
        self.assertEqual(animcore.stable_uuid("Slegrotation0"),
                         out["uuid-leg"]["keyframes"][0]["uuid"])

    def test_animation_entry_encodes_loop_as_a_string(self) -> None:
        looped = animcore.animation_entry("Model", "walk", 1.23456, True, {})
        once = animcore.animation_entry("Model", "walk", 1.0, False, {})
        self.assertEqual("loop", looped["loop"])
        self.assertEqual("once", once["loop"])
        self.assertEqual(1.2346, looped["length"])
        self.assertEqual(animcore.stable_uuid("anim:Model:walk"), looped["uuid"])

    def test_geckolib_document_naming_and_rounding(self) -> None:
        entries = [("walk", 1.0, True, {"leg": {"rotation": [(0.0, [1.23456, 0, 0])]}})]
        doc = animcore.geckolib_document(entries, "bong", "goose")
        self.assertEqual("1.8.0", doc["format_version"])
        key = "animation.bong.goose.walk"
        self.assertEqual([key], list(doc["animations"]))
        self.assertTrue(doc["animations"][key]["loop"])
        self.assertEqual([1.2346, 0, 0],
                         doc["animations"][key]["bones"]["leg"]["rotation"]["0.0"])

    def test_empty_entries_yield_an_empty_but_valid_document(self) -> None:
        doc = animcore.geckolib_document([], "bong", "goose")
        self.assertEqual({}, doc["animations"])


class DeduplicationTest(unittest.TestCase):
    """防复发：两个模块的公共名必须**就是** animcore 里那一个对象。"""

    SHARED = ("rotmat", "euler", "affine", "wrap", "smooth", "pulse", "keyed")

    def test_animkit_forwards_instead_of_copying(self) -> None:
        for name in self.SHARED + ("align", "slerp", "clamp01", "soft_clamp"):
            self.assertIs(getattr(animcore, name), getattr(animkit, name),
                          f"animkit.{name} 又变成自己的一份实现了")

    def test_anim_rig_forwards_instead_of_copying(self) -> None:
        for name in self.SHARED + ("ease_out", "jitter", "decay_shake"):
            self.assertIs(getattr(animcore, name), getattr(anim_rig, name),
                          f"anim_rig.{name} 又变成自己的一份实现了")

    def test_both_uuid_helpers_are_the_shared_one(self) -> None:
        self.assertIs(animcore.stable_uuid, animkit._uuid)
        self.assertIs(animcore.stable_uuid, anim_rig._uuid)

    def test_euler_of_keeps_each_modules_historical_return_type(self) -> None:
        R = animcore.euler((10.0, 20.0, 30.0))
        expect = animcore.euler_xyz(R)
        got_kit = animkit.euler_of(R)
        got_rig = anim_rig.euler_of(R)
        self.assertIsInstance(got_kit, np.ndarray, "animkit 的调用点拿它当向量算")
        self.assertIsInstance(got_rig, list, "anim_rig 的调用点把它当三元序列拆包")
        self.assertTrue(np.allclose(expect, got_kit))
        self.assertTrue(np.allclose(expect, got_rig))

    def test_keyframe_uuid_seeds_keep_their_historical_shapes(self) -> None:
        """两处拼法不同（anim_rig 的通道名出现两次）。统一它 = 既有产物 uuid 全变。"""
        kit = animkit._kf("rotation", 0.0, (0, 0, 0), 3, "walkleg_lrotation")
        rig = anim_rig._kf("rotation", 0.0, (0, 0, 0), 3, "walkleg_lrotation")
        self.assertEqual(animcore.stable_uuid("walkleg_lrotation3"), kit["uuid"],
                         "animkit: 名+骨+通道+序号")
        self.assertEqual(animcore.stable_uuid("walkleg_lrotationrotation3"), rig["uuid"],
                         "anim_rig: 名+骨+通道 + 通道 + 序号")
        self.assertNotEqual(kit["uuid"], rig["uuid"])

    def test_neither_module_still_imports_the_uuid_machinery_it_no_longer_uses(self) -> None:
        for mod in (animkit, anim_rig):
            self.assertFalse(hasattr(mod, "hashlib"),
                             f"{mod.__name__} 还留着 hashlib —— 说明有一份实现没搬干净")

    def test_math_matches_between_the_two_modules_on_random_input(self) -> None:
        rng = np.random.default_rng(23)
        for _ in range(100):
            rot = rng.uniform(-88, 88, 3)
            self.assertTrue(np.allclose(animkit.euler(rot), anim_rig.euler(rot)))
            t = float(rng.uniform(-2, 3))
            self.assertAlmostEqual(animkit.smooth(t), anim_rig.smooth(t), places=15)
            self.assertAlmostEqual(animkit.wrap(t), anim_rig.wrap(t), places=15)
            self.assertAlmostEqual(animkit.pulse(t, 0.3, 0.2), anim_rig.pulse(t, 0.3, 0.2),
                                   places=15)
            self.assertAlmostEqual(math.degrees(0.0) + animkit.keyed(t, [(0.0, 1.0), (1.0, 2.0)]),
                                   anim_rig.keyed(t, [(0.0, 1.0), (1.0, 2.0)]), places=15)


if __name__ == "__main__":
    unittest.main()
