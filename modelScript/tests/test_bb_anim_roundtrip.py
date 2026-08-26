#!/usr/bin/env python3
"""MC 动画 ↔ bbmodel 的**往返**锁。

## 为什么是往返锁，而不是正向断言

2026-08-26 踩的那个坑：`gen_jian_player_anim` 把动画通道的符号写反了（X/Y 多取了一次
反），生成的 bbmodel 在 Blockbench 里是 pitch / yaw 双双镜像的姿态。

**当时是有离线核验的，而且绿。** 那份核验脚本自己算了一遍 group 层级、和
`preview_player_anim` 的参考数学逐点对拍到 0.05px——因为两边用的是同一套（错的）假设。
自己写的正向断言永远抓不住"我对约定的理解整体偏了"这类错；能抓住它的只有两样：

1. 拿进真的 Blockbench 转一圈再读回来（那次就是这么发现的，但没法进 CI）；
2. **锁住去程和回程共用同一份常量、且互为逆运算**——一侧偷偷改了符号，往返立刻断。

本文件做的是 2。它锁不住"两侧同时改错"，这一点在 `bb_anim_axes` 的 docstring 里写明了；
但它能保证「读回来的姿态 == 写进去的姿态」，而回程（`bbmodel_to_pose`）已经被那次真实的
Blockbench 存盘校准过——于是去程也就跟着被钉住了。
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in (LIB_DIR / "core", LIB_DIR / "generators", LIB_DIR / "tools",
           REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

import bb_anim_axes as AX  # noqa: E402
import bbmodel_to_pose as BP  # noqa: E402
import gen_club_player_anim as GCP  # noqa: E402
import gen_jian_player_anim as GJP  # noqa: E402
import render_player_pose as RP  # noqa: E402

ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
MODELS = LIB_DIR / "models"


class AxisConversionTest(unittest.TestCase):
    """`bb_anim_axes` 自身：每条换算都必须可逆。"""

    def test_rotation_layers_round_trip(self):
        axes = {"pitch": -82.7, "yaw": -20.0, "roll": +12.1}
        for name in AX.AXIS_ORDER:
            triple = AX.rotation_to_bb(axes, name)
            self.assertAlmostEqual(axes[name], AX.rotation_from_bb(triple, name), places=6)

    def test_each_layer_only_touches_its_own_component(self):
        """单轴分层的全部意义：一层只动一个分量。混进第二个分量，Blockbench 与 MC 的
        欧拉顺序差异就又有了发挥余地。"""
        axes = {"pitch": 30.0, "yaw": 40.0, "roll": 50.0}
        for name, index, _sign in AX.AXIS_LAYERS:
            triple = AX.rotation_to_bb(axes, name)
            for k in range(3):
                if k != index:
                    self.assertEqual(0.0, triple[k], f"{name} 层渗到了分量 {k}")

    def test_euler_round_trip(self):
        axes = {"pitch": -82.7, "yaw": -20.0, "roll": +12.1}
        back = AX.euler_to_mc(AX.mc_to_euler(axes))
        for key, value in axes.items():
            self.assertAlmostEqual(value, back[key], places=6)

    def test_position_round_trip(self):
        body = {"x": 0.05, "y": 0.02, "z": -0.06}
        back = AX.body_position_from_bb(AX.body_position_to_bb(body))
        for key, value in body.items():
            self.assertAlmostEqual(value, back[key], places=6)

    def test_position_is_only_a_y_flip_and_a_scale(self):
        """位移只翻 y、乘 16。第一版按"Bedrock X 也取反"写，body.x 整个反向。"""
        self.assertEqual([16.0, -16.0, 16.0],
                         AX.body_position_to_bb({"x": 1.0, "y": 1.0, "z": 1.0}))

    def test_bend_round_trip_both_axes(self):
        for bend, axis in ((92.4, 180.0), (44.0, 0.0), (0.0, 180.0)):
            back_bend, back_axis = AX.bend_from_bb(AX.bend_to_bb(bend, axis))
            self.assertAlmostEqual(bend, back_bend, places=6)
            if bend > 0:
                self.assertEqual(axis, back_axis)

    def test_bend_axes_point_opposite_ways(self):
        """axis=0 与 axis=180 是相反的折向；同号就等于两者不分，肘会往错的方向折。"""
        self.assertLess(AX.bend_to_bb(90.0, 0.0) * AX.bend_to_bb(90.0, 180.0), 0.0)

    def test_oblique_bend_axis_is_refused(self):
        with self.assertRaisesRegex(AssertionError, "不是纯 X 折弯"):
            AX.bend_to_bb(90.0, 90.0)

    def test_bend_twist_residual_is_refused(self):
        """拖 gizmo 拧出来的 y/z 残差在 MC 里表达不了；静默丢掉会让读回来的姿态
        和 Blockbench 里看到的不是同一个。"""
        with self.assertRaisesRegex(ValueError, "残差"):
            AX.assert_pure_x([92.4, -4.8, -19.4], where="rightArm")
        self.assertEqual((92.4, 180.0), AX.assert_pure_x([92.4, 0.2, -0.3]))


class GeneratorSharesTheOneConversionTest(unittest.TestCase):
    """两个生成器 + 回程读取器必须用**同一份**常量。

    各自抄一份就是这次出错的根因：锏那份写反了，木棍那份一开始照抄了它。
    """

    def test_all_three_reference_the_same_module(self):
        self.assertIs(AX, GCP.AX)
        self.assertIs(AX, GJP.AX)
        self.assertIs(AX, BP.AX)

    def test_the_jian_generator_no_longer_defines_its_own_signs(self):
        """锏那份现在只是把公共常量取个别名；哪天有人把本地定义加回来，这里红。"""
        self.assertIs(AX.AXIS_LAYERS, GJP.AXIS_LAYERS)
        self.assertIs(AX.bend_to_bb, GJP.bend_single_axis)


class AnimationRoundTripTest(unittest.TestCase):
    """源 JSON → bbmodel → 读回来，逐轴必须还是同一个姿态。"""

    CASES = (("ClubPlayerAnim.bbmodel", "club_smash"),
             ("ClubPlayerAnim.bbmodel", "club_sweep"),
             ("JianPlayerAnim.bbmodel", "jian_dual_smash"))

    def _bbmodel_anim(self, filename, name):
        doc = json.loads((MODELS / filename).read_text(encoding="utf-8"))
        for anim in doc.get("animations", []):
            if anim["name"] == name:
                return anim
        self.fail(f"{filename} 里没有动画 {name}")

    def test_every_axis_survives_the_round_trip(self):
        for filename, name in self.CASES:
            anim = self._bbmodel_anim(filename, name)
            _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
            for tick, source in table:
                read_back = BP.read_pose(anim, float(tick))
                for part, axes in source.items():
                    got = read_back.get("_body" if part == "_body" else part)
                    self.assertIsNotNone(
                        got, f"{name} t{tick}: 读回来少了 {part}")
                    for axis, value in axes.items():
                        if axis == "axis":
                            continue
                        self.assertAlmostEqual(
                            float(value), float(got.get(axis, 0.0)), places=2,
                            msg=f"{name} t{tick} {part}.{axis}: "
                                f"源 {value} → 读回 {got.get(axis)}")

    def test_a_one_sided_sign_flip_is_caught(self):
        """变异用例：只把**去程**的符号改掉，往返必须断。

        这条是本文件的价值所在——它证明这套锁不是"自己和自己对拍"。
        """
        filename, name = self.CASES[0]
        anim = self._bbmodel_anim(filename, name)
        original = AX.AXIS_LAYERS
        try:
            AX.AXIS_LAYERS = (("pitch", 0, +1.0), ("yaw", 1, -1.0), ("roll", 2, -1.0))
            broken = GCP.convert_animation(ANIM / f"{name}.json", self._gmap(anim))
        finally:
            AX.AXIS_LAYERS = original
        _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
        tick, source = table[0]
        read_back = BP.read_pose(broken, float(tick))
        deltas = [abs(float(source["rightArm"][ax]) - float(read_back["rightArm"].get(ax, 0.0)))
                  for ax in ("pitch", "yaw")]
        self.assertGreater(max(deltas), 1.0,
                           "去程符号翻了，往返却还对得上 —— 这套锁是假的")

    def _gmap(self, anim):
        """把 bbmodel 里现成的 bone uuid 反出来当 gmap（不必重建整套几何）。"""
        return {entry["name"]: uuid for uuid, entry in anim["animators"].items()}


if __name__ == "__main__":
    unittest.main()
