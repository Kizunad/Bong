# plan-bughunt-lingtian-advancements-key-conflict-v1（骨架）

> **骨架（草案）**。一句话主题：灵田动作屏默认键 `GLFW_KEY_L` 与 Minecraft 1.20.1 原版进度屏 `key.advancements`（默认 `L`）冲突，且无任何 Bong↔vanilla 仲裁层——同一物理键在 `KeyBinding.KEY_TO_BINDINGS` 单值 map 语义下只会路由给一个 binding，结果是"原版进度屏默认键失效"或"灵田面板默认键失效"二选一；即便按本仓历史记录的双触发形态，则一次按 `L` 同时开两屏抢屏。

> 立项动机：Client-B 分区 bughunt 全量键位默认值扫描发现。**去重说明**：① `docs/plans-skeleton/plan-bughunt-client-input-keybind-collision-v1.md`（#929）只覆盖 **Bong 内部** `O/O`、`U/U` 双绑，其"根治"测试提案只扫 Bong client 的 `GLFW_KEY_*` 默认键互相去重，天然不查 vanilla 键表；② origin/main active `docs/plan-bughunt-spirit-treasure-chat-key-conflict-v1.md` 覆盖了同机制的 `T`/chatKey 冲突，但其 P1 回归测试只 pin `chatKey`/`commandKey`，**不含 `advancementsKey`**，且修复面只动灵宝 bootstrap；`git grep -il advancement origin/main -- docs/plan-*.md docs/plans-skeleton/` 零命中——`L` 冲突无人跟踪。本骨架不并入上述两者的原因：两者各自 scope 已收窄（一个 Bong 内部、一个只修 T），跨界追加会撞它们可能在飞的修复；本骨架同时提出把键位唯一性测试推广到完整 vanilla 默认键表，作为三案共同的根治收口。

## Bug 摘要

`LingtianActionScreenBootstrap` 把打开 `LingtianActionScreen` 的默认键硬编码为 `GLFW.GLFW_KEY_L`；MC 1.20.1 原版 `GameOptions.advancementsKey` 默认同为 `L`。注册走 `KeyBindingHelper.registerKeyBinding`（Fabric 不做物理键冲突检查，`KeyBindingRegistryImpl.process()` 仅追加进 `GameOptions.allKeys`）；vanilla `KeyBinding.KEY_TO_BINDINGS` 是 `Map<InputUtil.Key, KeyBinding>` 单值 map，`updateKeysByCode()` 用 `Map.put` 重建——同键双绑后按键只路由给 map 中最后写入的那个 binding（写入顺序对玩家不可预期）。tick 侧 `while (keyBinding().wasPressed()) requestOpenScreen(...)` 仅排除"当前已是 LingtianActionScreen"，无 vanilla 仲裁。

无论运行时落在哪种形态（单赢死键 / 本仓 `CombatKeybindings` 历史注释记录过的"单次按 V 两个 `wasPressed()` 都触发"双派发），结果都是坏的：原版进度屏或灵田面板必有一个默认入口失效，或一键双屏抢屏。

## 对实际游玩体验的影响

- 玩家按原版习惯按 `L` 看进度/成就：可能被灵田动作屏接管（且该屏不要求准星对着灵田，随时可开）。
- 反之若 advancements 赢下 map 槽位：灵田面板默认键死掉，玩家以为灵田功能坏了——`plan-lingtian-v1` 的 §1.2 入口设计（按 L 打开）静默失效。
- 控制设置界面会把两条绑定标红冲突，但默认键位玩家（绝大多数）不会进设置排查。

## 证据定位（行号基于 origin/main b398c4071）

- `client/src/main/java/com/bong/client/lingtian/LingtianActionScreenBootstrap.java:42`：`new KeyBinding(OPEN_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_L, CATEGORY)`。
- 同文件 `onEndClientTick`：`while (keyBinding().wasPressed()) requestOpenScreen(client)`；`requestOpenScreen` 仅 `instanceof LingtianActionScreen` 早退。
- `client/src/main/java/com/bong/client/BongClient.java:103`：`LingtianActionScreenBootstrap.register()` 生产 init 无条件注册。
- Yarn 1.20.1 mappings：`GameOptions.advancementsKey` 默认绑定 `GLFW_KEY_L`。
- `client/src/main/java/com/bong/client/mixin/MixinKeyboardSkillKeys.java`：仅仲裁 `F`/`1-9`，不涉 `L`。
- 同机制姊妹案：`docs/plan-bughunt-spirit-treasure-chat-key-conflict-v1.md`（active，`T`/chatKey）已完成对 vanilla `KeyBinding.KEY_TO_BINDINGS` 单值 map / Fabric 无冲突检查的反汇编级论证，本骨架同引。
- 历史事故佐证：`client/src/main/java/com/bong/client/combat/CombatKeybindings.java:66-69` 注释记录旧版 `V` 同键双绑"两个 `wasPressed()` 都触发"。

## 触发路径

1. 默认键位启动客户端，进入世界。
2. 按 `L`。
3. 期望：原版进度屏；实际：灵田动作屏与进度屏争夺同一物理键（单赢死键或双开抢屏，取决于 `KEY_TO_BINDINGS` 重建顺序）。

## 反方审查记录

### Round 1（无上下文 review-skeptic subagent）

**反方结论**：REAL。反方按三条攻击线核验后未能证伪：① 全仓无 `L` 相关仲裁层（`MixinKeyboardSkillKeys` 仅 F/1-9；`requestOpenScreen` 的 `instanceof` 门禁只防自身重开，不防 vanilla 屏被覆盖）；② `KeyBindingHelper` 是纯 wrapper，无自动改绑/避让机制（`KeyBindingRegistryImpl.process()` 只追加 `allKeys`）；③ 两 bootstrap 确在 `BongClient.onInitializeClient` 生产注册（103/143 行）。另确认 #929 骨架与 T/chatKey active plan 均未覆盖 `L`。

## Skeleton Fix Plan

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 灵田默认键挪离 `L` | fix_pr | ⬜ |
| P1 | 键位唯一性测试推广到 vanilla 默认键表 | fix_pr | ⬜ |

### P0 — 灵田默认键挪离 `L`

- `LingtianActionScreenBootstrap` 默认键改为不占 vanilla 基础操作的键（或默认 `GLFW_KEY_UNKNOWN` 交玩家自绑，对齐 `CombatKeybindings` 处理旧 `V` 冲突、以及 T/chatKey 姊妹 plan 的 P0 方向）。
- 面板内/文案同步新键位提示，不写死 `L`。

### P1 — 键位唯一性测试推广到 vanilla 默认键表

- 在既有（或姊妹 plan 将建的）默认键去重测试上扩一张 **vanilla 1.20.1 默认键表**（movement/inventory/chat/command/advancements/social 等），断言 Bong 全部 `GLFW_KEY_*` 默认绑定不落在表内；白名单机制留给确有仲裁层的键（如 `MixinKeyboardSkillKeys` 管的 F/1-9）。
- 该测试同时覆盖 #929（Bong 内部去重）与 T/chatKey plan（chat/command pin）各自的盲区，是三案共同的防回归收口——实施时若姊妹 plan 已 land 其测试，则在其上扩表而非另起炉灶。

## 验收测试计划

1. `cd client && ./gradlew test build`（JDK 17）——vanilla 键表去重测试绿，且能在人为把任一 Bong 默认键改回 `L`/`T` 时撞红。
2. `runClient` 目视：按 `L` 打开原版进度屏；灵田面板经新默认键（或自绑键）可开，功能不回归。

## 风险

- 可用字母键位余量有限（C/K/N/Y/H/G/V/M/O/U/T/L 已被占或冲突中），P0 选键需对拍 #929 的 O/U 归属裁决，避免挪出新冲突——vanilla 键表测试 + Bong 内部去重测试双管齐下即可机械化排除。
- 若 T/chatKey 姊妹 plan 的 P1 测试先 land，本 plan P1 改为在其测试上扩表，避免两套键表测试并存。

## 审计来源

Client-B 分区 bughunt 定点轮。方法：全量 `KeyBindingHelper.registerKeyBinding` 默认键扫描 → 与 vanilla 1.20.1 默认键表对拍 → 仲裁层全仓核验（mixin/门禁）→ origin/main（b398c4071）复验 + 三重文档去重（#929 骨架 / T-chatKey active / advancements 零命中）→ 无上下文 review-skeptic 对抗证伪（REAL）。本 PR 仅新增 report-only skeleton，不改代码。
