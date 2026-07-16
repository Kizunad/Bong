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
    bevy_ecs, ident, Added, App, BiomeRegistry, BlockState, Chunk, ChunkLayer, ChunkPos, ChunkView,
    Client, Commands, Component, DimensionTypeRegistry, Entity, IntoSystemConfigs, Local, Position,
    Query, Res, ResMut, Resource, Server, UnloadedChunk, Update, Username, View, ViewDistance,
    VisibleChunkLayer, With, Without,
};
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::ChunkRenderDistanceCenterS2c;
use valence::protocol::VarInt;

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
        .add_systems(Update, recover_fall_through)
        // 修复"join 后世界全虚空"：valence 在 join tick 永远不发
        // ChunkRenderDistanceCenterS2c（OldPosition 被拍成当前 Position，diff 恒空），
        // 原版客户端 chunk 缓存中心停留在默认 (0,0)，出生点 (11,6) 附近的 chunk
        // 全部在接收瞬间被静默丢弃。本系统在 join 的下一 tick 补发 center 包并重灌
        // 已被客户端丢弃的 chunk。无需 ordering 约束：Update 相写入的包总是先于本
        // tick PostUpdate 的 chunk LOAD 数据进入发送缓冲。
        .add_systems(Update, resync_view_after_join);
}

/// 玩家穿过自己脚下方块（即客户端丢失了服务端仍持有的 chunk 碰撞）时低于地板多少
/// 才判定为"掉出世界"、传回 spawn。方块最低只能落在 `min_y`（基岩），低于它必为虚空。
const VOID_RESCUE_MARGIN: i32 = 16;

/// [`scan_real_surface_y`] 向上取的扫描窗口上界相对玩家当前 y 的偏移。玩家本身就
/// 陷在某个固体方块里（否则走不到 ResendAndBounce 分支），窗口只需要覆盖到"万一
/// 头顶还叠着别的固体（比如楼梯/半砖组合）"这种紧凑场景，8 格足够、也远小于会
/// 扫到无关高空结构的程度。
const FALL_RECOVERY_SCAN_UP: i32 = 8;

/// 在当前 `ChunkLayer` 里从 `from_y`（含）向下扫到 `to_y`（含），返回该列第一个
/// （最高的那个）"服务端仍持有真实碰撞箱"的方块 y —— 语义与
/// [`TerrainProvider::query_surface`](raster::TerrainProvider) 的 `y` 字段一致
/// （顶部实心方块自身的 y，不是 y+1）。整段窗口都没有固体方块时返回 `None`。
///
/// 为什么弹回目标不能只信 raster：raster 是烘焙时的荒野本底，不认后来叠加的
/// 结构物、玩家建筑、装饰物 stamp——它们都真实写进了 `ChunkLayer`。玩家踩在这些
/// 方块顶上触发穿地误判时，如果只按 raster 弹回，会被瞬移到"荒野本底"高度，和
/// 玩家实际站立处（比如二楼地板）毫不相干。因此优先扫描当前 chunk 的真实方块，
/// raster 只在这一列彻底没有方块（scan 窗口整段落在维度地板之外等边角情况）时
/// 才兜底。
fn scan_real_surface_y(
    layer: &ChunkLayer,
    bx: i32,
    bz: i32,
    from_y: i32,
    to_y: i32,
) -> Option<i32> {
    let mut y = from_y;
    while y >= to_y {
        let obstructed = layer
            .block([bx, y, bz])
            .is_some_and(|b| collision_top_y(b.state).is_some());
        if obstructed {
            return Some(y);
        }
        y -= 1;
    }
    None
}

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

/// [`feet_stuck_in_block`] 的浮点容差：玩家站在非完整方块顶面（下半砖 0.5、
/// 耕地/小径 0.9375、雪层 n/8 等）时，`p.y - p.y.floor()` 理论上应精确等于顶面
/// 高度，但移动/物理引擎的浮点累积误差可能让它矮上 1e-4 量级。容差太小会把
/// 完全合法的站立姿势判成"陷进方块"；容差太大则会漏判真正的穿地。
const FEET_SURFACE_EPSILON: f64 = 1e-3;

/// 该方块体素碰撞箱里最高的那个顶面（方块本地坐标 0.0..=1.0；多个碰撞箱——如
/// 楼梯、栅栏——取其中最大的 y）。方块没有碰撞箱（空气、水、大多数植物）时
/// 返回 `None`。
fn collision_top_y(state: BlockState) -> Option<f64> {
    state
        .collision_shapes()
        .map(|aabb| aabb.max().y)
        .fold(None, |top: Option<f64>, y| {
            Some(top.map_or(y, |t: f64| t.max(y)))
        })
}

/// 纯函数：玩家脚部是否真的"陷进"了方块的碰撞体内部，而不是稳稳站在它的顶面上。
///
/// # 为什么不能只看"该体素碰撞箱是否非空"
///
/// 下半砖（顶面 0.5）、耕地/小径（0.9375）、雪层（n/8）、楼梯低台等非完整方块的
/// 碰撞箱顶面低于 1.0——玩家正常站在它们上面时，`floor(player.y)` 落的正是这个
/// 方块本体（而不是它上方的空气格），脚部小数偏移 `fract_y` 恰好等于顶面高度。
/// 旧实现只判"该体素碰撞箱非空"，把这类完全合法的站立姿势也当成"穿地"，弹回
/// 目标又是与玩家实际站立处毫不相干的荒野本底（raster 烘焙值），表现为站在下
/// 半砖/耕地/雪地上莫名被瞬移（PR #343 遗留 bug，勿再退回"体素非空即穿地"）。
///
/// # 为什么不能只比"体素内最高顶面"
///
/// 楼梯这类方块在同一体素里有**多个**碰撞箱（矮台 y∈[0,0.5] 铺满一半 XZ +
/// 高台 y∈[0,1] 占另一半）。只取全体素最大顶面（1.0）会把站在矮台上
/// （fract_y=0.5、脚点 XZ 在矮台一侧）的玩家误判成陷进方块。必须按脚点的
/// `fract_x`/`fract_z` 选中**实际落脚的那个子碰撞箱**做点包含测试。
///
/// # 判定
///
/// 对脚点 `(fract_x, fract_y, fract_z)` 做逐碰撞箱的点包含测试：存在某个碰撞箱
/// 在 XZ 上罩住脚点、且 `fract_y` 落在 `[min_y, max_y - 容差)` 区间内（真正
/// 位于箱体内部）才算穿地。站在任何子箱顶面（fract_y == 该箱 max_y，含
/// [`FEET_SURFACE_EPSILON`] 浮点容差）都是合法站立。完整方块行为完全不变——
/// 玩家站在其上方的空气格里，`floor(player.y)` 根本不会指向这个方块；只有身体
/// 真的陷入方块内部才会命中。脚点在箱体 min_y 之下（如站在同体素上半砖下方的
/// 底面）也不算穿地——身体没有与该箱重叠的证据，宁可漏救不误弹。
fn feet_stuck_in_block(state: BlockState, fract_x: f64, fract_y: f64, fract_z: f64) -> bool {
    state.collision_shapes().any(|aabb| {
        let min = aabb.min();
        let max = aabb.max();
        fract_x >= min.x
            && fract_x < max.x
            && fract_z >= min.z
            && fract_z < max.z
            && fract_y >= min.y
            && fract_y < max.y - FEET_SURFACE_EPSILON
    })
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
            // 玩家脚点真的陷进该体素某个子碰撞箱内部（见 feet_stuck_in_block 文档）
            // = 客户端缺这块 chunk、正穿过它。站在下半砖/耕地/雪层顶面、楼梯矮台
            // 等非完整方块上时不会命中。
            let feet_obstructed = layer.block([bx, by, bz]).is_some_and(|b| {
                feet_stuck_in_block(
                    b.state,
                    p.x - p.x.floor(),
                    p.y - p.y.floor(),
                    p.z - p.z.floor(),
                )
            });
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
                    // 弹回目标优先取当前 ChunkLayer 里这一列"真实"的地表——它认得
                    // 结构物/玩家建筑/装饰物 stamp，而 raster 只是烘焙时的荒野本底。
                    // 只有这一列在 ChunkLayer 里彻底找不到固体方块（scan 窗口内，
                    // 通常发生在玩家已深陷 floor-margin 以下、窗口整段落在维度地板
                    // 之外）才退回 raster 兜底，保底不至于把人留在半空。
                    let scan_top =
                        (by + FALL_RECOVERY_SCAN_UP).min(floor_y + layer.height() as i32 - 1);
                    let surface = scan_real_surface_y(&layer, bx, bz, scan_top, floor_y)
                        .unwrap_or_else(|| terrain.query_surface(bx, bz).y);
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

/// 修复"join 后世界全虚空"——已连接但尚未补发视野中心包的 client 标记。
///
/// valence 的 `update_view_and_layers` 只在 `old_view.pos != view.pos` 时发
/// `ChunkRenderDistanceCenterS2c`；而 join tick `init_entities` 先把 `OldPosition`
/// 拍成当前 `Position`，该包在 join tick **永远不发**。原版客户端 chunk 缓存中心
/// 默认 (0,0)、保留半径=登录视距+3，收到范围外的 chunk 在接收瞬间被静默丢弃 →
/// 出生点在 chunk (11,6) 的本世界，首连/重连全部落进虚空，直到跨 chunk 传送
/// （如 /spawn）才偶然触发 center 补发。
///
/// arm（join tick 打标记）→ fire（下一 tick 补发）分两拍是刻意的：join tick 由
/// valence `initial_join` 发 GameJoin，play 包若写在 GameJoin 之前会让客户端 NPE，
/// 必须等下一 tick 再写。
#[derive(Debug, Clone, Copy, Component)]
struct JoinViewResyncPending;

/// 纯决策（便于饱和单测）：view 内哪些 chunk 位置"原版客户端已经丢弃、需要重灌"。
///
/// 客户端 chunk 缓存默认中心 (0,0)、保留半径约为登录视距+3（方形判定
/// `|x|<=r && |z|<=r`）。这里保守地按 `r = login_view_distance`（不加 3）判定：
/// 宁可把客户端其实还留着的外圈多重发一遍，也绝不能漏掉真被丢弃的 chunk——
/// 漏发即虚空，多发只是几个 chunk 的带宽。玩家 join 在原点附近时列表基本为空
/// （客户端没丢，不必白重发几 MB）；出生点 (11,6) 这种场景则基本全量。
///
/// 注意：`ChunkView::iter()` 覆盖到 `dist + EXTRA_VIEW_RADIUS(=2)` 的圆盘，比 r
/// 大一圈，所以即便 view 中心就在原点，超出 r 的外环仍会被列入（客户端其实留着
/// 半径 vd+3，重发它们是冗余但无害的保守行为）。
fn chunk_positions_needing_resend(view: ChunkView, login_view_distance: u8) -> Vec<ChunkPos> {
    let r = i32::from(login_view_distance);
    view.iter()
        .filter(|cp| !(cp.x.abs() <= r && cp.z.abs() <= r))
        .collect()
}

/// 修复"join 后世界全虚空"（根因见 [`JoinViewResyncPending`]）。
///
/// arm 相：对本 tick 新连接的 client 打 [`JoinViewResyncPending`] 标记（Commands
/// 在 tick 末应用，fire 自然落到下一 tick）。
///
/// fire 相：对每个 pending client
/// 1. 补发 `ChunkRenderDistanceCenterS2c`（客户端据此把 chunk 缓存中心从默认 (0,0)
///    挪到玩家真实所在 chunk）。**Update 相里写的包会先于本 tick PostUpdate 的
///    chunk LOAD 数据进入发送缓冲——center 必须先于 chunk 数据到达客户端，否则
///    重灌的 chunk 又会被丢弃，这个顺序是本修复正确性的关键。**
/// 2. 对 [`chunk_positions_needing_resend`] 给出的每个位置 `remove_chunk`+
///    `insert_chunk` 重新插入（与 [`recover_fall_through`] 的 ResendAndBounce 分支
///    同款手法），产生 LOAD layer-message，本 tick 的 `handle_layer_messages` 会把
///    整块重发给该客户端（此时 OldView 已正确，消息不会被过滤）。尚未生成的
///    chunk 不用管——它们之后生成时 center 已正确。
#[allow(clippy::type_complexity)]
fn resync_view_after_join(
    mut commands: Commands,
    new_clients: Query<Entity, (Added<Client>, Without<JoinViewResyncPending>)>,
    mut pending_clients: Query<
        (
            Entity,
            &mut Client,
            &Position,
            &ViewDistance,
            &VisibleChunkLayer,
        ),
        With<JoinViewResyncPending>,
    >,
    mut layers: Query<&mut ChunkLayer>,
) {
    // arm 相：join tick 只打标记，不写包（GameJoin 之前写 play 包会让客户端 NPE）。
    for entity in &new_clients {
        commands.entity(entity).insert(JoinViewResyncPending);
    }

    // fire 相：join 的下一 tick 补发 center + 重灌客户端已丢弃的 chunk。
    for (entity, mut client, position, view_distance, visible_chunk_layer) in &mut pending_clients {
        let view = ChunkView::new(ChunkPos::from(position.get()), view_distance.get());
        client.write_packet(&ChunkRenderDistanceCenterS2c {
            chunk_x: VarInt(view.pos.x),
            chunk_z: VarInt(view.pos.z),
        });

        // layer 拿不到（如维度 layer 尚未就绪）就只发 center 不重灌——center 正确后
        // 后续生成/插入的 chunk 都能正常被客户端接收，不会永久虚空。
        let mut resent = 0usize;
        if let Ok(mut layer) = layers.get_mut(visible_chunk_layer.0) {
            for cp in chunk_positions_needing_resend(view, view_distance.get()) {
                if let Some(chunk) = layer.remove_chunk(cp) {
                    layer.insert_chunk(cp, chunk);
                    resent += 1;
                }
            }
        }

        commands.entity(entity).remove::<JoinViewResyncPending>();
        tracing::debug!(
            "[bong][world] {:?} joined → re-sent chunk render distance center ({},{}) + re-inserted {} chunks the client had silently discarded",
            entity,
            view.pos.x,
            view.pos.z,
            resent
        );
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

    // collision_top_y / feet_stuck_in_block —— 穿地判定的核心纯函数。用真实
    // BlockState 而非手造数字，锁住"非完整方块顶面绝不能被误判成穿地"这条 PR
    // #343 遗留 bug 的回归钉子（旧实现只看"碰撞箱是否非空"，恒判穿地）。

    use valence::prelude::{PropName, PropValue};

    #[test]
    fn collision_top_y_matches_real_block_shapes() {
        assert_eq!(
            collision_top_y(BlockState::STONE),
            Some(1.0),
            "完整方块的碰撞箱顶面必须是满格 1.0"
        );
        assert_eq!(
            collision_top_y(BlockState::AIR),
            None,
            "空气没有碰撞箱，必须是 None（不能编出一个假顶面，否则 feet_stuck_in_block \
             会把空中/水中的合法玩家判成陷进方块）"
        );
        assert_eq!(
            collision_top_y(BlockState::OAK_SLAB.set(PropName::Type, PropValue::Bottom)),
            Some(0.5),
            "下半砖顶面在 0.5，不是满格 1.0——这正是旧实现会把它误判成穿地的地方"
        );
        assert_eq!(
            collision_top_y(BlockState::OAK_SLAB.set(PropName::Type, PropValue::Top)),
            Some(1.0),
            "上半砖的碰撞箱只是把 min_y 抬高到 0.5，顶面本来就贴着满格 1.0"
        );
        assert_eq!(
            collision_top_y(BlockState::FARMLAND),
            Some(0.9375),
            "耕地顶面是 15/16=0.9375，玩家站在耕地上脚部小数偏移应为 0.9375 而非 1.0"
        );
        assert_eq!(
            collision_top_y(BlockState::SNOW.set(PropName::Layers, PropValue::_1)),
            None,
            "单层雪没有碰撞箱（原版碰撞高度=(层数-1)/8，1 层=0，玩家陷进视觉层踩在\
             下方方块上）——必须是 None，否则会把站在雪地上的玩家当成陷进方块"
        );
        assert_eq!(
            collision_top_y(BlockState::SNOW.set(PropName::Layers, PropValue::_2)),
            Some(0.125),
            "2 层雪的碰撞箱顶面是 (2-1)/8=0.125——玩家站在 2 层雪上脚部小数偏移 0.125，\
             不能被误判成穿地"
        );
    }

    #[test]
    fn feet_stuck_in_block_distinguishes_standing_from_embedded() {
        // 完整方块：真的陷进方块内部——触发；站在其上方时 floor(y) 指向空气格，
        // 见下一条空气用例。
        assert!(
            feet_stuck_in_block(BlockState::STONE, 0.5, 0.3, 0.5),
            "身体陷进完整方块内部（fract_y=0.3 位于石头碰撞箱 [0,1) 内）必须判穿地"
        );
        assert!(
            !feet_stuck_in_block(BlockState::AIR, 0.5, 0.0, 0.5),
            "空气（无碰撞箱）任何脚点都不应判穿地——站完整方块顶面时 floor(y) 指向的\
             正是这个空气格"
        );
        assert!(
            !feet_stuck_in_block(BlockState::WATER, 0.5, 0.5, 0.5),
            "水（无碰撞箱）里游泳不应判穿地"
        );

        // 下半砖：顶面站立（回归钉子——旧实现在这里恒误判）不触发；陷入内部触发。
        let bottom_slab = BlockState::OAK_SLAB.set(PropName::Type, PropValue::Bottom);
        assert!(
            !feet_stuck_in_block(bottom_slab, 0.5, 0.5, 0.5),
            "回归钉子：站在下半砖顶面（fract_y==顶面 0.5）绝不能判穿地，否则玩家会被\
             反复弹去荒野本底"
        );
        assert!(
            feet_stuck_in_block(bottom_slab, 0.5, 0.2, 0.5),
            "fract_y=0.2 低于下半砖顶面 0.5，身体确实陷进砖里，应判穿地"
        );

        // 上半砖：脚点贴其底面之下（站在同体素下半空间）不触发——身体没有与
        // 箱体重叠的证据，宁可漏救不误弹；陷进箱体内部触发。
        let top_slab = BlockState::OAK_SLAB.set(PropName::Type, PropValue::Top);
        assert!(
            !feet_stuck_in_block(top_slab, 0.5, 0.2, 0.5),
            "脚点在上半砖箱体（y∈[0.5,1]）之下不应判穿地（fract_y < 箱 min_y）"
        );
        assert!(
            feet_stuck_in_block(top_slab, 0.5, 0.7, 0.5),
            "fract_y=0.7 位于上半砖箱体内部，应判穿地"
        );

        // 耕地/小径 0.9375 顶面：站立不触发，陷入内部触发。
        assert!(
            !feet_stuck_in_block(BlockState::FARMLAND, 0.5, 0.9375, 0.5),
            "站在耕地顶面（0.9375）不应判穿地"
        );
        assert!(
            feet_stuck_in_block(BlockState::FARMLAND, 0.5, 0.5, 0.5),
            "fract_y=0.5 陷进耕地方块本体内部，应判穿地"
        );

        // 雪层：1 层无碰撞箱不触发；2 层顶面 0.125 站立不触发。
        assert!(
            !feet_stuck_in_block(
                BlockState::SNOW.set(PropName::Layers, PropValue::_1),
                0.5,
                0.0,
                0.5
            ),
            "1 层雪无碰撞箱，站在其中（脚陷视觉层踩下方方块）不应判穿地"
        );
        assert!(
            !feet_stuck_in_block(
                BlockState::SNOW.set(PropName::Layers, PropValue::_2),
                0.5,
                0.125,
                0.5
            ),
            "站在 2 层雪碰撞顶面（0.125）不应判穿地"
        );

        // 容差边界：恰好等于 top - epsilon 视为"仍站在顶面"（用 < 而非 <=，与
        // decide_fall_recovery 的边界哲学一致）；再往下哪怕一点点就必须判穿地。
        assert!(
            !feet_stuck_in_block(BlockState::STONE, 0.5, 1.0 - FEET_SURFACE_EPSILON, 0.5),
            "fract_y 恰好等于 top-epsilon 的边界不应判穿地（< 而非 <=）"
        );
        assert!(
            feet_stuck_in_block(
                BlockState::STONE,
                0.5,
                1.0 - FEET_SURFACE_EPSILON - 1e-6,
                0.5
            ),
            "fract_y 只要低于 top-epsilon 哪怕一点点，就必须判穿地"
        );
    }

    /// CR #906 回归钉子：楼梯在同一体素里有多个碰撞箱（矮台半格 + 高台整格），
    /// 判定若只取全体素最大顶面（1.0），站矮台（fract_y=0.5）会被误判穿地——
    /// 必须按脚点 XZ 选中实际落脚的子碰撞箱。
    #[test]
    fn feet_stuck_in_block_respects_stairs_sub_shapes() {
        // 默认朝向的橡木楼梯。先从真实碰撞箱里找出矮台（max_y==0.5）与高台
        // （max_y==1.0）的 XZ 中心，避免对朝向属性做脆弱假设。
        let stairs = BlockState::OAK_STAIRS;
        let mut low_center: Option<(f64, f64)> = None;
        let mut high_center: Option<(f64, f64)> = None;
        for aabb in stairs.collision_shapes() {
            let (min, max) = (aabb.min(), aabb.max());
            let center = ((min.x + max.x) / 2.0, (min.z + max.z) / 2.0);
            if (max.y - 0.5).abs() < 1e-9 {
                low_center = Some(center);
            } else if (max.y - 1.0).abs() < 1e-9 {
                high_center = Some(center);
            }
        }
        let (lx, lz) = low_center.expect("楼梯必须有 max_y=0.5 的矮台碰撞箱");
        let (hx, hz) = high_center.expect("楼梯必须有 max_y=1.0 的高台碰撞箱");

        assert!(
            !feet_stuck_in_block(stairs, lx, 0.5, lz),
            "回归钉子（CR #906）：站在楼梯矮台顶面（fract_y=0.5、脚点在矮台 XZ 内）\
             绝不能因高台箱 max_y=1.0 被误判穿地"
        );
        assert!(
            feet_stuck_in_block(stairs, hx, 0.5, hz),
            "脚点在高台 XZ 内且 fract_y=0.5 位于高台箱内部，是真穿地，应触发"
        );
        assert!(
            !feet_stuck_in_block(stairs, hx, 1.0 - FEET_SURFACE_EPSILON, hz),
            "站在楼梯高台顶面（fract_y≈1.0）不应判穿地"
        );
        assert!(
            feet_stuck_in_block(stairs, lx, 0.2, lz),
            "脚点陷进矮台箱体内部（fract_y=0.2 < 0.5）应判穿地"
        );
    }

    #[test]
    fn overworld_height_matches_valence_heightmap_encoding_budget() {
        const VALENCE_HEIGHTMAP_BITS_PER_ENTRY: u32 = 9;
        const COLUMN_COUNT: u32 = 16 * 16;

        let entries_per_long = i64::BITS / VALENCE_HEIGHTMAP_BITS_PER_ENTRY;
        let expected_longs = COLUMN_COUNT.div_ceil(entries_per_long);

        assert_eq!(WORLD_HEIGHT % 16, 0);
        assert!(heightmap_bits_for_dimension(WORLD_HEIGHT) <= VALENCE_HEIGHTMAP_BITS_PER_ENTRY);
        assert_eq!(expected_longs, 37);
    }

    fn heightmap_bits_for_dimension(height: u32) -> u32 {
        u32::BITS - height.leading_zeros()
    }

    // join 视野中心补发 —— chunk_positions_needing_resend 纯函数饱和测试 +
    // resync_view_after_join 系统级测试（marker 生命周期 + center 包 + chunk 重灌）。
    mod join_view_resync {
        use super::*;
        use std::collections::BTreeSet;
        use valence::prelude::OldPosition;
        use valence::protocol::Packet;
        use valence::testing::ScenarioSingleClient;

        /// 方形保留判定的参考实现，测试独立推导（不复用被测函数内部逻辑）。
        fn retained_by_client(cp: ChunkPos, r: i32) -> bool {
            cp.x.abs() <= r && cp.z.abs() <= r
        }

        #[test]
        fn far_from_origin_view_needs_full_resend() {
            // 出生点 chunk (11,6)、vd=4：view 圆盘（半径 vd+2=6）内没有任何位置落在
            // 客户端默认保留方形 |x|<=4 && |z|<=4 里（距中心最近的 (5,6) 也有 |z|=6>4），
            // 客户端把收到的 chunk 全丢了 → 必须全量重灌。
            let view = ChunkView::new(ChunkPos::new(11, 6), 4);
            let all: BTreeSet<ChunkPos> = view.iter().collect();
            let resend: BTreeSet<ChunkPos> = chunk_positions_needing_resend(view, 4)
                .into_iter()
                .collect();
            assert_eq!(
                resend,
                all,
                "期望出生点 (11,6) vd=4 的 view 全部 {} 个位置都要重灌，因为它们全在客户端\
                 默认保留方形 r=4 之外（接收瞬间已被丢弃）；实际重灌集合与 view 全集不一致",
                all.len()
            );
        }

        #[test]
        fn origin_join_keeps_client_retained_square() {
            // 原点 join：客户端保留方形 r=4 覆盖 view 的主体，这部分绝不能重发。
            // 注意 `ChunkView::iter()` 覆盖到 dist+EXTRA_VIEW_RADIUS(=2) 的圆盘，
            // 超出 r=4 的外环（max(|x|,|z|) ∈ 5..=6）仍会被保守地列入——客户端其实
            // 留着半径 vd+3，重发外环是冗余但无害（宁可多发不可漏发）。
            let view = ChunkView::new(ChunkPos::new(0, 0), 4);
            let resend: BTreeSet<ChunkPos> = chunk_positions_needing_resend(view, 4)
                .into_iter()
                .collect();
            for cp in view.iter() {
                if retained_by_client(cp, 4) {
                    assert!(
                        !resend.contains(&cp),
                        "期望 {cp:?} 不被重发，因为它在客户端保留方形 r=4 内（客户端没丢，\
                         重发纯属浪费带宽）；实际它出现在重灌列表里"
                    );
                }
            }
            for cp in &resend {
                let ring = cp.x.abs().max(cp.z.abs());
                assert!(
                    (5..=6).contains(&ring),
                    "期望原点 join 被列入重灌的只可能是 EXTRA_VIEW_RADIUS 外环\
                     （max(|x|,|z|) ∈ 5..=6），因为方形 r=4 内全部保留、view 圆盘半径只有 6；\
                     实际出现了 {cp:?}（ring={ring}）"
                );
            }
        }

        #[test]
        fn boundary_off_by_one_at_retained_square_edge() {
            // 跨界 case：view 中心 (5,0)、vd=4，r=4 → 恰好 |x|<=4 && |z|<=4 的保留、
            // 出界一格的重发。逐一验证 off-by-one。
            let view = ChunkView::new(ChunkPos::new(5, 0), 4);
            let resend: BTreeSet<ChunkPos> = chunk_positions_needing_resend(view, 4)
                .into_iter()
                .collect();

            // x=4 保留（|4|<=4 且 |0|<=4）
            assert!(
                !resend.contains(&ChunkPos::new(4, 0)),
                "期望 (4,0) 保留，因为 |4|<=r=4 客户端没丢；实际它被列入重灌"
            );
            // x=5 重发（|5|>4）
            assert!(
                resend.contains(&ChunkPos::new(5, 0)),
                "期望 (5,0) 重发，因为 |5|>r=4 客户端接收瞬间已丢弃；实际它不在重灌列表"
            );
            // z 轴同样的 off-by-one：(4,4) 保留、(4,5) 重发（两者都在 view 圆盘内）。
            assert!(view.contains(ChunkPos::new(4, 4)) && view.contains(ChunkPos::new(4, 5)));
            assert!(
                !resend.contains(&ChunkPos::new(4, 4)),
                "期望 (4,4) 保留，因为 |x|、|z| 均 <=4；实际它被列入重灌"
            );
            assert!(
                resend.contains(&ChunkPos::new(4, 5)),
                "期望 (4,5) 重发，因为 |z|=5>4（方形判定是 && 的取反，任一轴出界即丢）；\
                 实际它不在重灌列表"
            );

            // 全集逐一核验：重灌列表 == view 内所有出方形的位置，无遗漏无多余。
            for cp in view.iter() {
                assert_eq!(
                    resend.contains(&cp),
                    !retained_by_client(cp, 4),
                    "期望 {cp:?} 的重灌判定与「不在保留方形 r=4 内」严格一致，\
                     实际两者相反",
                );
            }
        }

        #[test]
        fn vd_zero_extreme_no_panic_and_correct() {
            // vd=0（ChunkView 允许 0，ViewDistance 组件才 clamp 到 2）：保留方形只剩
            // (0,0) 一格，view 圆盘（半径 0+2=2）里其余全部重发。
            let view = ChunkView::new(ChunkPos::new(0, 0), 0);
            let resend: BTreeSet<ChunkPos> = chunk_positions_needing_resend(view, 0)
                .into_iter()
                .collect();
            assert!(
                !resend.contains(&ChunkPos::new(0, 0)),
                "期望 vd=0 时 (0,0) 保留，因为 r=0 的方形恰含原点一格；实际它被列入重灌"
            );
            let expected: BTreeSet<ChunkPos> = view
                .iter()
                .filter(|cp| !retained_by_client(*cp, 0))
                .collect();
            assert_eq!(
                resend, expected,
                "期望 vd=0 时重灌集合 == view 内除 (0,0) 外的全部位置（r=0 只保原点），\
                 实际集合不一致"
            );
        }

        #[test]
        fn vd_max_extreme_no_panic_and_correct() {
            // vd=32（MAX_VIEW_DIST）：不 panic，且 off-by-one 语义在极端值下仍然成立。
            let view = ChunkView::new(ChunkPos::new(0, 0), 32);
            let resend: BTreeSet<ChunkPos> = chunk_positions_needing_resend(view, 32)
                .into_iter()
                .collect();
            assert!(
                !resend.contains(&ChunkPos::new(32, 0)),
                "期望 (32,0) 保留，因为 |32|<=r=32；实际它被列入重灌"
            );
            assert!(
                view.contains(ChunkPos::new(33, 0)),
                "前置：(33,0) 应在 vd=32 的 view 圆盘（半径 34）内，否则下一条断言无意义"
            );
            assert!(
                resend.contains(&ChunkPos::new(33, 0)),
                "期望 (33,0) 重发，因为 |33|>r=32（EXTRA_VIEW_RADIUS 外环）；\
                 实际它不在重灌列表"
            );
            for cp in view.iter() {
                assert_eq!(
                    resend.contains(&cp),
                    !retained_by_client(cp, 32),
                    "期望 {cp:?} 在 vd=32 极端值下的重灌判定与保留方形 r=32 严格互补，\
                     实际两者相反",
                );
            }
        }

        /// 把所有 mock client 的发送缓冲刷进连接，确保 `collect_received` 能拿到
        /// 本 tick 写入的包（valence 的 flush 在 PostUpdate，这里显式再刷一次防抖）。
        fn flush_all_client_packets(app: &mut App) {
            let world = app.world_mut();
            let mut query = world.query::<&mut Client>();
            for mut client in query.iter_mut(world) {
                client
                    .flush_packets()
                    .expect("mock client packets should flush");
            }
        }

        /// 系统级：join → 第 1 tick 只 arm 不 fire；第 2 tick fire（center 包 + chunk
        /// 重灌 + marker 移除）；第 3 tick 不再重复 fire。
        ///
        /// Position 与 OldPosition 一起写死为出生点，精确复现 join tick
        /// `old_view.pos == view.pos` → valence 自己永远不发 center 包的前置条件，
        /// 因此收到的所有 ChunkRenderDistanceCenterS2c 都必然来自被测系统。
        #[test]
        fn join_resync_fires_exactly_once_on_second_tick() {
            let scenario = ScenarioSingleClient::new();
            let mut app = scenario.app;
            let client = scenario.client;
            let layer = scenario.layer;
            let mut helper = scenario.helper;

            app.add_systems(Update, resync_view_after_join);

            // 出生点 chunk (11,6)（block x=190,z=111）——真实 bug 的复现坐标。
            app.world_mut().entity_mut(client).insert((
                Position::new([190.0, 64.0, 111.0]),
                OldPosition::new([190.0, 64.0, 111.0]),
                ViewDistance::new(4),
            ));
            // 服务端已持有 view 中心 chunk（客户端因 center 停在 (0,0) 而丢弃了它）。
            app.world_mut()
                .get_mut::<ChunkLayer>(layer)
                .expect("scenario layer should carry a ChunkLayer")
                .insert_chunk(ChunkPos::new(11, 6), UnloadedChunk::new());

            let count_center_packets = |helper: &mut valence::testing::MockClientHelper| {
                helper
                    .collect_received()
                    .0
                    .iter()
                    .filter(|frame| frame.id == ChunkRenderDistanceCenterS2c::ID)
                    .map(|frame| {
                        frame
                            .decode::<ChunkRenderDistanceCenterS2c>()
                            .expect("center packet frame should decode")
                    })
                    .map(|packet| (packet.chunk_x.0, packet.chunk_z.0))
                    .collect::<Vec<_>>()
            };

            // tick 1：arm。marker 在 tick 末挂上，但绝不能在 join tick 写 play 包
            // （GameJoin 之前的 play 包会让客户端 NPE）。
            app.update();
            flush_all_client_packets(&mut app);
            let centers_tick1 = count_center_packets(&mut helper);
            assert!(
                centers_tick1.is_empty(),
                "期望 join tick（第 1 次 update）不发 center 包，因为 fire 必须等到\
                 GameJoin 之后的下一 tick；实际收到 {centers_tick1:?}"
            );

            // tick 2：fire。center 包坐标必须是玩家真实所在 chunk (11,6)。
            app.update();
            flush_all_client_packets(&mut app);
            let centers_tick2 = count_center_packets(&mut helper);
            assert_eq!(
                centers_tick2,
                vec![(11, 6)],
                "期望第 2 次 update 恰好补发一个 center 包且坐标为玩家所在 chunk (11,6)\
                 （客户端据此把缓存中心从默认 (0,0) 挪过来）；实际收到 {centers_tick2:?}"
            );
            // 重灌走 remove+insert，chunk 必须仍留在 layer 里（丢了就等于把地形删了）。
            // 包内容（chunk 数据先后于 center 的字节序）由协议探针实测覆盖，这里只
            // 断言服务端侧可观察的契约：chunk 未丢失 + center 包「不发→恰发一次→
            // 不复发」的三拍时序（内部 marker 的挂/摘属实现细节，不直接断言）。
            assert!(
                app.world()
                    .get::<ChunkLayer>(layer)
                    .expect("scenario layer should still carry a ChunkLayer")
                    .chunk(ChunkPos::new(11, 6))
                    .is_some(),
                "期望重灌（remove+insert）后 chunk (11,6) 仍在 layer 中，因为重灌只是为了\
                 触发 LOAD layer-message 而非真正卸载；实际该 chunk 从 layer 消失了"
            );

            // tick 3：marker 已摘，绝不能重复 fire（否则每 tick 白重发整个 view）。
            app.update();
            flush_all_client_packets(&mut app);
            let centers_tick3 = count_center_packets(&mut helper);
            assert!(
                centers_tick3.is_empty(),
                "期望第 3 次 update 不再发 center 包，因为 marker 已在 fire 时移除、\
                 Added<Client> 也早已消费；实际收到 {centers_tick3:?}"
            );
        }
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

    // scan_real_surface_y —— ResendAndBounce 弹回目标必须认得当前 ChunkLayer 里
    // 真实存在的方块（结构物/玩家建筑/装饰物 stamp），而不是只信 raster 烘焙的
    // 荒野本底。有真实方块列弹真实顶面；scan 窗口内彻底没有固体方块才退 raster。
    mod bounce_target_scan {
        use super::*;
        use valence::testing::ScenarioSingleClient;

        #[test]
        fn finds_real_block_top_within_window() {
            let scenario = ScenarioSingleClient::new();
            let mut app = scenario.app;
            let layer_entity = scenario.layer;

            let min_y = app
                .world()
                .get::<ChunkLayer>(layer_entity)
                .expect("scenario layer should carry a ChunkLayer")
                .min_y();
            let stone_y = min_y + 50;

            let mut chunk = UnloadedChunk::with_height(384);
            chunk.set_block_state(8, (stone_y - min_y) as u32, 8, BlockState::STONE);
            app.world_mut()
                .get_mut::<ChunkLayer>(layer_entity)
                .expect("scenario layer should carry a ChunkLayer")
                .insert_chunk(ChunkPos::new(0, 0), chunk);

            let layer = app
                .world()
                .get::<ChunkLayer>(layer_entity)
                .expect("layer must exist after insert_chunk");

            let found = scan_real_surface_y(layer, 8, 8, stone_y + FALL_RECOVERY_SCAN_UP, min_y);
            assert_eq!(
                found,
                Some(stone_y),
                "扫描窗口内唯一的固体方块在 y={stone_y}，必须精确命中它本身（而不是随便一个\
                 更高/更低的 y，也不是 None 落到 raster 兜底）"
            );
        }

        #[test]
        fn returns_none_when_column_has_no_solid_block_in_window() {
            let scenario = ScenarioSingleClient::new();
            let mut app = scenario.app;
            let layer_entity = scenario.layer;

            let min_y = app
                .world()
                .get::<ChunkLayer>(layer_entity)
                .expect("scenario layer should carry a ChunkLayer")
                .min_y();

            // chunk 存在但整列都是默认 AIR——扫描窗口内彻底没有固体方块。
            let chunk = UnloadedChunk::with_height(384);
            app.world_mut()
                .get_mut::<ChunkLayer>(layer_entity)
                .expect("scenario layer should carry a ChunkLayer")
                .insert_chunk(ChunkPos::new(0, 0), chunk);

            let layer = app
                .world()
                .get::<ChunkLayer>(layer_entity)
                .expect("layer must exist after insert_chunk");

            let found = scan_real_surface_y(layer, 8, 8, min_y + 50 + FALL_RECOVERY_SCAN_UP, min_y);
            assert_eq!(
                found, None,
                "整列都是空气时必须返回 None，交给调用方回退到 raster query_surface 兜底，\
                 而不是编出一个假的 y"
            );
        }

        /// 系统级回归钉子：玩家所在列真的有一块服务端固体方块（比如二楼地板/结构
        /// 物），和 raster 烘焙的荒野本底完全不是一回事——弹回逻辑如果退化回只信
        /// raster（旧实现），会把玩家瞬移到与实际站立处毫不相干的高度。
        #[test]
        fn recover_fall_through_bounces_to_real_column_surface_not_raster_wilderness() {
            let scenario = ScenarioSingleClient::new();
            let mut app = scenario.app;
            let client = scenario.client;
            let layer_entity = scenario.layer;

            let min_y = app
                .world()
                .get::<ChunkLayer>(layer_entity)
                .expect("scenario layer should carry a ChunkLayer")
                .min_y();
            let stone_y = min_y + 50;

            let mut chunk = UnloadedChunk::with_height(384);
            chunk.set_block_state(8, (stone_y - min_y) as u32, 8, BlockState::STONE);
            app.world_mut()
                .get_mut::<ChunkLayer>(layer_entity)
                .expect("scenario layer should carry a ChunkLayer")
                .insert_chunk(ChunkPos::new(0, 0), chunk);

            // 玩家陷进这块石头内部（fract_y=0.3）——触发 ResendAndBounce。
            app.world_mut().entity_mut(client).insert(Position::new([
                8.5,
                f64::from(stone_y) + 0.3,
                8.5,
            ]));

            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: None,
            });
            app.insert_resource(DimensionLayers {
                overworld: layer_entity,
                tsy: layer_entity,
            });
            app.add_systems(Update, recover_fall_through);

            app.update();

            let rescued_y = app
                .world()
                .get::<Position>(client)
                .expect("client should still have Position")
                .get()
                .y;

            assert_eq!(
                rescued_y,
                f64::from(stone_y + 2),
                "必须弹到真实结构顶面(y={stone_y})+2，而不是 raster 荒野本底——弹回目标必须\
                 优先认当前 ChunkLayer 里的真实方块，而不是只信烘焙时的荒野高度"
            );
        }
    }
}
