# plan-bughunt-spirittreasure-affinity-desync-v1

> 一句话主题：`spirit_treasure_dialogue` 正常回包会立刻改掉 server 侧 `SpiritTreasureRegistry.active.affinity/sleeping`，但被动重算、`spirit_treasure_state` 推送、client `SpiritTreasureStateStore` 刷新全都只挂在 `Changed<PlayerInventory>` / `Changed<ActiveSpiritTreasures>` 上，导致**器灵已经沉睡或好感已变化，玩家身上的灵宝被动和灵宝面板仍长期停留在旧值**。这不是纯 UI 小毛刺，而是 server 实际数值、聊天交互门控、client 展示三条链同时分叉。

> 立项动机：本轮 bughunt 聚焦 `server/src/spiritwood` / `artifact|spirit_treasure` / 相关 client 展示与交互链路；该问题落在灵宝主链、玩家可正常触发、且不与近期 craft/social renown/tribulation/botany/dying elder/pseudo vein restart loss 题重复。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 灵宝对话改 affinity 后，被动/状态/UI 不刷新 | fix_pr | ⬜ |

## P0 — 灵宝对话改 affinity 后，被动/状态/UI 不刷新

- **复现路径（正常玩法）**：
  1. 按 `server/src/inventory/spirit_treasure.rs:332-367` 的正式路径拿到 `spirit_treasure_jizhaojing`，并通过触发位激活它，使 `passive_active=true`、玩家身上挂上 `SpiritTreasurePerception / MirrorConcealment / MirrorExposed`。
  2. 在聊天里走 `@寂照镜 ...`。`server/src/network/chat_collector.rs:303-383` 会把持有中的灵宝 mention 路由到器灵对话；该逻辑读取 registry 中的 `sleeping` / `affinity`，并把当前 affinity 塞进请求上下文。
  3. 让 agent 回一条带 `affinity_delta` 的正常 `SpiritTreasureDialogueV1`。`server/src/network/spirit_treasure_emit.rs:42-98` 在处理回包时先 `registry.apply_affinity_delta(...)`，即刻修改 server 侧 affinity 与 `sleeping`。
  4. 此时不要切包、不要重新装备、不要改 inventory。继续观察：后续 `@寂照镜` 已经会受到新的 `sleeping` 门控，但灵宝被动强度、`SpiritTreasureState` 面板里的“器灵清醒/沉睡”和“好感 xx%”不会同步变化，直到玩家再次改动 inventory、重连或触发别的重扫。
- **更强的确定性复现**：如果当前 affinity 恰好在 `0.21~0.30`，一条 `affinity_delta=-0.1` 的回包就能把它压到 `<=0.2`。之后 `chat_collector.rs:328-345` 会直接回“器灵仍在沉睡”，但玩家此前激活出的灵宝被动依然留在身上，因为 server 从未重跑 `sync_passive_status_effects`。
- **根因链路**：
  - `server/src/inventory/spirit_treasure.rs:134-139` 的 `apply_affinity_delta` 只改 `registry.active[*].affinity / sleeping / dialogue_count`。
  - 真正负责重算被动并写回 `StatusEffects` 的只有 `sync_spirit_treasures`，而它的 query filter 是 `Changed<PlayerInventory>`（`server/src/inventory/spirit_treasure.rs:188-228`）。只聊器灵不会改 inventory，所以不会进入这条系统。
  - `spirit_treasure_state` 推送也只看 `Added<ActiveSpiritTreasures> | Changed<ActiveSpiritTreasures>`（`server/src/network/spirit_treasure_emit.rs:17-39`）。affinity 变化不改 component，自然不会再次发包。
  - `status_snapshot` 也只在 `Changed<StatusEffects>` 时发（`server/src/network/status_snapshot_emit.rs:15-66`）。由于上一条没重算 status，这里同样不会刷新。
  - client 侧 `SpiritTreasureDialogueHandler` 只把对话塞进 `SpiritTreasureDialogueStore`（`client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureDialogueHandler.java:15-35`）；真正承载 affinity/sleeping 的 `SpiritTreasureStateStore` 只能由 `SpiritTreasureStateHandler` 的 `spirit_treasure_state` payload 覆盖（`.../SpiritTreasureStateHandler.java:19-39`、`.../SpiritTreasureStateStore.java:14-33`）。server 不重发 state，client 就永远拿旧快照。
  - `JiZhaoJingTabPanel` 渲染“器灵清醒/沉睡”“好感 xx%”和镜面亮度时，全部直接读 `SpiritTreasureState`（`client/src/main/java/com/bong/client/spirittreasure/JiZhaoJingTabPanel.java:13-38`），所以 UI 会稳定显示旧值。
- **为什么这是 bug，不是设计**：
  - `chat_collector` 已经把 affinity/sleeping 作为对话冷却与可交互性的真实 server 判定（`chat_collector.rs:328-345`），说明这不是“仅供叙事参考”的字段。
  - `sync_passive_status_effects` 又明确用 `registry.affinity_scale(...)` 缩放灵宝被动（`server/src/inventory/spirit_treasure.rs:428-452`），说明 affinity 本来就应该影响实际数值，不是纯展示字段。
  - 现在只有“对话门控”读取了新 affinity，而“被动效果”和“client 状态面板”都卡在旧 affinity，属于同一状态源被拆成三份不一致的真实行为分叉。

## 这个 bug 对实际游玩体验的影响

- 玩家与寂照镜互动后，会出现**器灵已经翻脸沉睡，但被动还在生效；或者好感已经回升，但感知/匿探强度仍停在旧档**的割裂体验。
- 更糟的是，玩家下一句再 `@寂照镜` 时会被 server 明确提示“镜面无光，器灵仍在沉睡”，但灵宝面板仍可能显示“器灵清醒 / 好感 25%”，等于 client UI 公开撒谎。
- 这条 bug 还会误导平衡判断：测试者会以为“对话系统对被动强度有影响”，但实际上当前 build 里影响只落到了 registry 和聊天门控，**没有落到正在战斗/探索时真正吃到的 status 数值**。

## 修复建议

- 在 `SpiritTreasureRegistry` affinity 变化时显式触发一次“灵宝状态脏标记”或 event，而不是依赖 `Changed<PlayerInventory>` 顺带重扫。
- server 至少要补两件事：
  1. 对所有 `passive_active` 的灵宝重跑 `sync_passive_status_effects`，让 `StatusEffects` 与新 affinity/sleeping 对齐。
  2. 对持有该灵宝的 client 主动重发 `spirit_treasure_state`，让 `SpiritTreasureStateStore` 刷新。
- 更稳妥的方向是把“inventory 扫描”和“registry affinity/sleeping 变化后的重算/发包”拆成两条系统，避免把运行时状态同步绑死在 inventory 变更上。
- 验收至少覆盖 4 组：
  1. 激活中的灵宝在 `affinity_delta` 后，server `StatusEffects` magnitude 必须同 tick 更新。
  2. affinity 掉到 `<=0.2` 时，被动必须撤销或降到设计预期，且聊天门控与状态效果一致。
  3. `spirit_treasure_dialogue` 后无需改背包，client 灵宝面板应看到新的 affinity/sleeping。
  4. 回升 affinity 后，client 面板和 status snapshot 不得继续停留在旧档。

## 反方裁决摘要

- **说明**：当前会话未提供可再开 subagent 的委派能力，因此这里如实采用“主代理两轮反方裁决”的退化处理；没有伪造外部审稿结论。
- **Round 1 反方论点**：也许 `Res<SpiritTreasureRegistry>` 变化会让 `emit_spirit_treasure_state_payloads` 因为读了 registry 而自动重跑全部 client。
  - **驳回理由**：`emit_spirit_treasure_state_payloads` 的 query filter 明确是 `With<Client> + Or<(Added<ActiveSpiritTreasures>, Changed<ActiveSpiritTreasures>)>`，是否遍历 client 由 component 变化决定，不由 `registry` 变化决定；函数体里也没有“broadcast on registry changed”的额外分支。
- **Round 2 反方论点**：就算 state 不刷新，也许 dialogue handler 会顺手把 affinity_delta 合并进 `SpiritTreasureStateStore`，所以只是 server 内部暂态延迟。
  - **驳回理由**：`SpiritTreasureDialogueHandler.java:23-35` 只 `SpiritTreasureDialogueStore.append(dialogue)`；`SpiritTreasureStateStore` 的唯一写入口是 `replace(...)`，且只在 `SpiritTreasureStateHandler` 收到 `spirit_treasure_state` 时调用。代码上不存在任何 dialogue→state 的补丁路径。

## 开放问题

1. affinity 掉到 `sleeping=true` 时，被动是应当立刻完全撤销，还是只按低 affinity 缩放到很弱但非零？现有设计意图需要在 fix PR 中定清。
2. `SpiritTreasureStatePayloadV1.passive_effects` 目前发的是定义值而非缩放后的实时值；修这次不同步问题时，是否顺手把 wire payload 也改成“已缩放值 + 原始说明”以减少 client 推导歧义。

## 审计来源

bughunt 定点轮（范围仅限 `artifact|spirit_treasure` 相关 server/client 主链）。候选经主代理全树 grep、server/client 对照、两轮手工反方裁决后保留；当前结论是 **report-only**，先立 skeleton 固化复现与根因，不在本 PR 修代码。
