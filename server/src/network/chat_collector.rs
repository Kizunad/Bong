use std::collections::HashMap;

use valence::message::ChatMessageEvent;
use valence::message::SendMessage;
use valence::prelude::{
    Client, DVec3, Entity, EventReader, EventWriter, GameMode, ParamSet, Position, Query, Res,
    Resource, Username, With,
};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::combat::components::{BodyPart, WoundKind};
use crate::combat::events::{DebugCombatCommand, DebugCombatCommandKind};
use crate::npc::scenario::{PendingScenario, ScenarioType};
use crate::player::{
    gameplay::{CombatAction, GameplayAction, GameplayActionQueue, GatherAction},
    state::canonical_player_id,
};
use crate::schema::chat_message::ChatMessageV1;
use crate::world::terrain::TerrainProvider;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

// chat_collector 当前同时承载普通聊天收集与开发期快捷命令（如 `!spawn`/`!gm`），
// 并保持现有函数签名与 clippy allow，以保证现有调试流程和消息路径行为稳定。

const CHAT_MESSAGE_MAX_LENGTH: usize = 256;
const MAX_CHAT_MESSAGES_PER_PLAYER_PER_TICK: usize = 3;

#[derive(Default)]
pub struct ChatCollectorRateLimit {
    per_player_count: HashMap<Entity, usize>,
}

impl Resource for ChatCollectorRateLimit {}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn collect_player_chat(
    redis: Res<RedisBridgeResource>,
    zone_registry: Option<Res<ZoneRegistry>>,
    terrain: Option<Res<TerrainProvider>>,
    mut player_sets: ParamSet<(
        Query<(&Username, &Position), With<Client>>,
        Query<(&mut Position, &mut GameMode, &mut Client, &Username), With<Client>>,
    )>,
    mut events: EventReader<ChatMessageEvent>,
    mut rate_limit: valence::prelude::ResMut<ChatCollectorRateLimit>,
    mut gameplay_queue: Option<valence::prelude::ResMut<GameplayActionQueue>>,
    mut pending_scenario: Option<valence::prelude::ResMut<PendingScenario>>,
    mut debug_combat_tx: EventWriter<DebugCombatCommand>,
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

        let Some(message_outcome) = classify_player_message(
            *client,
            message,
            *timestamp,
            player_info,
            &mut player_sets.p1(),
            &zone_registry,
            terrain.as_deref(),
            &mut rate_limit,
            pending_scenario.as_deref_mut(),
            &mut debug_combat_tx,
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

#[allow(clippy::too_many_arguments)]
fn classify_player_message(
    player_entity: Entity,
    message: &str,
    timestamp: u64,
    player_info: Option<(String, DVec3)>,
    clients: &mut Query<(&mut Position, &mut GameMode, &mut Client, &Username), With<Client>>,
    zone_registry: &ZoneRegistry,
    terrain: Option<&TerrainProvider>,
    rate_limit: &mut ChatCollectorRateLimit,
    pending_scenario: Option<&mut PendingScenario>,
    debug_combat_tx: &mut EventWriter<DebugCombatCommand>,
) -> Option<CollectedPlayerMessage> {
    let too_long = is_oversize_message(message);
    let over_budget = exceeds_rate_budget(player_entity, rate_limit);

    if too_long || over_budget {
        return None;
    }

    let (username, position) = player_info?;

    if try_handle_dev_command(
        player_entity,
        message,
        position,
        clients,
        zone_registry,
        terrain,
        pending_scenario,
        debug_combat_tx,
    ) {
        return None;
    }

    if let Some(action) = parse_gameplay_action(message) {
        return Some(CollectedPlayerMessage::GameplayAction {
            player: canonical_player_id(username.as_str()),
            action,
        });
    }

    if is_command_like(message) {
        return None;
    }

    let zone = zone_name_for_position(zone_registry, position);
    let canonical_player = canonical_player_id(username.as_str());

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
        (Some("/bong"), Some("combat"), Some(target), Some(qi_invest), None) => {
            Some(GameplayAction::Combat(CombatAction {
                target: target.to_string(),
                qi_invest: qi_invest.parse::<f64>().ok()?,
            }))
        }
        (Some("/bong"), Some("gather"), Some(resource), None, None) => {
            Some(GameplayAction::Gather(GatherAction {
                resource: resource.to_string(),
                target_entity: None,
                mode: None,
            }))
        }
        (Some("/bong"), Some("breakthrough"), None, None, None) => {
            Some(GameplayAction::AttemptBreakthrough)
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn try_handle_dev_command(
    player_entity: Entity,
    message: &str,
    player_pos: DVec3,
    clients: &mut Query<(&mut Position, &mut GameMode, &mut Client, &Username), With<Client>>,
    zone_registry: &ZoneRegistry,
    terrain: Option<&TerrainProvider>,
    pending_scenario: Option<&mut PendingScenario>,
    debug_combat_tx: &mut EventWriter<DebugCombatCommand>,
) -> bool {
    let trimmed = message.trim();
    if !trimmed.starts_with('!') {
        return false;
    }

    let mut tokens = trimmed.split_whitespace();
    let Some(command) = tokens.next() else {
        return false;
    };

    let Ok((mut position, mut game_mode, mut client, _username)) = clients.get_mut(player_entity)
    else {
        return false;
    };

    match command {
        "!spawn" => {
            position.set(crate::player::spawn_position());
            client.send_chat_message("Teleported to spawn.");
            true
        }
        "!top" => {
            let current = position.get();
            let target_y = if let Some(terrain) = terrain {
                let sample = terrain.sample(current.x.floor() as i32, current.z.floor() as i32);
                let surface_y = sample.height.round() as f64;
                let water_y = if sample.water_level >= 0.0 {
                    sample.water_level.round() as f64
                } else {
                    surface_y
                };
                surface_y.max(water_y) + 3.0
            } else {
                current.y + 24.0
            };

            position.set([current.x, target_y, current.z]);
            client.send_chat_message(format!("Teleported to top at Y={target_y:.0}."));
            true
        }
        "!gm" | "!gamemode" => {
            let Some(mode) = tokens.next() else {
                client.send_chat_message("Usage: !gm <c|a|s>");
                return true;
            };
            match mode {
                "c" | "creative" => {
                    *game_mode = GameMode::Creative;
                    client.send_chat_message("Gamemode set to Creative.");
                }
                "a" | "adventure" => {
                    *game_mode = GameMode::Adventure;
                    client.send_chat_message("Gamemode set to Adventure.");
                }
                "s" | "spectator" => {
                    *game_mode = GameMode::Spectator;
                    client.send_chat_message("Gamemode set to Spectator.");
                }
                _ => client.send_chat_message("Usage: !gm <c|a|s>"),
            }
            true
        }
        "!tptree" => {
            let Some(tree_name) = tokens.next() else {
                client.send_chat_message("Usage: !tptree <spirit|dead>");
                return true;
            };
            let zone_name = match tree_name {
                "spirit" => "spawn",
                "dead" => "north_wastes",
                _ => {
                    client.send_chat_message("Unknown tree. Use: spirit, dead");
                    return true;
                }
            };
            let Some(zone) = zone_registry.find_zone_by_name(zone_name) else {
                client.send_chat_message("Zone not found.");
                return true;
            };
            let center = zone.center();
            let target_y = if let Some(terrain) = terrain {
                let sample = terrain.sample(center.x.floor() as i32, center.z.floor() as i32);
                sample.height.round() as f64 + 40.0
            } else {
                center.y + 60.0
            };
            position.set([center.x, target_y, center.z]);
            client.send_chat_message(format!(
                "Teleported above {tree_name} tree zone (`{zone_name}`)."
            ));
            true
        }
        "!tpzone" => {
            let Some(zone_name) = tokens.next() else {
                client.send_chat_message(
                    "Usage: !tpzone <spawn|qingyun_peaks|lingquan_marsh|blood_valley|youan_depths|north_wastes>",
                );
                return true;
            };

            let Some(zone) = zone_registry.find_zone_by_name(zone_name) else {
                client.send_chat_message("Unknown zone.");
                return true;
            };

            let center = zone.center();
            position.set([center.x, center.y + 24.0, center.z]);
            client.send_chat_message(format!("Teleported to zone `{zone_name}`."));
            true
        }
        "!zones" => {
            let names = zone_registry
                .zones
                .iter()
                .map(|zone| zone.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            client.send_chat_message(format!("Zones: {names}"));
            true
        }
        "!wound" => {
            // plan §13 C1 调试命令 — 用法: !wound add <part> [severity]
            let sub = tokens.next();
            let part_raw = tokens.next();
            let severity_raw = tokens.next();
            let (Some("add"), Some(part_str)) = (sub, part_raw) else {
                client.send_chat_message(
                    "Usage: !wound add <head|chest|abdomen|arml|armr|legl|legr> [severity=0.3]",
                );
                return true;
            };
            let Some(location) = parse_body_part(part_str) else {
                client.send_chat_message(
                    "Unknown body part. Use: head, chest, abdomen, arml, armr, legl, legr",
                );
                return true;
            };
            let severity = severity_raw
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.3);
            debug_combat_tx.send(DebugCombatCommand {
                target: player_entity,
                kind: DebugCombatCommandKind::AddWound {
                    location,
                    kind: WoundKind::Blunt,
                    severity,
                },
            });
            client.send_chat_message(format!(
                "Queued !wound add {part_str} severity={severity:.2}"
            ));
            true
        }
        "!health" => {
            // plan §13 C1 调试命令 — 用法: !health set <n>
            let sub = tokens.next();
            let value_raw = tokens.next();
            let (Some("set"), Some(val_str)) = (sub, value_raw) else {
                client.send_chat_message("Usage: !health set <n>");
                return true;
            };
            let Ok(value) = val_str.parse::<f32>() else {
                client.send_chat_message("!health set value must be a number");
                return true;
            };
            debug_combat_tx.send(DebugCombatCommand {
                target: player_entity,
                kind: DebugCombatCommandKind::SetHealth(value),
            });
            client.send_chat_message(format!("Queued !health set {value:.1}"));
            true
        }
        "!stamina" => {
            // plan §13 C1 调试命令 — 用法: !stamina set <n>
            let sub = tokens.next();
            let value_raw = tokens.next();
            let (Some("set"), Some(val_str)) = (sub, value_raw) else {
                client.send_chat_message("Usage: !stamina set <n>");
                return true;
            };
            let Ok(value) = val_str.parse::<f32>() else {
                client.send_chat_message("!stamina set value must be a number");
                return true;
            };
            debug_combat_tx.send(DebugCombatCommand {
                target: player_entity,
                kind: DebugCombatCommandKind::SetStamina(value),
            });
            client.send_chat_message(format!("Queued !stamina set {value:.1}"));
            true
        }
        "!npc_scenario" | "!scenario" => {
            let Some(scenario_name) = tokens.next() else {
                client.send_chat_message(
                    "Usage: !npc_scenario <chase|flee|fight|kite|swarm|duel|clear>",
                );
                return true;
            };
            let Some(scenario_type) = ScenarioType::from_str(scenario_name) else {
                client.send_chat_message(
                    "Unknown scenario. Options: chase, flee, fight, kite, swarm, duel, clear",
                );
                return true;
            };
            if let Some(ps) = pending_scenario {
                ps.request = Some((scenario_type, player_pos));
                client.send_chat_message(format!("Scenario `{scenario_name}` queued."));
            } else {
                client.send_chat_message("Scenario system not available.");
            }
            true
        }
        _ => false,
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

fn parse_body_part(s: &str) -> Option<BodyPart> {
    match s.to_ascii_lowercase().as_str() {
        "head" => Some(BodyPart::Head),
        "chest" => Some(BodyPart::Chest),
        "abdomen" => Some(BodyPart::Abdomen),
        "arml" => Some(BodyPart::ArmL),
        "armr" => Some(BodyPart::ArmR),
        "legl" => Some(BodyPart::LegL),
        "legr" => Some(BodyPart::LegR),
        _ => None,
    }
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
        app.add_event::<DebugCombatCommand>();
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
    fn gameplay_commands_enqueue_actions() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        send_chat_event(&mut app, alice, "/bong combat Crimson 40", 1);
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
                        target: "Crimson".to_string(),
                        qi_invest: 40.0,
                    }),
                },
                QueuedGameplayAction {
                    player: "offline:Alice".to_string(),
                    action: GameplayAction::Gather(GatherAction {
                        resource: "spirit_herb".to_string(),
                        target_entity: None,
                        mode: None,
                    }),
                },
                QueuedGameplayAction {
                    player: "offline:Alice".to_string(),
                    action: GameplayAction::AttemptBreakthrough,
                },
            ]
        );
    }

    #[test]
    fn bong_combat_argument_is_qi_invest_not_health_hint() {
        let action = parse_gameplay_action("/bong combat Crimson 12.5");

        assert_eq!(
            action,
            Some(GameplayAction::Combat(CombatAction {
                target: "Crimson".to_string(),
                qi_invest: 12.5,
            }))
        );
    }

    /// plan §13 C1 — `!wound add` / `!health set` / `!stamina set` 走 DebugCombatCommand 事件通道。
    #[test]
    fn debug_combat_commands_emit_events() {
        let (mut app, _rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        send_chat_event(&mut app, alice, "!wound add chest 0.7", 1);
        send_chat_event(&mut app, alice, "!health set 25", 2);
        send_chat_event(&mut app, alice, "!stamina set 10", 3);

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<DebugCombatCommand>>();
        let mut reader = events.get_reader();
        let collected: Vec<_> = reader.read(events).cloned().collect();
        assert_eq!(collected.len(), 3);

        match &collected[0].kind {
            DebugCombatCommandKind::AddWound {
                location,
                kind,
                severity,
            } => {
                assert_eq!(*location, BodyPart::Chest);
                assert_eq!(*kind, WoundKind::Blunt);
                assert!((severity - 0.7).abs() < 1e-6);
            }
            other => panic!("expected AddWound, got {other:?}"),
        }
        match &collected[1].kind {
            DebugCombatCommandKind::SetHealth(n) => assert!((n - 25.0).abs() < 1e-6),
            other => panic!("expected SetHealth, got {other:?}"),
        }
        match &collected[2].kind {
            DebugCombatCommandKind::SetStamina(n) => assert!((n - 10.0).abs() < 1e-6),
            other => panic!("expected SetStamina, got {other:?}"),
        }
    }

    /// 用法串错 (!wound 缺 part / !health 缺 value) 只回显 usage，不发事件。
    #[test]
    fn debug_combat_commands_reject_malformed_usage() {
        let (mut app, _rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);

        send_chat_event(&mut app, alice, "!wound add bogus_part", 1);
        send_chat_event(&mut app, alice, "!wound", 2);
        send_chat_event(&mut app, alice, "!health", 3);
        send_chat_event(&mut app, alice, "!stamina set not_a_number", 4);

        app.update();

        let events = app
            .world()
            .resource::<valence::prelude::Events<DebugCombatCommand>>();
        let mut reader = events.get_reader();
        let collected: Vec<_> = reader.read(events).cloned().collect();
        assert!(
            collected.is_empty(),
            "malformed debug commands should not emit events, got {collected:?}"
        );
    }
}
