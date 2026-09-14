# plan-refactor-client-ui-base-v1 — 全客户端窗口管理器 + Inspect 工作台 + owo XML / SVG 表现（重构轨 R7）

> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md`。一句话：消费现有 Store、semantic surface 和 typed intent，把 Inspect、锻造、手搓、工作台制作等功能接入全客户端窗口管理器，支持独立多窗、最小化、拖动、输入尺寸、HUD 固定和可更换背景；窗口视图使用本地 owo XML，SVG / PNG 与 Minecraft GUI 承担表现，不改变 server/schema/wire 和领域权限规则。
>
> 阶段：P0 ✅ 2026-07-30；P0R ✅ 2026-08-25；P1 ✅ 2026-08-26；P2 ✅ 2026-08-27；P3 ✅ 2026-08-30；P4 ⏳（P4a 首窗已提交；P4b 窗口外观获用户认可，连续动画与联网验收待补）；P5 ⏳（P5a 容器子批次获用户认可，装备/快捷槽已迁移，待外观与联网验收）；P6 ⏳（按用户要求先接入既有 HUD 的工作台编辑，P6a 与旧绘制路径收口未完成）；P7 ⬜。2026-09-13 尚未完成 P4/P5 全部验收。

## 2026-09-12 范围修订：全客户端窗口体系

**用户决议**：Inspect 的标签改为可以并存的独立功能窗口；每窗可最小化、拖动、输入 `n × m` 自适应尺寸，并通过锁定图标固定在 HUD。锻造、手搓、制作等其他功能窗口也统一接入。Inspect 是窗口工作台和布局编辑入口，窗口管理器不从属于 Inspect。工作台使用 gimage 生成的无明显聚焦点背景，支持换图；动画需连续流畅。

**修订效力**：本节及 §4.9、§7、§9.3、§10.2 替代旧的“Inspect tab-first + 每个功能独占一个 Screen”目标。保留 P0R-P3 的历史证据和已落地契约；后续直接迁移为窗口视图，不先给全部旧 Screen 换 XML 再拆第二遍。旧决议中的 Store/Intent、R2/R6 所有权、owo 唯一 adapter、server 权威与 headless 路径继续成立。本文件仍是唯一 R7 active plan，不另建一份平行窗口计划；总纲的早期 Screen/adapter 数量不作为本批验收清单。

### 接入面与实地核验

- **基准**：2026-09-12 已 fetch `origin/main`，本工作区 `client/src/main/java/**` 和本 plan 与主线对应文件无差异。`screen-inventory.tsv` 与 Java 声明对照得到 **29 个 Screen 类**：9 个 XML host、14 个 BaseOwo（12 个 CODE、2 个 XML_MODEL）、6 个 vanilla；另有一个 `TechniqueScrollReadScreen` helper。类存在不等于玩家可达，见下表的加工条目。
- **既有窗口**：`inventory/PackWindowManager.java:24` 已有容器去重、置顶和 dispose；`combat/inspect/SkillConfigPanelManager.java:13` 管理单例配置浮窗；两者都绑定各自 owo 容器，尚不具备通用最小化、尺寸输入和 HUD 固定。迁移时吸收必要行为，移除旧局部管理路径，不再长期维护三套管理器。
- **进料**：Inventory/QuickUse/SkillBar、经脉/技艺/功法，以及 Craft/Alchemy/Forge/Loot/NPC/offer 等既有 Store、session identity 和 receipt；通过现有 `UiStateSource`、`UiStateBinder` 与 immutable ViewModel 读取。
- **出料**：本地窗口状态、XML 内容视图、SVG/PNG/GUI 绘制；业务操作仍经 typed `UiIntentSink` → `ClientRequestSender`，窗口布局与背景不写入业务 Store 或 wire。
- **共享模块**：复用 `UiScreenScope`、`UiScreenController`、`UiViewport`、`UiLayoutPolicy`、`UiBootstrapRegistry`、`OwoXmlTemplateRegistry`、`ScreenOpenPolicy`、`ScreenTransitionController`、`HudRenderBackend`；新增的窗口类型均为 client-local，不另造通用 UI 框架或业务总接口。
- **历史与在途计划**：已对照 `finished_plans/plan-inspect-cleanup-v1.md`、`plan-craft-v1.md`、`plan-forge-session-entry-wiring-v1.md`、`plan-alchemy-client-v1.md`，以及 active 的 client Store lifecycle、server session、alchemy/forge/lingtian session bugfix；查过 skeleton 的 `plan-craft-close-pause-loss-v1.md`、`plan-bughunt-forge-lingtian-processing-deadpath-v1.md` 和 `reminder.md`。本次只修订 R7，不自动归档或重写其他计划的结论。
- **跨仓库与正典**：server 的 session、物品事务、距离/维度/权限检查，TypeBox/protobuf 与 agent semantic surface 均只消费现状；R7 不增加生产者。背景是 client 装饰，不定义新世界、境界或真元公式；已读 `docs/worldview.md §一 L9-L29`，不改正典、图书馆或 qi ledger。

### 全量范围清单

以下路径均相对 `client/src/main/java/com/bong/client/`。`WINDOW` 为可拖动、调尺寸、最小化和固定 HUD 的普通功能窗；`OFFER` 同样由管理器承载，但保留有效期、确认与身份约束；`SYSTEM` 参与切换/遮挡仲裁，不提供普通窗的最小化与 HUD 固定。批次编号见 §10.2。

| 现有 Screen / 面板 | 当前入口与状态依据 | 迁移目标 / 批次 |
|---|---|---|
| `inventory/InspectScreen.java` | E 键 Mixin → `InspectScreenBootstrap`；五个标签、库存、拖放与菜单集中在本类 | 工作台壳；装备、修仙、技艺、功法拆为 WINDOW，手搓入口复用 Craft 窗口；P5a/P5b |
| `inventory/component/BackpackGridPanel.java`、`PackWindowManager.java`、`component/PackContainerWindow.java` | Inspect 库存容器和套包浮窗；按 `containerId` 去重 | 背包/容器 WINDOW；统一层级与跨窗拖放；P5a |
| `combat/inspect/TechniquesTabPanel.java`、`SkillConfigPanelManager.java`、`SkillConfigFloatingWindow.java` | 功法列表打开配置；配置受 skill id 与施法状态约束 | 功法及配置 WINDOW，沿用现有约束；P5b |
| `inspect/ItemInspectScreen.java` | Inspect 长按物品；`instance_id` | 首个真实 WINDOW；物品失效不保留可执行的旧操作；P4a/P4b |
| `ui/CultivationScreen.java` | `CultivationScreenBootstrap` / `UiOpenScreens` 的 `player_overview` | 玩家概览 WINDOW；与经脉窗口共享状态读取，不强行合并不同内容；P5b |
| `identity/IdentityPanelScreen.java` | `IdentityPanelScreenBootstrap`、identity Store | 身份 WINDOW；P5b |
| `cultivation/voidaction/VoidActionScreen.java` | `VoidActionScreenBootstrap`、`VoidActionStore` | 化虚行动 WINDOW；P5b |
| `craft/CraftScreen.java` | Inspect 手搓入口 / `CraftScreenBootstrap`；`CraftScreenController` | 手搓 WINDOW；复用已迁移 controller/intent；P5c |
| `craft/WorkbenchScreen.java` | `WorkbenchScreenBootstrap`；同样消费 `CraftStore` | 工作台制作 WINDOW；保留工位上下文，不能伪造第二个 Craft 会话；P5c |
| `forge/ForgeScreen.java` | `ForgeScreenBootstrap`；station/session/blueprint/outcome Store | 锻造 WINDOW；分步操作、计时和材料入口一起迁移；P5c |
| `alchemy/AlchemyScreen.java` | 炼丹炉交互 → `AlchemyScreenBootstrap`；controller/炉坐标 | 炼丹 WINDOW；有效炉/会话约束不变；P5d |
| `combat/screen/RepairScreen.java` | Inspect 装备菜单 → `RepairScreenFactory` | 养护 WINDOW；保留物品 identity 与现有请求契约；P5d |
| `combat/screen/ForgeCarrierScreen.java` | `ForgeCarrierScreenBootstrap` / Forge 分支 | 暗器注入 WINDOW；P5d |
| `combat/screen/ZhenfaLayoutScreen.java` | `ZhenfaLayoutScreenBootstrap` | 布阵 WINDOW；目标位置仍由领域 intent 校验；P5d |
| `lingtian/LingtianActionScreen.java` | `LingtianActionScreenBootstrap`；目标地块 | 灵田 WINDOW；P5d |
| `processing/ProcessingActionScreen.java` | 有类和 transition 登记；全 client 无构造调用；工艺按钮无启动 intent | 加工视图纳入适配清单，当前标记 **业务接线待补**；不暴露可开始加工的假入口；P5d |
| `inventory/LootContainerScreen.java`、`LootContainerPanel` | 当前生产 `LootContainerScreenBootstrap` 打开 Inspect 再挂 panel；独立 Screen 不是实际入口 | 同一 loot `session_id` 的 WINDOW；沿用 controller/session adapter；P5e |
| `npc/NpcInspectScreen.java` | NPC engagement / 对话页跳转；NPC identity | NPC 详情 WINDOW；P5e |
| `npc/NpcDialogueScreen.java` | `NpcEngagementIntentHandler`；详情/交易页互跳 | NPC 对话 WINDOW；P5e |
| `npc/NpcTradeScreen.java` | 对话交易入口；NPC metadata / inventory | NPC 交易 WINDOW；P5e |
| `social/TradeOfferScreen.java` | `TradeOfferScreenBootstrap`；offer 与物品选择 | 交易 OFFER；exact instance/session/receipt 不变；P5e |
| `social/SparringInviteScreen.java` | `SparringInviteScreenBootstrap`；权威战斗快照/TTL | 切磋 OFFER；保留延迟通知与失效处理；P5e |
| `scroll/ScrollReadScreen.java` | `ScrollReadScreenBootstrap`；offer + `sessionToken` | 卷轴 WINDOW；最小化不等于阅读关闭结算；P5e |
| `spirittreasure/SpiritTreasureScreen.java` | `SpiritTreasureScreenBootstrap`；法宝状态/会话 | 法宝 WINDOW；P5e |
| `coffin/CoffinMenuScreen.java` | `CoffinEnterIntentHandler`；棺木位置 | 棺木操作 WINDOW；不是死亡裁决界面；P5e |
| `insight/InsightOfferScreen.java` | `InsightOfferScreenBootstrap`；exact `offer_id` | 顿悟 OFFER；固定/最小化不暂停有效期，不丢 settlement；P6a |
| `agentui/AgentUiScreen.java` | agent UI dispatch + `AgentUiStore`；请求 identity、TTL、专属 VFX | 受控 OFFER；raw XML 迁移受 R6/schema amendment 约束；P6a |
| `ui/DynamicXmlScreen.java` | `UiOpenScreens` legacy raw XML 分支 | 受控窗口入口；最终收口本地模板，不能以窗口化扩大 raw XML 能力；P6a |
| `combat/screen/DeathScreen.java` | `DeathScreenBootstrap`；死亡裁决 | SYSTEM；遮挡/暂停工作台输入，保留骰子与权威死亡流程；P6a |
| `combat/screen/TerminateScreen.java` | 死亡终结链路 | SYSTEM；保持高于普通窗口的优先级；P6a |
| `menu/MainMenuScreen.java` | 主菜单 Mixin / 登录入口 | SYSTEM；连接外不恢复业务窗口；P6a |

`cultivation/TechniqueScrollReadScreen.java` 是 toast/text helper，随卷轴调用方核验，不重复注册为窗口。原版暂停、聊天、容器、断线与加载界面仅作为互操作边界，不在本计划内重画。按 2026-09-13 用户补充要求，现有 MiniBody/双手/Dash/状态效果/采集等 HUD 可从工作台底部列表打开编辑窗，自定义位置、尺寸与显隐；游戏内保持原来的无边框呈现。全屏染色、边缘效果与 Toast 不作为可移动面板。新固定窗口接入同一 HUD 可见性和绘制协调，剩余 SVG 迁移及直接 overlay 收口仍归 P6b。

### 已知迁移风险与依赖

1. `CraftScreen.removed():111` 和 `WorkbenchScreen.removed():126` 目前会取消制作；`InspectScreen.removed():235` 会关闭 loot；`ScrollReadScreen.removed():207` 会结算阅读。工作台切换、最小化、固定 HUD、视图重建必须脱离这些业务 close 副作用；明确关闭窗口时才按领域契约执行，服务器失效则立即禁用旧操作。
2. `ScreenOpenPolicy` 目前以“当前唯一 Screen”为仲裁输入；`ScreenHudVisibility` 通过 Screen 类型判断可见性。二者与 `ScreenTransitionController` 必须一起适配工作台/固定窗口/系统界面，避免 HUD 重复绘制、输入穿透或关工作台误关所有窗口。
3. Craft/Forge/Alchemy 等 Store 不是任意多会话容器。多功能窗并存不等于同领域多工位同时操作；同一权威会话去重，目标切换必须由新权威状态确认，不能仅改标题就把旧材料/请求映射到新工位。
4. 加工生产入口缺失、旧 raw XML 协议退出、领域会话范围/重连 bug 均记录对应 owner；R7 可以完成视图适配与必要预览，但不能把预览当生产接线证据，也不能擅自加 server endpoint。P7 前每项须有真实接入证据或用户明确批准的范围裁剪，不能带未说明的空壳宣称全量完成。
5. `plan-craft-close-pause-loss-v1` 骨架的 client 自动 cancel 问题由 P5c 对本批路径处理；其“server 仅在 UI 打开时推进”的建议不属于此次窗口方案，也不能作为迁移前置。保留现有服务端计时与显式取消语义，不新增 UI-open wire 标记。其他计划若继续处理暂停玩法，须先对照本节重新收窄范围。

## Integration Preflight（2026-08-25 历史基线）

按 `docs/CLAUDE.md:7-21` 的防孤岛流程复核后再更新本 plan：

- **正典**：已检查 `docs/worldview.md:1-35`；本计划只做 client UI 基础设施，不新增境界、经济、世界事件或真元/灵气公式，因此不改 worldview，也不创建新的 worldview 锚点。
- **已完成计划**：已检索 `docs/finished_plans/`，重点对照 `plan-client.md`、`plan-HUD-v1.md`、`plan-alchemy-client-v1.md`、`plan-agent-ui-data-v1.md`、`plan-agent-ui-close-reason-drop-v1.md` 及相关 client/session/UI 结论；R7 只抽取现有 Store、HUD、agent UI close 和 client screen 的外部行为，不重新拥有这些 domain。
- **进行中计划**：已枚举 `docs/plan-*.md`，重点核对 `plan-refactor-client-store-lifecycle-v1.md`、`plan-refactor-wire-s2c-v1.md`、`plan-client-login-ux-v1.md` 以及 alchemy/forge/lingtian session UI bugfix plans；R7 将 Store 断线清理留给 R2、bridge/router 留给 R6，不改其 owner 文件。
- **骨架与 reminder**：已检查 `docs/plans-skeleton/plan-refactor-master-v1.md`、`docs/plans-skeleton/reminder.md:1-28` 和全部 UI/client 相关 skeleton；没有同名的 R7 child skeleton，也没有 reminder 条目要求另建 UI contract。master skeleton 保留计划族的 Wave、ownership、headless 总约束；本文件作为 R7 active child 只负责 client UI contract、adapter、Screen/HUD/keybind 和 viewport seam，拆分理由是避免把 9 条重构轨道的跨轨裁决与单轨实施细节混在同一份可消费 plan 中。
- **HUD 现状（2026-09-05）**：生产链路由 `HudRenderCallback.EVENT` → `BongHud.render` → `BongHudOrchestrator.buildCommands` → `BongHud.renderCommands(DrawContext)` 提交；`HudRenderRegistry` 当前登记 61 个 `HudRenderLayer` 与 4 个无 layer 的直接 overlay，共 65 个 surface，生产 SVG surface 为 0。幻觉 overlay 自身已有 layer，不重复计数。全 client 现有 50 个 `*HudPlanner`；`renderSurface` 仅用于测试。SVG 基础设施和显式示例预览已实现，client 尚无 NanoSVG/JNI/native 依赖或跨平台 native 打包体系。

## Pre-P0 Decisions（2026-09-02）

### HUD 统一 SVG 表现后端

**范围裁剪（2026-09-05，用户决议）**：移除 `QI_RADAR` 整项功能，包括旧 `QiDensityRadarHudPlanner` 的浓度标记、周边气息白点、TSY 假信号，SVG 阵盘资源与专用 `SvgHudFrame`/`SvgHudFramePlanner`，以及 layer、布局、沉浸模式和 registry 登记。同步删除恢复雷达主路径的骨架，不再把恢复该 HUD 作为 P6 交付物。共用 `ZoneState`、境界门、`PerceptionEdgeState`、方向条和威胁指示继续服务现有功能；server/schema/wire 不变。

**裁剪依据与影响边界**：雷达在裁剪前的唯一生产绘制接线是 `client/src/main/java/com/bong/client/BongHud.java`（`474d9d93a^`: `SvgHudFramePlanner.plan` 与 `SvgHudBackend.render` 调用），语义来源是 `client/src/main/java/com/bong/client/hud/QiDensityRadarHudPlanner.java`（已由 `474d9d93a` 删除），表现登记是 `client/src/main/java/com/bong/client/hud/HudRenderRegistry.java`（旧 `QI_RADAR`/`SvgHudFramePlanner`/`qi-radar.svg` 行，现已删除）。同一提交删除 `client/src/main/resources/assets/bong-client/svg/hud/qi-radar.svg`、`client/src/main/java/com/bong/client/hud/SvgHudFrame.java` 和 `SvgHudFramePlanner.java`，并移除 `HudRenderLayer.QI_RADAR`、`HudLayoutPreset.Widget.QI_RADAR`、沉浸模式成员及对应 TSV/契约登记；这是“完整删除”，不是停用兼容路径。

共享功能未被裁剪：`client/src/main/java/com/bong/client/network/SpiritualSenseTargetsHandler.java:21-31` 仍将 `spiritual_sense_targets` 写入 `PerceptionEdgeStateStore` 并派生 SpiritEye 状态；`client/src/main/java/com/bong/client/visual/realm_vision/PerceptionEdgeStateStore.java:12-22` 仍提供 snapshot/replace/断线清理；`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:137-139,215-223` 仍读取感知快照并调用 `ThreatIndicatorHudPlanner`；`client/src/main/java/com/bong/client/hud/ThreatIndicatorHudPlanner.java:26-58` 继续生成威胁边缘/渡劫反馈。`BongClient.java:92-93` 仍注册 HUD 回调与 SVG reload listener，现有 SVG 登记与后端边界仍由 registry/TSV 契约测试保护。

**计划映射**：上述删除证据属于 P4 的“首个真实 HUD 选择与删除门”验收；共享感知/威胁链路属于 P4/P6 的保留动态 binding 与全量 inventory 对拍，P6 不恢复 QI_RADAR，而是继续迁移当前 registry 中保留的 layer/overlay。验收必须同时满足 `HudRenderRegistryTest` 的 layer/TSV 对拍、`ThreatIndicatorHudPlannerTest` 的共享感知契约和 Java 17 全量门禁；因此雷达裁剪不会被误判为删除 spiritual-sense/威胁功能。

**裁剪时的 SVG 进度（2026-09-05 历史）**：当时保留 parser、tessellator、GUI emitter、资源缓存/reload 与环境变量控制的 `example.svg`，尚无生产 SVG surface。**2026-09-12 复核**：`example.svg` 已退出；`HudRenderRegistry` 已登记 QUICK_BAR、CAST_BAR、GATHERING、STATUS_EFFECTS、MOVEMENT_HUD 等生产 SVG binding，配合 PNG/GUI 例外使用。首个真实 HUD 接线已发生，剩余全量迁移/旧 primitive 删除仍待逐项核验；不得恢复示例或把已有成果重做。

**测试调整记录**：删除退役功能专用的 `QiDensityRadarHudPlannerTest`、`SvgHudFramePlannerTest` 和 orchestrator 的雷达停用断言；删除雷达资源登记的重复断言与易漂移的 surface/三角形数量断言。完整登记仍由 `HudRenderRegistryTest` 对拍枚举、直接 overlay 和 TSV；布局、沉浸渐隐/Alt 预览测试改用保留的方向条或人体 HUD；资源加载与缓存测试合并到 `example.svg`，继续覆盖失败缓存和资源重载。测试范围随功能裁剪收窄，不保留失去生产消费者的旧契约。

**决议**：HUD 的几何表现统一由 runtime SVG 描述，`SvgParser` 只解析受限 SVG 子集，自有 `SvgTessellator` 生成不可变 `SvgMesh`，最终严格通过 Minecraft GUI API 提交。HUD Store、semantic planner、C2S intent、S2C payload 和 `HudRenderLayer` 不依赖 parser/native；动态文字与物品图标继续使用 Minecraft GUI 的文字/item API，作为明确例外。

**当前实现边界**：`NanoSvgParser` 是 `SvgParser` 的 Java compatibility slice，使用安全 StAX 解析受限 V1 子集；它不是 JNI/native NanoSVG loader，也不代表 native loader 已完成。若未来接入 NanoSVG native provider，只能替换 `SvgParser` 实现，并须单独交付跨平台打包、加载失败诊断和对应门禁。

**代码探索证据**：

- 注册入口是 `client/src/main/java/com/bong/client/BongClient.java:92-94`，由 `BongHudRenderer` 组合 `HudRenderBackend`，并注册 SVG 资源 reload listener；两者均位于 UI bootstrap 之前。
- 生产入口是 `client/src/main/java/com/bong/client/BongHud.java:70-97`；命令收集在 `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:124-145`，旧 primitive `DrawContext` 分支在 `BongHud.java:153-253`。
- semantic planner 主入口是 `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:124-145`；layer 枚举和顺序由 `client/src/main/java/com/bong/client/hud/HudRenderLayer.java:3-80` 固定。
- 资源生命周期由 `client/src/main/java/com/bong/client/hud/svg/SvgHudResourceReloadListener.java:10-26` 门禁，`SvgHudBackend.invalidateAssets()` 清除成功与失败缓存；`renderSurface` 仅出现在测试 harness。当前 `client/build.gradle:45-51` 没有 NanoSVG/JNI/native 依赖。P4 当前只固定 parser ABI、受限 Java compatibility parser、资源校验和 GUI 提交边界；native loader 属于后续独立交付，不得把本 slice 记为 native 完成。

**计划章节映射**：上述入口与边界证据对应本计划 §HUD SVG 表现后端契约、§6 P4 阶段总览、§7 P4 测试抓手和 §9.2 决议索引；实现顺序与删除门以 §HUD SVG 表现后端契约第 4 节为准。

**边界与交付顺序**：P4 的 SVG 基础设施和真实 layer 沿用已有成果；P5 聚焦功能窗口迁移；剩余 layer/直接 overlay 与旧 primitive 路径收口统一归 P6b，以当前 registry 为准，不锁死历史 61 层数量。`RenderLayer.getGui()` 在 1.20.1 中使用 `POSITION_COLOR + QUADS`，每个 tessellated triangle 必须编码为退化 quad `(a,c,b,b)`；禁止独立 framebuffer、实体渲染 layer、浏览器/Canvas/NanoVG、直接 OpenGL program。长期 fixture 使用 `ui-svg-hud-contract.tsv` 与 `ui-svg-hud-inventory.tsv`。

## 0. 改写目的与不可变范围

旧版 R7 以 `BongScreenBase extends BaseOwoScreen` 为公共基类，能改善当前 owo 屏幕，但不能作为未来 UI 库迁移边界。本版将 R7 从“owo UI 重构”改为“UI contract-first + adapter migration”计划：

1. **库无关核心**：状态读取、订阅生命周期、列表 diff/reconcile、intent dispatch、Screen open policy、bootstrap module contract 不得 import owo、Fabric widget 或 vanilla drawable。
2. **单一生产适配路径**：功能窗口、Inspect 工作台与专用系统界面统一使用本地 owo XML；现有 29 个 Screen 类按范围清单合并、迁移或收口，不要求最终仍有 29 个独立 Screen。删除后的历史条目只留在盘点，不建立 vanilla 兼容宿主。
3. **协议不变**：不修改 server、TypeBox shape、protobuf envelope、Redis key、`ClientRequestProtocol` 编码或 `bong:server_data`/`bong:client_request` channel。现有 sender/handler 行为只通过 adapter 复用。
4. **所有权不变**：R2 仍独占 Store 断线清理，R6 仍独占网络 receiver/bridge/router，R9 仍独占 cast domain；R7 只消费它们冻结的外部契约。HUD SVG 只消费已有 semantic snapshot，不改变 Store、wire、schema 或 server 数值。
5. **分批迁移**：沿用已完成的 contract/fake/XML/reference slices，先落公共窗口管理器，再按功能域迁移；每批同时处理内容布局、状态/intent、入口和生命周期，不把全量 Screen、Store 与 Handler 混成一次提交。

## 1. 历史基线证据（2026-08-24 复核）

本节数量与结构用于保留 P0R-P3 审计上下文；2026-09-12 当前范围以开头清单为准。

- client production Java 文件约 **1022** 个；当前真实 Screen **28** 个：22 个 owo host、6 个 vanilla host。`TechniqueScrollReadScreen` 是 helper。逐文件基线：`client/src/test/resources/bong/ui/screen-inventory.tsv`。
- P0R 冻结基线为 **29 个 production Screen**；旧的 `DiffListWidget<T, K, C extends Component>`、92 个 `Sizing.fill(100)` 站点和 Screen-local listener/unsubscriber 语义只作为迁移对拍，不构成 library-neutral core；`ClientThreadMarshal` 只冻结纯 helper API。
- 当前 inventory 中有 12 个 owo `CODE` 实现、8 个现有 `OWO_XML_TEMPLATE`、2 个 `XML_MODEL` 运行时入口和 6 个 vanilla Screen；6 个 vanilla Screen 全部列为 owo XML 重写对象。旧的 `BongScreenBase<R extends ParentComponent>` 和 `DiffListWidget<T,K,C extends Component>` 没有生产调用者，已从代码库移除；后续 neutral contract 不再为它们保留兼容壳。
- `InspectScreen.java` 约 4647 行，同时持有 tab 组合、Store snapshot/listener intake、drag/drop、context menu、tooltip、hotbar 和 overlay arbitration；它必须保持唯一 screen-level intake，tab panel 不得重复订阅同一 Store。
- production `*Store.java` 共 **109** 个（R2 的 108 个业务 Store 加 lifecycle infrastructure 口径）；R2 的 `SessionScopedStore` 只有 `clearOnDisconnect()`，不是 UI subscription contract。现有 listener/remove-listener 形态不统一，必须通过 R7 read adapter 归一，不能假定所有 Store 已有同一接口。
- `client/network` Handler 约 **80** 个；主路径为 `ProtoServerDataBridge -> ServerDataRouter -> domain Handler -> Store/HUD/Screen`。Handler 负责解析和写入 domain state，UI 不得反向依赖 Handler。
- `ClientRequestSender` 现有大量静态 `sendXxx` 入口（审计口径 117 个 public sending entry），同时存在 `void`、本地 transport `boolean`、tracked `request_id` 和 generic JSON 入口。UI 不能把该类当库无关 API；R7 只在其上提供 typed intent adapter。
- `BongClient.onInitializeClient()` 以显式顺序注册 network、HUD、keybind、Screen bootstrap、render/audio 等模块；R7 只收编 UI/HUD/keybind 子集，不改变 `BongNetworkHandler.register()` 的 channel 注册顺序。
- owo `Sizing.fill(100).inflate(space, ...)` 返回完整 `space`，不是同轴兄弟的剩余空间。现有 92 个 token（87 个 executable）以及 15 个 executable `clearChildren()` 站点已由 `fill100-inventory.tsv` 锁定，不能机械全量替换。

## 2. 目标架构与稳定数据流

目标态只允许以下依赖方向：

```text
server/protobuf/JSON
  -> BongNetworkHandler / ProtoServerDataBridge / ServerDataRouter (R6)
  -> domain Handler
  -> domain Store + immutable Snapshot/ViewModel (R2 owns lifecycle)
  -> semantic UiSurfaceProjection / UiStateSource / UiViewModelAdapter (R7)
  -> UiScreenController + UiWindowManager (client-local identity/scope/layout)
  -> local XML content view + owo window adapter
  -> Inspect workspace OR pinned HUD presentation (同一窗口状态，不重复订阅/绘制)

system terminal / main menu
  -> ScreenOpenPolicy / transition coordination
  -> OwoXmlScreenHost (专用系统界面)

UI input
  -> typed UiIntentSink (R7)
  -> existing ClientRequestSender / ClientRequestProtocol
  -> bong:client_request
```

禁止反向依赖：

- Screen、widget、HUD renderer 不解析 `ServerDataEnvelope`、protobuf 或 JSON。
- Screen、widget、HUD renderer 不直接调用 `*ServerDataHandler`、`ServerDataRouter` 或 `ProtoServerDataBridge`。
- UI adapter 不直接操作 Store 的静态业务字段；只能消费 `UiStateSource.snapshot()` 和订阅信号。
- UI 不把“本地 transport 已接受”当作 server gameplay 成功；权威结果仍来自 S2C state/receipt。
- network Handler 不直接构造 library-specific widget；需要打开 UI 时只提交 domain offer/state，由 bootstrap/transition owner 决策。
- server/agent 只拥有语义 surface、immutable view data 和 allowed action；client 只拥有本地 owo XML 模板、布局和交互呈现。任何 HTML/CSS/JS/DOM/像素坐标不得跨这条边界。
- bot/headless client 消费同一 `surface_id`、`template_id`、`action_id`、参数校验和 authoritative receipt；它跳过模板、渲染和物理输入，但不能走另一套“测试专用”业务接口。

### 2.1 UI backend 决议

- **唯一生产实现**：`owo + XML`。工作台、普通窗口内容与系统界面的组件树、布局和静态文案由本地 XML 模板描述；Java 保留 controller、窗口策略、状态绑定、intent wiring 和必要渲染桥接。固定 HUD 的窗口仍使用同一 adapter，不另开 HTML/原生窗口或第二套业务视图模型。
- **vanilla 全量退出**：当前 6 个 vanilla Screen 不建立兼容宿主，统一重写为 owo XML。`VanillaScreenHost`、`net.minecraft.client.gui.widget.*`、`addDrawableChild` 和 Screen-local 手工绘制不能进入重构后的生产 UI。
- **owo 代码构建全量退出**：当前 12 个 CODE、6 个 vanilla 与 2 个 XML_MODEL 按开头清单直接迁为窗口内容/受控宿主；已有 9 个 XML host 复用内容与 controller。两个 raw XML 入口 `AgentUiScreen` / `DynamicXmlScreen` 最终收口到本地白名单模板，其 wire 退出依赖保持不变。禁止先迁一套独立 Screen 再为窗口化复制一套。
- **语义与模板分离**：server/agent 仍只交换 semantic surface；client 通过 `template_id` 选择本地 owo XML 模板。Agent UI 的 server-supplied raw XML 仅保留为 legacy compatibility input，不作为新 Screen 模板来源。
- **撤回第三方宿主路线**：本计划不引入 MCEF、JCEF、CinemaMod、原生窗口、HTML/CSS/JS 页面或额外进程；未来若需要第二种生产宿主，另立计划并重新做 compatibility gate。
- **稳定边界**：library-neutral contract 不持有 owo host；窗口 adapter、工作台及系统 `OwoXmlScreenHost` 是当前唯一实现。backend capability 只登记 `OWO`，`VANILLA` 仅留在迁移前统计。

## 3. 接入面与跨轨所有权

### 3.1 进料

- **状态**：R2 管理的 session Store、persistent config Store、HUD state Store 的 immutable snapshot；R7 通过 read adapter 暴露给 UI。
- **网络**：R6 的 `ProtoServerDataBridge`、`ServerDataRouter`、`ServerDataHandler` 已完成 client-thread receive boundary；R7 不重新 marshal 网络 receiver。
- **输入**：现有 keybinding、Screen widget callback、HUD/overlay gesture；迁移后均由 owo XML host 转换成 typed intent。
- **启动**：`BongClient.onInitializeClient()` 的 UI/HUD/keybind bootstrap call set；网络、render、audio、debug module 不被 R7 统一吞并。
- **worldview / qi_physics**：纯 client 基础设施，不新增玩法、境界、经济或真元/灵气公式；不调用、不修改 `qi_physics`，不改变任何守恒 ledger。SVG 只消费已有 semantic HUD snapshot，不改变 Store、wire、schema 或 server 数值。
- **共享类型**：HUD 表现边界冻结为 `HudRenderBackend`、`SvgParser`、`SvgHudAssetRegistry`、`NanoSvgParser`、`SvgDocument`、`SvgMesh`、`SvgTessellator`、`MinecraftGuiMeshEmitter`；签名、线程约束和失败语义由 `ui-svg-hud-contract.tsv` 固定。

### 3.2 出料

- UI adapter 的渲染树、HUD render command、Screen transition decision。
- typed `UiIntentSink` 到既有 `ClientRequestSender`；wire payload 和 server ACK 语义不改变。
- subscription/cleanup 的 library-neutral lifecycle evidence。

### 3.3 跨仓库契约（只消费，不改形状）

- **server**：R7 只消费 server 现有 `ServerDataV1` producer、`server/src/network/client_request_handler.rs` 的 C2S request handling 和各 domain authoritative receipt/state；不新增 server endpoint。
- **schema/agent**：TypeBox `ClientRequestV1` / `ServerDataV1` 及其 JSON samples 是现有 wire source of truth；R7 不新增 union member、字段或 Redis key，不把 UI-only ViewModel 写回 schema。
- **client**：`ClientRequestProtocol` → `ClientRequestSender` 是唯一既有 C2S transport；`ProtoServerDataBridge` → `ServerDataRouter` → `ServerDataHandler` 是唯一 S2C receive path；R7 只在两端外层加 adapter/source gate。
- **验收**：UI intent 的 encoded payload 与现有 C2S sender tests 对拍，Store/ViewModel 的 authoritative update 与现有 S2C handler tests 对拍；任何 adapter replacement 都不能要求 server/schema/agent 改形状。

### 3.4 所有权矩阵

| 范围 | owner | R7 规则 |
|---|---|---|
| Store 业务字段、S2C hydration、断线清理 | R2 / domain owner | R7 不改字段语义；只写 read adapter 和 UI view-model mapping。 |
| `BongNetworkHandler.register()`、receiver、bridge、router | R6 | R7 不改 channel registration；Insight `offer_id` 只按窄接缝协调。 |
| `ClientRequestProtocol` 编码与 server request handler | 既有 network/C2S owner | R7 不改编码；intent adapter 复用 sender。 |
| `client/ui/contract/**`、`client/ui/window/**`、UI adapters、Screen/HUD/keybind 结构 | R7 | 库无关窗口策略、作用域与 owo XML 实现；窗口偏好 I/O 位于 adapter/本地配置层，不进入纯窗口策略；不引入 vanilla/第三方宿主。 |
| cast domain reducer/store/AV semantics | R9 | R7 只消费 view-model/intent contract。 |
| `BongClient` UI/HUD/keybind bootstrap call set | R7 | 只迁 UI 子集到显式 registry，保留 network/render/audio order。 |
| server、agent、worldgen、worldview、qi ledger | 其他 owner | R7 不触碰。 |

## 4. 库无关公共契约（P0 必须冻结）

库无关类型按职责放在 `client/src/main/java/com/bong/client/ui/{contract,state,intent,bootstrap}/`；这些包的源码不得出现 owo、`BaseOwoScreen`、`FlowLayout`、`net.minecraft.client.gui.widget` 或具体 UI library import。adapter 只允许出现在 `client/ui/adapter/owo/`，并且所有 production Screen 模板必须通过 owo `UIModel` XML 加载。

### 4.1 `UiStateSource<S>`、`UiSubscription` 与 source mode

```java
public interface UiStateSource<S> {
    S snapshot();
    UiSubscription subscribe(Consumer<? super S> listener);
}

public interface UiSubscription extends AutoCloseable {
    @Override void close();
    boolean isClosed();
}
```

source mode 是 adapter inventory 的显式字段，而不是隐藏实现：`PUSH` 由 Store listener 驱动，`PULL_ON_OPEN` 只在 Screen open 读取一次，`PULL_ON_TICK` 只能由已登记的 client tick owner 驱动。迁移 UI 库不能改变 source mode。

冻结规则：

- `snapshot()` 是当前唯一可读状态；不得暴露可变内部集合。
- `subscribe()` 只承诺后续变化信号，不承诺立即回调；绑定器先读 snapshot，再订阅，避免重复首帧。
- `close()` exactly-once、幂等；关闭后不得再收到回调。listener 异常不得破坏 Store 状态更新，异常传播策略由 adapter 测试固定。
- 注册、回调、关闭均在 client thread 完成；R6 receive-boundary 负责网络线程切换，R7 不在 network 文件叠加第二层 marshal。
- 该接口不等于 `SessionScopedStore`；断线清理仍由 R2 registry 调用 `clearOnDisconnect()`。

R7 提供 `StoreUiStateSource<S>` wrapper，把现有静态 Store 的 `snapshot`/listener/remove-listener 适配为上述形状。P0R 对每个被 UI 消费的 source 登记 `PUSH`、`PULL_ON_OPEN` 或 `PULL_ON_TICK` 模式；没有 listener 的 Store 只能使用显式 client-thread invalidation signal 或登记的本地 tick refresh，不在 UI 层轮询网络或直接读内部字段。

### 4.2 `UiScreenScope` 与生命周期

```java
public interface UiScreenScope {
    void onOpen();
    void addCleanup(Runnable cleanup);
    void onTick(long nowMs);
    void close();
    boolean isClosed();
}
```

实现契约：

- cleanup 按登记顺序确定、关闭时 LIFO、exactly-once；`close()` 先标记 closed，再执行 cleanup。
- cleanup、business `onRemoved`、library host teardown 任一步抛异常时，后续阶段仍执行；首个异常为 primary，后续按顺序 `addSuppressed`。
- `close()` 只关闭该 owner 的订阅、tick、input handle 和 pending UI callback；绝不调用 R2 `clearOnDisconnect()`。P4a 起业务窗口复用此 scope，其生命周期由窗口管理器持有，不再与工作台 `Screen.removed()` 等同；宿主输入资源使用独立 scope。
- late refresh、late intent、重复 `removed()` 均 fail-closed/no-op，不重新挂载已关闭 Screen。

### 4.3 `UiScreenController` 与 adapters

库无关 controller 只依赖 immutable `ViewModel`、`UiStateSource` 和一个按领域参数化的 typed `UiIntentSink`；controller 不接收裸 `UiIntent`，adapter 只能通过该 sink 发送允许的动作：

```java
public interface UiIntent {}

public interface UiScreenController<M, I extends UiIntent> {
    M viewModel();
    UiIntentSink<I> intentSink();
    void onOpen(UiScreenScope scope);
    void onClose();
}
```

`UiScreenController<M, I>` 的 `I` 必须与 `UiIntentSink<I>` 一致；adapter 不得把 `UiIntent` 向下转型或绕过 sink 调用 sender。需要多个动作域的 Screen 组合多个窄 sink/controller，而不是退回一个 117 方法的总接口。

具体 UI library 只实现 host/adapter，不定义业务规则：

- `client/ui/adapter/owo/OwoXmlScreenHost`：沿用现有系统 Screen/工作台宿主；新增窗口内容 adapter 将同一 controller/view-model 映射到 XML component tree，不把完整 Screen 嵌入另一个 Screen。
- 不建立 `client/ui/adapter/vanilla/VanillaScreenHost`。vanilla `Screen`、`Drawable`、`ClickableWidget` 仅作为迁移前源码审计对象，不能成为重构后的生产依赖。
- `BongScreenBase` 已删除；P2 若确实需要 owo host 基类，必须在 adapter 包内以真实生产切片为理由重新引入，不能恢复一个无调用者的根目录 legacy 类型。
- `DynamicXmlScreen` 继续属于 owo adapter；模板白名单和 XML 安全策略不迁入 library-neutral contract。

adapter 不等于 library-neutral contract：owo component 与 XML 模板细节锁在 owo adapter 包。若未来要支持第二种生产宿主，必须另立 plan，不得把具体组件树偷渡成公共 wire schema。

### 4.4 `UiListReconciler<T,K>`

列表核心只处理 key、顺序、patch 和 detached rebuild，不持有 owo component：

- equal key + equal order：只 patch，不替换 mounted row。
- reorder/add/remove：先 detached 创建完整 replacement，全部成功后整体 swap；失败保留旧 committed state。
- null list/item/key、duplicate key：mutation 前 fail-fast。
- patch 异常不回滚外部 row mutation，但内部 committed sequence 保持上一版本，下一次从第一行完整重试；patcher 必须幂等。
- 组件 identity、selection、callback、scroll 保留语义由 adapter 验证；scroll offset 不通过不存在的 owo 0.11.2 API 伪造。

`OwoDiffListWidget` 只实现 owo renderer bridge；不建立 `VanillaDiffListWidget`。原 `DiffListWidget<T,K,C extends Component>` 已删除，不再保留根目录兼容实现。

### 4.5 `UiIntentSink` 与发送语义

不新建一个包含 117 个方法的巨型接口。按领域定义小型 typed sink，例如：

- `InventoryIntentSink`：move/equip/discard/pickup，携带 immutable `instance_id`/location。
- `CraftIntentSink`：start/cancel/quantity，复用 `CraftStore` 的 accepted identity。
- `AlchemyIntentSink`、`ForgeIntentSink`、`InsightIntentSink`、`SocialIntentSink`：按现有 sender/protocol 语义分域。

最小公共形状为：

```java
public interface UiIntentSink<I extends UiIntent> {
    UiIntentResult dispatch(I intent);
}
```

`UiIntentResult` 只表达本地 transport，不表达 server 业务结果：

```java
public record UiIntentResult(Kind kind, String reason, String requestId) {
    public enum Kind { LOCAL_ACCEPTED, LOCAL_REJECTED, LOCAL_ERROR }
}
```

`reason` 和 `requestId` 可为空的组合由各 domain sink contract pin；`LOCAL_ACCEPTED` 不得被命名为 accepted-by-server。

每个 sink 的实现只负责把 intent 映射到既有 `ClientRequestSender`；返回值统一表达**本地 transport**结果：

```text
LOCAL_ACCEPTED { optional request_id }
LOCAL_REJECTED { reason }
LOCAL_ERROR { reason }
```

不得把 `LOCAL_ACCEPTED` 命名为 server accepted；server 成功/拒绝仍由 S2C authoritative state/receipt 回写 Store。tracked request 必须保留 request id，void sender 不被伪装成有 ACK。

### 4.6 `UiBootstrapModule` 与 `UiBootstrapRegistry`

```java
public interface UiRuntime {}

public interface UiBootstrapModule {
    String id();
    Set<String> dependencies();
    void register(UiRuntime runtime);
}
```

规则：

- registry 显式静态列出 UI/HUD/keybind module；禁止 reflection/annotation discovery/构造器自注册。
- `id` 全局唯一、依赖必须存在且无环；拓扑排序结果可观察并由测试 pin。
- `registerAll()` exactly-once；重复调用不重复注册 Fabric callback、KeyBinding、HudRenderCallback 或 tick。
- `BongNetworkHandler.register()` 在 registry 之前完成；`ScreenTransitionController` 先于 Screen bootstrap；HUD callback 只保留现有唯一 owner。
- 只收编 `Screen/HUD/keybind` 注册。render/audio/debug/Iris/资源包模块继续由 `BongClient` 原顺序显式注册，避免无关范围膨胀。

### 4.7 语义 UI surface 与前后端完全分离

R7 冻结的是**消费端接缝**，不是让 server/agent 知道某个 UI 库的组件树。目标语义 surface 只允许携带以下与渲染库无关的数据：

- `surface_id`、`template_id`、`session_id`、单调 `revision` 和有效期/关闭原因；
- immutable、带版本的 view data；集合必须有稳定 identity，不能依赖数组位置；
- `allowed_actions`：稳定 `action_id`、参数 schema、当前可用性和机器可读拒绝原因；
- 可选的本地化 message key、severity、icon id 等有限 presentation hint，不携带布局坐标。

server/agent **不得**下发 owo XML、HTML、CSS、JavaScript、任意 URL、DOM id 或像素坐标。client 用 `template_id` 在本地白名单中选择 owo XML 模板；同一 view data 和 action registry 只由 owo adapter 消费。现有 `UiOpen.xml`、`agent_ui_request` 的 raw XML 是迁移阻塞项，只能在兼容期保留，不能成为新 Screen 的生产输入。

本次 R7 不偷偷新增 wire union：P0R 先冻结 `UiSurfaceProjection`/action registry 的消费接缝和 source gate；若现有 `ServerDataV1` 无法表达某个语义 surface，由 R6/schema 以及对应 server/agent owner 另立 amendment，按 TypeBox source、generated mirror 和 atomic activation 规则接入。新 wire 未合入前，R7 只能从现有 authoritative payload 构造同形 projection 或使用 test fixture，不能把本地 ViewModel 伪装成服务端事实。

### 4.8 `UiViewport` 与响应式布局契约

当前代码证据表明这条边界必须集中收口：`MixinMouse.java:100-116` 和 `BotanyHudBootstrap.java:58-69` 都把 `MinecraftClient.mouse` 的 physical window 坐标按 `getScaledWidth()/getWidth()`、`getScaledHeight()/getHeight()` 换算为 GUI logical 坐标；`BongHud.java:131-143`、`:243-251`、`:528-541` 则统一从 `getWindow().getScaledWidth/Height()` 生成 HUD 输入、绘制覆盖层和测量文字。相反，`AlchemyScreen.java:664-710`、`ForgeScreen.java:365-375`、`InspectScreen.java:2255-2375` 仍在 Screen 内直接进行 `mouseX/mouseY` 命中和 grid 换算，正是迁移期间要被 `UiViewport`/layout policy 覆盖的耦合点。

当前依赖事实也必须写明：`client/build.gradle:45-51` 只有 Minecraft、Fabric、owo 依赖，`client/gradle.properties:1-19` 没有第三方 browser provider；因此第三方宿主不是已有 production API，也不进入 R7 的 adapter seam。

布局必须由可测试的纯策略计算，不在 controller、Store 或业务 intent 中写死屏幕像素。公共输入至少区分 Minecraft 窗口/帧缓冲 physical px 和 Minecraft GUI logical px，并显式记录 `gui_scale` 与 window scale factor。

`UiViewport` 至少包含 `logical_width`、`logical_height`、`framebuffer_width`、`framebuffer_height`、`gui_scale`、`device_pixel_ratio` 和 safe insets；`UiLayoutPolicy.measure(UiViewport, ViewModel)` 输出确定性的 `UiLayoutSnapshot`（layout mode、content rect、控件 bounds、focus order、overflow policy 和 hit regions）。同一输入必须得到同一快照，供 Java adapter 和 headless geometry test 共同消费。

冻结以下不变量：

- 业务层只使用逻辑坐标和 design tokens；不得把窗口 physical px 或某一 GUI scale 当作业务尺寸。
- GUI logical 尺寸优先使用 Minecraft 提供的 scaled viewport；鼠标、键盘焦点和 Screen 输入必须使用同一套正/逆变换，禁止在各 Screen 内散落手工比例换算。
- 字体 token 不随 viewport 宽度连续缩放；空间不足时只能换 `COMPACT`/`REGULAR`/`WIDE` 布局、换行、堆叠、滚动或折叠次要操作。主要操作不能被裁切、遮挡或变成不可点击的隐形区域。
- 每个窗口内部的 interactive hit region 必须在其可见内容区内；窗口间允许玩家主动叠放，由统一 z-order/遮挡规则确定唯一命中，不要求窗口之间绝不重叠。文本按字体 metrics 换行；resize 只更新布局或重建对应 XML 内容视图，不得复制 controller/订阅。
- P0R 必须根据实际 client window 限制冻结 `MIN_SUPPORTED_VIEWPORT`（默认验收下限为 `320x240`）。低于下限仍需进入 fail-safe compact/scroll 模式并保留关闭路径，但不得把“不支持”尺寸的绿灯算作完整布局支持。

固定回归矩阵至少覆盖：`320x240`、`400x240`、`640x360`、`854x480`、`1000x700`、`1024x768`、`1280x720`、`1365x768`、`1920x1080`、`2560x1080`、`3440x1440`、`1080x1920`；每个尺寸至少跑 GUI scale `1/2/3/4`，window scale `1.0/1.25/1.5/2.0`，并额外覆盖 odd aspect 和 resize 中间态。矩阵是 geometry/input contract，不要求 bot 启动真实渲染器。

### 4.9 全客户端窗口契约（2026-09-12）

以下为窗口目标契约，具体已实现部分及待验项见 P4a/P4b 工作记录。优先复用已有 contract，不为每个操作新增抽象层。

#### Owner、身份与状态读取

- `client/ui/window/UiWindowManager` 管理窗口注册、实例去重、布局、最小化、焦点、置顶、HUD 固定和关闭；只依赖纯 Java 数据、既有 controller/scope 与窄回调，不 import Minecraft/owo/网络。
- `UiWindowDefinition` 登记稳定 `window_type`、本地模板、最小内容尺寸、WINDOW/OFFER/SYSTEM 能力与领域工厂；登记由既有 `UiBootstrapRegistry` 组合，不引入反射/插件发现。窗口 key 由类型、当前连接 generation 和领域已有 identity 组成；个人页为单例，容器用 container id，物品用 instance id，offer/session 用既有 id/token。现有 wire 不支持多会话时，同领域最多保留一个可操作实例。
- 状态读取由窗口 runtime 统一组合既有 `UiStateSource`，同一 Store 的多个窗口消费者共享一次上游 intake；各内容视图只接 immutable ViewModel 与 typed callbacks。最小化、固定 HUD、布局变化不得新增上游 listener。保留的窗口仍接收权威失效信号；所有消费者关闭后释放对应读取资源。
- 每个窗口只有一个业务 controller/scope；工作台和 HUD 是它的两个呈现位置。任一帧仅在一个位置绘制该窗口。XML 内容视图可按 adapter 需要卸载/重建，但不得重放业务 open/close 或创建第二份 Store 订阅。最小化不销毁选择、滚动、输入草稿和 pending receipt。

#### 显示状态与业务生命周期

| 操作 | 呈现变化 | 业务效果 |
|---|---|---|
| 打开同 key | 恢复并置顶现有窗 | 不创建重复会话、controller 或订阅 |
| 最小化 / 恢复 | 收入工作台底部恢复条 / 展开；最小化时 HUD 也不展开该窗 | 不发送 cancel/close，不暂停业务时间；`pinned` 偏好保留 |
| 锁定图标开 / 关 | 设置 / 取消 `pinned`；不是禁止拖动 | 不调用业务 intent；非最小化的固定窗在游戏 HUD 显示 |
| E 或 Esc 离开工作台 | 背景和编辑控件消失；仅固定且展开的窗继续显示 | 保留本次连接内其他窗口状态，不取消制作/阅读/搜刮 |
| 点击窗口关闭图标 | 关闭此窗口，移出恢复条和 HUD | 领域 close policy 执行已有 cancel/settlement，scope exactly-once 清理 |
| 服务端 session/offer/物品失效 | 即时撤销旧 allowed actions；按领域收起或展示终态 | 不重发 start/cancel，不因最小化漏掉失效，不将旧响应作用到新 key |
| 断线 / 世界切换 | 清理全部运行态窗口、捕获与待处理操作 | 与 R2 cleanup 协调，不向下一连接发送旧请求；仅保留本地布局偏好 |

SYSTEM 界面不能被最小化或固定绕过。死亡/终焉抢占会遮挡固定窗并取消输入捕获，领域失效仍按真实状态处理；返回正常游戏后只恢复仍有效的个人窗口，不凭保存的 session id 重建业务会话。原版暂停/聊天/容器等 Screen 的遮挡与输入策略由同一协调层决定。

#### 输入、拖放与窗口尺寸

- 所有输入由管理器按顶层弹层 → 最上窗口 → 工作台处理，鼠标 capture 从按下持续到释放/取消；移动/关闭窗口、失焦、断线和系统抢占都必须能取消 capture。被窗口遮住的控件不能命中，菜单和 tooltip 不另建一套全局层级。
- 游戏 HUD 只展示固定窗，不获取鼠标或文本焦点；玩家打开 Inspect 工作台后才能拖动、输入尺寸、编辑或执行窗口按钮。切换时清理按键积压，不回放旧 hotkey，不影响正常视角/移动。工艺节拍等领域输入只能由当前有效且允许输入的窗口 owner 消费。
- Esc 先取消当前尺寸草稿、拖拽或顶层弹层，再退出工作台；文本输入获得焦点时 E 作为文本处理，不能触发关窗。最小化恢复条容纳不下时滚动，保持所有恢复入口可达，不压缩标题字号。
- 跨窗拖放携带既有 `instance_id`、来源容器/槽位和目的地 intent；焦点变化不改变物品 identity。目标只接受本领域已支持的操作，不能为视觉拖放发明新的 wire。拖动中容器消失、会话过期或被系统抢占时取消本地拖拽，禁止向新目标提交旧请求。
- 标题栏提供尺寸、最小化、固定、关闭图标与 tooltip；宽/高数值输入平时隐藏，点击尺寸图标后在标题栏下方展开，形式 `n × m`。单位为 GUI logical px，含标题栏的目标外框尺寸；Enter/确认提交并收起，Esc 回退草稿并收起，再点尺寸图标可收起；最小化或切换 HUD 时收起并释放输入焦点。拒绝空值、非有限/非正/溢出输入，按最小尺寸与当前可用 viewport 限制显示有效结果。
- 依据窗口自己的**内容区宽高**选择 compact/regular/wide 模板，而不是拿整屏宽度决定每个窗布局。空间不足优先换行、堆叠、滚动；图标/人体保持宽高比，文字不按窗口宽度连续缩小。极小 viewport 下允许内容滚动，但标题栏和关闭/恢复入口必须可达。
- 位置和期望尺寸与当帧动画矩形分开；屏幕/GUI scale 改变时将有效窗口钳回可见区，不永久覆写玩家期望尺寸。拖动直接跟随指针，不叠加有延迟的平滑滤波；命中测试与当帧可见 transform 一致。

#### 动画、背景与偏好

- `ui/adapter/owo/` 内统一窗口 frame 与 XML 内容桥接。打开/恢复以 180 ms `easeOutCubic` 从恢复条锚点或 0.98 比例淡入；最小化以 160 ms `easeInCubic` 收回恢复条；尺寸提交以 180 ms `easeInOutCubic` 过渡；固定位置切换以 180 ms 位移/透明度过渡。时间使用单调时钟；新操作从当前插值状态继续，不能等待旧动画结束。上述是首轮调参基线，人工预览后可调整，不做精确时长单测。
- 复用 `ScreenTransitionController` 的必要时钟/曲线行为，普通窗口转场改由窗口 adapter 驱动；避免旧全屏转场与新窗动画叠加。降低动态效果设置可改为即时布局/短淡入；关闭与权威失效立即停用动作，不等待动画完成。
- 工作台背景使用 `assets/bong-client/textures/gui/workspace/` 下经人工选定的 PNG：首批空旷宇宙与疏朗地形两类，无人物、居中主体、文字或高亮焦点。图像等比 cover；超宽/竖屏检查裁切与文字对比。背景只在工作台出现，HUD 回到真实游戏画面；不新增世界泡等装饰动画范围。
- 生图沿用 `gen-image` skill 的 `scripts/images/gen.py`、现有本地 env 和 cliproxy/openai 路由；这是完整底图，使用现有 `--style none`，不加 `--transparent`，不套 item/particle/HUD 透明边框前缀，不做算法去底。原图/prompt 留 `local_images/`，仅确认后的 runtime PNG 入资源目录，禁止密钥入仓库。
- 工作台设置提供背景缩略图选择，另支持本地 `config/bong/workspace-backgrounds/` 的 PNG/JPEG 导入与刷新；只在显式选择/刷新时解码并复用纹理。限制像素尺寸/文件大小，损坏或失踪文件保留当前有效背景/内置回退，替换和 reload 时释放旧纹理。没有运行时远程下载或 AI 调用。
- `ui/window/WindowLayoutPreferenceStore` 参考 `menu/MainMenuConfig` 已有 Gson/UTF-8 本地配置方式，保存到 `config/bong/window-layout.json`；`HudLayoutPreferenceStore` 当前只有内存偏好，不能当作已存在的持久化设施。文件只保存窗口类型的布局/固定/最小化偏好、背景 id 与动画偏好；运行期物品、NPC、工位、session/offer token、草稿、ViewModel 和服务端数据不持久化。重进游戏只在新权威数据满足后恢复个人窗，临时业务窗待真实入口再次打开。配置不兼容时回退默认，不做旧 tab 布局迁移。

## 5. UI 状态、网络 Handler 与 Bootstrap 的外部接口纪律

### 5.1 Handler → Store

- Handler 解析 `ServerDataEnvelope`/generated payload，做校验、revision/freshness 和 domain dispatch。
- Handler 只写 Store、生成 transient `ServerDataDispatch` 或提交 domain offer；不得直接持有 Screen/widget 引用。
- Store 暴露 `snapshot()` 和 R7 `UiStateSource` adapter；Store 不 import `com.bong.client.ui.adapter.*`。
- Screen/HUD 只消费 ViewModel；不得从 `BongNetworkHandler`、`ServerDataRouter` 或 Handler 取数据。

### 5.2 UI → Intent → Sender

- Screen/widget/input 只调用 domain `UiIntentSink`，不直接 import `ClientRequestProtocol`/`ClientRequestSender`。
- 迁移期间允许 compatibility adapter 在 `client/ui/intent/**` 调用旧 sender；source gate 只允许该目录直接依赖 sender。
- intent 必须带现有请求需要的 identity（如 `instance_id`、`session_id`、`offer_id`/`trigger_id`、坐标/slot），UI 不猜测 server state。
- 不做 client optimistic success；等待 Store/S2C receipt 更新 ViewModel。

### 5.3 BongClient bootstrap

- `BongClient.onInitializeClient()` 保持 network → UI runtime → UI modules 的依赖顺序。
- P2 起将 UI/HUD/keybind call set 迁移到 `UiBootstrapRegistry`; 每批迁移一组并保留旧 module 的 idempotent `register()` 语义。
- `BongNetworkHandler.register()`、`IrisBootstrap.register()`、render/audio/debug 注册不因 UI registry 重构被移动。
- source gate 固定 UI module 清单、owner、依赖、注册顺序和 duplicate registration 行为。

### 5.4 Headless UI driver 与 bot e2e

`UiDriver` 是 semantic UI contract 的无渲染消费方，不是第二套 gameplay API。它与 Java client 共享 action registry、参数校验、`ClientRequestV1` 编码和 authoritative result projection：

```text
semantic UiSurfaceProjection
  -> UiDriver.open(surface_id/session_id)
  -> UiDriver.dispatch(action_id, typed args)
  -> same UiIntentSink / ClientRequestV1
  -> bong:server_data + correlated receipt
  -> UiDriver.awaitRevision/awaitReceipt
```

bot 只允许使用真实 production wire（`Bot.intent(...)`、`bong:client_request`、`proto_min.py` 解码的 `bong:server_data`）以及服务端明确授权的 fixture/setup；不得调用 Java Store、屏幕私有 callback、像素坐标、截图 OCR、raw XML/HTML/JS 或 dev 命令绕过核心动作。dev 命令若用于铺垫，必须与核心 action/receipt 断言分段记录，不能算作 headless 闭环证据。

P0R 冻结 `UiDriver` 的最小外部接口：`open`、`snapshot`、`listActions`、`dispatch`、`awaitRevision`、`awaitReceipt`、`close`；每个方法都带 session identity、revision/request identity 和超时结果。`dispatch` 先做同一 action registry 的参数/availability 校验，非法 action、过期 session、权限不足、重复 request、超时和关闭后的 late result 都必须可观察且无副作用。成功判据是权威 receipt/state transition，不是 transport write 成功。

bot e2e 分三层记录：

1. **contract pin**：surface/action shape、稳定 identity、revision 单调、枚举和 invalid payload；
2. **semantic roundtrip**：open → action → server mutation/拒绝 receipt → projection 更新/关闭；覆盖 happy path、边界、权限、过期、重复、超时和跨 session isolation；
3. **adapter geometry**：同一 ViewModel 在 owo XML host 的布局和输入映射测试，另行验证，不把像素或真实渲染引入 bot 主路径。

`scripts/bot/_agent_ui_helpers.py` 已有 `bong:agent_ui_cmd`、`bong:agent_ui_request`、`bong:agent_ui_close`、`bong:agent_ui_response` 的 request shape、按钮回执、dismiss、关闭和负向路径；P0R 必须把这些 helper 重定位为 semantic driver 的兼容实现，并标出仍依赖 raw XML 的路径。raw XML 路径在新 semantic surface 未完成前只能作为 legacy regression，不能成为新 adapter 的验收门。

## HUD SVG 表现后端契约（P4 已有基础，P6b 收口）

R7 的最终目标是：HUD 的几何表现统一由 SVG 资源描述，NanoSVG 只负责解析，自有 tessellator 负责把路径变成不可变 mesh，最终仍在 Minecraft 1.20.1 的 GUI 渲染阶段提交。HUD 的 Store、semantic planner、C2S intent、S2C payload 和 `HudRenderLayer` 不依赖 NanoSVG；SVG 是可替换的表现后端，不是业务模型。

### 1. 生产链路与类型边界

```text
Store / server snapshot
  -> BongHudOrchestrator（semantic frame）
  -> HudRenderBackend
  -> SvgHudAssetRegistry（白名单资源 + 缓存）
  -> NanoSvgParser（只解析）
  -> SvgDocument（不可变 Java 数据）
  -> SvgTessellator（只做几何）
  -> SvgMesh（不可变三角形）
  -> MinecraftGuiMeshEmitter（DrawContext GUI buffer）
```

- `HudRenderBackend`、`SvgParser`、`SvgHudAssetRegistry`、`NanoSvgParser`、`SvgDocument`、`SvgMesh`、`SvgTessellator`、`MinecraftGuiMeshEmitter` 是 R7-owned 类型，签名、线程约束和失败语义冻结在 `client/src/test/resources/bong/ui/ui-svg-hud-contract.tsv`。
- `NanoSvgParser` 的 public API 不暴露 native pointer、arena、C struct 或 parser 生命周期；native 结果必须在 adapter 内复制成受约束的 Java 数值，native 解析器不得直接触碰 `DrawContext`、`RenderSystem` 或 OpenGL。
- `SvgHudAssetRegistry` 只接受 `assets/bong-client/svg/hud/` 下的资源和显式 manifest；拒绝 `..`、外部 URI、网络 URL、`DOCTYPE`/`ENTITY`、超出大小/节点/曲线段/顶点预算的资源。资源重载时完成 parse+tessellate，渲染帧只读取 immutable cache。
- dynamic HUD 值通过受限 binding（颜色、opacity、visibility、transform、clip 范围和 progress）注入 mesh；禁止每帧字符串拼 SVG、重新 parse 或重新 tessellate。动态文字保留 `DrawContext`/`TextRenderer` 提交，物品图标保留 Minecraft GUI item renderer；两者属于明确的 GUI 合规例外，不得回流到 planner。

### 2. NanoSVG/parser 交付边界

- P4 vertical slice 固定 `SvgParser` ABI、资源校验、失败语义和 Java compatibility `NanoSvgParser`；当前实现使用 JDK StAX 解析受限 V1 图元，明确不宣称 JNI/native NanoSVG 已交付。native provider 属于后续独立交付，接入时只能替换 `SvgParser` 实现并继续通过同一 contract gate。
- 后续 native provider 若落地，必须固定 NanoSVG 版本、源码许可证、ABI 版本和加载错误诊断；至少覆盖 Linux x86_64、Linux aarch64、Windows x86_64、macOS x86_64、macOS arm64 五个平台。平台库不存在或 ABI 不匹配时必须 fail closed 并给出可定位日志，不得静默切换到另一套 parser。
- native 库只能由 Fabric client 在受控资源路径解包后加载；不得允许任意路径、环境变量或网络内容指定 native library。JNI 层只提供 parser 函数，所有 tessellation、mesh 生命周期和 GUI 提交留在 Java 侧。NanoSVG 的文本、image、filter、mask、foreignObject、gradient、外部 stylesheet 等不在 V1 支持范围；资源校验必须明确拒绝或记录 unsupported，而不是生成半成品 mesh。

### 3. Minecraft GUI 提交约束

- emitter 只能接收 `DrawContext`、当前 GUI `MatrixStack` 和已缓存 `SvgMesh`；禁止世界坐标、`VertexConsumerProvider` 的实体 layer、独立 framebuffer、浏览器/Canvas/NanoVG、直接自建 OpenGL program 或 `RenderSystem` draw call。
- 1.20.1 的 `RenderLayer.getGui()` 是 `POSITION_COLOR + QUADS`，不是三角形 buffer。V1 emitter 必须把每个 tessellated triangle `(a,b,c)` 编码为退化 quad `(a,c,b,b)`，经 `context.getVertexConsumers().getBuffer(RenderLayer.getGui())` 提交，并在 `DrawContext.draw(...)` 边界内批量 flush；不得把三角形直接塞进 QUADS buffer。
- GUI layer 的 alpha、depth、blend、z 顺序必须沿用 Minecraft GUI phase；`HudRenderLayer` 的现有顺序是唯一 layer order，mesh 不能靠浮点 z 或世界深度重排。发射器必须拒绝 NaN/Infinity、越界坐标和超过顶点预算的 mesh。
- `BongHud.renderCommands` 最终只负责收集 frame、应用统一可见性和调用 `HudRenderBackend`；P6b 完成后不得再按 `isRect`、`isTexturedRect`、`isEdgeVignette` 等旧 primitive 分支直接画 HUD。固定功能窗通过 owo 窗口 adapter 在同一 GUI 绘制协调中提交，不把交互 XML 树转换成 SVG。

### 4. 全量迁移与删除门

- `ui-svg-hud-inventory.tsv` 逐行登记当前 `HudRenderLayer`、planner、直接 HUD overlay、SVG asset、dynamic binding、GUI 例外与测试 owner；历史 61 layer/50 planner 不是固定数量。允许复用资产，每个保留 layer 须有可核验 binding。
- P4 已有真实 SVG layer，后续沿用；不恢复 `example.svg`。旧 backend 仅作受测迁移过渡，不能进入最终归档状态。
- P6b 让 inventory 中所有保留 layer/直接 overlay 经过同一个 backend，迁移 semantic output，保留 parser/tessellation/资源重载/可见性/生命周期的必要失败契约，并删除旧 shape path、测试专用 `renderSurface` 和生产 fallback。用当前生产路径和必要截图对照证明删除安全，不为未接入的 native provider 补虚构完成证据。

## 6. 阶段总览

- ✅ 2026-07-30 **旧 P0 盘点基线**：迁移前真实 Screen **29** 个、92 fill、15 clearChildren、keybind 冲突、R2/R6 ownership fixture 已存在；仅 docs/tests/resources，未改变 production behavior。后续已删除一个退役 Screen，当前盘点为 28 个。
- ✅ 2026-08-25 **P0R contract rebase + semantic/owo migration gate**：补齐 library-neutral contract、semantic surface/action 接缝、Store read adapter、typed intent、bootstrap registry、依赖方向和迁移 exemption；冻结 owo XML 唯一生产宿主、`UiViewport`/layout policy 与 resolution matrix；更新长期 fixtures，不生成 production adapter。
- **历史 foundation 已清理，不计入 neutral P1 完成**：`5e40e5ced` 的 owo 专用 `DiffListWidget` 和 `5822dd51a` 的 owo 专用 `BongScreenBase` 没有生产调用者，已连同测试和契约登记删除；`b48dd162c` 的 `BongKeybindRegistry` 仍因生产 bootstrap 接入而保留。旧实现的行为证据不再作为长期 API。
- ✅ 2026-08-26 **P1 core contract + fake/headless projection**：落地 `ui/contract/**`、reconciler、scope、intent result、bootstrap graph、`UiViewport`/`UiLayoutPolicy`；提供不依赖渲染器的 `UiSurfaceProjection`/`UiDriver` fake 和 `StoreUiStateSource`；contract、intent、state、headless 包均未依赖 owo、vanilla widget、Minecraft 或具体 UI 库。
- ✅ 2026-08-27 **P2 owo XML adapter + bootstrap reference slice**：唯一 owo XML host、Craft wide/compact 本地模板、host 生命周期、分阶段 bootstrap 和真实 Fabric/owo 截图/交互门均已落地；Store/Intent 解耦留给 P3。
- ✅ 2026-08-30 **P3 Store/Intent 边界迁移批次 A**：用 semantic surface + 本地 owo XML template 接通同一 controller/view-model/typed intent，再迁移 `AlchemyScreen`、`CraftScreen`、`TradeOfferScreen`、`LootContainerScreen` 及其 panel；UI 不再直接引用 sender/handler；bot 用同一 action id 完成 roundtrip；保留现有 wire 与 server authoritative semantics，wire 形状变更按 R6/schema amendment 原子接入。
- ⏳ **P4 公共窗口基础与首窗**：已有 XML/SVG/parser/backend/open policy 基础；P4a 窗口 identity/scope/input/布局与物品详情窗 ⏳，P4b 最小化/HUD 固定/尺寸输入/动画/背景/偏好 ⏳。
- ⏳ **P5 功能窗口分批迁移**：P5a 已迁移背包/容器、装备/快捷槽与跨窗拖放，待本批完整验收；P5b 修仙/技艺/功法/配置；P5c 手搓/工作台制作/锻造；P5d 炼丹/养护/布阵等工位；P5e 搜刮/NPC/交易/阅读等交互。每批同时迁移真实入口与关闭语义。
- ⏳ **P6 受控界面、HUD 与 Bootstrap 收口**：先按用户要求接入既有 HUD 布局编辑；P6a offer/系统界面/raw XML 依赖和全局开窗仲裁未开始，P6b 剩余 SVG layer 和旧路径删除待续；保留 exact offer settlement、权威 combat snapshot 与 R2/R6 ownership。
- ⬜ **P7 全量验收 + 归档**：逐项核对范围表及运行时入口；必要逻辑测试、Java 17 门禁、真实窗口矩阵、受影响 wire smoke 和 reconnect 通过；未接线项有明确处理结果，旧窗口管理器和退役 Screen 路径删除后补 Finish Evidence。

## 7. 分阶段交付物与验收抓手

### P0R — contract rebase + semantic/owo migration gate（ZERO production behavior change）

- **模块**：`client/ui/{contract,state,intent,bootstrap}/` 的契约 fixture（含 `UiStateSourceMode`、`UiSurfaceProjection`、`UiActionRegistry`、`UiViewport`）；长期保留 `ui-contract.tsv`、`screen-adapters.tsv`、`store-state-sources.tsv`、`intent-boundary.tsv`、`ui-dependency-allowlist.tsv`、`ui-bootstrap-modules.tsv`、`semantic-surface.tsv`、`viewport-matrix.tsv`、`ui-backend-capabilities.tsv`、`ui-svg-hud-contract.tsv` 和 `ui-svg-hud-inventory.tsv`。

- **交付**：29 Screen adapter classification、UI import dependency rules、Store subscription semantics、Intent local-transport semantics、BongClient UI bootstrap module inventory；冻结 semantic surface 的必需 identity/revision/action 字段和 legacy raw XML 隔离；登记 `UiDriver` 外部接口；冻结 `OWO` 唯一 production backend capability、`MIN_SUPPORTED_VIEWPORT`、逻辑/physical 坐标转换和 odd-resolution matrix；登记 61 个 HUD layer、50 个 planner、直接 overlay 与 SVG backend owner。

- **测试**：`R7FoundationContractTest`、`R7ScreenInventoryContractTest`、`R7UiDependencyContractTest`、`R7BootstrapInventoryContractTest`、`R7StoreStateSourceContractTest`、`R7BackendCapabilitiesContractTest`、`R7SemanticViewportContractTest`；focused fixture 从 production source 派生并提供具体文件/符号诊断。全树 production digest 仅作为 P0R 一次性历史举证，不作为后续阶段的长期门禁。SVG backend 的 parser/tessellator/emitter contract 在 P4 单独验证，不能用全树 digest 代替。

长期门禁只覆盖 R7-owned 的 screen、state、intent、bootstrap、semantic 和 viewport 接缝；客户端其他资源、并行 PR 新增的 production 文件以及无关美术资产不属于 R7 contract drift。若需要证明某次阶段确实没有生产变更，应在该阶段单独生成一次性审计证据，不更新长期 fixture。

- **跨仓库**：不在 R7 内直接改 schema/proto/Redis/CustomPayload；现有 `proto/bong/envelope.proto`、`agent/packages/schema/src/server-data.ts`、`scripts/bot/_agent_ui_helpers.py` 的 raw XML 耦合登记为 R6/schema/agent amendment 输入，未完成 atomic activation 前只保留 legacy regression。

#### P0R Finish Evidence

- **落地清单**：新增 source-derived Screen adapter、Store source、bootstrap order、semantic/intent/viewport、UI dependency、backend capability 和 focused contract tests；长期 fixture 统一使用 `ui/` 语义文件名，不再使用阶段性 `r7-` 前缀。未新增 `client/src/main/java` production adapter、Screen migration 或 wire/schema 字段；P2 已完成 `CraftScreen` 的 XML host，当前 6 个 vanilla、12 个 owo CODE、8 个既有 XML 模板与 2 个 XML_MODEL 入口仍是 P3-P4 的明确迁移债务。
- **关键证据**：P0R 原始快照的 production Java source tree SHA-256 为 `66502bbf20e7be0999576c612eac5b53d23c81ca9c5e8c3cad91e67bc3558f2b`，该摘要是一次性历史证据，不再覆盖后续资源或并行 PR 的变更；迁移前 Screen inventory 对拍为 29 个（15 owo、14 vanilla），当前删除一个后为 28 个（22 owo host、6 vanilla）；BongClient UI bootstrap 对拍为 30 个模块；Store source fixture 对拍通过。
- **测试结果**：`./gradlew test --tests 'com.bong.client.ui.R7*ContractTest' --tests com.bong.client.insight.InsightOfferScreenTest --tests com.bong.client.insight.InsightOfferStoreTest -x runGametest` 通过；Java 17 `flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"` 通过，包含 3 个 Fabric GameTest。
- **跨仓库核验**：未修改 server、agent/schema、protobuf、Redis key、CustomPayload 或 R2/R6 owner 文件；semantic surface 继续与模板实现分离，现有 raw XML 仍标记为 legacy 输入，后续 wire cutover 仍由 R6/schema/agent amendment 负责。
- **遗留 / 后续**：P1 library-neutral core、P2 Craft XML reference slice 和 P3 状态/意图边界迁移已完成；P4-P7 仍待实施。无生产引用的 legacy foundation 已删除，下一阶段继续迁移 6 个 vanilla Screen、12 个剩余 owo CODE Screen 和 2 个 XML_MODEL 运行时入口；`VANILLA` 仅留在迁移前统计，第三方 host 明确 OUT_OF_SCOPE。

### P1 — library-neutral core ✅ 2026-08-26

- **模块**：`client/ui/contract/**`、`client/ui/intent/**`、`client/ui/state/**`、`client/ui/headless/**`；纯 Java fake 不依赖 Minecraft widget 或具体 adapter。
- **已有实现边界**：无生产引用的 `BongScreenBase` 和 `DiffListWidget` 已删除，不向 `ui/contract/**` 暴露兼容 API。`BongKeybindRegistry` 的全局注册与冲突校验已落地，P4 只处理其余生产覆盖、线程/打开策略和迁移收口。
- **交付**：scope LIFO/error aggregation、subscription close/idempotence、reconciler commit/retry、typed intent result、bootstrap dependency graph、semantic surface projection/action registry、`UiDriver` fake、`UiViewport`/`UiLayoutPolicy` 的纯函数实现。
- **测试**：empty→items、equal keys、reorder/add/remove、duplicate/null、patch failure/full retry、rebuild create failure、late callback、double close、dependency cycle/missing/duplicate/idempotent register；surface revision/session/action validation；driver invalid/expired/duplicate/timeout/close；viewport safe rect、compact/regular/wide、text/hit-region overflow 和 coordinate round-trip；每条失败信息带行为原因。定向 UI 测试和完整客户端 `test build` 均通过。
- **跨仓库**：不新增 wire；intent encoder 通过既有 sender contract tests 对拍。

### P2 — owo XML adapter + bootstrap reference slice ✅ 2026-08-27

- **模块**：`client/ui/adapter/owo/**`、`client/ui/preview/**`、owo XML template registry、`assets/bong/owo_ui/{craft,craft-compact}.xml`、`CraftScreen`、对应 bootstrap。
- **交付**：`CraftScreen` 已接入唯一 owo XML host；Screen removed/close/tick/input cleanup 一致；动态 XML 只留在明确的 legacy compatibility path，新 Screen 只使用受白名单保护的本地 XML；不建立 vanilla host。Craft 外框以 `viewport - 20 x viewport - 12` 连续填满安全区，在 `660x360` 断点选择三栏或 compact 纵向滚动模板，不再在断点两侧跳回固定面板尺寸。server/agent 仍只看语义 `template_id`，不知道本地布局变体。
- **真实渲染门**：`client/ui-preview-harness.json` + `runClientUiPreview` 使用固定本地 Store fixture 打开真实 production Screen，严格对拍 framebuffer、GUI scale、逻辑 viewport、所选模板和关键组件 bounds，再由 `ScreenshotRecorder` 输出 PNG + metadata。首批覆盖 `320x240` minimum、`401x241` odd 和 `683x384` wide；必须等资源重载与 owo adapter 初始化成功，禁止把离线 XML 模拟图或初始化失败后的空帧算 PASS。
- **测试**：fake ViewModel 到 owo XML host 的绑定行为；最低/odd viewport 的 bounds、文字、hit region、focus order和输入逆变换；subscription 不泄漏；adapter close 后 late state/intent no-op；bootstrap registration order/once；生产源码 gate 禁止新的 vanilla Screen 和 owo CODE root。截图供人工视觉复核，自动 PASS 由 metadata、模板选择、adapter ready 和 geometry 断言决定，不做脆弱的整图 hash 比对。
- **跨仓库**：Craft 既有 C2S/S2C type、request identity、server rejection semantics 完整保留；未修改 server、agent/schema、protobuf、Redis key 或 wire shape。

#### P2 Finish Evidence

- **落地清单**：`OwoXmlScreenHost`、`OwoXmlTemplateRegistry` 和 `OwoXmlHostLifecycle` 组成唯一生产 XML 宿主；`craft.xml` / `craft-compact.xml` 及 `CraftScreen` 完成 wide/compact reference slice；`ClientUiBootstrap` 仅迁入 `screen_transition`、`craft_screen` 两个模块，并由 `UiBootstrapRegistry` 按依赖闭包 exactly-once 注册；`UiPreviewClient`、`UiPreviewResultFile` 与 `runClientUiPreview` 构成真实客户端截图门。
- **关键证据**：正式矩阵 `320x240 compact`、`401x241 compact`、`683x384 wide` 全部通过；奇异矩阵 `659x360`、`660x359`、`660x360`、`1001x241`、`321x641`、`1001x721 @ GUI scale 3`、`997x263` 全部通过。`659x360 -> 660x360` 的安全区外框只连续变化 1 px；GUI scale 在 framebuffer 就绪后应用，部分截图、viewport mismatch、初始化失败均使 Gradle 任务失败。
- **测试结果**：adapter/preview/bootstrap 定向测试 29 条通过；真实 renderer 在 settle 后验证安全区、关键控件 in-bounds、中心点命中、physical/logical 坐标往返、Tab 焦点顺序与 compact scroll 内容范围。最终 Java 17 `test build` 与正式 `runClientUiPreview` 结果记录在本阶段 PR。
- **跨仓库核验**：改动仅位于 client 与本 plan；`ClientRequestSender`、Craft Store/S2C handler、server/schema/agent 均保持原契约，XML 模板只存在于 client 本地资源。
- **遗留 / 后续**：本阶段只验证 host/lifecycle/bootstrap/viewport reference slice。`CraftScreen` 仍直接消费现有 Store/Sender，semantic ViewModel、`UiStateSource` 与 typed `UiIntentSink` 接线属于 P3；其余 28 个 Screen 的 XML 化属于 P4，未在本阶段提前迁移。

### P3 — state/intent boundary migration A

- **模块**：`AlchemyScreen`、`CraftScreen`、`TradeOfferScreen`、`LootContainerScreen`、相关 panels/bootstrap、`client/ui/state/**`、`client/ui/intent/**`。
- **交付**：先用 semantic surface + 本地 owo XML template 跑通一条完整 vertical slice；Screen 不直接依赖 `ClientRequestSender`、`ClientRequestProtocol`、network Handler；所有 Store 读取经 `UiStateSource`/ViewModel，所有输入经 typed sink；明确交易显式 picker 和 inventory `instance_id`；`SemanticUiDriver` 用同一 action registry 跑对应 bot roundtrip。
- **测试**：非法参数、过期 session、late callback 和关闭后 intent 全部 fail closed；bot 不使用像素点击且能验证 open/action/receipt/revision/rejection/close/session isolation；Craft/Alchemy/Trade/Loot 的 server authoritative roundtrip、无 selection refusal、transport accepted 与 server accepted 分离、断线后 scope/Store 不串会话；existing UI C2S smoke 对拍。
- **跨仓库**：R2 lifecycle、R6 router、schema/proto 不改；CraftStore 只消费 M-09 冻结 contract。

#### P3 批次 A 验证证据

- **落地模块**：`alchemy/{AlchemyScreenViewModel,AlchemyUiStateSource,AlchemyScreenController,AlchemyIntent,AlchemyClientIntentSink}.java`、`social/{TradeOfferScreenViewModel,TradeOfferUiStateSource,TradeOfferScreenController,TradeOfferIntent,TradeOfferClientIntentSink}.java`、`inventory/{LootContainerScreenViewModel,LootContainerUiStateSource,LootContainerScreenController,LootContainerIntent,LootContainerClientIntentSink}.java`；Craft 的同型边界沿用 P3 Craft slice；`ui/headless/SemanticUiDriver.java` 接入同一 action registry。
- **边界结果**：目标 Screen/Panel 不再直接引用 Store、`ClientRequestSender`、`ClientRequestProtocol` 或 handler；所有状态通过 `UiStateSource` → immutable ViewModel，所有输入通过 guarded typed `UiIntentSink`；关闭 scope、late callback、过期 session、重复 request、stale revision 和 transport/local rejection 均 fail closed。
- **身份与会话**：Trade picker 只接受当前 authoritative inventory 中明确的 `instance_id`，选择对象消失时不自动换选；Loot panel 按 `session_id` 卸载旧实例，禁止跨会话继续发送 move/close。
- **测试结果**：定向 P3 JUnit 套件 15 条通过；同一 Gradle 任务额外执行的 Fabric GameTest 门 3 条通过；完整 `./gradlew test build` 通过，5044 条 JUnit 测试、0 failures、0 errors；未修改 server、agent/schema、protobuf、Redis key 或 wire shape。

### P4 — 公共窗口基础与首个真实窗口

- **P4a ⏳ 模块与交付**：`ui/window/{UiWindowManager,UiWindowDefinition}`、owo XML 窗口内容 adapter、工作台宿主与 `UiBootstrapRegistry` 登记；将 `inspect/ItemInspectScreen` 迁为首窗，物品统一左键双击查看、右键使用。冻结 key/重复打开/独立 scope/顶层命中/尺寸测量/窗口关闭语义。已有 `UiScreenController`、`UiStateBinder`、`OwoXmlScreenHost` 继续复用，不增加并列 Screen 框架。
- **P4a 真实证明**：在真实 Fabric/owo 中完成打开、置顶、拖动与关闭；证明 XML 内容可以由工作台和 `currentScreen == null` 时的 GUI adapter 呈现，无第二套 state source。不能仅用 fake driver 或截图贴片宣称 HUD 窗口可行。旧 Inspect 功能只允许在逐域迁移期继续使用，P5a 删除替换后的入口。
- **P4b ⏳ 模块与交付**：最小化恢复条、固定图标、宽高输入、动画、背景选择/本地图片导入、`WindowLayoutPreferenceStore`；按 §4.9 实现，首窗同时用于验证工作台/HUD 转换。背景 PNG 与窗口框架跟随本批交付，执行三轮视觉打磨，Round 2 必须给用户看接触表。
- **必要逻辑测试**：同 identity 重开不复制资源；最小化/固定/宿主切换不结束业务 scope；明确关闭/失效后动作不可执行、迟到回调不复活旧实例；重叠命中与捕获取消；非法尺寸和坏配置有可用回退。以业务动作与状态结果断言，不测试私有字段数、每帧坐标、固定 easing 时长或配置 JSON 字段顺序。
- **前置与边界**：检查现有 SVG/XML 与 `ScreenOpenPolicy` 实现，不重复实现已完成 vertical slice；native parser 独立交付。R6 receive boundary 不重复 marshal，R2 仍拥有 Store 的断线清理。

#### P4a 工作记录（2026-09-12，未验收）

- 规划 commit：`d791995c5`。首窗实现 commit：`2c9bb04eb`（2026-09-12）；用户确认详情窗口后授权提交并继续下一阶段，原 `r7-window-core` 草稿保留。
- 代码：`ui/window/{UiWindowDefinition,UiWindowManager,UiWindowRuntime}.java`、`ui/adapter/owo/OwoXmlWindowContentAdapter.java`；`ItemInspectScreen` 退役为 `inspect/{ItemInspectContent,ItemInspectWindows}.java`，左键双击在既有 `InspectScreen` 中打开独立 XML 窗口，不再替换 Screen。窗口 scope 与宿主关闭分离，明确关窗、物品失效、连接/世界切换才清理。
- 状态读取：全部物品窗共享库存和当前外部容器的 `UiStateSource`，每个客户端 tick 读取一次当前快照；XML 和 HUD 不创建库存订阅。外部容器会话结束后详情失效。首窗仅显示详情，无新增业务 action。窗口 generation 只管理本地 UI identity，不改 R2/R6 token。
- 新增用户范围：`inspect/{ItemInspectModel,ItemModelPreviewComponent}.java` 复用武器/工具/盾、方块、护甲与背包现有模型注册；有模型时显示 `3D`/`PNG` 切换。模型按包围盒居中等比显示，区域内按住左/右键反向连续旋转，释放、离开区域、失焦或退出宿主停止。属性刷新保留当前视图与角度，不新增资产映射或业务请求。
- 叠放与输入修复：`window-frame.xml` 使用不透明背景和扁平关闭按钮；每个窗口独立递增绘制深度，避免 owo 子组件文字穿过上层背景。独立 adapter 将屏幕坐标转换成根组件局部坐标，修复非零窗口位置下的按钮/滚动/模型命中偏移。
- 用户反馈修订：移除 Inspect 右栏的旧 `ItemTooltipPanel` 挂载及动态悬停重排；装备层数约束保留为槽位简短提示。`ItemInspectClickTracker` 统一网格、装备、快捷栏、套包和 loot 的双击判定，鼠标越过移动阈值才拾取，避免查看时发送库存移动/快捷栏解绑；右键沿用使用菜单和开包。铁镐恢复原版 handheld 模型，同时解除 SML 接管。窗口增加层叠边缘、阴影、渐变标题栏与凹入媒体区，本轮未引入新的图片资产。
- 测试调整：删除旧右键长按、双击开包判定测试以及重复的容器谓词测试，改为 3 条点击/拖动/取消状态转换回归；保留背包移动协议和工具槽位权限回归。退役 OBJ 的存在性断言改为原版模型与 SML 接线回归，不新增颜色或源码字符串测试。
- 详情数据修订：`ItemInspectContent.detailRows` 输出标签/值结构，`item-inspect.xml` 按左右两列排版，描述和丹药说明独立换行。删除 `durability` 冒充保质期、空充能占位和按名称猜灵材的规则；充能/铭文/灵核等仅在服务器元数据存在时展示，耐久仅在受损时展示。名称/描述/重量/占格继续来自 `server/assets/items/*.toml` → `ItemInstance` → inventory snapshot；不改 wire 或另建前端物品配置。`scripts/export-item-preview.py --output /tmp/bong-item-preview.json` 从这些 TOML 生成预览配置，预览复用正式物品解析器，不再写死名称、文案和尺寸。
- 预览入口：`UiWindowPreviewScene` 与 `client/window-ui-preview.json`，最低/奇数/宽屏三个 viewport，另含腿甲、背包、方块模型场景；覆盖两窗去重置顶、真实鼠标拖动、非零坐标关闭。`UiPreviewScene.prepareScreenshot` 在等待阶段前完成输入，后续正常帧才截图。Windows Java 17 原生 Fabric jar 预览已成功运行，不走 WSLg/HTML；临时启动参数与截图位于 `D:/Minecraft/.minecraft/Fabric_Bang_Test/bong-native/`，不改正常启动器。
- 构建验证：解除环境限制后，以 Java 17 在 client 执行 `../scripts/build-token.sh gradle test build --offline` 通过，5134 条 JUnit、3 条 Fabric GameTest，无失败。仅新增窗口状态与物品失效的必要逻辑测试；原详情测试随内容类迁名，旧 Screen 转场登记断言随退役移除，未增加截图像素或动画常数单测。
- 本轮反馈修订验证：预览夹具改为容纳真实 TOML 尺寸后，以 Java 17 完整门禁再次通过，5118 条 JUnit、3 条 GameTest，无失败；Windows 原生 `item-windows-check-20260912-204442` 记录 `status=passed / completed=6`。场景通过真实 `Screen` 输入验证悬停不重排、双击无库存请求、关闭后重开、拖放取消回源；截图留在该输出目录。本轮首窗视觉经用户确认，P4b 背景和窗口体验仍需单独验收。
- 待验：连接场景下的物品失效和重连尚未完成实机验收。无 Screen adapter 呈现能力已在 P4b 原生 fixture 证明；该证明不等于联网游戏 HUD 的完整验收。不得把当前记录视为 P4a 完成或整个 R7 的 Finish Evidence。

#### P4b 工作记录（2026-09-13，外观已认可，连续动画与联网待验）

- 本轮用户授权先提交 P4a 已验证部分，再在同一工作区推进 P4b；阶段依赖保持，PR/review/合入门尚未执行。
- 窗口状态：`UiWindowManager.minimize/restore/pin/resize` 保留 scope；最小化撤销捕获与命中，显式同 key 打开恢复并置顶。期望尺寸独立保存，viewport 缩小只钳制有效外框；窗口底部留出 28 logical px 恢复条。
- 表现：`OwoXmlWindowContentAdapter` + `window-frame.xml` 提供最小化、锁定、关闭、宽高输入；Enter 提交，非法输入保留旧布局，Esc 取消草稿。锁定后仍能在工作台拖动。HUD 隐藏标题操作与尺寸行，复用同一个内容 adapter；`BongHud.render` 接入 `UiWindowRuntime.renderHud`，仅正常游戏且未隐藏 HUD 时显示固定展开窗。
- 工作台：`WorkspaceControls` + `workspace-controls.xml` 提供底部横向滚动恢复条、动画开关、背景选择、打开本地目录和刷新。`WorkspaceBackgrounds` 等比 cover 内置宇宙/地形，支持 `config/bong/workspace-backgrounds/` 的 PNG/JPEG；检查 16 MiB、4096 边长、8 Mi 像素限制后解码，失败保留当前图像，替换和资源 reload 释放旧动态纹理。外部图片的未选中缩略图仍待打磨。
- 动画：`WindowMotion` 使用单调时钟；打开、尺寸变化和最小化/恢复已有矩形过渡，新操作从当前插值继续，拖动直接跟随；尚需连续帧验收和 §4.9 的渐隐/曲线终轮调参，不以静态截图声称动画全验收。
- 偏好：`WindowLayoutPreferenceStore` 以 Gson/UTF-8 和临时文件替换写入 `config/bong/window-layout.json`；仅保存类型布局、固定/最小化、背景与动画开关，已登记 persistent-config lifecycle。缺失配置使用默认，坏配置不部分覆盖；显式物品双击总是展开，磁盘偏好不会重建旧实例。预览使用独立内存偏好，退出还原，不污染用户磁盘配置。
- 生图：沿用 `gen-image` 的 `scripts/images/gen.py --style none`，`gpt-image-2` 生成 `textures/gui/workspace/{cosmos,terrain}.png`；初始渠道 502 后使用主仓库已有本地配置成功。原图与 prompt 位于 ignored 的 `local_images/workspace/`，资源为待人工确认稿。
- 验证：Java 17 `../scripts/build-token.sh gradle test build --offline` 通过，5122 条 JUnit、3 条 Fabric GameTest，无失败。`item-windows-check-20260913-095947` Windows 原生预览 `status=passed / completed=9`，覆盖最低/odd/宽屏、不同物品模型、锁定、尺寸输入与 Esc 回退、恢复条、背景设置，以及 `currentScreen == null` 的同 adapter 呈现（无世界 fixture，非联网证据）。HUD 场景在正常渲染帧中绘制，并检查背景像素与非空内容；此前 tick 内补画会截到旧工作台帧的预览路径已修正，旧输出不再作为 HUD 证据。
- 测试范围：增加两条窗口状态/尺寸契约、两条偏好落盘/坏配置回归，已有物品失效回归扩展到固定且最小化状态；未添加颜色、动画时间常数或每帧坐标断言。scope/XML/R7 既有清单随新增入口同步。
- Round 2 人工接触表：`local_images/workspace/p4b-round-2-contact.png`，含前后同一宽屏取景、宇宙/地形、背景设置、minimum 和无世界 HUD adapter；原始 PNG 在上述 Windows 输出目录。用户确认后再进入 Round 3，当前不标 P4b 完成。
- 用户后续确认“看着不错了，继续加一个阶段”：窗口控制统一为 Lucide `lock-keyhole` / `lock-keyhole-open` / `minus` / `x`，48×48 PNG 以 14×14 显示，SVG 源及许可证留在 `assets/bong-client/svg/ui/`；图标对比为 `local_images/workspace/window-controls-review.png`。本次外观认可不替代连续动画和联网验收；按用户指令在同一工作区继续 P5a 容器子批次，尚未提交或执行 PR 门禁。

### P5 — Inspect 与全部普通功能窗口

| 子阶段 | 模块 / 可核验交付 | 必要回归 |
|---|---|---|
| P5a ⏳ | `InventoryContainerWindows` / `InventoryContainerContent` 管理容器；`InventoryLoadoutWindows` 管理装备、quick-use/SkillBar；`InspectScreen` 保留工作台入口和既有领域操作 | 背包→装备/快捷槽的真实 identity 请求；拖动时容器消失；浮窗置顶与挡住的槽位不命中；缩放不拉伸物品/人体 |
| P5b ⬜ | 修仙、技艺、功法、身份、化虚、玩家概览窗口；吸收 `SkillConfigPanelManager`；ViewModel + 窄 intent；共享经脉/技能状态 | 搜索/选择/滚动在最小化恢复后保留；施法/经脉/种族/config 限制仍生效；配置关闭与迟到更新不串对象 |
| P5c ⬜ | `CraftScreen`、`WorkbenchScreen`、`ForgeScreen` 的 XML 内容和真实入口；移除 `removed()` 与制作取消的耦合 | 制作进行时最小化并回到游戏仍按服务器计时；明确关闭才按原约定取消；切工位不沿用旧 session；同一 CraftStore 不产生两个可操作会话 |
| P5d ⬜ | Alchemy、Repair、ForgeCarrier、ZhenfaLayout、Lingtian 窗口；Processing 内容适配与接线依赖登记 | 有效工位/物品/材料约束，终态收取/取消/拒绝仍走原 intent；未接线加工无假按钮，无预览冒充生产 |
| P5e ⬜ | Loot、NPC 三页、TradeOffer、SparringInvite、ScrollRead、SpiritTreasure、Coffin 操作窗口及入口 | session/offer/token 过期与替换；跨窗物品选择；阅读最小化不结算，关闭只结算当前 token；被动邀请不抢普通输入 |

每批必须完成窗口框架下的布局/动效、真实入口、状态/intent、业务关闭语义和定向实机验证；不以“类已拆出”算完成。普通功能入口只能请求窗口管理器 `openOrFocus`，删除本批旧 `setScreen(new DomainScreen)` 和局部窗口管理分支。单窗口最小尺寸、compact 模板与保持比例要求直接登记在本批窗口 definition，不能推给最后统一处理。

**边界**：不等待、不改 R10 server inventory 内部重排；不为多窗扩 server 并发会话。active session bugfix 与本批触碰同一文件时先核对现行修复，保留其业务契约；独立缺失的生产接线由领域 owner 处理。

#### P5a 容器子批次工作记录（2026-09-13）

- 范围：贴身口袋、穿戴背包、套包接入 `UiWindowManager`，每个真实 `containerId` 唯一窗口和网格。`InventoryContainerWindows` 从库存快照刷新，`InventoryContainerContent` + `inventory-container.xml` 负责双向滚动和固定比例格子。装备、快捷槽及修炼等原页仍待后续完整功能迁移，本批不标整个 P5a 完成。
- 入口与操作：Inspect 的常驻容器入口和容器物品右键均打开统一窗口；普通物品仍是左键双击详情、右键使用。顶层可见网格接收拾取/落位，跨窗拖放保留真实 instance/container identity 与旋转协议；菜单和拖动物品在窗口之上绘制，裁剪外格子与滚动条不接收物品操作。
- 生命周期：最小化/固定/隐藏工作台保留同一 scope；显式关闭只关闭容器视图，重开从当前库存恢复。权威移除容器则关闭对应窗口，正在使用该来源的拖动取消；取消拖放按原 `containerId` 回源。容器容量（行列数）变化重建网格而不复制窗口；窗口尺寸变化只重排内容与滚动区域。摘要仅在快照变化时更新，避免每帧触发 owo 布局。
- 退役：删除 `PackWindowManager`、`WornContainerPanel`、`PackContainerWindow` 及 Inspect 内旧 tab 网格/局部浮窗命中/屏幕级多格物品绘制，统一管理器复用既有窗口状态、动画、HUD 固定与尺寸控制。
- 测试替代：删除上述三类旧实现测试，其局部偏移、root 挂载顺序和每窗独立订阅已不再是生产行为；通用 identity/scope/置顶契约沿用 `UiWindowManagerTest`。新增 `InventoryContainerWindowsTest` 两条必要契约保护容器失效与容量变化，保留真实移动意图、取消回源、穿戴包协议等原有逻辑测试；未新增颜色、图标尺寸、动画常量或源码字符串断言。
- 验证：最终代码以 Java 17 完整运行 `../scripts/build-token.sh gradle test build --offline`，5104 条 JUnit、3 条 Fabric GameTest 无失败，日志 `/tmp/bong-p5a-resume-gate.log`。`inventory-window-ui-preview.json` 与 `UiInventoryWindowPreviewScene` 提供 minimum/odd/wide/裁剪场景，沿用服务端 TOML 导出夹具与 Windows 原生 Fabric 启动链路；`D:/Minecraft/.minecraft/Fabric_Bang_Test/bong-native/item-windows-check-20260913-135128` 记录 `status=passed / completed=4`，覆盖跨窗拖放及旋转请求、遮挡、最小化恢复、关闭重开和容器失效；同 jar 的旧详情场景 `item-windows-check-20260913-135711` 记录 `status=passed / completed=9`。
- Round 2 人工接触表：`local_images/workspace/p5a-round-2-contact.png`，含本批前次预览 `item-windows-check-20260913-113447` 与最终代码的同一宽屏取景、minimum/odd/滚动裁剪和详情回归截图；同取景 PNG 像素完全一致，最后收尾修改没有静态外观差异。截图尺寸与非空像素统计保存于同名 JSON，仅用于产物核验，不代表外观或联网验收。已将最新 jar 更新到 Windows 测试实例，待用户看图后继续视觉终轮。
- 实机反馈修订：用户认可容器窗口后要求尺寸输入按需展开。共享 `OwoXmlWindowContentAdapter` / `window-frame.xml` 新增 Lucide `maximize-2` 按钮，宽高输入默认不挂载，点击后在标题栏下方展开；提交、Esc、再次点击、最小化或进入 HUD 时收起并清除输入焦点。沿用既有尺寸校验与偏好存储，无新增 JUnit；原生详情场景调整为真实展开/提交/非法值/Esc 输入流程，增加收起后焦点释放检查。
- 修订验证：Java 17 `gradle test build --offline` 重跑通过（5104 JUnit、3 GameTest；首次进程退出 143 未计为通过，完整日志 `/tmp/bong-window-resize-toggle-gate-retry.log`）。同一 jar 的 Windows 原生 `item-windows-check-20260913-142704` 详情 9 场景与 `item-windows-check-20260913-142854` 容器 5 场景均通过；后者新增展开尺寸输入截图。`local_images/workspace/resize-toggle-review.png` 为同一容器窗口的此前/收起/展开局部取景对比。
- 待验与边界：当前原生场景通过真实 `Screen` 输入与 C2S 编码检查，传输由预览 harness 捕获，不能当作真实服务器回执或联网背包验收。新容器外观需以本批接触表交用户验收；PR/review/合入尚未执行。

#### P5a 装备与快捷槽子批次工作记录（2026-09-13）

- 前置验收：用户认可容器窗口及按需展开的尺寸输入，并要求继续下一阶段。本批接着迁移装备和快捷槽，不提前推进 P5b/P5c。
- 落点：`InventoryLoadoutWindows`、`inventory-equipment.xml`、`inventory-shortcuts.xml`；窗口 key 分别为 `inventory-equipment/player` 和 `inventory-shortcuts/player`。`UiWindowRuntime.openLoadout` 统一打开和置顶，沿用共享窗口的拖动、尺寸控制、最小化与 HUD 固定。装备槽保持固定比例，窗口缩小时滚动内容。
- 入口与状态：Inspect 原装备页收敛为“随身”入口，旧侧边槽条退役。库存、SkillBar、QuickUse Store 仍为状态来源，窗口隐藏时继续刷新；关闭先卸载组件再销毁 adapter，重开挂回同一组组件。物品仍走双击详情、右键使用与原拖放请求；装备保持 instance/location，快捷绑定采用 instance_id/request_id 及权威确认。
- 输入修订：只有顶层窗口可见且未被裁剪的槽位参与命中，原 Inspect 内容不接收覆盖区域输入；已绑定技能的拖动与右键解绑不再要求选中“功法”标签。技能松手先完成领域拖放，再由通用窗口处理其它鼠标释放，避免吞掉换槽请求。
- 测试范围：沿用既有装备限制、移动意图、快捷槽确认、实例耗尽清槽与选中态测试；删除绑定旧标签文案和索引的显示断言，保留手搓真实入口测试。原生场景扩展到最低/odd/宽屏，覆盖装备往返、两类窗口关闭重开、最小化与隐藏刷新、遮挡/裁剪、快捷绑定确认、技能换槽与解绑；未添加颜色、动画常量或私有布局字段测试。
- 验证：最终 Java 17 `../scripts/build-token.sh gradle test build --offline` 通过，5104 条 JUnit、3 条 GameTest 无失败，日志 `/tmp/bong-loadout-final-gate.log`。Windows 原生 `item-windows-check-20260913-153545` 为本批 8 场景通过；旧详情的 9 场景在 `item-windows-check-20260913-153028` 通过，此后只替换库存预览中的测试技能为带真实 PNG 的 `sword.thrust`，生产逻辑未变。预览健康人体快照显式注入，避免旧 `MockPhysicalData` 的断臂夹具阻止主手装备。
- Round 2 接触表：`local_images/workspace/p5a-loadout-round-2.png`，含前次/当前宽屏、minimum/odd 裁剪和容器同取景回归；同名 JSON 记录原始截图路径、尺寸与非空像素统计。新技能图标复用仓库现有 PNG，无新生成资产；Windows 测试实例已更新为最终 jar。
- 待验与边界：原生场景捕获真实 C2S 编码并注入确认快照，不代表服务器联调。外观等待本批接触表人工验收；P4 连续动画与联网验收仍待补，P5a 暂不标完成。当前未提交、未推送、未开 PR。

### P6 — 受控界面、HUD 与 Bootstrap 收口

- **P6a ⬜**：Insight/AgentUi/DynamicXml 受控窗口；Death/Terminate/MainMenu 系统界面；`ScreenOpenPolicy`、`ScreenTransitionController`、`ScreenHudVisibility` 与剩余 bootstrap。普通窗口并存、scope 与 HUD 固定以 §4.9 为准；被动社交邀请仍以权威 `combat_active` 判定，缺快照 fail closed；系统终端按现有优先级抢占。
- **身份与依赖**：保留 exact `offer_id`/reading token settlement 和专属 Agent UI VFX；旧 A 的迟到关闭不能影响 B。raw XML 退出须等待 R6/schema/domain amendment，不能把它描述为仅 XML 换皮即可完成；`InsightDecision` wire 仍只有 `trigger_id`/`choice_idx` 时，不宣称已有 wire-level offer isolation。
- **P6b ⏳**：既有 HUD 的布局编辑子批次见下；`BongHudOrchestrator`/render backend 与固定窗口 adapter 的顺序、遮挡、资源 reload 统一仍需收口。按当前 `ui-svg-hud-inventory.tsv` 处理剩余 layer/overlay，保留必要 PNG/文字/物品 GUI 例外，删除旧 primitive path、`renderSurface` 与生产 fallback。背景纹理不进 SVG parser；窗口内容不因 HUD 固定而改走另一套 controller。
- **必要回归**：普通窗打开/关闭与 pinned HUD 恰有一次呈现；死亡/终焉/暂停/聊天不发生输入穿透；stale offer、断线迟到回调、资源替换和 parser failure；保留已有 semantic/headless 业务路径，窗口布局不成为 gameplay admission 条件。

#### P5a 快捷使用链接修复（2026-09-14）

- 快捷槽按物品 `instance_id` 保存使用链接；拖起、取消或绑定均不搬动库存，Shift 点击解除链接，右键调用 `use_quick_slot`。相同模板的不同实例不会串绑。
- `ItemTemplate.quick_use` 从物品 TOML 读取，缺省关闭；`is_quick_use_eligible` 只允许已实现的直接自用效果。绑定、开始使用和完成消费都校验资格。定向夹板、经脉药、方块、工具等不进入快捷槽，方块仍可由原技能栏入口绑定。
- `quickslot_config` 携带实例、数量和允许快捷使用的模板列表；`emit_quickslot_config_payloads` 监听库存及绑定变化，用尽后清空所有引用该实例的槽位，保留冷却。Rust、TypeBox、protobuf、Java、bot 消费同一协议。
- 测试调整：删除旧模板扫描顺序、方块自动镜像和物品尺寸枚举测试，这些已不属于快捷使用契约；保留实例不移动、资格拒绝、持久化失败原子性、消耗后 HUD 清空和三端协议回归。未引入旧协议兼容分支。
- 主线同步：合入 `origin/main` 的 `b6eb6751a`，合并提交 `b5b4b6d64`；主线拆出的 player/schema/combat 等测试同步更新实例绑定与模板字段，保留此前 R7 工作区改动。
- 验证：合并后 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、完整 `cargo test` 通过，12,557 项通过、0 失败、6 项忽略；Java 17 `gradle test build` 及补强的 `ProtoServerDataBridgeTest` 通过；Schema 构建、408 份生成文件新鲜度检查与 912 项测试通过；bot 协议测试 569 项通过。
- 联网回归：独立端口、临时存档与 Redis 上运行 `network_quickslot_config`，1 场景通过、0 跳过、0 失败。现有场景增加两格同实例的消费数量推送、耗尽后全部链接清空和冷却保留断言；日志 `/tmp/bong-quick-use-live-verified.log`，完整 Rust 日志 `/tmp/bong-quick-use-rust-full.log`。未替换运行中的 Windows native 客户端和开发服。
- 提交检查点（2026-09-14）：用户要求先提交当前成果。窗口控制、背景与布局持久化已在 `56d5eddef` 提交；容器、装备、HUD 工作台接入与快捷链接修复随本记录提交。前面“未提交”的工作记录是当时状态；本轮仅本地 commit，未 push 或开 PR。P4/P5/P6 剩余验收及 P5b 后续迁移仍按阶段表保留，未以本次提交标成完成。

#### P6b HUD 布局编辑子批次工作记录（2026-09-13）

- 用户追加范围：原 HUD 接入工作台，自定义位置与显隐，底部增加一键还原。`HudWidgetWindows` 将已有 command layer 映射为编辑窗口，复用 `UiWindowManager` 的拖动、尺寸输入、固定、最小化与关闭；`BongHud.workspaceCommands` 读取现有快照，`HudRenderCommand.transformed` 只改变呈现坐标和等比尺寸，不发送领域意图。
- 渲染：`BongHud` 在游戏中应用本地布局，Inspect 中交由窗口内容展示；HUD 编辑窗不在游戏中重复绘制标题和边框。`BongHudRenderer` 注入原 SVG backend。屏幕染色、边缘效果、Toast 和同 layer 的全屏遮罩不随面板移动或消失；窗口内预览使用 scissor，保留原 PNG/SVG/文字资源。旧直接 overlay 和 primitive 删除仍是后续 P6b 工作。
- 控制：`WorkspaceControls` / `workspace-controls.xml` 的 HUD 列表以复选框设置显隐，编辑按钮打开或置顶目标窗并收起列表；仅展开列表不创建所有窗口。Lucide `panels-top-left` / `rotate-ccw` 图标沿用现有风格。还原调用 `UiWindowRuntime.resetLayout`，恢复本地布局与显隐，不关闭 scope 或重建业务 identity，不清除背景、动画设置、库存、装备或制作会话。
- 偏好：`WindowLayoutPreferenceStore` 在原版本配置追加 `hud` 对象，保存屏幕比例位移、等比缩放与显隐。未配置时保持原布局。锁定/最小化不因窗口最小尺寸或 viewport 钳制改写 HUD 几何；从列表恢复最小化窗口保留原 pinned 状态。
- 必要验证：`HudWidgetWindowsTest` 覆盖同 layer 全屏效果保留、锁定不改变默认几何、最小化重开与还原；原 `WindowLayoutPreferenceStoreTest` 扩展磁盘往返和还原，不新增属性镜像测试。`UiHudWindowPreviewScene` / `hud-window-ui-preview.json` 通过真实复选框、编辑、锁定、拖动、关闭和底部还原操作，验证现有业务窗口 scope 保留、编辑按钮不溢出；最低/odd/宽屏与无世界 HUD 渲染共 4 场景 `status=passed, completed=4`，HUD 像素校验包含季节全屏叠色。
- 最终门禁：Java 17 `scripts/build-token.sh gradle test build --offline --console=plain` 通过，5107 条 JUnit、3 条 Fabric GameTest，无失败，进程退出码 0；日志 `/tmp/bong-hud-window-final-gate.log`。首次全量检查发现 `WorkspaceControls.clearChildren` 的既有 R7 调用清单行号漂移，核对调用后更新清单并完整重跑通过。
- 产物：`client/run/hud-window-ui-preview/contact-sheet.png` 为四场景接触表，原始 PNG 和结果标记在同目录，预览日志 `/tmp/bong-hud-window-preview.log`。当前预览使用 Xvfb/Fabric，尚未完成 Windows 联网验收或连续动画验收；本批外观待用户确认，未提交、推送或开 PR，不标整个 P6b 完成。

### P7 — 验收与归档

- **清单**：开头的 Screen/面板逐项填入实际窗口 id、源入口、controller/source/intent、会话 owner、能力与验证结果；更新既有 `screen-inventory.tsv`、`screen-adapters.tsv` 和 HUD inventory 表达新的窗口归属，不新增只锁死类名或数量的测试。Processing/raw XML 等待接线项须先解决或取得明确范围裁剪结论。
- **逻辑**：只保留共享窗口生命周期、身份/权限、制作取消语义、跨窗物品操作、会话失效、关闭异常清理与已发现 bug 的最小回归。沿用 domain intent/handler 的现有契约测试，不给所有窗口复制同样的最小化/resize 测试，不做函数/enum 全覆盖。
- **实机**：复用 `runClientUiPreview` 验证最低 `320x240`、odd `401x241`、wide `683x384`，再选超宽/竖屏和 GUI scale 变化的代表组合；验证窗口主动叠放下的唯一命中、文字/图像比例、尺寸输入、最小化恢复、HUD 固定与背景切换。帧截图证明静态结果，连续操作录制/帧序列检查动画中断、无跳变和输入跟随，不以整图 hash 比对。
- **联调与门禁**：Java 17 在 client 执行 `scripts/build-token.sh gradle test build`；只在消息、时序或副作用跨栈时补/跑对应 `scripts/bot/**`、`ui_c2s_smoke`、`reconnect_state_freshness`。制作中离开工作台、死亡抢占、重连这类流程走真实服务器与客户端，预览 fixture 不替代权威链路。
- **归档**：完成所有子阶段、删除退役局部管理器/Screen 路径，补 `## Finish Evidence`（落点、commit、测试、跨端只读契约与遗留处理），再迁入 finished；不得仅完成 Inspect 就归档整个 R7。

## 8. 吸收清单与已知边界

| finding | R7 处理 |
|---|---|
| `alchemy-screen-fill-overflow` / `alchemy-screen-fill100-eviction` | canonical duplicate；P3/P4 修复并以 geometry fixture pin。 |
| `techniques-tab-scroll-bounce` | P1 reconciler + P5b 功法窗口；不机械 clear/rebuild。 |
| `botany-rkey-backlog-dispatch` | P4 keybind registry；blocked/inactive presses drain，不 replay。 |
| `client-input-keybind-collision` | P4 global registry；T/L/O/U/R 冲突和 vanilla reservation source gate。 |
| `dying-elder-give-dan-input` | effective binding 驱动 HUD；UNKNOWN 显示“未绑定”，不创建第二默认 G。 |
| `trade-offer-first-item-autopick` | P3 typed picker；必须选择 exact `instance_id`。 |
| `client-insight-offer-strand` | P6 exact offerId settlement + transition cancellation。 |
| `v-sparring-invite-screen-hijack` | P6 social policy；无 authoritative combat snapshot 时 fail closed。 |
| `cast-sync-config-window-thread` / `mineral-probe-result-network-thread-ui` | 已由 R6 receive boundary 修复，R7 不重复接线。 |
| `surface-stash-search-hud-label-gap` | 已修复，不重复实现。 |
| `preview-config-dead-server` / `weather-visual-overlay-collapse` | out-of-track，保留各自 owner。 |

## 9. 开放问题（P0R 决策门前需收口）

1. `UiStateSource` 是否立即回调首个 snapshot，还是由 binder 先 snapshot 再订阅？
2. `UiIntentSink` 是否建立一个巨型统一接口，还是按领域拆小接口？
3. `UiBootstrapRegistry` 是否吞并 BongClient 的全部 register call，还是只收编 UI/HUD/keybind 子集？
4. 无生产调用者的 `BongScreenBase` / `DiffListWidget` 是否应删除？
5. vanilla 与 owo 是否共享同一个 Screen controller/view-model 契约？
6. InspectScreen 是否按 tab-first 拆解，是否与 R10 server inventory 同窗口？
7. `ScreenOpenPolicy` 的 passive invite 是否战斗中延迟、通知一次，还是立即丢弃？
8. HUD 是否将 `docs/svg` 设计稿升级为 runtime SVG，以及 NanoSVG/native、tessellation 和 Minecraft GUI 提交边界如何固定？

旧问题在 §9.1/§9.2 收口，原表保留作历史回溯；2026-09-12 的窗口范围与生命周期修订以 §9.3 为准。

## §9.1 决议（pre-P0 contract rebase，2026-08-24）

### #1 State source 首帧语义

**决议**：`UiStateSource.subscribe()` 只通知后续变化，不保证立即回调；`UiStateBinder` 在 client thread 先读取一次 `snapshot()`，再登记 subscription。这样既兼容已有 Store listener，也避免 mount 时重复 patch。

**落点**：`client/src/main/java/com/bong/client/ui/contract/UiStateSource.java`、`UiSubscription.java`；本 plan §4.1、P1。

### #2 Intent interface 形状

**决议**：按领域拆小型 typed sink，不创建 117 方法总接口；sink 只适配现有 sender，并统一返回 local transport result。server acceptance/rejection 只由 S2C Store/receipt 表示。

**落点**：`client/src/main/java/com/bong/client/ui/intent/**`、`network/ClientRequestSender.java`（只读依赖）；本 plan §4.5、§5.2、P3。

### #3 Bootstrap scope

**决议**：registry 只管理 Screen/HUD/keybind；network、render、audio、debug、Iris 保持 `BongClient` 原有顺序。UI registry 以显式 dependency graph 证明顺序和 idempotence，不引入反射。

**落点**：`client/src/main/java/com/bong/client/ui/bootstrap/**`、`BongClient.java:81-165`；本 plan §4.6、§5.3、P2/P6。

### #4 Screen base ownership

**决议**：无生产调用者的 `BongScreenBase` / `DiffListWidget` 已删除，不保留空壳 compatibility host。新 controller/view-model/scope 不得依赖 `BaseOwoScreen`；所有 Screen 统一接入 owo XML host，vanilla Screen 只作为迁移前审计记录。

**落点**：`client/src/main/java/com/bong/client/ui/contract/**`、`ui/adapter/owo/**`；本 plan §4.3、P1/P2。

### #5 Inspect 拆解与 R10 解耦

**原决议（已由 §9.3 扩展）**：采用 tab-first；shell 保留唯一 intake 和交互 arbitration；tab panel 只接 immutable ViewModel + intent callback。**现行**：窗口管理器拥有统一 intake/交互协调，原标签成为独立窗口，Inspect 只作为工作台入口；不与 R10 server inventory 内部文件重排同窗口的边界继续有效。

**落点**：`client/src/main/java/com/bong/client/inventory/InspectScreen.java`；本 plan §7 P5。

### #6 Open policy

**决议**：passive social invite 保留 domain Store；战斗/已有屏时首次同 identity `DEFER_NOTIFY`，重复 `DEFER_SILENT`，空屏且 TTL 有效才 `OPEN`；普通 hotkey 不重放，不排队；Insight 按 exact `offer_id` settlement；system terminal 按优先级抢占。战斗门消费 server-authoritative combat snapshot，快照缺失时 fail closed。

该决议同时冻结 `BLOCK_DROP` 的普通 hotkey 结果（契约向量见
`client/src/test/resources/bong/ui/screen-open-policy.tsv:17-19`，断言见
`client/src/test/java/com/bong/client/ui/R7FoundationContractTest.java:265-266`）；当前
production policy seam 是 `SparringInviteScreenBootstrap.decide(...)`（含 combat fail-closed、
首次 `DEFER_NOTIFY`、重复 `DEFER_SILENT`，见
`client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java:64-119`），其行为回归
见 `client/src/test/java/com/bong/client/social/SparringInviteScreenBootstrapTest.java:231-265`。
Insight lifecycle 必须证明 stale A/duplicate callback 不影响 B；沿用 exact offerId claim/compare-and-clear exactly-once，精确 `offer_id` compare-and-clear
见 `client/src/main/java/com/bong/client/insight/InsightOfferStore.java:93-160`，旧屏回归见
`client/src/test/java/com/bong/client/insight/InsightOfferScreenTest.java:101-120`。

**落点**：`client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java`、
`client/src/main/java/com/bong/client/insight/InsightOfferScreenBootstrap.java`、
`client/src/main/java/com/bong/client/insight/InsightOfferStore.java`、
`client/src/main/java/com/bong/client/ui/ScreenTransitionController.java`；本 plan §4.3、§7 P4/P6。

### #7 网络与 UI 边界

**决议**：R6 的 receive-boundary 是唯一网络线程入口；R7 不改 `BongNetworkHandler.register()`、`ProtoServerDataBridge`、`ServerDataRouter`。UI 只能消费 Store/ViewModel 并调用 typed intent；source gate 阻止 UI 直接依赖 Handler/proto/sender。

**落点**：`client/src/main/java/com/bong/client/BongNetworkHandler.java:108-331`、`network/ProtoServerDataBridge.java`、`network/ServerDataRouter.java`；本 plan §2、§5、P4。

### #8 Semantic surface 与 headless driver

**决议**：server/agent 不再以 XML/HTML/JS 描述 UI；跨端只交换带 `surface_id`、`template_id`、`session_id`、`revision`、immutable view data 和 typed `allowed_actions` 的语义 surface。client 通过本地 owo XML template registry 渲染，bot 通过同一 action registry 和 authoritative receipt 消费；R7 不擅自改现有 wire，raw XML 只作为 legacy compatibility input，真正 wire cutover 由 R6/schema/agent amendment 按 atomic activation 完成。

**调研依据**：legacy proto `UiOpen.xml` 位于 `proto/bong/envelope.proto:2731-2735`；当前 Agent UI 的 raw XML schema 位于 `agent/packages/schema/src/payloads/agent-ui.ts:50-88` 和 `agent/packages/schema/src/server-data.ts:1994-2022`；server 清洗并经专属 JSON channel 下发的路径位于 `server/src/network/agent_ui.rs:415-498`；client 运行时解析入口位于 `client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:136-152,437-448`；本地模板白名单位于 `client/src/main/java/com/bong/client/ui/adapter/owo/OwoXmlTemplateRegistry.java:13-30,61-72`；bot 对当前四字段 raw payload 的验证位于 `scripts/bot/_agent_ui_helpers.py:68-145`。

**落点**：R7 `ui/contract/{surface,headless}/**`、`scripts/bot/` semantic driver；上述 legacy/raw XML 入口只作为 R6/schema/agent amendment 的接入证据，不是新 semantic surface；本 plan §2、§4.7、§5.4、P0R/P3。

### #9 Viewport、缩放与输入坐标

**决议**：公共 UI 只接受 `UiViewport` 的 logical dimensions 和显式 scale metadata；`UiLayoutPolicy` 以约束/布局模式处理 compact/regular/wide，不假设 16:9。physical px 与 MC GUI scale 的转换集中在 owo XML adapter，输入使用同一逆变换；最低 `320x240`、odd aspect、超宽/超窄/竖屏、GUI scale 1-4 和 resize 中间态全部进入 geometry/input contract。现有 physical→logical 证据是 `client/src/main/java/com/bong/client/mixin/MixinMouse.java:100-116`、`client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java:58-69`；现有 scaled viewport/HUD 消费证据是 `client/src/main/java/com/bong/client/BongHud.java:131-143,243-251,528-541`；现有 Screen-level hit-test 耦合证据是 `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:664-710`、`client/src/main/java/com/bong/client/forge/ForgeScreen.java:365-375`、`client/src/main/java/com/bong/client/inventory/InspectScreen.java:2255-2375`。

**落点**：`client/src/main/java/com/bong/client/ui/contract/UiViewport.java`、`UiLayoutPolicy.java`、`ui/adapter/owo/**`；本 plan §4.8、P2/P4/P7。

## §9.2 决议索引

HUD SVG 的范围与历史证据见 `Pre-P0 Decisions`；已实现 backend/真实 layer 沿用，后续全量收口统一归 P6b。详细契约见本文件 HUD SVG 章节及 `ui-svg-hud-contract.tsv`、`ui-svg-hud-inventory.tsv`。

## §9.3 窗口体系决议（2026-09-12）

本节记录用户确定的目标和本轮为落实目标采用的实施约定；新增类型尚未落地，源码锚点用于说明迁移起点。

1. **全局窗口范围**：Inspect 标签与锻造/手搓/制作等共用管理器；现有局部浮窗吸收其去重/置顶行为，不长期保留平行实现。落点：`inventory/InspectScreen.java:57`、`inventory/PackWindowManager.java:24`、`combat/inspect/SkillConfigPanelManager.java:13` → §4.9、P4a、P5。
2. **窗口生命周期独立**：最小化和隐藏工作台保留业务窗口；固定仅控制 HUD 呈现；明确 close 与权威失效各走领域既有契约。落点：`craft/CraftScreen.java:111`、`craft/WorkbenchScreen.java:126`、`inventory/InspectScreen.java:235`、`scroll/ScrollReadScreen.java:207` → §4.9 状态表、P5c/P5e。不会新造服务端多工位会话。
3. **输入和系统优先级**：工作台内编辑和操作，HUD 只展示；WINDOW/OFFER 允许最小化，offer 时限继续推进；SYSTEM 不允许最小化/固定绕过。普通窗口之间 open-or-focus，不再相互替换 Screen；被系统/原版阻挡的热键仍 drop，不能积压重放。落点：`ui/ScreenOpenPolicy.java:88`、`hud/ScreenHudVisibility.java:26`、`ui/ScreenTransitionController.java` → §4.9、P4a/P6a。此项扩展 §9.1 #6 的普通屏互斥，不改变社交权限或终端优先级。
4. **布局和资产**：`n × m` 是 GUI 逻辑宽高，内容按本窗尺寸重排；背景首批宇宙/地形 PNG，本地可换图；动效使用单调时间并允许打断。落点：`ui/adapter/owo/OwoXmlScreenHost.java:62`、`ui/contract/UiViewport.java`、`menu/MainMenuConfig.java:35`、`scripts/images/gen.py:542` → §4.9、P4b。新窗口不引入 HTML、外部 GUI provider、依赖升级或旧布局兼容层。
5. **未接线功能不冒充完成**：Processing 没有实际构造调用，raw XML 退出需要跨端 owner；先登记范围与依赖。落点：`processing/ProcessingActionScreen.java:27`、`ui/UiOpenScreens.java:53`、`agentui/AgentUiScreen.java:136` → 开头范围表、P5d/P6a/P7。本轮不据旧 skeleton 推断其他服务端 bug 已经成立或已修复。

本轮已收口功能范围；视觉细节在 P4b Round 2 接触表中由用户确认。不得把该视觉确认扩展为重新询问已经确定的多窗、HUD 固定或全客户端接入目标。

## 10. 实施工作流

### 10.1 适用边界

纯 client 窗口/adapter 重构与视觉资产制作；允许本 plan 的 runtime SVG、工作台 PNG、XML 模板、截图与动画帧序列。gimage 原稿/prompt 留本地，确认后的资产才接入。视觉内容执行三轮：Round 1 初稿 → Round 2 同取景接触表供用户确认 → Round 3 修订与实机验收，终轮 commit 带 `<PROMISE>`；不能用模型自评代替 Round 2。后续 native parser 独立交付，不产出 NBT/worldgen/实体模型，不改正典、图书馆、schema shape 或依赖版本。每个逻辑单元中文 atomic commit，带真实 `Model:` trailer。

### 10.2 多 PR 依赖顺序

P0R-P3 和已有 SVG/XML 切片是历史已完成批次，不重开。后续按下列顺序推进，每行是一个 review 边界；体量过大时允许按完整功能窗继续拆 PR，但保持同一 plan 和子阶段，不把入口/业务 close 语义留到下一 PR。

1. **P4a / 窗口核心与首窗**：库无关管理器、scope/identity/input/布局、XML 内容 adapter，物品详情真实入口，验证无 Screen 时的呈现能力。
2. **P4b / 窗口体验与视觉**：最小化、HUD 固定、尺寸输入、连续动画、两类背景/换图、本地偏好；真实首窗完成三轮视觉验收。
3. **P5a / Inspect 工作台与库存**：装备/背包/容器、跨窗拖放与快捷槽；收编局部 PackWindowManager。
4. **P5b / 修仙与功法**：技艺/经脉/身份/化虚/概览、功法和配置窗；共享状态，删除旧 tab/配置浮窗 owner。
5. **P5c / 手搓与锻造**：Craft、Workbench、Forge；会话在最小化/回到游戏后继续，显式取消不变。
6. **P5d / 其他工位**：Alchemy、Repair、ForgeCarrier、ZhenfaLayout、Lingtian；Processing 适配与领域接线依赖核验。
7. **P5e / 交互窗口**：Loot、NPC、交易/切磋、卷轴、法宝、棺木；identity 与会话结束完整回归。
8. **P6a / 受控与系统界面**：Insight、AgentUi、DynamicXml、Death/Terminate/MainMenu，全局切换、时限与优先级，等待必要 wire owner 的交付。
9. **P6b / HUD 与旧路径收口**：固定窗和现有 HUD 的可见性/输入协调，剩余 SVG layer、bootstrap 与 primitive 删除。
10. **P7 / 全量验收和归档**：范围表、必要回归、实机动画/分辨率矩阵、接线依赖与删除门闭环。

前一实施 PR 的最终 HEAD 通过受影响门禁、review、必要 e2e 并合入后推进下一批。P4 不再要求先将所有独立 Screen XML 化才允许 P5；各窗在所属批次直接迁移到目标结构。新 HEAD 按实际变更重验并绑定证据，不能引用旧 SHA 冒充当前通过。P4a 当前已开始实施，剩余验收见 P4a 工作记录。

### 10.3 每个 PR 的闭环门

1. 在独立 worktree/branch 实施，不修改脏 main checkout，不越界改 R2/R6/server owner 文件；semantic wire amendment 未合入前，R7 只做 declared/test-only projection，不接新 production traffic。
2. `git fetch origin` 后紧邻 `git merge origin/main`；merge 触及受影响文件即重跑该阶段全部测试。
3. Client 使用 Java 17，在 client 目录执行 `scripts/build-token.sh gradle test build`，复用仓库构建互斥；纯文档批次检查 diff、引用与范围一致性，不编造运行时测试结果。
4. 每个阶段最终 SHA 启动 explicit-worktree、read-only fresh-context validator；validator 必须回报 HEAD SHA 对拍。
5. source/contract gate 必须从 production source 派生集合，不用手写数量假绿；生产代码不得调用 test reset seam；bot UI 证据必须包含真实 wire、action id、request/revision/receipt 对拍，不能用像素或截图替代。
6. push 后确认 PR head 等于已验证 SHA，等待 Kody 自动 review，检查根评论和 inline threads；不发送 `/review`。修复后验证新 HEAD，需明确复审时才用 `@kody review --force`，按当前仓库 review gate 收口。
7. R7 实施 agent 不 merge；orchestrator 在 review/e2e 绿后按 plan 顺序收口。

### 10.4 PR 实施上下文隔离

R7 每个 PR 使用独立实施上下文；主线只调度、读取结论、等待 review/e2e 和收口，不在同一上下文连续堆叠多个 PR：

```text
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "实现本 PR 的限定阶段，先读本 plan 对应章节和 ownership；完成代码、测试、validator、PR 后只回报结论。\nultrathink"
)
```

实施 agent 不等待 review；review finding、返工和新 HEAD 的完整门禁由调度上下文重新派发。每个 PR 仍使用独立 worktree/branch，禁止共享脏工作区或绕过 Java 17/global-lock gate。

### 10.5 终态验收

- `ui/contract` 无 owo/vanilla/widget import；UI source gate 无 network Handler/proto/sender 越权 import，server/agent 语义 surface 无 XML/HTML/JS/DOM/像素坐标。
- 范围表中 29 个原 Screen 类、Inspect 功能页及两套旧浮窗全部有迁移/合并/系统边界的证据；不以最终类数量验收。无第二套普通窗口管理器，无独立 Screen 跳转绕过管理器，无未说明的 raw XML/未接线功能例外。
- Store subscription close、disconnect cleanup、late callback、跨 session freshness 全部通过；R2 registry 仍是唯一断线清理入口。
- `UiDriver` 与 Java client 共享 action registry、参数校验、`ClientRequestProtocol` 编码和 authoritative receipt；bot semantic roundtrip 能覆盖 UI 功能而不依赖渲染/输入设备；local transport accepted 与 server result 分离。
- `UiViewport`/`UiLayoutPolicy` 在最低/odd/超宽/竖屏代表矩阵中验证内容不溢出、图像比例正确、文字可读、顶层唯一命中和坐标往返；窗口主动叠放合法。最小化/恢复/尺寸输入/拖动/固定 HUD 不复制订阅或触发业务取消，系统界面不会被窗口或输入穿透。
- gimage 工作台背景、换图和窗口动画通过 Round 2 人工接触表与 Round 3 实机连续操作；本地布局不携带旧会话身份，坏配置/图片回退后仍可关闭、恢复与操作窗口。
- `BongClient` UI/HUD/keybind module registry 的 owner/dependency/order/idempotence pin 全绿；network/render/audio/debug registration 未被误收编。
- 当前 inventory 中的 HUD layer/planner/直接 overlay 均经同一 `HudRenderBackend`；固定功能窗的 owo adapter 与之统一协调可见性和顺序。截图证明 framebuffer 非空、bounds/alpha/layer order 正确；旧 primitive DrawContext 分支、`renderSurface` 与生产 fallback 已删除。
- Java 17 full gate、`ui_c2s_smoke`、`reconnect_state_freshness` 及必要 `runClient` 真实 Screen 回归通过。

终态补充 `## Finish Evidence`：阶段落点、关键 commit、测试命令/数量、server/agent/client symbol 对拍、遗留项；全部阶段完成后迁入 `docs/finished_plans/plan-refactor-client-ui-base-v1.md`。
