# plan-bughunt-client-local-render-identity-v1

> 来源 Issues：#1479、#1574。server 只把本地玩家的变异/背包状态发给该 client，但 client feature renderer 把全局快照应用到所有玩家实体。
>
> 阶段总览：P0 ⬜ 给本地状态 renderer 加实体身份门；P1 ⬜ 验证 mutation 与 worn-pack 两条渲染链互不污染。

## §0 摘要

MutationFeatureRenderer 注册给所有 PlayerEntityRenderer，却直接读取全局 MutationVisualState.activeSlots；WornPackFeatureRenderer 同样直接读取 InventoryStateStore.snapshot。server 的 mutation_visual_emit_system 和 inventory_snapshot_emit 都按所属实体只给对应 client 发 payload，所以远端玩家没有自己的快照可供这些 renderer 使用。缺少 entity/local-player 判断使本地玩家的变异贴图和穿戴背包被画到每个远端玩家身上。

## §1 实际游玩体验影响

- 第一人称/第三人称看似只是自己装备了背包或发生异化，但旁观其他玩家时，所有玩家模型都显示相同外观。
- 远端实体的实际 server 状态没有改变；这是 client renderer 选择目标错误，不能通过把 server payload 广播给所有玩家来“补齐”。

## §2 复现路径

1. 登录玩家 A，收到仅发给 A 的 bong:mutation_visual 和 inventory_snapshot，两个全局 store 有本地快照。
2. 在同一画面生成玩家 B、C；MutationRenderBootstrap/WornPackRenderBootstrap 将 feature renderer 注册给所有玩家 renderer。
3. 渲染每个 entity 时，MutationFeatureRenderer:66 或 WornPackFeatureRenderer:92 读取相同全局 store，没有比较 entity 身份。
4. A 的 slots/胸口背包被重复画在 B、C 身上。

## §3 根因证据

- client/src/main/java/com/bong/client/dandao/MutationRenderBootstrap.java:22-29 和 client/src/main/java/com/bong/client/armor/WornPackRenderBootstrap.java:21-28 都把 feature renderer 注册到所有 PlayerEntityRenderer。
- client/src/main/java/com/bong/client/dandao/MutationFeatureRenderer.java:53-80 直接读取 MutationVisualState.activeSlots，没有检查传入 entity；client/src/main/java/com/bong/client/armor/WornPackFeatureRenderer.java:79-99 直接读取 InventoryStateStore.snapshot。
- client/src/main/java/com/bong/client/dandao/MutationVisualState.java:10-29 与 client/src/main/java/com/bong/client/inventory/state/InventoryStateStore.java:14-54 都是本地全局快照，没有 renderer target 字段。
- server/src/network/mutation_visual_emit.rs:18-66 只对 event.entity 查询并向该实体的 client 发送 bong:mutation_visual；server/src/dandao/visual_sync.rs:24-37 payload 还带 entity 字段。
- server/src/network/inventory_snapshot_emit.rs:45-60、:93-145 按单个 client entity 发送 inventory_snapshot；agent/packages/schema/src/inventory.ts:341-368 定义的是该 client 的完整 inventory snapshot，没有“他人背包广播”语义。
- client/src/main/java/com/bong/client/dandao/MutationPayloadHandler.java:27-45 读取 slots 但丢弃 payload.entity，进一步无法在渲染时核验目标。

## §4 非重复比对

- #1479 与 #1574 共享“本地全局状态套到所有实体”的根因，合并在一份骨架；mutation 数据和 inventory 数据仍分别保留各自的 handler/store。
- 不与网络线程、payload session 隔离或 server 广播问题合并；server 已经按实体定向发送，修复重点是 renderer identity predicate。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“异化、伪皮、背包、装备、身体、身份、玩家”，核对身体/装备与身份相关章节；没有允许把本地外观复制到远端实体的规则。
- docs/finished_plans/：检索 MutationFeatureRenderer、WornPackFeatureRenderer、MutationVisualSyncPayload、InventorySnapshotV1；命中 dandao/backpack 基础 plan，未发现 renderer 身份门修复。
- active plan（docs/plan-*.md）：检索 mutation_visual、InventoryStateStore、PlayerEntityRenderer、local player predicate；未发现 active plan 拥有这两个 renderer 的共享门。
- docs/plans-skeleton/：检索上述符号及“所有玩家/实体过滤”；未发现同主题骨架。
- docs/plans-skeleton/reminder.md：检索异化贴图、背包渲染、实体身份；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：本地玩家身份、传入 PlayerEntity/AbstractClientPlayerEntity、MutationVisualSyncPayload.entity、InventorySnapshotV1 及其 revision。
- **Outputs**：只有本地玩家实体应用本地 mutation/inventory 视觉；远端实体直接跳过，server payload 定向行为不变。
- **共享类型 / 事件**：bong:mutation_visual、MutationVisualSyncPayload、MutationVisualState、inventory_snapshot、InventorySnapshotV1、InventoryStateStore。
- **server 契约符号**：mutation_visual_emit_system、MutationVisualSyncPayload::entity、send_inventory_snapshot_to_client、build_inventory_snapshot。
- **agent**：无变更；inventory.ts 只描述发给当前 client 的权威快照，mutation_visual 是 server/client CustomPayload，不经过 agent IPC。
- **client**：有变更；MutationPayloadHandler 应保留/核验 entity，MutationFeatureRenderer 与 WornPackFeatureRenderer 使用同一 local-player predicate。
- **worldview 锚点**：docs/worldview.md 的身份、身体异化、装备/背包章节；视觉只反映当前玩家自身状态。
- **qi_physics**：无真元计算；背包快照中的 qi_current/qi_max 仍只读展示，renderer 不得据此修改状态。

## §5 修复骨架

### P0 加实体身份门

- 在 renderer 入口先判断传入 entity 是否是 MinecraftClient 当前本地玩家；远端实体直接 return，不能仅凭“store 非空”渲染。
- mutation payload 的 entity 字段应在 handler/store 中保留或核验，若使用 entity wire id 必须与 client 当前玩家的稳定身份映射一致；inventory snapshot 因 server 已按 client 定向发送，可直接使用本地玩家 predicate。
- 两条 renderer 复用一个可测试的 local-player predicate，避免一条比较 UUID、一条比较对象引用而产生不同边界。

### P1 回归与验收

- 本地玩家有 mutation/背包快照时渲染，本地玩家换装/清空快照后立即消失。
- 远端玩家、NPC 或 client.player 未初始化时不渲染；断线清理 store 后不残留旧模型。
- 不增加 server 广播、不改 agent schema；payload entity/revision 的既有校验继续有效。
