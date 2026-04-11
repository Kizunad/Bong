# Worldgen v3.1 — 地形后续升级方向

> **前置**：v3.0 已完成 6 个 phase（世界高度 512、方块颜色、荒野起伏、灵泉千岛湖、血谷深裂、植被装饰）。
> 本文档记录下一步升级方向，按优先级排列。

---

## Phase 7: 巨树生成

**目的**：在特定区域生成 50-200+ 格高的古木巨树，体现修仙世界的灵气汇聚

### 算法选型

**Space Colonization Algorithm (SCA)** — Adam Runions et al. 2007

核心思路：
1. 定义树冠体积（椭球、锥体、自定义 SDF）
2. 在树冠内散布 N 个"吸引点"（attraction points）
3. 从树干基部开始，每个迭代：
   - 每个树节点找到影响半径内最近的吸引点
   - 向吸引点方向生长一步（step_size = 1-3 blocks）
   - 移除已被"到达"的吸引点（kill_distance）
4. 重复直到所有吸引点被消耗或迭代上限
5. 结果：有机分支骨架，自然填充树冠体积

**为什么选 SCA**：
- 天然适配体素世界（每步生长 = 1 block）
- 分支形态有机，不像 L-System 那样规则
- 可通过调整吸引点分布控制树形（伞形、塔形、垂柳形）
- 计算量可控：O(N × M × iterations)，N=吸引点数，M=树节点数
- Minecraft 的大型丛林树/黑橡木是模板式的，SCA 能产生更自然的结果

### 体素化流程

```
SCA skeleton (3D line segments)
    ↓
rasterize trunk: 圆柱体，半径随高度递减
    ↓
rasterize branches: 细圆柱体，沿骨架方向
    ↓
place leaves: 终端分支周围球形/椭球填充
    ↓
place roots: 从基部向外+向下的反向 SCA 或简单扩展
    ↓
add details: 藤蔓(vine)、发光苔(glow_lichen)、蜂巢等
```

### 实现位置

Rust 侧 `server/src/world/terrain/mega_tree.rs`：
- `struct MegaTreeParams { trunk_height, crown_radius, crown_shape, attraction_count, ... }`
- `fn generate_mega_tree(params, rng) -> Vec<(BlockPos, BlockState)>`
- 在 `decoration.rs` 中根据 `feature_mask` + biome + hash 触发

### 参数变体

| 树型 | 干高 | 冠径 | 吸引点 | 位置 |
|------|------|------|--------|------|
| 灵木（出生点） | 80-120 | 40-60 | 3000 | spawn biome=4 |
| 古松（青云峰） | 40-60 | 20-30 | 1500 | peaks biome=1, Y<200 |
| 枯树（荒原） | 30-50 | 15-25 | 800, 低密度 | wastes biome=6 |
| 水杉（灵泉岛） | 25-40 | 12-18 | 1200 | marsh biome=2, 岛上 |

### 推荐方案：混合式

最终推荐 **SCA 主干 + L-System 细枝 + Perlin 根系**：

1. **SCA** 生成主干和一级分支骨架（粗体素圆柱）
2. **参数化 L-System** 在一级分支末端展开二级/三级细枝（规则简单，速度快）
3. **Perlin 位移随机游走** 从树基向外+向下延伸根系藤蔓

这样主干有机自然，细枝有分形美感，根系有地形适应性。

### SCA 关键参数

```rust
struct ScaParams {
    // 吸引点
    attraction_count: u32,     // 散点数量，越大越密
    crown_shape: CrownShape,   // Ellipsoid / Cone / Custom
    crown_center: Vec3,        // 树冠中心（相对树基）
    crown_radii: Vec3,         // 树冠 XYZ 半径

    // 生长
    influence_radius: f32,     // 吸引点影响距离（通常 8-20 blocks）
    kill_distance: f32,        // 到达后移除距离（通常 2-4 blocks）
    step_size: f32,            // 每步生长距离（1-3 blocks）
    max_iterations: u32,       // 迭代上限（通常 500-2000）

    // 体素化
    trunk_base_radius: f32,    // 树干基部半径（3-8 blocks）
    trunk_taper: f32,          // 锥度（0.6-0.85，越小越快变细）
    branch_radius_ratio: f32,  // 分支半径 = 父节点半径 × ratio
    min_branch_radius: f32,    // 最小半径（0.5 = 1 block 宽）

    // 树叶
    leaf_radius: f32,          // 终端分支叶球半径（2-5）
    leaf_density: f32,         // 0.0-1.0，越高越密
}
```

### 跨 chunk 处理

巨树跨越多个 chunk（200 格高 × 60 格宽 = 至少 4×4 chunk）。两种方案：

**方案 A：预生成 + 缓存（推荐）**
- 在世界生成阶段（Python 或启动时 Rust），为每棵巨树预计算完整方块列表
- 存储为 `Vec<(BlockPos, BlockState)>`，序列化到 raster 目录
- chunk 生成时查询本 chunk 范围内的巨树方块并放置
- 优点：确定性，无遗漏；缺点：需要额外存储

**方案 B：种子 + 按需生成**
- 每个 chunk 生成时，检查附近是否有巨树种子点
- 如果有，运行完整 SCA 生成该树，只放置落在本 chunk 的方块
- 用 LRU cache 避免重复计算同一棵树
- 优点：无额外存储；缺点：同一棵树可能被计算多次

### 参考实现

- **Space Colonization**: Runions 2007 原论文 "Modeling Trees with a Space Colonization Algorithm"
- **jceipek/Space-Colonization** (C++): 干净的 SCA 实现，可直接参考
- **friggog/tree-gen** (Blender/Python): SCA-based 树生成器，有完整体素化
- **arbaro** (Java): Weber-Penn 参数化树生成器
- **Dynamic Trees** (Minecraft Forge mod): 类 SCA 信号系统模拟生长
- **TerraForged** (Minecraft mod): 分形噪声 + 模板混合
- MC 原版大型树（丛林巨木/黑橡木）是硬编码模板式，50+ 格就不够用了

---

## Phase 8: 洞穴内部装饰

**目的**：当前 cave_network 只有空腔，缺少洞穴氛围

### 装饰类型

| 位置 | 方块 | 条件 |
|------|------|------|
| 天花板 | pointed_dripstone (stalactite) | cave_mask > 0.6, hash < 80 |
| 天花板 | glow_lichen | cave_mask > 0.7, hash < 40 |
| 地面 | moss_carpet | cave_mask > 0.65, hash < 60 |
| 地面 | pointed_dripstone (stalagmite) | cave_mask > 0.6, hash < 50 |
| 水面 | dripleaf | 洞穴内水体上方 |

### 实现

在 `decoration.rs` 中新增 `decorate_cave_column()`，在 `carve_floor..carve_ceiling` 范围内放置装饰。

---

## Phase 9: 水体装饰

**目的**：水底和水面缺少植被

### 内容

- 灵泉浅水区：seagrass（海草），随机密度
- 灵泉深水区：kelp（海带），从底部向上生长 3-8 格
- 所有水面：lily_pad 已有，增加覆盖率
- 血谷低洼处：如有水面，放置 magma_block 产生气泡柱效果

---

## Phase 10: 子表面多层化

**目的**：当前 subsurface 只有 stone，应有自然分层

### 分层方案

```
surface_block          ← 来自 profile
dirt/mud/gravel        ← filler_block, 3-7 层
stone                  ← Y > 0 的主体
deepslate              ← Y < 0 的主体
bedrock                ← Y = min_y
```

### 改动

`column.rs` 的 `block_at()` 中 `deep_block` 逻辑改为：
- `world_y > 8`: STONE
- `world_y > -32`: 混合 STONE/DEEPSLATE（按 noise hash 过渡）
- `world_y <= -32`: DEEPSLATE
- `world_y == bedrock_y`: BEDROCK

---

## Phase 11: 区域过渡平滑

**目的**：zone→wilderness 边界的线性插值过于生硬

### 方案

`stitcher.py` 中 `boundary_weight` 的计算改用 Hermite 平滑：

```python
t = linear_ratio  # 0..1
smooth = t * t * (3.0 - 2.0 * t)  # smoothstep
```

同时在 Rust `column.rs` 中，当 `boundary_weight` 在 0.1-0.9 范围时，对 `filler_depth` 做渐变。

---

## Phase 12: 结构物生成

**目的**：利用已有的 `ruin_density` / `feature_mask` 放置小型结构

### 结构类型

| 结构 | 触发条件 | 区域 |
|------|----------|------|
| 废墟石柱 | ruin_density > 0.6 | waste_plateau |
| 灵石矿脉 | feature_mask > 0.8, hash | peaks |
| 残破祭坛 | neg_pressure > 0.5 | waste_plateau |
| 古桥残骸 | rift_axis_sdf 0.9-1.1 | rift_valley |

### 实现

新建 `server/src/world/terrain/structures.rs`，类似 decoration 但处理多方块结构体。用 schematic 模板 + 随机旋转/变体。

---

## Phase 13: 生物群系细化

**目的**：7 个硬编码 biome 太粗糙

### 细化方案

| 当前 biome_id | 细分 | 条件 |
|---------------|------|------|
| 0 (wilderness) | plains / forest / river | 按 drainage/height |
| 1 (peaks) | frozen_peaks / stony_peaks | Y > 300 / Y <= 300 |
| 2 (marsh) | swamp / mangrove_swamp | 水深 / 浅 |
| 4 (spawn) | meadow / flower_forest | feature_mask 高低 |

需要同时改 Python biome_id 分配和 Rust biome 映射。

---

## 优先级总结

```
Phase 7  巨树生成      ← 视觉冲击最大，修仙特色
Phase 8  洞穴装饰      ← 小工作量，体验提升明显
Phase 9  水体装饰      ← 小工作量
Phase 10 子表面分层    ← 挖矿时才看到
Phase 11 过渡平滑      ← 锦上添花
Phase 12 结构物        ← 需要设计 schematic
Phase 13 biome 细化    ← 影响粒子/音效/天气
```
