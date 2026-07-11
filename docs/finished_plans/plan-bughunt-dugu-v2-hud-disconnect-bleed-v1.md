# plan-bughunt-dugu-v2-hud-disconnect-bleed-v1

> 一句话主题：`dugu_v2_*` / `permanent_qi_max_decay_applied` 这条 server bridge 已经打通，但 client runtime wiring 漏了 disconnect reset，导致上一局的毒蛊 v2 HUD 状态会跨 session 残留到下一局；其中 `revealRisk` / `selfCurePercent` / `selfRevealed` 甚至可无限续命，直到再次收到新的毒蛊 v2 payload。

> 范围声明：本条只聚焦 `server_data` S2C → `ServerDataRouter` → `DuguV2ServerDataHandler` → `DuguV2HudStateStore` → `BongHudOrchestrator` 的 bridge / runtime wiring 路径；不涉及已排除的 tsy discovery target fallback、locust warning duration drift、forge step_state contract drift、zone_info stale。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 毒蛊 v2 HUD 断线跨 session 残留 | fix_pr | ✅ 2026-07-11 |

## P0 — 毒蛊 v2 HUD 断线跨 session 残留

- **复现路径**
  1. 在任意能触发毒蛊 v2 HUD 的玩法路径上制造一次 S2C：如 `EclipseNeedleEvent` / `PenetrateChainEvent` 推 `dugu_v2_skill_cast`，`SelfCureProgressEvent` 推 `dugu_v2_self_cure`，`ShroudActivatedEvent` 推 `dugu_v2_shroud_active`，或 `PermanentQiMaxDecayApplied` 推 `permanent_qi_max_decay_applied`（`server/src/network/dugu_v2_event_bridge.rs:23-229`）。
  2. client 已注册并消费这四类路由：`ServerDataRouter` 把它们都交给 `DuguV2ServerDataHandler`（`client/src/main/java/com/bong/client/network/ServerDataRouter.java:252-257`）。
  3. `DuguV2ServerDataHandler` 会把 `reveal_probability / gain_percent / self_revealed / shroudUntilMs / qi decay` merge 进全局单例 `DuguV2HudStateStore`；其中 `self_revealed` 还是 once-set-stays-true 的 sticky flag（`client/src/main/java/com/bong/client/combat/handler/DuguV2ServerDataHandler.java:40-138`）。
  4. 在 HUD 还亮着时直接断线、切服、重连或回主菜单再进另一局。
  5. 下一局里，即使新角色根本没触发过毒蛊 v2，`BongHudOrchestrator` 仍继续从旧的 `DuguV2HudStateStore.snapshot()` 渲染 HUD（`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:324-329`），会看到残留的“暴露 xx%”“自蕴 xx% 已露”或遮蔽 tint。

- **根因链路**
  - `DuguV2HudStateStore` 是 process-wide 单例，只有 `replace()` / `resetForTests()`，没有任何生产态 `clearOnDisconnect()`（`client/src/main/java/com/bong/client/hud/DuguV2HudStateStore.java:3-56`）。
  - `BongNetworkHandler` 的主 disconnect 清理链显式清了几十个 store，但没有清 `DuguV2HudStateStore`（`client/src/main/java/com/bong/client/BongNetworkHandler.java:131-170`）。
  - `CombatHudBootstrap.resetOnDisconnect()` 也只 reset 旧 combat HUD store，同样漏掉 `DuguV2HudStateStore`（`client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java:96-121`）。
  - server 侧这条 bridge 只有“发生事件时推增量/状态”的生产者，没有任何 join/disconnect 时的 inactive/reset payload；所以一旦 client 本地没清，旧状态就只能等下一次毒蛊 v2 事件来覆盖（`server/src/network/dugu_v2_event_bridge.rs:23-229`）。
  - `DuguV2HudPlanner` 对 `revealRisk > 0` 与 `selfCurePercent > 0 || selfRevealed()` 直接渲染，没有 session guard；因此旧快照会原样漏到下一局（`client/src/main/java/com/bong/client/hud/DuguV2HudPlanner.java:36-67`）。

- **为什么这是实际 bug，而不是“可接受的短暂旧帧”**
  - `revealRisk` 没有 expiry 字段，写进去后不会自动衰减到 0。
  - `selfRevealed` 明确是 sticky flag（`cur.selfRevealed() || selfRevealed`），没有新的 payload 就不会回 false。
  - 这不是像 `qiMaxDecay` 那样 3 秒闪完就没，也不是像 `sword_bond_hud_state` 那样 server 每秒强推 inactive；它可以在下一局无限残留。

- **这个 bug 对实际游玩体验的影响**
  - 玩家切服/重连后，明明当前角色没中毒、没自蕴、没开遮蔽，屏幕上却继续挂着上一局的毒蛊 HUD 提示。
  - 体感上会出现“上一局的毒蛊后效跟到了下一局”的错觉，误导玩家判断自己是否暴露、是否仍在自蕴、是否还处于遮蔽窗口。
  - 对新局决策有直接污染：玩家可能因为假 HUD 误以为自己还在高暴露风险或已自曝，从而改变走位、交战和撤退节奏。

- **影响面**
  - 受影响 payload：`dugu_v2_skill_cast`、`dugu_v2_self_cure`、`dugu_v2_shroud_active`、`permanent_qi_max_decay_applied`。
  - 受影响 client 路径：`ServerDataRouter`、`DuguV2ServerDataHandler`、`DuguV2HudStateStore`、`DuguV2HudPlanner`、`BongHudOrchestrator`、disconnect reset wiring。
  - 受影响玩法：毒蛊 v2 所有会点亮 reveal/self-cure/shroud/qi-decay HUD 的招式或结算。

- **修复建议**
  - 最小修法：给 `DuguV2HudStateStore` 增加生产态 clear/reset，并在 `BongNetworkHandler` 与 `CombatHudBootstrap.resetOnDisconnect()` 两条断线清理路径都接上。
  - 更稳修法：同时审一遍同批次的 AV/HUD store（如 `AnqiHudStateStore` / `SwordBondHudStateStore` / `ZhenmaiHudStateStore`），统一补 disconnect reset，避免同类 runtime bleed 反复再出。
  - 若想双保险：server 可补一个 join 首帧 inactive/reset 快照，但这只能兜底，不能替代 client 断线清理。

## 反方裁决

- **退化说明**
  - 当前会话没有可再开的 subagent / delegate tool；本次两轮反方裁决退化为本地对抗式复核，而非外部分身复核。

- **第 1 轮反方论点**
  - 反方：这可能只是“断线到重连之间 1 帧旧 HUD”，不算真 bug。
  - 驳回：不成立。`revealRisk` 与 `selfRevealed` 没有按时间自动清零；尤其 `selfRevealed` 是 sticky merge，server 侧也没有 inactive/reset S2C，所以下一局若没再触发毒蛊事件，旧值会一直留着，不是 1 帧问题。

- **第 2 轮反方论点**
  - 反方：也许另一个 disconnect reset 链已经把它清了，只是没在主网络文件里看到。
  - 驳回：不成立。`BongNetworkHandler` 的 disconnect 清单与 `CombatHudBootstrap.resetOnDisconnect()` 两条主清理链都逐项列出了被 reset 的 store，但均未包含 `DuguV2HudStateStore`；而 `DuguV2HudStateStore` 自身也只有 `resetForTests()`，没有被其他生产代码调用的 clear 路径。

## 结论

- 这是一个 **REAL / 高置信** 的 server-bridge 后半段 runtime wiring bug。
- 断点不在 schema、也不在 payload 没发到，而在 **client 收到后把跨 session 生命周期管理漏掉了**。

## Finish Evidence

- **第一性原理复验（2026-07-11）**：claim 到手后未直接信任 skeleton 结论，重新对拍 `origin/main`（`374475721657ea7c01f08b577474aeff42dd0627`）：
  - `client/src/main/java/com/bong/client/hud/DuguV2HudStateStore.java` 修复前只有 `replace()` / `resetForTests()` 两个方法，无任何生产态 clear 入口。
  - `client/src/main/java/com/bong/client/BongNetworkHandler.java` 的 `clearClientStateOnDisconnect()`（disconnect 时挂在 `ClientPlayConnectionEvents.DISCONNECT`）逐项列出几十个 store 清理调用，`grep -n "Dugu"` 命中数 = 0——确认未被顺带覆盖。
  - `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java` 的 `resetOnDisconnect()`（第二条独立 disconnect 清理链）同样 `grep -n "Dugu"` 命中数 = 0。
  - 排除"近期同批修复已顺带修掉"的可能：核对 #1171 (`IdentityPanelStateStore`)、#1172 (`FalseSkinHudStateStore`) 等断线残留家族修复，均各自只接线自己名下的 store，未触碰 `DuguV2HudStateStore`。判定为**仍未修复的真 bug**，按 fix_pr 路由继续执行。

- **落地清单**：
  - `client/src/main/java/com/bong/client/hud/DuguV2HudStateStore.java` — 新增生产态 `clearOnDisconnect()`，将静态单例 `snapshot` 复位为 `State.NONE`（含 sticky `selfRevealed`、无 expiry 字段 `revealRisk` 在内的全部 11 个字段）。
  - `client/src/main/java/com/bong/client/BongNetworkHandler.java` — import 接入 `DuguV2HudStateStore`，在 `clearClientStateOnDisconnect()` 尾部追加 `DuguV2HudStateStore.clearOnDisconnect();` 调用，接入生产态断线清理链（该方法已注册在 `ClientPlayConnectionEvents.DISCONNECT`，真实可达）。
  - `client/src/test/java/com/bong/client/hud/DuguV2HudStateStoreTest.java`（新文件）— store 本体隔离单测：默认快照为 NONE、`replace()` 写入、`replace(null)` 回落 NONE、`clearOnDisconnect()` 专项断言 sticky `selfRevealed` 与无 expiry `revealRisk` 及全部 11 字段归位、幂等性（对已是 NONE 的快照重复调用）、清理后不阻断新 session `replace()`。
  - `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java` — `@AfterEach` 补 `DuguV2HudStateStore.resetForTests()`；新增两条集成用例镜像既有 `FalseSkinHudStateStore` 断线清理先例：`disconnectClearsDuguV2HudStateStoreToPreventCrossSessionResidualHud()`、`disconnectClearingDuguV2HudStateStoreDoesNotBlockNewSessionSnapshotAfterReconnect()`。

- **关键 commit**：
  - `ce203333`（2026-07-11）docs(plan): 骨架转正 plan-bughunt-dugu-v2-hud-disconnect-bleed-v1
  - `4718efb3`（2026-07-11）fix(client): 断线时清理毒蛊 v2 HUD 状态防跨 session 残留

- **测试结果**：
  - `cd client && ./gradlew test --tests "com.bong.client.BongNetworkHandlerTest" --tests "com.bong.client.hud.DuguV2HudStateStoreTest" --tests "com.bong.client.hud.DuguV2HudPlannerTest" --tests "com.bong.client.combat.handler.DuguV2ServerDataHandlerTest"` → BUILD SUCCESSFUL（`DuguV2HudStateStoreTest` 6/6、`BongNetworkHandlerTest` 全绿，含 17 条 disconnect 场景用例）。
  - `cd client && ./gradlew test build` → BUILD SUCCESSFUL（全量 client 测试 + jar 构建门禁），`${PIPESTATUS[0]}` = 0。

- **对抗验证（validator）**：无上下文 Explore 类型 validator agent，对 worktree `/home/serverkizuna/Code/Bong/.agent-worktrees/bf-dugu-dc` HEAD `4718efb30fd0f0f27aaaa30b69b450462bd35ec4` 逐条核验（diff 落地、`State.NONE` 全字段归零、disconnect 事件真实可达、sticky/no-expiry 字段专项覆盖、无跨 session 合法读取假设回归、`CombatHudBootstrap` 未重复接线的合理性、非冗余修复排他确认）：结论 **PASS**。

- **跨仓库核验**：
  - client：`DuguV2HudStateStore.clearOnDisconnect()` / `BongNetworkHandler.clearClientStateOnDisconnect()` / `DuguV2HudStateStoreTest` / `BongNetworkHandlerTest` 四个 symbol 均已落地并绿测。
  - server：本条为 client-only runtime wiring bug，未改动 `server/src/network/dugu_v2_event_bridge.rs` 等 server bridge（bridge 本身按 skeleton 结论已打通，无需改动）。
  - agent：未涉及。

- **遗留 / 后续**：
  - skeleton「修复建议」提到的「同批次 AV/HUD store 统一审计」（`AnqiHudStateStore` / `SwordBondHudStateStore` / `ZhenmaiHudStateStore` 等是否也漏 disconnect reset）不在本 plan 范围，留给后续独立 bughunt 轮次或专项审计 plan。
  - skeleton「双保险」提到的 server 端 join 首帧 inactive/reset 快照兜底方案未实施（client 侧清理已是充分修复，server 兜底为可选加固，不阻塞本次收口）。
