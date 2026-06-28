# 末法残土 — 世界生成（worldgen）

固定坐标大地图的地形生成。**两段式**：

1. **离线烘焙（Python）** — 把整张大地图的每一列烘焙成 mmap 友好的二进制 raster 层 + `manifest.json`。
2. **运行时按需生成（Rust server）** — 启动时 `mmap` 这些 `.bin`，玩家走到哪就把对应 chunk 的列实时拼出来。

主流程在 `scripts/terrain_gen/`（blueprint → profiles → stitcher → bakers）。

> ⚠️ **历史路线（datapack + Chunky + multi-noise biome + postprocess.py）已弃用**，仅作参考，不再是固定布局大地图的主路径。本文档末尾「附录」保留其说明。

---

## 数据流总览

```
蓝图 blueprint            profiles              stitcher                bakers                Rust runtime
(固定坐标大地图)    →   (每套地貌一个      →  (zone↔wilderness    →  (raster .bin +    →  (mmap + 按需
 zones.worldview.json    fill_*_tile)          边界融合)              manifest.json)        chunk 生成)
```

| 阶段 | 代码 | 职责 |
|------|------|------|
| 蓝图 | `../server/zones.worldview.example.json` | 固定坐标布局：每个 zone 的 center/size/shape/边界/profile/POI |
| profiles | `scripts/terrain_gen/profiles/*.py` | 每套地貌一个 `fill_*_tile`，纯 numpy 向量化，用 `dsl.*` 噪声原语填层 |
| 层注册表 | `scripts/terrain_gen/fields.py` (`LAYER_REGISTRY`) | 所有层的唯一真相源：default / blend_mode / export_type |
| stitcher | `scripts/terrain_gen/stitcher.py` (`synthesize_fields`) | 逐 tile 把 zone overlay 按 boundary_weight 融进 wilderness 基底 |
| bakers | `scripts/terrain_gen/bakers/raster_export.py` | 写 little-endian `.bin` + `manifest.json`（version=2） |
| 入口 | `scripts/terrain_gen/__main__.py` (`_run_pipeline`) | 串起上面全部 |

---

## 快速开始

```bash
cd worldgen

# 默认主流程：raster backend，写 .bin layers + manifest + zone PNG 预览
bash pipeline.sh ../server/zones.worldview.example.json generated/terrain-gen-smoke raster

# 等价直接调模块
python3 -m scripts.terrain_gen \
  --blueprint ../server/zones.worldview.example.json \
  --output-dir generated/terrain-gen-smoke \
  --backend raster
```

默认参数（不传时）：`--blueprint ../server/zones.worldview.example.json`、
`--profiles terrain-profiles.example.json`、`--output-dir generated/terrain-gen`、
`--tile-size 512`、`--backend raster`。

关键产物：

- `generated/<out>/terrain-plan.json` — 生成计划元数据
- `generated/<out>/terrain-fields-summary.json` — 每 tile 每层 min/max 摘要
- `generated/<out>/rasters/manifest.json` — **运行时入口**（tile 网格、层清单、调色板、POI 元数据）
- `generated/<out>/rasters/*.bin` — 每 tile 每层的二进制
- `generated/<out>/focus-*.png` / `zone-*-*.png` — 总览图 / 分区近景预览

### 让 server 直接消费 raster

```bash
cd ../server
BONG_TERRAIN_RASTER_PATH=/abs/path/worldgen/generated/terrain-gen-smoke/rasters/manifest.json cargo run
```

server 读到该 env → `TerrainProvider::load` 解析 manifest → `mmap` 各 `.bin` → 按需生成 chunk。

---

## 核心概念

### 1. LAYER_REGISTRY（`fields.py`）— 层的唯一真相源

每一个被流水线用到的层都必须在这里登记，声明三件事：

- **`safe_default`** — 无 zone 数据时的列值。**必须与 Rust 侧「无效果」语义一致**（例：`qi_density=0.12` 是末法世界「薄灵」基线）。
- **`blend_mode`** — stitcher 如何把 zone overlay 融进 wilderness 基底：
  - `maximum`（掩码/权重，叠加不抹除）· `minimum`（SDF 距离，越近越强）
  - `lerp`（线性插值，灵气可升可降）· `swap`（离散 id，按抖动阈值二选一）
  - `special`（height/water_level 等由专用代码处理）
- **`export_type`** — `float32` 或 `uint8` 的 raster 序列化类型。

层大致分类：核心几何层（`height` / `surface_id` / `subsurface_id` / `water_level` / `biome_id`）、
修仙语义层（`qi_density` / `mofa_decay` / `qi_vein_flow` / `spirit_eye_candidates` / `realm_collapse_mask`）、
垂直层（`sky_island_mask` / `underground_tier`）、生态层（`flora_*` / `ground_cover_*`）、
矿物层（`mineral_density` / `mineral_kind`）、异常层（`anomaly_*`，给天道/血月/裂隙事件钩子）、
结构层（`fossil_bbox`）、以及 TSY 维度专用层（`tsy_*`，主世界 manifest 由 `layer_whitelist` 过滤掉）。

### 2. Spans — 列竖直结构的统一表示

一个「列」的竖直结构 = 最多 4 段 `(floor_y, ceiling_y)` 实心段（`fields.py::ColumnSpans`）。
这一单一表示替代了旧的 `height` + `cave_mask` + `sky_island_base_y` + `ceiling_height` 等一堆补丁层：

- **`spans[0]` 永远是地表段**，其 `ceiling` = 可行走表面（NPC 寻路 / 装饰锚定 / `surface_y` 都读它）。
- 地表段**下方**多一段 → 中间空气就是**洞穴**。
- 地表段**上方**多一段 → 那是**浮岛**。
- 段与段之间必须有真正的空气隙（构造时校验，非法编码直接报错）。

二进制布局（mmap 固定步长）：`spans_count.bin`（每列 1 字节，0..=4）+
`spans.bin`（每列 16 字节 = 4 段 × 2 个 little-endian `i16`，未用槽填哨兵 `i16::MAX`）。
世界 Y ∈ [-64, 432)。Rust 侧 `server/src/world/terrain/raster.rs` 的 `ColumnSpanList` 是它的镜像。

### 3. Profiles — 每套地貌一个填充器

`profiles/` 下约 20 个 profile，每个是一个纯 numpy 向量化的 tile 填充器，
用 `dsl.warped_height` / `fbm_height` / `radial_uplift` 等噪声原语合成多尺度地形，
再写进 `LAYER_REGISTRY` 里登记的层数组。profile 只填**自己 zone 内**的理想态 field，边界交给 stitcher。

已实现的 profile：

```
spawn_plain        broken_peaks       spring_marsh       rift_valley
cave_network       waste_plateau      pseudo_vein_oasis  rift_mouth_barrens
ash_dead_zone      sky_isle           abyssal_maze       ancient_battlefield
tribulation_scorch jiu_zong_ruin      dan_zong_yi_yuan   wangyintai
tsy_zongmen_ruin   tsy_daneng_crater  tsy_zhanchang      tsy_gaoshou_hermitage
```

profile 的数值标定（base 高度 / 灵气 / 生态）在 `terrain-profiles.example.json` 与各 profile 文件内。

### 4. Stitcher — zone ↔ wilderness 边界融合

`synthesize_fields`（`stitcher.py`）逐 tile：先 `fill_wilderness_tile` 铺野外基底，
再对每个与 tile 相交的 zone 算 `_compute_boundary_weight_array`（按 shape 隶属比 + 边界模式
hard/semi_hard/soft + 噪声扰动得 0~1 权重），用 `_blend_tile_layers` 把 overlay 按各层 `blend_mode` 融进基底。
height 连续 lerp + 噪声抖动接缝；离散 id 用**抖动阈值 swap**（避免硬边）；竖直结构由 `blend_spans` 处理
（0.5 抖动线决定洞穴/浮岛归属，只有表面 ceiling 连续 lerp）。

增量重生成（`--zone-filter a,b`）只挑**哪些 tile** 被合成，但被选 tile 仍融合**所有**重叠 zone →
输出 byte 级等同全量跑，接缝无缝。`--zone-filter` 写了不存在的 zone 名会 fail-fast。

---

## Backend 与可选 pass

`--backend` 三选一：

| backend | 产物 | 用途 |
|---------|------|------|
| `raster`（默认） | `rasters/*.bin` + `manifest.json` + PNG 预览 | **运行时正式路径** |
| `worldpainter` | `worldpainter/` 项目 | 调试 / 肉眼审 raster 是否合理 |
| `anvil` | 先跑 raster，再叠 `world/region/r.*.mca` | 直接产出可加载世界存档（snapshot/快照） |

> `pipeline.sh` 的 `anvil` backend 会先跑一遍 `raster`（保证 PNG 预览完整），再用
> `anvil_world_export` 读刚产出的真 spans 写 chunk。

其它可选 pass：

- **成套建筑布局**：zone 设 `architectural_layout` 时，`run_layout_pass` 跑 `COMPOUND_LAYOUT_REGISTRY`
  里的布局，把 NBT 摆放写进 `rasters/placement_manifest.json`（NBT 源在 `../server/structures/<subdir>/`）。
- **TSY 第二维度**：传 `--tsy-blueprint <json>` 会对 TSY 维度再跑一遍 export 到 `--tsy-output-dir`。
- **3D 预览控制台（dev-only）**：`bash pipeline.sh --console`，raster 后启动 FastAPI（`http://127.0.0.1:8765`），
  供 `worldgen/console/`（vite + three.js）查看。需先 `bash setup.sh --console` 装 fastapi+uvicorn。

---

## Rust 运行时如何消费

`server/src/world/terrain/`：

1. **入口** `world/mod.rs` 读 env `BONG_TERRAIN_RASTER_PATH` → `spawn_raster_world` →
   `TerrainProvider::load`（`raster.rs`）解析 manifest + 校验 `version == 2`。
2. **mmap** `TileFields`（`raster.rs`）每个 tile 的每层一个 `memmap2::Mmap`；
   严格校验文件大小，`read_f32` / `read_u8` 按 `index*N` 切 little-endian 解码（零拷贝）。
3. **采样** `TerrainProvider::sample(x, z)` → 定位 tile + 列索引 → 填出 `ColumnSample`（含 spans + 所有语义层）；
   越界列回退程序化 `wilderness::sample`。
4. **按需生成 chunk** `generate_chunks_around_players` system 遍历每个玩家 `View`，对未生成的 chunk 调
   `ensure_chunk_generated`（限流每 tick 每客户端 1 个 chunk 防卡）→ 16×16 列各跑 `column::fill_column`
   （spans → bedrock / 水 / 表面 / 洞穴掏空 / 浮岛逐 Y `set_block_state`）→ 再叠 decoration / flora /
   structures / authored / mineral / biome 各 pass → `layer.insert_chunk`。

> 世界高度刻意设 `WORLD_HEIGHT = 496`，让 Valence 9-bit packed heightmap 编码不溢出。

### ⚠️ 两套独立的「灵气 / zone」系统，别混

- **raster `qi_density` 层** — 烘焙进 `.bin` 的**静态地形属性**，被 botany 植物存活、cultivation 负压、
  terrain structures、worldgen/pseudo_vein 读取。
- **运行时 zone 经济** — `server/src/world/zone.rs` 的 `Zone.spirit_qi` + `world/karma.rs` 的
  `QiDensityHeatmap`，从 `zones.json` **动态加载**，技能扣灵气 / 区域守恒走它，**不从 raster 读**。

raster 里 POI / FossilBbox 携带的 `zone: String` 只是叙事元数据。

---

## 测试

```bash
cd worldgen
python3 -m pytest tests/ scripts/terrain_gen/        # 全量
python3 -m pytest tests/test_carvers.py -q           # 单文件示例
```

raster 后验（rift_axis_sdf 默认值 / height range / water depth）：
`scripts/terrain_gen/harness/raster_check.py`。

---

## 相关文档

- `../docs/worldgen-pipeline-v2.md` — 流水线 v2 设计
- `../docs/worldgen-terrain-profiles.md` — 地貌规则
- `terrain-profiles.example.json` — profile 数值标定示例
- `CLAUDE.md`「Architecture notes」/「Worldgen 流水线」节 — 顶层总览

---

## 附录：弃用的 datapack 路线（仅供参考）

早期通过 datapack 覆盖 overworld 维度的 `biome_source`（multi-noise 6 维噪声选 6 个自定义 biome），
配合 Chunky 预生成 + `scripts/postprocess.py` 在 Anvil `.mca` 上做方块级装饰。
该路线**不再用于固定布局大地图**——固定坐标地图改由上文 `terrain_gen` raster 流水线驱动。
`worldgen-mofa/`（datapack）与 `scripts/postprocess.py` 保留作历史参考。
