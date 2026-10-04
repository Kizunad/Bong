# plan-bughunt-npc-social-memory-prune-v1（骨架）

> **来源 issue**：#1750。`ScatteredCultivatorSocialMemory` 只在离开观察范围时清理，NPC despawn 和 player disconnect 不会删除实体 key。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 社交记忆按实体存活/连接生命周期清理，保留范围内的正常记忆 | ⬜ |

## §0 摘要

`scattered_cultivator` 的两个 HashMap 在离开范围时删除，但没有 live entity/disconnect prune。长跑服务器会保留不存在的 NPC/player identity，造成内存增长和旧关系复活。

## §1 游玩影响

重连或实体复用后 NPC 可能读到旧关系、声望或敌意；长时间运行还会积累不可达 key。计划只修内存/生命周期，不改变社交规则。

## §2 复现路径

1. 创建 NPC 与玩家的 social memory，确认两张表各有条目。
2. despawn NPC 或断开玩家，不触发“离开范围”路径。
3. 观察 `ScatteredCultivatorSocialMemory` 仍保留对应 key，反复循环可见无界增长。

## §3 今天 `origin/main` 的证据

- `server/src/npc/scattered_cultivator.rs:182-225` 两个 HashMap 仅在 `:207-210` 按距离删除，没有对 live entity 或 disconnect 的清理。

## §4 非重复比对

已查 `plan-social-v1`、NPC AI active plan、skeleton 的 `ScatteredCultivatorSocialMemory`、disconnect/prune；没有现有计划处理这个缓存生命周期。#1750 单独保留。

## §5 立项检查记录

- `docs/worldview.md`：查散修社交、身份和区域关系关键词；不改变匿名/声望正典。
- `docs/finished_plans/`：查 social/NPC memory 的现有组件与生命周期约定。
- active plan：查 `ScatteredCultivatorSocialMemory`、disconnect、despawn，未发现同一 map 改动。
- skeleton：查 #1750、两个 HashMap、memory prune，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无社交记忆条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：live entity query、连接/断开事件、现有 social memory map。
- **Outputs**：删除不可达 identity key；在线且在范围内的记忆仍按原规则更新。
- **共享类型/事件**：复用 `ScatteredCultivatorSocialMemory`、实体生命周期事件和 `UniqueId`；不新造 player identity。
- **三端契约符号**：server `scattered_cultivator`；agent **无变更**、client **无变更**（memory 是 server AI 内部状态）。
- **worldview 锚点**：散修社会关系/身份边界；不涉及 qi_physics。

## P0 验收

- NPC despawn、player disconnect、Entity generation 复用后旧 key 均被清理。
- 离开范围仍只清理应清理的条目，不误删仍在线且可见的关系。
