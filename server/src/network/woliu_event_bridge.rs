use valence::prelude::{EventReader, Query, Res, UniqueId};

use crate::combat::woliu::{
    projectile_drained_payload, vortex_backfire_payload, ProjectileQiDrainedEvent,
    VortexBackfireEvent,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;

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
