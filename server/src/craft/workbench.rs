//! plan-workbench-recipes-v1 §P0.1 — 制作台方块定义 + 放置/交互/拆除系统。
//!
//! `WorkbenchBlock` 是 ECS entity 上的 component，代表世界中放置的制作台方块。
//! 玩家右键制作台 → Server 发 `WorkbenchOpenPayload` → Client 打开 WorkbenchScreen。
//!
//! **放置**：消耗背包中 `workbench_item` × 1 → spawn block + entity。
//! **拆除**：左键长按 3s → 回收 `workbench_item` × 1 → despawn。
//! **交互**：右键 → 发送 payload 给客户端。
//!
//! §8.1 #3 决议：不设 per-chunk 数量限制（制作台是凡物，材料成本已限制滥放）。

use valence::prelude::{
    bevy_ecs, App, BlockPos, Client, Commands, Component, DVec3, DiggingEvent, Entity, Event,
    EventReader, Events, GameMode, Position, Query, Res, ResMut, Update, Username, With,
};

use crate::cultivation::components::Cultivation;
use crate::inventory::{
    add_item_to_player_inventory, spawn_template_dropped_loot, DroppedLootRegistry,
    InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory, TemplateDroppedLootRequest,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_AREA_RADIUS};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::gameplay::GameplayTick;
use crate::player::state::PlayerState;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::world::block_break::should_apply_default_break;
use crate::world::block_place::{break_placeable, PlaceableBlockKind};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::entity_model::{spawn_visual_marker, BongVisualKind};

/// 制作台方块 ECS component。
///
/// 放置时 spawn 到方块 entity 上，拆除时 despawn 整个 entity。
#[derive(Debug, Clone, Component, PartialEq)]
pub struct WorkbenchBlock {
    /// 放置该制作台的玩家 entity。
    pub placed_by: Entity,
    /// 放置时的 tick 时戳。
    pub placed_at_tick: u64,
}

/// 制作台交互 payload（server → client），通知客户端打开 WorkbenchScreen。
///
/// 当前仅携带制作台 entity 信息；后续 PR-3 client 端消费此 payload。
#[derive(Debug, Clone, PartialEq)]
pub struct WorkbenchOpenPayload {
    /// 制作台 entity。
    pub workbench_entity: Entity,
    /// 制作台世界坐标（方块坐标）。
    pub position: [i32; 3],
}

/// 制作台最大交互距离（方块数）。
///
/// §P2.4 & §8.1 #2：recipe.station == Some(Workbench) 时，玩家必须在此距离内。
pub const WORKBENCH_INTERACT_RANGE: f64 = 3.0;

/// 制作台物品 template_id。
pub const WORKBENCH_ITEM_TEMPLATE: &str = "workbench_item";
pub const WORKBENCH_PLACE_AUDIO_RECIPE_ID: &str = "workbench_place";
pub const WORKBENCH_BREAK_AUDIO_RECIPE_ID: &str = "workbench_break";
pub const WORKBENCH_OPEN_AUDIO_RECIPE_ID: &str = "workbench_open";

#[derive(Debug, Clone, Copy, Event)]
pub struct WorkbenchOpenRequest {
    pub client: Entity,
    pub workbench: Entity,
}

/// 检查玩家位置是否在制作台 3 格交互范围内。
///
/// 使用 Chebyshev 距离（各轴最大分量 ≤ 3，与 MC 方块交互判定一致）。
pub fn is_within_workbench_range(player_pos: [f64; 3], workbench_pos: [i32; 3]) -> bool {
    let dx = (player_pos[0] - workbench_pos[0] as f64).abs();
    let dy = (player_pos[1] - workbench_pos[1] as f64).abs();
    let dz = (player_pos[2] - workbench_pos[2] as f64).abs();
    // 使用 Chebyshev 距离（MC 式方块交互范围）
    dx.max(dy).max(dz) <= WORKBENCH_INTERACT_RANGE
}

pub fn register(app: &mut App) {
    app.add_event::<WorkbenchOpenRequest>()
        .add_systems(Update, (handle_workbench_interact, handle_workbench_break));
}

pub fn handle_workbench_place(
    commands: &mut Commands,
    layer: Entity,
    placed_by: Entity,
    pos: BlockPos,
    placed_at_tick: u64,
) -> Entity {
    let visual = spawn_visual_marker(
        commands,
        layer,
        None,
        BongVisualKind::Workbench,
        DVec3::new(
            f64::from(pos.x) + 0.5,
            f64::from(pos.y),
            f64::from(pos.z) + 0.5,
        ),
        0,
    );
    commands.entity(visual).insert(WorkbenchBlock {
        placed_by,
        placed_at_tick,
    });
    visual
}

pub fn handle_workbench_interact(
    mut requests: EventReader<WorkbenchOpenRequest>,
    mut clients: Query<&mut Client>,
    players: Query<&Position, With<Client>>,
    workbenches: Query<(&Position, &WorkbenchBlock)>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
) {
    for request in requests.read() {
        let Ok(player_pos) = players.get(request.client) else {
            continue;
        };
        let Ok((workbench_pos, _workbench)) = workbenches.get(request.workbench) else {
            continue;
        };
        let block_pos = workbench_block_pos(workbench_pos);
        if !is_within_workbench_range([player_pos.0.x, player_pos.0.y, player_pos.0.z], block_pos) {
            tracing::warn!(
                "[bong][workbench] rejected open: client={:?} workbench={:?} out of range",
                request.client,
                request.workbench
            );
            continue;
        }
        let Ok(mut client) = clients.get_mut(request.client) else {
            continue;
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::WorkbenchOpen {
            entity_id: request.workbench.to_bits(),
            position: block_pos,
        });
        let payload_type = payload_type_label(payload.payload_type());
        let bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(&mut client, bytes.as_slice());
        send_workbench_audio(
            audio_events.as_deref_mut(),
            WORKBENCH_OPEN_AUDIO_RECIPE_ID,
            block_pos,
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn handle_workbench_break(
    mut commands: Commands,
    mut digs: EventReader<DiggingEvent>,
    item_registry: Res<ItemRegistry>,
    current_tick: Option<Res<GameplayTick>>,
    mut instance_allocator: ResMut<InventoryInstanceIdAllocator>,
    mut dropped_registry: ResMut<DroppedLootRegistry>,
    mut players: Query<
        (
            &GameMode,
            &Position,
            Option<&CurrentDimension>,
            &mut PlayerInventory,
            &Username,
            &mut Client,
            &PlayerState,
            Option<&Cultivation>,
        ),
        With<Client>,
    >,
    workbenches: Query<(Entity, &Position, &WorkbenchBlock)>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
) {
    for event in digs.read() {
        let Ok((
            game_mode,
            player_position,
            current_dimension,
            mut inventory,
            username,
            mut client,
            player_state,
            cultivation,
        )) = players.get_mut(event.client)
        else {
            continue;
        };
        if !should_apply_default_break(event.state, *game_mode) {
            continue;
        }
        let Some((workbench_entity, workbench_position, _workbench)) =
            workbenches.iter().find(|(_, position, _)| {
                workbench_block_pos(position)
                    == [event.position.x, event.position.y, event.position.z]
            })
        else {
            continue;
        };

        let now = current_tick
            .as_ref()
            .map(|tick| tick.current_tick())
            .unwrap_or(0);
        let dimension = current_dimension
            .map(|component| component.0)
            .unwrap_or(DimensionKind::Overworld);
        let default_cultivation;
        let cultivation = match cultivation {
            Some(cultivation) => cultivation,
            None => {
                default_cultivation = Cultivation::default();
                &default_cultivation
            }
        };

        match add_item_to_player_inventory(
            &mut inventory,
            &item_registry,
            &mut instance_allocator,
            WORKBENCH_ITEM_TEMPLATE,
            1,
            now,
        ) {
            Ok(_) => {
                send_inventory_snapshot_to_client(
                    event.client,
                    &mut client,
                    username.0.as_str(),
                    &inventory,
                    player_state,
                    cultivation,
                    "workbench_break_returned",
                );
            }
            Err(error) => {
                tracing::warn!(
                    "[bong][workbench] inventory return failed, spawning drop: client={:?} error={error}",
                    event.client
                );
                if let Err(drop_error) = spawn_template_dropped_loot(
                    &mut dropped_registry,
                    &item_registry,
                    &mut instance_allocator,
                    TemplateDroppedLootRequest {
                        template_id: WORKBENCH_ITEM_TEMPLATE,
                        stack_count: 1,
                        world_pos: [
                            player_position.0.x,
                            player_position.0.y,
                            player_position.0.z,
                        ],
                        dimension,
                        current_tick: now,
                    },
                ) {
                    tracing::error!(
                        "[bong][workbench] rejected break: failed to return or drop workbench_item: {drop_error}"
                    );
                    continue;
                }
            }
        }
        if let Err(error) = break_placeable(
            PlaceableBlockKind::Workbench,
            &mut commands,
            workbench_entity,
        ) {
            tracing::error!("[bong][workbench] failed to break placeable workbench: {error}");
        } else {
            send_workbench_audio(
                audio_events.as_deref_mut(),
                WORKBENCH_BREAK_AUDIO_RECIPE_ID,
                workbench_block_pos(workbench_position),
            );
        }
    }
}

pub fn send_workbench_audio(
    audio_events: Option<&mut Events<PlaySoundRecipeRequest>>,
    recipe_id: &str,
    block_pos: [i32; 3],
) {
    let Some(audio_events) = audio_events else {
        return;
    };
    let origin = workbench_audio_origin(block_pos);
    audio_events.send(PlaySoundRecipeRequest {
        recipe_id: recipe_id.to_string(),
        instance_id: 0,
        pos: Some(block_pos),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin,
            radius: AUDIO_AREA_RADIUS,
        },
    });
}

pub fn workbench_audio_origin(block_pos: [i32; 3]) -> DVec3 {
    DVec3::new(
        f64::from(block_pos[0]) + 0.5,
        f64::from(block_pos[1]) + 0.5,
        f64::from(block_pos[2]) + 0.5,
    )
}

pub fn workbench_block_pos(position: &Position) -> [i32; 3] {
    [
        position.0.x.floor() as i32,
        position.0.y.floor() as i32,
        position.0.z.floor() as i32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::App;

    fn test_entity() -> Entity {
        let mut app = App::new();
        app.world_mut().spawn_empty().id()
    }

    #[test]
    fn workbench_block_component_stores_placed_by_and_tick() {
        let entity = test_entity();
        let wb = WorkbenchBlock {
            placed_by: entity,
            placed_at_tick: 12345,
        };
        assert_eq!(wb.placed_by, entity);
        assert_eq!(wb.placed_at_tick, 12345);
    }

    #[test]
    fn workbench_open_payload_carries_entity_and_position() {
        let entity = test_entity();
        let payload = WorkbenchOpenPayload {
            workbench_entity: entity,
            position: [10, 64, -20],
        };
        assert_eq!(payload.workbench_entity, entity);
        assert_eq!(payload.position, [10, 64, -20]);
    }

    #[test]
    fn workbench_interact_range_is_three_blocks() {
        assert_eq!(
            WORKBENCH_INTERACT_RANGE, 3.0,
            "workbench interact range must be exactly 3.0 blocks per plan §P2.4"
        );
    }

    #[test]
    fn workbench_item_template_is_correct() {
        assert_eq!(
            WORKBENCH_ITEM_TEMPLATE, "workbench_item",
            "workbench item template ID must match server/assets/items/core.toml"
        );
    }

    // ============= is_within_workbench_range =============

    #[test]
    fn within_range_at_origin() {
        assert!(is_within_workbench_range([0.0, 0.0, 0.0], [0, 0, 0]));
    }

    #[test]
    fn within_range_at_boundary() {
        // Exactly 3 blocks away on x axis
        assert!(is_within_workbench_range([3.0, 0.0, 0.0], [0, 0, 0]));
        // Exactly 3 blocks away on all axes (Chebyshev = 3)
        assert!(is_within_workbench_range([3.0, 3.0, 3.0], [0, 0, 0]));
    }

    #[test]
    fn out_of_range_just_beyond_boundary() {
        assert!(
            !is_within_workbench_range([3.1, 0.0, 0.0], [0, 0, 0]),
            "3.1 blocks on x axis should be out of range"
        );
    }

    #[test]
    fn within_range_negative_coords() {
        assert!(is_within_workbench_range([-2.0, 5.0, -3.0], [-5, 5, -3]));
    }

    #[test]
    fn out_of_range_far_away() {
        assert!(!is_within_workbench_range([100.0, 64.0, 200.0], [0, 64, 0]));
    }

    #[test]
    fn within_range_fractional_player_pos() {
        // Player at 2.9 blocks from workbench at origin
        assert!(is_within_workbench_range([2.9, 0.5, 1.0], [0, 0, 0]));
    }
}
