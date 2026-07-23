# plan-bughunt-craft-outcome-network-thread-sound-v1

> **Finished BugFix plan**。来源：`docs/plans-skeleton/plan-bughunt-craft-outcome-network-thread-sound-v1.md`；升格与归档日期：2026-07-13。主题：验证并修复 `craft_outcome` 完成反馈在 Fabric network thread 触达 screen / player / sound state 的线程契约违规。

## 接入面

- **进料**：server 生产路径在 `server/src/network/craft_emit.rs` 从 `CraftCompletedEvent` / `CraftFailedEvent` 构造并 emit `CraftOutcomeV1`（经 `ServerDataPayloadV1::CraftOutcome` / `bong:server_data`）；client raw receiver 复制 Netty buffer 后在 client thread 路由。
- **出料**：client `ServerDataRouter` → `CraftOutcomeHandler` → `CraftStore.recordOutcome(...)` → `CraftScreen` / `WorkbenchScreen` listener（共享 `CraftOutcomeFeedback`：completed 的 `flashTicks=6` / 完成音 / refresh；failed 仅 refresh）。
- **共享类型 / event**：复用既有 `CraftOutcomeV1` / `CraftStore.CraftOutcomeEvent` / screen outcome listener；本 plan 不另建 outcome event / schema 变体。
- **跨仓库契约**：
  - server：emit `CraftOutcomeV1`（本 PR 无 server 代码改动，只读确认生产可达）
  - client：`bong:server_data` → `BongNetworkHandler` / `ServerDataRouter` / `CraftOutcomeHandler` / `CraftStore` / `CraftScreen` / `WorkbenchScreen`
  - agent：**不消费**本 outcome 链路
  - schema：**不改**（无 TypeBox / sample / dist 变更）
- **worldview 锚点**：`worldview.md` §十 资源与匮乏的产出/加工闭环；本 plan 不改变资源、经济或制作数值规则。
- **qi_physics 锚点**：**不涉及**真元/灵气流动、衰减或 ledger。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 第一性原理证真：闭合生产可达路径与失败复现 | ✅ 2026-07-13 |
| P1 | 最小线程边界修复与饱和回归 | ✅ 2026-07-13 |
| P2 | JDK 17 完整门禁、主线同步与三轮绑定 SHA 验证 | ✅ 2026-07-13 |

## Bug 摘要

候选问题是：Fabric raw `ClientPlayNetworking.registerGlobalReceiver(Identifier, PlayChannelHandler)` 的 receiver 运行在 network thread，但 `BongNetworkHandler.registerServerDataChannel()` 在该 callback 内同步执行 `ROUTER.route(...)`。`ServerDataRouter.route(...)` 又同步调用 `CraftOutcomeHandler.handle(...)`，后者写入 `CraftStore` 并同步通知当前 screen 的 outcome listener。

当 payload 为 `type=craft_outcome`、`kind=completed` 时，`CraftScreen` / `WorkbenchScreen` listener 会写 `flashTicks` 并调用 `client.player.playSound(...)`；如果上述调用链确实未切回 client thread，即违反 Fabric 对 game state / screen / sound 访问的线程契约。

本 plan 先假设报告可能错误：必须核验 raw API 的真实线程语义、生产 server emit、router 注册、store listener 生命周期、现有 `client.execute(...)` 防护，以及同根因修复是否已在最新主线覆盖。只有证真后才允许修改代码。

## 范围与决策

- **主范围**：`bong:server_data` raw receiver 到 `ServerDataRouter` handler side effect 的线程边界，以及 `craft_outcome` completed / failed 对 `CraftStore`、`CraftScreen`、`WorkbenchScreen` 的可达链。
- **最小修复决策**：receiver 在 network thread 只复制/解码网络 buffer；`ROUTER.route(...)`、store/listener side effect 与 `applyDispatch(...)` 必须在同一个 `client.execute(...)` client-thread task 内有序执行。不得只把最终 `applyDispatch(...)` 包进主线程而让 handler 先跨线程落地。
- **局部兜底边界**：只有证据证明 `CraftOutcomeHandler.handle(...)` 还有绕过该 receiver 的生产异步调用方时，才给 craft listener 增加独立调度；否则不叠加双重 `client.execute(...)`，避免反馈顺序和测试语义复杂化。
- **排重**：不单独修 #1016 的 `cast_sync -> SkillConfigPanelManager` UI 生命周期；但若主 receiver 边界修复自然覆盖它，必须增加回归证明该 handler 不再在 network thread 执行。
- **禁止扩项**：不改制作数值、配方、server craft 状态机、声音资产、UI 视觉规格或协议 schema；不做全量 handler 重构。
- **玩家可感知规格**：保留既有完成音效、`flashTicks = 6` 闪光和输出预览刷新；本修复只保证三者在 client thread 按原顺序稳定发生，不新增粒子、动画、HUD、narration 或新资产。
- **worldview 锚点**：制作属于 §十资源与匮乏的产出/加工闭环；本 plan 不改变资源、真元或经济规则，不触碰 `qi_physics` ledger。

## 开放问题与 §N.1（归档审计说明）

**开放问题：无。**

依据 `docs/CLAUDE.md` §五：plan **常带**开放问题，**若有**则必须在 pre-P0 以 `§N → §N.1` 收口；**并非**每份 plan 无条件必须倒填开放问题清单。本 bugfix plan 在 promotion 时即由上方「范围与决策」一次性收口，历史实施前没有未决设计项，因此 **§N.1 决议结构不适用**。

不伪造“实施前决议”时序。现有决策锚点如下（file:line / plan 章节）：

- 线程边界最小修复：`client/src/main/java/com/bong/client/BongNetworkHandler.java` raw receiver 只复制 buffer，`route → handler/store/listener → applyDispatch` 统一进单一 `client.execute(...)` task；见本 plan「范围与决策」最小修复决策。
- 收包时刻 freshness + generation guard：`ClientConnectionStatusStore` 代次与 `markPayloadReceived(now, generation)`；见 P1 落地结果（2026-07-20 复审补齐）。
- 双屏共享反馈契约：`CraftOutcomeFeedback` + `CraftScreen` / `WorkbenchScreen` listener；完成音/闪光/刷新不改资产与数值。
- 跨栈边界：server 只读确认 emit；agent 不消费；schema 不改；见头部「接入面」。
- 明确非目标：其他 channel freshness、#1016 局部 UI 生命周期、制作数值/配方/schema。

## P0：第一性原理证真

- 核验当前 Fabric 1.20.1 / Fabric API sources 对 raw receiver 的 network-thread 契约。
- 闭合正常玩家路径：server craft completion emit → `bong:server_data` → receiver → router → `CraftOutcomeHandler` → `CraftStore` → 当前 `CraftScreen` / `WorkbenchScreen` listener。
- 检查现有防护：payload copy/decode、`client.execute(...)` 包围范围、screen refresh scheduling、session/disconnect 清理、listener 注销与重复 payload 语义。
- 先增加修复前可失败的线程契约测试：从模拟 network thread 触发 receiver/提取出的调度边界，证明 router/store/listener 不得在调用线程同步执行。
- 若候选已被主线覆盖或生产不可达，则转 `NOT_BUG`：只写入反证、`file:line` 与测试结果，不造空修复。

### P0 证真结果（2026-07-13）

- Fabric API `fabric-networking-api-v1:1.3.12+13a40c6677` 的
  `ClientPlayNetworking.registerGlobalReceiver(Identifier, PlayChannelHandler)` Javadoc 明确写明：
  raw handler 运行在 network thread；读取 buffer 后，访问 game state 必须调用
  `ThreadExecutor.execute(Runnable)` 切回 render thread。
- 生产 emit 可达：`server/src/network/craft_emit.rs:791-842` 从
  `CraftCompletedEvent` / `CraftFailedEvent` 构造 `CraftOutcomeV1` 并通过
  `ServerDataPayloadV1::CraftOutcome` 发给 caster，不是死协议或测试孤岛。
- 修复前 receiver 只把最终 `applyDispatch(...)` 包进 `client.execute(...)`，但
  `ROUTER.route(...)` 同步进入 `CraftOutcomeHandler`；后者在
  `client/src/main/java/com/bong/client/network/CraftOutcomeHandler.java:26-45`
  同步调用 `CraftStore.recordOutcome(...)`。
- `CraftStore.recordOutcome(...)` 在
  `client/src/main/java/com/bong/client/craft/CraftStore.java:99-103` 同步遍历 listener；
  `CraftScreen.java:47-52` 与 `WorkbenchScreen.java:51-56` 的 completed listener
  会立即写 `flashTicks` 并调用 `player.playSound(...)`。因此正常玩家出炉路径确实让
  screen/player/sound state 从 Fabric network thread 被访问。
- 修复前复现提交 `dbb8772c`：Temurin 17 下运行
  `./gradlew test --tests com.bong.client.BongServerDataThreadingTest`，7/7 失败；
  关键失败值为 `route@fabric-network-io-test`，completed、failed、`cast_sync`、
  连续 payload 与坏 payload 后的合法 payload 均在 client queue drain 前提前写 store。

## P1：最小修复与饱和回归

- 将 raw receiver 的 handler side effect 统一排入 client executor，并保持单 payload 内 `route → applyDispatch` 顺序。
- 覆盖 `CraftScreen` 与 `WorkbenchScreen` 的 completed outcome：完成音效、闪光、输出预览刷新只在 client thread 执行且各发生一次。
- 覆盖 failed outcome、未知 payload、route 返回空 dispatch、handler 抛错/坏 JSON、连续 payload 顺序与断线/无 player 边界，防止调度后吞包、重复反馈或延迟异常。
- 回归 `cast_sync` 同根因入口：handler side effect 只在 client executor 中发生；不改变其业务语义。
- 测试断言外部可观察契约与线程身份，不绑定私有实现调用次数；失败信息必须带实际线程/队列/事件值。

### P1 落地结果（2026-07-13 初版 + 2026-07-20 复审补齐）

- 初版修复提交 `867fd1a7` 把 `bong:server_data` 的
  `bridge → route → handler side effect → applyDispatch` 统一封装为一个
  `client.execute(...)` task；raw receiver 只复制 Netty buffer，不给 craft listener
  叠加局部线程兜底，也不改变协议、数值、声音资产或 UI 规格。
- **复审补齐（2026-07-20）**：初版把 `markConnectionPayload` 一并推迟到 client task
  执行时刻，导致 freshness 变成 processing time，且 disconnect-before-drain 的 stale
  task 可把 `ClientConnectionStatusStore` 复活为 connected。现已恢复收包时刻语义：
  receiver 捕获 `receivedAtMs` + `connectionGeneration`，task 开头用 generation guard
  回写；stale generation 整段 no-op。CraftScreen / WorkbenchScreen 共用
  `CraftOutcomeFeedback`（completed: flashTicks=6 → 一声完成音 → refresh；failed: 仅 refresh）。
- `BongServerDataThreadingTest` 现覆盖：
  completed/failed store 线程、`cast_sync`、route/apply 顺序、连续 payload、坏 JSON、
  handler exception、receipt-timestamp freshness、disconnect-before-drain 不复活、
  reconnect 后旧 task 不污染新 generation、unknown/null-dispatch no-op seam、
  真实 CraftScreen+WorkbenchScreen flash/sound/refresh 顺序、disconnect 清理 CraftStore
  且丢弃 queued side effect。
- `CraftOutcomeFeedbackTest` 锁定两屏共享反馈契约与 listener 注销生命周期。
- 同根因回归在 Temurin 17 下通过（见 Finish Evidence 最新计数）。

## P2：闭环验证

- 首次修复 HEAD：工作区干净后由全新无上下文 read-only validator 对抗检查真伪、可达性、主线程边界、顺序、回归与测试饱和度。
- 完整 client 门禁：JDK 17 下 `cd client && ./gradlew test build`。
- `git fetch origin` 后按 merge-base 分类同步最新主线；任何 HEAD 变化均重跑 client 完整门禁并启动新的 `REBASE_VALIDATING` validator。
- 填写 `## Finish Evidence`、受控归档并提交后，对最终归档 HEAD 再启动新的 `FINAL_VALIDATING` validator；最终 PASS 后禁止再修改分支。

## 验收测试矩阵

| 场景 | 预期 |
|------|------|
| network thread 收到 completed | receiver 返回前 router/store/screen/sound 均未执行；client executor drain 后按序执行一次 |
| network thread 收到 failed | 失败 outcome 只在 client thread 写 store/通知 listener，不播放完成音 |
| CraftScreen completed | `flashTicks`、完成音效、刷新均在 client thread，顺序稳定且不重复 |
| WorkbenchScreen completed | 同上，且不影响 workbench session / output preview |
| `cast_sync` | 配置窗相关 handler side effect 不在 network thread 执行 |
| 空/未知/坏 payload | 不越过既有错误处理边界，不触发 craft side effect，不破坏后续合法 payload |
| 连续两个合法 payload | client executor 保持提交顺序，各 payload 恰好应用一次 |
| 无 player / screen 已关闭 | 不崩溃、不播放伪完成音，store 与 listener 生命周期遵循既有契约 |

## 风险

- 把 route 移入 client executor 会把 handler 内 JSON 解析和所有 store 更新一起移到 client thread；需用现有 payload 尺寸与测试确认不会引入可感知卡顿，不顺手重构解析层。
- 旧测试可能默认 `ROUTER.route(...)` 在 receiver 返回前同步生效；必须改成显式 drain executor，而不是放宽断言。
- 双重调度会改变反馈顺序并扩大 race surface，因此局部 listener 兜底必须以额外生产调用方证据为前提。
- 线程修复不能吞掉 route exception、未知 payload 诊断或既有 dispatch；错误路径需保持可观测。

## Finish Evidence

### 落地清单

- `client/src/main/java/com/bong/client/BongNetworkHandler.java`
  - Fabric `ClientPlayConnectionEvents.INIT` 按物理 `ClientPlayNetworkHandler` identity 同步分配不可变
    `ClientConnectionStatusStore.SessionToken`；`JOIN` 捕获 callback 时刻并在 client task 中只激活
    既有 token，不再换代。
  - raw `bong:server_data` receiver 只复制 Netty buffer、按 callback `handler` 捕获 token 与
    `receivedAtMs`、排入单一 client-thread task；未 INIT 的 null/陌生 handler fail closed，连 task
    都不排。
  - task 在 protobuf bridge 之前执行 active-token guard；合法链仍按
    `bridge/fallback → ServerDataRouter.route → handler/store/listener → applyDispatch` 在同一 client
    task 内有序运行。DISCONNECT/reconnect 后旧 token task 在 bridge 前整段 no-op。
  - `disconnectSession(...)` 先同步失效当前物理连接，再且仅在该 handler 正是 active session 时
    排 `clearClientStateOnDisconnect`；旧 handler 的迟到 DISCONNECT 只移除自身 token，不得误清
    已激活的新 session。
- `client/src/main/java/com/bong/client/ui/ClientConnectionStatusStore.java`
  - 以 `IdentityHashMap<Object, SessionToken>` 保存 INIT 注册；同一 handler 重复 INIT 幂等，token
    构造器私有且 sequence 用 `Math.incrementExact` 防静默回绕。
  - `activateSession(handler, joinedAt)`、`invalidateSession(handler, disconnectedAt)`、
    `isActiveSession(token)` 与 `markPayloadReceived(receivedAt, token)` 共同锁定物理连接生命周期；
    未注册、未激活、已失效 token 均 fail closed。
  - 同一 active token 内以 `Math.max(lastPayloadAtMs, max(0, receivedAt))` 合并 freshness；乱序、
    较旧、0、负时间戳不得回退，旧 token 不得复活 connected 或污染新 session。
- `client/src/main/java/com/bong/client/craft/CraftScreen.java` /
  `WorkbenchScreen.java`
  - 五类 Store/Inventory listener 共用单一 `listenersAttached` 状态；重复 build/resize/测试 attach
    幂等，`removed()` 统一注销全部 listener。
  - 两屏继续复用 `CraftOutcomeFeedback` 的 completed 契约
    `flashTicks=6 → 一声完成音 → 一次 refresh` 与 failed 契约 `仅 refresh`；同型
    `CompleteSoundPlayer`/refresh 注入 seam 只用于直接观察生产 listener 的 exactly-once 行为。
- `client/src/test/java/com/bong/client/BongServerDataThreadingTest.java`
  - 23 项覆盖 network→client thread 边界、route/apply 顺序、completed/failed、`cast_sync`、连续
    payload、坏 JSON、handler exception、unknown/null-dispatch 与收包时刻 freshness。
  - 使用真实 protobuf `CraftRecipeList`、`CraftSessionState`、`CraftOutcome` 锁定
    `INIT token A → pre-JOIN capture → JOIN activate A → drain` 后三类 join hydration/store/listener/
    sound/refresh 各 exactly once。
  - 锁定 `A capture → disconnect A → INIT/JOIN B → drain A` 的 bridge/route/store/listener/sound/
    refresh/apply 全 0、cleanup-before-drain 不复活、迟到 A disconnect 不清 B、陌生/null handler
    fail closed、同 token 时间戳单调；并锁定 B INIT/JOIN 后 A 的迟到首次或重复 JOIN 均不得重新
    激活旧 token、清空 B 的 CraftStore 或回退 B freshness。
- `client/src/test/java/com/bong/client/craft/CraftOutcomeFeedbackTest.java`
  - 7 项覆盖共享反馈顺序、failed 无完成音、player 缺失，以及 CraftScreen/WorkbenchScreen 重复
    attach 后一声一刷、`removed()` 后所有 outcome 反馈为 0。
- `client/src/test/java/com/bong/client/ui/ConnectionStatusIndicatorTest.java`
  - 8 项改走真实 INIT/JOIN/DISCONNECT token 生命周期，保留断线/重连 toast 与 measuring-time 契约。
- `docs/finished_plans/plan-bughunt-craft-outcome-network-thread-sound-v1.md`
  - 只原地改写既有唯一 `## Finish Evidence`；不重复 promotion、归档、`git mv` 或新增同名 H2。

### 关键 commit

- `3e3c80d8`：升格 skeleton，收口线程验证范围。
- `dbb8772c`：提交修复前红测基线，7/7 复现 route/store 在 network thread 提前执行。
- `867fd1a7`：把 server_data bridge/route/handler/apply 链移入单一 client-thread task。
- `3ccf908b`：普通 merge `origin/main@2f9c70ad`，保留本修复与主线 disconnect 清理语义。
- `81fe479d5`：补收包时刻 freshness、初版 generation guard、共享反馈与生命周期矩阵。
- `93369a145`：同 generation freshness 改为单调最大值并补真实 route→CraftStore exactly-once。
- `6fa6b99bf`：更新上一轮 current-head 证据；本次返工的精确起始 HEAD。
- `d25dfba95300c5f3a0081c611cf20edbc6e7aa72`：以 handler-bound immutable session token 取代会丢
  pre-JOIN 首包的全局 generation 采样；同步失效 active token、阻止迟到旧 handler 清新 session；
  两屏 listener 幂等与真实 protobuf/生命周期饱和回归。
- `fd3339e9e`：修复 `disconnectSession(...)` 失效排队竞态，确保 DISCONNECT 与 in-flight payload 在
  client executor 上串行，cleanup 先于旧 payload 时旧 task 在 bridge 前 no-op。
- `821a0bcbe`：把 INIT/JOIN 生命周期接成 handler-bound token，并补 pre-JOIN payload、陌生/null
  handler fail closed 与迟到 DISCONNECT 不清新 session。
- `1dace09dc`：修复旧 handler 迟到 JOIN 抢占新 session；B INIT/JOIN 后 A 的迟到 JOIN 不得重新
  激活旧 token 或污染 B 的 CraftStore/freshness。
- `89a5e41c9`：收敛会话激活入口，删除不必要的 public token overload，保持生产 JOIN 与测试均走
  `activateSession(handler, joinedAt)` 单一路径。
- `437e3e0318f69aee3345107629579b024e28955c`：普通 merge `origin/main@746794871a91c843958e6692291c4194c0dad085`；
  当前待 push exact HEAD。

### 测试结果（历史基线）

- RED（Temurin 17）：
  `./gradlew test --tests com.bong.client.BongServerDataThreadingTest`
  → `7 tests completed, 7 failed`；关键实际值为 `route@fabric-network-io-test`，证明 raw receiver
  同步 route/store 违反 Fabric network-thread 契约。
- 2026-07-20 本次返工前的 current-head 基线 `93369a145`：完整 client 门禁
  **4172/0/0/0**（476 suites）+ GAME TESTS **3/3**；该版本仍用全局 generation，无法保证同一
  新 handler 的 pre-JOIN 一次性 craft hydration 不被 JOIN 换代丢弃。

### 测试结果（2026-07-23 当前候选 @ `437e3e0318f69aee3345107629579b024e28955c`）

- client 完整门禁（Temurin 17，命令无管道）：
  `JAVA_HOME=/home/serverkizuna/java/jdk-17.0.19+10 PATH="$JAVA_HOME/bin:$PATH" ./gradlew test build`
  → exit `0`，`BUILD SUCCESSFUL in 6m 56s`；JUnit XML **4304 tests, 0 failures, 0 errors,
  0 skipped**（486 files），GAME TESTS **3/3**。
- 目标测试计数：
  - `BongServerDataThreadingTest` **23/0/0/0**；
  - `CraftOutcomeFeedbackTest` **7/0/0/0**；
  - `ConnectionStatusIndicatorTest` **8/0/0/0**。
- 两份互相独立、read-only、第一性原理 validator 均先对拍 commit object，再审生产 wiring 与测试/
  evidence；结论都严格绑定 `437e3e0318f69aee3345107629579b024e28955c`：**VERDICT PASS，
  blocker/major: none**。
- 历史 E2E run `29716656367` 绑定远端旧 HEAD `6fa6b99bf423e834cf51eb09c40fbb5ac93d6a9f`：
  `Client stage (gradlew test)`、schema、agent 与 server release build 均成功；唯一失败是无关的 server
  `persistence::persistence_tests::phase9_throttled_write_regression_handles_1000_npc_and_50_players`
  （`lock_failures=1`，`errors=["database is locked"]`，11796 passed / 1 failed / 1 ignored），故后续
  Smoke/E2E 与 Bot e2e 被跳过。该 run 不能宣称整条 E2E 通过，也不构成本 client craft_outcome PR
  引入 SQLite/persistence 修改的理由。

### 主线同步与 SHA 纪律

- `437e3e0318f69aee3345107629579b024e28955c` 的第二父提交是
  `origin/main@746794871a91c843958e6692291c4194c0dad085`；merge 后完整 Java 17 client gate 与两份 validator
  已重新执行，未继承任何旧 SHA 结论。
- 更新本证据前，远端 claim branch 仍精确停在
  `6fa6b99bf423e834cf51eb09c40fbb5ac93d6a9f`；本地 worktree clean，未发现远端未知提交，允许普通
  fast-forward push，禁止 force/amend/rebase。
- 历史 validator、本地 gate、E2E、`/review` 与 CodeRabbit 结论只绑定各自旧 SHA；普通 push 后必须
  对新的 docs commit HEAD 重跑必要 exact-head client gate，并重新触发/等待同一 SHA 的 E2E、
  `/review` 与 CodeRabbit，不能把 `437e3e031` 的证据外推到未来 SHA。

### 跨栈核验

- client：修改 receiver 生命周期、connection store、两屏 listener 与回归；当前候选 Java 17 完整门禁
  **4304/0/0/0**（486 XML files）+ GAME TESTS **3/3**。
- server：只读确认 `server/src/network/craft_emit.rs` 的 completed/failed 生产 emit 既有可达；本次
  无 server 代码改动，不跑 cargo gate。
- agent/schema/worldgen：未改协议、TypeBox/sample/dist、agent consumer、资源或生成物。
- persistence：未改任何 SQLite/持久化代码、迁移或测试；不把无关 persistence 议题塞入本 PR。
- 真元/世界观/A/V：不改变制作数值、资源流、真元 ledger 或音效资产；只保证既有完成反馈在正确
  client session/thread exactly once 执行。

### 遗留 / 后续

- 其他 channel（vfx/audio/agent_ui 等）仍使用历史 `markPayloadReceived()` freshness 路径，明确非
  本 plan 范围；本修复只收口 `bong:server_data` 的 handler-bound session token。
- 远端 exact-head `/review`、CodeRabbit 与 E2E 仍须在本证据 docs commit push 后重新触发并收敛；
  任何新 blocker/major 都需独立复核、修复并从 validator/client gate 开始重验。
- 明确排除 PR #1228、PR #1215 worktree、Tiandao snapshots 与来源不明进程；未触碰其他 worktree、
  主 checkout 或 PID `2399867`。
