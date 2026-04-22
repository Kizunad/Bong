//! 化虚渡劫（plan §3.2）。
//!
//! Spirit → Void 的唯一通路，流程：
//!   1. 玩家 `InitiateXuhuaTribulation` → 进入 TribulationState
//!   2. 全服广播（由 network 层消费 `TribulationAnnounce`）
//!   3. calamity agent 生成天劫脚本（多波次），本 plan 接收 `TribulationWave`
//!      事件并让战斗 plan 施加伤害（此处不实现）
//!   4. 扛过所有波次 → realm = Void；任一波次死亡 → `TribulationFailure`
//!
//! P1/P5：本文件只定义状态机 + 事件；真实天劫伤害由战斗 plan 实施。

use valence::prelude::{
    bevy_ecs, Component, Entity, Event, EventReader, EventWriter, Position, Query,
};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::skill::components::SkillId;
use crate::skill::events::SkillCapChanged;

use super::breakthrough::skill_cap_for_realm;
use super::components::{Cultivation, MeridianSystem, Realm};
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use crate::persistence::{
    complete_tribulation_ascension, delete_active_tribulation, persist_active_tribulation,
    ActiveTribulationRecord, PersistenceSettings,
};

#[derive(Debug, Clone, Component)]
pub struct TribulationState {
    pub wave_current: u32,
    pub waves_total: u32,
    pub started_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct InitiateXuhuaTribulation {
    pub entity: Entity,
    pub waves_total: u32,
    pub started_tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct TribulationAnnounce {
    pub entity: Entity,
}

/// 单波次通过（由战斗 plan 发送）。
#[derive(Debug, Clone, Event)]
pub struct TribulationWaveCleared {
    pub entity: Entity,
    pub wave: u32,
}

/// 渡劫失败（战斗 plan 在玩家死亡时发送，或本 plan 检测 qi+health 双零）。
#[derive(Debug, Clone, Event)]
pub struct TribulationFailed {
    pub entity: Entity,
    pub wave: u32,
}

pub fn start_tribulation_system(
    settings: valence::prelude::Res<PersistenceSettings>,
    mut events: EventReader<InitiateXuhuaTribulation>,
    mut announce: EventWriter<TribulationAnnounce>,
    mut players: Query<(&Cultivation, &crate::combat::components::Lifecycle)>,
    mut commands: valence::prelude::Commands,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for ev in events.read() {
        if let Ok((c, lifecycle)) = players.get_mut(ev.entity) {
            if c.realm != Realm::Spirit {
                tracing::warn!(
                    "[bong][cultivation] {:?} tried to tribulate from {:?}, rejected",
                    ev.entity,
                    c.realm
                );
                continue;
            }
            let state = TribulationState {
                wave_current: 0,
                waves_total: ev.waves_total,
                started_tick: ev.started_tick,
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
            announce.send(TribulationAnnounce { entity: ev.entity });
            tracing::info!(
                "[bong][cultivation] {:?} initiated tribulation ({} waves)",
                ev.entity,
                ev.waves_total
            );
            // plan-particle-system-v1 §4.4：渡劫开场一道预警雷。
            if let Ok(pos) = positions.get(ev.entity) {
                let p = pos.get();
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
}

pub fn tribulation_wave_system(
    settings: valence::prelude::Res<PersistenceSettings>,
    mut cleared: EventReader<TribulationWaveCleared>,
    mut players: Query<(
        &mut Cultivation,
        &mut TribulationState,
        &MeridianSystem,
        &crate::combat::components::Lifecycle,
    )>,
    mut commands: valence::prelude::Commands,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
) {
    for ev in cleared.read() {
        if let Ok((mut c, mut state, _, lifecycle)) = players.get_mut(ev.entity) {
            state.wave_current = state.wave_current.max(ev.wave);
            if state.wave_current >= state.waves_total {
                // 渡劫成功
                c.realm = Realm::Void;
                c.qi_max *= super::breakthrough::qi_max_multiplier(Realm::Void);
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
                commands.entity(ev.entity).remove::<TribulationState>();
                tracing::info!(
                    "[bong][cultivation] {:?} ASCENDED to Void realm after {} waves",
                    ev.entity,
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

pub fn tribulation_failure_system(
    settings: valence::prelude::Res<PersistenceSettings>,
    mut failed: EventReader<TribulationFailed>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    lifecycles: Query<&crate::combat::components::Lifecycle>,
    mut commands: valence::prelude::Commands,
) {
    for ev in failed.read() {
        deaths.send(CultivationDeathTrigger {
            entity: ev.entity,
            cause: CultivationDeathCause::TribulationFailure,
            context: serde_json::json!({
                "wave": ev.wave,
                "no_fortune": true,
            }),
        });
        if let Ok(lifecycle) = lifecycles.get(ev.entity) {
            if let Err(error) =
                delete_active_tribulation(&settings, lifecycle.character_id.as_str())
            {
                tracing::warn!(
                    "[bong][cultivation] failed to delete failed active tribulation for {:?}: {error}",
                    ev.entity,
                );
            }
        }
        commands.entity(ev.entity).remove::<TribulationState>();
    }
}
