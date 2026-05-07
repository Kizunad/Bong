use valence::prelude::{EventReader, Res};

use crate::fauna::rat_phase::RatPhaseChangeEvent;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;

pub fn publish_rat_phase_events(
    redis: Option<Res<RedisBridgeResource>>,
    mut events: EventReader<RatPhaseChangeEvent>,
) {
    let Some(redis) = redis.as_deref() else {
        for _ in events.read() {}
        return;
    };

    for event in events.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::RatPhaseEvent(event.clone()));
    }
}
