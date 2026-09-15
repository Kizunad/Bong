use std::{collections::HashSet, fmt};

use valence::prelude::{
    bevy_ecs, App, BlockPos, BlockState, ChunkLayer, Client, Commands, Entity, Event, EventReader,
    Events, IntoSystemConfigs, Position, Query, Res, ResMut, Update, Username, With,
};

use crate::craft::{handle_workbench_place, send_workbench_audio, WORKBENCH_PLACE_AUDIO_RECIPE_ID};
use crate::cultivation::components::Cultivation;
use crate::inventory::external_container::ExternalContainerRegistry;
use crate::inventory::{
    consume_item_instance_once, ItemCategory, ItemInstance, ItemRegistry, PlayerInventory,
};
use crate::network::audio_event_emit::PlaySoundRecipeRequest;
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::player::gameplay::GameplayTick;
use crate::player::state::PlayerState;
use crate::world::bong_blocks::{is_bong_block, place_bong_block};
use crate::world::container_block::{
    container_place_audio_recipe_id, handle_container_block_place, send_container_audio,
    ContainerBlockKind, ContainerBlockPlacement,
};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::furniture::{furniture_kind_for_template_id, FurnitureRegistry};
use crate::zhenfa::trap_content::TrapTargetFace;

const PLAYER_HALF_WIDTH: f64 = 0.3;
const PLAYER_HEIGHT: f64 = 1.8;
const PLACE_REACH_BLOCKS: f64 = 6.0;
const PLACE_REACH_DISTANCE_SQ: f64 = PLACE_REACH_BLOCKS * PLACE_REACH_BLOCKS;
const HERB_CRATE_PLACED_TEMPLATE: &str = "herb_crate_placed";

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
        matches!(
            self,
            Self::Workbench | Self::StorageCrate { .. } | Self::DeadDrop
        )
    }

    fn is_container_backed(self) -> bool {
        matches!(self, Self::StorageCrate { .. } | Self::DeadDrop)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockPlaceRejectReason {
    UnknownBlockItem,
    ContainerBreakRequiresContainerSystem(PlaceableBlockKind),
    ChunkNotLoaded,
    YOutOfBounds,
    TooFar,
    TargetNotReplaceable(BlockState),
    PlayerCollision,
    BongBlockPlaceFailed,
}

impl fmt::Display for BlockPlaceRejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBlockItem => write!(f, "unknown block item"),
            Self::ContainerBreakRequiresContainerSystem(kind) => {
                write!(
                    f,
                    "container break kind {kind:?} must use container block system"
                )
            }
            Self::ChunkNotLoaded => write!(f, "target chunk is not loaded"),
            Self::YOutOfBounds => write!(f, "target y is outside layer bounds"),
            Self::TooFar => write!(f, "target is outside placement reach"),
            Self::TargetNotReplaceable(state) => {
                write!(f, "target block {state:?} is not replaceable")
            }
            Self::PlayerCollision => write!(f, "target block intersects player collision box"),
            Self::BongBlockPlaceFailed => write!(f, "custom Bong block placement failed"),
        }
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<ExternalContainerRegistry>();
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
    container_blocks: Query<
        (&Position, Option<&CurrentDimension>),
        With<crate::world::container_block::ContainerBlock>,
    >,
    mut clients: Query<(&Username, &mut Client, &PlayerState, Option<&Cultivation>)>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    mut furniture_registry: Option<ResMut<FurnitureRegistry>>,
    mut ext_registry: ResMut<ExternalContainerRegistry>,
) {
    let mut reserved_container_positions: HashSet<([i32; 3], DimensionKind)> = container_blocks
        .iter()
        .map(|(position, dimension)| {
            (
                crate::world::container_block::container_block_pos(position),
                dimension
                    .map(|component| component.0)
                    .unwrap_or(DimensionKind::Overworld),
            )
        })
        .collect();

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

        let container_position_key = match &target {
            BlockPlaceTarget::Placeable { kind, .. } if kind.is_container_backed() => {
                Some(([pos.x, pos.y, pos.z], dimension))
            }
            _ => None,
        };
        if let Some(key) = container_position_key {
            if reserved_container_positions.contains(&key) {
                tracing::warn!(
                    "[bong][block_place] rejected: player={:?} pos={:?} dimension={:?} item=`{}` reason=entity-backed container already occupies target",
                    req.client,
                    pos,
                    dimension,
                    template_id
                );
                continue;
            }
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
        if let Some(key) = container_position_key {
            reserved_container_positions.insert(key);
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
                let placed = place_placeable(
                    kind,
                    &mut commands,
                    &mut ext_registry,
                    PlaceablePlacement {
                        layer: layer_entity,
                        pos,
                        dimension,
                        placed_by: req.client,
                        placed_at_tick: now,
                    },
                );
                if placed.is_ok() && kind == PlaceableBlockKind::Workbench {
                    send_workbench_audio(
                        audio_events.as_deref_mut(),
                        WORKBENCH_PLACE_AUDIO_RECIPE_ID,
                        [pos.x, pos.y, pos.z],
                    );
                }
                if placed.is_ok() {
                    if let Some(container_kind) = container_block_kind(kind) {
                        send_container_audio(
                            audio_events.as_deref_mut(),
                            container_place_audio_recipe_id(container_kind),
                            [pos.x, pos.y, pos.z],
                            0.0,
                        );
                    }
                }
                placed.map(|entity| format!("entity={entity:?} kind={kind:?}"))
            }
        };
        match placement {
            Ok(placed) => {
                if let Some(kind) = furniture_kind_for_template_id(&template_id) {
                    if let Some(registry) = furniture_registry.as_deref_mut() {
                        registry.register([pos.x, pos.y, pos.z], kind);
                    } else {
                        tracing::warn!(
                            "[bong][block_place] placed furniture `{}` but FurnitureRegistry is missing",
                            template_id
                        );
                    }
                }
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
    let Some(item) = block_place_item_by_instance(inventory, req.item_instance_id) else {
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
    if let Some(placeable) = template.placeable.as_deref() {
        let Some(kind) = placeable_kind_for_item(&item.template_id, placeable) else {
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
    if template.category != ItemCategory::Block {
        tracing::warn!(
            "[bong][block_place] rejected: item `{}` category {:?} is not Block and has no placeable marker",
            item.template_id,
            template.category
        );
        return None;
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

fn block_place_item_by_instance(
    inventory: &PlayerInventory,
    instance_id: u64,
) -> Option<&ItemInstance> {
    for container in &inventory.containers {
        if let Some(placed) = container
            .items
            .iter()
            .find(|placed| placed.instance.instance_id == instance_id)
        {
            return Some(&placed.instance);
        }
    }
    inventory
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
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

fn placeable_kind_for_item(template_id: &str, raw: &str) -> Option<PlaceableBlockKind> {
    let kind = placeable_kind_from_str(raw)?;
    if matches!(kind, PlaceableBlockKind::StorageCrate { is_herb: false })
        && template_id == HERB_CRATE_PLACED_TEMPLATE
    {
        return Some(PlaceableBlockKind::StorageCrate { is_herb: true });
    }
    Some(kind)
}

#[derive(Debug, Clone, Copy)]
struct PlaceablePlacement {
    layer: Entity,
    pos: BlockPos,
    dimension: DimensionKind,
    placed_by: Entity,
    placed_at_tick: u64,
}

fn place_placeable(
    kind: PlaceableBlockKind,
    commands: &mut Commands,
    ext_registry: &mut ExternalContainerRegistry,
    placement: PlaceablePlacement,
) -> Result<Entity, BlockPlaceRejectReason> {
    match kind {
        PlaceableBlockKind::Workbench => Ok(handle_workbench_place(
            commands,
            placement.layer,
            placement.placed_by,
            placement.pos,
            placement.placed_at_tick,
        )),
        PlaceableBlockKind::StorageCrate { is_herb } => Ok(handle_container_block_place(
            commands,
            ext_registry,
            ContainerBlockPlacement {
                layer: placement.layer,
                pos: placement.pos,
                dimension: placement.dimension,
                placed_by: placement.placed_by,
                placed_at_tick: placement.placed_at_tick,
                kind: ContainerBlockKind::StorageCrate { is_herb },
            },
        )),
        PlaceableBlockKind::DeadDrop => Ok(handle_container_block_place(
            commands,
            ext_registry,
            ContainerBlockPlacement {
                layer: placement.layer,
                pos: placement.pos,
                dimension: placement.dimension,
                placed_by: placement.placed_by,
                placed_at_tick: placement.placed_at_tick,
                kind: ContainerBlockKind::DeadDrop,
            },
        )),
    }
}

fn container_block_kind(kind: PlaceableBlockKind) -> Option<ContainerBlockKind> {
    match kind {
        PlaceableBlockKind::StorageCrate { is_herb } => {
            Some(ContainerBlockKind::StorageCrate { is_herb })
        }
        PlaceableBlockKind::DeadDrop => Some(ContainerBlockKind::DeadDrop),
        PlaceableBlockKind::Workbench => None,
    }
}

pub fn break_placeable(
    kind: PlaceableBlockKind,
    commands: &mut Commands,
    entity: Entity,
) -> Result<(), BlockPlaceRejectReason> {
    match kind {
        PlaceableBlockKind::Workbench => {
            commands.entity(entity).insert(valence::prelude::Despawned);
            Ok(())
        }
        PlaceableBlockKind::StorageCrate { .. } | PlaceableBlockKind::DeadDrop => {
            Err(BlockPlaceRejectReason::ContainerBreakRequiresContainerSystem(kind))
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
    // plan-worldgen-v4 P5 §8.1#5 — vanilla:<block_id> 直通分支：剥前缀后用
    // valence BlockKind 解析为默认 BlockState，避免为每个 vanilla 方块穷举映射。
    // 与 inventory::VANILLA_TEMPLATE_PREFIX / vanilla_block_template 严格对齐。
    if let Some(bare) = template_id.strip_prefix(crate::inventory::VANILLA_TEMPLATE_PREFIX) {
        return valence::prelude::BlockKind::from_str(bare).map(|kind| kind.to_state());
    }
    match template_id {
        "earth_crumb" => Some(BlockState::DIRT),
        "hardened_soil" => Some(BlockState::COARSE_DIRT),
        "barren_sand" => Some(BlockState::SAND),
        "weathered_stone" => Some(BlockState::GRAVEL),
        "raw_clay_lump" => Some(BlockState::CLAY),
        "obsidian_shard" => Some(BlockState::OBSIDIAN),
        "torch_item" => Some(BlockState::TORCH),
        "lantern_item" => Some(BlockState::LANTERN),
        "door_bolt" => Some(BlockState::IRON_DOOR),
        "window_grate" => Some(BlockState::IRON_BARS),
        "simple_bed" => Some(BlockState::BONG_SIMPLE_BED),
        "meditation_mat" => Some(BlockState::BONG_MEDITATION_MAT),
        "moisture_base" => Some(BlockState::BONG_MOISTURE_BASE),
        "spirit_stone_rack" => Some(BlockState::BONG_SPIRIT_STONE_RACK),
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
    let block_center = valence::math::DVec3::new(
        f64::from(pos.x) + 0.5,
        f64::from(pos.y) + 0.5,
        f64::from(pos.z) + 0.5,
    );
    let distance_squared = block_center.distance_squared(player_pos);
    if !distance_squared.is_finite() || distance_squared > PLACE_REACH_DISTANCE_SQ {
        return Err(BlockPlaceRejectReason::TooFar);
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
#[path = "block_place_tests.rs"]
mod tests;
