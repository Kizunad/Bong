# plan-bughunt-spirit-treasure-chat-key-conflict-v1（active）

> **Active（待消费）**。一句话主题：灵宝面板默认 `T` 与 Minecraft 原版聊天键 `chatKey` 默认 `T` 冲突；Fabric/vanilla `KeyBinding` 对同一物理键不是多播，最终只会有一个 binding 收到按键，导致默认聊天入口或灵宝入口二选一失效。

## Bug 摘要

`SpiritTreasureScreenBootstrap` 把灵宝面板默认键设为 `GLFW_KEY_T`，而 Minecraft 1.20.1 原版 `GameOptions.chatKey` 默认也是 `T`。本地 Yarn/Fabric 反汇编显示 `KeyBinding.KEY_TO_BINDINGS` 是单值 map，`onKeyPressed` / `setKeyPressed` 只读取同一物理键对应的一个 `KeyBinding`；`updateKeysByCode()` 也是 `Map.put(boundKey, binding)`，不存在冲突仲裁或多播。

这不是 #929 记录的 Bong 内部 `O/U` 双派发问题；这里的故障形态是 Bong 自定义默认键压到 vanilla 基础聊天入口。灵宝设计又明确要求“对话仍走聊天栏 @”，因此默认 `T` 冲突会直接切断玩家最自然的灵宝对话输入路径。

## 实际游玩体验影响

- 玩家按原版习惯按 `T` 想打开普通聊天栏时，可能被灵宝面板接管；反过来若 chatKey 赢，灵宝面板默认键又不可用。
- 灵宝对话设计要求玩家通过聊天栏输入 `@灵宝名 ...`，默认 `T` 入口失效会让玩家以为器灵对话坏了。
- `/` 仍可打开命令前缀，但它不是普通聊天入口，不能视为对 `T` 聊天习惯的等价替代。

## 证据定位

- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreenBootstrap.java:14`：`DEFAULT_KEY = GLFW.GLFW_KEY_T`。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreenBootstrap.java:31`：tick 内 `while (keyBinding().wasPressed())` 后打开 `SpiritTreasureScreen`。
- `client/src/main/java/com/bong/client/BongClient.java:143`：客户端初始化注册 `SpiritTreasureScreenBootstrap.register()`。
- Yarn 1.20.1 mappings：`GameOptions.chatKey` 注释说明默认绑定 `GLFW_KEY_T`。
- Yarn 1.20.1 反汇编：`KeyBinding.KEY_TO_BINDINGS` 为 `Map<InputUtil.Key, KeyBinding>`；`onKeyPressed` / `setKeyPressed` 只 `get(key)` 一个 binding；`updateKeysByCode()` 用 `Map.put` 重建映射。
- Fabric key-binding API 1.0.37 源码：`KeyBindingHelper.registerKeyBinding` 不检查物理键冲突；`KeyBindingRegistryImpl.process()` 只是把 modded keybindings 追加进 `GameOptions.allKeys`。
- `docs/finished_plans/plan-spirit-treasure-v1.md:525`：灵宝面板用 `T` 打开，但“对话仍走聊天栏 @”。

## 触发路径

1. 使用默认键位启动客户端。
2. 进入世界后按 `T`。
3. 期望：原版聊天栏打开，玩家可以输入普通聊天或 `@灵宝名 ...`。
4. 实际风险：同一物理键只路由给一个 `KeyBinding`；灵宝面板与原版聊天入口冲突，导致其中一个默认入口失效。

## 反方审查记录

### Round 1

反方重点攻击 Fabric 是否会自动避让原版键、原版聊天是否绕过 `KeyBinding`、以及 #929 是否覆盖。结论：通过候选。反方确认 Fabric 不检查同物理键冲突，原版聊天依赖 `chatKey.wasPressed()`，#929 只记录 `O/U` Bong 内部冲突，未覆盖 `T` / vanilla chat / 灵宝 `@` 对话入口。

### Round 2

反方进一步攻击“这是否只是 #929 的轻重复”。结论：通过候选。反方认为 #929 是 `docs/plans-skeleton` 且只覆盖 Bong 内部双派发；本 bug 是 vanilla chatKey 被 Bong 面板默认键压掉，机制、体验入口、修复验收都不同，适合单独 active plan。

## 修复计划

- P0：把灵宝面板默认键从 `T` 挪走，优先考虑默认 `UNKNOWN` 或一个明确不占 vanilla 基础操作的键。
- P1：新增默认键位冲突回归测试，至少 pin 住 Bong 默认 keybinding 不得覆盖 vanilla `chatKey` / `commandKey`。
- P2：在灵宝面板提示/语言项里同步新默认键或“未绑定，请到控制设置绑定”的状态，避免 UI 文案继续写死 `T`。

## 验收测试计划

- 单元/源码扫描：断言 `SpiritTreasureScreenBootstrap.DEFAULT_KEY` 不再是 `GLFW_KEY_T`。
- 单元/源码扫描：断言全仓 Bong 自定义默认 keybinding 不使用 vanilla `chatKey` / `commandKey` 默认键。
- 手动/集成：默认配置下按 `T` 能打开聊天栏；灵宝面板通过新默认键或玩家自绑键打开。
- 灵宝对话回归：通过聊天栏输入 `@灵宝名 ...` 仍进入器灵对话链路。

## 风险

- 改默认键会影响已形成 `T` 肌肉记忆的玩家，需要在控制设置/提示文本中明确迁移。
- 若选择 `UNKNOWN`，新玩家需要主动绑定灵宝面板键；但这优于抢占 vanilla chatKey。
- 若选择另一个字母键，必须和 #929 记录的 O/U 以及现有 G/H/Y 等输入入口一起做冲突扫描，避免把问题平移。
