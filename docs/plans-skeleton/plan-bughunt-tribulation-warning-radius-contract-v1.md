# plan-bughunt-tribulation-warning-radius-contract-v1（骨架）

> **来源 issue**：#1412。承雷危险半径与可前往观战邀请是两个门，客户端当前把它们显示成一条 50 格提示。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 区分 50 格 spectate invite 与 100 格 danger warning | ⬜ |

## §0 摘要

`TribulationBroadcastHudPlanner` 只在 `SPECTATE_HINT_DISTANCE=50.0` 内显示文案，却写着“100 格内会承雷”。server 的 `SPECTATE_INVITE_RADIUS` 确实是 50，而 `TRIBULATION_DANGER_RADIUS` 是 100；当前 payload 没有单独的危险提示门，导致 50–100 格观战者没有明确的受击警告。

## §1 游玩影响

50–100 格玩家能看到天劫广播并可能被 AOE 命中，却看不到与危险半径一致的提示；若直接把邀请门改成 100，又会错误开放观战入口。

## §2 复现路径

在天劫中心外 75 格接收 `TribulationBroadcastV1`，观察 `spectate_invite`/HUD 文案：server 仍可按 `TRIBULATION_DANGER_RADIUS` 判定伤害，client 因距离大于 50 不渲染 hint。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/hud/TribulationBroadcastHudPlanner.java:20,82-84` 将显示门固定为 50，并在文案中宣称 100 格内承雷。
- `server/src/network/tribulation_broadcast_emit.rs:21-23,229-239` 的 `SPECTATE_INVITE_RADIUS=50.0` 只控制 `spectate_invite`；`server/src/cultivation/tribulation.rs:67,1421` 的 `TRIBULATION_DANGER_RADIUS=100.0` 控制受击范围。
- 共享 payload 是 `server/src/schema/server_data.rs:1275-1314` 的 `TribulationBroadcastV1`，当前没有“danger warning”字段。

## §4 非重复比对

已查 tribulation broadcast、AOE、client HUD 的 finished/active plans 与 skeleton；现有文档覆盖广播生命周期和伤害结算，没有覆盖“邀请半径与危险半径分离显示”。

## §5 立项检查记录

- `docs/worldview.md §四 L313-L382`：查越级战斗、天劫和距离风险；不改变天劫伤害公式。
- `docs/finished_plans/`：查 tribulation、combat feedback、broadcast payload；确认 50 格邀请门是既有契约。
- active plan：查 `TribulationBroadcastV1`、`SPECTATE_INVITE_RADIUS`、`TRIBULATION_DANGER_RADIUS`；未见 warning 字段接线。
- skeleton：查 `spectate_invite`、`spectate_distance`、`danger_radius`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server 的 `spectate_distance`、邀请布尔值、危险半径和当前天劫状态。
- **Outputs**：50 格内显示可观战入口；100 格内显示承雷风险；不把风险提示当成可观战授权。
- **共享类型/事件**：复用 `TribulationBroadcastV1`/client `TribulationBroadcastStore.State`；若增加 danger 字段，同步 Rust schema、proto/JSON bridge 和 client handler。
- **三端契约符号**：server `tribulation_broadcast_emit::TribulationBroadcastClientView`、`SPECTATE_INVITE_RADIUS`、`tribulation::TRIBULATION_DANGER_RADIUS`；agent **无变更**，广播不由 agent 决定；client `TribulationBroadcastHandler`/`TribulationBroadcastHudPlanner`。
- **worldview/qi_physics**：天劫风险遵循 `docs/worldview.md §四`；受击真元结算继续走 server `qi_physics` ledger，本骨架不新增转账。

## P0 验收

- 50 格邀请门保持不变；75 格玩家得到危险提示；100 格边界与 server 使用同一常量语义。
- payload 缺少新字段时按安全默认处理，不把远端客户端显示变成授权门。
