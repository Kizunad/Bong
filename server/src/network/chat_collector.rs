use std::collections::HashMap;

use valence::message::ChatMessageEvent;
use valence::message::SendMessage;
use valence::prelude::{
    Client, DVec3, Entity, EventReader, ParamSet, Position, Query, Res, Resource, Username, With,
};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::player::state::canonical_player_id;
use crate::schema::chat_message::ChatMessageV1;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const CHAT_MESSAGE_MAX_LENGTH: usize = 256;
const MAX_CHAT_MESSAGES_PER_PLAYER_PER_TICK: usize = 3;
const LEGACY_BANG_COMMANDS: &[&str] = &[
    "!shrine",
    "!spawn",
    "!top",
    "!gm",
    "!gamemode",
    "!tptree",
    "!tpzone",
    "!zones",
    "!wound",
    "!health",
    "!stamina",
    "!tsy-spawn",
    "!npc_scenario",
    "!scenario",
];

#[derive(Default)]
pub struct ChatCollectorRateLimit {
    per_player_count: HashMap<Entity, usize>,
}

impl Resource for ChatCollectorRateLimit {}

#[allow(clippy::type_complexity)]
pub fn collect_player_chat(
    redis: Res<RedisBridgeResource>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut player_sets: ParamSet<(
        Query<(&Username, &Position), With<Client>>,
        Query<&mut Client, With<Client>>,
    )>,
    mut events: EventReader<ChatMessageEvent>,
    mut rate_limit: valence::prelude::ResMut<ChatCollectorRateLimit>,
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
        let player_info = {
            let players = player_sets.p0();
            players
                .get(*client)
                .ok()
                .map(|(username, position)| (username.0.clone(), position.get()))
        };

        let Some(outbound) = classify_player_message(
            *client,
            message,
            *timestamp,
            player_info,
            &mut player_sets.p1(),
            &zone_registry,
            &mut rate_limit,
        ) else {
            continue;
        };

        let _ = redis.tx_outbound.send(outbound);
    }
}

fn classify_player_message(
    player_entity: Entity,
    message: &str,
    timestamp: u64,
    player_info: Option<(String, DVec3)>,
    clients: &mut Query<&mut Client, With<Client>>,
    zone_registry: &ZoneRegistry,
    rate_limit: &mut ChatCollectorRateLimit,
) -> Option<RedisOutbound> {
    let too_long = is_oversize_message(message);
    let over_budget = exceeds_rate_budget(player_entity, rate_limit);

    if too_long || over_budget {
        return None;
    }

    let (username, position) = player_info?;

    if is_legacy_bang_command(message) {
        if let Ok(mut client) = clients.get_mut(player_entity) {
            client.send_chat_message("`!` 命令已迁至 `/`，使用 Tab 补全");
        }
        return None;
    }

    if is_command_like(message) {
        return None;
    }

    let zone = zone_name_for_position(zone_registry, position);
    let canonical_player = canonical_player_id(username.as_str());

    Some(RedisOutbound::PlayerChat(ChatMessageV1 {
        v: 1,
        ts: timestamp,
        player: canonical_player,
        raw: message.to_string(),
        zone,
    }))
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

fn is_legacy_bang_command(message: &str) -> bool {
    message
        .trim_start()
        .split_whitespace()
        .next()
        .is_some_and(|command| LEGACY_BANG_COMMANDS.contains(&command))
}

fn is_oversize_message(message: &str) -> bool {
    message.chars().count() > CHAT_MESSAGE_MAX_LENGTH
}

fn zone_name_for_position(zone_registry: &ZoneRegistry, position: DVec3) -> String {
    zone_registry
        .find_zone(crate::world::dimension::DimensionKind::Overworld, position)
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

#[cfg(test)]
mod chat_collector_tests {
    use super::*;
    use crate::network::RedisBridgeResource;
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
            other => panic!("expected player chat outbound, got {other:?}"),
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
            other => panic!("expected player chat outbound, got {other:?}"),
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
    fn slash_commands_are_not_forwarded_as_chat() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        send_chat_event(&mut app, alice, "/bong combat Crimson 40", 1);
        send_chat_event(&mut app, alice, "/bong gather spirit_herb", 2);
        send_chat_event(&mut app, alice, "/bong breakthrough", 3);

        app.update();

        assert!(
            rx_outbound.try_recv().is_err(),
            "slash commands must be handled by brigadier command systems, not player_chat"
        );
    }

    #[test]
    fn legacy_bang_commands_are_dropped_not_forwarded() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        send_chat_event(&mut app, alice, "!wound add chest 0.7", 1);
        send_chat_event(&mut app, alice, "!shrine set", 2);
        send_chat_event(&mut app, alice, "!health set 25", 3);
        app.update();

        assert!(
            rx_outbound.try_recv().is_err(),
            "legacy ! commands should be dropped after migration to slash commands"
        );
    }

    #[test]
    fn unknown_bang_messages_are_forwarded_as_chat() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        send_chat_event(&mut app, alice, "!hello everyone", 1_712_345_704);
        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("unknown ! chat should still be forwarded");

        match outbound {
            RedisOutbound::PlayerChat(chat) => {
                assert_eq!(chat.raw, "!hello everyone");
            }
            other => panic!("expected player chat outbound, got {other:?}"),
        }
    }
}
