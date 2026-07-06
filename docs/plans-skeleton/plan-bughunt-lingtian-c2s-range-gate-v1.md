# plan-bughunt-lingtian-c2s-range-gate-v1（骨架）

> **骨架（BugHunt B7 / server-gameplay 第七轮）**。一句话主题：灵田六类 C2S 动作只信任客户端传入的 `x/y/z`，服务端没有复验玩家当前位置与当前维度，导致多人服或自定义客户端可远程种植、收获、补灵、偷灵。

## Bug 摘要

- **核心 bug**：`lingtian_start_till / renew / planting / harvest / replenish / drain_qi` 在 server C2S dispatch 与 `lingtian::systems` 启动会话时，都没有检查玩家是否仍在主世界、是否靠近目标 plot、是否仍能合法触达目标方块。
- **建议严重性**：major。普通客户端 UI 通常从准星方块打开面板，但服务端没有权威 gate；多人服里篡改客户端、复用旧 UI 坐标或手发 `bong:client_request` 都能绕过真实站位。
- **非重复说明**：避开 #973 灵龛、#981 炼丹炉、#990 容器断线、#1004 制作台跨维误拆、#1007 掉落物跨维拾取、#1014 玩家交易跨维换货。开放 PR 中仅 #910 是泛化实体交互距离漂移，未覆盖灵田 C2S 六动作；#876 是矿脉移动打断，不是灵田门禁。

## 对实际游玩体验的影响

玩家在灵田附近按 `L` 打开面板后，当前客户端会把准星方块坐标保存进界面；但按钮点击时服务端只收到坐标和动作，不知道玩家是否已经走远、传送进 TSY、或根本是自定义客户端发包。结果是：

- 远处玩家可以收走别人已经成熟的灵田作物，背包获得掉落，目标 plot 被清空。
- 远处玩家可以偷灵，把远端 plot 的 `plot_qi` 清零，并把一部分真元注入自己。
- 远处玩家可以消耗自己的骨币/材料给指定 plot 补灵，或远程种植/翻新，造成“田在远处自己变化、主人找不到交互来源”的多人服体验断裂。
- 这不是纯 UI 瑕疵；完成路径会实际修改 `LingtianPlot`、玩家背包、玩家真元、zone qi、world block 与 LifeRecord。

## 证据定位

### C2S 参数包没有位置 / 维度输入

- `server/src/network/client_request_handler.rs:270-278` 的 `LingtianRequestParams` 只有六个 `EventWriter` 和一个 `OverworldLayer` 查询，没有 `Position` / `CurrentDimension` 查询。
- `server/src/network/client_request_handler.rs:2401-2510` 六个 `ClientRequestV1::LingtianStart*` 分支直接把客户端传入的 `x/y/z` 转成 `BlockPos` 后发 `Start*Request`；`StartTill` 只从 `OverworldLayer` 读取 terrain / environment，未验证玩家在该层附近。

### 灵田启动会话只查资源和 plot 状态

- `server/src/lingtian/systems.rs:274-320` `handle_start_till` 只查 active session、背包、主手锄、terrain。
- `server/src/lingtian/systems.rs:323-367` `handle_start_renew` 只查 active session、主手锄、目标 plot 是否贫瘠。
- `server/src/lingtian/systems.rs:370-416` `handle_start_planting` 只查种子、背包、目标 plot 是否空且非贫瘠。
- `server/src/lingtian/systems.rs:419-444` `handle_start_drain_qi` 只查目标 plot 是否有 `plot_qi`。
- `server/src/lingtian/systems.rs:447-498` `handle_start_harvest` 只查技能和目标 plot 是否成熟。
- `server/src/lingtian/systems.rs:501-570` `handle_start_replenish` 只查冷却、材料、zone qi。

以上路径都没有玩家 `Position` / `CurrentDimension` gate。

### 完成路径有真实副作用

- `server/src/lingtian/systems.rs:661-671` 开垦完成会 spawn `LingtianPlot` 并把方块设为 `FARMLAND`。
- `server/src/lingtian/systems.rs:937-958` 种植完成会扣种子并写入 `plot.crop`。
- `server/src/lingtian/systems.rs:993-1070` 收获完成会给玩家背包加作物/种子并清空 plot。
- `server/src/lingtian/systems.rs:1114-1195` 偷灵完成会清空 plot qi、注入玩家 `Cultivation.qi_current`、回流 zone qi、写 LifeRecord。
- `server/src/lingtian/systems.rs:1265-1351` 补灵完成会扣骨币/材料或 zone qi，并增加 `plot.plot_qi`。

### 客户端近距 UI 不是服务端信任边界

- `client/src/main/java/com/bong/client/lingtian/LingtianActionScreenBootstrap.java:48-64` 正常 UI 打开时只 snapshot 当前准星方块。
- `client/src/main/java/com/bong/client/lingtian/LingtianActionScreen.java:84-93,129-148` 按钮点击时复用保存的 `target` 坐标发送请求，不在 server 侧绑定 UI 会话或重新 raycast。
- 对比已有权威 gate：普通容器打开会在 server 检查维度和距离（`server/src/world/container_open.rs:85-103`）。灵田缺少同等 gate。

## 触发路径

1. 世界中已有一个 `LingtianPlot`，例如玩家 A 的成熟作物或带 `plot_qi` 的田。
2. 玩家 B 不站在该 plot 附近，甚至已经进入 TSY 或主世界远处。
3. 玩家 B 用自定义客户端或复用旧 UI 坐标发：
   - `{"type":"lingtian_start_harvest","v":1,"x":...,"y":...,"z":...,"mode":"manual"}`
   - 或 `lingtian_start_drain_qi / planting / replenish / renew`。
4. server dispatch 直接产生 `StartHarvestRequest` 等事件。
5. `handle_start_*` 只按 `BlockPos` 找 plot 并开会话。
6. session 到时后 `apply_completed_sessions` 修改 plot / 背包 / qi。

## 反方审查记录

### Round 1

- **反方问题**：是否有上游客户端或服务端统一 range gate？是否只是 UI 近距入口？
- **结论**：成立。`LingtianRequestParams` 没有 `Position` / `CurrentDimension`；六个 `LingtianStart*` 分支直接转发坐标；`handle_start_*` 只查资源和 plot 状态；完成路径会实际改背包、plot、qi 和方块。正常客户端的准星 snapshot 不能作为服务端信任边界。

### Round 2

- **反方问题**：灵田是否本来 Overworld-only，因此“跨维”不是 bug？#910 / #876 是否已覆盖？`StartTill` 是否因 chunk / terrain 限制而影响较低？
- **结论**：继续写 plan。灵田确实近似 Overworld-only，所以表述应聚焦“服务端距离 / 主世界门禁缺失”，不是“TSY plot 数据模型”。`StartTill` 单独影响较低，但已有 plot 的种植、收获、补灵、偷灵只按 `BlockPos` 查找，足以构成真实多人服权威缺口。#910 / #876 未覆盖灵田六类 C2S。
- **保留意见**：不要写成普通玩家主路径随手触发；应明确主要风险来自自定义客户端、手发 payload、旧 UI 坐标和多人服服务端权威缺口。

## Skeleton Fix Plan

### P0：建立统一灵田交互门禁

- 在 server 端新增灵田交互 gate helper，例如 `validate_lingtian_interaction(player, pos, positions, dimensions)`。
- 要求玩家当前维度为 `DimensionKind::Overworld`，因为当前 `LingtianPlot` / `OverworldLayer` 模型没有多维字段。
- 要求玩家位置到目标方块中心距离不超过灵田交互范围；建议复用已有近距交互语义，允许少量容差，避免边界误拒。
- `LingtianRequestParams` 或 `lingtian::systems` 启动 handler 必须拿到 `Position` / `CurrentDimension` 并统一调用 gate。

### P1：所有六类 Start 请求接入 gate

- `LingtianStartTill`：读 terrain 前或发 `StartTillRequest` 前先 gate，避免远程读取和远程开田。
- `LingtianStartRenew / Planting / Harvest / Replenish / DrainQi`：发事件前 gate，或在 `handle_start_*` 内 gate；推荐在 server C2S dispatch 前置拒绝并给玩家明确反馈。
- 拒绝时不要消耗材料、不要创建 active session、不要发完成事件。

### P2：防止 session 中途漂移

- 长动作开始后记录起点或目标维度，tick / 完成前复验玩家仍在主世界且没有远离目标。
- 对 `Harvest / Replenish / DrainQi` 这类有资源副作用的动作尤其要在完成前再复验一次，避免“起手合法，传送/跑远后仍结算”。

## 验收测试计划

- server 单测：远距离玩家发送 `StartHarvestRequest` 不应创建 `ActiveSession`，成熟 plot 不变，背包不加物品。
- server 单测：TSY 玩家对主世界 plot 发送 `StartDrainQiRequest` 不应创建 session，plot_qi 不变，玩家 `qi_current` 不变。
- server 单测：近距离主世界玩家可正常 harvest / replenish / drain_qi，避免误杀正常玩法。
- server 单测：合法起手后玩家移动超出范围或切维，完成阶段取消，不扣材料、不发奖、不改 plot。
- 回归：`LingtianStartTill` 对近距离可耕地仍能读取 terrain 并开垦；远距离可耕地不能开垦。

## 风险

- 灵田系统当前支持 NPC 散修 actor 复用事件，直接把 gate 写死为 `Client` 查询可能误伤 NPC 自动耕作；修复时需要区分 C2S 玩家请求与内部 NPC 行为，或给内部系统单独入口。
- 如果把 gate 放在 `lingtian::systems` 层，测试和 NPC 调用都要补齐 actor position / dimension；如果放在 `client_request_handler` 层，仍需完成阶段复验来堵传送/移动中途漂移。
- `LingtianPlot` 目前无维度字段，修复不应顺手扩成多维灵田大重构；本 skeleton 的最小修复是 Overworld-only 门禁 + 距离复验。

## 本轮取证说明

- 本轮只新增本 skeleton 文档，不修改实际代码、配置、依赖或资源。
- 未消费、未归档任何 plan。
- 已先查开放 PR，并对 #973 / #981 / #990 / #1004 / #1007 / #1014 以及 #910 / #876 做去重。
