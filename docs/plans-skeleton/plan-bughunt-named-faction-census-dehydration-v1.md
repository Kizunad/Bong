# plan-bughunt-named-faction-census-dehydration-v1

> Skeleton plan。只读审计产物，来源 issue #1752。

## §0 摘要

具名势力人口普查 `sync_named_faction_census_system` 只查询 live ECS 的 `NamedFactionMembership`。NPC 脱水为 `NpcDormantSnapshot` 后，快照只有旧的三态 `FactionMembership`，没有具名势力字段；当所有成员进入 dormant，live query 计数变成零，系统就把仍有存量成员的势力标为 `Decayed` 并清除玩家 membership。

## §1 游玩影响

玩家会看到一个仍有大量离屏 NPC 的具名势力被永久判定为消亡，无法再加入；领袖/势力关系、战事分组和区域叙事也会因人口归零而断裂。NPC 回到 live 后即使能恢复旧 faction，也无法撤销已经发出的 `NamedFactionDecayEvent`。

## §2 复现路径

1. 生成带 `NamedFactionMembership` 的 NPC，确认 registry 的 `current_npc_count > 0`。
2. 让这些 NPC 全部进入 dormant store，观察 `NpcDormantSnapshot` 只保存 `faction: Option<FactionMembership>`。
3. 下一次 `sync_named_faction_census_system` 的 `members: Query<&NamedFactionMembership, With<NpcMarker>>` 为空。
4. `previous_count > 0 && next_count == 0` 分支把 faction 状态改为 `Decayed`，即使 dormant 快照仍在。

## §3 今天 `origin/main` 证据

- `server/src/npc/faction.rs:921-931`：`NamedFactionMembership` 是挂在 hydrated NPC entity 上的独立具名身份组件。
- `server/src/npc/faction.rs:1072-1085`：census 只从 `Query<&NamedFactionMembership, With<NpcMarker>>` 累加 counts，没有读取 dormant store。
- `server/src/npc/faction.rs:1091-1119`：`next_count == 0` 时直接设置 `FactionStatus::Decayed`、清玩家 membership 并发送 `NamedFactionDecayEvent`。
- `server/src/npc/dormant/mod.rs:343-378`：`NpcDormantSnapshot` 有旧 `faction: Option<FactionMembership>` 和 `emergent_group`，没有 `NamedFactionId`/具名成员字段。
- `server/src/npc/dormant/mod.rs:1646-1653`：现有迁移器只能从 legacy `FactionMembership` 反查 leader realm，证明 dormant 的具名信息并未被直接保留。

## §4 非重复比对

- `plan-npc-realm-distribution-v1` 只处理 dormant 境界重抽样和 legacy faction 到 realm 的映射，不保证 `NamedFactionMembership` census。
- `plan-bughunt-named-faction-join-refusal-gate-v1.md`（本批另一骨架）处理玩家加入门禁，不处理 NPC 脱水人口统计。
- `plan-bughunt-war-lifecycle-retention-v1.md` 处理战事 store 回收；本 issue 的先决状态错误发生在 faction census。

## §5 立项检查记录

- **worldview**：查 `智能 NPC`、`势力`、`Decay`、`离屏`；命中 `docs/worldview.md §七.智能 NPC` 与 §十一 社会，离屏不等于势力成员消失。
- **finished_plans**：查 `NamedFactionMembership`、`NpcDormantSnapshot`、`census`、`Decayed`；找到 faction expansion/realm distribution，未找到脱水 census 合并。
- **active plan**：查 `sync_named_faction_census_system`、`dormant snapshots`、`NamedFactionId`；未见另一个 active plan 负责两类人口合并。
- **skeleton**：查 `faction census`、`dehydration`、`NamedFactionMembership`；无同一根因骨架。
- **reminder.md**：查 `faction`、`脱水`、`census`、`Decayed`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：live `NamedFactionMembership`、`NpcDormantStore.snapshots`、`NamedFactionRegistry`、`FactionStatus`。
- **Outputs**：`current_npc_count`、`NamedFactionLeaderDownEvent`、`NamedFactionDecayEvent`、玩家 `FactionMembership` 清理。
- **共享类型或事件**：复用 `NamedFactionId`、`NamedFactionMembership`、`NpcDormantSnapshot`、`NamedFactionDecayEvent`；字段迁移需 `serde(default)` 兼容旧快照。
- **server 符号**：`npc::faction::sync_named_faction_census_system`、`NamedFactionRegistry`、`npc::dormant::{NpcDormantSnapshot,NpcDormantStore}`、hydrate 路径。
- **agent**：无变更；当前具名势力 census/decay 事件没有 agent schema 或 Redis producer。
- **client**：无变更；玩家 membership 的现有 server 状态与聊天反馈不改变 wire 结构。
- **worldview 锚点**：`docs/worldview.md §七` 的 NPC 生态与 `§十一` 的社会势力持续性。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 让 census 同时统计 live component 与 dormant snapshot 的具名成员，定义旧快照迁移/缺失语义 |
| P1 | ⬜ | 脱水/回水/真实死亡边界和 decay 事件回归测试 |

## P0：人口来源统一

- 为 dormant 快照保存稳定 `NamedFactionId`（或从明确且可验证的 legacy 映射派生），census 只在实体和快照两侧都完成后再判定 zero。
- `NamedFactionDecayEvent` 必须只在 live+dormant 合计为零且没有待 hydrate 成员时发送；回水不能重复增加计数。

## P1：兼容和状态转换

- 旧无具名字段快照按既有 legacy 语义处理，不能凭空猜具名势力；补充迁移 marker/证据。
- 覆盖一半脱水、全部脱水、回水、自然死亡、战死和 leader 缺失的组合。

## 验收测试计划

- 全部成员进入 dormant 后，registry 仍保持非零并且不发 `NamedFactionDecayEvent`。
- dormant 成员真实终结后计数递减，最后一名终结才进入 Decayed。
- hydrate 后 `NamedFactionMembership` 与 snapshot 身份一致，重复 census 不重复发事件。

## 来源 issue

- #1752 `[flash-review][major] faction成员脱水被误判势力灭亡，永久Decayed`
