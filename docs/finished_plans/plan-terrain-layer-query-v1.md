# Bong · plan-terrain-layer-query-v1

**worldgen 多通道 layer 按名查询接口**。在 `TerrainProvider` 上补 `sample_layer_f32(x, z, layer_name) -> Option<f32>` / `sample_layer_u8` 两条统一查询入口 + `layer_names()` 元数据列表，把现有 `ColumnSample` 各 layer 字段以 dynamic dispatch 暴露出来，供 `plan-botany-v2` P0、`plan-mineral-v2` 共享 layer 接入范式。

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | `sample_layer_f32` / `sample_layer_u8` adapter + `layer_names()` + 饱和单测（每个 layer 名各一条 happy / unknown / out-of-bounds 三档） | ✅ 2026-05-09 |
| P1 | botany-v2 / mineral-v2 调用方接入点声明（仅文档：列出哪几个调用点会从静态字段切到 `sample_layer_*`） | ✅ 2026-05-09 |

**世界观锚点**：无（纯接口适配，不引入新世界观语义）。

**library 锚点**：无（纯工程改造）。

**交叉引用**：
- `plan-botany-v2.md`（active；P0 §9 自报"sample_layer 接口缺口约 80–150 行 Rust"——**实际底层已实装**，仅需 ~30–50 行 thin adapter）
- `plan-mineral-v2.md`（骨架；§"共享 worldgen layer 接入范式"将复用本接口）
- `plan-worldgen-v3.1.md`（已 merged；`LAYER_REGISTRY` Python 侧定义见 `worldgen/scripts/terrain_gen/fields.py`，本 plan 在 Rust 侧暴露）

---

## §-1 前置事实核验（2026-04-29 audit；2026-05-09 消费时重核）

| 事实 | 位置 |
|------|------|
| `ColumnSample` / `TileFields` 已含当前 runtime 需要的 raster 字段 | `server/src/world/terrain/raster.rs`（消费时补齐 `spirit_eye_candidates` / `realm_collapse_mask` / `zongmen_origin_id` / `mineral_density` / `mineral_kind`） |
| `TerrainProvider::sample(x, z) -> ColumnSample` 已实装 | `raster.rs:486` |
| mmap raster 多通道读取已实装 | `raster.rs:555 TileFields::load`（每 layer 一个 `.bin` 文件） |
| `LAYER_REGISTRY` 元数据来源 | `worldgen/scripts/terrain_gen/fields.py:45`（当前 39 个 `LayerSpec`，含 `safe_default / blend_mode / export_type`） |

**结论**：botany-v2 §9 自报的"接口缺口"实质是**缺一个按字符串名查询的薄适配层**——底层 mmap + 字段解析全部就绪。本 plan 工作量约 30–50 行 Rust + 单测，远小于 plan-botany-v2 §9 估算的 80–150 行。

---

## §0 设计轴心

- [x] **不重写底层**：直接在 `TerrainProvider` 上加方法，复用既有 mmap / `sample()` fallback 语义；不做"按需 mmap 单 layer"的微优化（无证据表明热点）
- [x] **f32 / u8 双轨**：worldgen `LAYER_REGISTRY` 区分 `export_type: float32 | uint8`（见 fields.py:42）。Rust 侧也分两条 API，避免类型双关
- [x] **未知 layer 名返回 `None`**（不 panic）：调用方写错 layer 名时降级到 safe_default 路径，不崩 server
- [x] **`layer_names()` 元数据自检**：返回 `&'static [LayerSchema]`，配合单测对照 `LAYER_REGISTRY` 当前 39 条命名（防止 Rust ↔ Python 漂移；single source of truth 是 Python，Rust 只负责镜像）
- [x] **不要在本 plan 改任何 botany / mineral 调用方代码**——只做接口侧；调用方迁移留给 botany-v2 P0 / mineral-v2 自决

---

## §1 接口签名（下游 grep 抓手）

```rust
// server/src/world/terrain/raster.rs

impl TerrainProvider {
    /// 按 layer 名查询 float32 通道；未知名 → None，无 tile → safe_default
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
| 镜像表 39 条 layer schema 数组 | `raster.rs` 文件内 `const LAYER_SCHEMAS: &[LayerSchema] = &[...]` |
| Python 镜像比对单测 fixture | `worldgen/scripts/terrain_gen/fields.py:45 LAYER_REGISTRY`（不动；测试侧 dump 成 JSON 由 Rust 单测读） |

---

## §3 测试饱和（CLAUDE.md "饱和化测试"）

每条 layer × 三档：
- `sample_layer_f32_known_layer_returns_value` × 24 个 f32 layer 名（按 layer 循环逐条断言）
- `sample_layer_u8_known_layer_returns_value` × 15 个 u8 layer 名（按 layer 循环逐条断言）
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

- [x] **P0**（约半天）：
  - 加 `LayerSchema` / `LayerExportType` 类型 + `LAYER_SCHEMAS` 静态表（39 条）
  - 加 `sample_layer_f32` / `sample_layer_u8` / `layer_names` 三个方法
  - 写齐 §3 列出的全部 happy / unknown / type-mismatch / wilderness / 镜像比对单测
  - `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
  - 文档：在 `raster.rs` 顶端加一段注释说明"single source of truth 是 worldgen/.../fields.py"
- [x] **P1**（仅文档）：下游 `plan-botany-v2` / `plan-mineral-v2` 当前已归档；本 plan 遵守 AGENTS docs 写权限，不改其他 finished docs，而在 Finish Evidence 中列明接入现实与后续调用建议

---

## §5 验收

| 阶段 | 验收条件 |
|------|----------|
| P0 | 三个方法 + LayerSchema 镜像表落地；§3 单测全部跑通；与 Python `LAYER_REGISTRY` 名字+类型严格对齐（fixture 比对单测命中）；wilderness 路径返回 safe_default |
| P1 | 记录下游接入现实：`botany-v2` 已通过旧兼容 `sample_layer` 使用 grid layer；`mineral-v2` 已通过 `fossil_bbox` / manifest 接线归档；后续新调用方使用 `sample_layer_f32` / `sample_layer_u8` |

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

- [x] `LayerSchema` 是否要把 `blend_mode` 也镜像过来？决议：不镜像。Rust 侧不消费 `blend_mode`，避免维护两份无用字段。
- [x] 是否提供 `sample_all_layers(x, z) -> HashMap<&'static str, LayerValue>` 批量接口？决议：不提供。当前 botany / mineral 都是按需 1-3 个 layer 查，YAGNI。

---

## §8 进度日志

- **2026-04-29**：立项。来源：plan-botany-v2 §9 自报"sample_layer 接口缺口约 80–150 行 Rust"——审计 `server/src/world/terrain/raster.rs` 后确认底层 mmap + ColumnSample 28 字段全部就绪，实际工作量降为 thin adapter。本 plan 直接 active，期望 1 个 PR 收口。
- **2026-05-09**：consume-plan 落地。重核发现 worldgen `LAYER_REGISTRY` 已扩到 39 条，且 botany-v2 已先行落了旧 `sample_layer` 兼容接口；本 plan 补成 `sample_layer_f32` / `sample_layer_u8` 双轨 API、39 条 schema 元数据和真实 mmap 测试。

---

## Finish Evidence

- 落地清单：
  - P0：`server/src/world/terrain/raster.rs` 新增 `LayerExportType` / `LayerSchema` / `LAYER_SCHEMAS`（39 条，镜像 `worldgen/scripts/terrain_gen/fields.py::LAYER_REGISTRY`）、`TerrainProvider::sample_layer_f32`、`TerrainProvider::sample_layer_u8`、`TerrainProvider::layer_names`。
  - P0：补齐 `TileFields` / `ColumnSample` 对 `spirit_eye_candidates`、`realm_collapse_mask`、`zongmen_origin_id`、`mineral_density`、`mineral_kind` 的读取与 wilderness default。
  - P0：保留旧 `TerrainProvider::sample_layer(...)->Option<f32>` 作为 botany-v2 兼容 adapter，同时新 API 对 f32/u8 类型错配返回 `None`。
  - P0：新增 `server/src/world/terrain/layer_registry_fixture.json`，Rust 单测读取 fixture 对照 schema 名称、类型和 safe_default。
  - P1：下游接入现实已核验：`docs/finished_plans/plan-botany-v2.md` 记录 P0 已使用旧 `TerrainProvider::sample_layer` / `server/src/botany/env_lock.rs::env_sample_layer`；`docs/finished_plans/plan-mineral-v2.md` 记录 `plan-terrain-layer-query-v1` 是 P6 `fossil_bbox` 共享 layer 前置。两者已归档，本 PR 未越权改其他 docs。
- 关键 commit：
  - `b44883da5`（2026-05-09）`terrain: 补齐 layer 按名查询接口`
- 测试结果：
  - `cargo test raster::tests` 通过（9 passed; 3015 filtered out）。
  - `cargo fmt --check` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo test` 通过（3072 passed; 0 failed）。
- 跨仓库核验：
  - server：`TerrainProvider::sample_layer_f32` / `sample_layer_u8` / `layer_names` / `LayerSchema` / `LayerExportType` 均落在 `server/src/world/terrain/raster.rs`；`ColumnSample` / `TileFields` / `wilderness::sample` 已同步新增 layer fallback。
  - worldgen：schema fixture 由 `worldgen/scripts/terrain_gen/fields.py::LAYER_REGISTRY` 当前 39 条导出；Rust 测试断言名称、`export_type`、`safe_default` 对齐。
  - agent/client：本 plan 为 server-side terrain adapter，不新增 IPC schema、Redis key 或 client payload。
- 遗留 / 后续：
  - `LayerSchema` 不镜像 `blend_mode`，因为 Rust runtime 当前不消费该字段；仍以 Python `LAYER_REGISTRY` 为 source of truth。
  - 不提供 `sample_all_layers` 批量接口；当前 botany/mineral 调用场景按需查 1-3 个 layer，YAGNI。
  - 下游已归档 plan 若要把旧 `sample_layer` 文案改成 `sample_layer_f32` / `sample_layer_u8`，应由单独 docs 同步任务处理；本 consume-plan 只改当前 plan。
