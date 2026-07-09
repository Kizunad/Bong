# BugHunt: 阵法布置缺少服务端空间门禁

> server-gameplay r09。主题：`zhenfa_place` 只信客户端提交的裸坐标，服务端布阵消费端没有读取玩家 `Position` / `CurrentDimension`，导致玩家可远程、跨维向主世界写入阵眼并结算材料/真元。

## Bug 摘要

`ClientRequestV1::ZhenfaPlace` 在网络层把 `x/y/z` 直接封装成 `ZhenfaPlaceRequest { pos: [x, y, z], ... }`。消费端 `handle_zhenfa_place_requests` 只查询玩家的用户名、修为、真元色、背包、断脉和阵法熟练度，不查询玩家位置或当前维度；随后直接对 `OverworldLayer` 调 `place_zhenfa_anchor_block`。

这意味着 `zhenfa_place` 的空间授权只存在于客户端右键/布阵 UI 的正常路径里，服务端权威层没有同维度和距离校验。自定义客户端或陈旧 UI 可以在不靠近目标点、甚至人在 TSY 等非主世界维度时，对主世界远处坐标布置陷阱、灵聚、网阵等阵法。

## 实际游玩体验影响

- 玩家可以隔空在别人的行进路线、家门口或资源点布置凡阵陷阱，不需要实际潜入现场。
- 玩家人在非主世界维度时，也可能通过裸坐标请求向主世界 `OverworldLayer` 写阵眼，形成跨维远程施工。
- 成功路径会真实注册 `ZhenfaInstance`、写入自定义方块、消耗材料/道具并结算真元，影响不是 UI 假象。
- 普通玩家会感知为“周围突然出现从未有人到场布置的阵法/陷阱”，破坏阵法玩法的风险、潜入和地形控制语义。

## 证据定位

- `server/src/network/client_request_handler.rs:1545-1570`：`ZhenfaPlace` 分支直接把客户端坐标转发为 `ZhenfaPlaceRequest { pos: [x, y, z], ... }`，没有附带维度。
- `server/src/zhenfa/mod.rs:136-145`：`ZhenfaPlaceRequest` 结构只有玩家、裸坐标、阵法 kind、载体、投入比例、道具等字段，没有维度或服务端命中上下文。
- `server/src/zhenfa/mod.rs:1577-1592`：`handle_zhenfa_place_requests` 参数里 `players: Query<ZhenfaPlacePlayer<'_>>`，没有独立的玩家 `Position` / `CurrentDimension` 查询。
- `server/src/zhenfa/mod.rs:1602-1705`：玩家校验只覆盖修为、背包道具、阵旗、境界、断脉、材料等 gameplay 条件。
- `server/src/zhenfa/mod.rs:1763-1779`：通过上述校验后，直接 `place_zhenfa_anchor_block(&mut layers, req.pos, ...)` 写主世界层。
- `server/src/zhenfa/mod.rs:1792-1815`：随后 spawn 阵眼实体并注册 `ZhenfaInstance`，位置仍来自客户端裸坐标。
- `server/src/zhenfa/mod.rs:2759-2767`：`ZhenfaPlacePlayer` 类型不包含位置或维度，证明消费端无法做同维/距离授权。

## 去重说明

- 不是 `plan-zhenfa-array-flag-e2e-wiring-v1`：该 skeleton 关注阵旗/e2e 接线；本题是 server 已接收 `zhenfa_place` 后缺少权威空间授权。
- 不是 `plan-zhenfa-trap-client-equip-gate-v1`：该 active plan 关注 client 装备规则与右键入口；本题不改客户端入口，关注服务端 C2S 防线。
- 不重复 #1048、#1055、#1060、#1065、#1073、#1088、#1095；这些分别是满包吞产物、锻炉/物资棺/制作台/修理站/棺/炼丹 freshness 等主题。
- 不选择炼丹炉、垂死大能、灵田候选，因为已有 `plan-bughunt-alchemy-furnace-scope-gate.md`、`plan-bughunt-dying-elder-give-dan-input-v1.md`、`plan-bughunt-lingtian-c2s-range-gate-v1.md`。

## Skeleton Fix Plan

- [ ] 在 `zhenfa_place` 消费端加入统一 scope helper，读取玩家 `Position` 与 `CurrentDimension`。
- [ ] 要求布阵目标与玩家同维；当前阵眼写入只支持 `OverworldLayer` 时，非 Overworld 玩家请求必须在任何写块、扣材料、扣真元前拒绝。
- [ ] 增加布阵距离上限，普通陷阱/阵眼必须在玩家可交互距离内；如经典布阵 UI 允许预览远点，也必须在确认提交时由服务端重算距离。
- [ ] 拒绝路径保持原子：不得写自定义方块、不得注册 `ZhenfaInstance`、不得消耗 `item_instance_id`、不得扣 `cultivation.qi_current`。
- [ ] 普通陷阱、灵聚、欺天、幻阵、网阵统一走同一 helper，避免新增 kind 后绕过。

## 验收测试计划

- [ ] server 单测：玩家在 Overworld 且目标在范围内，合法 `zhenfa_place` 成功写块、注册实例并结算成本。
- [ ] server 单测：玩家在 Overworld 但目标超出范围，拒绝且背包、真元、registry、方块层都不变。
- [ ] server 单测：玩家在非 Overworld 发送主世界裸坐标，拒绝且无任何副作用。
- [ ] server 单测：普通陷阱带 `item_instance_id` 的拒绝路径不消耗道具。
- [ ] server 单测：灵聚/网阵等非普通陷阱同样受 scope helper 保护。
- [ ] bot e2e：用 `bong:client_request` 发送远距/跨维 `zhenfa_place`，断言连接保持、请求被拒绝、阵法未出现。

## 对抗审查记录

Round 1（主代理本地审计）：最初候选包括炼丹炉 scope、灵田 C2S range、垂死大能给丹、外部容器 move。炼丹炉/灵田/垂死大能均已有 skeleton；外部容器方向与已知 session gate 过近，降级为备选。

Round 2（subagent 对抗）：独立审查确认上述 3 个候选重复，并提出棺材交互和阵法布置两个新候选。棺材候选与用户明确排除的 #1088 普通延寿棺跨维门禁相撞，丢弃；阵法布置候选经代码复核成立，且不命中现有阵法客户端/e2e skeleton。

反方论点：阵法也许设计允许远程布阵。驳回：现有 client 入口仍是右键/布阵 UI，服务端消费端却直接写 `OverworldLayer` 并结算成本；若远程布阵是设计，应有明确的距离规则、可见反馈、目标维度字段和成本语义，而不是完全缺少 `Position` / `CurrentDimension` 查询。

## 本轮范围

本 PR 只新增 bughunt plan 文档，不修改代码、配置、资源或依赖。
