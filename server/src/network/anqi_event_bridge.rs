use valence::prelude::{EventReader, Query, Res, UniqueId};

use crate::combat::carrier::{
    CarrierChargedEvent, CarrierImpactEvent, CarrierKind, ProjectileDespawnedEvent,
};
use crate::combat::projectile::ProjectileDespawnReason;
use crate::combat::woliu::entity_wire_id;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::combat_carrier::{
    CarrierChargedEventV1, CarrierImpactEventV1, CarrierKindV1, ProjectileDespawnReasonV1,
    ProjectileDespawnedEventV1,
};
use crate::schema::cultivation::color_kind_to_string;

pub fn publish_carrier_charged_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<CarrierChargedEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = CarrierChargedEventV1 {
            carrier: entity_wire_id(unique_ids.get(event.carrier).ok(), event.carrier),
            instance_id: event.instance_id,
            qi_amount: event.qi_amount,
            qi_color: color_kind_to_string(event.qi_color).to_string(),
            full_charge: event.full_charge,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::CarrierCharged(payload));
    }
}

pub fn publish_carrier_impact_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<CarrierImpactEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = CarrierImpactEventV1 {
            attacker: entity_wire_id(unique_ids.get(event.attacker).ok(), event.attacker),
            target: entity_wire_id(unique_ids.get(event.target).ok(), event.target),
            carrier_kind: map_carrier_kind(event.carrier_kind),
            hit_distance: event.hit_distance,
            sealed_qi_initial: event.sealed_qi_initial,
            hit_qi: event.hit_qi,
            wound_damage: event.wound_damage,
            contam_amount: event.contam_amount,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::CarrierImpact(payload));
    }
}

pub fn publish_projectile_despawned_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<ProjectileDespawnedEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = ProjectileDespawnedEventV1 {
            owner: event
                .owner
                .map(|owner| entity_wire_id(unique_ids.get(owner).ok(), owner)),
            projectile: entity_wire_id(unique_ids.get(event.projectile).ok(), event.projectile),
            reason: map_despawn_reason(event.reason),
            distance: event.distance,
            qi_evaporated: event.qi_evaporated,
            residual_qi: event.residual_qi,
            pos: event.pos,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ProjectileDespawned(payload));
    }
}

fn map_carrier_kind(kind: CarrierKind) -> CarrierKindV1 {
    match kind {
        CarrierKind::YibianShougu => CarrierKindV1::YibianShougu,
    }
}

fn map_despawn_reason(reason: ProjectileDespawnReason) -> ProjectileDespawnReasonV1 {
    match reason {
        ProjectileDespawnReason::HitTarget => ProjectileDespawnReasonV1::HitTarget,
        ProjectileDespawnReason::HitBlock => ProjectileDespawnReasonV1::HitBlock,
        ProjectileDespawnReason::OutOfRange => ProjectileDespawnReasonV1::OutOfRange,
        ProjectileDespawnReason::NaturalDecay => ProjectileDespawnReasonV1::NaturalDecay,
    }
}
