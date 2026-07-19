# plan-bughunt-chat-signal-stale-window

> **Finished BugFix Plan（2026-07-17 纠错收口）**。历史来源：
> `docs/plans-skeleton/plan-bughunt-chat-signal-stale-window.md`。
> 本 plan 先修复 agent 在语义标注转换时丢失聊天时间戳的问题；后续 review 又证实
> Minecraft 1.20.1 C2S 时间由客户端提供，不能作为 freshness 权威。最终闭环以
> server observation clock 生成 Redis Unix 秒，同时保留内部 C2S 毫秒元数据，并补齐
> Tiandao 五分钟窗口上下界、异步 merge 前时钟刷新、可伪造 Bot 协议 seam 与真实 e2e。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | 修复前契约测试与可达性证真 | ✅ 2026-07-15 |
| P1 | `ChatSignal.ts` 契约与 runtime 注入 | ✅ 2026-07-15 |
| P2 | schema / tiandao / workspace 完整门禁 | ✅ 2026-07-15 |
| P3 | 同步主线、跨栈复验、Finish Evidence 纠错与归档 | ✅ 2026-07-17 |

## Bug 摘要

`ChatMessageV1.ts` 的共享契约是 server-observed Unix 秒，但修复链路先后暴露三处断链：

1. `agent/packages/tiandao/src/chat-processor.ts` 把聊天转成 `ChatSignal` 时丢掉 `ts`；`mergeChatSignals` / `buildChatSignalsBlock` 只能从 `mentions_mechanic` 猜 `ts:<digits>`，普通聊天会被视为永远 recent。
2. Valence 的 Minecraft 1.20.1 `ChatMessageC2s.timestamp` 是客户端可控 Unix 毫秒。早期实现让 Redis freshness 直接派生自客户端字段；即使修正单位，信任边界仍未闭合。恶意或偏时钟客户端只需伪造 `+1d`，便可制造一直处于未来的 signal，绕过只有下界的窗口。
3. runtime 在异步 Redis drain / LLM 标注之前缓存 `now`，若合法 server-observed 消息跨秒抵达，使用旧时钟 merge 会把 `ts == old_now + 1` 错当未来并删掉。

真实 Bot 原先又发送固定 `0` 时间戳。最终 Bot 默认按原版协议发送当前 Unix 毫秒，同时允许测试显式传入 `timestamp_millis`；真实 e2e 因而能在登录后紧贴聊天伪造 `+1d`，证明 server 未采信客户端时间，而不是靠伪 Redis payload 自证。

结果：低聊天量或长时间运行时，几小时前的玩家抱怨 / 求助 / 挑衅仍可能作为“近期民意 (最近 5 分钟)”进入天道 LLM prompt。

## 实际游玩体验影响

- 玩家一句旧聊天可能在很久后继续影响天道灾劫 / 变异 / 时代推演上下文，让天道像是在响应刚发生的民意。
- 低活跃服务器最明显：没有 20 条新聊天顶掉旧 signal 时，旧内容会长期残留。
- 偏时钟或恶意客户端还可把消息伪造成未来事件，使其避开旧实现的五分钟下界；旧 runtime 时钟复用则可能反向误删跨秒到达的合法消息。
- 表现不是崩溃或数据损坏，而是天道决策上下文污染：玩家会看到天道反馈与当前局势脱节。

## 证据定位

- `server/src/network/chat_collector.rs`：`ChatObservationClock` 以 server wall clock 生成 `ChatMessageV1.ts`；客户端 C2S 毫秒只保留在既有 `PlayerChatCollected.timestamp` 内部事件元数据链。
- `server/src/network/chat_collector.rs` 测试：客户端落后、相等、领先、epoch 与 `u64::MAX` 均不能改变 Redis wire 秒；内部事件仍逐值保留原始 C2S 毫秒。
- `scripts/bot/bot.py`：`Bot.chat()` 默认用 `time.time_ns() // 1_000_000` 写入 C2S signed 64-bit Unix 毫秒，并允许显式 `timestamp_millis`；`scripts/bot/test_protocol.py` 对默认与伪造时间的实际发送字节做 pin。
- `scripts/e2e-chat-signal-window.sh`、`scripts/e2e/chat-signal-window.mts`：启动本次 Rust server 与独立 Redis，真实 Bot 登录后伪造 `+1d` 发聊，再由 Tiandao drain、标注、检查 future upper、300/301 秒下界及 prompt block。
- `agent/packages/schema/src/chat-message.ts`：`ChatMessageV1.ts` 描述明确为 `Server-observed Unix timestamp (seconds)`；`ChatSignal.ts` 同样为必填非负整数，generated schema 与源码一致。
- `agent/packages/tiandao/src/chat-processor.ts`：`buildAnnotatePrompt(messages)` 仍只把 `player/zone/raw` 交给不可信 LLM；fallback / candidate 都从匹配的 `msg.ts` 注入 `ChatSignal.ts`。
- `agent/packages/tiandao/src/chat-processor.ts`：`isRecentSignal` 直接读取显式 `signal.ts`，以闭区间 `[now - 300, now]` 同时拒绝旧信号与未来信号，不再从 `mentions_mechanic` 解析隐式时间。
- `agent/packages/tiandao/src/runtime.ts`：`RuntimeDeps.now` 可注入；runtime 在异步 drain / LLM 标注完成后、实际 `mergeChatSignals` 前重新取时钟，避免复用 loop 开始时的旧秒。
- `agent/packages/tiandao/src/agent.ts:91`、`:128` 附近与 `agent/packages/tiandao/src/context.ts:233` 附近：`setChatSignals -> createContextInput -> chatSignalsBlock -> buildChatSignalsBlock`，这些 signal 会进入 LLM prompt。

## 触发路径

1. 原版客户端或 Python Bot 发送 `ChatMessageC2s`；其 Unix 毫秒字段按不可信客户端元数据处理。
2. server 将原始 C2S 毫秒保留到 `PlayerChatCollected.timestamp`，同时从 `ChatObservationClock` 独立生成 Redis `bong:player_chat` 的 `ChatMessageV1.ts` Unix 秒。
3. tiandao runtime drain `bong:player_chat`，调用 `processChatBatch`。
4. `processChatBatch` 从匹配的 `ChatMessageV1.ts` 注入 `ChatSignal.ts`，不采信 LLM 伪造时间。
5. runtime 在异步标注后刷新 `nowSeconds` 再 merge；`ts == now - 300` 保留，`ts == now - 301` 与 `ts > now` 淘汰。
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
- PR review 进一步证真客户端时间不可信与 runtime 跨秒旧时钟问题；`FIX_VALIDATING` 在 `057d4d52`、`REBASE_VALIDATING` 在 `438c47e7` 分别取得全新只读 validator PASS。

## 接入面与收口决议

- **进料**：server observation clock 在 `bong:player_chat` 发布的 `ChatMessageV1.ts` 是唯一权威观察时间；客户端 C2S 毫秒与 LLM 标注都不参与 freshness 判定。
- **出料**：`processChatBatch()` 生成携带 `ts` 的 `ChatSignal`，经 `mergeChatSignals()` 写入 `latestChatSignals`，最终由 `buildChatSignalsBlock()` 决定是否进入天道 user prompt。
- **共享契约**：直接把 `ts: integer >= 0` 加入现有 `@bong/schema` `ChatSignal`，不再另造 `ObservedChatSignal`；该类型当前只在 agent 内消费，收紧后所有手工 fixture 都必须显式声明观察时间。
- **窗口语义**：继续使用 `CHAT_CONTEXT_WINDOW_SECONDS = 300`，窗口为闭区间 `[now - 300, now]`；`ts == now - 300` 与 `ts == now` 保留，`ts == now - 301` 与 `ts > now` 淘汰。空数组与超过 20 条的截断语义不变。
- **不可信标注**：`ChatSignalInput` 不增加 `ts`；`buildAnnotatePrompt()` 继续不向 LLM 暴露时间，候选和 fallback 都只从对应 `ChatMessageV1.ts` 注入。即使 LLM 输出伪造 `ts`，parser 也不得采用。
- **去除隐式编码**：删除从 `mentions_mechanic` 解析 `ts:<digits>` 的 fallback。该字段只表达语义机制；`mentions_mechanic="ts:1"` 不能覆盖显式 `ChatSignal.ts`。
- **兼容策略**：`ChatSignal.ts` 必填，不保留“缺时间永远 recent”的生产兼容；schema pin 必须拒绝缺失、负数、非整数和额外字段。
- **未来时间戳**：Tiandao 必须 fail closed 拒绝 `ts > now`，作为 server 边界之后的纵深保护；本 plan 不引入 NTP、时钟漂移补偿或客户端时钟校准协议。
- **异步时钟**：`RuntimeDeps.now` 必须在 drain / LLM 完成后、实际 merge 前刷新；loop 开始时的旧秒只能用于当轮初始清理，不能决定新消息是否合法。
- **跨栈边界**：`ChatMessageC2s.timestamp` 是不可信 Unix 毫秒，只留在 server 内部事件元数据；`ChatMessageV1.ts` 是 server-observed Unix 秒。Python Bot 必须既能模拟原版当前毫秒，也能显式伪造协议时间。client 不消费 `ChatSignal`，无需本 plan 业务改动。
- **worldview / qi_physics**：这是 agent 上下文时效性修复，不改变世界观、玩法数值或任何真元/灵气转移。

## 实施阶段

- [x] P0（✅ 2026-07-15）：先补修复前应失败的契约测试：schema 要求显式 `ts`；fallback 与有效 annotation 都保留原消息时间；LLM 伪造时间不能覆盖；301 秒旧信号从 merge 和 prompt 中消失。
- [x] P1（✅ 2026-07-15）：在 `ChatSignal` TypeBox schema 增加必填 `ts`，重生成 `generated/chat-signal.json`；在 `processChatBatch()` 的 fallback / candidate 从 `msg.ts` 注入，并让 `isRecentSignal()` 直接读取显式字段。
- [x] P2（✅ 2026-07-15）：补齐所有 `ChatSignal` fixture 的权威测试时间，运行 schema build/test/generate check、tiandao test 与 agent workspace build。
- [x] P3（✅ 2026-07-17）：先补齐 Python Bot Unix 毫秒、字节级协议测试、独立 Redis 真实 e2e 与 CI 接线；review 返工再以 `ChatObservationClock` 取代客户端 wire 时间，增加 Tiandao future upper、`RuntimeDeps.now` merge 前刷新与伪造 `+1d` e2e。最终对 `origin/main@9d2e29d0` 的未提交 merge 树复跑所有受影响栈，并以 `438c47e7` 落显式 merge commit；既有归档文档只原地纠正，不重复 promotion、归档或 `Finish Evidence`。

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
| 窗口上界 | `ts == now` 保留，`ts == now + 1` 与 `ts == now + 86400` 淘汰 |
| merge 容量 | 先按时间过滤，再保留最近 20 条的既有行为不变 |
| prompt 低流量回归 | 仅一条 301 秒旧聊天时不渲染“近期民意”或旧 raw；边界内文本仍渲染 |
| 信任边界 | Python Bot 发 C2S Unix 毫秒；客户端负偏移、正偏移、epoch、极值均不影响 Redis server-observed 秒，内部事件仍保留原值 |
| 异步跨秒 | drain / LLM 期间跨秒后，合法 server-observed signal 保留，伪造未来 signal 淘汰 |
| 真实生产链 | Python Bot 登录后伪造 `+1d`→MC C2S→Rust server observation→独立 Redis→Tiandao future/300/301 秒边界→prompt block 全部命中 |
| 完整门禁 | schema/tiandao/workspace、server、Java 17 client、Python protocol 与主线合并前后真实 e2e 全绿 |

## 非目标

- 不改变 Redis channel 或 server 内部 `PlayerChatCollected.timestamp` 的客户端毫秒语义；freshness 改由独立 server observation clock 决定。
- 不把聊天时间交给 LLM 生成、修正或回传。
- 不改变 5 分钟窗口、20 条 merge 上限、prompt 末 5 条展示或 token budget 裁剪。
- 不增加 Redis 持久化、跨进程聊天记忆、NTP 协议或客户端时钟漂移补偿；但 `ts > now` 已作为纵深保护明确拒绝。

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
- 若 runtime 在异步 drain / LLM 前缓存时钟并复用到 merge，闭区间上界会误删跨秒合法消息；必须保留 merge 前刷新回归。
- 修复后低活跃服务器的天道响应会更“短记忆”，需要确认这符合“最近 5 分钟民意”的设计语义。

## Finish Evidence

### 落地清单

- P0：`agent/packages/schema/tests/chat-message.test.ts`、`agent/packages/tiandao/tests/chat-processor.test.ts`、`agent/packages/tiandao/tests/agent-real-context-injection.test.ts` 先锁定 schema、转换、300/301 秒窗口和真实 LLM prompt；review 返工又以 `aabfc244`、`99b0af1c` 分别复现客户端未来时间绕过与跨秒旧时钟误删。
- P1：`agent/packages/schema/src/chat-message.ts` 为 `ChatMessageV1.ts` 明确 server-observed 秒契约、为 `ChatSignal.ts` 固定必填非负整数；`agent/packages/tiandao/src/chat-processor.ts` 从原消息注入权威时间，删除 `mentions_mechanic` 隐式解析，并以 `[now - 300, now]` 同时过滤旧值与未来值。
- P2：`agent/packages/schema/generated/chat-message-v1.json`、`generated/chat-signal.json` 已重生成；`agent/packages/tiandao/src/runtime.ts` 增加 `RuntimeDeps.now` 注入面并在异步 drain / LLM 后、实际 merge 前刷新时钟，context/runtime fixtures 与跨秒回归全部对齐。
- P3：`server/src/network/chat_collector.rs` 引入 `ChatObservationClock`，Redis wire 使用 server observation 秒，`PlayerChatCollected.timestamp` 保留原始 C2S 毫秒；`scripts/bot/bot.py` / `test_protocol.py` 支持默认当前毫秒与显式伪造毫秒；`scripts/e2e-chat-signal-window.sh` / `.mts` 覆盖登录后 `+1d` Bot→Rust→独立 Redis→Tiandao→future/300/301 秒→prompt，`.github/workflows/e2e.yml` 接入 e2e。最终 ready-to-push 树在 `a3ce1b945`：双父 merge 含 `origin/main@2f9c70ad`（#1212 SearchHud）与此前 `#1233`/`#1241`。

### 关键 commit

- `8ff6eab4`（2026-07-15）：复现聊天信号五分钟窗口失效。
- `d87323cd`（2026-07-15）：收紧聊天时间戳 schema 契约。
- `bf4c795f`（2026-07-15）：修复聊天信号过期窗口。
- `e7ad238e`（2026-07-15）：补齐聊天时效上下文回归。
- `ea9849ba`（2026-07-15）：将已完成 plan 移入 `docs/finished_plans/`。
- `d1a8f945`（2026-07-17）：记录早期 Rust wire 单位纠错并补边界测试；后续 review 证明仍需切断客户端 freshness 信任。
- `2482743a`（2026-07-17）：补齐真实 Bot Unix 毫秒、Python 字节 pin、独立 Redis e2e 与 CI 接线。
- `5f04b7a0`（2026-07-17）：把真实 e2e runner 改为 `.mts`，强制 ESM 正确加载 `@bong/schema`。
- `38439bd1`（2026-07-17）：历史基线 merge，合入当时的 `origin/main@28cc3af4`。
- `81d35c6e`（2026-07-17）：第一次原地纠正归档 plan 的跨栈证据。
- `aabfc244`（2026-07-17）：复现客户端未来时间绕过聊天窗口。
- `863bf1ea`（2026-07-17）：以 server observation clock 修复聊天窗口信任边界，并增加 future upper。
- `74d351e5`（2026-07-17）：把真实 e2e 的伪造时钟锚定到 Bot 登录后的即时观测。
- `aa3d1be8`（2026-07-17）：稳固伪造未来时间证据文件与至少 23 小时差值断言。
- `99b0af1c`（2026-07-17）：复现异步聊天处理跨秒时复用旧时钟的误删。
- `057d4d52`（2026-07-17）：在实际 merge 前刷新 `RuntimeDeps.now`，保留跨秒合法消息。
- `438c47e7`（2026-07-17）：合并并复验 `origin/main@9d2e29d0`，保留聊天信任边界与主线 skillbar/quickslot 契约。
- `6cc92bdc`（2026-07-19）：普通 merge `origin/main@946ad6c2`（#1233 docs-only 归档）。
- `3228424e`（2026-07-19）：普通 merge `origin/main@5d9bdd8f`（#1241 skill-anim-fidelity PR-5；触及 server + client）。
- `a3ce1b945`（2026-07-19）：普通 merge `origin/main@2f9c70ad`（#1212 SearchHud 收口；仅 client + docs）。本 commit 之后的 docs-only evidence 纠错会再生成新 HEAD，最终 SHA 以 git 与 validator 对拍为准，不在文档中预写未来 docs commit。

### 测试结果

- 定向 red/green：Rust collector 信任边界（负/正偏移、epoch、`u64::MAX`）、schema chat 13/13、chat processor future upper + 300/301 秒、runtime 跨秒 `mergeNowSeconds` 刷新、Python Bot 默认/伪造毫秒字节 pin 均存在且能在回退时撞红。
- 最终代码树（server 全量在 `3228424e` 重跑；随后 `a3ce1b945` 仅合入 client SearchHud，server 源码与 `3228424e` 一致）：
  - server：`cargo fmt --check` = 0；`cargo clippy --all-targets -- -D warnings` = 0；`cargo test` = 0。计数：lib 11799 passed / 1 ignored，main 11，full_app_startup 1，tarkov_backpack_p0_e2e 4，doc-tests 0 passed / 5 ignored → **合计 11815 passed / 0 failed / 6 ignored**。
  - schema：build = 0；`npm test` 30 files / **893**；`npm run check` 405 generated fresh。
  - tiandao：`npm test` 72 files / **835**。
  - agent workspace：`npm run build` = 0。
  - bot protocol：`python3 -m unittest scripts.bot.test_protocol` **126/126**。
  - client Java 17（`JAVA_HOME=/home/serverkizuna/java/jdk-17.0.19+10`，HEAD `a3ce1b945`）：`./gradlew test build` = 0；JUnit **4153 tests / 0 failures / 0 errors / 0 skipped**；game tests 3/3。
  - 真实 e2e（HEAD `a3ce1b945`）：`bash scripts/e2e-chat-signal-window.sh` = 0；marker `chat-window-20260719-235421-3764525`；`client_ms=1784562865145 wire_ts=1784476465 now=1784476467 age=2`；客户端伪造时间比 wire 超前 `86,400,180 ms`（≥23h）；证据目录 `.sisyphus/evidence/chat-signal-window-20260719-235421-3764525`。
- 历史对抗验证：`FIX_VALIDATING` 对 `057d4d52` PASS；`REBASE_VALIDATING` 对 `438c47e7` PASS。最终 docs-only evidence 提交后必须对**新 HEAD** 再开无上下文只读 validator，结论绑定 exact SHA。

### 跨仓库核验

- server：`ChatObservationClock` / `observed_at_seconds` → Redis `ChatMessageV1.ts` server-observed Unix 秒；`PlayerChatCollected.timestamp` 保留 C2S 毫秒；`server/src/network/mod.rs` 注册 `ChatObservationClock::default()`。
- Python Bot：`Bot.chat(..., timestamp_millis=...)` 写 signed Unix 毫秒；`test_protocol.py` pin 默认时钟与伪造未来字节。
- agent/schema：`ChatMessageV1.ts` 描述、`ChatSignal.ts` 必填、`processChatBatch` 从 `msg.ts` 注入、`isRecentSignal` 闭区间 `[now-300, now]`、`runtime` 在 async drain/LLM 后 `mergeNowSeconds = Math.floor(now()/1000)` 再 merge。
- client：不消费 `ChatSignal`；因 main 合入 skill-anim + SearchHud，用 Java 17 全量回归锁住无回归。
- CI/runtime：`.github/workflows/e2e.yml` 接线；真实 e2e 使用独立 Redis + 本次 server 进程树端口归属，拒绝误连旧 25565。

### 遗留 / 后续

- 功能代码无阻塞遗留；NTP / 客户端时钟漂移补偿、Redis 跨进程聊天持久化仍按非目标保持独立；Tiandao `ts > now` fail closed 已在本 plan 内。
- **运维事故（非代码交付）**：本 worktree 内来源不明 / untracked 的 `agent/packages/tiandao/data/` 在测试流程中被误删，**无可证恢复**。禁止伪造复制“恢复”该目录内容，也**禁止自动 cleanup 删除**同类 ignored/untracked 数据。该事故不是聊天时效修复的交付物，仅作遗留记账；后续 cleanup 必须人工确认路径来源后再动。
- 最终 ready-to-push 代码祖先为 `a3ce1b945`（含 `origin/main@2f9c70ad`）。本 Finish Evidence 的 docs-only commit 会推高 HEAD；最终 exact SHA、fresh validator PASS、以及是否 push 均由 PR 会话外部绑定，**本会话不 push / 不 merge / 不 cleanup**。
