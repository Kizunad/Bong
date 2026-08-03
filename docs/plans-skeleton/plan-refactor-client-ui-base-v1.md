# plan-refactor-client-ui-base-v1 — Client UI 公共基类 + InspectScreen 拆解 + 输入/线程纪律（重构轨 R7）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：给 28 个各写各的 owo Screen 建公共基类（store 订阅/tick 刷新/关闭清理/礼貌抢屏），diff-then-patch 列表组件化，keybind 注册表防冲突，网络线程→client 线程 marshal 强制化，并拆掉 4647 行的 `InspectScreen`——owo 布局/输入/线程整簇（16+ 份 plan）收口。

## 现状证据（2026-07-27 侦察）

- 15 个 `BaseOwoScreen<FlowLayout>` 直接子类，无项目公共基类；每个 Screen 各写订阅/刷新/清理。
- `Sizing.fill(100)` 92 处（20 文件）——顶飞兄弟节点反模式高发（炼丹屏两份重复 skeleton 就是它）；`clearChildren()` 15 处（10 文件），`CraftRecipeListWidget.java:110-142` 已有 diff-then-patch 修复范本（id 序列不变则原地更新），但 NpcTradeScreen/InspectScreen 未推广。
- `InspectScreen.java` 4647 行是全 client 最大怪物文件。
- 输入：默认键冲突多起（T 撞聊天、L 撞进度屏、O/U 双派发、给丹 G 未绑定）——无集中 keybind 注册表冲突检查。
- 线程：网络线程直触 UI/HUD/SFX 多起（cast-sync 配置窗被网络线程关闭、探矿回执网络线程直触 HUD）——无 marshal 强制。
- 抢屏：邀请每 tick 强制顶屏（v-sparring-invite-screen-hijack）——无"礼貌打开"协议。

## 接入面

- **进料**：R2 的 `SessionScopedStore`（Screen 订阅的一律是会话态 store）；`ServerDataRouter` handler（一律经 client-thread marshal 投递 UI）。
- **出料**：Screen/HUD 展示；HUD 纪律沿用既有 memory 约束（未解锁隐藏不灰掉、沉浸式极简）。
- **共享类型**：新 `BongScreenBase`（生命周期 + 订阅 + 关闭清理）、`DiffListWidget`（推广 craft 范本）、`BongKeybindRegistry`（注册时冲突检测 + 测试期断言）、`ClientThreadMarshal` helper、`ScreenOpenPolicy`（礼貌抢屏：战斗中/已有模态时排队）。
- **跨仓库契约**：本轨不定义 wire；消费 R6 冻结的 `CraftOpen`/`CraftPause`/`CraftResume` 与 S2C session-state payload。Craft Screen 首次进入/点击继续生产发送 `CraftOpen`，关闭只发送 `CraftPause`，显式取消按钮只发送 `CraftCancel`，重开后由 server state hydrate 决定是否发送 `CraftResume`。三种新 intent 均必须回显 server-hydrated 的 `session_key` 与 `generation`，不得由 client 省略、默认或自行生成。R7 P2 client contract pin 覆盖四条 production intent：首次打开→`CraftOpen`、关闭→`CraftPause`、显式取消→`CraftCancel`、匹配的 paused session hydrate→恰好一次 `CraftResume`；idle/no-session、terminal、`AwaitingDelivery`、`DeliveryPending`、missing/stale/mismatched session key 或 generation hydrate 均不得发送 `CraftResume`。R6 P1 只冻结并验证 wire/bridge/store 的字段与负向身份契约；匹配 paused hydrate 的 production Resume producer 留在本轨 P2，并依赖总纲 `plan-refactor-master-v1.md §3 Wave 2` 的 R6 + R4 + R2 前置。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：92 处 fill(100) 全量分类（根节点合法/子节点顶飞）；28 Screen 普查；冻结基类 API 与四个共享组件。
- ⬜ P1 基础组件落地：BongScreenBase/DiffListWidget/KeybindRegistry/ClientThreadMarshal/ScreenOpenPolicy 上线；keybind 冲突全数改绑（T/L/O/U/G 簇）。
- ⬜ P2 Screen 迁移批次 A：炼丹/锻造/手搓/交易屏迁基类，fill(100) 顶飞点与 clearChildren 回弹点随迁修复；Craft Screen 完成生产接线：首次进入/点击继续→携带 hydrated `session_key`/`generation` 的 `CraftOpen`、close→携带同一 identity/version 的 `CraftPause`、独立取消按钮→`CraftCancel`、仅对匹配的 server-hydrated paused session reopen→携带同一 identity/version 且恰好一次 `CraftResume`，并以 client 单测锁住四条 intent 不互相替代及 idle/terminal/`AwaitingDelivery`/`DeliveryPending`/stale/mismatched hydration 不发 Resume。
- ⬜ P3 InspectScreen 拆解：按 tab/section 拆组件文件（body/container/tooltip 已有雏形），行为不变。
- ⬜ P4 Screen 迁移批次 B + 网络线程 marshal 强制（handler 层静态检查/测试）+ 删旧。
- ⬜ P5 验收 + 吸收 plan 批量归档。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：spirit-treasure-chat-key-conflict。
skeleton：alchemy-screen-fill100-eviction 与 alchemy-screen-fill-overflow（疑似重复开出，P0 合并核实）、techniques-tab-scroll-bounce、botany-rkey-backlog-dispatch、cast-sync-config-window-thread、client-input-keybind-collision、dying-elder-give-dan-input、lingtian-advancements-key-conflict、mineral-probe-result-network-thread-ui、preview-config-dead-server、v-sparring-invite-screen-hijack、trade-offer-first-item-autopick、surface-stash-search-hud-label-gap、hud-qi-radar-mainpath-regression、weather-visual-overlay-collapse（叠层去重键）、client-insight-offer-strand（client 弹窗排队部分）。

## 文件所有权与边界

- 独占：client 全部 Screen/`ui/`/`hud/` 结构性改动、keybind 注册、`InspectScreen.java`。
- 不碰：store 生命周期接口（R2 域，本轨消费）；`network/` 桥与 router（R6 域——marshal helper 由本轨提供、在 handler 注册处的接线与 R6 协调）；server 一切。
- 依赖：R2 P1 先合（基类要绑 `SessionScopedStore`）；Craft Screen 接线还需 R6 `CraftOpen`/`CraftPause`/`CraftResume` 契约与 R4 production handler/gate 先就绪，R1 craft adapter 不得在本轨 P2 合入前宣称 close/pause/reopen/resume 端到端可达；与 R6 在其他 handler 投递点的接缝于 P4 前对齐。

## 验收

bot 测不到 client 渲染，本轨主验收 = client 单测（基类生命周期 pin、DiffListWidget 滚动保持、keybind 注册表无冲突断言、marshal 强制扫描、CraftOpen/CraftPause/CraftCancel/CraftResume 四条 intent producer pin）+ `./gradlew runClient` 人工过一遍五大屏。bot 配合：`ui_c2s_smoke`（各屏的 C2S 动作链路照常可达，防拆解断线）。

## 开放问题（pre-P0 收口）

1. InspectScreen 拆解粒度（按 tab 还是按 section）；拆解与 R10 server 侧 inventory 拆分是否同窗口进行。
2. ScreenOpenPolicy 的排队语义（战斗中挂起邀请到何时弹出）——涉及玩法体验，需人工拍板。
