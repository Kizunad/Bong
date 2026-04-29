//! 化虚渡劫（plan §3.2）。
//!
//! Spirit → Void 的唯一通路，流程：
//!   1. 玩家 `InitiateXuhuaTribulation` → 进入 TribulationState
//!   2. 全服广播（由 network 层消费 `TribulationAnnounce`）
//!   3. calamity agent 生成天劫脚本（多波次），本 plan 接收 `TribulationWave`
//!      事件并让战斗 plan 施加伤害（此处不实现）
//!   4. 扛过所有波次 → realm = Void；任一波次失败 → 退回通灵初期，不进入死亡流程
//!
//! P1/P5：本文件只定义状态机 + 事件；真实天劫伤害由战斗 plan 实施。

use valence::prelude::{
    bevy_ecs, Client, Commands, Component, Entity, Event, EventReader, EventWriter, Position,
    Query, RemovedComponents, Res, Username, With,
};

use crate::combat::components::{BodyPart, Lifecycle, Wound, WoundKind, Wounds};
use crate::combat::events::{CombatEvent, DeathEvent};
use crate::combat::CombatClock;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{LifespanCapTable, LifespanComponent};
use crate::inventory::{transfer_all_inventory_contents, PlayerInventory};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::RedisBridgeResource;
use crate::schema::tribulation::{
    DuXuOutcomeV1, DuXuResultV1, TribulationEventV1, TribulationPhaseV1,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::skill::components::SkillId;
use crate::skill::events::SkillCapChanged;

use super::breakthrough::skill_cap_for_realm;
use super::components::{Cultivation, MeridianSystem, Realm};
use super::qi_zero_decay::{close_meridian, pick_closures};
use crate::persistence::{
    complete_tribulation_ascension, delete_active_tribulation, load_ascension_quota,
    persist_active_tribulation, ActiveTribulationRecord, PersistenceSettings,
};

pub const DUXU_OMEN_TICKS: u64 = 60 * 20;
pub const DUXU_LOCK_TICKS: u64 = 30 * 20;
pub const DUXU_WAVE_COOLDOWN_TICKS: u64 = 15 * 20;
pub const DUXU_MAX_WAVES: u32 = 5;
pub const TRIBULATION_DANGER_RADIUS: f64 = 100.0;
pub const DUXU_LOCK_RADIUS_SOFT: f64 = 50.0;
pub const DUXU_LOCK_RADIUS_HARD: f64 = 20.0;
pub const DUXU_LOCK_RADIUS_FINAL: f64 = 10.0;

const DUXU_DEFAULT_WAVES: u32 = 3;
const DUXU_AOE_DAMAGE_BASE: f32 = 18.0;
const DUXU_QI_DRAIN_BASE: f64 = 35.0;
const DUXU_CHAIN_LIGHTNING_STRIKES: u32 = 3;
const DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO: f64 = 0.20;
const DUXU_KAITIAN_WAVE: u32 = 5;
const DUXU_FULL_HEALTH_EPSILON: f32 = 0.001;
const DUXU_FULL_QI_EPSILON: f64 = 0.001;
const HALF_STEP_QI_MAX_MULTIPLIER: f64 = 1.10;
const HALF_STEP_LIFESPAN_YEARS: u32 = 200;

#[derive(Debug, Clone, Copy)]
struct DuXuWaveProfile {
    strikes: u32,
    damage: f32,
    qi_drain: f64,
    qi_max_freeze_ratio: f64,
    requires_full_resources: bool,
}

#[derive(Debug, Clone, Component)]
pub struct TribulationState {
    pub kind: TribulationKind,
    pub phase: TribulationPhase,
    pub epicenter: [f64; 3],
    pub wave_current: u32,
    pub waves_total: u32,
    pub started_tick: u64,
    pub phase_started_tick: u64,
    pub next_wave_tick: u64,
    pub participants: Vec<String>,
    pub failed: bool,
    pub half_step_on_success: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TribulationKind {
    DuXu,
    ZoneCollapse,
    Targeted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TribulationPhase {
    Omen,
    Lock,
    Wave(u32),
    HeartDemon,
    Settle,
}

impl TribulationState {
    pub fn restored(wave_current: u32, waves_total: u32, started_tick: u64) -> Self {
        Self {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(wave_current.max(1)),
            epicenter: [0.0, 64.0, 0.0],
            wave_current,
            waves_total,
            started_tick,
            phase_started_tick: started_tick,
            next_wave_tick: started_tick,
            participants: Vec::new(),
            failed: false,
            half_step_on_success: false,
        }
    }

    pub fn lock_radius(&self, now_tick: u64) -> f64 {
        match self.phase {
            TribulationPhase::Omen => {
                if now_tick.saturating_sub(self.started_tick) >= DUXU_OMEN_TICKS / 2 {
                    DUXU_LOCK_RADIUS_SOFT
                } else {
                    TRIBULATION_DANGER_RADIUS
                }
            }
            TribulationPhase::Lock => DUXU_LOCK_RADIUS_HARD,
            TribulationPhase::Wave(_) | TribulationPhase::HeartDemon => DUXU_LOCK_RADIUS_FINAL,
            TribulationPhase::Settle => 0.0,
        }
    }

    fn is_primary_tribulator(&self, character_id: &str) -> bool {
        self.participants
            .first()
            .is_some_and(|participant| participant == character_id)
    }

    fn record_interceptor(&mut self, character_id: &str) -> bool {
        if self
            .participants
            .iter()
            .any(|participant| participant == character_id)
        {
            return false;
        }
        self.participants.push(character_id.to_string());
        true
    }

    fn ensure_primary_tribulator(&mut self, character_id: &str) {
        if self.participants.is_empty() {
            self.participants.push(character_id.to_string());
        }
    }
}

#[derive(Debug, Clone, Event)]
pub struct InitiateXuhuaTribulation {
    pub entity: Entity,
    pub waves_total: u32,
    pub started_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct StartDuXuRequest {
    pub entity: Entity,
    pub requested_at_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationAnnounce {
    pub entity: Entity,
    pub char_id: String,
    pub actor_name: String,
    pub epicenter: [f64; 3],
    pub waves_total: u32,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationLocked {
    pub entity: Entity,
    pub char_id: String,
    pub actor_name: String,
    pub epicenter: [f64; 3],
    pub waves_total: u32,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationSettled {
    pub entity: Entity,
    pub result: DuXuResultV1,
}

#[derive(Debug, Clone, Event)]
pub struct AscensionQuotaOpened {
    pub occupied_slots: u32,
}

/// 单波次通过（由战斗 plan 发送）。
#[derive(Debug, Clone, Event)]
pub struct TribulationWaveCleared {
    pub entity: Entity,
    pub wave: u32,
}

/// 渡劫失败（战斗 plan 在天劫波次失败时发送；不进入死亡生命周期）。
#[derive(Debug, Clone, Event)]
pub struct TribulationFailed {
    pub entity: Entity,
    pub wave: u32,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationFled {
    pub entity: Entity,
    pub tick: u64,
}

#[allow(clippy::type_complexity)]
pub fn start_du_xu_request_system(
    mut requests: EventReader<StartDuXuRequest>,
    mut initiate: EventWriter<InitiateXuhuaTribulation>,
    players: Query<(&Cultivation, &MeridianSystem, Option<&TribulationState>)>,
) {
    for request in requests.read() {
        let Ok((cultivation, meridians, active)) = players.get(request.entity) else {
            continue;
        };
        if active.is_some() || !du_xu_prereqs_met(cultivation, meridians) {
            tracing::warn!(
                "[bong][cultivation] start_du_xu rejected entity={:?} realm={:?} opened_meridians={}",
                request.entity,
                cultivation.realm,
                meridians.opened_count(),
            );
            continue;
        }
        initiate.send(InitiateXuhuaTribulation {
            entity: request.entity,
            waves_total: DUXU_DEFAULT_WAVES,
            started_tick: request.requested_at_tick,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_tribulation_system(
    settings: Res<PersistenceSettings>,
    mut events: EventReader<InitiateXuhuaTribulation>,
    mut announce: EventWriter<TribulationAnnounce>,
    mut players: Query<(&Cultivation, &MeridianSystem, &Lifecycle, Option<&Username>)>,
    player_count: Query<(), With<Client>>,
    mut commands: Commands,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for ev in events.read() {
        if let Ok((c, meridians, lifecycle, username)) = players.get_mut(ev.entity) {
            if c.realm != Realm::Spirit {
                tracing::warn!(
                    "[bong][cultivation] {:?} tried to tribulate from {:?}, rejected",
                    ev.entity,
                    c.realm
                );
                continue;
            }
            if !du_xu_prereqs_met(c, meridians) {
                tracing::warn!(
                    "[bong][cultivation] {:?} tried to tribulate without all meridians open",
                    ev.entity,
                );
                continue;
            }
            let p = positions
                .get(ev.entity)
                .map(|pos| pos.get())
                .unwrap_or(valence::math::DVec3::new(0.0, 64.0, 0.0));
            let occupied_slots = load_ascension_quota(&settings)
                .map(|quota| quota.occupied_slots)
                .unwrap_or(0);
            let quota_limit = ascension_quota_limit(player_count.iter().count());
            let state = TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Omen,
                epicenter: [p.x, p.y, p.z],
                wave_current: 0,
                waves_total: ev.waves_total.clamp(1, DUXU_MAX_WAVES),
                started_tick: ev.started_tick,
                phase_started_tick: ev.started_tick,
                next_wave_tick: ev
                    .started_tick
                    .saturating_add(DUXU_OMEN_TICKS + DUXU_LOCK_TICKS),
                participants: vec![lifecycle.character_id.clone()],
                failed: false,
                half_step_on_success: occupied_slots >= quota_limit,
            };
            if let Err(error) = persist_active_tribulation(
                &settings,
                &ActiveTribulationRecord {
                    char_id: lifecycle.character_id.clone(),
                    wave_current: state.wave_current,
                    waves_total: state.waves_total,
                    started_tick: state.started_tick,
                },
            ) {
                tracing::warn!(
                    "[bong][cultivation] failed to persist active tribulation for {:?}: {error}",
                    ev.entity,
                );
            }
            commands.entity(ev.entity).insert(state);
            announce.send(TribulationAnnounce {
                entity: ev.entity,
                char_id: lifecycle.character_id.clone(),
                actor_name: username
                    .map(|name| name.0.clone())
                    .unwrap_or_else(|| lifecycle.character_id.clone()),
                epicenter: [p.x, p.y, p.z],
                waves_total: ev.waves_total.clamp(1, DUXU_MAX_WAVES),
            });
            tracing::info!(
                "[bong][cultivation] {:?} initiated tribulation ({} waves)",
                ev.entity,
                ev.waves_total
            );
            // plan-particle-system-v1 §4.4：渡劫开场一道预警雷。
            vfx_events.send(VfxEventRequest::new(
                p,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: "bong:tribulation_lightning".to_string(),
                    origin: [p.x, p.y, p.z],
                    direction: None,
                    color: Some("#D0C8FF".to_string()),
                    strength: Some(1.0),
                    count: Some(3),
                    duration_ticks: Some(14),
                },
            ));
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn tribulation_phase_tick_system(
    clock: Res<CombatClock>,
    mut query: Query<(
        Entity,
        &mut TribulationState,
        Option<&Lifecycle>,
        Option<&Username>,
    )>,
    mut locked: EventWriter<TribulationLocked>,
    mut cleared: EventWriter<TribulationWaveCleared>,
) {
    for (entity, mut state, lifecycle, username) in &mut query {
        match state.phase {
            TribulationPhase::Omen
                if clock.tick.saturating_sub(state.phase_started_tick) >= DUXU_OMEN_TICKS =>
            {
                let char_id = lifecycle
                    .map(|lifecycle| lifecycle.character_id.clone())
                    .or_else(|| state.participants.first().cloned())
                    .unwrap_or_else(|| format!("entity:{entity:?}"));
                let actor_name = username
                    .map(|name| name.0.clone())
                    .unwrap_or_else(|| char_id.clone());
                state.phase = TribulationPhase::Lock;
                state.phase_started_tick = clock.tick;
                locked.send(TribulationLocked {
                    entity,
                    char_id,
                    actor_name,
                    epicenter: state.epicenter,
                    waves_total: state.waves_total,
                });
            }
            TribulationPhase::Lock
                if clock.tick.saturating_sub(state.phase_started_tick) >= DUXU_LOCK_TICKS =>
            {
                let next_wave = state.wave_current.saturating_add(1);
                begin_tribulation_wave(&mut state, entity, next_wave, clock.tick, &mut cleared);
            }
            TribulationPhase::Wave(_) if clock.tick >= state.next_wave_tick && !state.failed => {
                let next_wave = state.wave_current.saturating_add(1);
                begin_tribulation_wave(&mut state, entity, next_wave, clock.tick, &mut cleared);
            }
            _ => {}
        }
    }
}

fn begin_tribulation_wave(
    state: &mut TribulationState,
    entity: Entity,
    wave: u32,
    tick: u64,
    cleared: &mut EventWriter<TribulationWaveCleared>,
) {
    if wave == 0 || wave > state.waves_total {
        return;
    }
    state.phase = TribulationPhase::Wave(wave);
    state.phase_started_tick = tick;
    state.next_wave_tick = tick.saturating_add(DUXU_WAVE_COOLDOWN_TICKS);
    cleared.send(TribulationWaveCleared { entity, wave });
}

#[allow(clippy::type_complexity)]
pub fn tribulation_aoe_system(
    clock: Res<CombatClock>,
    tribulations: Query<(Entity, &TribulationState)>,
    mut targets: Query<(
        Entity,
        &Position,
        &mut Cultivation,
        &mut Wounds,
        Option<&Lifecycle>,
    )>,
    mut failed: EventWriter<TribulationFailed>,
    mut deaths: EventWriter<DeathEvent>,
) {
    for (tribulator_entity, state) in &tribulations {
        let TribulationPhase::Wave(wave) = state.phase else {
            continue;
        };
        if clock.tick != state.phase_started_tick {
            continue;
        }
        let center =
            valence::math::DVec3::new(state.epicenter[0], state.epicenter[1], state.epicenter[2]);
        let profile = du_xu_wave_profile(wave);
        let strike_damage = profile.damage / profile.strikes.max(1) as f32;
        for (entity, pos, mut cultivation, mut wounds, lifecycle) in &mut targets {
            if pos.get().distance(center) > TRIBULATION_DANGER_RADIUS {
                continue;
            }
            let is_tribulator = entity == tribulator_entity
                || lifecycle
                    .map(|lifecycle| state.is_primary_tribulator(&lifecycle.character_id))
                    .unwrap_or(false);
            if profile.requires_full_resources
                && is_tribulator
                && !has_full_tribulation_resources(&cultivation, &wounds)
            {
                failed.send(TribulationFailed { entity, wave });
                continue;
            }
            cultivation.qi_current = (cultivation.qi_current - profile.qi_drain).max(0.0);
            if profile.qi_max_freeze_ratio > 0.0 {
                let frozen = cultivation.qi_max_frozen.unwrap_or(0.0);
                cultivation.qi_max_frozen = Some(
                    (frozen + cultivation.qi_max * profile.qi_max_freeze_ratio)
                        .min(cultivation.qi_max),
                );
            }
            let was_alive = wounds.health_current > 0.0;
            wounds.health_current =
                (wounds.health_current - profile.damage).clamp(0.0, wounds.health_max);
            for _ in 0..profile.strikes {
                wounds.entries.push(Wound {
                    location: BodyPart::Chest,
                    kind: WoundKind::Burn,
                    severity: strike_damage,
                    bleeding_per_sec: 0.0,
                    created_at_tick: clock.tick,
                    inflicted_by: Some("du_xu_tribulation".to_string()),
                });
            }
            if !was_alive || wounds.health_current > 0.0 {
                continue;
            }
            if is_tribulator {
                failed.send(TribulationFailed { entity, wave });
            } else {
                deaths.send(DeathEvent {
                    target: entity,
                    cause: "观劫而亡".to_string(),
                    attacker: None,
                    attacker_player_id: None,
                    at_tick: clock.tick,
                });
            }
        }
    }
}

fn du_xu_wave_profile(wave: u32) -> DuXuWaveProfile {
    DuXuWaveProfile {
        strikes: if wave == 2 {
            DUXU_CHAIN_LIGHTNING_STRIKES
        } else {
            1
        },
        damage: DUXU_AOE_DAMAGE_BASE * wave as f32,
        qi_drain: DUXU_QI_DRAIN_BASE * f64::from(wave),
        qi_max_freeze_ratio: if wave == 3 {
            DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO
        } else {
            0.0
        },
        requires_full_resources: wave == DUXU_KAITIAN_WAVE,
    }
}

fn has_full_tribulation_resources(cultivation: &Cultivation, wounds: &Wounds) -> bool {
    let effective_qi_max = (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0)).max(0.0);
    wounds.health_current + DUXU_FULL_HEALTH_EPSILON >= wounds.health_max
        && cultivation.qi_current + DUXU_FULL_QI_EPSILON >= effective_qi_max
}

#[allow(clippy::type_complexity)]
pub fn record_tribulation_interceptor_system(
    mut combat_events: EventReader<CombatEvent>,
    mut tribulators: Query<(&mut TribulationState, &Lifecycle)>,
    actors: Query<(&Lifecycle, &Position)>,
) {
    for event in combat_events.read() {
        let Ok((mut state, target_lifecycle)) = tribulators.get_mut(event.target) else {
            continue;
        };
        if state.kind != TribulationKind::DuXu
            || !matches!(
                state.phase,
                TribulationPhase::Lock | TribulationPhase::Wave(_)
            )
        {
            continue;
        }
        if state
            .participants
            .first()
            .is_some_and(|participant| participant != &target_lifecycle.character_id)
        {
            continue;
        }
        let Ok((attacker_lifecycle, attacker_position)) = actors.get(event.attacker) else {
            continue;
        };
        if attacker_lifecycle.character_id == target_lifecycle.character_id
            || !attacker_lifecycle.character_id.starts_with("offline:")
        {
            continue;
        }
        let center =
            valence::math::DVec3::new(state.epicenter[0], state.epicenter[1], state.epicenter[2]);
        if attacker_position.get().distance(center) > DUXU_LOCK_RADIUS_HARD {
            continue;
        }
        state.ensure_primary_tribulator(&target_lifecycle.character_id);
        if state.record_interceptor(&attacker_lifecycle.character_id) {
            tracing::info!(
                "[bong][cultivation] {} entered DuXu interception against {}",
                attacker_lifecycle.character_id,
                target_lifecycle.character_id,
            );
        }
    }
}

pub fn tribulation_wave_system(
    settings: Res<PersistenceSettings>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut players: Query<(
        &mut Cultivation,
        &mut TribulationState,
        &MeridianSystem,
        &Lifecycle,
        Option<&mut LifespanComponent>,
    )>,
    mut commands: Commands,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut settled: EventWriter<TribulationSettled>,
) {
    for ev in cleared.read() {
        if let Ok((mut c, mut state, _, lifecycle, lifespan)) = players.get_mut(ev.entity) {
            if state.failed {
                continue;
            }
            state.wave_current = state.wave_current.max(ev.wave);
            if state.wave_current >= state.waves_total {
                // 渡劫成功
                let outcome = if state.half_step_on_success {
                    c.realm = Realm::Spirit;
                    c.qi_max *= HALF_STEP_QI_MAX_MULTIPLIER;
                    if let Some(mut lifespan) = lifespan {
                        lifespan.cap_by_realm = lifespan
                            .cap_by_realm
                            .max(LifespanCapTable::SPIRIT.saturating_add(HALF_STEP_LIFESPAN_YEARS));
                    }
                    if let Err(error) =
                        delete_active_tribulation(&settings, lifecycle.character_id.as_str())
                    {
                        tracing::warn!(
                            "[bong][cultivation] failed to clear half-step tribulation for {:?}: {error}",
                            ev.entity,
                        );
                    }
                    DuXuOutcomeV1::HalfStep
                } else {
                    c.realm = Realm::Void;
                    c.qi_max *= super::breakthrough::qi_max_multiplier(Realm::Void);
                    if let Some(mut lifespan) = lifespan {
                        lifespan.apply_cap(LifespanCapTable::VOID);
                    }
                    if let Err(error) =
                        complete_tribulation_ascension(&settings, lifecycle.character_id.as_str())
                    {
                        tracing::warn!(
                            "[bong][cultivation] failed to finalize tribulation ascension for {:?}: {error}",
                            ev.entity,
                        );
                    }
                    // plan-skill-v1 §4：化虚 cap=10，全部 skill 解锁满级上限。
                    let new_cap = skill_cap_for_realm(Realm::Void);
                    for skill in [SkillId::Herbalism, SkillId::Alchemy, SkillId::Forging] {
                        skill_cap_events.send(SkillCapChanged {
                            char_entity: ev.entity,
                            skill,
                            new_cap,
                        });
                    }
                    DuXuOutcomeV1::Ascended
                };
                settled.send(TribulationSettled {
                    entity: ev.entity,
                    result: DuXuResultV1 {
                        char_id: lifecycle.character_id.clone(),
                        outcome,
                        killer: None,
                        waves_survived: state.waves_total,
                    },
                });
                state.phase = TribulationPhase::Settle;
                commands.entity(ev.entity).remove::<TribulationState>();
                tracing::info!(
                    "[bong][cultivation] {:?} settled DuXu as {:?} after {} waves",
                    ev.entity,
                    outcome,
                    state.waves_total
                );
            } else if let Err(error) = persist_active_tribulation(
                &settings,
                &ActiveTribulationRecord {
                    char_id: lifecycle.character_id.clone(),
                    wave_current: state.wave_current,
                    waves_total: state.waves_total,
                    started_tick: state.started_tick,
                },
            ) {
                tracing::warn!(
                    "[bong][cultivation] failed to update active tribulation for {:?}: {error}",
                    ev.entity,
                );
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn tribulation_failure_system(
    settings: Res<PersistenceSettings>,
    mut failed: EventReader<TribulationFailed>,
    mut players: Query<(
        &mut Cultivation,
        Option<&mut MeridianSystem>,
        &Lifecycle,
        Option<&mut Wounds>,
        Option<&mut TribulationState>,
    )>,
    mut commands: Commands,
    mut settled: EventWriter<TribulationSettled>,
) {
    for ev in failed.read() {
        if let Ok((mut cultivation, meridians, lifecycle, wounds, state)) =
            players.get_mut(ev.entity)
        {
            if let Some(mut state) = state {
                state.failed = true;
                state.phase = TribulationPhase::Settle;
            }
            apply_tribulation_failure_penalty(&mut cultivation, meridians, wounds);
            if let Err(error) =
                delete_active_tribulation(&settings, lifecycle.character_id.as_str())
            {
                tracing::warn!(
                    "[bong][cultivation] failed to delete failed active tribulation for {:?}: {error}",
                    ev.entity,
                );
            }
            tracing::info!(
                "[bong][cultivation] {:?} failed tribulation at wave {}; regressed to Spirit without death lifecycle",
                ev.entity,
                ev.wave,
            );
            settled.send(TribulationSettled {
                entity: ev.entity,
                result: DuXuResultV1 {
                    char_id: lifecycle.character_id.clone(),
                    outcome: DuXuOutcomeV1::Failed,
                    killer: None,
                    waves_survived: ev.wave.saturating_sub(1),
                },
            });
        }
        commands.entity(ev.entity).remove::<TribulationState>();
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn abort_du_xu_on_client_removed(
    clock: Res<CombatClock>,
    settings: Res<PersistenceSettings>,
    mut removed_clients: RemovedComponents<Client>,
    mut players: Query<(
        &mut Cultivation,
        Option<&mut MeridianSystem>,
        &Lifecycle,
        Option<&mut Wounds>,
        &mut TribulationState,
        Option<&mut LifeRecord>,
    )>,
    mut commands: Commands,
    mut settled: EventWriter<TribulationSettled>,
    mut fled: EventWriter<TribulationFled>,
) {
    for entity in removed_clients.read() {
        let Ok((mut cultivation, meridians, lifecycle, wounds, mut state, life_record)) =
            players.get_mut(entity)
        else {
            continue;
        };
        if state.kind != TribulationKind::DuXu {
            continue;
        }
        settle_fled_tribulation(
            entity,
            clock.tick,
            &settings,
            &mut commands,
            &mut cultivation,
            meridians,
            lifecycle,
            wounds,
            &mut state,
            life_record,
            &mut settled,
            &mut fled,
        );
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn tribulation_escape_boundary_system(
    clock: Res<CombatClock>,
    settings: Res<PersistenceSettings>,
    mut players: Query<(
        Entity,
        &Position,
        &mut Cultivation,
        Option<&mut MeridianSystem>,
        &Lifecycle,
        Option<&mut Wounds>,
        &mut TribulationState,
        Option<&mut LifeRecord>,
    )>,
    mut commands: Commands,
    mut settled: EventWriter<TribulationSettled>,
    mut fled: EventWriter<TribulationFled>,
) {
    for (entity, position, mut cultivation, meridians, lifecycle, wounds, mut state, life_record) in
        &mut players
    {
        if state.kind != TribulationKind::DuXu || matches!(state.phase, TribulationPhase::Omen) {
            continue;
        }
        let center =
            valence::math::DVec3::new(state.epicenter[0], state.epicenter[1], state.epicenter[2]);
        if position.get().distance(center) <= state.lock_radius(clock.tick) {
            continue;
        }
        settle_fled_tribulation(
            entity,
            clock.tick,
            &settings,
            &mut commands,
            &mut cultivation,
            meridians,
            lifecycle,
            wounds,
            &mut state,
            life_record,
            &mut settled,
            &mut fled,
        );
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn settle_fled_tribulation(
    entity: Entity,
    fled_tick: u64,
    settings: &PersistenceSettings,
    commands: &mut Commands,
    cultivation: &mut Cultivation,
    meridians: Option<valence::prelude::Mut<'_, MeridianSystem>>,
    lifecycle: &Lifecycle,
    wounds: Option<valence::prelude::Mut<'_, Wounds>>,
    state: &mut TribulationState,
    life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    settled: &mut EventWriter<TribulationSettled>,
    fled: &mut EventWriter<TribulationFled>,
) {
    state.failed = true;
    state.phase = TribulationPhase::Settle;
    let waves_survived = state.wave_current;
    if let Some(mut life_record) = life_record {
        life_record.push(BiographyEntry::TribulationFled {
            wave: waves_survived.saturating_add(1),
            tick: fled_tick,
        });
    }
    apply_tribulation_failure_penalty(cultivation, meridians, wounds);
    if let Err(error) = delete_active_tribulation(settings, lifecycle.character_id.as_str()) {
        tracing::warn!(
            "[bong][cultivation] failed to delete fled active tribulation for {:?}: {error}",
            entity,
        );
    }
    settled.send(TribulationSettled {
        entity,
        result: DuXuResultV1 {
            char_id: lifecycle.character_id.clone(),
            outcome: DuXuOutcomeV1::Fled,
            killer: None,
            waves_survived,
        },
    });
    fled.send(TribulationFled {
        entity,
        tick: fled_tick,
    });
    commands.entity(entity).remove::<TribulationState>();
}

#[allow(clippy::type_complexity)]
pub fn tribulation_intercept_death_system(
    mut deaths: EventReader<DeathEvent>,
    mut commands: Commands,
    settings: Res<PersistenceSettings>,
    mut q: Query<(&TribulationState, &Lifecycle)>,
    mut inventories: Query<&mut PlayerInventory>,
    mut life_records: Query<&mut LifeRecord>,
    mut settled: EventWriter<TribulationSettled>,
) {
    for death in deaths.read() {
        let Ok((state, lifecycle)) = q.get_mut(death.target) else {
            continue;
        };
        if death.attacker_player_id.is_none() {
            continue;
        }
        if let Err(error) = delete_active_tribulation(&settings, lifecycle.character_id.as_str()) {
            tracing::warn!(
                "[bong][cultivation] failed to clear intercepted tribulation for {:?}: {error}",
                death.target,
            );
        }
        if let Some(killer_entity) = death.attacker.filter(|attacker| *attacker != death.target) {
            let loot_outcome = inventories
                .get_many_mut([death.target, killer_entity])
                .ok()
                .map(|[mut victim_inventory, mut killer_inventory]| {
                    transfer_all_inventory_contents(&mut victim_inventory, &mut killer_inventory)
                });
            if let Some(outcome) = loot_outcome {
                tracing::info!(
                    "[bong][cultivation] {:?} intercepted DuXu target {:?}; transferred {} item(s), {} bone coin(s)",
                    killer_entity,
                    death.target,
                    outcome.items_moved,
                    outcome.bone_coins_moved,
                );
            }
            if let Ok(mut life_record) = life_records.get_mut(killer_entity) {
                life_record.push(BiographyEntry::TribulationIntercepted {
                    victim_id: lifecycle.character_id.clone(),
                    tick: death.at_tick,
                });
            }
        }
        settled.send(TribulationSettled {
            entity: death.target,
            result: DuXuResultV1 {
                char_id: lifecycle.character_id.clone(),
                outcome: DuXuOutcomeV1::Killed,
                killer: death.attacker_player_id.clone(),
                waves_survived: state.wave_current,
            },
        });
        commands.entity(death.target).remove::<TribulationState>();
    }
}

pub fn publish_tribulation_events(
    redis: Res<RedisBridgeResource>,
    mut announce: EventReader<TribulationAnnounce>,
    mut locked: EventReader<TribulationLocked>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut settled: EventReader<TribulationSettled>,
    mut quota_opened: EventReader<AscensionQuotaOpened>,
    states: Query<&TribulationState>,
) {
    for ev in announce.read() {
        let payload = TribulationEventV1::du_xu(
            TribulationPhaseV1::Omen,
            Some(ev.char_id.clone()),
            Some(ev.actor_name.clone()),
            Some(ev.epicenter),
            Some(0),
            Some(ev.waves_total),
            None,
        );
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in locked.read() {
        let payload = TribulationEventV1::du_xu(
            TribulationPhaseV1::Lock,
            Some(ev.char_id.clone()),
            Some(ev.actor_name.clone()),
            Some(ev.epicenter),
            Some(0),
            Some(ev.waves_total),
            None,
        );
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in cleared.read() {
        let Ok(state) = states.get(ev.entity) else {
            continue;
        };
        let payload = TribulationEventV1::du_xu(
            TribulationPhaseV1::Wave { wave: ev.wave },
            None,
            None,
            Some(state.epicenter),
            Some(ev.wave),
            Some(state.waves_total),
            None,
        );
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in settled.read() {
        let payload = TribulationEventV1::du_xu(
            TribulationPhaseV1::Settle,
            Some(ev.result.char_id.clone()),
            None,
            None,
            Some(ev.result.waves_survived),
            None,
            Some(ev.result.clone()),
        );
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
    for ev in quota_opened.read() {
        let payload = TribulationEventV1::ascension_quota_open(Some(ev.occupied_slots));
        let _ = redis
            .tx_outbound
            .send(crate::network::redis_bridge::RedisOutbound::TribulationEvent(payload));
    }
}

pub fn du_xu_prereqs_met(cultivation: &Cultivation, meridians: &MeridianSystem) -> bool {
    cultivation.realm == Realm::Spirit
        && meridians.iter().all(|meridian| meridian.opened)
        && meridians.opened_count() >= Realm::Void.required_meridians()
}

pub fn ascension_quota_limit(player_count: usize) -> u32 {
    let scaled = (player_count / 50).max(1) as u32;
    scaled.min(3)
}

fn apply_tribulation_failure_penalty(
    cultivation: &mut Cultivation,
    meridians: Option<valence::prelude::Mut<'_, MeridianSystem>>,
    wounds: Option<valence::prelude::Mut<'_, Wounds>>,
) {
    cultivation.realm = Realm::Spirit;
    cultivation.qi_current = 0.0;
    cultivation.last_qi_zero_at = None;
    cultivation.pending_material_bonus = 0.0;

    if let Some(mut meridians) = meridians {
        let keep = Realm::Spirit.required_meridians();
        let closures = pick_closures(&meridians, keep);
        for (is_regular, idx) in closures {
            if is_regular {
                close_meridian(&mut meridians.regular[idx]);
            } else {
                close_meridian(&mut meridians.extraordinary[idx]);
            }
        }
        cultivation.qi_max = 10.0 + meridians.sum_capacity();
    }

    if let Some(mut wounds) = wounds {
        let floor = (wounds.health_max.max(1.0) * 0.05).max(1.0);
        wounds.health_current = wounds
            .health_current
            .max(floor)
            .min(wounds.health_max.max(1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::components::{CombatState, Lifecycle, LifecycleState, Stamina, Wounds};
    use crate::combat::events::{CombatEvent, DeathEvent, DeathInsightRequested};
    use crate::combat::lifecycle::death_arbiter_tick;
    use crate::combat::CombatClock;
    use crate::cultivation::components::MeridianId;
    use crate::cultivation::death_hooks::{CultivationDeathTrigger, PlayerTerminated};
    use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
    use crate::cultivation::lifespan::{
        DeathRegistry, LifespanCapTable, LifespanComponent, ZoneDeathKind,
    };
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory, MAIN_PACK_CONTAINER_ID,
    };
    use crate::network::redis_bridge::RedisOutbound;
    use crate::network::vfx_event_emit::VfxEventRequest;
    use crate::network::RedisBridgeResource;
    use crate::persistence::{bootstrap_sqlite, load_active_tribulation};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Entity, Events, IntoSystemConfigs, Position, Update, Username};
    use valence::testing::create_mock_client;

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bong-tribulation-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = unique_temp_dir(test_name);
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        bootstrap_sqlite(&db_path, &format!("tribulation-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PersistenceSettings::with_paths(
                &db_path,
                &deceased_dir,
                format!("tribulation-{test_name}"),
            ),
            root,
        )
    }

    fn all_meridians_open() -> MeridianSystem {
        let mut meridians = MeridianSystem::default();
        for (idx, id) in MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
            .enumerate()
        {
            let meridian = meridians.get_mut(*id);
            meridian.opened = true;
            meridian.open_progress = 1.0;
            meridian.opened_at = idx as u64;
        }
        meridians
    }

    fn test_item(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: format!("test_item_{instance_id}"),
            display_name: format!("test {instance_id}"),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: "test".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        }
    }

    fn test_inventory(items: Vec<ItemInstance>, bone_coins: u64) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 5,
                items: items
                    .into_iter()
                    .enumerate()
                    .map(|(idx, instance)| PlacedItemState {
                        row: (idx / 5) as u8,
                        col: (idx % 5) as u8,
                        instance,
                    })
                    .collect(),
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins,
            max_weight: 50.0,
        }
    }

    #[test]
    fn omen_to_lock_emits_lock_event() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: DUXU_OMEN_TICKS,
        });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);

        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Username("Azure".to_string()),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Omen,
                    epicenter: [12.0, 66.0, -8.0],
                    wave_current: 0,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: DUXU_OMEN_TICKS + DUXU_LOCK_TICKS,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                    half_step_on_success: false,
                },
            ))
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Lock);

        let events = app.world().resource::<Events<TribulationLocked>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].char_id, "offline:Azure");
        assert_eq!(emitted[0].actor_name, "Azure");
        assert_eq!(emitted[0].epicenter, [12.0, 66.0, -8.0]);
        assert_eq!(emitted[0].waves_total, 3);
    }

    #[test]
    fn publish_lock_event_to_tribulation_channel() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_systems(Update, publish_tribulation_events);

        app.world_mut()
            .resource_mut::<Events<TribulationLocked>>()
            .send(TribulationLocked {
                entity: Entity::PLACEHOLDER,
                char_id: "offline:Azure".to_string(),
                actor_name: "Azure".to_string(),
                epicenter: [12.0, 66.0, -8.0],
                waves_total: 3,
            });

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("lock event should publish to redis bridge");
        match outbound {
            RedisOutbound::TribulationEvent(payload) => {
                assert_eq!(payload.phase, TribulationPhaseV1::Lock);
                assert_eq!(payload.char_id.as_deref(), Some("offline:Azure"));
                assert_eq!(payload.actor_name.as_deref(), Some("Azure"));
                assert_eq!(payload.epicenter, Some([12.0, 66.0, -8.0]));
                assert_eq!(payload.wave_total, Some(3));
            }
            other => panic!("unexpected outbound payload: {other:?}"),
        }
    }

    #[test]
    fn publish_ascension_quota_open_event_to_tribulation_channel() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<TribulationAnnounce>();
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationSettled>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_systems(Update, publish_tribulation_events);

        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOpened>>()
            .send(AscensionQuotaOpened { occupied_slots: 1 });

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("quota open event should publish to redis bridge");
        match outbound {
            RedisOutbound::TribulationEvent(payload) => {
                assert_eq!(
                    payload.kind,
                    crate::schema::tribulation::TribulationKindV1::AscensionQuotaOpen
                );
                assert_eq!(payload.phase, TribulationPhaseV1::Settle);
                assert_eq!(payload.occupied_slots, Some(1));
            }
            other => panic!("unexpected outbound payload: {other:?}"),
        }
    }

    #[test]
    fn lock_expiry_starts_first_wave_and_schedules_cooldown() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);

        let entity = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Lock,
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 300,
                next_wave_tick: 0,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
                half_step_on_success: false,
            })
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Wave(1));
        assert_eq!(state.phase_started_tick, 900);
        assert_eq!(state.next_wave_tick, 900 + DUXU_WAVE_COOLDOWN_TICKS);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].wave, 1);
    }

    #[test]
    fn wave_cooldown_starts_next_wave_without_reusing_first_wave_phase() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1200 });
        app.add_event::<TribulationLocked>();
        app.add_event::<TribulationWaveCleared>();
        app.add_systems(Update, tribulation_phase_tick_system);

        let entity = app
            .world_mut()
            .spawn(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(1),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 900,
                next_wave_tick: 1200,
                participants: vec!["offline:Azure".to_string()],
                failed: false,
                half_step_on_success: false,
            })
            .id();

        app.update();

        let state = app
            .world()
            .get::<TribulationState>(entity)
            .expect("tribulation should remain active");
        assert_eq!(state.phase, TribulationPhase::Wave(2));
        assert_eq!(state.phase_started_tick, 1200);
        assert_eq!(state.next_wave_tick, 1200 + DUXU_WAVE_COOLDOWN_TICKS);
        let events = app.world().resource::<Events<TribulationWaveCleared>>();
        let emitted: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].wave, 2);
    }

    #[test]
    fn aoe_uses_current_wave_strength_only_on_wave_start_tick() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1200 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        app.world_mut().spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(2),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 1,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 1200,
            next_wave_tick: 1500,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
            half_step_on_success: false,
        });
        let target = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 100.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 100.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Spectator".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let wounds = app
            .world()
            .get::<Wounds>(target)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 100.0 - DUXU_AOE_DAMAGE_BASE * 2.0);
        assert_eq!(wounds.entries.len(), DUXU_CHAIN_LIGHTNING_STRIKES as usize);
        for wound in &wounds.entries {
            assert_eq!(wound.kind, WoundKind::Burn);
            assert_eq!(wound.severity, DUXU_AOE_DAMAGE_BASE * 2.0 / 3.0);
            assert_eq!(wound.created_at_tick, 1200);
            assert_eq!(wound.inflicted_by.as_deref(), Some("du_xu_tribulation"));
        }
        let cultivation = app
            .world()
            .get::<Cultivation>(target)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 100.0 - DUXU_QI_DRAIN_BASE * 2.0);
        assert_eq!(cultivation.qi_max_frozen, None);

        app.world_mut().resource_mut::<CombatClock>().tick = 1201;
        app.update();

        let wounds = app
            .world()
            .get::<Wounds>(target)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 100.0 - DUXU_AOE_DAMAGE_BASE * 2.0);
        assert_eq!(wounds.entries.len(), DUXU_CHAIN_LIGHTNING_STRIKES as usize);
    }

    #[test]
    fn third_wave_freezes_qi_max_as_soul_devouring_lightning() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1500 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        app.world_mut().spawn(TribulationState {
            kind: TribulationKind::DuXu,
            phase: TribulationPhase::Wave(3),
            epicenter: [0.0, 66.0, 0.0],
            wave_current: 3,
            waves_total: 3,
            started_tick: 0,
            phase_started_tick: 1500,
            next_wave_tick: 1800,
            participants: vec!["offline:Azure".to_string()],
            failed: false,
            half_step_on_success: false,
        });
        let target = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 200.0,
                    qi_max: 210.0,
                    qi_max_frozen: Some(10.0),
                    ..Default::default()
                },
                Wounds {
                    health_current: 200.0,
                    health_max: 200.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Spectator".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let wounds = app
            .world()
            .get::<Wounds>(target)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 200.0 - DUXU_AOE_DAMAGE_BASE * 3.0);
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].severity, DUXU_AOE_DAMAGE_BASE * 3.0);
        let cultivation = app
            .world()
            .get::<Cultivation>(target)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 200.0 - DUXU_QI_DRAIN_BASE * 3.0);
        let expected_frozen = 10.0 + 210.0 * DUXU_SOUL_DEVOUR_QI_MAX_FREEZE_RATIO;
        assert!(
            (cultivation.qi_max_frozen.expect("qi max should freeze") - expected_frozen).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn kaitian_lightning_fails_tribulator_without_full_health() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2100 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 210.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 99.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(5),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 5,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                    half_step_on_success: false,
                },
            ))
            .id();

        app.update();

        let failures = app.world().resource::<Events<TribulationFailed>>();
        let emitted: Vec<_> = failures.get_reader().read(failures).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].wave, 5);
        let wounds = app
            .world()
            .get::<Wounds>(entity)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 99.0);
        assert!(wounds.entries.is_empty());
    }

    #[test]
    fn kaitian_lightning_fails_tribulator_without_full_available_qi() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2100 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 189.0,
                    qi_max: 210.0,
                    qi_max_frozen: Some(20.0),
                    ..Default::default()
                },
                Wounds {
                    health_current: 100.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(5),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 5,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                    half_step_on_success: false,
                },
            ))
            .id();

        app.update();

        let failures = app.world().resource::<Events<TribulationFailed>>();
        let emitted: Vec<_> = failures.get_reader().read(failures).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].entity, entity);
        assert_eq!(emitted[0].wave, 5);
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 189.0);
    }

    #[test]
    fn kaitian_lightning_hits_normally_when_tribulator_has_full_resources() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 2100 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 190.0,
                    qi_max: 210.0,
                    qi_max_frozen: Some(20.0),
                    ..Default::default()
                },
                Wounds {
                    health_current: 200.0,
                    health_max: 200.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(5),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 5,
                    waves_total: 5,
                    started_tick: 0,
                    phase_started_tick: 2100,
                    next_wave_tick: 2400,
                    participants: vec!["offline:Azure".to_string()],
                    failed: false,
                    half_step_on_success: false,
                },
            ))
            .id();

        app.update();

        assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);
        let wounds = app
            .world()
            .get::<Wounds>(entity)
            .expect("wounds should remain attached");
        assert_eq!(wounds.health_current, 200.0 - DUXU_AOE_DAMAGE_BASE * 5.0);
        assert_eq!(wounds.entries.len(), 1);
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.qi_current, 190.0 - DUXU_QI_DRAIN_BASE * 5.0);
    }

    #[test]
    fn tribulation_failure_regresses_without_death_lifecycle_side_effects() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("failure-not-death");
        let char_id = "offline:Azure";
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: char_id.to_string(),
                wave_current: 2,
                waves_total: 5,
                started_tick: 120,
            },
        )
        .expect("active tribulation should persist before failure");

        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 300 });
        app.add_event::<TribulationFailed>();
        app.add_event::<TribulationFled>();
        app.add_event::<TribulationSettled>();
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                tribulation_failure_system,
                death_arbiter_tick.after(tribulation_failure_system),
            ),
        );

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 880.0,
                    qi_max: 210.0,
                    last_qi_zero_at: Some(77),
                    pending_material_bonus: 0.3,
                    ..Default::default()
                },
                all_meridians_open(),
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: char_id.to_string(),
                    death_count: 2,
                    last_death_tick: Some(55),
                    state: LifecycleState::Alive,
                    ..Default::default()
                },
                DeathRegistry {
                    char_id: char_id.to_string(),
                    death_count: 2,
                    last_death_tick: Some(55),
                    prev_death_tick: Some(12),
                    last_death_zone: Some(ZoneDeathKind::Ordinary),
                },
                LifespanComponent {
                    born_at_tick: 0,
                    years_lived: 90.0,
                    cap_by_realm: LifespanCapTable::SPIRIT,
                    offline_pause_tick: None,
                },
                LifeRecord::new(char_id),
                Position::new([8.0, 66.0, 8.0]),
                TribulationState::restored(2, 5, 120),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<TribulationFailed>>()
            .send(TribulationFailed { entity, wave: 3 });
        app.update();

        let entity_ref = app.world().entity(entity);
        let cultivation = entity_ref
            .get::<Cultivation>()
            .expect("cultivation should remain attached");
        let meridians = entity_ref
            .get::<MeridianSystem>()
            .expect("meridians should remain attached");
        let wounds = entity_ref
            .get::<Wounds>()
            .expect("wounds should remain attached");
        let lifecycle = entity_ref
            .get::<Lifecycle>()
            .expect("lifecycle should remain attached");
        let registry = entity_ref
            .get::<DeathRegistry>()
            .expect("death registry should remain attached");
        let lifespan = entity_ref
            .get::<LifespanComponent>()
            .expect("lifespan should remain attached");

        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(cultivation.qi_current, 0.0);
        assert_eq!(cultivation.last_qi_zero_at, None);
        assert_eq!(cultivation.pending_material_bonus, 0.0);
        assert_eq!(meridians.opened_count(), Realm::Spirit.required_meridians());
        assert_eq!(cultivation.qi_max, 10.0 + meridians.sum_capacity());
        assert!(wounds.health_current > 0.0);
        assert_eq!(lifecycle.state, LifecycleState::Alive);
        assert_eq!(lifecycle.death_count, 2);
        assert_eq!(lifecycle.last_death_tick, Some(55));
        assert_eq!(registry.death_count, 2);
        assert_eq!(registry.last_death_tick, Some(55));
        assert_eq!(lifespan.years_lived, 90.0);
        assert!(entity_ref.get::<TribulationState>().is_none());

        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Events<DeathInsightRequested>>()
                .len(),
            0
        );
        assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 0);
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_none(),
            "failed tribulation should clear active row"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn intercepted_tribulation_transfers_all_inventory_to_killer() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("intercept-loot-transfer");
        app.insert_resource(settings.clone());
        app.add_event::<DeathEvent>();
        app.add_event::<TribulationSettled>();
        app.add_systems(Update, tribulation_intercept_death_system);

        let victim = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(2),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 2,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: 0,
                    participants: vec!["offline:Victim".to_string()],
                    failed: false,
                    half_step_on_success: false,
                },
                test_inventory(vec![test_item(101), test_item(102)], 7),
            ))
            .id();
        let killer = app
            .world_mut()
            .spawn((
                test_inventory(vec![test_item(201)], 3),
                LifeRecord::new("offline:Killer"),
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: victim,
            cause: "pvp:offline:Killer".to_string(),
            attacker: Some(killer),
            attacker_player_id: Some("offline:Killer".to_string()),
            at_tick: 120,
        });

        app.update();

        let victim_inventory = app
            .world()
            .get::<PlayerInventory>(victim)
            .expect("victim inventory should remain attached");
        assert_eq!(victim_inventory.bone_coins, 0);
        assert!(victim_inventory
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(victim_inventory.equipped.is_empty());
        assert!(victim_inventory.hotbar.iter().all(Option::is_none));

        let killer_inventory = app
            .world()
            .get::<PlayerInventory>(killer)
            .expect("killer inventory should remain attached");
        assert_eq!(killer_inventory.bone_coins, 10);
        let killer_item_ids = killer_inventory
            .containers
            .iter()
            .flat_map(|container| container.items.iter())
            .map(|placed| placed.instance.instance_id)
            .collect::<Vec<_>>();
        assert!(killer_item_ids.contains(&101));
        assert!(killer_item_ids.contains(&102));
        assert!(killer_item_ids.contains(&201));

        let killer_life_record = app
            .world()
            .get::<LifeRecord>(killer)
            .expect("killer life record should remain attached");
        assert!(matches!(
            killer_life_record.biography.last(),
            Some(BiographyEntry::TribulationIntercepted { victim_id, tick })
                if victim_id == "offline:Victim" && *tick == 120
        ));

        assert!(app.world().get::<TribulationState>(victim).is_none());
        let settled = app.world().resource::<Events<TribulationSettled>>();
        let emitted: Vec<_> = settled.get_reader().read(settled).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].result.outcome, DuXuOutcomeV1::Killed);
        assert_eq!(emitted[0].result.killer.as_deref(), Some("offline:Killer"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn attacking_locked_tribulator_records_interceptor_participant() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, record_tribulation_interceptor_system);

        let victim = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Lock,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 0,
                    waves_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: 0,
                    participants: vec!["offline:Victim".to_string()],
                    failed: false,
                    half_step_on_success: false,
                },
            ))
            .id();
        let interceptor = app
            .world_mut()
            .spawn((
                Position::new([12.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Killer".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        for _ in 0..2 {
            app.world_mut().send_event(CombatEvent {
                attacker: interceptor,
                target: victim,
                resolved_at_tick: 120,
                body_part: BodyPart::Chest,
                wound_kind: WoundKind::Cut,
                damage: 12.0,
                contam_delta: 0.0,
                description: "test interception hit".to_string(),
            });
        }
        app.update();

        let state = app
            .world()
            .get::<TribulationState>(victim)
            .expect("tribulation should remain active");
        assert_eq!(
            state.participants,
            vec!["offline:Victim".to_string(), "offline:Killer".to_string()]
        );
    }

    #[test]
    fn attacking_restored_tribulator_preserves_primary_participant() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, record_tribulation_interceptor_system);

        let victim = app
            .world_mut()
            .spawn((
                Position::new([0.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Victim".to_string(),
                    ..Default::default()
                },
                TribulationState::restored(1, 3, 0),
            ))
            .id();
        let interceptor = app
            .world_mut()
            .spawn((
                Position::new([12.0, 66.0, 0.0]),
                Lifecycle {
                    character_id: "offline:Killer".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(CombatEvent {
            attacker: interceptor,
            target: victim,
            resolved_at_tick: 120,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            damage: 12.0,
            contam_delta: 0.0,
            description: "test restored interception hit".to_string(),
        });
        app.update();

        let state = app
            .world()
            .get::<TribulationState>(victim)
            .expect("tribulation should remain active");
        assert_eq!(
            state.participants,
            vec!["offline:Victim".to_string(), "offline:Killer".to_string()]
        );
    }

    #[test]
    fn registered_interceptor_dies_to_aoe_without_failing_tribulation() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 300 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_systems(Update, tribulation_aoe_system);

        app.world_mut().spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 120.0,
                qi_max: 210.0,
                ..Default::default()
            },
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(1),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 300,
                next_wave_tick: 300,
                participants: vec!["offline:Victim".to_string(), "offline:Killer".to_string()],
                failed: false,
                half_step_on_success: false,
            },
        ));
        let interceptor = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 50.0,
                    qi_max: 80.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 1.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: "offline:Killer".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);
        let deaths = app.world().resource::<Events<DeathEvent>>();
        let emitted: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].target, interceptor);
        assert_eq!(emitted[0].cause, "观劫而亡");
        assert_eq!(emitted[0].attacker_player_id, None);
    }

    #[test]
    fn spectator_death_by_tribulation_aoe_is_written_to_life_record() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("spectator-death-biography");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 300 });
        app.add_event::<TribulationFailed>();
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                tribulation_aoe_system,
                death_arbiter_tick.after(tribulation_aoe_system),
            ),
        );

        app.world_mut().spawn((
            Position::new([0.0, 66.0, 0.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 120.0,
                qi_max: 210.0,
                ..Default::default()
            },
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Lifecycle {
                character_id: "offline:Victim".to_string(),
                state: LifecycleState::Alive,
                ..Default::default()
            },
            TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Wave(1),
                epicenter: [0.0, 66.0, 0.0],
                wave_current: 1,
                waves_total: 3,
                started_tick: 0,
                phase_started_tick: 300,
                next_wave_tick: 300,
                participants: vec!["offline:Victim".to_string()],
                failed: false,
                half_step_on_success: false,
            },
        ));
        let spectator = app
            .world_mut()
            .spawn((
                Position::new([20.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 40.0,
                    ..Default::default()
                },
                Wounds {
                    health_current: 1.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:Spectator".to_string(),
                    state: LifecycleState::Alive,
                    fortune_remaining: 1,
                    ..Default::default()
                },
                DeathRegistry::new("offline:Spectator".to_string()),
                LifespanComponent::new(LifespanCapTable::AWAKEN),
                LifeRecord::new("offline:Spectator"),
            ))
            .id();

        app.update();

        let lifecycle = app
            .world()
            .get::<Lifecycle>(spectator)
            .expect("spectator lifecycle should remain attached");
        assert_eq!(lifecycle.state, LifecycleState::NearDeath);
        let life = app
            .world()
            .get::<LifeRecord>(spectator)
            .expect("spectator life record should remain attached");
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::NearDeath { cause, tick }) if cause == "观劫而亡" && *tick == 300
        ));
        assert_eq!(app.world().resource::<Events<TribulationFailed>>().len(), 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn disconnecting_during_tribulation_flees_and_regresses_without_death() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("disconnect-fled");
        let char_id = "offline:Azure";
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: char_id.to_string(),
                wave_current: 1,
                waves_total: 3,
                started_tick: 80,
            },
        )
        .expect("active tribulation should persist before disconnect");

        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 320 });
        app.add_event::<TribulationSettled>();
        app.add_event::<TribulationFled>();
        app.add_systems(Update, abort_du_xu_on_client_removed);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 120.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: char_id.to_string(),
                    state: LifecycleState::Alive,
                    ..Default::default()
                },
                LifeRecord::new(char_id),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Wave(2),
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 1,
                    waves_total: 3,
                    started_tick: 80,
                    phase_started_tick: 280,
                    next_wave_tick: 320,
                    participants: vec![char_id.to_string()],
                    failed: false,
                    half_step_on_success: false,
                },
            ))
            .id();

        app.world_mut().entity_mut(entity).remove::<Client>();
        app.update();

        assert!(app.world().get::<TribulationState>(entity).is_none());
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(cultivation.qi_current, 0.0);
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::TribulationFled { wave: 2, tick: 320 })
        ));

        let settled: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .drain()
            .collect();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].result.outcome, DuXuOutcomeV1::Fled);
        assert_eq!(settled[0].result.waves_survived, 1);
        let fled: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<TribulationFled>>()
            .drain()
            .collect();
        assert_eq!(fled.len(), 1);
        assert_eq!(fled[0].entity, entity);
        assert_eq!(fled[0].tick, 320);
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_none(),
            "fled tribulation should clear active row"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn leaving_lock_radius_flees_and_regresses_without_death() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("boundary-fled");
        let char_id = "offline:Azure";
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: char_id.to_string(),
                wave_current: 1,
                waves_total: 3,
                started_tick: 80,
            },
        )
        .expect("active tribulation should persist before flee");

        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 340 });
        app.add_event::<TribulationSettled>();
        app.add_event::<TribulationFled>();
        app.add_systems(Update, tribulation_escape_boundary_system);

        let entity = app
            .world_mut()
            .spawn((
                Position::new([30.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 160.0,
                    qi_max: 210.0,
                    ..Default::default()
                },
                all_meridians_open(),
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Lifecycle {
                    character_id: char_id.to_string(),
                    state: LifecycleState::Alive,
                    ..Default::default()
                },
                LifeRecord::new(char_id),
                TribulationState {
                    kind: TribulationKind::DuXu,
                    phase: TribulationPhase::Lock,
                    epicenter: [0.0, 66.0, 0.0],
                    wave_current: 1,
                    waves_total: 3,
                    started_tick: 80,
                    phase_started_tick: 300,
                    next_wave_tick: 360,
                    participants: vec![char_id.to_string()],
                    failed: false,
                    half_step_on_success: false,
                },
            ))
            .id();

        app.update();

        assert!(app.world().get::<TribulationState>(entity).is_none());
        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.realm, Realm::Spirit);
        assert_eq!(cultivation.qi_current, 0.0);
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::TribulationFled { wave: 2, tick: 340 })
        ));

        let settled: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<TribulationSettled>>()
            .drain()
            .collect();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].result.outcome, DuXuOutcomeV1::Fled);
        assert_eq!(settled[0].result.waves_survived, 1);
        assert!(
            load_active_tribulation(&settings, char_id)
                .expect("active tribulation query should succeed")
                .is_none(),
            "fled tribulation should clear active row"
        );

        let _ = fs::remove_dir_all(root);
    }
}
