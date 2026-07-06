# plan-bughunt-anqi-hud-session-lasttick（skeleton）

> BugHunt C2 client-ui 第二轮结论：暗器 HUD 的 `AnqiHudStateStore` 在断线/切服时没有清理 per-dimension `lastTick`，导致同一客户端进程连接新 server / 新世界后，低 tick 的 `anqi_hud` payload 被当成旧包静默丢弃。该 plan 仅记录 skeleton，不消费、不归档。

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
  - `handle()` 读取 payload `tick`，缺省为 0，然后传入 `AnqiHudStateStore.update*`（约 L45-L68）。
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

- [ ] TODO 1：在生产断线 reset 路径补 `AnqiHudStateStore.clear()`。优先放入 `CombatHudBootstrap.resetOnDisconnect()`，因为该 store 属于 combat HUD 状态；如选择 `BongNetworkHandler` 的总清理清单，需说明归属理由，避免两边重复。
- [ ] TODO 2：补 store 级 pin 测试：高 tick 写入短 TTL，过期后 snapshot 为空；未 clear 时低 tick 新 payload 仍被挡，证明旧 bug 机制。
- [ ] TODO 3：补生产断线 reset 回归：高 tick 污染后执行实际 disconnect reset 路径，再喂低 tick `anqi_hud`，应能显示新 HUD。
- [ ] TODO 4：覆盖四个已实现维度：echo、charge、abrasion、multishot。`aim` 当前 server 不发，不纳入本 bug 验收。
- [ ] TODO 5：补 handler 级回归：模拟 `AnqiHudServerDataHandler` 接收新 session 低 tick payload，确认经过 reset 后能进入 `AnqiHudStateStore`。
- [ ] TODO 6：检查 debug `/bonghud` 命令仍可手动 clear，不作为生产 reset 的唯一依据；必要时只更新测试说明，不改变命令语义。

## 验收测试计划

- `cd client && ./gradlew test --tests com.bong.client.hud.AnqiHudStateStoreTest`
  - 新增 red/green：`expiredHighTickStillBlocksLowerTickUntilClear`。
  - 覆盖 echo / charge / abrasion / multishot 维度。
- `cd client && ./gradlew test --tests com.bong.client.combat.CombatHudBootstrapTest`
  - 新增断线 reset pin：`resetOnDisconnectClearsAnqiHudLastTickGate`。
- `cd client && ./gradlew test --tests com.bong.client.combat.handler.AnqiHudServerDataHandlerTest`
  - 新增 handler 级低 tick after reset 回归。
- 最终 client gate：`cd client && ./gradlew test build`，使用 JDK 17。

## 风险

- 如果把 clear 放错生命周期，可能误清同一 server 内短暂网络抖动期间仍应显示的 2 秒反馈；不过现有断线 reset 已清大量 HUD store，断线语义应以新 session 干净首帧为准。
- 如果只测 `AnqiHudStateStore.clear()`，无法保证真实 disconnect reset 调到它；必须测生产 reset 路径。
- 如果未来 server 改成持久化全局 tick，本 bug 的跨服低 tick 条件会变窄，但 static HUD store 跨 session 未清仍是不一致状态。
- 只清 client HUD store，不应影响暗器实际施放、真元守恒、server 事件发射。
