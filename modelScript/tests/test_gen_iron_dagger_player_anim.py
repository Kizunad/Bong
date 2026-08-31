"""`gen_iron_dagger_player_anim` 的锁：几何挂载 + **手持物骨头的往返**。

往返那半边是 2026-09-01 补的，起因是一次整条链路的失明：出料 JSON 里的 `rightItem`
（正握 / 反握）在烘 bbmodel 时被静默丢掉。后果不止"bbmodel 少一点信息"——人在
Blockbench 里看到的是一把永远正握的刀，而他一存盘，回读时这层没有关键帧就读成 0
（`bbmodel_to_pose._value_at` 只取关键帧、不插值），反握从此丢失。
"""

import json
import math
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
LIB = Path(__file__).resolve().parents[1]
for _d in (LIB / "generators", LIB / "tools", REPO / "client" / "tools"):
    if str(_d) not in sys.path:
        sys.path.insert(0, str(_d))

import anim_common as AC  # noqa: E402
import preview_player_anim as PPA  # noqa: E402
import render_animation as RA  # noqa: E402
from bbmodel_maker.workbench import bbmodel_to_pose as B2P  # noqa: E402
from gen_iron_dagger_player_anim import (  # noqa: E402
    ANIM_DIR,
    BLADE_EDGE_AXIS,
    OUT_BB,
    _dagger_display_rot,
    build_geometry,
)

GRIPPED = {"dagger_reverse_slash": {0: 180.0, 8: 180.0},
           "dagger_grip_switch": {0: 0.0, 2: 12.0, 3: -18.0, 4: -78.0,
                                  5: -140.0, 6: -172.0, 8: 180.0}}
UNGRIPPED = ("dagger_stab", "dagger_slash")


class TestGenIronDaggerPlayerAnim(unittest.TestCase):
    def test_geometry_and_dagger_attachment(self):
        elements, outliner, gmap, atlas = build_geometry()
        self.assertGreater(len(elements), 40, "应包含玩家身体 + 匕首几何元素")
        self.assertIn("dagger_right_pitch", gmap, "右臂下应挂载匕首 pitch 节点")
        self.assertIn("dagger_right_roll", gmap, "右臂下应挂载匕首 roll 节点")
        self.assertEqual(atlas.size, (128, 128), "图集应为 128x128 组合 Atlas")


class GripBakeRoundTripTest(unittest.TestCase):
    """JSON 的 `rightItem` → bbmodel 的 `dagger_right_pitch` → 回读，必须逐帧等值。"""

    @classmethod
    def setUpClass(cls):
        cls.doc = json.loads(OUT_BB.read_text(encoding="utf-8"))
        cls.anims = {a["name"]: a for a in cls.doc["animations"]}
        layers, _why = B2P.pick_layers(cls.doc)
        cls.sign = dict((n, sg) for n, _i, sg in layers)["pitch"]
        cls.disp = _dagger_display_rot()

    def _theta_from_json(self, name, tick):
        emote = json.loads((ANIM_DIR / f"{name}.json").read_text(encoding="utf-8"))["emote"]
        kfs = PPA.collect_keyframes(emote)
        axes = {k: RA.sample_axis(kfs, "rightItem", k, float(tick))
                for k in ("pitch", "yaw", "roll")}
        if not emote.get("degrees", True):
            axes = {k: math.degrees(v) for k, v in axes.items()}
        return AC.item_spin_angle(self.disp, BLADE_EDGE_AXIS, axes)

    def _theta_from_bbmodel(self, name, tick):
        tri = B2P._value_at(self.anims[name], "dagger_right_pitch", "rotation", float(tick))
        return None if tri is None else tri[0] / self.sign

    def test_every_gripped_frame_survives_the_bake(self):
        for name, frames in GRIPPED.items():
            for tick, expect in frames.items():
                with self.subTest(anim=name, tick=tick):
                    want, off = self._theta_from_json(name, tick)
                    got = self._theta_from_bbmodel(name, tick)
                    self.assertIsNotNone(
                        got, f"{name} t{tick} 的 dagger_right_pitch 没被烘出来 —— "
                             f"人在 Blockbench 里就看不到这一帧的握法")
                    self.assertAlmostEqual(
                        abs(want), abs(expect), places=3,
                        msg=f"{name} t{tick} 的设计握法角应是 {expect}°，JSON 里是 {want}°")
                    self.assertAlmostEqual(
                        got, want, places=3,
                        msg=f"{name} t{tick}: JSON θ={want}°，烘进 bbmodel 再读回来是 "
                            f"{got}° —— 写侧/读侧符号对不上，握法会在往返中被镜像")
                    self.assertLess(off, 1.0,
                                    f"{name} t{tick} 的 rightItem 不是纯刃口轴自转（偏轴 {off}°）")

    def test_animations_without_a_grip_get_no_dagger_keyframes(self):
        """正握两招不该凭空多出手持物关键帧 —— 多出来就是把静止姿态写死了。"""
        for name in UNGRIPPED:
            entry = B2P._animator(self.anims[name], "dagger_right_pitch")
            self.assertIsNone(
                entry, f"{name} 没有 rightItem，bbmodel 里也不该出现 dagger_right_pitch 轨道")

    def test_the_bake_refuses_a_spin_around_the_wrong_axis(self):
        """绕刀面法线翻同样能把刃倒过来，但会把刃口翻到另一侧 —— 单层 pitch 存不下。

        静默取 theta 会把这种旋转烘成一个**形状不同**的动作，所以要求显式报错。
        """
        import gen_iron_dagger_player_anim as G
        bad = AC.item_spin(self.disp, (0.0, 0.0, 1.0), 180.0)
        _theta, off = AC.item_spin_angle(self.disp, BLADE_EDGE_AXIS, bad)
        self.assertGreater(off, 1.0, "绕刀面法线翻必须被偏轴判据认出来")

        gmap = {"dagger_right_pitch": "fake-uuid"}
        anim = {"name": "probe", "animators": {}}
        import tempfile
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            json.dump({"emote": {"degrees": True, "moves": [
                {"tick": 0, "rightItem": bad}]}}, f)
            path = Path(f.name)
        with self.assertRaises(ValueError):
            G._fill_grip(anim, gmap, path)


class ItemSpinMathTest(unittest.TestCase):
    """`item_spin` ↔ `item_spin_angle`：换算错了，上面的往返会一致地错。"""

    def setUp(self):
        self.disp = _dagger_display_rot()

    def test_round_trip_over_the_whole_circle(self):
        for theta in range(-179, 180, 7):
            back, off = AC.item_spin_angle(self.disp, BLADE_EDGE_AXIS,
                                           AC.item_spin(self.disp, BLADE_EDGE_AXIS, theta))
            self.assertAlmostEqual(back, theta, places=3,
                                   msg=f"θ={theta}° 往返回来变成 {back}°")
            self.assertLess(off, 1e-3, f"θ={theta}° 的自转应是纯刃口轴，实测偏轴 {off}°")

    def test_zero_spin_is_the_identity(self):
        self.assertEqual({"pitch": 0.0, "yaw": -0.0, "roll": 0.0},
                         AC.item_spin(self.disp, BLADE_EDGE_AXIS, 0.0))

    def test_a_half_turn_reverses_the_blade_in_the_hand(self):
        """反握 = 刃向在刀自己的局部系里被倒转。"""
        axes = AC.item_spin(self.disp, BLADE_EDGE_AXIS, 180.0)
        r_item = AC._matmul(AC._matmul(AC._rot_axis((0, 0, 1), axes["roll"]),
                                       AC._rot_axis((0, 1, 0), axes["yaw"])),
                            AC._rot_axis((1, 0, 0), axes["pitch"]))
        rx, ry, rz = self.disp
        r_disp = AC._matmul(AC._matmul(AC._rot_axis((1, 0, 0), rx), AC._rot_axis((0, 1, 0), ry)),
                            AC._rot_axis((0, 0, 1), rz))
        r_disp_t = [[r_disp[j][i] for j in range(3)] for i in range(3)]
        local = AC._matmul(r_disp_t, AC._matmul(r_item, r_disp))
        blade = [local[i][1] for i in range(3)]   # 局部 +Y = 刃向
        self.assertAlmostEqual(blade[1], -1.0, places=4,
                               msg=f"半圈之后刃向应指向 −Y，实际 {blade}")
        self.assertAlmostEqual(local[0][0], 1.0, places=4,
                               msg="刃口轴（X）在这次翻转里必须原地不动")

    def test_it_refuses_axes_it_cannot_solve(self):
        with self.assertRaises(NotImplementedError):
            AC.item_spin_angle(self.disp, (0.0, 1.0, 0.0), {"pitch": 0.0})


class ItemPartValidationTest(unittest.TestCase):
    """`rightItem` 的合法轴：只开旋转。x/y/z 的单位是像素而不是方块，放行会静默踩陷阱。"""

    def _pose(self, axes):
        return {0: dict(easing="linear", rightItem=dict(axes)),
                8: dict(easing="linear", rightItem=dict(axes))}

    def test_rotation_axes_are_accepted(self):
        doc = AC.build_doc(self._pose({"pitch": 1.0, "yaw": 2.0, "roll": 3.0}),
                           name="p", description="p", end_tick=8, stop_tick=8)
        keys = {k for m in doc["emote"]["moves"] for k in m if k not in ("tick", "easing")}
        self.assertEqual({"rightItem"}, keys)

    def test_linear_axes_are_refused_with_the_unit_trap_spelled_out(self):
        with self.assertRaises(ValueError) as cm:
            AC.build_doc(self._pose({"x": 1.0}), name="p", description="p",
                         end_tick=8, stop_tick=8)
        self.assertIn("像素", str(cm.exception))

    def test_bend_is_refused_on_an_item_bone(self):
        with self.assertRaises(ValueError):
            AC.build_doc(self._pose({"bend": 10.0}), name="p", description="p",
                         end_tick=8, stop_tick=8)

    def test_unknown_parts_are_still_refused(self):
        with self.assertRaises(ValueError):
            AC.build_doc({0: dict(easing="linear", middleItem=dict(pitch=1.0))},
                         name="p", description="p", end_tick=8, stop_tick=8)


if __name__ == "__main__":
    unittest.main()
