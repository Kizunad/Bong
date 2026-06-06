//! plan-food-v1 P2 — 灵食消费路径。
//!
//! 提供 `consume_food` 纯函数：读取物品的 shelflife freshness + profile，
//! 计算当前腐败状态与峰值加成，最终返回 `ConsumeFoodResult` 供调用侧决策。
//! 调用侧（`cast_emit.rs` 的 `apply_cast_item_effect`）负责：
//! 1. 拦截 `CriticalBlock` → 拒绝消费（保持物品不扣除）
//! 2. `Warn` → 写 `SpoilConsumeWarning` event + 降效消费
//! 3. 将 `CultivationAcceleration` 通过 `push_status_effect` 写入 `StatusEffects`
//!
//! 守恒律：
//! - `CultivationAcceleration` 只改 regen 乘数（cultivation/tick.rs:327 的
//!   `cultivation_acceleration_multiplier`），不修改 `qi_current`，符合
//!   plan qi_physics 约束。
//! - 峰值状态下 `bonus_factor` 叠加 `peak_bonus`，但上限受
//!   `cultivation_acceleration_multiplier` 的 5× 帽控制（plan §1.2）。

use crate::combat::components::ActiveStatusEffect;
use crate::combat::components::StatusEffects;
use crate::combat::events::StatusEffectKind;
use crate::combat::status::push_status_effect;
use crate::inventory::freshness::GAME_DAY_TICKS;
use crate::shelflife::consume::{age_peak_check, spoil_check, AgePeakCheck, SpoilCheckOutcome};
use crate::shelflife::types::{DecayProfile, Freshness};

// ─────────────────────────────────────────────────────────────────────────────
// 公共常量
// ─────────────────────────────────────────────────────────────────────────────

/// 灵果（`food.spirit_fruit.ling_guo`）基础修炼加速比例：+20%。
#[allow(dead_code)] // plan-food-v1 P2 public API，food.toml ItemEffect::FoodRegen 消费路径接入后使用
pub const LING_GUO_BONUS_FACTOR: f32 = 0.20;

/// 灵果消费后效果持续时间：2 游戏日。
#[allow(dead_code)] // plan-food-v1 P2 public API，food.toml ItemEffect::FoodRegen 消费路径接入后使用
pub const LING_GUO_DURATION_TICKS: u64 = GAME_DAY_TICKS * 2;

/// 普通熟肉（`food.mundane.cooked_meat`）修炼加速比例：+5%（充饥小补，不含灵气）。
#[allow(dead_code)] // plan-food-v1 P2 public API，food.toml ItemEffect::FoodRegen 消费路径接入后使用
pub const COOKED_MEAT_BONUS_FACTOR: f32 = 0.05;

/// 普通熟肉效果持续时间：1 游戏日。
#[allow(dead_code)] // plan-food-v1 P2 public API，food.toml ItemEffect::FoodRegen 消费路径接入后使用
pub const COOKED_MEAT_DURATION_TICKS: u64 = GAME_DAY_TICKS;

// ─────────────────────────────────────────────────────────────────────────────
// `consume_food` 返回值
// ─────────────────────────────────────────────────────────────────────────────

/// `consume_food` 的判定结果。
///
/// 调用侧按以下优先级处理：
/// 1. `CriticalBlock` — 拒绝消费，不扣库存，弹二次确认 HUD
/// 2. `SpoiledWarn` — 降效消费（`reduced_bonus_factor`），写 `SpoilConsumeWarning` event
/// 3. `FoodApplied` — 正常消费，按 `bonus_factor` 写 `CultivationAcceleration`
/// 4. `Noop` — 无 freshness / 无效 profile，静默放行（无 bonus）
#[allow(dead_code)] // plan-food-v1 P2 public API，全食用流程在 P3 cast 接入后使用
#[derive(Debug, Clone, PartialEq)]
pub enum ConsumeFoodResult {
    /// 食物已过度腐败（current < 0.1 × spoil_threshold）— 拒绝消费。
    CriticalBlock {
        current_qi: f32,
        spoil_threshold: f32,
    },
    /// 食物部分腐败（0.1 × threshold ≤ current < threshold）— 降效消费。
    ///
    /// `reduced_bonus_factor`：已按 `current/threshold` 线性折算。
    SpoiledWarn {
        current_qi: f32,
        spoil_threshold: f32,
        /// 折算后的实际加速系数（< 原始 `bonus_factor`）。
        reduced_bonus_factor: f32,
        duration_ticks: u64,
    },
    /// 正常消费，直接应用修炼加速。
    FoodApplied {
        /// 实际加速系数（峰值状态下含 `peak_bonus` 叠加）。
        bonus_factor: f32,
        duration_ticks: u64,
        /// 是否处于陈化峰值窗口（峰值状态下 bonus 更高）。
        is_peak: bool,
    },
    /// 无 freshness / profile 不适用 — 无加速效果，静默放行。
    Noop,
}

// ─────────────────────────────────────────────────────────────────────────────
// 主入口
// ─────────────────────────────────────────────────────────────────────────────

/// plan-food-v1 P2 — 灵食消费判定（纯函数，无副作用）。
#[allow(dead_code)] // plan-food-v1 P2 public API
///
/// 参数：
/// - `freshness` / `profile`：物品当前 freshness 状态与注册 profile。
///   若物品没有 freshness，传 `None` → 返回 `Noop`。
/// - `base_bonus_factor`：模板层配置的基础加速系数（来自 `ItemEffect::FoodRegen.bonus_factor`）。
/// - `duration_ticks`：模板层配置的效果时长（来自 `ItemEffect::FoodRegen.duration_ticks`）。
/// - `now_tick`：当前 tick（`CombatClock.tick`）。
/// - `storage_multiplier`：容器 freshness 倍率（无容器传 `1.0`）。
///
/// 返回值见 `ConsumeFoodResult`。调用侧负责写副作用（event / status_effect）。
pub fn consume_food(
    freshness: Option<(&Freshness, &DecayProfile)>,
    base_bonus_factor: f32,
    duration_ticks: u64,
    now_tick: u64,
    storage_multiplier: f32,
) -> ConsumeFoodResult {
    let Some((freshness, profile)) = freshness else {
        return ConsumeFoodResult::Noop;
    };

    // ── 先检查 Spoil 路径（熟肉 / 灵果都是 Spoil profile）──
    let spoil = spoil_check(freshness, profile, now_tick, storage_multiplier);
    match spoil {
        SpoilCheckOutcome::CriticalBlock {
            current_qi,
            spoil_threshold,
        } => {
            return ConsumeFoodResult::CriticalBlock {
                current_qi,
                spoil_threshold,
            };
        }
        SpoilCheckOutcome::Warn {
            current_qi,
            spoil_threshold,
        } => {
            // 线性折算：ratio = current / threshold，bonus 按比例缩减。
            let ratio = (current_qi / spoil_threshold).clamp(0.0, 1.0);
            let reduced = base_bonus_factor * ratio;
            return ConsumeFoodResult::SpoiledWarn {
                current_qi,
                spoil_threshold,
                reduced_bonus_factor: reduced,
                duration_ticks,
            };
        }
        SpoilCheckOutcome::Safe { .. } => {
            // 继续检查 Age 路径峰值。
        }
        SpoilCheckOutcome::NotApplicable => {
            // 非 Spoil profile：继续检查 Age 路径。
        }
    }

    // ── 检查 Age 路径峰值（陈酒 / 老坛丹是 Age profile）──
    let age = age_peak_check(freshness, profile, now_tick, storage_multiplier);
    let (final_bonus, is_peak) = match age {
        AgePeakCheck::Peaking { bonus_strength } => {
            // 峰值叠加：bonus = base + base * peak_strength（plan §5.3 语义）
            let boosted = base_bonus_factor * (1.0 + bonus_strength);
            (boosted, true)
        }
        AgePeakCheck::NotPeaking | AgePeakCheck::NotApplicable => (base_bonus_factor, false),
    };

    ConsumeFoodResult::FoodApplied {
        bonus_factor: final_bonus,
        duration_ticks,
        is_peak,
    }
}

/// plan-food-v1 P2 — 将 `ConsumeFoodResult` 应用到 `StatusEffects`。
#[allow(dead_code)] // plan-food-v1 P2 public API
///
/// 此函数隔离副作用：仅当 `FoodApplied` / `SpoiledWarn` 时调用 `push_status_effect`。
/// `CriticalBlock` / `Noop` 调用侧不应传入此函数（或可安全忽略返回值）。
///
/// 返回 `true` 表示成功写入 status effect（堆叠层未被 per-source cap 拦截）。
pub fn apply_food_status_effect(
    status_effects: &mut StatusEffects,
    result: &ConsumeFoodResult,
    item_id: &str,
) -> bool {
    match result {
        ConsumeFoodResult::FoodApplied {
            bonus_factor,
            duration_ticks,
            ..
        } => push_status_effect(
            status_effects,
            ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: *bonus_factor,
                remaining_ticks: *duration_ticks,
                source_pill: Some(item_id.to_string()),
            },
        ),
        ConsumeFoodResult::SpoiledWarn {
            reduced_bonus_factor,
            duration_ticks,
            ..
        } => {
            if *reduced_bonus_factor > 0.0 {
                push_status_effect(
                    status_effects,
                    ActiveStatusEffect {
                        kind: StatusEffectKind::CultivationAcceleration,
                        magnitude: *reduced_bonus_factor,
                        remaining_ticks: *duration_ticks,
                        source_pill: Some(item_id.to_string()),
                    },
                )
            } else {
                false
            }
        }
        ConsumeFoodResult::CriticalBlock { .. } | ConsumeFoodResult::Noop => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 单测
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::StatusEffects;
    use crate::combat::events::StatusEffectKind;
    use crate::cultivation::tick::cultivation_acceleration_multiplier;
    use crate::inventory::freshness::GAME_DAY_TICKS;
    use crate::shelflife::types::{DecayFormula, DecayProfileId, Freshness};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn spoil_profile(half_life_ticks: u64, threshold: f32) -> DecayProfile {
        DecayProfile::Spoil {
            id: DecayProfileId::new("test_spoil"),
            formula: DecayFormula::Exponential { half_life_ticks },
            spoil_threshold: threshold,
        }
    }

    fn age_profile(peak_at: u64, bonus: f32, post_half: u64, spoil_th: f32) -> DecayProfile {
        DecayProfile::Age {
            id: DecayProfileId::new("test_age"),
            peak_at_ticks: peak_at,
            peak_bonus: bonus,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: post_half,
            post_peak_spoil_threshold: spoil_th,
            post_peak_spoil_profile: DecayProfileId::new("test_age_spoil"),
        }
    }

    fn fresh_item(profile: &DecayProfile, initial: f32) -> Freshness {
        Freshness::new(0, initial, profile)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // consume_food — 正常消费路径
    // ─────────────────────────────────────────────────────────────────────────

    /// 食物刚创建，fresh → FoodApplied，bonus_factor 与模板一致
    #[test]
    fn consume_food_fresh_spoil_returns_food_applied() {
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.01);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 0, 1.0);
        match result {
            ConsumeFoodResult::FoodApplied {
                bonus_factor,
                duration_ticks,
                is_peak,
            } => {
                assert!(
                    (bonus_factor - 0.20).abs() < 1e-4,
                    "期望 bonus_factor=0.20（模板值），实际 {bonus_factor}"
                );
                assert_eq!(
                    duration_ticks, LING_GUO_DURATION_TICKS,
                    "期望 duration_ticks={LING_GUO_DURATION_TICKS}，实际 {duration_ticks}"
                );
                assert!(!is_peak, "Spoil profile 不应报告 is_peak=true");
            }
            other => panic!("期望 FoodApplied，实际 {other:?}"),
        }
    }

    /// 无 freshness 信息 → Noop（静默放行，无 bonus）
    #[test]
    fn consume_food_no_freshness_returns_noop() {
        let result = consume_food(None, 0.20, LING_GUO_DURATION_TICKS, 0, 1.0);
        assert_eq!(result, ConsumeFoodResult::Noop, "无 freshness 应返回 Noop");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // consume_food — Spoil 路径边界
    // ─────────────────────────────────────────────────────────────────────────

    /// 食物 current 恰好等于 spoil_threshold → Safe（strict `<` 语义） → FoodApplied
    #[test]
    fn consume_food_at_exact_threshold_is_safe() {
        // half_life=GAME_DAY_TICKS * 3，threshold=0.5
        // tick = 3 * GAME_DAY_TICKS → current = 1.0 * 0.5 = 0.5 = threshold → Safe
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.5);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(
            Some((&f, &p)),
            0.20,
            LING_GUO_DURATION_TICKS,
            GAME_DAY_TICKS * 3,
            1.0,
        );
        assert!(
            matches!(result, ConsumeFoodResult::FoodApplied { .. }),
            "current == spoil_threshold 应为 Safe → FoodApplied（strict `<`），实际 {result:?}"
        );
    }

    /// 食物 Warn 区间 → SpoiledWarn，reduced_bonus_factor 按比例折算
    #[test]
    fn consume_food_warn_returns_spoiled_warn_with_reduced_bonus() {
        // half_life=GAME_DAY_TICKS * 3，threshold=0.5
        // tick = 6 * GAME_DAY_TICKS → current ≈ 0.25 < 0.5 但 ≥ 0.05（=0.1×0.5）→ Warn
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.5);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(
            Some((&f, &p)),
            0.20,
            LING_GUO_DURATION_TICKS,
            GAME_DAY_TICKS * 6,
            1.0,
        );
        match result {
            ConsumeFoodResult::SpoiledWarn {
                reduced_bonus_factor,
                ..
            } => {
                assert!(
                    reduced_bonus_factor < 0.20,
                    "腐败食物的 reduced_bonus_factor 应 < 原始 0.20，实际 {reduced_bonus_factor}"
                );
                assert!(
                    reduced_bonus_factor > 0.0,
                    "未完全腐败，reduced_bonus_factor 应 > 0，实际 {reduced_bonus_factor}"
                );
            }
            other => panic!("期望 SpoiledWarn，实际 {other:?}"),
        }
    }

    /// 食物极度腐败 → CriticalBlock
    #[test]
    fn consume_food_critical_block_when_nearly_rotten() {
        // threshold=0.5，current < 0.1 × 0.5 = 0.05
        // tick = 100 * GAME_DAY_TICKS → current ≈ 0（极小值）→ CriticalBlock
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.5);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(
            Some((&f, &p)),
            0.20,
            LING_GUO_DURATION_TICKS,
            GAME_DAY_TICKS * 100,
            1.0,
        );
        assert!(
            matches!(result, ConsumeFoodResult::CriticalBlock { .. }),
            "极度腐败应 CriticalBlock，实际 {result:?}"
        );
    }

    /// 腐败边界恰好 = 0.1 × threshold → Warn（strict `<` 语义，不触发 CriticalBlock）
    #[test]
    fn consume_food_at_critical_ratio_boundary_is_warn_not_block() {
        // Linear: decay_per_tick=0.95, initial=1.0, threshold=0.5
        // dt=1 → current = 1.0 - 0.95 = 0.05 = 0.1 × 0.5 → Warn（border）
        let p = DecayProfile::Spoil {
            id: DecayProfileId::new("t"),
            formula: DecayFormula::Linear {
                decay_per_tick: 0.95,
            },
            spoil_threshold: 0.5,
        };
        let f = fresh_item(&p, 1.0);
        let result = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 1, 1.0);
        assert!(
            matches!(result, ConsumeFoodResult::SpoiledWarn { .. }),
            "current == 0.1 × threshold 应为 Warn（strict `<`），实际 {result:?}"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // consume_food — Age 路径（陈酒）峰值
    // ─────────────────────────────────────────────────────────────────────────

    /// 陈酒处于峰值窗口 → FoodApplied，is_peak=true，bonus 含 peak_bonus 叠加
    #[test]
    fn consume_food_age_peaking_returns_boosted_bonus() {
        // peak_at=1000, peak_window_ratio=0.1 → 窗口 [900, 1100]
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0);
        let result = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 1000, 1.0);
        match result {
            ConsumeFoodResult::FoodApplied {
                bonus_factor,
                is_peak,
                ..
            } => {
                assert!(is_peak, "处于 Age 峰值窗口，is_peak 应为 true");
                // boosted = 0.20 * (1 + 0.5) = 0.30
                let expected = 0.20_f32 * (1.0 + 0.5);
                assert!(
                    (bonus_factor - expected).abs() < 1e-4,
                    "峰值 bonus_factor 应为 {expected}（base × (1+peak_bonus)），实际 {bonus_factor}"
                );
            }
            other => panic!("期望 FoodApplied，实际 {other:?}"),
        }
    }

    /// 陈酒过峰未进入 Spoil 迁移 → FoodApplied，is_peak=false，bonus 为原始值
    #[test]
    fn consume_food_age_past_peak_not_peaking() {
        let p = age_profile(1000, 0.5, 500, 30.0);
        let f = fresh_item(&p, 100.0);
        // tick=2000：PastPeak，不在峰值窗口
        let result = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 2000, 1.0);
        match result {
            ConsumeFoodResult::FoodApplied { is_peak, .. } => {
                assert!(!is_peak, "past-peak 不应为 is_peak=true");
            }
            other => panic!("期望 FoodApplied，实际 {other:?}"),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // apply_food_status_effect — StatusEffects 写入
    // ─────────────────────────────────────────────────────────────────────────

    /// 食用灵果后 StatusEffects.active 含 CultivationAcceleration，magnitude=0.20
    #[test]
    fn apply_food_status_effect_fresh_ling_guo_pushes_cultivation_accel() {
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.01);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 0, 1.0);
        let mut se = StatusEffects::default();
        let pushed = apply_food_status_effect(&mut se, &result, "food.spirit_fruit.ling_guo");
        assert!(pushed, "FoodApplied 应成功 push status effect，返回 true");
        let accel = se
            .active
            .iter()
            .find(|e| e.kind == StatusEffectKind::CultivationAcceleration);
        assert!(
            accel.is_some(),
            "StatusEffects.active 应含 CultivationAcceleration，实际 {:?}",
            se.active
        );
        let accel = accel.unwrap();
        assert!(
            (accel.magnitude - 0.20).abs() < 1e-4,
            "CultivationAcceleration magnitude 应=0.20（灵果 bonus），实际 {}",
            accel.magnitude
        );
        assert_eq!(
            accel.remaining_ticks, LING_GUO_DURATION_TICKS,
            "remaining_ticks 应=LING_GUO_DURATION_TICKS，实际 {}",
            accel.remaining_ticks
        );
    }

    /// cultivation_acceleration_multiplier 在 magnitude=0.20 时返回 1.20
    #[test]
    fn apply_food_status_effect_multiplier_reflects_one_point_two() {
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.01);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 0, 1.0);
        let mut se = StatusEffects::default();
        apply_food_status_effect(&mut se, &result, "food.spirit_fruit.ling_guo");
        let multiplier = cultivation_acceleration_multiplier(&se);
        assert!(
            (multiplier - 1.20).abs() < 1e-4,
            "cultivation_acceleration_multiplier 应=1.20（1 + 0.20），实际 {multiplier}"
        );
    }

    /// duration_ticks 耗尽后，status_effect_tick 移除效果，multiplier 回到 1.0
    #[test]
    fn apply_food_status_effect_expired_ticks_no_longer_active() {
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.01);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 0, 1.0);
        let mut se = StatusEffects::default();
        apply_food_status_effect(&mut se, &result, "food.spirit_fruit.ling_guo");

        // 模拟 tick 耗尽：将 remaining_ticks 置 0
        for e in se.active.iter_mut() {
            if e.kind == StatusEffectKind::CultivationAcceleration {
                e.remaining_ticks = 0;
            }
        }

        let multiplier = cultivation_acceleration_multiplier(&se);
        assert!(
            (multiplier - 1.0).abs() < 1e-4,
            "duration 耗尽后 multiplier 应回到 1.0，实际 {multiplier}"
        );
    }

    /// CriticalBlock 不写 status effect，返回 false
    #[test]
    fn apply_food_status_effect_critical_block_does_not_push() {
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.5);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(
            Some((&f, &p)),
            0.20,
            LING_GUO_DURATION_TICKS,
            GAME_DAY_TICKS * 100,
            1.0,
        );
        assert!(
            matches!(result, ConsumeFoodResult::CriticalBlock { .. }),
            "前提：should be CriticalBlock, got {result:?}"
        );
        let mut se = StatusEffects::default();
        let pushed = apply_food_status_effect(&mut se, &result, "food.spirit_fruit.ling_guo");
        assert!(!pushed, "CriticalBlock 不应 push status effect，返回 false");
        assert!(
            se.active.is_empty(),
            "CriticalBlock 后 StatusEffects.active 应为空，实际 {:?}",
            se.active
        );
    }

    /// Noop 不写 status effect，返回 false
    #[test]
    fn apply_food_status_effect_noop_does_not_push() {
        let result = consume_food(None, 0.20, LING_GUO_DURATION_TICKS, 0, 1.0);
        assert_eq!(result, ConsumeFoodResult::Noop);
        let mut se = StatusEffects::default();
        let pushed = apply_food_status_effect(&mut se, &result, "food.spirit_fruit.ling_guo");
        assert!(!pushed, "Noop 不应 push status effect");
    }

    /// 同种食物 source_pill cap：同一 item_id 最多 2 层（push_status_effect 的 per-pill cap）
    #[test]
    fn apply_food_status_effect_same_source_capped_at_two() {
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.01);
        let f = fresh_item(&p, 1.0);
        let result = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 0, 1.0);
        let mut se = StatusEffects::default();

        let r1 = apply_food_status_effect(&mut se, &result, "food.spirit_fruit.ling_guo");
        let r2 = apply_food_status_effect(&mut se, &result, "food.spirit_fruit.ling_guo");
        let r3 = apply_food_status_effect(&mut se, &result, "food.spirit_fruit.ling_guo");
        assert!(r1, "第 1 次 push 应成功");
        assert!(r2, "第 2 次 push 应成功");
        assert!(!r3, "第 3 次 push 应被 per-source cap 拦截，返回 false");

        let count = se
            .active
            .iter()
            .filter(|e| {
                e.kind == StatusEffectKind::CultivationAcceleration
                    && e.source_pill.as_deref() == Some("food.spirit_fruit.ling_guo")
            })
            .count();
        assert_eq!(
            count, 2,
            "同种灵果最多 2 条 CultivationAcceleration，实际 {count}"
        );
    }

    /// 普通熟肉 bonus_factor = COOKED_MEAT_BONUS_FACTOR（0.05），验证常量正确
    #[test]
    fn cooked_meat_bonus_factor_constant_is_correct() {
        assert!(
            (COOKED_MEAT_BONUS_FACTOR - 0.05).abs() < 1e-4,
            "COOKED_MEAT_BONUS_FACTOR 应为 0.05，实际 {COOKED_MEAT_BONUS_FACTOR}"
        );
    }

    /// storage_multiplier = 0.3（冰窖）下，food apply 效果不变（freshness 影响衰减速度，不影响 bonus）
    #[test]
    fn consume_food_ice_cellar_storage_multiplier_applied_correctly() {
        let p = spoil_profile(GAME_DAY_TICKS * 3, 0.01);
        let f = fresh_item(&p, 1.0);
        // 在 Normal 和冰窖两种 storage_multiplier 下，新鲜食物都应返回 FoodApplied
        let r_normal = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 0, 1.0);
        let r_cold = consume_food(Some((&f, &p)), 0.20, LING_GUO_DURATION_TICKS, 0, 0.3);
        assert!(
            matches!(r_normal, ConsumeFoodResult::FoodApplied { .. }),
            "Normal 容器，新鲜食物应 FoodApplied"
        );
        assert!(
            matches!(r_cold, ConsumeFoodResult::FoodApplied { .. }),
            "冰窖容器，新鲜食物应 FoodApplied（bonus 不变，只是衰减更慢）"
        );
    }

    /// 冰窖下食物腐败速度慢：同一 tick 下，冰窖食物 current > Normal 食物 current
    #[test]
    fn consume_food_ice_cellar_slower_spoil_rate() {
        // 半衰 = 2 GAME_DAY，threshold = 0.5
        let p = spoil_profile(GAME_DAY_TICKS * 2, 0.5);
        let f = fresh_item(&p, 1.0);
        // 3 GAME_DAY 后，Normal current ≈ 0.354，Cold (0.3×) ≈ 0.354 → 但冰窖效时减慢
        // storage_multiplier=0.3 → 有效 dt = 0.3×actual_dt；实际: 时间更慢衰减
        // 检查 Warn/Safe 状态差异：Normal 应进入 Warn，Cold 应仍 Safe
        let r_normal = consume_food(
            Some((&f, &p)),
            0.20,
            LING_GUO_DURATION_TICKS,
            GAME_DAY_TICKS * 3,
            1.0,
        );
        let r_cold = consume_food(
            Some((&f, &p)),
            0.20,
            LING_GUO_DURATION_TICKS,
            GAME_DAY_TICKS * 3,
            0.3,
        );
        // Normal: threshold=0.5，3 GAME_DAY → current ≈ 1.0 * 0.5^(3/2) ≈ 0.354 < 0.5 → Warn
        // Cold: effective_dt = 0.3 × 3 = 0.9 GAME_DAY → current ≈ 1.0 * 0.5^(0.9/2) ≈ 0.732 > 0.5 → Safe
        assert!(
            matches!(r_normal, ConsumeFoodResult::SpoiledWarn { .. }),
            "Normal 容器 3 GAME_DAY 后应 Warn，实际 {r_normal:?}"
        );
        assert!(
            matches!(r_cold, ConsumeFoodResult::FoodApplied { .. }),
            "冰窖 (×0.3) 同一 tick 应仍 Safe → FoodApplied，实际 {r_cold:?}"
        );
    }
}
