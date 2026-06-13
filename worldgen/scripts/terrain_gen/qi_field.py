"""worldgen-v4 P4 — 统一灵气场（一份场，两种导出）.

把灵气从"两份漂移"（worldgen ``qi_density`` raster vs 运行时 zone ``spirit_qi``）
重构为**一份统一灵气场两种导出**。

* **统一灵气场** native 值域 **[-1, 1]**（§8.1 #4），直接覆盖 worldview §二 三层
  环境语义：负灵域 [-1,-0.1) / 死域 [-0.1,0.02) / 馈赠区 (>0)。场由三部分合成：

    1. **base** —— 末法荒野基线（薄灵），全图低浓度常量。
    2. **zone 贡献核** —— 每个 zone 按 ``qi_grade`` 六档铺一个目标浓度核（径向
       falloff 由核心目标值向 base 过渡），可选 ``qi_override`` 连续值精调。
    3. **灵脉网络** —— 沿地形特征走线的稀疏线性灵脉，抬高沿脉浓度（正灵域专属，
       死域 / 负灵域不长脉）。

  ``qi_override`` 作为**硬约束**铺场：override 直接覆写核心区目标值，不被 base /
  veins 稀释（§8.1 #4 #2，override 精调浓度但**不豁免守恒**——配平在导出期由
  wilderness + 非 override 区域吸收差额，本段不实现配平，留给后续段）。

* **``qi_density`` raster** = ``clamp01((field+1)/2)`` 同源派生（[0,1]，维持
  ``harness/raster_check.py`` 值域规则、POI selector、``profiles/*.py`` 现有
  ``qi_base = zone.spirit_qi`` 单向依赖的兼容）。

* **zone ``spirit_qi``** = zone bounds 内场的**面积加权均值** clamp[-1,1]
  → 写 ``server/zones.json``（导出在后续段）。

本模块是**纯函数 / 可独立单测**：输入 numpy 场 / 标量参数 → 输出 numpy 场 / 标量，
不做任何 IO、不依赖 pipeline 状态、不写文件。派生公式 ``qi_density_from_field`` /
``spirit_qi_from_field`` 是 worldgen 与运行时 + raster_check 共享的**单一真相**。
"""

from __future__ import annotations

from collections.abc import Sequence

import numpy as np

from .dsl import (
    QI_FIELD_MAX,
    QI_FIELD_MIN,
    QI_GRADE_BOUNDS,
    QiGrade,
    qi_grade_from_value,
    validate_qi_override,
)
from .noise import ridge_2d

# ---------------------------------------------------------------------------
# 派生公式的容差（raster_check 同源硬断言用，§8.1 #8）。
#
# qi_density raster 与 zone spirit_qi 来自同一 [-1,1] 场两种导出：
#     qi_density   = clamp01((field+1)/2)
#     spirit_qi    = 面积加权均值(field) clamp[-1,1]
# 故 zone 内 qi_density 的均值 ≈ clamp01((spirit_qi+1)/2)，偏差只来自场的空间
# 噪声（灵脉抬升 / 边界过渡）。容差取 0.2：足够吸收 zone 内噪声波动（rift 核心
# 峰与边缘差 0.3+ 会被 zone 均值平滑掉一半），又能撞住真正的"两份漂移"（独立
# 生成的 qi_density 与声明 spirit_qi 系统性偏离 0.5+ 量级）。
# ---------------------------------------------------------------------------
QI_DENSITY_TOLERANCE = 0.2

# 末法荒野基线 native 灵气值（薄灵）。对齐 LAYER_REGISTRY qi_density safe_default
# = 0.12 的反推：clamp01((field+1)/2) = 0.12 → field = -0.76。
QI_FIELD_BASE = -0.76


def _clamp01(value: np.ndarray | float) -> np.ndarray | float:
    return np.clip(value, 0.0, 1.0)


def qi_density_from_field(field: np.ndarray | float) -> np.ndarray | float:
    """统一场 [-1,1] → ``qi_density`` raster [0,1]：``clamp01((field+1)/2)``.

    这是 worldgen / 运行时 / raster_check 共享的**单一派生真相**。接受 ndarray 或
    标量。field 已在 [-1,1] 时输出落 [0,1]；越界 field 由 clamp01 收口。
    """
    return _clamp01((np.asarray(field, dtype=np.float64) + 1.0) / 2.0)


def spirit_qi_from_density(density: np.ndarray | float) -> np.ndarray | float:
    """``qi_density`` [0,1] → native 场 [-1,1] 的**逆派生**：``2*density - 1``.

    与 ``qi_density_from_field`` 互逆（在 [-1,1] 值域内）。raster_check 用它把声明
    的 zone ``spirit_qi`` 投影回 qi_density 期望值做同源比对（等价于
    ``qi_density_from_field(spirit_qi)``，单列以表意：从 density 视角看的逆映射）。
    """
    return np.clip(2.0 * np.asarray(density, dtype=np.float64) - 1.0,
                   QI_FIELD_MIN, QI_FIELD_MAX)


def spirit_qi_from_field(
    field: np.ndarray,
    weights: np.ndarray | None = None,
) -> float:
    """zone bounds 内场 → zone 标量 ``spirit_qi``：面积加权均值 clamp[-1,1]（§8.1 #4）.

    ``field`` 是 zone 覆盖列的 native 灵气值（任意 shape，会 ravel）；``weights``
    可选同 shape 面积权重（None = 等权，即纯面积均值）。空场 / 全零权重报错（无法
    定义均值，宁炸不静默返回 0 掩盖配置错误）。结果 clamp 到 native 值域端点，
    防止极端 override 把均值推出 [-1,1]（server validate_zone 硬区间）。
    """
    arr = np.asarray(field, dtype=np.float64).ravel()
    if arr.size == 0:
        raise ValueError("spirit_qi_from_field: empty field has no defined mean")
    if weights is None:
        mean = float(arr.mean())
    else:
        w = np.asarray(weights, dtype=np.float64).ravel()
        if w.shape != arr.shape:
            raise ValueError(
                f"spirit_qi_from_field: weights shape {w.shape} != field shape "
                f"{arr.shape}"
            )
        total = float(w.sum())
        if total <= 0.0:
            raise ValueError(
                "spirit_qi_from_field: weights sum to <= 0, cannot weight-average"
            )
        mean = float((arr * w).sum() / total)
    return float(np.clip(mean, QI_FIELD_MIN, QI_FIELD_MAX))


def qi_grade_target(grade: QiGrade) -> float:
    """一档 ``QiGrade`` → 该档**代表性 native 目标值**（铺核中心值）.

    取档区间 [lo, hi) 的中点作目标浓度（zone 核心区铺到这个值，向 base 过渡）。
    端档（NEGATIVE / FONT）半开 / 闭合到 native 端点，中点仍落档区间内：
      negative [-1,-0.1)  → -0.55
      dead     [-0.1,0.02)→ -0.04
      trace    [0.02,0.15)→  0.085
      common   [0.15,0.45)→  0.30
      rich     [0.45,0.7) →  0.575
      font     [0.7,1.0]  →  0.85
    保证 ``qi_grade_from_value(qi_grade_target(g)) == g``（往返自洽，单测锁定）。
    """
    lo, hi = QI_GRADE_BOUNDS[grade]
    return (lo + hi) / 2.0


def zone_contribution_kernel(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    center_xz: tuple[float, float],
    half_w: float,
    half_d: float,
    target: float,
    base: float = QI_FIELD_BASE,
    falloff_exponent: float = 1.6,
    plateau_ratio: float = 0.0,
) -> np.ndarray:
    """单个 zone 的灵气贡献核：核心铺 ``target``，径向向 ``base`` 过渡.

    径向归一距离 ``r = sqrt((dx/half_w)^2 + (dz/half_d)^2)``。

    * ``plateau_ratio == 0``（默认，向后兼容）：纯尖核掩码
      ``m = clamp(1 - r**exp, 0, 1)``（中心=1，edge→0）—— 仅中心点接近 target。
    * ``plateau_ratio ∈ (0, 1)``：**平顶核**——``r <= plateau_ratio`` 的内圈整片
      铺死 target（mask=1），在 ``(plateau_ratio, 1]`` 的过渡环里按 exp 衰减到 base。
      这让 zone 的**整片 bounds**读到它声明的档位浓度（而非只有圆心），过渡发生在
      bounds 外缘——对应"zone 内部均匀目标浓度、向 wilderness 过渡"的物理语义。
      派生 zone spirit_qi（bounds 内面积加权均值）时必须用平顶核，否则均值被 edge
      的 base 拉低、所有 zone 误判为负灵域。

    核值 ``field = base + m * (target - base)``。返回的核**未 clamp**到 [-1,1]——
    合成阶段（``build_qi_field``）统一 clamp，使多核叠加 + veins 抬升的中间量不被
    过早截断。
    """
    if falloff_exponent <= 0.0:
        # exponent==0 让 radial**0==1（整核退回 base）；负数在中心点触发 0**负
        # （核心到不了 target）。ZoneQiInput.falloff_exponent 会原样透传进来，一次
        # 配置错误就会把整片 zone 的导出值算错且无显式失败——这里直接拒绝。
        raise ValueError(
            f"falloff_exponent must be > 0; got {falloff_exponent}"
        )
    if not (0.0 <= plateau_ratio < 1.0):
        raise ValueError(
            f"plateau_ratio must be in [0, 1); got {plateau_ratio} "
            "(1.0 would make the kernel a flat top with no falloff to base)"
        )
    cx, cz = center_xz
    dx = (wx - cx) / max(half_w, 1.0)
    dz = (wz - cz) / max(half_d, 1.0)
    radial = np.sqrt(dx * dx + dz * dz)
    if plateau_ratio <= 0.0:
        mask = np.clip(1.0 - radial**falloff_exponent, 0.0, 1.0)
    else:
        # 平顶：内圈 r<=plateau_ratio 全 1；过渡环把 r 线性重映射到 [0,1] 再施 exp 衰减。
        denom = max(1.0 - plateau_ratio, 1e-6)
        transition = np.clip((radial - plateau_ratio) / denom, 0.0, 1.0)
        mask = np.clip(1.0 - transition**falloff_exponent, 0.0, 1.0)
    return base + mask * (target - base)


def vein_network_field(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    seed: int = 0,
    scale: float = 320.0,
    ridge_threshold: float = 0.92,
    strength: float = 0.22,
) -> np.ndarray:
    """灵脉网络抬升场：沿 ridge 噪声脊线（地形特征走线）的稀疏正向抬升 [0, strength].

    用 ridge 噪声（脊线锐利、~[0.24,1.0]，分布偏高 mean≈0.77）阈值化成**稀疏**线状
    网络：``ridge > threshold`` 的列抬升 ``(ridge - threshold)/(1 - threshold) *
    strength``，其余 0。threshold 取 0.92（ridge 高位尾部）→ 约 1/5 列成脉，余皆
    无脉，呈"沿地形特征走线"的稀疏灵脉主干而非全图铺满。返回**非负**抬升量
    （veins 只加不减，对齐 LAYER_REGISTRY ``qi_vein_flow`` 的 maximum blend 语义）。
    """
    ridge = ridge_2d(wx, wz, scale=scale, octaves=4, seed=seed)
    denom = max(1.0 - ridge_threshold, 1e-6)
    lift = np.clip((ridge - ridge_threshold) / denom, 0.0, 1.0) * strength
    return lift


def _spec_true_bounds(spec: dict) -> tuple[float, float, float, float]:
    """一个 zone_kernel spec 的**真实 AABB**（min_x, max_x, min_z, max_z）.

    优先取 spec 显式携带的 ``bounds_xz``（``kernel_spec_from_grade`` 从**未放大**的
    half-extent 写入——footprint_scale 只放大 falloff 核的 half_w/half_d，不动真实
    bounds）。缺省时（手搓 dict spec）从 ``center_xz ± half_w/half_d`` 反推——这些
    手搓 spec 的 half 即真实半径，未经 footprint 放大。

    真实 bounds 是 in-AABB 主导规则的依据：zone 自身 bounds 内由它自己的核以 target
    铺满，邻核的 falloff（仅伸进 wilderness）不得越界压过。
    """
    bounds = spec.get("bounds_xz")
    if bounds is not None:
        bx0, bx1, bz0, bz1 = bounds
        return float(bx0), float(bx1), float(bz0), float(bz1)
    cx, cz = spec["center_xz"]
    half_w = float(spec["half_w"])
    half_d = float(spec["half_d"])
    return cx - half_w, cx + half_w, cz - half_d, cz + half_d


def build_qi_field(
    wx: np.ndarray,
    wz: np.ndarray,
    *,
    zone_kernels: Sequence[dict] = (),
    base: float = QI_FIELD_BASE,
    vein_seed: int = 0,
    vein_strength: float = 0.22,
    enable_veins: bool = True,
) -> np.ndarray:
    """合成全图统一灵气场 [-1, 1]：base + bounds 主导的 zone 核 + wilderness 过渡 + 灵脉.

    ``zone_kernels`` 每项是一个 dict：
        center_xz, half_w, half_d, target （必填）
        bounds_xz （可选 (min_x,max_x,min_z,max_z) 真实 AABB；缺省从 center±half 反推）
        falloff_exponent （可选，默认 1.6）
        override （可选 float | None；非 None 时该核为**硬约束核**，核心区直接铺
                  override 值且不被 veins 抬升稀释——§8.1 #4 #2）

    **合成规则（bounds-confined + specificity-ordered，修 footprint over-reach）**：

      1. 从全图 ``base`` 起步。
      2. **wilderness 过渡层**：对每个核，仅在**不属于任何 zone 真实 AABB** 的列上
         取逐元素最大叠加（核的 falloff 尾巴只伸进 wilderness，给 raster 平滑过渡）。
      3. **in-AABB 主导层**：落在 ≥1 个 zone 真实 AABB 内的列，由**面积最小（最具体）
         的覆盖 zone** 主导，整片铺该 zone 的 ``eff_target``（平顶 = target）。这让：
           · 每个 zone 自身 bounds 内读它声明的档位（派生 spirit_qi 落回声明档），
           · 嵌套小 zone（rift_mouth 落在 blood_valley 内）覆盖外层大 zone，
           · **邻核 footprint 不再越界污染**——邻 zone 的 falloff 只在 wilderness 生效，
             伸不进别的 zone 的 bounds（max-blend over-reach 根因消除）。
         同面积平手时取**较大值**（退化回 max-blend，共心核测试仍由强源主导）。
      4. **灵脉抬升**只作用在正灵域（field > 0 的列），死域 / 负灵域不长脉。
      5. **override 硬约束**：override zone 的 AABB 列铺 override 值且跳过 veins 抬升。
      6. 全图 clamp 到 [-1, 1]。

    返回 native 灵气场（float64，同 ``wx`` shape）。纯函数，无 IO。
    """
    wx_arr = np.asarray(wx, dtype=np.float64)
    wz_arr = np.asarray(wz, dtype=np.float64)
    field = np.full_like(wx_arr, float(base))
    # override 硬约束掩码：记录哪些列被 override zone 的 AABB 锁定（跳过 veins 稀释）。
    override_lock = np.zeros_like(field, dtype=bool)

    # 预处理每个 spec：算 falloff 核、真实 AABB inside 掩码、eff_target、面积。
    processed: list[dict] = []
    for spec in zone_kernels:
        center = spec["center_xz"]
        half_w = float(spec["half_w"])
        half_d = float(spec["half_d"])
        target = float(spec["target"])
        exponent = float(spec.get("falloff_exponent", 1.6))
        plateau_ratio = float(spec.get("plateau_ratio", 0.0))
        override = spec.get("override", None)
        eff_target = target
        if override is not None:
            # override 是硬约束铺场值——校验 native 值域，AABB 内铺 override。
            eff_target = validate_qi_override(float(override))
        kernel = zone_contribution_kernel(
            wx_arr,
            wz_arr,
            center_xz=center,
            half_w=half_w,
            half_d=half_d,
            target=eff_target,
            base=base,
            falloff_exponent=exponent,
            plateau_ratio=plateau_ratio,
        )
        bx0, bx1, bz0, bz1 = _spec_true_bounds(spec)
        inside = (
            (wx_arr >= bx0) & (wx_arr <= bx1)
            & (wz_arr >= bz0) & (wz_arr <= bz1)
        )
        area = max(bx1 - bx0, 0.0) * max(bz1 - bz0, 0.0)
        processed.append(
            {
                "kernel": kernel,
                "inside": inside,
                "eff_target": eff_target,
                "area": area,
                "is_override": override is not None,
            }
        )

    if processed:
        # 任一 zone AABB 覆盖的列（in-AABB 主导层 vs wilderness 过渡层的分界）。
        any_inside = np.zeros_like(field, dtype=bool)
        for p in processed:
            any_inside |= p["inside"]

        # (2) wilderness 过渡层：核 falloff 只在不属任何 AABB 的列上 max 叠加。
        wild = ~any_inside
        for p in processed:
            field = np.where(wild, np.maximum(field, p["kernel"]), field)

        # (3) in-AABB 主导层：面积大→小依次写入（小面积最后写 = 最具体者胜）；
        #     同面积按 eff_target 升序（大值最后写 = 平手取强源，退化回 max-blend）。
        for p in sorted(processed, key=lambda q: (-q["area"], q["eff_target"])):
            field = np.where(p["inside"], p["eff_target"], field)
            if p["is_override"]:
                override_lock |= p["inside"]

    if enable_veins:
        veins = vein_network_field(wx_arr, wz_arr, seed=vein_seed,
                                   strength=vein_strength)
        # 灵脉只抬升正灵域（field > 0），死域 / 负灵域不长脉；override 锁定列不抬升。
        liftable = (field > 0.0) & (~override_lock)
        field = np.where(liftable, field + veins, field)

    return np.clip(field, QI_FIELD_MIN, QI_FIELD_MAX)


# 平顶核默认形参。``build_qi_field`` 的 in-AABB 主导层已**精确铺满** zone 真实 bounds
# 为 target（不再靠核 footprint 把均值抬回档位），故 footprint / plateau 现在**只塑形
# wilderness 过渡**（核 falloff 尾巴伸进 zone 外的余区，给 raster 平滑过渡）：
#   FOOTPRINT —— falloff 核 half-extent 相对 zone 半径的放大倍率，让过渡环伸进
#                wilderness（in-AABB 列已被 eff_target 覆盖，不受此放大影响）。
#   PLATEAU   —— falloff 核平顶占比（过渡环起点），保证 AABB 外缘附近不会骤降到 base。
GRADE_KERNEL_FOOTPRINT = 1.7
GRADE_KERNEL_PLATEAU_RATIO = 0.6


def kernel_spec_from_grade(
    *,
    center_xz: tuple[float, float],
    half_w: float,
    half_d: float,
    grade: QiGrade,
    override: float | None = None,
    falloff_exponent: float = 1.6,
    footprint_scale: float = GRADE_KERNEL_FOOTPRINT,
    plateau_ratio: float = GRADE_KERNEL_PLATEAU_RATIO,
) -> dict:
    """便捷构造一个 ``build_qi_field`` zone_kernel dict（按 qi_grade 取目标值）.

    ``grade`` 决定核心目标值（``qi_grade_target``）；``override`` 非 None 时为硬约束
    （精调浓度铺死）。spec 携带 zone 的**真实 AABB** ``bounds_xz``（从**未放大**的
    ``half_w/half_d`` 算）—— ``build_qi_field`` 的 in-AABB 主导层据此把 zone 自身
    bounds 整片铺 target，保证 **zone 面积加权均值落回声明档位**，且**邻核 footprint
    不越界污染**（footprint over-reach 根因消除）。``footprint_scale`` / ``plateau_ratio``
    现在只放大 falloff 核塑形 wilderness 过渡，不影响 in-AABB 读数。返回的 dict 直接
    喂给 ``build_qi_field(zone_kernels=[...])``。
    """
    cx, cz = center_xz
    return {
        "center_xz": center_xz,
        # 真实 AABB（未放大）—— in-AABB 主导规则的依据。
        "bounds_xz": (cx - half_w, cx + half_w, cz - half_d, cz + half_d),
        "half_w": half_w * footprint_scale,
        "half_d": half_d * footprint_scale,
        "target": qi_grade_target(grade),
        "falloff_exponent": falloff_exponent,
        "plateau_ratio": plateau_ratio,
        "override": override,
    }


__all__ = [
    "QI_DENSITY_TOLERANCE",
    "QI_FIELD_BASE",
    "qi_density_from_field",
    "spirit_qi_from_density",
    "spirit_qi_from_field",
    "qi_grade_target",
    "zone_contribution_kernel",
    "vein_network_field",
    "build_qi_field",
    "kernel_spec_from_grade",
    "qi_grade_from_value",
]
