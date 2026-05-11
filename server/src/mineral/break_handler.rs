//! plan-mineral-v1 §2.2 — `BlockBreakEvent`（valence `DiggingEvent`）监听器。
//!
//! 流程：
//!  - **Survival + Stop**：完整 drop 流水（pickaxe 检查 → `MineralDropEvent` →
//!    karma 概率推送 → 客户端 mining_progress feedback → 减 unit → exhaust 清理）。
//!  - **Creative + Start**：cleanup-only。Creative 不掉物（vanilla 行为），但默认
//!    block_break 系统已把 chunk 抹成 AIR，本路径同步把 `MineralOreNode` 减 unit
//!    并在归零时 despawn —— 否则 `MineralOreIndex` 留下"chunk 已空但 entity 还在"
//!    的鬼影状态，`/probe` / 重 spawn 等会读到陈旧数据。
//!  - 其余 (state, mode) 组合（Survival Start/Abort、Adventure、Spectator）跳过。
//!
//! 与 `inventory::DroppedItemEvent` 解耦：本系统只发 mineral_id 语义的 drop 事件，
//! 由 inventory 侧的 listener 把 mineral_id 序列化到新建 InventoryItem 的 NBT。

use valence::prelude::{
    bevy_ecs, bevy_ecs::system::SystemParam, BlockPos, Client, Commands, DiggingEvent,
    DiggingState, Entity, EventReader, EventWriter, GameMode, Query, Res, ResMut, With,
};

use super::components::{MineralOreIndex, MineralOreNode};
use super::events::{
    KarmaFlagIntent, MineralDropEvent, MineralExhaustedEvent, MineralFeedbackEvent,
};
use super::registry::MineralRegistry;
use super::types::{MineralId, MineralRarity};
use crate::combat::components::Lifecycle;
use crate::cultivation::components::{Cultivation, Realm};
use crate::gathering::session::{
    GatheringCompleteEvent, GatheringProgressFrame, GatheringSession, GatheringSessionStart,
    GatheringSessionStore,
};
use crate::gathering::tools::{equipped_gathering_tool, GatheringTargetKind};
use crate::inventory::{ItemInstance, PlayerInventory, EQUIP_SLOT_MAIN_HAND, EQUIP_SLOT_TWO_HAND};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::send_server_data_payload;
use crate::player::gameplay::GameplayTick;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::social::{block_break_is_protected_by_registered_spirit_niche, SpiritNicheRegistry};
use crate::world::dimension::{CurrentDimension, DimensionKind};

/// plan-mineral-v1 §3 — 极品矿脉劫气概率（worldview §七）。
///
/// tier 1/2 = 0%（不推 KarmaFlag），tier 3 = 15%，tier 4 = 30%。
/// 概率值由 listener 直接写入 `KarmaFlagIntent.probability`，下游 agent 决定是否触发。
const KARMA_PROBABILITY_FAN: f32 = 0.0;
const KARMA_PROBABILITY_LING: f32 = 0.0;
const KARMA_PROBABILITY_XI: f32 = 0.15;
const KARMA_PROBABILITY_YI: f32 = 0.30;

pub fn karma_probability(rarity: MineralRarity) -> f32 {
    match rarity {
        MineralRarity::Fan => KARMA_PROBABILITY_FAN,
        MineralRarity::Ling => KARMA_PROBABILITY_LING,
        MineralRarity::Xi => KARMA_PROBABILITY_XI,
        MineralRarity::Yi => KARMA_PROBABILITY_YI,
    }
}

/// 区分本次破坏走哪条流水。`SurvivalDrop` 完整跑 drop / karma / feedback；
/// `CreativeCleanup` 不掉物只清状态（vanilla Creative 不掉物 + 防 index 鬼影）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakIntent {
    SurvivalStart,
    SurvivalDrop,
    SurvivalAbort,
    CreativeCleanup,
}

fn classify_break(state: DiggingState, mode: GameMode) -> Option<BreakIntent> {
    match (state, mode) {
        (DiggingState::Start, GameMode::Survival) => Some(BreakIntent::SurvivalStart),
        (DiggingState::Stop, GameMode::Survival) => Some(BreakIntent::SurvivalDrop),
        (DiggingState::Abort, GameMode::Survival) => Some(BreakIntent::SurvivalAbort),
        (DiggingState::Start, GameMode::Creative) => Some(BreakIntent::CreativeCleanup),
        _ => None,
    }
}

fn mining_session_id(pos: BlockPos, mineral_id: MineralId) -> String {
    format!("mining:{}:{}:{}:{:?}", pos.x, pos.y, pos.z, mineral_id)
}

fn mining_origin_position(pos: BlockPos) -> [f64; 3] {
    [pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5]
}

struct MiningSessionContext<'a> {
    player: Entity,
    pos: BlockPos,
    mineral_id: MineralId,
    target_name: &'a str,
    started_at_tick: u64,
    inventory: Option<&'a PlayerInventory>,
    cultivation: Option<&'a Cultivation>,
}

fn mining_gathering_session(context: &MiningSessionContext<'_>) -> GatheringSession {
    let tool = context
        .inventory
        .and_then(equipped_gathering_tool)
        .filter(|tool| tool.matches_target(GatheringTargetKind::Ore));
    let realm = context
        .cultivation
        .map(|cultivation| cultivation.realm)
        .unwrap_or(Realm::Awaken);
    GatheringSession::new(GatheringSessionStart {
        player: context.player,
        session_id: mining_session_id(context.pos, context.mineral_id),
        target: GatheringTargetKind::Ore,
        target_name: context.target_name.to_string(),
        started_at_tick: context.started_at_tick,
        origin_position: mining_origin_position(context.pos),
        tool,
        realm,
        auto_complete: false,
    })
}

fn mining_completion(
    context: &MiningSessionContext<'_>,
    now_tick: u64,
    store: Option<&mut GatheringSessionStore>,
) -> (GatheringProgressFrame, GatheringCompleteEvent) {
    let session = store
        .and_then(|store| store.remove(context.player))
        .unwrap_or_else(|| {
            let mut session = mining_gathering_session(context);
            session.started_at_tick = now_tick.saturating_sub(session.total_ticks);
            session
        });
    (
        session.progress_frame(now_tick, false, true),
        session.completion_event(now_tick),
    )
}

/// Bevy 0.14 顶层 SystemParam 有数量上限；这里按职责分组，主分支仍保持显式。
#[derive(SystemParam)]
pub(super) struct MineralBreakResources<'w> {
    gameplay_tick: Option<Res<'w, GameplayTick>>,
    index: ResMut<'w, MineralOreIndex>,
    registry: Res<'w, MineralRegistry>,
    gathering_sessions: Option<ResMut<'w, GatheringSessionStore>>,
    spirit_niches: Option<Res<'w, SpiritNicheRegistry>>,
}

#[derive(SystemParam)]
pub(super) struct MineralBreakQueries<'w, 's> {
    nodes: Query<'w, 's, &'static mut MineralOreNode>,
    dimensions: Query<'w, 's, &'static CurrentDimension>,
    game_modes: Query<'w, 's, &'static GameMode, With<Client>>,
    clients: Query<'w, 's, &'static mut Client>,
    inventories: Query<'w, 's, &'static PlayerInventory>,
    cultivations: Query<'w, 's, &'static Cultivation, With<Client>>,
    lifecycles: Query<'w, 's, &'static Lifecycle>,
}

#[derive(SystemParam)]
pub(super) struct MineralBreakEventWriters<'w> {
    drop_events: EventWriter<'w, MineralDropEvent>,
    exhausted_events: EventWriter<'w, MineralExhaustedEvent>,
    karma_events: EventWriter<'w, KarmaFlagIntent>,
    feedback_events: EventWriter<'w, MineralFeedbackEvent>,
    gathering_frames: EventWriter<'w, GatheringProgressFrame>,
    gathering_completions: EventWriter<'w, GatheringCompleteEvent>,
}

pub(super) fn handle_block_break_for_mineral(
    mut commands: Commands,
    mut digs: EventReader<DiggingEvent>,
    mut resources: MineralBreakResources,
    mut queries: MineralBreakQueries,
    mut events: MineralBreakEventWriters,
) {
    let now_tick = resources
        .gameplay_tick
        .as_deref()
        .map(GameplayTick::current_tick)
        .unwrap_or(0);
    for event in digs.read() {
        let player_mode = queries
            .game_modes
            .get(event.client)
            .copied()
            .unwrap_or_default();
        let Some(intent) = classify_break(event.state, player_mode) else {
            continue;
        };

        // 灵龛保护：niche 已登记的位置由 social 系统接管，矿脉系统两条流水都退让。
        let actor_char_id = queries
            .lifecycles
            .get(event.client)
            .ok()
            .map(|lifecycle| lifecycle.character_id.as_str());
        if resources.spirit_niches.as_deref().is_some_and(|registry| {
            block_break_is_protected_by_registered_spirit_niche(
                actor_char_id,
                [event.position.x, event.position.y, event.position.z],
                registry,
            )
        }) {
            tracing::info!(
                target: "bong::mineral",
                "block break protected by active spirit niche at {:?}",
                event.position
            );
            continue;
        }

        let dimension = queries
            .dimensions
            .get(event.client)
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some(entity) = resources.index.lookup(dimension, event.position) else {
            // 该方块不是矿脉 — 走 vanilla loot table（其他模块或默认行为决定）
            continue;
        };

        let Ok(mut node) = queries.nodes.get_mut(entity) else {
            // index 与 entity 失同步 — 清掉 stale 项以自愈
            tracing::warn!(
                target: "bong::mineral",
                "MineralOreIndex stale entry at {:?} — removing",
                event.position
            );
            resources.index.remove(dimension, event.position);
            continue;
        };

        let mineral_id = node.mineral_id;
        let Some(entry) = resources.registry.get(mineral_id) else {
            tracing::warn!(
                target: "bong::mineral",
                "MineralOreNode carries unregistered mineral_id {} at {:?}",
                mineral_id,
                event.position
            );
            // 反馈仅 Survival 玩家关心（Creative 拿不到 inventory 反馈）。
            if matches!(intent, BreakIntent::SurvivalDrop) {
                events
                    .feedback_events
                    .send(MineralFeedbackEvent::unknown_for_forge(event.client));
            }
            continue;
        };

        if matches!(intent, BreakIntent::SurvivalAbort) {
            if let Some(session) = resources
                .gathering_sessions
                .as_deref_mut()
                .and_then(|store| store.remove(event.client))
            {
                events
                    .gathering_frames
                    .send(session.progress_frame(now_tick, true, false));
                if let Ok(mut client) = queries.clients.get_mut(event.client) {
                    send_mining_progress_to_client(
                        &mut client,
                        session.session_id,
                        [event.position.x, event.position.y, event.position.z],
                        0.0,
                        true,
                        false,
                    );
                }
            }
            continue;
        }

        if matches!(intent, BreakIntent::SurvivalStart) {
            let held_tier = queries
                .inventories
                .get(event.client)
                .ok()
                .and_then(equipped_pickaxe_tier)
                .unwrap_or(0);
            if held_tier < entry.pickaxe_tier_min {
                events
                    .feedback_events
                    .send(MineralFeedbackEvent::pickaxe_tier_mismatch(
                        event.client,
                        pickaxe_tier_name(held_tier),
                        entry.display_name_zh,
                        entry.pickaxe_tier_min,
                    ));
                continue;
            }

            let context = MiningSessionContext {
                player: event.client,
                pos: event.position,
                mineral_id,
                target_name: entry.display_name_zh,
                started_at_tick: now_tick,
                inventory: queries.inventories.get(event.client).ok(),
                cultivation: queries.cultivations.get(event.client).ok(),
            };
            let session = mining_gathering_session(&context);
            if let Some(store) = resources.gathering_sessions.as_deref_mut() {
                if store.session_for(event.client).is_some() {
                    continue;
                }
                store.upsert(session.clone());
            }
            let progress_frame = session.progress_frame(now_tick, false, false);
            if let Ok(mut client) = queries.clients.get_mut(event.client) {
                send_mining_progress_to_client(
                    &mut client,
                    progress_frame.session_id.clone(),
                    [event.position.x, event.position.y, event.position.z],
                    0.0,
                    false,
                    false,
                );
            }
            events.gathering_frames.send(progress_frame);
            continue;
        }

        // Survival drop 流水：先做 pickaxe / drop / karma / mining_progress，再走通用清理。
        // Creative cleanup：跳过这一段，直接进通用清理。
        if matches!(intent, BreakIntent::SurvivalDrop) {
            let held_tier = queries
                .inventories
                .get(event.client)
                .ok()
                .and_then(equipped_pickaxe_tier)
                .unwrap_or(0);
            if held_tier < entry.pickaxe_tier_min {
                events
                    .feedback_events
                    .send(MineralFeedbackEvent::pickaxe_tier_mismatch(
                        event.client,
                        pickaxe_tier_name(held_tier),
                        entry.display_name_zh,
                        entry.pickaxe_tier_min,
                    ));
                tracing::debug!(
                    target: "bong::mineral",
                    "pickaxe tier {held_tier} < required {} for {} at {:?}",
                    entry.pickaxe_tier_min,
                    entry.canonical_name,
                    event.position
                );
                continue;
            }

            events.drop_events.send(MineralDropEvent {
                player: event.client,
                mineral_id,
                position: event.position,
            });
            let context = MiningSessionContext {
                player: event.client,
                pos: event.position,
                mineral_id,
                target_name: entry.display_name_zh,
                started_at_tick: now_tick,
                inventory: queries.inventories.get(event.client).ok(),
                cultivation: queries.cultivations.get(event.client).ok(),
            };
            let (gathering_frame, gathering_completion) = mining_completion(
                &context,
                now_tick,
                resources.gathering_sessions.as_deref_mut(),
            );
            if let Ok(mut client) = queries.clients.get_mut(event.client) {
                send_mining_progress_to_client(
                    &mut client,
                    gathering_frame.session_id.clone(),
                    [event.position.x, event.position.y, event.position.z],
                    1.0,
                    false,
                    true,
                );
            }
            events.gathering_frames.send(gathering_frame);
            events.gathering_completions.send(gathering_completion);

            let probability = karma_probability(mineral_id.rarity());
            if probability > 0.0 {
                events.karma_events.send(KarmaFlagIntent {
                    player: event.client,
                    mineral_id,
                    position: event.position,
                    probability,
                });
            }
        }

        // 通用清理（Survival 完整流程的尾段，也是 Creative 的全部动作）：减 unit，
        // 归零则发 exhausted + 移 index + despawn entity。
        node.remaining_units = node.remaining_units.saturating_sub(1);
        if node.remaining_units == 0 {
            events.exhausted_events.send(MineralExhaustedEvent {
                mineral_id,
                position: event.position,
            });
            resources.index.remove(dimension, event.position);
            commands.entity(entity).despawn();
        }
    }
}

pub fn equipped_pickaxe_tier(inventory: &PlayerInventory) -> Option<u8> {
    [EQUIP_SLOT_MAIN_HAND, EQUIP_SLOT_TWO_HAND]
        .into_iter()
        .filter_map(|slot| inventory.equipped.get(slot))
        .find_map(pickaxe_tier_from_usable_item)
}

fn pickaxe_tier_from_usable_item(item: &ItemInstance) -> Option<u8> {
    if !item.durability.is_finite() || item.durability <= 0.0 {
        return None;
    }
    pickaxe_tier_from_item(item)
}

pub fn pickaxe_tier_from_item(item: &ItemInstance) -> Option<u8> {
    let id = item.template_id.as_str();
    if id.contains("wooden_pickaxe") || id.contains("golden_pickaxe") || id == "pickaxe_bone" {
        Some(1)
    } else if id == "pickaxe_copper"
        || id.contains("stone_pickaxe")
        || id.contains("fan_iron_pickaxe")
    {
        Some(2)
    } else if id == "pickaxe_iron"
        || id.contains("iron_pickaxe")
        || id.contains("ling_iron_pickaxe")
    {
        Some(3)
    } else if id.contains("diamond_pickaxe")
        || id.contains("netherite_pickaxe")
        || id.contains("yi_pickaxe")
    {
        Some(4)
    } else {
        None
    }
}

fn send_mining_progress_to_client(
    client: &mut Client,
    session_id: String,
    ore_pos: [i32; 3],
    progress: f64,
    interrupted: bool,
    completed: bool,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::MiningProgress {
        session_id,
        ore_pos,
        progress,
        interrupted,
        completed,
    });
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::error!(
                "[bong][network] failed to serialize {payload_type} payload for {}: {:?}",
                SERVER_DATA_CHANNEL,
                error
            );
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
}

#[cfg(test)]
fn build_mining_progress_payload(
    session_id: String,
    ore_pos: [i32; 3],
    progress: f64,
    interrupted: bool,
    completed: bool,
) -> ServerDataV1 {
    ServerDataV1::new(ServerDataPayloadV1::MiningProgress {
        session_id,
        ore_pos,
        progress,
        interrupted,
        completed,
    })
}

fn pickaxe_tier_name(tier: u8) -> &'static str {
    match tier {
        1 => "凡镐",
        2 => "石镐",
        3 => "铁镐",
        4..=u8::MAX => "遗镐",
        0 => "空手",
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::MineralId;
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemRarity, PlacedItemState, MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;

    fn item(template_id: &str) -> ItemInstance {
        item_with_durability(template_id, 1.0)
    }

    fn item_with_durability(template_id: &str, durability: f64) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn inventory_with_main_hand(template_id: &str) -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert(EQUIP_SLOT_MAIN_HAND.to_string(), item(template_id));
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: MAIN_PACK_CONTAINER_ID.to_string(),
                rows: 1,
                cols: 1,
                items: Vec::<PlacedItemState>::new(),
            }],
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 10.0,
        }
    }

    #[test]
    fn karma_probability_zero_for_low_tier() {
        assert_eq!(karma_probability(MineralRarity::Fan), 0.0);
        assert_eq!(karma_probability(MineralRarity::Ling), 0.0);
    }

    #[test]
    fn karma_probability_nonzero_for_tier_3_and_4() {
        assert!(karma_probability(MineralRarity::Xi) > 0.0);
        assert!(karma_probability(MineralRarity::Yi) > karma_probability(MineralRarity::Xi));
    }

    #[test]
    fn karma_probability_per_tier_aligns_with_plan() {
        // plan §3 第 2 条：5% → 30% — 实装锚点：tier 3=15%, tier 4=30%
        assert_eq!(karma_probability(MineralId::SuiTie.rarity()), 0.15);
        assert_eq!(karma_probability(MineralId::KuJin.rarity()), 0.30);
        assert_eq!(karma_probability(MineralId::LingShiYi.rarity()), 0.30);
    }

    #[test]
    fn pickaxe_tier_from_vanilla_item_ids() {
        assert_eq!(
            pickaxe_tier_from_item(&item("minecraft:wooden_pickaxe")),
            Some(1)
        );
        assert_eq!(pickaxe_tier_from_item(&item("pickaxe_bone")), Some(1));
        assert_eq!(pickaxe_tier_from_item(&item("stone_pickaxe")), Some(2));
        assert_eq!(pickaxe_tier_from_item(&item("pickaxe_copper")), Some(2));
        assert_eq!(
            pickaxe_tier_from_item(&item("minecraft:iron_pickaxe")),
            Some(3)
        );
        assert_eq!(pickaxe_tier_from_item(&item("pickaxe_iron")), Some(3));
        assert_eq!(pickaxe_tier_from_item(&item("netherite_pickaxe")), Some(4));
        assert_eq!(pickaxe_tier_from_item(&item("iron_sword")), None);
    }

    #[test]
    fn equipped_pickaxe_tier_reads_main_hand() {
        let inv = inventory_with_main_hand("minecraft:iron_pickaxe");
        assert_eq!(equipped_pickaxe_tier(&inv), Some(3));
    }

    #[test]
    fn equipped_pickaxe_tier_reads_two_hand_when_main_hand_empty() {
        let mut inv = inventory_with_main_hand("minecraft:iron_sword");
        inv.equipped.clear();
        inv.equipped.insert(
            EQUIP_SLOT_TWO_HAND.to_string(),
            item("minecraft:diamond_pickaxe"),
        );

        assert_eq!(equipped_pickaxe_tier(&inv), Some(4));
    }

    #[test]
    fn equipped_pickaxe_tier_reads_two_hand_when_main_hand_is_not_pickaxe() {
        let mut inv = inventory_with_main_hand("minecraft:iron_sword");
        inv.equipped.insert(
            EQUIP_SLOT_TWO_HAND.to_string(),
            item("minecraft:diamond_pickaxe"),
        );

        assert_eq!(
            equipped_pickaxe_tier(&inv),
            Some(4),
            "two_hand pickaxe must count when main_hand holds a non-pickaxe item"
        );
    }

    #[test]
    fn equipped_pickaxe_tier_ignores_broken_pickaxes() {
        let mut inv = inventory_with_main_hand("minecraft:iron_pickaxe");
        inv.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            item_with_durability("minecraft:iron_pickaxe", 0.0),
        );
        inv.equipped.insert(
            EQUIP_SLOT_TWO_HAND.to_string(),
            item("minecraft:diamond_pickaxe"),
        );

        assert_eq!(
            equipped_pickaxe_tier(&inv),
            Some(4),
            "broken main_hand pickaxe must be ignored so a usable two_hand pickaxe can satisfy mining tier"
        );

        inv.equipped.remove(EQUIP_SLOT_TWO_HAND);
        assert_eq!(
            equipped_pickaxe_tier(&inv),
            None,
            "broken pickaxe must not satisfy mining tier checks"
        );
    }

    #[test]
    fn equipped_pickaxe_tier_does_not_fall_back_to_hotbar() {
        let mut inv = inventory_with_main_hand("minecraft:iron_sword");
        inv.hotbar[0] = Some(item("minecraft:netherite_pickaxe"));

        assert_eq!(equipped_pickaxe_tier(&inv), None);
    }

    #[test]
    fn mining_progress_payload_uses_existing_server_data_schema() {
        let payload = build_mining_progress_payload(
            "mining:1:64:2:FanTie".to_string(),
            [1, 64, 2],
            1.0,
            false,
            true,
        );

        let bytes = serialize_server_data_payload(&payload).expect("mining progress serializes");
        let value: serde_json::Value = serde_json::from_slice(bytes.as_slice()).unwrap();
        assert_eq!(value["type"], "mining_progress");
        assert_eq!(value["session_id"], "mining:1:64:2:FanTie");
        assert_eq!(value["ore_pos"], serde_json::json!([1, 64, 2]));
        assert_eq!(value["progress"], 1.0);
        assert_eq!(value["completed"], true);
    }

    /// 真值表：Survival Start/Stop/Abort 只管理采集进度与完成点；
    /// Creative Start 只 cleanup。Adventure/Spectator 全部跳过。
    #[test]
    fn classify_break_truth_table() {
        for state in [DiggingState::Start, DiggingState::Stop, DiggingState::Abort] {
            for mode in [
                GameMode::Survival,
                GameMode::Creative,
                GameMode::Adventure,
                GameMode::Spectator,
            ] {
                let expected = match (state, mode) {
                    (DiggingState::Start, GameMode::Survival) => Some(BreakIntent::SurvivalStart),
                    (DiggingState::Stop, GameMode::Survival) => Some(BreakIntent::SurvivalDrop),
                    (DiggingState::Abort, GameMode::Survival) => Some(BreakIntent::SurvivalAbort),
                    (DiggingState::Start, GameMode::Creative) => Some(BreakIntent::CreativeCleanup),
                    _ => None,
                };
                assert_eq!(
                    classify_break(state, mode),
                    expected,
                    "({state:?}, {mode:?}) misclassified"
                );
            }
        }
    }

    /// Creative cleanup 整链路集成测试：一次 Start/Creative 命中 ore →
    /// (1) 不发 MineralDropEvent；(2) MineralOreNode.remaining_units 减 1；
    /// (3) 不发 KarmaFlagIntent / MineralFeedbackEvent；(4) 节点未耗尽时 entity / index 仍在。
    #[test]
    fn creative_cleanup_decrements_units_without_drops() {
        use crate::mineral::components::{MineralOreIndex, MineralOreNode};
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use valence::prelude::{App, BlockPos, Events, GameMode, IntoSystemConfigs, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<DiggingEvent>();
        app.add_event::<MineralDropEvent>();
        app.add_event::<MineralExhaustedEvent>();
        app.add_event::<KarmaFlagIntent>();
        app.add_event::<MineralFeedbackEvent>();
        app.add_event::<GatheringProgressFrame>();
        app.add_event::<GatheringCompleteEvent>();
        app.insert_resource(crate::mineral::registry::build_default_registry());
        app.insert_resource(MineralOreIndex::default());
        app.add_systems(Update, handle_block_break_for_mineral.into_configs());

        let (client_bundle, _helper) = create_mock_client("Creative");
        let player = app.world_mut().spawn(client_bundle).id();
        // 覆盖 GameMode（默认 Survival）+ 挂 CurrentDimension（query 用得到）。
        app.world_mut().entity_mut(player).insert((
            GameMode::Creative,
            CurrentDimension(DimensionKind::Overworld),
        ));
        let pos = BlockPos::new(10, 64, 10);
        let mut node = MineralOreNode::new(crate::mineral::types::MineralId::FanTie, pos);
        node.remaining_units = 3;
        let ore_entity = app.world_mut().spawn(node).id();
        app.world_mut().resource_mut::<MineralOreIndex>().insert(
            DimensionKind::Overworld,
            pos,
            ore_entity,
        );

        app.world_mut().send_event(DiggingEvent {
            client: player,
            position: pos,
            direction: valence::protocol::Direction::Up,
            state: DiggingState::Start,
        });

        app.update();

        // (1) 没发 drop event
        let drops = app.world().resource::<Events<MineralDropEvent>>();
        assert_eq!(
            drops.get_reader().read(drops).count(),
            0,
            "Creative cleanup must not emit MineralDropEvent"
        );
        // (2) units 减 1
        let node_after = app.world().get::<MineralOreNode>(ore_entity).unwrap();
        assert_eq!(node_after.remaining_units, 2);
        // (3) 没发 karma / feedback
        let karma = app.world().resource::<Events<KarmaFlagIntent>>();
        assert_eq!(karma.get_reader().read(karma).count(), 0);
        let feedback = app.world().resource::<Events<MineralFeedbackEvent>>();
        assert_eq!(feedback.get_reader().read(feedback).count(), 0);
        let frames = app.world().resource::<Events<GatheringProgressFrame>>();
        assert_eq!(
            frames.get_reader().read(frames).count(),
            0,
            "Creative cleanup must not emit GatheringProgressFrame because it bypasses survival gathering UX"
        );
        // (4) entity / index 仍在（units 还没归零）
        assert!(app
            .world()
            .resource::<MineralOreIndex>()
            .lookup(DimensionKind::Overworld, pos)
            .is_some());
        let _ = ore_entity;
    }

    /// Creative cleanup 把最后一个 unit 也消掉时：必须发 MineralExhaustedEvent +
    /// 移除 MineralOreIndex 项 + despawn entity。否则 server 重启后 anchor 重撒会
    /// 把已挖空的位置当作"还可挖"，破坏 plan-mineral-v1 §M6 的耗尽语义。
    #[test]
    fn creative_cleanup_exhausts_last_unit() {
        use crate::mineral::components::{MineralOreIndex, MineralOreNode};
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use valence::prelude::{App, BlockPos, Events, GameMode, IntoSystemConfigs, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<DiggingEvent>();
        app.add_event::<MineralDropEvent>();
        app.add_event::<MineralExhaustedEvent>();
        app.add_event::<KarmaFlagIntent>();
        app.add_event::<MineralFeedbackEvent>();
        app.add_event::<GatheringProgressFrame>();
        app.add_event::<GatheringCompleteEvent>();
        app.insert_resource(crate::mineral::registry::build_default_registry());
        app.insert_resource(MineralOreIndex::default());
        app.add_systems(Update, handle_block_break_for_mineral.into_configs());

        let (client_bundle, _helper) = create_mock_client("Creative");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(player).insert((
            GameMode::Creative,
            CurrentDimension(DimensionKind::Overworld),
        ));
        let pos = BlockPos::new(10, 64, 10);
        let mut node = MineralOreNode::new(crate::mineral::types::MineralId::FanTie, pos);
        node.remaining_units = 1; // 最后一颗
        let ore_entity = app.world_mut().spawn(node).id();
        app.world_mut().resource_mut::<MineralOreIndex>().insert(
            DimensionKind::Overworld,
            pos,
            ore_entity,
        );

        app.world_mut().send_event(DiggingEvent {
            client: player,
            position: pos,
            direction: valence::protocol::Direction::Up,
            state: DiggingState::Start,
        });

        app.update();

        // exhausted event 命中
        let exhausted = app.world().resource::<Events<MineralExhaustedEvent>>();
        let exhausted_collected: Vec<_> = exhausted.get_reader().read(exhausted).cloned().collect();
        assert_eq!(exhausted_collected.len(), 1);
        assert_eq!(exhausted_collected[0].position, pos);
        // index 项已移
        assert!(app
            .world()
            .resource::<MineralOreIndex>()
            .lookup(DimensionKind::Overworld, pos)
            .is_none());
        // entity 已 despawn
        assert!(app.world().get::<MineralOreNode>(ore_entity).is_none());
    }

    /// Survival Start 只开启采集进度，不触发 drop / cleanup。回归保护：
    /// 若有人把 (Start, Survival) 误归到 SurvivalDrop，会让 drop 在挖到一半就发。
    #[test]
    fn survival_start_opens_gathering_progress_without_drop_or_cleanup() {
        use crate::mineral::components::{MineralOreIndex, MineralOreNode};
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use valence::prelude::{App, BlockPos, Events, GameMode, IntoSystemConfigs, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<DiggingEvent>();
        app.add_event::<MineralDropEvent>();
        app.add_event::<MineralExhaustedEvent>();
        app.add_event::<KarmaFlagIntent>();
        app.add_event::<MineralFeedbackEvent>();
        app.add_event::<GatheringProgressFrame>();
        app.add_event::<GatheringCompleteEvent>();
        app.insert_resource(GatheringSessionStore::default());
        app.insert_resource(crate::mineral::registry::build_default_registry());
        app.insert_resource(MineralOreIndex::default());
        app.add_systems(Update, handle_block_break_for_mineral.into_configs());

        let (client_bundle, _helper) = create_mock_client("Survivor");
        let player = app.world_mut().spawn(client_bundle).id();
        // GameMode 默认就是 Survival，仍显式设一遍防 valence 默认值漂移。
        app.world_mut().entity_mut(player).insert((
            GameMode::Survival,
            CurrentDimension(DimensionKind::Overworld),
            inventory_with_main_hand("minecraft:iron_pickaxe"),
        ));
        let pos = BlockPos::new(10, 64, 10);
        let mut node = MineralOreNode::new(crate::mineral::types::MineralId::FanTie, pos);
        node.remaining_units = 3;
        let ore_entity = app.world_mut().spawn(node).id();
        app.world_mut().resource_mut::<MineralOreIndex>().insert(
            DimensionKind::Overworld,
            pos,
            ore_entity,
        );

        app.world_mut().send_event(DiggingEvent {
            client: player,
            position: pos,
            direction: valence::protocol::Direction::Up,
            state: DiggingState::Start,
        });

        app.update();

        // 没 drop / 没 cleanup
        let drops = app.world().resource::<Events<MineralDropEvent>>();
        assert_eq!(drops.get_reader().read(drops).count(), 0);
        let node_after = app.world().get::<MineralOreNode>(ore_entity).unwrap();
        assert_eq!(
            node_after.remaining_units, 3,
            "units must not change on Survival Start"
        );
        let frames = app.world().resource::<Events<GatheringProgressFrame>>();
        let collected: Vec<_> = frames.get_reader().read(frames).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].progress_ticks, 0);
        assert_eq!(collected[0].total_ticks, 60);
        assert_eq!(collected[0].target_type, GatheringTargetKind::Ore);
        assert_eq!(collected[0].target_name, "凡铁");
        assert_eq!(collected[0].session_id, "mining:10:64:10:FanTie");
        assert_eq!(collected[0].origin_position, [10.5, 64.5, 10.5]);
        assert_eq!(collected[0].tool_used.as_deref(), Some("pickaxe_iron"));
        assert!(app
            .world()
            .resource::<GatheringSessionStore>()
            .session_for(player)
            .is_some());
    }

    #[test]
    fn survival_start_does_not_emit_new_frame_when_session_already_exists() {
        use crate::mineral::components::{MineralOreIndex, MineralOreNode};
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use valence::prelude::{App, BlockPos, Events, GameMode, IntoSystemConfigs, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<DiggingEvent>();
        app.add_event::<MineralDropEvent>();
        app.add_event::<MineralExhaustedEvent>();
        app.add_event::<KarmaFlagIntent>();
        app.add_event::<MineralFeedbackEvent>();
        app.add_event::<GatheringProgressFrame>();
        app.add_event::<GatheringCompleteEvent>();
        app.insert_resource(GatheringSessionStore::default());
        app.insert_resource(crate::mineral::registry::build_default_registry());
        app.insert_resource(MineralOreIndex::default());
        app.add_systems(Update, handle_block_break_for_mineral.into_configs());

        let (client_bundle, _helper) = create_mock_client("Survivor");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(player).insert((
            GameMode::Survival,
            CurrentDimension(DimensionKind::Overworld),
            inventory_with_main_hand("pickaxe_iron"),
        ));
        let old_session = GatheringSession::new(GatheringSessionStart {
            player,
            session_id: "mining:old".to_string(),
            target: GatheringTargetKind::Ore,
            target_name: "旧矿脉".to_string(),
            started_at_tick: 0,
            origin_position: [0.5, 64.5, 0.5],
            tool: None,
            realm: Realm::Awaken,
            auto_complete: false,
        });
        app.world_mut()
            .resource_mut::<GatheringSessionStore>()
            .upsert(old_session);

        let pos = BlockPos::new(10, 64, 10);
        let node = MineralOreNode::new(crate::mineral::types::MineralId::FanTie, pos);
        let ore_entity = app.world_mut().spawn(node).id();
        app.world_mut().resource_mut::<MineralOreIndex>().insert(
            DimensionKind::Overworld,
            pos,
            ore_entity,
        );

        app.world_mut().send_event(DiggingEvent {
            client: player,
            position: pos,
            direction: valence::protocol::Direction::Up,
            state: DiggingState::Start,
        });

        app.update();

        let frames = app.world().resource::<Events<GatheringProgressFrame>>();
        assert_eq!(
            frames.get_reader().read(frames).count(),
            0,
            "Survival Start must not emit a new frame when GatheringSessionStore still holds another active session"
        );
        let stored = app
            .world()
            .resource::<GatheringSessionStore>()
            .session_for(player)
            .expect("existing mining session should be preserved");
        assert_eq!(stored.session_id, "mining:old");
    }
}
