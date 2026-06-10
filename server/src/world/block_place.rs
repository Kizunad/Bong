use std::fmt;

use valence::prelude::{
    bevy_ecs, App, BlockPos, BlockState, ChunkLayer, Client, Commands, Entity, Event, EventReader,
    IntoSystemConfigs, Position, Query, Res, Update, Username,
};

use crate::craft::handle_workbench_place;
use crate::cultivation::components::Cultivation;
use crate::inventory::{
    consume_item_instance_once, inventory_item_by_instance_borrow, ItemCategory, ItemRegistry,
    PlayerInventory,
};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::player::gameplay::GameplayTick;
use crate::player::state::PlayerState;
use crate::world::bong_blocks::{is_bong_block, place_bong_block};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::zhenfa::trap_content::TrapTargetFace;

const PLAYER_HALF_WIDTH: f64 = 0.3;
const PLAYER_HEIGHT: f64 = 1.8;

#[derive(Debug, Clone, Copy, Event)]
pub struct BlockPlaceRequest {
    pub client: Entity,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub item_instance_id: u64,
    pub target_face: TrapTargetFace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceableBlockKind {
    Workbench,
    StorageCrate { is_herb: bool },
    DeadDrop,
}

impl PlaceableBlockKind {
    fn is_runtime_supported(self) -> bool {
        matches!(self, Self::Workbench)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockPlaceRejectReason {
    UnknownBlockItem,
    UnsupportedPlaceableKind(PlaceableBlockKind),
    ChunkNotLoaded,
    YOutOfBounds,
    TargetNotReplaceable(BlockState),
    PlayerCollision,
    BongBlockPlaceFailed,
}

impl fmt::Display for BlockPlaceRejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBlockItem => write!(f, "unknown block item"),
            Self::UnsupportedPlaceableKind(kind) => {
                write!(f, "unsupported placeable block kind {kind:?}")
            }
            Self::ChunkNotLoaded => write!(f, "target chunk is not loaded"),
            Self::YOutOfBounds => write!(f, "target y is outside layer bounds"),
            Self::TargetNotReplaceable(state) => {
                write!(f, "target block {state:?} is not replaceable")
            }
            Self::PlayerCollision => write!(f, "target block intersects player collision box"),
            Self::BongBlockPlaceFailed => write!(f, "custom Bong block placement failed"),
        }
    }
}

pub fn register(app: &mut App) {
    app.add_event::<BlockPlaceRequest>().add_systems(
        Update,
        handle_block_place_requests
            .after(crate::network::client_request_handler::handle_client_request_payloads),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn handle_block_place_requests(
    mut commands: Commands,
    mut requests: EventReader<BlockPlaceRequest>,
    item_registry: Res<ItemRegistry>,
    gameplay_tick: Option<Res<GameplayTick>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut layers: Query<&mut ChunkLayer>,
    mut inventories: Query<&mut PlayerInventory>,
    player_positions: Query<(&Position, Option<&CurrentDimension>)>,
    mut clients: Query<(&Username, &mut Client, &PlayerState, Option<&Cultivation>)>,
) {
    for req in requests.read() {
        let pos = BlockPos::new(req.x, req.y, req.z);
        let Ok((player_position, current_dimension)) = player_positions.get(req.client) else {
            tracing::warn!(
                "[bong][block_place] rejected: player {:?} has no Position",
                req.client
            );
            continue;
        };
        let dimension = current_dimension
            .map(|component| component.0)
            .unwrap_or(DimensionKind::Overworld);

        let Some(target) =
            block_place_target_for_request(&inventories, &item_registry, req.client, *req)
        else {
            continue;
        };
        let template_id = target.template_id().to_string();

        let Some(dimension_layers) = dimension_layers.as_deref() else {
            tracing::warn!(
                "[bong][block_place] rejected: DimensionLayers resource missing for {:?}",
                req.client
            );
            continue;
        };
        let layer_entity = dimension_layers.entity_for(dimension);
        let Ok(mut layer) = layers.get_mut(layer_entity) else {
            tracing::warn!(
                "[bong][block_place] rejected: layer {:?} for {:?} is missing",
                layer_entity,
                dimension
            );
            continue;
        };

        let collision_state = match &target {
            BlockPlaceTarget::Vanilla { state, .. } => *state,
            BlockPlaceTarget::Placeable { kind, .. } => {
                if !kind.is_runtime_supported() {
                    tracing::warn!(
                        "[bong][block_place] rejected: item `{}` placeable kind {:?} is declared but not implemented",
                        template_id,
                        kind
                    );
                    continue;
                }
                BlockState::DIRT
            }
        };
        if let Err(reason) = can_place_block(&layer, pos, collision_state, player_position.get()) {
            tracing::warn!(
                "[bong][block_place] rejected: player={:?} pos={:?} item=`{}` reason={reason}",
                req.client,
                pos,
                template_id
            );
            continue;
        }

        let Ok(mut inventory) = inventories.get_mut(req.client) else {
            tracing::warn!(
                "[bong][block_place] rejected: player {:?} has no PlayerInventory",
                req.client
            );
            continue;
        };
        if let Err(error) = consume_item_instance_once(&mut inventory, req.item_instance_id) {
            tracing::warn!(
                "[bong][block_place] rejected: consume instance_id={} failed: {error}",
                req.item_instance_id
            );
            continue;
        }

        let placement = match target {
            BlockPlaceTarget::Vanilla { template_id, .. } => {
                place_block_for_kind(&mut layer, pos, &template_id, req.target_face)
                    .map(|state| format!("state={state:?}"))
            }
            BlockPlaceTarget::Placeable {
                template_id: _,
                kind,
            } => {
                let now = gameplay_tick
                    .as_ref()
                    .map(|tick| tick.current_tick())
                    .unwrap_or(0);
                place_placeable(kind, &mut commands, layer_entity, pos, req.client, now)
                    .map(|entity| format!("entity={entity:?} kind={kind:?}"))
            }
        };
        match placement {
            Ok(placed) => {
                if let Ok((username, mut client, player_state, cultivation)) =
                    clients.get_mut(req.client)
                {
                    let default_cultivation;
                    let cultivation = match cultivation {
                        Some(cultivation) => cultivation,
                        None => {
                            default_cultivation = Cultivation::default();
                            &default_cultivation
                        }
                    };
                    send_inventory_snapshot_to_client(
                        req.client,
                        &mut client,
                        username.0.as_str(),
                        &inventory,
                        player_state,
                        cultivation,
                        "block_place_consumed",
                    );
                }
                tracing::info!(
                    "[bong][block_place] ok: player={:?} pos={:?} item=`{}` {placed}",
                    req.client,
                    pos,
                    template_id,
                );
            }
            Err(reason) => {
                tracing::error!(
                    "[bong][block_place] placed item was consumed but placement failed: player={:?} pos={:?} item=`{}` reason={reason}",
                    req.client,
                    pos,
                    template_id
                );
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockPlaceTarget {
    Vanilla {
        template_id: String,
        state: BlockState,
    },
    Placeable {
        template_id: String,
        kind: PlaceableBlockKind,
    },
}

impl BlockPlaceTarget {
    fn template_id(&self) -> &str {
        match self {
            Self::Vanilla { template_id, .. } | Self::Placeable { template_id, .. } => template_id,
        }
    }
}

fn block_place_target_for_request(
    inventories: &Query<&mut PlayerInventory>,
    item_registry: &ItemRegistry,
    client: Entity,
    req: BlockPlaceRequest,
) -> Option<BlockPlaceTarget> {
    let Ok(inventory) = inventories.get(client) else {
        tracing::warn!(
            "[bong][block_place] rejected: player {:?} has no PlayerInventory",
            client
        );
        return None;
    };
    let Some(item) = inventory_item_by_instance_borrow(inventory, req.item_instance_id) else {
        tracing::warn!(
            "[bong][block_place] rejected: instance_id={} not held by {:?}",
            req.item_instance_id,
            client
        );
        return None;
    };
    let Some(template) = item_registry.get(&item.template_id) else {
        tracing::warn!(
            "[bong][block_place] rejected: unknown item template `{}`",
            item.template_id
        );
        return None;
    };
    if template.category != ItemCategory::Block {
        tracing::warn!(
            "[bong][block_place] rejected: item `{}` category {:?} is not Block",
            item.template_id,
            template.category
        );
        return None;
    }
    if let Some(placeable) = template.placeable.as_deref() {
        let Some(kind) = placeable_kind_from_str(placeable) else {
            tracing::warn!(
                "[bong][block_place] rejected: block item `{}` has unknown placeable kind `{}`",
                item.template_id,
                placeable
            );
            return None;
        };
        return Some(BlockPlaceTarget::Placeable {
            template_id: item.template_id.clone(),
            kind,
        });
    }
    let Some(state) = block_item_to_state(&item.template_id, req.target_face) else {
        tracing::warn!(
            "[bong][block_place] rejected: block item `{}` is not placeable in v1",
            item.template_id
        );
        return None;
    };

    Some(BlockPlaceTarget::Vanilla {
        template_id: item.template_id.clone(),
        state,
    })
}

pub fn placeable_kind_from_str(raw: &str) -> Option<PlaceableBlockKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "workbench" => Some(PlaceableBlockKind::Workbench),
        "storage_crate" => Some(PlaceableBlockKind::StorageCrate { is_herb: false }),
        "herb_crate" | "storage_crate_herb" => {
            Some(PlaceableBlockKind::StorageCrate { is_herb: true })
        }
        "dead_drop" => Some(PlaceableBlockKind::DeadDrop),
        _ => None,
    }
}

pub fn place_placeable(
    kind: PlaceableBlockKind,
    commands: &mut Commands,
    layer: Entity,
    pos: BlockPos,
    placed_by: Entity,
    placed_at_tick: u64,
) -> Result<Entity, BlockPlaceRejectReason> {
    match kind {
        PlaceableBlockKind::Workbench => Ok(handle_workbench_place(
            commands,
            layer,
            placed_by,
            pos,
            placed_at_tick,
        )),
        PlaceableBlockKind::StorageCrate { .. } | PlaceableBlockKind::DeadDrop => {
            Err(BlockPlaceRejectReason::UnsupportedPlaceableKind(kind))
        }
    }
}

pub fn break_placeable(
    kind: PlaceableBlockKind,
    commands: &mut Commands,
    entity: Entity,
) -> Result<(), BlockPlaceRejectReason> {
    match kind {
        PlaceableBlockKind::Workbench => {
            commands.entity(entity).despawn();
            Ok(())
        }
        PlaceableBlockKind::StorageCrate { .. } | PlaceableBlockKind::DeadDrop => {
            Err(BlockPlaceRejectReason::UnsupportedPlaceableKind(kind))
        }
    }
}

/// 当前 v1 只把背包方块物品映射回 vanilla BlockState。
///
/// 未来 Bong/custom 方块接入只扩这一条分叉：
/// 1. 在 `bong_blocks.json` 追加方块定义，保持 boolean property 顺序与 MC raw state 对齐。
/// 2. 跑 client `generateBongBlockIds` / server codegen，让 raw id 在双端 registry 中一致。
/// 3. 在 `block_item_to_state` 增加 template_id + target_face -> Bong BlockState 映射。
/// 4. 若 `is_bong_block(state)` 命中，`write_block_state` 会走 `place_bong_block`；否则仍走 vanilla `set_block`。
/// 5. 踩踏/触发行为不放在本函数，后续按 zhenfa proximity system 自建 server-side registry。
pub fn place_block_for_kind(
    layer: &mut ChunkLayer,
    pos: BlockPos,
    template_id: &str,
    target_face: TrapTargetFace,
) -> Result<BlockState, BlockPlaceRejectReason> {
    let state = block_item_to_state(template_id, target_face)
        .ok_or(BlockPlaceRejectReason::UnknownBlockItem)?;
    write_block_state(layer, pos, state)?;
    Ok(state)
}

pub fn block_item_to_state(template_id: &str, _target_face: TrapTargetFace) -> Option<BlockState> {
    match template_id {
        "earth_crumb" => Some(BlockState::DIRT),
        "hardened_soil" => Some(BlockState::COARSE_DIRT),
        "barren_sand" => Some(BlockState::SAND),
        "weathered_stone" => Some(BlockState::GRAVEL),
        "raw_clay_lump" => Some(BlockState::CLAY),
        "obsidian_shard" => Some(BlockState::OBSIDIAN),
        _ => None,
    }
}

pub fn can_place_block(
    layer: &ChunkLayer,
    pos: BlockPos,
    placed_state: BlockState,
    player_pos: valence::math::DVec3,
) -> Result<(), BlockPlaceRejectReason> {
    if pos.y < layer.min_y() || pos.y >= layer.min_y() + layer.height() as i32 {
        return Err(BlockPlaceRejectReason::YOutOfBounds);
    }
    let Some(current) = layer.block(pos).map(|block| block.state) else {
        return Err(BlockPlaceRejectReason::ChunkNotLoaded);
    };
    if !current.is_air() && !current.is_replaceable() {
        return Err(BlockPlaceRejectReason::TargetNotReplaceable(current));
    }
    if placed_state.collision_shapes().next().is_some()
        && block_cell_intersects_player(pos, player_pos)
    {
        return Err(BlockPlaceRejectReason::PlayerCollision);
    }

    Ok(())
}

fn write_block_state(
    layer: &mut ChunkLayer,
    pos: BlockPos,
    state: BlockState,
) -> Result<(), BlockPlaceRejectReason> {
    if is_bong_block(state) {
        place_bong_block(layer, pos, state)
            .map(|_| ())
            .map_err(|_| BlockPlaceRejectReason::BongBlockPlaceFailed)
    } else {
        layer.set_block(pos, state);
        Ok(())
    }
}

fn block_cell_intersects_player(pos: BlockPos, player_pos: valence::math::DVec3) -> bool {
    let block_min_x = f64::from(pos.x);
    let block_max_x = block_min_x + 1.0;
    let block_min_y = f64::from(pos.y);
    let block_max_y = block_min_y + 1.0;
    let block_min_z = f64::from(pos.z);
    let block_max_z = block_min_z + 1.0;

    let player_min_x = player_pos.x - PLAYER_HALF_WIDTH;
    let player_max_x = player_pos.x + PLAYER_HALF_WIDTH;
    let player_min_y = player_pos.y;
    let player_max_y = player_pos.y + PLAYER_HEIGHT;
    let player_min_z = player_pos.z - PLAYER_HALF_WIDTH;
    let player_max_z = player_pos.z + PLAYER_HALF_WIDTH;

    block_max_x > player_min_x
        && block_min_x < player_max_x
        && block_max_y > player_min_y
        && block_min_y < player_max_y
        && block_max_z > player_min_z
        && block_min_z < player_max_z
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::craft::workbench::workbench_block_pos;
    use crate::craft::{WorkbenchBlock, WORKBENCH_ITEM_TEMPLATE};
    use crate::inventory::{
        ContainerState, InventoryInstanceIdAllocator, InventoryRevision, ItemInstance, ItemRarity,
        ItemTemplate, PlacedItemState,
    };
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
    use crate::world::entity_model::{BongVisualEntity, BongVisualKind};
    use valence::prelude::{
        ident, App, BiomeRegistry, DiggingEvent, DiggingState, DimensionTypeRegistry, GameMode,
        IntoSystemConfigs, Server, UnloadedChunk, Update, VisibleChunkLayer,
    };
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper, ScenarioSingleClient};

    #[test]
    fn block_item_to_state_maps_all_v1_block_items() {
        let cases = [
            ("earth_crumb", BlockState::DIRT),
            ("hardened_soil", BlockState::COARSE_DIRT),
            ("barren_sand", BlockState::SAND),
            ("weathered_stone", BlockState::GRAVEL),
            ("raw_clay_lump", BlockState::CLAY),
            ("obsidian_shard", BlockState::OBSIDIAN),
        ];

        for (template_id, expected) in cases {
            assert_eq!(
                block_item_to_state(template_id, TrapTargetFace::Top),
                Some(expected),
                "expected `{template_id}` to map to {expected:?}"
            );
        }
    }

    #[test]
    fn block_item_to_state_rejects_non_placeable_materials() {
        for template_id in ["stone_chunk", "crude_wood", "grass_fiber", "iron_ore"] {
            assert_eq!(
                block_item_to_state(template_id, TrapTargetFace::North),
                None,
                "expected `{template_id}` to stay non-placeable in v1"
            );
        }
    }

    #[test]
    fn block_item_to_state_keeps_workbench_out_of_vanilla_mapping() {
        assert_eq!(
            block_item_to_state(WORKBENCH_ITEM_TEMPLATE, TrapTargetFace::Top),
            None,
            "workbench_item must route through PlaceableBlockKind, not vanilla BlockState mapping"
        );
    }

    #[test]
    fn placeable_kind_from_str_pins_declared_variants() {
        assert_eq!(
            placeable_kind_from_str("workbench"),
            Some(PlaceableBlockKind::Workbench)
        );
        assert_eq!(
            placeable_kind_from_str("storage_crate"),
            Some(PlaceableBlockKind::StorageCrate { is_herb: false })
        );
        assert_eq!(
            placeable_kind_from_str("herb_crate"),
            Some(PlaceableBlockKind::StorageCrate { is_herb: true })
        );
        assert_eq!(
            placeable_kind_from_str("dead_drop"),
            Some(PlaceableBlockKind::DeadDrop)
        );
        assert_eq!(
            placeable_kind_from_str("nonsense"),
            None,
            "unknown placeable values must reject without falling into vanilla placement"
        );
    }

    #[test]
    fn can_place_accepts_air_and_replaceable_target() {
        let (mut app, layer_entity) = test_layer();
        let pos = BlockPos::new(1, 64, 1);
        let player_pos = valence::math::DVec3::new(3.5, 64.0, 3.5);

        let layer = app
            .world()
            .get::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");
        assert_eq!(
            can_place_block(layer, pos, BlockState::DIRT, player_pos),
            Ok(()),
            "AIR target in loaded chunk should be placeable"
        );

        app.world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer")
            .set_block(pos, BlockState::GRASS);
        let layer = app
            .world()
            .get::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");
        assert_eq!(
            can_place_block(layer, pos, BlockState::DIRT, player_pos),
            Ok(()),
            "replaceable grass target should be placeable"
        );
    }

    #[test]
    fn can_place_rejects_occupied_target_without_consuming() {
        let (mut app, layer_entity) = test_layer();
        let pos = BlockPos::new(1, 64, 1);
        app.world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer")
            .set_block(pos, BlockState::STONE);
        let layer = app
            .world()
            .get::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");

        assert_eq!(
            can_place_block(
                layer,
                pos,
                BlockState::DIRT,
                valence::math::DVec3::new(3.5, 64.0, 3.5)
            ),
            Err(BlockPlaceRejectReason::TargetNotReplaceable(
                BlockState::STONE
            ))
        );
    }

    #[test]
    fn can_place_rejects_y_out_of_bounds_and_unloaded_chunk() {
        let (app, layer_entity) = test_layer();
        let layer = app
            .world()
            .get::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");
        let player_pos = valence::math::DVec3::new(3.5, 64.0, 3.5);
        assert_eq!(
            can_place_block(
                layer,
                BlockPos::new(1, layer.min_y() - 1, 1),
                BlockState::DIRT,
                player_pos
            ),
            Err(BlockPlaceRejectReason::YOutOfBounds)
        );
        assert_eq!(
            can_place_block(
                layer,
                BlockPos::new(32, 64, 32),
                BlockState::DIRT,
                player_pos
            ),
            Err(BlockPlaceRejectReason::ChunkNotLoaded)
        );
    }

    #[test]
    fn can_place_rejects_player_collision() {
        let (app, layer_entity) = test_layer();
        let layer = app
            .world()
            .get::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");

        assert_eq!(
            can_place_block(
                layer,
                BlockPos::new(1, 64, 1),
                BlockState::DIRT,
                valence::math::DVec3::new(1.5, 64.0, 1.5)
            ),
            Err(BlockPlaceRejectReason::PlayerCollision)
        );
    }

    #[test]
    fn place_block_for_kind_writes_vanilla_state() {
        let (mut app, layer_entity) = test_layer();
        let pos = BlockPos::new(1, 64, 1);
        let mut layer = app
            .world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");

        assert_eq!(
            place_block_for_kind(&mut layer, pos, "earth_crumb", TrapTargetFace::Top),
            Ok(BlockState::DIRT)
        );
        assert_eq!(
            layer.block(pos).map(|block| block.state),
            Some(BlockState::DIRT)
        );
    }

    #[test]
    fn write_block_state_routes_bong_blocks_through_guarded_path() {
        let (mut app, layer_entity) = test_layer();
        let pos = BlockPos::new(1, 64, 1);
        let mut layer = app
            .world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");

        assert_eq!(
            write_block_state(&mut layer, pos, BlockState::BONG_ZHENFA_NODE),
            Ok(())
        );
        assert_eq!(
            layer.block(pos).map(|block| block.state),
            Some(BlockState::BONG_ZHENFA_NODE)
        );
    }

    #[test]
    fn handler_places_block_consumes_inventory_and_sends_snapshot() {
        let (mut app, client, layer_entity, mut helper) = block_place_app(
            inventory_with_item(item_instance(9101, "earth_crumb", 2)),
            DimensionKind::Overworld,
        );
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: 1,
            y: 64,
            z: 1,
            item_instance_id: 9101,
            target_face: TrapTargetFace::Top,
        });

        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            app.world()
                .get::<ChunkLayer>(layer_entity)
                .and_then(|layer| layer
                    .block(BlockPos::new(1, 64, 1))
                    .map(|block| block.state)),
            Some(BlockState::DIRT)
        );
        let inventory = app
            .world()
            .get::<PlayerInventory>(client)
            .expect("client should keep inventory");
        assert_eq!(
            inventory_item_by_instance_borrow(inventory, 9101).map(|item| item.stack_count),
            Some(1),
            "successful placement should consume exactly one stack item"
        );
        assert_eq!(
            inventory.revision,
            InventoryRevision(1),
            "successful placement should bump inventory revision"
        );
        assert!(
            has_inventory_snapshot_payload(&mut helper),
            "successful placement should push a corrective inventory snapshot"
        );
    }

    #[test]
    fn handler_places_workbench_entity_consumes_inventory_and_sends_snapshot() {
        let (mut app, client, layer_entity, mut helper) = block_place_app(
            inventory_with_item(item_instance(9201, WORKBENCH_ITEM_TEMPLATE, 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable(
                WORKBENCH_ITEM_TEMPLATE,
                ItemCategory::Block,
                Some("workbench"),
            ),
        ])));

        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: 1,
            y: 64,
            z: 1,
            item_instance_id: 9201,
            target_face: TrapTargetFace::Top,
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            block_state_at(&app, layer_entity, BlockPos::new(1, 64, 1)),
            Some(BlockState::AIR),
            "workbench placement is pure entity-backed and must not write a vanilla block"
        );

        let mut query = app
            .world_mut()
            .query::<(&WorkbenchBlock, &BongVisualEntity, &Position)>();
        let workbenches = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(
            workbenches.len(),
            1,
            "placing one workbench_item should spawn exactly one WorkbenchBlock entity"
        );
        let (workbench, visual, position) = workbenches[0];
        assert_eq!(workbench.placed_by, client);
        assert_eq!(workbench.placed_at_tick, 0);
        assert_eq!(visual.kind, BongVisualKind::Workbench);
        assert_eq!(workbench_block_pos(position), [1, 64, 1]);
        assert_eq!(
            inventory_template_count(&app, client, WORKBENCH_ITEM_TEMPLATE),
            0,
            "successful workbench placement should consume the held workbench item"
        );
        assert!(
            has_inventory_snapshot_payload(&mut helper),
            "successful workbench placement should push a corrective inventory snapshot"
        );
    }

    #[test]
    fn handler_rejects_missing_instance_without_consuming_or_writing() {
        let (mut app, client, layer_entity, _helper) = block_place_app(
            inventory_with_item(item_instance(9101, "earth_crumb", 1)),
            DimensionKind::Overworld,
        );
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: 1,
            y: 64,
            z: 1,
            item_instance_id: 9999,
            target_face: TrapTargetFace::Top,
        });

        app.update();

        assert_eq!(
            app.world()
                .get::<ChunkLayer>(layer_entity)
                .and_then(|layer| layer
                    .block(BlockPos::new(1, 64, 1))
                    .map(|block| block.state)),
            Some(BlockState::AIR)
        );
        assert_eq!(
            inventory_template_count(&app, client, "earth_crumb"),
            1,
            "missing instance rejection must not consume inventory"
        );
    }

    #[test]
    fn handler_rejects_non_block_category_without_consuming_or_writing() {
        let (mut app, client, layer_entity, _helper) = block_place_app(
            inventory_with_item(item_instance(9102, "earth_crumb", 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([item_template(
            "earth_crumb",
            ItemCategory::Misc,
        )])));
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: 1,
            y: 64,
            z: 1,
            item_instance_id: 9102,
            target_face: TrapTargetFace::Top,
        });

        app.update();

        assert_eq!(
            app.world()
                .get::<ChunkLayer>(layer_entity)
                .and_then(|layer| layer
                    .block(BlockPos::new(1, 64, 1))
                    .map(|block| block.state)),
            Some(BlockState::AIR)
        );
        assert_eq!(
            inventory_template_count(&app, client, "earth_crumb"),
            1,
            "non-Block category rejection must not consume inventory"
        );
    }

    #[test]
    fn handler_selects_layer_from_player_dimension() {
        let (mut app, client, overworld_layer, _helper) = block_place_app(
            inventory_with_item(item_instance(9103, "barren_sand", 1)),
            DimensionKind::Tsy,
        );
        let mut layer = new_test_chunk_layer(&app);
        layer.insert_chunk([0, 0], UnloadedChunk::new());
        let tsy_layer = app.world_mut().spawn(layer).id();
        app.insert_resource(DimensionLayers {
            overworld: overworld_layer,
            tsy: tsy_layer,
        });

        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: 1,
            y: 64,
            z: 1,
            item_instance_id: 9103,
            target_face: TrapTargetFace::Top,
        });
        app.update();

        assert_eq!(
            app.world()
                .get::<ChunkLayer>(overworld_layer)
                .and_then(|layer| layer
                    .block(BlockPos::new(1, 64, 1))
                    .map(|block| block.state)),
            Some(BlockState::AIR),
            "overworld layer should stay untouched for TSY player"
        );
        assert_eq!(
            app.world()
                .get::<ChunkLayer>(tsy_layer)
                .and_then(|layer| layer
                    .block(BlockPos::new(1, 64, 1))
                    .map(|block| block.state)),
            Some(BlockState::SAND),
            "TSY player should write into the TSY layer selected by DimensionLayers"
        );
    }

    #[test]
    fn p5_break_place_break_dirt_preserves_one_to_one_inventory() {
        let (mut app, client, layer_entity, _helper) =
            block_place_app(empty_inventory(), DimensionKind::Overworld);
        app.insert_resource(InventoryInstanceIdAllocator::new(9104));
        app.world_mut()
            .entity_mut(client)
            .insert((GameMode::Survival, VisibleChunkLayer(layer_entity)));
        app.add_event::<DiggingEvent>();
        app.add_systems(
            Update,
            (
                crate::world::block_drop::apply_block_drops
                    .before(crate::world::block_break::apply_default_block_break),
                crate::world::block_break::apply_default_block_break,
            ),
        );

        let pos = BlockPos::new(1, 64, 1);
        app.world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer")
            .set_block(pos, BlockState::DIRT);

        send_stop_break(&mut app, client, pos);
        app.update();
        assert_eq!(
            inventory_template_count(&app, client, "earth_crumb"),
            1,
            "first DIRT break should grant exactly one earth_crumb"
        );
        assert_eq!(
            block_state_at(&app, layer_entity, pos),
            Some(BlockState::AIR)
        );

        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            item_instance_id: 9104,
            target_face: TrapTargetFace::Top,
        });
        app.update();
        assert_eq!(
            inventory_template_count(&app, client, "earth_crumb"),
            0,
            "placing the block back must consume the only earth_crumb"
        );
        assert_eq!(
            block_state_at(&app, layer_entity, pos),
            Some(BlockState::DIRT)
        );

        send_stop_break(&mut app, client, pos);
        app.update();
        assert_eq!(
            inventory_template_count(&app, client, "earth_crumb"),
            1,
            "second DIRT break should grant one item, not duplicate the placed item"
        );
        assert_eq!(
            block_state_at(&app, layer_entity, pos),
            Some(BlockState::AIR)
        );
    }

    fn block_place_app(
        inventory: PlayerInventory,
        dimension: DimensionKind,
    ) -> (App, Entity, Entity, MockClientHelper) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template("earth_crumb", ItemCategory::Block),
            item_template("barren_sand", ItemCategory::Block),
        ])));
        app.insert_resource(DimensionLayers {
            overworld: scenario.layer,
            tsy: scenario.layer,
        });
        app.add_event::<BlockPlaceRequest>();
        app.add_systems(Update, handle_block_place_requests);

        let (mut bundle, helper) = create_mock_client("Azure");
        bundle.player.position = Position::new([3.5, 64.0, 3.5]);
        let client = app.world_mut().spawn(bundle).id();
        app.world_mut().entity_mut(client).insert((
            inventory,
            PlayerState::default(),
            Cultivation::default(),
            CurrentDimension(dimension),
        ));
        app.world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([0, 0], UnloadedChunk::new());
        (app, client, scenario.layer, helper)
    }

    fn test_layer() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([0, 0], UnloadedChunk::new());
        (app, scenario.layer)
    }

    fn send_stop_break(app: &mut App, client: Entity, pos: BlockPos) {
        app.world_mut().send_event(DiggingEvent {
            client,
            position: pos,
            direction: valence::protocol::Direction::Up,
            state: DiggingState::Stop,
        });
    }

    fn block_state_at(app: &App, layer_entity: Entity, pos: BlockPos) -> Option<BlockState> {
        app.world()
            .get::<ChunkLayer>(layer_entity)
            .and_then(|layer| layer.block(pos).map(|block| block.state))
    }

    fn new_test_chunk_layer(app: &App) -> ChunkLayer {
        ChunkLayer::new(
            ident!("overworld"),
            app.world().resource::<DimensionTypeRegistry>(),
            app.world().resource::<BiomeRegistry>(),
            app.world().resource::<Server>(),
        )
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

    fn has_inventory_snapshot_payload(helper: &mut MockClientHelper) -> bool {
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let Ok(payload) = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0) else {
                continue;
            };
            if matches!(payload.payload, ServerDataPayloadV1::InventorySnapshot(_)) {
                return true;
            }
        }
        false
    }

    fn item_template(template_id: &str, category: ItemCategory) -> (String, ItemTemplate) {
        item_template_with_placeable(template_id, category, None)
    }

    fn item_template_with_placeable(
        template_id: &str,
        category: ItemCategory,
        placeable: Option<&str>,
    ) -> (String, ItemTemplate) {
        (
            template_id.to_string(),
            ItemTemplate {
                id: template_id.to_string(),
                display_name: template_id.to_string(),
                category,
                placeable: placeable.map(str::to_string),
                max_stack_count: 16,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.1,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.0,
                description: "test template".to_string(),
                effect: None,
                cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
                cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shelflife_track: None,
            },
        )
    }

    fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 2,
                cols: 9,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 2,
                cols: 9,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    fn item_instance(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
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

    fn inventory_template_count(app: &App, player: Entity, template_id: &str) -> u32 {
        let inventory = app
            .world()
            .get::<PlayerInventory>(player)
            .expect("player should carry inventory");
        inventory
            .containers
            .iter()
            .flat_map(|container| container.items.iter())
            .map(|placed| &placed.instance)
            .chain(inventory.hotbar.iter().filter_map(|item| item.as_ref()))
            .chain(inventory.equipped.values())
            .filter(|item| item.template_id == template_id)
            .map(|item| item.stack_count)
            .sum()
    }
}
