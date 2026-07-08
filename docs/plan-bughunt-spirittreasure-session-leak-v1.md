# BugHunt: 灵宝面板跨 session 残留旧灵宝与旧对话

## Bug 摘要

客户端灵宝面板的 `SpiritTreasureStateStore` 与 `SpiritTreasureDialogueStore` 是进程级静态 store，虽然各自提供 `clear()`，但生产断线/切服路径没有调用。玩家从 A 服务器或存档断开后进入 B 服务器或新世界，在 B 的首个有效 `spirit_treasure_state` 到达前按 T 打开灵宝面板时，面板可能显示 A 的灵宝状态；更硬的问题是历史对话按 `templateId` 读取，即使 B 后续推送了同 template 灵宝，A 的器灵对话也可能挂到 B 的灵宝面板上。

## 实际游玩体验影响

- 切服、重进单人世界、断线重连后，玩家可能在新 session 看到上一局的灵宝、触发位状态和器灵对话，误以为当前角色仍持有这些灵宝。
- 如果新 session 还未收到灵宝状态，T 面板不会显示“暂无灵宝”，而是展示旧 snapshot；这会误导玩家继续操作旧 UI。
- 右键旧 UI 会向当前连接发送 `treasure_activate`。多数情况下服务端会因当前背包无该 `instance_id` 而拒绝并重推；只有 `instanceId` 碰撞且 B 当前玩家同 id 也是 Treasure 时，才存在错激活/卸下的次级风险。
- 不应声称该旧 state 必然长期存在：正常 B 服可能很快推送新的 `spirit_treasure_state` 覆盖 state store；但 dialogue store 不随 state 覆盖清理，残留窗口和错挂对话仍成立。

## 证据定位

- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreenBootstrap.java:21`：只注册 keybinding 与 `END_CLIENT_TICK`，没有 `ClientPlayConnectionEvents.DISCONNECT`。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreenBootstrap.java:27`：打开门禁只检查 `client.player != null`。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreenBootstrap.java:45`：按 T 直接 `setScreen(new SpiritTreasureScreen())`，没有当前连接 freshness gate。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureStateStore.java:8`：灵宝状态保存在静态 `snapshot`。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureStateStore.java:36`：`clear()` 存在，但生产路径 grep 只发现 handler replace、screen read 和 test reset。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureDialogueStore.java:12`：历史对话保存在静态 `DIALOGUES`。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureDialogueStore.java:41`：`clear()` 存在，但生产路径未调用。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreen.java:50` 与 `:78`：init/render 直接读取 `SpiritTreasureStateStore.snapshot()`。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreen.java:180` 与 `:196`：右键卸下/激活会把旧 snapshot 的 `instanceId` 发送到当前连接。
- `client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreen.java:254`：panel 按 `templateId` 从 `SpiritTreasureDialogueStore` 取 recent dialogue，state 更新不会自动清掉旧对话。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:131`：全局 disconnect 清理了大量 static client store，但没有清理灵宝 state/dialogue store。
- `client/src/main/java/com/bong/client/network/ServerDataRouter.java:197`：只注册 `spirit_treasure_state` / `spirit_treasure_dialogue` handler，没有 reset 入口。
- 相关历史查重：#905 已合并，主题是“灵宝对话后好感与被动状态不同步”，涉及 affinity/passive 同步与服务端广播补齐，不覆盖本 session leak。

## 触发路径

1. 在 A 服务器/存档收到 `spirit_treasure_state` 和至少一条 `spirit_treasure_dialogue`。
2. 客户端断开连接。
3. 进入 B 服务器/存档；B 的首个有效 `spirit_treasure_state` 尚未到达，或 B 后续拥有同 template 的灵宝。
4. 玩家按 T 打开灵宝面板。
5. 面板从静态 store 读取 A 的 snapshot/dialogue 并显示。
6. 若玩家右键旧灵宝，客户端向当前连接发送 `treasure_activate`；多数情况下服务端拒绝，少数 id 碰撞场景存在错操作风险。

## 反方审查记录

第一轮质疑：

- 反方指出正常 B 服可能在 join 后很快推送 `spirit_treasure_state`，因此“长期显示旧 state”不能作为无条件结论。
- 反方指出服务端会按当前玩家 inventory 校验 `instance_id`，旧 `instanceId` 多数会被拒绝，不能声称必然误激活。
- 反方确认 dialogue 残留更硬：`DialogueStore` 按 `templateId` 静态保存，B 若有同 template 灵宝，A 的对话会被挂到 B 面板。

补证/让步：

- 收窄主影响为“新 session 首个有效 state 到达前显示旧 state”，并把旧 `instanceId` 发包改为次级风险。
- 补充 `BongNetworkHandler` disconnect 清理范式：多个 static store 都在断线清理，灵宝两个 store 漏清。
- 补充 `ScrollReadScreenBootstrap`、`InsightOfferScreenBootstrap`、`LootContainerScreenBootstrap` 均有 disconnect 清理范式，说明这不是风格争议。

最终裁决：

- 反方通过候选，认定为高置信真实 bug。
- 必须保留限定：不声称旧 state 必然长期存在；不声称旧 `instanceId` 必然错激活；不把 #905 当成已覆盖。

## Skeleton Fix Plan

TODO:

- [ ] 在灵宝 client bootstrap 或统一 network disconnect 清理路径中调用 `SpiritTreasureStateStore.clear()` 与 `SpiritTreasureDialogueStore.clear()`。
- [ ] 如当前 screen 是 `SpiritTreasureScreen`，断线清理后关闭该 screen，避免空连接上继续操作旧 UI。
- [ ] 评估是否需要 join 时也清空灵宝 store，确保切服进入新 session 的第一帧不会读上一 session 数据。
- [ ] 给 `SpiritTreasureScreenBootstrap` 增加 freshness/session gate：没有当前 session 的 state 时，面板只允许显示空态，不允许对旧 snapshot 发操作包。
- [ ] 保持 `treasure_activate` 服务端校验不变，客户端只减少旧 UI 发包噪音，不依赖客户端作为安全边界。

## 验收测试计划

- [ ] 增加 client 单测：填充 `SpiritTreasureStateStore` 与 `SpiritTreasureDialogueStore` 后触发 disconnect 清理入口，断言两个 store 为空。
- [ ] 增加 screen/bootstrap 单测或可测试 helper：断线清理后当前 `SpiritTreasureScreen` 被关闭，或至少无法继续发送旧 `treasure_activate`。
- [ ] 增加 dialogue 回归：A session 写入 `spirit_treasure_jizhaojing` 对话，清理后 B session 同 template 灵宝不显示 A 对话。
- [ ] 手工验证：A 存档打开灵宝面板并产生对话，断开进入 B 存档，首包前按 T 显示“暂无灵宝”或当前 session 空态，不显示 A 的灵宝与对话。
- [ ] 手工验证：断线后右键旧面板不会向当前连接发送旧 `instanceId` 的 `treasure_activate`。

## 风险

- 清理时机过早可能短暂隐藏当前 session 的真实灵宝；需要确认 join 后由 server 正常重推 `spirit_treasure_state`。
- 如果未来设计要求跨角色保留器灵聊天历史，需要把 dialogue store 改成按 server/session/player 维度隔离，而不是完全进程级静态共享。
- 关闭 screen 可能打断玩家刚打开的面板；但断线/切服时当前 screen 已不再对应有效 server state，关闭比保留旧操作面更安全。
