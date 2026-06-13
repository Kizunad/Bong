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
    /// `/gallery` — stamp 全部资产到画廊网格。
    Stamp,
    /// `/structure save <name>` — 回序列化 `name` 格覆盖原 `.nbt`。
    Save { name: String },
}

impl Command for GalleryCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("gallery")
            .with_executable(|_| GalleryCmd::Stamp);

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
}
