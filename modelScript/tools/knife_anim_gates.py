#!/usr/bin/env python3
"""短兵（匕首/短刃）玩家动画的后验门禁 —— 把「这动作标不标准」变成可量的东西。

## 为什么单独写一套

`bbmodel_maker.gates.animgate` 那五道门是给**四足生物骨架**标定的（触地、打滑、支撑
相平衡……），量的是脚和地面。玩家持刃动画的败笔完全在另一处：**刀在哪、朝哪、走了
多远**。历史上这四条匕首动画同时犯了下面每一条，而当时仓库里没有一道门能报出来：

| 实测到的败笔 | 对应门 |
|---|---|
| 刃全程 +26~+48° 仰角、刀尖高出肩 7~10px —— 读成「举火把」 | `torch` |
| 蓄势帧刀尖穿过自己的脑袋、刃指向身后 | `head` / `backward` |
| 「直刺」握把只前行 4.2px（刀身长 11px），看不出是刺 | `reach` |
| 「反握斜斩」t4→t5 刀尖瞬移 20px —— 不是斩，是闪现 | `teleport` |
| 「转刀」全程刃向只变 26°，握姿根本没换 | `flip` |
| easing 写在撞击帧上，峰速落在收招段 | `peak` |

## 判据来自哪里

阈值全部是**量出来的分界**，不是拍的——每条门的 docstring 里写清返工前/返工后各是
多少。取值原则同 `gatekit`：门限要卡在「坏的报、好的过」之间，宽到能放行历史上那版
坏姿态的阈值等于没锁。

## 每道门配一个缺陷注入器

理由和 `gatekit` / `animgate` 一字不差：**判据本身会假绿，而模型不会怀疑它**。
`self_test()` 把注入器包过的测量源喂进同一道判据，报不出来的门直接算失效。
"""

from __future__ import annotations

import json
import math
import sys
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

_HERE = Path(__file__).resolve().parent
_REPO = _HERE.parents[1]
for _d in (_HERE, _REPO / "client" / "tools"):
    if str(_d) not in sys.path:
        sys.path.insert(0, str(_d))

import preview_player_anim as P  # noqa: E402
import render_animation as RA  # noqa: E402

ANIM_DIR = _REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"

# ── 标定门限 ────────────────────────────────────────────────────────────────
#
# 头方块（Bedrock 空间）x∈[-4,4] y∈[24,32] z∈[-4,4]，再外扩这个余量。刀贴着脸过去
# 和插进脸里，渲出来差别很小，读感差别很大 —— 所以留 0.6px 缓冲，擦边也算犯规。
HEAD_MARGIN = 0.6

# 刀尖允许的最高点。24 = 下巴/头底线。返工前四条动画的刀尖常年在 29~32（眉心到头顶），
# 这就是「举火把」的直接判据 —— 比仰角更贴近肉眼读到的东西。
# 26 给蓄势帧一点余量（刀举到眼睛高度还算蓄势，过头顶就不是了）。
TIP_CEIL = 26.0

# 刃仰角上限。返工前 dagger_slash 最坏 +48°、dagger_stab +41°；返工后分别是 +19/+3。
# 30 是这两组数之间唯一说得清的分界：45 那种"宽松点"的阈值放行返工前的姿态，等于没锁。
ELEV_CEIL = 30.0

# 刀尖允许越过躯干背面的深度（躯干本体后表面在体坐标 z=+2）。量的是**位置**不是朝向：
# 反握时刃自拳下垂、略朝后，是正经的握法（dir_z 可以到 +0.5），一刀切死朝向会把它误杀；
# 真正的败笔是**刀尖绕到人背后去**——返工前 dagger_reverse_slash 蓄势段刀尖到了体坐标
# z=+8.9（在后脑勺后面）。3.0 = 背面再往后 1px，擦着也算。
BEHIND_MAX = 3.0

# 发力段握把最小行程（px）。刀身出鞘长度 11px；握把走不到它的一半，动作在观众眼里
# 就不成立。返工前 dagger_stab 只有 4.2px（还包含了 body 位移），返工后 8~13px。
REACH_MIN = 6.0

# 相邻 1/8 tick 之间刀尖允许的最大位移（px）。这条不是抓「快」，是抓**不连续**：
# 返工前 dagger_reverse_slash 在 t2.125 一格跳了 20.6px（= 165px/tick），那不是斩，
# 是闪现；而一次正常的横划峰值也就 6.1px/格。9.0 卡在这两组数中间，两边都有余量。
TELEPORT_MAX = 9.0

# 换握动作里，世界刃向必须至少转过这么多度，同时握把不许"整个挥出去"（见 `gate_flip`）。
#
# 2026-08-31 重标：用户手摆的两端把转刀从「原地转腕」改成了「一边抬起来一边翻腕」
# （反握低位 → 正握胸前架），两个数都得跟着动。四条动画同口径实测：
#
#   动画                 握把离首帧最远   世界刃向最大转过
#   dagger_stab              6.2px            26.7°   ← 完全没换握的参照
#   dagger_grip_switch      10.1px            66.7°   ← 本条
#   dagger_slash            15.1px            87.1°   ← 一次完整挥砍的参照
#
# 下限取 55：远高于「等于没换握」的 26.7（v1 那版转刀就是 26°），又低于本设计达成的
# 66.7。上限取 13px：卡在本条的 10.1 和「一次挥砍」的 15.1 之间 —— 转刀要是退化成
# 挥一刀，这条会红。两个数都夹在**实测的坏值与好值之间**，不是贴着天花板设的。
FLIP_MIN_DEG = 55.0
FLIP_TRAVEL_MAX = 13.0

# 肘最小折角。匕首够不着，伸直只是把手腕送到对方面前 —— 这条是匕首和剑的分界。
ELBOW_MIN_DEG = 15.0

# 首末帧逐轴允许的差（弧度）。非循环招连击时首帧接末帧，差一点就跳一下。
GUARD_TOL = 1e-6

# 收势段刀尖速度相对峰速的上限。四条动画返工后的实测末格占比：直刺 8%、下劈 14%、
# 反握上撕 21%、转刀 26%；把「末格就是峰速」（100%）判死，取 0.45 卡在两者之间，
# 既容得下转刀那种全程匀速的短动作，也报得出被掐断的收招。
DECEL_RATIO = 0.45

# endTick → stopTick 的混出长度下限（tick）。本仓四条招都写 endTick 8 / stopTick 10。
BLENDOUT_MIN = 2.0

# 关键帧之间，握把允许偏离「两端连线」的走廊宽度。取 max(绝对下限, 比例×段长)：
# 一次真实的挥砍本来就走弧线，段越长弧越大，一刀切绝对值会误杀发力段；而转场这种
# 两端只隔 2px 的短段，任何鼓包都是欧拉插值绕远路绕出来的。
#
# 这条是**量出来补的**：某一版转刀在 t6→t8（两端相隔 2.0px）的中间把手甩到了肩高，
# 偏离连线 4.9px —— 前面每一道门都是绿的（刀尖没过下巴、没穿头、没瞬移、翻转够），
# 因为它们各自只看一帧或只看相邻两帧，没有一道在问「这一段走的是不是设计好的那条路」。
CORRIDOR_ABS = 2.2
CORRIDOR_REL = 0.38

# 刃向的走廊：采样帧的刃向离「两端之间那条大圆弧」的最大垂距（见 `dir_off_arc`）。
# 位置走廊管住了手，刃向还能自己绕远 —— 实测某版转刀在 t4→t6 段里刃先从 −41° 翘回
# −9° 再落到 −64°，手几乎没动，看起来就是刀"抖了一下"。
# 25° 是量出来的：那一版绕远段 31°；改成垂距口径后，四条动画全段实测 ≤9°，余量充足。
DIR_CORRIDOR_DEG = 25.0

SUBDIV = 8  # 每 tick 采样数


@dataclass
class KnifeGateResult:
    key: str
    label: str
    ok: bool
    worst: float
    detail: str


class InjectionImpossible(RuntimeError):
    """注入器造不出这道门该抓的缺陷 —— 门失效，不是动画没问题。"""


# ── 被测对象 ────────────────────────────────────────────────────────────────


def held_item_probe(bbmodel: Path):
    """→ (握把, 刃尖, 柄尾) 三个模型 px 坐标。刃沿 +Y，握把 = display 枢轴 (8,8,8)。"""
    doc = json.loads(Path(bbmodel).read_text(encoding="utf-8"))
    els = doc["elements"]
    lo = min(e["from"][1] for e in els)
    hi = max(e["to"][1] for e in els)
    return (np.array([8.0, 8.0, 8.0, 1.0]),
            np.array([8.0, hi, 8.0, 1.0]),
            np.array([8.0, lo, 8.0, 1.0]))


def display_of(bbmodel: Path, mode: str = "thirdperson_righthand") -> dict:
    doc = json.loads(Path(bbmodel).read_text(encoding="utf-8"))
    disp = (doc.get("display") or {}).get(mode)
    if disp is None:
        raise ValueError(f"{Path(bbmodel).name} 没有 display.{mode} —— 手持姿态无从谈起")
    return disp


class KnifeTake:
    """一条动画 + 一件手持物，提供所有门要用的测量闭包。"""

    def __init__(self, anim: str, bbmodel: Path, *, subdiv: int = SUBDIV):
        doc = json.loads((ANIM_DIR / f"{anim}.json").read_text(encoding="utf-8"))
        self.name = anim
        self.emote = doc.get("emote", doc)
        self.kfs = RA.collect_keyframes(self.emote)
        self.end = float(self.emote.get("endTick", 8))
        self.disp = display_of(bbmodel)
        self.grip_px, self.tip_px, self.butt_px = held_item_probe(bbmodel)
        self.ticks = [self.end * i / (subdiv * self.end)
                      for i in range(int(subdiv * self.end) + 1)]

    # -- 手持物 --------------------------------------------------------------
    def item_at(self, t: float):
        """→ (握把, 刃尖, 柄尾) 的 Bedrock 世界坐标。"""
        M = P.hand_transform(self.kfs, t, self.disp)
        return (M @ self.grip_px)[:3], (M @ self.tip_px)[:3], (M @ self.butt_px)[:3]

    def blade_dir_at(self, t: float) -> np.ndarray:
        g, tip, _ = self.item_at(t)
        d = tip - g
        return d / np.linalg.norm(d)

    # -- 肢体 ----------------------------------------------------------------
    def limb_segments_at(self, t: float) -> dict:
        """四肢每个分段的 (起点, 终点)，Bedrock 世界坐标。头/躯干不分段，不在此列。"""
        seg = P.segment_transforms(self.kfs, t)
        out = {}
        for name, M in seg.items():
            if not (name.endswith("_lo") or name.endswith("_up")):
                continue
            part = P.PART_OF[name]
            pivot = np.array(P.PIVOT_OF[name], float)
            if name.endswith("_lo"):
                a = P._pt(pivot + RA.bend_center(part))
                b = P._pt(pivot + RA.limb_end_local(part))
            else:
                a = P._pt(pivot)
                b = P._pt(pivot + RA.bend_center(part))
            out[name] = ((M @ np.append(a, 1.0))[:3], (M @ np.append(b, 1.0))[:3])
        return out

    def head_frame_at(self, t: float) -> np.ndarray:
        """头的世界变换的逆 —— 把别的点换算进头的本地系，盒子才跟着头走。

        钉一个静态盒子是错的：`body.yaw` 一转，头就不在 x∈[-4,4] 那一格里了，
        于是「刀穿过脑袋」既可能漏报也可能误报（本仓 body.yaw=-30，两种都出现过）。
        """
        return np.linalg.inv(P.segment_transforms(self.kfs, t)["head"])

    def torso_frame_at(self, t: float) -> np.ndarray:
        """躯干世界变换的逆 —— 「刀尖在不在背后」得按身体自己的前后轴量，不是世界 z。"""
        return np.linalg.inv(P.segment_transforms(self.kfs, t)["torso"])

    def body_frames_at(self, t: float) -> dict:
        """躯干与两条腿各自世界变换的逆，供自穿模判据把刀换算进各自本地系。"""
        seg = P.segment_transforms(self.kfs, t)
        return {nm: np.linalg.inv(seg[nm]) for nm in BODY_BOXES if nm in seg}

    def bend_deg_at(self, t: float, part: str = "rightArm") -> float:
        return math.degrees(float(RA.sample_part(self.kfs, part, t)["bend"]))

    def key_ticks(self) -> list:
        return sorted({float(m["tick"]) for m in self.emote["moves"]})

    def easings(self) -> dict:
        return {float(m["tick"]): m.get("easing") for m in self.emote["moves"]}

    def sample(self, part: str, axis: str, tick: float) -> float:
        return RA.sample_axis(self.kfs, part, axis, tick)

    def axes(self):
        return [(p, a) for p, ax in self.kfs.items() for a in ax]

    def axis_pairs(self):
        """(part, axis, 首帧值, 末帧值) —— 收势闭合门用。"""
        for part, axes in self.kfs.items():
            for axis in axes:
                yield (part, axis,
                       RA.sample_axis(self.kfs, part, axis, 0.0),
                       RA.sample_axis(self.kfs, part, axis, self.end))


# ── 几何小工具 ──────────────────────────────────────────────────────────────

HEAD_BOX = ((-4.0, 4.0), (24.0, 32.0), (-4.0, 4.0))
# 躯干 / 两条腿的方块（各自 ModelPart 静止坐标，Bedrock 口径）。刀插进自己胸口或大腿，
# 和插进脑袋一样是穿模，只是更不容易一眼看出来 —— 尤其反握时刃贴着大腿垂下来。
BODY_BOXES = {
    "torso": ((-4.0, 4.0), (12.0, 24.0), (-2.0, 2.0)),
    "rightLeg_up": ((-3.9, 0.1), (6.0, 12.0), (-2.0, 2.0)),
    "leftLeg_up": ((-0.1, 3.9), (6.0, 12.0), (-2.0, 2.0)),
    "rightLeg_lo": ((-3.9, 0.1), (0.0, 6.0), (-2.0, 2.0)),
    "leftLeg_lo": ((-0.1, 3.9), (0.0, 6.0), (-2.0, 2.0)),
}
# 刀插进躯干/腿的容差。比头那道松一点：短刃贴身是正常握法，擦着皮不算穿模，
# 真正要抓的是「刃没入体内」。1.2 = 刀身厚度量级。
SELF_CLIP_MARGIN = -1.2


def _seg_hits_box(a, b, box, margin, n=32) -> bool:
    lo = np.array([c[0] - margin for c in box])
    hi = np.array([c[1] + margin for c in box])
    for i in range(n + 1):
        p = a + (b - a) * (i / n)
        if np.all(p >= lo) and np.all(p <= hi):
            return True
    return False


# ── 门 ──────────────────────────────────────────────────────────────────────


def gate_torch(item_at, ticks, ceil=TIP_CEIL, since=None) -> KnifeGateResult:
    """刀尖不许升到下巴线以上 —— 「举火把」最直接的判据。

    `since` 把窗口挪到某一 tick 之后。过顶下劈的蓄势段刀尖本来就要举过头顶（用户手摆
    的起手式刀尖在 y=41），整段一刀切只能被迫把门限抬到 45 —— 那就成了纯棘轮，什么
    都锁不住。真正要防的是**劈下去之后刀还举在脸前**，所以窗口从撞击帧起算，门限维持
    在下巴线上，teeth 一点没丢；蓄势段交给 `gate_head` / `gate_selfclip` 兜。
    """
    lo = -1e9 if since is None else since
    worst, wt = -1e9, 0.0
    for t in ticks:
        if t < lo - 1e-9:
            continue
        _g, tip, _b = item_at(t)
        if tip[1] > worst:
            worst, wt = float(tip[1]), t
    ok = worst <= ceil
    win = "" if since is None else f"，窗口 t≥{since:g}"
    return KnifeGateResult("torch", "刀尖高度", ok, worst,
                           f"最高 y={worst:.1f}（t{wt:g}），上限 {ceil:.0f}{win}"
                           + ("" if ok else " —— 刀举到了脸前，读成举火把"))


def gate_elev(item_at, ticks, ceil=ELEV_CEIL, since=None) -> KnifeGateResult:
    """刃仰角上限。刀尖翘得比这更高就不是持刃，是举火把。`since` 同 `gate_torch`。"""
    lo = -1e9 if since is None else since
    worst, wt = -1e9, 0.0
    for t in ticks:
        if t < lo - 1e-9:
            continue
        g, tip, _b = item_at(t)
        d = tip - g
        e = math.degrees(math.asin(d[1] / np.linalg.norm(d)))
        if e > worst:
            worst, wt = e, t
    ok = worst <= ceil
    win = "" if since is None else f"，窗口 t≥{since:g}"
    return KnifeGateResult("elev", "刃仰角", ok, worst,
                           f"最大 {worst:+.0f}°（t{wt:g}），上限 {ceil:.0f}°{win}")


def gate_behind(item_at, torso_frame_at, ticks, cap=BEHIND_MAX,
                until=None) -> KnifeGateResult:
    """刀尖不许绕到自己背后去。

    `until` 把窗口收在跟随动作之前。反握上撕的收势本来就要把刃甩到右后（用户手摆的
    末帧刀尖在体坐标 z=+13.2）—— 那是撕开之后的余势，不是败笔。而这条门当初要抓的
    是**蓄势段刀尖绕到后脑勺**（返工前实测 z=+8.9），那一段在撞击帧之前，收在窗口里
    teeth 不丢；窗口之后由 `gate_selfclip` / `gate_head` / `gate_decel` 接着看。
    """
    lim = ticks[-1] if until is None else until
    worst, wt = -1e9, 0.0
    for t in ticks:
        if t > lim + 1e-9:
            continue
        _g, tip, _b = item_at(t)
        z = float((torso_frame_at(t) @ np.append(tip, 1.0))[2])
        if z > worst:
            worst, wt = z, t
    ok = worst <= cap
    win = "" if until is None else f"，窗口 t≤{until:g}"
    return KnifeGateResult("behind", "刀尖绕背", ok, worst,
                           f"体坐标最深 z={worst:+.1f}（t{wt:g}），上限 {cap:+.1f}{win}")


def gate_head(item_at, limbs_at, head_frame_at, ticks, margin=HEAD_MARGIN) -> KnifeGateResult:
    """刀身和两条前臂都不许穿过（或擦过）自己的脑袋。"""
    bad = []
    for t in ticks:
        inv = head_frame_at(t)

        def local(p):
            return (inv @ np.append(p, 1.0))[:3]

        _g, tip, butt = item_at(t)
        if _seg_hits_box(local(butt), local(tip), HEAD_BOX, margin):
            bad.append((t, "刀身"))
            continue
        segs = limbs_at(t)
        for nm in ("rightArm_lo", "leftArm_lo"):
            a, b = segs[nm]
            if _seg_hits_box(local(a), local(b), HEAD_BOX, margin):
                bad.append((t, nm))
                break
    ok = not bad
    return KnifeGateResult("head", "穿头", ok, float(len(bad)),
                           "无" if ok else
                           f"{len(bad)}/{len(ticks)} 帧穿头，首个 t{bad[0][0]:g}（{bad[0][1]}）")


def gate_selfclip(item_at, frames_at, ticks, margin=SELF_CLIP_MARGIN) -> KnifeGateResult:
    """刀身不许插进自己的躯干或大腿。"""
    bad = []
    for t in ticks:
        _g, tip, butt = item_at(t)
        frames = frames_at(t)
        for nm, box in BODY_BOXES.items():
            inv = frames.get(nm)
            if inv is None:
                continue
            a = (inv @ np.append(butt, 1.0))[:3]
            b = (inv @ np.append(tip, 1.0))[:3]
            if _seg_hits_box(a, b, box, margin):
                bad.append((t, nm))
                break
    ok = not bad
    return KnifeGateResult("selfclip", "刀插自己", ok, float(len(bad)),
                           "无" if ok else
                           f"{len(bad)}/{len(ticks)} 帧插进自己，首个 t{bad[0][0]:g}（{bad[0][1]}）")


def gate_smooth(item_at, key_ticks, ticks,
                abs_tol=CORRIDOR_ABS, rel_tol=CORRIDOR_REL) -> KnifeGateResult:
    """每个关键帧区间内，握把必须待在两端连线附近的走廊里。

    抓的是「关键帧摆得都对，中间那一段却绕了远路」—— 欧拉角线性插值在两端分支相差
    较大时会走出一条谁也没设计过的路。逐帧判据看不见它（每一帧单看都合法），相邻帧
    判据也看不见（每一步都不大），只有拿整段和它的设计意图比才看得出来。
    """
    keys = sorted(key_ticks)
    worst, where = 0.0, ""
    for ta, tb in zip(keys, keys[1:]):
        a = item_at(ta)[0]
        b = item_at(tb)[0]
        span = float(np.linalg.norm(b - a))
        tol = max(abs_tol, rel_tol * span)
        d = b - a
        n = float(np.linalg.norm(d))
        for t in ticks:
            if not (ta < t < tb):
                continue
            p = item_at(t)[0]
            if n < 1e-9:
                dev = float(np.linalg.norm(p - a))
            else:
                u = float(np.clip((p - a) @ d / (n * n), 0.0, 1.0))
                dev = float(np.linalg.norm(p - (a + d * u)))
            if dev - tol > worst - 1e-12 and dev > tol:
                worst, where = dev, f"t{t:g}（{ta:g}→{tb:g} 段，走廊 {tol:.1f}px）"
    ok = not where
    return KnifeGateResult("smooth", "走廊", ok, worst,
                           "每段都贴着两端连线走" if ok else
                           f"握把在 {where} 偏离连线 {worst:.1f}px —— 插值绕了远路")


def _angle(a, b) -> float:
    return math.degrees(math.acos(np.clip(float(a @ b), -1.0, 1.0)))


def dir_off_arc(v, a, b) -> float:
    """单位向量 `v` 离「a→b 大圆弧」的角距（度）—— 落在弧段内就是离平面的角，
    落在弧外就是到最近端点的角。

    **为什么不能直接比 slerp(u)**：那样量的是「偏离 + 走得快慢」的混合物。带缓动的
    一段本来就不是匀速的 —— INCUBIC 在 u=0.625 处只走了 0.24 的行程，一段 63° 的挥
    于是凭空报出 (0.625−0.24)×63 ≈ 24° 的"偏离"，而刃根本没离开设计的那条弧。
    2026-08-31 反握上撕就是这样卡在门限上（实测 25.0° vs 上限 25°），排查下来偏离量
    正好等于缓动曲线与直线的差 —— 门量错了东西，不是动画绕了远路。
    改成量到弧的**垂距**，就与走多快无关，只与走没走那条弧有关；位置走廊
    （`gate_smooth`）早就是这么做的（它把点投影到连线上），这里只是把同一条口径补齐。
    """
    n = np.cross(a, b)
    if np.linalg.norm(n) < 1e-9:                    # 两端共线：弧退化成一个点
        return min(_angle(v, a), _angle(v, b))
    n = n / np.linalg.norm(n)
    out = abs(math.degrees(math.asin(np.clip(float(v @ n), -1.0, 1.0))))
    inplane = v - float(v @ n) * n
    if np.linalg.norm(inplane) < 1e-9:
        return out
    inplane = inplane / np.linalg.norm(inplane)
    span = _angle(a, b)
    if _angle(a, inplane) + _angle(inplane, b) <= span + 1e-6:
        return out                                   # 投影落在弧段内
    return min(_angle(v, a), _angle(v, b))           # 冲出了端点 —— 那才是绕远


def gate_dir_corridor(item_at, key_ticks, ticks, cap=DIR_CORRIDOR_DEG) -> KnifeGateResult:
    """采样帧的刃向必须贴着「两端之间那条大圆弧」走，不许中途绕远。

    位置走廊（`gate_smooth`）只管住手。手不动而刃自己翘回去再落下来，读起来是刀
    抖了一下 —— 那同样是欧拉插值绕远路，只是绕在朝向上。判据用到弧的垂距而不是到
    slerp(u) 的角距，理由见 `dir_off_arc`。
    """
    def dirs(t):
        g, tip, _b = item_at(t)
        v = tip - g
        return v / np.linalg.norm(v)

    keys = sorted(key_ticks)
    worst, where = 0.0, ""
    for ta, tb in zip(keys, keys[1:]):
        da, db = dirs(ta), dirs(tb)
        for t in ticks:
            if not (ta < t < tb):
                continue
            ang = dir_off_arc(dirs(t), da, db)
            if ang > worst:
                worst, where = ang, f"t{t:g}（{ta:g}→{tb:g} 段）"
    ok = worst <= cap
    return KnifeGateResult("dircorridor", "刃向走廊", ok, worst,
                           f"最大偏离弧 {worst:.0f}°（{where}），上限 {cap:.0f}°")


def gate_reach(item_at, load_tick, hit_tick, floor=REACH_MIN) -> KnifeGateResult:
    """蓄势帧到撞击帧，握把必须真的走出距离 —— 不然观众看不出这是一次出手。"""
    g0, _t0, _b0 = item_at(load_tick)
    g1, _t1, _b1 = item_at(hit_tick)
    d = float(np.linalg.norm(g1 - g0))
    ok = d >= floor
    return KnifeGateResult("reach", "发力行程", ok, d,
                           f"t{load_tick:g}→t{hit_tick:g} 握把走了 {d:.1f}px，下限 {floor:.0f}px")


def gate_teleport(item_at, ticks, cap=TELEPORT_MAX) -> KnifeGateResult:
    """逐格刀尖位移上限 —— 抓「不是斩，是闪现」。"""
    worst, wt = 0.0, 0.0
    prev = None
    for t in ticks:
        _g, tip, _b = item_at(t)
        if prev is not None:
            d = float(np.linalg.norm(tip - prev))
            if d > worst:
                worst, wt = d, t
        prev = tip
    ok = worst <= cap
    return KnifeGateResult("teleport", "刀尖瞬移", ok, worst,
                           f"单格最大 {worst:.1f}px（t{wt:g}），上限 {cap:.1f}px")


def gate_peak(item_at, ticks, load_tick, hit_tick) -> KnifeGateResult:
    """刀尖峰速必须落在发力段 (load, hit] 内。

    §15.2 的坑：easing 管的是「本帧→下一帧」。写在撞击帧上，峰速就跑到收招段去了。
    """
    pts = [(t, item_at(t)[1]) for t in ticks]
    best, bt = -1.0, 0.0
    for (t0, p0), (t1, p1) in zip(pts, pts[1:]):
        v = float(np.linalg.norm(p1 - p0)) / max(t1 - t0, 1e-9)
        if v > best:
            best, bt = v, t1
    ok = load_tick < bt <= hit_tick + 1e-9
    return KnifeGateResult("peak", "峰速落点", ok, bt,
                           f"峰速在 t{bt:.2f}（{best:.1f}px/tick），应落在 ({load_tick:g}, {hit_tick:g}]")


def gate_elbow(bend_at, ticks, floor=ELBOW_MIN_DEG, until=None) -> KnifeGateResult:
    """蓄势与发力段内肘不许伸直 —— 匕首和剑的分界。

    `until` 把窗口收在收势之前。2026-08-31 用户手摆的末帧把直刺/下劈都收在**完全
    伸展**（bend 6.9° / 1.6°）—— 那是他要的定格，不是败笔。整段一刀切会把这两条
    动画判死；而真正要防的「拿匕首当剑使」发生在蓄势和挥击途中，收在窗口里就够。
    窗口外不放弃观测：`gate_decel` 仍然盯着末段有没有在减速。
    """
    lim = ticks[-1] if until is None else until
    worst, wt = 1e9, 0.0
    for t in ticks:
        if t > lim + 1e-9:
            continue
        b = bend_at(t)
        if b < worst:
            worst, wt = b, t
    ok = worst >= floor
    return KnifeGateResult("elbow", "肘不打直", ok, worst,
                           f"最小 bend {worst:.0f}°（t{wt:g}，窗口 t≤{lim:g}），"
                           f"下限 {floor:.0f}°")


def gate_chain(sample_mine, axes_mine, links, tol=GUARD_TOL) -> KnifeGateResult:
    """转场招的首/末帧必须逐轴等于它所衔接的那条动画的架势帧。

    转刀不该「回到自己的首帧」——它的职责就是把正握架换成反握架。但它两端**必须**
    分别焊死在 `dagger_stab` 的正握 guard 和 `dagger_reverse_slash` 的反握 guard 上，
    否则连招时会在衔接处跳一下，而这种跳只有连着播才看得见，静态图完全看不出来。

    `sample_mine(part, axis, tick)` 是被测动画的采样闭包（注入器包的就是它）；
    对照方每次现读，保证门量的是**磁盘上的那两条动画**而不是某份缓存。
    """
    bad = []
    for my_tick, other_name, other_tick in links:
        other = KnifeTake(other_name, DAGGER_MODEL)
        pairs = set(axes_mine()) | {(p, a) for p, ax in other.kfs.items() for a in ax}
        for part, axis in sorted(pairs):
            a = sample_mine(part, axis, my_tick)
            b = RA.sample_axis(other.kfs, part, axis, other_tick)
            if abs(a - b) > tol:
                bad.append((my_tick, other_name, part, axis, abs(a - b)))
    worst = max((r[-1] for r in bad), default=0.0)
    if not bad:
        return KnifeGateResult("chain", "衔接闭合", True, 0.0,
                               "两端分别与 "
                               + "、".join(f"{n}@t{t:g}" for _m, n, t in links) + " 逐轴一致")
    r = max(bad, key=lambda x: x[-1])
    unit = "" if r[3] in ("x", "y", "z") else "\u00b0"
    shown = r[-1] if r[3] in ("x", "y", "z") else math.degrees(r[-1])
    return KnifeGateResult("chain", "衔接闭合", False, worst,
                           f"{len(bad)} 条轴对不上，最坏 {shown:.2f}{unit}"
                           f"（t{r[0]:g} vs {r[1]} 的 {r[2]}.{r[3]}）")


def gate_guard(axis_pairs, tol=GUARD_TOL, parts=None) -> KnifeGateResult:
    """首末帧必须逐轴一致，否则连击时跳一下。

    `parts` 把这条要求收在**必须闭合的部位**上。2026-08-31 用户手摆的四条动画都收在
    跟随动作上（直刺收在伸展、下劈收在低位），手臂两端本来就不该相等；但他一根没动
    下盘 —— body / torso / head / 两腿在首末帧仍逐字相同。所以闭合这条约束**在下盘
    仍然成立**，把它留在那儿，别因为手臂不闭合就整条弃守。手臂那一头交给
    `gate_decel` + `gate_blendout`。
    """
    rows = [r for r in axis_pairs() if parts is None or r[0] in parts]
    bad = [(p, a, v0, v1) for p, a, v0, v1 in rows if abs(v0 - v1) > tol]
    worst = max((abs(v0 - v1) for _p, _a, v0, v1 in bad), default=0.0)
    if not bad:
        scope = "首末帧逐轴一致" if parts is None else \
            f"首末帧在 {'/'.join(sorted(parts))} 上逐轴一致"
        return KnifeGateResult("guard", "收势闭合", True, 0.0, scope)
    p0, a0, u0, u1 = max(bad, key=lambda r: abs(r[2] - r[3]))
    # x/y/z 是位移（格），其余是弧度 —— 一律按角度打印会把 0.2 格印成 11.5°
    unit = "" if a0 in ("x", "y", "z") else "°"
    shown = abs(u0 - u1) if a0 in ("x", "y", "z") else math.degrees(abs(u0 - u1))
    return KnifeGateResult("guard", "收势闭合", False, worst,
                           f"{len(bad)} 条轴不闭合，最坏 {shown:.2f}{unit}（{p0}.{a0}）")


def gate_decel(item_at, ticks, ratio=DECEL_RATIO) -> KnifeGateResult:
    """收势段必须在减速 —— 末段刀尖速度不许接近峰速。

    收势闭合（`gate_guard`）只在「末帧 == 首帧」时说得通。用户手摆的四条动画都收在
    跟随动作上，那条判据对手臂失效了，但它原本要防的事没消失：**动画不能在最快的
    那一格戛然而止**（读起来是被掐断，不是收招）。这条从速度侧接手。

    比值而不是绝对值：四条动画的幅度差着三倍（转刀 vs 下劈），绝对阈值只能二选一。
    """
    speeds = []
    for (t0, t1) in zip(ticks, ticks[1:]):
        v = float(np.linalg.norm(item_at(t1)[1] - item_at(t0)[1])) / max(t1 - t0, 1e-9)
        speeds.append((t1, v))
    peak = max(v for _t, v in speeds) if speeds else 0.0
    tail = speeds[-1][1] if speeds else 0.0
    frac = tail / peak if peak > 1e-9 else 0.0
    ok = frac <= ratio
    return KnifeGateResult("decel", "收势减速", ok, frac,
                           f"末格 {tail:.1f}px/tick = 峰速 {peak:.1f} 的 {frac * 100:.0f}%，"
                           f"上限 {ratio * 100:.0f}%"
                           + ("" if ok else " —— 动画在最快的那一格被掐断"))


def gate_blendout(emote, min_blend=BLENDOUT_MIN) -> KnifeGateResult:
    """末帧不回起手式的招，必须留出 stopTick 混出段，否则姿态会硬弹回去。

    PlayerAnimator 在 endTick→stopTick 之间把动画权重降到 0。末帧停在跟随动作上时，
    这段混出就是唯一把人带回站架的东西；stopTick == endTick 等于当场断电。
    """
    end = float(emote.get("endTick", 0))
    stop = float(emote.get("stopTick", end))
    span = stop - end
    ok = span >= min_blend
    return KnifeGateResult("blendout", "混出段", ok, span,
                           f"endTick {end:g} → stopTick {stop:g}，混出 {span:g} tick，"
                           f"下限 {min_blend:g}")


def gate_flip(item_at, ticks, floor=FLIP_MIN_DEG,
              travel_cap=FLIP_TRAVEL_MAX) -> KnifeGateResult:
    """换握必须真把刃翻过来：刃的世界朝向转过 ≥ floor，同时握把没有"整个挥出去"。

    第一版量的是「刃相对前臂的朝向」，那是个**恒等于零的量** —— 手持物被 display 变换
    焊死在前臂上，相对朝向按定义不会变，门永远报 0°，干净动画也过不了。换握在 MC 里
    只能靠转手腕/前臂实现，观众读到的正是「手没动，刀掉了个头」，所以判据是这两条的
    合取：世界刃向转够 + 握把行程小。
    """
    def dir_at(t):
        g, tip, _b = item_at(t)
        d = tip - g
        return d / np.linalg.norm(d)

    d0, g0 = dir_at(ticks[0]), item_at(ticks[0])[0]
    turn, wt, travel = 0.0, 0.0, 0.0
    for t in ticks:
        ang = math.degrees(math.acos(np.clip(float(d0 @ dir_at(t)), -1, 1)))
        if ang > turn:
            turn, wt = ang, t
        travel = max(travel, float(np.linalg.norm(item_at(t)[0] - g0)))
    ok = turn >= floor and travel <= travel_cap
    why = []
    if turn < floor:
        why.append("刃没翻过来")
    if travel > travel_cap:
        why.append("手整个挥出去了，不是转腕")
    return KnifeGateResult("flip", "换握幅度", ok, turn,
                           f"世界刃向最大转过 {turn:.0f}°（t{wt:g}，下限 {floor:.0f}°）、"
                           f"握把行程 {travel:.1f}px（上限 {travel_cap:.0f}px）"
                           + ("" if ok else " —— " + "，".join(why)))


def gate_easing(easings, load_tick) -> KnifeGateResult:
    """发力段起始帧的 easing 必须是 IN 族（不含 INOUT）。

    §15.2 的坑：每帧的 easing 管的是「本帧 → 下一帧」。直觉会把 OUTQUAD 写在撞击帧上
    以为那是「到撞击时减速」，实际它管的是撞击**之后**那一段，峰速于是落到收招段去。
    `gate_peak` 从结果侧抓这件事，这条从源头侧抓 —— 两条都留着，因为峰速门在动作
    幅度很小的招上分辨率不够。
    """
    e = str(easings.get(load_tick, ""))
    ok = e.startswith("IN") and not e.startswith("INOUT")
    return KnifeGateResult("easing", "发力段缓动", ok, 0.0,
                           f"t{load_tick:g} 的 easing = {e or '（无）'}"
                           + ("" if ok else "，应为 IN 族才能从静止单调加速到撞击"))


def gate_distinct(sample_a, sample_b, name_b, ticks=(0.0, 3.0, 5.0),
                  parts=("rightArm", "leftArm", "torso", "body"),
                  axes=("pitch", "yaw", "roll", "bend", "z"),
                  floor_deg=25.0) -> KnifeGateResult:
    """两条招必须是两个能分辨的动作，不能只是数值微调。"""
    worst, where = 0.0, ""
    for part in parts:
        for axis in axes:
            for t in ticks:
                d = abs(sample_a(part, axis, t) - sample_b(part, axis, t))
                if d > worst:
                    worst, where = d, f"{part}.{axis}@t{t:g}"
    ok = worst >= math.radians(floor_deg)
    return KnifeGateResult("distinct", f"与 {name_b} 可分辨", ok, worst,
                           f"最大差 {math.degrees(worst):.0f}°（{where}），下限 {floor_deg:.0f}°")


# ── 缺陷注入器 ──────────────────────────────────────────────────────────────
#
# 每个注入器把**测量源**包一层，制造这道门该抓的那种缺陷。判据和门限一格不动。

def _rot_about(axis, ang):
    return RA.rotate_about_axis(np.asarray(axis, float), math.radians(ang))


def lift_item_above(item_at, tip_y: float = TIP_CEIL + 4.0):
    """把整把刀**抬到固定高度** —— torch 门该抓的。

    第一版写的是「在原位上抬 8px」。反握那条动画的刃是垂着的（刀尖最高才 13.5），
    抬 8px 之后还在 21.5，门照样过 —— 注入等于没做。相对量的注入器都有这个毛病：
    它的效果取决于被注入的那条动画长什么样。改成绝对落位，任何姿态都必然越线。
    """
    def wrapped(t):
        g, tip, b = item_at(t)
        off = np.array([0.0, tip_y - float(tip[1]), 0.0])
        return g + off, tip + off, b + off
    return wrapped


def tilt_blade_up(item_at, elev: float = 55.0):
    """把刃**直接摆到**固定仰角 —— elev 门该抓的。

    注入器不能写成「在原朝向上再转 X 度」：反握那条动画的刃本来指着地下（−84°），
    相对旋转 55° 之后还是负的，门照样过 —— 那样的自证是假的。所以这里把刃向整个
    替换成一个明确越线的朝向，任何起始姿态都必然触发。
    """
    d = np.array([0.0, math.sin(math.radians(elev)), -math.cos(math.radians(elev))])

    def wrapped(t):
        g, tip, b = item_at(t)
        return g, g + np.linalg.norm(tip - g) * d, g - np.linalg.norm(b - g) * d
    return wrapped


def shove_item_behind(item_at, dz: float = 40.0):
    """把整把刀**平移到身后** —— behind 门该抓的。

    第一版是「绕竖直轴甩 150°」。反握时刃几乎是垂直的，绕 y 轴转它等于没转（z 分量
    几乎不变），门照样过。同 `lift_item_above`：注入器不能依赖被注入姿态的朝向。

    位移量也不能小气：直刺发力帧刀尖本来就伸到身前 14px，再加上站架 30° 的斜置，
    平移 16px 只能把它推到体坐标 +1.4，仍在 +3.0 的门限内 —— 注入照样落空。40 是
    「比最深的前伸还多一截」，任何一帧都必然越线。
    """
    def wrapped(t):
        g, tip, b = item_at(t)
        off = np.array([0.0, 0.0, dz])
        return g + off, tip + off, b + off
    return wrapped


def shove_item_into_torso(item_at):
    """把刀塞进胸口 —— selfclip 门该抓的。"""
    def wrapped(t):
        g, tip, b = item_at(t)
        c = np.array([0.0, 18.0, 0.0])
        mid = (tip + b) / 2.0
        return g + (c - mid), tip + (c - mid), b + (c - mid)
    return wrapped


def shove_item_into_head(item_at):
    """把刀整个塞进脑袋 —— head 门该抓的。"""
    def wrapped(t):
        g, tip, b = item_at(t)
        c = np.array([0.0, 28.0, 0.0])
        mid = (tip + b) / 2.0
        return g + (c - mid), tip + (c - mid), b + (c - mid)
    return wrapped


def flatten_reach_by(item_at, at_tick: float):
    """把撞击帧的握把按回蓄势位 —— reach 门该抓的。"""
    def wrapped(t):
        return item_at(at_tick if abs(t - at_tick) > 1e-9 else t)
    return wrapped


def jump_tip_by(item_at, at_tick: float, jump: float = 12.0):
    """在某一格上把刀尖挪开一大截 —— teleport 门该抓的。"""
    def wrapped(t):
        g, tip, b = item_at(t)
        if t >= at_tick:
            off = np.array([jump, 0.0, 0.0])
            return g + off, tip + off, b + off
        return g, tip, b
    return wrapped


def bulge_path_by(item_at, key_ticks, amount: float = 6.0):
    """在某个区间中点把握把顶出去 —— smooth 门该抓的「插值绕远路」。"""
    keys = sorted(key_ticks)
    ta, tb = keys[0], keys[1]

    def wrapped(t):
        g, tip, b = item_at(t)
        if ta < t < tb:
            w = math.sin(math.pi * (t - ta) / (tb - ta))
            off = np.array([0.0, amount * w, 0.0])
            return g + off, tip + off, b + off
        return g, tip, b
    return wrapped


def detour_dir_by(item_at, key_ticks, ang: float = 45.0):
    """在某个区间中点把刃向翘出去 —— 刃向走廊门该抓的。"""
    keys = sorted(key_ticks)
    ta, tb = keys[0], keys[1]

    def wrapped(t):
        g, tip, b = item_at(t)
        if ta < t < tb:
            w = math.sin(math.pi * (t - ta) / (tb - ta))
            R = _rot_about((1.0, 0.0, 0.0), -ang * w)
            return g, g + R @ (tip - g), g + R @ (b - g)
        return g, tip, b
    return wrapped


def straighten_elbow_by(bend_at, at: float = 3.0):
    def wrapped(t):
        return at
    return wrapped


def break_guard_by(axis_pairs, delta: float = 0.2, parts=None):
    """把一条轴的末帧值挪开 —— guard 门该抓的「首末帧对不上」。

    **必须挪门看得见的那条轴**。门被 `parts` 收窄之后，挪 `rows[0]`（往往是 body 之外
    的手臂轴）就等于挪在门的视野外，注入落空 —— 这正是「相对/盲挑式注入器」那一类
    假自证。所以这里按门自己的 parts 过滤后再挑。
    """
    def wrapped():
        rows = list(axis_pairs())
        target = next((r for r in rows if parts is None or r[0] in parts), None)
        if target is None:
            raise InjectionImpossible(
                f"这条动画在 {parts} 上一根轴都没有，造不出不闭合")
        for r in rows:
            if r is target:
                yield (r[0], r[1], r[2], r[3] + delta)
            else:
                yield r
    return wrapped


def break_chain_by(sample_mine, part: str = "rightArm", axis: str = "pitch",
                   delta: float = 0.2):
    """把被测动画的一条轴挪开 —— chain 门该抓的。"""
    def wrapped(p, a, t):
        v = sample_mine(p, a, t)
        return v + delta if (p == part and a == axis) else v
    return wrapped


def soften_easing_by(easings, load_tick):
    """把发力段的 easing 换成 OUT 族 —— easing 门该抓的。"""
    def wrapped():
        out = dict(easings)
        out[load_tick] = "OUTQUAD"
        return out
    return wrapped


def clone_other_by(sample_a):
    """把对照动画换成自己 —— distinct 门该抓的「两招长一个样」。"""
    return sample_a


def race_tail_by(item_at, ticks, jump: float = 40.0):
    """把整条动画冻住、只在最后一格把刀尖甩出去 —— decel 门该抓的「被掐断」。

    不能写成「在原速度上给末段乘个系数」：转刀那种全程匀速的动作，乘 2 之后末格
    占比也只到 40%，仍在门限内 —— 又是一个「效果取决于被注入姿态」的假注入器。
    冻住前面等于把峰速压到 0 附近，末格必然独占峰值。
    """
    last, prev = ticks[-1], ticks[-2]

    def wrapped(t):
        g, tip, b = item_at(0.0)
        if t > prev + 1e-9:
            off = np.array([0.0, 0.0, -jump])
            return g + off, tip + off, b + off
        return g, tip, b
    return wrapped


def kill_blendout_by(emote):
    """把 stopTick 压到 endTick —— blendout 门该抓的「当场断电」。"""
    out = dict(emote)
    out["stopTick"] = out.get("endTick", 0)
    return out


def freeze_flip_by(item_at, at: float = 0.0):
    """刃向焊死不动 —— flip 门该抓的「握姿根本没换」。"""
    def wrapped(t):
        return item_at(at)
    return wrapped


def linearize_peak_by(item_at, ticks, hit_tick):
    """把峰速搬到收招段 —— peak 门该抓的：撞击后再猛走一段。"""
    span = ticks[-1] - hit_tick

    def wrapped(t):
        # 撞击前**整段冻住**（不是"取 min(t, hit)"——那样发力段的原速度还在，
        # 峰值照样落在 (load, hit]，注入等于没做）。收招段再猛推一把。
        g, tip, b = item_at(0.0)
        if t > hit_tick and span > 0:
            off = np.array([0.0, 0.0, -30.0 * (t - hit_tick) / span])
            return g + off, tip + off, b + off
        return g, tip, b
    return wrapped


# ── 声明 ────────────────────────────────────────────────────────────────────


@dataclass
class KnifeGates:
    """一条匕首动画的门禁声明。"""

    take: KnifeTake
    load_tick: float | None = None
    hit_tick: float | None = None
    expect_flip: bool = False
    chain_links: tuple = ()   # (自己的 tick, 对照动画名, 对照 tick)
    chain_tol: float = GUARD_TOL
    distinct_from: str | None = None
    tip_ceil: float = TIP_CEIL
    tip_since: float | None = None
    elev_ceil: float = ELEV_CEIL
    elev_since: float | None = None
    behind_max: float = BEHIND_MAX
    behind_until: float | None = None
    reach_min: float = REACH_MIN
    elbow_min: float | None = ELBOW_MIN_DEG
    elbow_until: float | None = None
    skips: tuple = ()          # (门名, 不适用的理由) —— report 里照样打出来
    guard_parts: tuple | None = None
    flip_min: float = FLIP_MIN_DEG
    flip_travel: float = FLIP_TRAVEL_MAX
    extra: dict = field(default_factory=dict)

    @property
    def title(self) -> str:
        return self.take.name

    def specs(self):
        tk, ticks = self.take, self.take.ticks
        out = [
            (lambda src=None: gate_torch(src or tk.item_at, ticks, self.tip_ceil,
                                         self.tip_since),
             lambda: lift_item_above(tk.item_at)),
            (lambda src=None: gate_elev(src or tk.item_at, ticks, self.elev_ceil,
                                        self.elev_since),
             lambda: tilt_blade_up(tk.item_at)),
            (lambda src=None: gate_behind(src or tk.item_at, tk.torso_frame_at, ticks,
                                          self.behind_max, self.behind_until),
             lambda: shove_item_behind(tk.item_at)),
            (lambda src=None: gate_head(src or tk.item_at, tk.limb_segments_at,
                                        tk.head_frame_at, ticks),
             lambda: shove_item_into_head(tk.item_at)),
            (lambda src=None: gate_selfclip(src or tk.item_at, tk.body_frames_at, ticks),
             lambda: shove_item_into_torso(tk.item_at)),
            (lambda src=None: gate_teleport(src or tk.item_at, ticks),
             lambda: jump_tip_by(tk.item_at, ticks[len(ticks) // 2])),
            (lambda src=None: gate_smooth(src or tk.item_at, tk.key_ticks(), ticks),
             lambda: bulge_path_by(tk.item_at, tk.key_ticks())),
            (lambda src=None: gate_dir_corridor(src or tk.item_at, tk.key_ticks(), ticks),
             lambda: detour_dir_by(tk.item_at, tk.key_ticks())),

            (lambda src=None: gate_decel(src or tk.item_at, ticks),
             lambda: race_tail_by(tk.item_at, ticks)),
            (lambda src=None: gate_blendout(src or tk.emote),
             lambda: kill_blendout_by(tk.emote)),
        ]
        if self.elbow_min is not None:
            out.append((lambda src=None: gate_elbow(src or tk.bend_deg_at, ticks,
                                                    self.elbow_min, self.elbow_until),
                        lambda: straighten_elbow_by(tk.bend_deg_at)))
        if self.chain_links:
            out.append((lambda src=None: gate_chain(
                src or tk.sample, tk.axes, self.chain_links, self.chain_tol),
                lambda: break_chain_by(tk.sample, delta=self.chain_tol * 2 + 0.2)))
        if self.guard_parts is not None or not self.chain_links:
            out.append((lambda src=None: gate_guard(src or tk.axis_pairs,
                                                    parts=self.guard_parts),
                        lambda: break_guard_by(tk.axis_pairs, parts=self.guard_parts)))
        if self.load_tick is not None and self.hit_tick is not None:
            out.append((lambda src=None: gate_reach(
                src or tk.item_at, self.load_tick, self.hit_tick, self.reach_min),
                lambda: flatten_reach_by(tk.item_at, self.load_tick)))
            out.append((lambda src=None: gate_peak(
                src or tk.item_at, ticks, self.load_tick, self.hit_tick),
                lambda: linearize_peak_by(tk.item_at, ticks, self.hit_tick)))
        if self.load_tick is not None:
            out.append((lambda src=None: gate_easing(
                src() if callable(src) else (src or tk.easings()), self.load_tick),
                lambda: soften_easing_by(tk.easings(), self.load_tick)))
        if self.distinct_from:
            other = KnifeTake(self.distinct_from, DAGGER_MODEL)
            out.append((lambda src=None: gate_distinct(
                tk.sample, src or other.sample, self.distinct_from),
                lambda: clone_other_by(tk.sample)))
        if self.expect_flip:
            out.append((lambda src=None: gate_flip(src or tk.item_at, ticks,
                                                   self.flip_min, self.flip_travel),
                        lambda: freeze_flip_by(tk.item_at)))
        return tuple(out)

    def run_all(self):
        return [fn() for fn, _ in self.specs()]

    def report(self) -> int:
        print(f"{self.title} 刀法后验:")
        bad = 0
        for g in self.run_all():
            bad += 0 if g.ok else 1
            print(f"  {'✓' if g.ok else '✗'} {g.label}: {g.detail}")
        # 被声明「不适用」的门要照样打出来。悄悄少一行，下一个人只会以为它过了 —— 
        # 门禁最怕的不是红，是消失。
        for label, why in self.skips:
            print(f"  ⊘ {label}: 不适用 —— {why}")
        print(f"  → {bad} 道门未过"
              + (f"，{len(self.skips)} 道声明不适用" if self.skips else ""))
        return bad

    def self_test(self, *, verbose: bool = True) -> int:
        if verbose:
            print(f"{self.title} 门差分自证:")
        broken = 0
        for fn, injector in self.specs():
            clean = fn()
            if not clean.ok:
                broken += 1
                if verbose:
                    print(f"  ✗ {clean.label}: 干净动画上就没过（{clean.detail}），无从谈鉴别力")
                continue
            try:
                hit = fn(injector())
            except InjectionImpossible as exc:
                broken += 1
                if verbose:
                    print(f"  ✗ {clean.label}: 造不出缺陷 —— {exc}")
                continue
            if hit.ok:
                broken += 1
                if verbose:
                    print(f"  ✗ {clean.label}: 注入缺陷后仍然过（{hit.detail}）—— **没有鉴别力**")
            elif verbose:
                print(f"  ✓ {clean.label}: 干净过 / 注入后报「{hit.detail}」")
        if verbose:
            print(f"  → {broken} 道门失效")
        return broken


# ── 本仓四条匕首动画的声明 ──────────────────────────────────────────────────

DAGGER_MODEL = _REPO / "modelScript" / "models" / "IronDagger.bbmodel"

# 首末帧必须逐轴闭合的部位。2026-08-31 用户在 Blockbench 里重摆了四条动画的首末两帧：
# 每条都收在**跟随动作**上（直刺收在伸展、下劈收在低位、反握上撕收在右后），手臂两端
# 按设计就不相等；但他一根没动下盘 —— body / torso / head / 两腿在首末帧仍逐字相同。
# 所以「收势闭合」这条要求收在下盘上（那里它仍然成立、仍然有 teeth），手臂那一头改由
# `gate_decel`（末格不许还是峰速）+ `gate_blendout`（必须留出混出段）接手。
LOWER_PARTS = ("body", "torso", "head", "rightLeg", "leftLeg")

SUITE = {
    # 直刺：用户把末帧摆成完全伸展（bend 6.9°），肘那条门收在收势之前
    "dagger_stab": dict(load_tick=3.0, hit_tick=5.0, distinct_from="dagger_slash",
                        guard_parts=LOWER_PARTS, elbow_until=6.0),
    # 过顶下劈：起手式是刃朝天的高架（用户手摆，刀尖 y=41.1、仰角 +68°），刀尖/仰角
    # 两条门因此从撞击帧起算 —— 蓄势段举刀是设计，劈完还举着才是败笔。
    # 肘：整条是直臂挥（末帧 bend 1.6°），门收在蓄势段（实测 12~21°）。
    "dagger_slash": dict(load_tick=3.0, hit_tick=5.0, distinct_from="dagger_reverse_slash",
                         guard_parts=LOWER_PARTS,
                         tip_since=5.0, elev_since=5.0,
                         elbow_min=10.0, elbow_until=3.0),
    # 反握上撕：整条是刃自胸前下垂 → 扫过正下方 → 向右后撕开，**刃越过背面是招式本身**
    # （用户手摆的末帧刀尖在体坐标 z=+13.2）。绕背那条门因此收在弧线底之前（t≤4，实测
    # 窗口内最深 −5.9，余量 8.9px）—— 它当初要抓的「蓄势时刀尖绕到后脑勺」（返工前
    # z=+8.9）正落在这个窗口里，teeth 没丢。窗口之后由 `gate_selfclip` / `gate_head` 看。
    "dagger_reverse_slash": dict(load_tick=3.0, hit_tick=5.0, distinct_from="dagger_stab",
                                 guard_parts=LOWER_PARTS,
                                 behind_until=4.0, elbow_until=6.0),
    # 转刀不是发力招：没有蓄势→撞击段，换的是握姿。
    "dagger_grip_switch": dict(expect_flip=True, guard_parts=LOWER_PARTS),
}


def build(name: str, bbmodel: Path | None = None) -> KnifeGates:
    return KnifeGates(KnifeTake(name, bbmodel or DAGGER_MODEL), **SUITE[name])


def main(argv=None) -> int:
    import argparse
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("anims", nargs="*", default=sorted(SUITE), help="动画名，缺省跑全套")
    ap.add_argument("--model", type=Path, default=DAGGER_MODEL)
    ap.add_argument("--self-test", action="store_true", help="跑差分自证而不是后验")
    args = ap.parse_args(argv)
    bad = 0
    for name in (args.anims or sorted(SUITE)):
        g = build(name, args.model)
        bad += g.self_test() if args.self_test else g.report()
        print()
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
