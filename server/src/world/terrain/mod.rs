mod authored;
mod biome;
pub(crate) mod blocks;
pub mod broken_peaks;
mod column;
mod decoration;
mod flora;
mod giant_sword;
pub(crate) mod mega_tree;
mod noise;
mod raster;
mod spatial;
pub(super) mod structures;
mod wilderness;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use valence::prelude::{
    ident, App, BiomeRegistry, BlockState, Chunk, ChunkLayer, ChunkPos, ChunkView, Client,
    Commands, DimensionTypeRegistry, Entity, IntoSystemConfigs, Local, Position, Query, Res,
    ResMut, Resource, Server, UnloadedChunk, Update, View, VisibleChunkLayer, With,
};

use crate::mineral::{MineralOreIndex, MineralOreNode};
use crate::world::dimension::{DimensionKind, DimensionLayers, OverworldLayer};

#[allow(unused_imports)]
pub use raster::{
    raster_dir_from_manifest_path, FossilBbox, Poi, TerrainProvider, TerrainProviders,
};

// plan-supply-coffin-v1：物资棺刷新选点需要 zone xz 边界。其余 giant_sword 内部
// 实现仍保持 `mod`-private，只对外暴露 zone bounds 这一最小接口。
pub use giant_sword::sword_sea_xz_bounds;

// Valence 0.2x still serializes chunk heightmaps as fixed 9-bit packed arrays
// (37 longs). Vanilla clients choose the expected heightmap size from the
// advertised dimension height; 512 would require 10-bit arrays (43 longs) and
// make the client ignore every chunk heightmap. 496 is the highest 16-aligned
// height that stays within the 9-bit client contract.
pub const WORLD_HEIGHT: u32 = 496;
pub const MIN_Y: i32 = -64;

/// Surface information for a single world column, used by NPC navigation and
/// supply coffin spawn validation.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct SurfaceInfo {
    /// The Y coordinate of the top solid block.
    pub y: i32,
    /// Whether an NPC can stand on this column (no deep water or lava).
    pub passable: bool,
    /// Per-column water top Y. `i32::MIN` when no water body exists in this column.
    pub water_y: i32,
}

/// Trait for querying terrain surface height and walkability.
///
/// Implemented by [`TerrainProvider`] for production use.  Tests can supply
/// lightweight mocks (flat plane, slope, cliff, etc.) without touching raster
/// files.
pub trait SurfaceProvider {
    fn query_surface(&self, world_x: i32, world_z: i32) -> SurfaceInfo;
}

impl SurfaceProvider for TerrainProvider {
    fn query_surface(&self, world_x: i32, world_z: i32) -> SurfaceInfo {
        let sample = self.sample(world_x, world_z);
        let y = column::surface_y_for_sample(&sample, MIN_Y, WORLD_HEIGHT as i32);
        let has_water = sample.water_level >= 0.0;
        let water_top = if has_water {
            sample.water_level.round() as i32
        } else {
            i32::MIN
        };
        let passable = (!has_water || water_top <= y) && sample.surface_block != BlockState::LAVA;
        SurfaceInfo {
            y,
            passable,
            water_y: water_top,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterBootstrapConfig {
    pub manifest_path: PathBuf,
    pub raster_dir: PathBuf,
}

#[derive(Default)]
struct GeneratedChunks {
    loaded: HashSet<ChunkPos>,
}

impl Resource for GeneratedChunks {}

pub fn register(app: &mut App) {
    app.insert_resource(GeneratedChunks::default())
        .insert_resource(TickRateProbe::default())
        .add_systems(
            Update,
            generate_chunks_around_players
                .after(crate::player::attach_player_state_to_joined_clients),
        )
        .add_systems(
            Update,
            remove_unviewed_chunks.after(generate_chunks_around_players),
        )
        .add_systems(Update, log_tick_rate)
        // 修复"非 spawn 位置穿地坠落"：会话中途 ViewDistance 变化让原版客户端重建
        // 区块存储、丢掉已加载 chunk，而 Valence 只补发视野差集 → 客户端缺脚下 chunk
        // 的碰撞 → 穿地。本系统检测并恢复（重发 chunk + 弹回地表，真虚空则回 spawn）。
        .add_systems(Update, recover_fall_through);
}

/// 玩家穿过自己脚下方块（即客户端丢失了服务端仍持有的 chunk 碰撞）时低于地板多少
/// 才判定为"掉出世界"、传回 spawn。方块最低只能落在 `min_y`（基岩），低于它必为虚空。
const VOID_RESCUE_MARGIN: i32 = 16;

/// [`recover_fall_through`] 的恢复决策（纯函数，便于饱和单测）。
#[derive(Debug, PartialEq, Eq)]
enum FallRecovery {
    /// 正常站立 / 合法地下（脚下无碰撞空间）—— 不处理。
    None,
    /// 脚下是服务端固体方块（有碰撞箱）= 客户端缺这块 chunk、正穿过它 ——
    /// 重发该 chunk 给客户端 + 把人弹回地表。
    ResendAndBounce,
    /// 真·掉出世界底（脚下无碰撞且已低于地板 - margin）—— 传回 spawn 兜底。
    Spawn,
}

/// 纯决策：合法的挖矿/洞穴/游泳玩家所在处是无碰撞空间（air/water/植物碰撞箱为空），
/// 永远不会命中 `ResendAndBounce`；只有"身体重叠服务端固体方块"才判为穿地。
fn decide_fall_recovery(
    feet_obstructed: bool,
    player_y: f64,
    dimension_floor_y: i32,
) -> FallRecovery {
    if feet_obstructed {
        FallRecovery::ResendAndBounce
    } else if player_y < f64::from(dimension_floor_y - VOID_RESCUE_MARGIN) {
        FallRecovery::Spawn
    } else {
        FallRecovery::None
    }
}

/// 修复"进服/传送到非 spawn 位置后穿地无限坠落"。
///
/// 根因：会话中途 ViewDistance 变化（玩家客户端的渲染距离设置在登录后 ~2s 到达 →
/// 服务端 VD 跳变）会让原版 MC 客户端**重建区块存储、丢掉已加载的 chunk**，而 Valence
/// 的 `update_view_and_layers` 在 VD 变化时只补发"视野差集"（新增外圈），不重发客户端
/// 丢掉的（含玩家脚下那块）→ 客户端没了脚下 chunk 的碰撞 → 穿过实心地表无限坠落。
/// （spawn 不受影响：chunk 被反复进出焐热，且玩家从高空落向近地表先落地。）
///
/// 修法（服务端 workaround，无需 fork valence_server）：每 tick 检查玩家身体是否重叠
/// 服务端的固体方块——若是，说明客户端缺这块 chunk，于是 `remove_chunk`+`insert_chunk`
/// 重新插入它产生 LOAD layer-message，`handle_layer_messages` 据此把整块重发给在场
/// 客户端（此时 `OldView` 已追上玩家位置，消息不会被过滤），客户端重新拿到 → 恢复碰撞；
/// 同时把玩家弹回该列地表上方落稳。真·掉出世界底（脚下无 chunk）则传回 spawn 兜底。
#[allow(clippy::type_complexity)]
fn recover_fall_through(
    providers: Option<Res<TerrainProviders>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut layers: Query<&mut ChunkLayer>,
    mut clients: Query<(Entity, &mut Position, &VisibleChunkLayer), With<Client>>,
    server: Res<Server>,
    mut last_resend_tick: Local<HashMap<Entity, i64>>,
) {
    let (Some(providers), Some(dimension_layers)) = (providers, dimension_layers) else {
        return;
    };
    let overworld = dimension_layers.overworld;
    let tick = server.current_tick();
    let Ok(mut layer) = layers.get_mut(overworld) else {
        return;
    };
    let floor_y = layer.min_y();
    for (entity, mut position, visible_chunk_layer) in &mut clients {
        if visible_chunk_layer.0 != overworld {
            continue;
        }
        let p = position.get();
        let bx = p.x.floor() as i32;
        let by = p.y.floor() as i32;
        let bz = p.z.floor() as i32;
        // 玩家身体所在方块在服务端有碰撞箱 = 重叠固体 = 客户端缺这块 chunk、正穿过它。
        let feet_obstructed = layer
            .block([bx, by, bz])
            .map(|b| b.state.collision_shapes().len() > 0)
            .unwrap_or(false);
        match decide_fall_recovery(feet_obstructed, p.y, floor_y) {
            FallRecovery::None => {
                last_resend_tick.remove(&entity);
            }
            FallRecovery::Spawn => {
                position.set(crate::player::spawn_position());
                last_resend_tick.remove(&entity);
                tracing::warn!(
                    "[bong][world] {:?} fell out of world (y={:.1} < floor {} - {}) → rescued to spawn",
                    entity,
                    p.y,
                    floor_y,
                    VOID_RESCUE_MARGIN
                );
            }
            FallRecovery::ResendAndBounce => {
                // 节流：一次重发 + 弹回通常就能让客户端落稳；10 tick 内不重复，避免
                // 在客户端尚未处理完重发时反复 remove/insert。
                if last_resend_tick
                    .get(&entity)
                    .is_some_and(|&t| tick - t < 10)
                {
                    continue;
                }
                let cp = ChunkPos::new(bx.div_euclid(16), bz.div_euclid(16));
                if let Some(chunk) = layer.remove_chunk(cp) {
                    layer.insert_chunk(cp, chunk);
                }
                let surface = providers.overworld.query_surface(bx, bz).y;
                position.set([p.x, f64::from(surface + 2), p.z]);
                last_resend_tick.insert(entity, tick);
                tracing::debug!(
                    "[bong][world] {:?} phased through chunk ({},{}) at y={:.1} → re-sent chunk + bounced to surface {}",
                    entity,
                    cp.x,
                    cp.z,
                    p.y,
                    surface
                );
            }
        }
    }
}

struct TickRateProbe {
    last_log_tick: i64,
    last_log_instant: std::time::Instant,
}

impl Default for TickRateProbe {
    fn default() -> Self {
        Self {
            last_log_tick: 0,
            last_log_instant: std::time::Instant::now(),
        }
    }
}

impl Resource for TickRateProbe {}

/// 每 200 tick 输出一次实测 TPS。理想 20.0；明显低于（如 5–10）说明某 system
/// 单 tick 跑超 50ms，所有 packet 处理（drop/pickup/cmd/chat）会按比例延迟。
fn log_tick_rate(server: Res<Server>, mut probe: ResMut<TickRateProbe>) {
    let tick = server.current_tick();
    let delta_ticks = tick - probe.last_log_tick;
    if delta_ticks < 200 {
        return;
    }
    let now = std::time::Instant::now();
    let elapsed = now.duration_since(probe.last_log_instant);
    let actual_tps = delta_ticks as f64 / elapsed.as_secs_f64();
    tracing::info!(
        target: "bong::tick",
        "tick {tick}: actual TPS = {actual_tps:.1} (target 20.0; below 15 means systems overrun)"
    );
    probe.last_log_tick = tick;
    probe.last_log_instant = now;
}

pub fn spawn_raster_world(
    commands: &mut Commands,
    server: &Server,
    dimensions: &mut DimensionTypeRegistry,
    biomes: &BiomeRegistry,
    config: RasterBootstrapConfig,
) -> Entity {
    let provider = TerrainProvider::load(&config.manifest_path, &config.raster_dir, biomes)
        .unwrap_or_else(|error| panic!("failed to bootstrap raster terrain: {error}"));
    tracing::info!(
        "[bong][world] loaded {} terrain tiles / {} POIs / {} decorations / {} placements from {}",
        provider.tile_count(),
        provider.pois().len(),
        provider.decoration_count(),
        provider.placement_block_count(),
        config.manifest_path.display()
    );

    if let Some((_, _, dim)) = dimensions
        .iter_mut()
        .find(|(_, name, _)| *name == ident!("overworld").as_str_ident())
    {
        dim.height = WORLD_HEIGHT as i32;
        dim.logical_height = WORLD_HEIGHT as i32;
    }

    let layer = valence::prelude::LayerBundle::new(ident!("overworld"), dimensions, biomes, server);
    let entity = commands.spawn((layer, OverworldLayer)).id();

    // plan-tsy-worldgen-v1 §6.1 — optional TSY raster manifest from
    // BONG_TSY_RASTER_PATH; absent → tsy=None (legacy behaviour).
    let tsy_provider = load_tsy_provider_from_env(biomes);

    commands.insert_resource(TerrainProviders {
        overworld: provider,
        tsy: tsy_provider,
    });
    entity
}

const TSY_RASTER_PATH_ENV_VAR: &str = "BONG_TSY_RASTER_PATH";

fn load_tsy_provider_from_env(biomes: &BiomeRegistry) -> Option<TerrainProvider> {
    let raw = std::env::var_os(TSY_RASTER_PATH_ENV_VAR)?;
    if raw.is_empty() {
        return None;
    }
    let manifest_path = PathBuf::from(raw);
    let raster_dir = match raster_dir_from_manifest_path(&manifest_path) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(
                "[bong][world] BONG_TSY_RASTER_PATH={} unreadable: {error}",
                manifest_path.display()
            );
            return None;
        }
    };
    match TerrainProvider::load(&manifest_path, &raster_dir, biomes) {
        Ok(provider) => {
            tracing::info!(
                "[bong][world] loaded TSY {} terrain tiles / {} POIs from {}",
                provider.tile_count(),
                provider.pois().len(),
                manifest_path.display()
            );
            Some(provider)
        }
        Err(error) => {
            tracing::warn!(
                "[bong][world] failed to load TSY raster {}: {error}",
                manifest_path.display()
            );
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_chunks_around_players(
    mut layers: Query<&mut ChunkLayer>,
    clients: Query<(View, &VisibleChunkLayer), With<Client>>,
    providers: Option<Res<TerrainProviders>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut generated: ResMut<GeneratedChunks>,
    mineral_index: Option<Res<MineralOreIndex>>,
    mineral_nodes: Query<&MineralOreNode>,
    harvested_spiritwood: Option<Res<crate::spiritwood::SpiritWoodHarvestedLogs>>,
) {
    let Some(providers) = providers else {
        return;
    };
    let terrain = &providers.overworld;
    let generated = generated.as_mut();

    // For now we only generate raster-backed chunks for the overworld layer.
    // TSY chunk routing arrives with `plan-tsy-worldgen-v1`.
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let overworld_layer_entity = dimension_layers.overworld;

    let Ok(mut layer) = layers.get_mut(overworld_layer_entity) else {
        return;
    };

    // 每 client 每 tick 最多新生成的 chunk 数 —— 防止首次连接 / 远程传送
    // 时一帧内同步装填整个 view（200+ chunk）冻住 server tick，让玩家所有
    // 交互包括 drop/pickup/chat/cmd 都卡几秒。每 chunk 装填实测约 30-50ms
    // （column resolve + flora 双 loop + decoration + structures + mineral
    // overlay），4/tick 会让 tick 实际 ~150ms（5 TPS）依然卡 packet。
    // 降到 1/tick：tick budget 50ms 内尽量留给 packet 处理；view 256 chunk
    // 满载需 13 秒，期间 server tick 维持 20 TPS、操作即时响应。
    // 1/tick：每 chunk 装填 ~30ms，剩余 tick budget 给 packet/system；
    // NPC=0 + 这个值实测 TPS ≈ 20。NPC > 30 时再降到 0 + 加 LOD。
    //
    // 用 per-client budget 而非全局 budget：多人时全局 budget 会让靠前迭代
    // 的玩家持续吃光配额（移动中总有未见 chunk），后面玩家被无限饿死。
    // per-client 1/tick → N 玩家时总量 N/tick，但每个玩家都向前推进。
    const MAX_NEW_CHUNKS_PER_CLIENT_PER_TICK: usize = 1;

    for (view, visible_chunk_layer) in &clients {
        if visible_chunk_layer.0 != overworld_layer_entity {
            continue;
        }
        let mut client_budget = MAX_NEW_CHUNKS_PER_CLIENT_PER_TICK;
        for pos in view.get().iter() {
            if client_budget == 0 {
                break;
            }
            let already = generated.loaded.contains(&pos) || layer.chunk(pos).is_some();
            ensure_chunk_generated(
                &mut layer,
                pos,
                terrain,
                &mut generated.loaded,
                mineral_index.as_deref(),
                &mineral_nodes,
                harvested_spiritwood.as_deref(),
            );
            if !already {
                client_budget -= 1;
            }
        }
    }
}

fn remove_unviewed_chunks(
    mut layers: Query<&mut ChunkLayer>,
    clients: Query<(View, &VisibleChunkLayer), With<Client>>,
    providers: Option<Res<TerrainProviders>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut generated: ResMut<GeneratedChunks>,
) {
    if providers.is_none() {
        return;
    }
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let generated = generated.as_mut();

    let Ok(mut layer) = layers.get_mut(dimension_layers.overworld) else {
        return;
    };
    let visible_overworld_views = clients
        .iter()
        .filter_map(|(view, visible_chunk_layer)| {
            (visible_chunk_layer.0 == dimension_layers.overworld).then(|| view.get())
        })
        .collect::<Vec<_>>();

    generated.loaded.retain(|pos| layer.chunk(*pos).is_some());

    let mut removed = Vec::new();
    layer.retain_chunks(|pos, chunk| {
        let keep = chunk.viewer_count_mut() > 0
            || chunk_is_visible_in_any_view(pos, visible_overworld_views.iter().copied());
        if !keep {
            removed.push(pos);
        }
        keep
    });

    for pos in removed {
        generated.loaded.remove(&pos);
    }
}

fn chunk_is_visible_in_any_view(pos: ChunkPos, views: impl IntoIterator<Item = ChunkView>) -> bool {
    views.into_iter().any(|view| view.contains(pos))
}

fn ensure_chunk_generated(
    layer: &mut ChunkLayer,
    pos: ChunkPos,
    terrain: &TerrainProvider,
    generated: &mut HashSet<ChunkPos>,
    mineral_index: Option<&MineralOreIndex>,
    mineral_nodes: &Query<&MineralOreNode>,
    harvested_spiritwood: Option<&crate::spiritwood::SpiritWoodHarvestedLogs>,
) {
    if generated.contains(&pos) || layer.chunk(pos).is_some() {
        return;
    }

    let min_y = layer.min_y();
    let mut chunk = UnloadedChunk::with_height(WORLD_HEIGHT);
    let mut top_y_by_column = [[min_y; 16]; 16];
    for local_z in 0..16 {
        for local_x in 0..16 {
            let world_x = pos.x * 16 + local_x;
            let world_z = pos.z * 16 + local_z;
            let sample = terrain.sample(world_x, world_z);
            top_y_by_column[local_z as usize][local_x as usize] =
                column::fill_column(&mut chunk, local_x as u32, local_z as u32, min_y, &sample);
        }
    }

    decoration::decorate_chunk(&mut chunk, pos, min_y, terrain, &top_y_by_column);
    flora::decorate_chunk(&mut chunk, pos, min_y, terrain, &top_y_by_column);
    structures::decorate_chunk(&mut chunk, pos, min_y, terrain);
    // P1 — stamp authored structures from placement_manifest.json sidecar.
    // Runs after procedural decoration so authored buildings override them;
    // density mask (P0 worldgen) already removed flora inside compound radius.
    authored::place_authored_structures(&mut chunk, pos, min_y, terrain);
    giant_sword::decorate_chunk(&mut chunk, pos, min_y, terrain);
    overlay_mineral_ores(&mut chunk, pos, min_y, mineral_index, mineral_nodes);
    erase_harvested_spiritwood_logs(&mut chunk, pos, min_y, harvested_spiritwood);
    biome::fill_chunk_biomes(&mut chunk, pos.x, pos.z, WORLD_HEIGHT, terrain);
    layer.insert_chunk(pos, chunk);
    generated.insert(pos);
}

fn erase_harvested_spiritwood_logs(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    harvested_spiritwood: Option<&crate::spiritwood::SpiritWoodHarvestedLogs>,
) {
    let Some(harvested_spiritwood) = harvested_spiritwood else {
        return;
    };
    for block_pos in harvested_spiritwood.positions_in_chunk(DimensionKind::Overworld, pos) {
        let local_y = block_pos.y - min_y;
        if !(0..WORLD_HEIGHT as i32).contains(&local_y) {
            continue;
        }
        let local_x = block_pos.x.rem_euclid(16) as u32;
        let local_z = block_pos.z.rem_euclid(16) as u32;
        chunk.set_block_state(local_x, local_y as u32, local_z, BlockState::AIR);
    }
}

fn overlay_mineral_ores(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    mineral_index: Option<&MineralOreIndex>,
    mineral_nodes: &Query<&MineralOreNode>,
) {
    let Some(mineral_index) = mineral_index else {
        return;
    };

    for (dimension, block_pos, entity) in mineral_index.iter() {
        if dimension != DimensionKind::Overworld {
            continue;
        }
        if block_pos.x.div_euclid(16) != pos.x || block_pos.z.div_euclid(16) != pos.z {
            continue;
        }
        let Ok(node) = mineral_nodes.get(entity) else {
            continue;
        };
        set_mineral_block(chunk, block_pos, min_y, node.mineral_id);
    }
}

fn set_mineral_block(
    chunk: &mut UnloadedChunk,
    block_pos: valence::prelude::BlockPos,
    min_y: i32,
    mineral_id: crate::mineral::MineralId,
) {
    let local_y = block_pos.y - min_y;
    if !(0..WORLD_HEIGHT as i32).contains(&local_y) {
        return;
    }
    let local_x = block_pos.x.rem_euclid(16) as u32;
    let local_z = block_pos.z.rem_euclid(16) as u32;
    chunk.set_block_state(
        local_x,
        local_y as u32,
        local_z,
        mineral_block_state(mineral_id),
    );

    // 矿脉露头装饰：当矿石上方是 air（地表露头）时，在 4 邻方向 air 位上 50%
    // 概率堆 cobblestone，形成"石堆+矿石"的地表露头观感（用户明确要求）。
    // 地下深矿（上方仍是石头/矿石）不触发，保持原 vanilla 风格。
    let above_y = local_y + 1;
    if above_y >= WORLD_HEIGHT as i32 {
        return;
    }
    if !chunk.block_state(local_x, above_y as u32, local_z).is_air() {
        return;
    }
    for (i, (dx, dz)) in [(1_i32, 0_i32), (-1, 0), (0, 1), (0, -1)]
        .iter()
        .enumerate()
    {
        let nx = local_x as i32 + dx;
        let nz = local_z as i32 + dz;
        if !(0..16).contains(&nx) || !(0..16).contains(&nz) {
            continue;
        }
        let h = ore_outcrop_hash(block_pos.x + dx, block_pos.z + dz, 401 + i as u32);
        if h % 100 >= 50 {
            continue;
        }
        if !chunk
            .block_state(nx as u32, above_y as u32, nz as u32)
            .is_air()
        {
            continue;
        }
        chunk.set_block_state(
            nx as u32,
            above_y as u32,
            nz as u32,
            BlockState::COBBLESTONE,
        );
    }
}

fn ore_outcrop_hash(world_x: i32, world_z: i32, salt: u32) -> u32 {
    let mut value = (world_x as u32).wrapping_mul(0x85EB_CA6B);
    value = value.wrapping_add((world_z as u32).wrapping_mul(0xC2B2_AE35));
    value ^= salt.wrapping_mul(0x9E37_79B1);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value
}

fn mineral_block_state(mineral_id: crate::mineral::MineralId) -> BlockState {
    match mineral_id.vanilla_block() {
        "iron_ore" => BlockState::IRON_ORE,
        "deepslate_iron_ore" => BlockState::DEEPSLATE_IRON_ORE,
        "copper_ore" => BlockState::COPPER_ORE,
        "redstone_ore" => BlockState::REDSTONE_ORE,
        "ancient_debris" => BlockState::ANCIENT_DEBRIS,
        "obsidian" => BlockState::OBSIDIAN,
        "gold_ore" => BlockState::GOLD_ORE,
        "emerald_ore" => BlockState::EMERALD_ORE,
        "lapis_ore" => BlockState::LAPIS_ORE,
        "coal_ore" => BlockState::COAL_ORE,
        "nether_gold_ore" => BlockState::NETHER_GOLD_ORE,
        "nether_quartz_ore" => BlockState::NETHER_QUARTZ_ORE,
        "diamond_ore" => BlockState::DIAMOND_ORE,
        _ => BlockState::STONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mineral::MineralId;
    use valence::prelude::BlockPos;

    #[test]
    fn set_mineral_block_writes_matching_vanilla_block() {
        let pos = BlockPos::new(3, -12, 5);
        let mut chunk = UnloadedChunk::with_height(WORLD_HEIGHT);

        set_mineral_block(&mut chunk, pos, MIN_Y, MineralId::ZaGang);

        assert_eq!(
            chunk.block_state(3, (pos.y - MIN_Y) as u32, 5),
            BlockState::COPPER_ORE
        );
    }

    #[test]
    fn current_player_view_keeps_newly_generated_chunk() {
        let view = ChunkView::new(ChunkPos::new(0, 0), 2);

        assert!(chunk_is_visible_in_any_view(ChunkPos::new(0, 0), [view]));
    }

    #[test]
    fn chunk_outside_all_player_views_can_be_removed() {
        let view = ChunkView::new(ChunkPos::new(0, 0), 2);

        assert!(!chunk_is_visible_in_any_view(ChunkPos::new(64, 64), [view]));
    }

    #[test]
    fn fall_recovery_resends_when_feet_overlap_server_solid() {
        // 脚下是服务端固体方块（有碰撞箱）= 客户端缺该 chunk、正穿过它，必须重发 +
        // 弹回，无论 y 高低（这是 VD 跳变穿地的核心修复路径）。
        assert_eq!(
            decide_fall_recovery(true, 80.0, MIN_Y),
            FallRecovery::ResendAndBounce,
            "脚下重叠服务端固体方块时必须走重发+弹回，不论玩家 y"
        );
        assert_eq!(
            decide_fall_recovery(true, -1000.0, MIN_Y),
            FallRecovery::ResendAndBounce,
            "即便已掉到地板下，只要脚下仍有服务端固体方块就重发（说明 chunk 还在，客户端丢了）"
        );
    }

    #[test]
    fn fall_recovery_spawns_only_on_true_void_below_floor() {
        // 脚下无碰撞（真空洞）且已低于地板 - margin → 掉出世界，回 spawn 兜底。
        assert_eq!(
            decide_fall_recovery(false, f64::from(MIN_Y - VOID_RESCUE_MARGIN) - 0.1, MIN_Y),
            FallRecovery::Spawn,
            "脚下无固体且低于 floor-margin 即真·掉出世界，应回 spawn"
        );
    }

    #[test]
    fn fall_recovery_none_for_legit_underground_and_standing() {
        // 合法挖矿/洞穴（脚下是无碰撞空间）+ 在地板以上 → 绝不处理，杜绝误把矿工弹上地表。
        assert_eq!(
            decide_fall_recovery(false, 80.0, MIN_Y),
            FallRecovery::None,
            "地表站立（脚下无碰撞）不应触发恢复"
        );
        assert_eq!(
            decide_fall_recovery(false, -50.0, MIN_Y),
            FallRecovery::None,
            "深处洞穴（y=-50，仍在 floor-16=-80 以上，脚下无碰撞）属合法地下，不应触发"
        );
        // 边界：正好等于 floor - margin 不算掉出世界（用 < 而非 <=）。
        assert_eq!(
            decide_fall_recovery(false, f64::from(MIN_Y - VOID_RESCUE_MARGIN), MIN_Y),
            FallRecovery::None,
            "y == floor-margin 的边界不应判定为掉出世界"
        );
    }

    #[test]
    fn overworld_height_matches_valence_heightmap_encoding_budget() {
        const VALENCE_HEIGHTMAP_BITS_PER_ENTRY: u32 = 9;
        const COLUMN_COUNT: u32 = 16 * 16;

        let entries_per_long = i64::BITS / VALENCE_HEIGHTMAP_BITS_PER_ENTRY;
        let expected_longs =
            COLUMN_COUNT / entries_per_long + (COLUMN_COUNT % entries_per_long != 0) as u32;

        assert_eq!(WORLD_HEIGHT % 16, 0);
        assert!(heightmap_bits_for_dimension(WORLD_HEIGHT) <= VALENCE_HEIGHTMAP_BITS_PER_ENTRY);
        assert_eq!(expected_longs, 37);
    }

    fn heightmap_bits_for_dimension(height: u32) -> u32 {
        u32::BITS - height.leading_zeros()
    }
}
