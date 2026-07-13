//! plan-supply-coffin-loot-ui P2 — 开棺交互（会话式搜刮 UI）。
//!
//! 玩家右键物资棺 → 距离校验（≤ 4 格）→ 检查无人占用 →
//! 按 grade roll loot → pack 进格子 → attach ExternalContainer →
//! 注册 session → 发 `LootContainerOpen` S2C → 发 InventorySnapshot →
//! emit `SupplyCoffinOpened` 事件。
//!
//! 棺不再即碎——由 `lifecycle.rs` 的 timeout / 距离 / 主动关闭逻辑管理销毁。

use bevy_ecs::event::EventReader;
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, Client, Commands, Entity, Event, EventWriter, Position, Query, Res, ResMut, Username,
};

use crate::cultivation::components::Cultivation;
use crate::inventory::external_container::{
    pack_loot_into_grid, ExternalContainer, ExternalContainerKind, ExternalContainerRegistry,
};
use crate::inventory::{
    ContainerState, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_AREA_RADIUS};
use crate::network::inventory_snapshot_emit::{
    item_view_from_instance, send_inventory_snapshot_to_client,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::PlayerState;
use crate::schema::inventory::PlacedInventoryItemV1;
use crate::schema::server_data::{
    LootContainerOpenV1, LootContainerSourceKindV1, ServerDataPayloadV1, ServerDataV1,
};

use super::{current_wall_clock_secs, loot::roll_loot, SupplyCoffinGrade, SupplyCoffinRegistry};

const OPEN_RANGE_BLOCKS: f64 = 4.0;
const OPEN_RANGE_TOLERANCE: f64 = 0.5;

/// C2S request emitted by `client_request_handler` when the client sends
/// `supply_coffin_open`. The handler resolves the MC protocol entity_id to an
/// ECS `Entity` and produces this event for the interact system to consume.
#[derive(Debug, Clone, Event)]
pub struct SupplyCoffinOpenRequest {
    pub client: Entity,
    pub target: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct SupplyCoffinOpened {
    #[allow(dead_code)]
    pub player: Entity,
    #[allow(dead_code)]
    pub grade: SupplyCoffinGrade,
    #[allow(dead_code)]
    pub pos: valence::prelude::DVec3,
    #[allow(dead_code)]
    pub rolled: Vec<(String, u8)>,
}

type PlayerQueryItem<'a> = (
    &'a mut PlayerInventory,
    &'a Position,
    &'a mut Client,
    &'a Username,
    &'a PlayerState,
    &'a Cultivation,
);

#[allow(clippy::too_many_arguments)]
pub fn handle_supply_coffin_interact(
    mut commands: Commands,
    mut interactions: EventReader<SupplyCoffinOpenRequest>,
    mut registry: ResMut<SupplyCoffinRegistry>,
    mut ext_registry: ResMut<ExternalContainerRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
    mut opened: EventWriter<SupplyCoffinOpened>,
    mut players: Query<PlayerQueryItem<'_>>,
    mut ext_containers: Query<&mut ExternalContainer>,
) {
    for ev in interactions.read() {
        let Some(active) = registry.active.get(&ev.target).cloned() else {
            continue;
        };

        let Ok((inventory, player_pos, mut client, username, player_state, cultivation)) =
            players.get_mut(ev.client)
        else {
            continue;
        };

        let dist = active.pos.distance(player_pos.get());
        if dist > OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE {
            tracing::debug!(
                "[bong][supply_coffin] interact rejected (out of range): grade={:?} dist={:.2}",
                active.grade,
                dist
            );
            continue;
        }

        if let Ok(mut ext) = ext_containers.get_mut(ev.target) {
            if ext.opened_by.is_some() {
                tracing::debug!(
                    "[bong][supply_coffin] interact rejected (already occupied): grade={:?}",
                    active.grade,
                );
                client.send_chat_message("§c[物资棺] 有人正在翻找。");
                continue;
            }
            // Re-lock released coffin — send existing items, don't re-roll
            ext.opened_by = Some(ev.client);
            let placed_items: Vec<PlacedInventoryItemV1> = ext
                .container
                .items
                .iter()
                .map(|p| PlacedInventoryItemV1 {
                    container_id: ext.container.id.clone(),
                    row: u64::from(p.row),
                    col: u64::from(p.col),
                    item: item_view_from_instance(&p.instance),
                })
                .collect();
            let open_payload = ServerDataV1::new(ServerDataPayloadV1::LootContainerOpen(
                LootContainerOpenV1 {
                    session_id: ext.session_id,
                    source_kind: LootContainerSourceKindV1::SupplyCoffin {
                        grade: active.grade.as_str().to_string(),
                    },
                    rows: ext.container.rows,
                    cols: ext.container.cols,
                    placed_items,
                    timeout_wall_secs: ext.timeout_wall_secs,
                },
            ));
            let payload_type = payload_type_label(open_payload.payload_type());
            match serialize_server_data_payload(&open_payload) {
                Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
                Err(e) => {
                    log_payload_build_error(payload_type, &e);
                    ext.opened_by = None;
                    continue;
                }
            }
            send_inventory_snapshot_to_client(
                ev.client,
                &mut client,
                username.as_str(),
                &inventory,
                player_state,
                cultivation,
                "supply_coffin_reopen",
            );
            tracing::info!(
                "[bong][supply_coffin] session {} re-opened {:?} by {:?}",
                ext.session_id,
                active.grade,
                ev.client
            );
            continue;
        }

        let seed = registry.next_rand_u64();
        let rolled = roll_loot(active.grade, seed);

        let (cols, rows) = active.grade.loot_grid();
        let mut container = ContainerState {
            id: "ext_pending".to_string(),
            name: format!("supply_coffin_{}", active.grade.as_str()),
            rows,
            cols,
            items: Vec::new(),
            owner_instance_id: None,
            quick_access: false, // 外部供给棺容器，非快捷来源。
        };
        pack_loot_into_grid(&mut container, &rolled, &item_registry, &mut allocator);

        let now = current_wall_clock_secs();
        let timeout_wall_secs = now + active.grade.loot_timeout_secs();
        let session_id = ext_registry.allocate_session(ev.target);
        container.id = ExternalContainer::container_id(session_id);

        let ext = ExternalContainer {
            session_id,
            container,
            opened_by: Some(ev.client),
            timeout_wall_secs,
            source_kind: ExternalContainerKind::SupplyCoffin {
                grade: active.grade,
            },
        };

        let placed_items: Vec<PlacedInventoryItemV1> = ext
            .container
            .items
            .iter()
            .map(|p| PlacedInventoryItemV1 {
                container_id: ext.container.id.clone(),
                row: u64::from(p.row),
                col: u64::from(p.col),
                item: item_view_from_instance(&p.instance),
            })
            .collect();

        let open_payload = ServerDataV1::new(ServerDataPayloadV1::LootContainerOpen(
            LootContainerOpenV1 {
                session_id,
                source_kind: LootContainerSourceKindV1::SupplyCoffin {
                    grade: active.grade.as_str().to_string(),
                },
                rows,
                cols,
                placed_items,
                timeout_wall_secs,
            },
        ));

        let payload_type = payload_type_label(open_payload.payload_type());
        match serialize_server_data_payload(&open_payload) {
            Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
            Err(e) => {
                log_payload_build_error(payload_type, &e);
                ext_registry.remove_session(session_id);
                continue;
            }
        }

        send_inventory_snapshot_to_client(
            ev.client,
            &mut client,
            username.as_str(),
            &inventory,
            player_state,
            cultivation,
            "supply_coffin_open",
        );

        commands.entity(ev.target).insert(ext);

        let open_recipe = match active.grade {
            SupplyCoffinGrade::Common => "supply_coffin_open_common",
            SupplyCoffinGrade::Rare => "supply_coffin_open_rare",
            SupplyCoffinGrade::Precious => "supply_coffin_open_precious",
        };
        audio.send(PlaySoundRecipeRequest {
            recipe_id: open_recipe.to_string(),
            instance_id: 0,
            pos: Some([
                active.pos.x as i32,
                active.pos.y as i32,
                active.pos.z as i32,
            ]),
            flag: None,
            volume_mul: 0.7,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin: active.pos,
                radius: AUDIO_AREA_RADIUS,
            },
        });

        opened.send(SupplyCoffinOpened {
            player: ev.client,
            grade: active.grade,
            pos: active.pos,
            rolled,
        });

        tracing::info!(
            "[bong][supply_coffin] session {session_id} opened {:?} at ({:.1},{:.1},{:.1}) by {:?}",
            active.grade,
            active.pos.x,
            active.pos.y,
            active.pos.z,
            ev.client
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::external_container::ExternalContainerRegistry;
    use crate::inventory::{InventoryRevision, PlacedItemState};
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use valence::prelude::{App, DVec3, Update};
    use valence::testing::{create_mock_client, MockClientHelper};

    const COFFIN_POS: DVec3 = DVec3::new(0.0, 64.0, 0.0);

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "main_pack".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::<PlacedItemState>::new(),
                owner_instance_id: None,
                quick_access: false,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
            triggered_treasures: Vec::new(),
        }
    }

    fn setup_open_app(
        player_dimension: Option<DimensionKind>,
        player_pos: DVec3,
    ) -> (App, Entity, Entity, MockClientHelper) {
        let mut app = App::new();
        app.insert_resource(ExternalContainerRegistry::default());
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_event::<SupplyCoffinOpenRequest>();
        app.add_event::<SupplyCoffinOpened>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, handle_supply_coffin_interact);

        let target = app.world_mut().spawn_empty().id();
        let mut registry =
            SupplyCoffinRegistry::new((DVec3::ZERO, DVec3::new(100.0, 100.0, 100.0)), 65.0, 0x1234);
        registry.insert_active(
            target,
            SupplyCoffinGrade::Common,
            COFFIN_POS,
            current_wall_clock_secs(),
        );
        app.insert_resource(registry);

        let (client_bundle, helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                empty_inventory(),
                PlayerState::default(),
                Cultivation::default(),
            ))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(player_pos));
        if let Some(dimension) = player_dimension {
            app.world_mut()
                .entity_mut(player)
                .insert(CurrentDimension(dimension));
        }

        (app, player, target, helper)
    }

    fn send_open(app: &mut App, player: Entity, target: Entity) {
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<SupplyCoffinOpenRequest>>()
            .send(SupplyCoffinOpenRequest {
                client: player,
                target,
            });
        app.update();
    }

    #[test]
    fn open_rejects_cross_dimension_same_xyz_without_side_effects() {
        let (mut app, player, target, _helper) =
            setup_open_app(Some(DimensionKind::Tsy), COFFIN_POS);
        let rng_before = app.world().resource::<SupplyCoffinRegistry>().rng_state;

        send_open(&mut app, player, target);

        assert!(
            app.world().get::<ExternalContainer>(target).is_none(),
            "TSY player at the same numeric XYZ must not create an Overworld supply-coffin session"
        );
        assert!(
            app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .is_empty(),
            "dimension rejection must happen before allocating a session"
        );
        assert_eq!(
            app.world().resource::<SupplyCoffinRegistry>().rng_state,
            rng_before,
            "dimension rejection must happen before rolling loot or advancing RNG"
        );
    }

    #[test]
    fn open_rejects_player_missing_current_dimension() {
        let (mut app, player, target, _helper) = setup_open_app(None, COFFIN_POS);

        send_open(&mut app, player, target);

        assert!(
            app.world().get::<ExternalContainer>(target).is_none(),
            "player without CurrentDimension must be rejected instead of implicitly treated as Overworld"
        );
        assert!(
            app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .is_empty(),
            "missing-dimension rejection must not allocate a session"
        );
    }

    #[test]
    fn open_range_accepts_exact_boundary_and_rejects_just_outside() {
        let boundary = OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE;
        let (mut at_boundary, player, target, _helper) = setup_open_app(
            Some(DimensionKind::Overworld),
            COFFIN_POS + DVec3::new(boundary, 0.0, 0.0),
        );
        send_open(&mut at_boundary, player, target);
        assert!(
            at_boundary
                .world()
                .get::<ExternalContainer>(target)
                .is_some(),
            "distance exactly {boundary} must remain inside the existing open contract"
        );

        let (mut outside, player, target, _helper) = setup_open_app(
            Some(DimensionKind::Overworld),
            COFFIN_POS + DVec3::new(boundary + 0.001, 0.0, 0.0),
        );
        send_open(&mut outside, player, target);
        assert!(
            outside.world().get::<ExternalContainer>(target).is_none(),
            "distance just beyond {boundary} must be rejected"
        );
    }

    #[test]
    fn reopen_restores_session_registry_mapping_after_distance_close() {
        let (mut app, player, target, _helper) =
            setup_open_app(Some(DimensionKind::Overworld), COFFIN_POS);
        let session_id = 77;
        app.world_mut()
            .entity_mut(target)
            .insert(ExternalContainer {
                session_id,
                container: ContainerState {
                    id: ExternalContainer::container_id(session_id),
                    name: "supply_coffin_common".to_string(),
                    rows: 3,
                    cols: 4,
                    items: Vec::new(),
                    owner_instance_id: None,
                    quick_access: false,
                },
                opened_by: None,
                timeout_wall_secs: u64::MAX,
                source_kind: ExternalContainerKind::SupplyCoffin {
                    grade: SupplyCoffinGrade::Common,
                },
            });
        assert!(
            app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .is_empty(),
            "test precondition: distance close removed the session mapping"
        );

        send_open(&mut app, player, target);

        let ext = app
            .world()
            .get::<ExternalContainer>(target)
            .expect("reopen keeps the existing external container");
        assert_eq!(
            ext.opened_by,
            Some(player),
            "reopen must reacquire the lock"
        );
        assert_eq!(
            app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .get(&session_id),
            Some(&target),
            "reopen must restore session_id -> coffin mapping so subsequent moves remain routable"
        );
    }
}
