use valence::prelude::{EventReader, Query, Res, UniqueId};

use crate::combat::dugu_v2::{
    events::{DuguSkillVisual, TaintTier},
    EclipseNeedleEvent, PenetrateChainEvent, ReverseTriggeredEvent, SelfCureProgressEvent,
    ShroudActivatedEvent,
};
use crate::combat::woliu::entity_wire_id;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::dugu_v2::{
    DuguReverseTriggeredV1, DuguSelfCureProgressV1, DuguTaintTierV1, DuguV2SkillCastV1,
    DuguV2SkillIdV1,
};

pub fn publish_dugu_v2_eclipse_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<EclipseNeedleEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = cast_payload(
            entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            Some(entity_wire_id(
                unique_ids.get(event.target).ok(),
                event.target,
            )),
            DuguV2SkillIdV1::Eclipse,
            event.tick,
            Some(taint_tier_payload(event.tier)),
            event.hp_loss,
            event.qi_loss,
            event.qi_max_loss,
            event.permanent_decay_rate_per_min,
            event.returned_zone_qi,
            event.reveal_probability,
            event.visual,
        );
        let _ = redis.tx_outbound.send(RedisOutbound::DuguV2Cast(payload));
    }
}

pub fn publish_dugu_v2_penetrate_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<PenetrateChainEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = cast_payload(
            entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            Some(entity_wire_id(
                unique_ids.get(event.target).ok(),
                event.target,
            )),
            DuguV2SkillIdV1::Penetrate,
            event.tick,
            Some(taint_tier_payload(event.taint_tier)),
            0.0,
            0.0,
            0.0,
            event.permanent_decay_rate_per_min,
            0.0,
            event.reveal_probability,
            event.visual,
        );
        let _ = redis.tx_outbound.send(RedisOutbound::DuguV2Cast(payload));
    }
}

pub fn publish_dugu_v2_shroud_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<ShroudActivatedEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = cast_payload(
            entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            None,
            DuguV2SkillIdV1::Shroud,
            event.tick,
            None,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            (1.0 - event.strength).clamp(0.0, 1.0),
            event.visual,
        );
        let _ = redis.tx_outbound.send(RedisOutbound::DuguV2Cast(payload));
    }
}

pub fn publish_dugu_v2_self_cure_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<SelfCureProgressEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = DuguSelfCureProgressV1 {
            caster: entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            hours_used: event.hours_used,
            daily_hours_after: event.daily_hours_after,
            gain_percent: event.gain_percent,
            insidious_color_percent: event.insidious_color_percent,
            morphology_percent: event.morphology_percent,
            self_revealed: event.self_revealed,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::DuguV2SelfCure(payload));
    }
}

pub fn publish_dugu_v2_reverse_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<ReverseTriggeredEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = DuguReverseTriggeredV1 {
            caster: entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            affected_targets: event.affected_targets,
            burst_damage: event.burst_damage,
            returned_zone_qi: event.returned_zone_qi,
            juebi_delay_ticks: event.juebi_delay_ticks,
            center: [event.center.x, event.center.y, event.center.z],
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::DuguV2Reverse(payload));
    }
}

#[allow(clippy::too_many_arguments)]
fn cast_payload(
    caster: String,
    target: Option<String>,
    skill: DuguV2SkillIdV1,
    tick: u64,
    taint_tier: Option<DuguTaintTierV1>,
    hp_loss: f32,
    qi_loss: f32,
    qi_max_loss: f32,
    permanent_decay_rate_per_min: f32,
    returned_zone_qi: f32,
    reveal_probability: f32,
    visual: DuguSkillVisual,
) -> DuguV2SkillCastV1 {
    DuguV2SkillCastV1 {
        caster,
        target,
        skill,
        tick,
        taint_tier,
        hp_loss,
        qi_loss,
        qi_max_loss,
        permanent_decay_rate_per_min,
        returned_zone_qi,
        reveal_probability,
        animation_id: visual.animation_id.to_string(),
        particle_id: visual.particle_id.to_string(),
        sound_recipe_id: visual.sound_recipe_id.to_string(),
        icon_texture: visual.icon_texture.to_string(),
    }
}

fn taint_tier_payload(tier: TaintTier) -> DuguTaintTierV1 {
    match tier {
        TaintTier::Immediate => DuguTaintTierV1::Immediate,
        TaintTier::Temporary => DuguTaintTierV1::Temporary,
        TaintTier::Permanent => DuguTaintTierV1::Permanent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    use crate::combat::dugu_v2::events::{DuguSkillVisual, TaintTier};
    use crate::cultivation::components::Realm;
    use crate::network::redis_bridge::RedisOutbound;

    fn app_with_bridge() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<EclipseNeedleEvent>();
        app.add_event::<PenetrateChainEvent>();
        app.add_systems(
            Update,
            (
                publish_dugu_v2_eclipse_events,
                publish_dugu_v2_penetrate_events,
            ),
        );
        (app, rx_outbound)
    }

    fn visual() -> DuguSkillVisual {
        DuguSkillVisual {
            animation_id: "bong:dugu_needle_throw",
            particle_id: "bong:dugu_taint_pulse",
            sound_recipe_id: "dugu_needle_hiss",
            hud_hint: "蚀针",
            icon_texture: "bong:textures/gui/skill/dugu_eclipse.png",
        }
    }

    #[test]
    fn eclipse_payload_uses_resolved_reveal_probability() {
        let (mut app, rx_outbound) = app_with_bridge();
        let caster = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(EclipseNeedleEvent {
            caster,
            target,
            target_realm: Realm::Solidify,
            tier: TaintTier::Temporary,
            injected_qi: 25.0,
            hp_loss: 15.0,
            qi_loss: 25.0,
            qi_max_loss: 3.0,
            permanent_decay_rate_per_min: 0.0,
            returned_zone_qi: 24.75,
            reveal_probability: 0.0042,
            tick: 42,
            visual: visual(),
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("dugu eclipse payload should publish")
        {
            RedisOutbound::DuguV2Cast(payload) => {
                assert_eq!(payload.skill, DuguV2SkillIdV1::Eclipse);
                assert_eq!(payload.taint_tier, Some(DuguTaintTierV1::Temporary));
                assert!((payload.reveal_probability - 0.0042).abs() < f32::EPSILON);
            }
            other => panic!("expected dugu v2 cast outbound, got {other:?}"),
        }
    }

    #[test]
    fn penetrate_payload_preserves_event_taint_tier() {
        let (mut app, rx_outbound) = app_with_bridge();
        let caster = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(PenetrateChainEvent {
            caster,
            target,
            taint_tier: TaintTier::Temporary,
            multiplier: 2.5,
            affected_targets: 1,
            permanent_decay_rate_per_min: 0.0,
            reveal_probability: 0.007,
            tick: 99,
            visual: visual(),
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("dugu penetrate payload should publish")
        {
            RedisOutbound::DuguV2Cast(payload) => {
                assert_eq!(payload.skill, DuguV2SkillIdV1::Penetrate);
                assert_eq!(payload.taint_tier, Some(DuguTaintTierV1::Temporary));
                assert!((payload.reveal_probability - 0.007).abs() < f32::EPSILON);
            }
            other => panic!("expected dugu v2 cast outbound, got {other:?}"),
        }
    }
}
