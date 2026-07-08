# plan-bughunt-client-insight-offer-strand-v1（骨架）

> **骨架（草案）**。一句话主题：client flow / screen flow / open-close sequencing 角度确认 1 个高置信真 bug——**`InsightOfferScreen`（普通顿悟 + 心魔共用）被其他 client-only screen 顶掉后不会提交决定、不会超时、也不会重开，当前顿悟会话被静默吞掉并在 server/client 两侧悬挂**。已避开用户点名不查的 sparring invite hijack / identity stale session / preview pause / client input 双绑。

## 结论

- **#1 major（plan_skeleton）**：`client/src/main/java/com/bong/client/insight/InsightOfferScreen.java:215-255` 只在 `tick()` 内处理超时、只在 `close()` 内处理未结算关闭；**没有 `removed()` 收口**。与此同时，`CraftScreenBootstrap`、`InspectScreenBootstrap`、`IdentityPanelScreenBootstrap`、`LingtianActionScreenBootstrap`、`SpiritTreasureScreenBootstrap` 都允许在当前已有别的 screen 时直接 `setScreen(new ...)`。因此：
- 当玩家正看 `InsightOfferScreen` 时按 `C` / `E` / `O` / `L` / `T` 之类 client-only 开屏键，Minecraft 会把旧 `InsightOfferScreen` 从栈上移除，但不会走它的 `close()` 语义；
- `InsightOfferStore` 仍保留旧 offer，`InsightOfferScreenBootstrap` 又只在 **store 变化** 时才重开（`applyStoreChange()` 无 tick/watchdog），所以这份 offer **不会自动回来**；
- `InsightOfferScreen.tick()` 是唯一 timeout 入口，screen 被顶掉后也**不再推进超时**；
- server 侧 `PendingInsightOffer` 只在 `InsightChosen` 成功/拒绝/非法分支被移除，未见任何 deadline/timeout 清理 system；
- 结果是一次真实顿悟/心魔选择可被本地切屏静默吞掉，形成“UI 消失但会话未结算”的半开状态。

## 复现路径

1. 进入能触发普通顿悟或心魔抉择的场景，让 server 发送 `insight_offer` 或 `heart_demon_offer`。
2. client 通过 `InsightOfferHandler` / `HeartDemonOfferHandler` 写入 `InsightOfferStore`，`InsightOfferScreenBootstrap` 自动 `setScreen(new InsightOfferScreen(...))`。
3. **不要点任何选项，也不要按 ESC**；直接按任一会主动开本地 screen 的键：
   - `C` → `CraftScreenBootstrap`（`client/.../craft/CraftScreenBootstrap.java:25-35`）
   - `E` → `InspectScreenBootstrap`（`client/.../inventory/InspectScreenBootstrap.java:35-57`）
   - `O` → `IdentityPanelScreenBootstrap`（`client/.../identity/IdentityPanelScreenBootstrap.java:27-51`）
   - `L` → `LingtianActionScreenBootstrap`（`client/.../lingtian/LingtianActionScreenBootstrap.java:32-55`）
   - `T` → `SpiritTreasureScreenBootstrap`（`client/.../spirittreasure/SpiritTreasureScreenBootstrap.java:27-50`）
4. `InsightOfferScreen` 消失，新的本地 screen 打开。
5. 之后等待超过 client 默认 90s（普通顿悟）或心魔 offer TTL，也不会自动提交 `declined/timed_out`，原 offer 也不会自动重新弹出。

## 根因链路

- `InsightOfferHandler` 把普通顿悟 TTL 只落在 client 本地 `expiresAtMillis = now + 90_000`（`client/.../network/InsightOfferHandler.java:55-89`）；普通顿悟没有 server 权威 deadline。
- `InsightOfferScreen` 的未结算收口只有两条：
  - `tick()` 检查过期后 `settle(timedOut)`（`InsightOfferScreen.java:215-230`）
  - `close()` 检查未结算后 `settle(declined)`（`InsightOfferScreen.java:232-240`）
- 该类**没有 `removed()` override**，所以被别的 screen 替换时不会调用 `settle(...)`。
- `settle(...)` 才会 `onDecision.accept(...)`，进而走 `InsightOfferStore.submit()` → `dispatcher.dispatch()` → `replace(null)`（`InsightOfferScreen.java:247-255`，`InsightOfferStore.java:40-43`）。
- `InsightOfferScreenBootstrap.applyStoreChange()` 只响应 store 变化；store 未清空时，没有任何 tick 逻辑会“发现当前该有 offer 但 screen 不在”并补开（`InsightOfferScreenBootstrap.java:39-57`）。
- server 侧 `process_insight_request()` / Redis ingest 都会插入 `PendingInsightOffer`（`server/src/cultivation/insight_flow.rs:176-217`，`server/src/network/mod.rs:2452-2458`），而 `apply_insight_chosen()` 只在收到 client decision 后移除它（`insight_flow.rs:220-325`）。全仓 grep `PendingInsightOffer` 未见独立 timeout/deadline 清理系统。

## 这个 bug 对实际游玩体验的影响

- 玩家会在**没有做出选择、也没有收到失败提示**的情况下丢掉一次顿悟/心魔抉择，体感是“弹窗自己没了，机缘也没了”。
- 对普通顿悟，这会吞掉一次成长抉择；对心魔劫，这会把高风险高收益分叉变成**无 UI、无超时结算、无重开入口**的悬空态。
- 因为是 client 本地切屏触发，玩家很容易在下意识按 `C/E/O/L/T` 时复现，属于正常游玩热键路径，不是测试器特供。

## 修复建议

- **client 收口补齐**：给 `InsightOfferScreen` 增加 `removed()` 语义；若 screen 在未结算状态下被替换，必须显式走 `settle(declined)`、`settle(timedOut)` 或“禁止被普通本地 screen 顶掉”的一致策略。
- **open policy 收紧**：`Craft/Inspect/Identity/Lingtian/SpiritTreasure` 这类本地开屏 bootstrap 不应在任意 `currentScreen` 上直接覆盖；至少要对 `InsightOfferScreen`、其他强制决策屏做保护。
- **server 兜底**：为 `PendingInsightOffer` 增加权威 deadline/timeout cleanup，避免 client UI 丢失后 server 侧永久悬挂。

## 反方裁决

- **第 1 轮反方论点**：`InsightOfferScreenBootstrap` 设计上“应当抢焦点”，所以玩家理论上不该再打开别的 screen，这不是 bug。
- **驳回理由**：抢焦点只发生在 offer 写入 store 的那一刻；之后 `CraftScreenBootstrap`、`InspectScreenBootstrap`、`IdentityPanelScreenBootstrap`、`LingtianActionScreenBootstrap`、`SpiritTreasureScreenBootstrap` 都只防“同类 screen 重复打开”，**没有任何 `currentScreen == null` / modal guard**，所以玩家后续依然能主动把它顶掉。
- **第 2 轮反方论点**：即便 UI 被顶掉，client 90s 超时或 heart-demon TTL 最终也会把会话收掉，最多只是视觉问题。
- **驳回理由**：普通顿悟 timeout 只在 `InsightOfferScreen.tick()` 跑；screen 被替换后不再 tick。store 只是一个被动快照，没有定时器；server 侧也未看到 `PendingInsightOffer` deadline system。它不是“视觉没回来”，而是**结算链路本身断了**。

## 退化处理说明

- 本会话没有可用的 subagent / delegate_task 工具可再开独立反方代理；以上两轮反方裁决为主代理手工执行，但论点与驳回理由均基于实际代码路径，不做“凭感觉裁决”。

## 审计来源

- bug-hunt（本轮，worktree `bughunt-loop-20260705-cc-client-flow`）；
- 聚焦 client flow / screen flow / open-close sequencing；
- 证据来自 `client/src/main/java/com/bong/client/insight/`、`client/.../*ScreenBootstrap.java`、`server/src/cultivation/insight_flow.rs`、`server/src/network/mod.rs` 的静态读树与去重复核。
