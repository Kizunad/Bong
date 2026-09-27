# plan-bughunt-client-insight-offer-strand-v1

> **Active BugFix（2026-09-27 接续本地未推送提交）**。一句话主题：client flow / screen flow / open-close sequencing 角度修复 **`InsightOfferScreen`（普通顿悟 + 心魔共用）被其他 client-only screen 顶掉后没有提交决定**，导致 `InsightOfferStore` 悬挂、server/client 两侧没有终态。已避开 sparring invite hijack、identity stale session、preview pause、client input 双绑。

## 阶段总览

| 阶段 | 可核验交付物 | 状态 |
|---|---|---|
| P0 | 以 vanilla `Screen.removed()` 路径复现；核对 `InsightOfferStore.settleIfCurrent` 与 `InsightOfferScreen` 生命周期；真实 `removed()` 回归测试先失败 | ✅ 2026-09-27 |
| P1 | `InsightOfferScreen.removed()` 对未结算实例提交 `declined` 或 `timedOut`，不递归关闭新屏；重复/过期/旧实例仍由 compare-and-clear 保护 | ✅ 2026-09-27 |
| P2 | client Java 编译、JUnit、GameTest、jar 构建门禁通过；`git fetch origin && git merge origin/main` 无新增变更 | ✅ 2026-09-27 |
| P3 | 完成 Finish Evidence、归档、推送并开 PR | ✅ 2026-09-27 |

## 执行边界

- 只收口 `InsightOfferScreen` 的直接生命周期与契约测试，不改造所有 screen bootstrap。
- 继续使用现有 `InsightOfferStore.settleIfCurrent` 实例身份和 `ScreenTransitionController` 转场回调；不扩大到 server deadline，因为本次 client `removed()` 已能保证本地替换有终态。
- 不改 `qi_physics`、agent 或协议 schema；C2S 当前没有 `offer_id`，本地替换仍按既有 tombstone 语义处理。

## 结论

- **#1 major（已证真）**：当前主线的 `InsightOfferScreen` 只有 `tick()`、`close()`、`onCurrentScreenCancelled()` 和 `onPendingOpenCancelled()` 收口，真实 `Screen.removed()` 没有覆盖。`CraftScreenBootstrap`、`InspectScreenBootstrap`、`IdentityPanelScreenBootstrap`、`LingtianActionScreenBootstrap`、`SpiritTreasureScreenBootstrap` 仍可请求新 screen；`ScreenSetMixin` 会把请求交给 `ScreenTransitionController`，转场完成后 vanilla 直接调用旧 screen 的 `removed()`。
- 因此玩家在 `InsightOfferScreen` 上按 `C` / `E` / `O` / `L` / `T` 时，旧屏从栈上移除但不会走 `close()`；`InsightOfferStore` 保留 current offer，`InsightOfferScreenBootstrap.applyStoreChange()` 也不会因 store 未变化而重开。
- screen 被移除后不再 tick，普通顿悟和心魔 offer 的 client TTL 也不会再触发 timeout；server `PendingInsightOffer` 仍等 `InsightChosen` 终态，未找到独立 deadline cleanup。
- 修复在 `InsightOfferScreen.removed()` 中按过期状态提交 `InsightDecision.declined(...)` 或 `InsightDecision.timedOut(...)`，并调用 `settle(decision, false)`，避免 vanilla 正在安装新屏时递归 `setScreen(null)`。`InsightOfferStore.settleIfCurrent(offerId, decision)` 继续保证 stale/duplicate 回调只影响自己的实例。

## 复现路径

1. 进入能触发普通顿悟或心魔抉择的场景，让 server 发送 `insight_offer` 或 `heart_demon_offer`。
2. client 通过 `InsightOfferHandler` / `HeartDemonOfferHandler` 写入 `InsightOfferStore`，`InsightOfferScreenBootstrap` 自动 `setScreen(new InsightOfferScreen(...))`。
3. **不要点任何选项，也不要按 ESC**；直接按任一会主动开本地 screen 的键：
   - `C` → `CraftScreenBootstrap`（`client/src/main/java/com/bong/client/craft/CraftScreenBootstrap.java`）
   - `E` → `InspectScreenBootstrap`（`client/src/main/java/com/bong/client/inventory/InspectScreenBootstrap.java`）
   - `O` → `IdentityPanelScreenBootstrap`（`client/src/main/java/com/bong/client/identity/IdentityPanelScreenBootstrap.java`）
   - `L` → `LingtianActionScreenBootstrap`（`client/src/main/java/com/bong/client/lingtian/LingtianActionScreenBootstrap.java`）
   - `T` → `SpiritTreasureScreenBootstrap`（`client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreenBootstrap.java`）
4. `InsightOfferScreen` 消失，新的本地 screen 打开。
5. 修复前等待超过 client 默认 90s（普通顿悟）或心魔 offer TTL，不会自动提交 `declined/timed_out`，原 offer 也不会自动重新弹出；修复后旧屏在 vanilla `removed()` 阶段完成 exactly-once 终态。

## 根因链路

- `InsightOfferHandler.handle(...)` 把普通顿悟 TTL 只落在 client 本地 `expiresAtMillis = now + 90_000`（`client/src/main/java/com/bong/client/network/InsightOfferHandler.java`）；普通顿悟没有 server 权威 deadline。
- `InsightOfferScreen.tick()` 过期时调用 `settle(InsightDecision.timedOut(...))`，`close()` 和转场取消调用 `settle(InsightDecision.declined(...))`。
- 修复前真实 `removed()` 只执行基类逻辑；修复后 `removed()` 调用 `decisionForImplicitRemoval()`，再以 `closeCurrentScreen=false` 结算，最后才调用 `super.removed()`。
- `settle(...)` 先调用 `InsightOfferStore.settleIfCurrent(offer.offerId(), decision)`，由 store compare-and-clear 后 dispatch；普通关闭路径才允许 `mc.setScreen(null)`。
- `InsightOfferScreenBootstrap.applyStoreChange()` 仍只响应 store listener；因此必须在生命周期移除点完成终态，不能依赖 watchdog 补开。
- server 侧 `process_insight_request()` / `ingest_agent_insight_offer()` 都会插入 `PendingInsightOffer`（`server/src/cultivation/insight_flow.rs`），而 `apply_insight_chosen()` 只在收到 client decision 后移除它。全仓 grep `PendingInsightOffer` 未见独立 timeout/deadline 清理系统。

## 这个 bug 对实际游玩体验的影响

- 玩家会在**没有做出选择、也没有收到失败提示**的情况下丢掉一次顿悟/心魔抉择，体感是“弹窗自己没了，机缘也没了”。
- 对普通顿悟，这会吞掉一次成长抉择；对心魔劫，这会把高风险高收益分叉变成**无 UI、无超时结算、无重开入口**的悬空态。
- 因为是 client 本地切屏触发，玩家很容易在下意识按 `C/E/O/L/T` 时复现，属于正常游玩热键路径，不是测试器特供。

## 修复结果与未纳入范围

- **已落地 client 收口**：`InsightOfferScreen.removed()` 按过期状态调用 `settle(decision, false)`；正常点击、ESC、tick、转场取消仍复用原有 exactly-once 路径。
- **未改 open policy**：当前弹窗设计允许被普通本地 screen 抢焦点，移除生命周期现在会结算，不需要为所有 bootstrap 增加 modal guard。
- **未改 server 兜底**：server `PendingInsightOffer` 与 agent/schema 契约没有本次代码变更；权威 deadline 属于后续独立 plan。

## 反方裁决

- **第 1 轮反方论点**：`InsightOfferScreenBootstrap` 设计上“应当抢焦点”，所以玩家理论上不该再打开别的 screen，这不是 bug。
- **驳回理由**：抢焦点只发生在 offer 写入 store 的那一刻；之后 `CraftScreenBootstrap`、`InspectScreenBootstrap`、`IdentityPanelScreenBootstrap`、`LingtianActionScreenBootstrap`、`SpiritTreasureScreenBootstrap` 都只防“同类 screen 重复打开”，**没有任何 `currentScreen == null` / modal guard**，所以玩家后续依然能主动把它顶掉。
- **第 2 轮反方论点**：即便 UI 被顶掉，client 90s 超时或 heart-demon TTL 最终也会把会话收掉，最多只是视觉问题。
- **驳回理由**：普通顿悟 timeout 只在 `InsightOfferScreen.tick()` 跑；screen 被替换后不再 tick。store 只是一个被动快照，没有定时器；server 侧也未看到 `PendingInsightOffer` deadline system。它不是“视觉没回来”，而是**结算链路本身断了**。

## 旧提交处置

- `backup/plan-bughunt-client-insight-offer-strand-v1-local-20260927` 保留旧本地分支，唯一超出主线的提交是 `fbf475cc7`（2026-07-18，只有骨架升格，且 trailer 为 `Model: gpt-5`）。
- 旧提交未直接 cherry-pick；本次按当前主线重新执行升格并提交为 `478a34411`，再以当前生命周期实现重写 bug 修复。旧分支不删除，待 PR 合并后由主干统一清理。

## Finish Evidence

### 落地清单

- P0：核对 `client/src/main/java/com/bong/client/insight/InsightOfferScreen.java`、`InsightOfferStore.java`、`ScreenTransitionController.java` 与五个本地 screen bootstrap；真实 `InsightOfferScreen.removed()` 回归测试在修复前失败。
- P1：`InsightOfferScreen.removed()` 调用 `decisionForImplicitRemoval()` 和 `settle(decision, false)`；`client/src/test/java/com/bong/client/insight/InsightOfferScreenTest.java` 覆盖 exactly-once 与 store 清空。
- P2：client 完整门禁通过；fetch 后 merge `origin/main` 已是最新。
- P3：本节已填写，随后将 plan 归档到 `docs/finished_plans/`，推送同一 claim 分支并创建 PR。

### 关键 commit

- `478a34411`（2026-09-27）：重新升格 plan，接续旧本地验真并使用 `Model: gpt-6-luna`。
- `f64a862fe`（2026-09-27）：补齐 `InsightOfferScreen.removed()` 终态收口及真实生命周期回归测试，使用 `Model: gpt-6-luna`。

### 测试结果

- 修复前定向测试：`scripts/build-token.sh gradle test --tests com.bong.client.insight.InsightOfferScreenTest.exceptionalRemovalSettlesDeclinedExactlyOnce` 按预期失败，证明真实 `removed()` 不会结算。
- 修复后定向测试：同一命令通过。
- 完整 client 门禁：`scripts/build-token.sh gradle test build` 通过；JUnit 报告 5,055 tests、0 failures，GameTest 3/3，通过 jar/remap 构建。
- 主线同步：`git fetch origin && git merge origin/main` 输出 `Already up to date.`，没有带入需要重跑的 client 变更。

### 跨仓库核验

- client 命中 `InsightOfferScreen.removed`、`InsightOfferStore.settleIfCurrent`、`InsightOfferScreenBootstrap.applyStoreChange`；本次只改 client 生命周期。
- server 命中 `PendingInsightOffer`、`apply_insight_chosen`（`server/src/cultivation/insight_flow.rs`），确认 server 仍依赖客户端决定，本次未改 server。
- agent/schema 命中 `InsightDecisionRequestV1`（`agent/packages/schema/src/client-request.ts`）、`InsightOfferV1`/`bong:insight_offer`（`agent/packages/tiandao/src/insight-runtime.ts`、`agent/packages/schema/src/channels.ts`），契约未改；`qi_physics` 不涉及。

### 遗留 / 后续

- server 侧 `PendingInsightOffer` 的权威 deadline/timeout cleanup 不在本 plan；如需服务端兜底应另立 plan。
- C2S `InsightDecision` 当前未携带 `offer_id`，本次沿用已有本地 tombstone 与 compare-and-clear 语义。

## 审计来源

- bug-hunt（本轮，worktree `bughunt-loop-20260705-cc-client-flow`）；
- 聚焦 client flow / screen flow / open-close sequencing；
- 证据来自 `client/src/main/java/com/bong/client/insight/`、`client/.../*ScreenBootstrap.java`、`server/src/cultivation/insight_flow.rs`、`server/src/network/mod.rs` 的静态读树与去重复核。
