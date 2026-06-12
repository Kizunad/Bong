"""worldgen-v4 P2 — 声明式地形 DSL 核心 + 可复用算子库.

P2 目标：把 20 个手写 numpy profile 重写为**声明式 DSL 组合**。这个模块提供：

1. **算子库** —— 从 Scout 归纳的 profile 共性抽取的可复用原语，分四类：
     * height ops  —— 基底高度场（fbm / ridge / warped / radial 隆起 / 轴向剖面）
     * mask ops    —— 掩码（radial / 同心带 / 轴向 SDF / 锚点 SDF / 模数网格散点）
     * carve ops   —— 雕刻器（rift / fracture / neg_pressure / 雪线分段）
     * scatter ops —— 散布（noise band 选变种 / anomaly argmax 竞争）
   每个算子是纯函数（输入 numpy 场 → 输出 numpy 场），可独立单测。

2. **TerrainStyle 解释器** —— 把声明式 ``TerrainStyleSpec``（基底高度场 op 链 +
   雕刻器 op 链 + 特征件）解释成 height 场。surface_palette / flora_table 是
   平行声明。

3. **escape hatch** —— ``register_custom_op`` 让复杂 profile（spawn 动态灵泉选点、
   fossil mask、ascension pit 等）把任意 python numpy 函数挂进算子库，DSL 用
   ``op="custom:<name>"`` 引用，避免把不可声明化的逻辑硬塞进 schema。

4. **qi_grade 档位制（§8.1#4）** —— 统一灵气场 native 值域 [-1,1]，六档枚举 +
   可选 qi_override 连续值。**P2 只锁 schema 声明**（terrain 几何不受影响），实际
   灵气场生成在 P4。

P2 本段只**建立 DSL 核心 + 算子库 + schema**；profile 迁移在后续段做。算子库是
迁移的素材，schema 是迁移的目标格式，两者先于实现锁定（接口先于实现）。
"""

from __future__ import annotations

from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass, field
from enum import Enum
from typing import Any

import numpy as np

from .noise import fbm_2d, ridge_2d, warped_fbm_2d

# ---------------------------------------------------------------------------
# §8.1 #4 — qi_grade 六档制
#
# 统一灵气场 native 值域 [-1, 1]（覆盖 worldview §二 三层环境：负灵域 / 死域 /
# 馈赠区）。zone DSL 声明 qi_grade 六档之一，可选 qi_override 连续值精调。
# 派生（P4 实装）：spirit_qi = zone 内场面积加权均值 clamp[-1,1]；
#                  qi_density raster = clamp01((field+1)/2)。
# ---------------------------------------------------------------------------


class QiGrade(str, Enum):
    """灵气浓度六档。值域边界见 ``QI_GRADE_BOUNDS``。

    继承 ``str`` 让枚举可直接 JSON 序列化 / blueprint 字段对齐（snake_case）。
    """

    NEGATIVE = "negative"  # [-1, -0.1)   负灵域 —— 吞噬真元（涡流宗、镇灵台）
    DEAD = "dead"          # [-0.1, 0.02) 死域 —— 灵气断绝（灰烬死域）
    TRACE = "trace"        # [0.02, 0.15) 微量 —— 末法荒野基线（裂口荒原、北陲）
    COMMON = "common"      # [0.15, 0.45) 常量 —— 主世界多数区域
    RICH = "rich"          # [0.45, 0.7)  丰沛 —— 灵脉残存（青云峰、五行渊）
    FONT = "font"          # [0.7, 1]     灵源 —— 馈赠区（灵泉沼、天界岛、TSY）


# 半开区间 [lo, hi) 边界（最后一档 FONT 上界闭合到 1.0）。严格按 §8.1 #4。
QI_GRADE_BOUNDS: dict[QiGrade, tuple[float, float]] = {
    QiGrade.NEGATIVE: (-1.0, -0.1),
    QiGrade.DEAD: (-0.1, 0.02),
    QiGrade.TRACE: (0.02, 0.15),
    QiGrade.COMMON: (0.15, 0.45),
    QiGrade.RICH: (0.45, 0.7),
    QiGrade.FONT: (0.7, 1.0),
}

# 灵气场 native 值域（§8.1 #4）。
QI_FIELD_MIN = -1.0
QI_FIELD_MAX = 1.0

# 档位顺序（升序）—— FONT 上界用闭区间收口。
_QI_GRADE_ORDER: tuple[QiGrade, ...] = (
    QiGrade.NEGATIVE,
    QiGrade.DEAD,
    QiGrade.TRACE,
    QiGrade.COMMON,
    QiGrade.RICH,
    QiGrade.FONT,
)


def qi_grade_from_value(value: float) -> QiGrade:
    """把一个 [-1, 1] 的灵气 native 值映射到它所属的六档之一.

    半开区间 [lo, hi)，FONT 闭合到 1.0。低于 -1 归 NEGATIVE，高于 1 归 FONT
    （越界值 clamp 到端档，而非报错——派生路径已 clamp，但调用方可能传原始值）。
    """
    if value < QI_GRADE_BOUNDS[QiGrade.NEGATIVE][0]:
        return QiGrade.NEGATIVE
    if value >= QI_GRADE_BOUNDS[QiGrade.FONT][1]:
        return QiGrade.FONT
    for grade in _QI_GRADE_ORDER:
        lo, hi = QI_GRADE_BOUNDS[grade]
        if lo <= value < hi:
            return grade
    # FONT 闭区间收口（value == 1.0 已在上面拦截，这里兜 hi 边界相等的浮点尾巴）。
    return QiGrade.FONT


def parse_qi_grade(raw: str) -> QiGrade:
    """从 blueprint 字符串解析 qi_grade，非法值给出可调试错误."""
    try:
        return QiGrade(raw)
    except ValueError as exc:
        valid = ", ".join(g.value for g in _QI_GRADE_ORDER)
        raise ValueError(
            f"unknown qi_grade {raw!r}; expected one of: {valid}"
        ) from exc


def validate_qi_override(value: float | None) -> float | None:
    """校验可选 qi_override 落在 native 值域 [-1, 1]，越界报错.

    override 是硬约束铺场值（§8.1 #4 #2 进配平闭环），必须在合法 native 值域内；
    None 表示不精调，由 qi_grade 档位决定场。
    """
    if value is None:
        return None
    v = float(value)
    if not (QI_FIELD_MIN <= v <= QI_FIELD_MAX):
        raise ValueError(
            f"qi_override={v} out of native qi field range "
            f"[{QI_FIELD_MIN}, {QI_FIELD_MAX}]"
        )
    return v


# ---------------------------------------------------------------------------
# 算子库 —— 四类可复用原语 + 自定义注册 escape hatch
#
# 约定：所有算子接收 (wx, wz) world-coordinate meshgrid（float64 ndarray，shape
# (H, W)）+ 关键字参数，返回同 shape 的 ndarray。这与 profiles/*.py 现有的
# ``_tile_coords`` → fbm/ridge/warped 调用约定完全一致，使迁移是"抽取"而非"重写"。
# ---------------------------------------------------------------------------

# 一个 op 是 (wx, wz, **params) -> ndarray 的可调用。
OpFn = Callable[..., np.ndarray]


# --- height ops：基底高度场 --------------------------------------------------


def fbm_height(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    scale: float = 256.0,
    octaves: int = 4,
    seed: int = 0,
    amplitude: float = 1.0,
    base: float = 0.0,
) -> np.ndarray:
    """base + amplitude * fbm。覆盖 spawn/broken_peaks 的滚动基底."""
    return base + amplitude * fbm_2d(wx, wz, scale=scale, octaves=octaves, seed=seed)


def ridge_height(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    scale: float = 256.0,
    octaves: int = 4,
    lacunarity: float = 2.0,
    gain: float = 0.5,
    seed: int = 0,
    amplitude: float = 1.0,
    base: float = 0.0,
) -> np.ndarray:
    """base + amplitude * ridge。覆盖 broken_peaks 的山脊主轨."""
    return base + amplitude * ridge_2d(
        wx,
        wz,
        scale=scale,
        octaves=octaves,
        lacunarity=lacunarity,
        gain=gain,
        seed=seed,
    )


def warped_height(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    scale: float = 256.0,
    octaves: int = 4,
    warp_scale: float = 400.0,
    warp_strength: float = 80.0,
    seed: int = 0,
    amplitude: float = 1.0,
    base: float = 0.0,
) -> np.ndarray:
    """base + amplitude * warped_fbm。覆盖 swale/侵蚀/岛屿合成的有机扭曲场."""
    return base + amplitude * warped_fbm_2d(
        wx,
        wz,
        scale=scale,
        octaves=octaves,
        warp_scale=warp_scale,
        warp_strength=warp_strength,
        seed=seed,
    )


def radial_uplift(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    center_xz: tuple[float, float],
    half_w: float,
    half_d: float,
    exponent: float = 1.9,
    amplitude: float = 1.0,
) -> np.ndarray:
    """heartland 隆起：amplitude * max(0, 1 - radial**exponent)。

    radial = sqrt(((x-cx)/half_w)^2 + ((z-cz)/half_d)^2)。覆盖 spawn 的 heartland /
    inner_meadow 隆起、所有"中心高边缘低"的径向高度贡献。
    """
    cx, cz = center_xz
    dx = (wx - cx) / max(half_w, 1.0)
    dz = (wz - cz) / max(half_d, 1.0)
    radial = np.sqrt(dx * dx + dz * dz)
    return amplitude * np.maximum(0.0, 1.0 - radial**exponent)


def axial_profile(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    center_xz: tuple[float, float],
    angle_deg: float,
    half_length: float,
    amplitude: float = 1.0,
) -> np.ndarray:
    """沿旋转轴的纵向剖面强度：amplitude * max(0, 1 - |along|/half_length)。

    把世界坐标旋转 ``angle_deg`` 后取轴向距离衰减。覆盖 rift_valley 的轴向
    valley_strength（沿裂谷主轴的影响沿轴衰减）。
    """
    cx, cz = center_xz
    theta = np.radians(angle_deg)
    cos_t, sin_t = np.cos(theta), np.sin(theta)
    rx = wx - cx
    rz = wz - cz
    along = rx * cos_t + rz * sin_t
    return amplitude * np.maximum(0.0, 1.0 - np.abs(along) / max(half_length, 1.0))


def compose_height(*terms: np.ndarray) -> np.ndarray:
    """把多个高度贡献场逐元素相加。覆盖所有 profile 的 height = a + b - c 合成."""
    if not terms:
        raise ValueError("compose_height requires at least one height term")
    out = np.array(terms[0], dtype=np.float64)
    for term in terms[1:]:
        out = out + term
    return out


# --- mask ops：掩码 ----------------------------------------------------------


def radial_mask(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    center_xz: tuple[float, float],
    half_w: float,
    half_d: float,
    exponent: float = 2.0,
) -> np.ndarray:
    """归一化径向掩码 max(0, 1 - radial**exponent)，center=1 edge→0。

    与 radial_uplift 同形但语义是 0~1 掩码（不带 amplitude），供 surface/qi/flora
    的"距中心远近"权重复用。
    """
    cx, cz = center_xz
    dx = (wx - cx) / max(half_w, 1.0)
    dz = (wz - cz) / max(half_d, 1.0)
    radial = np.sqrt(dx * dx + dz * dz)
    return np.clip(1.0 - radial**exponent, 0.0, 1.0)


def concentric_bands(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    center_xz: tuple[float, float],
    thresholds: Sequence[float],
    values: Sequence[float],
) -> np.ndarray:
    """同心环带赋值：按到中心的欧氏距离落入 thresholds 区间取对应 value。

    ``thresholds`` 升序半径边界（长度 N），``values`` 长度 N+1（最内圈 + N 个外圈）。
    覆盖 ash_dead_zone / pseudo_vein_oasis / rift_mouth_barrens 的 core/body/edge
    radial 分段常量赋值（qi/mofa/flora 逐段阶梯）。
    """
    if len(values) != len(thresholds) + 1:
        raise ValueError(
            f"concentric_bands needs len(values)=len(thresholds)+1; "
            f"got {len(values)} values for {len(thresholds)} thresholds"
        )
    for i in range(1, len(thresholds)):
        if thresholds[i] <= thresholds[i - 1]:
            raise ValueError(
                f"concentric_bands thresholds must be strictly ascending; "
                f"got {list(thresholds)}"
            )
    cx, cz = center_xz
    dist = np.sqrt((wx - cx) ** 2 + (wz - cz) ** 2)
    out = np.full(dist.shape, float(values[0]), dtype=np.float64)
    for i, thr in enumerate(thresholds):
        out = np.where(dist >= thr, float(values[i + 1]), out)
    return out


def axial_sdf(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    center_xz: tuple[float, float],
    angle_deg: float,
    half_width: float = 1.0,
) -> np.ndarray:
    """旋转轴的横向带状 SDF：|cross| / half_width（轴上=0，远离轴→大）。

    覆盖 rift_valley 的 rift_axis_sdf（沿裂谷主轴的归一化横向距离，0 在轴心）。
    """
    cx, cz = center_xz
    theta = np.radians(angle_deg)
    cos_t, sin_t = np.cos(theta), np.sin(theta)
    rx = wx - cx
    rz = wz - cz
    cross = -rx * sin_t + rz * cos_t
    return np.abs(cross) / max(half_width, 1e-6)


def anchor_sdf(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    anchor_xz: tuple[float, float],
) -> np.ndarray:
    """到固定锚点的欧氏距离 SDF。覆盖 portal_anchor_sdf / cave hotspot SDF."""
    ax, az = anchor_xz
    return np.sqrt((wx - ax) ** 2 + (wz - az) ** 2)


def modulo_grid_scatter(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    period_x: int,
    period_z: int,
    cell_match: int = 0,
) -> np.ndarray:
    """确定性 floor-grid 模数散点：(floor(x)%period_x==cell_match) & 同 z → 1 else 0。

    与噪声场无关的确定性打散，覆盖 ash_dead_zone / tribulation_scorch /
    dan_zong_yi_yuan 的"防 blob"离散摆件（floor-grid modulo 单格散点而非连片）。
    """
    if period_x <= 0 or period_z <= 0:
        raise ValueError(
            f"modulo_grid_scatter periods must be positive; "
            f"got period_x={period_x}, period_z={period_z}"
        )
    gx = np.mod(np.floor(wx).astype(np.int64), period_x) == (cell_match % period_x)
    gz = np.mod(np.floor(wz).astype(np.int64), period_z) == (cell_match % period_z)
    return (gx & gz).astype(np.float64)


# --- carve ops：雕刻器（高度场上的减法 / 分段）-------------------------------


def rift_carve(
    height: np.ndarray,
    *,
    rift_axis_sdf: np.ndarray,
    rim_edge_mask: np.ndarray,
    sdf_threshold: float = 0.9,
    axis_strength: float = 22.0,
    rim_strength: float = 4.0,
) -> np.ndarray:
    """轴向裂谷下切：sdf < threshold 处 height -= (1-sdf)*axis + rim*rim_strength。

    与 spans_shim.v3_surface_top_y 步骤①同公式（这里作用在连续 height 场而非
    整数 top_y，迁移后由 fold 统一离散化）。覆盖 rift_valley 的轴向雕刻。
    """
    carve = (1.0 - rift_axis_sdf) * axis_strength + rim_edge_mask * rim_strength
    return np.where(rift_axis_sdf < sdf_threshold, height - carve, height)


def fracture_carve(
    height: np.ndarray,
    *,
    fracture_mask: np.ndarray,
    mask_threshold: float = 0.7,
    crack_strength: float = 300.0,
    floor_y: float = -58.0,
) -> np.ndarray:
    """碎裂深切：mask > threshold 处 height = max(floor_y, height - (mask-thr)*crack)。

    与 spans_shim 步骤②同公式。覆盖 rift_valley / ancient_battlefield 的裂缝深沟。
    """
    crack = (fracture_mask - mask_threshold) * crack_strength
    carved = np.maximum(floor_y, height - crack)
    return np.where(fracture_mask > mask_threshold, carved, height)


def neg_pressure_carve(
    height: np.ndarray,
    *,
    neg_pressure: np.ndarray,
    threshold: float = 0.18,
    strength: float = 14.0,
) -> np.ndarray:
    """负压沉降：neg > threshold 处 height -= neg*strength。

    与 spans_shim 步骤③同公式。覆盖 waste_plateau / rift_mouth 的死域沉陷。
    """
    return np.where(
        neg_pressure > threshold, height - neg_pressure * strength, height
    )


def snowline_split(
    field: np.ndarray,
    *,
    height: np.ndarray,
    snowline_y: float,
    below_value: float,
    above_value: float,
) -> np.ndarray:
    """雪线分段：height >= snowline 取 above_value，否则 below_value。

    覆盖 broken_peaks 的雪线 285m surface 分段（也可复用为任意高度阈值的两段赋值）。
    """
    return np.where(height >= snowline_y, above_value, below_value)


# --- scatter ops：散布（变种 / anomaly 选择）---------------------------------


def flora_variant_by_noise(
    *,
    band_field: np.ndarray,
    pick_field: np.ndarray,
    band_threshold: float,
    pick_threshold: float,
    variant_id: int,
) -> np.ndarray:
    """双尺度 band+pick 防 blob 散点：(band > bt) & (pick > pt) 处置 variant_id。

    与 spawn_plain 的 fallen_select/fallen_pick 双层 noise 同模式——大尺度选"区域"，
    小尺度高频"打孔"成单格散点，避免一片 cell 全标记同一 variant。
    """
    placed = (band_field > band_threshold) & (pick_field > pick_threshold)
    return np.where(placed, int(variant_id), 0).astype(np.int32)


def anomaly_compete(
    *,
    fields: Sequence[np.ndarray],
    kind_ids: Sequence[int],
    threshold: float = 0.0,
) -> tuple[np.ndarray, np.ndarray]:
    """多 anomaly 场 argmax 竞争 → (intensity, kind_id)。

    ``fields`` 各候选 anomaly 强度场，``kind_ids`` 对应 anomaly_kind 编码（与
    fields 等长）。每列取最强场：intensity = max；kind = 该场的 kind_id，但若
    最强值 <= threshold 则 kind=0（无 anomaly）。覆盖 ancient_battlefield 的
    5-kind np.stack argmax 竞争。
    """
    if len(fields) != len(kind_ids):
        raise ValueError(
            f"anomaly_compete needs len(fields)==len(kind_ids); "
            f"got {len(fields)} fields, {len(kind_ids)} kind_ids"
        )
    if not fields:
        raise ValueError("anomaly_compete requires at least one field")
    stack = np.stack([np.asarray(f, dtype=np.float64) for f in fields], axis=0)
    winner = np.argmax(stack, axis=0)
    intensity = np.max(stack, axis=0)
    kind_lookup = np.asarray(kind_ids, dtype=np.int32)
    kind = kind_lookup[winner]
    kind = np.where(intensity > threshold, kind, 0).astype(np.int32)
    return intensity, kind


# --- 算子注册表 + escape hatch ----------------------------------------------

_BUILTIN_OPS: dict[str, dict[str, OpFn]] = {
    "height": {
        "fbm_height": fbm_height,
        "ridge_height": ridge_height,
        "warped_height": warped_height,
        "radial_uplift": radial_uplift,
        "axial_profile": axial_profile,
    },
    "mask": {
        "radial_mask": radial_mask,
        "concentric_bands": concentric_bands,
        "axial_sdf": axial_sdf,
        "anchor_sdf": anchor_sdf,
        "modulo_grid_scatter": modulo_grid_scatter,
    },
    "carve": {
        "rift_carve": rift_carve,
        "fracture_carve": fracture_carve,
        "neg_pressure_carve": neg_pressure_carve,
        "snowline_split": snowline_split,
    },
    "scatter": {
        "flora_variant_by_noise": flora_variant_by_noise,
        "anomaly_compete": anomaly_compete,
    },
}

OP_CATEGORIES: tuple[str, ...] = tuple(_BUILTIN_OPS.keys())

# 自定义算子（escape hatch）。key = 全名 ``custom:<name>``，value = 任意 numpy 函数。
_CUSTOM_OPS: dict[str, OpFn] = {}

CUSTOM_OP_PREFIX = "custom:"


def register_custom_op(name: str, fn: OpFn, *, replace: bool = False) -> str:
    """注册一个自定义 numpy 算子，返回 DSL 引用名 ``custom:<name>``.

    复杂 profile（spawn 动态灵泉选点 / fossil mask / ascension pit 等）无法用标准
    声明算子表达时，挂自定义函数进算子库，DSL 用 ``op="custom:<name>"`` 引用。
    这是 P2 保留的 escape hatch，不强迫所有逻辑声明化。

    ``replace=False`` 时重复注册同名算子报错（防静默覆盖）。
    """
    if not name or CUSTOM_OP_PREFIX in name or ":" in name:
        raise ValueError(
            f"custom op name {name!r} must be non-empty and contain no ':' "
            f"(the {CUSTOM_OP_PREFIX!r} prefix is added automatically)"
        )
    if not callable(fn):
        raise ValueError(f"custom op {name!r} must be callable, got {type(fn)!r}")
    ref = f"{CUSTOM_OP_PREFIX}{name}"
    if ref in _CUSTOM_OPS and not replace:
        raise ValueError(
            f"custom op {ref!r} already registered; pass replace=True to override"
        )
    _CUSTOM_OPS[ref] = fn
    return ref


def unregister_custom_op(name: str) -> None:
    """注销一个自定义算子（测试隔离用）。接受裸名或 ``custom:<name>`` 全名."""
    ref = name if name.startswith(CUSTOM_OP_PREFIX) else f"{CUSTOM_OP_PREFIX}{name}"
    _CUSTOM_OPS.pop(ref, None)


def resolve_op(category: str, op_name: str) -> OpFn:
    """按 (category, op_name) 解析算子。``custom:<name>`` 跳过 category 直查 escape hatch.

    非法 category / 未知 op / 未注册 custom 均给出带候选名的可调试错误。
    """
    if op_name.startswith(CUSTOM_OP_PREFIX):
        if op_name not in _CUSTOM_OPS:
            registered = ", ".join(sorted(_CUSTOM_OPS)) or "(none)"
            raise KeyError(
                f"custom op {op_name!r} not registered; registered custom ops: "
                f"{registered}"
            )
        return _CUSTOM_OPS[op_name]
    if category not in _BUILTIN_OPS:
        raise KeyError(
            f"unknown op category {category!r}; expected one of {OP_CATEGORIES}"
        )
    table = _BUILTIN_OPS[category]
    if op_name not in table:
        available = ", ".join(sorted(table))
        raise KeyError(
            f"unknown {category} op {op_name!r}; available: {available} "
            f"(or {CUSTOM_OP_PREFIX}<name> for a registered custom op)"
        )
    return table[op_name]


def list_ops(category: str | None = None) -> tuple[str, ...]:
    """列出某类（或全部）内建算子名，含已注册的 custom 算子（带前缀）。"""
    names: list[str] = []
    if category is None:
        for table in _BUILTIN_OPS.values():
            names.extend(table)
    else:
        if category not in _BUILTIN_OPS:
            raise KeyError(
                f"unknown op category {category!r}; expected one of {OP_CATEGORIES}"
            )
        names.extend(_BUILTIN_OPS[category])
    names.extend(_CUSTOM_OPS)
    return tuple(sorted(names))


# ---------------------------------------------------------------------------
# DSL schema —— 声明式 zone 地形定义
#
# 一个 ``ZoneTerrainDSL`` 声明：
#   terrain_style   —— 基底高度场 op 链 + 雕刻器 op 链 + 特征件
#   surface_palette —— 块选择规则（条件 → block 名）
#   flora_table     —— 作物/植被集（band 条件 → variant + 密度）
#   qi_grade        —— 六档之一（§8.1 #4）
#   qi_override     —— 可选连续值精调（None = 由 grade 决定）
#
# schema 字段与现有 blueprint TerrainProfileSpec 对齐 snake_case。每个 op 步是
# ``OpStep(category, op, params)``，解释器按链顺序求值 + compose。
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class OpStep:
    """一条 op 调用：category（height/mask/carve/scatter）+ op 名 + 参数。"""

    category: str
    op: str
    params: Mapping[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.op.startswith(CUSTOM_OP_PREFIX):
            # custom op 跳过 category 校验（escape hatch）。
            return
        if self.category not in _BUILTIN_OPS:
            raise ValueError(
                f"OpStep category {self.category!r} invalid; "
                f"expected one of {OP_CATEGORIES} (or a custom: op)"
            )


@dataclass(frozen=True)
class TerrainStyleSpec:
    """声明式地形风格：基底高度场链 + 雕刻器链 + 特征件标签。

    height_ops  —— 基底高度贡献（按链顺序 compose 相加成 base height）
    carve_ops   —— 在 base height 上依次施加的雕刻器（rift / fracture / neg ...）
    features    —— 特征件标签（特征件几何由 profile 自定义算子或 P3 carve 提供，
                   P2 只声明清单，不强制几何）
    """

    height_ops: tuple[OpStep, ...] = ()
    carve_ops: tuple[OpStep, ...] = ()
    features: tuple[str, ...] = ()


@dataclass(frozen=True)
class SurfaceRule:
    """一条 surface 块规则：condition 标签 + block 名（+ 可选阈值参数）。

    condition 是自由文本标签（如 "snowline_above" / "swale_below"），由 profile
    解释——P2 schema 只锁结构，把语义留给迁移期映射，避免过早把所有条件枚举死。
    """

    block: str
    condition: str = "default"
    params: Mapping[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class SurfacePaletteSpec:
    """surface_id 选择规则集（有序，后命中覆盖前者，与 np.where 链同语义）。"""

    rules: tuple[SurfaceRule, ...] = ()


@dataclass(frozen=True)
class FloraEntry:
    """一条 flora 散布：variant 标识 + 密度曲线条件 + band/pick 阈值。"""

    variant: str
    density: float = 0.0
    band_condition: str = "default"
    params: Mapping[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class FloraTableSpec:
    """作物/植被集 + 地表覆盖集（与 LAYER_REGISTRY 的 flora_* / ground_cover_*）。"""

    variants: tuple[FloraEntry, ...] = ()
    ground_cover: tuple[FloraEntry, ...] = ()


@dataclass(frozen=True)
class ZoneTerrainDSL:
    """一个 zone 的完整声明式地形定义（P2 DSL 顶层）。

    terrain_style / surface_palette / flora_table 描述几何 + 表面 + 植被；
    qi_grade / qi_override 描述灵气（§8.1 #4，P2 只锁 schema，场生成在 P4）。
    """

    terrain_style: TerrainStyleSpec
    surface_palette: SurfacePaletteSpec = field(default_factory=SurfacePaletteSpec)
    flora_table: FloraTableSpec = field(default_factory=FloraTableSpec)
    qi_grade: QiGrade = QiGrade.COMMON
    qi_override: float | None = None

    def __post_init__(self) -> None:
        # qi_override 落在 native 值域 [-1,1]，越界报错（硬约束，不静默 clamp）。
        validate_qi_override(self.qi_override)


# ---------------------------------------------------------------------------
# DSL 解析（dict → dataclass）—— 供 blueprint / profile catalog JSON 解析复用
# ---------------------------------------------------------------------------


def _parse_op_step(raw: Mapping[str, Any]) -> OpStep:
    return OpStep(
        category=str(raw.get("category", "")),
        op=str(raw["op"]),
        params=dict(raw.get("params", {})),
    )


def parse_terrain_style(raw: Mapping[str, Any] | None) -> TerrainStyleSpec:
    if not raw:
        return TerrainStyleSpec()
    return TerrainStyleSpec(
        height_ops=tuple(_parse_op_step(s) for s in raw.get("height_ops", [])),
        carve_ops=tuple(_parse_op_step(s) for s in raw.get("carve_ops", [])),
        features=tuple(str(f) for f in raw.get("features", [])),
    )


def parse_surface_palette(raw: Mapping[str, Any] | None) -> SurfacePaletteSpec:
    if not raw:
        return SurfacePaletteSpec()
    rules = tuple(
        SurfaceRule(
            block=str(r["block"]),
            condition=str(r.get("condition", "default")),
            params=dict(r.get("params", {})),
        )
        for r in raw.get("rules", [])
    )
    return SurfacePaletteSpec(rules=rules)


def parse_flora_table(raw: Mapping[str, Any] | None) -> FloraTableSpec:
    if not raw:
        return FloraTableSpec()

    def _entry(r: Mapping[str, Any]) -> FloraEntry:
        return FloraEntry(
            variant=str(r["variant"]),
            density=float(r.get("density", 0.0)),
            band_condition=str(r.get("band_condition", "default")),
            params=dict(r.get("params", {})),
        )

    return FloraTableSpec(
        variants=tuple(_entry(r) for r in raw.get("variants", [])),
        ground_cover=tuple(_entry(r) for r in raw.get("ground_cover", [])),
    )


def parse_zone_terrain_dsl(raw: Mapping[str, Any]) -> ZoneTerrainDSL:
    """从 blueprint zone 的 DSL 子节点解析出 ``ZoneTerrainDSL``.

    缺省时退回安全默认（空 terrain_style / common qi_grade / 无 override），保持
    旧 blueprint（无 DSL 字段）向后兼容。
    """
    qi_grade_raw = raw.get("qi_grade", QiGrade.COMMON.value)
    qi_grade = (
        qi_grade_raw
        if isinstance(qi_grade_raw, QiGrade)
        else parse_qi_grade(str(qi_grade_raw))
    )
    override_raw = raw.get("qi_override", None)
    qi_override = None if override_raw is None else float(override_raw)
    return ZoneTerrainDSL(
        terrain_style=parse_terrain_style(raw.get("terrain_style")),
        surface_palette=parse_surface_palette(raw.get("surface_palette")),
        flora_table=parse_flora_table(raw.get("flora_table")),
        qi_grade=qi_grade,
        qi_override=qi_override,
    )


# ---------------------------------------------------------------------------
# TerrainStyle 解释器 —— 声明式 height op 链 + carve op 链 → height 场
# ---------------------------------------------------------------------------


def _split_op_params(
    params: Mapping[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    """从 op params 里分离出解释器控制键（以 ``$`` 开头）与算子真实 kwargs."""
    control: dict[str, Any] = {}
    kwargs: dict[str, Any] = {}
    for key, value in params.items():
        if key.startswith("$"):
            control[key] = value
        else:
            kwargs[key] = value
    return control, kwargs


def evaluate_height_ops(
    steps: Iterable[OpStep],
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    context: Mapping[str, Any] | None = None,
) -> np.ndarray:
    """求值 height op 链：逐 op 算出贡献场，compose（相加）成 base height。

    每个 ``OpStep.params`` 是算子的 kwargs；以 ``$`` 前缀的键是解释器控制位
    （目前支持 ``$weight`` 线性缩放该项贡献）。``context`` 注入 profile 级共享
    参数（center_xz / half_w / half_d 等）供 op 复用，op 自身 params 覆盖之。
    """
    ctx = dict(context or {})
    contributions: list[np.ndarray] = []
    for step in steps:
        fn = resolve_op(step.category, step.op)
        control, kwargs = _split_op_params(step.params)
        merged = {**ctx, **kwargs}
        # 只透传该算子签名接受的 context 键，避免把无关 context 灌进 op。
        contribution = _call_op(fn, wx, wz, merged)
        weight = float(control.get("$weight", 1.0))
        if weight != 1.0:
            contribution = contribution * weight
        contributions.append(np.asarray(contribution, dtype=np.float64))
    if not contributions:
        return np.zeros_like(wx, dtype=np.float64)
    return compose_height(*contributions)


def evaluate_carve_ops(
    steps: Iterable[OpStep],
    height: np.ndarray,
    *,
    fields: Mapping[str, np.ndarray],
) -> np.ndarray:
    """依次施加 carve op 链到 height 场。

    carve op 的非 (wx,wz) 输入（rift_axis_sdf / fracture_mask / neg_pressure ...）
    从 ``fields`` 取。每个 carve op 的 params 给标量参数（threshold/strength），
    其 mask/sdf ndarray 入参由 op params 里的字符串 ``"field:<name>"`` 引用解析。
    """
    out = np.asarray(height, dtype=np.float64)
    for step in steps:
        fn = resolve_op(step.category, step.op)
        _control, kwargs = _split_op_params(step.params)
        resolved = _resolve_field_refs(kwargs, fields)
        out = fn(out, **resolved)
    return out


def _resolve_field_refs(
    kwargs: Mapping[str, Any], fields: Mapping[str, np.ndarray]
) -> dict[str, Any]:
    """把 kwargs 里的 ``"field:<name>"`` 字符串替换为 ``fields[name]`` ndarray."""
    out: dict[str, Any] = {}
    for key, value in kwargs.items():
        if isinstance(value, str) and value.startswith("field:"):
            field_name = value[len("field:") :]
            if field_name not in fields:
                available = ", ".join(sorted(fields)) or "(none)"
                raise KeyError(
                    f"carve op references unknown field {field_name!r}; "
                    f"available fields: {available}"
                )
            out[key] = fields[field_name]
        else:
            out[key] = value
    return out


def _call_op(
    fn: OpFn, wx: np.ndarray, wz: np.ndarray, merged: Mapping[str, Any]
) -> np.ndarray:
    """以 (wx, wz, **kwargs) 调 op，只透传 op 签名里实际存在的 kwargs。

    这样 ``context`` 可以携带超集参数（center_xz / half_w / half_d / seeds），每个
    op 只取自己需要的那几个，互不干扰。
    """
    import inspect

    sig = inspect.signature(fn)
    accepted = set(sig.parameters)
    has_var_kw = any(
        p.kind == inspect.Parameter.VAR_KEYWORD for p in sig.parameters.values()
    )
    if has_var_kw:
        call_kwargs = dict(merged)
    else:
        call_kwargs = {k: v for k, v in merged.items() if k in accepted}
    return fn(wx, wz, **call_kwargs)


def evaluate_terrain_style(
    style: TerrainStyleSpec,
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    context: Mapping[str, Any] | None = None,
    fields: Mapping[str, np.ndarray] | None = None,
) -> np.ndarray:
    """端到端求值一个 TerrainStyleSpec → height 场（基底链 compose → carve 链施加）。

    ``context`` 注入 profile 级共享参数给 height ops；``fields`` 提供 carve ops 的
    mask/sdf 输入（rift_axis_sdf / fracture_mask / neg_pressure 等，迁移期由 mask
    ops 或自定义算子先算出来）。
    """
    base = evaluate_height_ops(style.height_ops, wx, wz, context=context)
    if style.carve_ops:
        base = evaluate_carve_ops(style.carve_ops, base, fields=fields or {})
    return base
