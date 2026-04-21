use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Added, Client, Entity, Query, Username, With};

use crate::cultivation::death_hooks::PlayerRevived;
use crate::inventory::{
    calculate_current_weight, ContainerState, ItemInstance, ItemRarity, PlayerInventory,
    EQUIP_SLOT_CHEST, EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS, EQUIP_SLOT_MAIN_HAND,
    EQUIP_SLOT_OFF_HAND, EQUIP_SLOT_TWO_HAND, FRONT_SATCHEL_CONTAINER_ID, MAIN_PACK_CONTAINER_ID,
    SMALL_POUCH_CONTAINER_ID,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::{canonical_player_id, PlayerState};
use crate::schema::inventory::{
    ContainerIdV1, ContainerSnapshotV1, EquippedInventorySnapshotV1, InventoryItemViewV1,
    InventorySnapshotV1, InventoryWeightV1, ItemRarityV1, PlacedInventoryItemV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

const ORDERED_CONTAINER_IDS: [&str; 3] = [
    MAIN_PACK_CONTAINER_ID,
    SMALL_POUCH_CONTAINER_ID,
    FRONT_SATCHEL_CONTAINER_ID,
];

type JoinedClientQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a PlayerInventory,
    &'a PlayerState,
);

pub fn emit_join_inventory_snapshots(
    mut joined_clients: Query<JoinedClientQueryItem<'_>, (With<Client>, Added<PlayerInventory>)>,
) {
    for (entity, mut client, username, inventory, player_state) in &mut joined_clients {
        send_inventory_snapshot_to_client(
            entity,
            &mut client,
            username.0.as_str(),
            inventory,
            player_state,
            "join",
        );
    }
}

pub fn emit_revive_inventory_resyncs(
    mut revived: bevy_ecs::event::EventReader<PlayerRevived>,
    mut clients: Query<(&mut Client, &Username, &PlayerInventory, &PlayerState), With<Client>>,
) {
    for ev in revived.read() {
        let Ok((mut client, username, inventory, player_state)) = clients.get_mut(ev.entity) else {
            continue;
        };
        send_inventory_snapshot_to_client(
            ev.entity,
            &mut client,
            username.0.as_str(),
            inventory,
            player_state,
            "revive_death_drop_resync",
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
    reason: &str,
) {
    let snapshot = build_inventory_snapshot(inventory, player_state);
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
) -> InventorySnapshotV1 {
    let normalized_state = player_state.normalized();
    let containers_by_id: HashMap<&str, &ContainerState> = inventory
        .containers
        .iter()
        .map(|container| (container.id.as_str(), container))
        .collect();

    let mut containers = Vec::with_capacity(ORDERED_CONTAINER_IDS.len());
    let mut placed_items = Vec::new();

    for ordered_container_id in ORDERED_CONTAINER_IDS {
        let Some(container) = containers_by_id.get(ordered_container_id).copied() else {
            continue;
        };

        let container_id = container_id_from_runtime(ordered_container_id);
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
        main_hand: equipped_slot_item(inventory, EQUIP_SLOT_MAIN_HAND),
        off_hand: equipped_slot_item(inventory, EQUIP_SLOT_OFF_HAND),
        two_hand: equipped_slot_item(inventory, EQUIP_SLOT_TWO_HAND),
    };

    let hotbar = inventory
        .hotbar
        .iter()
        .map(|slot| slot.as_ref().map(item_view_from_instance))
        .collect::<Vec<_>>();

    let body_level = if normalized_state.spirit_qi_max <= 0.0 {
        0.0
    } else {
        (normalized_state.spirit_qi / normalized_state.spirit_qi_max).clamp(0.0, 1.0)
    };

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
        realm: normalized_state.realm,
        qi_current: normalized_state.spirit_qi,
        qi_max: normalized_state.spirit_qi_max,
        body_level,
    }
}

fn equipped_slot_item(inventory: &PlayerInventory, slot: &str) -> Option<InventoryItemViewV1> {
    inventory.equipped.get(slot).map(item_view_from_instance)
}

fn container_id_from_runtime(container_id: &str) -> ContainerIdV1 {
    match container_id {
        MAIN_PACK_CONTAINER_ID => ContainerIdV1::MainPack,
        SMALL_POUCH_CONTAINER_ID => ContainerIdV1::SmallPouch,
        FRONT_SATCHEL_CONTAINER_ID => ContainerIdV1::FrontSatchel,
        _ => ContainerIdV1::MainPack,
    }
}

pub(crate) fn item_view_from_instance(item: &ItemInstance) -> InventoryItemViewV1 {
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
    }
}

/// plan-shelflife-v1 M3a — 用 DecayProfileRegistry + clock + 容器行为，把当下
/// `current_qi` + `track_state` 算好挂到 `view.freshness_current`。
///
/// **None 早返**：freshness 字段缺失 / profile 未在 registry / item.freshness 为 None
/// 时静默返，不修改 view（防止误覆盖）。
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
    view.freshness_current = Some(crate::schema::inventory::FreshnessDerivedV1 {
        current_qi: crate::shelflife::compute_current_qi(freshness, profile, now_tick, multiplier),
        track_state: crate::shelflife::compute_track_state(
            freshness, profile, now_tick, multiplier,
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
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use valence::prelude::{App, Position, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use super::*;
    use crate::inventory::{
        ContainerState, DroppedItemEvent, DroppedItemRecord, InventoryRevision, ItemInstance,
        ItemRarity, PlacedItemState,
    };
    use crate::schema::inventory::InventoryEventV1;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<DroppedItemEvent>();
        app.add_systems(
            Update,
            (
                emit_join_inventory_snapshots,
                crate::network::inventory_event_emit::emit_dropped_item_inventory_events,
            ),
        );
        app
    }

    fn spawn_client_with_state_and_inventory(
        app: &mut App,
        username: &str,
        player_state: PlayerState,
        inventory: Option<PlayerInventory>,
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app.world_mut().spawn(client_bundle).id();

        app.world_mut().entity_mut(entity).insert(player_state);
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
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: main_items,
            },
            ContainerState {
                id: SMALL_POUCH_CONTAINER_ID.to_string(),
                name: "小口袋".to_string(),
                rows: 3,
                cols: 3,
                items: vec![],
            },
            ContainerState {
                id: FRONT_SATCHEL_CONTAINER_ID.to_string(),
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
            realm: "qi_refining_1".to_string(),
            spirit_qi: 24.0,
            spirit_qi_max: 100.0,
            karma: 0.1,
            experience: 10,
            inventory_score: 0.1,
        };
        let other_state = PlayerState {
            realm: "qi_refining_3".to_string(),
            spirit_qi: 70.0,
            spirit_qi_max: 140.0,
            karma: 0.0,
            experience: 22,
            inventory_score: 0.2,
        };

        let (_target_entity, mut target_helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Azure",
            target_state,
            Some(make_inventory(11, true)),
        );
        let (_other_entity, mut other_helper) = spawn_client_with_state_and_inventory(
            &mut app,
            "Bob",
            other_state,
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
        assert_eq!(target_snapshot.containers[0].id, ContainerIdV1::MainPack);
        assert_eq!(target_snapshot.containers[1].id, ContainerIdV1::SmallPouch);
        assert_eq!(
            target_snapshot.containers[2].id,
            ContainerIdV1::FrontSatchel
        );

        assert_eq!(
            target_snapshot.placed_items[0].container_id,
            ContainerIdV1::MainPack
        );
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
        assert_eq!(target_snapshot.realm, "qi_refining_1");
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
            realm: "qi_refining_1".to_string(),
            spirit_qi: 24.0,
            spirit_qi_max: 100.0,
            karma: 0.0,
            experience: 1,
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

        let (_entity, mut helper) =
            spawn_client_with_state_and_inventory(&mut app, "Azure", state, Some(inventory));

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
            Some(make_inventory(21, true)),
        );

        app.world_mut().send_event(DroppedItemEvent {
            entity,
            revision: InventoryRevision(21),
            dropped: vec![DroppedItemRecord {
                container_id: MAIN_PACK_CONTAINER_ID.to_string(),
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
            ServerDataPayloadV1::InventoryEvent(InventoryEventV1::Dropped {
                revision,
                instance_id,
                from,
                world_pos,
                item,
            }) => {
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
                        assert_eq!(
                            *container_id,
                            crate::schema::inventory::ContainerIdV1::MainPack
                        );
                        assert_eq!(*row, 0);
                        assert_eq!(*col, 0);
                    }
                    other => panic!("expected container from location, got {other:?}"),
                }
            }
            other => panic!("expected dropped inventory_event payload, got {other:?}"),
        }
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
        // 1 half_life @ Normal multiplier 1.0 → current = 50
        assert!((derived.current_qi - 50.0).abs() < 1e-3);
        // 50/100 = 0.5 → Declining (headroom-based)
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
}
