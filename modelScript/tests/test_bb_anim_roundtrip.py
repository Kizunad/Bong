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

本文件做的是 2。它锁不住"两侧同时改错"，这一点在 `bb_anim_axes` 的 docstring 里写明了。

## 还有一层：读和写**本来就不是**同一套符号

Blockbench 读 animation 通道时对 X/Y 取反、存盘时不取反（两次实测，见 `bb_anim_axes`）。
所以：

    生成器写出去    → WRITE_LAYERS（预取反，抵消它读入时的取反）
    它存盘后我们读  → READ_LAYERS

"往返"因此有两种，别混：**生成器 → 读回**用 WRITE 侧（本文件测的就是这个，因为 CI 里
没有 Blockbench）；**Blockbench 存盘 → 读回**用 READ 侧，只能靠人工复现。
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

    def test_write_then_read_back_with_the_same_side(self):
        """同一侧符号必须自洽：写出去再按同一侧读回来，逐轴不变。"""
        axes = {"pitch": -82.7, "yaw": -20.0, "roll": +12.1}
        for name, index, sign in AX.WRITE_LAYERS:
            triple = AX.rotation_to_bb(axes, name)
            self.assertAlmostEqual(axes[name], triple[index] / sign, places=6)

    def test_the_two_sides_differ_on_exactly_pitch_and_yaw(self):
        """读写不对称是**设计**，不是笔误——Blockbench 读入时取反 X/Y、存盘时不取反。

        差异必须**恰好**落在 pitch 与 yaw 上：roll 也跟着翻的话，说明有人把两侧当成
        同一套整体取反了（2026-08-26 就是这么把资产写镜像的）。
        """
        write = {name: sign for name, _i, sign in AX.WRITE_LAYERS}
        read = {name: sign for name, _i, sign in AX.READ_LAYERS}
        self.assertEqual({"pitch", "yaw"},
                         {name for name in write if write[name] != read[name]},
                         f"两侧差异应恰好是 pitch/yaw；实际 write={write} read={read}")
        self.assertEqual(write["roll"], read["roll"], "roll 两侧同号")

    def test_rotation_from_bb_uses_the_read_side(self):
        """`rotation_from_bb` 是给**Blockbench 存盘的文件**用的，走读侧。"""
        for name, index, sign in AX.READ_LAYERS:
            triple = [0.0, 0.0, 0.0]
            triple[index] = 30.0
            self.assertAlmostEqual(30.0 / sign, AX.rotation_from_bb(triple, name), places=6)

    def test_each_layer_only_touches_its_own_component(self):
        """单轴分层的全部意义：一层只动一个分量。混进第二个分量，Blockbench 与 MC 的
        欧拉顺序差异就又有了发挥余地。"""
        axes = {"pitch": 30.0, "yaw": 40.0, "roll": 50.0}
        for name, index, _sign in AX.AXIS_LAYERS:
            triple = AX.rotation_to_bb(axes, name)
            for k in range(3):
                if k != index:
                    self.assertEqual(0.0, triple[k], f"{name} 层渗到了分量 {k}")

    def test_euler_helpers_are_the_static_right_handed_pair(self):
        """`mc_to_euler` / `euler_to_mc` 是**静态 group.rotation** 那一套（读侧口径），
        它俩互逆。静态字段不走 animation 通道，不吃那层取反。"""
        axes = {"pitch": -82.7, "yaw": -20.0, "roll": +12.1}
        back = AX.euler_to_mc(AX.mc_to_euler(axes))
        for key, value in axes.items():
            self.assertAlmostEqual(value, back[key], places=6)
        self.assertEqual([82.7, -20.0, -12.1], AX.mc_to_euler(axes))

    def test_position_write_side_pre_negates_x(self):
        """position 也走 animation 通道，X 同样要预取反。"""
        self.assertEqual([-16.0, -16.0, 16.0],
                         AX.body_position_to_bb({"x": 1.0, "y": 1.0, "z": 1.0}))

    def test_position_read_side_is_the_other_sign(self):
        """读侧（Blockbench 存盘）的 X 不带那层取反。"""
        self.assertAlmostEqual(-1.0, AX.body_position_from_bb([16.0, 0.0, 0.0])["x"])

    def test_bend_round_trip_on_the_write_side(self):
        """写出去再按写侧读回来，(bend, axis) 必须原样还原。

        `bend_from_bb` 是**读侧**的，和 `bend_to_bb` 不互逆——两侧差一个 X 取反，
        这正是本模块最容易被误用的一处，所以两边分开测。
        """
        for bend, axis in ((92.4, 180.0), (44.0, 0.0), (0.0, 180.0)):
            written = AX.bend_to_bb(bend, axis)
            back_bend, back_axis = BP._bend_from(written, AX.WRITE_LAYERS)
            self.assertAlmostEqual(bend, back_bend, places=6)
            if bend > 0:
                self.assertEqual(axis, back_axis)

    def test_bend_read_side_is_the_other_sign(self):
        """读侧：Blockbench 存出来的是未取反的内部值。"""
        self.assertEqual((90.0, 180.0), AX.bend_from_bb(+90.0))
        self.assertEqual((90.0, 0.0), AX.bend_from_bb(-90.0))

    def test_bend_axes_point_opposite_ways(self):
        """axis=0 与 axis=180 是相反的折向；同号就等于两者不分，肘会往错的方向折。"""
        self.assertLess(AX.bend_to_bb(90.0, 0.0) * AX.bend_to_bb(90.0, 180.0), 0.0)

    def test_bend_rides_the_write_side_x_sign(self):
        """bend 走的也是 animation 通道，符号必须跟 pitch 同一侧；跟错了肘膝反折。"""
        pitch_sign = dict((name, sign) for name, _i, sign in AX.WRITE_LAYERS)["pitch"]
        self.assertEqual(pitch_sign > 0, AX.bend_to_bb(90.0, 0.0) > 0)

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

    def test_the_legacy_alias_points_at_the_write_side(self):
        """`AXIS_LAYERS` 这个老名字一律指写侧——生成器用得最多，指错就是写镜像资产。"""
        self.assertIs(AX.WRITE_LAYERS, AX.AXIS_LAYERS)


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
        """生成器直出的文件，按**写侧**读回来必须逐轴还原（CI 里没有 Blockbench，
        能测的就是这一半）。"""
        for filename, name in self.CASES:
            anim = self._bbmodel_anim(filename, name)
            _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
            for tick, source in table:
                read_back = BP.read_pose(anim, float(tick), layers=AX.WRITE_LAYERS)
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

    def test_reading_a_generator_file_with_the_wrong_side_comes_out_mirrored(self):
        """变异用例：拿**读侧**符号去解生成器直出的文件，pitch/yaw 必须整个镜像。

        这条是本文件的价值所在——它证明这套锁不是"自己和自己对拍"，也把
        `bbmodel_to_pose` 那个按 `format_version` 选边的逻辑变成了有后果的事：选错边，
        读出来的姿态就是镜像的（2026-08-26 正是这么把资产写反的）。
        """
        filename, name = self.CASES[0]
        anim = self._bbmodel_anim(filename, name)
        _n, _e, table = RP.anim_pose_table(ANIM / f"{name}.json")
        tick, source = table[2]
        wrong = BP.read_pose(anim, float(tick), layers=AX.READ_LAYERS)
        for axis in ("pitch", "yaw"):
            self.assertAlmostEqual(
                -float(source["rightArm"][axis]),
                float(wrong["rightArm"].get(axis, 0.0)), places=2,
                msg=f"用错侧读，{axis} 应当正好是镜像值")
        self.assertAlmostEqual(float(source["rightArm"]["roll"]),
                               float(wrong["rightArm"].get("roll", 0.0)), places=2,
                               msg="roll 两侧同号，不该跟着镜像")

    def test_the_reader_picks_its_side_from_format_version(self):
        """生成器只写 4.10，Blockbench 5 存盘一律变 5.0——这是本仓既有的"手改过"判据，
        `bbmodel_to_pose` 拿它来选边。"""
        self.assertIs(AX.WRITE_LAYERS,
                      BP.pick_layers({"meta": {"format_version": "4.10"}})[0])
        self.assertIs(AX.READ_LAYERS,
                      BP.pick_layers({"meta": {"format_version": "5.0"}})[0])
        self.assertIs(AX.READ_LAYERS,
                      BP.pick_layers({"meta": {"format_version": "4.10"}}, "blockbench")[0])


if __name__ == "__main__":
    unittest.main()
