# plan-bughunt-network-ephemeral-cache-prune-v1（骨架）

> **来源 issue**：#1559、#1347、#1349、#1411、#1387。多处 server emit/bridge 的 Local HashMap 按实体缓存没有断线、despawn 或 generation prune。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 所有列出的 ephemeral cache 只保留 live session/entity，disconnect 后清除 | ⬜ |

## §0 摘要

AudioTriggerState、CarrierStateEmitCache、yidao previous、Tiandao `last_eval_position_by_entity` 和相关音频缓存会在实体离场后继续持有 key。它们还可能把旧状态发给新 generation，属于同一类生命周期缺口，可统一建立 prune 入口/验收。

## §1 游玩影响

长时间运行会出现内存无界增长；重连玩家/NPC 可能继承低血、低真元、carrier 或 HUD previous，导致音效与 AI 反馈错乱。修复不改变音效配方或 Tiandao 决策逻辑。

## §2 复现路径

1. 连接玩家、触发 low_hp/low_qi/audio/carrier/yidao/Tiandao 事件，使各 cache 写入。
2. 断线或 despawn，不执行正常范围离开路径。
3. 重复连接并观察旧 key 留存、map 长大或新实体命中旧 previous。

## §3 今天 `origin/main` 的证据

- `server/src/network/audio_trigger.rs:128-213` 的 `low_hp/low_qi` cache 写入无 disconnect prune。
- `server/src/network/carrier_state_emit.rs:18-67` `CarrierStateEmitCache` 在 `:49` insert 无 prune。
- `server/src/network/yidao_state_emit.rs:27-53` `previous` 无 live entity prune；`server/src/world/tiandao_hunt.rs:154-166,500-539` 的 `last_eval_position_by_entity` 同样无断线清理。
- #1559、#1347、#1349、#1411、#1387 的相邻缓存均缺同一生命周期门。

## §4 非重复比对

已查 `plan-audio-v1`、Tiandao、carrier、yidao active plans 与 skeleton，确认没有一份已有计划统一处理这些 Local/Resource cache。五个来源 issue 的共同根因是 live session 边界缺失，业务阈值仍由各原模块负责。

## §5 立项检查记录

- `docs/worldview.md`：查声音反馈、天道观察和 NPC/玩家身份；不改正典。
- `docs/finished_plans/`：查 audio、yidao、Tiandao、carrier 生命周期约定。
- active plan：查 `AudioTriggerState`、`CarrierStateEmitCache`、`last_eval_position_by_entity`、`previous`，未见同一 prune 方案。
- skeleton：查五个 issue 号、`disconnect`、`despawn`、`clearOnDisconnect`，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 ephemeral cache 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：连接/断开事件、live `Entity`/`UniqueId` 查询、各模块当前 cache。
- **Outputs**：清理旧条目；仍由原 emit 产生 `ServerDataPayloadV1::AudioTrigger`、`CarrierState`、`YidaoHudState` 或 Tiandao 内部更新。
- **共享类型/事件**：复用 `SessionScopedStore` 对应的 server 生命周期、现有 cache 类型；不得用新的全局 map 代替。
- **三端契约符号**：server `audio_trigger`、`carrier_state_emit`、`yidao_state_emit`、`tiandao_hunt`；agent/client **无 schema 变更**，现有消费者只会收到当前会话状态。
- **worldview 锚点**：角色身份和天道观察应随会话边界失效；不涉及 qi_physics。

## P0 验收

- 每个列出的 cache 在断线、despawn、generation 变化后删除旧条目。
- 同一会话内的正常去重仍工作，不因每 tick prune 而重复发送。
- 新实体不会读取旧身份的 previous/audio/carrier 状态。
