# BugHunt: 聊天信号 5 分钟窗口失效

## Bug 摘要

`ChatMessageV1` 从 server 进入 agent 时携带权威 `ts`，但 `agent/packages/tiandao/src/chat-processor.ts` 在把聊天转成 `ChatSignal` 时丢掉了该时间戳。后续 `mergeChatSignals` / `buildChatSignalsBlock` 宣称维护“最近 5 分钟”窗口，却只会从 `mentions_mechanic` 里解析 `ts:<digits>`；普通聊天信号没有这个字段时会被视为永远 recent。

结果：低聊天量或长时间运行时，几小时前的玩家抱怨 / 求助 / 挑衅仍可能作为“近期民意 (最近 5 分钟)”进入天道 LLM prompt。

## 实际游玩体验影响

- 玩家一句旧聊天可能在很久后继续影响天道灾劫 / 变异 / 时代推演上下文，让天道像是在响应刚发生的民意。
- 低活跃服务器最明显：没有 20 条新聊天顶掉旧 signal 时，旧内容会长期残留。
- 表现不是崩溃或数据损坏，而是天道决策上下文污染：玩家会看到天道反馈与当前局势脱节。

## 证据定位

- `server/src/network/chat_collector.rs:276` 附近：`RedisOutbound::PlayerChat(ChatMessageV1 { v, ts: context.timestamp, player, raw, zone })`，server 端确实提供聊天时间戳。
- `agent/packages/schema/src/chat-message.ts:6` 附近：`ChatMessageV1` schema 要求 `ts`；但 `ChatSignal` schema 在同文件 `:20` 附近只有 `player/raw/sentiment/intent/mentions_mechanic/influence_weight`，没有明确时间字段。
- `agent/packages/tiandao/src/chat-processor.ts:98` 附近：`buildAnnotatePrompt(messages)` 发给标注 LLM 的内容只序列化 `player/zone/raw`，没有 `ts`。
- `agent/packages/tiandao/src/chat-processor.ts:120` 和 `:134` 附近：fallback / candidate `ChatSignal` 都不携带 `msg.ts`。
- `agent/packages/tiandao/src/chat-processor.ts:176` 附近：`isRecentSignal` 在 `extractSignalTimestamp` 返回 `null` 时直接返回 `true`。
- `agent/packages/tiandao/src/chat-processor.ts:248` 附近：`extractSignalTimestamp` 只从 `mentions_mechanic` 匹配 `ts:<digits>`。
- `agent/packages/tiandao/src/runtime.ts:1246` 和 `:1282` 附近：runtime 每轮用 `mergeChatSignals(..., nowSeconds)` 试图清理旧 signal，但无时间戳 signal 不会过期。
- `agent/packages/tiandao/src/agent.ts:91`、`:128` 附近与 `agent/packages/tiandao/src/context.ts:233` 附近：`setChatSignals -> createContextInput -> chatSignalsBlock -> buildChatSignalsBlock`，这些 signal 会进入 LLM prompt。

## 触发路径

1. 玩家普通聊天进入 `server/src/network/chat_collector.rs`。
2. server 发布 `ChatMessageV1` 到 Redis `bong:player_chat`，payload 含 `ts`。
3. tiandao runtime drain `bong:player_chat`，调用 `processChatBatch`。
4. `processChatBatch` 生成 `ChatSignal` 时丢弃 `msg.ts`。
5. 后续 tick 调用 `mergeChatSignals(latestChatSignals, [], nowSeconds)`，无时间戳 signal 被 `isRecentSignal` 判定为 recent。
6. agent tick 时 `chatSignalsBlock` 仍把旧 signal 渲染为“近期民意 (最近 5 分钟)”。

## 反方审查记录

第一轮质疑：

- `ChatSignal` schema 本身没有 `ts`，可能原设计是“当前民意摘要”而非事件流。
- 影响不是无界累积：`CHAT_CONTEXT_MAX_SIGNALS=20`，prompt 只取末 5 条，高聊天量会自然顶掉。
- `chatSignalsBlock` 是 optional，上下文 token 压力大时可能被裁掉。
- LLM 偶然输出 `mentions_mechanic: "ts:..."` 时过期逻辑可生效，但 prompt 没要求，也没有把 `msg.ts` 给 LLM，不能当可靠保护。
- 开放 PR 未发现覆盖该问题；#970 是暗器充能天道叙事断链，不重复。

补证 / 让步：

- 接受严重度边界：这是低聊天量 / 长时间运行下的上下文污染，不是无界内存泄漏、Redis 数据损坏或必现 server 崩溃。
- 补充确认：server 和 schema 都有 `ts`，但 `processChatBatch` 的 prompt、fallback、candidate 三处都不保留。
- 补充确认：过期逻辑只认 `mentions_mechanic` 中的 `ts:`，无时间戳时显式返回 recent。
- 补充确认：信号最终进入 TiandaoAgent 的 LLM user prompt，并被标为“最近 5 分钟”。

最终裁决：

- 反方通过。结论：这是时间窗口契约断链，足够作为 BugHunt skeleton plan。

## Skeleton Fix Plan

- [ ] 在 `ChatSignal` 契约中增加显式 `ts` 字段，或引入内部 `ObservedChatSignal` 类型，避免把时间塞进 `mentions_mechanic`。
- [ ] 修改 `processChatBatch`：fallback 与 LLM 标注成功路径都从对应 `ChatMessageV1.ts` 写入 signal 时间。
- [ ] 修改 `buildAnnotatePrompt` / `ChatSignalInput`：不要依赖 LLM 回传时间戳；LLM 只负责语义标注，时间由 runtime 合并。
- [ ] 修改 `isRecentSignal`：对生产聊天 signal 必须按显式 `ts` 判断；仅对测试/手工注入无时间 signal 保留清晰兼容策略。
- [ ] 补齐 tiandao 单测：`processChatBatch` 保留 `ts`、`mergeChatSignals` 清掉 5 分钟外旧聊天、`buildChatSignalsBlock` 不渲染过期 signal。
- [ ] 若改 `@bong/schema`，同步重建 schema dist 并补 schema pin 测试，避免 agent 包引用旧构建产物。

## 验收测试计划

- `cd agent/packages/schema && npm test`
- `cd agent/packages/tiandao && npm test`
- `cd agent && npm run build`
- 增加一条回归：构造 `ts=now-301` 的聊天 signal，下一轮 `mergeChatSignals(..., now)` 后不再进入 `chatSignalsBlock`。
- 增加一条回归：低聊天量场景中，只有一条旧聊天时，5 分钟后 prompt 不再包含“近期民意”中的旧 raw 文本。

## 风险

- 改 `ChatSignal` schema 会牵动 `@bong/schema` dist、tiandao 测试 fixtures 和手工注入测试。
- 如果直接复用 `mentions_mechanic` 存 `ts:`，会继续污染语义字段，后续 LLM 标注可能覆盖或拼接出错。
- 如果无时间戳兼容策略过严，现有测试/手工构造的 `ChatSignal` 可能需要补 `ts`。
- 修复后低活跃服务器的天道响应会更“短记忆”，需要确认这符合“最近 5 分钟民意”的设计语义。
