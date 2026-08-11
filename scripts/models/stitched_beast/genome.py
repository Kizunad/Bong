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
from dataclasses import dataclass

import core as C

# ---------------------------------------------------------------- 部件模板
# 节段长度（MC 像素，16 = 1 格）。蛛足直接取拟态灰烬蛛的实测比例
# （scripts/models/mimic_spider/gen_frame.py：基节 2.5 / 腿节 10 / 胫节 13 / 跗节 5.5）。
LIMB_TEMPLATES: dict[str, tuple[float, ...]] = {
    "spider": (2.5, 10.0, 13.0, 5.5),      # 细长多节，摆得慢、跨得远
    "mammal": (11.0, 10.0, 6.0, 2.5),      # 猪/狼那类，粗壮承重
    "bird": (6.0, 12.0, 10.0, 4.0),        # 鸡/鹅那类，膝反折
    "tentacle": (5.0, 5.0, 5.0, 5.0, 5.0, 5.0),   # 吞进来没长成腿的东西，不承重
    "vestigial": (4.0, 3.0),               # 退化残肢，够不着地
}

# 承重与否是**几何事实**不是标签：够不着地的肢体不参与步态求解。
LOAD_BEARING: frozenset[str] = frozenset({"spider", "mammal", "bird"})

# 供体：这条肢是从**哪只死掉的野兽**身上拆的，尺寸乘数即那个物种的体型。
#
# 这不是装饰性设定，是运动层能不能出错拍的前提。肢体自然摆动频率 f ∝ 1/√L，
# 一个身体周期内的步数 n = round(f/f_min)——要让某条肢一周期迈两步，它得比最慢的
# 那条**短一半以上**（频率差 ≥1.5 ⇒ 长度差 ≥2.25）。只靠 ±35% 的 scale 抖动做不到，
# 实测所有肢都落进 0.38-0.48 Hz，n 全等于 1，走出来是齐步——那就不是缝合兽了。
# 鼠腿和牛腿差三四倍，这个差距是正典自带的（"由几只普通野兽互相吞噬融合而成"）。
LIMB_SOURCES: dict[str, dict[str, float]] = {
    "mammal": {"rat": 0.42, "rabbit": 0.55, "fox": 0.78, "goat": 0.92,
               "wolf": 1.00, "pig": 1.12, "cow": 1.55},
    "bird": {"chicken": 0.52, "goose": 0.85, "vulture": 1.15},
    "spider": {"hatchling": 0.60, "ash_spider": 1.05, "brood": 1.45},
    "tentacle": {"unknown": 1.0},
    "vestigial": {"unknown": 1.0},
}

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
    kind: str
    scale: float           # 个体差异抖动
    source: str = "unknown"   # 供体物种；体型乘数见 LIMB_SOURCES

    @property
    def size(self) -> float:
        """实际尺寸乘数 = 供体体型 × 个体抖动。"""
        return LIMB_SOURCES[self.kind].get(self.source, 1.0) * self.scale

    @property
    def segments(self) -> tuple[float, ...]:
        return tuple(x * self.size for x in LIMB_TEMPLATES[self.kind])

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
            rows.append(f"    {lg.socket:<10} {lg.kind:<9} {lg.source:<10} "
                        f"×{lg.size:.2f}  总长 {lg.length:5.1f}  {mark}")
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

    长度差异主要由**供体物种**给（LIMB_SOURCES，0.42-1.55），scale 只做 ±15% 的个体
    抖动。这个分工是必须的：只靠 scale 抖动做不出 2.25 倍以上的长度差，所有肢会落进
    同一个频段、每条一周期都迈一步，走出来是齐步——那是蜘蛛不是缝合兽。
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
        src = _pick(seed, f"src{i}", sorted(LIMB_SOURCES[kind]))
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
