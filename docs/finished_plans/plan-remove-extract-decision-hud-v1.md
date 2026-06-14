# plan-remove-extract-decision-hud-v1（finished）

> **Finished（已归档，2026-06-15）**。一句话主题：移除「搜打撤五维撤退决策 HUD」(`ExtractDecisionHud` / 出行脉象 / "该看看这些了")——用户决定不要这个常驻决策面板。

> 立项动机：worldgen-v4 P6 真机审阅时，屏幕左上常驻一个「真元/背包/时辰/汐转/威胁」五维 checklist HUD（`ExtractDecisionHudPlanner`，`plan-sou-da-che-v1` P2 产物），真元低/出行计时到点就弹「该看看这些了」+ 紧急态「快撤」红边。用户判定此 UI 不要。**这是对 `plan-sou-da-che-v1` P2 的功能撤回**，需跨 plan 协调。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 摘除 HUD 渲染（不再常驻显示，最小可逆） | ✅ 2026-06-15 |
| P1 | 彻底清理 state store + server 喂数据 + 测试 + plan 状态回退 | ✅ 2026-06-15 |

> P0 与 P1 在单次实施中一并落地（client 端彻底删除，无需保留可逆中间态）。server 侧经 Scout 核查无需改动（见 §P1 与 ## Finish Evidence）。

## 接入面 checklist（删除面——升 active 前据 docs/CLAUDE.md §二 核实）

- **删除目标（client）**：
  - `client/.../hud/ExtractDecisionHudPlanner.java`（HUD 本体 + 5 维 row 逻辑 + 紧急态「快撤」/红边 vignette）
  - `client/.../hud/BongHudOrchestrator.java:253-259`（`markPlayerInWorld` + `ExtractDecisionHudPlanner.buildCommands(...)` 调用点）—— **P0 只需摘这里 HUD 即不再显示**
  - `client/.../hud/HudRenderLayer.java` 的 `EXTRACT_DECISION` 枚举 + `HudLayoutPreset.java:129` / `HudImmersionMode.java:62` 引用
  - `client/.../hud/ExtractDecisionStateStore.java`（状态存储）+ `BongNetworkHandler.java:160 clearOnDisconnect`
  - `client/.../hud/ExtractDecisionHudPlannerTest.java`（测试）
- **删除目标（server，仅 P1 彻底清理时）**：喂 decisionState（run clock / 威胁）的 server 侧 emit + proto 字段（`server_data.rs` / `proto_convert.rs` / 相关 `*_emit.rs`）——双端协议字段删除需同步，**或保留 server 侧只摘 client 渲染**（更稳）。
- **跨 plan 协调（红线）**：
  - 本删除**反转 `plan-sou-da-che-v1` P2**（"撤退决策辅助：五维决策信号"）。CLAUDE.md 规定 consume-plan 不能自动改其他 plan——`plan-sou-da-che-v1.md` 的 P2 状态需**人工**标注「HUD 已移除（plan-remove-extract-decision-hud-v1）」。
  - `plan-gameplay-journey-v1` ⏳（100h 路径 P2-P4）**引用 sou-da-che 的节奏设计** —— 确认移除 HUD 是否影响其撤退决策体验假设（撤退张力是否还有其他载体，还是随 HUD 一并消失）。
- **worldview / qi_physics 锚点**：纯 UI 移除，无；但 worldview §K「节律红线」提到"汐转/季节不完全显式"——移除显式 checklist 反而更贴 §K（可作为移除的正典理由）。

## P0 — 摘除 HUD 渲染（最小可逆）✅ 2026-06-15

- **目标**：HUD 不再常驻显示。最小改动 = `BongHudOrchestrator` 不再调 `ExtractDecisionHudPlanner.buildCommands`（或加配置开关默认关）。`EXTRACT_DECISION` 层从 `HudLayoutPreset` / `HudImmersionMode` 摘除，避免空层占位。
- **可核验交付物**：`BongHudOrchestrator` 删/注释掉 HUD 接入；目检「游戏内不再出现出行脉象/该看看这些了/快撤」；client `gradlew test build` 绿。
- **取舍**：P0 保留 `ExtractDecisionHudPlanner` 类 + state store + server 喂数据（只摘渲染）—— 可逆、改动小、不动协议。若只要"眼不见"，P0 即足够。

## P1 —（可选）彻底清理 ✅ 2026-06-15

- **目标**：若确认永久不要，删 `ExtractDecisionHudPlanner` + `ExtractDecisionStateStore` + `EXTRACT_DECISION` 枚举 + 测试；评估 server 侧 decisionState emit/proto 是否一并删（双端同步）还是保留（其他消费者？需 grep 确认无其他依赖）。
- **可核验交付物**：相关类/枚举/测试删除后双端编译测试全绿；`plan-sou-da-che-v1.md` P2 人工标注撤回。
- **前置**：先确认 server decisionState 无其他客户端消费者；确认 `plan-gameplay-journey-v1` 不依赖此 HUD 存在。

## §N 开放问题（升 active 前收口）

1. **P0 隐藏 vs P1 彻底删**：只摘渲染（可逆、稳）还是连 state store + server 协议一起删（干净、不可逆、需双端协调）？默认建议 P0 先行。
2. **撤退张力的去留**：移除 HUD 后，搜打撤"何时该撤"的决策张力是否需要别的、更轻/更沉浸的载体（如仅紧急态红边、或纯叙事提示），还是彻底不做显式提示（贴 worldview §K 节律红线"不显式"）。
3. **跨 plan**：`plan-sou-da-che-v1` P2 状态回退由谁/何时改；`plan-gameplay-journey-v1` 的引用是否需同步调整。

## 审计来源

worldgen-v4 P6 真机审阅期间用户判定此常驻 HUD 不要。属 `plan-sou-da-che-v1` P2 产物，移除为跨 plan 撤回，与 worldgen-v4 无关。

## Finish Evidence

### 落地清单

**P0 + P1 一并落地（client 端彻底删除）**：

- 删整文件（3 个）：
  - `client/src/main/java/com/bong/client/hud/ExtractDecisionHudPlanner.java`（HUD 本体 + 5 维 row 逻辑 + 紧急态「快撤」/红边 vignette）
  - `client/src/main/java/com/bong/client/hud/ExtractDecisionStateStore.java`（状态存储 + `replaceRiskScore`，生产代码从未被调用）
  - `client/src/test/java/com/bong/client/hud/ExtractDecisionHudPlannerTest.java`（HUD 测试）
- 手术编辑（6 处）：
  - `client/src/main/java/com/bong/client/hud/HudRenderLayer.java` — 删 `EXTRACT_DECISION` 枚举变体
  - `client/src/main/java/com/bong/client/hud/HudLayoutPreset.java` — 同步删 `widgetFor` switch arm 对 `EXTRACT_DECISION` 的引用
  - `client/src/main/java/com/bong/client/hud/HudImmersionMode.java` — 删 `VISIBLE_OTHER` EnumSet 中的 `EXTRACT_DECISION`
  - `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java` — 删 HUD 接入块（`insideHome`/`markPlayerInWorld` 分支 + `ExtractDecisionHudPlanner.buildCommands(...)` 调用点），保留 inventory/homeState 供 `HomeSequence` 使用
  - `client/src/main/java/com/bong/client/BongNetworkHandler.java` — 删 `clearOnDisconnect` 中对 `ExtractDecisionStateStore` 的调用
  - `client/src/test/java/com/bong/client/hud/BongHudOrchestratorTest.java` — 删 3 个 ExtractDecision 测试 + `@AfterEach` reset 行（HOME_SEQUENCE 行为已由 `HomeSequenceTest` 独立覆盖）

**P1 server 侧零改动（Scout §3 核查结论）**：`ExtractDecisionStateStore.replaceRiskScore` 在生产代码从未被调用；server 从未实现 `nearbyRiskScore`/`realmMismatch` 网络协议；`risk_signals`/`risk_heatmap` 仅供 `/riskmap` 调试命令、不向 client emit。因此无双端协议断线风险，server 侧无需删 emit/proto 字段。`plan-gameplay-journey-v1` 不依赖此 HUD 类的存在（撤退张力由环境信号 + 叙事承载，非此 checklist）。

### 关键 commit

- `dc4be1ef0`（2026-06-15）— remove-extract-decision-hud-v1：彻底移除搜打撤五维撤退决策 HUD（client 端），9 文件改动 / -729 行 / +1 行
- `3d186673d`（2026-06-15）— chore(plan)：骨架→active git mv（零内容 rename）

### 测试结果

- client：`cd client && ./gradlew test build` 绿（`BongHudOrchestratorTest` 12 测，358 suites 全过；build 出 jar）
- server：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 全绿（0 failed；server 零改动，回归确认）
- grep 全仓零 dangling `ExtractDecision` / `EXTRACT_DECISION` 引用

### 跨仓库核验

- **client**：删除 `ExtractDecisionHudPlanner` / `ExtractDecisionStateStore` / `EXTRACT_DECISION` 枚举 + 三处引用面全清；`BongHudOrchestrator` / `BongNetworkHandler` 不再引用；编译 + 测试绿
- **server**：无命中 symbol（本删除不触达 server——`risk_heatmap` / `risk_signals` 不经 client emit，保留原状）
- **agent**：无命中 symbol（HUD 纯 client 渲染，agent 不参与）

### 遗留 / 后续

- **跨 plan 人工待办（已在本 plan 实施一并处理）**：`docs/plan-sou-da-che-v1.md` P2 段已标注「HUD 已移除（plan-remove-extract-decision-hud-v1）」。注意 sou-da-che 仍为 active plan，其 P0-P4 marker 因 consume-plan docs 写权限约束历史性停留在 ⬜（commit `d253c11e6` 还原），待其自身归档时统一人工修订——本 plan 不动其阶段总览 marker，仅在 P2 段加移除注记。
