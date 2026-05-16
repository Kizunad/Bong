use valence::prelude::{
    bevy_ecs, Added, Changed, Client, DetectChanges, Entity, Query, Ref, Username, With,
};

use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::PlayerRevived;
use crate::inventory::{
    calculate_current_weight, ItemInstance, ItemRarity, PlayerInventory, EQUIP_SLOT_BACK_PACK,
    EQUIP_SLOT_CHEST, EQUIP_SLOT_CHEST_SATCHEL, EQUIP_SLOT_FALSE_SKIN, EQUIP_SLOT_FEET,
    EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS, EQUIP_SLOT_MAIN_HAND, EQUIP_SLOT_OFF_HAND,
    EQUIP_SLOT_TREASURE_BELT_0, EQUIP_SLOT_TREASURE_BELT_1, EQUIP_SLOT_TREASURE_BELT_2,
    EQUIP_SLOT_TREASURE_BELT_3, EQUIP_SLOT_TWO_HAND, EQUIP_SLOT_WAIST_POUCH,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::{canonical_player_id, PlayerState};
use crate::schema::cultivation::realm_to_string;
use crate::schema::inventory::{
    ContainerIdV1, ContainerSnapshotV1, EquippedInventorySnapshotV1, InventoryItemViewV1,
    InventorySnapshotV1, InventoryWeightV1, ItemRarityV1, PlacedInventoryItemV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
#[cfg(test)]
use crate::world::dimension::DimensionKind;
use crate::world::season::query_season;

type JoinedClientQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a PlayerInventory,
    &'a PlayerState,
    &'a Cultivation,
);

type ChangedClientQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Ref<'a, PlayerInventory>,
    &'a PlayerState,
    &'a Cultivation,
);

pub fn emit_join_inventory_snapshots(
    mut joined_clients: Query<JoinedClientQueryItem<'_>, (With<Client>, Added<PlayerInventory>)>,
) {
    for (entity, mut client, username, inventory, player_state, cultivation) in &mut joined_clients
    {
        send_inventory_snapshot_to_client(
            entity,
            &mut client,
            username.0.as_str(),
            inventory,
            player_state,
            cultivation,
            "join",
        );
    }
}

pub fn emit_revive_inventory_resyncs(
    mut revived: bevy_ecs::event::EventReader<PlayerRevived>,
    mut clients: Query<
        (
            &mut Client,
            &Username,
            &PlayerInventory,
            &PlayerState,
            &Cultivation,
        ),
        With<Client>,
    >,
) {
    for ev in revived.read() {
        let Ok((mut client, username, inventory, player_state, cultivation)) =
            clients.get_mut(ev.entity)
        else {
            continue;
        };
        send_inventory_snapshot_to_client(
            ev.entity,
            &mut client,
            username.0.as_str(),
            inventory,
            player_state,
            cultivation,
            "revive_death_drop_resync",
        );
    }
}

pub fn emit_changed_inventory_snapshots(
    mut changed_clients: Query<
        ChangedClientQueryItem<'_>,
        (With<Client>, Changed<PlayerInventory>),
    >,
) {
    for (entity, mut client, username, inventory, player_state, cultivation) in &mut changed_clients
    {
        if inventory.is_added() {
            continue;
        }
        send_inventory_snapshot_to_client(
            entity,
            &mut client,
            username.0.as_str(),
            &inventory,
            player_state,
            cultivation,
            "inventory_changed",
        );
    }
}

/// Push a fresh inventory_snapshot payload to a single client. Used for both
/// join hydration and corrective resync after a rejected move intent.
pub(crate) fn send_inventory_snapshot_to_client(
    entity: Entity,
    client: &mut Client,
    username: &str,
    inventory: &PlayerInventory,
    player_state: &PlayerState,
    cultivation: &Cultivation,
    reason: &str,
) {
    let snapshot = build_inventory_snapshot(inventory, player_state, cultivation);
    let payload = ServerDataV1::new(ServerDataPayloadV1::InventorySnapshot(Box::new(snapshot)));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::info!(
        "[bong][network] sent {} {} payload to client entity {entity:?} for `{}` ({reason})",
        SERVER_DATA_CHANNEL,
        payload_type,
        canonical_player_id(username)
    );
}

/// Build a full inventory snapshot from current ECS state.
/// Exposed for callers that need to push a corrective resync (e.g. after a
/// rejected client move intent left the optimistic UI diverged).
pub(crate) fn build_inventory_snapshot(
    inventory: &PlayerInventory,
    player_state: &PlayerState,
    cultivation: &Cultivation,
) -> InventorySnapshotV1 {
    // Keep normalization call for future derived fields; currently unused.
    let _normalized_state = player_state.normalized();

    let mut containers = Vec::with_capacity(inventory.containers.len());
    let mut placed_items = Vec::new();

    for container in &inventory.containers {
        let container_id: ContainerIdV1 = container.id.clone();
        containers.push(ContainerSnapshotV1 {
            id: container_id.clone(),
            name: container.name.clone(),
            rows: container.rows,
            cols: container.cols,
        });

        let mut ordered_items = container.items.clone();
        ordered_items.sort_by(|left, right| {
            left.row
                .cmp(&right.row)
                .then(left.col.cmp(&right.col))
                .then(left.instance.instance_id.cmp(&right.instance.instance_id))
        });

        placed_items.extend(
            ordered_items
                .into_iter()
                .map(|placed| PlacedInventoryItemV1 {
                    container_id: container_id.clone(),
                    row: placed.row as u64,
                    col: placed.col as u64,
                    item: item_view_from_instance(&placed.instance),
                }),
        );
    }

    let equipped = EquippedInventorySnapshotV1 {
        head: equipped_slot_item(inventory, EQUIP_SLOT_HEAD),
        chest: equipped_slot_item(inventory, EQUIP_SLOT_CHEST),
        legs: equipped_slot_item(inventory, EQUIP_SLOT_LEGS),
        feet: equipped_slot_item(inventory, EQUIP_SLOT_FEET),
        false_skin: equipped_slot_item(inventory, EQUIP_SLOT_FALSE_SKIN),
        main_hand: equipped_slot_item(inventory, EQUIP_SLOT_MAIN_HAND),
        off_hand: equipped_slot_item(inventory, EQUIP_SLOT_OFF_HAND),
        two_hand: equipped_slot_item(inventory, EQUIP_SLOT_TWO_HAND),
        treasure_belt_0: equipped_slot_item(inventory, EQUIP_SLOT_TREASURE_BELT_0),
        treasure_belt_1: equipped_slot_item(inventory, EQUIP_SLOT_TREASURE_BELT_1),
        treasure_belt_2: equipped_slot_item(inventory, EQUIP_SLOT_TREASURE_BELT_2),
        treasure_belt_3: equipped_slot_item(inventory, EQUIP_SLOT_TREASURE_BELT_3),
        // plan-backpack-equip-v1 P0 — 背包装备槽。
        back_pack: equipped_slot_item(inventory, EQUIP_SLOT_BACK_PACK),
        waist_pouch: equipped_slot_item(inventory, EQUIP_SLOT_WAIST_POUCH),
        chest_satchel: equipped_slot_item(inventory, EQUIP_SLOT_CHEST_SATCHEL),
    };

    let hotbar = inventory
        .hotbar
        .iter()
        .map(|slot| slot.as_ref().map(item_view_from_instance))
        .collect::<Vec<_>>();

    let qi_max = cultivation.qi_max.max(1.0);
    let body_level = (cultivation.qi_current / qi_max).clamp(0.0, 1.0);

    InventorySnapshotV1 {
        revision: inventory.revision.0,
        containers,
        placed_items,
        equipped,
        hotbar,
        bone_coins: inventory.bone_coins,
        weight: InventoryWeightV1 {
            current: calculate_current_weight(inventory),
            max: inventory.max_weight,
        },
        realm: realm_to_string(cultivation.realm).to_string(),
        qi_current: cultivation.qi_current,
        qi_max,
        body_level,
    }
}

fn equipped_slot_item(inventory: &PlayerInventory, slot: &str) -> Option<InventoryItemViewV1> {
    inventory.equipped.get(slot).map(item_view_from_instance)
}

pub(crate) fn item_view_from_instance(item: &ItemInstance) -> InventoryItemViewV1 {
    let (scroll_kind, scroll_skill_id, scroll_xp_grant) =
        skill_scroll_metadata(item.template_id.as_str());
    InventoryItemViewV1 {
        instance_id: item.instance_id,
        item_id: item.template_id.clone(),
        display_name: item.display_name.clone(),
        grid_width: item.grid_w,
        grid_height: item.grid_h,
        weight: item.weight,
        rarity: rarity_from_runtime(item.rarity),
        description: item.description.clone(),
        stack_count: item.stack_count as u64,
        spirit_quality: item.spirit_quality,
        durability: item.durability,
        freshness: item.freshness.clone(),
        // M3a — 衍生数据由 caller 调 `enrich_with_derived_freshness` 后填；
        // 默认 None 防止未注入 registry 的 caller 漏算。
        freshness_current: None,
        // plan-mineral-v1 §2.2 — mineral_id 由 mineral::inventory_grant 系统
        // 在 MineralDropEvent 落地时写入 ItemInstance.mineral_id；此处透传到 snapshot view。
        mineral_id: item.mineral_id.clone(),
        scroll_kind,
        scroll_skill_id,
        scroll_xp_grant,
        // plan-tsy-loot-v1 §1.3 — Ancient 物品 charges 透传；非 ancient 恒为 None。
        charges: item.charges,
        forge_quality: item.forge_quality,
        forge_color: item.forge_color,
        forge_side_effects: item.forge_side_effects.clone(),
        forge_achieved_tier: item.forge_achieved_tier,
        alchemy: item.alchemy.clone(),
        lingering_owner_qi: item.lingering_owner_qi.clone(),
    }
}

fn skill_scroll_metadata(template_id: &str) -> (Option<String>, Option<String>, Option<u32>) {
    if let Some(skill_id) = woliu_scroll_skill_id(template_id) {
        return (
            Some("combat_technique".to_string()),
            Some(skill_id.to_string()),
            None,
        );
    }

    match template_id {
        "skill_scroll_herbalism_baicao_can" => (
            Some("skill_scroll".to_string()),
            Some("herbalism".to_string()),
            Some(500),
        ),
        "skill_scroll_alchemy_danhuo_can" => (
            Some("skill_scroll".to_string()),
            Some("alchemy".to_string()),
            Some(500),
        ),
        "skill_scroll_forging_duantie_can" => (
            Some("skill_scroll".to_string()),
            Some("forging".to_string()),
            Some(500),
        ),
        id if id.starts_with("recipe_scroll_") => (Some("recipe_scroll".to_string()), None, None),
        id if id.starts_with("blueprint_scroll_") => {
            (Some("blueprint_scroll".to_string()), None, None)
        }
        _ => (None, None, None),
    }
}

fn woliu_scroll_skill_id(template_id: &str) -> Option<&'static str> {
    match template_id {
        "scroll_woliu_vortex" => Some("woliu.vortex"),
        "scroll_woliu_hold" => Some("woliu.hold"),
        "scroll_woliu_burst" => Some("woliu.burst"),
        "scroll_woliu_mouth" => Some("woliu.mouth"),
        "scroll_woliu_pull" => Some("woliu.pull"),
        "scroll_woliu_heart" => Some("woliu.heart"),
        "scroll_woliu_vacuum_palm" => Some("woliu.vacuum_palm"),
        "scroll_woliu_vortex_shield" => Some("woliu.vortex_shield"),
        "scroll_woliu_vacuum_lock" => Some("woliu.vacuum_lock"),
        "scroll_woliu_vortex_resonance" => Some("woliu.vortex_resonance"),
        "scroll_woliu_turbulence_burst" => Some("woliu.turbulence_burst"),
        _ => None,
    }
}

/// plan-shelflife-v1 M3a — 用 DecayProfileRegistry + clock + 容器行为，把当下
/// `current_qi` + `track_state` 算好挂到 `view.freshness_current`。
///
/// **None 早返**：freshness 字段缺失 / profile 未在 registry / item.freshness 为 None
/// 时静默返，不修改 view（防止误覆盖）。
#[allow(dead_code)]
pub(crate) fn enrich_with_derived_freshness(
    view: &mut InventoryItemViewV1,
    registry: &crate::shelflife::DecayProfileRegistry,
    now_tick: u64,
    container_behavior: &crate::shelflife::ContainerFreshnessBehavior,
) {
    let Some(freshness) = view.freshness.as_ref() else {
        return;
    };
    let Some(profile) = registry.get(&freshness.profile) else {
        return;
    };
    let multiplier = crate::shelflife::container_storage_multiplier(container_behavior, profile);
    let season = query_season("", now_tick).season;
    let entropy_seed = view.instance_id;
    view.freshness_current = Some(crate::schema::inventory::FreshnessDerivedV1 {
        current_qi: crate::shelflife::compute_current_qi_with_season(
            freshness,
            profile,
            now_tick,
            multiplier,
            season,
            entropy_seed,
        ),
        track_state: crate::shelflife::compute_track_state_with_season(
            freshness,
            profile,
            now_tick,
            multiplier,
            season,
            entropy_seed,
        ),
    });
}

fn rarity_from_runtime(rarity: ItemRarity) -> ItemRarityV1 {
    match rarity {
        ItemRarity::Common => ItemRarityV1::Common,
        ItemRarity::Uncommon => ItemRarityV1::Uncommon,
        ItemRarity::Rare => ItemRarityV1::Rare,
        ItemRarity::Epic => ItemRarityV1::Epic,
        ItemRarity::Legendary => ItemRarityV1::Legendary,
        ItemRarity::Ancient => ItemRarityV1::Ancient,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use valence::prelude::{App, Position, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use super::*;
    use crate::inventory::InventoryDurabilityChangedEvent;
    use crate::inventory::{
        ContainerState, DroppedItemEvent, DroppedItemRecord, InventoryRevision, ItemInstance,
        ItemRarity, PlacedItemState,
    };
    use crate::schema::inventory::InventoryEventV1;

    use crate::alchemy::RecipeRegistry;
    use crate::combat::events::{ApplyStatusEffectIntent, DefenseIntent, RevivalActionIntent};
    use crate::combat::CombatClock;
    use crate::cultivation::breakthrough::BreakthroughRequest;
    use crate::cultivation::forging::ForgeRequest;
    use crate::cultivation::insight::InsightChosen;
    use crate::inventory::{DroppedLootEntry, DroppedLootRegistry, ItemRegistry};
    use crate::lingtian::events::{
        StartDrainQiRequest, StartHarvestRequest, StartPlantingRequest, StartRenewRequest,
        StartReplenishRequest, StartTillRequest,
    };
    use crate::network::client_request_handler::AlchemyMockState;
    use crate::network::dropped_loot_sync_emit;
    use crate::schema::client_request::ClientRequestV1;
    use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
    use valence::custom_payload::CustomPayloadEvent;
    use valence::prelude::{ident, IntoSystemConfigs};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<DroppedItemEvent>();
        app.add_event::<crate::inventory::InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                emit_join_inventory_snapshots,
                emit_changed_inventory_snapshots,
                crate::network::inventory_event_emit::emit_dropped_item_inventory_events,
                crate::network::inventory_event_emit::emit_durability_changed_inventory_events,
            ),
        );
        app
    }

    fn setup_app_for_dropped_loot_pickup() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(AlchemyMockState::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(RecipeRegistry::default());

        app.add_event::<CustomPayloadEvent>();
        app.add_event::<BreakthroughRequest>();
        app.add_event::<ForgeRequest>();
        app.add_event::<InsightChosen>();
        app.add_event::<DefenseIntent>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<crate::alchemy::PlaceFurnaceRequest>();
        app.add_event::<StartTillRequest>();
        app.add_event::<StartRenewRequest>();
        app.add_event::<StartPlantingRequest>();
        app.add_event::<StartHarvestRequest>();
        app.add_event::<StartReplenishRequest>();
        app.add_event::<StartDrainQiRequest>();

        // Run request handler, then broadcast dropped_loot_sync if the registry changed.
        app.add_systems(
            Update,
            (
                crate::network::client_request_handler::handle_client_request_payloads,
                dropped_loot_sync_emit::emit_join_dropped_loot_syncs,
                dropped_loot_sync_emit::emit_changed_dropped_loot_syncs,
            )
                .chain(),
        );

        app
    }

    fn spawn_client_with_state_and_inventory(
        app: &mut App,
        username: &str,
        player_state: PlayerState,
        cultivation: Cultivation,
        inventory: Option<PlayerInventory>,
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app.world_mut().spawn(client_bundle).id();

        app.world_mut()
            .entity_mut(entity)
            .insert((player_state, cultivation));
        if let Some(inventory) = inventory {
            app.world_mut().entity_mut(entity).insert(inventory);
        }

        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_inventory_snapshot_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server_data payload should decode");
            if matches!(payload.payload, ServerDataPayloadV1::InventorySnapshot(_)) {
                payloads.push(payload);
            }
        }

        payloads
    }

    fn collect_inventory_event_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server_data payload should decode");
            if matches!(payload.payload, ServerDataPayloadV1::InventoryEvent(_)) {
                payloads.push(payload);
            }
        }

        payloads
    }

    fn collect_dropped_loot_sync_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }

            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server_data payload should decode");
            if matches!(payload.payload, ServerDataPayloadV1::DroppedLootSync(_)) {
                payloads.push(payload);
            }
        }

        payloads
    }

    fn clear_all_pending_frames(helper: &mut MockClientHelper) {
        let _ = helper.collect_received();
    }

    fn make_item(
        instance_id: u64,
        template_id: &str,
        display_name: &str,
        weight: f64,
        stack_count: u32,
    ) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: display_name.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight,
            rarity: ItemRarity::Common,
            description: "fixture".to_string(),
            stack_count,
            spirit_quality: 0.5,
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

    fn make_inventory(revision: u64, include_starter_talisman: bool) -> PlayerInventory {
        let mut main_items = vec![PlacedItemState {
            row: 1,
            col: 1,
            instance: make_item(2002, "field_ration", "行军干粮", 0.4, 3),
        }];

        if include_starter_talisman {
            main_items.push(PlacedItemState {
                row: 0,
                col: 0,
                instance: make_item(2001, "starter_talisman", "启程护符", 0.2, 1),
            });
        }

        let containers = vec![
            ContainerState {
                id: "main_pack".to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: main_items,
            },
            ContainerState {
                id: "small_pouch".to_string(),
                name: "小口袋".to_string(),
                rows: 3,
                cols: 3,
                items: vec![],
            },
            ContainerState {
                id: "front_satchel".to_string(),
                name: "前挂包".to_string(),
                rows: 3,
                cols: 4,
                items: vec![PlacedItemState {
                    row: 1,
                    col: 2,
                    instance: make_item(2003, "forest_herb", "林地草药", 0.1, 5),
                }],
            },
        ];

        let mut equipped = HashMap::new();
        equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            make_item(2004, "training_blade", "训练短刃", 1.1, 1),
        );

        let mut hotbar: [Option<ItemInstance>; 9] = Default::default();
        hotbar[0] = Some(make_item(2005, "healing_draught", "疗伤药剂", 0.3, 2));

        PlayerInventory {
            revision: InventoryRevision(revision),
            containers,
            equipped,
            hotbar,
            bone_coins: 57,
            max_weight: 45.0,
        }
    }

    fn approx_eq(left: f64, right: f64) {
        assert!(
            (left - right).abs() < 1e-9,
            "expected {left} approximately equals {right}"
        );
    }

    #[test]
    fn join_emits_single_inventory_snapshot_without_cross_client_broadcast() {
        let mut app = setup_app();

        let target_state = PlayerState {
            karma: 0.1,
            inventory_score: 0.1,
        };
        let other_state = PlayerState {
            karma: 0.0,
            inventory_score: 0.2,
        };

        let (_target_entity, mut target_helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Azure",
            target_state,
            Cultivation {
                realm: crate::cultivation::components::Realm::Awaken,
                qi_current: 24.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            Some(make_inventory(11, true)),
        );
        let (_other_entity, mut other_helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Bob",
            other_state,
            Cultivation {
                realm: crate::cultivation::components::Realm::Condense,
                qi_current: 70.0,
                qi_max: 140.0,
                ..Cultivation::default()
            },
            Some(make_inventory(22, false)),
        );

        app.update();
        flush_all_client_packets(&mut app);

        let target_payloads = collect_inventory_snapshot_payloads(&mut target_helper);
        let other_payloads = collect_inventory_snapshot_payloads(&mut other_helper);

        assert_eq!(
            target_payloads.len(),
            1,
            "joined target should receive exactly one inventory_snapshot"
        );
        assert_eq!(
            other_payloads.len(),
            1,
            "other client should only receive its own inventory_snapshot"
        );

        let target_snapshot = match &target_payloads[0].payload {
            ServerDataPayloadV1::InventorySnapshot(snapshot) => snapshot,
            other => panic!("expected inventory_snapshot payload, got {other:?}"),
        };
        let other_snapshot = match &other_payloads[0].payload {
            ServerDataPayloadV1::InventorySnapshot(snapshot) => snapshot,
            other => panic!("expected inventory_snapshot payload, got {other:?}"),
        };

        assert_eq!(target_snapshot.revision, 11);
        assert_eq!(other_snapshot.revision, 22);

        assert_eq!(target_snapshot.containers.len(), 3);
        assert_eq!(target_snapshot.containers[0].id, "main_pack");
        assert_eq!(target_snapshot.containers[1].id, "small_pouch");
        assert_eq!(target_snapshot.containers[2].id, "front_satchel");

        assert_eq!(target_snapshot.placed_items[0].container_id, "main_pack");
        assert_eq!(target_snapshot.placed_items[0].row, 0);
        assert_eq!(target_snapshot.placed_items[0].col, 0);
        assert_eq!(
            target_snapshot.placed_items[0].item.item_id,
            "starter_talisman"
        );

        assert_eq!(target_snapshot.hotbar.len(), 9);
        assert_eq!(target_snapshot.bone_coins, 57);
        approx_eq(target_snapshot.weight.current, 3.6);
        approx_eq(target_snapshot.weight.max, 45.0);
        assert_eq!(target_snapshot.realm, "Awaken");
        approx_eq(target_snapshot.qi_current, 24.0);
        approx_eq(target_snapshot.qi_max, 100.0);
        approx_eq(target_snapshot.body_level, 0.24);

        let payload_json = serde_json::to_value(&target_payloads[0])
            .expect("snapshot payload should serialize to json");
        assert_eq!(
            payload_json.get("type"),
            Some(&serde_json::json!("inventory_snapshot"))
        );
        assert!(payload_json.get("revision").is_some());
        assert!(payload_json.get("containers").is_some());
        assert!(payload_json.get("placed_items").is_some());
        assert!(payload_json.get("equipped").is_some());
        assert!(payload_json.get("hotbar").is_some());
        assert!(payload_json.get("bone_coins").is_some());
        assert!(payload_json.get("weight").is_some());
        assert!(payload_json.get("realm").is_some());
        assert!(payload_json.get("qi_current").is_some());
        assert!(payload_json.get("qi_max").is_some());
        assert!(payload_json.get("body_level").is_some());
    }

    #[test]
    fn rejects_oversize_inventory_snapshot() {
        let mut app = setup_app();
        let state = PlayerState {
            karma: 0.0,
            inventory_score: 0.0,
        };

        let mut inventory = make_inventory(33, true);
        let huge = "x".repeat(20_000);
        for container in &mut inventory.containers {
            for placed in &mut container.items {
                placed.instance.description = huge.clone();
            }
        }
        for item in inventory.equipped.values_mut() {
            item.description = huge.clone();
        }
        for item in inventory.hotbar.iter_mut().flatten() {
            item.description = huge.clone();
        }

        let (_entity, mut helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Azure",
            state,
            Cultivation {
                realm: crate::cultivation::components::Realm::Awaken,
                qi_current: 24.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            Some(inventory),
        );

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_inventory_snapshot_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "oversize inventory_snapshot must be rejected without any send"
        );
    }

    #[test]
    fn dropped_item_event_emits_inventory_event_payload() {
        let mut app = setup_app();
        let state = PlayerState::default();
        let (entity, mut helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Azure",
            state,
            Cultivation::default(),
            Some(make_inventory(21, true)),
        );

        app.world_mut().send_event(DroppedItemEvent {
            entity,
            revision: InventoryRevision(21),
            dropped: vec![DroppedItemRecord {
                container_id: "main_pack".to_string(),
                row: 0,
                col: 0,
                instance: make_item(1004, "starter_talisman", "启程护符", 0.2, 1),
            }],
        });

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_inventory_event_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        match &payloads[0].payload {
            ServerDataPayloadV1::InventoryEvent(event) => match event.as_ref() {
                InventoryEventV1::Dropped {
                    revision,
                    instance_id,
                    from,
                    world_pos,
                    item,
                } => {
                    assert_eq!(*revision, 21);
                    assert_eq!(*instance_id, 1004);
                    assert!(world_pos[0] > 8.0);
                    assert_eq!(world_pos[1], 66.0);
                    assert!(world_pos[2] > 8.0);
                    assert_eq!(item.item_id, "starter_talisman");
                    assert_eq!(item.display_name, "启程护符");
                    assert_eq!(item.stack_count, 1);
                    match from {
                        crate::schema::inventory::InventoryLocationV1::Container {
                            container_id,
                            row,
                            col,
                        } => {
                            assert_eq!(*container_id, "main_pack");
                            assert_eq!(*row, 0);
                            assert_eq!(*col, 0);
                        }
                        other => panic!("expected container from location, got {other:?}"),
                    }
                }
                other => panic!("expected dropped inventory event, got {other:?}"),
            },
            other => panic!("expected dropped inventory_event payload, got {other:?}"),
        }
    }

    #[test]
    fn dropped_loot_sync_allows_other_client_to_pickup_and_removes_from_registry() {
        let mut app = setup_app_for_dropped_loot_pickup();

        // Both players have inventories/state so the request handler can resync snapshots.
        let state_a = PlayerState::default();
        let state_b = PlayerState::default();
        let inv_a = make_inventory(11, true);
        let mut inv_b = make_inventory(22, false);
        // Ensure B has at least one free slot.
        inv_b.containers[0].items.clear();

        let (_owner_entity, mut owner_helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Owner",
            state_a,
            Cultivation::default(),
            Some(inv_a),
        );
        let (picker_entity, mut picker_helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Picker",
            state_b,
            Cultivation::default(),
            Some(inv_b),
        );

        // Seed a single drop owned by Owner, placed near Picker.
        {
            let mut registry = app.world_mut().resource_mut::<DroppedLootRegistry>();
            registry.entries.insert(
                1004,
                DroppedLootEntry {
                    instance_id: 1004,
                    source_container_id: "main_pack".to_string(),
                    source_row: 0,
                    source_col: 0,
                    world_pos: [8.5, 66.0, 8.5],
                    dimension: DimensionKind::Overworld,
                    item: make_item(1004, "starter_talisman", "启程护符", 0.2, 1),
                },
            );
        }

        app.update();
        flush_all_client_packets(&mut app);

        // Both clients should see the global dropped_loot_sync on join/change.
        let owner_syncs = collect_dropped_loot_sync_payloads(&mut owner_helper);
        let picker_syncs = collect_dropped_loot_sync_payloads(&mut picker_helper);
        assert!(
            !owner_syncs.is_empty(),
            "owner should receive dropped_loot_sync"
        );
        assert!(
            !picker_syncs.is_empty(),
            "other client should receive dropped_loot_sync"
        );

        // Clear any pending frames so subsequent assertions only see pickup effects.
        clear_all_pending_frames(&mut owner_helper);
        clear_all_pending_frames(&mut picker_helper);

        // Picker sends pickup request.
        app.world_mut()
            .resource_mut::<valence::prelude::Events<CustomPayloadEvent>>()
            .send(CustomPayloadEvent {
                client: picker_entity,
                channel: ident!("bong:client_request").into(),
                data: serde_json::to_vec(&ClientRequestV1::PickupDroppedItem {
                    v: 1,
                    instance_id: 1004,
                })
                .unwrap()
                .into_boxed_slice(),
            });

        app.update();
        flush_all_client_packets(&mut app);

        // Drop should be removed from registry.
        let registry = app.world().resource::<DroppedLootRegistry>();
        let remaining = registry.entries.len();
        assert_eq!(remaining, 0, "picked up drop should be removed");

        // Both clients should receive a sync showing no drops.
        let owner_syncs = collect_dropped_loot_sync_payloads(&mut owner_helper);
        let picker_syncs = collect_dropped_loot_sync_payloads(&mut picker_helper);
        assert!(
            owner_syncs.iter().any(|payload| {
                matches!(
                    &payload.payload,
                    ServerDataPayloadV1::DroppedLootSync(sync) if sync.is_empty()
                )
            }),
            "owner should receive empty dropped_loot_sync after pickup"
        );
        assert!(
            picker_syncs.iter().any(|payload| {
                matches!(
                    &payload.payload,
                    ServerDataPayloadV1::DroppedLootSync(sync) if sync.is_empty()
                )
            }),
            "picker should receive empty dropped_loot_sync after pickup"
        );

        // Picker inventory should now contain the item.
        let picker_inv = app.world().get::<PlayerInventory>(picker_entity).unwrap();
        let has_item = picker_inv
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .any(|placed| placed.instance.instance_id == 1004);
        assert!(has_item, "picker inventory should contain picked-up item");
    }

    #[test]
    fn durability_changed_event_emits_inventory_event_payload() {
        let mut app = setup_app();
        let state = PlayerState::default();
        let (entity, mut helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Azure",
            state,
            Cultivation::default(),
            Some(make_inventory(21, true)),
        );

        app.world_mut().send_event(InventoryDurabilityChangedEvent {
            entity,
            revision: InventoryRevision(34),
            instance_id: 2004,
            durability: 0.25,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let payloads = collect_inventory_event_payloads(&mut helper);
        assert_eq!(payloads.len(), 1);
        match &payloads[0].payload {
            ServerDataPayloadV1::InventoryEvent(event) => match event.as_ref() {
                InventoryEventV1::DurabilityChanged {
                    revision,
                    instance_id,
                    durability,
                } => {
                    assert_eq!(*revision, 34);
                    assert_eq!(*instance_id, 2004);
                    approx_eq(*durability, 0.25);
                }
                other => panic!("expected durability_changed inventory event, got {other:?}"),
            },
            other => panic!("expected durability_changed inventory_event payload, got {other:?}"),
        }
    }

    #[test]
    fn item_view_marks_skill_scroll_metadata() {
        let item = make_item(
            3001,
            "skill_scroll_herbalism_baicao_can",
            "《百草图考·残》",
            0.05,
            1,
        );

        let view = item_view_from_instance(&item);
        assert_eq!(view.scroll_kind.as_deref(), Some("skill_scroll"));
        assert_eq!(view.scroll_skill_id.as_deref(), Some("herbalism"));
        assert_eq!(view.scroll_xp_grant, Some(500));
    }

    #[test]
    fn item_view_marks_recipe_and_blueprint_scroll_metadata() {
        let recipe = make_item(3002, "recipe_scroll_qixue_pill", "丹方残卷·气血丹", 0.05, 1);
        let blueprint = make_item(
            3003,
            "blueprint_scroll_bronze_tripod",
            "器图残卷·青铜鼎",
            0.08,
            1,
        );

        let recipe_view = item_view_from_instance(&recipe);
        assert_eq!(recipe_view.scroll_kind.as_deref(), Some("recipe_scroll"));
        assert!(recipe_view.scroll_skill_id.is_none());
        assert!(recipe_view.scroll_xp_grant.is_none());

        let blueprint_view = item_view_from_instance(&blueprint);
        assert_eq!(
            blueprint_view.scroll_kind.as_deref(),
            Some("blueprint_scroll")
        );
        assert!(blueprint_view.scroll_skill_id.is_none());
        assert!(blueprint_view.scroll_xp_grant.is_none());
    }

    #[test]
    fn changed_inventory_emits_fresh_snapshot() {
        let mut app = setup_app();
        let state = PlayerState {
            karma: 0.0,
            inventory_score: 0.0,
        };

        let (entity, mut helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Azure",
            state,
            Cultivation {
                realm: crate::cultivation::components::Realm::Condense,
                qi_current: 32.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            Some(make_inventory(11, true)),
        );

        app.update();
        flush_all_client_packets(&mut app);
        let initial_payloads = collect_inventory_snapshot_payloads(&mut helper);
        assert_eq!(
            initial_payloads.len(),
            1,
            "join should emit one initial snapshot"
        );

        {
            let mut inventory = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
            inventory.revision = InventoryRevision(12);
            inventory.hotbar[1] = Some(make_item(2010, "ningmai_powder", "凝脉散", 0.2, 1));
        }

        app.update();
        flush_all_client_packets(&mut app);

        let changed_payloads = collect_inventory_snapshot_payloads(&mut helper);
        assert_eq!(
            changed_payloads.len(),
            1,
            "changed inventory should emit one fresh snapshot"
        );
        let changed_snapshot = match &changed_payloads[0].payload {
            ServerDataPayloadV1::InventorySnapshot(snapshot) => snapshot,
            other => panic!("expected inventory_snapshot payload, got {other:?}"),
        };
        assert_eq!(changed_snapshot.revision, 12);
        assert_eq!(
            changed_snapshot.hotbar[1]
                .as_ref()
                .map(|item| item.item_id.as_str()),
            Some("ningmai_powder")
        );
    }

    // =========== plan-shelflife-v1 M3a — enrich_with_derived_freshness ===========

    fn make_test_item_with_freshness(
        instance_id: u64,
        profile: &crate::shelflife::DecayProfile,
        initial_qi: f32,
        created_at_tick: u64,
    ) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "ling_shi_fan".to_string(),
            display_name: "凡品灵石".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: "末法残石".to_string(),
            stack_count: 1,
            spirit_quality: 0.7,
            durability: 1.0,
            freshness: Some(crate::shelflife::Freshness::new(
                created_at_tick,
                initial_qi,
                profile,
            )),
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

    #[test]
    fn enrich_with_no_freshness_is_noop() {
        let registry = crate::shelflife::DecayProfileRegistry::new();
        let item = ItemInstance {
            instance_id: 1,
            template_id: "iron_axe".to_string(),
            display_name: "凡铁斧".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.5,
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
        };
        let mut view = item_view_from_instance(&item);
        assert!(view.freshness_current.is_none());
        enrich_with_derived_freshness(
            &mut view,
            &registry,
            10_000,
            &crate::shelflife::ContainerFreshnessBehavior::Normal,
        );
        assert!(
            view.freshness_current.is_none(),
            "no-freshness item should stay None"
        );
    }

    #[test]
    fn enrich_with_unknown_profile_is_noop() {
        let registry = crate::shelflife::DecayProfileRegistry::new(); // 空 registry
        let p = crate::shelflife::DecayProfile::Decay {
            id: crate::shelflife::DecayProfileId::new("unknown_profile"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        };
        let item = make_test_item_with_freshness(1, &p, 100.0, 0);
        let mut view = item_view_from_instance(&item);

        enrich_with_derived_freshness(
            &mut view,
            &registry,
            500,
            &crate::shelflife::ContainerFreshnessBehavior::Normal,
        );
        assert!(
            view.freshness_current.is_none(),
            "unknown profile should leave freshness_current None"
        );
    }

    #[test]
    fn enrich_with_known_profile_computes_current_and_state() {
        let p = crate::shelflife::DecayProfile::Decay {
            id: crate::shelflife::DecayProfileId::new("test_decay"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        };
        let mut registry = crate::shelflife::DecayProfileRegistry::new();
        registry.insert(p.clone()).unwrap();

        let item = make_test_item_with_freshness(1, &p, 100.0, 0);
        let mut view = item_view_from_instance(&item);

        enrich_with_derived_freshness(
            &mut view,
            &registry,
            1000,
            &crate::shelflife::ContainerFreshnessBehavior::Normal,
        );

        let derived = view.freshness_current.expect("derived should be Some");
        // 1 half_life @ Normal, then summer dispersal applies ×1.3.
        let expected = 100.0 * (0.5_f32).powf(1.3);
        assert!((derived.current_qi - expected).abs() < 1e-3);
        // Summer-shifted current stays below half headroom → Declining.
        assert_eq!(derived.track_state, crate::shelflife::TrackState::Declining);
    }

    #[test]
    fn enrich_with_freeze_container_preserves_initial_via_derive() {
        // Freeze 容器下，time-based 公式应保留 initial_qi（multiplier=0）
        let p = crate::shelflife::DecayProfile::Decay {
            id: crate::shelflife::DecayProfileId::new("test_decay"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        };
        let mut registry = crate::shelflife::DecayProfileRegistry::new();
        registry.insert(p.clone()).unwrap();

        let item = make_test_item_with_freshness(1, &p, 100.0, 0);
        let mut view = item_view_from_instance(&item);

        enrich_with_derived_freshness(
            &mut view,
            &registry,
            1_000_000,
            &crate::shelflife::ContainerFreshnessBehavior::Freeze,
        );

        let derived = view.freshness_current.expect("derived should be Some");
        assert!(
            (derived.current_qi - 100.0).abs() < 1e-3,
            "Freeze should preserve initial"
        );
        assert_eq!(derived.track_state, crate::shelflife::TrackState::Fresh);
    }

    #[test]
    fn enrich_with_spoil_profile_derives_spoiled_state() {
        // Spoil 端到端：兽肉 half_life=1000 @ spoil_threshold=20。
        // 3000 tick 在夏散 ×1.3 下约 3.9 half_lives → Spoiled。
        let p = crate::shelflife::DecayProfile::Spoil {
            id: crate::shelflife::DecayProfileId::new("test_spoil"),
            formula: crate::shelflife::DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            spoil_threshold: 20.0,
        };
        let mut registry = crate::shelflife::DecayProfileRegistry::new();
        registry.insert(p.clone()).unwrap();

        let item = make_test_item_with_freshness(1, &p, 100.0, 0);
        let mut view = item_view_from_instance(&item);

        enrich_with_derived_freshness(
            &mut view,
            &registry,
            3000,
            &crate::shelflife::ContainerFreshnessBehavior::Normal,
        );

        let derived = view.freshness_current.expect("derived should be Some");
        let expected = 100.0 * (0.5_f32).powf(3.9);
        assert!((derived.current_qi - expected).abs() < 0.1);
        assert_eq!(derived.track_state, crate::shelflife::TrackState::Spoiled);
    }

    #[test]
    fn enrich_with_age_profile_derives_peaking_state() {
        // Age 端到端：陈酒 peak@1000, bonus=0.5, window=0.1 → Peaking @ tick 1000
        let p = crate::shelflife::DecayProfile::Age {
            id: crate::shelflife::DecayProfileId::new("test_age"),
            peak_at_ticks: 1000,
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 500,
            post_peak_spoil_threshold: 30.0,
            post_peak_spoil_profile: crate::shelflife::DecayProfileId::new("test_age_post_spoil"),
        };
        let mut registry = crate::shelflife::DecayProfileRegistry::new();
        registry.insert(p.clone()).unwrap();

        let item = make_test_item_with_freshness(1, &p, 100.0, 0);
        let mut view = item_view_from_instance(&item);

        enrich_with_derived_freshness(
            &mut view,
            &registry,
            769,
            &crate::shelflife::ContainerFreshnessBehavior::Normal,
        );

        let derived = view.freshness_current.expect("derived should be Some");
        assert!((derived.current_qi - 150.0).abs() < 1e-3);
        assert_eq!(derived.track_state, crate::shelflife::TrackState::Peaking);
    }

    #[test]
    fn enrich_with_age_past_peak_derives_past_peak_state() {
        // Age 过峰：peak=1000, post_half=500, tick 1500 经夏散后 effective_dt=1950。
        let p = crate::shelflife::DecayProfile::Age {
            id: crate::shelflife::DecayProfileId::new("test_age_pp"),
            peak_at_ticks: 1000,
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 500,
            post_peak_spoil_threshold: 30.0,
            post_peak_spoil_profile: crate::shelflife::DecayProfileId::new("test_age_pp_spoil"),
        };
        let mut registry = crate::shelflife::DecayProfileRegistry::new();
        registry.insert(p.clone()).unwrap();

        let item = make_test_item_with_freshness(1, &p, 100.0, 0);
        let mut view = item_view_from_instance(&item);

        enrich_with_derived_freshness(
            &mut view,
            &registry,
            1500,
            &crate::shelflife::ContainerFreshnessBehavior::Normal,
        );

        let derived = view.freshness_current.expect("derived should be Some");
        let expected = 150.0 * (0.5_f32).powf(950.0 / 500.0);
        assert!((derived.current_qi - expected).abs() < 1e-3);
        assert_eq!(derived.track_state, crate::shelflife::TrackState::PastPeak);
    }
}
