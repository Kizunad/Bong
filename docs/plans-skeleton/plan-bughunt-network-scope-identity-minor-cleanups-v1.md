# plan-bughunt-network-scope-identity-minor-cleanups-v1（骨架）

> **来源 issue**：#1778、#1745、#1605、#1461、#1419、#1415、#1455。它们都是 network emit 在受众范围、实体身份或字段语义上的边界错误；按子项逐条验收，不把不相关的业务状态合并。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 受众按隐私/维度过滤，实体使用稳定身份，wire 字段不再伪造或错位 | ⬜ |

## §0 摘要

当前 burst meridian 向所有 client 广播完整经脉快照；void erosion 没有 dimension gate；NPC bubble 用 `Entity` 调试串当玩家身份；DamageTilt 固定 entity 0；spider 只比 `Entity::index()`；settled/cleared tribulation 把 char_id、actor_name、(0,0) 混用。它们会泄露信息、把效果发给错误实体或让 HUD 丢失语义。

## §1 游玩影响

玩家能看到不属于自己维度/实体的视觉和战斗反馈，或被广播经脉完整度等隐私；generation 复用还可能把旧蛛的效果发给新实体。tribulation HUD/agent 叙事会出现空名、错误 actor 或 char_id 显示。真元结算本身不在本计划范围。

## §2 复现路径

1. 连接多个玩家/维度并触发 BurstMeridianEvent、VoidErosion visual，观察非目标客户端仍收到。
2. 让一个 client entity 断线后由新实体复用 index，触发 spider event；比较收到的 entity id。
3. 触发 NPC bubble、DamageTilt、settled/cleared tribulation，核对 identity、actor_name、char_id 与原事件。

## §3 今天 `origin/main` 的证据

- `server/src/network/burst_event_emit.rs:8-43` 遍历所有 `Client`；payload 含 `BurstMeridianEventV1.integrity_snapshot/overload_ratio`，来源 `server/src/cultivation/burst_meridian.rs:147-195`。
- `server/src/network/void_erosion_visual_emit.rs:45-50,83-115` 的 client query/广播只按距离 `:108-111`，未比较 CurrentDimension。
- `server/src/network/npc_bubble.rs:89-112,236-257` 在 `:108-109` 把 `format!("{client_entity:?}")` 传给 memory bubble identity。
- `server/src/network/combat_event_emit.rs:106-114` 写 `DamageTiltS2c.entity_id=VarInt(0)`；`spider_disguise_emit.rs:205-224,252-270` 只比 `entity.index()`。
- `server/src/network/tribulation_state_emit.rs:117-124` 把 `char_id` 写入 `actor_name`；`tribulation_broadcast_emit.rs:123-131` cleared 分支伪造 `active("", stage, 0, 0, ...)`。

## §4 非重复比对

已查现有 burst/tribulation/woliu/identity skeleton、active plan 和 finished plans，确认没有一份计划同时修复这些 emit 边界。#1778/#1745/#1605/#1461/#1419/#1415/#1455 是本计划全部来源；每个子项必须保留独立的 payload/受众回归断言，不能用一个“网络清理完成”替代。

## §5 立项检查记录

- `docs/worldview.md`：查经脉隐私、维度/区域边界、六境界与天劫叙事；不改正典或真元。
- `docs/finished_plans/`：查 `plan-tribulation-v1`、经脉事件、NPC bubble、spider disguise 和网络 schema。
- active plan：查 `BurstMeridianEventV1`、`VoidErosionVisualSyncPayloadV1`、`DamageTiltS2c`、`TribulationBroadcastV1` 等符号，未发现同一修复集合。
- skeleton：查七个来源 issue 号和 `scope/identity/entity_id/actor_name` 关键词，无重复骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无本组条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：server 事件、`Client`/`CurrentDimension`、`UniqueId`/generation、`char_id`/actor 字段。
- **Outputs**：按受众发送 `ServerDataPayloadV1::BurstMeridianEvent`、`bong:void_erosion_visual`、`NpcBubbleS2c`、vanilla `DamageTiltS2c`、spider payload、tribulation state/broadcast。
- **共享类型/事件**：复用 `BurstMeridianEventV1`、`VoidErosionVisualSyncPayloadV1`、`SpiderDisguiseS2c`、`TribulationStateV1`/`TribulationBroadcastV1` 和现有 `UniqueId`；不以 debug 字符串或裸 index 充当身份。
- **三端契约符号**：server `burst_event_emit`、`void_erosion_visual_emit`、`npc_bubble`、`combat_event_emit`、`spider_disguise_emit`、`tribulation_*_emit`；agent `BurstMeridianEventV1`、`VoidErosionVisualSyncPayloadV1`、`ServerDataTribulationStateV1`、`ServerDataTribulationBroadcastV1` 按现有消费保持兼容；client `BurstMeridianHandler`、`VoidErosionVisualHandler`/`VoidErosionVisualStore`、`NpcBubbleHandler`、`SpiderDisguiseHandler`、`TribulationStateHandler`/`TribulationBroadcastHandler`，原版 DamageTilt 无 agent 变更。
- **worldview 锚点**：经脉信息属于角色隐私，区域/维度边界来自 worldview 区域表，天劫广播遵循六境界叙事；不引入 `qi_physics` 入口。

## P0 验收

- 每个 payload 只到应有的 client/维度/实体，burst 不广播 integrity 私密字段给无关玩家。
- 所有实体引用使用稳定 `UniqueId` 或完整 generation；DamageTilt、spider、bubble 不再使用 0、裸 index、debug 字符串。
- settled/cleared tribulation 的 `actor_name`、`char_id`、坐标分别来自真实事件；缺身份时不伪造空 actor。
