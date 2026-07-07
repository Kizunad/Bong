# plan-bughunt-alchemy-process-agent-drop-v1

> Skeleton Plan. BugHunt r13 agent-schema 分区只立项，不修代码。

## 一句话 Bug

炼丹起炉与干预结果已经由 server 发布到 Redis，并在 `@bong/schema` 中声明为 Server -> Agent channel，但 Tiandao runtime 只订阅炼丹结算与丹心洞察，导致起炉和调火/注气过程事件在 agent 侧静默丢失。

## 避重结论

- 已检查 `gh pr list --state all --limit 600 --json number,title,headRefName,url`。
- 不重复 #1054、#1059、#1061、#1075、#1081、#1090、#1093、#1098、#1111、#1116。
- 不重复非 BugHunt #1068-#1072。
- `docs/plans-skeleton/plan-bughunt-alchemy-start-intervention-agent-drop-v1.md` 已有同题占位，但 skeleton 目录不可消费；本文件是可进入流水线的 active plan。

## 证据

- `agent/packages/schema/src/channels.ts` 声明 `ALCHEMY_SESSION_START = "bong:alchemy/session_start"`，注释为 Server -> Agent: 炼丹起炉。
- `agent/packages/schema/src/channels.ts` 声明 `ALCHEMY_INTERVENTION_RESULT = "bong:alchemy/intervention_result"`，注释为 Server -> Agent: 炼丹干预结果。
- `agent/packages/schema/src/channels.ts` 将两者放入 `REDIS_V1_CHANNELS`。
- `agent/packages/schema/src/alchemy.ts` 定义并导出 `AlchemySessionStartV1` 与 `AlchemyInterventionResultV1` 校验器。
- `server/src/network/client_request_handler.rs` 在起炉成功后发送 `RedisOutbound::AlchemySessionStart`。
- `server/src/network/client_request_handler.rs` 在干预成功后发送 `RedisOutbound::AlchemyInterventionResult`。
- `server/src/network/redis_bridge.rs` 将二者分别发布到 `CH_ALCHEMY_SESSION_START` 与 `CH_ALCHEMY_INTERVENTION_RESULT`。
- `agent/packages/tiandao/src/redis-ipc.ts` 只解构、订阅、解析 `ALCHEMY_SESSION_END` 和 `ALCHEMY_INSIGHT`。
- `agent/packages/tiandao/src/redis-ipc.ts` 的 `AlchemyRuntimeEventV1` 只包含 `AlchemySessionEndV1 | AlchemyInsightV1`。

## 实际游玩体验影响

玩家炼丹起炉后，客户端仍能看到炉体 snapshot 和起炉 VFX，但天道不知道玩家已经开始炼丹；玩家调火或注气后，server 已确认干预成功，但 Tiandao 上下文、叙事和观测回放都看不到这些过程变化。结果是炼丹过程在天道侧只剩结算点，无法形成“某人正在某炉炼某丹、期间如何干预”的连续叙事或后续系统判断。

## 对抗复核

- 第 1 轮 adversarial subagent：确认 repo 内没有其他 consumer 消费 `session_start`，不重复指定 PR；影响应表述为 Tiandao/agent 观测层失明，不是炼丹主流程损坏。
- 第 2 轮 adversarial subagent：确认 `session_start` 与 `intervention_result` 都是真实 server 发布、schema 声明 Server -> Agent，但 Tiandao 无订阅、解析、缓存或 callback；反证失败。

## 修复范围占位

- P0: 补齐 `RedisIpc` 对 `ALCHEMY_SESSION_START` 与 `ALCHEMY_INTERVENTION_RESULT` 的订阅、校验、缓冲和 callback。
- P1: 明确炼丹过程事件进入 tick context 还是独立 narration runtime，并定义干预事件节流语义。
- P2: 增加 `agent/packages/tiandao/tests/redis-ipc.test.ts` 回归，覆盖订阅清单、正样本进入观测流、非法消息丢弃。

## 验收占位

- 起炉事件进入 Tiandao 可观测炼丹事件流。
- 干预结果进入 Tiandao 可观测炼丹事件流。
- 结算与洞察现有行为不回退。
- 无效起炉/干预 payload 不污染观测流。
- schema channel、runtime 订阅清单、测试样本三者保持一致。
