#!/usr/bin/env python3
"""采药刀四条动画的设计意图锁 —— 每道门旁边就是它的缺陷注入器。

和 `test_anim_preview_fidelity.DaggerBladeReadTest`（刃向）、`test_club_anims`（棍头
行程）同一路数，主角换成采药刀：它是 `category=tool` 的凡器，判据围绕「这是在干活，
不是在打架」建。

## 为什么每道门都必须配一个注入器

**「自检全绿」在做差分注入之前，信息量是零。** 这不是洁癖，是本仓真实翻过的车：

- `test_anim_preview_fidelity` 里那六道刃向门，从拆库（#2117）起因为 `sys.path` 少了
  `generators/` 而一直是 **ModuleNotFoundError 而不是断言失败**。unittest 把 ERROR 和
  FAIL 分开计，扫一眼「没红」就过去了——于是刀举在脸旁边像举火把照样放行了整整一段
  时间。门在那儿，只是没在跑。
- 更早一次是穿模判据的白名单写反，**坏版本和修好的版本都报 17 处**，零区分力却两边
  都「有输出」。

所以本文件里每一条判据，`GateInjectionTest` 里都有一条对应的用例：把它该抓的缺陷
**造出来**，断言它真的报得出来。报不出来的门直接算失效。

## 四条动画与它们的可辨识差异（这套差异本身就是判据）

    动画                 形态      endTick  刃尖离地   刃仰角    肘最浅
    harvest_crouch      循环作业     20     7.5px     -45°      28°
    sickle_reap         一次性收获   10    10.8px     -27°      41°
    sickle_stand_cut    循环作业     24    15.9px     -10°      25°
    sickle_defend       一次性防身    8    13.2px     -11°      42°

「地上的药株」和「齐胸的藤蔓」差半个身高，「割活」和「防身」差在上身是压上去还是
往后躲——玩家在远处只看剪影就该分得出来。

## 三条骨架硬几何（推导写在 `client/tools/gen_harvest_crouch.py` 的 docstring 里）

1. 手最低离地 10px（肩枢轴 y=2 + 臂长 12）⇒ 够到地的是**刀**不是手。
2. 髋不下沉（膝高恒在离地 6~6.8px）⇒ `leg.bend` 一大脚就漂，「蹲」不可表示。
3. `torso` 枢轴在**颈**⇒ `torso.pitch` 撕腰缝（`gap ≈ 12·sin(pitch)`，躯干厚 4px），
   而 `torso.yaw` 免费（胯点落在旋转轴上）。
"""

from __future__ import annotations

import importlib.util
import json
import math
import sys
import unittest
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in (LIB_DIR / "generators", LIB_DIR / "tools", REPO / "client" / "tools"):
    if not _d.is_dir():
        raise RuntimeError(f"测试依赖的目录不存在: {_d}")
    sys.path.insert(0, str(_d))

import anim_common as AC  # noqa: E402
import gen_herb_sickle as GHS  # noqa: E402
import preview_player_anim as P  # noqa: E402
import render_animation as RA  # noqa: E402

ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
GEN = REPO / "client" / "tools"

# 全部从几何真值取，别在测试里另写一份——写死过一次就会有第二次（`DaggerAnimationTest`
# 的注释记着同一条教训：刃向改了而测试还在量旧握法）。
_DOC = GHS.build_bbmodel_json()
DISPLAY = _DOC["display"]["thirdperson_righthand"]
TIP_PX = np.array([8.0, max(e["to"][1] for e in _DOC["elements"]), 8.0, 1.0])

# ModelPart 空间：+X 玩家左、+Y 朝下、+Z 身后。脚底 = 腿枢轴 12 + 腿长 12。
GROUND_Y = 24.0
SHOULDER_Y = 2.0

# ── 标定门限 ────────────────────────────────────────────────────────────────
# 刃仰角上限。沿用 `DaggerBladeReadTest` 量出来的 30°：返工前那批「举火把」的姿态是
# 42.9°/39.6°，返工后 23.6°/16.3°，45° 那种看着宽松的阈值会**放行返工前的姿态**。
MAX_BLADE_ELEV = 30.0
# 刀尖不许高过肩线（+ 朝下，所以要求 > -1.0）。比仰角更贴近肉眼读到的东西。
MIN_TIP_BELOW_SHOULDER = -1.0
# 刃尖能达到的最低高度。**这不是门限，是骨架事实**：扫遍整个手臂参数空间
# （pitch×yaw×roll×bend 全网格）刃尖最低只到离地 3.85px，「刃尖扎进地里」这个缺陷
# 在这套骨架上**造不出来**。原先这里挂过一道"刃尖不许入地"的门，被 `GateInjectionTest`
# 当场证伪：注入之后它还是绿的。造不出缺陷的门是装饰品，所以撤掉门、把事实留成断言。
BLADE_FLOOR_PX = 3.85
# 腰缝：torso 下端（胯）到腿枢轴的距离。躯干厚 4px，超过 2.6 就看得见断开。
# gap ≈ 12·sin(torso.pitch)，所以这条等价于「torso.pitch 别超过 ~11°」。
MAX_WAIST_GAP = 2.6
# 脚离地。0.9px 以下是腿几何的固有余量（跗跖静止就有零点几），再多就是腿在漂。
MAX_FOOT_LIFT = 0.9
# `leg.pitch` 上限，conventions §7.2：大 pitch 必然腿腹断连，深度要由 bend 承担。
MAX_LEG_PITCH = 40.0

# (模块名, endTick, isLoop, 发力段起始帧)
SICKLE_ANIMS = (
    ("gen_harvest_crouch", "harvest_crouch", 20, True, 5),
    ("gen_sickle_reap", "sickle_reap", 10, False, 3),
    ("gen_sickle_stand_cut", "sickle_stand_cut", 24, True, 7),
    ("gen_sickle_defend", "sickle_defend", 8, False, 3),
)


def load_pose(module_name: str) -> dict:
    """从生成器里取 POSE 表本身（不是出料 JSON）。

    判据查的是**授权侧**的表，这样注入缺陷时不用去改磁盘上的资产。出料 JSON 与 POSE
    的一致性由 `test_emitted_json_matches_the_pose_table` 单独钉住。
    """
    spec = importlib.util.spec_from_file_location(module_name, GEN / f"{module_name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.POSE


def kfs_of(pose: dict, end_tick: int, is_loop: bool):
    doc = AC.build_doc(pose, name="_probe", description="", end_tick=end_tick,
                       stop_tick=end_tick + 2, is_loop=is_loop)
    return RA.collect_keyframes(doc["emote"])


def blade_at(kfs, tick: float) -> dict:
    """某一 tick 的刃向读感。全部在 ModelPart 空间量。"""
    M = P.item_attach_modelpart(kfs, float(tick), DISPLAY)
    d = M[:3, :3] @ np.array([0.0, 1.0, 0.0])
    d /= np.linalg.norm(d)
    tip = (M @ TIP_PX)[:3]
    return dict(
        elev=math.degrees(math.asin(-d[1])),      # +Y 朝下 ⇒ 取负
        forward=-d[2],                            # +Z 身后 ⇒ 取负
        tip_below_shoulder=tip[1] - SHOULDER_Y,   # >0 = 在肩下
        tip_above_ground=GROUND_Y - tip[1],
        tip=tip,
    )


def waist_gap(kfs, tick: float) -> float:
    sk = RA.solve_skeleton(kfs, float(tick))
    hip = sk["torso"]["end"]
    legs = (sk["rightLeg"]["start"] + sk["leftLeg"]["start"]) / 2.0
    return float(np.linalg.norm(hip - legs))


def foot_lift(kfs, tick: float) -> float:
    sk = RA.solve_skeleton(kfs, float(tick))
    return float(max(GROUND_Y - sk["rightLeg"]["end"][1],
                     GROUND_Y - sk["leftLeg"]["end"][1]))


def samples(end_tick: int, n: int = 41):
    return [end_tick * i / (n - 1) for i in range(n)]


# ═══════════════════════════════════════════════════════════════ 判据本体
#
# 下面每个 `check_*` 都返回 (是否通过, 最坏值, 说明)。写成函数而不是直接写进用例，
# 是为了让 `GateInjectionTest` 能拿同一个判据去检验注入的缺陷——**门和注入器必须查
# 同一段代码**，各写一份就等于没验。

def check_blade_never_points_up(pose, end_tick, is_loop):
    kfs = kfs_of(pose, end_tick, is_loop)
    worst = max((blade_at(kfs, t)["elev"], t) for t in samples(end_tick))
    return worst[0] < MAX_BLADE_ELEV, worst, f"刃仰角 {worst[0]:+.1f}° @t{worst[1]:.1f}"


def check_blade_never_points_back(pose, end_tick, is_loop):
    kfs = kfs_of(pose, end_tick, is_loop)
    worst = min((blade_at(kfs, t)["forward"], t) for t in samples(end_tick))
    return worst[0] > 0.0, worst, f"前向分量 {worst[0]:+.3f} @t{worst[1]:.1f}"


def check_tip_stays_below_shoulder(pose, end_tick, is_loop):
    kfs = kfs_of(pose, end_tick, is_loop)
    worst = min((blade_at(kfs, t)["tip_below_shoulder"], t) for t in samples(end_tick))
    return worst[0] > MIN_TIP_BELOW_SHOULDER, worst, f"刀尖在肩下 {worst[0]:+.2f}px @t{worst[1]:.1f}"


def check_waist_stays_closed(pose, end_tick, is_loop):
    kfs = kfs_of(pose, end_tick, is_loop)
    worst = max((waist_gap(kfs, t), t) for t in samples(end_tick))
    return worst[0] < MAX_WAIST_GAP, worst, f"腰缝 {worst[0]:.2f}px @t{worst[1]:.1f}"


def check_feet_stay_planted(pose, end_tick, is_loop):
    kfs = kfs_of(pose, end_tick, is_loop)
    worst = max((foot_lift(kfs, t), t) for t in samples(end_tick))
    return worst[0] < MAX_FOOT_LIFT, worst, f"脚离地 {worst[0]:.2f}px @t{worst[1]:.1f}"


def check_no_ambiguous_axes(pose, end_tick=None, is_loop=None):
    """`body` 的平移轴与部件的 `x`/`y` 一个都不许写。

    两条都是「预览与运行时不一致」的轴，详见 `gen_harvest_crouch.py`：
      - `body.x/y/z` 运行时走 MatrixStack（单位=格、+Y 朝上），预览当像素加进 ModelPart
        空间（+Y 朝下）。同一个 `body.y=+0.32` 预览里往下 0.32px、真机往上 0.32 格。
      - 部件 `x/y/z` 是**绝对枢轴 px**（`Axis.getValueAtCurrentTick` 只在无关键帧时才
        回落到 vanilla 值），预览却算 `PIVOTS + offset`：手臂 rest y=2 差 2px、腿 rest
        y=12 差 12px（写 `leg.y` 腿会飞到头顶）。
    `leg.z` 是例外——预览 rest 记 0.0、真机 0.1，只差 0.1px，vanilla 蹲伏用的就是它。
    """
    bad = []
    for tick, frame in pose.items():
        for part, axes in frame.items():
            if part == "easing":
                continue
            for axis in axes:
                if part == "body" and axis in ("x", "y", "z"):
                    bad.append(f"t{tick} body.{axis}")
                elif part != "body" and axis in ("x", "y"):
                    bad.append(f"t{tick} {part}.{axis}")
    return not bad, bad, "用了预览与运行时不一致的轴: " + ", ".join(bad[:6])


def check_leg_pitch_within_limit(pose, end_tick=None, is_loop=None):
    worst = (0.0, None, None)
    for tick, frame in pose.items():
        for part in ("leftLeg", "rightLeg"):
            if part in frame and "pitch" in frame[part]:
                v = abs(float(frame[part]["pitch"]))
                if v > worst[0]:
                    worst = (v, tick, part)
    return worst[0] <= MAX_LEG_PITCH, worst, f"{worst[2]}.pitch={worst[0]:.0f}° @t{worst[1]}"


GATES = {
    "blade_up": check_blade_never_points_up,
    "blade_back": check_blade_never_points_back,
    "tip_shoulder": check_tip_stays_below_shoulder,
    "waist": check_waist_stays_closed,
    "feet": check_feet_stay_planted,
    "axes": check_no_ambiguous_axes,
    "leg_pitch": check_leg_pitch_within_limit,
}


class SickleGateTest(unittest.TestCase):
    """八道门逐条跑过四条动画。"""

    def test_all_gates_pass_for_every_sickle_animation(self):
        failures = []
        for mod, name, end, loop, _ in SICKLE_ANIMS:
            pose = load_pose(mod)
            for key, fn in GATES.items():
                ok, _worst, detail = fn(pose, end, loop)
                if not ok:
                    failures.append(f"{name} / {key}: {detail}")
        self.assertEqual(
            [], failures,
            "采药刀动画有门没过（判据与阈值的理由见本文件顶部与各 check_ 的 docstring）:\n  "
            + "\n  ".join(failures))


class SicklePostureTest(unittest.TestCase):
    """姿态框架：循环闭合、guard 复位、肘不打直。"""

    def test_looped_animations_close_on_every_axis(self):
        """§7.1：`isLoop=true` 时任一轴在 endTick 缺同值帧，中段就被插值回
        `defaultValue`——「参数翻倍也看不出效果」的经典症状。`build_doc` 自己会拦，
        这里再钉一道，防止哪天有人把 `is_loop` 改成 False 绕过去。"""
        for mod, name, end, loop, _ in SICKLE_ANIMS:
            if not loop:
                continue
            pose = load_pose(mod)
            self.assertIn(0, pose, f"{name} 缺 tick 0")
            self.assertIn(end, pose, f"{name} 缺 tick {end}")
            for part in set(pose[0]) | set(pose[end]):
                if part == "easing":
                    continue
                a, b = pose[0].get(part, {}), pose[end].get(part, {})
                for axis in set(a) | set(b):
                    self.assertEqual(
                        a.get(axis), b.get(axis),
                        f"{name} 的 {part}.{axis} 首末不等（t0={a.get(axis)} "
                        f"t{end}={b.get(axis)}）——循环会在中段衰减回 defaultValue")

    def test_one_shot_animations_return_to_the_guard_pose(self):
        """§2.1：末帧回到首帧，连续触发时不必先垂手再举。"""
        for mod, name, end, loop, _ in SICKLE_ANIMS:
            if loop:
                continue
            pose = load_pose(mod)
            # 只比姿态轴：`easing` 天生该不同——写在 t0 的管 t0→下一帧，写在末帧的
            # 谁也不管（§15.1）。把它算进去等于要求首帧的缓动和一个不存在的段一致。
            axes_only = lambda f: {k: v for k, v in f.items() if k != "easing"}
            self.assertEqual(
                axes_only(pose[0]), axes_only(pose[end]),
                f"{name} 的 t{end} 与 t0 姿态不一致——连采/连挥时会跳一下")

    def test_the_elbow_never_locks_out(self):
        """采药刀是工具：肘打直既够不到也没有意义。

        对照 `dagger_slash`（兵器）impact 最浅 58°、`sword_stab` 打直到 3°。这里要求
        四条全程 ≥ 20°，`sickle_defend`（防身，人是缩着的）另外要求 ≥ 40°。
        """
        for mod, name, end, loop, _ in SICKLE_ANIMS:
            pose = load_pose(mod)
            bends = [float(f["rightArm"]["bend"]) for f in pose.values() if "rightArm" in f]
            floor = 40.0 if name == "sickle_defend" else 20.0
            self.assertGreaterEqual(
                min(bends), floor,
                f"{name} 右肘最小 bend={min(bends):.0f}°，低于 {floor:.0f}°——"
                f"采药刀是凡器工具，打直会读成短剑")

    def test_torso_carries_the_body_work_as_yaw_not_pitch(self):
        """身体的参与度必须走 `torso.yaw`（免费）而不是 `pitch`（撕腰缝）。

        判据是「yaw 的行程要显著大于 pitch 的行程」。不设成绝对阈值是因为 pitch 的
        绝对值已经由 `waist` 那道门管住了，这里管的是**分工**：别为了"看起来有在动"
        去加 pitch。
        """
        for mod, name, end, loop, _ in SICKLE_ANIMS:
            pose = load_pose(mod)
            yaws = [float(f["torso"]["yaw"]) for f in pose.values() if "torso" in f]
            pitches = [float(f["torso"]["pitch"]) for f in pose.values() if "torso" in f]
            self.assertGreater(
                max(yaws) - min(yaws), max(pitches) - min(pitches),
                f"{name} 的 torso 转体主要靠 pitch（行程 {max(pitches)-min(pitches):.0f}°）"
                f"而不是 yaw（{max(yaws)-min(yaws):.0f}°）——pitch 会撕腰缝，yaw 不会")


class SkeletonFactTest(unittest.TestCase):
    """骨架本身的几何事实 —— 把「做不到什么」也锁住。

    这些不是设计判据，是 MC 玩家骨架的物理上限。锁住它们是为了让后来人不必再花一轮
    去撞：写着"蹲下去用手薅草"的 plan，做出来一定是别的东西。
    """

    def test_the_hand_can_never_reach_the_ground(self):
        """肩枢轴 y=2、上臂+前臂共 12px ⇒ 手心最低到 y=14，离地 10px（膝高）。"""
        best = 1e9
        for pitch in range(-180, 181, 15):
            for bend in range(0, 121, 15):
                pose = {0: dict(easing="LINEAR",
                                head=dict(pitch=0), torso=dict(pitch=0),
                                rightArm=dict(pitch=pitch, yaw=0, roll=0, bend=bend, axis=180),
                                leftArm=dict(pitch=0, bend=0, axis=180),
                                rightLeg=dict(pitch=0, bend=0), leftLeg=dict(pitch=0, bend=0))}
                pose[2] = pose[0]
                kfs = kfs_of(pose, 2, False)
                hand = RA.solve_skeleton(kfs, 0.0)["rightArm"]["end"]
                best = min(best, GROUND_Y - hand[1])
        self.assertGreater(
            best, 8.0,
            f"手心居然能到离地 {best:.1f}px——骨架几何变了，"
            f"`gen_harvest_crouch.py` 里「够到地的是刀不是手」那套论证要重做")

    def test_the_blade_can_never_be_buried_in_the_ground(self):
        """扫遍手臂参数空间，刃尖最低只到离地 3.85px。

        所以「刃尖扎进地里」这个缺陷**造不出来**——`GateInjectionTest` 当场证伪过一道
        为它设的门（注入之后还是绿的）。这条断言把这个事实钉住：哪天刃变长了或者
        `hand_display` 的 scale 改了，它会红，那时候才该把那道门加回来。
        """
        best = 1e9
        for pitch in range(-180, 181, 20):
            for yaw in range(-90, 91, 30):
                for roll in range(-90, 91, 30):
                    for bend in range(0, 121, 30):
                        pose = {0: dict(easing="LINEAR",
                                        head=dict(pitch=0), torso=dict(pitch=0),
                                        rightArm=dict(pitch=pitch, yaw=yaw, roll=roll,
                                                      bend=bend, axis=180),
                                        leftArm=dict(pitch=0, bend=0, axis=180),
                                        rightLeg=dict(pitch=0, bend=0),
                                        leftLeg=dict(pitch=0, bend=0))}
                        pose[2] = pose[0]
                        best = min(best, blade_at(kfs_of(pose, 2, False), 0.0)["tip_above_ground"])
        self.assertGreater(
            best, 1.5,
            f"刃尖现在能压到离地 {best:.1f}px——低到这个程度就该把「刃尖不许入地」那道门"
            f"加回 GATES 并配上注入器")


class SickleDistinctTest(unittest.TestCase):
    """四条必须彼此分得出来，不能只是数值微调。"""

    def _blade_track(self, mod, end, loop):
        kfs = kfs_of(load_pose(mod), end, loop)
        return [blade_at(kfs, t) for t in samples(end, 21)]

    def test_ground_work_and_chest_work_differ_by_half_a_body(self):
        """`harvest_crouch`（地上药株）与 `sickle_stand_cut`（齐胸藤蔓）的刃高必须
        拉开——否则玩家看不出在割什么。"""
        low = min(b["tip_above_ground"] for b in self._blade_track("gen_harvest_crouch", 20, True))
        high = min(b["tip_above_ground"] for b in self._blade_track("gen_sickle_stand_cut", 24, True))
        self.assertGreater(
            high - low, 6.0,
            f"站立割茎的刃只比蹲身割药高 {high-low:.1f}px（要求 >6px ≈ 半个身高的量级）——"
            f"两条读起来会是同一个动作")

    def test_the_reap_cuts_lower_than_the_panic_slash(self):
        """`sickle_reap`（割地上的药株）与 `sickle_defend`（防身横划）都是横扫，
        差异必须在**高度**上：割药是低位，防身是护在胸腹前。"""
        reap = self._blade_track("gen_sickle_reap", 10, False)
        defend = self._blade_track("gen_sickle_defend", 8, False)
        r = min(b["tip_above_ground"] for b in reap)
        d = min(b["tip_above_ground"] for b in defend)
        self.assertGreater(
            d - r, 3.0,
            f"防身横划的刃只比割根一刀高 {d-r:.1f}px（要求 >3px）——两条会读成同一个动作")

    def test_the_loop_is_gentler_than_the_one_shot(self):
        """作业循环的刀尖行程必须明显小于一次性收获——同一个人同一把刀，
        收势那一刀比作业中的每一刀都更狠。"""
        def travel(mod, end, loop):
            track = self._blade_track(mod, end, loop)
            pts = np.array([b["tip"] for b in track])
            return float(np.max(np.linalg.norm(pts - pts[0], axis=1)))
        loop_travel = travel("gen_harvest_crouch", 20, True)
        reap_travel = travel("gen_sickle_reap", 10, False)
        self.assertGreater(
            reap_travel, loop_travel * 1.8,
            f"割根一刀的刀尖只走了 {reap_travel:.1f}px，作业循环走 {loop_travel:.1f}px——"
            f"收获那一刀读不出比平常更狠")

    def test_the_panic_slash_does_not_lean_in_like_the_dagger(self):
        """`sickle_defend` 的核心是「往后躲着划」，和刀三件套的 `dagger_slash`
        「沉肩送刀」相反。判据取 impact 帧的 `head.yaw`：匕首那条把脸转向目标，
        这里的采药人把脸继续偏开。"""
        pose = load_pose("gen_sickle_defend")
        impact_head_yaw = float(pose[5]["head"]["yaw"])
        self.assertGreater(
            impact_head_yaw, 15.0,
            f"sickle_defend 的 impact 帧 head.yaw={impact_head_yaw:+.0f}°，"
            f"脸没有偏开——读成沉肩送刀（那是 dagger_slash 的身法，兵器的）")


class SickleEasingTest(unittest.TestCase):
    """§15：写在某帧的 easing 管的是「本帧 → 下一帧」。"""

    def test_the_drive_frame_carries_an_accelerating_easing(self):
        """发力段的加速必须写在段的**起始**帧上。

        写到顶点帧就会跑去管收势段——`anqi_single_snipe` 的 docstring 白纸黑字写着
        strike 用 INQUAD，实测却是「出手即泄力」，就是这个 off-by-one。
        """
        for mod, name, end, loop, drive in SICKLE_ANIMS:
            pose = load_pose(mod)
            ease = str(pose[drive].get("easing", ""))
            self.assertTrue(
                ease.startswith("IN") and not ease.startswith("INOUT"),
                f"{name} 的 t{drive}（发力段起始帧）easing 是 {ease!r}，"
                f"应为 IN 族（INQUAD/INCUBIC…）才能从静止加速到顶点")


class EmittedAssetTest(unittest.TestCase):
    """磁盘上的资产必须就是这些 POSE 表出料出来的。"""

    def test_emitted_json_matches_the_pose_table(self):
        """防的是「改了生成器忘了重跑」。同时钉住 §9.3 那条等式：POSE 的键 ==
        出料 JSON 的 tick（`bbmodel_to_pose` 的回程全靠它）。"""
        for mod, name, end, loop, _ in SICKLE_ANIMS:
            pose = load_pose(mod)
            spec = importlib.util.spec_from_file_location(mod, GEN / f"{mod}.py")
            m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(m)
            want = AC.build_doc(pose, name=name, description=m.DESCRIPTION,
                                end_tick=end, stop_tick=end + 2, is_loop=loop)
            got = json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))
            self.assertEqual(
                want["emote"], got["emote"],
                f"{name}.json 与 client/tools/{mod}.py 的 POSE 表对不上——"
                f"改完生成器要重跑：python3 client/tools/{mod}.py")
            self.assertEqual(
                sorted(pose.keys()),
                sorted({mv["tick"] for mv in got["emote"]["moves"]}),
                f"{name} 的 POSE 键与出料 tick 不等（§9.3 的回程契约）")


class GateInjectionTest(unittest.TestCase):
    """**差分自证：把每道门该抓的缺陷造出来，断言它真的报得出来。**

    没有这一节，上面那些「全绿」是没有信息量的——`test_anim_preview_fidelity` 的六道
    刃向门就曾经因为 `sys.path` 少一个目录而整段时间是 ERROR 不是 FAIL，扫一眼「没红」
    就放行了「刀举在脸旁边像举火把」。

    每条用例的形状都一样：取一条**本来全绿**的动画 → 注入一处缺陷 → 断言指定的门变红。
    """

    BASE = ("gen_harvest_crouch", 20, True)

    def _mutate(self, **part_overrides):
        """复制 POSE，在每一帧上覆盖指定部件的指定轴。"""
        pose = {t: {k: (dict(v) if isinstance(v, dict) else v) for k, v in f.items()}
                for t, f in load_pose(self.BASE[0]).items()}
        for part, axes in part_overrides.items():
            for frame in pose.values():
                frame.setdefault(part, {}).update(axes)
        return pose

    def _assert_gate_flips(self, gate_key, mutated, what):
        end, loop = self.BASE[1], self.BASE[2]
        clean_ok, _, _ = GATES[gate_key](load_pose(self.BASE[0]), end, loop)
        self.assertTrue(clean_ok, f"基线动画本身就没过 {gate_key} 门，注入试验无意义")
        bad_ok, _, detail = GATES[gate_key](mutated, end, loop)
        self.assertFalse(
            bad_ok,
            f"注入了「{what}」之后 {gate_key} 门**还是绿的** —— 这道门是失效的。"
            f"实测值: {detail}")

    def test_blade_up_gate_catches_a_raised_blade(self):
        """注入**上一版 harvest_crouch 真实的手臂值**（`pitch=-65 yaw=-15 roll=+8
        bend=95`）——实测刃仰角 +75.7°，就是那张"举着火把"的预览图。
        用真实的历史缺陷做注入，比自己编一个数更能证明这道门抓的是对的东西。"""
        self._assert_gate_flips(
            "blade_up",
            self._mutate(rightArm=dict(pitch=-65, yaw=-15, roll=+8, bend=95, axis=180)),
            "上一版那套「举火把」的手臂（实测刃仰 +75.7°）")

    def test_blade_back_gate_catches_a_blade_pointing_behind(self):
        """把刃转到指向身后。上一版的 `harvest_crouch` 实测前向分量 -0.348。"""
        self._assert_gate_flips(
            "blade_back", self._mutate(rightArm=dict(pitch=+40, yaw=+70, bend=10)),
            "刃指向身后")

    def test_tip_shoulder_gate_catches_a_tip_above_the_shoulder(self):
        """刀尖越过肩线——「举火把」最直接的判据，比仰角更贴近肉眼。"""
        self._assert_gate_flips(
            "tip_shoulder", self._mutate(rightArm=dict(pitch=-95, bend=15)),
            "刀尖高过肩")

    def test_waist_gate_catches_a_torn_waist(self):
        """`torso.pitch` 拉到 `loot_bend` 那档 45°：胯往后甩 9px，躯干才厚 4px。"""
        self._assert_gate_flips(
            "waist", self._mutate(torso=dict(pitch=45)), "torso.pitch 拉到 45°")

    def test_feet_gate_catches_floating_feet(self):
        """`leg.bend` 拉到上一版的 44/38：右脚悬空 2.5px，SIDE 机位一眼看得出。"""
        self._assert_gate_flips(
            "feet", self._mutate(rightLeg=dict(bend=44), leftLeg=dict(bend=38)),
            "leg.bend 拉到 44/38 让脚漂起来")

    def test_axes_gate_catches_body_translation(self):
        """写 `body.y`——上一版就是拿它当"蹲伏"，预览 0.32px、真机往上 0.32 格。"""
        self._assert_gate_flips(
            "axes", self._mutate(body=dict(y=+0.32)), "用 body.y 当蹲伏")

    def test_axes_gate_catches_a_part_level_y(self):
        """写 `leg.y`——预览 `PIVOTS+offset` 会加到 12+v，运行时是绝对 v，腿飞到头顶。"""
        self._assert_gate_flips(
            "axes", self._mutate(rightLeg=dict(y=12.2)), "写部件级 leg.y")

    def test_leg_pitch_gate_catches_an_over_rotated_leg(self):
        """`leg.pitch` 越过 40°：conventions §7.2 的腿腹断连。"""
        self._assert_gate_flips(
            "leg_pitch", self._mutate(rightLeg=dict(pitch=-55)), "leg.pitch 拉到 55°")

    def test_every_gate_has_an_injection_case(self):
        """**元判据**：每道门都必须有至少一条注入用例覆盖，新加门却不加注入器会红。

        这条是为了让「门必须配注入器」这件事本身也有机器把关——否则下一个人加了门
        不加注入器，本文件就悄悄退化成又一份没有信息量的全绿自检。
        """
        covered = set()
        for attr in dir(self):
            if not attr.startswith("test_"):
                continue
            src = getattr(type(self), attr).__doc__ or ""
            body = getattr(type(self), attr)
            code = body.__code__
            for const in code.co_consts:
                if isinstance(const, str) and const in GATES:
                    covered.add(const)
        self.assertEqual(
            set(GATES), covered,
            f"这些门没有对应的缺陷注入用例: {sorted(set(GATES) - covered)}。"
            f"没被注入验证过的门，绿了也不能算数（见本类 docstring）")


if __name__ == "__main__":
    unittest.main()
