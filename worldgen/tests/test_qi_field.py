"""worldgen-v4 P4 §8.1 #3/#4/#8 — 统一灵气场饱和单测.

锁四件事（一份统一场两种导出，派生函数是单一真相）：

1. **派生函数正反 pin（富集区 / 死域 / 负灵域三态值域）** —— ``qi_density_from_field``
   = clamp01((field+1)/2) 在三态 native 区间各取代表值断言落点，边界
   (-1 / 0 / +1 / base) off-by-one，越界 field clamp 收口。
2. **``qi_density = clamp01((field+1)/2)`` 对拍** —— 公式逐元素精确，标量 / ndarray
   两路，与 ``spirit_qi_from_density`` 逆派生互逆。
3. **同源闭环** —— 改 field（抬升 / 压低）→ ``qi_density`` 与 ``spirit_qi``
   **同步同向**变化（一份场两种导出，不再两份漂移）。
4. **场合成 + 配平接口** —— ``build_qi_field`` 三态 zone 核值域、override 硬约束铺死、
   max-blend 叠加、灵脉只抬正灵域、全图 clamp；``spirit_qi_from_field`` 面积加权 +
   错误分支；``qi_grade_target`` 往返自洽。
"""

from __future__ import annotations

import unittest

import numpy as np

from scripts.terrain_gen import qi_field as qf
from scripts.terrain_gen.dsl import (
    QI_FIELD_MAX,
    QI_FIELD_MIN,
    QiGrade,
    qi_grade_from_value,
)
from scripts.terrain_gen.noise import _tile_coords


def _centered_grid(size: int = 200):
    """A tile centred on (0,0) so radial kernels land real samples around origin."""
    return _tile_coords(-size // 2, -size // 2, size)


# ===========================================================================
# 1. 派生函数正反 pin —— 富集区 / 死域 / 负灵域三态值域
# ===========================================================================


class QiDensityDerivationTest(unittest.TestCase):
    def test_boundary_values_exact(self) -> None:
        # 三个 native 端点 + 中点的精确派生（clamp01((field+1)/2)）。
        cases = [
            (QI_FIELD_MIN, 0.0, "native -1 (深负灵域) → qi_density 0.0"),
            (0.0, 0.5, "native 0 (零灵) → qi_density 0.5"),
            (QI_FIELD_MAX, 1.0, "native +1 (灵源极值) → qi_density 1.0"),
            (qf.QI_FIELD_BASE, 0.12, "末法基线 base=-0.76 → qi_density ~0.12"),
        ]
        for field_v, expected, why in cases:
            got = float(qf.qi_density_from_field(field_v))
            self.assertAlmostEqual(
                got, expected, places=4,
                msg=f"{why}: expected {expected}, got {got}",
            )

    def test_three_state_ranges(self) -> None:
        # 富集区 (>0) → density > 0.5；死域 (≈0) → density ≈ 0.5；
        # 负灵域 (<0) → density < 0.5。三态分别落在 [0,1] 的对应半区。
        rich = float(qf.qi_density_from_field(0.85))  # font 富集区
        dead = float(qf.qi_density_from_field(-0.04))  # 死域档中点
        negative = float(qf.qi_density_from_field(-0.55))  # 负灵域档中点
        self.assertGreater(
            rich, 0.5,
            f"富集区 native 0.85 must map above 0.5; got {rich}",
        )
        self.assertAlmostEqual(
            dead, 0.48, places=2,
            msg=f"死域 native -0.04 maps near 0.5 (零灵); got {dead}",
        )
        self.assertLess(
            negative, 0.5,
            f"负灵域 native -0.55 must map below 0.5; got {negative}",
        )
        # 单调性：富集 > 死域 > 负灵（同源派生保序）。
        self.assertGreater(rich, dead)
        self.assertGreater(dead, negative)

    def test_out_of_range_field_clamps(self) -> None:
        # 越界 native（理论不该出现，但派生必须 clamp 收口不溢出 [0,1]）。
        self.assertEqual(float(qf.qi_density_from_field(-3.0)), 0.0)
        self.assertEqual(float(qf.qi_density_from_field(5.0)), 1.0)

    def test_array_path_matches_scalar_path(self) -> None:
        # ndarray 路径与逐标量路径逐元素一致（向量化无偏差）。必须真比向量化输出
        # arr[idx] vs 标量分支——否则两边都跑标量、向量化分叉也照样绿。
        field = np.array([-1.0, -0.5, 0.0, 0.5, 1.0])
        arr = np.asarray(qf.qi_density_from_field(field))
        for idx, v in enumerate(field):
            scalar = float(qf.qi_density_from_field(float(v)))
            self.assertAlmostEqual(
                float(arr[idx]),
                scalar,
                msg=(
                    f"array path value at idx={idx} (field={float(v)}) must match "
                    f"scalar path {scalar} (向量化无偏差), got {float(arr[idx])}"
                ),
            )
        np.testing.assert_allclose(arr, (field + 1.0) / 2.0)


# ===========================================================================
# 2. qi_density = clamp01((field+1)/2) 对拍 + 逆派生互逆
# ===========================================================================


class QiDensityFormulaPinTest(unittest.TestCase):
    def test_formula_matches_reference_elementwise(self) -> None:
        field = np.linspace(-1.0, 1.0, 41)
        got = np.asarray(qf.qi_density_from_field(field))
        reference = np.clip((field + 1.0) / 2.0, 0.0, 1.0)
        np.testing.assert_allclose(
            got, reference, atol=1e-12,
            err_msg="qi_density_from_field must equal clamp01((field+1)/2)",
        )

    def test_inverse_round_trips_inside_native_range(self) -> None:
        # density = (field+1)/2 ; spirit_qi_from_density 逆回 field（[-1,1] 内互逆）。
        for field_v in (-1.0, -0.55, -0.04, 0.085, 0.3, 0.575, 0.85, 1.0):
            density = float(qf.qi_density_from_field(field_v))
            back = float(qf.spirit_qi_from_density(density))
            self.assertAlmostEqual(
                back, field_v, places=6,
                msg=f"density {density} should invert to native {field_v}, got {back}",
            )

    def test_inverse_equals_qi_density_from_field_of_spirit_qi(self) -> None:
        # raster_check 的同源比对用 clamp01((spirit_qi+1)/2)；逆函数与正派生应等价。
        for sq in (-0.8, -0.15, 0.0, 0.05, 0.45, 0.7, 0.85):
            forward = float(qf.qi_density_from_field(sq))  # 把 spirit_qi 当 field 投影
            self.assertGreaterEqual(forward, 0.0)
            self.assertLessEqual(forward, 1.0)
            self.assertAlmostEqual(forward, (sq + 1.0) / 2.0, places=6)


# ===========================================================================
# 3. 同源闭环 —— 改 field → qi_density 与 spirit_qi 同步同向
# ===========================================================================


class SameSourceClosedLoopTest(unittest.TestCase):
    def test_lifting_field_raises_both_density_and_spirit_qi(self) -> None:
        # 一份场抬升 +0.3：qi_density 均值与 spirit_qi 必须同时升高（同源派生）。
        base_field = np.full((40, 40), 0.10, dtype=np.float64)
        lifted_field = np.clip(base_field + 0.3, QI_FIELD_MIN, QI_FIELD_MAX)

        dens_base = float(np.asarray(qf.qi_density_from_field(base_field)).mean())
        dens_lift = float(np.asarray(qf.qi_density_from_field(lifted_field)).mean())
        sq_base = qf.spirit_qi_from_field(base_field)
        sq_lift = qf.spirit_qi_from_field(lifted_field)

        self.assertGreater(
            dens_lift, dens_base,
            "抬升场后 qi_density 均值必须升高（同源派生）",
        )
        self.assertGreater(
            sq_lift, sq_base,
            "抬升场后 spirit_qi 必须升高（同源派生）",
        )
        # 同向幅度对齐：density 升幅 ≈ spirit_qi 升幅的一半（density 斜率 = 0.5）。
        self.assertAlmostEqual(
            dens_lift - dens_base, (sq_lift - sq_base) / 2.0, places=6,
            msg="qi_density 升幅必须恰为 spirit_qi 升幅的一半（派生斜率 0.5）",
        )

    def test_lowering_field_drops_both(self) -> None:
        # 反向：压低场（推入死域）→ 两个导出同步下降。
        base_field = np.full((30, 30), 0.30, dtype=np.float64)
        lowered = np.clip(base_field - 0.5, QI_FIELD_MIN, QI_FIELD_MAX)
        dens_base = float(np.asarray(qf.qi_density_from_field(base_field)).mean())
        dens_low = float(np.asarray(qf.qi_density_from_field(lowered)).mean())
        self.assertLess(dens_low, dens_base)
        self.assertLess(
            qf.spirit_qi_from_field(lowered), qf.spirit_qi_from_field(base_field)
        )

    def test_zone_spirit_qi_matches_density_mean_via_derivation(self) -> None:
        # zone 内 spirit_qi 与 qi_density 均值通过派生公式互推（同源闭环硬约束）。
        # 这正是 raster_check §8.1 #8 同源断言依赖的不变量。
        wx, wz = _centered_grid(160)
        ks = [qf.kernel_spec_from_grade(
            center_xz=(0, 0), half_w=70, half_d=70, grade=QiGrade.RICH,
        )]
        field = qf.build_qi_field(wx, wz, zone_kernels=ks, enable_veins=False)
        sq = qf.spirit_qi_from_field(field)
        density_mean = float(np.asarray(qf.qi_density_from_field(field)).mean())
        # spirit_qi = mean(field)，density_mean = mean(clamp01((field+1)/2))。
        # 因 clamp 不触发（field 全在 [-1,1]），density_mean == (sq+1)/2 精确。
        self.assertAlmostEqual(
            density_mean, (sq + 1.0) / 2.0, places=6,
            msg="zone density 均值必须 == clamp01((spirit_qi+1)/2)（同源闭环）",
        )


# ===========================================================================
# 4a. spirit_qi_from_field —— 面积加权均值 + clamp + 错误分支
# ===========================================================================


class SpiritQiFromFieldTest(unittest.TestCase):
    def test_unweighted_mean(self) -> None:
        field = np.array([[0.0, 0.4], [0.2, -0.2]])
        self.assertAlmostEqual(qf.spirit_qi_from_field(field), 0.1, places=6)

    def test_area_weighted_mean(self) -> None:
        # 权重把一格放大：加权均值偏向高权列。
        field = np.array([0.8, -0.2])
        weights = np.array([3.0, 1.0])  # 高灵格权重 3
        expected = (0.8 * 3 + (-0.2) * 1) / 4.0  # 0.55
        self.assertAlmostEqual(
            qf.spirit_qi_from_field(field, weights), expected, places=6
        )

    def test_clamps_to_native_range(self) -> None:
        # 极端 override 把均值推出 [-1,1] → clamp 到端点（server validate_zone 硬区间）。
        too_high = np.full(9, 1.5)
        too_low = np.full(9, -2.0)
        self.assertEqual(qf.spirit_qi_from_field(too_high), QI_FIELD_MAX)
        self.assertEqual(qf.spirit_qi_from_field(too_low), QI_FIELD_MIN)

    def test_empty_field_raises(self) -> None:
        with self.assertRaisesRegex(ValueError, "empty field"):
            qf.spirit_qi_from_field(np.array([]))

    def test_weight_shape_mismatch_raises(self) -> None:
        with self.assertRaisesRegex(ValueError, "weights shape"):
            qf.spirit_qi_from_field(np.zeros(4), np.zeros(3))

    def test_zero_weight_sum_raises(self) -> None:
        with self.assertRaisesRegex(ValueError, "weights sum"):
            qf.spirit_qi_from_field(np.ones(4), np.zeros(4))


# ===========================================================================
# 4b. qi_grade_target —— 往返自洽（每档 target 落回本档）
# ===========================================================================


class QiGradeTargetTest(unittest.TestCase):
    def test_target_round_trips_to_same_grade(self) -> None:
        # qi_grade_from_value(qi_grade_target(g)) == g，每档一条专属 case。
        for grade in QiGrade:
            target = qf.qi_grade_target(grade)
            back = qi_grade_from_value(target)
            self.assertEqual(
                back, grade,
                f"{grade.value} target={target:+.3f} must classify back as "
                f"{grade.value}, got {back.value}",
            )

    def test_targets_strictly_ascending_by_grade(self) -> None:
        # 六档目标值严格升序（负灵 < 死 < 微 < 常 < 丰 < 灵源）。
        order = (
            QiGrade.NEGATIVE, QiGrade.DEAD, QiGrade.TRACE,
            QiGrade.COMMON, QiGrade.RICH, QiGrade.FONT,
        )
        targets = [qf.qi_grade_target(g) for g in order]
        for lo, hi in zip(targets, targets[1:]):
            self.assertLess(lo, hi, f"grade targets must ascend; {lo} !< {hi}")


# ===========================================================================
# 4c. zone_contribution_kernel —— 核心=target, edge→base
# ===========================================================================


class ZoneContributionKernelTest(unittest.TestCase):
    def test_center_reaches_target_edge_decays_to_base(self) -> None:
        wx, wz = _centered_grid(200)
        kernel = qf.zone_contribution_kernel(
            wx, wz, center_xz=(0, 0), half_w=50, half_d=50,
            target=0.85, base=-0.76,
        )
        center = float(kernel[100, 100])
        corner = float(kernel[0, 0])  # 远离中心 → 退回 base
        self.assertAlmostEqual(
            center, 0.85, places=3,
            msg=f"kernel center must reach target 0.85; got {center}",
        )
        self.assertAlmostEqual(
            corner, -0.76, places=3,
            msg=f"kernel far corner must decay to base -0.76; got {corner}",
        )

    def test_negative_target_creates_sink(self) -> None:
        # 负灵域核：core 铺负目标（吞噬区），edge 退回 base（仍高于 core）。
        wx, wz = _centered_grid(120)
        kernel = qf.zone_contribution_kernel(
            wx, wz, center_xz=(0, 0), half_w=40, half_d=40,
            target=-0.55, base=-0.76,
        )
        # 负灵域核心 target=-0.55 高于 base=-0.76（base 更负），核是局部"抬升坑"。
        self.assertAlmostEqual(float(kernel[60, 60]), -0.55, places=3)


# ===========================================================================
# 4d. vein_network_field —— 非负抬升 + 稀疏
# ===========================================================================


class VeinNetworkFieldTest(unittest.TestCase):
    def test_lift_is_nonnegative_and_bounded(self) -> None:
        wx, wz = _centered_grid(256)
        veins = qf.vein_network_field(wx, wz, seed=7, strength=0.22)
        self.assertGreaterEqual(
            float(veins.min()), 0.0, "veins only add (>=0), never subtract"
        )
        self.assertLessEqual(
            float(veins.max()), 0.22 + 1e-9, "vein lift bounded by strength"
        )

    def test_sparse_not_everywhere(self) -> None:
        # 灵脉是稀疏线状网络，不是全图铺满：大部分列抬升应为 0。
        wx, wz = _centered_grid(256)
        veins = qf.vein_network_field(wx, wz, seed=3)
        zero_fraction = float((veins == 0.0).mean())
        self.assertGreater(
            zero_fraction, 0.5,
            f"veins must be sparse (>50% columns unlifted); got "
            f"{zero_fraction:.2f} zero fraction",
        )

    def test_deterministic_by_seed(self) -> None:
        wx, wz = _centered_grid(64)
        a = qf.vein_network_field(wx, wz, seed=11)
        b = qf.vein_network_field(wx, wz, seed=11)
        np.testing.assert_array_equal(a, b)


# ===========================================================================
# 4e. build_qi_field —— 三态 zone + override 硬约束 + max-blend + clamp
# ===========================================================================


class BuildQiFieldTest(unittest.TestCase):
    def test_three_state_zones_land_in_their_grade_ranges(self) -> None:
        # 富集 / 死域 / 负灵三态 zone 核中心值各落对应 qi_grade 区间。
        wx, wz = _centered_grid(200)
        cases = [
            (QiGrade.FONT, 0.7, 1.0),
            (QiGrade.DEAD, -0.1, 0.02),
            (QiGrade.NEGATIVE, -1.0, -0.1),
        ]
        for grade, lo, hi in cases:
            ks = [qf.kernel_spec_from_grade(
                center_xz=(0, 0), half_w=50, half_d=50, grade=grade,
            )]
            field = qf.build_qi_field(wx, wz, zone_kernels=ks, enable_veins=False)
            center = float(field[100, 100])
            self.assertTrue(
                lo <= center < hi or (hi == 1.0 and center == 1.0),
                f"{grade.value} zone center field={center:.3f} must land in "
                f"[{lo}, {hi}); got {center}",
            )

    def test_field_always_clamped_to_native_range(self) -> None:
        # 多核叠加 + veins 抬升后全图仍 clamp 到 [-1, 1]。
        wx, wz = _centered_grid(200)
        ks = [
            qf.kernel_spec_from_grade(
                center_xz=(0, 0), half_w=80, half_d=80, grade=QiGrade.FONT,
            ),
            qf.kernel_spec_from_grade(
                center_xz=(20, 20), half_w=60, half_d=60, grade=QiGrade.FONT,
            ),
        ]
        field = qf.build_qi_field(wx, wz, zone_kernels=ks, vein_strength=0.5)
        self.assertGreaterEqual(float(field.min()), QI_FIELD_MIN - 1e-9)
        self.assertLessEqual(float(field.max()), QI_FIELD_MAX + 1e-9)

    def test_override_is_hard_constraint_not_diluted_by_veins(self) -> None:
        # override 硬约束：核心区铺死 override 值，灵脉不抬升它（精调浓度铺死）。
        wx, wz = _centered_grid(120)
        ks = [{
            "center_xz": (0, 0), "half_w": 40, "half_d": 40,
            "target": qf.qi_grade_target(QiGrade.COMMON),
            "override": 0.5,  # 硬约束精调到 0.5
        }]
        # 即便开 veins（正灵域会被抬升），override 核心仍锁 0.5。
        field = qf.build_qi_field(wx, wz, zone_kernels=ks, vein_strength=0.3)
        center = float(field[60, 60])
        self.assertAlmostEqual(
            center, 0.5, places=3,
            msg=f"override=0.5 核心必须铺死 0.5（veins 不稀释硬约束）; got {center}",
        )

    def test_override_validates_native_range(self) -> None:
        # override 越界（>1）→ 校验报错（硬约束必须在 native 值域内）。
        wx, wz = _centered_grid(40)
        ks = [{
            "center_xz": (0, 0), "half_w": 10, "half_d": 10,
            "target": 0.3, "override": 1.5,
        }]
        with self.assertRaisesRegex(ValueError, "out of native qi field range"):
            qf.build_qi_field(wx, wz, zone_kernels=ks)

    def test_max_blend_strongest_source_wins(self) -> None:
        # 两核重叠区取最强：font 核 + dead 核重叠 → 重叠列由 font 主导。
        wx, wz = _centered_grid(160)
        ks = [
            qf.kernel_spec_from_grade(
                center_xz=(0, 0), half_w=60, half_d=60, grade=QiGrade.FONT,
            ),
            qf.kernel_spec_from_grade(
                center_xz=(0, 0), half_w=60, half_d=60, grade=QiGrade.DEAD,
            ),
        ]
        field = qf.build_qi_field(wx, wz, zone_kernels=ks, enable_veins=False)
        # 共心重叠 → max(font_target, dead_target) = font_target 主导。
        self.assertGreater(
            float(field[80, 80]), 0.7,
            "重叠区 max-blend 必须由 font 核主导（最强源胜）",
        )

    def test_empty_kernels_yields_base_field(self) -> None:
        # 无 zone 核（纯 wilderness）→ 全图 base（关 veins 时）。
        wx, wz = _centered_grid(40)
        field = qf.build_qi_field(wx, wz, zone_kernels=(), enable_veins=False)
        np.testing.assert_allclose(field, qf.QI_FIELD_BASE)

    def test_veins_only_lift_positive_domain(self) -> None:
        # 灵脉只抬正灵域：纯死域 base 场（field<0）开 veins 也不被抬升。
        wx, wz = _centered_grid(128)
        # base=-0.76 < 0 → 死域，无 zone 核，开 veins。
        field = qf.build_qi_field(wx, wz, zone_kernels=(), enable_veins=True)
        np.testing.assert_allclose(
            field, qf.QI_FIELD_BASE,
            err_msg="负灵域 / 死域不长灵脉：base<0 场开 veins 后仍全 base",
        )


# ===========================================================================
# 4f. zone_contribution_kernel plateau_ratio —— 平顶核（P4 第2段加）
# ===========================================================================


class PlateauKernelTest(unittest.TestCase):
    def test_plateau_zero_is_backward_compatible_tip_kernel(self) -> None:
        # plateau_ratio=0（默认）= 旧尖核：仅圆心到 target，edge 退 base。
        wx, wz = _centered_grid(200)
        tip = qf.zone_contribution_kernel(
            wx, wz, center_xz=(0, 0), half_w=50, half_d=50,
            target=0.85, base=-0.76, plateau_ratio=0.0,
        )
        center = float(tip[100, 100])
        self.assertAlmostEqual(center, 0.85, places=3)
        # 半径中点（r≈0.5）尖核已明显衰减（不是平顶）。
        mid = float(tip[100, 125])  # 25 cells from center, half_w=50 → r≈0.5
        self.assertLess(mid, 0.85 - 0.05, "尖核半径中点应已衰减，非平顶")

    def test_plateau_flattens_inner_disk_to_target(self) -> None:
        # plateau_ratio=0.6：内 60% 半径整片铺死 target（平顶），不止圆心。
        wx, wz = _centered_grid(200)
        flat = qf.zone_contribution_kernel(
            wx, wz, center_xz=(0, 0), half_w=50, half_d=50,
            target=0.85, base=-0.76, plateau_ratio=0.6,
        )
        center = float(flat[100, 100])
        inner = float(flat[100, 115])  # 15 cells, half_w=50 → r=0.3 < 0.6 → 平顶
        self.assertAlmostEqual(center, 0.85, places=3)
        self.assertAlmostEqual(
            inner, 0.85, places=3,
            msg="平顶核内 60% 半径必须整片铺 target（r=0.3 仍在平顶）",
        )

    def test_plateau_still_decays_to_base_far_out(self) -> None:
        # 平顶外仍衰减到 base（不是无限平顶）。
        wx, wz = _centered_grid(200)
        flat = qf.zone_contribution_kernel(
            wx, wz, center_xz=(0, 0), half_w=40, half_d=40,
            target=0.85, base=-0.76, plateau_ratio=0.6,
        )
        corner = float(flat[0, 0])  # 远角 r >> 1 → base
        self.assertAlmostEqual(corner, -0.76, places=3)

    def test_plateau_ratio_out_of_range_raises(self) -> None:
        wx, wz = _centered_grid(40)
        with self.assertRaisesRegex(ValueError, "plateau_ratio"):
            qf.zone_contribution_kernel(
                wx, wz, center_xz=(0, 0), half_w=10, half_d=10,
                target=0.3, plateau_ratio=1.0,
            )

    def test_falloff_exponent_zero_raises(self) -> None:
        # falloff_exponent==0 让 radial**0==1 → 整核退回 base（zone 核静默算错）。
        # ZoneQiInput.falloff_exponent 原样透传，必须显式拒绝而非静默错算。
        wx, wz = _centered_grid(40)
        with self.assertRaisesRegex(ValueError, "falloff_exponent must be > 0"):
            qf.zone_contribution_kernel(
                wx, wz, center_xz=(0, 0), half_w=10, half_d=10,
                target=0.3, falloff_exponent=0.0,
            )

    def test_falloff_exponent_negative_raises(self) -> None:
        # 负 exponent 在中心点触发 0**负 → 核心到不了 target，同样静默算错。
        wx, wz = _centered_grid(40)
        with self.assertRaisesRegex(ValueError, "falloff_exponent must be > 0"):
            qf.zone_contribution_kernel(
                wx, wz, center_xz=(0, 0), half_w=10, half_d=10,
                target=0.3, falloff_exponent=-1.6,
            )

    def test_falloff_exponent_positive_ok(self) -> None:
        # 正常正 exponent 不报错（边界 just-above-zero 与默认值都可用）。
        wx, wz = _centered_grid(40)
        for exp in (1e-6, 1.0, 1.6, 4.0):
            kernel = qf.zone_contribution_kernel(
                wx, wz, center_xz=(0, 0), half_w=10, half_d=10,
                target=0.3, base=-0.5, falloff_exponent=exp,
            )
            self.assertEqual(
                kernel.shape, wx.shape,
                f"falloff_exponent={exp} (>0) must produce a valid kernel",
            )

    def test_kernel_spec_from_grade_uses_plateau_and_footprint(self) -> None:
        # kernel_spec_from_grade 默认带 plateau_ratio + footprint_scale（zone 读自身档）。
        spec = qf.kernel_spec_from_grade(
            center_xz=(0, 0), half_w=100, half_d=100, grade=QiGrade.FONT,
        )
        self.assertEqual(spec["plateau_ratio"], qf.GRADE_KERNEL_PLATEAU_RATIO)
        # footprint 放大了 half-extent（过渡发生在 zone 外缘）。
        self.assertGreater(spec["half_w"], 100)
        self.assertAlmostEqual(
            spec["half_w"], 100 * qf.GRADE_KERNEL_FOOTPRINT, places=6
        )

    def test_grade_kernel_zone_aabb_mean_lands_in_grade(self) -> None:
        # 核心不变量：用 kernel_spec_from_grade 建的核，zone 自身 AABB 的面积加权均值
        # 落回声明档位（派生 spirit_qi 不被 edge base 拉成负灵域）。
        for grade, lo, hi in (
            (QiGrade.FONT, 0.7, 1.0),
            (QiGrade.RICH, 0.45, 0.7),
            (QiGrade.COMMON, 0.15, 0.45),
            (QiGrade.NEGATIVE, -1.0, -0.1),
        ):
            # zone AABB half=150；采样仅覆盖 AABB（不含核的外过渡环）。
            wx, wz = _centered_grid(300)  # ±150 grid == zone AABB
            ks = [qf.kernel_spec_from_grade(
                center_xz=(0, 0), half_w=150, half_d=150, grade=grade,
            )]
            field = qf.build_qi_field(wx, wz, zone_kernels=ks, enable_veins=False)
            mean = qf.spirit_qi_from_field(field)
            self.assertTrue(
                lo <= mean < hi or (hi == 1.0 and mean <= 1.0),
                f"{grade.value} zone AABB mean={mean:+.4f} must land in declared "
                f"grade [{lo}, {hi}); plateau+footprint should keep zone reading "
                f"its own grade, got {mean}",
            )


# ===========================================================================
# 4g. 半开区间 AABB —— 相邻 zone 共边界列恰属一个 zone（CodeRabbit P4 #3）
# ===========================================================================


class HalfOpenAabbBoundaryTest(unittest.TestCase):
    """共享边界采样线在半开 [min,max) 下归属确定、与 zone 顺序无关。"""

    @staticmethod
    def _adjacent_specs() -> list[dict]:
        # 两 zone 沿 x=0 接壤：A=[-20,0]×[-10,10] target=-0.6（负灵）；
        # B=[0,20]×[-10,10] target=0.5（常灵）。x=0 是共享边界列。
        a = {
            "center_xz": (-10.0, 0.0), "half_w": 10.0, "half_d": 10.0,
            "target": -0.6, "bounds_xz": (-20.0, 0.0, -10.0, 10.0),
            "plateau_ratio": 0.6,
        }
        b = {
            "center_xz": (10.0, 0.0), "half_w": 10.0, "half_d": 10.0,
            "target": 0.5, "bounds_xz": (0.0, 20.0, -10.0, 10.0),
            "plateau_ratio": 0.6,
        }
        return [a, b]

    def test_shared_boundary_column_belongs_to_exactly_one_zone(self) -> None:
        # 半开 [min,max)：x=0 列对 A 右开（不含）、对 B 左含（含）→ 恰属 B。
        # 在 x=0、y=0 处采样：该列应取 B 的 target，而非 A/B tie-break 的不定值。
        specs = self._adjacent_specs()
        wx = np.array([[0.0]])
        wz = np.array([[0.0]])
        field = qf.build_qi_field(wx, wz, zone_kernels=specs, enable_veins=False)
        # B 在 x=0 的核心 plateau 区铺 0.5；x=0 归 B（左含）→ 值应 == B.target。
        self.assertAlmostEqual(
            float(field[0, 0]), 0.5, places=6,
            msg=(
                "共享边界列 x=0 半开归属应确定取 B（左含 [0,20)）的 target=0.5，"
                f"不是 A/B tie-break 的不定值；得 {float(field[0, 0])}"
            ),
        )

    def test_boundary_assignment_independent_of_zone_order(self) -> None:
        # 关键契约：共边界列的 field 值与 zone_kernels 排列顺序无关（消除 tie-break
        # 不稳定）。闭区间下 x=0 同属 A、B → 由 sort 顺序决定，交换顺序值会变。
        specs = self._adjacent_specs()
        # 沿 x=0 整条采样线（z 从 -10 到 10）。
        zs = np.linspace(-10.0, 10.0, 11)
        wx = np.zeros((zs.size, 1))
        wz = zs.reshape(-1, 1)
        f_ab = qf.build_qi_field(wx, wz, zone_kernels=specs, enable_veins=False)
        f_ba = qf.build_qi_field(
            wx, wz, zone_kernels=list(reversed(specs)), enable_veins=False
        )
        np.testing.assert_allclose(
            f_ab, f_ba, atol=1e-9,
            err_msg=(
                "共边界采样线的 field 必须与 zone 顺序无关（半开归属确定）；"
                "[A,B] 与 [B,A] 输出分叉说明仍有 tie-break 抖动"
            ),
        )


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
