use valence::prelude::{EventReader, Query, Res, UniqueId};

use crate::combat::dugu_v2::{
    events::{DuguSkillVisual, TaintTier},
    physics::reveal_probability,
    EclipseNeedleEvent, PenetrateChainEvent, ReverseTriggeredEvent, SelfCureProgressEvent,
    ShroudActivatedEvent,
};
use crate::combat::woliu::entity_wire_id;
use crate::cultivation::components::{Cultivation, Realm};
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
    cultivations: Query<&Cultivation>,
) {
    for event in events.read() {
        let reveal = reveal_for(
            &cultivations,
            event.caster,
            Some(event.target),
            event.target_realm,
        );
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
            reveal,
            event.visual,
        );
        let _ = redis.tx_outbound.send(RedisOutbound::DuguV2Cast(payload));
    }
}

pub fn publish_dugu_v2_penetrate_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<PenetrateChainEvent>,
    unique_ids: Query<&UniqueId>,
    cultivations: Query<&Cultivation>,
) {
    for event in events.read() {
        let target_realm = cultivations
            .get(event.target)
            .map(|cultivation| cultivation.realm)
            .unwrap_or(Realm::Awaken);
        let payload = cast_payload(
            entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            Some(entity_wire_id(
                unique_ids.get(event.target).ok(),
                event.target,
            )),
            DuguV2SkillIdV1::Penetrate,
            event.tick,
            Some(DuguTaintTierV1::Permanent),
            0.0,
            0.0,
            0.0,
            event.permanent_decay_rate_per_min,
            0.0,
            reveal_for(
                &cultivations,
                event.caster,
                Some(event.target),
                target_realm,
            ),
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

fn reveal_for(
    cultivations: &Query<&Cultivation>,
    caster: valence::prelude::Entity,
    target: Option<valence::prelude::Entity>,
    target_realm: Realm,
) -> f32 {
    let caster_realm = cultivations
        .get(caster)
        .map(|cultivation| cultivation.realm)
        .unwrap_or(Realm::Awaken);
    let distance = target.map(|_| 1.0).unwrap_or(20.0);
    reveal_probability(caster_realm, 0.0, distance, target_realm)
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
