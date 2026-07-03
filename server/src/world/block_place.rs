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
    use std::collections::{HashMap, HashSet};

    use crate::combat::components::{BodyPart, WoundKind};
    use crate::combat::events::{
        ApplyStatusEffectIntent, AttackSource, CombatEvent, StatusEffectKind,
    };
    use crate::craft::workbench::workbench_block_pos;
    use crate::craft::{WorkbenchBlock, WORKBENCH_ITEM_TEMPLATE};
    use crate::inventory::external_container::{ExternalContainer, ExternalContainerKind};
    use crate::inventory::{
        inventory_item_by_instance_borrow, ContainerState, InventoryInstanceIdAllocator,
        InventoryRevision, ItemInstance, ItemRarity, ItemTemplate, PlacedItemState, SlotContents,
        EQUIP_SLOT_MAIN_HAND,
    };
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::network::audio_event_emit::PlaySoundRecipeRequest;
    use crate::network::gameplay_vfx;
    use crate::network::vfx_event_emit::VfxEventRequest;
    use crate::schema::server_data::{
        LootContainerCloseReasonV1, ServerDataPayloadV1, ServerDataV1,
    };
    use crate::schema::vfx_event::VfxEventPayloadV1;
    use crate::world::container_block::{
        container_block_pos, ContainerBlock, ContainerBlockKind, DeadDropWard,
        DEAD_DROP_WARD_BREAK_AUDIO_RECIPE_ID, DEAD_DROP_WARD_CONTAM_DELTA, DEAD_DROP_WARD_DAMAGE,
        DEAD_DROP_WARD_SLOW_DURATION_TICKS, DEAD_DROP_WARD_SLOW_MAGNITUDE,
    };
    use crate::world::entity_model::{BongVisualEntity, BongVisualKind};
    use crate::world::furniture::FurnitureKind;
    use valence::prelude::{
        ident, App, BiomeRegistry, DiggingEvent, DiggingState, DimensionTypeRegistry, GameMode,
        IntoSystemConfigs, Server, UnloadedChunk, Update, VisibleChunkLayer, With,
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
            ("torch_item", BlockState::TORCH),
            ("lantern_item", BlockState::LANTERN),
            ("door_bolt", BlockState::IRON_DOOR),
            ("window_grate", BlockState::IRON_BARS),
            ("simple_bed", BlockState::BONG_SIMPLE_BED),
            ("meditation_mat", BlockState::BONG_MEDITATION_MAT),
            ("moisture_base", BlockState::BONG_MOISTURE_BASE),
            ("spirit_stone_rack", BlockState::BONG_SPIRIT_STONE_RACK),
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

    // ─── plan-worldgen-v4 P5 §8.1#5 — vanilla: 直通分支专属矩阵 ───

    /// happy path：`vanilla:<known>` 剥前缀后用 BlockKind 解析为默认 BlockState。
    /// 锁住 give-block（ItemRegistry vanilla:<id>）→ 放置链路对齐（给得到必放得下）。
    #[test]
    fn block_item_to_state_resolves_known_vanilla_prefix() {
        use valence::prelude::BlockKind;
        // expected 由 bare 名经 BlockKind::from_str 派生（与实现同源），不硬编码变体名。
        for bare in ["stone", "stone_bricks", "oak_log"] {
            let expected = BlockKind::from_str(bare)
                .unwrap_or_else(|| panic!("{bare} 应是合法 BlockKind"))
                .to_state();
            let template_id = format!("vanilla:{bare}");
            assert_eq!(
                block_item_to_state(&template_id, TrapTargetFace::Top),
                Some(expected),
                "`{template_id}` 应剥前缀后解析为 BlockKind 默认 state {expected:?}"
            );
        }
    }

    /// 边界：`vanilla:`（空 bare id）剥前缀后是空串，BlockKind::from_str("") → None。
    #[test]
    fn block_item_to_state_rejects_empty_vanilla_bare_id() {
        assert_eq!(
            block_item_to_state("vanilla:", TrapTargetFace::Top),
            None,
            "空 bare id（vanilla:）无对应 BlockKind，必须拒绝而非 panic 或落 air"
        );
    }

    /// 错误分支：`vanilla:<unknown>` 剥前缀后是未知块名，BlockKind 解析不出 → None。
    #[test]
    fn block_item_to_state_rejects_unknown_vanilla_block() {
        assert_eq!(
            block_item_to_state("vanilla:not_a_real_block", TrapTargetFace::North),
            None,
            "未知 vanilla 块名必须拒绝，不得静默落成其他方块"
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
        assert_eq!(
            placeable_kind_from_str("  STORAGE_CRATE  "),
            Some(PlaceableBlockKind::StorageCrate { is_herb: false }),
            "placeable parser should trim and normalize declared TOML markers"
        );
        assert_eq!(
            placeable_kind_from_str("   "),
            None,
            "blank placeable markers must reject instead of routing to a default"
        );
    }

    #[test]
    fn break_placeable_rejects_container_kinds_without_container_system() {
        let (mut app, _) = test_layer();
        let entity = app.world_mut().spawn_empty().id();

        for kind in [
            PlaceableBlockKind::DeadDrop,
            PlaceableBlockKind::StorageCrate { is_herb: false },
            PlaceableBlockKind::StorageCrate { is_herb: true },
        ] {
            let result = break_placeable(kind, &mut app.world_mut().commands(), entity);

            assert_eq!(
                result,
                Err(BlockPlaceRejectReason::ContainerBreakRequiresContainerSystem(kind)),
                "expected {kind:?} break to require the container block system"
            );
        }
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
    fn handler_places_furniture_block_and_registers_position() {
        let cases = [
            (
                "simple_bed",
                BlockState::BONG_SIMPLE_BED,
                FurnitureKind::SimpleBed,
            ),
            (
                "meditation_mat",
                BlockState::BONG_MEDITATION_MAT,
                FurnitureKind::MeditationMat,
            ),
            (
                "moisture_base",
                BlockState::BONG_MOISTURE_BASE,
                FurnitureKind::MoistureBase,
            ),
            (
                "spirit_stone_rack",
                BlockState::BONG_SPIRIT_STONE_RACK,
                FurnitureKind::SpiritStoneRack,
            ),
        ];

        for (index, (template_id, expected_state, expected_kind)) in cases.into_iter().enumerate() {
            let item_instance_id = 9301 + index as u64;
            let (mut app, client, layer_entity, mut helper) = block_place_app(
                inventory_with_item(item_instance(item_instance_id, template_id, 1)),
                DimensionKind::Overworld,
            );
            app.insert_resource(ItemRegistry::from_map(HashMap::from([item_template(
                template_id,
                ItemCategory::Block,
            )])));

            app.world_mut().send_event(BlockPlaceRequest {
                client,
                x: 1,
                y: 64,
                z: 1,
                item_instance_id,
                target_face: TrapTargetFace::Top,
            });
            app.update();
            flush_all_client_packets(&mut app);

            assert_eq!(
                block_state_at(&app, layer_entity, BlockPos::new(1, 64, 1)),
                Some(expected_state),
                "{template_id} should place as its Bong custom block"
            );
            assert_eq!(
                app.world()
                    .resource::<FurnitureRegistry>()
                    .kind_at([1, 64, 1]),
                Some(expected_kind),
                "successful {template_id} placement should register its furniture coordinate"
            );
            assert_eq!(
                inventory_template_count(&app, client, template_id),
                0,
                "successful {template_id} placement should consume the held item"
            );
            assert!(
                has_inventory_snapshot_payload(&mut helper),
                "successful {template_id} placement should push a corrective inventory snapshot"
            );
        }
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
    fn handler_rejects_equipped_block_place_item() {
        let equipped_item = item_instance(9102, "earth_crumb", 1);
        let (mut app, client, layer_entity, mut helper) = block_place_app(
            inventory_with_equipped_held_item(equipped_item),
            DimensionKind::Overworld,
        );

        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: 1,
            y: 64,
            z: 1,
            item_instance_id: 9102,
            target_face: TrapTargetFace::Top,
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            block_state_at(&app, layer_entity, BlockPos::new(1, 64, 1)),
            Some(BlockState::AIR),
            "equipped block item must not be placeable through forged C2S instance ids"
        );
        let inventory = app
            .world()
            .get::<PlayerInventory>(client)
            .expect("client should keep inventory");
        assert!(
            inventory_item_by_instance_borrow(inventory, 9102).is_some(),
            "rejected equipped block item must remain equipped"
        );
        assert_eq!(
            inventory.revision,
            InventoryRevision(0),
            "rejected equipped block place must not mutate inventory"
        );
        assert!(
            !has_inventory_snapshot_payload(&mut helper),
            "rejected equipped block place should not push a consumption snapshot"
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
    fn handler_places_container_entities_from_misc_placeable_templates() {
        let cases = [
            (
                "trade_crate",
                "storage_crate",
                ContainerBlockKind::StorageCrate { is_herb: false },
                ExternalContainerKind::StorageCrate { is_herb: false },
                (4, 4),
            ),
            (
                "herb_crate_placed",
                "storage_crate",
                ContainerBlockKind::StorageCrate { is_herb: true },
                ExternalContainerKind::StorageCrate { is_herb: true },
                (4, 4),
            ),
            (
                "dead_drop_box",
                "dead_drop",
                ContainerBlockKind::DeadDrop,
                ExternalContainerKind::DeadDrop,
                (3, 3),
            ),
        ];

        for (index, (template_id, placeable, expected_kind, expected_source, (rows, cols))) in
            cases.into_iter().enumerate()
        {
            let item_instance_id = 9401 + index as u64;
            let (mut app, client, layer_entity, mut helper) = block_place_app(
                inventory_with_item(item_instance(item_instance_id, template_id, 1)),
                DimensionKind::Overworld,
            );
            app.insert_resource(ItemRegistry::from_map(HashMap::from([
                item_template_with_placeable(template_id, ItemCategory::Misc, Some(placeable)),
            ])));

            app.world_mut().send_event(BlockPlaceRequest {
                client,
                x: 1,
                y: 64,
                z: 1,
                item_instance_id,
                target_face: TrapTargetFace::Top,
            });
            app.update();
            flush_all_client_packets(&mut app);

            assert_eq!(
                block_state_at(&app, layer_entity, BlockPos::new(1, 64, 1)),
                Some(BlockState::AIR),
                "{template_id} is entity-backed and must not write a vanilla block"
            );
            let mut query = app.world_mut().query::<(
                &ContainerBlock,
                &ExternalContainer,
                &Position,
                &CurrentDimension,
            )>();
            let containers = query.iter(app.world()).collect::<Vec<_>>();
            assert_eq!(
                containers.len(),
                1,
                "placing {template_id} should spawn exactly one container entity"
            );
            let (block, ext, position, container_dimension) = containers[0];
            assert_eq!(block.kind, expected_kind, "{template_id} block kind");
            assert_eq!(block.placed_by, client, "{template_id} placed_by");
            assert_eq!(block.placed_at_tick, 0, "{template_id} placed_at_tick");
            assert_eq!(
                ext.source_kind, expected_source,
                "{template_id} source kind"
            );
            assert_eq!(ext.container.rows, rows, "{template_id} rows");
            assert_eq!(ext.container.cols, cols, "{template_id} cols");
            assert_eq!(
                container_block_pos(position),
                [1, 64, 1],
                "{template_id} block position"
            );
            assert_eq!(
                container_dimension.0,
                DimensionKind::Overworld,
                "{template_id} container entity should store its placed dimension"
            );
            assert_eq!(
                app.world()
                    .resource::<ExternalContainerRegistry>()
                    .sessions
                    .len(),
                1,
                "{template_id} should register one ExternalContainer session"
            );
            assert_eq!(
                inventory_template_count(&app, client, template_id),
                0,
                "successful {template_id} placement should consume the held item"
            );
            assert!(
                has_inventory_snapshot_payload(&mut helper),
                "successful {template_id} placement should push a corrective inventory snapshot"
            );
        }
    }

    #[test]
    fn handler_rejects_same_tick_duplicate_container_position_without_consuming_second_item() {
        let (mut app, client, layer_entity, _helper) = block_place_app(
            inventory_with_item(item_instance(9451, "trade_crate", 2)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("trade_crate", ItemCategory::Misc, Some("storage_crate")),
        ])));

        let pos = BlockPos::new(1, 64, 1);
        for _ in 0..2 {
            app.world_mut().send_event(BlockPlaceRequest {
                client,
                x: pos.x,
                y: pos.y,
                z: pos.z,
                item_instance_id: 9451,
                target_face: TrapTargetFace::Top,
            });
        }

        app.update();

        assert_eq!(
            block_state_at(&app, layer_entity, pos),
            Some(BlockState::AIR),
            "container placement stays pure entity-backed and must not hide occupancy in ChunkLayer"
        );
        let container_count = app
            .world_mut()
            .query_filtered::<Entity, With<ContainerBlock>>()
            .iter(app.world())
            .count();
        assert_eq!(
            container_count, 1,
            "same-tick duplicate placement must reserve the entity-backed target and spawn one container"
        );
        assert_eq!(
            app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .len(),
            1,
            "duplicate placement must not allocate a hidden second external container session"
        );
        assert_eq!(
            inventory_template_count(&app, client, "trade_crate"),
            1,
            "duplicate entity-backed placement must reject before consuming the second stack item"
        );
    }

    #[test]
    fn handler_places_dead_drop_with_active_ward_owner() {
        let (mut app, client, _layer_entity, _helper) = block_place_app(
            inventory_with_item(item_instance(9501, "dead_drop_box", 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("dead_drop_box", ItemCategory::Misc, Some("dead_drop")),
        ])));

        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: 1,
            y: 64,
            z: 1,
            item_instance_id: 9501,
            target_face: TrapTargetFace::Top,
        });
        app.update();

        let mut query = app
            .world_mut()
            .query::<(&ContainerBlock, &ExternalContainer, &DeadDropWard)>();
        let (block, ext, ward) = query
            .iter(app.world())
            .next()
            .expect("dead_drop_box placement should spawn one warded container");
        assert_eq!(block.kind, ContainerBlockKind::DeadDrop);
        assert_eq!(ext.source_kind, ExternalContainerKind::DeadDrop);
        assert_eq!(
            ward.owner, client,
            "dead drop ward owner must be the placing player for break attribution"
        );
        assert!(
            ward.ward_active,
            "dead drop ward must default active immediately after placement"
        );
    }

    #[test]
    fn breaking_open_container_entity_closes_session_and_drops_contents() {
        let (mut app, client, _layer_entity, mut helper) = block_place_app(
            inventory_with_item(item_instance(9601, "trade_crate", 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("trade_crate", ItemCategory::Misc, Some("storage_crate")),
            item_template("bone_coin_stack", ItemCategory::Misc),
        ])));
        app.insert_resource(InventoryInstanceIdAllocator::new(9700));
        app.init_resource::<crate::inventory::DroppedLootRegistry>();
        app.world_mut().entity_mut(client).insert((
            GameMode::Survival,
            CurrentDimension(DimensionKind::Overworld),
        ));
        app.add_event::<DiggingEvent>();
        app.add_systems(
            Update,
            crate::world::container_block::handle_container_block_break,
        );

        let pos = BlockPos::new(1, 64, 1);
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            item_instance_id: 9601,
            target_face: TrapTargetFace::Top,
        });
        app.update();

        let container_entity = {
            let mut query = app.world_mut().query::<(Entity, &mut ExternalContainer)>();
            let (entity, mut ext) = query
                .iter_mut(app.world_mut())
                .next()
                .expect("placed crate should spawn an ExternalContainer");
            ext.opened_by = Some(client);
            ext.container.items.push(PlacedItemState {
                row: 0,
                col: 0,
                instance: item_instance(9602, "bone_coin_stack", 3),
            });
            entity
        };
        let session_id = app
            .world()
            .get::<ExternalContainer>(container_entity)
            .expect("container should still exist before break")
            .session_id;

        send_stop_break(&mut app, client, pos);
        app.update();
        flush_all_client_packets(&mut app);

        assert!(
            app.world().get_entity(container_entity).is_none(),
            "breaking the container marker should despawn the entity"
        );
        assert!(
            !app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .contains_key(&session_id),
            "breaking should remove the external container session"
        );
        let dropped = app
            .world()
            .resource::<crate::inventory::DroppedLootRegistry>();
        assert!(
            dropped
                .entries
                .values()
                .any(|entry| entry.item.template_id == "trade_crate"),
            "breaking should drop the container item itself"
        );
        assert!(
            dropped
                .entries
                .values()
                .any(|entry| entry.item.template_id == "bone_coin_stack"
                    && entry.item.stack_count == 3),
            "breaking should drop every item stored inside"
        );
        assert!(
            has_loot_container_close_payload(
                &mut helper,
                LootContainerCloseReasonV1::ContainerDestroyed
            ),
            "breaking an opened container should force-close the player's loot UI"
        );
    }

    #[test]
    fn breaking_closed_empty_container_drops_only_container_without_close_payload() {
        let (mut app, client, _layer_entity, mut helper) = block_place_app(
            inventory_with_item(item_instance(9611, "trade_crate", 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("trade_crate", ItemCategory::Misc, Some("storage_crate")),
        ])));
        app.insert_resource(InventoryInstanceIdAllocator::new(9710));
        app.init_resource::<crate::inventory::DroppedLootRegistry>();
        app.world_mut().entity_mut(client).insert((
            GameMode::Survival,
            CurrentDimension(DimensionKind::Overworld),
        ));
        app.add_event::<DiggingEvent>();
        app.add_systems(
            Update,
            crate::world::container_block::handle_container_block_break,
        );

        let pos = BlockPos::new(1, 64, 1);
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            item_instance_id: 9611,
            target_face: TrapTargetFace::Top,
        });
        app.update();

        let container_entity = app
            .world_mut()
            .query_filtered::<Entity, With<ExternalContainer>>()
            .iter(app.world())
            .next()
            .expect("placed crate should spawn an ExternalContainer");
        let session_id = app
            .world()
            .get::<ExternalContainer>(container_entity)
            .expect("container should still exist before break")
            .session_id;

        send_stop_break(&mut app, client, pos);
        app.update();
        flush_all_client_packets(&mut app);

        assert!(
            app.world().get_entity(container_entity).is_none(),
            "breaking a closed empty container should despawn the entity"
        );
        assert!(
            !app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .contains_key(&session_id),
            "breaking should remove the closed container session"
        );
        let dropped = app
            .world()
            .resource::<crate::inventory::DroppedLootRegistry>();
        assert_eq!(
            dropped.entries.len(),
            1,
            "empty container break should drop only the container item"
        );
        assert!(
            dropped
                .entries
                .values()
                .all(|entry| entry.item.template_id == "trade_crate"),
            "empty container break should not invent content drops"
        );
        assert!(
            !has_loot_container_close_payload(
                &mut helper,
                LootContainerCloseReasonV1::ContainerDestroyed
            ),
            "closed container break should not emit a loot close payload"
        );
    }

    #[test]
    fn breaking_container_cleans_session_when_container_item_drop_fails() {
        let (mut app, client, _layer_entity, mut helper) = block_place_app(
            inventory_with_item(item_instance(9621, "trade_crate", 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("trade_crate", ItemCategory::Misc, Some("storage_crate")),
            item_template("bone_coin_stack", ItemCategory::Misc),
        ])));
        app.insert_resource(InventoryInstanceIdAllocator::new(9720));
        app.init_resource::<crate::inventory::DroppedLootRegistry>();
        app.world_mut().entity_mut(client).insert((
            GameMode::Survival,
            CurrentDimension(DimensionKind::Overworld),
        ));
        app.add_event::<DiggingEvent>();
        app.add_systems(
            Update,
            crate::world::container_block::handle_container_block_break,
        );

        let pos = BlockPos::new(1, 64, 1);
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            item_instance_id: 9621,
            target_face: TrapTargetFace::Top,
        });
        app.update();

        let container_entity = {
            let mut query = app.world_mut().query::<(Entity, &mut ExternalContainer)>();
            let (entity, mut ext) = query
                .iter_mut(app.world_mut())
                .next()
                .expect("placed crate should spawn an ExternalContainer");
            ext.opened_by = Some(client);
            ext.container.items.push(PlacedItemState {
                row: 0,
                col: 0,
                instance: item_instance(9622, "bone_coin_stack", 3),
            });
            entity
        };
        let session_id = app
            .world()
            .get::<ExternalContainer>(container_entity)
            .expect("container should still exist before break")
            .session_id;

        app.insert_resource(ItemRegistry::from_map(HashMap::from([item_template(
            "bone_coin_stack",
            ItemCategory::Misc,
        )])));

        send_stop_break(&mut app, client, pos);
        app.update();
        flush_all_client_packets(&mut app);

        assert!(
            app.world().get_entity(container_entity).is_none(),
            "failed container item drop must not leave a zombie container entity"
        );
        assert!(
            !app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .contains_key(&session_id),
            "failed container item drop must still remove the session"
        );
        let dropped = app
            .world()
            .resource::<crate::inventory::DroppedLootRegistry>();
        assert!(
            dropped
                .entries
                .values()
                .any(|entry| entry.item.template_id == "bone_coin_stack"
                    && entry.item.stack_count == 3),
            "failed container item drop should still drop stored contents"
        );
        assert!(
            dropped
                .entries
                .values()
                .all(|entry| entry.item.template_id != "trade_crate"),
            "missing container template should skip only the container item drop"
        );
        assert!(
            has_loot_container_close_payload(
                &mut helper,
                LootContainerCloseReasonV1::ContainerDestroyed
            ),
            "failed container item drop should still force-close an opened container"
        );
    }

    #[test]
    fn breaking_container_twice_in_same_tick_drops_once() {
        let (mut app, client, _layer_entity, _helper) = block_place_app(
            inventory_with_item(item_instance(9631, "trade_crate", 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("trade_crate", ItemCategory::Misc, Some("storage_crate")),
            item_template("bone_coin_stack", ItemCategory::Misc),
        ])));
        app.insert_resource(InventoryInstanceIdAllocator::new(9730));
        app.init_resource::<crate::inventory::DroppedLootRegistry>();
        app.world_mut().entity_mut(client).insert((
            GameMode::Survival,
            CurrentDimension(DimensionKind::Overworld),
        ));
        app.add_event::<DiggingEvent>();
        app.add_systems(
            Update,
            crate::world::container_block::handle_container_block_break,
        );

        let pos = BlockPos::new(1, 64, 1);
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            item_instance_id: 9631,
            target_face: TrapTargetFace::Top,
        });
        app.update();

        {
            let mut query = app.world_mut().query::<&mut ExternalContainer>();
            let mut ext = query
                .iter_mut(app.world_mut())
                .next()
                .expect("placed crate should spawn an ExternalContainer");
            ext.container.items.push(PlacedItemState {
                row: 0,
                col: 0,
                instance: item_instance(9632, "bone_coin_stack", 3),
            });
        }

        send_stop_break(&mut app, client, pos);
        send_stop_break(&mut app, client, pos);
        app.update();

        assert_eq!(
            dropped_template_count(&app, "trade_crate"),
            1,
            "two same-tick break events should drop one container item, not duplicate economy output"
        );
        assert_eq!(
            dropped_template_count(&app, "bone_coin_stack"),
            1,
            "two same-tick break events should drain stored contents once"
        );
    }

    #[test]
    fn breaking_container_matches_player_dimension() {
        let (mut app, client, _layer_entity, _helper) = block_place_app(
            inventory_with_item(item_instance(9641, "trade_crate", 1)),
            DimensionKind::Tsy,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("trade_crate", ItemCategory::Misc, Some("storage_crate")),
        ])));
        app.insert_resource(InventoryInstanceIdAllocator::new(9740));
        app.init_resource::<crate::inventory::DroppedLootRegistry>();
        app.world_mut()
            .entity_mut(client)
            .insert((GameMode::Survival, CurrentDimension(DimensionKind::Tsy)));
        app.add_event::<DiggingEvent>();
        app.add_systems(
            Update,
            crate::world::container_block::handle_container_block_break,
        );

        let pos = BlockPos::new(1, 64, 1);
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            item_instance_id: 9641,
            target_face: TrapTargetFace::Top,
        });
        app.update();

        let container_entity = app
            .world_mut()
            .query_filtered::<Entity, With<ExternalContainer>>()
            .iter(app.world())
            .next()
            .expect("TSY placement should spawn a container entity");
        let session_id = app
            .world()
            .get::<ExternalContainer>(container_entity)
            .expect("container should carry external state")
            .session_id;

        app.world_mut()
            .entity_mut(client)
            .insert(CurrentDimension(DimensionKind::Overworld));
        send_stop_break(&mut app, client, pos);
        app.update();

        assert!(
            app.world().get_entity(container_entity).is_some(),
            "Overworld break event at same coordinates must not despawn a TSY container"
        );
        assert!(
            app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .contains_key(&session_id),
            "cross-dimension miss must keep the TSY container session registered"
        );
        assert_eq!(
            dropped_template_count(&app, "trade_crate"),
            0,
            "cross-dimension miss must not create dropped loot"
        );

        app.world_mut()
            .entity_mut(client)
            .insert(CurrentDimension(DimensionKind::Tsy));
        send_stop_break(&mut app, client, pos);
        app.update();

        assert!(
            app.world().get_entity(container_entity).is_none(),
            "TSY break event should despawn the TSY container"
        );
        assert_eq!(
            dropped_template_dimension_count(&app, "trade_crate", DimensionKind::Tsy),
            1,
            "matched TSY break should drop the container item in TSY"
        );
    }

    #[test]
    fn breaking_empty_dead_drop_by_non_owner_triggers_ward_without_drops() {
        let (mut app, owner, _layer_entity, _helper) = dead_drop_break_app(9801);
        let pos = place_dead_drop(&mut app, owner, 9801);
        move_client(
            &mut app,
            owner,
            [12.5, 64.0, 12.5],
            DimensionKind::Overworld,
        );
        let breaker = spawn_test_client(
            &mut app,
            "Breaker",
            [1.5, 64.0, 1.5],
            DimensionKind::Overworld,
        );

        send_stop_break(&mut app, breaker, pos);
        app.update();

        assert_eq!(
            dropped_template_count(&app, "dead_drop_box"),
            0,
            "illegal empty dead drop break should ash the container instead of dropping it"
        );
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .any(|event| event.target == breaker && event.attacker == owner),
            "illegal empty dead drop break should still trigger poison AoE against the breaker"
        );
    }

    #[test]
    fn breaking_legacy_dead_drop_without_ward_uses_placed_by_as_owner() {
        let (mut app, owner, _layer_entity, _helper) = dead_drop_break_app(9806);
        let pos = place_dead_drop(&mut app, owner, 9806);
        let container_entity = placed_dead_drop_entity(&mut app);
        app.world_mut()
            .entity_mut(container_entity)
            .remove::<DeadDropWard>();
        move_client(
            &mut app,
            owner,
            [12.5, 64.0, 12.5],
            DimensionKind::Overworld,
        );
        let breaker = spawn_test_client(
            &mut app,
            "Breaker",
            [1.5, 64.0, 1.5],
            DimensionKind::Overworld,
        );

        send_stop_break(&mut app, breaker, pos);
        app.update();

        assert_eq!(
            dropped_template_count(&app, "dead_drop_box"),
            0,
            "legacy dead drop without DeadDropWard must not fall through to normal drops"
        );
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .any(|event| event.target == breaker && event.attacker == owner),
            "legacy dead drop without DeadDropWard should fall back to ContainerBlock::placed_by for attacker attribution"
        );
    }

    #[test]
    fn breaking_full_open_dead_drop_by_non_owner_ashes_contents_and_emits_ward_feedback() {
        let (mut app, owner, _layer_entity, mut helper) = dead_drop_break_app(9811);
        let pos = place_dead_drop(&mut app, owner, 9811);
        move_client(
            &mut app,
            owner,
            [12.5, 64.0, 12.5],
            DimensionKind::Overworld,
        );
        let breaker = spawn_test_client(
            &mut app,
            "Breaker",
            [1.5, 64.0, 1.5],
            DimensionKind::Overworld,
        );
        let boundary_target = spawn_test_client(
            &mut app,
            "Boundary",
            [4.5, 64.0, 1.5],
            DimensionKind::Overworld,
        );
        let outside_target = spawn_test_client(
            &mut app,
            "Outside",
            [4.51, 64.0, 1.5],
            DimensionKind::Overworld,
        );
        let container_entity = fill_open_dead_drop(&mut app, owner, 9, 9820);
        let session_id = app
            .world()
            .get::<ExternalContainer>(container_entity)
            .expect("dead drop should exist before illegal break")
            .session_id;

        send_stop_break(&mut app, breaker, pos);
        app.update();
        flush_all_client_packets(&mut app);

        assert!(
            app.world().get_entity(container_entity).is_none(),
            "illegal break should despawn the dead drop marker"
        );
        assert!(
            !app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .contains_key(&session_id),
            "illegal break should remove the external container session"
        );
        assert!(
            app.world()
                .resource::<crate::inventory::DroppedLootRegistry>()
                .entries
                .is_empty(),
            "full illegal dead drop break should ash both container and contents with zero dropped loot"
        );
        assert!(
            has_loot_container_close_payload(
                &mut helper,
                LootContainerCloseReasonV1::ContainerDestroyed
            ),
            "open illegal break must force-close the owner UI before the ash branch"
        );

        let combat_events = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        let hit_targets = combat_events
            .iter()
            .map(|event| event.target)
            .collect::<HashSet<_>>();
        assert_eq!(
            hit_targets,
            HashSet::from([breaker, boundary_target]),
            "ward AoE should include breaker and exact 3.0-block boundary target, but exclude outside target {:?}",
            outside_target
        );
        for event in &combat_events {
            assert_eq!(event.attacker, owner, "ward attacker should be owner");
            assert_eq!(event.body_part, BodyPart::Chest);
            assert_eq!(event.wound_kind, WoundKind::Blunt);
            assert_eq!(event.source, AttackSource::Melee);
            assert_eq!(event.damage, DEAD_DROP_WARD_DAMAGE);
            assert_eq!(event.contam_delta, DEAD_DROP_WARD_CONTAM_DELTA);
        }

        let status_events = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        let slowed_targets = status_events
            .iter()
            .map(|event| event.target)
            .collect::<HashSet<_>>();
        assert_eq!(
            slowed_targets,
            HashSet::from([breaker, boundary_target]),
            "ward status AoE should match combat AoE target set"
        );
        for event in &status_events {
            assert_eq!(event.kind, StatusEffectKind::Slowed);
            assert_eq!(event.magnitude, DEAD_DROP_WARD_SLOW_MAGNITUDE);
            assert_eq!(event.duration_ticks, DEAD_DROP_WARD_SLOW_DURATION_TICKS);
        }

        let vfx_events = app
            .world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(vfx_events.len(), 1, "ward break should emit one VFX event");
        match &vfx_events[0].payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                color,
                count,
                duration_ticks,
                ..
            } => {
                assert_eq!(event_id, gameplay_vfx::DEAD_DROP_WARD_BREAK);
                assert_eq!(color.as_deref(), Some("#3AA0C0"));
                assert_eq!(*count, Some(12));
                assert_eq!(*duration_ticks, Some(20));
            }
            other => panic!("expected SpawnParticle ward VFX payload, got {other:?}"),
        }

        let audio_events = app
            .world()
            .resource::<Events<PlaySoundRecipeRequest>>()
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            audio_events
                .iter()
                .any(|event| event.recipe_id == DEAD_DROP_WARD_BREAK_AUDIO_RECIPE_ID),
            "illegal break should emit the three-layer dead_drop_ward_break SFX recipe"
        );
    }

    #[test]
    fn breaking_dead_drop_by_owner_uses_normal_drop_without_ward_feedback() {
        let (mut app, owner, _layer_entity, _helper) = dead_drop_break_app(9901);
        let pos = place_dead_drop(&mut app, owner, 9901);
        fill_open_dead_drop(&mut app, owner, 1, 9910);

        send_stop_break(&mut app, owner, pos);
        app.update();

        assert_eq!(
            dropped_template_count(&app, "dead_drop_box"),
            1,
            "owner break should preserve P0 common container drop path"
        );
        assert_eq!(
            dropped_template_count(&app, "bone_coin_stack"),
            1,
            "owner break should drain stored contents instead of ashing them"
        );
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "owner break must not trigger ward combat"
        );
        assert!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "owner break must not trigger ward VFX"
        );
        let ward_audio_count = app
            .world()
            .resource::<Events<PlaySoundRecipeRequest>>()
            .iter_current_update_events()
            .filter(|event| event.recipe_id == DEAD_DROP_WARD_BREAK_AUDIO_RECIPE_ID)
            .count();
        assert_eq!(ward_audio_count, 0, "owner break must not play ward SFX");
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
    fn handler_rejects_unknown_placeable_without_consuming_or_writing() {
        let (mut app, client, layer_entity, _helper) = block_place_app(
            inventory_with_item(item_instance(9501, "bad_crate", 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("bad_crate", ItemCategory::Misc, Some("unknown_crate")),
        ])));
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: 1,
            y: 64,
            z: 1,
            item_instance_id: 9501,
            target_face: TrapTargetFace::Top,
        });

        app.update();

        assert_eq!(
            block_state_at(&app, layer_entity, BlockPos::new(1, 64, 1)),
            Some(BlockState::AIR)
        );
        assert_eq!(
            inventory_template_count(&app, client, "bad_crate"),
            1,
            "unknown placeable marker must reject before consuming inventory"
        );
    }

    #[test]
    fn herb_crate_placed_uses_storage_crate_marker_but_places_herb_variant() {
        assert_eq!(
            placeable_kind_for_item("trade_crate", "storage_crate"),
            Some(PlaceableBlockKind::StorageCrate { is_herb: false })
        );
        assert_eq!(
            placeable_kind_for_item("herb_crate_placed", "storage_crate"),
            Some(PlaceableBlockKind::StorageCrate { is_herb: true })
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

    fn dead_drop_break_app(item_instance_id: u64) -> (App, Entity, Entity, MockClientHelper) {
        let (mut app, client, layer_entity, helper) = block_place_app(
            inventory_with_item(item_instance(item_instance_id, "dead_drop_box", 1)),
            DimensionKind::Overworld,
        );
        app.insert_resource(ItemRegistry::from_map(HashMap::from([
            item_template_with_placeable("dead_drop_box", ItemCategory::Misc, Some("dead_drop")),
            item_template("bone_coin_stack", ItemCategory::Misc),
        ])));
        app.insert_resource(InventoryInstanceIdAllocator::new(20_000));
        app.init_resource::<crate::inventory::DroppedLootRegistry>();
        app.add_event::<DiggingEvent>();
        app.add_event::<CombatEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(
            Update,
            crate::world::container_block::handle_container_block_break,
        );
        app.world_mut().entity_mut(client).insert((
            GameMode::Survival,
            CurrentDimension(DimensionKind::Overworld),
        ));
        move_other_clients_far(&mut app, client);
        (app, client, layer_entity, helper)
    }

    fn place_dead_drop(app: &mut App, client: Entity, item_instance_id: u64) -> BlockPos {
        let pos = BlockPos::new(1, 64, 1);
        app.world_mut().send_event(BlockPlaceRequest {
            client,
            x: pos.x,
            y: pos.y,
            z: pos.z,
            item_instance_id,
            target_face: TrapTargetFace::Top,
        });
        app.update();
        pos
    }

    fn placed_dead_drop_entity(app: &mut App) -> Entity {
        let mut query = app.world_mut().query::<(Entity, &ContainerBlock)>();
        query
            .iter(app.world())
            .find(|(_, block)| block.kind == ContainerBlockKind::DeadDrop)
            .map(|(entity, _)| entity)
            .expect("placed dead drop should spawn a ContainerBlock entity")
    }

    fn fill_open_dead_drop(
        app: &mut App,
        opened_by: Entity,
        item_count: usize,
        first_instance_id: u64,
    ) -> Entity {
        let mut query = app.world_mut().query::<(Entity, &mut ExternalContainer)>();
        let (entity, mut ext) = query
            .iter_mut(app.world_mut())
            .find(|(_, ext)| ext.source_kind == ExternalContainerKind::DeadDrop)
            .expect("placed dead drop should spawn an ExternalContainer");
        ext.opened_by = Some(opened_by);
        ext.container.items.clear();
        for index in 0..item_count {
            ext.container.items.push(PlacedItemState {
                row: (index / 3) as u8,
                col: (index % 3) as u8,
                instance: item_instance(first_instance_id + index as u64, "bone_coin_stack", 1),
            });
        }
        entity
    }

    fn spawn_test_client(
        app: &mut App,
        username: &str,
        position: [f64; 3],
        dimension: DimensionKind,
    ) -> Entity {
        let (mut bundle, _helper) = create_mock_client(username);
        bundle.player.position = Position::new(position);
        let entity = app.world_mut().spawn(bundle).id();
        app.world_mut()
            .entity_mut(entity)
            .insert((GameMode::Survival, CurrentDimension(dimension)));
        entity
    }

    fn move_client(app: &mut App, entity: Entity, position: [f64; 3], dimension: DimensionKind) {
        app.world_mut()
            .entity_mut(entity)
            .insert((Position::new(position), CurrentDimension(dimension)));
    }

    fn move_other_clients_far(app: &mut App, keep: Entity) {
        let extras = app
            .world_mut()
            .query_filtered::<Entity, With<Client>>()
            .iter(app.world())
            .filter(|entity| *entity != keep)
            .collect::<Vec<_>>();
        for entity in extras {
            move_client(app, entity, [12.5, 64.0, 12.5], DimensionKind::Overworld);
        }
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
        app.insert_resource(FurnitureRegistry::default());
        app.init_resource::<ExternalContainerRegistry>();
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

    fn has_loot_container_close_payload(
        helper: &mut MockClientHelper,
        expected_reason: LootContainerCloseReasonV1,
    ) -> bool {
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
            if let ServerDataPayloadV1::LootContainerClose(close) = payload.payload {
                if close.reason == expected_reason {
                    return true;
                }
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
                readable_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
            },
        )
    }

    fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 2,
                cols: 9,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],

                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 2,
                cols: 9,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    fn inventory_with_equipped_held_item(item: ItemInstance) -> PlayerInventory {
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents {
                worn: Vec::new(),
                held: Some(item),
            },
        );
        inventory
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
            .chain(inventory.equipped.values().flat_map(|s| s.iter_all()))
            .filter(|item| item.template_id == template_id)
            .map(|item| item.stack_count)
            .sum()
    }

    fn dropped_template_count(app: &App, template_id: &str) -> usize {
        app.world()
            .resource::<crate::inventory::DroppedLootRegistry>()
            .entries
            .values()
            .filter(|entry| entry.item.template_id == template_id)
            .count()
    }

    fn dropped_template_dimension_count(
        app: &App,
        template_id: &str,
        dimension: DimensionKind,
    ) -> usize {
        app.world()
            .resource::<crate::inventory::DroppedLootRegistry>()
            .entries
            .values()
            .filter(|entry| entry.item.template_id == template_id && entry.dimension == dimension)
            .count()
    }
}
