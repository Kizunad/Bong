# BugHunt：全力一击蓄力 HUD 断线跨会话残留

> 分区：client-combat / r08
> 结论：高置信真 bug。只记录 plan，不在本分支修复代码。

## 一句话

玩家在全力一击蓄力中断线时，客户端 `FullPowerStateStore` 的 charging 态不会被断线清理；下一次进入任意服务器前后，`ChargingProgressBarHud` 仍可能渲染旧会话的“蓄力中 X/Y 真元”进度条。

## 实际游玩体验影响

实战中玩家按下全力一击蓄力后如果掉线、切服或回到标题再进服，屏幕底部会残留上一局的全力一击蓄力条。这个条不代表当前角色真实正在蓄力，容易误导玩家以为技能仍在准备、真元仍被提交或服务器状态卡住；在 PvP/越级偷袭这种需要读条反制的场景，会直接污染战斗 HUD 判断。

## 复现链路

1. 进入服务器，触发 `bao_mai.full_power_charge`，客户端收到 `full_power_charging_state active=true`。
2. 在释放或被打断前断线，或者切到另一个连接。
3. 客户端断线清理执行，但未清理 `FullPowerStateStore`。
4. 再次进入游戏后，HUD orchestrator 每帧调用 `ChargingProgressBarHud.buildCommands(...)`，继续读到旧的 active charging state 并渲染旧进度条。

## 根因证据

- `client/src/main/java/com/bong/client/combat/store/FullPowerStateStore.java:6-8`：`charging` 是进程级 singleton 状态；`FullPowerStateStore.java:28-30` 只有显式 `clearCharging()`；`FullPowerStateStore.java:44-48` 的全量 reset 仅测试可见，生产断线清理没有调用。
- `client/src/main/java/com/bong/client/network/FullPowerStateHandler.java:39-45`：收到 `active=true` 时写入 charging，只有收到 `active=false` 时清掉；`FullPowerStateHandler.java:64-67` 释放事件也会清掉，但断线不会自动触发这两个 payload。
- `client/src/main/java/com/bong/client/hud/ChargingProgressBarHud.java:18-38`：HUD 只判断 `state.active()`，没有 TTL、连接代际、玩家 UUID 或 session gate。
- `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java:96-120`：断线会清理大量 combat HUD store，但漏掉 `FullPowerStateStore`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:857-895`：总断线清理路径也清理了多类跨会话 store，但漏掉 `FullPowerStateStore`。
- `server/src/network/full_power_emit.rs:26-48`：服务端只对 `Changed<ChargingState>` 发 active charging payload；`server/src/network/full_power_emit.rs:66-84` 只在 release/interrupted 事件上发 clear。断线中途客户端收不到 release/interrupted clear 时，服务端不会在新连接上为旧客户端补发 inactive 快照。

## 去重结论

- 已按要求先跑 `gh pr list --state all --limit 470 --json number,title,headRefName,url` 做过去重；本主题不重复 #1051 绝脉断链 HUD false positive、#1057 VFX/SFX 跨维广播、#1063 逆脉护体缺失独立动画、#1074 涡流虚蚀五招缺 PlayerAnimator JSON/fallback、#1085 shield_raise loop 边界不闭合。
- 已额外搜索 `docs/plan-*.md`、`docs/plans-skeleton/*.md`、`docs/finished_plans/*.md` 中的 `full_power` / `全力` / `蓄力` / `ChargingProgressBar` / `FullPowerStateStore`。已有内容主要是 `plan-baomai-v2` 的功能实现与 `plan-baomai-v3` 集成说明，以及一个已存在的 baomai v3 A/V 双源骨架；未发现“全力一击蓄力 HUD 断线跨会话残留”的 bughunt plan。
- 曾评估并放弃 `Dugu v2 HUD disconnect bleed`，因为它已在 `docs/plans-skeleton/plan-bughunt-dugu-v2-hud-disconnect-bleed-v1.md` 和既有 PR 主题中出现；本 plan 不复用该主题。

## 对抗审查记录

- Round 1：对抗 subagent 提出独孤 v2 HUD 跨会话残留。主线核对后发现是技术上可成立但已有骨架/PR 覆盖的重复主题，淘汰。
- Round 2：对抗 subagent 复审独孤 v2，仍确认重复，不建议作为本轮候选。随后主线改查全力一击 HUD store 生命周期。
- 本候选经本地反证：服务端 active/clear 生命周期依赖 `Changed<ChargingState>` 与 release/interrupted 事件，客户端断线清理列表确实漏掉 `FullPowerStateStore`，HUD 又无自然过期，因此未被推翻。

## 修复 TODO

- [ ] 在生产断线清理路径中清理 `FullPowerStateStore`，优先接入 `CombatHudBootstrap.resetOnDisconnect()` 或 `BongNetworkHandler.clearClientStateOnDisconnect()` 的现有 store reset 队列。
- [ ] 为 `FullPowerStateStore` 增加明确的 production clear API，避免继续依赖 `resetForTests()` 命名进入生产路径。
- [ ] 补 client 单测：模拟 active charging payload 后调用断线清理，断言 `ChargingProgressBarHud.buildCommands(...)` 不再产生蓄力条。
- [ ] 补 handler/HUD 边界测试：`active=false`、release payload、断线 reset 三条路径都必须清掉 charging；`exhausted` 若继续依赖剩余 tick 可单独保留，但不能影响 charging。

## 验收

- `cd client && ./gradlew test build`
- 手动：蓄力全力一击期间断线重连，重进后底部不再出现旧“蓄力中 X/Y 真元”条；重新正常蓄力时 HUD 仍按新 payload 渲染。
