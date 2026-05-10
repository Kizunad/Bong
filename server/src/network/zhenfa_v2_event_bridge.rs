use valence::prelude::{EventReader, Res};

use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::zhenfa_v2::{ZhenfaArrayKindV2, ZhenfaV2EventKind, ZhenfaV2EventV1};
use crate::zhenfa::{
    ArrayBreakthroughEvent, ArrayDecayEvent, DeceiveHeavenEvent, DeceiveHeavenExposedEvent,
    IllusionArrayDeployEvent, LingArrayDeployEvent, WardArrayDeployEvent, ZhenfaKind,
};

#[allow(clippy::too_many_arguments)]
pub fn publish_zhenfa_v2_events(
    redis: Option<Res<RedisBridgeResource>>,
    mut ward: EventReader<WardArrayDeployEvent>,
    mut ling: EventReader<LingArrayDeployEvent>,
    mut deceive: EventReader<DeceiveHeavenEvent>,
    mut deceive_exposed: EventReader<DeceiveHeavenExposedEvent>,
    mut illusion: EventReader<IllusionArrayDeployEvent>,
    mut decay: EventReader<ArrayDecayEvent>,
    mut breakthrough: EventReader<ArrayBreakthroughEvent>,
) {
    let Some(redis) = redis else {
        return;
    };

    for event in ward.read() {
        let mut payload = deploy_payload(
            event.array_id,
            ZhenfaArrayKindV2::ShrineWard,
            event.owner,
            event.pos,
            event.placed_at_tick,
        );
        payload.radius = Some(f64::from(event.radius));
        send_zhenfa_v2_event(&redis, payload);
    }
    for event in ling.read() {
        let mut payload = deploy_payload(
            event.array_id,
            ZhenfaArrayKindV2::Lingju,
            event.owner,
            event.pos,
            event.placed_at_tick,
        );
        payload.radius = Some(f64::from(event.radius));
        payload.density_multiplier = Some(event.density_multiplier);
        payload.tiandao_gaze_weight = Some(event.tiandao_gaze_weight);
        send_zhenfa_v2_event(&redis, payload);
    }
    for event in deceive.read() {
        let mut payload = deploy_payload(
            event.array_id,
            ZhenfaArrayKindV2::DeceiveHeaven,
            event.owner,
            event.pos,
            event.placed_at_tick,
        );
        payload.reveal_chance_per_tick = Some(event.reveal_chance_per_tick);
        payload.self_weight_multiplier = Some(event.self_weight_multiplier);
        payload.target_weight_multiplier = Some(event.target_weight_multiplier);
        send_zhenfa_v2_event(&redis, payload);
    }
    for event in deceive_exposed.read() {
        let mut payload = event_payload(
            ZhenfaV2EventKind::DeceiveHeavenExposed,
            event.array_id,
            ZhenfaArrayKindV2::DeceiveHeaven,
            event.owner,
            event.pos,
            event.exposed_at_tick,
        );
        payload.reveal_chance_per_tick = Some(event.reveal_chance_per_tick);
        payload.self_weight_multiplier = Some(event.self_weight_multiplier);
        payload.target_weight_multiplier = Some(event.target_weight_multiplier);
        send_zhenfa_v2_event(&redis, payload);
    }
    for event in illusion.read() {
        let mut payload = deploy_payload(
            event.array_id,
            ZhenfaArrayKindV2::Illusion,
            event.owner,
            event.pos,
            event.placed_at_tick,
        );
        payload.reveal_threshold = Some(event.reveal_threshold);
        send_zhenfa_v2_event(&redis, payload);
    }
    for event in decay.read() {
        let payload = event_payload(
            ZhenfaV2EventKind::Decay,
            event.array_id,
            map_kind(event.kind),
            event.owner,
            event.pos,
            event.decayed_at_tick,
        );
        send_zhenfa_v2_event(&redis, payload);
    }
    for event in breakthrough.read() {
        let mut payload = event_payload(
            ZhenfaV2EventKind::Breakthrough,
            event.array_id,
            map_kind(event.kind),
            event.owner,
            event.pos,
            event.broken_at_tick,
        );
        payload.force_break = Some(event.force_break);
        send_zhenfa_v2_event(&redis, payload);
    }
}

fn send_zhenfa_v2_event(redis: &RedisBridgeResource, payload: ZhenfaV2EventV1) {
    let array_id = payload.array_id;
    let event = payload.event;
    let kind = payload.kind;
    if let Err(error) = redis
        .tx_outbound
        .send(RedisOutbound::ZhenfaV2Event(payload))
    {
        tracing::warn!(
            ?error,
            array_id,
            ?event,
            ?kind,
            "[bong][zhenfa] failed to queue zhenfa v2 redis event"
        );
    }
}

fn deploy_payload(
    array_id: u64,
    kind: ZhenfaArrayKindV2,
    owner: valence::prelude::Entity,
    pos: [i32; 3],
    tick: u64,
) -> ZhenfaV2EventV1 {
    event_payload(ZhenfaV2EventKind::Deploy, array_id, kind, owner, pos, tick)
}

fn event_payload(
    event: ZhenfaV2EventKind,
    array_id: u64,
    kind: ZhenfaArrayKindV2,
    owner: valence::prelude::Entity,
    pos: [i32; 3],
    tick: u64,
) -> ZhenfaV2EventV1 {
    let mut payload = ZhenfaV2EventV1::deploy(
        array_id,
        kind,
        format!("entity_bits:{}", owner.to_bits()),
        pos,
        tick,
    );
    payload.event = event;
    payload
}

fn map_kind(kind: ZhenfaKind) -> ZhenfaArrayKindV2 {
    match kind {
        ZhenfaKind::Trap => ZhenfaArrayKindV2::Trap,
        ZhenfaKind::Ward => ZhenfaArrayKindV2::Ward,
        ZhenfaKind::ShrineWard => ZhenfaArrayKindV2::ShrineWard,
        ZhenfaKind::Lingju => ZhenfaArrayKindV2::Lingju,
        ZhenfaKind::DeceiveHeaven => ZhenfaArrayKindV2::DeceiveHeaven,
        ZhenfaKind::Illusion => ZhenfaArrayKindV2::Illusion,
    }
}
