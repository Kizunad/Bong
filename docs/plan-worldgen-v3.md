# Worldgen v3 — 服务端程序化地形生成

> **目标**：删除 Python Anvil baker，改为 Rust server 在运行时按需生成 chunk。
> Python 仅负责导出 field raster 二进制文件，Rust 消费这些文件填充真正的 MC 方块和 biome。

---

## 一、当前架构问题

现有 `worldgen/scripts/terrain_gen/bakers/anvil.py`（~460 行）用纯 Python 逐格构造 NBT section 写 `.mca` 文件。问题：

1. **极慢**：Python 嵌套循环 × mcworldlib NBT 序列化，一个中等地图分钟级
2. **biome 假数据**：`DEFAULT_BIOME = "minecraft:plains"` 全 hardcode，各 profile 产出的 `biome_id` 整数从未使用
3. **重复劳动**：server 启动后用 `valence_anvil::AnvilLevel` 再解析一遍相同的 NBT
4. **维护负担**：Python 里手写 `_encode_packed_indices`、section NBT 结构、column block resolver，与 Valence 的 `ChunkLayer::set_block` 功能完全重叠

---

## 二、新架构总览

```
worldgen/ (Python)                          server/src/world/ (Rust)
┌───────────────────────┐                  ┌──────────────────────────────────┐
│ blueprint.json        │                  │                                  │
│       ↓               │                  │  startup:                        │
│ terrain_gen pipeline  │                  │    load blueprint JSON           │
│ (噪声 + zone blend)   │                  │    load field rasters (mmap)     │
│       ↓               │                  │    build zone lookup + palette   │
│ export_rasters()      │   .bin + .json   │                                  │
│  → height.bin (f32)   │─────────────────→│  on chunk request:               │
│  → surface_id.bin(u8) │                  │    sample fields @ (x,z)         │
│  → biome_id.bin (u8)  │                  │    fill column blocks            │
│  → water_level.bin    │                  │    set biome 4×4×4               │
│  → extra layers...    │                  │    carve (rift/cave/neg_pressure)│
│  → manifest.json      │                  │                                  │
│  → palette.json       │                  │  fallback (no raster):           │
│                       │                  │    procedural wilderness noise   │
└───────────────────────┘                  └──────────────────────────────────┘
```

**关键决策**：

- **不使用 Anvil 文件**。Valence 的 `ChunkLayer` 可以直接 `insert_chunk` + `set_block`，不需要经过文件系统
- **Python 仅输出 field 数据**。把现有 `TileFieldBuffer.layers` 序列化为 raw binary，不再碰方块/NBT
- **Rust server 消费 raster 按需生成 chunk**。玩家走到哪，生成到哪
- **Wilderness 区域也在 Rust 侧生成**。搬运现有 Python `sample_wilderness_point` 的三角函数噪声

---

## 三、Python 侧改动

### 3.1 新增 `bakers/raster_export.py`（替代 `anvil.py`）

**职责**：把 `GeneratedFieldSet` 序列化为 raw binary 文件 + manifest JSON。

**输出文件格式**：

每个 tile 一组文件，放在 `generated/rasters/tile_{x}_{z}/` 下：

```
tile_-1_-1/
  height.bin        # f32 little-endian, tile_size × tile_size 个值
  surface_id.bin    # u8, 索引 palette.json
  subsurface_id.bin # u8
  biome_id.bin      # u8, 索引 biome_palette
  water_level.bin   # f32 little-endian
  feature_mask.bin  # f32 little-endian
  boundary_weight.bin # f32 little-endian
  # zone-specific extra layers（仅在该 tile 存在时才写）：
  rift_axis_sdf.bin   # f32
  rim_edge_mask.bin   # f32
  cave_mask.bin       # f32
  ceiling_height.bin  # f32
  entrance_mask.bin   # f32
  neg_pressure.bin    # f32
  ruin_density.bin    # f32
```

**manifest.json**（总清单，一份）：

```json
{
  "version": 1,
  "tile_size": 512,
  "world_bounds": { "min_x": -10000, "max_x": 10000, "min_z": -10000, "max_z": 10000 },
  "surface_palette": ["stone", "coarse_dirt", "gravel", "grass_block", "dirt", ...],
  "biome_palette": [
    "minecraft:plains",          // 0 = wilderness
    "minecraft:stony_peaks",     // 1 = broken_peaks
    "minecraft:swamp",           // 2 = spring_marsh
    "minecraft:badlands",        // 3 = rift_valley
    "minecraft:deep_dark",       // 4 = spawn (用 lush_caves 也可)
    "minecraft:dripstone_caves", // 5 = cave_network
    "minecraft:desert"           // 6 = waste_plateau
  ],
  "tiles": [
    { "tile_x": -1, "tile_z": -1, "dir": "tile_-1_-1", "zones": ["spawn"], "layers": ["height", "surface_id", ...] },
    ...
  ]
}
```

**实现要点**：

- 代码量约 60 行。核心循环：遍历 `GeneratedFieldSet.tiles`，对每层 `list[float|int]` 做 `struct.pack` 写入
- `surface_palette` 和 `biome_palette` 提供 `u8 index → string name` 的映射，Rust 侧用它查 `BlockState` 和 biome identifier
- **各 profile 里的 `biome_id` 整数不再是魔法数字**，而是 `biome_palette` 数组的索引，在 manifest 里有明确映射

### 3.2 修改各 profile 对齐 biome_palette

各 profile 文件里 `xxx_biome_id = N` 的 N 需要与 `manifest.json` 里 `biome_palette` 数组的下标一致：

| 整数 | 区域 | MC biome |
|------|------|----------|
| 0 | wilderness | `minecraft:plains` |
| 1 | broken_peaks | `minecraft:stony_peaks` |
| 2 | spring_marsh | `minecraft:swamp` |
| 3 | rift_valley | `minecraft:badlands` |
| 4 | spawn_plain | `minecraft:meadow` |
| 5 | cave_network | `minecraft:dripstone_caves` |
| 6 | waste_plateau | `minecraft:desert` |

这些映射决定了客户端渲染的天空颜色、草地色调、粒子效果等，选择应与世界观氛围匹配。以上是建议值，可以调整。

### 3.3 删除 `bakers/anvil.py`

整个文件删除。`bakers/worldpainter.py` 保留（仍用于可视化调试）。

### 3.4 `__main__.py` 适配

- `--backend` 选项新增 `raster`（作为默认值），去掉 `anvil`
- `raster` 后端调用 `export_rasters(plan, field_set)` 输出到 `--output-dir`

---

## 四、Rust 侧改动

### 4.1 新文件结构

```
server/src/world/
  mod.rs          # 修改：新增 TerrainProvider bootstrap 路径
  zone.rs         # 不动
  events.rs       # 不动
  terrain/
    mod.rs        # TerrainProvider resource + chunk 生成调度 system
    raster.rs     # Raster 文件加载 + 采样（mmap）
    column.rs     # 单列方块填充逻辑（surface/filler/deep/carve/water）
    wilderness.rs # 荒野程序化噪声（移植自 Python）
    biome.rs      # biome_id → BiomeId 映射 + 4×4×4 section 填充
    noise.rs      # coherent_noise_2d 移植
```

### 4.2 TerrainProvider（Resource）

```rust
/// 世界地形数据源。
/// 启动时加载 raster manifest + field 二进制文件（mmap），
/// 运行时提供 sample(world_x, world_z) → ColumnSample。
pub struct TerrainProvider {
    /// 每个 tile 的 field 数据，key = (tile_x, tile_z)
    tiles: HashMap<(i32, i32), TileFields>,
    /// tile 尺寸（块），来自 manifest
    tile_size: i32,
    /// surface_palette：u8 index → BlockState
    surface_palette: Vec<BlockState>,
    /// biome_palette：u8 index → BiomeId（Valence 注册表里的 ID）
    biome_palette: Vec<BiomeId>,
    /// 世界边界
    world_bounds: Bounds2D,
}
```

**TileFields**：

```rust
struct TileFields {
    tile_x: i32,
    tile_z: i32,
    /// 每个 layer 是 mmap'd 的 &[u8]，运行时按 index 读取
    height: Mmap,         // f32 × tile_size²
    surface_id: Mmap,     // u8 × tile_size²
    subsurface_id: Mmap,  // u8 × tile_size²
    biome_id: Mmap,       // u8 × tile_size²
    water_level: Mmap,    // f32 × tile_size²
    feature_mask: Mmap,   // f32 × tile_size²
    boundary_weight: Mmap,// f32 × tile_size²
    // 可选 extra layers
    rift_axis_sdf: Option<Mmap>,
    rim_edge_mask: Option<Mmap>,
    cave_mask: Option<Mmap>,
    ceiling_height: Option<Mmap>,
    entrance_mask: Option<Mmap>,
    neg_pressure: Option<Mmap>,
    ruin_density: Option<Mmap>,
}
```

使用 `memmap2` crate 做 mmap，zero-copy 读取。

### 4.3 ColumnSample + 采样方法

```rust
/// 一根世界列（world_x, world_z）的完整地形信息。
struct ColumnSample {
    height: f32,
    surface_block: BlockState,
    subsurface_block: BlockState,
    biome_id: u8,
    water_level: f32,
    feature_mask: f32,
    boundary_weight: f32,
    // carving 参数
    rift_axis_sdf: f32,
    rim_edge_mask: f32,
    cave_mask: f32,
    ceiling_height: f32,
    entrance_mask: f32,
    neg_pressure: f32,
    ruin_density: f32,
}
```

**采样逻辑**（`terrain/raster.rs`）：

```rust
impl TerrainProvider {
    fn sample(&self, world_x: i32, world_z: i32) -> ColumnSample {
        let tile_x = world_x.div_euclid(self.tile_size);
        let tile_z = world_z.div_euclid(self.tile_size);

        if let Some(tile) = self.tiles.get(&(tile_x, tile_z)) {
            // 从 mmap 读取对应 index 的数据
            let local_x = world_x.rem_euclid(self.tile_size);
            let local_z = world_z.rem_euclid(self.tile_size);
            let index = (local_z * self.tile_size + local_x) as usize;
            // ... 读 f32/u8，查 palette 得 BlockState/BiomeId
        } else {
            // fallback: 荒野程序化生成
            wilderness::sample(world_x, world_z)
        }
    }
}
```

### 4.4 Chunk 生成 system（`terrain/mod.rs`）

**核心 system**：在 Valence 的 chunk 生命周期中，当新 chunk 被插入（`UnloadedChunk::new()`）后立刻填充方块。

```rust
fn fill_new_chunks(
    mut chunk_layer: Query<&mut ChunkLayer>,
    terrain: Res<TerrainProvider>,
) {
    // Valence 会对玩家视距内的 chunk 自动调用。
    // 对每个需要填充的 chunk (chunk_x, chunk_z)：
    for chunk_x, chunk_z in chunks_to_fill {
        let chunk = chunk_layer.insert_chunk([chunk_x, chunk_z], UnloadedChunk::new());

        // 逐列填充
        for local_z in 0..16 {
            for local_x in 0..16 {
                let world_x = chunk_x * 16 + local_x;
                let world_z = chunk_z * 16 + local_z;
                let sample = terrain.sample(world_x, world_z);
                fill_column(chunk, local_x, local_z, &sample);
            }
        }

        // biome：4×4×4 网格
        for section_y in MIN_SECTION..MAX_SECTION {
            for bz in 0..4 {
                for bx in 0..4 {
                    // 采样 4×4 区域中心点的 biome_id
                    let world_x = chunk_x * 16 + bx * 4 + 2;
                    let world_z = chunk_z * 16 + bz * 4 + 2;
                    let sample = terrain.sample(world_x, world_z);
                    let biome = terrain.biome_palette[sample.biome_id as usize];
                    for by in 0..4 {
                        chunk.set_biome([bx, section_y * 4 + by, bz], biome);
                    }
                }
            }
        }
    }
}
```

### 4.5 列填充逻辑（`terrain/column.rs`）

从 Python `anvil.py` 的 `_resolve_column_blocks` + `_column_block_at` 移植，但用 Rust 表达：

```rust
fn fill_column(chunk: &mut Chunk, local_x: i32, local_z: i32, sample: &ColumnSample) {
    let top_y = compute_top_y(sample);
    let water_top = compute_water_top(sample);
    let filler_depth = compute_filler_depth(sample);
    let (carve_floor, carve_ceiling) = compute_carving(sample);
    let deep_block = if top_y > 92 { DEEPSLATE } else { STONE };

    for world_y in 0..=max(top_y, water_top).max(0) {
        let block = match world_y {
            y if y <= BEDROCK_Y => BlockState::BEDROCK,
            // carving（rift/cave）: 在 carve_floor..carve_ceiling 范围掏空
            y if is_carved(y, carve_floor, carve_ceiling) => {
                if water_top >= 0 && y <= water_top && y > top_y {
                    BlockState::WATER
                } else {
                    BlockState::AIR
                }
            }
            y if y > top_y => {
                if water_top >= 0 && y <= water_top { BlockState::WATER }
                else { BlockState::AIR }
            }
            y if y == top_y => sample.surface_block,
            y if y >= top_y - filler_depth => sample.subsurface_block,
            _ => deep_block,
        };
        chunk.set_block([local_x, world_y, local_z], block);
    }
}
```

**carving 规则**（直接移植自 Python `_resolve_column_blocks`）：

- **rift_valley**：`rift_axis_sdf < 0.9` 时降低 top_y，`< 0.42` 时替换表面为 red_sandstone
- **cave_network**：`cave_mask > 0.58` 时在 `carve_floor..carve_ceiling` 掏洞
- **waste_plateau**：`neg_pressure > 0.18` 时地面下沉，`ruin_density > 0.5` 时替换表面
- **entrance**：`entrance_mask > 0.16` 时降低 top_y 形成天坑入口

这些 carving 逻辑不需要修改算法，只需忠实翻译现有 Python 代码。

### 4.6 荒野噪声（`terrain/wilderness.rs`）

移植 Python `wilderness.py` 的 `sample_wilderness_point`：

```rust
pub fn sample(world_x: i32, world_z: i32) -> ColumnSample {
    let x = world_x as f64;
    let z = world_z as f64;

    let continental = (x / 2400.0).sin() * 2.1
        + (z / 2700.0).cos() * 1.7
        + ((x + z) / 3600.0).sin() * 1.3;

    let ridge = (x / 680.0).sin() * 0.95
        + (z / 760.0).cos() * 0.82
        + ((x - z) / 940.0).sin() * 0.68;

    let drainage = 0.5
        + (x / 520.0).sin() * (z / 610.0).cos() * 0.22
        + ((x - z) / 870.0).sin() * 0.16
        + ((x + z) / 1040.0).cos() * 0.12;

    let scar = 0.5
        + ((x + z) / 760.0).sin() * ((x - z) / 690.0).cos() * 0.2
        + (x / 430.0).sin() * (z / 470.0).cos() * 0.14;

    let mut height = 70.0 + continental * 2.4 + ridge * 1.7;
    if drainage < 0.12 { height -= (0.12 - drainage) * 8.0; }
    if scar > 0.82 { height += (scar - 0.82) * 9.5; }

    // surface 选择、water_level 等同理...
    ColumnSample { height, ... }
}
```

**注意**：Python 用 `math.sin/cos`（f64），Rust 用 `f64::sin/cos`，结果完全一致，不需要调参。

### 4.7 `noise.rs`

```rust
pub fn coherent_noise_2d(world_x: f64, world_z: f64, scale: f64, seed: i32) -> f64 {
    let sx = world_x / scale.max(1.0);
    let sz = world_z / scale.max(1.0);
    let sp = seed as f64 * 0.017;
    (sx * 1.17 + sz * 0.83 + sp).sin() * 0.5
        + (sx * -0.71 + sz * 1.29 - sp * 1.3).cos() * 0.3
        + (sx * 2.03 - sz * 1.61 + sp * 0.7).sin() * 0.2
}
```

### 4.8 WorldBootstrap 改造（`world/mod.rs`）

现有 `setup_world` 有两条路径：`FallbackFlat` / `AnvilIfPresent`。改为三条：

```rust
enum WorldBootstrap {
    FallbackFlat(..),        // 现有保留（开发/测试用）
    AnvilIfPresent(..),      // 现有保留（兼容旧存档）
    TerrainRaster(RasterBootstrapConfig),  // 新增
}

struct RasterBootstrapConfig {
    manifest_path: PathBuf,
    raster_dir: PathBuf,
}
```

**选择优先级**：

1. 环境变量 `BONG_TERRAIN_RASTER_PATH` 指向 manifest.json → `TerrainRaster`
2. 环境变量 `BONG_WORLD_PATH` 指向 Anvil 目录 → `AnvilIfPresent`（旧路径保留）
3. 都没有 → `FallbackFlat`

`TerrainRaster` 路径的 `setup_world`：

```rust
fn spawn_raster_world(mut commands: Commands, server: .., dimensions: .., biomes: .., config: RasterBootstrapConfig) {
    let provider = TerrainProvider::load(&config.manifest_path, &config.raster_dir, &biomes);
    let layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);
    commands.spawn(layer);
    commands.insert_resource(provider);
}
```

然后 `fill_new_chunks` system 在每帧检查玩家视距范围内缺失的 chunk 并填充。

---

## 五、chunk 加载策略

Valence 有内建的 chunk 视距管理。当 `ChunkLayer` 配合 `VisibleChunkLayer` 使用时：

1. 玩家移动 → Valence 计算视距内需要的 chunk 集合
2. 对于 `ChunkLayer` 中尚不存在的 chunk，需要在 system 中 `insert_chunk` + 填充
3. 对于玩家离开视距的 chunk，Valence 自动卸载

**实现方式**：写一个 system 在 `Update` schedule 中运行：

```rust
fn generate_chunks_around_players(
    players: Query<&Position, With<Client>>,
    mut layers: Query<&mut ChunkLayer>,
    terrain: Res<TerrainProvider>,
) {
    let view_distance = 12; // chunks
    for pos in &players {
        let center_cx = (pos.0.x / 16.0).floor() as i32;
        let center_cz = (pos.0.z / 16.0).floor() as i32;
        for cx in (center_cx - view_distance)..=(center_cx + view_distance) {
            for cz in (center_cz - view_distance)..=(center_cz + view_distance) {
                for mut layer in &mut layers {
                    if layer.chunk([cx, cz]).is_none() {
                        layer.insert_chunk([cx, cz], UnloadedChunk::new());
                        fill_chunk(&mut layer, cx, cz, &terrain);
                    }
                }
            }
        }
    }
}
```

**性能**：Rust 填充一个 16×16×max_height 的 chunk 约 0.1-0.5ms。在 view_distance=12 时初次加载约 625 chunks，总计 ~0.3 秒（首帧填充后不再重复）。不需要异步/线程化。

---

## 六、Biome 处理细节

MC 1.20.1 的 biome 是 **per-4×4×4-cell** 粒度，不是 per-block。Valence 的 `Chunk::set_biome` 接口已经处理了这个细节。

**Rust 侧流程**：

1. manifest.json 里 `biome_palette` 数组提供 `u8 → "minecraft:xxx"` 映射
2. 启动时将每个 MC biome name 解析为 Valence `BiomeRegistry` 里的 `BiomeId`
3. 对 chunk 内每个 4×4×4 cell，采样中心点 `(bx*4+2, bz*4+2)` 的 `biome_id`
4. 整个 Y 轴列使用相同 biome（因为 field 数据是 2D 的）
5. 调用 `chunk.set_biome([bx, section_y * 4 + by, bz], biome_id)`

**biome 选择原则**：选择能在客户端产生合适氛围的原版 biome（天空颜色、水色、草色、粒子）。以下是建议映射，可以根据视觉效果调整：

| 区域 | 推荐 biome | 理由 |
|------|-----------|------|
| wilderness | `minecraft:plains` | 中性底色 |
| spawn_plain | `minecraft:meadow` | 温和绿色，鸟鸣 |
| broken_peaks | `minecraft:stony_peaks` | 灰色调，无降水 |
| spring_marsh | `minecraft:swamp` | 沼泽水色、蛙鸣 |
| rift_valley | `minecraft:badlands` | 红色调天空 |
| cave_network | `minecraft:dripstone_caves` | 暗色调 |
| waste_plateau | `minecraft:desert` | 荒芜黄调 |

---

## 七、新增 Cargo 依赖

```toml
[dependencies]
# 新增
memmap2 = "0.9"    # mmap 读取 raster 二进制文件
# 以下已有，不需要新增：
# serde, serde_json, valence
```

只需一个新 crate。

---

## 八、实现步骤（按顺序）

### Phase 1：Python 侧 raster 导出

1. **新建 `worldgen/scripts/terrain_gen/bakers/raster_export.py`**
   - `export_rasters(plan, field_set) -> dict[str, Path]`：遍历 tiles，每层 `struct.pack('<' + 'f' * n)` 或 `bytes(u8_list)` 写 `.bin`
   - `build_raster_manifest(plan, field_set, output_dir) -> dict`：生成 manifest.json
   - 代码量目标：< 100 行

2. **修改 `__main__.py`**
   - `--backend` 新增 `raster` 选项（设为默认）
   - `anvil` 选项标记为 deprecated 或直接移除

3. **删除 `bakers/anvil.py`**

4. **验证**：`cd worldgen && python -m scripts.terrain_gen --backend raster` 输出 `.bin` + `manifest.json` 到 `generated/rasters/`

### Phase 2：Rust 侧基础设施

5. **新建 `server/src/world/terrain/` 目录** 及 `mod.rs`、`raster.rs`、`noise.rs`、`wilderness.rs`

6. **实现 `TerrainProvider::load`**
   - 解析 manifest.json
   - mmap 所有 `.bin` 文件
   - 将 `surface_palette` 字符串 → `BlockState` 映射表
   - 将 `biome_palette` 字符串 → `BiomeId`（从 `BiomeRegistry` 查询）

7. **实现 `TerrainProvider::sample`**
   - 有 tile 数据 → 读 mmap
   - 无数据 → `wilderness::sample()`

8. **实现 `wilderness.rs`**
   - 翻译 Python `sample_wilderness_point` 的三角函数噪声
   - 翻译 `coherent_noise_2d`

### Phase 3：Chunk 填充

9. **实现 `terrain/column.rs`**
   - 翻译 `_resolve_column_blocks` 的 carving 逻辑（rift、cave、neg_pressure、entrance）
   - `fill_column(chunk, local_x, local_z, sample)` 逐 Y 写 BlockState

10. **实现 `terrain/biome.rs`**
    - `fill_chunk_biomes(chunk, chunk_x, chunk_z, terrain)` 按 4×4×4 网格采样

11. **实现 chunk 生成调度 system**
    - `generate_chunks_around_players`：检查玩家视距 → insert_chunk → fill_column × 256 → fill_biomes

### Phase 4：Bootstrap 集成

12. **修改 `world/mod.rs`**
    - 新增 `WorldBootstrap::TerrainRaster` 变体
    - 新增 `BONG_TERRAIN_RASTER_PATH` 环境变量读取
    - `spawn_raster_world` 函数：加载 TerrainProvider，spawn LayerBundle

13. **测试**
    - `cd worldgen && python -m scripts.terrain_gen --backend raster`
    - `BONG_TERRAIN_RASTER_PATH=../worldgen/generated/rasters/manifest.json cargo run`
    - 客户端连入后应能看到有地形变化的世界，不同区域天空/草色不同

### Phase 5：清理

14. **删除 Python `mcworldlib`/`nbtlib` 依赖**（`setup.sh` / `.venv` 相关）
15. **更新 `worldgen/README.md`** 和 `CLAUDE.md`

---

## 九、关键约束

1. **不要在 Rust 侧重新实现 terrain_gen 的噪声设计**。Rust 只做两件事：读 raster（有数据的区域）、程序化荒野（无数据区域的 fallback）。Zone 内的地形细节全部由 Python 预计算并导出为 raster。

2. **不要引入异步 chunk 生成**。Valence 的 `ChunkLayer::set_block` 是同步的，在 Bevy system 里直接调用即可。首帧可能稍慢（< 1s），但不影响体验。

3. **surface_palette 到 BlockState 的映射必须覆盖所有 profile 使用的方块**。当前完整列表：
   ```
   stone, coarse_dirt, gravel, grass_block, dirt, sand, red_sandstone,
   terracotta, mud, clay, moss_block, andesite, deepslate, dead_bush
   ```
   外加固定方块：`bedrock`, `water`, `air`。

4. **biome 只需要 2D 采样**。field 数据是 2D 的（没有 Y 轴变化），所以同一列 (x, z) 的所有 section 使用相同 biome。

5. **Python coherent_noise_2d 和 wilderness 噪声的 Rust 翻译必须数值一致**。使用 `f64` 运算，不要改参数。这保证了 raster 覆盖区域和荒野 fallback 区域的地形无缝衔接。

6. **保留 `FallbackFlat` 和 `AnvilIfPresent` bootstrap 路径**。开发和测试时仍需要快速启动的 flat world。

---

## 十、验收标准

1. `python -m scripts.terrain_gen --backend raster` 输出 raster 文件，无 mcworldlib 依赖
2. `BONG_TERRAIN_RASTER_PATH=... cargo run` 启动成功，日志显示 "loaded N terrain tiles"
3. MC 客户端连入后：
   - spawn 区域：绿色草地起伏，biome 显示为 meadow
   - 走向 blood_valley 方向：过渡为红色裂谷地貌，天空颜色变为 badlands 色调
   - 走向 youan_depths：地面出现塌陷/入口
   - 走到 zone 外的荒野：石头/碎石地表，与 zone 边界平滑过渡
4. `cargo test` 全部通过
5. `anvil.py` 已删除，`worldgen/` 不再依赖 `mcworldlib`/`nbtlib`
