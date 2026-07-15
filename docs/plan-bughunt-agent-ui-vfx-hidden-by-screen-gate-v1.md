# plan-bughunt-agent-ui-vfx-hidden-by-screen-gate-v1（active）

> **来源**：`docs/plans-skeleton/plan-bughunt-agent-ui-vfx-hidden-by-screen-gate-v1.md`，2026-07-15 升格。  
> **一句话主题**：`AgentUiScreen` 的计划内 HUD 覆层已经接到 `BongHudOrchestrator`，但 `BongHud` 在 screen 门控阶段把它判成 `HIDDEN` 并直接 `return`，导致 `agent_ui` 三条生产入口（`tsy_discovery` / `elder_legacy` / `tiandao_revelation`）的 fade-in tint 全部永远不显示；这和已存在的 `#937 agent_ui 天道启示 VFX 语义位丢失` 不是同题，后者即使修掉，本题的屏幕门控仍会继续吞掉覆层。

## 接入面与边界

- **进料**：`AgentUiScreen.init()` 写入 `AgentUiVfxStore`，`BongHudOrchestrator.buildCommands(...)` 从 `AgentUiVfxPlanner` 取得 `HudRenderLayer.AGENT_UI` 命令。
- **出料**：`BongHud.render(...)` 在 `AgentUiScreen` 打开时继续渲染 `SCREEN_TINT` / `EDGE_VIGNETTE` / shake `RECT`，但不显示其他 HUD layer。
- **共享类型**：复用 `ScreenHudVisibility`、`HudRenderCommand`、`HudRenderLayer.AGENT_UI`；不新建第二套 VFX store、planner 或渲染入口。
- **跨仓库契约**：本修复只改 Fabric client 的 screen→HUD layer 策略；server、agent、schema wire 契约不变。
- **worldview 锚点**：沿用 `plan-agent-ui-data-v1` 的 `worldview.md §五` 境界感知与 `§八` 天道行为准则；本修复不新增正典。
- **qi_physics 锚点**：纯 client 视觉路由，不产生或修改任何 `QiTransfer`。
- **非目标**：不修 `#937` 的天道启示语义位，不改 VFX 数值/颜色/时长，不新增视觉资产，不放开完整 HUD，也不调整其他 Screen 的既有策略。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 第一性原理复现 screen gate + layer filter 双重断链 | fix_pr | ⬜ |
| P1 | 新增 `AGENT_UI_ONLY` 策略并接入集中式命令过滤 | fix_pr | ⬜ |
| P2 | 饱和回归、Java 17 client 门禁、主线同步与归档 | fix_pr | ⬜ |

## P0 — `AgentUiScreen` 被屏幕门控提前吞掉，计划内覆层永远不出

### 复现路径

1. 走任一生产 `agent_ui` 面板入口即可：`agent/packages/tiandao/src/ui/xmlTemplates.ts:68-72` 明确存在 `tsy_discovery` / `elder_legacy` / `tiandao_revelation` 三类模板。
2. client 收到 `bong:agent_ui_request` 后，`AgentUiPayloadHandler.handleRawRequest(...)` 会 `AgentUiStore.setActive(screen); client.setScreen(screen);`，把当前 screen 切成 `AgentUiScreen`（`client/src/main/java/com/bong/client/network/AgentUiPayloadHandler.java:128-136`）。
3. `AgentUiScreen.init()` 会把 `AgentUiVfxState` 写入 `AgentUiVfxStore`，注释明确这是“供 HUD planner 读取”的 P3 VFX 链路（`client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:41-46,173-195`）。
4. `BongHudOrchestrator` 也确实每帧把 `AgentUiVfxPlanner.buildCommands(...)` 追加进 HUD 命令流（`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:443-449`）；`AgentUiVfxPlannerTest` 还锁了 fade-in `SCREEN_TINT` / tiandao `EDGE_VIGNETTE` / shake `RECT` 命令会生成（`client/src/test/java/com/bong/client/agentui/AgentUiVfxPlannerTest.java:147-220`）。
5. 但实际渲染时，`BongHud.render` 先取 `currentScreen`，再调用 `ScreenHudVisibility.forScreen(currentScreen)`；只要结果是 `HIDDEN` 就直接 `return`，连 `BongHudOrchestrator.buildCommands(...)` 都不会执行（`client/src/main/java/com/bong/client/BongHud.java:76-82`）。
6. `ScreenHudVisibility` 的白名单只含 `InspectScreen` / `CultivationScreen` / `DynamicXmlScreen` / `InsightOfferScreen`，`AgentUiScreen` 不在名单里；而它继承的是 `BaseOwoScreen`，也不会命中 `HandledScreen<?>` 分支，因此最终落到兜底 `return HIDDEN`（`client/src/main/java/com/bong/client/hud/ScreenHudVisibility.java:21-35`，`client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:49`）。

### 根因链路

- 设计规格要求 `AgentUiScreen` 打开时出现面板 fade-in，且天道启示额外带暗蓝 vignette + 轻微 shake（`docs/finished_plans/plan-agent-ui-data-v1.md:283-290`）。
- 生产代码已经按这个规格拆成了三段：`AgentUiScreen.init()` 写状态、`AgentUiVfxPlanner` 产命令、`BongHudOrchestrator` 接 HUD 命令流。
- 真正断链点发生在更外层的 HUD screen 门控：`AgentUiScreen` 被 `ScreenHudVisibility` 当成“应完全隐藏 HUD 的普通 Screen”，`BongHud.render` 直接早退。
- 因为早退发生在命令构建前，结果不是“效果生成了但被层级盖住”，而是**整个 `AGENT_UI` 命令流根本没有进入渲染阶段**。
- 这也是为什么现有测试没报红：`AgentUiVfxPlannerTest` 只测“命令能否生成”，但仓内没有“当 `currentScreen instanceof AgentUiScreen` 时 HUD 是否仍会渲染 `AGENT_UI` layer”的集成护栏。

### 影响面

- 所有 `agent_ui` 生产入口都会丢失通用 fade-in 黑幕淡入，不只是天道启示；玩家看到的是“普通 Owo 弹窗瞬开”，而不是 plan 规定的仪式化入场。
- `tiandao_revelation` 即便后续补上 `#937` 里的语义位，当前这条 `HIDDEN` 早退仍会继续吞掉暗蓝 vignette / shake，所以两题是串联而非重复。
- 这类问题属于典型 screen-layer 反馈断链：screen 本体能打开、按钮也能点，但负责“这不是普通弹窗”的覆层语言完全消失，事件层级感和危险辨识度会显著下降。

### 修复建议

- 不建议只把 `AgentUiScreen` 粗暴加入 `CAST_BAR_ONLY`。原因是 `filterCastBarOnly(...)` 只保留 `CAST_BAR` 层，`AGENT_UI` 仍会被二次过滤掉（`client/src/main/java/com/bong/client/BongHud.java:336-339`）。
- 两个可行方向：
  - 新增一个专门给“自带 screen 但仍需渲染覆层”的可见性档位，例如仅保留 `AGENT_UI`（必要时带 `CAST_BAR`）而非全量 HUD。
  - 或把 `AgentUiVfxPlanner` 的输出挪到 `HIDDEN` 早退之前单独渲染，并显式约束只在 `AgentUiVfxStore` 非空时生效。
- 无论选哪条，都应补一条集成测试，锁住“`currentScreen instanceof AgentUiScreen` 时 `AGENT_UI` 命令不会被 `ScreenHudVisibility`/过滤器吞掉”。

### pre-P0 实施决议（2026-07-15）

1. `ScreenHudVisibility` 新增 `AGENT_UI_ONLY`，并让 `AgentUiScreen` 精确映射到该策略；不复用 `CAST_BAR_ONLY`，因为它会继续过滤 `AGENT_UI`。
2. `BongHud` 把 visibility→layer 的规则收口为 package-private 纯函数 `filterCommandsForVisibility(...)`；`AGENT_UI_ONLY` 只保留 `HudRenderLayer.AGENT_UI`，不附带 `CAST_BAR`、`QUICK_BAR`、`EVENT_STREAM` 或 `BASELINE`。
3. `HIDDEN` 仍在命令构建前早退；其他策略继续构建后过滤。新增策略因此能走真实 `BongHudOrchestrator` 生产链，同时不改变 Death/Pause/未知 Screen 的隐藏语义。
4. 测试直接构造真实 `AgentUiScreen.create(...)` 锁定分类，并用混合 layer 命令列表锁定过滤契约；现有 `AgentUiVfxPlannerTest` 继续证明 fade/vignette/shake 都产出在 `AGENT_UI` layer。

**落点**：`client/src/main/java/com/bong/client/hud/ScreenHudVisibility.java`、`client/src/main/java/com/bong/client/BongHud.java`、`client/src/test/java/com/bong/client/BongHudTest.java`；对应 plan P0-P2。

### 验收抓手

- 打开任意 `agent_ui` 面板时，首 500ms 必须能看到 plan §5 定义的全屏黑 tint 淡出，而不是纯瞬开。
- 打开 `tiandao_revelation` 时，在 `#937` 修复到位后，还应继续看到 `vignette + shake`；若只修语义位不修本题，效果仍应缺失，这可作为回归判据区分两题。
- 现有 `InspectScreen` / `CultivationScreen` / `InsightOfferScreen` / `HandledScreen` 的 HUD 策略不能被连带改坏。
- 新测试至少覆盖：
  - `ScreenHudVisibility` 对 `AgentUiScreen` 的分类。
  - `BongHud` 在 `AgentUiVfxStore` 非空且 `currentScreen` 为 `AgentUiScreen` 时不会早退吞掉 `AGENT_UI` 层。

## 反方裁决

> 当前会话无 subagent 能力，按退化流程执行 **2 轮人工反方裁决**；结论与驳回理由一并固化如下。

### Round 1

- **反方论点**：这可能不是 bug，而是设计上故意“打开任意自定义 screen 时都隐藏 HUD”，`AgentUiVfxPlanner` 只是遗留代码。
- **驳回理由**：
  - `plan-agent-ui-data-v1` 的 §5 明确把 fade-in / vignette / shake 写成验收规格（`docs/finished_plans/plan-agent-ui-data-v1.md:283-290`）。
  - `AgentUiScreen` 注释明确写“注册 `AgentUiVfxState` 到 `AgentUiVfxStore`（供 HUD planner 读取）”，`BongHudOrchestrator` 也有同名接线注释（`client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:41-46`，`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:443-449`）。
  - 若这是有意停用，不会同时保留规格、生产接线和专门单测三套证据链。

### Round 2

- **反方论点**：把 `AgentUiScreen` 加到 `CAST_BAR_ONLY` 白名单就行，所以根因并不成立，最多只是一个待打磨的小策略问题。
- **驳回理由**：
  - `CAST_BAR_ONLY` 后续还会过 `filterCastBarOnly(...)`，该过滤器只保留 `CAST_BAR`，不保留 `AGENT_UI`（`client/src/main/java/com/bong/client/BongHud.java:336-339`）。
  - 这说明问题不是单点白名单漏配，而是“screen 分类 + 后置 layer 过滤”两级共同造成的真实断链；若误判成单点小修，修复 PR 很容易做成假绿。

## 去重与退化说明

- 已按用户排除项去重：不是 `toast cross-session`、不是 `false_skin_state`、不是毒蛊 HUD 串局、不是 `surface stash` 标签缺口、不是“顿悟弹窗被本地切屏吞掉”、不是“切磋邀请强制抢屏”。
- 已联网核对近期远端同类 PR/issue 标题：
  - `#937 docs(skeleton): 记录 agent_ui 天道启示 VFX 语义位丢失` 是 **S2C 语义位丢失**，与本题的 **HUD screen gate 早退吞覆层** 不同。
  - `#939 agent-ui realm gate 广播泄漏`、`#932 tiandao agent-ui 点击上下文丢失`、`#924 TSY 发现面板错发 fallback` 也均非同题。
- skeleton 阶段为 **report-only**；本 active plan 只授权上述 client HUD gate 修复与对应测试，不扩展到相邻 bug。

## P1 — `AGENT_UI_ONLY` 可见性与命令过滤

- `ScreenHudVisibility.forScreen(...)` 对真实 `AgentUiScreen` 返回 `AGENT_UI_ONLY`。
- `BongHud.filterCommandsForVisibility(...)` 对 `FULL` 原样返回，对 `CAST_BAR_ONLY` / `INVENTORY_DIMMED` 保持既有白名单，对 `AGENT_UI_ONLY` 只保留 `AGENT_UI`。
- `BongHud.render(...)` 使用统一过滤入口；`AgentUiScreen` 不再在 `HIDDEN` 分支提前返回。
- 不改变 `AgentUiVfxPlanner` 的 500ms fade、天道 vignette、shake 或 `AgentUiVfxStore` 生命周期。

## P2 — 测试矩阵与完成门禁

- **分类 happy path**：真实 `AgentUiScreen` → `AGENT_UI_ONLY`。
- **分类回归**：`null` → `FULL`；未知普通 Screen、Death/Pause → `HIDDEN`；现有自定义/handled screen 策略不变。
- **过滤 happy path**：`AGENT_UI_ONLY` 保留 tint、vignette、shake 等所有 `AGENT_UI` kind。
- **过滤负例**：`BASELINE`、`CAST_BAR`、`QUICK_BAR` 等非 `AGENT_UI` layer 全被剔除；空列表保持为空。
- **其他状态**：`FULL` 不丢命令，`CAST_BAR_ONLY` 与 `INVENTORY_DIMMED` 的既有 layer 集合保持不变，`HIDDEN` 返回空列表作为纯函数契约。
- **针对性测试**：`./gradlew test --tests com.bong.client.BongHudTest --tests com.bong.client.agentui.AgentUiVfxPlannerTest`（JDK 17）。
- **完整门禁**：`./gradlew test build`（JDK 17）；随后 fetch 最新 `origin/main`，按 merge-base 分类同步并对当前干净 HEAD 复验。
