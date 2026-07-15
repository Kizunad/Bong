# plan-bughunt-chat-signal-stale-window

> **Active BugFix Plan（2026-07-15 升格）**。历史来源：
> `docs/plans-skeleton/plan-bughunt-chat-signal-stale-window.md`。
> 本 plan 修复 server 已提供权威聊天 Unix 秒时间戳、但 agent 在语义标注转换时丢失，
> 导致“最近 5 分钟”窗口无法淘汰低流量旧聊天的问题。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | 修复前契约测试与可达性证真 | ⬜ |
| P1 | `ChatSignal.ts` 契约与 runtime 注入 | ⬜ |
| P2 | schema / tiandao / workspace 完整门禁 | ⬜ |
| P3 | 同步主线、自审、Finish Evidence 与归档 | ⬜ |

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

## 接入面与收口决议

- **进料**：server 在 `bong:player_chat` 发布的 `ChatMessageV1.ts` 是唯一权威观察时间；LLM 只标注 sentiment / intent / influence / mechanic，不参与时间判定。
- **出料**：`processChatBatch()` 生成携带 `ts` 的 `ChatSignal`，经 `mergeChatSignals()` 写入 `latestChatSignals`，最终由 `buildChatSignalsBlock()` 决定是否进入天道 user prompt。
- **共享契约**：直接把 `ts: integer >= 0` 加入现有 `@bong/schema` `ChatSignal`，不再另造 `ObservedChatSignal`；该类型当前只在 agent 内消费，收紧后所有手工 fixture 都必须显式声明观察时间。
- **窗口语义**：继续使用现有 `CHAT_CONTEXT_WINDOW_SECONDS = 300` 和 `>=` 判定；`ts == now - 300` 保留，`ts == now - 301` 淘汰。空数组与超过 20 条的截断语义不变。
- **不可信标注**：`ChatSignalInput` 不增加 `ts`；`buildAnnotatePrompt()` 继续不向 LLM 暴露时间，候选和 fallback 都只从对应 `ChatMessageV1.ts` 注入。即使 LLM 输出伪造 `ts`，parser 也不得采用。
- **去除隐式编码**：删除从 `mentions_mechanic` 解析 `ts:<digits>` 的 fallback。该字段只表达语义机制；`mentions_mechanic="ts:1"` 不能覆盖显式 `ChatSignal.ts`。
- **兼容策略**：`ChatSignal.ts` 必填，不保留“缺时间永远 recent”的生产兼容；schema pin 必须拒绝缺失、负数、非整数和额外字段。
- **未来时间戳**：来源是 server 权威 Unix 秒，本 plan 不另加客户端时钟校准或 future clamp；未来时间异常属于独立输入质量问题，不扩大本次修复。
- **跨栈边界**：server wire `ChatMessageV1` 已有 `ts`，无需改 Rust；client 不消费该内部信号。改动限定 schema + tiandao + tracked generated schema。
- **worldview / qi_physics**：这是 agent 上下文时效性修复，不改变世界观、玩法数值或任何真元/灵气转移。

## 实施阶段

- [ ] P0：先补修复前应失败的契约测试：schema 要求显式 `ts`；fallback 与有效 annotation 都保留原消息时间；LLM 伪造时间不能覆盖；301 秒旧信号从 merge 和 prompt 中消失。
- [ ] P1：在 `ChatSignal` TypeBox schema 增加必填 `ts`，重生成 `generated/chat-signal.json`；在 `processChatBatch()` 的 fallback / candidate 从 `msg.ts` 注入，并让 `isRecentSignal()` 直接读取显式字段。
- [ ] P2：补齐所有 `ChatSignal` fixture 的权威测试时间，运行 schema build/test/generate check、tiandao test 与 agent workspace build。
- [ ] P3：fetch 后按 merge-base 同步最新 `origin/main`；若 HEAD 变化则重跑受影响门禁；主 agent 逐入口对抗自审，填写 Finish Evidence 后归档。

## 验收矩阵

| 场景 | 必须断言 |
|---|---|
| schema 正样本 | 非负整数 `ts` 通过，generated schema 将 `ts` 列为 required |
| schema 负样本 | 缺失、负数、小数、字符串 `ts` 与额外字段全部拒绝 |
| fallback | 缺失 annotation 时 `ChatSignal.ts === ChatMessageV1.ts` |
| 有效 annotation | 原消息顺序和语义标注保持，时间仍来自匹配的原消息 |
| LLM 伪造时间 | annotation 中的 `ts` 不得覆盖 server 时间 |
| 窗口下界 | `now - 300` 保留，`now - 301` 淘汰 |
| 语义字段隔离 | `mentions_mechanic="ts:..."` 不影响显式时间判定 |
| merge 容量 | 先按时间过滤，再保留最近 20 条的既有行为不变 |
| prompt 低流量回归 | 仅一条 301 秒旧聊天时不渲染“近期民意”或旧 raw；边界内文本仍渲染 |
| 完整门禁 | schema test/check/build、tiandao test、workspace build 全绿 |

## 非目标

- 不改变 server `ChatMessageV1` wire、Redis channel 或聊天采集时钟。
- 不把聊天时间交给 LLM 生成、修正或回传。
- 不改变 5 分钟窗口、20 条 merge 上限、prompt 末 5 条展示或 token budget 裁剪。
- 不增加 Redis 持久化、跨进程聊天记忆或未来时间戳校准。

## 验收测试计划

- `cd agent && npm run build -w @bong/schema`
- `cd agent/packages/schema && npm test && npm run check`
- `cd agent/packages/tiandao && npm test`
- `cd agent && npm run build`
- 定向 red/green：`npm test -w @bong/schema -- --run tests/chat-message.test.ts` 与 `npm test -w @bong/tiandao -- --run tests/chat-processor.test.ts`。

## 风险

- 改 `ChatSignal` schema 会牵动 `@bong/schema` dist、tiandao 测试 fixtures 和手工注入测试。
- 如果直接复用 `mentions_mechanic` 存 `ts:`，会继续污染语义字段，后续 LLM 标注可能覆盖或拼接出错。
- `ChatSignal.ts` 改为必填会使现有手工 fixture 编译失败；必须逐个补与测试 `nowSeconds` 一致的显式时间，不能用全局 optional 逃避契约。
- 修复后低活跃服务器的天道响应会更“短记忆”，需要确认这符合“最近 5 分钟民意”的设计语义。
