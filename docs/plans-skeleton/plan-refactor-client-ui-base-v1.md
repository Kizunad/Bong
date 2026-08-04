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
- **跨仓库契约**：本轨不定义 wire，只消费 A-CS A-row、master M-02/M-07/M-09/M-10 与 R1 S-row。Idle/no-session 初次 `CraftOpen` target 为 `Handcraft` 或 retained `Workbench { workbench_key }`；server identity 尚未 hydrate 时进入 client-local `OpenPending`（不是 gameplay phase），普通 close 置一次性 `pause_when_hydrated`，不得发送缺 identity 的 `CraftPause`；匹配 `Running` hydration 到达后只发一次带 `session_key + generation` 的 `CraftPause` 且不重开 Screen。已 hydrate close 发 `CraftPause`，explicit cancel 发 `CraftCancel`，仅匹配 server-hydrated `Paused` session 发一次 `CraftResume`。R1 `HandoffPreparing`/`Ended` 或 stale identity 均不发 Resume；delivery Pending/InFlight/DeadLetter 是 obligation 状态，R7 不把它们存为 resumable gameplay phase。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：92 处 fill(100) 全量分类（根节点合法/子节点顶飞）；28 Screen 普查；冻结基类 API 与四个共享组件。
- ⬜ P1 基础组件落地：BongScreenBase/DiffListWidget/KeybindRegistry/ClientThreadMarshal/ScreenOpenPolicy 上线；keybind 冲突全数改绑（T/L/O/U/G 簇）。
- ⬜ P2 Screen 迁移批次 A：炼丹/锻造/手搓/交易屏迁基类，随迁修复 fill(100)/clearChildren；Craft Screen 接线为 Idle→带 `Handcraft` 或 retained `Workbench { workbench_key }` 的 `CraftOpen`，未 hydration close→`OpenPending.pause_when_hydrated`，匹配 Running hydrate 后恰一次 `CraftPause`，已 hydration close→`CraftPause`，显式取消→`CraftCancel`，匹配 paused hydrate→恰一次 `CraftResume`。client 单测锁住 target key roundtrip、四种 intent 不互相替代、不可恢复 phase/stale identity 不发 Resume，以及 `Open→close→Running hydrate→Pause once`、duplicate hydrate 不重复、stale/mismatched hydrate 不消费较新 latch、disconnect/session clear 丢弃 latch、Paused/HandoffPreparing/Ended hydrate 只清 latch且不重开/误发。
- ⬜ P3 InspectScreen 拆解：按 tab/section 拆组件文件（body/container/tooltip 已有雏形），行为不变。
- ⬜ P4 Screen 迁移批次 B + 网络线程 marshal 强制（handler 层静态检查/测试）+ 删旧。
- ⬜ P5 验收 + 吸收 plan 批量归档。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：spirit-treasure-chat-key-conflict。
skeleton：alchemy-screen-fill100-eviction 与 alchemy-screen-fill-overflow（疑似重复开出，P0 合并核实）、techniques-tab-scroll-bounce、botany-rkey-backlog-dispatch、cast-sync-config-window-thread、client-input-keybind-collision、dying-elder-give-dan-input、lingtian-advancements-key-conflict、mineral-probe-result-network-thread-ui、preview-config-dead-server、v-sparring-invite-screen-hijack、trade-offer-first-item-autopick、surface-stash-search-hud-label-gap、hud-qi-radar-mainpath-regression、weather-visual-overlay-collapse（叠层去重键）、client-insight-offer-strand（client 弹窗排队部分）。

## 文件所有权与边界

- 独占：client 全部 Screen/`ui/`/`hud/` 结构性改动、keybind 注册、`InspectScreen.java`。
- 不碰：store 生命周期接口（R2 域，本轨消费）；`network/` 桥与 router（R6 域——marshal helper 由本轨提供、在 handler 注册处的接线与 R6 协调）；server 一切。
- 依赖：本轨只引用 master M-02/M-07/M-08/M-09/M-10；R2 Store、R6 machinery、R4 gate 与 R1 session 的 production 接缝在 master atomic activation row 完成前只能提交 contract pins，不宣称端到端可达。

## 验收

bot 测不到 client 渲染，本轨主验收 = client 单测（基类生命周期 pin、DiffListWidget 滚动保持、keybind 注册表无冲突断言、marshal 强制扫描、CraftOpen/CraftPause/CraftCancel/CraftResume 四条 intent producer pin，以及 close-before-hydration latch 全矩阵）+ `./gradlew runClient` 人工过一遍五大屏。bot 配合：`ui_c2s_smoke`（各屏的 C2S 动作链路照常可达，防拆解断线）。

## 开放问题（pre-P0 收口）

1. InspectScreen 拆解粒度（按 tab 还是按 section）；拆解与 R10 server 侧 inventory 拆分是否同窗口进行。
2. ScreenOpenPolicy 的排队语义（战斗中挂起邀请到何时弹出）——涉及玩法体验，需人工拍板。
3. `OpenPending` 仅是等待 server identity 的 client-local latch，不进入 A-06/R1 phase enum；R2/R6 Store 先拒绝 stale/mismatched hydration，latch 同时禁止 unresolved 期间发送第二个 `CraftOpen`。matching hydration one-shot、typed rejection 与断线清理语义已由 P2 acceptance 冻结；latch 不按本地 timeout 自行丢弃，也不得以 Cancel 替代普通 close。
