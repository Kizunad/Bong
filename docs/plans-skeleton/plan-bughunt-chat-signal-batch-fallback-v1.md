# BugHunt：聊天信号批处理解析失败时未进入逐条 fallback

> 来源 Issue：#1701。只修 agent 聊天标注器对畸形 LLM 输出的批次隔离；保留 server `bong:player_chat` 队列与 ChatMessageV1 契约。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 让整批 JSON/数组解析失败转为逐条 unknown fallback | ⬜ |
| P1 | 保留合法行、坏行与 LLM 异常的可观测性测试 | ⬜ |

## §0 摘要

`processChatBatch` 在调用 `parseChatSignalBatch` 前没有捕获解析异常；当 LLM 返回坏 JSON 或非数组时，解析器直接 throw，后续本来用于缺行/坏字段的逐条 fallback 永远不会执行，整批已从 Redis drain 的玩家聊天信号消失。

## §1 游玩影响

玩家聊天仍可进入 server 队列，却在一次标注输出畸形时整批不进入天道上下文和社交/叙事决策；不会影响原始聊天发送或真元结算。

## §2 复现路径

1. server 向 `bong:player_chat` 写入一批 `ChatMessageV1`，runtime drain 后调用 `processChatBatch`。
2. mock/真实 LLM 返回非 JSON、JSON 对象或代码围栏中的非数组。
3. `parseChatSignalBatch` 在 `:64-70` throw；`processChatBatch:114-157` 的逐条 fallback 未执行，runtime 只记录异常。

## §3 今天 `origin/main` 的根因证据

- `agent/packages/tiandao/src/chat-processor.ts:55-70` 对坏 JSON/非数组直接抛错；`:114-157` 只有在成功得到 batch 后才建立 `byKey` 并补 unknown。
- `agent/packages/tiandao/src/runtime.ts:1268-1290` drain 后处理异常只记录日志；`agent/packages/tiandao/src/redis-ipc.ts:1013` 的 `drainListAtomically` 已通过 `LRANGE/LTRIM` 取走消息，没有调用方会自动 RPUSH 回补。
- server `server/src/network/chat_collector.rs:105-...` 通过 `CH_PLAYER_CHAT` 发布，`server/src/schema/channels.rs:3` 将 channel 固定为 `bong:player_chat`；输入数据本身不是本问题根因。

## §4 非重复比对与立项检查记录

- **worldview**：查 `docs/worldview.md` 的“玩家聊天”“天道叙事”“社交”关键词；本骨架只保证输入不因一次 LLM 格式错误整批消失。
- **finished_plans**：查 `plan-agent-v1/v2`、player-chat queue、narration pipeline 归档；确认 queue 容量与 agent drain 的其他计划没有覆盖 parse fallback。
- **active plan**：查 `docs/plan-*.md` 的 `processChatBatch`、`parseChatSignalBatch`、`drainListAtomically`；没有同一修法。
- **skeleton**：查 `docs/plans-skeleton/` 的 chat processor、Redis drain、unknown signal；未发现重复骨架。
- **reminder.md**：已查 `docs/plans-skeleton/reminder.md`；没有聊天信号 fallback 条目。

## §5 修复骨架

- P0：把批次解析失败转换为按原消息 key 的 unknown signal，或提供等价的逐条降级入口；坏行不能让合法消息一起丢失。
- P1：区分 JSON 解析失败、非数组、单行字段非法和 LLM 请求失败，分别记录统计；测试确认输出数量与输入消息的可观察契约一致。
- 验收：任何畸形 batch 仍为每条输入提供 deterministic unknown/fallback；合法 rows 保留；不重复处理同一 drained batch。

## §6 接入面与跨仓契约

- **Inputs**：Redis list `bong:player_chat`、`ChatMessageV1`、`RedisIpc.drainPlayerChat`、LLM annotate response。
- **Outputs**：`ChatSignal[]`、`WorldModel`/runtime chat context 与已有 narration/agent decision；错误统计和日志。
- **共享类型或事件**：复用 `ChatMessageV1`、`ChatSignal`、`PLAYER_CHAT` channel；不新增 client chat wire 类型。
- **server 符号**：`collect_player_chat`、`CH_PLAYER_CHAT`、`PLAYER_CHAT_QUEUE_MAX_LEN`；server 继续负责入队和容量界限，不负责 LLM fallback。
- **agent 符号**：`parseChatSignalBatch`、`processChatBatch`、`RedisIpc.drainPlayerChat`、`runtime.ts` 的 tick 处理。
- **client**：无变更；Fabric client 已通过既有聊天协议把消息送到 server，问题在 agent drain 后的标注容错。
- **worldview / qi_physics**：聊天信号只影响叙事/agent context；不涉及真元转移或 `qi_physics`。
