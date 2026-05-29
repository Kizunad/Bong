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
    ext_containers: Query<&ExternalContainer>,
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

        if let Ok(ext) = ext_containers.get(ev.target) {
            if ext.opened_by.is_some() {
                tracing::debug!(
                    "[bong][supply_coffin] interact rejected (already occupied): grade={:?}",
                    active.grade,
                );
                client.send_chat_message("§c[物资棺] 有人正在翻找。");
                continue;
            }
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
