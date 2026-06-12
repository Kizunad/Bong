"""worldgen-v4 P4 — zone ``spirit_qi`` 派生 + 预算配平 + zones.json 导出治理.

第2段（zones-export）：把第1段的统一灵气场（``qi_field.py``）落到 **zone 标量 +
导出物**：

1. **zone ``spirit_qi`` 派生** —— 每个 zone 在其 bounds 内对统一场做**面积加权
   均值** clamp[-1,1]（``qi_field.spirit_qi_from_field``）。这是写进
   ``server/zones.json`` 的幂等灵气标量，取代历史"两份漂移"里手填的 spirit_qi。

2. **导出期预算配平（§8.1 #3）** —— 全图统一场的灵气积分在**导出期**归一为预算
   口径 ``SPIRIT_QI_TOTAL``（= Rust ``DEFAULT_SPIRIT_QI_TOTAL`` = 100.0，但**绝不
   写字面 100**，从 env var ``BONG_SPIRIT_QI_TOTAL`` 读，与
   ``server/src/qi_physics/ledger.rs`` ``WorldQiBudget::from_env`` 同源）。

   配平模型（守恒：override 计入总量，不豁免）：
     * 每个 zone 的**灵气质量** ``mass_i = density_mean_i * area_i``，其中
       ``density_mean_i = clamp01((spirit_qi_i + 1)/2)``（同源派生，[0,1]）。
     * **wilderness**（zone 之外的全图余区）质量
       ``mass_wild = base_density * area_wild``（末法基线薄灵）。
     * 全图原始质量 ``M = Σ mass_i + mass_wild``。归一系数 ``k = budget / M``。
     * 各 zone **份额** ``share_i = k * mass_i``；**wilderness 余量**
       ``wild_share = k * mass_wild``。
     * **配平断言**：``Σ share_i + wild_share == budget``（带容差）。
       override zone 的 mass 照常进 ``M`` 分母（精调浓度不豁免守恒），归一时由所有
       区域（含 wilderness 与非 override zone）按比例吸收差额。

   注意：``share_i`` 是**预算口径的灵气份额**（进 manifest 报表），与写进
   zones.json 的 ``spirit_qi_i``（native 面积加权均值）是**两个不同的导出**——
   一份统一场两种导出口径。

3. **qi_grade 归档（§8.1 #4）** —— 现有 28 zone 的历史 ``spirit_qi`` 标量就近落到
   ``QiGrade`` 六档（``qi_grade_from_value``），作为初始档位写进报表，供后续
   blueprint DSL 迁移参考。

4. **zones.json 双写治理（§8.1 #11）** —— 导出只写**幂等字段**
   (name / aabb / spirit_qi / danger_level)，运行时手工字段
   (active_events / patrol_anchors / blocked_tiles) 及其它非幂等扩展字段
   (ambient_recipe_id / spawn_distribution ...) 按 zone name match **merge 保留**，
   **绝不 clobber**；文件头加 ``generated_by: "worldgen-v4 <commit>"``。

派生 / 配平是**纯函数**（输入 zone 列表 → 输出标量 / 报表 dict），IO（读写
zones.json）单列在 ``write_zones_json`` 里，方便单测对拍。
"""

from __future__ import annotations

import json
import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np

from .dsl import (
    QI_FIELD_MAX,
    QI_FIELD_MIN,
    QiGrade,
    qi_grade_from_value,
)
from .qi_field import (
    QI_FIELD_BASE,
    build_qi_field,
    kernel_spec_from_grade,
    qi_density_from_field,
    spirit_qi_from_field,
)

# ---------------------------------------------------------------------------
# 预算口径单一真相（§8.1 #3）.
#
# ``SPIRIT_QI_TOTAL`` 从环境变量 ``BONG_SPIRIT_QI_TOTAL`` 读，与 Rust 侧
# ``server/src/qi_physics/ledger.rs`` ``WorldQiBudget::from_env()`` 同名同源；
# 缺省 fallback 与 ``server/src/qi_physics/constants.rs`` ``DEFAULT_SPIRIT_QI_TOTAL``
# 数值对齐（= 100.0），但**绝不写字面 100**：默认值经 ``_default_spirit_qi_total``
# 解析（默认从同名 env var 读，与 Rust from_env 路径对称）。
# ---------------------------------------------------------------------------
SPIRIT_QI_TOTAL_ENV = "BONG_SPIRIT_QI_TOTAL"

# Rust ``DEFAULT_SPIRIT_QI_TOTAL`` 数值锚（constants.rs:64）。单列为模块常量而非
# 散落字面量，便于 CI 对拍（grep constants.rs 验证未漂移，见
# ``assert_const_matches_rust``）。命名刻意区别于 ``SPIRIT_QI_TOTAL``：这是
# "Rust 默认值的 Python 镜像锚"，env var 未设时由它兜底。
_RUST_DEFAULT_SPIRIT_QI_TOTAL = 100.0


def resolve_spirit_qi_total() -> float:
    """读 ``BONG_SPIRIT_QI_TOTAL`` env var（同 Rust from_env）；无效 / 缺省 fallback 默认锚.

    与 ``WorldQiBudget::from_env()`` 路径对称：env 有合法 f64 用之，否则回退
    ``_RUST_DEFAULT_SPIRIT_QI_TOTAL``（= constants.rs DEFAULT_SPIRIT_QI_TOTAL）。
    单源口径——配平断言 / manifest 报表均取此返回值，**不写字面 100**。
    """
    raw = os.environ.get(SPIRIT_QI_TOTAL_ENV)
    if raw is not None:
        try:
            parsed = float(raw)
        except ValueError:
            parsed = None
        if parsed is not None and parsed > 0.0:
            return parsed
    return _RUST_DEFAULT_SPIRIT_QI_TOTAL


# 配平断言容差：浮点归一后 ``Σ share + wild_share`` 与 budget 的允许误差。
# 归一是单次乘法（k = budget / M），误差只来自 float64 累加舍入，1e-6 足够紧。
QI_BUDGET_EPSILON = 1e-6

# 幂等字段白名单（§8.1 #11）—— 导出只覆写这些；其余字段 merge 保留。
IDEMPOTENT_ZONE_FIELDS = ("name", "aabb", "spirit_qi", "danger_level")


# ---------------------------------------------------------------------------
# zone 几何 + 灵气声明的最小输入契约
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ZoneQiInput:
    """配平 / 派生所需的单个 zone 最小输入.

    ``name`` —— zone 唯一名（merge match 键）。
    ``bounds_xz`` —— (min_x, max_x, min_z, max_z) 世界坐标 AABB 的 xz 投影。
    ``grade`` —— qi_grade 档位（核心目标灵气值的兜底来源；无 ``target_value`` 时
                 取该档代表值 ``qi_grade_target``）。
    ``target_value`` —— 可选精确核心目标灵气值（[-1,1]）。从历史手填 ``spirit_qi``
                 归档来时传入原值，使 zone 读到它**声明的精确浓度**（而非只读档位
                 中点），保留 baolongwang=-0.80 / dan_zong=0.40 这类强设计意图。
                 None → 退回 grade 中点。``override`` 优先级高于 ``target_value``。
    ``override`` —— 可选 qi_override 硬约束（None = 由 target/grade 决定）。
    ``falloff_exponent`` —— 核径向衰减指数（默认 1.6，与 qi_field 一致）。
    """

    name: str
    bounds_xz: tuple[float, float, float, float]
    grade: QiGrade
    target_value: float | None = None
    override: float | None = None
    falloff_exponent: float = 1.6

    @property
    def center_xz(self) -> tuple[float, float]:
        min_x, max_x, min_z, max_z = self.bounds_xz
        return ((min_x + max_x) / 2.0, (min_z + max_z) / 2.0)

    @property
    def half_w(self) -> float:
        min_x, max_x, _, _ = self.bounds_xz
        return max((max_x - min_x) / 2.0, 1.0)

    @property
    def half_d(self) -> float:
        _, _, min_z, max_z = self.bounds_xz
        return max((max_z - min_z) / 2.0, 1.0)

    @property
    def area(self) -> float:
        min_x, max_x, min_z, max_z = self.bounds_xz
        return max(max_x - min_x, 0.0) * max(max_z - min_z, 0.0)


def archive_grade_from_spirit_qi(spirit_qi: float) -> QiGrade:
    """把历史 ``spirit_qi`` 标量就近归档到 ``QiGrade`` 六档（§8.1 #4）.

    薄封装 ``qi_grade_from_value``，单列以表意"历史值 → 初始档位"的归档语义。
    """
    return qi_grade_from_value(spirit_qi)


def zone_qi_input_from_blueprint(zone: Any) -> ZoneQiInput:
    """从一个 ``BlueprintZone`` 适配出 ``ZoneQiInput``（geometry + 灵气档位）.

    qi_grade / qi_override 取自 zone 的 DSL 块（``zone.worldgen.dsl``，P2 schema）；
    DSL 缺失（旧 blueprint 尚未声明灵气档位）→ **就近从历史 ``spirit_qi`` 归档**
    （§8.1 #4 "现有 28 zone 的 spirit_qi 就近归档为初始 qi_grade 六档"）。这样在
    blueprint 补 DSL 之前，配平 / 派生仍按历史浓度档位运转，迁移后自动改读声明值。

    归档路径额外把历史 ``spirit_qi`` 原值传作 ``target_value``（核心精确目标），让
    zone 读到它声明的浓度而非档位中点——保留 baolongwang=-0.80 / dan_zong=0.40 等
    强设计意图（否则派生会把强意图值拉到档位中点，丢失精度）。
    """
    bounds = zone.bounds_xz
    bounds_xz = (
        float(bounds.min_x),
        float(bounds.max_x),
        float(bounds.min_z),
        float(bounds.max_z),
    )
    dsl = getattr(zone.worldgen, "dsl", None)
    if dsl is not None and getattr(dsl, "qi_grade", None) is not None:
        # DSL 已声明档位：用声明档位中点作目标（精确值靠 qi_override），不回填历史值。
        grade = dsl.qi_grade
        override = getattr(dsl, "qi_override", None)
        target_value = None
    else:
        # 归档路径：grade 就近落档，原 spirit_qi 作精确目标值。
        orig = float(zone.spirit_qi)
        grade = archive_grade_from_spirit_qi(orig)
        override = None
        target_value = orig
    return ZoneQiInput(
        name=str(zone.name),
        bounds_xz=bounds_xz,
        grade=grade,
        target_value=target_value,
        override=override,
    )


# ---------------------------------------------------------------------------
# 统一场采样 —— 在 zone bounds 内对全图场采样，算面积加权均值 spirit_qi
# ---------------------------------------------------------------------------


def _zone_kernels(zones: Sequence[ZoneQiInput]) -> list[dict]:
    """把 zone 输入转成 ``build_qi_field`` 的 zone_kernel dict 列表.

    ``target_value`` 非 None 时覆写核心目标灵气值（精确声明浓度），否则用 grade 中点
    （``kernel_spec_from_grade`` 默认）。``override`` 仍走硬约束路径（优先级最高）。
    """
    specs: list[dict] = []
    for z in zones:
        spec = kernel_spec_from_grade(
            center_xz=z.center_xz,
            half_w=z.half_w,
            half_d=z.half_d,
            grade=z.grade,
            override=z.override,
            falloff_exponent=z.falloff_exponent,
        )
        if z.target_value is not None:
            spec["target"] = float(
                np.clip(z.target_value, QI_FIELD_MIN, QI_FIELD_MAX)
            )
        specs.append(spec)
    return specs


def sample_zone_spirit_qi(
    zone: ZoneQiInput,
    all_zones: Sequence[ZoneQiInput],
    *,
    grid: int = 24,
    enable_veins: bool = True,
    vein_seed: int = 0,
) -> float:
    """在 ``zone`` bounds 内对**全图统一场**采样，返回面积加权均值 spirit_qi clamp[-1,1].

    采样 ``grid × grid`` 个均匀点覆盖 zone 的 xz AABB（含邻 zone 核 + 灵脉的真实
    叠加效果，故传 ``all_zones``）。等权采样（均匀网格 → 面积权重恒等），均值即
    ``qi_field.spirit_qi_from_field``。grid 默认 24 足够稳定（zone 核径向平滑，
    无需高分辨率），可调高做收敛校验。
    """
    min_x, max_x, min_z, max_z = zone.bounds_xz
    xs = np.linspace(min_x, max_x, grid)
    zs = np.linspace(min_z, max_z, grid)
    wx, wz = np.meshgrid(xs, zs)
    field = build_qi_field(
        wx,
        wz,
        zone_kernels=_zone_kernels(all_zones),
        vein_seed=vein_seed,
        enable_veins=enable_veins,
    )
    return spirit_qi_from_field(field)


def derive_zone_spirit_qi(
    zones: Sequence[ZoneQiInput],
    *,
    grid: int = 24,
    enable_veins: bool = True,
    vein_seed: int = 0,
) -> dict[str, float]:
    """逐 zone 派生面积加权均值 ``spirit_qi``（同源场两种导出之一）.

    返回 ``{zone_name: spirit_qi}``。重名 zone 报错（merge match 键必须唯一）。
    """
    seen: set[str] = set()
    out: dict[str, float] = {}
    for zone in zones:
        if zone.name in seen:
            raise ValueError(
                f"derive_zone_spirit_qi: duplicate zone name {zone.name!r} "
                "(zone names must be unique for zones.json merge match)"
            )
        seen.add(zone.name)
        out[zone.name] = sample_zone_spirit_qi(
            zone,
            zones,
            grid=grid,
            enable_veins=enable_veins,
            vein_seed=vein_seed,
        )
    return out


# ---------------------------------------------------------------------------
# 导出期预算配平（§8.1 #3）
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class QiBudgetReport:
    """全图灵气预算配平报表（进 manifest ``qi_budget_report``）.

    ``total`` —— 预算口径（= ``SPIRIT_QI_TOTAL``）。
    ``zone_shares`` —— ``{zone_name: 归一后的预算份额}``（Σ ≤ total）。
    ``wilderness_share`` —— zone 外余区吸收的余量份额。
    ``zone_sum`` —— ``Σ zone_shares``（== total - wilderness_share）。
    ``histogram`` —— qi_grade 六档的 zone 计数 + 份额直方图。
    """

    total: float
    zone_shares: dict[str, float]
    wilderness_share: float
    zone_sum: float
    histogram: list[dict[str, Any]]

    def to_manifest(self) -> dict[str, Any]:
        """序列化进 manifest 的 ``qi_budget_report`` 字段（JSON-safe）。"""
        return {
            "total": self.total,
            "zone_sum": self.zone_sum,
            "wilderness_share": self.wilderness_share,
            "zone_shares": dict(self.zone_shares),
            "histogram": self.histogram,
        }


def _grade_histogram(
    zones: Sequence[ZoneQiInput],
    derived_spirit_qi: Mapping[str, float],
    zone_shares: Mapping[str, float],
) -> list[dict[str, Any]]:
    """按 qi_grade 六档聚合 zone 计数 + 份额（直方图，进报表）。"""
    buckets: dict[QiGrade, dict[str, Any]] = {
        g: {"grade": g.value, "zone_count": 0, "share_sum": 0.0, "zones": []}
        for g in QiGrade
    }
    for zone in zones:
        sq = derived_spirit_qi[zone.name]
        grade = qi_grade_from_value(sq)
        bucket = buckets[grade]
        bucket["zone_count"] += 1
        bucket["share_sum"] += float(zone_shares.get(zone.name, 0.0))
        bucket["zones"].append(zone.name)
    return [buckets[g] for g in QiGrade]


def balance_qi_budget(
    zones: Sequence[ZoneQiInput],
    derived_spirit_qi: Mapping[str, float],
    *,
    world_area: float,
    budget: float | None = None,
    base_field: float = QI_FIELD_BASE,
) -> QiBudgetReport:
    """把全图灵气积分归一到预算口径，产出 zone 份额 + wilderness 余量报表（§8.1 #3）.

    配平模型（守恒，override 不豁免）：
      mass_i      = clamp01((spirit_qi_i + 1)/2) * area_i      （zone 灵气质量）
      mass_wild   = clamp01((base_field + 1)/2)  * area_wild   （末法薄灵余区）
      M           = Σ mass_i + mass_wild
      k           = budget / M
      share_i     = k * mass_i ；wild_share = k * mass_wild
    断言 ``Σ share_i + wild_share == budget``（容差 ``QI_BUDGET_EPSILON``）。

    ``budget`` 缺省取 ``resolve_spirit_qi_total()``（env var / 默认锚，**不写字面
    100**）。``world_area`` 必须 ≥ Σ zone area（否则 wilderness 面积为负，配置错误
    宁炸不静默）。全 zone 质量为 0 且无 wilderness 质量（病态退化）→ 报错而非除 0。
    """
    if budget is None:
        budget = resolve_spirit_qi_total()
    if budget <= 0.0:
        raise ValueError(f"balance_qi_budget: budget must be > 0, got {budget}")

    zone_area_sum = float(sum(z.area for z in zones))
    if world_area < zone_area_sum - QI_BUDGET_EPSILON:
        raise ValueError(
            f"balance_qi_budget: world_area {world_area} < Σ zone area "
            f"{zone_area_sum} (wilderness area would be negative — check bounds)"
        )
    wild_area = max(world_area - zone_area_sum, 0.0)

    zone_mass: dict[str, float] = {}
    for zone in zones:
        sq = float(derived_spirit_qi[zone.name])
        density = float(qi_density_from_field(sq))
        zone_mass[zone.name] = density * zone.area

    base_density = float(qi_density_from_field(base_field))
    wild_mass = base_density * wild_area

    total_mass = float(sum(zone_mass.values())) + wild_mass
    if total_mass <= 0.0:
        raise ValueError(
            "balance_qi_budget: total qi mass is 0 (all zones dead AND no "
            "wilderness mass) — cannot normalize to budget"
        )

    k = budget / total_mass
    zone_shares = {name: k * mass for name, mass in zone_mass.items()}
    wild_share = k * wild_mass
    zone_sum = float(sum(zone_shares.values()))

    # 配平断言：归一后份额总和精确等于预算（守恒收口）。
    assert abs(zone_sum + wild_share - budget) < QI_BUDGET_EPSILON, (
        f"qi budget balance failed: Σ zone_shares ({zone_sum}) + wilderness "
        f"({wild_share}) = {zone_sum + wild_share} != budget {budget} "
        f"(tolerance {QI_BUDGET_EPSILON})"
    )

    histogram = _grade_histogram(zones, derived_spirit_qi, zone_shares)
    return QiBudgetReport(
        total=budget,
        zone_shares=zone_shares,
        wilderness_share=wild_share,
        zone_sum=zone_sum,
        histogram=histogram,
    )


# ---------------------------------------------------------------------------
# zones.json 双写治理（§8.1 #11）—— 幂等字段覆写 + 运行时字段 merge 保留
# ---------------------------------------------------------------------------


def merge_zone_spirit_qi(
    existing_zones: Sequence[Mapping[str, Any]],
    derived_spirit_qi: Mapping[str, float],
) -> list[dict[str, Any]]:
    """按 zone name match 把派生 ``spirit_qi`` 覆写进现有 zones，其余字段原样保留.

    **只覆写 ``spirit_qi``**（幂等字段中唯一由场派生的值；name/aabb/danger_level 不
    由本段改）。运行时手工字段（active_events / patrol_anchors / blocked_tiles）及
    扩展字段（ambient_recipe_id / spawn_distribution ...）**逐字段保留不动**——这是
    §8.1 #11 的"绝不 clobber 运行时字段"硬约束。

    派生字典里有但现有 zones.json 没有的 zone 名报错（结构性新增必须人工 review，
    不在导出脚本里悄悄建 zone）。现有 zone 不在派生字典里 → 保留旧 spirit_qi（不
    强制全覆盖，缺一个就炸太脆）。
    """
    existing_names = {str(z["name"]) for z in existing_zones}
    unknown = set(derived_spirit_qi) - existing_names
    if unknown:
        raise ValueError(
            f"merge_zone_spirit_qi: derived spirit_qi for zones not in "
            f"zones.json: {sorted(unknown)} (structural additions need manual "
            "review — export only updates existing zones)"
        )

    merged: list[dict[str, Any]] = []
    for zone in existing_zones:
        name = str(zone["name"])
        # 深拷贝（dict 浅拷贝够用：值是 scalar / list，覆写 spirit_qi 不动嵌套）。
        new_zone = dict(zone)
        if name in derived_spirit_qi:
            sq = float(np.clip(derived_spirit_qi[name], QI_FIELD_MIN, QI_FIELD_MAX))
            # 6 位小数足够（场派生噪声远粗于此），避免 float64 尾巴污染 diff。
            new_zone["spirit_qi"] = round(sq, 6)
        merged.append(new_zone)
    return merged


def build_zones_file(
    existing_doc: Mapping[str, Any],
    derived_spirit_qi: Mapping[str, float],
    *,
    generated_by: str,
) -> dict[str, Any]:
    """构造导出后的 zones.json 顶层文档：merge spirit_qi + 加 generated_by 标记.

    ``existing_doc`` 是现有 ``{"zones": [...]}`` 文档；保留其所有顶层键，只
    替换 ``zones`` 为 merge 后列表 + 注入 ``generated_by``。
    """
    out = dict(existing_doc)
    out["generated_by"] = generated_by
    out["zones"] = merge_zone_spirit_qi(
        existing_doc.get("zones", []), derived_spirit_qi
    )
    return out


def write_zones_json(
    path: Path,
    derived_spirit_qi: Mapping[str, float],
    *,
    generated_by: str,
) -> dict[str, Any]:
    """读现有 zones.json → merge 派生 spirit_qi + generated_by → 原子写回（IO 边界）.

    返回写出的文档 dict（供调用方做 diff 预检）。读 / 写都在这里，纯派生逻辑在
    ``build_zones_file`` / ``merge_zone_spirit_qi``（可独立单测）。
    """
    existing = json.loads(path.read_text(encoding="utf-8"))
    doc = build_zones_file(existing, derived_spirit_qi, generated_by=generated_by)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(
        json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    tmp.replace(path)
    return doc


# ---------------------------------------------------------------------------
# CI 漂移防护：Python 默认锚 vs Rust constants.rs（§8.1 #3 风险栏）
# ---------------------------------------------------------------------------


def parse_rust_default_spirit_qi_total(constants_rs: str) -> float:
    """从 ``constants.rs`` 源文本正则提取 ``DEFAULT_SPIRIT_QI_TOTAL`` 值（CI 校验用）.

    脆弱（多行 / 宏会失效），**只作 CI 二级对拍**，不作主路径取值。主路径仍走
    ``resolve_spirit_qi_total``（env var / 默认锚）。
    """
    import re

    match = re.search(
        r"DEFAULT_SPIRIT_QI_TOTAL\s*:\s*f64\s*=\s*([0-9.]+)", constants_rs
    )
    if match is None:
        raise ValueError(
            "DEFAULT_SPIRIT_QI_TOTAL not found in constants.rs source "
            "(grep pattern stale? — check server/src/qi_physics/constants.rs)"
        )
    return float(match.group(1))


def assert_const_matches_rust(constants_rs_path: Path) -> None:
    """断言 Python 默认锚 ``_RUST_DEFAULT_SPIRIT_QI_TOTAL`` 未与 Rust 常量漂移（CI）.

    防"Rust 改常量、Python 未跟"导致两端预算口径悄然分裂（§8.1 #3 风险栏）。
    """
    rust_value = parse_rust_default_spirit_qi_total(
        constants_rs_path.read_text(encoding="utf-8")
    )
    if abs(rust_value - _RUST_DEFAULT_SPIRIT_QI_TOTAL) > QI_BUDGET_EPSILON:
        raise AssertionError(
            f"spirit_qi total drift: Rust DEFAULT_SPIRIT_QI_TOTAL={rust_value} "
            f"!= Python _RUST_DEFAULT_SPIRIT_QI_TOTAL="
            f"{_RUST_DEFAULT_SPIRIT_QI_TOTAL}; sync the two anchors"
        )


# ---------------------------------------------------------------------------
# 高层编排 —— blueprint zones → 派生 + 配平 → manifest 报表 / zones.json 写出
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ZonesExportResult:
    """一次 zone 灵气导出的产物聚合（manifest 报表 + 派生标量 + 配平报表）。"""

    derived_spirit_qi: dict[str, float]
    budget_report: QiBudgetReport
    archived_grades: dict[str, str]


def bake_zone_qi(
    blueprint_zones: Sequence[Any],
    *,
    world_area: float,
    budget: float | None = None,
    grid: int = 24,
    enable_veins: bool = True,
    vein_seed: int = 0,
) -> ZonesExportResult:
    """从 blueprint zones 端到端派生 spirit_qi + 配平预算 + 归档 grade（纯计算）.

    这是 manifest ``qi_budget_report`` 与 zones.json spirit_qi merge 的共同上游：
    一次场合成既得 zone 面积加权均值（写 zones.json），又得预算份额（进报表），
    保证两个导出口径来自同一份场（一份统一场两种导出）。
    """
    inputs = [zone_qi_input_from_blueprint(z) for z in blueprint_zones]
    derived = derive_zone_spirit_qi(
        inputs, grid=grid, enable_veins=enable_veins, vein_seed=vein_seed
    )
    report = balance_qi_budget(
        inputs, derived, world_area=world_area, budget=budget
    )
    archived = {
        z.name: qi_grade_from_value(derived[z.name]).value for z in inputs
    }
    return ZonesExportResult(
        derived_spirit_qi=derived,
        budget_report=report,
        archived_grades=archived,
    )


def _world_area_from_bounds(world_bounds: Any) -> float:
    """从 plan/blueprint 的 world bounds 算 xz 平面面积（wilderness 面积分母）。"""
    return float(
        max(world_bounds.max_x - world_bounds.min_x, 0.0)
        * max(world_bounds.max_z - world_bounds.min_z, 0.0)
    )


def git_commit_label() -> str:
    """当前 git short commit（``generated_by`` 标记用）；非 git 环境回退 'unknown'。"""
    import subprocess

    try:
        out = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        )
        return out.stdout.strip() or "unknown"
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def bake_zones_json_file(
    *,
    blueprint_path: Path,
    zones_json_path: Path,
    commit: str | None = None,
    grid: int = 24,
) -> ZonesExportResult:
    """端到端：读 blueprint + zones.json → 派生 spirit_qi + 配平 → merge 写回 zones.json.

    §8.1 #11 治理：只覆写幂等 spirit_qi，运行时字段 merge 保留，文件头加
    ``generated_by: "worldgen-v4 <commit>"``。配平断言在 ``bake_zone_qi`` 内引用
    预算口径常量（不写字面 100），不达标即炸。

    ``zones.json`` 里但 blueprint 没有的 zone（如 giant_sword_sea）由 merge 逻辑
    保留旧 spirit_qi（不强制覆盖、不报错），结构性差异留人工。返回派生 + 配平结果
    供调用方打印 / diff 预检。
    """
    from .blueprint import load_blueprint

    blueprint = load_blueprint(blueprint_path)
    world_area = _world_area_from_bounds(blueprint.bounds_xz)
    result = bake_zone_qi(blueprint.zones, world_area=world_area, grid=grid)

    label = commit if commit is not None else git_commit_label()
    write_zones_json(
        zones_json_path,
        result.derived_spirit_qi,
        generated_by=f"worldgen-v4 {label}",
    )
    return result


def main(argv: Sequence[str] | None = None) -> int:
    """CLI：``python -m scripts.terrain_gen.zones_export`` —— bake zones.json spirit_qi."""
    import argparse

    from .blueprint import DEFAULT_BLUEPRINT_PATH, REPO_ROOT

    parser = argparse.ArgumentParser(
        description="worldgen-v4 P4 — derive zone spirit_qi from the unified qi "
        "field, balance to budget, and merge into server/zones.json."
    )
    parser.add_argument(
        "--blueprint", type=Path, default=DEFAULT_BLUEPRINT_PATH,
        help="blueprint source (zones.worldview.example.json)",
    )
    parser.add_argument(
        "--zones-json", type=Path, default=REPO_ROOT / "server" / "zones.json",
        help="zones.json to merge spirit_qi into (idempotent fields only)",
    )
    parser.add_argument(
        "--commit", type=str, default=None,
        help="commit label for generated_by (default: git short HEAD)",
    )
    args = parser.parse_args(argv)

    result = bake_zones_json_file(
        blueprint_path=args.blueprint,
        zones_json_path=args.zones_json,
        commit=args.commit,
    )
    report = result.budget_report
    print(f"zones.json baked: {args.zones_json}")
    print(
        f"  budget total={report.total} zone_sum={report.zone_sum:.4f} "
        f"wilderness={report.wilderness_share:.4f}"
    )
    for bucket in report.histogram:
        if bucket["zone_count"]:
            print(
                f"  grade {bucket['grade']:9s}: {bucket['zone_count']:2d} zones "
                f"share={bucket['share_sum']:.4f}"
            )
    return 0


__all__ = [
    "SPIRIT_QI_TOTAL_ENV",
    "QI_BUDGET_EPSILON",
    "IDEMPOTENT_ZONE_FIELDS",
    "ZoneQiInput",
    "QiBudgetReport",
    "ZonesExportResult",
    "resolve_spirit_qi_total",
    "archive_grade_from_spirit_qi",
    "zone_qi_input_from_blueprint",
    "sample_zone_spirit_qi",
    "derive_zone_spirit_qi",
    "balance_qi_budget",
    "bake_zone_qi",
    "merge_zone_spirit_qi",
    "build_zones_file",
    "write_zones_json",
    "bake_zones_json_file",
    "git_commit_label",
    "main",
    "parse_rust_default_spirit_qi_total",
    "assert_const_matches_rust",
]


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
