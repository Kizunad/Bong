# plan-bughunt-quick-slot-function-key-collision-v1（Skeleton）

## Bug 摘要

默认键位中 `F6/F7` 同时被两套正式入口占用：

- `CombatKeybindings` 把 `F1-F9` 注册为 9 个快捷使用槽，因 `GLFW_KEY_F1 + i`，第 6/7 槽自然落在 `F6/F7`。
- `HudImmersionControls` 把 HUD 沉浸开关默认绑到 `F6`。
- `NpcInteractionLogControls` 把 NPC 交互日志默认绑到 `F7`。

这不是视觉标签冲突，而是输入入口冲突：快捷槽命中后会走 `QuickUseSlotStore`、本地 cast 反馈，并发送 `use_quick_slot` C2S。MC/Fabric 同物理键绑定存在单值映射抢占或仓库历史记录的双消费风险，因此默认配置下至少一侧入口会变得不可预测或不可达。

## 实际游玩体验影响

玩家把丹药、符具、物品能力或其它快捷项放到第 6/7 个快捷使用槽后，按 HUD 标注的 `F6/F7` 期望触发对应物品，却可能被 HUD 沉浸开关或 NPC 交互日志抢同一默认键；反过来，如果 quick slot 抢赢，玩家按 `F6` 切沉浸 HUD、按 `F7` 查看 NPC 交互历史也会失效。

实际游玩里这会破坏两层快捷栏的肌肉记忆：第 6/7 槽看起来是可配置、可触发的正式槽位，但默认键不可靠。`F6` 的误触会让 HUD 隐显状态在战斗或探索中突然变化，`F7` 的误触会把 NPC 历史面板带到正常世界视野里，遮挡目标信息和事件流。

## 证据定位

- `client/src/main/java/com/bong/client/combat/CombatKeybindings.java:48-55`：循环注册 `QuickSlotConfig.SLOT_COUNT` 个 keybinding，默认键为 `GLFW.GLFW_KEY_F1 + i`。
- `client/src/main/java/com/bong/client/combat/CombatKeybindings.java:113-119`：每 tick 消费 `QUICK_SLOT_KEYS[i].wasPressed()` 并派发对应 slot。
- `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java:33-50`：quick slot 命中后读取 `QuickUseSlotStore`、本地 `beginCast`，并调用 `ClientRequestSender.sendUseQuickSlot(slot)`。
- `client/src/main/java/com/bong/client/combat/QuickSlotConfig.java:5-10`：注释明确 `Slot 0 <-> F1`、`Slot 8 <-> F9`，总数 9。
- `client/src/main/java/com/bong/client/hud/QuickBarHudPlanner.java:13-20`：HUD 上排定义为 `F1-F9 custom quick-use slots`。
- `client/src/main/java/com/bong/client/hud/HudImmersionControls.java:23-32`：HUD 沉浸开关直接消费 `wasPressed()`，默认 `GLFW.GLFW_KEY_F6`。
- `client/src/main/java/com/bong/client/npc/NpcInteractionLogControls.java:24-39`：NPC 交互日志默认 `GLFW.GLFW_KEY_F7`，世界内无 screen 时直接 `toggleVisible()`。
- `client/src/main/java/com/bong/client/npc/NpcInteractionLogStore.java:31-36`：`toggleVisible()` 是真实 HUD 可见状态切换。
- `client/src/test/java/com/bong/client/input/NoDuplicateDefaultGKeybindingTest.java:13-24`：现有默认键唯一性测试只守 `G`。
- `client/src/test/java/com/bong/client/combat/JiemaiKeyConflictTest.java:31-56`：现有冲突回归只守旧 `V` 键问题。
- `docs/plans-skeleton/plan-bughunt-client-input-keybind-collision-v1.md:15-28`：已归档/骨架问题只列 `O/U`，未枚举 `F6/F7` 这组 quick slot 碰撞。

## 触发路径

1. 玩家进入世界，服务端已同步 quick slot 配置，其中第 6 或第 7 槽有可用条目。
2. 玩家按 `F6` 期望使用第 6 槽。该物理键同时是 quick slot 6 和 HUD 沉浸 toggle 的默认键。
3. 玩家按 `F7` 期望使用第 7 槽。该物理键同时是 quick slot 7 和 NPC 交互日志 toggle 的默认键。
4. 根据 MC/Fabric 对同键 keybinding 的映射重建/注册顺序，可能 quick slot 抢赢，也可能 HUD/NPC 控制抢赢；仓库历史也把同键默认绑定视为可能双消费的事故模式。无论哪种实现细节，默认键语义都不再唯一。

## 反方审查记录

Round 1（ACCEPT，修正表述）：反方指出不能强断言“两个动作必然同时触发”。MC `KeyBinding` 更可能用单值 map 竞争，同键时只有一个 binding 获得 `wasPressed()`。但这并不推翻 bug，反而说明默认入口会抢占，导致 quick slot 或 HUD/NPC 入口至少一侧不可达。

Round 2（ACCEPT）：反方专门检查是否为有意保留键、quick slot 是否只是显示标签、注册顺序是否天然安全、是否重复 #929/#1005。结论是 `F1-F9` 在 HUD 文档和代码里都是正式 quick-use 行，`F6/F7` 不是保留键；quick slot 会发 `use_quick_slot`；注册顺序最多决定谁抢赢，不构成安全；#929 只记录 `O/U`，#1005 是 `T` 与聊天入口，本 bug 是新的 `F6/F7` 具体碰撞实例。

## Skeleton Fix Plan

P0：止血默认键冲突。

- 给 `HudImmersionControls` 或 quick slot 第 6 槽改默认策略，避免 `F6` 双占用。
- 给 `NpcInteractionLogControls` 或 quick slot 第 7 槽改默认策略，避免 `F7` 双占用。
- 若某个功能不适合占用全局默认键，优先改成 `GLFW_KEY_UNKNOWN`，对齐截脉键从默认 `V` 改未绑定的处理方式。

P1：建立默认键唯一性约束。

- 新增通用源码扫描测试，覆盖 `client/src/main/java/com/bong/client` 中所有 `GLFW.GLFW_KEY_*` 默认绑定。
- 对确需复用的键必须显式白名单，并要求存在仲裁测试，证明不会抢入口或让一侧不可达。
- 把 #929 的 `O/U` 与本 plan 的 `F6/F7` 一并纳入同一验收矩阵，避免只修局部。

P2：补用户体验回归。

- 验证 quick bar HUD 的 F1-F9 标注与真实可触发 keybinding 一致。
- 验证 HUD 沉浸和 NPC 交互日志仍有可发现、可配置且不冲突的入口。

## 验收测试计划

- client 单测：默认键唯一性扫描应在当前 `F6/F7` 状态下失败，修复后通过。
- client 单测：`F1-F9` quick slot 默认键集合不得与其它全局 keybinding 默认键重复，除非对应复用项有明确仲裁白名单。
- client 单测：HUD 沉浸 toggle 默认键不等于 quick slot 任一默认键。
- client 单测：NPC 交互日志默认键不等于 quick slot 任一默认键。
- 手动验证（JDK 17，`client/`）：`./gradlew test build` 后进世界，绑定第 6/7 quick slot，按对应键只触发目标 quick slot；HUD 沉浸和 NPC 日志各自的新入口可用且不抢 quick slot。

## 风险

- 改 quick slot 的 `F1-F9` 设计会影响既有 HUD 认知，风险较高；更保守的修法是改 HUD 沉浸/NPC 日志默认键或设为未绑定。
- 改默认键会影响老玩家的本地 `options.txt`，需要确认 Minecraft/Fabric 对既有配置的迁移行为，避免“新默认键不生效”的误判。
- 若把全局默认键唯一性测试做得过硬，可能会卡住有意复用的 hold/toggle 组合；需要白名单机制和仲裁测试一起落地。
