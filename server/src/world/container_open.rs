//! plan-placeable-container-blocks-v1 P1 — 通用世界容器打开链路。

use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, App, Client, Entity, Event, EventReader, Events, Position, Query, RemovedComponents,
    Res, ResMut, Update, Username, With,
};

use crate::cultivation::components::Cultivation;
use crate::inventory::external_container::{
    external_kind_to_source_kind, ExternalContainer, ExternalContainerRegistry,
};
use crate::inventory::PlayerInventory;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::network::inventory_snapshot_emit::{
    item_view_from_instance, send_inventory_snapshot_to_client,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::PlayerState;
use crate::schema::inventory::PlacedInventoryItemV1;
use crate::schema::server_data::{LootContainerOpenV1, ServerDataPayloadV1, ServerDataV1};
use crate::world::container_block::{container_open_audio_cue, send_container_audio};
use crate::world::dimension::{CurrentDimension, DimensionKind};

const OPEN_RANGE_BLOCKS: f64 = 4.0;
const OPEN_RANGE_TOLERANCE: f64 = 0.5;

#[derive(Debug, Clone, Event)]
pub struct ContainerOpenRequest {
    pub client: Entity,
    pub target: Entity,
}

type PlayerQueryItem<'a> = (
    &'a PlayerInventory,
    &'a Position,
    Option<&'a CurrentDimension>,
    &'a mut Client,
    &'a Username,
    &'a PlayerState,
    &'a Cultivation,
);

pub fn register(app: &mut App) {
    app.add_event::<ContainerOpenRequest>();
    app.add_systems(
        Update,
        (handle_container_open, release_disconnected_container_locks),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn handle_container_open(
    mut requests: EventReader<ContainerOpenRequest>,
    registry: Res<ExternalContainerRegistry>,
    mut players: Query<PlayerQueryItem<'_>>,
    mut containers: Query<(&mut ExternalContainer, &Position, Option<&CurrentDimension>)>,
    live_clients: Query<(), With<Client>>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
) {
    for ev in requests.read() {
        let Ok((
            inventory,
            player_pos,
            player_dimension,
            mut client,
            username,
            player_state,
            cultivation,
        )) = players.get_mut(ev.client)
        else {
            continue;
        };
        let Ok((mut ext, container_pos, container_dimension)) = containers.get_mut(ev.target)
        else {
            client.send_chat_message("§c[容器] 目标不是可打开容器。");
            continue;
        };
        if registry.sessions.get(&ext.session_id).copied() != Some(ev.target) {
            tracing::warn!(
                "[bong][container_open] rejected stale session {} for target {:?}",
                ext.session_id,
                ev.target
            );
            client.send_chat_message("§c[容器] 容器会话已失效。");
            continue;
        }
        let player_dimension = dimension_or_overworld(player_dimension);
        let container_dimension = dimension_or_overworld(container_dimension);
        if player_dimension != container_dimension {
            tracing::debug!(
                "[bong][container_open] rejected dimension mismatch: player={:?} container={:?}",
                player_dimension,
                container_dimension
            );
            continue;
        }
        let dist = container_pos.get().distance(player_pos.get());
        if dist > OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE {
            tracing::debug!(
                "[bong][container_open] rejected out of range: target={:?} dist={dist:.2}",
                ev.target
            );
            client.send_chat_message("§c[容器] 离得太远。");
            continue;
        }
        if let Some(opened_by) = ext.opened_by {
            if opened_by != ev.client {
                if live_clients.contains(opened_by) {
                    tracing::debug!(
                        "[bong][container_open] rejected occupied session {} by {:?}",
                        ext.session_id,
                        opened_by
                    );
                    client.send_chat_message("§c[容器] 有人正在翻找。");
                    continue;
                }
                // plan-bughunt-external-container-disconnect-lock — 防御性 stale-owner
                // 释放：`opened_by` 指向的 entity 已无 Client（断线/已 despawn），说明
                // `release_disconnected_container_locks` 本 tick 尚未跑到或存在其他遗漏
                // 路径。这里直接放行当前 open 请求，避免容器因残留占用锁被永久软锁。
                tracing::info!(
                    "[bong][container_open] releasing stale opened_by {:?} for session {} (no live client) and granting open to {:?}",
                    opened_by,
                    ext.session_id,
                    ev.client
                );
            }
        }

        let previous_opened_by = ext.opened_by;
        ext.opened_by = Some(ev.client);
        let (audio_recipe_id, pitch_shift) = container_open_audio_cue(&ext.source_kind);
        let open_payload = ServerDataV1::new(ServerDataPayloadV1::LootContainerOpen(
            loot_container_open_from_external_container(&ext),
        ));
        let payload_type = payload_type_label(open_payload.payload_type());
        match serialize_server_data_payload(&open_payload) {
            Ok(bytes) => send_server_data_payload(&mut client, bytes.as_slice()),
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                ext.opened_by = previous_opened_by;
                continue;
            }
        }
        send_inventory_snapshot_to_client(
            ev.client,
            &mut client,
            username.as_str(),
            inventory,
            player_state,
            cultivation,
            "container_open",
        );
        send_container_audio(
            audio_events.as_deref_mut(),
            audio_recipe_id,
            [
                container_pos.0.x.floor() as i32,
                container_pos.0.y.floor() as i32,
                container_pos.0.z.floor() as i32,
            ],
            pitch_shift,
        );
    }
}

/// plan-bughunt-external-container-disconnect-lock — 断线清理外部容器占用锁。
///
/// 普通世界容器（`StorageCrate` / `DeadDrop`）没有像 `SupplyCoffin` 那样的专属生命周期
/// tick（见 `supply_coffin::lifecycle::external_container_lifecycle_tick`，其查询以
/// `is_supply_coffin_lifecycle_managed` 过滤，显式不管这两种）。玩家打开容器后崩溃、
/// 网络中断或客户端异常退出，都无法保证 `external_container_close` C2S 被送达，
/// `ExternalContainer.opened_by` 会残留指向断线前的旧 entity；`handle_container_open`
/// 的占用检查随后会把该 entity 当作仍在翻找，导致容器永久软锁（见本 bug 的证据定位）。
///
/// 本系统消费 `RemovedComponents<Client>`，为**所有** `ExternalContainer`（不区分
/// `source_kind`）释放指向断线 entity 的占用锁。对 `SupplyCoffin` 而言，这与其专属
/// tick 里的断线释放锁逻辑重复执行但幂等安全（同一 tick 内两次置 `None` 无副作用，
/// 不影响 timeout/despawn/冷却等 `SupplyCoffin` 专属行为）。对 `DeadDrop` 而言，
/// 这里只清 UI 占用锁本身——不触碰 `ContainerBlock`、`DeadDropWard`、掉落或
/// `ExternalContainerRegistry` session 注册，破坏惩罚语义完全不变。
pub fn release_disconnected_container_locks(
    mut disconnected_clients: RemovedComponents<Client>,
    mut containers: Query<&mut ExternalContainer>,
) {
    for entity in disconnected_clients.read() {
        for mut ext in containers.iter_mut() {
            if ext.opened_by == Some(entity) {
                tracing::info!(
                    "[bong][container_open] releasing stale opened_by lock for session {} (owner {:?} disconnected)",
                    ext.session_id,
                    entity
                );
                ext.opened_by = None;
            }
        }
    }
}

pub fn loot_container_open_from_external_container(ext: &ExternalContainer) -> LootContainerOpenV1 {
    LootContainerOpenV1 {
        session_id: ext.session_id,
        source_kind: external_kind_to_source_kind(&ext.source_kind),
        rows: ext.container.rows,
        cols: ext.container.cols,
        placed_items: placed_items_from_external_container(ext),
        timeout_wall_secs: ext.timeout_wall_secs,
    }
}

fn placed_items_from_external_container(ext: &ExternalContainer) -> Vec<PlacedInventoryItemV1> {
    ext.container
        .items
        .iter()
        .map(|placed| PlacedInventoryItemV1 {
            container_id: ext.container.id.clone(),
            row: u64::from(placed.row),
            col: u64::from(placed.col),
            item: item_view_from_instance(&placed.instance),
        })
        .collect()
}

fn dimension_or_overworld(dimension: Option<&CurrentDimension>) -> DimensionKind {
    dimension
        .map(|component| component.0)
        .unwrap_or(DimensionKind::Overworld)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Cultivation;
    use crate::inventory::external_container::ExternalContainerKind;
    use crate::inventory::{ContainerState, ItemInstance, ItemRarity, PlacedItemState};
    use crate::inventory::{InventoryRevision, PlayerInventory};
    use crate::player::state::PlayerState;
    use crate::supply_coffin::SupplyCoffinGrade;
    use valence::prelude::Despawned;
    use valence::protocol::packets::play::GameMessageS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    #[test]
    fn open_payload_preserves_storage_crate_source_kind_and_items() {
        let mut ext = ExternalContainer {
            session_id: 12,
            container: ContainerState {
                quick_access: false,
                id: ExternalContainer::container_id(12),
                name: "货箱".to_string(),
                rows: 4,
                cols: 4,
                items: vec![],
                owner_instance_id: None,
            },
            opened_by: None,
            timeout_wall_secs: 0,
            source_kind: ExternalContainerKind::StorageCrate { is_herb: false },
        };
        ext.container.items.push(PlacedItemState {
            row: 2,
            col: 1,
            instance: item_instance(9001, "bone_coin_stack", 3),
        });

        let payload = loot_container_open_from_external_container(&ext);

        assert_eq!(payload.session_id, 12, "session_id must stay stable");
        assert_eq!(
            payload.source_kind,
            crate::schema::server_data::LootContainerSourceKindV1::StorageCrate { is_herb: false },
            "source_kind must mirror ExternalContainerKind::StorageCrate"
        );
        assert_eq!(
            (payload.rows, payload.cols),
            (4, 4),
            "grid size must survive"
        );
        assert_eq!(
            payload.timeout_wall_secs, 0,
            "placed containers must not invent timeout"
        );
        assert_eq!(
            payload.placed_items.len(),
            1,
            "placed item must be visible to client"
        );
        let item = &payload.placed_items[0];
        assert_eq!(
            item.container_id, "ext_12",
            "item container id must match session"
        );
        assert_eq!(
            (item.row, item.col),
            (2, 1),
            "item grid position must survive"
        );
        assert_eq!(
            item.item.item_id, "bone_coin_stack",
            "item template must survive"
        );
    }

    #[test]
    fn open_payload_preserves_dead_drop_source_kind() {
        let ext = ExternalContainer {
            session_id: 9,
            container: ContainerState {
                quick_access: false,
                id: ExternalContainer::container_id(9),
                name: "死信箱".to_string(),
                rows: 3,
                cols: 3,
                items: vec![],
                owner_instance_id: None,
            },
            opened_by: None,
            timeout_wall_secs: 0,
            source_kind: ExternalContainerKind::DeadDrop,
        };

        let payload = loot_container_open_from_external_container(&ext);

        assert_eq!(
            payload.source_kind,
            crate::schema::server_data::LootContainerSourceKindV1::DeadDrop,
            "dead drop must stay distinguishable from storage crates"
        );
        assert!(
            payload.placed_items.is_empty(),
            "empty container should open empty"
        );
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    fn spawn_test_player(
        app: &mut App,
        username: &str,
        pos: [f64; 3],
    ) -> (Entity, MockClientHelper) {
        let (mut bundle, helper) = create_mock_client(username);
        bundle.player.position = Position::new(pos);
        let entity = app
            .world_mut()
            .spawn(bundle)
            .insert((
                PlayerState::default(),
                Cultivation::default(),
                empty_inventory(),
            ))
            .id();
        (entity, helper)
    }

    fn spawn_test_container(
        app: &mut App,
        registry: &mut ExternalContainerRegistry,
        session_id: u64,
        kind: ExternalContainerKind,
        pos: [f64; 3],
    ) -> Entity {
        let ext = ExternalContainer {
            session_id,
            container: ContainerState {
                quick_access: false,
                id: ExternalContainer::container_id(session_id),
                name: "货箱".to_string(),
                rows: 4,
                cols: 4,
                items: vec![],
                owner_instance_id: None,
            },
            opened_by: None,
            timeout_wall_secs: 0,
            source_kind: kind,
        };
        let entity = app.world_mut().spawn((ext, Position::new(pos))).id();
        registry.sessions.insert(session_id, entity);
        entity
    }

    fn container_ext(app: &App, entity: Entity) -> ExternalContainer {
        app.world()
            .get::<ExternalContainer>(entity)
            .expect("container entity should still carry ExternalContainer")
            .clone()
    }

    fn collect_chat_messages(helper: &mut MockClientHelper) -> Vec<String> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                frame
                    .decode::<GameMessageS2c>()
                    .ok()
                    .map(|p| p.chat.to_legacy_lossy())
            })
            .collect()
    }

    // ── release_disconnected_container_locks ────────────────────────────────

    #[test]
    fn release_disconnected_container_locks_clears_storage_crate_opened_by() {
        let mut app = App::new();
        app.add_systems(Update, release_disconnected_container_locks);

        let (bundle, _helper) = create_mock_client("Astray");
        let player = app.world_mut().spawn(bundle).id();

        let ext = ExternalContainer {
            session_id: 1,
            container: ContainerState {
                quick_access: false,
                id: ExternalContainer::container_id(1),
                name: "货箱".to_string(),
                rows: 4,
                cols: 4,
                items: vec![],
                owner_instance_id: None,
            },
            opened_by: Some(player),
            timeout_wall_secs: 0,
            source_kind: ExternalContainerKind::StorageCrate { is_herb: false },
        };
        let container = app.world_mut().spawn(ext).id();

        app.world_mut().entity_mut(player).remove::<Client>();
        app.update();

        let ext_after = container_ext(&app, container);
        assert_eq!(
            ext_after.opened_by, None,
            "expected disconnect cleanup to release stale StorageCrate opened_by lock, actual {:?}",
            ext_after.opened_by
        );
    }

    #[test]
    fn release_disconnected_container_locks_clears_dead_drop_without_side_effects() {
        let mut app = App::new();
        app.add_systems(Update, release_disconnected_container_locks);

        let (bundle, _helper) = create_mock_client("Runner");
        let player = app.world_mut().spawn(bundle).id();

        let mut ext = ExternalContainer {
            session_id: 2,
            container: ContainerState {
                quick_access: false,
                id: ExternalContainer::container_id(2),
                name: "死信箱".to_string(),
                rows: 3,
                cols: 3,
                items: vec![],
                owner_instance_id: None,
            },
            opened_by: Some(player),
            timeout_wall_secs: 0,
            source_kind: ExternalContainerKind::DeadDrop,
        };
        ext.container.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: item_instance(4001, "bone_coin_stack", 2),
        });
        let container = app.world_mut().spawn(ext).id();

        app.world_mut().entity_mut(player).remove::<Client>();
        app.update();

        let ext_after = container_ext(&app, container);
        assert_eq!(
            ext_after.opened_by, None,
            "expected disconnect cleanup to release stale DeadDrop opened_by lock, actual {:?}",
            ext_after.opened_by
        );
        assert_eq!(
            ext_after.container.items.len(),
            1,
            "dead drop cleanup must not touch item contents / ward-break semantics, actual {} items",
            ext_after.container.items.len()
        );
        assert!(
            app.world().get::<Despawned>(container).is_none(),
            "dead drop cleanup releases only the UI occupancy lock; the container entity itself must not be despawned (breaking/ward punishment is a separate path)"
        );
    }

    #[test]
    fn release_disconnected_container_locks_covers_all_external_container_kinds() {
        let mut app = App::new();
        app.add_systems(Update, release_disconnected_container_locks);

        let (bundle, _helper) = create_mock_client("Wanderer");
        let player = app.world_mut().spawn(bundle).id();

        let make_ext = |session_id: u64, kind: ExternalContainerKind| ExternalContainer {
            session_id,
            container: ContainerState {
                quick_access: false,
                id: ExternalContainer::container_id(session_id),
                name: "test".to_string(),
                rows: 1,
                cols: 1,
                items: vec![],
                owner_instance_id: None,
            },
            opened_by: Some(player),
            timeout_wall_secs: 0,
            source_kind: kind,
        };

        let supply = app
            .world_mut()
            .spawn(make_ext(
                10,
                ExternalContainerKind::SupplyCoffin {
                    grade: SupplyCoffinGrade::Common,
                },
            ))
            .id();
        let storage_crate = app
            .world_mut()
            .spawn(make_ext(
                11,
                ExternalContainerKind::StorageCrate { is_herb: true },
            ))
            .id();
        let dead_drop = app
            .world_mut()
            .spawn(make_ext(12, ExternalContainerKind::DeadDrop))
            .id();

        app.world_mut().entity_mut(player).remove::<Client>();
        app.update();

        for (label, entity) in [
            ("supply_coffin", supply),
            ("storage_crate", storage_crate),
            ("dead_drop", dead_drop),
        ] {
            let ext = container_ext(&app, entity);
            assert_eq!(
                ext.opened_by, None,
                "expected {label} opened_by to be released regardless of source_kind, actual {:?}",
                ext.opened_by
            );
        }
    }

    #[test]
    fn release_disconnected_container_locks_ignores_locks_owned_by_still_connected_players() {
        let mut app = App::new();
        app.add_systems(Update, release_disconnected_container_locks);

        let (bundle_a, _helper_a) = create_mock_client("Alive");
        let player_a = app.world_mut().spawn(bundle_a).id();
        let (bundle_b, _helper_b) = create_mock_client("AlsoAlive");
        let player_b = app.world_mut().spawn(bundle_b).id();

        let ext = ExternalContainer {
            session_id: 20,
            container: ContainerState {
                quick_access: false,
                id: ExternalContainer::container_id(20),
                name: "货箱".to_string(),
                rows: 4,
                cols: 4,
                items: vec![],
                owner_instance_id: None,
            },
            opened_by: Some(player_b),
            timeout_wall_secs: 0,
            source_kind: ExternalContainerKind::StorageCrate { is_herb: false },
        };
        let container = app.world_mut().spawn(ext).id();

        // Only player_a disconnects; player_b (the actual lock holder) stays connected.
        app.world_mut().entity_mut(player_a).remove::<Client>();
        app.update();

        let ext_after = container_ext(&app, container);
        assert_eq!(
            ext_after.opened_by,
            Some(player_b),
            "disconnecting an unrelated player must not release a lock held by a still-connected owner, actual {:?}",
            ext_after.opened_by
        );
    }

    // ── handle_container_open × disconnect lock release (integration) ──────

    #[test]
    fn disconnect_then_second_player_and_reconnected_owner_can_reopen_crate() {
        let mut app = App::new();
        app.add_event::<ContainerOpenRequest>();
        app.add_systems(
            Update,
            (handle_container_open, release_disconnected_container_locks),
        );

        let mut registry = ExternalContainerRegistry::default();
        let container = spawn_test_container(
            &mut app,
            &mut registry,
            0,
            ExternalContainerKind::StorageCrate { is_herb: false },
            [0.0, 64.0, 0.0],
        );
        app.insert_resource(registry);

        let (player_a, _helper_a) = spawn_test_player(&mut app, "Astray", [1.0, 64.0, 0.0]);

        // A opens successfully.
        app.world_mut().send_event(ContainerOpenRequest {
            client: player_a,
            target: container,
        });
        app.update();
        assert_eq!(
            container_ext(&app, container).opened_by,
            Some(player_a),
            "A should hold the lock immediately after opening"
        );

        // A crashes / disconnects without ever sending external_container_close.
        app.world_mut().entity_mut(player_a).remove::<Client>();
        app.update();
        assert_eq!(
            container_ext(&app, container).opened_by,
            None,
            "disconnect cleanup should release A's stale lock"
        );

        // B (a different player in range) opens the same crate — must succeed now,
        // not be told "有人正在翻找" against A's stale, disconnected entity.
        let (player_b, _helper_b) = spawn_test_player(&mut app, "Bramble", [1.0, 64.0, 0.0]);
        app.world_mut().send_event(ContainerOpenRequest {
            client: player_b,
            target: container,
        });
        app.update();
        assert_eq!(
            container_ext(&app, container).opened_by,
            Some(player_b),
            "B must be able to open the crate after A's disconnect released the stale lock"
        );

        // Model B finishing up (the close C2S handler itself is out of scope here;
        // directly clearing opened_by mirrors its one effect on this component).
        app.world_mut()
            .entity_mut(container)
            .get_mut::<ExternalContainer>()
            .expect("container should still exist")
            .opened_by = None;

        // The original player reconnects — this allocates a brand-new ECS entity,
        // never reusing player_a. The new entity must also be able to open the crate.
        let (player_a_reconnected, _helper_a2) =
            spawn_test_player(&mut app, "Astray", [1.0, 64.0, 0.0]);
        assert_ne!(
            player_a_reconnected, player_a,
            "reconnecting must allocate a new ECS entity, not resurrect the old one"
        );
        app.world_mut().send_event(ContainerOpenRequest {
            client: player_a_reconnected,
            target: container,
        });
        app.update();
        assert_eq!(
            container_ext(&app, container).opened_by,
            Some(player_a_reconnected),
            "the original owner reconnecting as a new entity must also be able to reopen the crate"
        );
    }

    #[test]
    fn handle_container_open_still_rejects_second_player_while_owner_is_connected() {
        let mut app = App::new();
        app.add_event::<ContainerOpenRequest>();
        app.add_systems(
            Update,
            (handle_container_open, release_disconnected_container_locks),
        );

        let mut registry = ExternalContainerRegistry::default();
        let container = spawn_test_container(
            &mut app,
            &mut registry,
            0,
            ExternalContainerKind::StorageCrate { is_herb: false },
            [0.0, 64.0, 0.0],
        );
        app.insert_resource(registry);

        let (player_a, _helper_a) = spawn_test_player(&mut app, "Astray", [1.0, 64.0, 0.0]);
        let (player_b, mut helper_b) = spawn_test_player(&mut app, "Bramble", [1.0, 64.0, 0.0]);

        app.world_mut().send_event(ContainerOpenRequest {
            client: player_a,
            target: container,
        });
        app.update();
        assert_eq!(container_ext(&app, container).opened_by, Some(player_a));

        // A stays connected (no Client removal) — B must still be rejected.
        app.world_mut().send_event(ContainerOpenRequest {
            client: player_b,
            target: container,
        });
        app.update();
        assert_eq!(
            container_ext(&app, container).opened_by,
            Some(player_a),
            "opened_by must stay with A while A is still connected; B's request must be rejected"
        );

        // Mock clients buffer packets until explicitly flushed; without this the
        // helper sees an empty stream even though send_chat_message was called
        // (same pattern as cmd::completions tests).
        {
            let mut client = app
                .world_mut()
                .get_mut::<Client>(player_b)
                .expect("player B should still have a Client");
            client
                .flush_packets()
                .expect("mock client flush should succeed");
        }
        let messages = collect_chat_messages(&mut helper_b);
        assert!(
            messages.iter().any(|m| m.contains("有人正在翻找")),
            "expected B to receive the occupied-container rejection chat message, actual messages: {messages:?}"
        );
    }

    #[test]
    fn handle_container_open_defensively_releases_stale_lock_even_without_cleanup_system() {
        let mut app = App::new();
        app.add_event::<ContainerOpenRequest>();
        // Deliberately register ONLY handle_container_open, not
        // release_disconnected_container_locks — this proves the defensive
        // stale-owner check inside handle_container_open is a real second line
        // of defense, not something that only works because the dedicated
        // cleanup system already ran first.
        app.add_systems(Update, handle_container_open);

        let mut registry = ExternalContainerRegistry::default();
        let container = spawn_test_container(
            &mut app,
            &mut registry,
            0,
            ExternalContainerKind::StorageCrate { is_herb: false },
            [0.0, 64.0, 0.0],
        );
        app.insert_resource(registry);

        let (player_a, _helper_a) = spawn_test_player(&mut app, "Astray", [1.0, 64.0, 0.0]);
        app.world_mut()
            .entity_mut(container)
            .get_mut::<ExternalContainer>()
            .expect("container should still exist")
            .opened_by = Some(player_a);
        // A disconnects (Client removed), but no cleanup system is registered to react.
        app.world_mut().entity_mut(player_a).remove::<Client>();

        let (player_b, _helper_b) = spawn_test_player(&mut app, "Bramble", [1.0, 64.0, 0.0]);
        app.world_mut().send_event(ContainerOpenRequest {
            client: player_b,
            target: container,
        });
        app.update();

        assert_eq!(
            container_ext(&app, container).opened_by,
            Some(player_b),
            "handle_container_open must defensively release a lock held by a disconnected \
             (no-Client) entity even when the dedicated cleanup system never ran"
        );
    }

    fn item_instance(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count,
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
}
