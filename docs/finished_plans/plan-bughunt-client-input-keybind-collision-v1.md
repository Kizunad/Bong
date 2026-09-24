# plan-bughunt-client-input-keybind-collision-v1（骨架）

> **骨架（草案）**。一句话主题：client 端**默认键位唯一性约束失效**，同一物理键会同时驱动两条独立 bootstrap 路径，已确认至少两组真实冲突：`O` 同时绑定 `IdentityPanelScreenBootstrap` 与 `VoidActionScreenBootstrap`，`U` 同时绑定 `ForgeScreenBootstrap` 与 `ExtractInteractionBootstrap.cancelKey`。结果不是“哪个先注册就赢”，而是**一次按键触发两条 `wasPressed()` 消费链各自独立派发**，造成 UI 抢屏与交互误触发。

> 立项动机：本轮只审 client input / keybind / interaction input 路径，避开已知的 G 键距离漂移、silent signal runtime bridge、movement interaction。审计发现这里不是单点 typo，而是**项目级输入约束回归**：仓库已经在 `CombatKeybindings` 中记录过“同键双绑会双触发”的前车之鉴并修过一次，但当前 `O/U` 又重新引入同型问题；同时测试只守 `G`，没有全局默认键去重门。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 默认键位冲突导致双派发（`O` 抢屏 / `U` 误触发撤离取消） | fix_pr | ✅ 2026-08-26 |

## P0 — 默认键位冲突导致双派发（`O` 抢屏 / `U` 误触发撤离取消）

- **现象**：`client/src/main/java/com/bong/client/identity/IdentityPanelScreenBootstrap.java:15,31-50` 把身份面板默认绑到 `GLFW_KEY_O`；`client/src/main/java/com/bong/client/cultivation/voidaction/VoidActionScreenBootstrap.java:26-39` 也把化虚行动面板默认绑到 `GLFW_KEY_O`。同理，`client/src/main/java/com/bong/client/forge/ForgeScreenBootstrap.java:27-46` 把锻炉 UI 默认绑到 `GLFW_KEY_U`，而 `client/src/main/java/com/bong/client/tsy/ExtractInteractionBootstrap.java:26-45` 把 TSY 撤离取消也默认绑到 `GLFW_KEY_U`。这四处都是独立 `KeyBinding` 实例、独立 tick listener，没有共享仲裁层。
- **复现路径 A（`O` 抢屏）**：正常进世界后按一次 `O`。`BongClient` 在 `client/src/main/java/com/bong/client/BongClient.java:99-103` 先后注册 `IdentityPanelScreenBootstrap.register()` 与 `VoidActionScreenBootstrap.register()`；两边各自 `while (keyBinding().wasPressed())`，且都只判断“当前 screen 不是我自己”就直接 `client.setScreen(...)`。因此一次 `O` 会产出**两次开屏请求**，默认键位语义不再单义，玩家无法稳定预测自己会被带到哪个界面。
- **复现路径 B（`U` 误触发）**：站在 TSY 裂口旁按 `Y` 开始撤离，使 `ExtractStateStore.snapshot().extracting()==true`；此时按一次 `U`。`ExtractInteractionBootstrap` 会发送 `ClientRequestSender.sendCancelExtract()`，同时 `ForgeScreenBootstrap` 也会把 `ForgeScreen` 打开。也就是说，**玩家想取消撤离时会额外弹出锻炉 UI**，而且这个 UI 打开不要求正站在锻炉旁。
- **根因链路**：
  1. 仓库没有中心化“默认键位注册表”或统一冲突检测，默认键直接散落在各 bootstrap 内硬编码。
  2. `client/src/test/java/com/bong/client/input/NoDuplicateDefaultGKeybindingTest.java:13-24` 只扫描 `GLFW_KEY_G`，约束是“Only InteractionKeybindings may default an environment action to G”；**它不是通用去重测试**，天然漏掉 `O/U`。
  3. 仓库自己已经承认“同键双绑 = 两个 `KeyBinding.wasPressed()` 都会触发”：`client/src/main/java/com/bong/client/combat/CombatKeybindings.java:57-61` 记录旧版 `V` 冲突时，“单次按 V 两个 KeyBinding.wasPressed() 都触发”，所以才把 `jiemai` 改成默认未绑定。当前 `O/U` 与那次是同一类型回归，不是理论猜测。
- **这个 bug 对实际游玩体验的影响**：
  - `O` 冲突会让“身份面板”和“化虚行动面板”这两个都带 progression / 决策信息的入口**抢同一个默认键**。新玩家按提示键进入身份面板时，可能被另一个界面覆盖；老玩家也很难建立稳定肌肉记忆。
  - `U` 冲突发生在 TSY 撤离这种高压时刻。玩家本意是“立刻取消撤离、恢复移动/观察”，结果客户端还会**平白弹出锻炉 UI**，遮挡世界视野并打断操作节奏，属于实战中的误触发。
- **建议修复范围 / 模块**：优先收口 `IdentityPanelScreenBootstrap`、`VoidActionScreenBootstrap`、`ForgeScreenBootstrap`、`ExtractInteractionBootstrap` 与 `NoDuplicateDefault*KeybindingTest`。修复建议分两层：
  1. **立即止血**：给 `O/U` 冲突中的一侧改成未占用默认键或默认 `UNKNOWN`，对齐 `CombatKeybindings` 处理旧 `V` 冲突的做法。
  2. **根治**：新增通用默认键唯一性测试，扫描整个 `client/src/main/java/com/bong/client` 的 `GLFW_KEY_*` 默认绑定，允许白名单式例外（如确有仲裁层的保留键），避免以后只守 `G`、别的键继续回归。
- **验收抓手**：至少补 4 组 pin。1) `O` 默认键在全仓只能命中一个“开独占屏幕”的 bootstrap。2) `U` 默认键不会在 `extracting()==true` 时同时触发 `sendCancelExtract()` 与 `ForgeScreen`。3) 若确实保留任何重复默认键，必须有像 `BotanyHudBootstrap.shouldCaptureSpellVolumeKey()` 这种显式仲裁测试证明不会双派发。4) 新增通用 keybinding 去重测试，后续再出现 `GLFW_KEY_*` 双绑会直接红。

## 反方裁决摘要

> 当前会话**没有可用 subagent / delegate 工具**可再开外部怀疑者，以下两轮为主代理退化处理，但都给出明确反方论点与驳回理由，而不是口头“感觉像 bug”。

1. **Round 1 反方论点**：“也许 Minecraft/Fabric 对同一物理键只会让一个 `KeyBinding` 变成 pressed，后注册的监听器未必还能看到事件。”
   **驳回理由**：仓库现成证据已否定这点。`CombatKeybindings.java:57-61` 白纸黑字记录旧版 `V` 冲突时“单次按 V 两个 `KeyBinding.wasPressed()` 都触发”，说明同键双绑在本项目里不是假设，而是历史真事故。
2. **Round 2 反方论点**：“就算 `O/U` 重复，可能也因为额外门禁而无害，例如其中一条路径会检测当前模式/当前 screen，从而自然退化成 no-op。”
   **驳回理由**：我逐条核了门禁。`IdentityPanelScreenBootstrap` 与 `VoidActionScreenBootstrap` 都只排除“自己已经是当前 screen”，没有互斥对方；`ForgeScreenBootstrap` 只排除“当前已经是 ForgeScreen”，`ExtractInteractionBootstrap.cancelKey` 只看 `extracting()`。这与 `BotanyHudBootstrap.shouldCaptureSpellVolumeKey()` 那种**显式仲裁**完全不同，所以 `O/U` 不是“有保护的复用键”，而是无仲裁双派发。

## 开放问题

1. `O` 的最终归属应是身份面板还是化虚行动面板，需要按产品优先级裁决；另一侧是改新默认键还是改 `UNKNOWN` 等玩家自绑。
2. `U` 冲突里，锻炉 UI 是否本就不该提供“随时热键打开”的全局入口；若答案是否定的，修复可能不只是改默认键，还要收紧 `ForgeScreenBootstrap` 的上下文门禁。
3. 是否顺手把“默认键唯一性”抽成一份通用测试基建，覆盖 `O/U/G` 之外所有 `GLFW_KEY_*`，避免下次又靠 bug-hunt 才发现。

## 审计来源

bug-hunt 定点轮（worktree `bughunt-loop-20260705-br`，范围只看 client input / keybind / intent-adjacent 路径）。方法：全仓 grep 默认键位定义 → 复核 `BongClient` 注册顺序 → 回读既有计划与历史修复注释，排除已知 G 键问题和带显式仲裁的保留键后，留下这条 **report-only** 高置信结论：**client 默认键位没有全局唯一性约束，`O/U` 已经形成真实双派发 bug。**

## Finish Evidence

### 落地清单

- **P0 默认键位修复**：`client/src/main/java/com/bong/client/forge/ForgeScreenBootstrap.java` 将 Forge 默认键改为 `UNKNOWN`；`client/src/main/java/com/bong/client/tsy/ExtractInteractionBootstrap.java` 保留撤离取消 `U` 并同步修正启动日志；Identity 面板保留 `O`，VoidAction 面板改为 `UNKNOWN`，从默认物理键层面消除两组无仲裁双派发。
- **P0 存量配置迁移**：`client/src/main/java/com/bong/client/ui/BongKeybindRegistry.java` 在 `CLIENT_STARTED`、首个 Forge tick 消费前按完整 translation key 定位存量绑定；仅将仍为旧默认 `U` 的 Forge 配置迁移到 `UNKNOWN`，通过 `GameOptions#setKeyCode` 持久化并调用 `KeyBinding.updateKeysByCode()` 刷新物理索引，自定义键与已为 `UNKNOWN` 的配置保持不变。`client/src/main/java/com/bong/client/input/KeybindMigrationPersistence.java` 以 `hasCompleted` / `markCompleted` 隔离 marker 文件实现，UI registry 与 Forge bootstrap 不再暴露 `Path` / `markerFile` 存储细节。
- **P0 全局契约**：新增 `client/src/test/java/com/bong/client/input/DefaultKeybindingUniquenessTest.java`，扫描 direct `KeyBinding` 与 `BongKeybindRegistry.BindingSpec`，覆盖所有默认键表达式、`UNKNOWN` fail-closed、`O/U` 唯一归属，并为唯一保留的 R 键重复绑定锁定 `BotanyHudBootstrap.shouldCaptureSpellVolumeKey()` 仲裁。
- **P0 迁移契约**：`client/src/test/java/com/bong/client/ui/BongKeybindRegistryTest.java` 覆盖旧 `U` 迁移、自定义键保留、`UNKNOWN` 幂等，以及迁移前后物理 `U` 对 Forge `wasPressed()` 的真实派发差异；`client/src/test/java/com/bong/client/input/KeybindMigrationPersistenceTest.java` 覆盖跨实例持久化、多个 marker 保留与非法 ID；`R7KeybindProductionMigrationTest` 锁定 `CLIENT_STARTED` 注册早于 Forge `END_CLIENT_TICK` 消费及 UI 层无文件路径泄漏。
- **P0 R7 对拍**：更新 `client/src/test/resources/bong/ui/keybind-migration.tsv`、`keybind-production-sites.tsv` 与 R7 source digest baseline；测试全部位于 `client/src/test/java/**`，未改 #2098/#2099 资产、动画或工具链。

### 关键 commit

- `c9f78dbca` · 2026-08-26 · 将 bughunt skeleton 升格为 active plan。
- `8aab1c225` · 2026-08-26 · 修复 O/U 默认键冲突归属。
- `3166d32e5` · 2026-08-26 · 补齐全局默认键唯一性契约测试。
- `52f088602` · 2026-08-26 · 修正 O/U 屏幕断言契约。
- `d9712e0c8` · 2026-08-26 · 更新 R7 生产源冻结摘要。
- `5c852ce7b` · 2026-08-26 · 修正 TSY 撤离键位日志。
- `d0b8466dd` · 2026-08-26 · 同步日志变更后的 R7 生产源冻结摘要。
- `f11f2bca0` · 2026-08-26 · 迁移 Forge 存量 U 键位配置。
- `9490b5dbf` · 2026-08-26 · 补齐 Forge 存量键位运行时索引测试。
- `8da381c9f` · 2026-08-26 · 同步 Forge 键位迁移的 R7 源码摘要。
- `fa6bd905a` · 2026-08-27 · 合并最新主线并同步联合 R7 源码基线。
- `507f39228` · 2026-08-27 · 补齐 Forge 键位迁移一次性版本标记及回改 U 测试。
- `5d41219a4` · 2026-08-27 · 同步键位迁移版本标记的 R7 基线。
- `f2db1ce59` · 2026-08-27 · 按 inline review 初步抽离键位迁移持久化边界。
- `81baef679` · 2026-08-27 · 将持久化服务正名为非 session store，并同步 R7 冻结基线。
- `1d90c7899` · 2026-08-27 · 合并最新 `origin/main`（`7623bc2f8`），主线变更未触及 client。

### 测试结果

- `JAVA_HOME=/home/serverkizuna/opt/jdk-17.0.19+10 PATH=/home/serverkizuna/opt/jdk-17.0.19+10/bin:$PATH ../scripts/build-token.sh gradle test build`：持久化边界返工后的 `4953 tests`、failures `0`、errors `0`，Gametest `3/3`，`BUILD SUCCESSFUL`；合并最新主线后再次运行通过。
- `git fetch origin && git merge origin/main`：紧邻合并最新 `origin/main`（`7623bc2f8`）；带入内容仅为 `modelScript/` 与根 `CLAUDE.md`，未触及 client，合并后完整 Java 17 gate 复验通过。
- fresh-context、read-only validator：对 merge HEAD `1d90c789998d81f67779058d171ffcfdda48d48c` PASS，重新核验默认键唯一性、一次性存量迁移、`KeybindMigrationPersistence` 边界、R2 store 命名约束和 R7 digest；本 Finish Evidence commit 后继续对最终 SHA 复验。
- GitHub workflow `33030064148`：对 review 返工前 HEAD `3a233437065bb0ebc543ca56e4c57568c0802bab` 的 preflight/schema/agent/client/server-test/build-release/smoke/chat-window/bot-e2e (1)/(2) 全部通过；当前持久化边界返工 HEAD 推送后由同一 PR 重新取得最终 workflow 证据。

### 跨仓库核验

- **client**：`BongKeybindRegistry.BindingSpec`、`KeybindMigrationPersistence`、`DefaultKeybindingUniquenessTest`、`IdentityPanelScreenBootstrap`、`VoidActionScreenBootstrap`、`ForgeScreenBootstrap`、`ExtractInteractionBootstrap` 均命中，覆盖注册、默认键扫描、一次性持久化与实际 `wasPressed()` 路由。
- **server / agent / schema**：本 plan 仅涉及 Fabric client 输入与测试契约，不修改 server、agent、schema、Redis key 或 wire payload。

### 遗留 / 后续

- 玩家仍可在 Minecraft Controls 中手动把多个动作绑定到同一键；本修复约束的是项目默认绑定，运行时手动配置冲突不在本 plan 范围。
- R 键重复绑定依赖现有明确仲裁层并由专测锁定；新增重复默认键必须先建立同等级仲裁与专属测试。
