# plan-bughunt-quick-slot-function-key-collision-v1

> **Finished Plan（2026-07-13 promotion，2026-07-14 归档）**。来源：
> `docs/plans-skeleton/plan-bughunt-quick-slot-function-key-collision-v1.md`。
> 一句话主题：保留 F1-F9 作为快捷使用槽的唯一默认入口，把 HUD 沉浸与 NPC 交互日志改为默认未绑定，并用客户端契约测试阻止功能键再次撞位。

## 阶段总览

| 阶段 | 目标 | 状态 |
|---|---|---|
| P0 | 第一性原理确认 F6/F7 默认键冲突及可达链路 | ✅ 2026-07-13 |
| P1 | 最小修改两个便利入口的默认键，并补快捷槽保留区契约测试 | ✅ 2026-07-14 |
| P2 | 完成 client 全量门禁、主线同步复验与归档证据 | ✅ 2026-07-14 |

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

- `client/src/main/java/com/bong/client/combat/CombatKeybindings.java:56-95`：九个快捷槽与四个 Combat 辅助键统一经 registrar 安装，快捷槽默认键为 `GLFW.GLFW_KEY_F1 + i`。
- `client/src/main/java/com/bong/client/combat/CombatKeybindings.java:156-165`：每 tick 消费安装到 `QUICK_SLOT_KEYS[i]` 的 registrar 返回对象，并派发同一 slot。
- `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java:33-50`：quick slot 命中后读取 `QuickUseSlotStore`、本地 `beginCast`，并调用 `ClientRequestSender.sendUseQuickSlot(slot)`。
- `client/src/main/java/com/bong/client/combat/QuickSlotConfig.java:5-10`：注释明确 `Slot 0 <-> F1`、`Slot 8 <-> F9`，总数 9。
- `client/src/main/java/com/bong/client/hud/QuickBarHudPlanner.java:13-20`：HUD 上排定义为 `F1-F9 custom quick-use slots`。
- **修复前基线** `eb6cc2b5^`：`HudImmersionControls.java:23-35` 的 HUD 沉浸开关默认 `GLFW_KEY_F6`；`NpcInteractionLogControls.java:24-38` 的 NPC 交互日志默认 `GLFW_KEY_F7`。可用 `git show eb6cc2b5^:<path>` 复核历史冲突。
- **当前落地**：`HudImmersionControls.java:51-56` 与 `NpcInteractionLogControls.java:68-73` 均安装 `GLFW_KEY_UNKNOWN`；对应的 `:27-32` 与 `:34-49` 消费 registrar 返回的同一对象。
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
4. **测试锁注册结果与同对象消费**：Combat 的 13 个绑定全部通过一个可注入 installer，行为测试断言只有 `quick_slot_1..9` 的运行时默认码落入 F1-F9。三个 installer 测试都让 registrar 返回与定义对象不同的可识别 `KeyBinding`，再通过 `KeyBinding.onKeyPressed` 与生产 consumer 证明安装字段读到的正是该返回对象。`QuickSlotDefaultKeyConflictTest` 只保留 token/简单偏移/原始 GLFW 码、Fabric installer/顶层 bootstrap 与真实 tick 入口的窄型扫描；transport capture 继续锁定同槽位 C2S。
5. **不跨题处理 O/U**：项目级 O/U 冲突由 `plan-bughunt-client-input-keybind-collision-v1` 独立收口；本 PR 不建立全键盘无重复规则，也不修改其它入口。

## 实施范围

### P0 — 证真与边界

**状态：✅ 2026-07-13**

- 从 `BongClient` 注册链、三个 `KeyBinding` 消费点和快捷槽 C2S 派发链确认正常玩家路径可达。
- 复核 F1-F9 没有显式仲裁层；注册顺序只能决定谁抢占，不能让两个动作同时保持可靠入口。

### P1 — 止血与契约测试

**状态：✅ 2026-07-14**

- `client/src/main/java/com/bong/client/hud/HudImmersionControls.java`：F6 → `GLFW_KEY_UNKNOWN`。
- `client/src/main/java/com/bong/client/npc/NpcInteractionLogControls.java`：F7 → `GLFW_KEY_UNKNOWN`。
- `CombatKeybindings.installBindings(UnaryOperator<KeyBinding>)`、`HudImmersionControls.installToggleKey(...)` 与 `NpcInteractionLogControls.installInteractionLogKey(...)`：把 registrar 返回的真实 `KeyBinding` 直接安装到 `QUICK_SLOT_KEYS` / `toggleKey` / `key`，同时允许 JUnit 用不同对象证明返回值没有被丢弃。
- `HudImmersionControls.consumeTogglePresses(BooleanSupplier, LongSupplier)` 与 `NpcInteractionLogControls.consumeTogglePresses(boolean, boolean, BooleanSupplier)`：真实 tick handler 委托给无 Minecraft 框架参数的消费边界，保留 `wasPressed()` 排空语义与最终状态切换。
- `client/src/test/java/com/bong/client/input/QuickSlotDefaultKeyConflictTest.java`：仅保留 3 项窄型契约：全 client 只允许一处快捷槽 `F1 + i` 起点表达式，并拒绝静态 import、F10 简单偏移回 F1-F9 与 290-298 原始码；三个入口仍走 Fabric installer 并由 `BongClient` 启动；installer 消费函数仍由真实 tick 入口调用。
- `HudImmersionControlsTest` / `NpcInteractionLogControlsTest`：验证显式重绑后的 false→true→false 状态转换、无按键、单 tick 多边沿、玩家不存在与界面打开分支。

### P2 — 门禁与用户体验核验

**状态：✅ 2026-07-14**

- 在 JDK 17 下运行 `cd client && ./gradlew test build`。
- 静态核验翻译键未删除，HUD 沉浸与 NPC 日志仍会注册到控制菜单；快捷栏 F1-F9 注册与 HUD 标注保持一致。
- 若具备图形客户端环境，补充 `./gradlew runClient` 人工检查控制菜单默认值；该检查不替代自动契约测试。

## 验收测试计划

- `CombatKeybindingsTest.onlyNineQuickSlotDefinitionsOwnF1ThroughF9`：捕获 Combat 全部 13 个绑定定义，断言落入 F1-F9 的恰好 9 个且 owner 只能是 `quick_slot_1..9`，另外锁定截脉/R/事件流/盾牌的默认码。
- `CombatKeybindingsTest.registrarResultIsInstalledAndReadByQuickSlotConsumer`、`HudImmersionControlsTest.installsRegistrarResultAndConsumesIt`、`NpcInteractionLogControlsTest.installsRegistrarResultAndConsumesIt`：registrar 返回可识别替身对象，生产 consumer 必须消费该同一对象。
- `QuickSlotDefaultKeyConflictTest.onlyExpectedQuickSlotExpressionReferencesReservedFunctionKeys`：不再豁免 Combat 文件，全 client 只允许快捷槽的 `GLFW_KEY_F1 + i`；同时覆盖静态 import、数字偏移与原始 GLFW 键码。
- `QuickSlotDefaultKeyConflictTest.productionUsesFabricInstallersAndTopLevelBootstrap` / `installedBindingsRemainConnectedToRealTickEntrypoints`：窄型锁定 Fabric installer、`BongClient` 启动与真实 tick 调用可测 consumer。
- `CombatHudBootstrapTest`：已绑定 F9 边界槽向真实 transport 发送同一 slot，空槽零发包且不起 cast。
- `HudImmersionControlsTest`（4 项）：默认注册、无按键不读时钟、显式重绑按键开/关往返、单 tick 多边沿排空与注入时钟。
- `NpcInteractionLogControlsTest`（6 项）：默认注册、无按键、显式重绑开/关、玩家缺失、界面打开、单 tick 多边沿排空。
- client 全量：JDK 17 下 `./gradlew test build` 必须全绿。

## 风险

- 已保存 `options.txt` 的老玩家可能继续保留 F6/F7；本 plan 有意不覆盖用户配置，只修默认值并在 Finish Evidence 记录该限制。
- 两个便利入口改为默认未绑定后，需要玩家从控制菜单主动分配；翻译键和注册入口不得删除。
- 测试只保留 F1-F9，不会误伤 R 键等已有显式仲裁场景，也不会抢先处理 O/U 独立 skeleton。

## Finish Evidence

### 落地清单

- **P0 证真**：`BongClient` 同时注册 `NpcInteractionLogControls`、`HudImmersionControls` 与 `CombatHudBootstrap`；后者把 F1-F9 快捷槽接到 `CombatHudBootstrap.onQuickSlotPressed`，最终调用 `ClientRequestSender.sendUseQuickSlot(slot)`。修复前契约测试 4 项中 3 项稳定失败，分别钉住 F6、F7 与保留区冲突。
- **P1 修复**：`HudImmersionControls` 与 `NpcInteractionLogControls` 均改为 `GLFW_KEY_UNKNOWN`，保留原翻译键和 Fabric 控制菜单注册；两个 tick handler 委托给可直接行为测试的 `consumeTogglePresses`，仍由真实 `KeyBinding.wasPressed()` 驱动最终 HUD/NPC 可见状态转换。最终测试方案把“定义 → registrar 返回 → 安装字段 → consumer”收在三个薄 installer 中，用替身 `KeyBinding` 实际按下证明同对象接线；Combat 全 13 绑定运行时审计与全 client 窄型 token/偏移/原始码扫描共同锁定 F1-F9 排他性。通用 `JavaSourceIndex` 及其夹具保持删除。
- **P2 门禁**：最新 `/review` run `29322432403` 将所有重复 finding 收敛为 Combat 整文件豁免、registrar 返回值断链假绿与历史证据定位失效三个根因。提交 `ed68994b`、`5144cf21`、`75cf15e8` 逐项修复；负向变异把非快捷键改为 F6 并同时丢弃三个 registrar 返回值后，7 项中 5 项稳定失败。恢复后 JDK 17 定向 24 项与 client 全量 4077 项均全绿。随后显式合并 `origin/main@4ad0c170`，在未提交合并结果上完成 server `fmt`、`clippy -D warnings` 与全量测试复验，合并提交为 `8f67296a`。

### 关键 commit

- `ea83d4cb`（2026-07-13）— 升格 plan 并收口为“F1-F9 归快捷槽、两个便利入口默认未绑定”的单 PR 范围。
- `eb6cc2b5`（2026-07-13）— 修改 F6/F7 默认值并新增快捷槽默认键冲突回归测试。
- `71a1617c`（2026-07-14）— 按 `/review` 两项 major finding 改为 AST 级默认键参数审计，并覆盖间接常量路径。
- `a10d1a69`（2026-07-14）— 合并 `origin/main@390f22e5`，在未提交合并结果上复验客户端完整门禁。
- `80c4af1e`（2026-07-14）— 按复审 finding 锁定 Fabric 注册链，并覆盖 `KeyBinding` 子类 `super(...)` 默认键路径。
- `df88fbe0`（2026-07-14）— 恢复 javac analyze 后错误门禁，并以缺失类型负向夹具阻止语义审计假绿。
- `b100756d`（2026-07-14）— 提取 HUD 沉浸按键消费边界，锁定显式重绑后的开关往返、队列排空与时钟语义。
- `a90024f5`（2026-07-14）— 提取 NPC 日志按键消费边界，锁定玩家/界面 guard 与最终可见状态转换。
- `6a858d22`（2026-07-14）— 把快捷槽循环改为 AST 逐次语义求值，并锁定 tick 注册与真实 `wasPressed()` 接线。
- `84ab2f19`（2026-07-14）— 暴露包级快捷槽入口给行为测试，锁定已绑定槽位的真实 `use_quick_slot` C2S 与空槽零出料。
- `e9509bf0`（2026-07-14）— 把 javac/AST 索引、求值器、匹配器与 7 项正反夹具拆到独立 test-support。
- `1abaf7ff`（2026-07-14）— 精确锁定 HUD/NPC 实参数据流、顶层 bootstrap 与快捷槽 slot 端到端传递链。
- `283c6f14`（2026-07-14）— 合并 `origin/main@c231666d`，在未提交合并结果上复验客户端完整门禁。
- `f7f5da9b`（2026-07-14）— 以可注入 registrar 收窄三个键位注册入口，删除生产注释中的归档 plan 标识。
- `173510e4`（2026-07-14）— 直接验证真实 `KeyBinding` 的翻译键、分类和 F1-F9/UNKNOWN 默认值。
- `4814206b`（2026-07-14）— 删除 1093 行 `JavaSourceIndex` 及 475 行夹具，仅保留 105 行快捷槽窄型接线测试。
- `ed68994b`（2026-07-14）— 把 Combat/HUD/NPC 的 registrar 返回值直接安装到真实消费字段，提取可直接驱动的 consumer。
- `5144cf21`（2026-07-14）— 用 registrar 替身对象实际按键，证明三条消费链读取同一安装对象，并审计 Combat 全 13 个绑定。
- `75cf15e8`（2026-07-14）— 取消 Combat 整文件豁免，扩展窄型保留区扫描至静态 import、数字偏移与原始 GLFW 码。
- `c8f386fc`（2026-07-14）— 修正 finished plan 的历史基线与当前源码定位，避免把修复前 F6/F7 误指向当前 UNKNOWN 实现。
- `8f67296a`（2026-07-14）— 合并 `origin/main@4ad0c170`，在未提交合并结果上完成 server 三联门禁复验。

### 测试结果

- 修复前复现：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew --no-daemon test --tests com.bong.client.input.QuickSlotDefaultKeyConflictTest` → 4 tests，3 failed（预期红：HUD F6、NPC F7、保留区扫描）。
- 首轮修复后定向：同命令 → 4 tests，0 failed，`BUILD SUCCESSFUL`。
- review 返工定向：同命令 → 5 tests，0 failed；临时把 HUD 构造器第三参数改回 F6 后 → 5 tests，2 failed（精确参数断言与全仓保留区审计同时红），恢复后重新全绿。
- review 返工全量：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew --no-daemon test build` 在 `71a1617c` 上 `BUILD SUCCESSFUL`（23s）。
- 主线同步复验：同一全量命令在未提交的 `origin/main@390f22e5` 合并结果上 → 4065 tests，0 failures，0 errors，0 skipped，`BUILD SUCCESSFUL`（1m30s）。
- 复审二次返工定向：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew test --tests com.bong.client.input.QuickSlotDefaultKeyConflictTest` → 7 tests，0 failures；正反夹具证明移除 `registerKeyBinding` 包装会失败、子类 `super(F6)` 会命中碰撞、动态子类默认键会 fail closed。
- 复审二次返工全量：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew test build` → 4067 tests，0 failures，0 errors，0 skipped，`BUILD SUCCESSFUL`（1m55s）。
- 复审三次返工定向：同一定向命令 → 8 tests，0 failures；`sourceIndexFailsClosedOnSemanticErrors` 确认 analyze 阶段缺失类型会以“语义分析失败”终止索引。
- 复审三次返工全量：同一全量命令 → 4068 tests，0 failures，0 errors，0 skipped，`BUILD SUCCESSFUL`（1m27s）。
- 本轮状态转换契约预期红：新增 HUD/NPC 行为测试后、提取消费函数前，定向命令在 `compileTestJava` 产生 10 个 `consumeTogglePresses` 缺失错误，证明测试先于实现阻止假绿。
- 本轮消费行为定向：JDK 17 下运行 HUD/NPC 两个测试类 → 8 tests，0 failures；覆盖无按键、false→true→false、单 tick 多边沿、玩家不存在与界面打开。
- 本轮语义与接线定向：JDK 17 下同时运行 `QuickSlotDefaultKeyConflictTest`、`HudImmersionControlsTest`、`NpcInteractionLogControlsTest` → 20 tests，0 failures；其中 AST 12 项、HUD 3 项、NPC 5 项。
- 本轮 client 全量：代码 HEAD `6a858d22` 上 `JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew --no-daemon test build` → 4080 tests，0 failures，0 errors，0 skipped，`BUILD SUCCESSFUL`（3m54s）。
- 本轮主线分类：`git fetch origin` 后 `origin/main@390f22e5` 是 `6a858d22` 的祖先，classification=`already-up-to-date`，未改变 HEAD，因此复用同一全量门禁证据。
- 复审四次返工定向：JDK 17 下同时运行 `QuickSlotDefaultKeyConflictTest`、`JavaSourceIndexTest`、`CombatHudBootstrapTest` → 23 tests，0 failures；其中生产接线契约 7 项、AST 正反夹具 7 项、真实 bootstrap/C2S 行为 9 项。
- 复审四次返工全量：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew --no-daemon test build` → 4084 tests，0 failures，0 errors，0 skipped，`BUILD SUCCESSFUL`（4m01s）。
- 复审四次主线同步：`git fetch origin` 后 `origin/main@c231666d` 与代码 HEAD 分叉；主线未触及本 PR 五个 client 文件，以 `--no-commit --no-ff` 合并后再次运行 client 全量 `test build` → 4084 tests，0 failures，0 errors，0 skipped，`BUILD SUCCESSFUL`（33s，任务均 up-to-date），合并提交 `283c6f14`。
- 复审五次返工定向：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew --no-daemon test --rerun-tasks` 定向 `QuickSlotDefaultKeyConflictTest`、`CombatKeybindingsTest`、`CombatHudBootstrapTest`、HUD/NPC 两个测试类 → 23 tests，0 failures，0 errors，0 skipped，`BUILD SUCCESSFUL`（3m30s）。
- 复审五次返工全量：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew --no-daemon test build --rerun-tasks` → 4076 tests，0 failures，0 errors，0 skipped，13 个任务全部实际执行，`BUILD SUCCESSFUL`（5m33s）。
- 复审六次负向变异：临时把 `jiemai_react` 改为 F6，并丢弃 Combat/HUD/NPC 三个 registrar 返回值；运行保留区、Combat installer、HUD installer、NPC installer 共 7 tests → 5 failed（Combat 非快捷键独占、Combat 同对象消费、HUD 同对象消费、NPC 同对象消费、全 client 保留区均精确变红）。恢复生产代码后重新全绿。
- 复审六次返工定向：JDK 17 `--rerun-tasks` 运行 `QuickSlotDefaultKeyConflictTest`、`CombatKeybindingsTest`、`CombatHudBootstrapTest`、HUD/NPC 两个测试类 → 24 tests，0 failures，0 errors，0 skipped，`BUILD SUCCESSFUL`（2m31s）。
- 复审六次返工全量：`JAVA_HOME=$HOME/.cache/codex-jdks/jdk-17 ./gradlew --no-daemon test build --rerun-tasks` → 4077 tests，0 failures，0 errors，0 skipped，13 个任务全部实际执行，`BUILD SUCCESSFUL`（5m37s）。
- 复审六次主线同步：显式合并 `origin/main@4ad0c170` 后，在未提交合并结果上运行 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → 命令最终退出码 0；`fmt` 与 `clippy` 全绿，library target 11648 tests（11647 passed、1 ignored），main target 11 passed，integration targets 5 passed，doctest 5 ignored；合并提交 `8f67296a`。
- `git diff --check` 在 review 修复、主线合并和 Finish Evidence 更新前均通过；每轮验证前后均核验工作区状态与目标 HEAD。
- 用户明确要求本轮不启动 subagent，因此 FIX/REBASE validator 由主 agent 对绑定 SHA 的干净 diff 独立复核；未伪造外部 validator 身份。

### 跨仓库核验

- **client**：`CombatKeybindings` / `QuickSlotConfig` 继续提供 F1-F9 九槽；Combat 全 13 绑定运行时审计保证非快捷键不落入保留区；`HudImmersionControls` / `NpcInteractionLogControls` 改为 UNKNOWN，三条 installer → field → consumer 以替身对象实际按键证明不断链；`QuickSlotDefaultKeyConflictTest` 的 3 项窄型扫描与 `CombatHudBootstrapTest` 共同锁住 Fabric 安装、bootstrap、同槽位 C2S 及空槽零出料契约。
- **server / agent / schema**：本修复不改变 `use_quick_slot` 协议、服务端 handler 或 agent schema，无跨栈产物需要重建。

### 遗留 / 后续

- 不迁移或覆盖现有玩家 `options.txt`；已保存的 F6/F7 冲突绑定需玩家在控制菜单自行调整，避免本 PR 擅自覆盖用户选择。
- O/U 默认键冲突继续由 `docs/plans-skeleton/plan-bughunt-client-input-keybind-collision-v1.md` 独立处理，本 PR 不跨题修改。
- 本轮未启动交互式 `runClient`；默认值、入口保留、显式重绑后的状态转换、F1-F9 排他性与快捷槽真实出料链已由 Combat 全绑定运行时审计、installer/consumer/transport 同对象行为测试、3 项窄型接线检查及 4077 项 client 全量测试闭环。
