# plan-bughunt-anqi-hud-session-lasttick

> **Finished BugFix Plan（2026-07-15 归档）**。历史来源（升格前）：
> `docs/plans-skeleton/plan-bughunt-anqi-hud-session-lasttick.md`。
> BugHunt C2 client-ui 第二轮结论：暗器 HUD 的 `AnqiHudStateStore` 在断线/切服时
> 没有清理 per-dimension `lastTick`，导致同一客户端进程连接新 server / 新世界后，
> 低 tick 的 `anqi_hud` payload 被当成旧包静默丢弃。

阶段总览：P0 修复前失败契约 ✅ 2026-07-15；P1 生产断线 reset ✅ 2026-07-15；
P2 定向测试与 client 门禁 ✅ 2026-07-15；P3 主线同步、审查与归档 ✅ 2026-07-15。

## Bug 摘要

`AnqiHudStateStore` 用 `DimSlot.lastTick` 对 echo / charge / abrasion / multishot 四个暗器 HUD 维度做乱序保护。`snapshot()` 只在渲染时把过期维度贡献成空值，不会把 slot 写回 empty；断线清理路径也没有调用 `AnqiHudStateStore.clear()`。因此旧 session 的高 `lastTick` 会跨 session 留在 static store 里，新 server 从较低 `CombatClock.tick` 开始发送暗器 HUD 时，`updateSlotCas()` 直接 return，HUD 反馈不再更新，直到新 tick 追上旧 tick。

边界：只限定同一个 Minecraft 客户端 JVM 进程内断线、切服、回标题后再连接新 server / 新世界。完全退出游戏重启客户端会清空 static 状态，不复现。

## 实际游玩体验影响

暗器技能本身可能仍然施放、扣真元、播放 VFX/SFX，但玩家看不到对应 HUD 反馈：

- EchoFractal 的回声/分身数量不显示。
- 破甲注射或真元注射的 charge 强度条不显示。
- 载体磨损后的容器与剩余真元载荷不显示。
- MultiShot 齐射弹数提示不显示。

这不是 2 秒 toast 残留。显示值过期后会消失，但 `lastTick` 门禁仍保留。若旧服运行到 `tick=72000` 后切到新服 `tick=0`，受污染维度约 1 小时后才会自然恢复；旧 tick 越高，恢复越晚。玩家会误判暗器招式没有触发、载体没有磨损，或无法判断齐射/注射反馈。

## 证据定位

- `client/src/main/java/com/bong/client/hud/AnqiHudStateStore.java`
  - `DimSlot` 保存 `expiresAtMillis` 和 `lastTick`，empty 只在显式 clear 时把 `lastTick` 设回 `Long.MIN_VALUE`（约 L21-L30）。
  - echo / charge / abrasion / multishot 都调用 `updateSlotCas(...)`（约 L80-L106）。
  - `snapshot(long)` 只按 `expiresAtMillis` 计算渲染值，不清 AtomicReference，不重置 `lastTick`（约 L116-L139）。
  - `clear()` 能清所有维度，但生产断线路径未调用（约 L151-L157）。
  - `updateSlotCas()` 在 `newTick < current.lastTick()` 时静默 return（约 L187-L194）。
- `client/src/main/java/com/bong/client/combat/handler/AnqiHudServerDataHandler.java`
  - `handle()` 要求 payload 精确包含 `tick`，并在字段缺失、类型错误或越界时拒绝该 payload；
    合法 tick 才会传入 `AnqiHudStateStore.update*`（约 L49-L86）。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java`
  - 每帧用 `AnqiHudStateStore.snapshot()` 构建暗器 HUD（约 L312-L317）。
- `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java`
  - `resetOnDisconnect()` 清理大量 combat HUD store，但没有 `AnqiHudStateStore.clear()`（约 L96-L120）。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java`
  - `ClientPlayConnectionEvents.DISCONNECT` 清理 RealmCollapse / NPC / Tsy / Coffin / Gathering / AgentUi / HalfStep 等 static 状态，但没有清 `AnqiHudStateStore`（约 L131-L170）。
- `server/src/combat/mod.rs` 与 `server/src/combat/debug.rs`
  - `CombatClock` 是 `Default` 的 `tick: u64`（`mod.rs` 约 L77-L80）。
  - 生产注册 `CombatClock::default()`（`mod.rs` 约 L206）。
  - `tick_combat_clock` 每 tick `saturating_add(1)`（`debug.rs` 约 L12-L14），并在 combat Intent 阶段调度（`mod.rs` 约 L254-L261）。
- `server/src/combat/anqi_v2.rs` 与 `server/src/network/anqi_hud_emit.rs`
  - 暗器施放读取 `CombatClock.tick`（`anqi_v2.rs` 约 L427-L430）。
  - `MultiShotEvent` / `QiInjectionEvent` / `ArmorPierceEvent` / `CarrierAbrasionEvent` / `DecoyDeployEvent` 都携带 tick（`anqi_v2.rs` 约 L197-L257）。
  - `emit_anqi_hud_payloads` 将 `event.tick` 直接写入 `AnqiHudV1.tick`（`anqi_hud_emit.rs` 约 L51-L59、L88-L96、L122-L130、L158-L166、L193-L200）。

## 触发路径

1. 玩家在 server A 使用暗器技能，某维度写入高 tick，例如 `AnqiHudStateStore.updateEcho(..., tick=72000)`。
2. 2 秒显示期结束后，`snapshot(now)` 不再显示 echo，但 `ECHO_SLOT.lastTick` 仍是 72000。
3. 玩家不断开客户端进程，回标题或切服后连接 server B / 新世界。server B 的 `CombatClock` 从 0 附近开始。
4. 玩家在 server B 使用同一暗器维度，客户端收到 `anqi_hud kind=echo tick=10`。
5. `updateSlotCas(ECHO_SLOT, 10, ...)` 发现 `10 < 72000`，静默 return。
6. HUD 不显示新的 echo 反馈；charge / abrasion / multishot 维度按各自旧 `lastTick` 独立受污染。

## 反方审查记录

第一轮反方质疑：

- 复现是否只限同一客户端进程，而非重启游戏。
- 是否存在其他 disconnect / join / hydration 路径间接清理 `AnqiHudStateStore`。
- `CombatClock.tick` 是否生产持久化，保证跨重连/跨服单调。
- 过期 slot 是否仍保留 `lastTick` 并继续挡包。
- 影响是否只是 2 秒 TTL 残留。
- 是否与 #970 或已有暗器 HUD PR 重复。

补证与让步：

- 明确收窄为同 JVM 进程内断线/切服；退出游戏不复现。
- 全仓 grep 显示生产路径只有 handler update、orchestrator snapshot、debug 命令 clear/replace；常规断线 reset 清单未包含 `AnqiHudStateStore.clear()`。
- `CombatClock` 生产注册 default 并每 tick 自增，未见 save/load/restore 生产路径；暗器 HUD tick 直接透传事件 tick。
- `snapshot()` 只隐藏过期值，不清 slot；低 tick 丢弃由现有 `staleTick*IsDropped` 测试语义覆盖，缺组合 pin 测试。
- 影响是 per-dimension 门禁残留，不是 2 秒显示残留。
- PR 搜索：#970 是暗器充能完成天道叙事断链，历史 #400/#648/#174/#121 是暗器功能/AV/接线，不覆盖 client HUD session reset + lastTick 门禁。

最终反方裁决：通过。必须在后续修复中写清同进程边界、分维度污染、旧 tick 高于新 tick 的复现条件，以及 debug 命令不是生产保护。

## Skeleton Fix Plan

- [x] TODO 1：在生产断线 reset 路径补 `AnqiHudStateStore.clear()`。已接入 `CombatHudBootstrap.resetOnDisconnect()`；该 store 属于 combat HUD 状态，未在 `BongNetworkHandler` 重复接线。
- [x] TODO 2：补 store 级 pin 测试：高 tick 写入短 TTL，过期后 snapshot 为空；未 clear 时低 tick 新 payload 仍被挡，证明旧 bug 机制。
- [x] TODO 3：补生产断线 reset 回归：高 tick 污染后执行实际 disconnect reset 路径，再喂低 tick `anqi_hud`，确认新 HUD 可显示。
- [x] TODO 4：覆盖四个已实现维度：echo、charge、abrasion、multishot。`aim` 当前 server 不发，未纳入本 bug 验收。
- [x] TODO 5：补 handler 级回归：真实 `AnqiHudServerDataHandler` 接收新 session 低 tick payload，确认经过 reset 后进入 `AnqiHudStateStore`。
- [x] TODO 6：检查 debug `/bonghud` 命令仍可手动 clear；命令语义未改，也未将其作为生产 reset 的唯一依据。

## 接入面与收口决议

- 唯一生产修复点放在 `CombatHudBootstrap.resetOnDisconnect()`：该方法已由
  `ClientPlayConnectionEvents.DISCONNECT` 注册，且集中清理 combat HUD stores；加入
  `AnqiHudStateStore.clear()` 不再在 `BongNetworkHandler` 重复接线。
- store 的过期语义保持不变：`snapshot(now)` 只隐藏 TTL 已过期的反馈，不在同一 session
  内重置 `lastTick`，继续保护乱序包。只有明确 disconnect 才开启新的 tick epoch。
- 修复前红灯直接驱动真实 `CombatHudBootstrap.resetOnDisconnect()`：先向 echo、charge、
  abrasion、multishot 各写高 tick，再断线 reset，再写低 tick；当前代码应因四个低 tick
  全被 stale gate 拒绝而失败。
- store 机制测试另锁定“TTL 过期不等于 tick gate 清除”，避免未来为修此 bug 误删同 session
  乱序保护。
- handler 层用真实 `AnqiHudServerDataHandler` 证明 reset 后低 tick payload 能进入 store；
  wire/schema/server emitter 不改。
- `aim` 虽由 store 实现，但当前 server 不发该 kind，不纳入生产回归；`clear()` 仍自然清它。
- debug `/bonghud` 的手动 clear 保持原样，它不是生产生命周期保护。
- 本修复仅处理 client static HUD 生命周期，不改变暗器施放、真元 ledger、A/V 或技能资产。

## 实施阶段

- P0 ✅ 2026-07-15：加入 TTL/stale gate 与生产 disconnect reset 的修复前失败契约；`cbcea83c` 初始运行出现
  3 项失败，其中 1 项是错误比较历史 `expiresAt` 的测试 oracle；`e1759121` 校正后、
  `9a3c839f` 生产修复前实际保留 2 项目标红灯。
- P1 ✅ 2026-07-15：在 combat HUD 生产断线路径清理 `AnqiHudStateStore`。
- P2 ✅ 2026-07-15：完成 store/bootstrap/handler 定向测试与 Java 17 client 完整门禁。
- P3 ✅ 2026-07-15：同步主线、主 agent 对抗自审、填写 Finish Evidence 并归档。

## 验收矩阵

| 场景 | 必须断言 |
|---|---|
| 同 session 高 tick → 低 tick | echo/charge/abrasion/multishot 的低 tick 均被拒绝 |
| 高 tick TTL 过期 | snapshot 不显示旧反馈，但未 disconnect 时低 tick 仍被拒绝 |
| disconnect reset | 所有维度快照清空，随后低 tick 四维均接受 |
| handler after reset | 真实 `anqi_hud` 低 tick payload 被 handler 接受并写入 store |
| 同 session 正常乱序保护 | 既有 stale/same-tick/跨维测试继续通过 |
| debug clear | 原手动 clear 路径仍保留，不充当生产断线接线 |
| 完整门禁 | JDK 17 下 `./gradlew test build` 全绿 |

## 非目标

- 不把 tick 改成 wall clock、session UUID 或服务端全局持久化 tick。
- 不在 TTL 过期时自动重置 `lastTick`，避免接受同 session 迟到旧包。
- 不修改暗器 HUD schema、server emitter、视觉样式或其它非 combat static store。

## 验收测试计划

- `cd client && ./gradlew test --tests com.bong.client.hud.AnqiHudStateStoreTest`
  - 新增 red/green：`expiredHighTicksStillRejectLowerTicksUntilExplicitSessionClear`。
  - 覆盖 echo / charge / abrasion / multishot 维度。
- `cd client && ./gradlew test --tests com.bong.client.combat.CombatHudBootstrapTest`
  - 新增断线 reset pin：`resetOnDisconnectOpensNewTickEpochForAllProducedAnqiHudDimensions`。
  - 新增四维真实 handler pin：`resetOnDisconnectLetsRealHandlerAcceptLowerTicksForAllProducedKinds`。
- `cd client && ./gradlew test --tests com.bong.client.combat.handler.AnqiHudServerDataHandlerTest`
  - 运行既有 handler schema、kind 路由与乱序保护回归；disconnect 集成回归位于
    `CombatHudBootstrapTest`，因为它需要直接驱动包级可见的生产 reset 方法。
- 最终 client gate：`cd client && ./gradlew test build`，使用 JDK 17。

## 风险

- 如果把 clear 放错生命周期，可能误清同一 server 内短暂网络抖动期间仍应显示的 2 秒反馈；不过现有断线 reset 已清大量 HUD store，断线语义应以新 session 干净首帧为准。
- 如果只测 `AnqiHudStateStore.clear()`，无法保证真实 disconnect reset 调到它；必须测生产 reset 路径。
- 如果未来 server 改成持久化全局 tick，本 bug 的跨服低 tick 条件会变窄，但 static HUD store 跨 session 未清仍是不一致状态。
- 只清 client HUD store，不应影响暗器实际施放、真元守恒、server 事件发射。

## Finish Evidence

### 落地清单

- `CombatHudBootstrap.resetOnDisconnect()` 现在调用 `AnqiHudStateStore.clear()`，在同一 Minecraft 客户端 JVM 断线、切服或回标题后清除五个维度的 `lastTick`，开启新的 tick epoch。
- `AnqiHudStateStore` 的 TTL 语义和同 session 乱序保护保持不变：过期只隐藏显示值，未断线的低 tick 仍被拒绝；只有显式 disconnect reset 才清 gate。
- store pin 覆盖 echo、charge、abrasion、multishot；bootstrap pin 走真实 reset；handler pin 走真实 `ServerDataEnvelope`/`AnqiHudServerDataHandler` 低 tick payload。aim 仍因 server 无生产事件而不纳入验收。
- debug `/bonghud` 的手动 clear 路径保持不变；未改 schema、server emitter、A/V、真元逻辑或其它 static store。

### 关键提交

- `25e2fc6c`（2026-07-15）：升格并补全本 BugFix plan。
- `cbcea83c`（2026-07-15）：加入修复前失败契约与 store/bootstrap/handler 回归测试。
- `e1759121`（2026-07-15）：校准过期快照断言，区分“显示字段为空”和历史 expiresAt。
- `9a3c839f`（2026-07-15）：在生产断线 reset 接入 `AnqiHudStateStore.clear()`。
- `311e40de`（2026-07-15）：稳定 handler 回归的测试时钟读取，避免慢 CI 的 2 秒 TTL 抖动。
- `6c6721c3`、`2c13f7a4`（2026-07-15）：归档 plan 并补入首版 Finish Evidence。
- `463e278b`（2026-07-15）：按 `/review` 意见，让 echo、charge、abrasion、multishot 四种低 tick payload
  均经过真实 envelope parser 与 handler，并分别断言 store 写入。
- `5a67caf0`（2026-07-15）：收口首轮 review 返工证据。
- `31a1dbe4`（2026-07-16）：补全 boolean 断言的可诊断失败信息。
- `5ca2906a`（2026-07-16）：把当时最新 `origin/main` 合并进 PR 分支。
- `4f02b240`（2026-07-16）：对合并主线后的最终代码树复验暗器 HUD 断言与门禁。
- `0972f7c9`（2026-07-16）：PR #1214 合入 `main` 的最终 merge commit。
- `5a4d4287`（2026-07-18）：校正 handler 必须显式携带 `tick` 的严格契约，并补齐
  P0–P3 已完成状态与验收日期。
- `c1c7218d`（2026-07-18）：显式合并主线 `62f90990`，在审计分支上复验归档证据。
- `885a0d5f`（2026-07-18）：继续同步主线 `9a9d48a7`；该提交仅新增饱食饮水系统
  skeleton，未改变本 plan 的 client/server 代码树。

### 测试结果

- 修复前定向测试：`cbcea83c` 初始有 3 项失败；其中 1 项是错误的 `expiresAt` 快照 oracle，
  `e1759121` 校正后、`9a3c839f` 生产修复前实际有 2 项目标红灯，锁定真实 disconnect
  reset 与 handler 路径。
- 定向回归（JDK 17.0.19）：
  `./gradlew test --tests com.bong.client.hud.AnqiHudStateStoreTest --tests com.bong.client.combat.CombatHudBootstrapTest --tests com.bong.client.combat.handler.AnqiHudServerDataHandlerTest`，`BUILD SUCCESSFUL`。
- 完整 client 门禁（JDK 17.0.19）：`./gradlew test build`，13 actionable tasks，`BUILD SUCCESSFUL`。
- `/review` 返工代码树 `463e278b`：四维真实 handler 定向回归通过；随后
  `./gradlew test build` 再次通过（58s，13 actionable tasks）。
- GitHub e2e run `29403697278` 在 `2c13f7a4` 上通过，run `29406478061` 在
  `5a67caf0` 上再次通过。
- 最终 PR HEAD `4f02b240` 的 e2e run `29496244134` 成功；Client stage（Java 17）、
  schema、agent、server、smoke/E2E 与 bot e2e 各阶段均为 `success`。
- 2026-07-18 归档核对完整 client 门禁（JDK 17）：`./gradlew test build`，4118 tests，
  0 failures / 0 errors / 0 skipped，13 actionable tasks；合并 `62f90990` 后再次完整通过。
- 2026-07-18 归档核对完整 server 门禁：
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，lib 11775、
  main 11、integration 5，合计 11791 passed、0 failed、6 ignored。

### 主线、review 与独立复核

- PR 分支并非“无需合并主线”：`5ca2906a` 明确把当时最新主线提交 `14b34a62`
  合入分支，随后 `4f02b240` 对合并后的相同代码树完成复验；PR 最终以 `0972f7c9`
  合入 `main`。
- 归档核对分支随后又以 `c1c7218d` 合并 `62f90990`，并以 `885a0d5f` 同步
  `9a9d48a7`；后者只新增一份 skeleton，未触碰本 plan 相关栈。
- 最终 `/review` 评论
  `https://github.com/Kizunad/Bong/pull/1214#issuecomment-4991561202` 在 `4f02b240`
  上给出 `4/0` APPROVE、无 blocker/major；模型记录为 `gpt-5.6-sol`。
- CodeRabbit 汇总评论在最终 HEAD 更新为 “No actionable comments”，PR status 为 `SUCCESS`。
- 逐入口复核确认：断线 hook 已注册并执行 `resetOnDisconnect()`；四个生产维度均由同一
  store clear 重置并各自通过真实 parser/handler；TTL 过期不自动清 gate；debug clear
  不是生产唯一依据。
- `/review` 提出的“断线后在途旧连接 payload”缺少 Fabric 客户端生命周期倒序的可达证据，
  最终复投也判定不构成 major；引入 session UUID/generation 明确属于本 plan 非目标，未扩写。
- `/review` 中 `.github/scripts/review.mjs` finding 来自审查模型端点 502/空响应，未指向本 PR
  修改，也未改 review 基础设施；代码侧有效 finding 已在 `463e278b`、`31a1dbe4` 与
  `4f02b240` 中处理。
- 2026-07-18 归档核对时，早期 fresh read-only validator 未发现代码、接线或测试 blocker；
  其 FAIL 指向旧 Finish Evidence 的主线/review/e2e 事实缺口，以及 handler `tick` 契约与
  finished 状态格式。`5a4d4287` 完成后 generation 4 在该 HEAD 给出 PASS。
- 主线两次同步后，generation 6 fresh read-only validator（`gpt-5.6-sol`）在
  `885a0d5f239c91a6634cbcf6cb9172854d433826` 给出固定结论：生产接线、五槽清理、
  stale tick、schema/守恒及历史测试、主线、e2e/review 证据均闭环；HEAD 对拍一致、
  worktree clean，且最新 `origin/main` 为该 HEAD 祖先。

### 跨仓库核验

- Client：`CombatHudBootstrap.register()` 注册 `ClientPlayConnectionEvents.DISCONNECT`，
  `resetOnDisconnect()` 调用 `AnqiHudStateStore.clear()`；`CombatHudBootstrapTest` 通过真实
  `ServerDataEnvelope` 与 `AnqiHudServerDataHandler` 锁定四种生产 kind 的跨会话低 tick 恢复。
- Server：`CombatClock` / `tick_combat_clock` 从新 server 的低 tick epoch 起步；
  `emit_anqi_hud_payloads` 把事件 tick 写入 `AnqiHudV1.tick`，与本 bug 的跨服触发条件一致。
- Schema / proto：`AnqiHudV1`、`ServerDataAnqiHudV1`、`message AnqiHud` 及
  `server-data.anqi-hud.*.sample.json` 继续锁定 `anqi_hud` 完整字段和 tick 契约；本修复未改 wire。
- Agent：无运行时消费或命令变更；agent/schema 阶段已由最终 e2e run `29496244134` 验证。

### 遗留 / 后续

- `aim` 是正式 wire/store 维度，但当前 server 无生产事件源，因此不属于本 bug 的四维生产验收；
  `clear()` 仍会自然重置它。
- 无代码、schema、守恒或 A/V 遗留；本轮仅修正归档证据。无 `[BLOCKED: ...]` 项。
