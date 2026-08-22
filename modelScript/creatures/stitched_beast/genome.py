#!/usr/bin/env python3
"""异变缝合兽 —— 基因组：一只兽由哪些捡来的部件拼成。

缝合兽和其它生物的根本区别是**它没有固定形态**。正典（《异兽三形考》§兽·噬）里那头
幼兽只有三条腿，第四条是从一只路过被杀死的野狗尸体上"借"的，花了约七日长进去。所以
这条流水线产出的不是一个模型，是一个**生成器**：核心固定，头颅与肢体由基因组装配。

本模块只管**数据**——挑哪些槽、挂哪种部件、多大。几何在部件层，步态在 locomotion。
这样运动层不必等部件库齐活就能先算：摆动频率只跟节段长度有关，跟贴图长什么样无关。

两条硬约束写死在采样器里，不靠调参数碰运气：

  · **至少 3 条承重肢**，否则站不住（静态稳定至少要 3 个支撑点）
  · **禁止左右镜像对称**：镜像的肢体配置会走出整齐步态，那就不是缝合兽了

头颅取材对齐 server 的 `MundaneFaunaKind`（牛/猪/羊/鸡/兔/山羊/蛙/狐/狼）+ 噬元鼠——
缝合兽正是吞噬这些普通野兽融成的，头颅名单不该另起一套。
"""

from __future__ import annotations

import hashlib
import math
from dataclasses import dataclass

import core as C

# ---------------------------------------------------------------- 骨架
# **一条腿先是一副骨架。** 上一版只有"哺乳类/禽类/蛛类"三张模板，狼腿羊腿猪腿全是同一
# 组比例乘一个体型系数——渲出来六条腿一个剪影，看不出是从谁身上拆的（用户实测反馈）。
# 现在每个供体物种一副真骨架，四根骨各自的长度是观察值（真实动物的股骨:胫骨:掌骨:趾骨
# 比例），不是设计值。
#
# 决定剪影的**不只是长度，更是站姿类别**——同样四根骨，跖行（人、鼠：整个脚掌贴地）、
# 趾行（狼、狐：脚跟抬起、踮着趾走）、蹄行（羊、牛：掌骨竖成一根管，只有蹄尖着地）
# 站出来完全是三种腿。这是解剖学分类，不是造型选择。
LIMB_CLASS: dict[str, str] = {}          # 物种 → 大类（折向与摆动模型按大类走）

# **掌骨相对地面的倾角**——这一个数字就把三类腿分开了：
#   · 蹄行 掌骨竖成一根管，只有蹄尖着地 ⇒ 踝抬得最高，小腿以下是一根细杆
#   · 趾行 脚跟离地、踮着趾走 ⇒ 中间那个"向后折的关节"其实是踝
#   · 跖行 整个脚掌贴地 ⇒ 踝低、脚后跟支出来一块
# 运动层也要它：**腿只连到踝，不连到地面**。蹄行动物的踝比趾行高一大截，可达半径
# 和骑乘高度都得按髋到踝算——按髋到地面算会以为羊腿能够到远得多的地方。
META_DEG: dict[str, float] = {
    "unguligrade": 80.0, "digitigrade": 52.0, "plantigrade": 22.0, "sprawling": 62.0,
}


@dataclass(frozen=True)
class Skeleton:
    """一条腿的骨架。`bones` 里是**骨**的长度（px），不是外形粗细——粗细在 limbs 层
    由载荷算。`stance` 决定哪几根骨着地，`foot`/`coat` 决定脚和体表长什么样。

    最后两根骨永远是"脚"（掌骨 + 趾/蹄），由站姿摆放；其余是"腿"，由 IK 解到踝。
    """

    cls: str        # mammal / bird / spider —— 关节折向按大类
    stance: str     # plantigrade 跖行 / digitigrade 趾行 / unguligrade 蹄行 / sprawling 外展
    bones: tuple[tuple[str, float], ...]
    foot: str       # paw 肉垫爪 / cloven 偶蹄 / human 人足 / bird 禽爪 / claw 蛛钩
    coat: str       # fur 毛 / wool 羊毛 / bristle 鬃 / hide 皮 / scute 鳞 / chitin 甲 / bare 裸

    @property
    def lengths(self) -> tuple[float, ...]:
        return tuple(L for _n, L in self.bones)

    @property
    def total(self) -> float:
        return sum(self.lengths)


def _sk(cls, stance, foot, coat, **bones) -> Skeleton:
    return Skeleton(cls, stance, tuple(bones.items()), foot, coat)


# 骨名用中文对照：股骨 femur / 胫骨 tibia / 掌骨 meta（蹄行动物即"管骨"）/ 趾骨 toes。
# 禽腿的第二节其实是胫跗骨、第三节是跗跖骨，"反折的膝"是踝——名字沿用同一套位置。
SKELETONS: dict[str, Skeleton] = {
    # —— 兽腿：跖行
    "rat":    _sk("mammal", "plantigrade", "paw", "fur",
                  femur=5.0, tibia=6.0, meta=4.5, toes=1.5),
    "human":  _sk("mammal", "plantigrade", "human", "bare",
                  femur=13.0, tibia=12.5, meta=7.0, toes=3.5),
    # —— 兽腿：趾行（脚跟抬起，"后弯的膝"其实是跗关节）
    "rabbit": _sk("mammal", "digitigrade", "paw", "fur",
                  femur=6.5, tibia=9.5, meta=8.5, toes=3.0),
    "fox":    _sk("mammal", "digitigrade", "paw", "fur",
                  femur=9.0, tibia=10.0, meta=8.5, toes=3.0),
    "wolf":   _sk("mammal", "digitigrade", "paw", "fur",
                  femur=11.5, tibia=12.5, meta=10.0, toes=3.5),
    # —— 兽腿：蹄行（掌骨竖成一根管，只有蹄尖着地）
    "pig":    _sk("mammal", "unguligrade", "cloven", "bristle",
                  femur=8.0, tibia=8.0, meta=6.5, toes=2.5),
    "sheep":  _sk("mammal", "unguligrade", "cloven", "wool",
                  femur=8.5, tibia=10.0, meta=9.0, toes=2.5),
    "goat":   _sk("mammal", "unguligrade", "cloven", "wool",
                  femur=9.0, tibia=11.0, meta=10.0, toes=2.5),
    "cow":    _sk("mammal", "unguligrade", "cloven", "hide",
                  femur=12.5, tibia=14.0, meta=12.5, toes=3.5),
    # —— 禽腿：趾行，跗跖骨细长裸露
    "chicken": _sk("bird", "digitigrade", "bird", "plume",
                   femur=4.5, tibia=8.5, meta=6.5, toes=3.5),
    "goose":   _sk("bird", "digitigrade", "bird", "plume",
                   femur=5.5, tibia=11.0, meta=8.5, toes=4.0),
    "vulture": _sk("bird", "digitigrade", "bird", "plume",
                   femur=6.5, tibia=13.0, meta=10.0, toes=4.5),
    # —— 蛛足：外展，基节极短（这条短基节正是它杠杆最差、肌肉塞不进腿里的原因）
    "hatchling":  _sk("spider", "sprawling", "claw", "chitin",
                      coxa=2.0, femur=7.0, tibia=8.5, tarsus=3.5),
    "ash_spider": _sk("spider", "sprawling", "claw", "chitin",
                      coxa=2.5, femur=9.5, tibia=12.0, tarsus=4.5),
    "brood":      _sk("spider", "sprawling", "claw", "chitin",
                      coxa=3.0, femur=13.0, tibia=16.0, tarsus=6.0),
}
for _n, _sk_ in SKELETONS.items():
    LIMB_CLASS[_n] = _sk_.cls

# 没有骨架的两类：吞进来没长成腿的东西 / 退化残肢。它们不承重，形状另有来源
# （粗细走组织预算，姿态是自由铰或软梁——见 limbs.py）。
BONELESS: dict[str, tuple[float, ...]] = {
    "tentacle": (5.0, 5.0, 5.0, 5.0, 5.0, 5.0),
    "vestigial": (4.0, 3.0),
}

# 承重与否是**几何事实**不是标签：够不着地的肢体不参与步态求解。
LOAD_BEARING: frozenset[str] = frozenset({"spider", "mammal", "bird"})

# 每个大类下可选的供体物种。长度差异现在直接来自骨架本身（最短 17.0 的鼠腿到最长
# 42.5 的巢蛛足，2.5 倍），不再需要一张体型乘数表——那张表原本是用来补"三张模板不够
# 分化"的，骨架分了之后它就是重复的。
LIMB_SOURCES: dict[str, list[str]] = {
    cls: sorted(n for n, sk in SKELETONS.items() if sk.cls == cls)
    for cls in ("mammal", "bird", "spider")
}
LIMB_SOURCES["tentacle"] = ["unknown"]
LIMB_SOURCES["vestigial"] = ["unknown"]

# 头颅库。对齐 server MundaneFaunaKind + 噬元鼠；(长, 高, 宽) 特征尺寸。
HEAD_TEMPLATES: dict[str, tuple[float, float, float]] = {
    "rat": (7.0, 5.0, 5.0),
    "wolf": (11.0, 8.0, 7.0),
    "pig": (10.0, 8.0, 8.0),
    "cow": (13.0, 10.0, 9.0),
    "sheep": (9.0, 8.0, 7.0),
    "goat": (9.0, 8.0, 6.5),
    "chicken": (6.0, 6.0, 4.5),
    "rabbit": (6.5, 5.5, 4.5),
    "fox": (9.0, 6.5, 6.0),
    "frog": (7.5, 5.0, 8.0),
}


@dataclass(frozen=True)
class LimbGene:
    socket: str
    kind: str              # 大类 mammal / bird / spider / tentacle / vestigial
    scale: float           # 个体差异抖动
    source: str = "unknown"   # 供体物种 —— 骨架就是按它查的，见 SKELETONS

    @property
    def size(self) -> float:
        return self.scale

    @property
    def skeleton(self) -> Skeleton | None:
        """这条肢的骨架；触手/退化残肢没有骨架，返回 None。"""
        return SKELETONS.get(self.source)

    @property
    def stance(self) -> str:
        sk = self.skeleton
        return sk.stance if sk else "none"

    @property
    def segments(self) -> tuple[float, ...]:
        sk = self.skeleton
        base = sk.lengths if sk else BONELESS[self.kind]
        return tuple(x * self.scale for x in base)

    @property
    def foot_bones(self) -> int:
        """最后几根骨归"脚"，由站姿摆放而不是 IK 解。外展只有跗节一根。"""
        return 1 if self.stance == "sprawling" else 2

    @property
    def leg_len(self) -> float:
        """髋到踝的骨长——**可达半径要用它，不是总长**。"""
        sk = self.skeleton
        if sk is None:
            return self.length
        return sum(self.segments[:len(self.segments) - self.foot_bones])

    @property
    def ankle_lift(self) -> float:
        """踝离地多高。蹄行 ≈ 掌骨全长 + 蹄，跖行只有掌骨的三成多一点。"""
        sk = self.skeleton
        if sk is None:
            return 0.0
        segs = self.segments[len(self.segments) - self.foot_bones:]
        th = math.radians(META_DEG[sk.stance])
        if sk.stance == "sprawling":
            return segs[0] * math.sin(th)
        meta, toes = segs
        if sk.stance == "unguligrade":
            return toes + meta * math.sin(th)
        if sk.stance == "digitigrade":
            return meta * math.sin(th)
        return meta * math.sin(th) + 0.6

    @property
    def ankle_back(self) -> float:
        """踝相对接触点往**身后**退多远。掌骨越斜退得越多（趾行退 0.62 个掌骨长）。

        可达半径必须把它算进去：它和径向外展是两个正交方向，勾股里各占一项。漏掉它
        腿就够不着自己的踝，IK 无解退回直链，关节落到地面以下（实测 y=−4.13）。
        """
        sk = self.skeleton
        if sk is None or sk.stance == "sprawling":
            return 0.0
        meta = self.segments[len(self.segments) - self.foot_bones]
        th = math.radians(META_DEG[sk.stance])
        return meta * math.cos(th) * (0.25 if sk.stance == "plantigrade" else 1.0)

    @property
    def bone_names(self) -> tuple[str, ...]:
        sk = self.skeleton
        return tuple(n for n, _L in sk.bones) if sk else ()

    @property
    def length(self) -> float:
        return sum(self.segments)

    @property
    def load_bearing(self) -> bool:
        return self.kind in LOAD_BEARING


@dataclass(frozen=True)
class HeadGene:
    socket: str
    kind: str
    scale: float

    @property
    def size(self) -> tuple[float, float, float]:
        return tuple(x * self.scale for x in HEAD_TEMPLATES[self.kind])  # type: ignore[return-value]


@dataclass(frozen=True)
class Genome:
    seed: int
    limbs: tuple[LimbGene, ...]
    heads: tuple[HeadGene, ...]

    @property
    def load_limbs(self) -> tuple[LimbGene, ...]:
        return tuple(lg for lg in self.limbs if lg.load_bearing)

    def describe(self) -> str:
        rows = [f"基因组 seed={self.seed}"]
        rows.append(f"  肢 ×{len(self.limbs)}（承重 {len(self.load_limbs)}）")
        for lg in self.limbs:
            mark = "承重" if lg.load_bearing else "  — "
            sk = lg.skeleton
            rows.append(f"    {lg.socket:<10} {lg.source:<11}{lg.stance:<13}"
                        f"{(sk.foot + '/' + sk.coat) if sk else '—':<14}"
                        f"×{lg.scale:.2f}  总长 {lg.length:5.1f}  {mark}")
        rows.append(f"  头 ×{len(self.heads)}")
        for hg in self.heads:
            rows.append(f"    {hg.socket:<10} {hg.kind:<9} ×{hg.scale:.2f}  "
                        f"{hg.size[0]:.1f}×{hg.size[1]:.1f}×{hg.size[2]:.1f}")
        return "\n".join(rows)


# ---------------------------------------------------------------- 采样
def _rand(seed: int, *parts) -> float:
    """确定性 [0,1)。同 seed 必得同一只兽——出了问题要能复现那一只。"""
    h = hashlib.md5((f"{seed}|" + "|".join(str(p) for p in parts)).encode()).digest()
    return int.from_bytes(h[:4], "big") / 2**32


def _pick(seed: int, tag: str, items: list):
    return items[int(_rand(seed, tag) * len(items)) % len(items)]


def sample(seed: int, *, socks: dict[str, C.Socket] | None = None) -> Genome:
    """按 seed 装配一只兽。

    肢体数 3-6 条承重 + 0-2 条非承重；头 1-3 个。

    长度差异由**供体骨架**给（17.0 的鼠腿到 42.5 的牛腿，2.5 倍），scale 只做 ±15%
    的个体抖动。这个分工是必须的：只靠 scale 抖动做不出 2.25 倍以上的长度差，所有肢
    会落进同一个频段、每条一周期都迈一步，走出来是齐步——那是蜘蛛不是缝合兽。
    """
    socks = socks or C.sockets()
    limb_slots = sorted(s for s, v in socks.items() if v.kind == "limb")
    head_slots = sorted(s for s, v in socks.items() if v.kind == "head")
    vest_slots = sorted(s for s, v in socks.items() if v.kind == "vestige")

    n_load = 3 + int(_rand(seed, "nload") * 4)          # 3..6
    n_extra = int(_rand(seed, "nextra") * 3)            # 0..2
    n_head = 1 + int(_rand(seed, "nhead") * 3)          # 1..3

    order = sorted(limb_slots, key=lambda s: _rand(seed, "ord", s))
    limbs: list[LimbGene] = []
    for i, slot in enumerate(order[:n_load]):
        kind = _pick(seed, f"k{i}", sorted(LOAD_BEARING))
        src = _pick(seed, f"src{i}", LIMB_SOURCES[kind])
        scale = 0.85 + _rand(seed, "sc", slot) * 0.3
        limbs.append(LimbGene(slot, kind, round(scale, 3), src))
    for i, slot in enumerate(order[n_load:n_load + n_extra]):
        kind = _pick(seed, f"x{i}", ["tentacle", "vestigial"])
        limbs.append(LimbGene(slot, kind, round(0.7 + _rand(seed, "xs", slot) * 0.6, 3)))
    # 退化残肢挂在退化槽上——那是吞下去没长完的东西留下的疤，本来就该有
    for i, slot in enumerate(sorted(vest_slots, key=lambda s: _rand(seed, "v", s))[:1]):
        limbs.append(LimbGene(slot, "vestigial", round(0.6 + _rand(seed, "vs", slot) * 0.7, 3)))

    heads: list[HeadGene] = []
    for i, slot in enumerate(sorted(head_slots, key=lambda s: _rand(seed, "h", s))[:n_head]):
        kind = _pick(seed, f"hk{i}", sorted(HEAD_TEMPLATES))
        heads.append(HeadGene(slot, kind, round(0.75 + _rand(seed, "hs", slot) * 0.6, 3)))

    g = Genome(seed, tuple(limbs), tuple(heads))
    problems = validate(g, socks)
    if problems:
        raise ValueError(f"seed={seed} 生成的基因组不合法：{'; '.join(problems)}")
    return g


def validate(g: Genome, socks: dict[str, C.Socket] | None = None) -> list[str]:
    """基因组合法性。这些不是"最好满足"，是"不满足就站不住 / 就不是缝合兽"。"""
    socks = socks or C.sockets()
    bad: list[str] = []
    if len(g.load_limbs) < 3:
        bad.append(f"承重肢只有 {len(g.load_limbs)} 条——静态稳定至少要 3 个支撑点")
    if not g.heads:
        bad.append("一个头都没有")
    seen = [lg.socket for lg in g.limbs] + [hg.socket for hg in g.heads]
    if len(seen) != len(set(seen)):
        bad.append("同一个挂载点挂了两个部件，必然穿模")
    for name in seen:
        if name not in socks:
            bad.append(f"挂到不存在的槽 {name}")

    # 左右镜像对称 = 走得出整齐步态 = 不是缝合兽。按 _l/_r 配对检查承重肢。
    def mate(s: str) -> str:
        return s[:-1] + ("r" if s.endswith("l") else "l")

    load = {lg.socket: lg for lg in g.load_limbs}
    paired = [s for s in load if mate(s) in load]
    if paired and all(abs(load[s].length - load[mate(s)].length) < 1.5
                      and load[s].kind == load[mate(s)].kind for s in paired):
        bad.append("左右承重肢完全镜像——会走出整齐步态，缝合兽不该对称")

    # 承重肢长度必须拉开跨度：全部等长 ⇒ 自然频率相同 ⇒ 每条肢一周期都迈一步 ⇒ 齐步走。
    # 错拍步态是缝合兽的招牌，这条断言把它从"碰运气"变成"采样保证"。
    if len(g.load_limbs) >= 2:
        ls = [lg.length for lg in g.load_limbs]
        if max(ls) / max(min(ls), 1e-6) < 1.6:
            bad.append(f"承重肢长度跨度仅 {max(ls) / min(ls):.2f}×——太齐，走不出错拍")
    return bad


if __name__ == "__main__":
    socks = C.sockets()
    for seed in (1, 7, 42):
        print(sample(seed, socks=socks).describe())
        print()
