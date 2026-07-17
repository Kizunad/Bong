# plan-bughunt-chat-signal-stale-window

> **Finished BugFix Plan（2026-07-17 纠错收口）**。历史来源：
> `docs/plans-skeleton/plan-bughunt-chat-signal-stale-window.md`。
> 本 plan 先修复 agent 在语义标注转换时丢失聊天时间戳的问题；真实协议验真随后发现
> Minecraft 1.20.1 C2S 提供 Unix **毫秒**，server 却曾把它原样写入 Unix **秒**契约。
> 最终闭环同时固定 Python Bot 协议时间、Rust server→agent 单位转换、Tiandao 五分钟窗口和真实 e2e。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | 修复前契约测试与可达性证真 | ✅ 2026-07-15 |
| P1 | `ChatSignal.ts` 契约与 runtime 注入 | ✅ 2026-07-15 |
| P2 | schema / tiandao / workspace 完整门禁 | ✅ 2026-07-15 |
| P3 | 同步主线、跨栈复验、Finish Evidence 纠错与归档 | ✅ 2026-07-17 |

## Bug 摘要

`ChatMessageV1.ts` 的共享契约是 Unix 秒，但修复前存在两处断链：

1. `agent/packages/tiandao/src/chat-processor.ts` 把聊天转成 `ChatSignal` 时丢掉 `ts`；`mergeChatSignals` / `buildChatSignalsBlock` 只能从 `mentions_mechanic` 猜 `ts:<digits>`，普通聊天会被视为永远 recent。
2. Valence 的 Minecraft 1.20.1 `ChatMessageC2s.timestamp` 是 Unix 毫秒；server 曾把该 13 位值原样写入 `ChatMessageV1.ts`。即使 agent 改为显式保留时间，毫秒值相对 Unix 秒仍像“遥远未来”，同样无法过期。

真实 Bot 原先又发送固定 `0` 时间戳；server 正确归一化后它会像 1970 年消息一样被窗口淘汰。因此最终修复还必须让 Python Bot 按原版协议发送当前 Unix 毫秒，才能以真实链路验真而不是靠伪 payload 自证。

结果：低聊天量或长时间运行时，几小时前的玩家抱怨 / 求助 / 挑衅仍可能作为“近期民意 (最近 5 分钟)”进入天道 LLM prompt。

## 实际游玩体验影响

- 玩家一句旧聊天可能在很久后继续影响天道灾劫 / 变异 / 时代推演上下文，让天道像是在响应刚发生的民意。
- 低活跃服务器最明显：没有 20 条新聊天顶掉旧 signal 时，旧内容会长期残留。
- 表现不是崩溃或数据损坏，而是天道决策上下文污染：玩家会看到天道反馈与当前局势脱节。

## 证据定位

- `server/src/network/chat_collector.rs`：`ChatMessageV1.ts = context.timestamp / UNIX_MILLIS_PER_SECOND`，只在 server→agent wire 边界转换为 Unix 秒；`PlayerChatCollected.timestamp` 继续保留协议毫秒。
- `server/src/network/chat_collector.rs` 测试：真实 13 位毫秒以及 `0 / 1000 / 1999` 边界固定 `/ 1000` 截断，并断言 server 内部事件未被改单位。
- `scripts/bot/bot.py`：`Bot.chat()` 用 `time.time_ns() // 1_000_000` 写入 C2S signed 64-bit Unix 毫秒；`scripts/bot/test_protocol.py` 对实际发送字节做边界 pin。
- `scripts/e2e-chat-signal-window.sh`、`scripts/e2e/chat-signal-window.mts`：启动本次 Rust server 与独立 Redis，用真实 Python Bot 发聊天，再由 Tiandao drain、标注、检查 300/301 秒边界及 prompt block。
- `agent/packages/schema/src/chat-message.ts`：`ChatMessageV1` 与 `ChatSignal` 现在都要求非负整数 `ts`；`agent/packages/schema/generated/chat-signal.json` 把它列为 required。
- `agent/packages/tiandao/src/chat-processor.ts`：`buildAnnotatePrompt(messages)` 仍只把 `player/zone/raw` 交给不可信 LLM；fallback / candidate 都从匹配的 `msg.ts` 注入 `ChatSignal.ts`。
- `agent/packages/tiandao/src/chat-processor.ts`：`isRecentSignal` 直接读取显式 `signal.ts`，不再从 `mentions_mechanic` 解析隐式时间。
- `agent/packages/tiandao/src/runtime.ts:1246` 和 `:1282` 附近：runtime 每轮用 `mergeChatSignals(..., nowSeconds)` 试图清理旧 signal，但无时间戳 signal 不会过期。
- `agent/packages/tiandao/src/agent.ts:91`、`:128` 附近与 `agent/packages/tiandao/src/context.ts:233` 附近：`setChatSignals -> createContextInput -> chatSignalsBlock -> buildChatSignalsBlock`，这些 signal 会进入 LLM prompt。

## 触发路径

1. 原版客户端或 Python Bot 发送 `ChatMessageC2s`，协议字段为 Unix 毫秒。
2. server 在 `server/src/network/chat_collector.rs` 只对 Redis `bong:player_chat` 的 `ChatMessageV1.ts` 做 `/ 1000`，server 内部 `PlayerChatCollected.timestamp` 仍为毫秒。
3. tiandao runtime drain `bong:player_chat`，调用 `processChatBatch`。
4. `processChatBatch` 从匹配的 `ChatMessageV1.ts` 注入 `ChatSignal.ts`，不采信 LLM 伪造时间。
5. 后续 tick 调用 `mergeChatSignals(latestChatSignals, [], nowSeconds)`；`now - 300` 保留，`now - 301` 淘汰。
6. agent tick 只把窗口内 signal 渲染为“近期民意 (最近 5 分钟)”。

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
- **跨栈边界**：`ChatMessageC2s.timestamp` 是 Unix 毫秒，`ChatMessageV1.ts` 是 Unix 秒；Rust 必须在唯一 server→agent 边界转换，Python Bot 必须模拟原版毫秒协议。client 不消费 `ChatSignal`，无需业务改动。
- **worldview / qi_physics**：这是 agent 上下文时效性修复，不改变世界观、玩法数值或任何真元/灵气转移。

## 实施阶段

- [x] P0（✅ 2026-07-15）：先补修复前应失败的契约测试：schema 要求显式 `ts`；fallback 与有效 annotation 都保留原消息时间；LLM 伪造时间不能覆盖；301 秒旧信号从 merge 和 prompt 中消失。
- [x] P1（✅ 2026-07-15）：在 `ChatSignal` TypeBox schema 增加必填 `ts`，重生成 `generated/chat-signal.json`；在 `processChatBatch()` 的 fallback / candidate 从 `msg.ts` 注入，并让 `isRecentSignal()` 直接读取显式字段。
- [x] P2（✅ 2026-07-15）：补齐所有 `ChatSignal` fixture 的权威测试时间，运行 schema build/test/generate check、tiandao test 与 agent workspace build。
- [x] P3（✅ 2026-07-17）：补齐 Rust `/ 1000` wire 转换、Python Bot Unix 毫秒、字节级协议测试、独立 Redis 真实 e2e 与 CI 接线；在 `5f04b7a0` 上完成第一轮全门禁和真实链路后，紧邻 fetch 合并 `origin/main@28cc3af4`，对未提交 merge 树完成 server/client/Python/真实 e2e 第二轮复验，再以 `38439bd1` 落 merge commit。最后原地纠正既有归档文档，不重复 promotion、归档或 `Finish Evidence`。

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
| 协议单位 | Python Bot 发 Unix 毫秒；Rust wire 对真实 13 位值及 `0/1000/1999` 做 `/ 1000`，内部事件仍保留毫秒 |
| 真实生产链 | Python Bot→MC C2S→Rust→独立 Redis→Tiandao→300/301 秒边界→prompt block 全部命中 |
| 完整门禁 | schema/tiandao/workspace、server、Java 17 client、Python protocol 与两次真实 e2e 全绿 |

## 非目标

- 不改变 Redis channel、客户端时钟来源或 server 内部 `PlayerChatCollected.timestamp` 的毫秒语义；只纠正 `ChatMessageV1.ts` wire 的秒单位。
- 不把聊天时间交给 LLM 生成、修正或回传。
- 不改变 5 分钟窗口、20 条 merge 上限、prompt 末 5 条展示或 token budget 裁剪。
- 不增加 Redis 持久化、跨进程聊天记忆或未来时间戳校准。

## 验收测试计划

- `cd agent && npm run build -w @bong/schema`
- `cd agent/packages/schema && npm test && npm run check`
- `cd agent/packages/tiandao && npm test`
- `cd agent && npm run build`
- 定向 red/green：`npm test -w @bong/schema -- --run tests/chat-message.test.ts` 与 `npm test -w @bong/tiandao -- --run tests/chat-processor.test.ts`。
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `cd client && JAVA_HOME=<JDK17> PATH=<JDK17>/bin:$PATH ./gradlew test build`
- `python3 -m unittest scripts.bot.test_protocol`
- `bash scripts/e2e-chat-signal-window.sh`

## 风险

- 改 `ChatSignal` schema 会牵动 `@bong/schema` dist、tiandao 测试 fixtures 和手工注入测试。
- 如果直接复用 `mentions_mechanic` 存 `ts:`，会继续污染语义字段，后续 LLM 标注可能覆盖或拼接出错。
- `ChatSignal.ts` 改为必填会使现有手工 fixture 编译失败；必须逐个补与测试 `nowSeconds` 一致的显式时间，不能用全局 optional 逃避契约。
- 修复后低活跃服务器的天道响应会更“短记忆”，需要确认这符合“最近 5 分钟民意”的设计语义。

## Finish Evidence

### 落地清单

- P0：`agent/packages/schema/tests/chat-message.test.ts`、`agent/packages/tiandao/tests/chat-processor.test.ts` 与 `agent/packages/tiandao/tests/agent-real-context-injection.test.ts` 先锁定 schema、转换、窗口边界和真实 LLM prompt 的修复前失败契约。
- P1：`agent/packages/schema/src/chat-message.ts` 为 `ChatMessageV1.ts` 与 `ChatSignal.ts` 固定非负整数契约；`agent/packages/tiandao/src/chat-processor.ts` 从原始消息注入权威时间，并移除 `mentions_mechanic` 隐式时间解析。
- P2：`agent/packages/schema/generated/chat-message-v1.json` 与 `agent/packages/schema/generated/chat-signal.json` 已重生成；tiandao context/runtime fixtures 均补齐与测试时钟一致的显式时间。后续纠错在 `server/src/network/chat_collector.rs` 固定 server→agent `/ 1000` 边界，并用测试锁住真实 13 位 Unix 毫秒、`0/1000/1999` 截断及内部毫秒事件不变。
- P3：`scripts/bot/bot.py`、`scripts/bot/test_protocol.py` 固定真实 Bot C2S Unix 毫秒与字节布局；`scripts/e2e-chat-signal-window.sh`、`scripts/e2e/chat-signal-window.mts` 覆盖 Bot→Rust→独立 Redis→Tiandao→窗口→prompt；`.github/workflows/e2e.yml` 把该链路接入 e2e。合并 `origin/main@28cc3af4` 后按受影响栈复验并以 `38439bd1` 落 merge commit。

### 关键 commit

- `8ff6eab4`（2026-07-15）：复现聊天信号五分钟窗口失效。
- `d87323cd`（2026-07-15）：收紧聊天时间戳 schema 契约。
- `bf4c795f`（2026-07-15）：修复聊天信号过期窗口。
- `e7ad238e`（2026-07-15）：补齐聊天时效上下文回归。
- `345e68e8`（2026-07-15）：回填完整门禁与实现基线 Finish Evidence。
- `ea9849ba`（2026-07-15）：将已完成 plan 移入 `docs/finished_plans/`。
- `d1a8f945`（2026-07-17）：保全 PR #1215 现场，修正 Rust server→agent 毫秒/秒 wire 并补边界测试。
- `2482743a`（2026-07-17）：补齐真实 Bot Unix 毫秒、Python 字节 pin、独立 Redis e2e 与 CI 接线。
- `5f04b7a0`（2026-07-17）：把真实 e2e runner 改为 `.mts`，强制 ESM 正确加载 `@bong/schema`。
- `38439bd1`（2026-07-17）：合并 `origin/main@28cc3af4`，落地已复验的跨栈 merge 树。

### 测试结果

- 第一轮（`5f04b7a0`，合并主线前）：schema 30 个测试文件 / 892 项通过，405 份 generated schema fresh；tiandao 72 个测试文件 / 833 项通过；agent workspace build 通过；server `fmt`、`clippy --all-targets -D warnings`、`cargo test` 通过（11,708 passed / 6 ignored / 0 failed）；Python protocol 90/90 通过。
- 第一轮真实 e2e：marker `chat-window-20260717-173629-3388360`，`wire_ts=1784280991 now=1784280992 age=1`；证据目录 `.sisyphus/evidence/chat-signal-window-20260717-173629-3388360`。真实 Bot→MC→Rust→独立 Redis→Tiandao→300/301 秒边界→prompt block 全部 PASS。
- 第二轮（未提交 merge 树 `5f04b7a0 + origin/main@28cc3af4`，随后落 `38439bd1`）：server `fmt`、`clippy` 与 `cargo test` 通过（11,736 passed / 6 ignored / 0 failed）；Temurin 17.0.19 client `./gradlew test build` 通过（4,095 tests / 0 failures / 0 errors / 0 skipped）；Python protocol 125/125 通过；`git diff --check` 通过。主线 merge 未触及本 plan 修改的 `agent/packages/schema` / `agent/packages/tiandao` 文件，因此第一轮 agent 完整门禁覆盖的内容在 merge 树中未变。
- 第二轮真实 e2e：marker `chat-window-20260717-185347-3643728`，`wire_ts=1784285803 now=1784285805 age=2`；证据目录 `.sisyphus/evidence/chat-signal-window-20260717-185347-3643728`。脚本退出 0；25565 listener 经 PID 树校验属于本次 Rust server，独立 Redis 随脚本启动并清理，退出后 25565 为空。
- 定向契约覆盖：schema 缺失/负数/小数/字符串/额外字段，fallback/annotation 权威时间，LLM 伪造隔离，300/301 秒边界，先过滤后 20 条截断，真实 `TiandaoAgent` prompt 旧信号淘汰，Python C2S 毫秒字节布局，以及 Rust wire 单位转换与内部事件隔离。

### 跨仓库核验

- server：`server/src/network/chat_collector.rs` 把协议毫秒在唯一 Redis wire 边界转换为 `ChatMessageV1.ts` Unix 秒，同时保留 `PlayerChatCollected.timestamp` 毫秒语义。
- Python Bot：`scripts/bot/bot.py` 模拟原版 `PacketByteBuf.writeInstant()` 的 signed Unix 毫秒；`scripts/bot/test_protocol.py` 对发包字节和边界做 pin。
- agent/schema：`ChatMessageV1`、`ChatSignal`、`processChatBatch`、`mergeChatSignals`、`buildChatSignalsBlock` 与 committed generated schema 已对齐。
- client：不消费 `ChatSignal` 内部契约；因合并主线带入 client 变更，仍用 Java 17 完成 4,095 项全量回归。
- CI/runtime：`.github/workflows/e2e.yml` 调用 `scripts/e2e-chat-signal-window.sh`；runner 用独立 Redis 和真实 server listener 归属校验，避免误连旧进程造成假绿。

### 遗留 / 后续

- 无代码遗留或阻塞标记；未来时间戳校准与跨进程聊天持久化仍按本 plan 非目标保持独立。
- 两个全新只读 validator 分别对 `2482743a` 与 `5f04b7a0` 给出 FAIL，唯一 blocker 都是本归档 plan 仍错误声称“无需改 Rust / 不改变 server wire”且漏记 Rust、Python 和真实 e2e 证据；两轮均未发现额外代码或运行时 blocker。本提交只纠正该文档失真。
- 代码与 merge 树的已验证 SHA 是 `38439bd1`。归档文档提交会产生新的最终 HEAD，不能在自身内容中稳定写入自己的 SHA 或预写未来 validator PASS；最终 HEAD、独立 validator 结论、GitHub e2e、`/review` 与 CodeRabbit 统一在 PR body / 评论 / checks 中外部绑定。
