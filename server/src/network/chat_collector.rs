use std::collections::HashMap;

use valence::message::ChatMessageEvent;
use valence::message::SendMessage;
use valence::prelude::{
    Client, DVec3, Entity, EventReader, ParamSet, Position, Query, Res, ResMut, Resource, Username,
    With,
};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::combat::components::Lifecycle;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::inventory::spirit_treasure::{ActiveSpiritTreasures, SpiritTreasureRegistry};
use crate::player::state::canonical_player_id;
use crate::schema::chat_message::ChatMessageV1;
use crate::schema::cultivation::realm_to_string;
use crate::schema::spirit_treasure::{
    SpiritTreasureDialogueContextV1, SpiritTreasureDialogueHistoryEntryV1,
    SpiritTreasureDialogueRequestV1, SpiritTreasureDialogueTriggerV1,
};
use crate::social::events::PlayerChatCollected;
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
    clock: Option<Res<CombatClock>>,
    mut spirit_treasure_registry: Option<ResMut<SpiritTreasureRegistry>>,
    mut player_sets: ParamSet<(
        Query<
            (
                &Username,
                &Position,
                Option<&Lifecycle>,
                Option<&ActiveSpiritTreasures>,
                Option<&Cultivation>,
            ),
            With<Client>,
        >,
        Query<(&mut Client, &Position), With<Client>>,
    )>,
    mut events: EventReader<ChatMessageEvent>,
    mut collected_chats: valence::prelude::EventWriter<PlayerChatCollected>,
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
            players.get(*client).ok().map(
                |(username, position, lifecycle, active_treasures, cultivation)| PlayerChatInfo {
                    username: username.0.clone(),
                    position: position.get(),
                    char_id: lifecycle.map(|lifecycle| lifecycle.character_id.clone()),
                    active_treasures: active_treasures.cloned(),
                    realm: cultivation
                        .map(|cultivation| realm_to_string(cultivation.realm).to_string())
                        .unwrap_or_else(|| "Awaken".to_string()),
                    qi_percent: cultivation
                        .map(|cultivation| {
                            if cultivation.qi_max > 0.0 {
                                (cultivation.qi_current / cultivation.qi_max).clamp(0.0, 1.0)
                            } else {
                                0.0
                            }
                        })
                        .unwrap_or_default(),
                },
            )
        };

        let now_tick = clock
            .as_deref()
            .map(|clock| clock.tick)
            .unwrap_or(*timestamp);
        let Some(classified) = classify_player_message(
            *client,
            message,
            *timestamp,
            player_info,
            &mut player_sets.p1(),
            &zone_registry,
            &mut rate_limit,
            spirit_treasure_registry.as_deref_mut(),
            now_tick,
        ) else {
            continue;
        };

        match classified {
            ClassifiedChat::PlayerChat {
                outbound,
                collected,
            } => {
                collected_chats.send(collected);
                let _ = redis.tx_outbound.send(outbound);
            }
            ClassifiedChat::SpiritTreasureDialogue {
                outbound,
                zone,
                public_message,
            } => {
                let mut clients = player_sets.p1();
                for (mut client, position) in &mut clients {
                    if zone_name_for_position(&zone_registry, position.get()) == zone {
                        client.send_chat_message(public_message.clone());
                    }
                }
                let _ = redis.tx_outbound.send(outbound);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PlayerChatInfo {
    username: String,
    position: DVec3,
    char_id: Option<String>,
    active_treasures: Option<ActiveSpiritTreasures>,
    realm: String,
    qi_percent: f64,
}

enum ClassifiedChat {
    PlayerChat {
        outbound: RedisOutbound,
        collected: PlayerChatCollected,
    },
    SpiritTreasureDialogue {
        outbound: RedisOutbound,
        zone: String,
        public_message: String,
    },
}

fn classify_player_message(
    player_entity: Entity,
    message: &str,
    timestamp: u64,
    player_info: Option<PlayerChatInfo>,
    clients: &mut Query<(&mut Client, &Position), With<Client>>,
    zone_registry: &ZoneRegistry,
    rate_limit: &mut ChatCollectorRateLimit,
    spirit_treasure_registry: Option<&mut SpiritTreasureRegistry>,
    now_tick: u64,
) -> Option<ClassifiedChat> {
    let too_long = is_oversize_message(message);
    let over_budget = exceeds_rate_budget(player_entity, rate_limit);

    if too_long || over_budget {
        return None;
    }

    let player_info = player_info?;
    let username = player_info.username.clone();
    let position = player_info.position;

    if is_legacy_bang_command(message) {
        if let Ok((mut client, _)) = clients.get_mut(player_entity) {
            client.send_chat_message("`!` 命令已迁至 `/`，使用 Tab 补全");
        }
        return None;
    }

    if is_command_like(message) {
        return None;
    }

    let zone = zone_name_for_position(zone_registry, position);
    let canonical_player = canonical_player_id(username.as_str());
    let char_id = player_info
        .char_id
        .as_deref()
        .unwrap_or(canonical_player.as_str())
        .to_string();

    if let Some(registry) = spirit_treasure_registry {
        if let Some(route) = classify_spirit_treasure_dialogue(
            player_entity,
            message,
            &player_info,
            char_id.as_str(),
            zone.as_str(),
            registry,
            now_tick,
            timestamp,
        ) {
            match route {
                SpiritTreasureRoute::PromptSelf(text) => {
                    if let Ok((mut client, _)) = clients.get_mut(player_entity) {
                        client.send_chat_message(text);
                    }
                    return None;
                }
                SpiritTreasureRoute::Dialogue {
                    outbound,
                    public_message,
                } => {
                    return Some(ClassifiedChat::SpiritTreasureDialogue {
                        outbound,
                        zone,
                        public_message,
                    });
                }
            }
        }
    }

    Some(ClassifiedChat::PlayerChat {
        outbound: RedisOutbound::PlayerChat(ChatMessageV1 {
            v: 1,
            ts: timestamp,
            player: canonical_player,
            raw: message.to_string(),
            zone: zone.clone(),
        }),
        collected: PlayerChatCollected {
            entity: player_entity,
            username,
            char_id,
            zone,
            raw: message.to_string(),
            timestamp,
        },
    })
}

enum SpiritTreasureRoute {
    PromptSelf(String),
    Dialogue {
        outbound: RedisOutbound,
        public_message: String,
    },
}

fn classify_spirit_treasure_dialogue(
    player_entity: Entity,
    message: &str,
    player_info: &PlayerChatInfo,
    char_id: &str,
    zone: &str,
    registry: &mut SpiritTreasureRegistry,
    now_tick: u64,
    timestamp: u64,
) -> Option<SpiritTreasureRoute> {
    let (target_name, player_message) = parse_spirit_treasure_mention(message)?;
    let def = registry.find_by_display_name(target_name)?.clone();
    let active = player_info
        .active_treasures
        .as_ref()
        .and_then(|active| {
            active
                .treasures
                .iter()
                .find(|entry| entry.template_id == def.template_id)
        })
        .cloned();
    let Some(active) = active else {
        return Some(SpiritTreasureRoute::PromptSelf(
            "§8[灵宝] §7你并未持有此物。".to_string(),
        ));
    };

    let (sleeping, last_dialogue_tick, affinity) = registry
        .active
        .get(&def.template_id)
        .map(|state| (state.sleeping, state.last_dialogue_tick, state.affinity))
        .unwrap_or((false, 0, 0.5));
    if sleeping {
        return Some(SpiritTreasureRoute::PromptSelf(
            "§8[灵宝] §7镜面无光，器灵仍在沉睡。".to_string(),
        ));
    }

    let cooldown_ticks = u64::from(def.dialogue_cooldown_s).saturating_mul(20);
    let ready_at = last_dialogue_tick.saturating_add(cooldown_ticks);
    if last_dialogue_tick > 0 && now_tick < ready_at {
        let seconds = ready_at.saturating_sub(now_tick).div_ceil(20);
        return Some(SpiritTreasureRoute::PromptSelf(format!(
            "§8[灵宝] §7寂照镜尚未回神，还需 {seconds}s。"
        )));
    }

    if let Some(state) = registry.active.get_mut(&def.template_id) {
        state.last_dialogue_tick = now_tick;
    }

    let request = SpiritTreasureDialogueRequestV1 {
        v: 1,
        request_id: format!("spirit_treasure:{:x}:{timestamp}", player_entity.to_bits()),
        character_id: char_id.to_string(),
        treasure_id: def.template_id.clone(),
        trigger: SpiritTreasureDialogueTriggerV1::Player,
        player_message: Some(player_message.to_string()),
        context: SpiritTreasureDialogueContextV1 {
            realm: player_info.realm.clone(),
            qi_percent: player_info.qi_percent,
            zone: zone.to_string(),
            recent_events: Vec::new(),
            affinity,
            dialogue_history: vec![SpiritTreasureDialogueHistoryEntryV1 {
                speaker: "player".to_string(),
                content: player_message.to_string(),
            }],
            equipped: active.equipped,
        },
    };

    Some(SpiritTreasureRoute::Dialogue {
        outbound: RedisOutbound::SpiritTreasureDialogueRequest(request),
        public_message: format!(
            "§7[灵宝] §f{} §8@{}§7：{}",
            player_info.username, def.display_name, player_message
        ),
    })
}

fn parse_spirit_treasure_mention(message: &str) -> Option<(&str, &str)> {
    let trimmed = message.trim_start();
    let rest = trimmed.strip_prefix('@')?;
    let (name, body) = rest
        .split_once(char::is_whitespace)
        .map(|(name, body)| (name, body.trim()))
        .unwrap_or((rest, ""));
    if name.is_empty() {
        return None;
    }
    Some((name, body))
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
        app.add_event::<PlayerChatCollected>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(ChatCollectorRateLimit::default());
        app.insert_resource(SpiritTreasureRegistry::default());

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

    #[test]
    fn emits_collected_chat_event_after_filtering() {
        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        app.world_mut().entity_mut(alice).insert(Lifecycle {
            character_id: "char:alice".to_string(),
            ..Default::default()
        });
        send_chat_event(&mut app, alice, "现身一言", 1_712_345_705);

        app.update();

        assert!(matches!(
            rx_outbound.try_recv(),
            Ok(RedisOutbound::PlayerChat(_))
        ));
        let events = app
            .world()
            .resource::<valence::prelude::Events<PlayerChatCollected>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).cloned().collect::<Vec<_>>();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].char_id, "char:alice");
        assert_eq!(collected[0].raw, "现身一言");
    }

    #[test]
    fn routes_owned_spirit_treasure_mention_to_dialogue_runtime() {
        use crate::inventory::spirit_treasure::{ActiveTreasureEntry, JIZHAOJING_TEMPLATE_ID};

        let (mut app, rx_outbound) = setup_chat_collector_app(true);
        let alice = spawn_test_client(&mut app, "Alice", [8.0, 66.0, 8.0]);
        app.world_mut().entity_mut(alice).insert((
            Lifecycle {
                character_id: "char:alice".to_string(),
                ..Default::default()
            },
            ActiveSpiritTreasures {
                treasures: vec![ActiveTreasureEntry {
                    template_id: JIZHAOJING_TEMPLATE_ID.to_string(),
                    instance_id: 88,
                    equipped: true,
                    passive_active: true,
                }],
            },
        ));
        send_chat_event(&mut app, alice, "@寂照镜 镜中可见什么？", 1_712_345_706);

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("owned spirit treasure mention should publish dialogue request");
        match outbound {
            RedisOutbound::SpiritTreasureDialogueRequest(request) => {
                assert_eq!(request.v, 1);
                assert_eq!(request.character_id, "char:alice");
                assert_eq!(request.treasure_id, JIZHAOJING_TEMPLATE_ID);
                assert_eq!(request.trigger, SpiritTreasureDialogueTriggerV1::Player);
                assert_eq!(request.player_message.as_deref(), Some("镜中可见什么？"));
                assert!(request.context.equipped);
            }
            other => panic!("expected spirit treasure dialogue request, got {other:?}"),
        }
        assert!(
            rx_outbound.try_recv().is_err(),
            "spirit treasure mention should not also enter normal player_chat"
        );
    }
}
