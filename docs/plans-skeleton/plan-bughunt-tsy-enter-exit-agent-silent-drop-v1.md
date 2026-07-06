# BugHunt Skeleton: TSY enter/exit agent 事件静默丢失

## Bug 摘要

`server` 已把活坍缩渊 `tsy_enter` / `tsy_exit` 事件发布到 Redis `bong:tsy_event`，`agent/packages/schema/src/tsy.ts` 也把它们定义为 Server -> Agent 事件，但 `agent/packages/tiandao/src/redis-ipc.ts` 的 `handleTsyEventMessage` 只识别 `tsy_npc_spawned`、`tsy_sentinel_phase_changed`、`tsy_zone_activated`。合法的 `tsy_enter` / `tsy_exit` payload 会被解析后无分支消费、无 warning、无 buffer，导致 tiandao runtime 永久看不到玩家进出 TSY 的真实信号。

范围收窄：这不是 `tsy_zone_activated` 首次发现 UI 全断；也不是当前真实 `qi_drained_total` 非零数据丢失，因为 server bridge 现阶段仍填 `0.0` 占位。问题是已上线发布、schema 已登记的 TSY enter/exit 事件在 agent runtime bridge 层静默掉包。

## 对实际游玩体验的影响

玩家进入已经激活过的活坍缩渊时，天道不会收到“踏进秘境”的事件上下文，也无法基于 `filtered_items` / `return_to` 做入场风险叙事或后续推演。玩家出关时，`duration_ticks` 不会进入 agent，天道无法感知这次探索停留了多久，也无法把“短暂探路 / 深入滞留 / 惊险撤离”区分成不同反馈。实际体验上，TSY 的进出过程会少一层天道响应，调试时也会误以为 server 没发事件。

## 证据定位

- TS schema 明确把 `tsy_enter` 定义为 Server -> Agent，且注释写明用于“踏进秘境 narration / 风险评估”：`agent/packages/schema/src/tsy.ts:36`。
- TS schema 定义 `tsy_exit`，包含 `duration_ticks` 与 `qi_drained_total`：`agent/packages/schema/src/tsy.ts:58`。
- `bong:tsy_event` channel 注释写明 entry / exit 共享同一频道，consumer 按 `kind` dispatch：`agent/packages/schema/src/channels.ts:325`。
- tiandao 订阅 `TSY_EVENT` 后直接进入 `handleTsyEventMessage`：`agent/packages/tiandao/src/redis-ipc.ts:282`、`agent/packages/tiandao/src/redis-ipc.ts:752`。
- `handleTsyEventMessage` 只处理 `tsy_npc_spawned`、`tsy_sentinel_phase_changed`、`tsy_zone_activated`：`agent/packages/tiandao/src/redis-ipc.ts:366`、`:376`、`:390`。
- server portal 入场真实 emit `TsyEnterEmit`，出场真实 emit `TsyExitEmit`：`server/src/world/tsy_portal.rs:124`、`:172`。
- server network 注册 enter/exit bridge：`server/src/network/mod.rs:686`。
- bridge 构造 `kind: "tsy_enter"` / `kind: "tsy_exit"`：`server/src/network/tsy_event_bridge.rs:44`、`:84`。
- Redis outbound 把二者 publish 到 `CH_TSY_EVENT`：`server/src/network/redis_bridge.rs:1094`、`:1103`。
- 现有 tiandao Redis IPC 测试只覆盖 hostile TSY kind，没有 enter/exit pin：`agent/packages/tiandao/tests/redis-ipc.test.ts:312`。

## 触发路径

1. 玩家在主世界靠近 TSY entry portal。
2. `tsy_entry_portal_tick` attach `TsyPresence` 并 emit `TsyEnterEmit`。
3. `publish_tsy_enter_events` 转成 `TsyEnterEventV1 { kind: "tsy_enter", ... }`。
4. `RedisOutbound::TsyEnter` 发布到 `bong:tsy_event`。
5. tiandao `RedisIpc` 收到 `TSY_EVENT`，进入 `handleTsyEventMessage`。
6. 因无 `tsy_enter` 分支，函数结束且不记录、不告警。

出关同理：`tsy_exit_portal_tick` -> `TsyExitEmit` -> `TsyExitEventV1 { kind: "tsy_exit", duration_ticks, ... }` -> `bong:tsy_event` -> tiandao 静默丢弃。

## 反方审查记录

第一轮反方结论：不通过。反方未找到其它 tiandao runtime 直接消费 `tsy_enter` / `tsy_exit`，确认 server 生产路径真实发布，开放 PR 搜索 `tsy_enter|tsy_exit|bong:tsy_event` 未命中同题。

第二轮反方结论：不通过。反方最强质疑是历史 `plan-tsy-zone-followup-v1` 把 agent enter/exit narration 划为 out-of-scope、`qi_drained_total` 当前为 0 占位、`tsy_zone_activated` 已覆盖首次发现 UI。但这些只能收窄严重度，不能推翻“已登记合法 kind 在已订阅频道被静默吞掉”的 schema/runtime bridge 漂移。

## Skeleton Fix Plan

- [ ] 在 `RedisIpc` 增加 TSY enter/exit 类型、校验 import 与专用 buffer/callback/drain API，或将其纳入一个通用 `TsyRuntimeEventV1` buffer。
- [ ] 在 `handleTsyEventMessage` 对 `tsy_enter` / `tsy_exit` 调用 `validateTsyEnterEventV1Contract` / `validateTsyExitEventV1Contract`，验证失败必须 warning。
- [ ] 为未知 TSY `kind` 增加 warning，避免未来合法 schema 再次静默掉包。
- [ ] 在 `runRuntime` 中消费 enter/exit drain，最小落地可先写入 tiandao 上下文 / telemetry；若要叙事，补确定性 narration 或后续 agent plan 接线。
- [ ] 明确保留 `qi_drained_total=0` 现状注释，不把当前修复伪装成真实真元累计修复。

## 验收测试计划

- [ ] `agent/packages/tiandao`：补 `redis-ipc.test.ts`，发布 `kind:"tsy_enter"` 到 `TSY_EVENT` 后可从新 drain/callback 取到完整 payload。
- [ ] `agent/packages/tiandao`：补 `kind:"tsy_exit"` 测试，断言 `duration_ticks` 保留。
- [ ] `agent/packages/tiandao`：补 invalid enter/exit payload 测试，断言 warning 且不入 buffer。
- [ ] `agent/packages/tiandao`：补 unknown TSY kind 测试，断言 warning。
- [ ] `agent/packages/schema`：确认 `TsyEnterEventV1` / `TsyExitEventV1` generated artifact 与 samples 仍通过。
- [ ] 按仓库矩阵执行：`cd agent/packages/schema && npm test`；`cd agent/packages/tiandao && npm test`；若改 schema src，另跑 `cd agent && npm run build -w @bong/schema`。

## 风险

- 如果直接生成 broadcast narration，可能和 `tsy_zone_activated` 首次发现 UI 重复刷屏；修复应区分“首次激活”与“普通入场”。
- `qi_drained_total` 当前是 server 占位 0，任何 UI/叙事都不能把它当真实累计。
- 新 drain API 若每 tick 清空，会影响未来多 agent 消费者；建议先定义单一 owner 或保留 callback + buffer 的明确语义。
