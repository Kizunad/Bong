# plan-bughunt-raster-chunk-persistence-v1

> Skeleton plan。只读审计产物，来源 issue #1844。

## §0 摘要

Raster overworld 由 `spawn_raster_world` 创建普通 `LayerBundle`，每次视野需要时 `ensure_chunk_generated` 从 raster 重新生成 `UnloadedChunk`。`remove_unviewed_chunks` 会逐出不在玩家视野内的 chunk，`GeneratedChunks.loaded` 只是内存集合；玩家通过 `block_place`/破坏写入的方块没有 Anvil 或其他持久化 overlay。chunk 再次生成时，玩家建筑和破坏状态会丢失。

## §1 游玩影响

玩家离开视野、传送或重启后，自己放置的家具/方块与挖空区域会恢复成 raster 本底。建筑无法成为可靠的基地，破坏/采集结果也会回滚，尤其影响灵龛、容器和长期资源规划。

## §2 复现路径

1. 选择 raster overworld，进入一个已生成 chunk，在 `block_place` 放置方块或挖掉一个方块。
2. 让所有玩家离开该 chunk，`remove_unviewed_chunks` 从 `ChunkLayer` 删除它并从 `GeneratedChunks.loaded` 移除。
3. 玩家返回，`generate_chunks_around_players` 调 `ensure_chunk_generated`，从 `TerrainProvider::sample` 与 decoration/structure 重新填充 chunk。
4. 观察先前的玩家修改没有任何持久化 overlay，最终状态回到 raster 生成结果。

## §3 今天 `origin/main` 证据

- `server/src/world/terrain/mod.rs:767-817`：`spawn_raster_world` 只 spawn `LayerBundle`、`OverworldLayer` 和 provider resource，没有 `AnvilLevel`/chunk storage。
- `server/src/world/terrain/mod.rs:900-948`：`remove_unviewed_chunks` 对不在 viewer 范围的 chunk 调 `retain_chunks` 删除，并同步移除 `GeneratedChunks.loaded`。
- `server/src/world/terrain/mod.rs:955-1010`：`ensure_chunk_generated` 在 `generated` 或 layer 已有 chunk 时 early return，否则从 raster sample、装饰、结构重新构造并插入。
- `server/src/world/block_place.rs:217-249`：合法放置消费背包实例后直接写入当前 `ChunkLayer`/placeable registry，没有写入 raster overlay 或持久层。
- `server/src/world/block_break.rs:45-64`：默认破坏直接 `layer.set_block(..., AIR)` 并移除 furniture registry，同样没有持久化记录。
- 对照 `server/src/world/mod.rs:535-550`：Anvil bootstrap 明确创建 `AnvilLevel`；raster bootstrap 没有等价组件。

## §4 非重复比对

- `plan-worldgen-v3*` 与 raster bootstrap 处理地形生成/装饰，不承诺玩家运行时方块修改的持久化。
- `plan-bughunt-tsy-start-raster-env-gap-v1.md` 处理 TSY raster 环境启动，不覆盖 overworld chunk eviction。
- 家具掉落骨架只处理破坏后的物品返还；本 issue 是修改本身在 chunk 重载后丢失，两个问题可独立验收。

## §5 立项检查记录

- **worldview**：查 `基地`、`灵龛`、`地形`、`方块`、`资源`；命中 `docs/worldview.md §十一.安全空间`、§十三世界地理，玩家建筑/灵龛必须是稳定世界状态。
- **finished_plans**：查 `spawn_raster_world`、`GeneratedChunks`、`AnvilLevel`、`block_place`；已有 world bootstrap 与放置 plan，未找到 raster overlay 持久化。
- **active plan**：查 `remove_unviewed_chunks`、`ensure_chunk_generated`、`ChunkLayer`；未见 active plan 接管玩家方块变更存储。
- **skeleton**：查 `raster chunk persistence`、`AnvilLevel`、`GeneratedChunks.loaded`；无同根因骨架。
- **reminder.md**：查 `raster`、`chunk`、`建筑`、`持久化`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`TerrainProvider` raster 本底、`ChunkPos`、`ChunkLayer` 当前方块、`block_place`/`block_break` 修改、viewer eviction/reload 生命周期。
- **Outputs**：重载 chunk 时合并本底与玩家 overlay，继续通过 Valence vanilla chunk/block update 发送给客户端。
- **共享类型或事件**：复用 `ChunkLayer`、`ChunkPos`、`GeneratedChunks`、`DimensionLayers`、`BlockPlaceRequest`/`DiggingEvent`；持久化格式需与 world bootstrap 生命周期明确绑定。
- **server 符号**：`world::terrain::{spawn_raster_world,generate_chunks_around_players,remove_unviewed_chunks,ensure_chunk_generated}`、`world::{block_place,block_break}`、`AnvilLevel` 对照实现。
- **agent**：无变更；agent 不拥有方块世界状态，也没有 raster/chunk persistence schema。
- **client**：无变更；客户端继续接收标准 chunk/block update，修复在 server 生成/存储层完成，不改自定义 payload。
- **worldview 锚点**：`docs/worldview.md §十一` 灵龛安全空间与 §十三地理/地形持续性。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 为 raster chunk 引入持久化 overlay 或 Anvil 等价 backing，并定义生成/修改/逐出顺序 |
| P1 | ⬜ | 重载、重启、跨维度和损坏恢复测试，证明本底不会覆盖玩家修改 |

## P0：本底与 overlay 合并

- 玩家方块修改必须在 `ChunkLayer` 写入的同时记录稳定 `DimensionKind + ChunkPos + local block` 变更；重新生成先读本底再应用 overlay。
- eviction 只移除运行时 chunk，不删除持久记录；重启时 overlay 与 raster manifest 版本要有明确兼容策略。
- 结构物/矿脉/家具 registry 的现有生命周期要和 overlay 原子更新，避免方块存在而 registry 丢失或反之。

## P1：可靠性边界

- 覆盖离开视野再返回、服务端重启、同 tick 放置后逐出、破坏后重载、不同 dimension 的 key 隔离。
- 持久层写失败时 fail closed 并保留运行时状态，不能静默把玩家建筑当作已保存。

## 验收测试计划

- 放置/破坏一个方块，逐出并重新生成 chunk 后状态与 registry 均保持。
- 重启后从 raster 本底 + overlay 恢复同一 chunk；未修改 chunk 仍与 raster 快照一致。
- Overworld 与 TSY 使用不同维度 key，不互相覆盖；旧 overlay/manifest 版本按明确迁移或拒绝策略处理。

## 来源 issue

- #1844 `[flash-review][major] raster 世界 chunk 逐出丢玩家建筑(无持久化)`
