# plan-bughunt-sodium-chunk-resync-lifecycle-v1

> 来源 Issue：#1713。Sodium 重同步在第一次“当前没有缺口”扫描后永久停止，且换维度时 ClientWorld 不经过 null，旧状态没有复位。
>
> 阶段总览：P0 ⬜ 绑定 ClientWorld 生命周期并持续覆盖渐进 chunk；P1 ⬜ 验证换维度、延迟到达和反射失败路径。

## §0 摘要

SodiumChunkReload 只在 client.world == null 时把 fullySynced 置回 false；普通换维度会直接替换 ClientWorld 对象，不保证经过 null。resyncMissingChunks 扫描 256×256 范围，若本次 synced==0 就在 :130 将 fullySynced=true。Valence 仍可能按渐进视距在随后 tick 送来 chunk，新 chunk 永远不会再触发补发。

## §1 实际游玩体验影响

- 进入新维度或视距逐步扩大时，部分 chunk 可能已存在于 vanilla ClientChunkManager，却没有进入 Sodium ChunkTracker，表现为局部虚空或不可渲染。
- 第一次扫描恰好发生在增量 chunk 到达前时，问题会永久保留到下一次完整 world unload。

## §2 复现路径

1. 进入世界，前 20 tick 扫描时只看到当前已有 chunk，resyncMissingChunks 返回 synced=0。
2. fullySynced 变为 true，后续 END_CLIENT_TICK 不再调用 resyncMissingChunks。
3. Valence 继续发送渐进 chunk；或直接把 client.world 替换为新维度但不经过 null。
4. 新 world 的 chunkTracker 没有被扫描，观察 Sodium 渲染缺口。

## §3 根因证据

- client/src/main/java/com/bong/client/compat/SodiumChunkReload.java:43-47 保存 fullySynced、ticksSinceLastSync 和 failure 状态；:62-75 仅在 world==null 时复位并以 fullySynced 提前返回。
- :94-132 扫描已存在的 WorldChunk；:126-131 在 synced==0 时永久设置 fullySynced=true，没有等待 server 增量流静默。
- :114-123 的范围扫描不包含未来尚未到达的 chunk，因此“当前无缺口”不能推出 view 已完成。
- server/src/world/terrain/mod.rs:584-648 的 resync_view_after_join 先发 ChunkRenderDistanceCenterS2c，再按 layer remove/insert 产生 LOAD 消息；这是持续的增量输入，不是一次性快照。

## §4 非重复比对

- server 的 join resync 已保证中心包先于重灌 chunk；本 plan 不重复修改该 server 顺序，只修 client 对增量流和 ClientWorld identity 的消费。
- 其他 Sodium 兼容文档处理反射版本诊断；本 issue 的可观察故障是生命周期/终止条件，不是反射异常。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“维度、世界切换、区块、视距、虚空、地形”，核对世界/区域与探索章节；没有把渲染同步视为一次扫描即可结束的规则。
- docs/finished_plans/：检索 SodiumChunkReload、ChunkTrackerHolder、resync_view_after_join、fullySynced；命中 server/client 基础计划，未发现本终止条件的修复。
- active plan（docs/plan-*.md）：检索 fullySynced、ClientWorld、Sodium、chunk resync；未发现 active plan 负责此状态机。
- docs/plans-skeleton/：检索 SodiumChunkReload、chunk tracker、换维度；未发现同主题骨架。
- docs/plans-skeleton/reminder.md：检索 Sodium、chunk resync、fullySynced；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：Minecraft ClientWorld/ClientChunkManager、Sodium ChunkTrackerHolder、Valence 的中心包与增量 chunk LOAD 流。
- **Outputs**：client Sodium tracker 的补发调用、world identity 复位和有界的轮询终止；server chunk 内容与顺序保持不变。
- **共享类型 / 事件**：ClientWorld、WorldChunk、ChunkRenderDistanceCenterS2c、ChunkLayer、ChunkTrackerHolder.onChunkStatusAdded。
- **server 契约符号**：resync_view_after_join、chunk_positions_needing_resend、ChunkRenderDistanceCenterS2c、ChunkLayer::remove_chunk/insert_chunk。
- **agent**：无变更；区块包是 vanilla/Valence 连接层数据，不经过 agent IPC 或 TypeBox。
- **client**：有变更；SodiumChunkReload.register、resyncMissingChunks、fullySynced/lastWorld 状态。
- **worldview 锚点**：docs/worldview.md 的区域/维度探索和地形可见性章节；不改地形生成或世界观 zone。
- **qi_physics**：无真元流动；只修渲染缓存，不得重放会改变 gameplay 的 server 事件。

## §5 修复骨架

### P0 绑定 world identity，禁止过早永久停止

- 保存上一次 ClientWorld 引用或稳定 world identity；检测对象变化时复位 fullySynced、计时器和失败熔断状态，即使旧 world 从未变成 null。
- 将“本次没有缺口”改为“在 view 稳定且连续若干次扫描没有缺口”或持续有界轮询；必须覆盖随后到达的增量 chunk，不能在第一场空扫描后永久退出。
- 反射失败仍按 Sodium 版本和阈值熔断，但新 world 不能沿用旧 world 的 fullySynced 结果。

### P1 回归与验收

- world identity 替换、world==null 后重连、渐进 chunk 在第一次扫描后到达三条路径都能触发补发。
- 已同步 world 在稳定窗口后停止高成本扫描；新 chunk 或新 world 能重新打开窗口。
- server join resync 的 center-before-load 顺序和 agent 无变更证据保持在 plan 验收记录中。
