# BugHunt：涡流 HUD 断线短窗口残留

## 结论

`VortexStateStore` 是客户端静态 HUD 状态，`vortex_state` payload 写入后会被 `BongHudOrchestrator` 每帧读取并渲染涡流面板、反噬 vignette、紊流 tint 等反馈。但断线清理链没有重置它，玩家在涡流施放、冷却、反噬或紊流显示期间断线/切服后，新连接首帧到下一次 server baseline 前会短暂看到上一连接的涡流 HUD 状态。

严重性：中低。server 当前每秒会发一次 inactive baseline，正常重连到 Bong server 后通常会在约 1 秒内自愈；但断线后的客户端状态不应依赖下一包覆盖，切到无对应 payload 的环境时也可能持续误导。

## 证据

- `client/src/main/java/com/bong/client/combat/handler/VortexStateHandler.java:16`-`31`：`vortex_state` 被解析后直接 `VortexStateStore.replace(next)`。
- `client/src/main/java/com/bong/client/combat/store/VortexStateStore.java:33`-`44`：状态是 static volatile snapshot，生产路径没有 clear，只有 `resetForTests()` 可回到 `State.NONE`。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:318`-`323`：主 HUD 每帧读取 `VortexStateStore.snapshot()` 交给 `WoliuV2HudPlanner`。
- `client/src/main/java/com/bong/client/hud/WoliuV2StatusPanelHud.java:33`-`40`：只要 active skill、冷却、反噬或紊流可见任一成立，就渲染涡流状态面板。
- `client/src/main/java/com/bong/client/hud/BackfireWarningHud.java:23`-`31`：旧 active + backfire 会继续渲染反噬边缘警示。
- `client/src/main/java/com/bong/client/hud/TurbulenceFieldVisualizeHud.java:20`-`24`：旧 active + turbulence 会继续渲染屏幕 tint。
- `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java:96`-`120`：combat 断线 reset 清理了大量 store，但没有调用 `VortexStateStore.resetForTests()`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:857`-`900`：另一条断线清理链也没有清理 `VortexStateStore`。

## 最强反证

`server/src/network/woliu_state_emit.rs:48`-`70` 显示 server 会按秒周期发送 `vortex_state` baseline，即使 inactive 也会发；因此这不是高置信永久残留。现象更准确地说是断线后到下一次 baseline 之前的短窗口串会话。已有 HUD 测试也覆盖了 inactive residual turbulence 不应单独保活 HUD，所以不要把问题描述成“紊流永久续命”。

## 实际游玩体验影响

玩家在涡流施放、反噬或紊流可见时掉线/切服，新会话刚进入游戏时可能仍看到上一局的“涡流/冷却/反噬/紊流”面板、反噬 vignette 或屏幕 tint。PVP 或读招场景中，这会误导玩家判断当前角色是否仍处在涡流风险、是否还有冷却、是否发生反噬；即使通常约 1 秒后被 server baseline 覆盖，首帧错误反馈仍会造成错误操作和视觉噪声。

## 去重

不重复近期禁止主题：

- #1051 是绝脉断链 HUD false positive。
- #1057 是 VFX/SFX 跨维广播。
- #1063 是逆脉护体缺独立动画。
- #1074 是涡流虚蚀五招缺 PlayerAnimator JSON/fallback。
- #1085 是 `shield_raise` loop 边界不闭合。
- #1094 是全力一击蓄力 HUD 断线残留，涉及 `FullPowerStateStore`，不是 `VortexStateStore`。
- #1068-#1072 相关主题未覆盖该客户端 store 生命周期缺口。

## 对抗结论

- 第 1 轮 subagent：支持候选，要求将影响降级为“重连首帧/短窗口串会话”，因为 server 每秒 inactive baseline 可自愈。
- 第 2 轮 subagent：支持开 BugHunt plan，定级偏小型 fix；根因高置信，影响窗口受 baseline 限制。

## 修复计划

- [ ] 在断线清理链中重置 `VortexStateStore`，优先接入 `CombatHudBootstrap.resetOnDisconnect()`，必要时同步评估 `BongNetworkHandler.clearClientStateOnDisconnect()` 是否也应覆盖。
- [ ] 为 `CombatHudBootstrapTest` 增加断线清理 pin：先写入非 `NONE` 的 `VortexStateStore.State`，调用 `resetOnDisconnect()` 后断言回到 `State.NONE`。
- [ ] 补一条 HUD 回归：旧 active/backfire/turbulence 状态断线清理后，`WoliuV2HudPlanner` 不再产生命令。
