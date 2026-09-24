#!/usr/bin/env python3
"""携行步态的契约锁：脚下必须和全局步态**逐帧字节一致**，只多出手臂。

这条锁存在的理由很具体：携行变体是"复制一份全局步态再叠手臂"，一旦有人顺手在变体里
调了膝弯或步幅，玩家就会"换了把刀连走路都变了"——而这种漂移在图上根本看不出来
（两条动画不会同时出现在一个画面里）。所以它只能由机器盯。

另外锁三件在游戏里会静默出问题的事：
- **循环闭合**：`isLoop` 的每个轴首末不同值 → `findAfter` fabricate 一个
  `(endTick+1, defaultValue)` 虚拟帧，整条循环被拖回 0（conventions §7.1）；
- **不写 torso/head**：写了就把玩家"边走边看四周"的视线按在动画值上；
- **肘的折向**：`axis=180` 配正 bend 才是往身前折。
"""

from __future__ import annotations

import json
import math
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
for _d in (REPO / "client" / "tools",):
    if str(_d) not in sys.path:
        sys.path.insert(0, str(_d))

from anim_common import assert_joint_fold_is_anatomical  # noqa: E402
from gen_herb_knife_carry_gait import CARRY, CARRY_PARTS  # noqa: E402

ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
LOWER_PARTS = ("leftLeg", "rightLeg", "body")


def _emote(name: str) -> dict:
    return json.loads((ANIM / f"{name}.json").read_text(encoding="utf-8"))["emote"]


def _axes(emote: dict, parts) -> dict:
    out = {}
    for move in emote["moves"]:
        tick = move["tick"]
        for part, axes in move.items():
            if part in ("tick", "easing", "turn") or part not in parts:
                continue
            for axis, value in axes.items():
                out[(tick, part, axis)] = round(float(value), 9)
    return out


class FootworkIdentityTest(unittest.TestCase):
    """脚下逐帧等于基准步态——换武器只换手臂。"""

    def test_legs_and_body_are_identical_to_the_base_gait(self):
        for name, spec in CARRY.items():
            with self.subTest(anim=name):
                carry = _axes(_emote(name), LOWER_PARTS)
                base = _axes(_emote(spec["base"]), LOWER_PARTS)
                self.assertEqual(
                    base, carry,
                    f"{name} 的腿/body 与 {spec['base']} 不一致——"
                    f"玩家会'换了把刀连走路都变了'")

    def test_the_carry_variant_actually_adds_arms(self):
        """反向锁：只是复制一份基准步态（没叠手臂）就等于这条变体不存在。"""
        for name in CARRY:
            with self.subTest(anim=name):
                arms = _axes(_emote(name), ("leftArm", "rightArm"))
                self.assertGreater(len(arms), 0, f"{name} 一条手臂轨道都没有")

    def test_period_matches_the_base_gait(self):
        for name, spec in CARRY.items():
            with self.subTest(anim=name):
                self.assertEqual(_emote(spec["base"])["endTick"], _emote(name)["endTick"])


class ChannelContractTest(unittest.TestCase):
    """携行层写什么、不写什么。"""

    def test_never_writes_torso_or_head(self):
        for name in CARRY:
            with self.subTest(anim=name):
                used = {p for m in _emote(name)["moves"]
                        for p in m if p not in ("tick", "easing", "turn")}
                self.assertEqual(
                    set(), used - set(CARRY_PARTS),
                    f"{name} 写了 {sorted(used - set(CARRY_PARTS))}——"
                    f"torso/head 要留给'边走边看四周'与招式的躯干拧转")

    def test_is_a_looping_animation(self):
        for name in CARRY:
            with self.subTest(anim=name):
                self.assertTrue(_emote(name)["isLoop"], f"{name} 必须是循环步态")

    def test_loop_closes_on_every_axis(self):
        """每个用到的轴首末同值，否则整条循环被虚拟帧拖回 0。"""
        for name in CARRY:
            with self.subTest(anim=name):
                em = _emote(name)
                end = em["endTick"]
                first = _axes(em, CARRY_PARTS)
                bad = []
                for (tick, part, axis), value in first.items():
                    if tick != 0:
                        continue
                    tail = first.get((end, part, axis))
                    if tail is None or abs(tail - value) > 1e-9:
                        bad.append(f"{part}.{axis}: t0={value} t{end}={tail}")
                self.assertEqual([], bad, f"{name} 首末不等：{bad}")


class ArmReadTest(unittest.TestCase):
    """手臂本身读得对不对。"""

    def test_every_elbow_folds_forward(self):
        """判 bend 折向必须先按 (tick, part) 把轴**汇总**起来。

        `emit_json` 把每个 (tick, part, axis) 拆成独立的 move 记录（这样 LLM 生成时
        错字的盲区最小），所以 `bend` 和它的 `axis` 落在**不同的 move 里**。按单条
        move 查 `axes.get("axis", 0)` 会拿到默认的 0，把每一个 `axis=180` 的正常
        前折都误判成"肘反了"——第一版就是这么红的。
        """
        for name in CARRY:
            with self.subTest(anim=name):
                merged = {}
                for move in _emote(name)["moves"]:
                    for part, axes in move.items():
                        if part in ("tick", "easing", "turn"):
                            continue
                        merged.setdefault((move["tick"], part), {}).update(axes)
                for (tick, part), axes in merged.items():
                    if part not in ("leftArm", "rightArm") or "bend" not in axes:
                        continue
                    assert_joint_fold_is_anatomical(
                        part, math.degrees(axes["bend"]),
                        math.degrees(axes.get("axis", 0.0)),
                        where=f"{name} tick {tick}")

    def test_the_knife_arm_swings_opposite_its_own_leg(self):
        """同手同脚是走路动画最刺眼的错。右腿在前时右臂必须在身后。

        符号：臂 `pitch` 负 = 手往身前，腿 `pitch` 负 = 脚往身前（两边都实测过）。
        判据取"两者乘积 > 0"——同号即同侧同向，就是同手同脚。
        """
        for name in CARRY:
            with self.subTest(anim=name):
                em = _emote(name)
                per_tick = {}
                for move in em["moves"]:
                    for part in ("rightArm", "rightLeg"):
                        if part in move and "pitch" in move[part]:
                            per_tick.setdefault(move["tick"], {})[part] = move[part]["pitch"]
                pairs = [(t, v["rightArm"], v["rightLeg"]) for t, v in per_tick.items()
                         if "rightArm" in v and "rightLeg" in v]
                self.assertGreater(len(pairs), 1, f"{name} 采样点不足，判据落空")
                # 至少要在摆幅两端反号；只看一帧会被"恰好都接近 0"蒙混
                extremes = sorted(pairs, key=lambda p: abs(p[2]))[-2:]
                for tick, arm, leg in extremes:
                    self.assertLess(
                        arm * leg, 0.0,
                        f"{name} t{tick}: 右臂 pitch={math.degrees(arm):.1f}° 与"
                        f"右腿 pitch={math.degrees(leg):.1f}° 同号 —— 同手同脚")


if __name__ == "__main__":
    unittest.main()
