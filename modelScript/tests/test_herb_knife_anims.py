#!/usr/bin/env python3
"""凡铁采药刀三条玩家动画的设计意图锁 + 门禁的差分自证。

分两层，缺一不可：

1. **门禁跑绿**——三条动画逐条过它自己那套几何门（`player_anim_gates`）。
2. **门禁有区分力**——每道门先注入它该抓的缺陷，报不出违例就算这道门失效。

第 2 层才是这份测试真正的价值。第一轮写完九道门时全绿，看着很像"做对了"；跑差分自证
立刻塌掉五道：`_overlap` 跨轴混算导致刀怼进脑袋也报 0；`inject_sink` 给腿加 pitch 只会
把脚甩起来、根本沉不下去；补偿从"腿后挪"改成"上半身前移"之后注入器还在抽腿的 z，抽了
个空。**没有这一层，前一层的全绿是零信息量。**
"""

from __future__ import annotations

import json
import math
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
for _d in (REPO / "modelScript" / "tools", REPO / "client" / "tools"):
    if str(_d) not in sys.path:
        sys.path.insert(0, str(_d))

import player_anim_gates as G  # noqa: E402
import render_animation as RA  # noqa: E402
from herb_knife_stance import (  # noqa: E402
    ARM_ROLL_MAX, GUARD, HERB_ZONE, HIP_TWIST, _hip_follow,
)

ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
KNIFE = REPO / "modelScript" / "models" / "HerbKnifeIron.bbmodel"

#: 动画名 → 门禁 profile
ANIMS = {
    "herb_harvest": "harvest",
    "herb_knife_slash": "slash",
    "herb_knife_unfold": "draw",
}


def _emote(name: str) -> dict:
    doc = json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))
    return doc.get("emote", doc)


def _knife() -> dict:
    return json.loads(KNIFE.read_text(encoding="utf-8"))


class HerbKnifeGateTest(unittest.TestCase):
    """三条动画逐条过门。"""

    def test_every_animation_passes_its_profile(self):
        knife = _knife()
        for name, profile in ANIMS.items():
            with self.subTest(anim=name):
                bad = [r for r in G.run_gates(_emote(name), knife, profile) if not r.ok]
                self.assertEqual(
                    [], [r.key for r in bad],
                    "\n".join(f"{name}: {r.key} 实测 {r.worst:.2f} / 门限 {r.limit:.2f}"
                              f"（{r.detail}）" for r in bad))


class GateDiscriminationTest(unittest.TestCase):
    """差分自证：每道门必须在它自己的缺陷注入下报违例。

    门限是照这两个数标的——干净版和注入版必须落在门限两侧，而且都留出余量。跑这条测试
    的成本是把每个注入版重新采样一遍（九次），几秒钟，值得每次 CI 都跑。
    """

    def test_every_gate_catches_its_own_defect(self):
        failed = G.self_test(_knife())
        self.assertEqual(
            0, failed,
            f"{failed} 道门在注入缺陷后仍然报'过'——那道门对它该抓的东西没有区分力，"
            f"留着只会给人虚假的安全感")


class SharedStanceTest(unittest.TestCase):
    """三条动画共用一个持刀架势，才接得上。"""

    def test_all_three_settle_into_the_same_guard(self):
        """末帧逐轴等于 `herb_knife_stance.GUARD`（含俯身补偿给的 z）。"""
        for name in ANIMS:
            with self.subTest(anim=name):
                r = G.gate_settle(_emote(name))
                self.assertTrue(r.ok, f"{name}: {r.detail} 偏差 {r.worst:.4f} rad")

    def test_harvest_and_slash_start_where_they_end(self):
        """一次性动作的首末帧必须一致，连着放第二遍不跳格（conventions §2.1）。

        `herb_knife_unfold` 不在此列：它是"从没拿刀到拿着刀"的过渡，首帧在胯边、
        末帧在架势上，本来就不该相等。
        """
        for name in ("herb_harvest", "herb_knife_slash"):
            with self.subTest(anim=name):
                self.assertTrue(G.gate_guard_return(_emote(name)).ok)

    def test_unfold_really_starts_somewhere_else(self):
        """反过来锁 unfold：它要是也从架势起手，"拔刀"这个动作就不存在了。"""
        kfs = RA.collect_keyframes(_emote("herb_knife_unfold"))
        drift = max(abs(RA.sample_axis(kfs, "rightArm", axis, 0.0)
                        - math.radians(GUARD["rightArm"][axis]))
                    for axis in ("pitch", "yaw", "roll", "bend"))
        self.assertGreater(
            math.degrees(drift), 20.0,
            "unfold 的起手姿态和持刀架势几乎一样，那它就不是'拔刀'而是'原地抖一下'")


class ChannelDisciplineTest(unittest.TestCase):
    """通道纪律：这三条动画一格 `body.*` 都不许用。

    理由写在 `herb_knife_stance` 的模块文档里——`body` 的位移单位是**格**而预览按
    px 用（差 16 倍），旋转还差一层手性共轭（pitch/yaw 符号相反），两条都没进游戏
    实测过。仓库里 `+z 当前进`（fist_punch）和 `-z 当前进`（dodge_back）两种写法并存，
    本身就是这个不确定性的证据。绕开它，这三条动画就不会因为哪天把符号定死而返工。
    """

    def test_no_body_channel_anywhere(self):
        for name in ANIMS:
            with self.subTest(anim=name):
                used = sorted({axis for m in _emote(name)["moves"]
                               for part, axes in m.items() if part == "body"
                               for axis in axes})
                self.assertEqual([], used, f"{name} 用到了 body.{used}")

    def test_no_pivot_offset_on_arms_or_legs_except_the_documented_one(self):
        """手臂/腿的 x/y 不许用：运行时是**绝对赋值**（`getBodyOffset` →
        `getValueAtCurrentTick(value0)`，value0 = 静止枢轴 臂 y=2 / 腿 y=12），
        而预览按"静止枢轴 + 偏移"算，两边差 2~12px。腿的 z 可以用（静止枢轴 0.1，
        差 0.1px），也正是俯身补偿要用的那条。
        """
        for name in ANIMS:
            with self.subTest(anim=name):
                bad = sorted({f"{part}.{axis}" for m in _emote(name)["moves"]
                              for part, axes in m.items()
                              if part in ("leftArm", "rightArm", "leftLeg", "rightLeg")
                              for axis in axes if axis in ("x", "y")})
                self.assertEqual([], bad, f"{name} 用到了 {bad}")

    def test_the_legs_follow_the_hip_on_every_keyframe(self):
        """俯身补偿不许漏：每个关键帧两条腿的 `z` 都必须 == `_hip_follow(torso.pitch)`。

        `torso` 的枢轴在脖子，前倾把胯甩到身后 `12·sinθ`；腿不跟着挪就是腰断。漏掉
        一帧就断一帧，而静态图上不盯着髋缝根本看不出来——上一版就是这么全绿交出去、
        被一眼看出"上半身下半身直接分离"的。

        判据取**关键帧**：中间帧由插值给出，而 `z` 与 `torso.pitch` 都是线性插值，
        两端对上、中间就对得上（`sin` 的弧度差在 34° 内 < 0.3px）。
        """
        for name in ANIMS:
            with self.subTest(anim=name):
                em = _emote(name)
                kfs = RA.collect_keyframes(em)
                ticks = sorted({m["tick"] for m in em["moves"]})
                for t in ticks:
                    pitch = math.degrees(RA.sample_axis(kfs, "torso", "pitch", float(t)))
                    want = _hip_follow(pitch)
                    for leg in ("rightLeg", "leftLeg"):
                        got = RA.sample_axis(kfs, leg, "z", float(t))
                        self.assertAlmostEqual(
                            want, got, places=1,
                            msg=f"{name} t{t} {leg}.z={got:.2f}，"
                                f"但 torso.pitch={pitch:.1f}° 要求 {want:.2f}")

    def test_the_hips_turn_with_the_torso(self):
        """躯干拧转时胯必须跟着转 —— 否则就是"胸口拧着、腿正对前方"。

        `torso.yaw` 只作用于躯干那**一个** ModelPart（conventions §7.3），头/臂/腿
        各自独立。上一版全程 `torso.yaw=14°` 而两条腿 yaw 只有站架的固定值，那正是
        "上半身下半身分离"的另一半。判据：腿的 yaw / 躯干的 yaw 恒等于设计比值。
        """
        want_ratio = HIP_TWIST / (1.0 - HIP_TWIST)
        for name in ANIMS:
            with self.subTest(anim=name):
                em = _emote(name)
                kfs = RA.collect_keyframes(em)
                for t in sorted({m["tick"] for m in em["moves"]}):
                    torso = RA.sample_axis(kfs, "torso", "yaw", float(t))
                    if abs(math.degrees(torso)) < 0.5:
                        continue
                    for leg in ("rightLeg", "leftLeg"):
                        got = RA.sample_axis(kfs, leg, "yaw", float(t)) / torso
                        self.assertAlmostEqual(
                            want_ratio, got, places=2,
                            msg=f"{name} t{t}: {leg}.yaw / torso.yaw = {got:.3f}，"
                                f"设计值 {want_ratio:.3f} —— 胯没跟着躯干转")

    def test_the_two_legs_stay_a_single_pelvis(self):
        """两条腿的屈膝量、跟随位移、跟随转体必须逐帧相等。

        它们由 `stance()` 的同一个数派生，分开写就会漂。只有 `pitch` 允许分前后脚。
        """
        for name in ANIMS:
            with self.subTest(anim=name):
                em = _emote(name)
                kfs = RA.collect_keyframes(em)
                for t in sorted({m["tick"] for m in em["moves"]}):
                    for axis in ("bend", "z", "yaw"):
                        r = RA.sample_axis(kfs, "rightLeg", axis, float(t))
                        lf = RA.sample_axis(kfs, "leftLeg", axis, float(t))
                        self.assertAlmostEqual(
                            r, lf, places=4,
                            msg=f"{name} t{t}: rightLeg.{axis}={r:.4f} ≠ "
                                f"leftLeg.{axis}={lf:.4f} —— 两条腿不是同一个胯了")

    def test_arm_roll_stays_inside_the_repo_envelope(self):
        """手臂 roll 封顶 —— roll 转的是**肘的折弯平面**。

        上一版右腕 roll 最深到 58°，把前臂从矢状面掀到了侧面，读感就是"肘往外翻"，
        而仓库 163 条动画里手臂 roll 从没超过 ±35°（`fist_punch_right` v10 的 ±35 是
        极值，绝大多数在 ±12 以内）。
        """
        for name in ANIMS:
            with self.subTest(anim=name):
                kfs = RA.collect_keyframes(_emote(name))
                for part in ("rightArm", "leftArm"):
                    rolls = [abs(math.degrees(v)) for _, v, _ in kfs[part]["roll"]]
                    # 容差是**度→弧度→度**的往返误差（JSON 存弧度），不是给姿态放水
                    self.assertLessEqual(
                        max(rolls), ARM_ROLL_MAX + 1e-3,
                        f"{name} {part} roll 峰值 {max(rolls):.0f}° > {ARM_ROLL_MAX}°")


class KnifeReadTest(unittest.TestCase):
    """刀在手里读得对不对。"""

    def test_display_uses_the_canonical_hand_rotation(self):
        """第三人称握持必须走库里那份 `hand_display`，`rotation` 里的 `-80` 是关键。

        少了那一层 `Rx(-80)`，刀不在拳头里、而是竖在胸口正中往上戳（上一版实测：t0 刀身
        压在躯干贴图上、t4 顶到下巴）。手持物动画全靠挂点定位，挂点错了姿态再准也白调。
        """
        disp = _knife()["display"]["thirdperson_righthand"]
        self.assertEqual([-80, 90, 0], list(disp["rotation"]))

    def test_blade_never_rises_above_the_shoulder(self):
        """刀尖不许过肩——"举火把"读感最直接的判据（同 `DaggerBladeReadTest`）。"""
        knife = _knife()
        for name, profile in ANIMS.items():
            with self.subTest(anim=name):
                r = G.gate_torch_read(G.sample(_emote(name), knife))
                self.assertTrue(r.ok, f"{name}: 刀尖高出肩线 {r.worst:.2f}px（{r.detail}）")

    def test_harvest_blade_actually_reaches_the_herb(self):
        """采割帧刃要同时够低（≤{}px）且够前（≤{}px）。

        上限是**这套骨架的极限**而不是"贴地"：屈膝不会让上半身下沉（各部件是兄弟
        不是链，torso 枢轴恒在 y=24），真正的下蹲只有 `body.y` 做得到，而那条通道的
        符号未定（见 `herb_knife_stance` 模块文档）。所以割的是一格高灵草的茎中段。
        """.format(HERB_ZONE["y_max"], HERB_ZONE["z_max"])
        frames = G.sample(_emote("herb_harvest"), _knife())
        cut = min(frames, key=lambda f: abs(f.tick - 6.0))
        self.assertLessEqual(float(cut.blade_pts[:, 1].min()), HERB_ZONE["y_max"])
        self.assertLessEqual(float(cut.blade_pts[:, 2].min()), HERB_ZONE["z_max"])

    def test_elbow_never_locks_straight(self):
        """短刃打直手臂够不到更远，只会把手腕送出去（同匕首那条的理由）。

        门限比匕首的 15° 松：采割那一帧人是俯身往下够，手臂接近伸直是对的，但仍要留
        一点余量，别真的锁死成一根棍。
        """
        for name in ANIMS:
            with self.subTest(anim=name):
                bends = [math.degrees(v) for _, v, _ in
                         RA.collect_keyframes(_emote(name))["rightArm"]["bend"]]
                self.assertGreaterEqual(
                    min(bends), 10.0,
                    f"{name} 右肘最小 bend={min(bends):.0f}°")


class ActionDistinctionTest(unittest.TestCase):
    """三条动作必须互相分得开——都叫"拿着刀动一下"就等于只做了一条。"""

    def test_pairwise_distinguishable(self):
        kfs = {n: RA.collect_keyframes(_emote(n)) for n in ANIMS}
        ends = {n: float(_emote(n)["endTick"]) for n in ANIMS}
        names = list(ANIMS)
        for i, a in enumerate(names):
            for b in names[i + 1:]:
                diffs = []
                for part in ("rightArm", "torso", "head", "leftArm"):
                    for axis in ("pitch", "yaw", "roll", "bend"):
                        if axis not in kfs[a].get(part, {}) or axis not in kfs[b].get(part, {}):
                            continue
                        for frac in (0.0, 0.25, 0.5, 0.7):
                            diffs.append(abs(
                                RA.sample_axis(kfs[a], part, axis, ends[a] * frac)
                                - RA.sample_axis(kfs[b], part, axis, ends[b] * frac)))
                self.assertGreater(
                    math.degrees(max(diffs)), 30.0,
                    f"{a} 和 {b} 在所有采样点上都太接近，玩家分辨不出是两个动作")

    def test_strike_segment_carries_an_accelerating_easing(self):
        """发力段的 easing 写在**段首帧**上，且必须是 IN 族。

        §15.2 的坑：直觉会把 OUTQUAD 写在撞击帧上，以为那是"到撞击时减速"，实际它管的
        是撞击**之后**那一段。
        """
        windup = {"herb_harvest": 3, "herb_knife_slash": 2, "herb_knife_unfold": 2}
        for name, tick in windup.items():
            with self.subTest(anim=name):
                eases = {m["tick"]: m.get("easing") for m in _emote(name)["moves"]}
                got = str(eases.get(tick, ""))
                self.assertTrue(
                    got.startswith("IN") and not got.startswith("INOUT"),
                    f"{name} t{tick}（发力段起始帧）easing 是 {got!r}，应为 IN 族")


if __name__ == "__main__":
    unittest.main()
