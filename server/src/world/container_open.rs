//! plan-placeable-container-blocks-v1 P1 — 通用世界容器打开链路。

use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, App, Client, Entity, Event, EventReader, Events, Position, Query, Res, ResMut,
    Update, Username,
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
    app.add_systems(Update, handle_container_open);
}

#[allow(clippy::too_many_arguments)]
pub fn handle_container_open(
    mut requests: EventReader<ContainerOpenRequest>,
    registry: Res<ExternalContainerRegistry>,
    mut players: Query<PlayerQueryItem<'_>>,
    mut containers: Query<(&mut ExternalContainer, &Position, Option<&CurrentDimension>)>,
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
                tracing::debug!(
                    "[bong][container_open] rejected occupied session {} by {:?}",
                    ext.session_id,
                    opened_by
                );
                client.send_chat_message("§c[容器] 有人正在翻找。");
                continue;
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
    use crate::inventory::external_container::ExternalContainerKind;
    use crate::inventory::{ContainerState, ItemInstance, ItemRarity, PlacedItemState};

    #[test]
    fn open_payload_preserves_storage_crate_source_kind_and_items() {
        let mut ext = ExternalContainer {
            session_id: 12,
            container: ContainerState {
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
