# plan-bughunt-realm-vision-minor-cleanups-v1（骨架）

> **来源 issue**：#1671、#1690、#1539、#1534。境界视界的 server 环境参数接线、ramp/天气恢复竞态，以及客户端距离边界和屏幕方向计算没有形成一条生产链路。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | fog 命令受本地视距约束，神识边缘标记左右方向正确 | ⬜ |
| P1 | 低视距与左右边界回归 | ⬜ |

## §0 摘要

`realm_vision::push` 生产入口只调用 `compute_base_params`，`compute_vision_params` 的环境/状态修正未接入；server 的 `view_distance_ramp_system` 又可能在天气恢复后留下高于境界目标的视距。客户端 `RealmVisionPlanner.clampToRenderDistance` 只有测试调用，`RealmVisionFogController.apply` 直接使用未裁剪的 `plan` 结果；低视距客户端可能看不到应该出现的雾。`PerceptionEdgeProjector` 把 yaw 方向的左向量当作右轴，屏外神识目标的边缘标记左右镜像。

## §1 游玩影响

境界视界雾效在低 render distance 下会失效，神识提示会把玩家引向相反方向。server 下发的 realm vision 和 spiritual sense 目标仍应保持权威，问题在 client 解释层。

## §2 复现路径

1. 视距设为 6 chunks，接收 Void 境 `realm_vision_params`，观察 fog end 超出本地可见范围且生产 sink 未裁剪。
2. yaw=0 时把目标放在玩家 +X/右侧，观察 `PerceptionEdgeProjector` 把屏外标记推到左缘。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/visual/realm_vision/RealmVisionPlanner.java:8-18,21-39` 的 `plan` 返回插值结果，但 `clampToRenderDistance` 未被生产调用；`RealmVisionFogController.java:10-14` 直接将 plan 交给 sink。
- `client/src/main/java/com/bong/client/visual/realm_vision/PerceptionEdgeProjector.java:27-48` 令 `rx=cos(yaw), rz=sin(yaw)`，随后用 `vr=dx*rx+dz*rz` 作为右轴；在 MC yaw=0 约定下该向量是左向量。
- `server/src/cultivation/realm_vision/push.rs:16-33` 生产推送只调用 `compute_base_params`；`server/src/cultivation/realm_vision/planner.rs:40-70` 的 `compute_vision_params`/环境修正无生产 caller（#1534）。
- `server/src/cultivation/realm_vision/view_distance_ramp.rs:51-55` 收敛时移除 ramp；`server/src/world/weather_physics/vision.rs:32-49` 恢复进入雾前快照，竞态可把值抬回境界目标以上（#1539）。
- server `server/src/cultivation/realm_vision/push.rs:99-114` 下发 `RealmVisionParamsV1`，`server/src/schema/realm_vision.rs`/client `SpiritualSenseTargetsHandler` 下发目标坐标，均没有 client 端方向修正。

## §4 非重复比对

已查 realm vision、spiritual sense 的 finished/active plans 和 skeleton；已有计划定义 server 参数和 payload，没有覆盖生产 fog sink 接入及 projector 右轴。两个 issue 共享 client 视界解释边界，分别验收。

## §5 立项检查记录

- `docs/worldview.md §三 L61-L207`：查境界与视野层次；不改变 server 境界规则。
- `docs/finished_plans/`：查 realm vision、spiritual sense 和 render-distance advisor。
- active plan：查 `RealmVisionPlanner`、`RealmVisionFogController`、`PerceptionEdgeProjector`、`RealmVisionParamsHandler`，未见同一接入修法。
- skeleton：查 `clampToRenderDistance`、`GlFogParamsSink`、`resolveWireId` 及左右向量测试；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server `RealmVisionParamsV1` 的 fog 参数、本地 render distance、`SpiritualSenseTargetsV1` 的坐标和相机 yaw/pitch。
- **Outputs**：裁剪后的 fog sink 命令、方向正确的 edge indicator；不修改 server payload 数值。
- **共享类型/事件**：复用 `realm_vision_params`、`spiritual_sense_targets`、`RealmVisionCommand`、`PerceptionEdgeProjector.EdgeIndicatorCmd`。
- **三端契约符号**：server `realm_vision::push::send_realm_vision_params`/`SpiritualSenseTargetsV1`；agent **无变更**，这些 server-data 直接给 client；client `RealmVisionParamsHandler`、`RealmVisionFogController`、`PerceptionEdgeProjector`。
- **worldview/qi_physics**：视界表现锚定 `docs/worldview.md §三`；神识/境界状态的真元来源仍是 server ledger，本骨架不新增 transfer。

## P0/P1 验收

- fog start/end 在 production sink 前不超过当前客户端视距上限；视距变化后下一帧采用新上限。
- yaw=0 的 +X 目标落在右侧，反向/背后和上下边界保持原有语义。
