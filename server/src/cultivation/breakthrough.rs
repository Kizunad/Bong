//! 境界突破（plan §3.1 / §3.2）。
//!
//! 支持 5 条升阶路径：Awaken→Induce→Condense→Solidify→Spirit→Void。
//! 成功率公式（plan §3.1）：
//!   `success = base × meridian_integrity × composure × completeness × (1 + bonus)`
//! 辅助材料 bonus 封顶 +0.30。
//!
//! 化虚渡劫为特殊流程（§3.2）：不走本 system 的 try_breakthrough，而是
//! `tribulation.rs::initiate_tribulation` 分发天劫事件。

use valence::prelude::{
    bevy_ecs, bevy_ecs::system::SystemParam, BlockPos, Entity, Event, EventReader, EventWriter,
    Events, Position, Query, Res, ResMut, Username,
};

use crate::combat::components::StatusEffects;
use crate::combat::status::{
    clear_breakthrough_boost, clear_du_jie_dan_damage_reduction, sum_breakthrough_boost,
};
use crate::network::gameplay_vfx;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::spawn::NpcMarker;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::qi_physics::{
    QiAccountId, QiPhysicsError, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::schema::common::NarrationStyle;
use crate::skill::components::SkillId;
use crate::skill::events::{SkillCapChanged, SkillXpGain, XpGainSource};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::{KarmaWeightStore, KARMA_WEIGHT_MAX};
use crate::world::season::{query_season, Season};
use crate::world::spirit_eye::{SpiritEyeId, SpiritEyeRegistry, SpiritEyeUsedForBreakthroughEvent};
use crate::world::zone::ZoneRegistry;

use super::components::{CrackCause, Cultivation, MeridianCrack, MeridianSystem, Realm};
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use super::life_record::{BiographyEntry, LifeRecord};
use super::meridian_open::MIN_ZONE_QI_TO_OPEN;
use super::tick::CultivationClock;

pub const RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS: u64 = 30 * 24 * 60 * 60 * 20;
pub const RAPID_BREAKTHROUGH_KARMA_WEIGHT_DELTA: f32 = KARMA_WEIGHT_MAX;
pub const MIN_ZONE_QI_TO_BREAKTHROUGH: f64 = MIN_ZONE_QI_TO_OPEN;
pub const MIN_ZONE_QI_TO_GUYUAN: f64 = 0.80;
pub const SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS: f64 = 0.30;
pub const BLOOD_VALLEY_BREAKTHROUGH_SUCCESS_BONUS: f64 = 0.50;
/// 突破失败时 `qi_max_frozen` 累计上限（占 qi_max 的比例）。
///
/// 与 `overload.rs` 的 0.5 上限对齐：多次连续失败不会把有效真元上限打到 0，
/// 玩家始终保留至少 50% 的 qi_max 可用额度，避免永久废人。
pub const BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO: f64 = 0.5;

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
    NotEnoughMeridians {
        need: usize,
        have: usize,
    },
    NotEnoughRegularMeridians {
        need: usize,
        have: usize,
    },
    NotEnoughExtraordinaryMeridians {
        need: usize,
        have: usize,
    },
    NotEnoughQi {
        need: f64,
        have: f64,
    },
    ZoneTooWeak {
        need: f64,
        have: f64,
    },
    EnvInsufficient {
        need: f64,
        have: f64,
        in_spirit_eye: bool,
    },
    LedgerUnavailable,
    RolledFailure {
        severity: f64,
    }, // 骰子输了
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BreakthroughLedgerError {
    MissingStableActorId { is_npc: bool },
    QiPhysics(QiPhysicsError),
}

impl From<QiPhysicsError> for BreakthroughLedgerError {
    fn from(error: QiPhysicsError) -> Self {
        Self::QiPhysics(error)
    }
}

fn breakthrough_error_message(error: &BreakthroughError) -> String {
    match error {
        BreakthroughError::AtMaxRealm => "突破未成：你已抵达当前最高境界。".to_string(),
        BreakthroughError::RequiresTribulation => {
            "突破未成：通灵至化虚必须先走渡虚劫。".to_string()
        }
        BreakthroughError::NotEnoughMeridians { need, have } => {
            format!("突破未成：需先打通 {need} 条经脉（当前 {have}）。")
        }
        BreakthroughError::NotEnoughRegularMeridians { need, have } => {
            format!("突破未成：需先打通 {need} 条正经（当前 {have}）。")
        }
        BreakthroughError::NotEnoughExtraordinaryMeridians { need, have } => {
            format!("突破未成：需先打通 {need} 条奇经（当前 {have}）。")
        }
        BreakthroughError::NotEnoughQi { need, have } => {
            format!("突破未成：真元不足（需 {need:.1}，当前 {have:.1}）。")
        }
        BreakthroughError::ZoneTooWeak { need, have } => {
            format!("突破未成：此地灵气不足（需 {need:.2}，当前 {have:.2}）。")
        }
        BreakthroughError::EnvInsufficient {
            need,
            have,
            in_spirit_eye,
        } => {
            if *in_spirit_eye {
                format!("突破未成：灵眼扰动未稳（需 {need:.2}，当前 {have:.2}）。")
            } else {
                format!("突破未成：固元须在灵气浓处或灵眼内（需 {need:.2}，当前 {have:.2}）。")
            }
        }
        BreakthroughError::LedgerUnavailable => "突破未成：真元账本未就绪，仪式暂缓。".to_string(),
        BreakthroughError::RolledFailure { severity } => {
            format!("突破失败：气机反噬，伤势强度 {severity:.2}。")
        }
    }
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

pub fn compute_success_rate_with_env_bonus(
    next: Realm,
    meridian_integrity_avg: f64,
    composure: f64,
    completeness: f64,
    material_bonus: f64,
    env_bonus: f64,
) -> f64 {
    let material = material_bonus.clamp(0.0, 0.30);
    let env_bonus = env_bonus.clamp(0.0, 0.50);
    let raw = base_success_rate(next)
        * meridian_integrity_avg
        * composure
        * completeness
        * (1.0 + material)
        * (1.0 + env_bonus);
    raw.clamp(0.0, 1.0)
}

pub fn season_success_modifier(season: Season) -> f64 {
    match season {
        Season::Summer => 1.05,
        Season::Winter => 0.95,
        Season::SummerToWinter | Season::WinterToSummer => 0.85,
    }
}

pub fn compute_success_rate_with_env_and_season_bonus(
    next: Realm,
    meridian_integrity_avg: f64,
    composure: f64,
    completeness: f64,
    material_bonus: f64,
    env_bonus: f64,
    season: Season,
) -> f64 {
    let material = material_bonus.clamp(0.0, 0.30);
    let env_bonus = env_bonus.clamp(0.0, 0.50);
    let raw = base_success_rate(next)
        * meridian_integrity_avg
        * composure
        * completeness
        * (1.0 + material)
        * (1.0 + env_bonus)
        * season_success_modifier(season);
    raw.clamp(0.0, 1.0)
}

pub fn add_pending_material_bonus(cultivation: &mut Cultivation, magnitude: f64) -> f64 {
    let delta = magnitude.clamp(0.0, 0.30);
    cultivation.pending_material_bonus =
        (cultivation.pending_material_bonus + delta).clamp(0.0, 0.30);
    cultivation.pending_material_bonus
}

fn breakthrough_precondition_error(
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
) -> Option<BreakthroughError> {
    let next = match cultivation.realm {
        Realm::Awaken => Realm::Induce,
        Realm::Induce => Realm::Condense,
        Realm::Condense => Realm::Solidify,
        Realm::Solidify => Realm::Spirit,
        Realm::Spirit => return Some(BreakthroughError::RequiresTribulation),
        Realm::Void => return Some(BreakthroughError::AtMaxRealm),
    };
    let need = next.required_meridians();
    let have = meridians.opened_count();
    if have < need {
        return Some(BreakthroughError::NotEnoughMeridians { need, have });
    }

    let regular_have = meridians.regular_opened_count();
    let extraordinary_have = meridians.extraordinary_opened_count();
    match next {
        Realm::Induce if regular_have < 3 => {
            return Some(BreakthroughError::NotEnoughRegularMeridians {
                need: 3,
                have: regular_have,
            });
        }
        Realm::Condense if regular_have < 6 => {
            return Some(BreakthroughError::NotEnoughRegularMeridians {
                need: 6,
                have: regular_have,
            });
        }
        Realm::Solidify if regular_have < 12 => {
            return Some(BreakthroughError::NotEnoughRegularMeridians {
                need: 12,
                have: regular_have,
            });
        }
        Realm::Spirit if regular_have < 12 => {
            return Some(BreakthroughError::NotEnoughRegularMeridians {
                need: 12,
                have: regular_have,
            });
        }
        Realm::Spirit if extraordinary_have < 4 => {
            return Some(BreakthroughError::NotEnoughExtraordinaryMeridians {
                need: 4,
                have: extraordinary_have,
            });
        }
        _ => {}
    }

    let cost = breakthrough_qi_cost(next);
    if cultivation.qi_current < cost {
        return Some(BreakthroughError::NotEnoughQi {
            need: cost,
            have: cultivation.qi_current,
        });
    }
    None
}

fn breakthrough_environment_error(
    position: &Position,
    dimension: DimensionKind,
    zones: Option<&ZoneRegistry>,
    spirit_eyes: Option<&SpiritEyeRegistry>,
    from: Realm,
) -> Option<BreakthroughError> {
    let zone_qi = zones
        .and_then(|zones| zones.find_zone(dimension, position.get()))
        .map(|zone| zone.spirit_qi)
        .unwrap_or(0.0);
    let in_spirit_eye = spirit_eyes
        .and_then(|registry| registry.spirit_eye_qi_at(dimension, position.get()))
        .is_some();

    if next_realm(from) == Some(Realm::Solidify) {
        if zone_qi >= MIN_ZONE_QI_TO_GUYUAN || in_spirit_eye {
            None
        } else {
            Some(BreakthroughError::EnvInsufficient {
                need: MIN_ZONE_QI_TO_GUYUAN,
                have: zone_qi,
                in_spirit_eye,
            })
        }
    } else if zone_qi < MIN_ZONE_QI_TO_BREAKTHROUGH {
        Some(BreakthroughError::ZoneTooWeak {
            need: MIN_ZONE_QI_TO_BREAKTHROUGH,
            have: zone_qi,
        })
    } else {
        None
    }
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
    try_breakthrough_with_env_bonus(cultivation, meridians, material_bonus, 0.0, roll)
}

pub fn attempt_breakthrough_guyuan<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    spirit_eye_bonus: f64,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    try_breakthrough_with_env_bonus(
        cultivation,
        meridians,
        material_bonus,
        spirit_eye_bonus,
        roll,
    )
}

pub fn try_breakthrough_with_env_bonus<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    env_bonus: f64,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    try_breakthrough_with_env_season_bonus(
        cultivation,
        meridians,
        material_bonus,
        env_bonus,
        None,
        roll,
    )
}

pub fn try_breakthrough_with_env_season_bonus<R: RollSource>(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    material_bonus: f64,
    env_bonus: f64,
    season: Option<Season>,
    roll: &mut R,
) -> Result<BreakthroughSuccess, BreakthroughError> {
    let from = cultivation.realm;
    if let Some(error) = breakthrough_precondition_error(cultivation, meridians) {
        return Err(error);
    }
    let next = next_realm(from).expect("precondition check rejects max realm");
    let need = next.required_meridians();
    let have = meridians.opened_count();
    let cost = breakthrough_qi_cost(next);

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

    let success_rate = match season {
        Some(season) => compute_success_rate_with_env_and_season_bonus(
            next,
            integrity_avg,
            cultivation.composure,
            completeness,
            effective_material_bonus,
            env_bonus,
            season,
        ),
        None => compute_success_rate_with_env_bonus(
            next,
            integrity_avg,
            cultivation.composure,
            completeness,
            effective_material_bonus,
            env_bonus,
        ),
    };

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
        // 突破失败：真元上限冻结。severity ∈ [0.1, 0.9]，每次加 severity * 10.0（即 1.0..9.0）。
        // 无 cap 时多次失败可致 qi_max_frozen ≥ qi_max → 有效上限归零 → 玩家永久废人。
        // 与 overload.rs 对齐：冻结量不超过 qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO (0.5)。
        let new_frozen = (cultivation.qi_max_frozen.unwrap_or(0.0) + severity * 10.0)
            .min(cultivation.qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO);
        cultivation.qi_max_frozen = Some(new_frozen);
        cultivation.composure = (cultivation.composure - 0.3).max(0.0);
        Err(BreakthroughError::RolledFailure { severity })
    }
}

fn breakthrough_season(
    position_context: Option<(&Position, DimensionKind)>,
    zones: Option<&ZoneRegistry>,
    tick: u64,
) -> Season {
    let zone_name = position_context
        .and_then(|(position, dimension)| {
            zones.and_then(|zones| {
                zones
                    .find_zone(dimension, position.get())
                    .map(|zone| zone.name.as_str())
            })
        })
        .unwrap_or("");
    query_season(zone_name, tick).season
}

fn spirit_eye_env_bonus_for(from: Realm, blood_valley: Option<bool>) -> f64 {
    if next_realm(from) != Some(Realm::Solidify) {
        return 0.0;
    }

    match blood_valley {
        Some(true) => BLOOD_VALLEY_BREAKTHROUGH_SUCCESS_BONUS,
        Some(false) => SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS,
        None => 0.0,
    }
}

pub(crate) fn breakthrough_actor_account_id(
    life_record: Option<&LifeRecord>,
    is_npc: bool,
) -> Result<QiAccountId, BreakthroughLedgerError> {
    let id = life_record
        .and_then(|life_record| {
            let id = life_record.character_id.trim();
            (!id.is_empty()).then(|| life_record.character_id.clone())
        })
        .ok_or(BreakthroughLedgerError::MissingStableActorId { is_npc })?;
    if is_npc {
        Ok(QiAccountId::npc(id))
    } else {
        Ok(QiAccountId::player(id))
    }
}

pub(crate) fn credit_active_breakthrough_cost(
    account: &mut WorldQiAccount,
    zone_name: &str,
    from: QiAccountId,
    amount: f64,
) -> Result<(), BreakthroughLedgerError> {
    if amount == 0.0 {
        return Ok(());
    }
    let to = QiAccountId::zone(zone_name.to_string());
    let transfer = QiTransfer::new(from, to.clone(), amount, QiTransferReason::Breakthrough)?;
    if !account.has_account(&to) {
        account.set_balance(to.clone(), 0.0)?;
    }
    let zone_balance = account.balance(&to);
    account.set_balance(to, zone_balance + amount)?;
    account.push_transfer_audit(transfer);
    Ok(())
}

#[derive(SystemParam)]
pub(crate) struct BreakthroughResources<'w> {
    zones: Option<Res<'w, ZoneRegistry>>,
    spirit_eyes: Option<ResMut<'w, SpiritEyeRegistry>>,
    pending_narrations: Option<ResMut<'w, PendingGameplayNarrations>>,
    spirit_eye_used_events: Option<ResMut<'w, Events<SpiritEyeUsedForBreakthroughEvent>>>,
    skill_xp_events: Option<ResMut<'w, Events<SkillXpGain>>>,
    qi_account: Option<ResMut<'w, WorldQiAccount>>,
}

#[allow(clippy::too_many_arguments)] // Bevy system signature; one Query/EventWriter per concern.
pub fn breakthrough_system(
    clock: Res<CultivationClock>,
    mut requests: EventReader<BreakthroughRequest>,
    mut outcomes: EventWriter<BreakthroughOutcome>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut players: Query<(
        &mut Cultivation,
        &mut MeridianSystem,
        &mut LifeRecord,
        Option<&NpcMarker>,
    )>,
    mut status_effects_q: Query<&mut StatusEffects>,
    positions: Query<&Position>,
    usernames: Query<&Username>,
    current_dimensions: Query<&CurrentDimension>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut resources: BreakthroughResources,
) {
    let mut roll = XorshiftRoll(0x9e3779b97f4a7c15);
    let now = clock.tick;
    for req in requests.read() {
        let Ok((mut cultivation, mut meridians, mut life, npc_marker)) =
            players.get_mut(req.entity)
        else {
            continue;
        };
        let from = cultivation.realm;
        if let Some(target) = next_realm(from) {
            life.push(BiographyEntry::BreakthroughStarted {
                realm_target: target,
                tick: now,
            });
        }
        let character_id = life.character_id.clone();
        let username = usernames.get(req.entity).ok().map(|name| name.0.clone());

        // plan §3.1：material_bonus = req.material_bonus（手动传入，默认 0）
        //   ⊕ 服用突破辅助丹药挂在 StatusEffects 的 BreakthroughBoost buff 聚合值。
        //   最终 clamp 由 compute_success_rate 内部处理。
        let buff_bonus = status_effects_q
            .get(req.entity)
            .map(|se| sum_breakthrough_boost(se) as f64)
            .unwrap_or(0.0);
        let material_bonus = req.material_bonus + buff_bonus;

        let position_context = positions.get(req.entity).ok().map(|position| {
            let dimension = current_dimensions
                .get(req.entity)
                .map(|current| current.0)
                .unwrap_or_default();
            (position, dimension)
        });
        let zone_snapshot: Option<(String, f64)> =
            position_context.and_then(|(position, dimension)| {
                resources.zones.as_deref().and_then(|registry| {
                    registry
                        .find_zone(dimension, position.get())
                        .map(|zone| (zone.name.clone(), zone.spirit_qi))
                })
            });
        let spirit_eye_snapshot: Option<(SpiritEyeId, Option<String>, bool)> = position_context
            .and_then(|(position, dimension)| {
                resources
                    .spirit_eyes
                    .as_deref()
                    .and_then(|registry| registry.eye_at(dimension, position.get()))
                    .map(|eye| (eye.id.clone(), eye.zone_name.clone(), eye.blood_valley))
            });
        let env_bonus = spirit_eye_env_bonus_for(
            from,
            spirit_eye_snapshot
                .as_ref()
                .map(|(_, _, blood_valley)| *blood_valley),
        );
        let season = breakthrough_season(position_context, resources.zones.as_deref(), now);

        let zone_error = position_context.and_then(|(position, dimension)| {
            breakthrough_environment_error(
                position,
                dimension,
                resources.zones.as_deref(),
                resources.spirit_eyes.as_deref(),
                from,
            )
        });
        let ledger_error = if zone_error.is_none()
            && (resources.qi_account.is_none() || zone_snapshot.is_none())
            && breakthrough_precondition_error(&cultivation, &meridians).is_none()
        {
            Some(BreakthroughError::LedgerUnavailable)
        } else {
            None
        };

        let res = zone_error
            .or_else(|| breakthrough_precondition_error(&cultivation, &meridians))
            .or(ledger_error)
            .map_or_else(
                || {
                    let actor_account = breakthrough_actor_account_id(
                        Some(&life),
                        npc_marker.is_some(),
                    )
                    .map_err(|error| {
                        tracing::warn!(
                            "[bong][cultivation] breakthrough ledger actor id unavailable entity={:?} error={:?}",
                            req.entity,
                            error
                        );
                        BreakthroughError::LedgerUnavailable
                    });
                    let Ok(actor_account) = actor_account else {
                        return Err(BreakthroughError::LedgerUnavailable);
                    };
                    let cultivation_before = cultivation.clone();
                    let meridians_before = meridians.clone();
                    let before_qi = cultivation.qi_current.max(0.0);
                    let result = try_breakthrough_with_env_season_bonus(
                        &mut cultivation,
                        &mut meridians,
                        material_bonus,
                        env_bonus,
                        Some(season),
                        &mut roll,
                    );
                    let used_qi = (before_qi - cultivation.qi_current.max(0.0)).max(0.0);
                    if let (Some(account), Some((zone_name, _zone_qi))) =
                        (resources.qi_account.as_deref_mut(), zone_snapshot.as_ref())
                    {
                        if let Err(error) = credit_active_breakthrough_cost(
                            account,
                            zone_name.as_str(),
                            actor_account,
                            used_qi,
                        ) {
                            tracing::warn!(
                                "[bong][cultivation] breakthrough ledger credit failed entity={:?} zone={} amount={} error={:?}",
                                req.entity,
                                zone_name,
                                used_qi,
                                error
                            );
                            *cultivation = cultivation_before;
                            *meridians = meridians_before;
                            return Err(BreakthroughError::LedgerUnavailable);
                        }
                    }
                    result
                },
                Err,
            );

        match &res {
            Ok(success) => {
                life.push(BiographyEntry::BreakthroughSucceeded {
                    realm: success.to,
                    tick: now,
                });
                // plan-skill-v1 §4 境界软挂钩：突破到新境界 → 三个 MVP skill 的 cap 全部上调。
                // Client / agent 订阅 SkillCapChanged 做 narration / inspect 面板 effective_lv 展示。
                let new_cap = skill_cap_for_realm(success.to);
                for skill in SkillId::ALL {
                    skill_cap_events.send(SkillCapChanged {
                        char_entity: req.entity,
                        skill,
                        new_cap,
                    });
                }
                if let Some(skill_xp_events) = resources.skill_xp_events.as_deref_mut() {
                    skill_xp_events.send(SkillXpGain {
                        char_entity: req.entity,
                        skill: SkillId::Cultivation,
                        amount: 3,
                        source: XpGainSource::Action {
                            plan_id: "cultivation",
                            action: "breakthrough_success",
                        },
                    });
                }
                if from == Realm::Condense && success.to == Realm::Solidify {
                    if let Some((eye_id, zone_name, _blood_valley)) = spirit_eye_snapshot.as_ref() {
                        if let Some(payload) =
                            resources.spirit_eyes.as_deref_mut().and_then(|registry| {
                                registry.record_breakthrough_use_by_id(
                                    eye_id,
                                    character_id.as_str(),
                                    from,
                                    success.to,
                                    now,
                                )
                            })
                        {
                            life.push(BiographyEntry::SpiritEyeBreakthrough {
                                eye_id: payload.eye_id.clone(),
                                zone: zone_name.clone(),
                                tick: now,
                            });
                            if let Some(spirit_eye_used_events) =
                                resources.spirit_eye_used_events.as_deref_mut()
                            {
                                spirit_eye_used_events
                                    .send(SpiritEyeUsedForBreakthroughEvent { payload });
                            }
                            if let Some(narrations) = resources.pending_narrations.as_deref_mut() {
                                narrations.push_broadcast(
                                    "某处灵机结作一线，旋又归于沉寂。",
                                    NarrationStyle::Narration,
                                );
                            }
                        }
                    }
                }
                // plan-particle-system-v1 §4.4：突破成功发 breakthrough_pillar 光柱。
                if let Ok(pos) = positions.get(req.entity) {
                    let p = pos.get();
                    vfx_events.send(gameplay_vfx::spawn_request(
                        gameplay_vfx::BREAKTHROUGH_PILLAR,
                        p,
                        None,
                        "#FFE8A0",
                        1.0,
                        12,
                        60,
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
                if let Ok(pos) = positions.get(req.entity) {
                    let p = pos.get();
                    vfx_events.send(gameplay_vfx::spawn_request(
                        gameplay_vfx::BREAKTHROUGH_FAIL,
                        p,
                        None,
                        "#FF3344",
                        (*severity as f32).clamp(0.35, 1.0),
                        (8.0 + *severity as f32 * 16.0).round() as u32,
                        60,
                    ));
                }
            }
            Err(_) => {}
        }

        if let Err(error) = &res {
            if let (Some(narrations), Some(username)) = (
                resources.pending_narrations.as_deref_mut(),
                username.as_deref(),
            ) {
                narrations.push_player(
                    username,
                    breakthrough_error_message(error),
                    NarrationStyle::SystemWarning,
                );
            }
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

        // 不论成败，一次性消费 BreakthroughBoost buff（plan §3.1：辅助丹药为突破"仪式"消耗）。
        // bughunt r4-P2#7：同步清除渡劫丹来源的 DamageReduction(u64::MAX)，防止永久减伤泄漏。
        if let Ok(mut se) = status_effects_q.get_mut(req.entity) {
            clear_breakthrough_boost(&mut se);
            clear_du_jie_dan_damage_reduction(&mut se);
        }

        outcomes.send(BreakthroughOutcome {
            entity: req.entity,
            from,
            result: res,
        });
    }
}

#[allow(clippy::type_complexity)]
pub fn rapid_breakthrough_karma_mark_system(
    clock: Res<CultivationClock>,
    mut outcomes: EventReader<BreakthroughOutcome>,
    mut weights: Option<ResMut<KarmaWeightStore>>,
    players: Query<(
        &LifeRecord,
        Option<&Username>,
        &Position,
        Option<&CurrentDimension>,
    )>,
    zones: Option<Res<ZoneRegistry>>,
) {
    let Some(weights) = weights.as_deref_mut() else {
        return;
    };
    let now = clock.tick;

    for outcome in outcomes.read() {
        if outcome.result.is_err() {
            continue;
        }
        let Ok((life_record, username, position, current_dimension)) = players.get(outcome.entity)
        else {
            continue;
        };
        if !has_rapid_breakthrough_karma_trigger(life_record, now) {
            continue;
        }

        let dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let position_vec = position.get();
        let zone_name = zones.as_deref().and_then(|registry| {
            registry
                .find_zone(dimension, position_vec)
                .map(|zone| zone.name.clone())
        });
        let player_id = username
            .map(|name| name.0.clone())
            .unwrap_or_else(|| life_record.character_id.clone());

        weights.mark_player(
            player_id,
            zone_name,
            block_pos_from_position(position),
            RAPID_BREAKTHROUGH_KARMA_WEIGHT_DELTA,
            now,
        );
    }
}

fn has_rapid_breakthrough_karma_trigger(life_record: &LifeRecord, now: u64) -> bool {
    life_record
        .biography
        .iter()
        .filter(|entry| matches_recent_breakthrough_success(entry, now))
        .take(2)
        .count()
        >= 2
}

fn matches_recent_breakthrough_success(entry: &BiographyEntry, now: u64) -> bool {
    matches!(
        entry,
        BiographyEntry::BreakthroughSucceeded { tick, .. }
            if *tick <= now && now - *tick <= RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS
    )
}

fn block_pos_from_position(position: &Position) -> BlockPos {
    let p = position.get();
    BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;
    use crate::npc::spawn::NpcMarker;
    use crate::qi_physics::{QiAccountId, QiTransferReason, WorldQiAccount};
    use crate::schema::common::NarrationScope;
    use crate::schema::vfx_event::VfxEventPayloadV1;
    use crate::world::karma::KarmaWeightStore;
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, Events, Update, Username};

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
        open_regular(&mut m, 3);
        (c, m)
    }

    fn open_regular(meridians: &mut MeridianSystem, count: usize) {
        for id in MeridianId::REGULAR.iter().take(count) {
            meridians.get_mut(*id).opened = true;
        }
    }

    fn open_extraordinary(meridians: &mut MeridianSystem, count: usize) {
        for id in MeridianId::EXTRAORDINARY.iter().take(count) {
            meridians.get_mut(*id).opened = true;
        }
    }

    #[test]
    fn breakthrough_actor_account_id_uses_stable_life_record_id() {
        let player_life = LifeRecord::new("player_a");
        let npc_life = LifeRecord::new("npc_a");

        assert_eq!(
            breakthrough_actor_account_id(Some(&player_life), false)
                .expect("player life record with character_id should produce an account id"),
            QiAccountId::player("player_a"),
            "player breakthrough ledger id must come from stable LifeRecord.character_id"
        );
        assert_eq!(
            breakthrough_actor_account_id(Some(&npc_life), true)
                .expect("npc life record with character_id should produce an account id"),
            QiAccountId::npc("npc_a"),
            "npc breakthrough ledger id must come from stable LifeRecord.character_id"
        );
    }

    #[test]
    fn breakthrough_actor_account_id_rejects_missing_or_blank_id() {
        let blank_life = LifeRecord::new("   ");

        assert!(
            matches!(
                breakthrough_actor_account_id(None, true),
                Err(BreakthroughLedgerError::MissingStableActorId { is_npc: true })
            ),
            "npc breakthrough ledger id must reject missing LifeRecord instead of falling back to unstable Entity ids"
        );
        assert!(
            matches!(
                breakthrough_actor_account_id(Some(&blank_life), false),
                Err(BreakthroughLedgerError::MissingStableActorId { is_npc: false })
            ),
            "player breakthrough ledger id must reject blank LifeRecord.character_id"
        );
    }

    #[test]
    fn credit_active_breakthrough_cost_handles_boundaries() {
        let mut ledger = WorldQiAccount::default();
        let from = QiAccountId::player("player_a");

        credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), 0.0)
            .expect("zero breakthrough cost should be a no-op");
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            0.0,
            "zero breakthrough cost should not create zone balance"
        );
        assert!(
            ledger.transfers().is_empty(),
            "zero breakthrough cost should not append a transfer audit"
        );

        let err = credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), -1.0)
            .expect_err("negative breakthrough cost must be rejected");
        assert!(
            matches!(
                err,
                BreakthroughLedgerError::QiPhysics(QiPhysicsError::InvalidAmount {
                    field: "transfer.amount",
                    ..
                })
            ),
            "negative breakthrough cost should surface the QiPhysics invalid amount error; got {err:?}"
        );

        credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), 8.0)
            .expect("positive breakthrough cost should credit the zone ledger");
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            8.0,
            "first positive breakthrough cost should create the zone account and credit the spent qi"
        );
        let transfer = ledger
            .transfers()
            .last()
            .expect("positive breakthrough cost should append one transfer audit");
        assert_eq!(
            transfer.from, from,
            "breakthrough audit transfer must preserve the stable actor account as source"
        );
        assert_eq!(
            transfer.to,
            QiAccountId::zone("spawn"),
            "breakthrough audit transfer must target the resolved zone account"
        );
        assert_eq!(
            transfer.reason,
            QiTransferReason::Breakthrough,
            "breakthrough audit transfer must use the dedicated reason"
        );
        assert_eq!(
            transfer.amount, 8.0,
            "breakthrough audit transfer amount must equal the spent qi"
        );
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
    fn breakthrough_season_modifier_matches_four_phases() {
        assert_eq!(season_success_modifier(Season::Summer), 1.05);
        assert_eq!(season_success_modifier(Season::Winter), 0.95);
        assert_eq!(season_success_modifier(Season::SummerToWinter), 0.85);
        assert_eq!(season_success_modifier(Season::WinterToSummer), 0.85);
    }

    #[test]
    fn breakthrough_in_xizhuan_phase_has_lower_success_rate() {
        let summer = compute_success_rate_with_env_and_season_bonus(
            Realm::Induce,
            1.0,
            1.0,
            1.0,
            0.0,
            0.0,
            Season::Summer,
        );
        let xizhuan = compute_success_rate_with_env_and_season_bonus(
            Realm::Induce,
            1.0,
            1.0,
            1.0,
            0.0,
            0.0,
            Season::SummerToWinter,
        );

        assert!(xizhuan < summer);
        assert!((xizhuan - 0.765).abs() < 1e-9);
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
    fn breakthrough_error_message_covers_all_error_variants() {
        let cases = [
            (
                BreakthroughError::AtMaxRealm,
                "突破未成：你已抵达当前最高境界。",
            ),
            (
                BreakthroughError::RequiresTribulation,
                "突破未成：通灵至化虚必须先走渡虚劫。",
            ),
            (
                BreakthroughError::NotEnoughMeridians { need: 16, have: 15 },
                "突破未成：需先打通 16 条经脉（当前 15）。",
            ),
            (
                BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 8 },
                "突破未成：需先打通 12 条正经（当前 8）。",
            ),
            (
                BreakthroughError::NotEnoughExtraordinaryMeridians { need: 4, have: 3 },
                "突破未成：需先打通 4 条奇经（当前 3）。",
            ),
            (
                BreakthroughError::NotEnoughQi {
                    need: 100.0,
                    have: 42.5,
                },
                "突破未成：真元不足（需 100.0，当前 42.5）。",
            ),
            (
                BreakthroughError::ZoneTooWeak {
                    need: 0.8,
                    have: 0.4,
                },
                "突破未成：此地灵气不足（需 0.80，当前 0.40）。",
            ),
            (
                BreakthroughError::EnvInsufficient {
                    need: 0.7,
                    have: 0.3,
                    in_spirit_eye: true,
                },
                "突破未成：灵眼扰动未稳（需 0.70，当前 0.30）。",
            ),
            (
                BreakthroughError::EnvInsufficient {
                    need: 0.7,
                    have: 0.3,
                    in_spirit_eye: false,
                },
                "突破未成：固元须在灵气浓处或灵眼内（需 0.70，当前 0.30）。",
            ),
            (
                BreakthroughError::LedgerUnavailable,
                "突破未成：真元账本未就绪，仪式暂缓。",
            ),
            (
                BreakthroughError::RolledFailure { severity: 0.75 },
                "突破失败：气机反噬，伤势强度 0.75。",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(
                breakthrough_error_message(&error),
                expected,
                "expected stable breakthrough error text for {error:?}"
            );
        }
    }

    #[test]
    fn breakthrough_system_pushes_system_warning_on_precondition_error() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_systems(Update, breakthrough_system);
        let player = app
            .world_mut()
            .spawn((
                Cultivation::default(),
                MeridianSystem::default(),
                LifeRecord::default(),
                Username("Azure".to_string()),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 1);
        assert_eq!(narrations[0].scope, NarrationScope::Player);
        assert_eq!(narrations[0].target.as_deref(), Some("Azure"));
        assert_eq!(narrations[0].style, NarrationStyle::SystemWarning);
        assert!(
            narrations[0].text.contains("突破未成"),
            "expected system warning to include breakthrough failure reason, actual text={}",
            narrations[0].text
        );
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
        open_regular(&mut m, 3);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert!(matches!(err, BreakthroughError::NotEnoughQi { .. }));
        assert!((c.pending_material_bonus - 0.12).abs() < 1e-9);
    }

    #[test]
    fn induce_requires_three_regular_meridians_not_extraordinary_padding() {
        let mut c = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_extraordinary(&mut m, 3);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert_eq!(
            err,
            BreakthroughError::NotEnoughRegularMeridians { need: 3, have: 0 }
        );
        assert_eq!(c.realm, Realm::Awaken);
        assert_eq!(c.qi_current, 100.0);
    }

    #[test]
    fn solidify_requires_all_twelve_regular_meridians() {
        let mut c = Cultivation {
            realm: Realm::Condense,
            qi_current: 500.0,
            qi_max: 500.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 10);
        open_extraordinary(&mut m, 6);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert_eq!(
            err,
            BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 10 }
        );
        assert_eq!(c.realm, Realm::Condense);
        assert_eq!(c.qi_current, 500.0);
    }

    #[test]
    fn spirit_rejects_before_structure_when_total_meridians_are_too_few() {
        let mut c = Cultivation {
            realm: Realm::Solidify,
            qi_current: 1000.0,
            qi_max: 1000.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 12);
        open_extraordinary(&mut m, 3);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert_eq!(
            err,
            BreakthroughError::NotEnoughMeridians { need: 16, have: 15 }
        );
        assert_eq!(c.realm, Realm::Solidify);
        assert_eq!(c.qi_current, 1000.0);
    }

    #[test]
    fn spirit_rejects_extraordinary_padding_without_regular_foundation() {
        let mut c = Cultivation {
            realm: Realm::Solidify,
            qi_current: 1000.0,
            qi_max: 1000.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 8);
        open_extraordinary(&mut m, 8);

        let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap_err();

        assert_eq!(
            err,
            BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 8 }
        );
        assert_eq!(c.realm, Realm::Solidify);
        assert_eq!(c.qi_current, 1000.0);
    }

    #[test]
    fn spirit_allows_twelve_regular_and_four_extraordinary_meridians() {
        let mut c = Cultivation {
            realm: Realm::Solidify,
            qi_current: 1000.0,
            qi_max: 1000.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 12);
        open_extraordinary(&mut m, 4);

        let out = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

        assert_eq!(out.to, Realm::Spirit);
        assert_eq!(c.realm, Realm::Spirit);
    }

    #[test]
    fn breakthrough_rejects_when_zone_qi_too_weak() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.0;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (mut cultivation, meridians) = setup_for_induce();
        cultivation.pending_material_bonus = 0.12;
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::default(),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let outcomes = app.world().resource::<Events<BreakthroughOutcome>>();
        let outcome = outcomes.iter_current_update_events().next().unwrap();
        assert!(matches!(
            outcome.result,
            Err(BreakthroughError::ZoneTooWeak { .. })
        ));
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.qi_current, 100.0);
        assert!((cultivation.pending_material_bonus - 0.12).abs() < 1e-9);
    }

    #[test]
    fn breakthrough_system_without_ledger_does_not_consume_qi() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (cultivation, meridians) = setup_for_induce();
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("player_a"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let outcomes = app.world().resource::<Events<BreakthroughOutcome>>();
        let outcome = outcomes.iter_current_update_events().next().unwrap();
        assert!(matches!(
            outcome.result,
            Err(BreakthroughError::LedgerUnavailable)
        ));
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.realm, Realm::Awaken);
        assert_eq!(cultivation.qi_current, 100.0);
    }

    #[test]
    fn breakthrough_success_credits_cost_to_zone_ledger() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (cultivation, meridians) = setup_for_induce();
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("player_a"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(
            cultivation.realm,
            Realm::Induce,
            "successful breakthrough should advance the player realm"
        );
        assert_eq!(
            cultivation.qi_current, 92.0,
            "successful breakthrough should spend exactly 8 qi from the player"
        );
        let ledger = app.world().resource::<WorldQiAccount>();
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            8.0,
            "successful breakthrough should credit the spent 8 qi to the zone ledger"
        );
        let transfer = ledger
            .transfers()
            .last()
            .expect("breakthrough should leave a QiTransfer audit");
        assert_eq!(
            transfer.reason,
            QiTransferReason::Breakthrough,
            "successful breakthrough audit should use the dedicated reason"
        );
        assert_eq!(
            transfer.from,
            QiAccountId::player("player_a"),
            "successful breakthrough audit should use stable player id as source"
        );
        assert_eq!(
            transfer.to,
            QiAccountId::zone("spawn"),
            "successful breakthrough audit should target the resolved zone"
        );
        assert_eq!(
            transfer.amount, 8.0,
            "successful breakthrough audit amount should match spent qi"
        );
    }

    #[test]
    fn breakthrough_failure_also_credits_cost_to_zone_ledger() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (mut cultivation, meridians) = setup_for_induce();
        cultivation.composure = 0.0;
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("player_a"),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(
            cultivation.realm,
            Realm::Awaken,
            "failed breakthrough should keep the player in the original realm"
        );
        assert_eq!(
            cultivation.qi_current, 92.0,
            "failed breakthrough should still spend exactly 8 qi"
        );
        let ledger = app.world().resource::<WorldQiAccount>();
        assert_eq!(
            ledger.balance(&QiAccountId::zone("spawn")),
            8.0,
            "failed breakthrough should still credit spent qi to the zone ledger"
        );
        let transfer = ledger
            .transfers()
            .last()
            .expect("failed breakthrough should leave a QiTransfer audit");
        assert_eq!(
            transfer.reason,
            QiTransferReason::Breakthrough,
            "failed breakthrough audit should use the dedicated reason"
        );
        assert_eq!(
            transfer.amount, 8.0,
            "failed breakthrough audit amount should match spent qi"
        );
    }

    #[test]
    fn npc_breakthrough_emits_vfx() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (cultivation, meridians) = setup_for_induce();
        let npc = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::new("npc_42v0"),
                Position::new([8.0, 66.0, 8.0]),
                NpcMarker,
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: npc,
            material_bonus: 0.0,
        });
        app.update();

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let ids = vfx_events
            .iter_current_update_events()
            .filter_map(|event| match &event.payload {
                VfxEventPayloadV1::SpawnParticle { event_id, .. } => Some(event_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(ids.contains(&"bong:breakthrough_pillar"));
    }

    #[test]
    fn breakthrough_fail_emits_vfx() {
        let mut app = App::new();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(CultivationClock { tick: 10 });
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<BreakthroughRequest>();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<SkillXpGain>();
        app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
        app.add_systems(Update, breakthrough_system);

        let (mut cultivation, meridians) = setup_for_induce();
        cultivation.composure = 0.0;
        let player = app
            .world_mut()
            .spawn((
                cultivation,
                meridians,
                LifeRecord::default(),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughRequest {
            entity: player,
            material_bonus: 0.0,
        });
        app.update();

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted = events
            .iter_current_update_events()
            .find(|event| {
                matches!(
                    &event.payload,
                    VfxEventPayloadV1::SpawnParticle { event_id, .. }
                        if event_id == gameplay_vfx::BREAKTHROUGH_FAIL
                )
            })
            .expect("rolled breakthrough failure should emit breakthrough_fail vfx");
        match &emitted.payload {
            VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(event_id, gameplay_vfx::BREAKTHROUGH_FAIL);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn guyuan_requires_high_qi_or_spirit_eye() {
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.6;
        let position = Position::new([8.0, 66.0, 8.0]);

        let err = breakthrough_environment_error(
            &position,
            DimensionKind::Overworld,
            Some(&zones),
            None,
            Realm::Condense,
        )
        .expect("low qi outside spirit eye should reject guyuan");

        assert_eq!(
            err,
            BreakthroughError::EnvInsufficient {
                need: MIN_ZONE_QI_TO_GUYUAN,
                have: 0.6,
                in_spirit_eye: false,
            }
        );
    }

    #[test]
    fn spirit_eye_bonus_raises_guyuan_success_rate() {
        let base = compute_success_rate_with_env_bonus(Realm::Solidify, 1.0, 1.0, 1.0, 0.0, 0.0);
        let boosted = compute_success_rate_with_env_bonus(
            Realm::Solidify,
            1.0,
            1.0,
            1.0,
            0.0,
            SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS,
        );

        assert!(boosted > base);
    }

    #[test]
    fn spirit_eye_bonus_is_gated_to_guyuan_breakthrough() {
        assert_eq!(
            spirit_eye_env_bonus_for(Realm::Condense, Some(false)),
            SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS
        );
        assert_eq!(
            spirit_eye_env_bonus_for(Realm::Condense, Some(true)),
            BLOOD_VALLEY_BREAKTHROUGH_SUCCESS_BONUS
        );
        assert_eq!(spirit_eye_env_bonus_for(Realm::Solidify, Some(false)), 0.0);
        assert_eq!(spirit_eye_env_bonus_for(Realm::Induce, Some(false)), 0.0);
        assert_eq!(spirit_eye_env_bonus_for(Realm::Condense, None), 0.0);
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

    fn setup_rapid_breakthrough_karma_app(now: u64) -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: now });
        app.insert_resource(KarmaWeightStore::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<BreakthroughOutcome>();
        app.add_systems(Update, rapid_breakthrough_karma_mark_system);
        app
    }

    fn breakthrough_success_outcome(entity: Entity) -> BreakthroughOutcome {
        BreakthroughOutcome {
            entity,
            from: Realm::Awaken,
            result: Ok(BreakthroughSuccess {
                to: Realm::Induce,
                success_rate: 1.0,
                used_qi: 0.0,
            }),
        }
    }

    #[test]
    fn rapid_breakthrough_success_marks_hidden_karma_weight() {
        let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
        let mut app = setup_rapid_breakthrough_karma_app(now);
        let mut life = LifeRecord::new("offline:Azure");
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Awaken,
            tick: now - 100,
        });
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Induce,
            tick: now,
        });
        let entity = app
            .world_mut()
            .spawn((
                life,
                Username("Azure".to_string()),
                Position::new([8.8, 66.2, 8.1]),
            ))
            .id();

        app.world_mut()
            .send_event(breakthrough_success_outcome(entity));
        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        let entry = weights
            .entry_for_player("Azure")
            .expect("rapid breakthroughs should mark hidden karma weight");
        assert_eq!(entry.weight, RAPID_BREAKTHROUGH_KARMA_WEIGHT_DELTA);
        assert_eq!(entry.zone.as_deref(), Some("spawn"));
        assert_eq!(entry.last_position, [8, 66, 8]);
        assert_eq!(entry.last_tick, now);
    }

    #[test]
    fn old_breakthrough_success_outside_window_does_not_mark_karma() {
        let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
        let mut app = setup_rapid_breakthrough_karma_app(now);
        let mut life = LifeRecord::new("offline:Azure");
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Awaken,
            tick: now - RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS - 1,
        });
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Induce,
            tick: now,
        });
        let entity = app
            .world_mut()
            .spawn((
                life,
                Username("Azure".to_string()),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut()
            .send_event(breakthrough_success_outcome(entity));
        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        assert!(weights.entry_for_player("Azure").is_none());
    }

    #[test]
    fn failed_breakthrough_outcome_does_not_mark_karma() {
        let now = RAPID_BREAKTHROUGH_KARMA_WINDOW_TICKS + 100;
        let mut app = setup_rapid_breakthrough_karma_app(now);
        let mut life = LifeRecord::new("offline:Azure");
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Awaken,
            tick: now - 100,
        });
        life.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Induce,
            tick: now,
        });
        let entity = app
            .world_mut()
            .spawn((
                life,
                Username("Azure".to_string()),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();

        app.world_mut().send_event(BreakthroughOutcome {
            entity,
            from: Realm::Awaken,
            result: Err(BreakthroughError::RolledFailure { severity: 0.2 }),
        });
        app.update();

        let weights = app.world().resource::<KarmaWeightStore>();
        assert!(weights.entry_for_player("Azure").is_none());
    }

    // ───────────────────────────────────────────────────────────────────────
    // qi_max_frozen cap: 突破失败不能永久废人
    // ───────────────────────────────────────────────────────────────────────

    /// 单次失败：qi_max_frozen 精确加上 severity * 10.0，且不超过 qi_max * 0.5。
    #[test]
    fn single_breakthrough_failure_freezes_qi_within_cap() {
        let (mut c, mut m) = setup_for_induce();
        // 强制失败：roll > base_success_rate(Induce)=0.90
        let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0));

        // severity = (1.0 - success_rate).clamp(0.1, 0.9)
        // success_rate = base × composure × integrity × completeness = 0.90 × 1.0 × 1.0 × 1.0 = 0.90
        // severity = 0.10, freeze_add = 0.10 * 10.0 = 1.0
        // cap = qi_max(100.0) * 0.5 = 50.0 → 1.0 < 50.0, no clamping
        let frozen = c
            .qi_max_frozen
            .expect("qi_max_frozen should be Some after failure");
        assert!(
            (frozen - 1.0).abs() < 1e-9,
            "期望 qi_max_frozen = 1.0（severity=0.10 × 10.0），实际 = {frozen}"
        );
        let effective = c.qi_max - frozen;
        assert!(
            effective > 0.0,
            "期望有效 qi_max > 0 以防玩家废人，实际 effective_qi_max = {effective}"
        );
    }

    /// 连续多次失败，qi_max_frozen 不超过 qi_max * 0.5 的硬上限。
    #[test]
    fn repeated_breakthrough_failures_frozen_capped_at_half_qi_max() {
        let qi_max = 100.0;
        let mut c = Cultivation {
            realm: Realm::Awaken,
            qi_current: qi_max,
            qi_max,
            composure: 0.0, // composure=0 → success_rate 极低 → severity 接近 0.9
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 3);

        // 失败 20 次：无 cap 时 severity≈0.9, freeze_add≈9.0, 2 次即超 qi_max=100 × 0.5=50
        for _ in 0..20 {
            // qi_current 须 ≥ breakthrough_qi_cost(Induce)=8.0，补满避免 NotEnoughQi 前置错误
            c.qi_current = qi_max;
            let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0));
        }

        let frozen = c
            .qi_max_frozen
            .expect("qi_max_frozen should be Some after repeated failures");
        let cap = qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO;

        assert!(
            frozen <= cap,
            "期望 qi_max_frozen ≤ {cap}（qi_max×0.5，防废人），实际 qi_max_frozen = {frozen}"
        );

        let effective_qi_max = c.qi_max - frozen;
        assert!(
            effective_qi_max > 0.0,
            "期望有效真元上限 > 0（玩家不应被永久废），实际 effective_qi_max = {effective_qi_max}"
        );
    }

    /// 验证 cap 边界：pre-existing frozen 接近 cap 时，再叠一次不会超过 cap。
    #[test]
    fn breakthrough_failure_does_not_exceed_cap_when_already_near_cap() {
        let qi_max = 100.0;
        // 预填到接近 cap（40/100 = 0.4 × qi_max，距 0.5×qi_max=50 还差 10）
        let mut c = Cultivation {
            realm: Realm::Awaken,
            qi_current: qi_max,
            qi_max,
            composure: 0.0, // severity 接近 0.9 → freeze_add 接近 9.0，足够触碰 cap
            qi_max_frozen: Some(40.0),
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        open_regular(&mut m, 3);

        let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0));

        let frozen = c
            .qi_max_frozen
            .expect("qi_max_frozen should be Some after failure");
        let cap = qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO;

        assert!(
            frozen <= cap + 1e-9,
            "期望 qi_max_frozen ≤ {cap}（cap = qi_max×0.5），实际 qi_max_frozen = {frozen}（超 cap）"
        );
        // 有效上限仍须 > 0
        let effective = c.qi_max - frozen;
        assert!(
            effective >= qi_max * (1.0 - BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO) - 1e-9,
            "期望有效 qi_max ≥ qi_max×0.5={cap}，实际 = {effective}"
        );
    }

    /// 成功突破不应修改 qi_max_frozen。
    #[test]
    fn successful_breakthrough_does_not_change_qi_max_frozen() {
        let (mut c, mut m) = setup_for_induce();
        c.qi_max_frozen = Some(5.0); // 预存冻结，验证成功路径不碰它

        let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(0.0)).unwrap();

        assert_eq!(
            c.qi_max_frozen,
            Some(5.0),
            "期望成功突破不修改 qi_max_frozen（仍为 5.0），实际 = {:?}",
            c.qi_max_frozen
        );
    }
}
