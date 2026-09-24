//! QiRegenTick + ZoneQiDrainTick（plan §2 QiRegenTick / ZoneQiDrainTick）。
//!
//! 两者合并到一个 system 里执行以天然保证零和：玩家每 tick 吸纳的 qi
//! 必然等量从 zone.spirit_qi 扣除（按 qi_physics 底盘换算系数换算）。符合
//! worldview §一"灵气零和守恒"公理。
//!
//! P1 简化：无「静坐/行动」区分，全部按被动小系数回；静坐/打坐在 P1 末
//! 加客户端指令时再接入。

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, Despawned, Entity, Events, Position, Query, Res, ResMut, Resource, With, Without,
};

use crate::combat::baomai_v3::state::BloodBurnActive;
use crate::combat::components::{DerivedAttrs, StatusEffects};
use crate::combat::events::StatusEffectKind;
use crate::combat::woliu_v2::state::TurbulenceExposure;
use crate::combat::CombatClock;
use crate::fauna::mundane::MundaneFaunaSpecies;
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::npc::scenario::ScenarioNpc;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::{
    constants::QI_NPC_ABSORB_FLOOR, regen_from_zone, QiAccountId, QiTransferReason, WorldQiAccount,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

use super::color::{
    CultivationSessionPracticeEvent, CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE,
};
use super::components::{
    ActorQiIdentity, ActorQiKind, ColorKind, Cultivation, MeridianSystem, QiColor, Realm,
};
use super::life_record::LifeRecord;
use super::lifespan::LifespanComponent;
use super::tribulation::JueBiAftershockDebuff;

/// 全局 tick 计数器 — 用于标记 last_qi_zero_at 等时间戳。
#[derive(Debug, Default, Resource)]
pub struct CultivationClock {
    pub tick: u64,
}

/// 修炼 session 实际引气 tick 累计器。只有发生真实 qi gain 的 tick 才计入，
/// 避免玩家在全局分钟边界短暂在线也拿到整分钟 PracticeLog 进料。
#[derive(Debug, Default, Resource)]
pub struct CultivationSessionPracticeAccumulator {
    ticks_by_entity: HashMap<Entity, u64>,
    last_gain_tick_by_entity: HashMap<Entity, u64>,
}

impl CultivationSessionPracticeAccumulator {
    pub const AUDIO_RECENT_WINDOW_TICKS: u64 = 5 * 20;

    pub fn is_recently_practicing(&self, entity: Entity, now_tick: u64) -> bool {
        self.last_gain_tick_by_entity
            .get(&entity)
            .is_some_and(|last_tick| {
                now_tick >= *last_tick
                    && now_tick.saturating_sub(*last_tick) <= Self::AUDIO_RECENT_WINDOW_TICKS
            })
    }

    fn note_practice_tick(&mut self, entity: Entity, now_tick: u64) -> u64 {
        self.last_gain_tick_by_entity.insert(entity, now_tick);

        let ticks = self.ticks_by_entity.entry(entity).or_default();
        *ticks = ticks.saturating_add(1);

        let minutes = *ticks / CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE;
        if minutes == 0 {
            return 0;
        }

        *ticks %= CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE;
        minutes
    }

    #[cfg(test)]
    pub fn note_practice_tick_for_tests(&mut self, entity: Entity, now_tick: u64) {
        self.note_practice_tick(entity, now_tick);
    }
}

/// 纯函数：给定 zone 浓度、rate、可用额度（qi_max - qi_current - qi_max_frozen 等）
/// 计算本 tick 的实际 gain 与 zone 浓度变化量（均为非负）。
pub fn compute_regen(zone_qi: f64, rate: f64, avg_integrity: f64, qi_room: f64) -> (f64, f64) {
    regen_from_zone(zone_qi, rate, avg_integrity, qi_room)
}

/// QiRegenTick + ZoneQiDrainTick 合并实现。零和：玩家 qi 增量 = zone 浓度减量 × coef。
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn qi_regen_and_zone_drain_tick(
    mut clock: ResMut<CultivationClock>,
    zone_registry: Option<ResMut<ZoneRegistry>>,
    war_bonus: Option<Res<crate::npc::war::settle::ZoneSpiritBonusStore>>,
    // plan-territory-v1 P1: 霸主 zone qi regen 速率倍率（ZoneDominanceRegenStore）。
    // Option<Res> 防资源缺失 panic（克隆 war_bonus 写法）。守恒安全：只乘 rate。
    dominance_regen: Option<Res<crate::world::territory_perks::ZoneDominanceRegenStore>>,
    mut qi_ledger: Option<ResMut<WorldQiAccount>>,
    mut practice_events: Option<ResMut<Events<CultivationSessionPracticeEvent>>>,
    mut practice_accumulator: Option<ResMut<CultivationSessionPracticeAccumulator>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    combat_clock: Option<Res<CombatClock>>,
    mut players: Query<
        (
            Entity,
            &Position,
            Option<&CurrentDimension>,
            &MeridianSystem,
            &mut Cultivation,
            Option<&QiColor>,
            Option<&LifespanComponent>,
            Option<&StatusEffects>,
            Option<&TurbulenceExposure>,
            Option<&JueBiAftershockDebuff>,
            Option<&DerivedAttrs>,
            Option<&BloodBurnActive>,
            Option<&LifeRecord>,
            Option<&NpcMarker>,
            // plan-mundane-fauna-v1：凡兽无灵——`Without<MundaneFaunaSpecies>` 把凡兽整体排除出
            // 真元吐纳。凡兽经 `npc_runtime_bundle_with_age` 拿到 live Cultivation+MeridianSystem
            // （满足 hunt 猎物契约的 realm 门），但绝不吸/放 zone 灵气：不加此过滤，凡兽会逐 tick
            // 抽 zone.spirit_qi 进 qi_current，死亡/超距回收时（凡兽无 Rat/MimicSpider 归还路径、
            // 裸 insert(Despawned)）100% 蒸发，破守恒（plan §59 qi_physics 锚点 + mundane.rs 模块 doc）。
            // spawn 与 hydrate 两路均挂 MundaneFaunaSpecies（hydrate 走 spawn_mundane_fauna_at），过滤稳。
        ),
        (
            Without<MundaneFaunaSpecies>,
            Without<Despawned>,
            Without<ScenarioNpc>,
        ),
    >,
) {
    clock.tick = clock.tick.wrapping_add(1);
    let effect_tick = combat_clock
        .as_deref()
        .map(|clock| clock.tick)
        .unwrap_or(clock.tick);

    let Some(mut zones) = zone_registry else {
        return;
    };

    for (
        entity,
        pos,
        current_dim,
        meridians,
        mut cultivation,
        qi_color,
        lifespan,
        statuses,
        turbulence,
        juebi_aftershock,
        derived_attrs,
        blood_burn,
        life_record,
        npc_marker,
    ) in players.iter_mut()
    {
        // 通过 pos 找到 zone 的 name（不持可变借用）；entity 缺 CurrentDimension
        // 时按 Overworld 处理（NPC 暂未跨位面）。Player 在 spawn 时一定带
        // CurrentDimension（apply_spawn_defaults / restore_player_dimension）。
        let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Overworld);
        let Some(zone_name) = zones.find_zone(dim, pos.0).map(|z| z.name.clone()) else {
            continue;
        };
        let Some(zone) = zones.find_zone_mut(&zone_name) else {
            continue;
        };
        if zone
            .active_events
            .iter()
            .any(|event| event == EVENT_REALM_COLLAPSE)
        {
            zone.spirit_qi = 0.0;
            continue;
        }
        if zone.spirit_qi <= 0.0 {
            continue;
        }

        let rate = {
            let sum = meridians.sum_rate();
            if sum > 0.0 {
                sum
            } else {
                0.1 // Awaken 期的「基础吸纳」
            }
        };
        let avg_integrity = {
            let total: f64 = meridians.iter().map(|m| m.integrity).sum();
            let n = meridians.iter().count() as f64;
            if n > 0.0 {
                total / n
            } else {
                1.0
            }
        };
        // bughunt r4-P2#5：QiCapPermMinus 永久真元上限折损 debuff 消费侧。
        // attribute_aggregate_tick 已把折损系数写入 DerivedAttrs.qi_max_multiplier；
        // 此处读取后乘入 effective_max，使 qi_room 正确反映折损后的可用容量。
        let qi_max_multiplier = derived_attrs
            .map(|a| a.qi_max_multiplier.clamp(0.01, 1.0))
            .unwrap_or(1.0);
        let effective_max = (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0))
            .max(0.0)
            * qi_max_multiplier;
        let qi_room = (effective_max - cultivation.qi_current).max(0.0);

        let wind_candle_multiplier = if lifespan.is_some_and(LifespanComponent::is_wind_candle)
            || statuses.is_some_and(has_frailty_status)
        {
            frailty_qi_recovery_multiplier_for_realm(cultivation.realm)
        } else {
            1.0
        };
        let humility_multiplier = statuses.map(humility_qi_recovery_multiplier).unwrap_or(1.0);
        let qi_regen_pause_multiplier = statuses.map(qi_regen_pause_multiplier).unwrap_or(1.0);
        let cultivation_accel = statuses
            .map(cultivation_acceleration_multiplier)
            .unwrap_or(1.0);
        // bughunt r4-P2#6：QiRegenBoost 回气提升 buff 消费侧。
        // 叠加所有 remaining_ticks > 0 的 magnitude，上限 3×，接入 rate 乘区。
        let qi_regen_boost = statuses.map(qi_regen_boost_multiplier).unwrap_or(1.0);
        let qi_regen_slowed = statuses.map(qi_regen_slowed_multiplier).unwrap_or(1.0);
        // 虚脱 debuff：qi 回复 ×magnitude（旧 Exhausted.qi_recovery_modifier，恒为 0.5）。
        // 现走 StatusEffects（StatusEffectKind::Exhausted），由标准 status 生命周期管理。
        let exhausted_multiplier = statuses
            .map(exhausted_qi_recovery_multiplier)
            .unwrap_or(1.0)
            .clamp(0.05, 1.0);
        let turbulence_multiplier = turbulence
            .map(|exposure| exposure.absorption_multiplier())
            .unwrap_or(1.0);
        let juebi_aftershock_multiplier = juebi_aftershock
            .filter(|debuff| clock.tick <= debuff.until_tick)
            .map(|debuff| debuff.rhythm_multiplier.clamp(0.0, 1.0))
            .unwrap_or(1.0);
        let scar_circuit_qi_regen_multiplier =
            baomai_scar_qi_regen_multiplier(derived_attrs, blood_burn, effect_tick);
        let war_zone_multiplier = war_bonus
            .as_deref()
            .map(|s| s.multiplier_for(&zone_name))
            .unwrap_or(1.0);
        // plan-territory-v1 P1：霸主 zone regen 速率倍率（守恒安全：只乘 rate）。
        let dominance_regen_multiplier = dominance_regen
            .as_deref()
            .map(|s| s.multiplier_for(&zone_name))
            .unwrap_or(1.0);
        // plan-zone-qi-economy-v1 P2：NPC 只喝地板（QI_NPC_ABSORB_FLOOR）以上的溢出层；
        // 玩家不受此约束（保留 zone.spirit_qi 全量作为吸取输入）。用地板以上的余量
        // 驱动公式而非事后钳位，保证 drain <= zone.spirit_qi - FLOOR，写回天然守地板。
        let npc_absorbable_zone_qi = if npc_marker.is_some() {
            (zone.spirit_qi - QI_NPC_ABSORB_FLOOR).max(0.0)
        } else {
            zone.spirit_qi
        };
        let (gain, _drain) = compute_regen(
            npc_absorbable_zone_qi,
            rate * wind_candle_multiplier
                * humility_multiplier
                * qi_regen_pause_multiplier
                * cultivation_accel
                * qi_regen_boost
                * qi_regen_slowed
                * exhausted_multiplier
                * turbulence_multiplier
                * juebi_aftershock_multiplier
                * scar_circuit_qi_regen_multiplier
                * war_zone_multiplier
                * dominance_regen_multiplier,
            avg_integrity,
            qi_room,
        );
        if gain <= 0.0 {
            continue;
        }

        let Some(qi_ledger) = qi_ledger.as_deref_mut() else {
            continue;
        };
        let Some(life_record) = life_record else {
            continue;
        };
        let actor_kind = if npc_marker.is_some() {
            ActorQiKind::Npc
        } else {
            ActorQiKind::Player
        };
        let Ok(actor) = ActorQiIdentity::from_life_record(life_record, actor_kind) else {
            continue;
        };
        let Ok(outcome) = cultivation.gain_from_zone(
            zone,
            qi_ledger,
            &actor,
            gain,
            QiTransferReason::CultivationRegen,
        ) else {
            continue;
        };
        let actual_gain = outcome.target_credited;
        if actual_gain <= 0.0 {
            continue;
        }
        if clock.tick.is_multiple_of(40) {
            if let Some(events) = vfx_events.as_deref_mut() {
                let origin = pos.get() + valence::prelude::DVec3::new(0.0, 0.9, 0.0);
                let spirit_qi = zone.spirit_qi.clamp(0.0, 1.0) as f32;
                let count = (spirit_qi * 10.0).round().clamp(1.0, 16.0) as u32;
                gameplay_vfx::send_spawn(
                    events,
                    gameplay_vfx::spawn_request(
                        gameplay_vfx::CULTIVATION_ABSORB,
                        origin,
                        None,
                        "#66FFCC",
                        spirit_qi.max(0.2),
                        count,
                        30,
                    ),
                );
            }
        }

        if cultivation.qi_current > 0.0 {
            cultivation.last_qi_zero_at = None;
        }

        if let (Some(events), Some(accumulator)) = (
            practice_events.as_deref_mut(),
            practice_accumulator.as_deref_mut(),
        ) {
            accumulate_cultivation_session_practice_tick(
                accumulator,
                events,
                entity,
                clock.tick,
                qi_color
                    .map(|color| color.main)
                    .unwrap_or(ColorKind::Mellow),
            );
        }
    }
}

fn baomai_scar_qi_regen_multiplier(
    derived_attrs: Option<&DerivedAttrs>,
    blood_burn: Option<&BloodBurnActive>,
    now_tick: u64,
) -> f64 {
    if !blood_burn.is_some_and(|active| active.is_active_at(now_tick)) {
        return 1.0;
    }
    derived_attrs
        .map(|attrs| attrs.qi_regen_multiplier.max(0.0))
        .unwrap_or(1.0)
}

fn cultivation_regen_account_id(
    entity: Entity,
    life_record: Option<&LifeRecord>,
    is_npc: bool,
) -> QiAccountId {
    let id = life_record
        .and_then(|life_record| {
            let id = life_record.character_id.trim();
            (!id.is_empty()).then(|| life_record.character_id.clone())
        })
        .unwrap_or_else(|| format!("entity:{entity:?}"));
    if is_npc {
        QiAccountId::npc(id)
    } else {
        QiAccountId::player(id)
    }
}

pub fn accumulate_cultivation_session_practice_tick(
    accumulator: &mut CultivationSessionPracticeAccumulator,
    events: &mut Events<CultivationSessionPracticeEvent>,
    entity: Entity,
    now_tick: u64,
    active_color: ColorKind,
) -> u64 {
    let minutes = accumulator.note_practice_tick(entity, now_tick);
    if minutes == 0 {
        return 0;
    }

    events.send(CultivationSessionPracticeEvent {
        entity,
        active_color,
        elapsed_ticks: minutes * CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE,
    });
    minutes
}

pub fn prune_cultivation_session_practice_accumulator(
    mut accumulator: ResMut<CultivationSessionPracticeAccumulator>,
    live_cultivators: Query<(), (With<Cultivation>, Without<Despawned>)>,
) {
    accumulator
        .ticks_by_entity
        .retain(|entity, _| live_cultivators.get(*entity).is_ok());
    accumulator
        .last_gain_tick_by_entity
        .retain(|entity, _| live_cultivators.get(*entity).is_ok());
}

fn humility_qi_recovery_multiplier(status_effects: &StatusEffects) -> f64 {
    status_effects
        .active
        .iter()
        .filter(|effect| effect.kind == StatusEffectKind::Humility && effect.remaining_ticks > 0)
        .fold(1.0, |acc, effect| {
            acc * (1.0 - f64::from(effect.magnitude).clamp(0.0, 0.95))
        })
        .clamp(0.05, 1.0)
}

fn qi_regen_pause_multiplier(status_effects: &StatusEffects) -> f64 {
    if status_effects
        .active
        .iter()
        .any(|effect| effect.kind == StatusEffectKind::QiRegenPaused && effect.remaining_ticks > 0)
    {
        0.0
    } else {
        1.0
    }
}

pub fn frailty_qi_recovery_multiplier_for_realm(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken | Realm::Induce => 0.7,
        Realm::Condense => 0.6,
        Realm::Solidify => 0.5,
        Realm::Spirit => 0.4,
        Realm::Void => 0.3,
    }
}

/// plan-cultivation-pacing-v1 P1.2：修炼加速乘数。
/// 叠加所有 CultivationAcceleration buff 的 magnitude，上限 5×。
pub(crate) fn cultivation_acceleration_multiplier(se: &StatusEffects) -> f64 {
    let sum: f32 = se
        .active
        .iter()
        .filter(|e| e.kind == StatusEffectKind::CultivationAcceleration && e.remaining_ticks > 0)
        .map(|e| e.magnitude.max(0.0))
        .sum();
    (1.0 + sum as f64).min(5.0)
}

/// bughunt r4-P2#6：QiRegenBoost 回气加速乘数（consumer 端）。
/// 叠加所有 remaining_ticks > 0 的 QiRegenBoost magnitude，上限 3×（约束：
/// QiCapPermMinus 已在 attribute_aggregate_tick 消费，此处只管回气速率）。
pub(crate) fn qi_regen_boost_multiplier(se: &StatusEffects) -> f64 {
    let sum: f32 = se
        .active
        .iter()
        .filter(|e| e.kind == StatusEffectKind::QiRegenBoost && e.remaining_ticks > 0)
        .map(|e| e.magnitude.max(0.0))
        .sum();
    (1.0 + sum as f64).min(3.0)
}

/// plan-cultivation-pacing-v1 P1.2：qi 回复减速乘数。
/// 叠加所有 QiRegenSlowed debuff 的 magnitude，clamp 到 [0.0, 1.0]。
fn qi_regen_slowed_multiplier(se: &StatusEffects) -> f64 {
    let sum: f32 = se
        .active
        .iter()
        .filter(|e| e.kind == StatusEffectKind::QiRegenSlowed && e.remaining_ticks > 0)
        .map(|e| e.magnitude.max(0.0))
        .sum();
    (1.0 - sum as f64).clamp(0.0, 1.0)
}

fn has_frailty_status(status_effects: &StatusEffects) -> bool {
    status_effects
        .active
        .iter()
        .any(|effect| effect.kind == StatusEffectKind::Frailty && effect.remaining_ticks > 0)
}

/// 虚脱 debuff 的 qi 回复乘数。取所有 active `StatusEffectKind::Exhausted`
/// 的 magnitude 连乘（实际只会有一条，magnitude 恒为 0.5）；无虚脱时返回 1.0。
/// 取代旧的游离 `Exhausted` 组件的 `qi_recovery_modifier` 字段，数值守恒不变。
pub(crate) fn exhausted_qi_recovery_multiplier(se: &StatusEffects) -> f64 {
    se.active
        .iter()
        .filter(|e| e.kind == StatusEffectKind::Exhausted && e.remaining_ticks > 0)
        .fold(1.0_f64, |acc, e| {
            acc * f64::from(e.magnitude.clamp(0.0, 1.0))
        })
}

#[cfg(test)]
#[path = "tick_tests.rs"]
mod tests;
