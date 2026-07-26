# plan-tribulation-concurrent-broadcast-clobber-v1（骨架）

> **骨架（草案）**。一句话主题：`tribulation` 观战主链把**多场同时进行的天劫**压成了客户端单槽 `last-write-wins` 状态；server 侧明明按 `Entity` 维护多路活跃 broadcast/state，但 client `TribulationBroadcastStore` / `TribulationStateStore` 只保留最后一条，且任意一场 `settled` 还会向所有玩家广播 `clear()`。影响是：**满世界灵气预算下允许 2 个化虚名额并发时，玩家只能看到最后一场天劫；其中一场结束时，另一场仍在进行的坐标/方位/距离/波次 HUD 会被一起抹掉**。

> 立项动机：`plan-tribulation-v1` 明确把“全服广播 + 地点公开 + 观战/截胡 HUD”定义成正式玩法闭环；当前实现却只在 server 端支持多路活跃，在 client 端退化成单实例缓存。这不是抽象架构瑕疵，而是会直接误导跑路、围观和截胡决策的实机 bug，值得先立 skeleton 收口证据与修复面。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 并发渡劫 broadcast/state 串台与误清空 | fix_pr | ✅ 2026-07-26 |

## P0 — 并发渡劫 broadcast/state 串台与误清空

- **并发可达性不是假设**：`server/src/cultivation/tribulation.rs:115-123` 把默认满灵气预算的化虚名额锁成 `quota_limit=2`；`docs/finished_plans/plan-tribulation-v1:14-15,44-46,63,98-100` 又把“全服广播 + 地点公开 + 观战/截胡 HUD + 100 格危险范围”定义成正式玩法。也就是说，**至少 2 场 DuXu 并发**本来就是运营目标内的正常路径；再叠加 `JueBi`/`ZoneCollapse`/`Targeted`，运行时出现多场活跃天劫并不罕见。
- **server 侧已经按多实例建模**：`server/src/network/tribulation_broadcast_emit.rs:57-60` 用 `Local<HashMap<Entity, ActiveTribulationBroadcast>>` 持有活跃广播；`announce/juebi/locked/cleared` 都是按 `ev.entity` 更新并 `broadcast(&mut clients, data.clone())`。`server/src/network/tribulation_state_emit.rs:21-22,116-120` 同样持有 `Query<(&TribulationState, ...)>`，新 client join 时会把**每个活跃 state** 逐条广播一遍。
- **client 侧却只有单槽**：`client/.../TribulationBroadcastStore.java:4-6,31-41` 注释直接写明 “A single broadcast is held at a time; last-write-wins”；`client/.../TribulationStateStore.java:61-73` 也是单个 `snapshot`。对应 handler `client/.../TribulationBroadcastHandler.java:25-39` 与 `client/.../TribulationStateHandler.java:20-44` 每来一条就 `replace(...)` 覆盖上一条，没有按 `char_id` / `entity` / 坐标做分桶，也没有“我附近优先”筛选。
- **因此会出现两类实机故障**：
  1. **并发串台**：A、B 两场天劫同时活跃时，后到的 payload 会把先到的 HUD 完整覆盖。玩家若正赶往 A，顶栏却可能突然跳成 B 的名字、坐标、方位、距离和波次。
  2. **误清空剩余天劫**：`server/src/network/tribulation_broadcast_emit.rs:118-120` 在任意 `TribulationSettled` 时直接 `broadcast(..., TribulationBroadcastV1::clear())` 给所有 client；client handler 再无条件 `clear()`。若 A/B 并发，A 先结算，B 仍在进行时，**所有玩家的 tribulation broadcast 会被清空**，直到 B 后续恰好再触发一次新事件才可能补回来；而 `tribulation_state` 也只会保留最后一次到达的单条快照。
- **为什么这不是“UI 简化可接受”**：`plan-tribulation-v1` 把坐标公开、方位/距离提示、观战/跑路选择都定为核心玩法，不是装饰层。单槽覆盖会让附近观战者误判哪一场在 100 格危险圈内，误清空则会让仍在持续的雷劫失去公共坐标与顶部告警，直接损害“赶去截胡 / 远离避雷”的决策质量。
- **对实际游玩体验的影响**：当两名修士同时渡虚，或一场 DuXu 与一场 JueBi/Targeted 叠在同一时间窗，玩家客户端只会看到最后刷到的那一场；若另一场先结算，还会把剩余天劫的红幅广播一起抹掉。结果是：**观战者可能朝错坐标赶路、路过玩家失去仍然生效的危险提示、截胡者看不到正确波次与方向信息，整条“公开消息 → 赶路判断 → 观战/避难”玩法链被打断**。
- **建议修复范围 / 模块**：优先收口 `server/src/network/tribulation_broadcast_emit.rs`、`server/src/network/tribulation_state_emit.rs`、`client/.../TribulationBroadcastStore.java`、`client/.../TribulationStateStore.java`、`client/.../TribulationBroadcastHudPlanner.java`。方向需要一次性拍板：要么 client 改成多实例 store 并按“最近 / 最危险 / 正在观战对象”选主显示，要么 server 就只能合法保证全局永远单活跃并把 quota/事件链同步收紧；**当前“server 多活跃 + client 单槽”是自相矛盾的半接线状态**。
- **验收抓手**：至少补 4 组 pin。1) 两场活跃 DuXu 并发时，新 client join 不应只剩最后一条。2) A/B 并发且 A settle 后，B 的 broadcast/state 仍应持续可见。3) 同 tick 连续收到两条 tribulation payload 时，HUD 选择逻辑必须稳定且可解释（不是纯到达顺序）。4) 50/100 格附近的观战提示与危险判断要与“当前展示的那场天劫”保持一致，不出现方位/距离指向错场。

## 反方裁决摘要

1. **Round 1 反方主张**：也许产品就只想同时展示一场天劫，client 单槽是刻意设计。  
   **裁决**：驳回。server 已显式维护 `HashMap<Entity, ActiveTribulationBroadcast>`，`tribulation_state_emit` 也会在 join 时逐条重放所有活跃 state；如果设计真是“全局只允许一场”，这些多实例结构、quota=2 以及逐条广播都不该存在。
2. **Round 2 反方主张**：即便并发存在，任一 `settled` 的 `clear()` 也许很快会被另一场后续事件刷新，玩家体感不明显。  
   **裁决**：驳回。`clear()` 是立刻对所有 client 生效的无条件全局清空，而另一场是否“恰好马上再发事件”并无保证；在 `locked` 后到下一波前的空窗里，剩余天劫会真实失去顶栏告警与坐标指引，这正好打在玩家做跑路/观战决策的时间窗上。

## 开放问题

1. 客户端最终应该支持“多场并发列表 + 主展示择优”，还是只允许主 HUD 展示 1 场、其余降级成事件流/小地图标记？需要在 fix PR 明确 UX。
2. `TribulationBroadcast` 与 `TribulationState` 是否都应按同一 key（`char_id` 或 runtime entity id）维护；若用 `char_id`，JueBi/域崩这类非玩家源事件要定义稳定 key。

## 审计来源

bug-hunt 定点轮（仅收窄 `tribulation/omen` 主路径与其 client HUD bridge）。主代理人工复核了 `quota_limit=2` 的并发前提、server 多实例状态结构、client 单槽 `last-write-wins` 存储、以及 `settled -> clear()` 的全局副作用后保留该候选。当前结论是 **report-only**：先提交 skeleton plan，把玩法影响、根因路径、修复面与验收抓手固定，再由后续 fix PR 单独实现。

## 验证结论（2026-07-26 整理审计追认）

client `TribulationBroadcastStore.java` 已改为多实例 keyed Map + 优先级择主显示，不再是单槽 last-write-wins；server 侧 `server/src/network/tribulation_broadcast_emit.rs:130-142` 的 `settled` 清空逻辑已改为按 entity 精确清除，只有 `active_broadcasts` 空时才全局 clear，堵住了误清空剩余天劫的问题。修复主 commit 0ac5be04e 加竞态补丁 33c635316（2026-07-06，PR #968）已合入 origin/main。

## Finish Evidence

- **落地清单**：`client/src/main/java/com/bong/client/.../TribulationBroadcastStore.java`（多实例 keyed Map + 优先级择主显示）、`server/src/network/tribulation_broadcast_emit.rs:130-142`（settled 按 entity 精确清除，`active_broadcasts` 空才全局 clear）
- **关键 commit**：0ac5be04e + 竞态补丁 33c635316（2026-07-06，修复并发渡劫 broadcast/state 串台与误清空，PR #968 已 merge）
- **测试结果**：2026-07-26 审计为只读核验（Read+grep+git log 对拍 origin/main），未重跑测试套件
- **跨仓库核验**：server（`tribulation_broadcast_emit.rs`）+ client（`TribulationBroadcastStore.java`）
- **遗留 / 后续**：无
