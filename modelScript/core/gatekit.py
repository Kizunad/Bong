#!/usr/bin/env python3
"""几何自检门的共享库，**每道门自带缺陷注入器**。

抽取自 `gen_grass_pouch.py` 的七道门（仓库里最全的一份），原本三个生成器各手抄一遍。
手抄一次几十行，这就是为什么 34 个生成器里只有 3 个有门；变成 `AssetGates(...).report()`
一行之后，覆盖率才有可能上去。

**为什么每道门旁边必须就是它的注入器。** 判据本身会假绿，而模型不会怀疑它 —— 两个实证：

  · 某版 `_interpenetrating()` 的材质白名单是反的（把「柱头扎穿皮盖」这个正要抓的缺陷
    放行，却去抓合法的 bamboo×weave）。结果**坏版本和修好的版本都报 17 处违例** ——
    零区分力，而两边都「有输出、看起来在工作」。
  · 第一版编带判据统计「明度翻转」，对 11 条带数出 82 flips —— 数的是贴图颗粒不是编缝。
    换成按行连通域数暗带后：3/4 视 seam 3460px 分 7 道；抽掉 band_* 件后掉到 264px 分
    2 道（残余是 pocket_band + flap_crease，本来就不叫 band_*）。**13 倍差距才叫有区分力。**

所以 `self_test()` 对每道门做两件事：干净模型上必须报 0（否则这道门已经在自己报警），
注入对应缺陷后必须报出来（否则这道门根本没有鉴别力）。**「自检全绿」在没做差分注入
之前，信息量是零。**

用法：

    GATES = gatekit.AssetGates(
        "小草包 / grass_pouch", MATS,
        asym=ASYM, free_floating={"sprig_a", "sprig_b"},
        soft_over=SOFT_OVER, seats=SEATS, seat_materials={"seam", "stitch", "bone"},
    )
    GATES.report(rig, px=PX, note="...")        # 与手抄版逐字同格式
    GATES.self_test(rig)                        # 差分自证
"""

from __future__ import annotations

import copy
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

sys.path.insert(0, str(Path(__file__).resolve().parent))

from rigkit import Rig, element_bounds, mirror_violations  # noqa: E402

# ---------------------------------------------------------------- 默认门限
# 全部来自实测标定，改之前先读旁边那段理由。
MIN_THICKNESS = 0.2      # 薄于此的件在 16px 空间里渲不出一个像素
CONTACT_TOL = 0.12       # 「第三轴的可见缝隙」容差
CONTACT_OVERLAP = 0.15   # 「两轴有实打实的重叠面」门限
INTERPEN_BITE = 0.55     # 薄贴与真扎进去的分界
BLOCK_SIZE = 16.0        # MC 方块空间边长
BLOCK_TOL = 0.01


@dataclass(frozen=True)
class Seat:
    """贴面件就位规格：某组件沿某轴咬进宿主外表面。

    这道门不做通用碰撞，而是按**构造**直接算咬入深度 —— 因为通用门在这儿靠不住：
      · `interpenetrating` 为了放过同构件的相邻分段而跳过同 bone 组合，而编带与壁同属
        body bone，「带压穿整壁 2.2px」在穿模门里从来没被检查过（差分实测仍报 0）。
      · `floating` 靠 AABB 重叠判接触，而斜插件的轴对齐包围盒很胖；侧袋针脚整排平移
        0.9px 飘在兜外，却因为「碰到斜插草药的包围盒」被判成搭上了。旋转件的 AABB
        一律高估接触。

    **符号方向是这道门唯一容易写反的地方**，写反了缺陷版报 0、干净版报 2：
    件长在宿主面**外侧**时，「咬合」是两者的**重叠厚度**（宿主面 − 件的内侧面），
    不是件到宿主面的距离。写成后者，「把件往外推」这种明显的脱离反而让数字变小。

    match     —— 件名前缀（匹配不到任何 Seat 的件不查）
    axis      —— 0/1/2
    outward   —— +1 表示件在宿主面的正方向外侧（bite = surface − lo[axis]）；
                 −1 表示负方向外侧（bite = hi[axis] − surface）
    surface   —— 宿主外表面坐标；可给一个吃件的 y 中心的函数（壁随高度收口时用）
    min_bite  —— 咬入下界。小于它 = 浮在外面，渲不出贴合甚至整件飘走
    max_bite  —— 咬入上界（通常 = 壁厚）。大于它 = 扎穿，从内侧能看见一截捅进来。
                 None = 这类件不查扎穿（栓在外面的硬件没有「内壁」可穿）
    exclude   —— 精确件名白名单
    host      —— 报错里怎么称呼宿主面
    """

    match: str
    axis: int
    outward: int
    surface: float | Callable[[float], float]
    min_bite: float
    max_bite: float | None = None
    exclude: tuple[str, ...] = ()
    host: str = "宿主面"

    def surface_at(self, mid: float) -> float:
        return self.surface(mid) if callable(self.surface) else float(self.surface)


@dataclass
class GateResult:
    key: str
    label: str
    violations: list[str]

    @property
    def ok(self) -> bool:
        return not self.violations


def bone_of(rig: Rig, eid: str) -> str:
    for name, b in rig.bones.items():
        if eid in b["children"]:
            return name
    return "?"


def mats_by_color(mats) -> dict[int, str]:
    """rigkit 把 `color` 写成 `材质序号 % 8`。这里照抄同一个映射，别自己另编一套。"""
    return {i % 8: name for i, name in enumerate(mats)}


# ================================================================ 七道门
def gate_orphans(rig: Rig) -> list[str]:
    """没被任何骨骼收养的 element（渲染时会丢）。"""
    owned = {eid for b in rig.bones.values() for eid in b["children"]}
    return [e["name"] for e in rig.elements if e["uuid"] not in owned]


def gate_overflow(rig: Rig, shift=(8.0, 0.0, 8.0), size: float = BLOCK_SIZE,
                  tol: float = BLOCK_TOL) -> list[str]:
    """越出 0..16 方块空间的件（平移后会被 MC 裁掉）。"""
    bad = []
    for el in rig.elements:
        lo, hi = element_bounds([el])
        if any(lo[a] + shift[a] < -tol or hi[a] + shift[a] > size + tol for a in range(3)):
            bad.append(f"{el['name']}: {tuple(round(v, 2) for v in lo)}→"
                       f"{tuple(round(v, 2) for v in hi)}")
    return bad


def gate_degenerate(rig: Rig, min_thickness: float = MIN_THICKNESS) -> list[str]:
    """任一轴薄于门限的件 —— 生图提示词里的「不细于二十分之一」下限。"""
    bad = []
    for el in rig.elements:
        d = [el["to"][i] - el["from"][i] for i in range(3)]
        if min(d) < min_thickness:
            bad.append(f"{el['name']}: {tuple(round(v, 2) for v in d)}")
    return bad


def gate_floating(rig: Rig, free: frozenset = frozenset(),
                  contact_tol: float = CONTACT_TOL,
                  overlap: float = CONTACT_OVERLAP) -> list[str]:
    """悬空件：与其它件无面接触。

    **判据必须三轴一起看**。早先只要「≥2 轴重叠 > 0.15」就算接触，于是一件贴在另一件
    **旁边**、第三轴上明明差着一道缝，也被判成搭上了 —— 骨扣离前檐 0.06px 分离就是这么
    漏过去的；侧袋针脚整排平移 0.9px 飘在兜外，门同样报 0。
    真接触 = 两轴有实打实的重叠面（> overlap）**且**第三轴不许有可见缝隙
    （重叠 > −contact_tol，负值即缝）。

    free 是刻意悬空件的白名单：露头草药的上段伸出兜口（读作「插着的草」）、背带下端
    脱离篓身（读作可套肩）—— 那是设计意图，不是缺陷。
    """
    boxes = [(el["name"], *element_bounds([el])) for el in rig.elements]
    bad = []
    for i, (name, lo, hi) in enumerate(boxes):
        if name in free:
            continue
        touch = False
        for j, (_, lo2, hi2) in enumerate(boxes):
            if i == j:
                continue
            ovs = [min(hi[k], hi2[k]) - max(lo[k], lo2[k]) for k in range(3)]
            if min(ovs) > -contact_tol and sum(1 for o in ovs if o > overlap) >= 2:
                touch = True
                break
        if not touch:
            bad.append(name)
    return bad


def gate_interpenetrating(rig: Rig, colors: dict, soft_over: frozenset = frozenset(),
                          min_bite: float = INTERPEN_BITE,
                          hard_override: Callable[[str, str, str, str], bool] | None = None
                          ) -> list[str]:
    """穿模：跨 bone 的两件在三轴上都实体重叠且体积可观。

    **必须区分「搭接」和「穿模」**：编带压壁、绳压盖、封边罩口、针脚咬壁都是贴合，本来
    就该有薄重叠。判据两条 ——
      1. 只查跨 bone 组合（同 bone 内是同一构件的分段，如 taper/shaft 相邻段）；
      2. 三轴同时重叠且最小重叠深度 > min_bite（真扎进去，不是薄贴）。

    `soft_over` 只放行「软覆盖硬」的设计意图。**关键是留下硬对硬不放行** —— 背篓那轮把
    bamboo×hide（柱头扎穿皮盖，正是要抓的缺陷）错误放行、又去抓合法的 bamboo×weave，
    结果坏版和修好版都报 17 处，门完全没有鉴别力。

    `hard_override(n1, m1, n2, m2)` 返回 True 的组合**强制查**，绕开软覆盖豁免（同材质
    仍豁免）。穿模判据不能只看材质对：捆绳是压在件外的短绳，背带是绕过整个篓身的长带，
    同材质不同构件语义 —— 背篓的左背带正是 cord，会顺着 cord×hide / cord×bamboo 两条
    软覆盖被整体放行。
    """
    items = []
    for el in rig.elements:
        lo, hi = element_bounds([el])
        items.append((el["name"], bone_of(rig, el["uuid"]),
                      colors.get(el["color"], "?"), lo, hi))
    bad = []
    for i, (n1, b1, m1, lo1, hi1) in enumerate(items):
        for n2, b2, m2, lo2, hi2 in items[i + 1:]:
            if b1 == b2:
                continue
            forced = bool(hard_override and hard_override(n1, m1, n2, m2))
            if forced:
                if m1 == m2:
                    continue
            elif frozenset((m1, m2)) in soft_over or m1 == m2:
                continue
            bite = min(min(hi1[k], hi2[k]) - max(lo1[k], lo2[k]) for k in range(3))
            if bite > min_bite:
                bad.append(f"{n1}({m1}) × {n2}({m2}) 互穿 {bite:.2f}px")
    return bad


def gate_seating(rig: Rig, seats, colors: dict, materials: frozenset | None = None) -> list[str]:
    """贴面件就位：按 Seat 规格量咬入深度，太浅 = 浮在外面，太深 = 扎穿内壁。

    一条判据同时封住两种缺陷。匹配不到任何 Seat 的件不查（兜身横缝、盖折痕这类贴的是
    自己那件，另算）。
    """
    bad = []
    for el in rig.elements:
        if materials is not None and colors.get(el["color"], "?") not in materials:
            continue
        name = el["name"]
        seat = next((s for s in seats
                     if name.startswith(s.match) and name not in s.exclude), None)
        if seat is None:
            continue
        lo, hi = element_bounds([el])
        surface = seat.surface_at((lo[1] + hi[1]) / 2)
        bite = (surface - lo[seat.axis]) if seat.outward > 0 else (hi[seat.axis] - surface)
        if bite < seat.min_bite:
            bad.append(f"{name} 没咬住{seat.host}（bite={bite:.2f} < {seat.min_bite}）")
        elif seat.max_bite is not None and bite > seat.max_bite:
            bad.append(f"{name} 扎穿{seat.host} {bite - seat.max_bite:.2f}px"
                       f"（可咬深度 {seat.max_bite}）")
    return bad


def gate_mirror(rig: Rig, asym=()) -> list[str]:
    """对称件左右不镜像。刻意不对称的件按 bone 走白名单排除。"""
    els = [e for e in rig.elements if bone_of(rig, e["uuid"]) not in asym]
    return mirror_violations(els)


# ================================================================ 缺陷注入器
# 每个注入器返回 (改坏的 rig 副本, 受害件名, 一句话说明)。它们必须是**资产无关**的：
# 从 rig 自己查出合适的受害者，而不是硬编某个件名。
class InjectionImpossible(RuntimeError):
    """这份资产上造不出该门要抓的那种缺陷（例如全是同材质，无从造穿模）。"""


def _clone(rig: Rig) -> Rig:
    return copy.deepcopy(rig)


def _translate(el: dict, axis: int, delta: float) -> None:
    for key in ("from", "to", "origin"):
        el[key][axis] += delta


def _pick(rig: Rig, exclude=()) -> dict:
    for el in rig.elements:
        if el["name"] not in exclude:
            return el
    raise InjectionImpossible("rig 里没有可用的 element")


def inject_orphan(rig: Rig, **_):
    r = _clone(rig)
    el = r.elements[-1]
    for b in r.bones.values():
        if el["uuid"] in b["children"]:
            b["children"].remove(el["uuid"])
            break
    else:
        raise InjectionImpossible("末件本来就是孤儿，造不出增量缺陷")
    return r, el["name"], f"把 {el['name']} 从骨骼里摘掉"


def inject_overflow(rig: Rig, **_):
    r = _clone(rig)
    el = max(r.elements, key=lambda e: element_bounds([e])[1][0])
    _translate(el, 0, 20.0)
    return r, el["name"], f"把 {el['name']} 沿 x 推出方块空间 20px"


def inject_degenerate(rig: Rig, min_thickness: float = MIN_THICKNESS, **_):
    r = _clone(rig)
    el = _pick(r)
    el["to"][1] = el["from"][1] + min_thickness / 4.0
    return r, el["name"], f"把 {el['name']} 压成 {min_thickness / 4:.2f}px 薄片"


def inject_floating(rig: Rig, free: frozenset = frozenset(), **_):
    r = _clone(rig)
    el = _pick(r, exclude=free)
    _translate(el, 1, 50.0)
    return r, el["name"], f"把 {el['name']} 抬高 50px 悬空"


def inject_interpenetrating(rig: Rig, colors: dict, soft_over: frozenset = frozenset(),
                            hard_override=None, **_):
    """挑一对**本来就该被查**的跨 bone 硬件，把其中一件挪成同心 —— 重叠必然可观。"""
    r = _clone(rig)
    items = []
    for el in r.elements:
        lo, hi = element_bounds([el])
        items.append((el, bone_of(r, el["uuid"]), colors.get(el["color"], "?"), lo, hi))
    best = None
    for i, (e1, b1, m1, lo1, hi1) in enumerate(items):
        for e2, b2, m2, lo2, hi2 in items[i + 1:]:
            if b1 == b2 or m1 == m2:
                continue
            forced = bool(hard_override and hard_override(e1["name"], m1, e2["name"], m2))
            if not forced and frozenset((m1, m2)) in soft_over:
                continue
            depth = min(min(hi1[k] - lo1[k], hi2[k] - lo2[k]) for k in range(3))
            if best is None or depth > best[0]:
                best = (depth, e1, lo1, hi1, e2, lo2, hi2)
    if best is None:
        raise InjectionImpossible("找不到一对会被穿模门检查的跨 bone 异材质件")
    depth, e1, lo1, hi1, e2, lo2, hi2 = best
    for axis in range(3):
        c1 = (lo1[axis] + hi1[axis]) / 2
        c2 = (lo2[axis] + hi2[axis]) / 2
        _translate(e1, axis, c2 - c1)
    return r, e1["name"], f"把 {e1['name']} 挪进 {e2['name']} 内部（同心，重叠 {depth:.2f}px）"


def inject_seating(rig: Rig, seats=(), colors: dict | None = None,
                   materials: frozenset | None = None, **_):
    """把一个贴面件沿它自己的座轴往外推，咬合变成分离 —— 「侧袋针脚整排飘在兜外」那一档。"""
    r = _clone(rig)
    for el in r.elements:
        if materials is not None and colors is not None \
                and colors.get(el["color"], "?") not in materials:
            continue
        seat = next((s for s in seats
                     if el["name"].startswith(s.match) and el["name"] not in s.exclude), None)
        if seat is None:
            continue
        push = seat.min_bite + 1.0
        _translate(el, seat.axis, push * seat.outward)
        return r, el["name"], f"把 {el['name']} 沿座轴推出 {push:.2f}px（咬合变分离）"
    raise InjectionImpossible("没有件落在任何 Seat 上，造不出就位缺陷")


def inject_mirror(rig: Rig, asym=(), **_):
    r = _clone(rig)
    for el in r.elements:
        if bone_of(r, el["uuid"]) in asym:
            continue
        name = el["name"]
        if name.endswith("_l") or "_l_" in name:
            _translate(el, 0, 0.9)
            return r, name, f"把 {name} 单侧平移 0.9px（左右不再镜像）"
    for el in r.elements:
        if bone_of(r, el["uuid"]) in asym:
            continue
        _translate(el, 0, 0.9)
        return r, el["name"], f"把中线件 {el['name']} 平移 0.9px（不再关于中轴对称）"
    raise InjectionImpossible("没有参与镜像自检的件")


# ================================================================ 组装
@dataclass
class AssetGates:
    """一份资产的门禁声明。门的算法在库里，几何事实由资产自己交代。"""

    title: str
    mats: dict
    asym: tuple = ()
    free_floating: frozenset = frozenset()
    soft_over: frozenset = frozenset()
    hard_override: Callable[[str, str, str, str], bool] | None = None
    seats: tuple = ()
    seat_materials: frozenset | None = None
    block_shift: tuple = (8.0, 0.0, 8.0)
    min_thickness: float = MIN_THICKNESS
    contact_tol: float = CONTACT_TOL
    interpen_bite: float = INTERPEN_BITE
    colors: dict = field(init=False)

    def __post_init__(self) -> None:
        self.colors = mats_by_color(self.mats)

    # ------------------------------------------------------------ 门
    def specs(self):
        """(key, 标签, 门函数, 注入器) 四元组，顺序即报告顺序。"""
        out = [
            ("orphans", "孤儿 element",
             lambda r: gate_orphans(r), inject_orphan),
            ("overflow", "越出 0..16 方块空间",
             lambda r: gate_overflow(r, self.block_shift), inject_overflow),
            ("degenerate", f"退化薄片 (<{self.min_thickness}px)",
             lambda r: gate_degenerate(r, self.min_thickness), inject_degenerate),
            ("floating", "悬空无接触",
             lambda r: gate_floating(r, self.free_floating, self.contact_tol), inject_floating),
            ("interpenetrating", "硬件互穿（穿模）",
             lambda r: gate_interpenetrating(r, self.colors, self.soft_over,
                                             self.interpen_bite, self.hard_override),
             inject_interpenetrating),
        ]
        if self.seats:
            out.append(("seating", "贴面件未就位/扎穿",
                        lambda r: gate_seating(r, self.seats, self.colors, self.seat_materials),
                        inject_seating))
        out.append(("mirror", "对称件左右不镜像",
                    lambda r: gate_mirror(r, self.asym), inject_mirror))
        return tuple(out)

    def _inject_kwargs(self) -> dict:
        return {
            "colors": self.colors,
            "soft_over": self.soft_over,
            "hard_override": self.hard_override,
            "seats": self.seats,
            "materials": self.seat_materials,
            "free": self.free_floating,
            "asym": self.asym,
            "min_thickness": self.min_thickness,
        }

    def run_all(self, rig: Rig) -> list[GateResult]:
        return [GateResult(key, label, fn(rig)) for key, label, fn, _ in self.specs()]

    def total(self, rig: Rig) -> int:
        return sum(len(g.violations) for g in self.run_all(rig))

    # ------------------------------------------------------------ 报告
    def report(self, rig: Rig, *, px: float = 16.0, note: str = "") -> int:
        print(f"{self.title} 自检:")
        lo, hi = rig.bounds()
        dims = tuple(hi[i] - lo[i] for i in range(3))
        print(f"  bbox   : {dims[0]:.1f}×{dims[1]:.1f}×{dims[2]:.1f}px = "
              f"{dims[0] / px:.2f}W × {dims[1] / px:.2f}H × {dims[2] / px:.2f}D 格")
        print(f"  cubes  : {len(rig.elements)}  bones: {len(rig.bones)}")
        used: dict[str, int] = {}
        for el in rig.elements:
            m = self.colors.get(el["color"], "?")
            used[m] = used.get(m, 0) + 1
        print(f"  材质   : {len(used)}/{len(self.mats)} 种在用 — "
              + ", ".join(f"{k}:{v}" for k, v in used.items()))

        total = 0
        for gate in self.run_all(rig):
            total += len(gate.violations)
            mark = "✓" if gate.ok else "✗"
            print(f"  {mark} {gate.label}: {len(gate.violations)}")
            for b in gate.violations[:6]:
                print(f"      - {b}")
        print(f"  → 共 {total} 处违例")
        if note:
            print(f"  {note}")
        return total

    # ------------------------------------------------------------ 差分自证
    def self_test(self, rig: Rig, *, verbose: bool = True) -> int:
        """每道门：干净必须 0，注入对应缺陷后必须报出来。返回失效门数。"""
        if verbose:
            print(f"{self.title} 差分自证:")
        kwargs = self._inject_kwargs()
        broken = 0
        for key, label, fn, injector in self.specs():
            clean = fn(rig)
            try:
                bad_rig, victim, what = injector(rig, **kwargs)
            except InjectionImpossible as exc:
                broken += 1
                if verbose:
                    print(f"  ✗ {label}: 造不出缺陷 —— {exc}")
                continue
            hits = fn(bad_rig)
            if clean:
                broken += 1
                if verbose:
                    print(f"  ✗ {label}: 干净模型上就报了 {len(clean)} 处，"
                          f"这道门已经在自己报警，鉴别力无从谈起")
                continue
            if not hits:
                broken += 1
                if verbose:
                    print(f"  ✗ {label}: {what} —— 门仍报 0，**没有鉴别力**")
                continue
            named = any(victim in h for h in hits)
            if verbose:
                where = f"命中 {victim}" if named else f"报了 {len(hits)} 处但没点到 {victim}"
                print(f"  ✓ {label}: 干净 0 → {what} 后 {len(hits)}（{where}）")
            if not named:
                broken += 1
        n = len(self.specs())
        if verbose:
            print(f"  → {n - broken}/{n} 道门有鉴别力")
        return broken
