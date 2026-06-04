use valence::prelude::{EventReader, Query, Res, UniqueId, Username};

use crate::combat::woliu::{
    projectile_drained_payload, vortex_backfire_payload, ProjectileQiDrainedEvent,
    VortexBackfireEvent,
};
use crate::combat::woliu_v2::erosion::VoidErosionAdvanceEvent;
use crate::combat::woliu_v2::{
    BackfireCauseV2, BackfireLevel, TurbulenceFieldSpawned, VortexBackfireEventV2, VortexCastEvent,
    WoliuSkillId,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::woliu_erosion::{VoidErosionEventV1, VoidErosionStageV1};
use crate::schema::woliu_v2::{
    TurbulenceFieldV1, WoliuBackfireLevelV1, WoliuBackfireV1, WoliuSkillCastV1, WoliuSkillIdV1,
};

pub fn publish_woliu_backfire_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<VortexBackfireEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = vortex_backfire_payload(event, unique_ids.get(event.caster).ok());
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::VortexBackfire(payload));
    }
}

pub fn publish_projectile_qi_drained_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<ProjectileQiDrainedEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = projectile_drained_payload(
            event,
            unique_ids.get(event.field_caster).ok(),
            unique_ids.get(event.projectile).ok(),
            event.owner.and_then(|owner| unique_ids.get(owner).ok()),
        );
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ProjectileQiDrained(payload));
    }
}

pub fn publish_woliu_v2_cast_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<VortexCastEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let caster =
            crate::combat::woliu::entity_wire_id(unique_ids.get(event.caster).ok(), event.caster);
        let payload = WoliuSkillCastV1 {
            caster,
            skill: woliu_skill_payload(event.skill),
            tick: event.tick,
            lethal_radius: event.lethal_radius,
            influence_radius: event.influence_radius,
            turbulence_radius: event.turbulence_radius,
            absorbed_qi: event.absorbed_qi,
            swirl_qi: event.swirl_qi,
            animation_id: event.visual.animation_id.to_string(),
            particle_id: event.visual.particle_id.to_string(),
            sound_recipe_id: event.visual.sound_recipe_id.to_string(),
            icon_texture: event.visual.icon_texture.to_string(),
        };
        let _ = redis.tx_outbound.send(RedisOutbound::WoliuV2Cast(payload));
    }
}

pub fn publish_woliu_v2_backfire_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<VortexBackfireEventV2>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = WoliuBackfireV1 {
            caster: crate::combat::woliu::entity_wire_id(
                unique_ids.get(event.caster).ok(),
                event.caster,
            ),
            skill: woliu_skill_payload(event.skill),
            level: backfire_level_payload(event.level),
            cause: backfire_cause_wire(event.cause).to_string(),
            overflow_qi: event.overflow_qi,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::WoliuV2Backfire(payload));
    }
}

pub fn publish_woliu_v2_turbulence_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<TurbulenceFieldSpawned>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = TurbulenceFieldV1 {
            caster: crate::combat::woliu::entity_wire_id(
                unique_ids.get(event.caster).ok(),
                event.caster,
            ),
            skill: woliu_skill_payload(event.skill),
            center: [event.center.x, event.center.y, event.center.z],
            radius: event.radius,
            intensity: event.intensity,
            swirl_qi: event.swirl_qi,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::WoliuV2Turbulence(payload));
    }
}

fn woliu_skill_payload(skill: WoliuSkillId) -> WoliuSkillIdV1 {
    match skill {
        WoliuSkillId::Hold => WoliuSkillIdV1::Hold,
        WoliuSkillId::Burst => WoliuSkillIdV1::Burst,
        WoliuSkillId::Mouth => WoliuSkillIdV1::Mouth,
        WoliuSkillId::Pull => WoliuSkillIdV1::Pull,
        WoliuSkillId::Heart => WoliuSkillIdV1::Heart,
        WoliuSkillId::VacuumPalm => WoliuSkillIdV1::VacuumPalm,
        WoliuSkillId::VortexShield => WoliuSkillIdV1::VortexShield,
        WoliuSkillId::VacuumLock => WoliuSkillIdV1::VacuumLock,
        WoliuSkillId::VortexResonance => WoliuSkillIdV1::VortexResonance,
        WoliuSkillId::TurbulenceBurst => WoliuSkillIdV1::TurbulenceBurst,
        WoliuSkillId::AmbientVortex => WoliuSkillIdV1::AmbientVortex,
        WoliuSkillId::VoidVortex => WoliuSkillIdV1::VoidVortex,
        WoliuSkillId::SwallowingVortex => WoliuSkillIdV1::SwallowingVortex,
        WoliuSkillId::VortexEcho => WoliuSkillIdV1::VortexEcho,
        WoliuSkillId::VoidCore => WoliuSkillIdV1::VoidCore,
    }
}

fn backfire_level_payload(level: BackfireLevel) -> WoliuBackfireLevelV1 {
    match level {
        BackfireLevel::Sensation => WoliuBackfireLevelV1::Sensation,
        BackfireLevel::MicroTear => WoliuBackfireLevelV1::MicroTear,
        BackfireLevel::Torn => WoliuBackfireLevelV1::Torn,
        BackfireLevel::Severed => WoliuBackfireLevelV1::Severed,
    }
}

fn backfire_cause_wire(cause: BackfireCauseV2) -> &'static str {
    match cause {
        BackfireCauseV2::MeridianOverflow => "meridian_overflow",
        BackfireCauseV2::TsyNegativeField => "tsy_negative_field",
        BackfireCauseV2::VoidHeartTribulation => "void_heart_tribulation",
    }
}

// ────────────────────────────────────────────────────────
// plan-combat-skill-feedback-bridges-v1 P3 — 虚蚀阶段推进 → agent 叙事桥
// ────────────────────────────────────────────────────────

/// `VoidErosionAdvanceEvent` → `bong:void_erosion_event` Redis publish。
///
/// 仿 `publish_woliu_v2_backfire_events` 结构；读取 `VoidErosion.cumulative_erosion`
/// 以填充 payload（如实体已消亡则跳过）。
///
/// entity_id 格式：`"offline:{username}"`（有 Username 组件时）或 `"char:{bits}"`（NPC/无名实体）。
/// 仿 meridian_severed_emit.rs:90 的 resolve_entity_id 逻辑，保证 agent narration 路由
/// normalize_player_target 能命中真实玩家（UUID 格式永不命中）。
///
/// TODO(P3-ext): emit_echo_replay_vfx - ScheduledEcho tick system pending
pub fn publish_void_erosion_advance_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<VoidErosionAdvanceEvent>,
    usernames: Query<&Username>,
    erosion_query: Query<&crate::combat::woliu_v2::erosion::VoidErosion>,
    clock: Res<crate::combat::CombatClock>,
) {
    for event in events.read() {
        let entity_id = usernames
            .get(event.entity)
            .map(|u| format!("offline:{}", u.0))
            .unwrap_or_else(|_| format!("char:{}", event.entity.to_bits()));
        let cumulative_erosion = erosion_query
            .get(event.entity)
            .map(|e| e.cumulative_erosion)
            .unwrap_or(0.0);
        let payload = VoidErosionEventV1 {
            entity: entity_id,
            from_stage: void_erosion_stage_v1(event.from),
            to_stage: void_erosion_stage_v1(event.to),
            cumulative_erosion,
            server_tick: clock.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::VoidErosionEvent(payload));
    }
}

fn void_erosion_stage_v1(
    stage: crate::combat::woliu_v2::erosion::VoidErosionStage,
) -> VoidErosionStageV1 {
    use crate::combat::woliu_v2::erosion::VoidErosionStage;
    match stage {
        VoidErosionStage::None => VoidErosionStageV1::None,
        VoidErosionStage::LowPressure => VoidErosionStageV1::LowPressure,
        VoidErosionStage::VoidShadow => VoidErosionStageV1::VoidShadow,
        VoidErosionStage::EchoBody => VoidErosionStageV1::EchoBody,
        VoidErosionStage::VoidEroded => VoidErosionStageV1::VoidEroded,
    }
}

// ────────────────────────────────────────────────────────────────────────────
// P3 回归测试 — publish_void_erosion_advance_events entity_id 格式
// ────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    use crate::combat::woliu_v2::erosion::{
        VoidErosion, VoidErosionAdvanceEvent, VoidErosionStage,
    };
    use crate::network::RedisBridgeResource;

    fn app_with_bridge() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(crate::combat::CombatClock::default());
        app.add_event::<VoidErosionAdvanceEvent>();
        app.add_systems(Update, publish_void_erosion_advance_events);
        (app, rx_outbound)
    }

    /// 有 Username 组件的实体 → entity_id 必须写成 `"offline:{username}"`,
    /// 保证 agent narration 路由 `normalize_player_target` 能命中真实玩家。
    ///
    /// 若将来 Username 组件漏挂，publish_void_erosion_advance_events 会静默降回
    /// `"char:{bits}"` 而 CI 不红 —— 本测试锁住该回归。
    #[test]
    fn publishes_offline_username_entity_id_for_player() {
        let (mut app, rx_outbound) = app_with_bridge();

        let entity = app
            .world_mut()
            .spawn((
                Username("Azure".to_string()),
                VoidErosion {
                    cumulative_erosion: 55.0,
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(VoidErosionAdvanceEvent {
            entity,
            from: VoidErosionStage::LowPressure,
            to: VoidErosionStage::VoidShadow,
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("publish_void_erosion_advance_events should emit one outbound message")
        {
            RedisOutbound::VoidErosionEvent(payload) => {
                assert_eq!(
                    payload.entity, "offline:Azure",
                    "player entity_id must be 'offline:{{username}}' so agent \
                     normalize_player_target routes correctly; got {:?}",
                    payload.entity,
                );
                assert_eq!(
                    payload.from_stage,
                    crate::schema::woliu_erosion::VoidErosionStageV1::LowPressure,
                    "from_stage must reflect event.from"
                );
                assert_eq!(
                    payload.to_stage,
                    crate::schema::woliu_erosion::VoidErosionStageV1::VoidShadow,
                    "to_stage must reflect event.to"
                );
                assert!(
                    (payload.cumulative_erosion - 55.0).abs() < f64::EPSILON,
                    "cumulative_erosion must come from VoidErosion component, got {}",
                    payload.cumulative_erosion,
                );
            }
            other => panic!("expected RedisOutbound::VoidErosionEvent, got {other:?}"),
        }

        assert!(
            rx_outbound.try_recv().is_err(),
            "exactly one outbound message expected per event"
        );
    }

    /// 无 Username 组件的实体（NPC / 未注册实体）→ entity_id 回退到 `"char:{bits}"`。
    /// 确保回退路径不会 panic 且格式符合 IPC 约定。
    #[test]
    fn fallback_char_bits_entity_id_for_npc() {
        let (mut app, rx_outbound) = app_with_bridge();

        let entity = app
            .world_mut()
            .spawn(VoidErosion {
                cumulative_erosion: 10.0,
                ..Default::default()
            })
            .id();
        let expected_prefix = format!("char:{}", entity.to_bits());

        app.world_mut().send_event(VoidErosionAdvanceEvent {
            entity,
            from: VoidErosionStage::None,
            to: VoidErosionStage::LowPressure,
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("publish_void_erosion_advance_events should emit for NPC entity")
        {
            RedisOutbound::VoidErosionEvent(payload) => {
                assert_eq!(
                    payload.entity, expected_prefix,
                    "NPC without Username must produce 'char:{{bits}}' entity_id; \
                     got {:?}",
                    payload.entity,
                );
            }
            other => panic!("expected RedisOutbound::VoidErosionEvent for NPC, got {other:?}"),
        }
    }
}
