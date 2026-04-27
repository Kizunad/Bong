//! plan-forge-v1 §1.3 四步核心逻辑 —— 纯函数，便于单测。
//!
//! 每步独立可失败；失败 = 下调 bucket（flawed → waste/explode）。

use std::collections::HashMap;

use super::blueprint::{
    BilletProfile, Blueprint, ConsecrationProfile, InscriptionProfile, MaterialStack, StepKind,
    StepSpec, TemperBeat, TemperingProfile,
};
use super::events::ForgeBucket;
use super::session::{
    BilletState, ConsecrationState, ForgeSession, ForgeStep, InscriptionState, StepState,
    TemperingState,
};
use crate::cultivation::components::ColorKind;

// ══════════════════════════════ Billet ══════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BilletError {
    /// 缺料：return materials missing count > tolerance。
    ShortMaterial { material: String, missing: u32 },
    /// 多了未知材料（走 flawed_fallback）。
    UnknownMaterial { material: String },
}

#[derive(Debug, Clone)]
pub struct BilletResolution {
    pub state: BilletState,
    /// 是否完全达标（所有 required 满足 + 无冗余）。
    pub perfect: bool,
    /// 此次进入 flawed 路径？
    pub flawed: bool,
}

/// 解析一次坯料投料：
/// - required 全满 + 无异物 → perfect
/// - required 缺 count_miss 以内 → flawed
/// - required 缺得更多 → Err(ShortMaterial)（触发 waste）
/// - 有未知材料 → flawed（走 fallback）
pub fn resolve_billet(
    profile: &BilletProfile,
    inputs: &HashMap<String, u32>,
    blueprint_tier_cap: u8,
) -> Result<BilletResolution, BilletError> {
    let mut state = BilletState {
        materials_in: inputs.clone(),
        ..Default::default()
    };

    let mut missing_total = 0u32;
    for MaterialStack { material, count } in &profile.required {
        let have = inputs.get(material).copied().unwrap_or(0);
        if have < *count {
            missing_total = missing_total.saturating_add(*count - have);
        }
    }

    if missing_total > profile.tolerance.count_miss {
        // 找出第一个不足的返回
        for MaterialStack { material, count } in &profile.required {
            let have = inputs.get(material).copied().unwrap_or(0);
            if have < *count {
                return Err(BilletError::ShortMaterial {
                    material: material.clone(),
                    missing: count - have,
                });
            }
        }
    }

    // 异物检测：inputs 中不在 required/optional_carriers 列表的 material
    let mut known: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for m in &profile.required {
        known.insert(m.material.as_str());
    }
    for c in &profile.optional_carriers {
        known.insert(c.material.as_str());
    }
    let mut flawed = missing_total > 0;
    for k in inputs.keys() {
        if !known.contains(k.as_str()) {
            flawed = true;
        }
    }

    // 载体决定 tier_cap（选"最强"的）。
    let mut carrier_cap: u8 = 1; // 无载体 → 法器上限由图谱 step 数量决定，这里用 1 打底
    let mut active_carrier: Option<String> = None;
    for c in &profile.optional_carriers {
        if inputs.contains_key(&c.material) && c.unlocks_tier > carrier_cap {
            carrier_cap = c.unlocks_tier;
            active_carrier = Some(c.material.clone());
        }
    }
    // 无 optional carriers 配置时使用图谱 tier_cap（凡铁/法器路径）。
    let resolved = if profile.optional_carriers.is_empty() {
        blueprint_tier_cap
    } else {
        carrier_cap.min(blueprint_tier_cap)
    };

    state.active_carrier = active_carrier;
    state.resolved_tier_cap = resolved;

    Ok(BilletResolution {
        state,
        perfect: !flawed,
        flawed,
    })
}

// ══════════════════════════════ Tempering ══════════════════════════════

pub fn apply_tempering_hit(
    profile: &TemperingProfile,
    state: &mut TemperingState,
    beat: TemperBeat,
    ticks_remaining: u32,
    window_bonus_ticks: u32,
) {
    if state.beat_cursor >= profile.pattern.len() {
        return;
    }
    let expected = profile.pattern[state.beat_cursor];
    let window_limit = profile.window_ticks.saturating_add(window_bonus_ticks);
    let in_window = ticks_remaining > 0 && ticks_remaining <= window_limit;
    if expected == beat && in_window {
        state.hits = state.hits.saturating_add(1);
    } else {
        state.misses = state.misses.saturating_add(1);
        state.deviation = state.deviation.saturating_add(1);
    }
    state.qi_spent += profile.qi_per_hit;
    state.beat_cursor += 1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemperingResult {
    Perfect,
    Good,
    Flawed,
    Waste,
}

pub fn resolve_tempering(
    profile: &TemperingProfile,
    state: &TemperingState,
    allowed_miss_bonus: u32,
) -> TemperingResult {
    let allowed = profile
        .tolerance
        .miss_allowed
        .saturating_add(allowed_miss_bonus);
    let total = profile.pattern.len() as u32;
    if state.misses == 0 && state.hits >= total {
        TemperingResult::Perfect
    } else if state.misses <= allowed {
        TemperingResult::Good
    } else if state.misses <= allowed.saturating_mul(2) {
        TemperingResult::Flawed
    } else {
        TemperingResult::Waste
    }
}

// ══════════════════════════════ Inscription ══════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InscriptionResult {
    Filled,
    /// 公差内允许的少填（flawed）。
    Partial,
    Failed,
}

pub fn apply_scroll(state: &mut InscriptionState, scroll_id: String) {
    state.scrolls_in.push(scroll_id);
    state.filled_slots = state.filled_slots.saturating_add(1);
}

/// 在该步结束时判定。`roll_fail ∈ [0,1)` 由调用方提供（通常 RNG）。
pub fn resolve_inscription(
    profile: &InscriptionProfile,
    state: &InscriptionState,
    roll_fail: f32,
    failure_rate_reduction: f32,
) -> InscriptionResult {
    if state.filled_slots < profile.required_scroll_count {
        return InscriptionResult::Partial;
    }
    let adjusted_fail_chance =
        profile.tolerance.fail_chance * (1.0 - failure_rate_reduction.clamp(0.0, 1.0));
    if roll_fail < adjusted_fail_chance {
        InscriptionResult::Failed
    } else {
        InscriptionResult::Filled
    }
}

// ══════════════════════════════ Consecration ══════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsecrationResult {
    Succeeded { color: ColorKind },
    Insufficient,
    Failed,
}

pub fn inject_qi(state: &mut ConsecrationState, amount: f64) {
    state.qi_injected += amount;
}

pub fn resolve_consecration(
    profile: &ConsecrationProfile,
    state: &ConsecrationState,
    caster_color: ColorKind,
    caster_realm: crate::cultivation::components::Realm,
) -> ConsecrationResult {
    // 境界门槛：min_realm 以下必败。
    if (caster_realm as u8) < (profile.min_realm as u8) {
        return ConsecrationResult::Failed;
    }
    let missing = (profile.qi_cost - state.qi_injected) / profile.qi_cost;
    if missing > profile.tolerance.qi_miss_ratio {
        return ConsecrationResult::Insufficient;
    }
    ConsecrationResult::Succeeded {
        color: caster_color,
    }
}

// ══════════════════════════════ 驱动总表 ══════════════════════════════

/// 依据 session 当前 step_index 返回下一个 ForgeStep（若无 = Done）。
pub fn next_step_after(bp: &Blueprint, current_idx: usize) -> ForgeStep {
    let next_idx = current_idx + 1;
    bp.steps
        .get(next_idx)
        .map(|s| ForgeStep::from_kind(s.kind()))
        .unwrap_or(ForgeStep::Done)
}

/// 进入下一步并初始化 StepState。
pub fn advance_step(session: &mut ForgeSession, bp: &Blueprint) {
    session.step_index += 1;
    if let Some(spec) = bp.steps.get(session.step_index) {
        session.current_step = ForgeStep::from_kind(spec.kind());
        session.step_state = init_state_for(spec);
    } else {
        session.current_step = ForgeStep::Done;
        session.step_state = StepState::None;
    }
}

fn init_state_for(spec: &StepSpec) -> StepState {
    match spec {
        StepSpec::Billet { .. } => StepState::Billet(BilletState::default()),
        StepSpec::Tempering { .. } => StepState::Tempering(TemperingState::default()),
        StepSpec::Inscription { .. } => StepState::Inscription(InscriptionState::default()),
        StepSpec::Consecration { profile } => StepState::Consecration(ConsecrationState {
            qi_injected: 0.0,
            qi_required: profile.qi_cost,
            color_imprint: None,
        }),
    }
}

/// 计算最终达成品阶（plan §2）。
///
/// - 坯料成：凡器（tier 1）
/// - + 淬炼达标：法器（tier 2）
/// - + 铭文成：灵器（tier 3）
/// - + 开光成：道器（tier 4）
///
/// 但不超过 `billet.resolved_tier_cap` 与 `blueprint.tier_cap`。
pub fn compute_achieved_tier(
    bp: &Blueprint,
    billet_ok: bool,
    tempering_good: Option<bool>,
    inscription_ok: Option<bool>,
    consecration_ok: Option<bool>,
    billet_carrier_cap: u8,
) -> u8 {
    if !billet_ok {
        return 0;
    }
    let mut tier: u8 = 1;
    if matches!(tempering_good, Some(true)) && bp.has_step(StepKind::Tempering) {
        tier = 2;
    } else if bp.has_step(StepKind::Tempering) && matches!(tempering_good, Some(false) | None) {
        // 淬炼定义但未通过 → 锁死在凡器
        tier = 1;
    }
    if matches!(inscription_ok, Some(true)) && bp.has_step(StepKind::Inscription) {
        tier = tier.max(3);
    }
    if matches!(consecration_ok, Some(true)) && bp.has_step(StepKind::Consecration) {
        tier = tier.max(4);
    }
    // 道器必须开光（plan §1.3.4）。
    if bp.has_step(StepKind::Consecration) && !matches!(consecration_ok, Some(true)) {
        tier = tier.min(3);
    }
    tier.min(bp.tier_cap).min(billet_carrier_cap)
}

/// bucket 选择：根据各步结果聚合。
pub fn select_bucket(
    billet_ok: bool,
    billet_flawed: bool,
    tempering: Option<TemperingResult>,
    inscription: Option<InscriptionResult>,
    consecration: Option<ConsecrationResult>,
) -> ForgeBucket {
    if !billet_ok {
        return ForgeBucket::Waste;
    }
    if matches!(tempering, Some(TemperingResult::Waste)) {
        return ForgeBucket::Waste;
    }
    let any_flaw = billet_flawed
        || matches!(tempering, Some(TemperingResult::Flawed))
        || matches!(
            inscription,
            Some(InscriptionResult::Failed | InscriptionResult::Partial)
        )
        || matches!(
            consecration,
            Some(ConsecrationResult::Insufficient | ConsecrationResult::Failed)
        );
    if any_flaw {
        return ForgeBucket::Flawed;
    }
    let all_perfect = matches!(tempering, Some(TemperingResult::Perfect) | None)
        && matches!(inscription, Some(InscriptionResult::Filled) | None)
        && matches!(
            consecration,
            Some(ConsecrationResult::Succeeded { .. }) | None
        );
    if all_perfect {
        ForgeBucket::Perfect
    } else {
        ForgeBucket::Good
    }
}

// ══════════════════════════════ Tests ══════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::forge::blueprint::{
        BilletTolerance, CarrierSpec, ConsecrationTolerance, InscriptionTolerance,
        TemperingTolerance,
    };

    fn billet_profile_iron() -> BilletProfile {
        BilletProfile {
            required: vec![MaterialStack {
                material: "fan_tie".into(),
                count: 3,
            }],
            optional_carriers: vec![],
            tolerance: BilletTolerance { count_miss: 0 },
        }
    }

    #[test]
    fn billet_perfect_when_all_required_satisfied() {
        let p = billet_profile_iron();
        let mut inputs = HashMap::new();
        inputs.insert("fan_tie".to_string(), 3);
        let r = resolve_billet(&p, &inputs, 1).unwrap();
        assert!(r.perfect);
        assert!(!r.flawed);
        assert_eq!(r.state.resolved_tier_cap, 1);
    }

    #[test]
    fn billet_flawed_when_unknown_material_present() {
        let p = billet_profile_iron();
        let mut inputs = HashMap::new();
        inputs.insert("fan_tie".to_string(), 3);
        inputs.insert("dirt".to_string(), 1);
        let r = resolve_billet(&p, &inputs, 1).unwrap();
        assert!(r.flawed);
    }

    #[test]
    fn billet_waste_when_missing_beyond_tolerance() {
        let p = billet_profile_iron();
        let mut inputs = HashMap::new();
        inputs.insert("fan_tie".to_string(), 1);
        let err = resolve_billet(&p, &inputs, 1).unwrap_err();
        assert!(matches!(err, BilletError::ShortMaterial { .. }));
    }

    #[test]
    fn carrier_unlocks_higher_tier_cap() {
        // plan-mineral-v1 §5 — placeholder 替换：xuan_iron → sui_tie。
        // yi_beast_bone 属 fauna 范围（plan §5 依赖切分），保留 TODO 等 plan-fauna-v1 立项替换。
        let p = BilletProfile {
            required: vec![MaterialStack {
                material: "sui_tie".into(),
                count: 3,
            }],
            optional_carriers: vec![
                CarrierSpec {
                    material: "ling_wood".into(),
                    unlocks_tier: 3,
                },
                CarrierSpec {
                    // TODO[fauna]: yi_beast_bone → 妖兽材料正典（plan-fauna-v1 立项后替换）
                    material: "yi_beast_bone".into(),
                    unlocks_tier: 4,
                },
            ],
            tolerance: BilletTolerance::default(),
        };
        let mut inputs = HashMap::new();
        inputs.insert("sui_tie".into(), 3);
        inputs.insert("yi_beast_bone".into(), 1);
        let r = resolve_billet(&p, &inputs, 4).unwrap();
        assert_eq!(r.state.resolved_tier_cap, 4);
        assert_eq!(r.state.active_carrier.as_deref(), Some("yi_beast_bone"));
    }

    #[test]
    fn tempering_perfect_when_all_hit() {
        let profile = TemperingProfile {
            pattern: vec![TemperBeat::Light, TemperBeat::Heavy, TemperBeat::Fold],
            window_ticks: 10,
            qi_per_hit: 0.5,
            tolerance: TemperingTolerance { miss_allowed: 0 },
        };
        let mut s = TemperingState::default();
        apply_tempering_hit(&profile, &mut s, TemperBeat::Light, 5, 0);
        apply_tempering_hit(&profile, &mut s, TemperBeat::Heavy, 5, 0);
        apply_tempering_hit(&profile, &mut s, TemperBeat::Fold, 5, 0);
        assert_eq!(resolve_tempering(&profile, &s, 0), TemperingResult::Perfect);
        assert!((s.qi_spent - 1.5).abs() < 1e-9);
    }

    #[test]
    fn tempering_miss_counts_deviation() {
        let profile = TemperingProfile {
            pattern: vec![TemperBeat::Light, TemperBeat::Heavy],
            window_ticks: 10,
            qi_per_hit: 0.5,
            tolerance: TemperingTolerance { miss_allowed: 1 },
        };
        let mut s = TemperingState::default();
        apply_tempering_hit(&profile, &mut s, TemperBeat::Heavy, 5, 0); // wrong
        apply_tempering_hit(&profile, &mut s, TemperBeat::Heavy, 5, 0); // right
        assert_eq!(s.misses, 1);
        assert_eq!(resolve_tempering(&profile, &s, 0), TemperingResult::Good);
    }

    #[test]
    fn tempering_out_of_window_counts_as_miss() {
        let profile = TemperingProfile {
            pattern: vec![TemperBeat::Light],
            window_ticks: 5,
            qi_per_hit: 0.5,
            tolerance: TemperingTolerance { miss_allowed: 0 },
        };
        let mut s = TemperingState::default();
        apply_tempering_hit(&profile, &mut s, TemperBeat::Light, 0, 0); // 过窗
        assert_eq!(s.misses, 1);
    }

    #[test]
    fn tempering_window_bonus_allows_late_hit_inside_extended_window() {
        let profile = TemperingProfile {
            pattern: vec![TemperBeat::Light],
            window_ticks: 5,
            qi_per_hit: 0.5,
            tolerance: TemperingTolerance { miss_allowed: 0 },
        };
        let mut s = TemperingState::default();
        apply_tempering_hit(&profile, &mut s, TemperBeat::Light, 7, 3);
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 0);
    }

    #[test]
    fn tempering_allowed_miss_bonus_upgrades_result() {
        let profile = TemperingProfile {
            pattern: vec![TemperBeat::Light, TemperBeat::Heavy],
            window_ticks: 10,
            qi_per_hit: 0.5,
            tolerance: TemperingTolerance { miss_allowed: 1 },
        };
        let mut s = TemperingState::default();
        apply_tempering_hit(&profile, &mut s, TemperBeat::Heavy, 5, 0);
        apply_tempering_hit(&profile, &mut s, TemperBeat::Light, 5, 0);
        assert_eq!(resolve_tempering(&profile, &s, 0), TemperingResult::Flawed);
        assert_eq!(resolve_tempering(&profile, &s, 1), TemperingResult::Good);
    }

    #[test]
    fn inscription_partial_when_not_enough_scrolls() {
        let p = InscriptionProfile {
            slots: 2,
            required_scroll_count: 2,
            tolerance: InscriptionTolerance { fail_chance: 0.0 },
        };
        let mut s = InscriptionState::default();
        apply_scroll(&mut s, "insc_a".into());
        assert_eq!(
            resolve_inscription(&p, &s, 0.5, 0.0),
            InscriptionResult::Partial
        );
        apply_scroll(&mut s, "insc_b".into());
        assert_eq!(
            resolve_inscription(&p, &s, 0.5, 0.0),
            InscriptionResult::Filled
        );
    }

    #[test]
    fn inscription_fails_by_roll() {
        let p = InscriptionProfile {
            slots: 1,
            required_scroll_count: 1,
            tolerance: InscriptionTolerance { fail_chance: 0.5 },
        };
        let mut s = InscriptionState::default();
        apply_scroll(&mut s, "x".into());
        assert_eq!(
            resolve_inscription(&p, &s, 0.1, 0.0),
            InscriptionResult::Failed
        );
        assert_eq!(
            resolve_inscription(&p, &s, 0.9, 0.0),
            InscriptionResult::Filled
        );
    }

    #[test]
    fn inscription_failure_reduction_lowers_effective_fail_chance() {
        let p = InscriptionProfile {
            slots: 1,
            required_scroll_count: 1,
            tolerance: InscriptionTolerance { fail_chance: 0.5 },
        };
        let mut s = InscriptionState::default();
        apply_scroll(&mut s, "x".into());
        assert_eq!(
            resolve_inscription(&p, &s, 0.4, 0.0),
            InscriptionResult::Failed
        );
        assert_eq!(
            resolve_inscription(&p, &s, 0.4, 0.3),
            InscriptionResult::Filled
        );
    }

    #[test]
    fn consecration_requires_min_realm() {
        let p = ConsecrationProfile {
            qi_cost: 80.0,
            min_realm: Realm::Spirit,
            tolerance: ConsecrationTolerance {
                qi_miss_ratio: 0.05,
            },
        };
        let mut s = ConsecrationState {
            qi_injected: 80.0,
            qi_required: 80.0,
            color_imprint: None,
        };
        // Condense < Spirit → Failed
        assert_eq!(
            resolve_consecration(&p, &s, ColorKind::Sharp, Realm::Condense),
            ConsecrationResult::Failed
        );
        // Spirit 达标 → Succeeded
        assert_eq!(
            resolve_consecration(&p, &s, ColorKind::Sharp, Realm::Spirit),
            ConsecrationResult::Succeeded {
                color: ColorKind::Sharp
            }
        );
        // 真元不足 → Insufficient
        s.qi_injected = 50.0;
        assert_eq!(
            resolve_consecration(&p, &s, ColorKind::Sharp, Realm::Spirit),
            ConsecrationResult::Insufficient
        );
    }

    #[test]
    fn achieved_tier_climbs_with_each_step() {
        // Mock blueprint with all 4 steps.
        let bp = Blueprint {
            id: "x".into(),
            name: "x".into(),
            station_tier_min: 1,
            tier_cap: 4,
            steps: vec![
                StepSpec::Billet {
                    profile: billet_profile_iron(),
                },
                StepSpec::Tempering {
                    profile: TemperingProfile {
                        pattern: vec![TemperBeat::Light],
                        window_ticks: 5,
                        qi_per_hit: 0.1,
                        tolerance: TemperingTolerance::default(),
                    },
                },
                StepSpec::Inscription {
                    profile: InscriptionProfile {
                        slots: 1,
                        required_scroll_count: 1,
                        tolerance: InscriptionTolerance::default(),
                    },
                },
                StepSpec::Consecration {
                    profile: ConsecrationProfile {
                        qi_cost: 10.0,
                        min_realm: Realm::Awaken,
                        tolerance: ConsecrationTolerance::default(),
                    },
                },
            ],
            outcomes: crate::forge::blueprint::OutcomesSpec {
                perfect: None,
                good: None,
                flawed: None,
                waste: None,
                explode: None,
            },
            flawed_fallback: None,
        };
        assert_eq!(
            compute_achieved_tier(&bp, true, Some(true), Some(true), Some(true), 4),
            4
        );
        // 跳开光 → 道器锁死回灵器
        assert_eq!(
            compute_achieved_tier(&bp, true, Some(true), Some(true), None, 4),
            3
        );
        // 仅坯料 → 凡器
        assert_eq!(
            compute_achieved_tier(&bp, true, Some(false), None, None, 4),
            1
        );
        // carrier_cap 压制
        assert_eq!(
            compute_achieved_tier(&bp, true, Some(true), Some(true), Some(true), 2),
            2
        );
    }

    #[test]
    fn bucket_selection() {
        // All clean → Perfect
        assert_eq!(
            select_bucket(
                true,
                false,
                Some(TemperingResult::Perfect),
                Some(InscriptionResult::Filled),
                Some(ConsecrationResult::Succeeded {
                    color: ColorKind::Sharp
                })
            ),
            ForgeBucket::Perfect
        );
        // Billet flawed → Flawed
        assert_eq!(
            select_bucket(true, true, None, None, None),
            ForgeBucket::Flawed
        );
        // Tempering waste → Waste
        assert_eq!(
            select_bucket(true, false, Some(TemperingResult::Waste), None, None),
            ForgeBucket::Waste
        );
    }
}
