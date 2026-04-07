use std::collections::HashMap;

use valence::message::ChatMessageEvent;
use valence::prelude::{
    DVec3, Entity, EventReader, Position, Query, Res, Resource, Username, With,
};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::player::gameplay::{CombatAction, GameplayAction, GameplayActionQueue, GatherAction};
use crate::schema::chat_message::ChatMessageV1;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const CHAT_MESSAGE_MAX_LENGTH: usize = 256;
const MAX_CHAT_MESSAGES_PER_PLAYER_PER_TICK: usize = 3;

#[derive(Default)]
pub struct ChatCollectorRateLimit {
    per_player_count: HashMap<Entity, usize>,
}

impl Resource for ChatCollectorRateLimit {}

pub fn collect_player_chat(
    redis: Res<RedisBridgeResource>,
    zone_registry: Option<Res<ZoneRegistry>>,
    players: Query<(&Username, &Position), With<valence::prelude::Client>>,
    mut events: EventReader<ChatMessageEvent>,
    mut rate_limit: valence::prelude::ResMut<ChatCollectorRateLimit>,
    mut gameplay_queue: Option<valence::prelude::ResMut<GameplayActionQueue>>,
) {
    rate_limit.per_player_count.clear();

    let zone_registry = zone_registry
        .as_deref()
        .cloned()
        .unwrap_or_else(ZoneRegistry::fallback);

    for ChatMessageEvent {
        client,
        message,
        timestamp,
    } in events.read()
    {
        let Some(message_outcome) = classify_player_message(
            *client,
            message,
            *timestamp,
            &players,
            &zone_registry,
            &mut rate_limit,
        ) else {
            continue;
        };

        match message_outcome {
            CollectedPlayerMessage::RedisOutbound(outbound) => {
                let _ = redis.tx_outbound.send(outbound);
            }
            CollectedPlayerMessage::GameplayAction { player, action } => {
                let Some(queue) = gameplay_queue.as_deref_mut() else {
                    tracing::warn!(
                        "[bong][network] dropped gameplay command from `{player}` because GameplayActionQueue is missing"
                    );
                    continue;
                };

                queue.enqueue(player, action);
            }
        }
    }
}

#[derive(Debug, Clone)]
enum CollectedPlayerMessage {
    RedisOutbound(RedisOutbound),
    GameplayAction {
        player: String,
        action: GameplayAction,
    },
}

fn classify_player_message(
    player_entity: Entity,
    message: &str,
    timestamp: u64,
    players: &Query<(&Username, &Position), With<valence::prelude::Client>>,
    zone_registry: &ZoneRegistry,
    rate_limit: &mut ChatCollectorRateLimit,
) -> Option<CollectedPlayerMessage> {
    let too_long = is_oversize_message(message);
    let over_budget = exceeds_rate_budget(player_entity, rate_limit);

    if too_long || over_budget {
        return None;
    }

    let Ok((username, position)) = players.get(player_entity) else {
        return None;
    };

    if let Some(action) = parse_gameplay_action(message) {
        return Some(CollectedPlayerMessage::GameplayAction {
            player: canonical_player_id(username.0.as_str()),
            action,
        });
    }

    if is_command_like(message) {
        return None;
    }

    let zone = zone_name_for_position(zone_registry, position.get());
    let canonical_player = canonical_player_id(username.0.as_str());

    Some(CollectedPlayerMessage::RedisOutbound(
        RedisOutbound::PlayerChat(ChatMessageV1 {
            v: 1,
            ts: timestamp,
            player: canonical_player,
            raw: message.to_string(),
            zone,
        }),
    ))
}

fn parse_gameplay_action(message: &str) -> Option<GameplayAction> {
    let mut tokens = message.split_whitespace();
    match (
        tokens.next(),
        tokens.next(),
        tokens.next(),
        tokens.next(),
        tokens.next(),
    ) {
        (Some("/bong"), Some("combat"), Some(target), Some(target_health), None) => {
            Some(GameplayAction::Combat(CombatAction {
                target: target.to_string(),
                target_health: target_health.parse::<f64>().ok()?,
            }))
        }
        (Some("/bong"), Some("gather"), Some(resource), None, None) => {
            Some(GameplayAction::Gather(GatherAction {
                resource: resource.to_string(),
            }))
        }
        (Some("/bong"), Some("breakthrough"), None, None, None) => {
            Some(GameplayAction::AttemptBreakthrough)
        }
        _ => None,
    }
}

fn exceeds_rate_budget(player_entity: Entity, rate_limit: &mut ChatCollectorRateLimit) -> bool {
    let counter = rate_limit
        .per_player_count
        .entry(player_entity)
        .or_default();
    if *counter >= MAX_CHAT_MESSAGES_PER_PLAYER_PER_TICK {
        return true;
    }

    *counter += 1;
    false
}

fn is_command_like(message: &str) -> bool {
    message.trim_start().starts_with('/')
}

fn is_oversize_message(message: &str) -> bool {
    message.chars().count() > CHAT_MESSAGE_MAX_LENGTH
}

fn zone_name_for_position(zone_registry: &ZoneRegistry, position: DVec3) -> String {
    zone_registry
        .find_zone(position)
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

fn canonical_player_id(username: &str) -> String {
    format!("offline:{username}")
}

#[cfg(test)]
mod chat_collector_tests {
    use super::*;
    use crate::network::RedisBridgeResource;
    use crate::player::gameplay::{
        CombatAction, GameplayAction, GameplayActionQueue, GatherAction, QueuedGameplayAction,
    };
    use crossbeam_channel::unbounded;
    use valence::prelude::{App, Position, Update};
    use valence::testing::create_mock_client;

    fn setup_chat_collector_app(
        with_zone_registry: bool,
    ) -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();

        let mut app = App::new();
        app.add_event::<ChatMessageEvent>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(ChatCollectorRateLimit::default());
        app.insert_resource(GameplayActionQueue::default());

        if with_zone_registry {
            app.insert_resource(ZoneRegistry::fallback());
        }

        app.add_systems(Update, collect_player_chat);

        (app, rx_outbound)
    }

    fn spawn_test_client(app: &mut App, username: &str, position: [f64; 3]) -> Entity {
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);

        app.world_mut().spawn(client_bundle).id()
    }

    fn send_chat_event(app: &mut App, client: Entity, message: &str, timestamp: u64) {
        app.world_mut().send_event(ChatMessageEvent {
            client,
            message: message.to_string().into_boxed_str(),
            timestamp,
        });
    }

    #[test]
    fn captures_plain_chat() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        send_chat_event(&mut app, alice, "这里灵气真足", 1_712_345_700);

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("plain chat should be forwarded to Redis outbound");

        match outbound {
            RedisOutbound::PlayerChat(chat) => {
                assert_eq!(chat.v, 1);
                assert_eq!(chat.ts, 1_712_345_700);
                assert_eq!(chat.player, "offline:Alice");
                assert_eq!(chat.raw, "这里灵气真足");
            }
            RedisOutbound::WorldState(_) => {
                panic!("expected player chat outbound, got world state")
            }
        }
    }

    #[test]
    fn skips_commands() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        send_chat_event(&mut app, alice, "/say hello", 1_712_345_701);

        app.update();

        assert!(
            rx_outbound.try_recv().is_err(),
            "slash command should not be forwarded to player_chat"
        );
    }

    #[test]
    fn adds_zone_context() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        send_chat_event(&mut app, alice, "在这里修炼", 1_712_345_702);

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("plain chat should include zone context");

        match outbound {
            RedisOutbound::PlayerChat(chat) => {
                assert_eq!(chat.zone, DEFAULT_SPAWN_ZONE_NAME);
            }
            RedisOutbound::WorldState(_) => {
                panic!("expected player chat outbound, got world state")
            }
        }
    }

    #[test]
    fn drops_oversize_messages() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        let oversize = "灵".repeat(CHAT_MESSAGE_MAX_LENGTH + 1);
        send_chat_event(&mut app, alice, oversize.as_str(), 1_712_345_703);

        app.update();

        assert!(
            rx_outbound.try_recv().is_err(),
            "oversize chat should be dropped before enqueue"
        );
    }

    #[test]
    fn drops_over_budget_messages() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        send_chat_event(&mut app, alice, "m1", 1);
        send_chat_event(&mut app, alice, "m2", 2);
        send_chat_event(&mut app, alice, "m3", 3);
        send_chat_event(&mut app, alice, "m4", 4);

        app.update();

        let mut forwarded = Vec::new();
        while let Ok(outbound) = rx_outbound.try_recv() {
            if let RedisOutbound::PlayerChat(chat) = outbound {
                forwarded.push(chat.raw);
            }
        }

        assert_eq!(forwarded, vec!["m1", "m2", "m3"]);
    }

    #[test]
    fn gameplay_commands_enqueue_actions() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        send_chat_event(&mut app, alice, "/bong combat rogue_boar 40", 1);
        send_chat_event(&mut app, alice, "/bong gather spirit_herb", 2);
        send_chat_event(&mut app, alice, "/bong breakthrough", 3);

        app.update();

        assert!(
            rx_outbound.try_recv().is_err(),
            "recognized gameplay commands must not be forwarded as player_chat"
        );

        let queued = app
            .world()
            .resource::<GameplayActionQueue>()
            .pending_actions_snapshot();
        assert_eq!(
            queued,
            vec![
                QueuedGameplayAction {
                    player: "offline:Alice".to_string(),
                    action: GameplayAction::Combat(CombatAction {
                        target: "rogue_boar".to_string(),
                        target_health: 40.0,
                    }),
                },
                QueuedGameplayAction {
                    player: "offline:Alice".to_string(),
                    action: GameplayAction::Gather(GatherAction {
                        resource: "spirit_herb".to_string(),
                    }),
                },
                QueuedGameplayAction {
                    player: "offline:Alice".to_string(),
                    action: GameplayAction::AttemptBreakthrough,
                },
            ]
        );
    }
}
