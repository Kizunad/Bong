//! plan-shelflife-v1 §3 / §6 — 容器 freshness 行为。
//!
//! 提供两类 API：
//! - **`container_storage_multiplier`** 纯函数 — 按容器行为 + item profile 推算
//!   传给 `compute_current_qi` 的 `storage_multiplier` 值。
//! - **`enter_container` / `exit_container`** 状态变更 — 当 item 进 / 出 Freeze 容器时，
//!   维护 `Freshness.frozen_since_tick` / `frozen_accumulated`，让 lazy eval 能正确
//!   减除冻结期。

use super::types::{ContainerFreshnessBehavior, DecayFormula, DecayProfile, DecayTrack, Freshness};

/// plan §3 — 解析容器对当前 item 的 storage_multiplier。
///
/// 不同 ContainerFreshnessBehavior 对不同 track / formula 行为不同。
/// **关键语义**：`storage_multiplier` 在 time-based 公式（Exp/Linear/PeakAndFall）
/// 里是 rate 缩放，在 Stepwise 公式里是 `current_qi = initial * multiplier` 的直乘。
/// 这让"冻结"对两种公式有不同实现：
/// - time-based + Freeze → multiplier=0.0 → effective_dt=0 → current 停在 initial ✓
/// - Stepwise + Freeze → multiplier=1.0（**不是 0.0**）→ current=initial*1.0=initial ✓
///
/// 若 Stepwise+Freeze 误用 0.0 会把物品瞬间归零（Codex review r#34 P1）。
///
/// 分流细则：
/// - `Normal` → 1.0（基准）
/// - `Halve` → 0.5（除 Stepwise，对其退 Normal）
/// - `Freeze` → 0.0（time-based）/ 1.0（Stepwise — 参考上述语义注）
/// - `DryingRack { m }` → m（仅 Stepwise 公式，其他退 Normal）
/// - `SpoilOnly { r }` → r（仅 Spoil track，其他退 Normal）
/// - `AgeAccelerate { f }` → 1 / max(f, 0.01)（仅 Age track，其他退 Normal）
pub fn container_storage_multiplier(
    behavior: &ContainerFreshnessBehavior,
    profile: &DecayProfile,
) -> f32 {
    let track = profile.track();
    let is_stepwise = is_stepwise_profile(profile);

    match behavior {
        ContainerFreshnessBehavior::Normal => 1.0,
        ContainerFreshnessBehavior::Halve => {
            if is_stepwise {
                1.0
            } else {
                0.5
            }
        }
        ContainerFreshnessBehavior::Freeze => {
            if is_stepwise {
                // Stepwise 语义：current = initial * multiplier；Freeze 应保留 initial
                // → multiplier = 1.0（不是 0.0，避免瞬间归零）。`frozen_since_tick` 仍记
                // 账以便物品后续被迁移到 time-based profile 时冻结期被正确减除。
                1.0
            } else {
                0.0
            }
        }
        ContainerFreshnessBehavior::DryingRack { multiplier } => {
            if is_stepwise {
                multiplier.max(0.0)
            } else {
                1.0
            }
        }
        ContainerFreshnessBehavior::SpoilOnly { rate } => {
            if track == DecayTrack::Spoil {
                rate.max(0.0)
            } else {
                1.0
            }
        }
        ContainerFreshnessBehavior::AgeAccelerate { factor } => {
            if track == DecayTrack::Age {
                1.0 / factor.max(0.01)
            } else {
                1.0
            }
        }
    }
}

/// plan §6.1 — 物品进容器：若是 Freeze 容器，记 `frozen_since_tick`。
/// 重复 enter（已在 freezing）保持原 `frozen_since_tick` 不变（防止时间倒流）。
pub fn enter_container(
    freshness: &mut Freshness,
    behavior: &ContainerFreshnessBehavior,
    now_tick: u64,
) {
    if behavior.is_freeze() && freshness.frozen_since_tick.is_none() {
        freshness.frozen_since_tick = Some(now_tick);
    }
}

/// plan §6.1 — 物品出容器：若 inflight freeze 中，把已过 ticks 累加到 `frozen_accumulated`，
/// 然后清空 `frozen_since_tick`。非 freezing 状态下 no-op。
pub fn exit_container(freshness: &mut Freshness, now_tick: u64) {
    if let Some(since) = freshness.frozen_since_tick.take() {
        let elapsed = now_tick.saturating_sub(since);
        freshness.frozen_accumulated = freshness.frozen_accumulated.saturating_add(elapsed);
    }
}

fn is_stepwise_profile(profile: &DecayProfile) -> bool {
    match profile {
        DecayProfile::Decay { formula, .. } | DecayProfile::Spoil { formula, .. } => {
            matches!(formula, DecayFormula::Stepwise)
        }
        DecayProfile::Age { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::DecayProfileId;
    use super::*;

    fn decay_exp() -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("test_decay_exp"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        }
    }

    fn decay_stepwise() -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("test_decay_stepwise"),
            formula: DecayFormula::Stepwise,
            floor_qi: 0.0,
        }
    }

    fn spoil_exp() -> DecayProfile {
        DecayProfile::Spoil {
            id: DecayProfileId::new("test_spoil"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            spoil_threshold: 10.0,
        }
    }

    fn age_profile() -> DecayProfile {
        DecayProfile::Age {
            id: DecayProfileId::new("test_age"),
            peak_at_ticks: 1000,
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 500,
            post_peak_spoil_threshold: 30.0,
            post_peak_spoil_profile: DecayProfileId::new("test_age_post_spoil"),
        }
    }

    // =========== container_storage_multiplier ===========

    #[test]
    fn normal_returns_unity_for_all_profiles() {
        let b = ContainerFreshnessBehavior::Normal;
        assert!((container_storage_multiplier(&b, &decay_exp()) - 1.0).abs() < 1e-6);
        assert!((container_storage_multiplier(&b, &decay_stepwise()) - 1.0).abs() < 1e-6);
        assert!((container_storage_multiplier(&b, &spoil_exp()) - 1.0).abs() < 1e-6);
        assert!((container_storage_multiplier(&b, &age_profile()) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn halve_applies_to_non_stepwise_only() {
        let b = ContainerFreshnessBehavior::Halve;
        assert!((container_storage_multiplier(&b, &decay_exp()) - 0.5).abs() < 1e-6);
        assert!((container_storage_multiplier(&b, &spoil_exp()) - 0.5).abs() < 1e-6);
        assert!((container_storage_multiplier(&b, &age_profile()) - 0.5).abs() < 1e-6);
        // Stepwise 退 Normal — 玉盒不影响阴干药草（阴干药草本就是容器决定状态）
        assert!((container_storage_multiplier(&b, &decay_stepwise()) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn freeze_returns_zero_for_time_based_profiles() {
        // time-based (Exp/Linear/PeakAndFall) — multiplier=0 → effective_dt=0 → current=initial
        let b = ContainerFreshnessBehavior::Freeze;
        assert!((container_storage_multiplier(&b, &decay_exp()) - 0.0).abs() < 1e-6);
        assert!((container_storage_multiplier(&b, &spoil_exp()) - 0.0).abs() < 1e-6);
        assert!((container_storage_multiplier(&b, &age_profile()) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn freeze_preserves_stepwise_via_unity_multiplier() {
        // Regression: Codex review r#34 P1 — Stepwise + Freeze 返回 0.0 会把
        // current_qi = initial * 0 = 0 瞬间归零。应返 1.0 保留 initial。
        let b = ContainerFreshnessBehavior::Freeze;
        let m = container_storage_multiplier(&b, &decay_stepwise());
        assert!(
            (m - 1.0).abs() < 1e-6,
            "Freeze + Stepwise must return 1.0 to preserve initial, got {m}"
        );

        // 端到端：Stepwise 物品经 compute_current_qi 在 Freeze 容器下保持 initial
        use super::super::compute::compute_current_qi;
        let p = decay_stepwise();
        let f = Freshness::new(0, 100.0, &p);
        let current = compute_current_qi(&f, &p, 1_000_000, m);
        assert!(
            (current - 100.0).abs() < 1e-3,
            "Stepwise + Freeze must preserve initial_qi, got {current}"
        );
    }

    #[test]
    fn drying_rack_applies_to_stepwise_only() {
        let b = ContainerFreshnessBehavior::DryingRack { multiplier: 0.7 };
        // 阴干药草（Stepwise）— 干燥架 ×0.7
        assert!((container_storage_multiplier(&b, &decay_stepwise()) - 0.7).abs() < 1e-6);
        // 灵石（Exp）— 干燥架不适用
        assert!((container_storage_multiplier(&b, &decay_exp()) - 1.0).abs() < 1e-6);
        // 兽血（Spoil Exp）— 干燥架不适用
        assert!((container_storage_multiplier(&b, &spoil_exp()) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn drying_rack_clamps_negative_multiplier() {
        let b = ContainerFreshnessBehavior::DryingRack { multiplier: -0.3 };
        assert!((container_storage_multiplier(&b, &decay_stepwise()) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn spoil_only_applies_to_spoil_track() {
        let b = ContainerFreshnessBehavior::SpoilOnly { rate: 0.3 };
        // 兽血 Spoil → 0.3
        assert!((container_storage_multiplier(&b, &spoil_exp()) - 0.3).abs() < 1e-6);
        // 灵石 Decay → 1.0
        assert!((container_storage_multiplier(&b, &decay_exp()) - 1.0).abs() < 1e-6);
        // 陈酒 Age → 1.0（冰窖不影响 Age）
        assert!((container_storage_multiplier(&b, &age_profile()) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn age_accelerate_applies_to_age_track() {
        let b = ContainerFreshnessBehavior::AgeAccelerate { factor: 2.0 };
        // 陈酒 Age → 1.0 / 2.0 = 0.5? Wait — accelerate 应是加速，rate 应 > 1.0 (effective_dt 增长更快)
        // 我设计：multiplier = 1 / factor，factor=2.0 表 "peak 2 倍快"，但 multiplier=0.5 是 dt 慢一半...
        // 重新看 plan §3：陈化窖 peak_at_ticks ×0.7（加速陈化）— 等价 effective_dt = dt * (1/0.7) ≈ 1.43
        // 所以 multiplier 应为 1 / 0.7 ≈ 1.43，factor=0.7 是"压缩到 0.7 倍时长"。
        // 用 factor=0.7（plan 标准值）：multiplier = 1 / 0.7 ≈ 1.4286
        let b2 = ContainerFreshnessBehavior::AgeAccelerate { factor: 0.7 };
        let m = container_storage_multiplier(&b2, &age_profile());
        assert!(
            (m - 1.428_571).abs() < 1e-3,
            "expected 1/0.7 ≈ 1.4286, got {m}"
        );
        // Decay / Spoil 不受影响
        assert!((container_storage_multiplier(&b, &decay_exp()) - 1.0).abs() < 1e-6);
        assert!((container_storage_multiplier(&b, &spoil_exp()) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn age_accelerate_clamps_tiny_factor() {
        // factor 接近 0 会得到极大 multiplier — clamp factor.max(0.01) 防止除零 / 暴增
        let b = ContainerFreshnessBehavior::AgeAccelerate { factor: 0.001 };
        let m = container_storage_multiplier(&b, &age_profile());
        assert!((m - 100.0).abs() < 0.1, "expected 1/0.01 = 100, got {m}");
    }

    // =========== enter / exit container ===========

    fn fresh_with_freshness() -> Freshness {
        Freshness::new(0, 100.0, &decay_exp())
    }

    #[test]
    fn enter_freeze_records_since_tick() {
        let mut f = fresh_with_freshness();
        let b = ContainerFreshnessBehavior::Freeze;
        enter_container(&mut f, &b, 500);
        assert_eq!(f.frozen_since_tick, Some(500));
        assert_eq!(f.frozen_accumulated, 0);
    }

    #[test]
    fn enter_non_freeze_does_not_record_since_tick() {
        let mut f = fresh_with_freshness();
        for b in [
            ContainerFreshnessBehavior::Normal,
            ContainerFreshnessBehavior::Halve,
            ContainerFreshnessBehavior::DryingRack { multiplier: 1.0 },
            ContainerFreshnessBehavior::SpoilOnly { rate: 0.3 },
            ContainerFreshnessBehavior::AgeAccelerate { factor: 0.7 },
        ] {
            f.frozen_since_tick = None;
            enter_container(&mut f, &b, 500);
            assert!(
                f.frozen_since_tick.is_none(),
                "non-Freeze container should not set frozen_since_tick, got {:?}",
                f.frozen_since_tick
            );
        }
    }

    #[test]
    fn enter_freeze_idempotent_when_already_frozen() {
        let mut f = fresh_with_freshness();
        let b = ContainerFreshnessBehavior::Freeze;
        enter_container(&mut f, &b, 500);
        enter_container(&mut f, &b, 700); // 重复 enter — 应保留 500
        assert_eq!(f.frozen_since_tick, Some(500));
    }

    #[test]
    fn exit_after_freeze_accumulates() {
        let mut f = fresh_with_freshness();
        enter_container(&mut f, &ContainerFreshnessBehavior::Freeze, 500);
        exit_container(&mut f, 1500);
        assert!(f.frozen_since_tick.is_none());
        assert_eq!(f.frozen_accumulated, 1000);
    }

    #[test]
    fn exit_without_freeze_is_noop() {
        let mut f = fresh_with_freshness();
        f.frozen_accumulated = 42;
        exit_container(&mut f, 999);
        assert!(f.frozen_since_tick.is_none());
        assert_eq!(
            f.frozen_accumulated, 42,
            "exit without active freeze should not modify accumulated"
        );
    }

    #[test]
    fn multiple_freeze_cycles_accumulate_correctly() {
        let mut f = fresh_with_freshness();
        // cycle 1: enter@100, exit@300 → +200 accumulated
        enter_container(&mut f, &ContainerFreshnessBehavior::Freeze, 100);
        exit_container(&mut f, 300);
        assert_eq!(f.frozen_accumulated, 200);
        // cycle 2: enter@500, exit@1000 → +500 accumulated
        enter_container(&mut f, &ContainerFreshnessBehavior::Freeze, 500);
        exit_container(&mut f, 1000);
        assert_eq!(f.frozen_accumulated, 700);
    }

    #[test]
    fn exit_with_clock_drift_saturates() {
        // now < since（时空穿越 / clock drift）— saturating_sub 防 panic
        let mut f = fresh_with_freshness();
        f.frozen_since_tick = Some(1000);
        exit_container(&mut f, 500); // 时间倒流
        assert!(f.frozen_since_tick.is_none());
        assert_eq!(f.frozen_accumulated, 0); // 0 累加
    }

    // =========== 集成 — container + compute end-to-end ===========

    #[test]
    fn freeze_container_preserves_initial_qi_via_compute() {
        // 集成验证：物品进 Freeze 容器后立即 compute_current_qi 应仍为 initial。
        use super::super::compute::compute_current_qi;

        let p = decay_exp();
        let mut f = Freshness::new(0, 100.0, &p);

        // 容器选 Freeze，正常 multiplier 0.0
        let mult = container_storage_multiplier(&ContainerFreshnessBehavior::Freeze, &p);
        let current = compute_current_qi(&f, &p, 5000, mult);
        assert!((current - 100.0).abs() < 1e-3);

        // 同时 enter — frozen_since_tick 也维护好（若调用方双管齐下）
        enter_container(&mut f, &ContainerFreshnessBehavior::Freeze, 0);
        let current2 = compute_current_qi(&f, &p, 5000, mult);
        assert!((current2 - 100.0).abs() < 1e-3);
    }

    #[test]
    fn halve_container_doubles_effective_half_life() {
        // 集成验证：Halve 容器使 1 half_life 时间过去后只衰减 sqrt(2) 倍。
        use super::super::compute::compute_current_qi;

        let p = decay_exp(); // half_life=1000
        let f = Freshness::new(0, 100.0, &p);
        let mult = container_storage_multiplier(&ContainerFreshnessBehavior::Halve, &p);
        // 1000 ticks 在 Halve 下相当于 500 ticks → current = 100 * 0.5^0.5 ≈ 70.71
        let current = compute_current_qi(&f, &p, 1000, mult);
        assert!((current - 70.71).abs() < 0.1);
    }
}
