use valence::message::ChatMessageEvent;
use valence::prelude::{EventReader, Position, Query, Res};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::schema::chat_message::ChatMessageV1;
use crate::world::ZoneRegistry;

const CHAT_MESSAGE_VERSION: u8 = 1;

pub fn collect_player_chat_to_redis(
    redis: Res<RedisBridgeResource>,
    mut chat_events: EventReader<ChatMessageEvent>,
    clients: Query<(&valence::prelude::UniqueId, &Position)>,
    zone_registry: Res<ZoneRegistry>,
) {
    for event in chat_events.read() {
        if let Some(message) = build_chat_message(event, &clients, zone_registry.as_ref()) {
            let _ = redis.tx_outbound.send(RedisOutbound::PlayerChat(message));
        }
    }
}

fn build_chat_message(
    event: &ChatMessageEvent,
    clients: &Query<(&valence::prelude::UniqueId, &Position)>,
    zone_registry: &ZoneRegistry,
) -> Option<ChatMessageV1> {
    let Ok((unique_id, position)) = clients.get(event.client) else {
        tracing::warn!(
            "[bong][network][chat_collector] skip chat from unknown client entity {:?}",
            event.client
        );
        return None;
    };

    Some(build_chat_message_from_client(
        event.timestamp,
        unique_id,
        position.get(),
        &event.message,
        zone_registry,
    ))
}

fn build_chat_message_from_client(
    timestamp_secs: u64,
    unique_id: &valence::prelude::UniqueId,
    position: valence::prelude::DVec3,
    raw_message: &str,
    zone_registry: &ZoneRegistry,
) -> ChatMessageV1 {
    let zone = zone_registry.find_zone_or_default(position).name.clone();

    ChatMessageV1 {
        v: CHAT_MESSAGE_VERSION,
        ts: timestamp_secs,
        player: format!("offline:{}", unique_id.0),
        raw: raw_message.to_string(),
        zone,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{DVec3, Uuid};

    #[test]
    fn chat_collector_encodes_chat_message_v1_with_zone_lookup() {
        let zone_registry = ZoneRegistry::fallback();
        let unique_id = valence::prelude::UniqueId(Uuid::nil());
        let msg = build_chat_message_from_client(
            1_712_345_700,
            &unique_id,
            DVec3::new(
                crate::world::DEFAULT_SPAWN_POSITION[0],
                crate::world::DEFAULT_SPAWN_POSITION[1],
                crate::world::DEFAULT_SPAWN_POSITION[2],
            ),
            "这破地方灵气也太少了吧",
            &zone_registry,
        );

        assert_eq!(msg.v, 1);
        assert_eq!(msg.ts, 1_712_345_700);
        assert_eq!(msg.player, format!("offline:{}", Uuid::nil()));
        assert_eq!(msg.raw, "这破地方灵气也太少了吧");
        assert_eq!(msg.zone, crate::world::DEFAULT_SPAWN_ZONE);
    }

    #[test]
    fn chat_collector_uses_uuid_identity_even_with_name_present() {
        let zone_registry = ZoneRegistry::fallback();
        let unique_id = valence::prelude::UniqueId(
            Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap(),
        );

        let msg = build_chat_message_from_client(
            42,
            &unique_id,
            DVec3::new(8.0, 66.0, 8.0),
            "测试",
            &zone_registry,
        );

        assert_eq!(
            msg.player,
            "offline:123e4567-e89b-12d3-a456-426614174000".to_string()
        );
    }

    #[test]
    fn chat_collector_message_is_json_roundtrippable() {
        let zone_registry = ZoneRegistry::fallback();
        let unique_id = valence::prelude::UniqueId(Uuid::nil());

        let msg = build_chat_message_from_client(
            7,
            &unique_id,
            DVec3::new(8.0, 66.0, 8.0),
            "hello",
            &zone_registry,
        );

        let encoded = serde_json::to_string(&msg).expect("chat message should serialize");
        let decoded: ChatMessageV1 =
            serde_json::from_str(&encoded).expect("chat message should deserialize");

        assert_eq!(decoded.v, 1);
        assert_eq!(decoded.player, format!("offline:{}", Uuid::nil()));
        assert_eq!(decoded.zone, crate::world::DEFAULT_SPAWN_ZONE);
    }
}
