# Bong · plan-terrain-layer-query-v1

**worldgen 多通道 layer 按名查询接口**。在 `TerrainProvider` 上补 `sample_layer_f32(x, z, layer_name) -> Option<f32>` / `sample_layer_u8` 两条统一查询入口 + `layer_names()` 元数据列表，把现有 `ColumnSample` 各 layer 字段以 dynamic dispatch 暴露出来，供 `plan-botany-v2` P0、`plan-mineral-v2` 共享 layer 接入范式。

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | `sample_layer_f32` / `sample_layer_u8` adapter + `layer_names()` + 饱和单测（每个 layer 名各一条 happy / unknown / out-of-bounds 三档） | ⬜ |
| P1 | botany-v2 / mineral-v2 调用方接入点声明（仅文档：列出哪几个调用点会从静态字段切到 `sample_layer_*`） | ⬜ |

**世界观锚点**：无（纯接口适配，不引入新世界观语义）。

**library 锚点**：无（纯工程改造）。

**交叉引用**：
- `plan-botany-v2.md`（active；P0 §9 自报"sample_layer 接口缺口约 80–150 行 Rust"——**实际底层已实装**，仅需 ~30–50 行 thin adapter）
- `plan-mineral-v2.md`（骨架；§"共享 worldgen layer 接入范式"将复用本接口）
- `plan-worldgen-v3.1.md`（已 merged；`LAYER_REGISTRY` Python 侧定义见 `worldgen/scripts/terrain_gen/fields.py`，本 plan 在 Rust 侧暴露）

---

## §-1 前置事实核验（2026-04-29 audit）

| 事实 | 位置 |
|------|------|
| `ColumnSample` 已含全部 16 layer 字段 | `server/src/world/terrain/raster.rs:29-81`（`height / surface_block / subsurface_block / biome_id / biome / water_level / feature_mask / boundary_weight / rift_axis_sdf / rim_edge_mask / cave_mask / ceiling_height / entrance_mask / fracture_mask / neg_pressure / ruin_density / qi_density / mofa_decay / qi_vein_flow / sky_island_* / underground_tier / cavern_floor_y / flora_density / flora_variant_id / anomaly_intensity / anomaly_kind / tsy_*`，全部 28 个） |
| `TerrainProvider::sample(x, z) -> ColumnSample` 已实装 | `raster.rs:486` |
| mmap raster 多通道读取已实装 | `raster.rs:555 TileFields::load`（每 layer 一个 `.bin` 文件） |
| `LAYER_REGISTRY` 元数据来源 | `worldgen/scripts/terrain_gen/fields.py:45`（28 个 `LayerSpec`，含 `safe_default / blend_mode / export_type`） |

**结论**：botany-v2 §9 自报的"接口缺口"实质是**缺一个按字符串名查询的薄适配层**——底层 mmap + 字段解析全部就绪。本 plan 工作量约 30–50 行 Rust + 单测，远小于 plan-botany-v2 §9 估算的 80–150 行。

---

## §0 设计轴心

- [ ] **不重写底层**：直接在 `TerrainProvider` 上加方法，`sample(x, z)` 内部一次性读全部字段然后按名分发；不做"按需 mmap 单 layer"的微优化（无证据表明热点）
- [ ] **f32 / u8 双轨**：worldgen `LAYER_REGISTRY` 区分 `export_type: float32 | uint8`（见 fields.py:42）。Rust 侧也分两条 API，避免类型双关
- [ ] **未知 layer 名返回 `None`**（不 panic）：调用方写错 layer 名时降级到 safe_default 路径，不崩 server
- [ ] **`layer_names()` 元数据自检**：返回 `&'static [&'static str]`，配合单测对照 `LAYER_REGISTRY` 的 28 条命名（防止 Rust ↔ Python 漂移；single source of truth 是 Python，Rust 只负责镜像）
- [ ] **不要在本 plan 改任何 botany / mineral 调用方代码**——只做接口侧；调用方迁移留给 botany-v2 P0 / mineral-v2 自决

---

## §1 接口签名（下游 grep 抓手）

```rust
// server/src/world/terrain/raster.rs

impl TerrainProvider {
    /// 按 layer 名查询 float32 通道；未知名 / out-of-bounds → None
    pub fn sample_layer_f32(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<f32>;

    /// 按 layer 名查询 uint8 通道
    pub fn sample_layer_u8(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<u8>;

    /// 返回所有可查询的 layer 名（含 export_type 提示）
    pub fn layer_names() -> &'static [LayerSchema];
}

/// 每条 layer 的元数据（与 worldgen LAYER_REGISTRY 镜像）
pub struct LayerSchema {
    pub name: &'static str,
    pub export_type: LayerExportType, // F32 | U8
    pub safe_default_f32: Option<f32>,
    pub safe_default_u8: Option<u8>,
}
```

**调用约定**：
- f32 layer 调用 `sample_layer_u8` → 返回 `None`（类型不匹配视为未知）
- u8 layer 调用 `sample_layer_f32` → 返回 `None`
- 不存在的 layer 名 → 返回 `None`
- 在 wilderness（无 tile）但 layer 已在 schema → 返回 layer 的 `safe_default`（与 `sample()` wilderness fallback 行为一致）

---

## §2 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|------|------|
| `TerrainProvider::sample_layer_f32` / `sample_layer_u8` | `server/src/world/terrain/raster.rs`（同文件 impl） |
| `TerrainProvider::layer_names` | `server/src/world/terrain/raster.rs` |
| `LayerSchema` / `LayerExportType` | `server/src/world/terrain/raster.rs` |
| 镜像表 28 条 layer schema 数组 | `raster.rs` 文件内 `const LAYER_SCHEMAS: &[LayerSchema] = &[...]` |
| Python 镜像比对单测 fixture | `worldgen/scripts/terrain_gen/fields.py:45 LAYER_REGISTRY`（不动；测试侧 dump 成 JSON 由 Rust 单测读） |

---

## §3 测试饱和（CLAUDE.md "饱和化测试"）

每条 layer × 三档：
- `sample_layer_f32_known_layer_returns_value` × 19 个 f32 layer 名（per-layer test，避免一坨大 case）
- `sample_layer_u8_known_layer_returns_value` × 9 个 u8 layer 名
- `sample_layer_f32_unknown_name_returns_none`
- `sample_layer_u8_unknown_name_returns_none`
- `sample_layer_f32_called_on_u8_layer_returns_none`（类型双关防御）
- `sample_layer_u8_called_on_f32_layer_returns_none`
- `sample_layer_in_wilderness_returns_safe_default`（无 tile 时返回 `LayerSchema.safe_default_*`）
- `layer_names_size_matches_python_registry`（fixture：从 worldgen 侧导一份 JSON，Rust 单测读取并断言名字+类型对齐）
- `layer_names_no_duplicates`
- `out_of_world_bounds_returns_none_or_default`（视 wilderness 行为）

---

## §4 实施节点

- [ ] **P0**（约半天）：
  - 加 `LayerSchema` / `LayerExportType` 类型 + `LAYER_SCHEMAS` 静态表（28 条）
  - 加 `sample_layer_f32` / `sample_layer_u8` / `layer_names` 三个方法
  - 写齐 §3 列出的全部 happy / unknown / type-mismatch / wilderness / 镜像比对单测
  - `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
  - 文档：在 `raster.rs` 顶端加一段注释说明"single source of truth 是 worldgen/.../fields.py"
- [ ] **P1**（仅文档）：列出 plan-botany-v2 / plan-mineral-v2 调用点应改的位置——本 plan 不动那些文件，只标记给后续 plan

---

## §5 验收

| 阶段 | 验收条件 |
|------|----------|
| P0 | 三个方法 + LayerSchema 镜像表落地；§3 单测全部跑通；与 Python `LAYER_REGISTRY` 名字+类型严格对齐（fixture 比对单测命中）；wilderness 路径返回 safe_default |
| P1 | docs/plan-botany-v2.md / docs/plan-mineral-v2.md 在 §"前置依赖"小节标注"本 plan 提供 sample_layer_* 接口，可调用"；不改代码 |

---

## §6 风险

| 风险 | 缓解 |
|------|------|
| Rust `LAYER_SCHEMAS` 与 Python `LAYER_REGISTRY` 漂移 | §3 镜像比对单测 + worldgen 侧加 dump-to-json 脚本，CI 一致性自检；single source of truth 锁定为 Python，Rust 镜像 |
| 调用方误用 f32/u8 类型 | API 双轨 + 单测显式覆盖类型双关返回 None |
| 性能：按字符串 dispatch 比静态字段慢 | 不在热路径（生成 chunk 用 `sample()` 一次性读全部），按名查询仅在 plan-botany / mineral 物种 spawn 谓词，频率低 |
| 未来加新 layer | LAYER_SCHEMAS 静态表 + Python LAYER_REGISTRY 同步加一行；CI 比对单测会立刻撞红 |

---

## §7 开放问题

- [ ] `LayerSchema` 是否要把 `blend_mode` 也镜像过来？（目前 Rust 侧不需要 blend_mode，仅在 stitcher Python 侧用；建议**不镜像**，避免维护两份）
- [ ] 是否提供 `sample_all_layers(x, z) -> HashMap<&'static str, LayerValue>` 批量接口？（YAGNI——目前 botany / mineral 都是按需 1–3 个 layer 查；推迟）

---

## §8 进度日志

- **2026-04-29**：立项。来源：plan-botany-v2 §9 自报"sample_layer 接口缺口约 80–150 行 Rust"——审计 `server/src/world/terrain/raster.rs` 后确认底层 mmap + ColumnSample 28 字段全部就绪，实际工作量降为 thin adapter。本 plan 直接 active，期望 1 个 PR 收口。

---

## Finish Evidence

<!-- 全部阶段 ✅ 后填以下小节，迁入 docs/finished_plans/ 前必填 -->

- 落地清单：
- 关键 commit：
- 测试结果：
- 跨仓库核验：
- 遗留 / 后续：
