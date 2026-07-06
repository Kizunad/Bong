# plan-bughunt-alchemy-start-intervention-agent-drop-v1

> Skeleton Plan。只记录 BugHunt E7 发现，不消费、不归档、不修改实际代码。

## Bug 摘要

`bong:alchemy/session_start` 与 `bong:alchemy/intervention_result` 已经是 server -> agent 的共享契约：server 在玩家炼丹起炉、干预成功时真实发布，`agent/packages/schema` 也定义了 channel 与 TypeBox payload。但 `agent/packages/tiandao/src/redis-ipc.ts` 只订阅和解析 `bong:alchemy/session_end` / `bong:alchemy_insight`，导致起炉和干预结果在 Tiandao runtime 侧静默丢失。

这不重复 #995：#995 是 `alchemy_insight` 真实 payload 多 `ts`、agent schema 少字段导致洞察事件被校验拒收；本 bug 是 `session_start` / `intervention_result` 两个独立 channel 根本没有被 Tiandao 订阅/解析。

## 实际游玩体验影响

玩家正常点燃炼丹炉后，server 已经把“某玩家在某炉起炉炼某丹”的 `session_start` 发布到 Redis；玩家每次调火、注气等干预成功后，server 也会发布 `intervention_result`，携带当前温度与注气量。但 agent runtime 完全收不到这两类事件。

结果是实际游玩中的炼丹过程只剩结算点可能进入 agent：天道无法感知玩家已经起炉，也看不到中途干预是否被接受、温度和注气如何变化。玩家在炼丹过程中本应出现的过程性天道介入、调试观测和后续 agent 逻辑入口都会缺失；出问题时日志会呈现“server 已发 Redis，但 Tiandao 没反应”的断链体验。

## 证据定位

- 契约声明：`agent/packages/schema/src/channels.ts:340-345` 标注 `ALCHEMY_SESSION_START`、`ALCHEMY_SESSION_END`、`ALCHEMY_INTERVENTION_RESULT` 均为 Server -> Agent；`agent/packages/schema/src/channels.ts:493-496` 又把三者加入 `REDIS_V1_CHANNELS`。
- TypeBox payload：`agent/packages/schema/src/alchemy.ts:109-121` 定义 `AlchemySessionStartV1`，`agent/packages/schema/src/alchemy.ts:143-159` 定义 `AlchemyInterventionResultV1`，`agent/packages/schema/src/alchemy.ts:174-183` 提供对应 validator。
- server 起炉发送：`server/src/network/client_request_handler.rs:12126-12132` 在 ignite 成功路径调用 `publish_alchemy_session_start`；`server/src/network/client_request_handler.rs:12681-12692` 构造并发送 `RedisOutbound::AlchemySessionStart`。
- server 干预发送：`server/src/network/client_request_handler.rs:12053-12061` 在 intervention 成功路径调用 `publish_alchemy_intervention_result`；`server/src/network/client_request_handler.rs:12707-12722` 构造并发送 `RedisOutbound::AlchemyInterventionResult`。
- Redis bridge 发布：`server/src/network/redis_bridge.rs:769-799` 把两类 outbound 分别发布到 `CH_ALCHEMY_SESSION_START` 与 `CH_ALCHEMY_INTERVENTION_RESULT`。
- Tiandao 缺口：`agent/packages/tiandao/src/redis-ipc.ts:6-7` 只 import `validateAlchemyInsightV1Contract` / `validateAlchemySessionEndV1Contract`；`agent/packages/tiandao/src/redis-ipc.ts:149` 的 `AlchemyRuntimeEventV1` 只含 `AlchemySessionEndV1 | AlchemyInsightV1`；`agent/packages/tiandao/src/redis-ipc.ts:292-294` 只 route `ALCHEMY_SESSION_END || ALCHEMY_INSIGHT`；`agent/packages/tiandao/src/redis-ipc.ts:756-757` connect 只订阅这两个 channel。
- 覆盖缺口：`agent/packages/tiandao/tests/redis-ipc.test.ts:478-560` 只测试 `session_end` 与 `alchemy_insight`，没有 `session_start` / `intervention_result`。

## 触发路径

1. 玩家打开炼丹炉并选择配方，发起点火。
2. server `client_request_handler` 成功创建/推进 session 后，调用 `publish_alchemy_session_start`。
3. `RedisOutbound::AlchemySessionStart` 经 `redis_bridge` 发布到 `bong:alchemy/session_start`。
4. Tiandao `RedisIpc` 未订阅该 channel，事件不会进入 `latestAlchemyEvents`，也不会触发 callback。
5. 玩家中途调火或注气，server 成功应用 intervention 后发布 `bong:alchemy/intervention_result`。
6. Tiandao 同样未订阅/解析该 channel，干预结果静默丢失。

## 反方审查记录

### Round 1：是否只是有意只消费 session_end

反方最强论点：`docs/finished_plans/plan-alchemy-client-v1.md` 的 P4 确实只把明确 narration 订阅写成 `session_end`。

裁决：不推翻。相同 P4 也明确列出 `session_start` / `session_end` / `intervention_result` 三条 Redis channel，目标是“agent 侧需要订阅炼丹进展，实现天道 narration 介入”。共享 schema 进一步把 `session_start` 与 `intervention_result` 标成 Server -> Agent 并加入 Redis v1 channel 清单，因此不能解释成纯 server 内部字段。

### Round 2：是否重复或不可达

反方检查开放 PR 后未发现 `session_start` / `intervention_result` / `AlchemySessionStart` / `AlchemyInterventionResult` 同题 PR。#995 只覆盖 `alchemy_insight` payload `ts` 漂移；#974/#981/#1001 等是炼丹其他链路，不覆盖本缺口。

裁决：通过。server 玩家起炉与干预成功路径有明确生产发送点，Redis bridge 有明确 publish arm；Tiandao 端缺订阅/解析是真实 runtime bridge 断链。

## Skeleton Fix Plan

1. 扩展 `agent/packages/tiandao/src/redis-ipc.ts`：
   - import `validateAlchemySessionStartV1Contract` 与 `validateAlchemyInterventionResultV1Contract`。
   - 将 `AlchemyRuntimeEventV1` 扩为 `AlchemySessionStartV1 | AlchemySessionEndV1 | AlchemyInterventionResultV1 | AlchemyInsightV1`。
   - 在 `onMessage` 与 `connect()` 中接入 `ALCHEMY_SESSION_START` / `ALCHEMY_INTERVENTION_RESULT`。
   - `handleAlchemyRuntimeEventMessage` 按 channel 分派对应 validator，拒绝非法 payload 时输出 channel 维度错误。
2. 明确消费策略：
   - 最小修复可先保证 `onAlchemyRuntimeEvent` / `getLatestAlchemyEvents` 能观测四类炼丹事件。
   - 若要直接补叙事，新增轻量 deterministic renderer 或独立 alchemy runtime，只对起炉和关键干预做节流，避免每次微调都刷屏。
3. 补文档/注释边界：
   - `session_start` 表示起炉观测。
   - `intervention_result` 表示被 server 接受的干预结果；失败干预若未来要进 agent，需要另扩 payload 或发布点。

## 验收测试计划

- `agent/packages/tiandao`：
  - 在 `redis-ipc.test.ts` 新增 `observes alchemy session_start events`。
  - 新增 `observes alchemy intervention_result events`。
  - 新增非法 payload 测试，确认不会进入 callback / buffer。
  - 如新增 renderer/runtime，补 publish 到 `AGENT_NARRATE` 的 pin 测试与节流测试。
- `agent/packages/schema`：
  - 跑既有 alchemy schema/sample 测试，确认新增 tiandao 消费不需要改 contract。
- 构建命令：
  - `cd agent/packages/tiandao && npm test`
  - 如改 schema export 或 generated，再跑 `cd agent/packages/schema && npm test` 与 `cd agent && npm run build`

## 风险

- 事件频率：`intervention_result` 可能比 `session_end` 高频，直接叙事必须节流或只记录 context。
- 兼容性：历史 Redis 上的脏 payload 不应让 Tiandao runtime 崩溃，只能 warn 后丢弃。
- 产品语义：若当前设计只想对炸炉/结丹播报，修复也至少应保证起炉/干预可被 agent 观测；是否即时播报应由后续 fix PR 明确收口。
