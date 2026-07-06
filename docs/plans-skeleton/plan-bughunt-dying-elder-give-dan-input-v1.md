# plan-bughunt-dying-elder-give-dan-input-v1

> Skeleton Plan. BugHunt G2 e2e-protocol 第二轮记录。只立项，不消费、不归档。

## Bug 摘要

垂死大能遭遇 HUD 写死显示 `给丹 [G] / 拒绝 [H] / 拖延 [J]`，但给丹/拒绝/拖延三个专属 KeyBinding 默认都是 `InputUtil.UNKNOWN_KEY`。默认安装下玩家按 HUD 提示的 `G` 只会进入统一 `InteractKeyRouter`，而默认 router 没有垂死大能 handler，因此不会调用 `DyingElderInteractionKeybindings.handleGiveDan()`，也不会发送 `give_dan_to_elder` C2S。

这不是 server handler 缺失：`GiveDanToElder` C2S 和 server `handle_give_dan_to_elder` 都已存在。断点在客户端 HUD 提示到 C2S 触发入口之间。

## 实际游玩体验影响

玩家遇到垂死大能时，HUD 明确提示按 `G` 给丹；默认键位下按 `G` 无法给丹，回元丹不会消耗，大能真元不会恢复，后续收丹、背叛/守信结局和对应叙事都不会进入。玩家会认为自己按错、目标无效或 server 没反应。

即使后续修复 `docs/plan-bughunt-r2-findings-v1.md` P0 里的 entity id / sentinel 后段问题，默认输入入口仍会让普通玩家到不了 `give_dan_to_elder` 请求。

## 证据定位

- `client/src/main/java/com/bong/client/hud/DyingElderHudPlanner.java:189`：HUD 注释与渲染标签写死三按钮。
- `client/src/main/java/com/bong/client/hud/DyingElderHudPlanner.java:194`：`String[] labels = {"给丹 [G]", "拒绝 [H]", "拖延 [J]"}`。
- `client/src/main/java/com/bong/client/dying_elder/DyingElderInteractionKeybindings.java:51`：注释说明默认不绑定固定键，由玩家自行配置。
- `client/src/main/java/com/bong/client/dying_elder/DyingElderInteractionKeybindings.java:55` 到 `63`：三个 keybinding 默认均为 `InputUtil.UNKNOWN_KEY.getCode()`。
- `client/src/main/java/com/bong/client/dying_elder/DyingElderInteractionKeybindings.java:80` 到 `81`：只有 `giveDanKey.wasPressed()` 才调用 `handleGiveDan()`。
- `client/src/main/java/com/bong/client/dying_elder/DyingElderInteractionKeybindings.java:118`：`handleGiveDan()` 才发送 `ClientRequestSender.sendGiveDanToElder(...)`。
- `client/src/main/java/com/bong/client/input/InteractionKeybindings.java:38` 到 `39`：默认 `G` 进入 `InteractKeyRouter.global().route(client)`。
- `client/src/main/java/com/bong/client/input/DefaultInteractionHandlers.java:18` 到 `29`：默认 router 注册列表没有垂死大能 handler。
- `client/src/main/java/com/bong/client/npc/NpcEngagementIntentHandler.java:43`：通用 NPC handler 命中时只发 `sendNpcInspectRequest`，不发 `give_dan_to_elder`。
- `client/src/main/java/com/bong/client/BongHud.java:123` 到 `139`：HUD 只绘制 text / scaledText / rect，没有点击回调或 action 字段。
- `server/src/network/client_request_handler.rs:2632` 到 `2650`：server 已有 `ClientRequestV1::GiveDanToElder` handler，入口问题不是 Rust 端缺分支。
- `docs/plan-bughunt-r2-findings-v1.md:17` 到 `20`：已有 active P0 覆盖的是给丹后段 entity id 命名空间和 sentinel 问题，未覆盖默认输入入口。

## 触发路径

1. 默认安装客户端，不手动改键位。
2. 进入垂死大能 active 遭遇，`DyingElderHudPlanner` 显示 `给丹 [G]`。
3. 玩家按 `G`。
4. `InteractionKeybindings` 把 `G` 路由到 `InteractKeyRouter`。
5. 默认 handler 列表没有垂死大能给丹 handler；若命中普通 NPC handler，也只发 inspect。
6. `DyingElderInteractionKeybindings.handleGiveDan()` 未运行，`ClientRequestSender.sendGiveDanToElder` 未发送，server 永远收不到本次给丹请求。

## 反方审查记录

Round 1 反方结论：通过。反方确认 HUD 写死 `[G]`，专属给丹键默认 `UNKNOWN_KEY`，默认 G 没有垂死大能 router handler，HUD 也不可点击。`NoDuplicateDefaultGKeybindingTest` 只能证明不能新增第二个默认 G，不能证明 HUD 显示 `[G]` 但默认 G 不工作是合理体验。

Round 2 反方结论：通过，但建议消费时并入现有 `docs/plan-bughunt-r2-findings-v1.md` P0。反方确认既有 P0 只覆盖 entity id / sentinel 后段；本 bug 是更前段输入入口断链。正确修复不应简单新增第二个默认 G，而应接统一 G router 或让 HUD 文案反映真实可用键位。

## Skeleton Fix Plan

- P0：为垂死大能 active 遭遇接入统一 `InteractKeyRouter`，新增专用 `IntentHandler`，默认 `G` 命中时调用与 `handleGiveDan()` 等价的给丹路径。
- P0：保持 `NoDuplicateDefaultGKeybindingTest` 约束，不新增第二个默认 `GLFW_KEY_G` 专属 keybinding。
- P1：HUD 按钮文案从硬编码 `[G] [H] [J]` 改为真实绑定状态；未绑定时显示可配置提示，已通过统一 G router 可用时显示 `G`。
- P1：保留专属 keybinding 作为高级用户自定义入口，但默认未绑定时不能在 HUD 暗示它可用。
- P2：与 `docs/plan-bughunt-r2-findings-v1.md` P0 的 entity id / sentinel 修复同批验收，确保给丹从默认输入入口到 server 权威消耗链路完整。

## 验收测试计划

- client 单测：默认 keymap 下 `DyingElderHudPlanner` 不再硬编码不可用 `[G]`，或 active 遭遇下统一 `G` router 可产生给丹 intent。
- client 单测：`DefaultInteractionHandlers.registerDefaults()` 包含垂死大能给丹 handler，且优先级不会抢占 TSY 搜刮、NPC 对话、容器打开等现有 G 主路径。
- client 单测：默认 `G` 路由到垂死大能 active 遭遇时会调用 `ClientRequestSender.sendGiveDanToElder`，payload 包含 pill instance id 和 elder entity id。
- client 单测：`NoDuplicateDefaultGKeybindingTest` 仍通过，证明没有新增第二个默认 G keybinding。
- server/client e2e：构造垂死大能 active + 背包有 `huiyuan_pill`，默认 G 触发后 server 收到 `give_dan_to_elder`，消耗丹并 emit `GiveDanToElderIntent`。
- 回归：无 active 垂死大能时默认 G 仍走原 `InteractKeyRouter` 主路径；无丹、无 elder id、目标不存在时有明确反馈且不发非法请求。

## 风险

- 与 `docs/plan-bughunt-r2-findings-v1.md` P0 同属垂死大能给丹主链路；消费时建议合并执行，避免只修后段 id/sentinel 或只修前段输入。
- 统一 G router 的优先级必须谨慎设置，避免在 NPC/容器/TSY 搜刮附近误抢交互。
- 如果设计决议坚持“垂死大能给丹必须玩家手动绑定专属键”，则 HUD 不能继续显示 `[G]`，否则仍是体验断链。
