# plan-bughunt-alchemy-start-intervention-agent-drop-v1（骨架）

> **骨架（草案）**。一句话主题：炼丹起炉与中途干预事件已经由 server 发布、由 schema 声明为 Server -> Agent channel，但 Tiandao `RedisIpc` 未订阅这两类 channel，导致过程事件在 agent runtime 侧静默丢失。

> 立项动机：本 PR 只记录 BugHunt E7 发现，不修业务代码；后续 fix PR 需要把 alchemy runtime bridge 的订阅、校验、缓存和测试补齐。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 起炉与干预结果未进入 Tiandao runtime | fix_pr | ⬜ |
| P1 | 是否对过程事件触发叙事介入 | design | ⬜ |

## P0 — 起炉与干预结果未进入 Tiandao runtime

- **#1 major（fix_pr）**：`agent/packages/tiandao/src/redis-ipc.ts` 目前只消费 `bong:alchemy/session_end` 与 `bong:alchemy_insight`，没有订阅或解析 `bong:alchemy/session_start` / `bong:alchemy/intervention_result`。
- contract 侧已经有对应 channel 与 payload：`agent/packages/schema/src/channels.ts`、`agent/packages/schema/src/alchemy.ts`。
- server 侧玩家起炉成功、调火/注气干预成功后均有 Redis publish 路径。
- Tiandao 侧缺口会让 `onAlchemyRuntimeEvent` 与 `getLatestAlchemyEvents` 看不到炼丹过程事件。
- 本题不重复 #995：#995 关注 `alchemy_insight` payload `ts` 漂移；本题关注两个独立 channel 根本未进入 Tiandao。

## 玩家可见影响

- 玩家点燃炼丹炉后，天道只能在结算点附近观察结果，无法感知“某玩家已经起炉炼某丹”。
- 玩家中途成功调火或注气后，当前温度、注气量等过程状态不会进入 agent runtime。
- 后续若要做过程性天道介入、调试观测或炼丹节奏反馈，这条 bridge 断链会让 server 已发事件但 Tiandao 无反应。

## 建议修法

- `RedisIpc` import `validateAlchemySessionStartV1Contract` 与 `validateAlchemyInterventionResultV1Contract`。
- 扩展 `AlchemyRuntimeEventV1` union，并在 `connect()` 订阅两个缺失 channel。
- `handleAlchemyRuntimeEventMessage` 按 channel 分派 validator，非法 payload 只 warn 后丢弃。
- 最小修复先保证四类炼丹 runtime event 都进入 callback 与 latest buffer。
- 若后续要直接叙事，需对 `intervention_result` 做节流，避免频繁调火刷屏。

## 测试抓手

- `agent/packages/tiandao/tests/redis-ipc.test.ts` 增加 `session_start` 可观测测试。
- 同文件增加 `intervention_result` 可观测测试。
- 增加非法 payload 不进入 callback / buffer 的负向测试。
- 若 schema export 或 dist 发生变化，同步跑 schema tests 与 agent build。

## 审计来源

- 来源：BugHunt E7，范围为 agent-schema / runtime bridge。
- 结论：**real-on-main，player-facing，局部明确，可 fix_pr**。
- 本骨架只立项，不在该 PR 内改 agent/server/client 代码。
