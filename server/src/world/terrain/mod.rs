mod authored;
mod biome;
pub(crate) mod blocks;
pub mod broken_peaks;
pub mod column;
mod decoration;
mod flora;
mod giant_sword;
pub(crate) mod mega_tree;
// worldgen-v4 §8.1 #10 — `pub` so the out-of-crate criterion bench
// (`benches/nbt_stamp.rs`) can name `StructureNbt` (returned by the registry) and
// read its `blocks` len. No runtime behaviour change.
pub mod nbt_io;
// worldgen-v4 §8.1 #10 — `pub` (not `pub(crate)`) so the out-of-crate criterion
// bench (`benches/nbt_stamp.rs`) can drive the real `DecorationNbtRegistry::stamp`
// memcpy hot path. No runtime behaviour change.
pub mod nbt_registry;
mod noise;
mod raster;
mod spatial;
pub(super) mod structures;
mod wilderness;

// worldgen-v4 §8.1 #10 — bench/test-only fixture builder, kept in the lib so the
// out-of-crate criterion benches (`benches/chunk_generation.rs`) reach the real
// `TerrainProvider::load` → `sample` → `column::fill_column` path without
// duplicating the on-disk raster layout. Not referenced by any runtime system.
pub mod bench_support;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use valence::prelude::{
    ident, App, BiomeRegistry, BlockState, Chunk, ChunkLayer, ChunkPos, ChunkView, Client,
    Commands, DimensionTypeRegistry, Entity, IntoSystemConfigs, Local, Position, Query, Res,
    ResMut, Resource, Server, UnloadedChunk, Update, Username, View, VisibleChunkLayer, With,
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

// F12 — 按维度分桶的已生成 chunk 记录。overworld / TSY 各自独立的 `ChunkLayer`
// 实体，但 `ChunkPos` 坐标空间是共享的（都从 (0,0) 起算），若用单一
// `HashSet<ChunkPos>` 会导致"overworld 已生成 (x,z)"错误地让 TSY 同坐标的
// chunk 被判定为"已生成"而跳过（`ensure_chunk_generated` 的 early-return 只看
// `generated.contains(&pos)`，不知道是哪个维度）。
#[derive(Default)]
struct GeneratedChunks {
    loaded: HashMap<DimensionKind, HashSet<ChunkPos>>,
}

impl Resource for GeneratedChunks {}

pub fn register(app: &mut App) {
    // worldgen-v4 P6 §8.1 #10 — decompress every decoration NBT template once at
    // app-build time and hold it resident for the process lifetime, so chunk-gen
    // stamps are memcpy-level (no runtime gzip). Bootstrap-path agnostic: inserted
    // for raster / flat / anvil worlds alike. A missing `decorations/` dir (assets
    // authored in a later P6 stage) loads an empty registry — never panics.
    let deco_registry = nbt_registry::DecorationNbtRegistry::load_default();
    tracing::info!(
        "[bong][world] decoration NBT registry: {} templates resident",
        deco_registry.len()
    );
    app.insert_resource(deco_registry);
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
    mut clients: Query<
        (Entity, &mut Position, &VisibleChunkLayer, Option<&Username>),
        With<Client>,
    >,
    server: Res<Server>,
    mut last_resend_tick: Local<HashMap<Entity, i64>>,
) {
    let (Some(providers), Some(dimension_layers)) = (providers, dimension_layers) else {
        return;
    };
    let tick = server.current_tick();

    // F12 — 按维度遍历而非硬编码 overworld；无 raster 的维度（provider=None）
    // 跳过，与之前"TSY 玩家永远不会命中此系统"的行为一致，直到该维度真的有
    // raster provider 才开始为其玩家跑穿地恢复。
    for kind in [DimensionKind::Overworld, DimensionKind::Tsy] {
        let Some(terrain) = providers.for_dimension(kind) else {
            continue;
        };
        let layer_entity = dimension_layers.entity_for(kind);
        let Ok(mut layer) = layers.get_mut(layer_entity) else {
            continue;
        };
        let floor_y = layer.min_y();
        for (entity, mut position, visible_chunk_layer, username) in &mut clients {
            if visible_chunk_layer.0 != layer_entity {
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
                    let seed = username
                        .map(|username| username.0.as_str())
                        .unwrap_or("fall-recovery");
                    position.set(crate::player::spawn_position_for_seed(
                        seed,
                        crate::player::spawn_selector::SpawnPurpose::FallRecovery,
                    ));
                    last_resend_tick.remove(&entity);
                    tracing::warn!(
                        "[bong][world] {:?} fell out of world in {:?} (y={:.1} < floor {} - {}) → rescued to spawn",
                        entity,
                        kind,
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
                    let surface = terrain.query_surface(bx, bz).y;
                    position.set([p.x, f64::from(surface + 2), p.z]);
                    last_resend_tick.insert(entity, tick);
                    tracing::debug!(
                        "[bong][world] {:?} phased through chunk ({},{}) in {:?} at y={:.1} → re-sent chunk + bounced to surface {}",
                        entity,
                        cp.x,
                        cp.z,
                        kind,
                        p.y,
                        surface
                    );
                }
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
    deco_registry: Res<nbt_registry::DecorationNbtRegistry>,
) {
    let Some(providers) = providers else {
        return;
    };
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let generated = generated.as_mut();

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

    // F12 — 按维度遍历，而非硬编码 overworld。`for_dimension(kind)` 为 None
    // （目前只有未设 BONG_TSY_RASTER_PATH 时的 TSY）就跳过该维度，保持"没有
    // raster 就没有 chunk"的 legacy 空洞行为；提供该维度 raster 后自动接上同一套
    // 生成路径，不需要再改这三个系统。
    for kind in [DimensionKind::Overworld, DimensionKind::Tsy] {
        let Some(terrain) = providers.for_dimension(kind) else {
            continue;
        };
        let layer_entity = dimension_layers.entity_for(kind);
        let Ok(mut layer) = layers.get_mut(layer_entity) else {
            continue;
        };
        let loaded = generated.loaded.entry(kind).or_default();

        for (view, visible_chunk_layer) in &clients {
            if visible_chunk_layer.0 != layer_entity {
                continue;
            }
            let mut client_budget = MAX_NEW_CHUNKS_PER_CLIENT_PER_TICK;
            for pos in view.get().iter() {
                if client_budget == 0 {
                    break;
                }
                let already = loaded.contains(&pos) || layer.chunk(pos).is_some();
                ensure_chunk_generated(
                    &mut layer,
                    pos,
                    terrain,
                    loaded,
                    mineral_index.as_deref(),
                    &mineral_nodes,
                    harvested_spiritwood.as_deref(),
                    &deco_registry,
                    kind,
                );
                if !already {
                    client_budget -= 1;
                }
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
    let Some(providers) = providers else {
        return;
    };
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let generated = generated.as_mut();

    // F12 — 同 `generate_chunks_around_players`：按维度遍历而非硬编码 overworld，
    // 无 raster 的维度天然没有 `loaded` 记录，`retain`/`retain_chunks` 均为 no-op。
    for kind in [DimensionKind::Overworld, DimensionKind::Tsy] {
        if providers.for_dimension(kind).is_none() {
            continue;
        }
        let layer_entity = dimension_layers.entity_for(kind);
        let Ok(mut layer) = layers.get_mut(layer_entity) else {
            continue;
        };
        let visible_views = clients
            .iter()
            .filter_map(|(view, visible_chunk_layer)| {
                (visible_chunk_layer.0 == layer_entity).then(|| view.get())
            })
            .collect::<Vec<_>>();

        let loaded = generated.loaded.entry(kind).or_default();
        loaded.retain(|pos| layer.chunk(*pos).is_some());

        let mut removed = Vec::new();
        layer.retain_chunks(|pos, chunk| {
            let keep = chunk.viewer_count_mut() > 0
                || chunk_is_visible_in_any_view(pos, visible_views.iter().copied());
            if !keep {
                removed.push(pos);
            }
            keep
        });

        for pos in removed {
            loaded.remove(&pos);
        }
    }
}

fn chunk_is_visible_in_any_view(pos: ChunkPos, views: impl IntoIterator<Item = ChunkView>) -> bool {
    views.into_iter().any(|view| view.contains(pos))
}

#[allow(clippy::too_many_arguments)]
fn ensure_chunk_generated(
    layer: &mut ChunkLayer,
    pos: ChunkPos,
    terrain: &TerrainProvider,
    generated: &mut HashSet<ChunkPos>,
    mineral_index: Option<&MineralOreIndex>,
    mineral_nodes: &Query<&MineralOreNode>,
    harvested_spiritwood: Option<&crate::spiritwood::SpiritWoodHarvestedLogs>,
    deco_registry: &nbt_registry::DecorationNbtRegistry,
    dimension: DimensionKind,
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
    flora::decorate_chunk(
        &mut chunk,
        pos,
        min_y,
        terrain,
        &top_y_by_column,
        deco_registry,
    );
    structures::decorate_chunk(&mut chunk, pos, min_y, terrain, deco_registry);
    // P1 — stamp authored structures from placement_manifest.json sidecar.
    // Runs after procedural decoration so authored buildings override them;
    // density mask (P0 worldgen) already removed flora inside compound radius.
    authored::place_authored_structures(&mut chunk, pos, min_y, terrain);
    giant_sword::decorate_chunk(&mut chunk, pos, min_y, terrain);
    overlay_mineral_ores(
        &mut chunk,
        pos,
        min_y,
        mineral_index,
        mineral_nodes,
        dimension,
    );
    erase_harvested_spiritwood_logs(&mut chunk, pos, min_y, harvested_spiritwood, dimension);
    biome::fill_chunk_biomes(&mut chunk, pos.x, pos.z, WORLD_HEIGHT, terrain);
    layer.insert_chunk(pos, chunk);
    generated.insert(pos);
}

fn erase_harvested_spiritwood_logs(
    chunk: &mut UnloadedChunk,
    pos: ChunkPos,
    min_y: i32,
    harvested_spiritwood: Option<&crate::spiritwood::SpiritWoodHarvestedLogs>,
    dimension: DimensionKind,
) {
    let Some(harvested_spiritwood) = harvested_spiritwood else {
        return;
    };
    for block_pos in harvested_spiritwood.positions_in_chunk(dimension, pos) {
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
    dimension: DimensionKind,
) {
    let Some(mineral_index) = mineral_index else {
        return;
    };

    for (node_dimension, block_pos, entity) in mineral_index.iter() {
        if node_dimension != dimension {
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

    // F12 — TSY chunk routing pin 测试。用 `valence::testing::ScenarioSingleClient`
    // 建 overworld layer + 手动补一个 TSY `LayerBundle`，锁"按维度遍历"契约：
    // provider 存在则该维度玩家能拿到 chunk / 穿地恢复，provider 缺失则维持
    // legacy 空洞，且 overworld 路径不因 TSY provider 是否存在而回归。
    mod tsy_routing {
        use super::*;
        use crate::world::dimension::{register_tsy_dimension, TsyLayer};
        use std::collections::BTreeSet;
        use valence::prelude::{EntityLayerId, LayerBundle, VisibleEntityLayers};
        use valence::testing::ScenarioSingleClient;

        /// 建一个同时有 overworld + TSY 两个 `ChunkLayer` 的测试 App。
        /// 返回 (app, client, overworld_layer, tsy_layer)。
        fn two_dimension_test_app() -> (App, Entity, Entity, Entity) {
            let scenario = ScenarioSingleClient::new();
            let mut app = scenario.app;
            crate::world::dimension::mark_test_layer_as_overworld(&mut app);

            {
                let mut dimensions = app.world_mut().resource_mut::<DimensionTypeRegistry>();
                register_tsy_dimension(&mut dimensions);
            }
            let tsy_layer = {
                let world = app.world();
                let bundle = LayerBundle::new(
                    ident!("bong:tsy"),
                    world.resource::<DimensionTypeRegistry>(),
                    world.resource::<BiomeRegistry>(),
                    world.resource::<Server>(),
                );
                app.world_mut().spawn((bundle, TsyLayer)).id()
            };

            app.insert_resource(DimensionLayers {
                overworld: scenario.layer,
                tsy: tsy_layer,
            });
            app.insert_resource(GeneratedChunks::default());
            app.insert_resource(nbt_registry::DecorationNbtRegistry::empty());

            (app, scenario.client, scenario.layer, tsy_layer)
        }

        fn move_client_to_layer(app: &mut App, client: Entity, layer: Entity) {
            app.world_mut().entity_mut(client).insert((
                EntityLayerId(layer),
                VisibleChunkLayer(layer),
                VisibleEntityLayers(BTreeSet::from([layer])),
            ));
        }

        fn set_client_position(app: &mut App, client: Entity, pos: [f64; 3]) {
            app.world_mut()
                .entity_mut(client)
                .insert(Position::new(pos));
        }

        fn loaded_contains(app: &App, kind: DimensionKind, pos: ChunkPos) -> bool {
            app.world()
                .resource::<GeneratedChunks>()
                .loaded
                .get(&kind)
                .is_some_and(|set| set.contains(&pos))
        }

        fn loaded_count(app: &App, kind: DimensionKind) -> usize {
            app.world()
                .resource::<GeneratedChunks>()
                .loaded
                .get(&kind)
                .map_or(0, |set| set.len())
        }

        #[test]
        fn generate_chunks_creates_tsy_chunk_when_tsy_provider_present() {
            let (mut app, client, _overworld, tsy_layer) = two_dimension_test_app();
            move_client_to_layer(&mut app, client, tsy_layer);
            set_client_position(&mut app, client, [8.5, 64.0, 8.5]);
            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: Some(TerrainProvider::empty_for_tests()),
            });
            app.add_systems(Update, generate_chunks_around_players);

            app.update();

            assert_eq!(
                loaded_count(&app, DimensionKind::Tsy),
                1,
                "F12: a client whose VisibleChunkLayer points at the TSY layer must get a \
                 chunk generated once TerrainProviders.tsy is Some — this is the exact gap F12 \
                 closes (TerrainRuntime previously only ever looked at dimension_layers.overworld)"
            );
            assert_eq!(
                loaded_count(&app, DimensionKind::Overworld),
                0,
                "no client is viewing the overworld layer in this scenario, so it must stay untouched"
            );
        }

        #[test]
        fn generate_chunks_skips_tsy_entirely_when_provider_absent() {
            let (mut app, client, _overworld, tsy_layer) = two_dimension_test_app();
            move_client_to_layer(&mut app, client, tsy_layer);
            set_client_position(&mut app, client, [8.5, 64.0, 8.5]);
            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: None,
            });
            app.add_systems(Update, generate_chunks_around_players);

            app.update();

            assert_eq!(
                loaded_count(&app, DimensionKind::Tsy),
                0,
                "F12 must not regress the 'no BONG_TSY_RASTER_PATH configured' legacy behaviour: \
                 TerrainProviders.tsy=None must keep TSY a void, not panic or synthesize chunks"
            );
            assert!(
                app.world()
                    .get::<ChunkLayer>(tsy_layer)
                    .expect("tsy layer entity should still carry a ChunkLayer component")
                    .chunk(ChunkPos::new(0, 0))
                    .is_none(),
                "no chunk should have been inserted into the TSY ChunkLayer when its provider is absent"
            );
        }

        #[test]
        fn generate_chunks_overworld_unaffected_by_tsy_provider_presence() {
            let (mut app, client, overworld_layer, _tsy_layer) = two_dimension_test_app();
            move_client_to_layer(&mut app, client, overworld_layer);
            set_client_position(&mut app, client, [8.5, 64.0, 8.5]);
            // TSY provider 也存在 —— 证明 overworld 路径不受"TSY 是否也在跑"影响。
            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: Some(TerrainProvider::empty_for_tests()),
            });
            app.add_systems(Update, generate_chunks_around_players);

            app.update();

            assert_eq!(
                loaded_count(&app, DimensionKind::Overworld),
                1,
                "F12 regression check: overworld chunk generation for an overworld viewer must \
                 keep working exactly as before, even with a TSY provider now also present"
            );
            assert_eq!(
                loaded_count(&app, DimensionKind::Tsy),
                0,
                "no client is viewing TSY in this scenario, so TSY must stay untouched"
            );
        }

        #[test]
        fn generate_chunks_per_dimension_bookkeeping_does_not_cross_contaminate_same_chunkpos() {
            // 回归点：`GeneratedChunks.loaded` 从单一 `HashSet<ChunkPos>` 改成按维度分桶的
            // `HashMap<DimensionKind, HashSet<ChunkPos>>`。若退化回单一集合，overworld 在
            // (0,0) 生成的记录会让 TSY 同坐标 (0,0) 被误判"已生成"而跳过 —— 本测试用两个
            // 分别看 overworld / TSY 的 client，一次 `app.update()` 后两边都应各自拿到 1 个
            // chunk（而不是后者因"已生成"被跳过）。
            let (mut app, client_a, overworld_layer, tsy_layer) = two_dimension_test_app();
            move_client_to_layer(&mut app, client_a, overworld_layer);
            set_client_position(&mut app, client_a, [8.5, 64.0, 8.5]);

            // 第二个 client：用同一套 `create_mock_client` 先建出一个完整、合法的
            // `ClientBundle`（含 Position/ViewDistance 等所有必需组件），再用和
            // `move_client_to_layer`/`set_client_position` 相同的方式改写 layer 归属，
            // 避免直接猜测 `PlayerEntityBundle` 内部字段路径。
            let (second_bundle, _second_helper) =
                valence::testing::create_mock_client("second-tsy-client");
            let client_b = app.world_mut().spawn(second_bundle).id();
            move_client_to_layer(&mut app, client_b, tsy_layer);
            set_client_position(&mut app, client_b, [8.5, 64.0, 8.5]);

            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: Some(TerrainProvider::empty_for_tests()),
            });
            app.add_systems(Update, generate_chunks_around_players);

            app.update();

            assert_eq!(
                loaded_count(&app, DimensionKind::Overworld),
                1,
                "overworld viewer must still get its chunk at (0,0)"
            );
            assert_eq!(
                loaded_count(&app, DimensionKind::Tsy),
                1,
                "TSY viewer at the same world (x,z) must independently get its own chunk — a \
                 shared HashSet<ChunkPos> would have made this a false 'already generated' hit"
            );
        }

        #[test]
        fn remove_unviewed_chunks_evicts_tsy_chunks_independently_of_overworld() {
            let (mut app, client, _overworld, tsy_layer) = two_dimension_test_app();
            move_client_to_layer(&mut app, client, tsy_layer);
            set_client_position(&mut app, client, [8.5, 64.0, 8.5]);
            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: Some(TerrainProvider::empty_for_tests()),
            });
            app.add_systems(
                Update,
                (
                    generate_chunks_around_players,
                    remove_unviewed_chunks.after(generate_chunks_around_players),
                ),
            );

            let origin = ChunkPos::new(0, 0);
            app.update();
            assert!(
                loaded_contains(&app, DimensionKind::Tsy, origin),
                "sanity check: chunk (0,0) must exist before we can assert its eviction"
            );

            // 把 view distance 缩到最小并把玩家挪出很远，让原先那个 TSY chunk 脱离视野。
            // 注意：`generate_chunks_around_players` 仍挂在 schedule 上，之后的 update 会在
            // 新位置附近生成别的 chunk（预期行为），所以断言只认 chunk (0,0) 是否被清掉，
            // 不用整体 loaded 计数（否则会被"新位置又生成了别的 chunk"假阳性污染）。
            app.world_mut()
                .entity_mut(client)
                .insert(valence::prelude::ViewDistance::new(2));
            set_client_position(&mut app, client, [100_000.0, 64.0, 100_000.0]);
            for _ in 0..10 {
                app.update();
            }

            assert!(
                !loaded_contains(&app, DimensionKind::Tsy, origin),
                "F12: remove_unviewed_chunks must evict TSY chunks the same way it always did \
                 for overworld — this only exercises correctly if the system's per-dimension \
                 loop (and the per-dimension `loaded` bucket) actually reaches the TSY layer"
            );
            assert!(
                app.world()
                    .get::<ChunkLayer>(tsy_layer)
                    .expect("tsy layer entity should still carry a ChunkLayer component")
                    .chunk(origin)
                    .is_none(),
                "the evicted chunk must also be gone from the actual ChunkLayer, not just the \
                 bookkeeping set"
            );
        }

        #[test]
        fn recover_fall_through_rescues_tsy_player_in_the_void_when_provider_present() {
            let (mut app, client, _overworld, tsy_layer) = two_dimension_test_app();
            move_client_to_layer(&mut app, client, tsy_layer);
            // 深深低于 TSY floor(min_y=-64) - VOID_RESCUE_MARGIN(16)，且 TSY layer 未插入
            // 任何 chunk（脚下自然无碰撞）→ 纯函数层面必判 FallRecovery::Spawn。
            set_client_position(&mut app, client, [8.5, -500.0, 8.5]);
            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: Some(TerrainProvider::empty_for_tests()),
            });
            app.add_systems(Update, recover_fall_through);

            app.update();

            let rescued_y = app
                .world()
                .get::<Position>(client)
                .expect("client should still have Position after rescue")
                .get()
                .y;
            assert_ne!(
                rescued_y, -500.0,
                "F12: a TSY player fallen far below the TSY floor must be rescued once TSY has \
                 a provider — previously recover_fall_through never even looked at TSY clients"
            );
        }

        #[test]
        fn recover_fall_through_leaves_tsy_player_untouched_when_provider_absent() {
            let (mut app, client, _overworld, tsy_layer) = two_dimension_test_app();
            move_client_to_layer(&mut app, client, tsy_layer);
            set_client_position(&mut app, client, [8.5, -500.0, 8.5]);
            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: None,
            });
            app.add_systems(Update, recover_fall_through);

            app.update();

            let y_after = app
                .world()
                .get::<Position>(client)
                .expect("client should still have Position")
                .get()
                .y;
            assert_eq!(
                y_after, -500.0,
                "legacy behaviour must hold when TSY has no provider: no rescue, no panic"
            );
        }
    }
}
