//! 境界突破（plan §3.1 / §3.2）。
//!
//! 支持 5 条升阶路径：Awaken→Induce→Condense→Solidify→Spirit→Void。
//! 成功率公式（plan §3.1）：
//!   `success = base × meridian_integrity × composure × completeness × (1 + bonus)`
//! 辅助材料 bonus 封顶 +0.30。
//!
//! 化虚渡劫为特殊流程（§3.2）：不走本 system 的 try_breakthrough，而是
//! `tribulation.rs::initiate_tribulation` 分发天劫事件。

use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Position, Query, Res};

use crate::combat::components::StatusEffects;
use crate::combat::status::{clear_breakthrough_boost, sum_breakthrough_boost};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::skill::components::SkillId;
use crate::skill::events::SkillCapChanged;

use super::components::{CrackCause, Cultivation, MeridianCrack, MeridianSystem, Realm};
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;

/// 每境界的基础成功率（未叠心境/完整度/材料）。
pub fn base_success_rate(next: Realm) -> f64 {
    match next {
        Realm::Awaken => 1.0,
        Realm::Induce => 0.90,
        Realm::Condense => 0.80,
        Realm::Solidify => 0.70,
        Realm::Spirit => 0.55,
        Realm::Void => 0.30,
    }
}

/// 各境界的 qi 消耗门槛。
pub fn breakthrough_qi_cost(next: Realm) -> f64 {
    match next {
        Realm::Awaken => 0.0,
        Realm::Induce => 8.0,
        Realm::Condense => 25.0,
        Realm::Solidify => 80.0,
        Realm::Spirit => 250.0,
        Realm::Void => 800.0,
    }
}

/// 下一境界（与 try_breakthrough 内部 match 一致）。Void 返回 None。
pub fn next_realm(r: Realm) -> Option<Realm> {
    match r {
        Realm::Awaken => Some(Realm::Induce),
        Realm::Induce => Some(Realm::Condense),
        Realm::Condense => Some(Realm::Solidify),
        Realm::Solidify => Some(Realm::Spirit),
        Realm::Spirit => Some(Realm::Void),
        Realm::Void => None,
    }
}

/// qi_max 乘数（突破后真元池扩张）。
pub fn qi_max_multiplier(next: Realm) -> f64 {
    match next {
        Realm::Awaken => 1.0,
        Realm::Induce => 2.0,
        Realm::Condense => 2.5,
        Realm::Solidify => 3.0,
        Realm::Spirit => 3.5,
        Realm::Void => 5.0,
    }
}

/// plan-skill-v1 §4 境界软挂钩：每个境界压制 skill 的 `effective_lv = min(real_lv, cap)`。
///
/// 数值表（plan §4）：醒灵=3 · 引气=5 · 凝脉=7 · 固元=8 · 通灵=9 · 化虚=10。
/// 代码 Realm 枚举的中文对照见 `components.rs`（Awaken=醒灵 / Induce=引气 / Condense=凝脉 /
/// Solidify=固元 / Spirit=通灵 / Void=化虚）。
pub fn skill_cap_for_realm(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 3,
        Realm::Induce => 5,
        Realm::Condense => 7,
        Realm::Solidify => 8,
        Realm::Spirit => 9,
        Realm::Void => 10,
    }
}

#[derive(Debug, Clone, Event)]
pub struct BreakthroughRequest {
    pub entity: Entity,
    pub material_bonus: f64, // 0.0..=0.30
}

#[derive(Debug, Clone, Event)]
pub struct BreakthroughOutcome {
    pub entity: Entity,
    pub from: Realm,
    pub result: Result<BreakthroughSuccess, BreakthroughError>,
}

#[derive(Debug, Clone, Copy)]
pub struct BreakthroughSuccess {
    pub to: Realm,
    pub success_rate: f64,
    pub used_qi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BreakthroughError {
    AtMaxRealm,
    RequiresTribulation, // Spirit→Void 必须走 tribulation 流程
    NotEnoughMeridians { need: usize, have: usize },
    NotEnoughQi { need: f64, have: f64 },
    RolledFailure { severity: f64 }, // 骰子输了
}

/// 计算修正后的成功率 — plan §3.1 公式。
pub fn compute_success_rate(
    next: Realm,
    meridian_integrity_avg: f64,
    composure: f64,
    completeness: f64,
    material_bonus: f64,
) -> f64 {
    let base = base_success_rate(next);
    let bonus = material_bonus.clamp(0.0, 0.30);
    let raw = base * meridian_integrity_avg * composure * completeness * (1.0 + bonus);
    raw.clamp(0.0, 1.0)
}

pub fn add_pending_material_bonus(cultivation: &mut Cultivation, magnitude: f64) -> f64 {
    let delta = magnitude.clamp(0.0, 0.30);
    cultivation.pending_material_bonus =
        (cultivation.pending_material_bonus + delta).clamp(0.0, 0.30);
    cultivation.pending_material_bonus
}

/// 随机骰子抽象 — 测试时可注入确定值。
pub trait RollSource {
    fn roll_unit(&mut self) -> f64;
}

/// 默认 roll：PRNG 的简单 xorshift（可重现，无需引 rand 依赖）。
pub struct XorshiftRoll(pub u64);
impl RollSource for XorshiftRoll {
    fn roll_unit(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        ((x as f64) / (u64::MAX as f64)).clamp(0.0, 1.0)
    }
}

/// 纯函数：尝试突破。`roll` 可由调用方注入以方便测试（<= success_rate 则成功）。
pub fn try_breakthrough<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    let from = cultivation.realm;
    let next = match from {
        Realm::Awaken => Realm::Induce,
        Realm::Induce => Realm::Condense,
        Realm::Condense => Realm::Solidify,
        Realm::Solidify => Realm::Spirit,
        Realm::Spirit => return Err(BreakthroughError::RequiresTribulation),
        Realm::Void => return Err(BreakthroughError::AtMaxRealm),
    };
    let need = next.required_meridians();
    let have = meridians.opened_count();
    if have < need {
        return Err(BreakthroughError::NotEnoughMeridians { need, have });
    }
    let cost = breakthrough_qi_cost(next);
    if cultivation.qi_current < cost {
        return Err(BreakthroughError::NotEnoughQi {
            need: cost,
            have: cultivation.qi_current,
        });
    }

    let n = meridians.iter().count() as f64;
    let integrity_avg = if n > 0.0 {
        meridians.iter().map(|m| m.integrity).sum::<f64>() / n
    } else {
        1.0
    };
    // completeness：刚好达标 = 1.0，超额每多一条 +0.05（封顶 1.3）
    let completeness = 1.0 + 0.05 * (have as f64 - need as f64);
    let completeness = completeness.clamp(0.8, 1.3);

    let effective_material_bonus =
        (material_bonus + cultivation.pending_material_bonus).clamp(0.0, 0.30);

    let success_rate = compute_success_rate(
        next,
        integrity_avg,
        cultivation.composure,
        completeness,
        effective_material_bonus,
    );

    // 扣费（不论成败）
    cultivation.qi_current -= cost;
    cultivation.pending_material_bonus = 0.0;

    let r = roll.roll_unit();
    if r <= success_rate {
        cultivation.realm = next;
        cultivation.qi_max *= qi_max_multiplier(next);
        cultivation.composure = (cultivation.composure - 0.1).max(0.0);
        Ok(BreakthroughSuccess {
            to: next,
            success_rate,
            used_qi: cost,
        })
    } else {
        // 失败：严重度由 success_rate 反推（越高越惨烈的翻车更罕见）
        let severity = (1.0 - success_rate).clamp(0.1, 0.9);
        // 给 integrity 最高 2 条经脉上裂痕
        let mut targets: Vec<_> = meridians.iter_mut().filter(|m| m.opened).collect();
        targets
            .sort_by(|a, b| (b.rate_tier + b.capacity_tier).cmp(&(a.rate_tier + a.capacity_tier)));
        for m in targets.into_iter().take(2) {
            m.cracks.push(MeridianCrack {
                severity,
                healing_progress: 0.0,
                cause: CrackCause::Backfire,
                created_at: 0,
            });
            m.integrity = (m.integrity - severity * 0.2).max(0.0);
        }
        cultivation.qi_max_frozen =
            Some(cultivation.qi_max_frozen.unwrap_or(0.0) + severity * 10.0);
        cultivation.composure = (cultivation.composure - 0.3).max(0.0);
        Err(BreakthroughError::RolledFailure { severity })
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system signature; one Query/EventWriter per concern.
pub fn breakthrough_system(
    clock: Res<CultivationClock>,
    mut requests: EventReader<BreakthroughRequest>,
    mut outcomes: EventWriter<BreakthroughOutcome>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut players: Query<(&mut Cultivation, &mut MeridianSystem, &mut LifeRecord)>,
    mut status_effects_q: Query<&mut StatusEffects>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
) {
    let mut roll = XorshiftRoll(0x9e3779b97f4a7c15);
    let now = clock.tick;
    for req in requests.read() {
        let Ok((mut cultivation, mut meridians, mut life)) = players.get_mut(req.entity) else {
            continue;
        };
        let from = cultivation.realm;
        if let Some(target) = next_realm(from) {
            life.push(BiographyEntry::BreakthroughStarted {
                realm_target: target,
                tick: now,
            });
        }

        // plan §3.1：material_bonus = req.material_bonus（手动传入，默认 0）
        //   ⊕ 服用突破辅助丹药挂在 StatusEffects 的 BreakthroughBoost buff 聚合值。
        //   最终 clamp 由 compute_success_rate 内部处理。
        let buff_bonus = status_effects_q
            .get(req.entity)
            .map(|se| sum_breakthrough_boost(se) as f64)
            .unwrap_or(0.0);
        let material_bonus = req.material_bonus + buff_bonus;

        let res = try_breakthrough(&mut cultivation, &mut meridians, material_bonus, &mut roll);

        match &res {
            Ok(success) => {
                life.push(BiographyEntry::BreakthroughSucceeded {
                    realm: success.to,
                    tick: now,
                });
                // plan-skill-v1 §4 境界软挂钩：突破到新境界 → 三个 MVP skill 的 cap 全部上调。
                // Client / agent 订阅 SkillCapChanged 做 narration / inspect 面板 effective_lv 展示。
                let new_cap = skill_cap_for_realm(success.to);
                for skill in [SkillId::Herbalism, SkillId::Alchemy, SkillId::Forging] {
                    skill_cap_events.send(SkillCapChanged {
                        char_entity: req.entity,
                        skill,
                        new_cap,
                    });
                }
                // plan-particle-system-v1 §4.4：突破成功发 breakthrough_pillar 光柱。
                if let Ok(pos) = positions.get(req.entity) {
                    let p = pos.get();
                    vfx_events.send(VfxEventRequest::new(
                        p,
                        VfxEventPayloadV1::SpawnParticle {
                            event_id: "bong:breakthrough_pillar".to_string(),
                            origin: [p.x, p.y, p.z],
                            direction: None,
                            color: Some("#FFE8A0".to_string()),
                            strength: Some(1.0),
                            count: Some(12),
                            duration_ticks: Some(60),
                        },
                    ));
                }
            }
            Err(BreakthroughError::RolledFailure { severity }) => {
                if let Some(target) = next_realm(from) {
                    life.push(BiographyEntry::BreakthroughFailed {
                        realm_target: target,
                        severity: *severity,
                        tick: now,
                    });
                }
            }
            Err(_) => {}
        }

        if let Err(BreakthroughError::RolledFailure { severity }) = &res {
            if *severity >= 0.7 {
                // 严重失败 → 走火入魔
                deaths.send(CultivationDeathTrigger {
                    entity: req.entity,
                    cause: CultivationDeathCause::BreakthroughBackfire,
                    context: serde_json::json!({
                        "from": format!("{:?}", from),
                        "severity": severity,
                    }),
                });
            }
        }

        // 不论成败，一次性消费 BreakthroughBoost buff（plan §3.1：辅助丹药为突破"仪式"消耗）
        if let Ok(mut se) = status_effects_q.get_mut(req.entity) {
            clear_breakthrough_boost(&mut se);
        }

        outcomes.send(BreakthroughOutcome {
            entity: req.entity,
            from,
            result: res,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;

    struct FixedRoll(f64);
    impl RollSource for FixedRoll {
        fn roll_unit(&mut self) -> f64 {
            self.0
        }
    }

    fn setup_for_induce() -> (Cultivation, MeridianSystem) {
        let mut c = Cultivation {
            qi_current: 100.0,
            qi_max: 100.0,
            composure: 1.0,
            realm: Realm::Awaken,
            ..Default::default()
        };
        c.realm = Realm::Awaken;
        let mut m = MeridianSystem::default();
        m.get_mut(MeridianId::Lung).opened = true;
        (c, m)
    }

    #[test]
    fn awaken_to_induce_always_succeeds_with_roll_zero() {
        let (mut c, mut m) = setup_for_induce();
        let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();
        assert_eq!(out.to, Realm::Induce);
        assert_eq!(c.realm, Realm::Induce);
    }

    #[test]
    fn awaken_to_induce_fails_with_high_roll() {
        let (mut c, mut m) = setup_for_induce();
        // base 0.9 * integrity 1.0 * composure 1.0 * completeness 1.0 = 0.9 → roll 0.99 fails
        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.99)).unwrap_err();
        assert!(matches!(err, BreakthroughError::RolledFailure { .. }));
        assert_eq!(c.realm, Realm::Awaken);
        // qi 已扣
        assert!(c.qi_current < 100.0);
    }

    #[test]
    fn spirit_to_void_is_gated_by_tribulation() {
        let mut c = Cultivation {
            realm: Realm::Spirit,
            qi_current: 1000.0,
            qi_max: 1000.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        for id in MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
        {
            m.get_mut(*id).opened = true;
        }
        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();
        assert_eq!(err, BreakthroughError::RequiresTribulation);
    }

    #[test]
    fn material_bonus_capped_at_30_percent() {
        let r = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 5.0);
        let r_cap = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 0.30);
        assert!((r - r_cap).abs() < 1e-9);
    }

    #[test]
    fn pending_material_bonus_accumulates_and_caps_at_30_percent() {
        let mut c = Cultivation::default();
        assert!((add_pending_material_bonus(&mut c, 0.12) - 0.12).abs() < 1e-9);
        assert!((add_pending_material_bonus(&mut c, 0.50) - 0.30).abs() < 1e-9);
        assert!((c.pending_material_bonus - 0.30).abs() < 1e-9);
    }

    #[test]
    fn completeness_bounded() {
        // 超额很多不会无限放大
        let r = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.3, 0.0);
        assert!(r <= 1.0);
    }

    #[test]
    fn void_breakthrough_returns_max_realm_error() {
        let mut c = Cultivation {
            realm: Realm::Void,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();
        assert_eq!(err, BreakthroughError::AtMaxRealm);
    }

    #[test]
    fn pending_material_bonus_is_consumed_on_real_attempt() {
        let (mut c, mut m) = setup_for_induce();
        c.pending_material_bonus = 0.12;

        let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

        let expected = compute_success_rate(Realm::Induce, 1.0, 1.0, 1.0, 0.12);
        assert!((out.success_rate - expected).abs() < 1e-9);
        assert_eq!(c.pending_material_bonus, 0.0);
    }

    #[test]
    fn pending_material_bonus_is_preserved_when_preconditions_fail() {
        let mut c = Cultivation {
            qi_current: 1.0,
            pending_material_bonus: 0.12,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        m.get_mut(MeridianId::Lung).opened = true;

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert!(matches!(err, BreakthroughError::NotEnoughQi { .. }));
        assert!((c.pending_material_bonus - 0.12).abs() < 1e-9);
    }

    /// plan-skill-v1 §4 cap 表锚点：六境界分别对应 3/5/7/8/9/10。
    #[test]
    fn skill_cap_for_realm_matches_plan_section_four() {
        assert_eq!(skill_cap_for_realm(Realm::Awaken), 3);
        assert_eq!(skill_cap_for_realm(Realm::Induce), 5);
        assert_eq!(skill_cap_for_realm(Realm::Condense), 7);
        assert_eq!(skill_cap_for_realm(Realm::Solidify), 8);
        assert_eq!(skill_cap_for_realm(Realm::Spirit), 9);
        assert_eq!(skill_cap_for_realm(Realm::Void), 10);
    }
}
