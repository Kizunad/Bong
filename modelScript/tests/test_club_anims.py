#!/usr/bin/env python3
"""木棍两条动画（`club_smash` / `club_sweep`）的设计意图锁。

和 `test_anim_preview_fidelity.DaggerAnimationTest` 同一路数，但判据换了主角：匕首那两条
量的是**刃向**（仰角、刀尖高度），棍量的是**棍头的行程**——钝器的杀伤在头部，靠的是
半径 × 角速度，手到哪里根本不是重点。

这里钉的每一条，都对应三轮打磨里真的翻过车的一处：

1. **抡砸得真的砸下来。** 第一版按"手臂往前伸就是砸"写了 `pitch=-58 / bend=34`，量出来
   棍仰角 **+15.7°——还朝上**，整条动画读成"举着棍往前捅"。display 的 `Rx(-80)` 让棍沿
   前臂出虎口，`pitch` 和 `bend` 在同一旋向上**相加**决定棍的朝向，靠直觉调必然做错。
2. **两条必须正交。** 一纵一横，玩家在远处只看剪影就能分出用的是哪一招。
3. **横抡不许从脸前面扫过去。** 第一版整条弧摆在肩高，数字上更平（竖直行程 0.7px），
   渲出来棍是贴着下巴扫的。
4. **重量感来自节奏，不是来自把回程调慢。** 抡砸底部那一拍近乎静止的滞留是"收不住"的
   全部来源，删掉它就退化成"砸下去顺势弹回来"。
"""

from __future__ import annotations

import json
import math
import sys
import unittest
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in (LIB_DIR / "core", LIB_DIR / "generators", LIB_DIR / "tools",
           REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

import gen_wooden_club as GC  # noqa: E402
import preview_player_anim as P  # noqa: E402
import render_animation as RA  # noqa: E402

ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
DISPLAY = GC.WOODEN_CLUB.display["thirdperson_righthand"]
SHOULDER = np.array(P.PIVOT_OF["rightArm_lo"], float)
# 棍头 / 握把点在**出料系** px（`emit_offset` 已把握把点放到方块中心 (8,8,8)）
HEAD_PX = np.array([8.0, 8.0 + (GC.LENGTH - GC.GRIP) * 16.0, 8.0, 1.0])
GRIP_PX = np.array([8.0, 8.0, 8.0, 1.0])

# (动画名, endTick, LOAD tick, IMPACT tick)
SMASH = ("club_smash", 12.0, 5.0, 7.0)
SWEEP = ("club_sweep", 8.0, 3.0, 5.0)
BOTH = (SMASH, SWEEP)


def _emote(name):
    return json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))["emote"]


def _kfs(name):
    return RA.collect_keyframes(_emote(name))


def _club_head(kfs, tick):
    """棍头相对右肩枢轴的位置，ModelPart 空间（+X 玩家左、**+Y 朝下**、+Z 身后）。"""
    M = P.item_attach_modelpart(kfs, tick, DISPLAY)
    return (M @ HEAD_PX)[:3] - SHOULDER


def _grip(kfs, tick):
    M = P.item_attach_modelpart(kfs, tick, DISPLAY)
    return (M @ GRIP_PX)[:3] - SHOULDER


def _elevation(kfs, tick):
    """棍指向的仰角，度。正 = 棍头朝上。"""
    M = P.item_attach_modelpart(kfs, tick, DISPLAY)
    d = M[:3, :3] @ np.array([0.0, 1.0, 0.0])
    d /= np.linalg.norm(d)
    return math.degrees(math.asin(-d[1]))


def _track(name, end, samples=97):
    kfs = _kfs(name)
    return np.array([_club_head(kfs, end * i / (samples - 1)) for i in range(samples)])


def _speed(name, end, samples=193):
    kfs = _kfs(name)
    pts = np.array([_club_head(kfs, end * i / (samples - 1)) for i in range(samples)])
    dt = end / (samples - 1)
    return np.linalg.norm(np.diff(pts, axis=0), axis=1) / dt, dt


class ClubSwingShapeTest(unittest.TestCase):
    """棍头走的是什么形状——这两条动画真正在交付的东西。"""

    def test_the_smash_actually_comes_down_from_above(self) -> None:
        """抡砸：LOAD 时棍头**高于肩**，IMPACT 时**低于肩**，落差 ≥ 18px。

        第一版量出来撞击帧棍仰角 +15.7°、棍头仍在肩上 2px——那不是砸，是"举着棍往前
        捅"。姿态在数值上"没毛病"，只有把棍真正摆对了才看得出来，所以这条必须量棍头
        而不是量手臂角度。
        """
        name, end, load, impact = SMASH
        kfs = _kfs(name)
        up_at_load = -_club_head(kfs, load)[1]        # +Y 朝下，取负 = 高于肩
        up_at_impact = -_club_head(kfs, impact)[1]
        self.assertGreater(up_at_load, 4.0,
                           f"LOAD 时棍头只高于肩 {up_at_load:.1f}px —— 没举起来")
        self.assertLess(up_at_impact, -8.0,
                        f"IMPACT 时棍头在肩下 {-up_at_impact:.1f}px —— 没砸下来")
        self.assertGreater(up_at_load - up_at_impact, 18.0,
                           "LOAD→IMPACT 的竖直落差不足 18px，读不出「抡砸」")

    def test_the_smash_lands_out_in_front(self) -> None:
        """砸到的地方要在**身前**，不是贴着自己的腿。"""
        name, end, load, impact = SMASH
        forward = -_club_head(_kfs(name), impact)[2]   # +Z 朝身后，取负 = 朝前
        self.assertGreater(forward, 6.0,
                           f"IMPACT 时棍头只在身前 {forward:.1f}px，够不着人")

    def test_the_sweep_stays_level(self) -> None:
        """横抡：整段的竖直起伏必须显著小于横向行程。"""
        name, end, load, impact = SWEEP
        pts = _track(name, end)
        lateral = pts[:, 0].max() - pts[:, 0].min()
        vertical = pts[:, 1].max() - pts[:, 1].min()
        self.assertGreater(lateral, 24.0, f"横向只扫了 {lateral:.1f}px，不成其为横抡")
        self.assertGreater(lateral / vertical, 1.8,
                           f"横/竖 = {lateral / vertical:.2f}，弧线太斜，和抡砸分不开")

    def test_the_sweep_never_crosses_the_face(self) -> None:
        """横抡全程棍头不得高过肩线。

        第一版整条弧摆在肩高（撞击帧棍头高于肩 1.3px），竖直行程只有 0.7px——数字上
        比现在还平，渲出来却是贴着**下巴**扫过去的，正视图里整根棍横在脸前面。棍确实
        在人体前方、并没穿模，但"挡住脸"本身就是缺陷。压到胸腹高的代价是竖直行程涨到
        14.6px，这是**刻意换的**，别再按"更平"把它调回去。
        """
        name, end, _, _ = SWEEP
        kfs = _kfs(name)
        worst = max(((-_club_head(kfs, end * i / 96)[1], end * i / 96) for i in range(97)))
        self.assertLess(
            worst[0], 0.0,
            f"t{worst[1]:g} 棍头高于肩 {worst[0]:.1f}px —— 会从脸前面扫过去")

    def test_the_two_swings_are_orthogonal(self) -> None:
        """一纵一横。这是招式差异化的底线：远处只看剪影就得分得出。"""
        smash = _track(SMASH[0], SMASH[1])
        sweep = _track(SWEEP[0], SWEEP[1])

        def box(pts):
            return (pts[:, 0].max() - pts[:, 0].min(),
                    pts[:, 1].max() - pts[:, 1].min())

        s_lat, s_vert = box(smash)
        w_lat, w_vert = box(sweep)
        self.assertGreater(
            s_vert / s_lat, 2.5,
            f"club_smash 竖/横 = {s_vert / s_lat:.2f}，不够「纵」")
        self.assertGreater(
            w_lat / w_vert, 1.8,
            f"club_sweep 横/竖 = {w_lat / w_vert:.2f}，不够「横」")
        self.assertGreater(
            (s_vert / s_lat) * (w_lat / w_vert), 6.0,
            "两条的主轴比值乘积太小 —— 剪影上分不出是砸还是扫")

    def test_the_club_head_travels_much_further_than_the_hand(self) -> None:
        """钝器的杀伤在**头部**，靠半径 × 角速度。棍头行程必须远大于手的位移，
        否则这条动画等于在用棍做拳的动作。"""
        for name, end, load, impact in BOTH:
            kfs = _kfs(name)
            head = np.linalg.norm(_club_head(kfs, impact) - _club_head(kfs, load))
            hand = np.linalg.norm(_grip(kfs, impact) - _grip(kfs, load))
            self.assertGreater(
                head / hand, 2.0,
                f"{name}: 棍头行程只有手位移的 {head / hand:.2f} 倍")


class ClubTimingTest(unittest.TestCase):
    """节奏——重量感的真正来源。"""

    def test_peak_speed_lands_in_the_strike_segment(self) -> None:
        """棍头峰速必须落在发力段内，不能落到收招段。

        §15.2 的坑：直觉会把 OUTQUAD 写在撞击帧上，以为那是"到撞击时减速"，实际它管的
        是撞击**之后**。后果可量：峰速落在收招段。
        """
        for name, end, load, impact in BOTH:
            speed, dt = _speed(name, end)
            peak = float(np.argmax(speed)) * dt
            self.assertTrue(
                load < peak <= impact + 0.05,
                f"{name} 峰速在 t{peak:.2f}，应落在发力段 ({load:g}, {impact:g}]")

    def test_the_drive_frame_carries_an_accelerating_easing(self) -> None:
        """发力段起始帧的 easing 必须是 IN 族（非 INOUT）。

        `assertAxisDense` 那类检查只查"显式且非 linear"，`INOUTSINE` 照样放行——所以
        这条得单独锁。
        """
        for name, end, load, impact in BOTH:
            eases = {m["tick"]: m.get("easing") for m in _emote(name)["moves"]}
            ease = str(eases.get(int(load), ""))
            self.assertTrue(
                ease.startswith("IN") and not ease.startswith("INOUT"),
                f"{name} 的 t{load:g}（发力段起始帧）easing 是 {ease!r}，"
                f"应为 IN 族才能从静止加速到撞击")

    def test_the_smash_holds_at_the_bottom(self) -> None:
        """抡砸砸到底之后必须有**近乎静止的一拍**。

        这一拍就是"收不住"的全部来源：重心在前的兵器抡到底之后要靠人把它拽回来，不是
        弹回来。删掉它，棍头速度会从打击峰值一路连续地拐进回程，读成"砸下去顺势弹回"。
        回程峰速本身**不是**判据（加不加滞留帧它都是打击峰速的 37% 左右）。
        """
        name, end, load, impact = SMASH
        speed, dt = _speed(name, end)
        peak = speed.max()
        window = speed[int(8.0 / dt):int(9.0 / dt)]     # overshoot → 滞留
        self.assertLess(
            window.max() / peak, 0.10,
            f"t8→t9 段棍头最快还有峰速的 {window.max() / peak:.0%}，没有「停住」那一拍")

    def test_the_smash_is_the_slow_one_and_the_sweep_the_quick_one(self) -> None:
        """两条的**时长**也要拉开：重兵器那一记 12 tick、快的那记 8 tick。"""
        self.assertEqual(12, int(_emote(SMASH[0])["endTick"]))
        self.assertEqual(8, int(_emote(SWEEP[0])["endTick"]))
        for name, *_ in BOTH:
            emote = _emote(name)
            self.assertGreaterEqual(
                emote["stopTick"], emote["endTick"] + 2,
                f"{name}: stopTick 必须 ≥ endTick+2，否则构造函数 +3 兜底、状态锁可能异常")

    def test_impact_sits_around_sixty_percent(self) -> None:
        """撞击落在动画 55%~65%，给回收段留时间（conventions §1）。"""
        for name, end, load, impact in BOTH:
            frac = impact / end
            self.assertTrue(0.55 <= frac <= 0.65,
                            f"{name} 撞击在 {frac:.0%} 处，应在 55%~65%")


class ClubPostureTest(unittest.TestCase):
    """姿态约定：guard / 肘 / 站架。"""

    def test_returns_exactly_to_the_guard_pose(self) -> None:
        """末帧必须与首帧逐轴一致，否则连击时会跳一下。"""
        for name, end, *_ in BOTH:
            kfs = _kfs(name)
            for part, axes in kfs.items():
                for axis in axes:
                    first = RA.sample_axis(kfs, part, axis, 0.0)
                    last = RA.sample_axis(kfs, part, axis, end)
                    self.assertAlmostEqual(
                        first, last, places=6,
                        msg=f"{name} 的 {part}.{axis} 首帧 {first:.4f} ≠ 末帧 {last:.4f}")

    def test_the_guard_is_already_a_swing_ready_pose(self) -> None:
        """tick 0 就得是"要抡了"的架势，不是垂手。

        这不只是 §2.1 的可辨识性要求，还是让发力肢**单调朝打击方向**的手段：把举起来
        （抡砸）/ 摆到右侧（横抡）的状态放进 guard，整条动画里棍就只做一件事。
        """
        for name, end, load, impact in BOTH:
            kfs = _kfs(name)
            guard = _club_head(kfs, 0.0)
            strike = _club_head(kfs, impact)
            travel = float(np.linalg.norm(strike - guard))
            self.assertGreater(
                travel, 20.0,
                f"{name}: guard 到撞击棍头只走了 {travel:.1f}px —— guard 多半还没进入架势")

    def test_the_elbow_opens_but_never_locks(self) -> None:
        """肘在撞击时要打开（不像匕首那样全程蜷着），但绝不锁死（不像剑刺打直到 3）。

        锁肘挥棍是真会伤到自己的动作；而全程蜷着等于放弃棍长带来的力矩。
        """
        for name, end, load, impact in BOTH:
            bends = [math.degrees(v) for _, v, _ in _kfs(name)["rightArm"]["bend"]]
            at_impact = math.degrees(RA.sample_part(_kfs(name), "rightArm", impact)["bend"])
            self.assertGreaterEqual(
                min(bends), 12.0,
                f"{name} 右肘最小 bend={min(bends):.0f}° —— 锁死了")
            self.assertLess(
                at_impact, 50.0,
                f"{name} 撞击帧 bend={at_impact:.0f}° —— 肘没打开，棍长白费")
            self.assertGreater(
                max(bends), 60.0,
                f"{name} 右肘最大 bend={max(bends):.0f}° —— 蓄势时没折起来")

    def test_stance_is_a_constant_whole_body_rotation(self) -> None:
        """`body.yaw` 必须存在且**全程恒定**。

        `torso.*` 只作用于躯干 ModelPart，头/臂/腿各自独立（conventions §L243）——只用
        torso 的话胯和腿全程正对前方，那是"扭了下腰"不是站架。恒定同样是硬要求：站架
        跟着逐帧转的话脚会在地上打滑。
        """
        for name, end, *_ in BOTH:
            kfs = _kfs(name)
            yaws = {round(math.degrees(RA.sample_part(kfs, "body", end * i / 16)["yaw"]), 6)
                    for i in range(17)}
            self.assertEqual(1, len(yaws),
                             f"{name}: body.yaw 在动（{sorted(yaws)}）—— 站架应当恒定")
            self.assertGreater(abs(yaws.pop()), 5.0,
                               f"{name}: body.yaw ≈ 0，整个人没转")

    def test_the_two_stances_are_not_the_same(self) -> None:
        """两招的站架刻意不同：横抡比抡砸更侧身（正面站着抡会把弧线甩到自己胸前）。"""
        stance = {}
        for name, *_ in BOTH:
            stance[name] = math.degrees(RA.sample_part(_kfs(name), "body", 0.0)["yaw"])
        self.assertGreater(
            abs(stance[SWEEP[0]]) - abs(stance[SMASH[0]]), 3.0,
            f"横抡站架 {stance[SWEEP[0]]:.0f}° 不比抡砸 {stance[SMASH[0]]:.0f}° 更侧身")

    def test_head_still_looks_at_the_target(self) -> None:
        """转身之后头要反向补偿回来，否则角色是"侧着身、还把脸扭开"。

        世界朝向 = body.yaw + head.yaw（head 是 body 的子节点，不是 torso 的）。
        """
        for name, end, *_ in BOTH:
            kfs = _kfs(name)
            for i in range(17):
                tick = end * i / 16
                world = math.degrees(RA.sample_part(kfs, "body", tick)["yaw"]
                                     + RA.sample_part(kfs, "head", tick)["yaw"])
                self.assertLess(abs(world), 30.0,
                                f"{name} t{tick:g}: 头的世界朝向 {world:+.1f}°，脸扭离目标太远")

    def test_body_carries_no_pitch_or_roll(self) -> None:
        """`body.*` 走 MatrixStack、会把**整个人连同手持物**一起转（§7.3）。站架只用
        纯 yaw，本文件所有量棍头高度的断言才成立；有人往上加 pitch/roll 这里先红。"""
        for name, end, *_ in BOTH:
            kfs = _kfs(name)
            for i in range(9):
                tick = end * i / 8
                b = RA.sample_part(kfs, "body", tick)
                self.assertAlmostEqual(0.0, float(b["pitch"]), places=9,
                                       msg=f"{name} t{tick:g}: body.pitch 非零")
                self.assertAlmostEqual(0.0, float(b["roll"]), places=9,
                                       msg=f"{name} t{tick:g}: body.roll 非零")

    def test_the_off_hand_breathes(self) -> None:
        """副手不能全程静止，否则观众会觉得那只手是挂在肩上的假肢（§2.4）。
        而且 LOAD 与 IMPACT 要**反相**：都朝同方向变只是"慢慢收紧"，读作跟随不是呼吸。"""
        for name, end, load, impact in BOTH:
            kfs = _kfs(name)
            guard = math.degrees(RA.sample_part(kfs, "leftArm", 0.0)["bend"])
            at_load = math.degrees(RA.sample_part(kfs, "leftArm", load)["bend"])
            at_impact = math.degrees(RA.sample_part(kfs, "leftArm", impact)["bend"])
            self.assertLess(at_load, guard - 5.0,
                            f"{name}: 副手 LOAD 时没有微展（{guard:.0f}→{at_load:.0f}）")
            self.assertGreater(at_impact, guard + 5.0,
                               f"{name}: 副手 IMPACT 时没有猛收（{guard:.0f}→{at_impact:.0f}）")


class ClubGripTest(unittest.TestCase):
    """棍必须真的被握在手里——所有量棍头位置的断言都建立在这上面。"""

    def test_the_grip_sits_in_the_fist_every_tick(self) -> None:
        """握把点全程落在拳心 0.5px 内。

        `emit_offset` 把握把点放到方块中心（= display 枢轴），所以这条同时锁住
        `held_item_common` 的出料平移和 `preview_player_anim` 的挂点链——任何一边漂了
        都会红。拳头只有 4px 宽，超过 0.5px 就看得出棍没被握住。
        """
        fist = np.array([-1.0, 8.5, 0.0, 1.0])       # 臂盒底面往上 1.5px、z 居中
        for name, end, *_ in BOTH:
            kfs = _kfs(name)
            for i in range(25):
                tick = end * i / 24
                part = RA.sample_part(kfs, "rightArm", tick)
                pivot = (np.array(P.PIVOT_OF["rightArm_lo"], float)
                         + np.array([part["x"], part["y"], part["z"]], float))
                R = RA.part_rotation_matrix(part["pitch"], part["yaw"], part["roll"])
                a = float(part["axis"])
                Rb = RA.rotate_about_axis(
                    np.array([np.cos(-a), 0.0, np.sin(-a)]), float(part["bend"]))
                arm = (P._aff(np.eye(3), pivot) @ P._aff(R, np.zeros(3))
                       @ P._about(Rb, P.ITEM_BEND_PIVOT_PX))
                M = P.item_attach_modelpart(kfs, tick, DISPLAY)
                d = float(np.linalg.norm((M @ np.array([8.0, 8.0, 8.0, 1.0]))[:3]
                                         - (arm @ fist)[:3]))
                self.assertLess(d, 0.5,
                                f"{name} t{tick:g}: 握把离拳心 {d:.2f}px")

    def test_the_club_never_points_back_at_the_player(self) -> None:
        """撞击前后棍头不得指向自己身后——那是"抡到自己背上"。"""
        for name, end, load, impact in BOTH:
            kfs = _kfs(name)
            for i in range(13):
                tick = load + (impact + 1.0 - load) * i / 12
                forward = -_club_head(kfs, tick)[2]
                self.assertGreater(
                    forward, -8.0,
                    f"{name} t{tick:.1f}: 棍头在身后 {-forward:.1f}px")


if __name__ == "__main__":
    unittest.main()
