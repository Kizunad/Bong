# plan-bughunt-tsy-enter-exit-agent-silent-drop-v1

> 一句话主题：修复 `bong:tsy_event` 中合法 `tsy_enter` / `tsy_exit` 在 tiandao Redis bridge 被静默丢弃，使完整进出事件进入 Agent 上下文。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | Redis typed dispatch、runtime drain 与 Agent context 注入 | ✅ 2026-08-26 |

验收日期：P0 验收 2026-08-26。

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

## P0 修复计划

- [x] ✅ 2026-08-26 在 `RedisIpc` 增加 TSY enter/exit 类型、校验 import 与有界 buffer/callback/drain API。
- [x] ✅ 2026-08-26 在 `handleTsyEventMessage` 对 `tsy_enter` / `tsy_exit` 做 schema 校验，invalid payload warning 且不入 buffer。
- [x] ✅ 2026-08-26 为未知 TSY `kind` 增加 warning，避免未来合法 schema 再次静默掉包。
- [x] ✅ 2026-08-26 在 `runRuntime` 消费 enter/exit drain，并把完整事件注入三类 Agent context；fresh tick 失败或无 state 时保留 pending。
- [x] ✅ 2026-08-26 明确保留 `qi_drained_total=0` 现状注释，不把当前修复伪装成真实真元累计修复。

## 验收测试计划

- [x] ✅ 2026-08-26 `redis-ipc.test.ts`：`tsy_enter` / `tsy_exit` drain/callback 完整 payload 对拍。
- [x] ✅ 2026-08-26 `redis-ipc.test.ts`：invalid enter/exit 与 unknown TSY kind warning 且不入 buffer。
- [x] ✅ 2026-08-26 `context.test.ts` / `runtime.test.ts`：完整事件字段进入 context，runtime drain 注入 fresh tick。
- [x] ✅ 2026-08-26 `agent/packages/schema`：`TsyEnterEventV1` / `TsyExitEventV1` 既有 generated artifact 与 samples 通过。
- [x] ✅ 2026-08-26 完整 gate：`cd agent/packages/schema && npm test`；`cd agent/packages/tiandao && npm test`。

## 验证结论（2026-08-26）

第一性原理复核确认 bug 成立：server 已通过 `bong:tsy_event` 发布 schema 合法的 `tsy_enter` / `tsy_exit`，旧 `handleTsyEventMessage` 只处理其它 TSY kind，因而会无告警静默丢弃。修复仅在 tiandao Redis bridge/runtime/context 接线，未修改 server、schema、wire 或 `qi_drained_total` 占位语义。

## Finish Evidence

- **落地清单**：`agent/packages/tiandao/src/redis-ipc.ts`（typed TSY runtime buffer、schema validation、callback/drain、unknown-kind warning）；`agent/packages/tiandao/src/runtime.ts`（drain 与 pending 注入）；`agent/packages/tiandao/src/context.ts` / `src/agent.ts`（完整事件上下文）；对应 `tests/redis-ipc.test.ts`、`tests/context.test.ts`、`tests/runtime.test.ts` 回归。
- **关键 commit**：`5571e7e35`（2026-08-26，promotion）；`f89004824`（2026-08-26，Redis typed dispatch/buffer）；`bfb69a9b5`（2026-08-26，runtime/context 注入）。均带 `Model: gpt-5.6-luna-max`。
- **测试结果**：受影响三组 `vitest` 共 118 tests 通过；`cd agent/packages/schema && npm test`：31 files / 904 tests 通过；`cd agent/packages/tiandao && npm test`：72 files / 865 tests 通过（含 TypeScript check）。
- **对抗验证**：无上下文 read-only `gpt-5.6-luna` validator 对拍最终实现 SHA `bfb69a9b5eb107bb689ea09011bf63c7a0f7864b`，结果 PASS；随后已关闭 validator。
- **跨仓库核验**：server 的 `TsyEnterEventV1` / `TsyExitEventV1` bridge 与 `bong:tsy_event` publisher 未改；schema 定义与 generated artifact 未改；agent 现完整接收并注入 `return_to`、`filtered_items`、`duration_ticks` 与 `qi_drained_total`。
- **遗留 / 后续**：server 侧 `qi_drained_total=0` 仍是独立 loot 累计后续风险，本 PR 不宣称已修复；本 PR 创建后等待 inline review 与 e2e，不在本流程内 merge。

## 风险

- 如果直接生成 broadcast narration，可能和 `tsy_zone_activated` 首次发现 UI 重复刷屏；修复应区分“首次激活”与“普通入场”。
- `qi_drained_total` 当前是 server 占位 0，任何 UI/叙事都不能把它当真实累计。
- 新 drain API 若每 tick 清空，会影响未来多 agent 消费者；建议先定义单一 owner 或保留 callback + buffer 的明确语义。
