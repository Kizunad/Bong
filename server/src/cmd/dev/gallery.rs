//! plan-worldgen-v4 P5 — 画廊审阅闭环 dev 命令（`/gallery` + `/structure save`）。
//!
//! `/gallery` 遍历 `server/structures/**/*.nbt`，用 [`nbt_io::read_structure_nbt`] 读出，
//! 按网格 stamp 到画廊区（当前已加载 Overworld `ChunkLayer`），每格上方挂一块写有资产名
//! 的悬浮名牌（sign block entity，参 `preview::decorations`），并把每格的包围盒记进
//! [`GalleryState`]。
//!
//! `/structure save <name>` 反向：按 `name` 查 [`GalleryState`] 里的 slot，读取该格包围盒内
//! 的 `BlockState`，经 [`nbt_io::write_structure_nbt`] 回序列化覆盖原 `.nbt`（与
//! `scripts/nbt/nbt_builder.py` round-trip 对拍）。
//!
//! **dev-only 红线**：本命令挂 dev 命令树，stamp 直写 ChunkLayer、save 覆盖资产文件，
//! 显式绕过生产 gameplay 路径，不进自然世界生成链。

use std::path::{Path, PathBuf};

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::parsers::CommandArg;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::nbt::{compound, Compound, List};
use valence::prelude::{
    bevy_ecs, App, Block, BlockPos, BlockState, ChunkLayer, Client, EventReader, IntoSystemConfigs,
    PropName, PropValue, Query, Res, ResMut, Resource, Update,
};
use valence::text::IntoText;

use crate::world::dimension::{DimensionKind, DimensionLayers};
use crate::world::terrain::nbt_io::{self, PaletteEntry, StructureBlockEntry, StructureNbt};
use crate::world::terrain::nbt_registry::{DecorationAnchor, DecorationNbtRegistry, Rotation};

/// `server/structures` 相对 `CARGO_MANIFEST_DIR`（= `server/`）的目录名。
const STRUCTURES_DIR: &str = "structures";

/// 画廊网格布局参数。每格预留 `cell_size` 边长 + `gap` 间隔，沿 X 排 `columns` 列后换行
/// （+Z 方向）。base 是第一格的 min 角（世界坐标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GalleryGrid {
    pub base: [i32; 3],
    pub cell_size: i32,
    pub gap: i32,
    pub columns: i32,
}

impl Default for GalleryGrid {
    fn default() -> Self {
        // base 选在出生平台附近上空一格高度，cell 32 容纳现有最大结构，列宽 4。
        GalleryGrid {
            base: [0, 70, 0],
            cell_size: 32,
            gap: 4,
            columns: 4,
        }
    }
}

impl GalleryGrid {
    /// 第 `index` 个结构格的 origin（min 角，世界坐标）。纯函数，不碰 ECS。
    pub fn slot_origin(&self, index: usize) -> [i32; 3] {
        let stride = self.cell_size + self.gap;
        let col = (index as i32) % self.columns;
        let row = (index as i32) / self.columns;
        [
            self.base[0] + col * stride,
            self.base[1],
            self.base[2] + row * stride,
        ]
    }
}

/// 装饰审阅台网格布局参数。比大型 [`GalleryGrid`] 紧凑得多：每个装饰变体一格，最大
/// footprint 是 bush 7×7，故 `cell_size` 取 12（台基 7×7 + 名牌 + 留白），列宽 8 让 54
/// 个变体排成近正方（8×7）。base 选出生点正上空高处，四周纯空气=虚空审阅感。
///
/// 与 [`GalleryGrid`] 共用 `slot_origin` 的「列优先、+X 排列满列换 +Z 行」语义，但参数
/// 独立——刻意不复用同一 struct，避免改这里波及 `/gallery` 无参的字节级布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecorationGalleryGrid {
    pub base: [i32; 3],
    pub cell_size: i32,
    pub gap: i32,
    pub columns: i32,
}

impl Default for DecorationGalleryGrid {
    fn default() -> Self {
        // 出生点上空 y≈140：spawn chunk 已加载，四周天空=虚空。cell 12（台基 7 + 5 留白），
        // 8 列让 54 格排成 8×7 近正方。
        DecorationGalleryGrid {
            base: [0, 140, 0],
            cell_size: 12,
            gap: 0,
            columns: 8,
        }
    }
}

impl DecorationGalleryGrid {
    /// 第 `index` 个装饰格的 origin（台基西北角 min，世界坐标）。纯函数，不碰 ECS。
    /// 与 [`GalleryGrid::slot_origin`] 同构（列优先 +X，满列换行 +Z），仅参数不同。
    pub fn slot_origin(&self, index: usize) -> [i32; 3] {
        let stride = self.cell_size + self.gap;
        let col = (index as i32) % self.columns;
        let row = (index as i32) / self.columns;
        [
            self.base[0] + col * stride,
            self.base[1],
            self.base[2] + row * stride,
        ]
    }
}

/// 台基边长（x/z）：7 容纳最大装饰 footprint（bush 7×7）且与 [`DecorationGalleryGrid`]
/// 的 `cell_size=12` 留出名牌 + 间隔空间。
const PEDESTAL_SIZE: i32 = 7;
/// 台基用方块：抛光安山岩，平整、与天空对比明显，便于在虚空里看清落脚点。
const PEDESTAL_BLOCK: BlockState = BlockState::POLISHED_ANDESITE;
/// Hanging 装饰需要的天花板条用方块（与台基同料，悬挂件从其底面向下生长）。
const CEILING_BLOCK: BlockState = BlockState::POLISHED_ANDESITE;
/// Hanging 天花板距台基顶面的高度：留足装饰垂下的纵向空间。
const CEILING_CLEARANCE: i32 = 10;

/// 推断某装饰 template 的 [`DecorationAnchor`]——审阅台据此把每件摆正。
///
/// template_id 形如 `decorations/<kind_dir>/<variant>.nbt`；anchor 由 `<kind_dir>` 决定，
/// **镜像** worldgen `profiles/base.py` 的 `_KIND_NBT_ANCHOR`（`grave_mound→embedded`）+
/// `_NAME_NBT_OVERRIDE`（`tian_mai_crystal→hanging_crystal/hanging`）这两处正典：
/// * `grave/*` → [`DecorationAnchor::Embedded`]（grave mound dome 下沉一格）。
/// * `hanging_crystal/*` → [`DecorationAnchor::Hanging`]（悬挂件从天花板底面向下生长）。
/// * 其余全部 → [`DecorationAnchor::Ground`]（站在台基顶面上）。
///
/// 注意目录名 ≠ kind 名（kind `grave_mound` 对应目录 `grave`，override 把 crystal 路由到
/// `hanging_crystal/` 目录），所以这里按**目录段**判定才与落盘资产对齐。
pub fn anchor_for_template(template_id: &str) -> DecorationAnchor {
    let kind_dir = template_id
        .strip_prefix("decorations/")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("");
    match kind_dir {
        "grave" => DecorationAnchor::Embedded,
        "hanging_crystal" => DecorationAnchor::Hanging,
        _ => DecorationAnchor::Ground,
    }
}

/// 一个画廊格记录：资产相对路径名 + origin + size（来自结构 NBT 的 `size`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GallerySlot {
    /// 资产相对 `server/structures` 的路径（如 `dan_zong/master_sarcophagus.nbt`）——
    /// 同时作为 `/structure save <name>` 的 key 与名牌文字。
    pub name: String,
    /// 资产 `.nbt` 文件绝对路径（save 时覆盖写）。
    pub nbt_path: PathBuf,
    /// 该格 stamp 的 origin（min 角，世界坐标）。
    pub origin: [i32; 3],
    /// 结构尺寸 `[x, y, z]`（来自 NBT `size`），决定包围盒。
    pub size: [i32; 3],
}

/// 画廊运行时状态：已 stamp 的格子。`/structure save` 从此查 slot。
#[derive(Resource, Debug, Default, Clone)]
pub struct GalleryState {
    pub slots: Vec<GallerySlot>,
}

impl GalleryState {
    pub fn find(&self, name: &str) -> Option<&GallerySlot> {
        self.slots.iter().find(|s| s.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GalleryCmd {
    /// `/gallery` — stamp 全部大型 authored 资产到画廊网格（排除 `decorations/`）。
    Stamp,
    /// `/gallery decorations` — 把全部 54 个 P6 NBT 装饰变体铺到出生点上空的「虚空审阅台」。
    Decorations,
    /// `/structure save <name>` — 回序列化 `name` 格覆盖原 `.nbt`。
    Save { name: String },
}

impl Command for GalleryCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        // `/gallery`（无参）保持 Stamp 不变；`/gallery decorations` 是装饰专属审阅路。
        let gallery = graph
            .root()
            .literal("gallery")
            .with_executable(|_| GalleryCmd::Stamp)
            .id();
        graph
            .at(gallery)
            .literal("decorations")
            .with_executable(|_| GalleryCmd::Decorations);

        let structure = graph.root().literal("structure").id();
        graph
            .at(structure)
            .literal("save")
            .argument("name")
            .with_parser::<String>()
            .with_executable(|input| GalleryCmd::Save {
                name: String::parse_arg(input).unwrap(),
            });
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<GalleryState>()
        .add_command::<GalleryCmd>()
        .add_systems(
            Update,
            handle_gallery_cmd
                .after(crate::network::client_request_handler::handle_client_request_payloads),
        );
}

/// The `server/structures/` subdir of authored decoration templates. The gallery
/// review grid is for the large authored compounds (dan_zong / wangyintai); the
/// small worldgen-v4 P6 decoration variants under this subdir have their own
/// review path (the `scripts/nbt/render_structure.py` PNG renders) and their own
/// runtime registry ([`crate::world::terrain::nbt_registry::DecorationNbtRegistry`]),
/// so they are excluded from the gallery sweep to keep the grid uncluttered.
const GALLERY_EXCLUDED_SUBDIR: &str = "decorations";

/// 递归收集 `dir` 下全部 `.nbt`，按相对路径排序（稳定网格顺序）。纯 I/O，不碰 ECS。
/// 跳过 `decorations/`（P6 装饰模板，有独立 registry + 渲染审阅路径）。
pub fn discover_structure_nbt_paths(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_nbt(dir, &mut out);
    out.sort();
    out
}

fn collect_nbt(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip the P6 decoration template tree — it has its own registry +
            // render review path and would clutter the large-layout gallery grid.
            if path.file_name().and_then(|n| n.to_str()) == Some(GALLERY_EXCLUDED_SUBDIR) {
                continue;
            }
            collect_nbt(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("nbt") {
            out.push(path);
        }
    }
}

/// 一个 stamp 落位：世界坐标 + BlockState + 逐块 block entity nbt（透传，告示牌/箱子/旗帜）。
pub type StampPlacement = (BlockPos, BlockState, Option<Compound>);

/// 把结构 NBT lower 成 [`StampPlacement`] 列表（在 `origin` 落位）。
///
/// 返回 `(placements, unresolved)`：`unresolved` 是 palette 里 `block_from_name` 解析不出的
/// 块名（caller 应 warn——这些块会被跳过，对应 scout 风险「palette 含未知 block 时结果中该块
/// 被跳过」）。逐块 `block_nbt`（告示牌文本 / 箱子 loot / 旗帜图案等 block entity 数据）原样
/// 透传给 caller，让 `/gallery` stamp 时一并写进世界——否则一次画廊编辑闭环就会把它清空。
pub fn structure_placements(
    structure: &StructureNbt,
    origin: [i32; 3],
) -> (Vec<StampPlacement>, Vec<String>) {
    let unresolved = structure.unresolved_palette_blocks();
    let mut placements = Vec::with_capacity(structure.blocks.len());
    for block in &structure.blocks {
        let Some(entry) = structure.palette.get(block.state as usize) else {
            continue;
        };
        let Some(state) = entry.block_state() else {
            // palette 未知块：跳过（已记入 unresolved），不写空洞。
            continue;
        };
        let pos = BlockPos::new(
            origin[0] + block.pos[0],
            origin[1] + block.pos[1],
            origin[2] + block.pos[2],
        );
        placements.push((pos, state, block.block_nbt.clone()));
    }
    (placements, unresolved)
}

/// 从画廊格包围盒区域读 `(BlockState, block_nbt)` 重建 `StructureNbt`（`/structure save` 用）。
///
/// `read_block` 给定世界坐标返回 `Some((state, block_nbt))`，或 `None` 表示**该格 chunk 未加载**。
///
/// **未加载即失败**：只要区域内任一格 chunk 未加载（玩家离开画廊 / 视距缩小 / slot 超出已加载
/// 范围），立即返回 `Err`——绝不把未加载格降级成 air 然后确定性地把原 `.nbt` 写没。caller 应提示
/// 玩家重新靠近后重试。已加载的 air 格正常跳过（与 `nbt_builder.py::load_structure` 的 air-filter
/// 语义对齐——写出的 blocks 不含 air，re-read 行为一致）。
///
/// 逐块 `block_nbt`（告示牌 / 箱子 / 旗帜等 block entity 数据）原样保留进 `StructureBlockEntry`，
/// 让 save→write→read 闭环不丢这些数据。palette 去重保持首次出现顺序。纯函数，便于单测对拍。
pub fn structure_from_region(
    origin: [i32; 3],
    size: [i32; 3],
    mut read_block: impl FnMut(BlockPos) -> Option<(BlockState, Option<Compound>)>,
) -> Result<StructureNbt, String> {
    let mut palette: Vec<PaletteEntry> = Vec::new();
    let mut blocks: Vec<StructureBlockEntry> = Vec::new();

    for dx in 0..size[0] {
        for dy in 0..size[1] {
            for dz in 0..size[2] {
                let world = BlockPos::new(origin[0] + dx, origin[1] + dy, origin[2] + dz);
                let Some((state, block_nbt)) = read_block(world) else {
                    return Err(format!(
                        "block at [{},{},{}] is in an unloaded chunk; move closer to the gallery \
                         slot so the whole bounding box is loaded, then retry /structure save \
                         (refusing to write the region: unloaded blocks would be lost as air)",
                        world.x, world.y, world.z
                    ));
                };
                if state.is_air() {
                    continue;
                }
                let entry = PaletteEntry::from_block_state(state);
                let palette_idx = match palette.iter().position(|p| *p == entry) {
                    Some(idx) => idx,
                    None => {
                        palette.push(entry);
                        palette.len() - 1
                    }
                };
                blocks.push(StructureBlockEntry {
                    pos: [dx, dy, dz],
                    state: palette_idx as i32,
                    block_nbt,
                });
            }
        }
    }

    Ok(StructureNbt {
        data_version: nbt_io::DATA_VERSION,
        size,
        palette,
        blocks,
        // `/structure save` 从 ChunkLayer 的方块重建结构，没有世界 entity 来源，恒为空列表。
        // 根级 entities 的无损 round-trip 由 nbt_io::read/write_structure_nbt 负责（外部带
        // entity 的 .nbt 经 read→write 不丢），此处保持空与现有 11 个 authored 资产一致。
        entities: Vec::new(),
    })
}

/// dev 命令处理：`/gallery` stamp / `/structure save <name>`。
#[allow(clippy::too_many_arguments)]
pub fn handle_gallery_cmd(
    mut events: EventReader<CommandResultEvent<GalleryCmd>>,
    mut gallery: ResMut<GalleryState>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut layers: Query<&mut ChunkLayer>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        let Some(dimension_layers) = dimension_layers.as_deref() else {
            client.send_chat_message("§c[dev] gallery: DimensionLayers resource missing");
            continue;
        };
        let layer_entity = dimension_layers.entity_for(DimensionKind::Overworld);
        let Ok(mut layer) = layers.get_mut(layer_entity) else {
            client.send_chat_message("§c[dev] gallery: Overworld ChunkLayer missing");
            continue;
        };

        match &event.result {
            GalleryCmd::Stamp => {
                let report = stamp_gallery(&mut layer, &mut gallery);
                client.send_chat_message(report);
            }
            GalleryCmd::Decorations => {
                let report = stamp_decoration_gallery(&mut layer, &mut gallery);
                client.send_chat_message(report);
            }
            GalleryCmd::Save { name } => {
                let report = save_structure(&layer, &gallery, name);
                client.send_chat_message(report);
            }
        }
    }
}

/// stamp 全部资产到网格，记录 slots，返回一行人读报告。
fn stamp_gallery(layer: &mut ChunkLayer, gallery: &mut GalleryState) -> String {
    let root = structures_root();
    let paths = discover_structure_nbt_paths(&root);
    if paths.is_empty() {
        return format!("§c[dev] gallery: no .nbt under {}", root.display());
    }

    let grid = GalleryGrid::default();
    gallery.slots.clear();
    let mut stamped = 0usize;
    let mut warnings = 0usize;

    for (index, path) in paths.iter().enumerate() {
        let name = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let structure = match nbt_io::read_structure_nbt(path) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!("[bong][dev][gallery] read {name} failed: {err}");
                warnings += 1;
                continue;
            }
        };
        let origin = grid.slot_origin(index);
        let (placements, unresolved) = structure_placements(&structure, origin);
        if !unresolved.is_empty() {
            tracing::warn!(
                "[bong][dev][gallery] {name}: unresolved palette blocks skipped: {unresolved:?}"
            );
            warnings += 1;
        }
        for (pos, state, block_nbt) in placements {
            // 透传逐块 block entity nbt（告示牌 / 箱子 / 旗帜等），否则 stamp 出来的方块会丢
            // 掉这些数据，下一次 /structure save 又把空数据写回原 .nbt，一次闭环即清空。
            layer.set_block(pos, Block::new(state, block_nbt));
        }
        // 名牌挂在该格 origin 正上方一格（结构顶 + 2）。
        let sign_pos = BlockPos::new(origin[0], origin[1] + structure.size[1] + 2, origin[2]);
        stamp_nameplate(layer, sign_pos, &name);

        gallery.slots.push(GallerySlot {
            name,
            nbt_path: path.clone(),
            origin,
            size: structure.size,
        });
        stamped += 1;
    }

    format!(
        "§a[dev] gallery stamped {stamped}/{} structures ({warnings} warnings) — \
         see grid at base {:?}",
        paths.len(),
        grid.base
    )
}

/// `/gallery decorations`：把全部 54 个 NBT 装饰变体铺到出生点上空的「虚空审阅台」。
///
/// 每个变体一格：① 7×7 抛光安山岩台基（虚空里的落脚点）；② 按 [`anchor_for_template`]
/// 推断的 anchor 把装饰 stamp 上去（Ground 立台基顶 / Embedded 下沉台基 / Hanging 先在格子
/// 上方铺一小块天花板再从底面挂下）；③ 格前挂名牌写 `<kind>/<variant>`。每格 bbox 记进
/// [`GalleryState::slots`]，`/structure save <name>` 仍可对装饰格用（游戏内改→存回闭环）。
///
/// **dev-only**：直写 ChunkLayer，绕过生产生成链；与 `/gallery` 无参（排除 `decorations/`）
/// 是两条独立审阅路。
fn stamp_decoration_gallery(layer: &mut ChunkLayer, gallery: &mut GalleryState) -> String {
    let registry = DecorationNbtRegistry::load_default();
    if registry.is_empty() {
        return "§c[dev] no decorations (server/structures/decorations/ missing or all failed \
                to parse — run scripts/nbt/decorations/gen_decorations.py)"
            .to_string();
    }

    // 稳定网格顺序：按 template_id 排序后逐格铺开。
    let mut variants: Vec<(String, StructureNbt)> = registry
        .iter()
        .map(|(id, s)| (id.to_string(), s.clone()))
        .collect();
    variants.sort_by(|a, b| a.0.cmp(&b.0));

    let grid = DecorationGalleryGrid::default();
    gallery.slots.clear();
    let mut stamped = 0usize;
    let mut warnings = 0usize;
    let total = variants.len();

    for (index, (template_id, structure)) in variants.iter().enumerate() {
        let cell = grid.slot_origin(index);
        let plan = stamp_one_decoration_cell(layer, &registry, &grid, cell, template_id, structure);
        warnings += plan.warnings;

        // 名牌：挂在格子前缘（-Z 边）、台基顶面上方一格，写 `<kind>/<variant>`（去掉
        // `decorations/` 前缀让标签紧凑可读）。
        let label = template_id
            .strip_prefix("decorations/")
            .unwrap_or(template_id);
        let sign_pos = BlockPos::new(cell[0], cell[1] + 2, cell[2]);
        stamp_nameplate(layer, sign_pos, label);

        gallery.slots.push(GallerySlot {
            name: template_id.clone(),
            nbt_path: DecorationNbtRegistry::default_structures_dir().join(template_id),
            origin: plan.bbox_origin,
            size: plan.bbox_size,
        });
        stamped += 1;
    }

    format!(
        "§a[dev] decoration gallery: stamped {stamped}/{total} 变体 @ base {:?}，\
         飞到出生点上空 y≈{} 审阅（{warnings} warnings）",
        grid.base, grid.base[1],
    )
}

/// 单格审阅台的落位结果：审阅台 bbox（供 `/structure save`）+ warning 计数。
struct DecorationCellPlan {
    bbox_origin: [i32; 3],
    bbox_size: [i32; 3],
    warnings: usize,
}

/// 在 `cell`（台基西北角 min）铺一格审阅台并 stamp 一个装饰变体。返回该格 bbox + warnings。
/// 纯 ChunkLayer 写入；几何决策（anchor → surface_pos）由可单测的纯函数 [`decoration_cell_layout`] 给出。
fn stamp_one_decoration_cell(
    layer: &mut ChunkLayer,
    registry: &DecorationNbtRegistry,
    grid: &DecorationGalleryGrid,
    cell: [i32; 3],
    template_id: &str,
    structure: &StructureNbt,
) -> DecorationCellPlan {
    let layout = decoration_cell_layout(grid, cell, template_id, structure);

    // ① 平整 7×7 台基（单层），台基顶面 = cell.y。
    for dx in 0..PEDESTAL_SIZE {
        for dz in 0..PEDESTAL_SIZE {
            let pos = BlockPos::new(cell[0] + dx, cell[1], cell[2] + dz);
            layer.set_block(pos, Block::new(PEDESTAL_BLOCK, None));
        }
    }

    // ②a Hanging 装饰需要天花板才挂得住：在台基上方 CEILING_CLEARANCE 处铺一小块顶。
    if layout.anchor == DecorationAnchor::Hanging {
        let cy = layout.ceiling_y;
        for dx in 0..PEDESTAL_SIZE {
            for dz in 0..PEDESTAL_SIZE {
                let pos = BlockPos::new(cell[0] + dx, cy, cell[2] + dz);
                layer.set_block(pos, Block::new(CEILING_BLOCK, None));
            }
        }
    }

    // ②b stamp 装饰本体（registry 走 memcpy-level lowering，无 IO/inflate）。Embedded 会
    // 覆盖台基顶（下沉感），Ground/Hanging 直接落位。
    let mut warnings = 0usize;
    if let Some((placements, unresolved)) = registry.stamp(
        template_id,
        layout.surface_pos,
        layout.anchor,
        Rotation::None,
    ) {
        if !unresolved.is_empty() {
            tracing::warn!(
                "[bong][dev][gallery] {template_id}: unresolved palette blocks skipped: \
                 {unresolved:?}"
            );
            warnings += 1;
        }
        for (pos, state, block_nbt) in placements {
            layer.set_block(pos, Block::new(state, block_nbt));
        }
    } else {
        // registry.iter() 提供的 id 必常驻 → stamp 必返回 Some；走到这里说明资产消失（不该
        // 发生），记一条 warning 而非 panic。
        tracing::warn!("[bong][dev][gallery] {template_id}: template not resident in registry");
        warnings += 1;
    }

    DecorationCellPlan {
        bbox_origin: layout.bbox_origin,
        bbox_size: layout.bbox_size,
        warnings,
    }
}

/// 单格审阅台几何（纯函数，便于单测 anchor 三态落点 + bbox，不碰 ECS）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DecorationCellLayout {
    anchor: DecorationAnchor,
    /// 传给 [`DecorationNbtRegistry::stamp`] 的 `surface_pos`（装饰据此定位）。
    surface_pos: BlockPos,
    /// Hanging 天花板所在 y（仅 Hanging 有意义；其余等于台基顶 y，不会被铺）。
    ceiling_y: i32,
    /// 该格审阅 bbox 的 min 角（`/structure save` region 起点）。
    bbox_origin: [i32; 3],
    /// 该格审阅 bbox 尺寸 `[x, y, z]`。
    bbox_size: [i32; 3],
}

/// 计算一格审阅台的 anchor / surface_pos / bbox。`cell` 是台基西北角 min，台基顶面 = `cell.y`。
///
/// surface_pos 的 y 按 anchor 对齐到 registry stamp 语义（见 [`DecorationNbtRegistry::stamp`]）：
/// * Ground   — 装饰 `[0,0,0]` 落 `surface_pos.y+1`，故 `surface_pos.y = 台基顶 y` → 立在台基上。
/// * Embedded — 装饰 `[0,0,0]` 落 `surface_pos.y`，故 `surface_pos.y = 台基顶 y` → 下沉覆盖台基顶。
/// * Hanging  — 装饰顶对齐 `surface_pos.y-1`，故 `surface_pos.y = 天花板 y` → 从天花板底面挂下。
///
/// 装饰水平居中到 7×7 台基中心（+3,+3）。bbox 自台基顶（或地下一格容 Embedded 下沉）向上覆到
/// 装饰/天花板顶，水平覆满 7×7，让 `/structure save` 能把整格审阅列读回。
fn decoration_cell_layout(
    grid: &DecorationGalleryGrid,
    cell: [i32; 3],
    template_id: &str,
    structure: &StructureNbt,
) -> DecorationCellLayout {
    let _ = grid; // 当前几何只依赖 cell + structure；保留参数以备未来按 grid 调台基尺寸。
    let anchor = anchor_for_template(template_id);
    let pedestal_top_y = cell[1];
    let center_x = cell[0] + PEDESTAL_SIZE / 2;
    let center_z = cell[2] + PEDESTAL_SIZE / 2;
    let ceiling_y = pedestal_top_y + CEILING_CLEARANCE;

    let surface_y = match anchor {
        DecorationAnchor::Ground | DecorationAnchor::Embedded => pedestal_top_y,
        DecorationAnchor::Hanging => ceiling_y,
    };
    let surface_pos = BlockPos::new(center_x, surface_y, center_z);

    let deco_h = structure.size[1].max(1);
    // bbox 自台基顶下方一格（容 Embedded 下沉）起，向上覆盖装饰/天花板全高。
    let bbox_min_y = pedestal_top_y - 1;
    let bbox_top_y = match anchor {
        // Ground: 装饰从 pedestal_top+1 起，高 deco_h → 顶 = pedestal_top + deco_h。
        DecorationAnchor::Ground => pedestal_top_y + deco_h,
        // Embedded: 装饰从 pedestal_top 起，顶 = pedestal_top + deco_h - 1。
        DecorationAnchor::Embedded => pedestal_top_y + deco_h - 1,
        // Hanging: 顶 = 天花板 y（含天花板本身）。
        DecorationAnchor::Hanging => ceiling_y,
    };
    let bbox_origin = [cell[0], bbox_min_y, cell[2]];
    let bbox_size = [
        PEDESTAL_SIZE,
        (bbox_top_y - bbox_min_y + 1).max(1),
        PEDESTAL_SIZE,
    ];

    DecorationCellLayout {
        anchor,
        surface_pos,
        ceiling_y,
        bbox_origin,
        bbox_size,
    }
}

/// `/structure save <name>`：回序列化该格包围盒覆盖原 `.nbt`。
fn save_structure(layer: &ChunkLayer, gallery: &GalleryState, name: &str) -> String {
    let Some(slot) = gallery.find(name) else {
        return format!("§c[dev] structure save: no gallery slot `{name}` (run /gallery first)");
    };
    // layer.block(pos) 对未加载 chunk 返回 None → structure_from_region 据此 Err，避免把
    // 未加载格写成 air；已加载的 air 格返回 Some((AIR, _)) 正常跳过。block entity nbt 一并读出。
    let structure = match structure_from_region(slot.origin, slot.size, |pos| {
        layer.block(pos).map(|b| (b.state, b.nbt.cloned()))
    }) {
        Ok(structure) => structure,
        Err(err) => return format!("§c[dev] structure save `{name}` aborted: {err}"),
    };
    match nbt_io::write_structure_nbt(&structure, &slot.nbt_path) {
        Ok(()) => format!(
            "§a[dev] structure saved `{name}` ({} blocks) → {}",
            structure.blocks.len(),
            slot.nbt_path.display()
        ),
        Err(err) => format!("§c[dev] structure save `{name}` failed: {err}"),
    }
}

/// `server/structures` 绝对路径（基于编译期 `CARGO_MANIFEST_DIR`）。
fn structures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(STRUCTURES_DIR)
}

/// 在 `pos` 挂一块写有 `label` 的悬浮名牌（oak sign block entity NBT）。
fn stamp_nameplate(layer: &mut ChunkLayer, pos: BlockPos, label: &str) {
    let messages: Vec<String> = vec![
        label.to_string().into_text().into(),
        String::new().into_text().into(),
        String::new().into_text().into(),
        String::new().into_text().into(),
    ];
    let block = Block {
        state: BlockState::OAK_SIGN.set(PropName::Rotation, PropValue::_8),
        nbt: Some(compound! {
            "front_text" => compound! {
                "messages" => List::String(messages),
            }
        }),
    };
    layer.set_block(pos, block);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn slot_origins_lay_out_in_rows_of_columns() {
        let grid = GalleryGrid {
            base: [0, 70, 0],
            cell_size: 32,
            gap: 4,
            columns: 4,
        };
        let stride = 36;
        // index 0 = base.
        assert_eq!(grid.slot_origin(0), [0, 70, 0]);
        // index 3 = last column of row 0.
        assert_eq!(grid.slot_origin(3), [3 * stride, 70, 0]);
        // index 4 = wraps to row 1, column 0 (+Z).
        assert_eq!(grid.slot_origin(4), [0, 70, stride]);
        // index 5 = row 1 column 1.
        assert_eq!(grid.slot_origin(5), [stride, 70, stride]);
    }

    #[test]
    fn slot_origins_never_overlap_for_distinct_indices() {
        let grid = GalleryGrid::default();
        let mut seen: HashMap<[i32; 3], usize> = HashMap::new();
        for i in 0..20 {
            let origin = grid.slot_origin(i);
            assert!(
                seen.insert(origin, i).is_none(),
                "slot {i} origin {origin:?} collides with slot {} — grid cells must be disjoint",
                seen[&origin]
            );
        }
    }

    #[test]
    fn discover_finds_the_large_layout_assets_and_excludes_decorations() {
        let root = structures_root();
        let paths = discover_structure_nbt_paths(&root);
        // The 11 large authored compounds (dan_zong ×7 + wangyintai ×4) are the
        // gallery's review subjects. P6 added small decoration templates under
        // `decorations/`; those have their own registry + render review path and
        // must NOT show up in the large-layout gallery grid.
        assert_eq!(
            paths.len(),
            11,
            "expected the 11 large-layout .nbt under {} (decorations/ excluded), found {}: {:?}",
            root.display(),
            paths.len(),
            paths
        );
        assert!(
            paths
                .iter()
                .all(|p| !p.components().any(|c| c.as_os_str() == "decorations")),
            "the gallery sweep must skip the decorations/ subdir (P6 templates have \
             a dedicated registry + render review path), but a decorations path leaked: {paths:?}"
        );
        // sorted + every path ends in .nbt
        assert!(
            paths
                .iter()
                .all(|p| p.extension().and_then(|e| e.to_str()) == Some("nbt")),
            "discover must only return .nbt files"
        );
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(
            paths, sorted,
            "discover must return sorted paths for stable grid order"
        );
    }

    #[test]
    fn structure_placements_offsets_blocks_to_origin() {
        let structure = StructureNbt {
            data_version: nbt_io::DATA_VERSION,
            size: [2, 1, 1],
            palette: vec![PaletteEntry {
                name: "minecraft:stone".into(),
                properties: vec![],
            }],
            blocks: vec![
                StructureBlockEntry {
                    pos: [0, 0, 0],
                    state: 0,
                    block_nbt: None,
                },
                StructureBlockEntry {
                    pos: [1, 0, 0],
                    state: 0,
                    block_nbt: None,
                },
            ],
            entities: vec![],
        };
        let (placements, unresolved) = structure_placements(&structure, [100, 64, -50]);
        assert!(unresolved.is_empty(), "stone palette must resolve");
        assert_eq!(
            placements,
            vec![
                (BlockPos::new(100, 64, -50), BlockState::STONE, None),
                (BlockPos::new(101, 64, -50), BlockState::STONE, None),
            ],
            "blocks must be offset by origin and keep palette state (no block entity nbt here)"
        );
    }

    /// 逐块 block entity nbt（告示牌文本等）必须从结构透传到 placements，
    /// 否则 /gallery stamp 时丢 block entity → /structure save 写回空数据清空原 .nbt。
    #[test]
    fn structure_placements_preserve_per_block_entity_nbt() {
        let mut sign_text = Compound::new();
        sign_text.insert("Text1", valence::nbt::Value::String("\"残碑\"".into()));
        let structure = StructureNbt {
            data_version: nbt_io::DATA_VERSION,
            size: [1, 1, 1],
            palette: vec![PaletteEntry {
                name: "minecraft:oak_sign".into(),
                properties: vec![("rotation".into(), "0".into())],
            }],
            blocks: vec![StructureBlockEntry {
                pos: [0, 0, 0],
                state: 0,
                block_nbt: Some(sign_text.clone()),
            }],
            entities: vec![],
        };
        let (placements, unresolved) = structure_placements(&structure, [5, 64, 5]);
        assert!(unresolved.is_empty(), "oak_sign palette must resolve");
        assert_eq!(placements.len(), 1, "single sign block expected");
        assert_eq!(
            placements[0].0,
            BlockPos::new(5, 64, 5),
            "sign must land at origin"
        );
        assert_eq!(
            placements[0].2,
            Some(sign_text),
            "per-block sign nbt must be carried through to the placement so /gallery stamp keeps it"
        );
    }

    #[test]
    fn structure_placements_flags_and_skips_unknown_palette_blocks() {
        let structure = StructureNbt {
            data_version: nbt_io::DATA_VERSION,
            size: [2, 1, 1],
            palette: vec![
                PaletteEntry {
                    name: "minecraft:stone".into(),
                    properties: vec![],
                },
                PaletteEntry {
                    name: "minecraft:totally_not_a_real_block".into(),
                    properties: vec![],
                },
            ],
            blocks: vec![
                StructureBlockEntry {
                    pos: [0, 0, 0],
                    state: 0,
                    block_nbt: None,
                },
                StructureBlockEntry {
                    pos: [1, 0, 0],
                    state: 1, // unknown → skipped
                    block_nbt: None,
                },
            ],
            entities: vec![],
        };
        let (placements, unresolved) = structure_placements(&structure, [0, 0, 0]);
        assert_eq!(
            unresolved,
            vec!["minecraft:totally_not_a_real_block".to_string()],
            "unknown palette block must be reported"
        );
        assert_eq!(
            placements.len(),
            1,
            "unknown palette block must be skipped, only the resolvable stone placed"
        );
        assert_eq!(placements[0].1, BlockState::STONE);
    }

    /// 核心：region 回序列化 → nbt_io 写出 → 再读回，结构等价（包围盒往返）。
    #[test]
    fn region_save_then_read_round_trips() {
        // 摆一个 2x1x2 的小区域：stone + 一个带属性的 stairs，含一格 air（应被跳过）。
        let origin = [10, 64, 20];
        let mut world: HashMap<BlockPos, BlockState> = HashMap::new();
        world.insert(BlockPos::new(10, 64, 20), BlockState::STONE);
        world.insert(
            BlockPos::new(11, 64, 20),
            BlockState::STONE_BRICK_STAIRS
                .set(PropName::Facing, PropValue::South)
                .set(PropName::Half, PropValue::Top),
        );
        // (10,64,21) 留空 = air，(11,64,21) = stone
        world.insert(BlockPos::new(11, 64, 21), BlockState::STONE);

        let size = [2, 1, 2];
        // 整个区域已加载：缺失格 = 已加载的 air（Some((AIR, None))），不是未加载（None）。
        let structure = structure_from_region(origin, size, |pos| {
            Some((world.get(&pos).copied().unwrap_or(BlockState::AIR), None))
        })
        .expect("fully-loaded region must save without an unloaded-chunk error");

        // air 格被跳过：3 个非 air block。
        assert_eq!(
            structure.blocks.len(),
            3,
            "region 内 3 个非 air 块应被收集（air 跳过），实为 {}",
            structure.blocks.len()
        );

        // 写出 → 读回，结构全等（palette/blocks/properties 不漂移）。
        let bytes = nbt_io::write_structure_nbt_bytes(&structure).expect("write region structure");
        let reparsed =
            nbt_io::read_structure_nbt_bytes(&bytes).expect("read back region structure");
        assert_eq!(
            structure, reparsed,
            "region → write → read 必须结构等价（save round-trip 契约）"
        );

        // 再 lower 回 placements：stairs 属性必须存活。
        let (placements, unresolved) = structure_placements(&reparsed, origin);
        assert!(unresolved.is_empty(), "saved palette 必须全可解析");
        let stairs = placements
            .iter()
            .find(|(pos, _, _)| *pos == BlockPos::new(11, 64, 20))
            .expect("stairs 位置应在 placements 中");
        assert_eq!(
            stairs.1.get(PropName::Facing),
            Some(PropValue::South),
            "save→read→lower 后 stairs facing 属性必须保留"
        );
    }

    /// CR critical：未加载 chunk 绝不能被序列化成 air 覆盖原 .nbt。
    /// region 内任一格的 read_block 返回 None（chunk 未加载）→ 整个 save 必须 Err，
    /// 不产出残缺结构。
    #[test]
    fn unloaded_chunk_aborts_save_instead_of_writing_air() {
        let origin = [0, 64, 0];
        let size = [2, 1, 1];
        // (0,64,0) 已加载 stone；(1,64,0) chunk 未加载 → None。
        let result = structure_from_region(origin, size, |pos| {
            if pos == BlockPos::new(0, 64, 0) {
                Some((BlockState::STONE, None))
            } else {
                None
            }
        });
        let err = result.expect_err(
            "a region with an unloaded block must abort the save, not silently write air",
        );
        assert!(
            err.contains("unloaded chunk"),
            "abort message should explain the unloaded-chunk cause for the player; got: {err}"
        );
    }

    /// CR major + 任务 #8：region save→write→read 闭环必须保住逐块 block entity nbt。
    /// 世界里一个 oak_sign 带告示牌文本，save 出来的结构应携带该 nbt，写盘再读回仍在。
    #[test]
    fn region_save_round_trips_block_level_nbt() {
        let origin = [0, 64, 0];
        let size = [1, 1, 1];

        let mut sign_text = Compound::new();
        sign_text.insert(
            "Text1",
            valence::nbt::Value::String("{\"text\":\"丹宗残碑\"}".into()),
        );
        let sign_text_for_assert = sign_text.clone();

        let structure = structure_from_region(origin, size, move |pos| {
            assert_eq!(pos, BlockPos::new(0, 64, 0), "only the single slot is read");
            Some((
                BlockState::OAK_SIGN.set(PropName::Rotation, PropValue::_0),
                Some(sign_text.clone()),
            ))
        })
        .expect("loaded single-sign region must save");

        assert_eq!(structure.blocks.len(), 1, "one sign block expected");
        assert_eq!(
            structure.blocks[0].block_nbt.as_ref(),
            Some(&sign_text_for_assert),
            "save must capture the sign's block entity nbt from the world"
        );

        // 写盘 → 读回：block_nbt 必须存活（不被清空）。
        let bytes = nbt_io::write_structure_nbt_bytes(&structure).expect("write sign structure");
        let reparsed = nbt_io::read_structure_nbt_bytes(&bytes).expect("read sign structure");
        assert_eq!(
            structure, reparsed,
            "region save → write → read 必须保住 block-level nbt（告示牌文本不丢）"
        );
        assert_eq!(
            reparsed.blocks[0].block_nbt.as_ref(),
            Some(&sign_text_for_assert),
            "round-tripped sign must still carry its Text1 block entity nbt"
        );
    }

    /// 空区域（全 air）save 出 0 block 结构，仍可 round-trip。
    #[test]
    fn empty_region_saves_zero_block_structure() {
        let structure =
            structure_from_region([0, 64, 0], [3, 3, 3], |_| Some((BlockState::AIR, None)))
                .expect("all-air loaded region must save fine");
        assert!(
            structure.blocks.is_empty() && structure.palette.is_empty(),
            "全 air 区域应产出 0 block / 0 palette 结构"
        );
        let bytes = nbt_io::write_structure_nbt_bytes(&structure).expect("write empty region");
        let reparsed = nbt_io::read_structure_nbt_bytes(&bytes).expect("read empty region");
        assert_eq!(structure, reparsed, "空区域结构必须 round-trip");
    }

    /// `/structure save` ↔ nbt_builder.py 跨语言对拍：Rust 把画廊 region 序列化写盘，
    /// Python 端 `load_structure` 必须读出相同的非 air 块（含属性）。python3 不可用时跳过
    /// （不静默吞错——python 端报错则硬 assert 撞红）。
    #[test]
    fn region_save_is_readable_by_nbt_builder_python() {
        use std::process::Command;

        let origin = [0, 64, 0];
        let mut world: HashMap<BlockPos, BlockState> = HashMap::new();
        world.insert(BlockPos::new(0, 64, 0), BlockState::MOSSY_STONE_BRICKS);
        world.insert(
            BlockPos::new(1, 64, 0),
            BlockState::STONE_BRICK_STAIRS
                .set(PropName::Facing, PropValue::North)
                .set(PropName::Half, PropValue::Top),
        );
        // (2,64,0) air → skipped.
        let size = [3, 1, 1];
        let structure = structure_from_region(origin, size, |pos| {
            Some((world.get(&pos).copied().unwrap_or(BlockState::AIR), None))
        })
        .expect("fully-loaded region must save without an unloaded-chunk error");

        let dir = std::env::temp_dir();
        let path = dir.join(format!("bong_gallery_save_{}.nbt", std::process::id()));
        nbt_io::write_structure_nbt(&structure, &path).expect("write gallery save structure");

        // repo root = CARGO_MANIFEST_DIR/.. (server/.. = repo).
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("server has parent")
            .to_path_buf();
        let path_str = path.to_string_lossy().replace('\\', "\\\\");
        let code = format!(
            "import sys; sys.path.insert(0, 'scripts/nbt'); \
             from nbt_builder import load_structure; \
             blocks = load_structure('{path_str}'); \
             names = sorted(b.block_name for b in blocks); \
             stairs = [b for b in blocks if b.block_name == 'minecraft:stone_brick_stairs']; \
             print(len(blocks)); \
             print(','.join(names)); \
             print(dict(stairs[0].properties) if stairs else {{}})"
        );

        let output = match Command::new("python3")
            .arg("-c")
            .arg(&code)
            .current_dir(&repo_root)
            .output()
        {
            Ok(o) => o,
            Err(_) => {
                let _ = std::fs::remove_file(&path);
                eprintln!("python3 unavailable — skipping gallery save cross-check");
                return;
            }
        };
        let _ = std::fs::remove_file(&path);
        assert!(
            output.status.success(),
            "python load_structure of gallery-saved nbt failed:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.trim().lines().collect();
        assert_eq!(
            lines[0], "2",
            "python should read 2 non-air blocks (air skipped); got {stdout:?}"
        );
        assert_eq!(
            lines[1], "minecraft:mossy_stone_bricks,minecraft:stone_brick_stairs",
            "python should see exactly the two non-air block names; got {stdout:?}"
        );
        // from_block_state 捕获 BlockState 的全部属性（含默认 shape/waterlogged），
        // 比只存非默认更忠实——pin 完整属性集，确认我们设的 facing/half 在内且未漂移。
        assert!(
            lines[2].contains("'facing': 'north'") && lines[2].contains("'half': 'top'"),
            "python should recover the stairs facing=north/half=top from the gallery save; got {stdout:?}"
        );
    }

    #[test]
    fn gallery_state_find_matches_by_name() {
        let mut state = GalleryState::default();
        state.slots.push(GallerySlot {
            name: "dan_zong/master_sarcophagus.nbt".into(),
            nbt_path: PathBuf::from("/x/dan_zong/master_sarcophagus.nbt"),
            origin: [0, 70, 0],
            size: [5, 4, 5],
        });
        assert!(state.find("dan_zong/master_sarcophagus.nbt").is_some());
        assert!(state.find("missing.nbt").is_none());
    }

    // ── /gallery decorations — 虚空审阅台 ────────────────────────────────────

    /// 用最小 size 造一个装饰 structure（高 `h`，单块），只为驱动 layout 的纯几何。
    fn deco_structure(h: i32) -> StructureNbt {
        StructureNbt {
            data_version: nbt_io::DATA_VERSION,
            size: [1, h.max(1), 1],
            palette: vec![PaletteEntry {
                name: "minecraft:stone".into(),
                properties: vec![],
            }],
            blocks: vec![StructureBlockEntry {
                pos: [0, 0, 0],
                state: 0,
                block_nbt: None,
            }],
            entities: vec![],
        }
    }

    /// anchor 推断：三态 + 默认。镜像 worldgen `_KIND_NBT_ANCHOR`/`_NAME_NBT_OVERRIDE`，
    /// 是审阅台「按目录段摆正」的契约——改这里必须连同 base.py 一起改。
    #[test]
    fn anchor_for_template_mirrors_kind_dir_mapping() {
        assert_eq!(
            anchor_for_template("decorations/grave/small_v1.nbt"),
            DecorationAnchor::Embedded,
            "grave/* 必须 Embedded（grave mound 下沉），镜像 _KIND_NBT_ANCHOR[grave_mound]=embedded"
        );
        assert_eq!(
            anchor_for_template("decorations/hanging_crystal/amethyst_stalactite_v1.nbt"),
            DecorationAnchor::Hanging,
            "hanging_crystal/* 必须 Hanging（从天花板挂下），镜像 _NAME_NBT_OVERRIDE 的 hanging"
        );
        // 其余 kind 全部 Ground。
        for id in [
            "decorations/small_tree/oak_round_v1.nbt",
            "decorations/boulder/mossy_cobble_v1.nbt",
            "decorations/bush_cold/ice_thorn_v1.nbt",
            "decorations/crystal/amethyst_spire_v1.nbt",
            "decorations/spawn_portal/main_v1.nbt",
        ] {
            assert_eq!(
                anchor_for_template(id),
                DecorationAnchor::Ground,
                "{id} 不是 grave/hanging_crystal → 必须落 Ground（立台基顶）"
            );
        }
    }

    /// 畸形 / 无前缀 id 不 panic，安全回退 Ground（与 from_manifest 的兜底语义一致）。
    #[test]
    fn anchor_for_template_defaults_to_ground_on_malformed_id() {
        for id in ["", "grave/small.nbt", "decorations/", "garbage", "/grave/"] {
            assert_eq!(
                anchor_for_template(id),
                DecorationAnchor::Ground,
                "畸形 id {id:?} 必须安全回退 Ground，不 panic"
            );
        }
    }

    /// 审阅台网格 slot_origin determinism：同 index 同坐标。
    #[test]
    fn decoration_grid_slot_origin_is_deterministic() {
        let grid = DecorationGalleryGrid::default();
        for i in 0..54 {
            assert_eq!(
                grid.slot_origin(i),
                grid.slot_origin(i),
                "slot {i} 的 origin 必须确定性（同 index 同坐标）"
            );
        }
    }

    /// 54 格网格布局：列优先 +X，满列换 +Z 行；首格 = base，第 columns 格换行。
    #[test]
    fn decoration_grid_lays_out_columns_then_rows() {
        let grid = DecorationGalleryGrid {
            base: [0, 140, 0],
            cell_size: 12,
            gap: 0,
            columns: 8,
        };
        let stride = 12;
        assert_eq!(grid.slot_origin(0), [0, 140, 0], "index 0 = base");
        assert_eq!(
            grid.slot_origin(7),
            [7 * stride, 140, 0],
            "index 7 = 第 0 行最后一列"
        );
        assert_eq!(
            grid.slot_origin(8),
            [0, 140, stride],
            "index 8 = 换到第 1 行第 0 列（+Z）"
        );
        assert_eq!(
            grid.slot_origin(9),
            [stride, 140, stride],
            "index 9 = 行1列1"
        );
    }

    /// 54 个装饰格两两不重叠，且格间距 ≥ 台基 footprint（7）——确保审阅时各格互不侵占。
    #[test]
    fn decoration_grid_54_cells_never_overlap() {
        let grid = DecorationGalleryGrid::default();
        assert!(
            grid.cell_size >= PEDESTAL_SIZE,
            "cell_size {} 必须 ≥ 台基 footprint {} 否则相邻台基相交",
            grid.cell_size,
            PEDESTAL_SIZE
        );
        let mut seen: HashMap<[i32; 3], usize> = HashMap::new();
        for i in 0..54 {
            let origin = grid.slot_origin(i);
            assert!(
                seen.insert(origin, i).is_none(),
                "格 {i} origin {origin:?} 与格 {} 撞了 — 54 格必须互不重叠",
                seen[&origin]
            );
        }
        assert_eq!(seen.len(), 54, "应有 54 个不同的格 origin");
    }

    /// Ground anchor：装饰落在台基顶面之上一格。surface_pos.y = 台基顶 y（registry Ground
    /// 把 [0,0,0] 摆到 surface_pos.y+1），居中到 7×7 台基中心。
    #[test]
    fn layout_ground_anchor_sits_on_pedestal_top() {
        let grid = DecorationGalleryGrid::default();
        let cell = [0, 140, 0];
        let layout = decoration_cell_layout(
            &grid,
            cell,
            "decorations/small_tree/oak_round_v1.nbt",
            &deco_structure(5),
        );
        assert_eq!(layout.anchor, DecorationAnchor::Ground);
        assert_eq!(
            layout.surface_pos.y, 140,
            "Ground: surface_pos.y = 台基顶 y(140)，装饰据此落 141（台基上一格）"
        );
        assert_eq!(
            (layout.surface_pos.x, layout.surface_pos.z),
            (cell[0] + PEDESTAL_SIZE / 2, cell[2] + PEDESTAL_SIZE / 2),
            "装饰必须水平居中到 7×7 台基中心 (+3,+3)"
        );
    }

    /// Embedded anchor（grave）：surface_pos.y = 台基顶 y，装饰下沉覆盖台基顶（registry
    /// Embedded 把 [0,0,0] 摆到 surface_pos.y）。
    #[test]
    fn layout_embedded_anchor_sinks_into_pedestal() {
        let grid = DecorationGalleryGrid::default();
        let layout = decoration_cell_layout(
            &grid,
            [0, 140, 0],
            "decorations/grave/small_v1.nbt",
            &deco_structure(3),
        );
        assert_eq!(layout.anchor, DecorationAnchor::Embedded);
        assert_eq!(
            layout.surface_pos.y, 140,
            "Embedded: surface_pos.y = 台基顶 y(140)，base 行下沉覆盖台基顶块"
        );
    }

    /// Hanging anchor（hanging_crystal）：surface_pos.y = 天花板 y（台基顶 + CEILING_CLEARANCE），
    /// 装饰顶对齐天花板底面（registry Hanging 顶落 surface_pos.y-1），整体在台基与天花板之间挂下。
    #[test]
    fn layout_hanging_anchor_attaches_under_ceiling() {
        let grid = DecorationGalleryGrid::default();
        let pedestal_top = 140;
        let layout = decoration_cell_layout(
            &grid,
            [0, pedestal_top, 0],
            "decorations/hanging_crystal/amethyst_stalactite_v1.nbt",
            &deco_structure(4),
        );
        assert_eq!(layout.anchor, DecorationAnchor::Hanging);
        let expected_ceiling = pedestal_top + CEILING_CLEARANCE;
        assert_eq!(
            layout.ceiling_y, expected_ceiling,
            "Hanging 天花板必须在台基顶上方 CEILING_CLEARANCE({CEILING_CLEARANCE}) 处"
        );
        assert_eq!(
            layout.surface_pos.y, expected_ceiling,
            "Hanging: surface_pos.y = 天花板 y，装饰顶据此对齐到天花板底面(-1)向下生长"
        );
        assert!(
            layout.surface_pos.y > pedestal_top,
            "Hanging 天花板必须高于台基顶，装饰才挂在二者之间的虚空里"
        );
    }

    /// bbox 覆盖台基 + 装饰：水平满 7×7，纵向自台基顶下一格起覆到装饰/天花板顶，size 全正。
    /// 让 `/structure save <name>` 能把整格审阅列读回（审阅闭环）。
    #[test]
    fn layout_bbox_spans_pedestal_and_decoration() {
        let grid = DecorationGalleryGrid::default();
        let cell = [0, 140, 0];
        for (id, h) in [
            ("decorations/small_tree/oak_round_v1.nbt", 6), // Ground
            ("decorations/grave/small_v1.nbt", 2),          // Embedded
            ("decorations/hanging_crystal/amethyst_stalactite_v1.nbt", 5), // Hanging
        ] {
            let layout = decoration_cell_layout(&grid, cell, id, &deco_structure(h));
            assert_eq!(
                [layout.bbox_size[0], layout.bbox_size[2]],
                [PEDESTAL_SIZE, PEDESTAL_SIZE],
                "{id}: bbox 水平必须覆满 7×7 台基"
            );
            assert!(
                layout.bbox_size.iter().all(|&d| d > 0),
                "{id}: bbox size 必须全正，实为 {:?}",
                layout.bbox_size
            );
            assert_eq!(
                [layout.bbox_origin[0], layout.bbox_origin[2]],
                [cell[0], cell[2]],
                "{id}: bbox 水平 origin 必须对齐台基西北角"
            );
            assert_eq!(
                layout.bbox_origin[1],
                cell[1] - 1,
                "{id}: bbox 自台基顶下一格起（容 Embedded 下沉）"
            );
        }
    }

    // ── 真实资产（P6 Stage2 落盘的 54 个）契约 ───────────────────────────────

    /// 真实 registry：load_default 必须给出 54 个变体，且每个 anchor 都能推断（不 panic）。
    /// 这是审阅台「stamped N/54」报告的真值来源——资产被删/改名会撞红。
    #[test]
    fn real_registry_has_54_variants_each_with_resolvable_anchor() {
        let registry = DecorationNbtRegistry::load_default();
        assert!(
            !registry.is_empty(),
            "审阅台依赖真实装饰资产；为空说明 server/structures/decorations/ 缺失或全解析失败"
        );
        let count = registry.iter().count();
        assert_eq!(
            count, 54,
            "审阅台应铺 54 个变体（task 背景声明）；实为 {count} —— 资产数变了请同步本断言"
        );
        // 每个 id 的 anchor 都能推断，且与目录段一致。
        for (id, _) in registry.iter() {
            let anchor = anchor_for_template(id);
            if id.starts_with("decorations/grave/") {
                assert_eq!(anchor, DecorationAnchor::Embedded, "{id} 应 Embedded");
            } else if id.starts_with("decorations/hanging_crystal/") {
                assert_eq!(anchor, DecorationAnchor::Hanging, "{id} 应 Hanging");
            } else {
                assert_eq!(anchor, DecorationAnchor::Ground, "{id} 应 Ground");
            }
        }
    }

    /// 审阅台 slot 数 == registry.len()(54)：layout 对每个真实变体都产出合法 bbox。
    /// 直接对真实资产派生几何（不碰 ECS），覆盖 stamp_decoration_gallery 的纯函数核。
    #[test]
    fn every_real_variant_yields_a_valid_review_cell() {
        let registry = DecorationNbtRegistry::load_default();
        let grid = DecorationGalleryGrid::default();
        let mut variants: Vec<(String, StructureNbt)> = registry
            .iter()
            .map(|(id, s)| (id.to_string(), s.clone()))
            .collect();
        variants.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(variants.len(), 54, "应有 54 个变体铺成 54 格");

        let mut origins: HashMap<[i32; 3], usize> = HashMap::new();
        for (index, (id, structure)) in variants.iter().enumerate() {
            let cell = grid.slot_origin(index);
            assert!(
                origins.insert(cell, index).is_none(),
                "格 {index}({id}) 与格 {} cell 撞了",
                origins[&cell]
            );
            let layout = decoration_cell_layout(&grid, cell, id, structure);
            assert!(
                layout.bbox_size.iter().all(|&d| d > 0),
                "{id}: bbox 必须全正，实为 {:?}",
                layout.bbox_size
            );
            // Hanging 的天花板必须真的高于装饰最高块，否则挂不住。
            if layout.anchor == DecorationAnchor::Hanging {
                assert!(
                    layout.ceiling_y > cell[1] + structure.size[1],
                    "{id}: Hanging 天花板({}) 必须高于装饰满高({})，才挂得下",
                    layout.ceiling_y,
                    cell[1] + structure.size[1]
                );
            }
        }
        assert_eq!(origins.len(), 54, "54 格 origin 必须两两不同");
    }

    /// 空 registry 不 panic：is_empty 时 stamp_decoration_gallery 应给出友好报告而非崩。
    /// （这里直接驱动空 registry 的派生逻辑——load_default 在缺资产时回退空。）
    #[test]
    fn empty_registry_path_does_not_panic() {
        let registry = DecorationNbtRegistry::empty();
        assert!(registry.is_empty());
        // 空 registry 下 variants 为空 → 没有任何格被铺，报告走 is_empty 分支。
        let count = registry.iter().count();
        assert_eq!(
            count, 0,
            "空 registry 应零变体，审阅台据此报 §c[dev] no decorations"
        );
    }
}
