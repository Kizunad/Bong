use valence::prelude::{EventReader, Query, Res, UniqueId};

use crate::combat::anqi_v2::{
    AnqiSkillId, ArmorPierceEvent, CarrierAbrasionEvent, ContainerSwapEvent, EchoFractalEvent,
    MultiShotEvent, QiInjectionEvent,
};
use crate::combat::carrier::{
    CarrierChargedEvent, CarrierImpactEvent, CarrierKind, ProjectileDespawnedEvent,
};
use crate::combat::projectile::ProjectileDespawnReason;
use crate::combat::woliu::entity_wire_id;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::combat_carrier::{
    AbrasionDirectionV1, AnqiContainerKindV1, AnqiSkillKindV1, CarrierAbrasionEventV1,
    CarrierChargedEventV1, CarrierImpactEventV1, CarrierKindV1, ContainerSwapEventV1,
    EchoFractalEventV1, MultiShotEventV1, ProjectileDespawnReasonV1, ProjectileDespawnedEventV1,
    QiInjectionEventV1,
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

pub fn publish_multi_shot_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<MultiShotEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let cone_degrees = event
            .shots
            .iter()
            .map(|shot| shot.yaw_degrees.abs())
            .fold(0.0_f64, f64::max)
            * 2.0;
        let tracking_degrees = event
            .shots
            .iter()
            .map(|shot| shot.tracking_degrees)
            .fold(0.0_f64, f64::max);
        let payload = MultiShotEventV1 {
            caster: entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            carrier_kind: map_carrier_kind(event.carrier_kind),
            projectile_count: event.projectile_count,
            cone_degrees,
            tracking_degrees,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AnqiMultiShot(payload));
    }
}

pub fn publish_qi_injection_events(
    redis: Res<RedisBridgeResource>,
    mut injections: EventReader<QiInjectionEvent>,
    mut armor_pierces: EventReader<ArmorPierceEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in injections.read() {
        let payload = QiInjectionEventV1 {
            caster: entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            target: event
                .target
                .map(|target| entity_wire_id(unique_ids.get(target).ok(), target)),
            skill: map_anqi_skill(event.skill),
            carrier_kind: map_carrier_kind(event.carrier_kind),
            payload_qi: event.outcome.payload_qi,
            wound_qi: event.outcome.wound_qi,
            contamination_qi: event.outcome.contamination_qi,
            overload_ratio: event.outcome.overload_ratio,
            triggers_overload_tear: event.outcome.triggers_overload_tear,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AnqiQiInjection(payload));
    }
    for event in armor_pierces.read() {
        let payload = QiInjectionEventV1 {
            caster: entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            target: event
                .target
                .map(|target| entity_wire_id(unique_ids.get(target).ok(), target)),
            skill: AnqiSkillKindV1::ArmorPierce,
            carrier_kind: map_carrier_kind(event.carrier_kind),
            payload_qi: event.outcome.base_damage,
            wound_qi: event.outcome.effective_damage,
            contamination_qi: 0.0,
            overload_ratio: event.outcome.ignored_defense_ratio,
            triggers_overload_tear: event.outcome.carrier_shatter_probability >= 0.5,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AnqiQiInjection(payload));
    }
}

pub fn publish_echo_fractal_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<EchoFractalEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let payload = EchoFractalEventV1 {
            caster: entity_wire_id(unique_ids.get(event.caster).ok(), event.caster),
            carrier_kind: map_carrier_kind(event.carrier_kind),
            local_qi_density: event.outcome.local_qi_density,
            threshold: event.outcome.threshold,
            echo_count: event.outcome.echo_count,
            damage_per_echo: event.outcome.damage_per_echo,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AnqiEchoFractal(payload));
    }
}

pub fn publish_container_events(
    redis: Res<RedisBridgeResource>,
    mut abrasions: EventReader<CarrierAbrasionEvent>,
    mut swaps: EventReader<ContainerSwapEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in abrasions.read() {
        let payload = CarrierAbrasionEventV1 {
            carrier: entity_wire_id(unique_ids.get(event.carrier).ok(), event.carrier),
            container: map_container_kind(event.container),
            direction: match event.direction {
                crate::qi_physics::AbrasionDirection::Store => AbrasionDirectionV1::Store,
                crate::qi_physics::AbrasionDirection::Draw => AbrasionDirectionV1::Draw,
            },
            lost_qi: event.lost_qi,
            after_qi: event.after_qi,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AnqiCarrierAbrasion(payload));
    }
    for event in swaps.read() {
        let payload = ContainerSwapEventV1 {
            carrier: entity_wire_id(unique_ids.get(event.carrier).ok(), event.carrier),
            from: map_container_kind(event.from),
            to: map_container_kind(event.to),
            switching_until_tick: event.switching_until_tick,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AnqiContainerSwap(payload));
    }
}

fn map_carrier_kind(kind: CarrierKind) -> CarrierKindV1 {
    match kind {
        CarrierKind::BoneChip => CarrierKindV1::BoneChip,
        CarrierKind::YibianShougu => CarrierKindV1::YibianShougu,
        CarrierKind::LingmuArrow => CarrierKindV1::LingmuArrow,
        CarrierKind::DyedBone => CarrierKindV1::DyedBone,
        CarrierKind::FenglingheBone => CarrierKindV1::FenglingheBone,
        CarrierKind::ShangguBone => CarrierKindV1::ShangguBone,
    }
}

fn map_anqi_skill(skill: AnqiSkillId) -> AnqiSkillKindV1 {
    match skill {
        AnqiSkillId::SingleSnipe => AnqiSkillKindV1::SingleSnipe,
        AnqiSkillId::MultiShot => AnqiSkillKindV1::MultiShot,
        AnqiSkillId::SoulInject => AnqiSkillKindV1::SoulInject,
        AnqiSkillId::ArmorPierce => AnqiSkillKindV1::ArmorPierce,
        AnqiSkillId::EchoFractal => AnqiSkillKindV1::EchoFractal,
    }
}

fn map_container_kind(kind: crate::qi_physics::AnqiContainerKind) -> AnqiContainerKindV1 {
    match kind {
        crate::qi_physics::AnqiContainerKind::HandSlot => AnqiContainerKindV1::HandSlot,
        crate::qi_physics::AnqiContainerKind::Quiver => AnqiContainerKindV1::Quiver,
        crate::qi_physics::AnqiContainerKind::PocketPouch => AnqiContainerKindV1::PocketPouch,
        crate::qi_physics::AnqiContainerKind::Fenglinghe => AnqiContainerKindV1::Fenglinghe,
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
