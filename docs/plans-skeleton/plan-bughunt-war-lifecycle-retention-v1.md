# plan-bughunt-war-lifecycle-retention-v1

> Skeleton plan。只读审计产物，来源 issue #1710、#1858；两条表现归并为同一战事终态清理根因。

## §0 摘要

`WarConflictStore` 的战事状态机能把战事推进到 `Aftermath`，但生命周期收口不一致：既有 active war 在 `escalate_or_create` 的连跳分支进入 `Aftermath` 时只 `break`，没有清理 `zone_active`；新建战事和 `advance_settling_wars` 另有清理逻辑。更根本的是 `self.wars` 只有 `insert`，没有终态 `remove`/`retain`/持久化回收，因此每场历史战事永久留在内存中。

## §1 游玩影响

同一 zone 的旧战事可能继续占用 `zone_active`，后续压力无法创建新战事或玩家无法重新参战；即使某些路径清掉了索引，`wars` 仍无界增长，长服运行会增加查询、复制和 telemetry 成本。玩家看到的是“战事已经结束但区域仍锁住”或旧战事记录越来越多。

## §2 复现路径

1. 在 `WarConflictStore` 中创建一场已有 `zone_active` 的 `Settling`/`Skirmish` 战事，使一次 `escalate_or_create` 连跳到 `Aftermath`。
2. 观察已有战事分支 `new_phase == Aftermath` 直接 `break`，`zone_active` 仍指向该 `WarId`。
3. 重复创建并推进多场战事，观察 `wars.len()` 只增不减；任何路径都不调用 `remove` 或 `retain`。
4. 后续同 zone `escalate_or_create` 继续命中旧 `WarId`，或在其他 zone 继续积累终态历史。

## §3 今天 `origin/main` 证据

- `server/src/npc/war/mod.rs:373-382`：`WarConflictStore` 同时持有 `wars: HashMap` 与 `zone_active`，注释称 Aftermath 后应清 active。
- `server/src/npc/war/mod.rs:405-455`：既有 active war 的推进分支在 `:447-449` 遇到 `Aftermath` 只 `break`，没有 `zone_active.remove`。
- `server/src/npc/war/mod.rs:457-480`：新建战事写入 `self.wars` 和 `zone_active`。
- `server/src/npc/war/mod.rs:497-503`：只有新建战事连跳到 Aftermath 的分支会清 `zone_active`。
- `server/src/npc/war/mod.rs:513-535`：`advance_settling_wars` 清 `zone_active`，但没有删除对应 `self.wars` 条目。
- 全文件 `WarConflictStore` 实现只有 `wars.insert`，未找到终态 `remove`/`retain`/容量策略。

## §4 非重复比对

- `plan-offscreen-war-v1` 的 P9/settle 只消费 `WarPhaseChanged` 更新区域加成和奖励，不拥有 `WarConflictStore` 的历史回收。
- `plan-defense-hardening-v1` 与 faction 参与门禁不负责战事终态索引。
- `server/src/npc/war/tests.rs` 已有 `zone_active` 清理测试覆盖新建/Settling 路径，但没有既有 active 连跳与 `wars` 长期保留断言；因此本骨架补的是未覆盖的生命周期组合。

## §5 立项检查记录

- **worldview**：查 `战事`、`势力`、`余波`、`区域`；命中 `docs/worldview.md §七/§十一` 的 NPC 社会与区域冲突语义，余波终态不应永久锁区。
- **finished_plans**：查 `WarConflictStore`、`WarPhaseChanged`、`Aftermath`、`ZoneSpiritBonusStore`；找到离屏战事结算，但没有 store 历史回收。
- **active plan**：查 `wars.insert`、`zone_active.remove`、`WarConflictStore`；未见另一个 active plan 负责删除/保留策略。
- **skeleton**：查 `war lifecycle`、`Aftermath`、`retain`、`zone_active`；无同一根因骨架。
- **reminder.md**：查 `war`、`Aftermath`、`zone_active`、`ConflictStore`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`ZoneConflictPressure`、`WarParticipateIntent`、`WarConflictStore::escalate_or_create`/`advance_settling_wars`、当前 tick。
- **Outputs**：`WarPhaseChanged`、`ZoneSpiritBonusStore` 更新、`SocialRenownDeltaEvent` 奖励，以及可再次创建战事的 `zone_active` 状态。
- **共享类型或事件**：复用 `WarPhase`、`FactionWar`、`FactionWarOutcome`、`WarPhaseChanged`、`WarConflictStore`；历史回收不得另造平行战事表。
- **server 符号**：`npc::war::{WarConflictStore,escalate_or_create,advance_settling_wars,WarPhaseChanged}`、`npc::war::settle::{apply_war_zone_spirit_bonus,award_war_winner_renown}`。
- **agent**：无变更；当前仓内没有 agent schema/Redis consumer 直接持有 `WarConflictStore`，生命周期修复只影响 server 内部事件。
- **client**：无变更；没有 war 专用 CustomPayload，玩家反馈通过既有聊天/telemetry 路径呈现。
- **worldview 锚点**：`docs/worldview.md §七` NPC 生态联动与 §十一 社会/势力关系。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 统一 Aftermath 终态转移、清理 `zone_active`，并定义历史 `wars` 保留/回收策略 |
| P1 | ⬜ | 终态回收、重复事件幂等和长期运行边界测试 |

## P0：收口单一生命周期

- 抽出一个终态收口入口，所有进入 `Aftermath` 的路径都调用；既有 active、新建连跳、settling tick 不得各自维护一套清理。
- 为 `wars` 定义可审计的 retention（例如保留最近终态快照或转移到明确的 history store），禁止无条件永久增长；`WarPhaseChanged` 的消费者仍先收到完整快照。

## P1：回归与容量

- 覆盖既有 active 连跳、新建连跳、settling tick、重复 Aftermath、同 zone 新战事和多 zone 长期推进。
- 断言索引与历史条目最终一致、phase event 只发一次，且回收不会删掉仍可 join 的 Emerging/Skirmish 战事。

## 验收测试计划

- 两个 issue 的最小回归：已有战事进入 Aftermath 后 `zone_active` 为空；推进大量战事后 `wars` 不再无界增长。
- `WarPhaseChanged`、区域 bonus 清理和奖励仍按一次性终态顺序执行。
- 新战事可在旧战事终态收口后于同一 zone 创建，旧快照仍按约定可审计。

## 来源 issue

- #1710 `[flash-review][major] 既有战事升 Aftermath 不清 zone_active，zone 永久锁死无法再开战`
- #1858 `[flash-review][major] 涌现战事永不清除，WarConflictStore.wars 无界增长`
