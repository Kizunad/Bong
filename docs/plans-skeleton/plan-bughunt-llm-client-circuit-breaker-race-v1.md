# BugHunt：天道狩猎叙事 LLM 客户端熔断状态并发竞态

> 来源 Issue：#1770。只处理 `createClient` 的共享失败计数与狩猎叙事 runtime 的并发入口；不改变 LLM provider、Redis channel 或 narration 文案契约。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 固定并发调用下 `consecutiveFailures/backoffUntil` 的状态转移 | ⬜ |
| P1 | 狩猎 runtime 的并发/熔断回归测试与指标保持一致 | ⬜ |

## §0 摘要

`createClient` 把失败计数和退避截止时间放在闭包共享变量中，而 `TiandaoHuntNarrationRuntime.onMessage` 对每条 Redis 消息 fire-and-forget。多个 `chat` 同时完成时，成功请求会把另一条请求刚累加的失败计数清零，失败请求也可能覆盖退避窗口，熔断变成偶发而非可靠门禁。

## §1 游玩影响

LLM 超时或 provider 错误时，狩猎叙事会继续发起并发请求，退避和 fallback 触发次数不稳定，造成额外请求、延迟和叙事缺口；不会改变 server 的真元或玩家状态。

## §2 复现路径

1. 让 `bong:tiandao_hunt_narration_request` 在同一 runtime 短时间收到两条以上合法请求。
2. 让一条 `chat` 在 `runChatLoop` 中失败，另一条稍后成功。
3. 观察成功路径 `consecutiveFailures = 0` 覆盖失败路径的计数，或多个失败路径交错写 `backoffUntil`，随后仍继续请求。

## §3 今天 `origin/main` 的根因证据

- `agent/packages/tiandao/src/llm.ts:123-124,164-195` 在 `createClient` 闭包内读写共享的 `consecutiveFailures`、`backoffUntil`；成功分支无条件清零，失败分支递增并设置退避。
- `agent/packages/tiandao/src/tiandao-hunt-narration-runtime.ts:84-87` 的 `onMessage` 对每条消息执行 `void this.handleRequestPayload(message)`，没有串行队列或并发上限；`:127-140` 仍会先校验 `TiandaoHuntNarrationRequestV1`，所以问题位于合法请求之后的 LLM 状态管理。
- 共享输入契约是 `agent/packages/schema/src/tiandao-hunt-narration.ts:20-43`，server 通过 `server/src/schema/channels.rs:11` 的 `CH_TIANDAO_HUNT_NARRATION_REQUEST` 和 `server/src/network/redis_bridge.rs:917-921` 发布同一 payload。

## §4 非重复比对与立项检查记录

- **worldview**：查 `docs/worldview.md` 的“天道”“叙事”“真元”关键词；本问题只影响反馈可靠性，不新增正典规则或真元流动。
- **finished_plans**：查 `plan-agent-v1.md`、`plan-agent-v2.md`、已归档 narration/Redis plan 的 LLM client、retry、fallback 关键词；没有一份把该闭包状态与狩猎 runtime 绑定起来。
- **active plan**：查 `docs/plan-*.md` 的 `createClient`、`backoffUntil`、`TiandaoHuntNarrationRuntime`；未发现同一修法。
- **skeleton**：查 `docs/plans-skeleton/` 的 `llm.ts`、`consecutiveFailures`、`bong:tiandao_hunt_narration_request`；没有同根因骨架。
- **reminder.md**：已查 `docs/plans-skeleton/reminder.md`；现有条目是套包、放置、经济等遗留事项，没有 LLM 熔断竞态。

## §5 修复骨架

- P0：为同一个 client 明确单写者顺序或等价的原子状态机，让“检查退避 → 发请求 → 成功/失败提交”保持线性化；不能用一次普通变量快照掩盖并发。
- P1：补并发失败/成功交错、退避期间拒绝、退避过期恢复和 fallback 计数回归；保留请求校验失败与 provider 失败的区分。
- 验收：达到阈值后并发请求不会绕过 backoff；一个成功响应不会抹掉仍在途的失败；退避结束后只恢复一次正常请求。

## §6 接入面与跨仓契约

- **Inputs**：`bong:tiandao_hunt_narration_request`、`TiandaoHuntNarrationRequestV1`、`TiandaoHuntNarrationRuntime.onMessage` 和 `LlmClient.chat`。
- **Outputs**：`AGENT_NARRATE` 上的 `NarrationV1` 或既有 fallback；状态指标 `llmFailures/fallbackUsed` 仍反映真实结果。
- **共享类型或事件**：复用 TypeBox `TiandaoHuntNarrationRequestV1`、Rust `TiandaoHuntNarrationRequestV1`、`NarrationV1`；不另造熔断 wire 字段。
- **server 符号**：`CH_TIANDAO_HUNT_NARRATION_REQUEST`、`RedisOutbound::TiandaoHuntNarrationRequest`、`TiandaoHuntNarrationRequestV1`；server 只提供输入，不修改熔断状态。
- **agent 符号**：`createClient`、`LlmClient.chat`、`TiandaoHuntNarrationRuntime.handleRequestPayload`、`CHANNELS.TIANDAO_HUNT_NARRATION_REQUEST` 与 `AGENT_NARRATE`。
- **client**：无变更；输出仍由 server 现有 narration/chat 下发链路显示，客户端没有 LLM 熔断状态。
- **worldview / qi_physics**：只涉及天道反馈可靠性；不改 `docs/worldview.md`，不触及 `qi_physics` 或 `SPIRIT_QI_TOTAL`。
