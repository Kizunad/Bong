# plan-bughunt-quick-slot-function-key-collision-v1

> **Active Plan（2026-07-13 promotion）**。来源：
> `docs/plans-skeleton/plan-bughunt-quick-slot-function-key-collision-v1.md`。
> 一句话主题：保留 F1-F9 作为快捷使用槽的唯一默认入口，把 HUD 沉浸与 NPC 交互日志改为默认未绑定，并用客户端契约测试阻止功能键再次撞位。

## 阶段总览

| 阶段 | 目标 | 状态 |
|---|---|---|
| P0 | 第一性原理确认 F6/F7 默认键冲突及可达链路 | ⏳ |
| P1 | 最小修改两个便利入口的默认键，并补快捷槽保留区契约测试 | ⬜ |
| P2 | 完成 client 全量门禁、主线同步复验与归档证据 | ⬜ |

## Bug 摘要

默认键位中 `F6/F7` 同时被两套正式入口占用：

- `CombatKeybindings` 把 `F1-F9` 注册为 9 个快捷使用槽，因 `GLFW_KEY_F1 + i`，第 6/7 槽自然落在 `F6/F7`。
- `HudImmersionControls` 把 HUD 沉浸开关默认绑到 `F6`。
- `NpcInteractionLogControls` 把 NPC 交互日志默认绑到 `F7`。

这不是视觉标签冲突，而是输入入口冲突：快捷槽命中后会走 `QuickUseSlotStore`、本地 cast 反馈，并发送 `use_quick_slot` C2S。MC/Fabric 同物理键绑定存在单值映射抢占或仓库历史记录的双消费风险，因此默认配置下至少一侧入口会变得不可预测或不可达。

## 接入面

- **进料**：`CombatKeybindings` 的 F1-F9 快捷槽注册、`HudImmersionControls` 与 `NpcInteractionLogControls` 的独立 tick 消费链。
- **出料**：快捷槽继续派发 `use_quick_slot`；HUD 沉浸和 NPC 日志仍保留原翻译键与控制菜单入口，由玩家显式绑定后触发。
- **共享类型 / event**：仅复用 Fabric `KeyBinding` / `KeyBindingHelper`，不新增协议、schema 或服务端事件。
- **跨仓库契约**：纯 client 默认键修复，不修改 server / agent wire contract。
- **worldview / qi_physics**：不涉及世界观命名、真元或灵气流动。

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

## 实施决议（2026-07-13）

1. **F1-F9 归快捷槽独占**：`QuickSlotConfig.SLOT_COUNT == 9` 与 HUD 的 F1-F9 标注是正式玩家入口，本 plan 不改快捷槽数量、顺序或默认键。
2. **便利入口默认未绑定**：把 `HudImmersionControls` 和 `NpcInteractionLogControls` 的默认键改为 `GLFW_KEY_UNKNOWN`。两个功能仍在原版控制菜单可发现、可配置；不另拍新默认键，避免把冲突平移到别处。
3. **不强改既有玩家配置**：不重写 `options.txt`，也不覆盖玩家主动保存的绑定。修复锁定新安装/重置键位的默认契约；既有冲突配置可在控制菜单自行调整。
4. **测试范围只守 F1-F9 保留区**：新增 `QuickSlotDefaultKeyConflictTest`，断言快捷槽仍注册 F1-F9、两个便利入口默认 UNKNOWN、除 `CombatKeybindings` 外没有其它生产 Java 文件直接默认绑定 F1-F9。
5. **不跨题处理 O/U**：项目级 O/U 冲突由 `plan-bughunt-client-input-keybind-collision-v1` 独立收口；本 PR 不建立全键盘无重复规则，也不修改其它入口。

## 实施范围

### P0 — 证真与边界

- 从 `BongClient` 注册链、三个 `KeyBinding` 消费点和快捷槽 C2S 派发链确认正常玩家路径可达。
- 复核 F1-F9 没有显式仲裁层；注册顺序只能决定谁抢占，不能让两个动作同时保持可靠入口。

### P1 — 止血与契约测试

- `client/src/main/java/com/bong/client/hud/HudImmersionControls.java`：F6 → `GLFW_KEY_UNKNOWN`。
- `client/src/main/java/com/bong/client/npc/NpcInteractionLogControls.java`：F7 → `GLFW_KEY_UNKNOWN`。
- `client/src/test/java/com/bong/client/input/QuickSlotDefaultKeyConflictTest.java`：锁定快捷槽保留区、两个默认未绑定入口及未来 F1-F9 直接占用回归。

### P2 — 门禁与用户体验核验

- 在 JDK 17 下运行 `cd client && ./gradlew test build`。
- 静态核验翻译键未删除，HUD 沉浸与 NPC 日志仍会注册到控制菜单；快捷栏 F1-F9 注册与 HUD 标注保持一致。
- 若具备图形客户端环境，补充 `./gradlew runClient` 人工检查控制菜单默认值；该检查不替代自动契约测试。

## 验收测试计划

- `QuickSlotDefaultKeyConflictTest.quickSlotsStillOwnFunctionKeyDefaults`：锁定 9 槽与 `GLFW_KEY_F1 + i`。
- `QuickSlotDefaultKeyConflictTest.hudImmersionDefaultsUnbound`：锁定 HUD 沉浸仍注册且默认 UNKNOWN。
- `QuickSlotDefaultKeyConflictTest.npcInteractionLogDefaultsUnbound`：锁定 NPC 日志仍注册且默认 UNKNOWN。
- `QuickSlotDefaultKeyConflictTest.noOtherClientBindingClaimsF1ToF9ByDefault`：扫描生产 Java 源码，除 `CombatKeybindings` 外不得直接出现 F1-F9 默认常量。
- client 全量：JDK 17 下 `./gradlew test build` 必须全绿。

## 风险

- 已保存 `options.txt` 的老玩家可能继续保留 F6/F7；本 plan 有意不覆盖用户配置，只修默认值并在 Finish Evidence 记录该限制。
- 两个便利入口改为默认未绑定后，需要玩家从控制菜单主动分配；翻译键和注册入口不得删除。
- 测试只保留 F1-F9，不会误伤 R 键等已有显式仲裁场景，也不会抢先处理 O/U 独立 skeleton。
