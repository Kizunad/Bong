# BugHunt：全力一击蓄力 HUD 断线跨会话残留

> 一句话主题：全力一击蓄力 HUD 的 `FullPowerStateStore` charging 态缺少 disconnect session 边界，导致重连后 `ChargingProgressBarHud` 可能继续渲染上一会话蓄力条。
> 分区：client-combat / r08。结论：高置信真 bug。只记录 plan，不在本分支修复代码。

## 一句话

玩家在全力一击蓄力中断线时，客户端 `FullPowerStateStore` 的 charging 态不会被断线清理；下一次进入任意服务器前后，`ChargingProgressBarHud` 仍可能渲染旧会话的“蓄力中 X/Y 真元”进度条。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 | 可核验抓手 |
|------|------|------|----------|------------|
| P0 | 断线生命周期契约 | ⬜ | 待验收 | `FullPowerStateStore.clearOnDisconnect()`、`CombatHudBootstrap.resetOnDisconnect()` |
| P1 | handler / HUD 清理边界测试 | ⬜ | 待验收 | `FullPowerStateHandlerTest`、`ChargingProgressBarHudTest`、disconnect reset 单测 |
| P2 | 正常蓄力链路回归 | ⬜ | 待验收 | `active=true` 渲染、`active=false`/release/disconnect 三路清理 |
| P3 | 手动重连验证 | ⬜ | 待验收 | 蓄力中断线重连后无旧“蓄力中 X/Y 真元”条 |
| P4 | 低负载 client gate | ⬜ | 待验收 | `cd client && ./gradlew test build --max-workers=1` |
| P5 | closeout / 归档证据 | ⬜ | 待验收 | `## Finish Evidence`、关键 commit、测试结果、已知限制 |

## P0 — 断线生命周期契约

- 在 `client/src/main/java/com/bong/client/combat/store/FullPowerStateStore.java` 增加生产用 `clearOnDisconnect()`，不得让生产路径复用 `resetForTests()` 命名。
- 唯一断线挂点锁定为 `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java` 的 `ClientPlayConnectionEvents.DISCONNECT` 回调，即 `CombatHudBootstrap.resetOnDisconnect()`；它已经负责 combat HUD stores，`FullPowerStateStore` 归入这里清理。
- `clearOnDisconnect()` 至少清掉 `charging`；若保留 `exhausted` / `lastRelease`，必须说明它们不会驱动跨会话 HUD，否则一并 reset 为 inactive / empty。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java` 的 `clearClientStateOnDisconnect()` 只作为“全局断线清理也未覆盖”的证据，不作为本修复的第二挂点，避免同一 store 生命周期分摊到两个入口。

## P1 — handler / HUD 清理边界测试

- 在 `client/src/test/java/com/bong/client/network/FullPowerStateHandlerTest.java` 覆盖 `active=false` 与 `full_power_release` 都会清掉 `FullPowerStateStore.charging()`。
- 在 `client/src/test/java/com/bong/client/hud/ChargingProgressBarHudTest.java` 覆盖 active charging 后调用断线 reset，`ChargingProgressBarHud.buildCommands(...)` 必须为空。
- 新增或扩展 disconnect reset 单测，直接驱动 `CombatHudBootstrap.resetOnDisconnect()`，断言 `FullPowerStateStore.charging().active()` 为 false。

## P2 — 正常蓄力链路回归

- `full_power_charging_state active=true` 仍能写入 `FullPowerStateStore.ChargingState`，HUD 仍显示进度条和“蓄力中 X/Y 真元”文本。
- `full_power_charging_state active=false`、`full_power_release`、disconnect reset 三条路径都必须清掉 charging，且不互相依赖到达顺序。
- `exhausted` 若继续依赖剩余 tick 显示，必须证明它不影响 `ChargingProgressBarHud`；若有跨会话风险，纳入 P0 同一 `clearOnDisconnect()`。

## P3 — 手动重连验证

- 同一 Minecraft 进程内，连接服务器 A，触发 `bao_mai.full_power_charge` 后在释放或打断前断线。
- 连接服务器 B 或同服重连后，底部不得出现上一会话的“蓄力中 X/Y 真元”条。
- 重新正常蓄力时，HUD 必须只按新连接收到的 `full_power_charging_state` 渲染。

## P4 — 低负载 client gate

- 按 client 栈约定使用 JDK 17。
- 修复 PR 跑 `cd client && ./gradlew test build --max-workers=1`；若只改测试可先跑定向测试，但 closeout 前需有上述 gate 或明确阻塞原因。

## P5 — closeout / 归档证据

- 全部阶段完成后补 `## Finish Evidence`，列出关键 commit、测试命令、手动重连结果、未覆盖限制。
- 归档时通过标准 plan 流程迁入 `docs/finished_plans/`，不在本 bughunt plan-only PR 中提前归档。

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

- [ ] 在 `CombatHudBootstrap.resetOnDisconnect()` 中调用 `FullPowerStateStore.clearOnDisconnect()`；`BongNetworkHandler.clearClientStateOnDisconnect()` 不作为本修复挂点。
- [ ] 为 `FullPowerStateStore` 增加明确的 production clear API，避免继续依赖 `resetForTests()` 命名进入生产路径。
- [ ] 补 client 单测：模拟 active charging payload 后调用断线清理，断言 `ChargingProgressBarHud.buildCommands(...)` 不再产生蓄力条。
- [ ] 补 handler/HUD 边界测试：`active=false`、release payload、断线 reset 三条路径都必须清掉 charging；`exhausted` 若继续依赖剩余 tick 可单独保留，但不能影响 charging。

## 验收

- `cd client && ./gradlew test build`
- 手动：蓄力全力一击期间断线重连，重进后底部不再出现旧“蓄力中 X/Y 真元”条；重新正常蓄力时 HUD 仍按新 payload 渲染。
