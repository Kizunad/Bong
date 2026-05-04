//! plan-mineral-v1 §2.2 — `BlockBreakEvent`（valence `DiggingEvent`）监听器。
//!
//! 流程（plan §2.2）：
//!  1. 读取 valence `DiggingEvent` 的"挖掘完成"语义（Stop = Survival 完成）。
//!  2. 反查 `MineralOreIndex` 看 BlockPos 是否对应一个 `MineralOreNode`。
//!  3. 是 → 不让 vanilla loot table 走默认掉落，主动发 `MineralDropEvent`，
//!     让 inventory listener 按 mineral_id 写入物品 NBT。
//!  4. `remaining_units` 减一；归零则发 `MineralExhaustedEvent`、移除 entity 与 index。
//!  5. 品阶 ≥ 3 按概率 5% / 15% / 30% 发 `KarmaFlagIntent` 给天道 agent。
//!
//! 与 `inventory::DroppedItemEvent` 解耦：本系统只发 mineral_id 语义的 drop 事件，
//! 由 inventory 侧的 listener 把 mineral_id 序列化到新建 InventoryItem 的 NBT。

use valence::prelude::{
    Client, Commands, DiggingEvent, DiggingState, EventReader, EventWriter, Query, Res, ResMut,
};

use super::components::{MineralOreIndex, MineralOreNode};
use super::events::{
    KarmaFlagIntent, MineralDropEvent, MineralExhaustedEvent, MineralFeedbackEvent,
};
use super::registry::MineralRegistry;
use super::types::MineralRarity;
use crate::combat::components::Lifecycle;
use crate::inventory::{ItemInstance, PlayerInventory, EQUIP_SLOT_MAIN_HAND, EQUIP_SLOT_TWO_HAND};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::send_server_data_payload;
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

#[allow(clippy::too_many_arguments)] // Bevy system signature; queries/events stay explicit.
pub fn handle_block_break_for_mineral(
    mut commands: Commands,
    mut digs: EventReader<DiggingEvent>,
    mut nodes: Query<&mut MineralOreNode>,
    dimensions: Query<&CurrentDimension>,
    mut index: ResMut<MineralOreIndex>,
    mut drop_events: EventWriter<MineralDropEvent>,
    mut exhausted_events: EventWriter<MineralExhaustedEvent>,
    mut karma_events: EventWriter<KarmaFlagIntent>,
    mut feedback_events: EventWriter<MineralFeedbackEvent>,
    registry: Res<MineralRegistry>,
    mut clients: Query<&mut Client>,
    inventories: Query<&PlayerInventory>,
    lifecycles: Query<&Lifecycle>,
    spirit_niches: Option<valence::prelude::Res<SpiritNicheRegistry>>,
) {
    for event in digs.read() {
        // Survival 模式 Stop = 挖掘动画走完；plan §2.2 重写 drop 必须等 Stop 才触发，
        // 避免 Start/Abort 误判。Creative 模式（Start）的特例由 worldview 暂不支持。
        if event.state != DiggingState::Stop {
            continue;
        }

        let actor_char_id = lifecycles
            .get(event.client)
            .ok()
            .map(|lifecycle| lifecycle.character_id.as_str());
        if spirit_niches.as_deref().is_some_and(|registry| {
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

        let dimension = dimensions
            .get(event.client)
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some(entity) = index.lookup(dimension, event.position) else {
            // 该方块不是矿脉 — 走 vanilla loot table（其他模块或默认行为决定）
            continue;
        };

        let Ok(mut node) = nodes.get_mut(entity) else {
            // index 与 entity 失同步 — 清掉 stale 项以自愈
            tracing::warn!(
                target: "bong::mineral",
                "MineralOreIndex stale entry at {:?} — removing",
                event.position
            );
            index.remove(dimension, event.position);
            continue;
        };

        let mineral_id = node.mineral_id;
        let Some(entry) = registry.get(mineral_id) else {
            tracing::warn!(
                target: "bong::mineral",
                "MineralOreNode carries unregistered mineral_id {} at {:?}",
                mineral_id,
                event.position
            );
            feedback_events.send(MineralFeedbackEvent::unknown_for_forge(event.client));
            continue;
        };

        let held_tier = inventories
            .get(event.client)
            .ok()
            .and_then(equipped_pickaxe_tier)
            .unwrap_or(0);
        if held_tier < entry.pickaxe_tier_min {
            feedback_events.send(MineralFeedbackEvent::pickaxe_tier_mismatch(
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

        drop_events.send(MineralDropEvent {
            player: event.client,
            mineral_id,
            position: event.position,
        });
        if let Ok(mut client) = clients.get_mut(event.client) {
            send_mining_progress_to_client(
                &mut client,
                format!(
                    "mining:{}:{}:{}:{:?}",
                    event.position.x, event.position.y, event.position.z, mineral_id
                ),
                [event.position.x, event.position.y, event.position.z],
                1.0,
                false,
                true,
            );
        }

        let probability = karma_probability(mineral_id.rarity());
        if probability > 0.0 {
            karma_events.send(KarmaFlagIntent {
                player: event.client,
                mineral_id,
                position: event.position,
                probability,
            });
        }

        node.remaining_units = node.remaining_units.saturating_sub(1);
        if node.remaining_units == 0 {
            exhausted_events.send(MineralExhaustedEvent {
                mineral_id,
                position: event.position,
            });
            index.remove(dimension, event.position);
            commands.entity(entity).despawn();
        }
    }
}

pub fn equipped_pickaxe_tier(inventory: &PlayerInventory) -> Option<u8> {
    inventory
        .equipped
        .get(EQUIP_SLOT_MAIN_HAND)
        .or_else(|| inventory.equipped.get(EQUIP_SLOT_TWO_HAND))
        .and_then(pickaxe_tier_from_item)
}

pub fn pickaxe_tier_from_item(item: &ItemInstance) -> Option<u8> {
    let id = item.template_id.as_str();
    if id.contains("wooden_pickaxe") || id.contains("golden_pickaxe") {
        Some(1)
    } else if id.contains("stone_pickaxe") || id.contains("fan_iron_pickaxe") {
        Some(2)
    } else if id.contains("iron_pickaxe") || id.contains("ling_iron_pickaxe") {
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
            durability: 1.0,
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
        assert_eq!(pickaxe_tier_from_item(&item("stone_pickaxe")), Some(2));
        assert_eq!(
            pickaxe_tier_from_item(&item("minecraft:iron_pickaxe")),
            Some(3)
        );
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
}
