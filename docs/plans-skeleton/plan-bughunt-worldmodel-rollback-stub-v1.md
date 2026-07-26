# BugHunt: 天道 WorldModel 失败重试回滚把真实世界快照塌缩成空壳，静默丢失负域越域遥测

## Bug 摘要

**严重度：medium（skeptic 由 high 调整为 medium）**——`WorldModel.toJSON()`/`applySnapshot()` 这套原本为跨重启持久化镜像设计的序列化对被 `runFreshTickWithRollback` 复用做"本 tick 失败回滚"，但它从未真正携带 `latestStateValue.players/zones/npcs`。一旦回滚被触发，`worldModel.latestState` 会被强行塌缩成一个 `players: []` / `zones: []` / `npcs: []` 的空壳，而不是回滚前那一刻的真实世界状态。下一 tick（或同一 tick 重试）读取 `previousState` 做负域（negative domain，`worldview.md` 越域/负域相关章节所指的高压真元区域）越域叙事与遥测计数时，因为"上一 tick 玩家表"永远是空的，所有 `continue` 分支必然命中，导致「失锁」「重新锁定」「淹溺战术」叙事和 `negDomainEscapeEntry/Exit` 配对遥测在该次回滚窗口内被永久静默吞掉。

不涉及真元/灵气流动的凭空增减（不是 qi 守恒律问题），也不影响任何真实 gameplay 判定（不影响渡劫、境界、战斗结算）——影响面确认限于：① 该 tick 窗口内的负域进出叙事文案缺失 ② `negDomainEscapeEntryCount`/`postEscapeRealmDropCount`/`successfulTribulationAvoidanceCount` 这类只喂给 `telemetrySink`（可观测性/仪表盘）的计数器被打断配对、不会回灌进任何 agent 决策 prompt。

## 实际游玩体验影响

玩家在渡劫躲避（跳入负域规避天劫锁定）的那个精确 tick，如果恰好撞上一次 Redis 发布抖动（网络瞬断、Redis 重启，或本项目历史上记录过的 WSL2 loopback 退化导致的"server↔redis 发布 100ms 超时→reconnect livelock"），天道 agent 会：

- 少一句「失去锁定/重新入锁/淹溺战术」的沉浸叙事——玩家在聊天栏看不到这次越域动作被世界"注意到"，破坏沉浸感但不影响实际判定结果。
- 该次越域的 `negDomainEscapeEntry` 记录丢失，导致后续对称的 `negDomainEscapeExit` 因找不到 session 而 no-op——运营侧用来观察"渡劫规避战术"频率的遥测计数出现漏配对，影响的是数据分析仪表盘，不是玩家当下可感知的体验，也不影响任何真实的境界/真元结算。

综合来看，这是一个门槛较高（需要恰好命中一次瞬时 Redis 发布失败）、影响面较窄（仅叙事文案 + 遥测计数）的静默数据丢失，而非核心玩法或经济漏洞。

## 证据定位

- `agent/packages/tiandao/src/world-model.ts:216-238`（`toJSON()`）：只序列化 `currentEra`/`zoneHistory`/`lastDecisions`/`playerFirstSeenTick`/`negDomainPendingTribulations`/`negDomainEscapeTelemetry`/`negDomainEscapeSessions`/`lastTick`/`lastStateTs`，从未包含 `latestStateValue.players/zones/npcs`。
- `agent/packages/tiandao/src/world-model.ts:646-717`（`applySnapshot()`）：`normalizedLastTick !== null` 分支下（L692-715）手搓一个 `latestStateValue`桩：`players: []`、`npcs: []`、`zones: []`、`recent_events: []`（L708-714），与回滚前的真实状态无关。
- `agent/packages/tiandao/src/runtime.ts:716-729`（`runFreshTickWithRollback`）：`const rollbackSnapshot = worldModel.toJSON();`（L721）在 `run()` 之前取快照；`catch` 分支 `worldModel.restoreFromJSON(rollbackSnapshot);`（L726）触发上面的空壳重建。
- `agent/packages/tiandao/src/runtime.ts:492-501`（`runTick` 内）：`renderNegDomainNarrations`/`recordNegDomainEscapeTelemetry` 用 `worldModel?.latestState ?? null` 作为 `previousState`（L492-499），紧接着才 `worldModel?.updateState(state)`（L501）——顺序确认：负域比较发生在"用上一 tick 状态"，一旦上一 tick 状态是回滚出的空壳，比较必然全 miss。
- `agent/packages/tiandao/src/runtime.ts:626-638`：`publishCommands`/`publishNarrations` 调用完全无 `try/catch` 包裹，异常会直接向上抛给 `runFreshTickWithRollback` 的 `catch`。
- `agent/packages/tiandao/src/redis-ipc.ts:909-941`（`publishCommands`/`publishNarrations` 实现）：均为裸 `await this.pub.publish(...)`，ioredis 连接失败/超时会 reject，无内部兜底。
- `agent/packages/tiandao/src/neg-domain-escape.ts:77-119`（`renderNegDomainNarrations`）与 `:121-163`（`recordNegDomainEscapeTelemetry`）：两者都先 `new Map(previousState.players.map(...))`（L87、L132）构造 `previousPlayers`，再对 `args.state.players` 逐个 `previousPlayers.get(player.uuid)`，`!previousPlayer` 时 `continue`（L91-93、L135-137）——`previousState.players = []` 时该 Map 恒为空，全体 `continue`。
- `agent/packages/tiandao/src/world-model.ts:188`（`restoreFromJSON`）→ 内部调用 `applySnapshot`（确认调用链闭合）。
- `agent/packages/tiandao/tests/runtime.test.ts:1920-1980`（`"restores world model history exactly before retrying a failed tick"`）：现有测试用 `FailingPublishRuntimeRedis` 精确复现了"发布失败→回滚→重试"链路，但只断言 `worldModel.getZoneHistory("starter_zone")` 和 `worldModel.lastTick`，从未断言 `latestState.players/zones/npcs` 或负域遥测——证实此缺口此前完全未被覆盖。
- `agent/packages/tiandao/src/telemetry.ts:36` + `runtime.ts:696`（`negDomainEscape: worldModel?.getNegDomainEscapeTelemetrySnapshot()`）：确认 `negDomainEscape` 遥测只喂进 `TickMetrics` → `telemetrySink`，未接回任何 agent 决策 prompt——佐证影响面限定在可观测性层面，不影响 calamity/mutation/era agent 的实际推演输入。

## 触发路径

1. 天道 runtime 进入一个正常 tick，`runTick` 先用 `worldModel?.latestState`（上一 tick 真实状态）计算负域叙事与遥测（runtime.ts:492-499），随后 `updateState(state)` 把 `latestStateValue` 刷新为本 tick 真实状态（L501）。
2. 同一 tick 内继续跑 agent 推演、合并决策，走到 `publishCommands`/`publishNarrations`（L626-638）。
3. Redis 连接在这一刻发生瞬时故障（网络抖动 / Redis 重启 / WSL2 loopback 退化），`this.pub.publish(...)` reject。
4. 异常沿调用栈冒泡到 `runFreshTickWithRollback` 的 `catch`（runtime.ts:725-727），触发 `worldModel.restoreFromJSON(rollbackSnapshot)`。
5. `rollbackSnapshot` 是 tick 开始前 `worldModel.toJSON()` 取的快照，本就不含 `players/zones/npcs`；`applySnapshot` 把 `latestStateValue` 重建成空壳（world-model.ts:698-715）。
6. runtime 重试该 tick（或推进到下一 tick），`renderNegDomainNarrations`/`recordNegDomainEscapeTelemetry` 再次读取 `worldModel.latestState` 作为 `previousState`，命中的是这个空壳——所有玩家在 `previousPlayers` 里查不到，越域/复位判定全部 `continue`，该次窗口内的负域叙事与 `negDomainEscapeEntry`/`Exit` 配对永久丢失（`recordNegDomainEscapeExit` 找不到未建的 session，同样 no-op）。

## 反方审查记录

- 第一轮质疑：
  - 是否只是理论推演、无法在现有测试链路里实证？—— 补证：`tests/runtime.test.ts:1920` 的 `FailingPublishRuntimeRedis` 场景本身就是"发布失败→触发 rollback→重试同一 tick"的真实可执行路径，不是假设；只是该测试没断言 `latestState`/负域遥测，掩盖了此缺口。
  - 是否已有其它 in-flight plan/PR 覆盖同一符号？—— 排查 `dormant-negative-qi-release`/`qi-needle-negative-zone-release` 系列：这些是 `qi_physics` ledger 层面的负域真元衰减守恒问题，模块和故障模式都不同；`docs/plan-bughunt-tiandao-attention-persistence-v1.md` 是 Rust 侧 `TiandaoAttention` 组件跨重连持久化缺口，与本 finding 的 TS 侧 agent runtime 内 tick 级回滚无关；`docs/finished_plans/plan-neg-domain-escape-v1.md` 是该功能原始建设 plan，未提及 `toJSON`/`restoreFromJSON`/rollback/空壳这条链路。判定不重复。
  - 初裁：这是真实、可复现、但影响面需要进一步收窄核实的 bug，倾向通过但需重新评估严重度。
- 第二轮补证（严重度收窄）：
  - 追踪 `negDomainEscapeEntryCount`/`postEscapeRealmDropCount`/`successfulTribulationAvoidanceCount` 全部消费点：仅 `runtime.ts:696` 把 `worldModel?.getNegDomainEscapeTelemetrySnapshot()` 塞进 `TickMetrics.negDomainEscape`（`telemetry.ts:36`），再传给 `telemetrySink.recordTick`——全程未回灌进 calamity/mutation/era 任意一个 agent 的推演 prompt 或决策输入。
  - 结论：原始 finding 措辞"agents 依赖该信号做平衡决策"未被代码证实，实际爆炸半径收窄为「该 tick 窗口的叙事文案缺失 + 运营可观测性遥测计数漂移」，不触及核心玩法判定、不触及 qi 守恒。
  - 终裁：**严重度由 high 下调为 medium**。仍需修复（静默数据丢失属于系统性正确性问题，且触发条件——Redis 瞬时故障——在本项目历史上真实发生过），但不构成 critical/high 级别的玩法或经济破坏。
- 主循环复核：已亲读关键行确认（`world-model.ts:216-238`/`646-717`、`runtime.ts:492-501`/`626-638`/`716-729`、`redis-ipc.ts:909-941`、`neg-domain-escape.ts:77-163`、`tests/runtime.test.ts:1920-1980`、`telemetry.ts:36`），行号与代码内容逐一核对无误，无需修正。

## Skeleton Fix Plan

- [ ] **停止用持久化快照兼职 tick 级回滚**：`WorldModelSnapshot`/`toJSON()`/`applySnapshot()` 这套契约继续只服务"跨重启 Redis 镜像持久化"这一个用途，不再被 `runFreshTickWithRollback` 直接拿来做完整回滚源。
- [ ] **采纳方案（倾向）：rollback 单独深拷贝 `latestState`，不扩 `toJSON`**——理由：`toJSON()` 产出的镜像本就是刻意瘦身过的持久化快照（只保留 `zoneHistory`/`lastDecisions`/负域计数等聚合信息），若为了 tick 级回滚需求把完整 `players/zones/npcs` 塞回 `WorldModelSnapshot`，会让持久化镜像重新膨胀、违背此前瘦身诉求（历史决策：`negDomainEscapeTelemetry` 等已刻意聚合而非存全量玩家表）。具体改法：
  - [ ] 在 `WorldModel` 上新增一个不进入 `WorldModelSnapshot` 类型的内部方法（如 `snapshotLatestStateForRollback(): WorldStateV1 | null`），复用现有 `cloneWorldState`（world-model.ts:946）对 `this.latestStateValue` 做一次深拷贝返回（`null` 时原样返回 `null`，不占位空壳）。
  - [ ] 新增对称的 `restoreLatestStateForRollback(state: WorldStateV1 | null): void`，直接 `this.latestStateValue = state ? cloneWorldState(state) : null;`（不经过 `applySnapshot`，不触碰 `zoneHistory`/`lastDecisions`/负域计数字段——这些仍归 `toJSON`/`restoreFromJSON` 管）。
  - [ ] 改造 `runFreshTickWithRollback`（runtime.ts:716-729）：在取 `rollbackSnapshot = worldModel.toJSON()` 的同时，额外 `const rollbackLatestState = worldModel.snapshotLatestStateForRollback();`；`catch` 分支先 `worldModel.restoreFromJSON(rollbackSnapshot)` 恢复聚合字段，再 `worldModel.restoreLatestStateForRollback(rollbackLatestState)` 恢复真实的 `players/zones/npcs`（顺序：后者必须在 `restoreFromJSON` 之后调用，因为 `applySnapshot` 内部会无条件覆盖 `latestStateValue`，见 world-model.ts:692-716）。
- [ ] **备选方案（不采纳，记录理由供后续参考）**：扩展 `WorldModelSnapshot.toJSON()`/`applySnapshot()` 直接携带完整 `latestStateValue`（深拷贝 players/zones/npcs），让 `restoreFromJSON` 天然还原真实状态。缺点：这会让**每一次**跨重启持久化落盘（不只是罕见的 tick 级回滚）都携带全量玩家/区域/NPC 表，与该镜像此前刻意瘦身（只留聚合统计）的设计意图相悖，且放大 Redis 持久化 key 体积。除非确认持久化镜像本就该携带全量 state（需人工确认设计意图），否则不作为首选。
- [ ] 保持负域越域判定的所有 gameplay 结算逻辑不变——本次修复只影响 `previousState` 来源的正确性，不改 `renderNegDomainNarrations`/`recordNegDomainEscapeTelemetry` 内部的判定分支。
- [ ] 补一条明确文档/代码注释：`toJSON()`/`restoreFromJSON()` 是"持久化镜像专用契约，不含完整 world state"，避免未来又有调用方误当成通用回滚快照复用。

## 验收测试计划

所属栈：`agent/packages/tiandao`，`npm test`（vitest + 类型检查）。

- **happy path**：正常 tick（无发布失败）不触发任何回滚路径，`worldModel.latestState` 在 tick 结束后等于本 tick `updateState` 写入的真实状态；负域叙事/遥测按既有行为正常产出——回归既有 `neg-domain-escape` 相关测试全部保持绿。
- **边界 — 回滚前 `latestState` 为 `null`**（首次启动、`toJSON`/`snapshotLatestStateForRollback` 时还没跑过任何 `updateState`）：`restoreLatestStateForRollback(null)` 后 `worldModel.latestState` 必须仍是 `null`，不产出错误的"空玩家表"或抛异常。
- **边界 — 回滚前 `latestState` 非空**（核心回归，锁定本次修复目标行为）：构造 `worldModel.updateState(seedState)`（含 ≥1 个玩家、≥1 个 zone）建立"上一 tick 真实状态"，模拟 `runFreshTickWithRollback` 触发一次 `publishCommands`/`publishNarrations` 失败（复用现有 `FailingPublishRuntimeRedis` helper），断言回滚后 `worldModel.latestState.players`/`zones`/`npcs` 与 `seedState` 深度相等（而不是空数组桩），且是深拷贝（改 `seedState.players[0]` 不应影响回滚后的 `latestState`）。
- **错误分支 — 重试 tick 后的负域比较必须命中**：在上述回滚场景基础上，令重试 tick 的 `state` 里同一玩家从"非负域 zone"变为"负域 zone"，断言 `renderNegDomainNarrations` 产出「失去锁定」叙事、`recordNegDomainEscapeTelemetry` 调用 `worldModel.recordNegDomainEscapeEntry`——即修复前会因 `previousPlayer` 查不到而 `continue`、修复后必须正确命中并记录。对称地验证"负域→非负域"触发「重新锁定」叙事 + `recordNegDomainEscapeExit` 找到配对 session（不 no-op）。
- **状态转换 — 现有回归测试 `tests/runtime.test.ts:1920`（"restores world model history exactly before retrying a failed tick"）保持通过**并**追加断言**：除了原有 `zoneHistory`/`lastTick` 检查外，新增对 `worldModel.latestState.players`（非空、字段与 `failedTickState.players` 一致）的断言，把这条既有测试从"只测聚合字段"升级为"聚合字段 + 真实 state 都验证"，防止回归悄悄退化回空壳桩。
- **状态转换 — `toJSON()`/`restoreFromJSON()`（持久化路径）行为不变**：新增/沿用测试确认跨重启持久化场景（`toJSON` → 写 Redis → 进程重启 → `restoreFromJSON`）后紧跟随 `updateState()` 刷新，产出结果与修复前一致（持久化契约本身不扩不改，只是不再被 tick 级回滚误用）。
- **契约测试**：新增 `snapshotLatestStateForRollback`/`restoreLatestStateForRollback` 的正反 pin 测试——非 `null` 输入深拷贝往返、`null` 输入往返、连续两次调用互不干扰（第二次快照不受第一次恢复影响）。

## 风险

- 修复只应触碰 `runFreshTickWithRollback` 及 `WorldModel` 新增的两个 rollback 专用方法，**不应顺手改动** `toJSON`/`applySnapshot`/`WorldModelSnapshot` 类型本体——否则会把一个"回滚复用错了契约"的 bug 变成"持久化镜像格式变更"的另一件事，牵连跨重启兼容性（已落盘的旧快照缺新字段时的兼容读取）。
- `cloneWorldState`（world-model.ts:946）当前是模块内 `function`（非导出），新增的 `snapshotLatestStateForRollback`/`restoreLatestStateForRollback` 应作为 `WorldModel` 类方法内部调用它，不需要导出改变其可见性；若发现深拷贝开销在高频 tick 下不可忽略，需要和 `updateState` 里已有的同款 `cloneWorldState` 调用一并评估性能，而不是本 plan 单独优化。
- 触发条件依赖真实 Redis 瞬时故障，本地默认测试环境不会自然触发——验收测试必须用现有 `FailingPublishRuntimeRedis` 之类的故障注入 helper 显式模拟，不能只测"正常路径不受影响"就算过。
- 本 plan 严格限定在 agent 侧（TypeScript），不触碰 server（Rust）或 client（Java）——若排查中发现 server 侧也有类似"用持久化快照兼职运行时回滚"的模式，应另开独立 finding/plan，不在本 plan 范围内顺手扩大。
