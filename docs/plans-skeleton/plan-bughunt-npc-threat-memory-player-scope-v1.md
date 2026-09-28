# plan-bughunt-npc-threat-memory-player-scope-v1

> Skeleton plan。只读审计产物，来源 issue #1774。

## §0 摘要

`compute_threat_assessments` 已从 `NpcBlackboard.nearest_player` 选定当前目标，却在内存偏置阶段对 `NpcMemoryComponent.interactions` 全量做 `any`。因此 NPC 曾被玩家 A 攻击后，面对最近的玩家 B 也会获得攻击/交易/盗窃偏置。`interaction_memory.rs` 已有按 player UUID 过滤的 `has_been_attacked_by`、`has_traded_with`、`has_been_robbed_by` helper，但当前系统没有目标 UUID 可用。

## §1 游玩影响

多人靠近同一 NPC 时，第三方会继承别人的仇恨、交易优惠或抢劫记忆。NPC 可能对无辜玩家直接 Flee/攻击，也可能把对 A 的交易记忆错误地用于 B，破坏“NPC 记住具体修士”的社交可预测性。

## §2 复现路径

1. 给 NPC 的 `NpcMemoryComponent` 写入 `NpcMemoryEntry { player_uuid: A, interaction_type: Attack }`。
2. 让 `NpcBlackboard.nearest_player` 指向玩家 B，且 B 的 `Lifecycle.character_id` 与 A 不同。
3. 运行 `compute_threat_assessments`。
4. 由于 `:331-343` 不比较 UUID，`was_attacked=true`，B 获得攻击偏置；把条目换成 Trade/Theft 可分别复现另外两种泄漏。

## §3 今天 `origin/main` 证据

- `server/src/npc/brain/threat.rs:277-304`：系统读取最近玩家 entity，并用其 cultivation/wounds 构建 assessment。
- `server/src/npc/brain/threat.rs:320-343`：memory lookup 逐项检查 `interaction_type`，没有读取目标玩家 UUID。
- `server/src/npc/interaction_memory.rs:23-29`：每条记忆明确保存 `player_uuid`。
- `server/src/npc/interaction_memory.rs:88-110`：已有 `has_been_attacked_by`、`has_traded_with`、`has_been_robbed_by`，均按 UUID 过滤；当前系统未调用它们。
- `server/src/npc/interaction_memory.rs:184-205`：记录攻击时从玩家 `Lifecycle.character_id` 生成稳定 UUID/char id，说明可用的权威身份来源已存在。

## §4 非重复比对

- `plan-npc-threat-memory-v1`/现有 interaction memory 模块提供记忆存储与淘汰规则，但没有把 threat system 的最近目标绑定到 UUID。
- `plan-npc-trade-gate-desync-v1` 处理交易门禁/信誉，不处理战斗 threat bias 的目标选择。
- 本问题不是“最近玩家选择错误”：黑板已经选定 B，错误发生在之后的全量 memory lookup。

## §5 立项检查记录

- **worldview**：查 `拾荒散修`、`评估威胁度`、`记住`、`身份与信誉`；命中 `docs/worldview.md §七.智能 NPC` 与 §十一，记忆应绑定具体修士。
- **finished_plans**：查 `NpcMemoryComponent`、`has_been_attacked_by`、`compute_threat_assessments`、`nearest_player`；已有 memory 基础设施，但无调用绑定。
- **active plan**：查 `threat.rs`、`player_uuid`、`Lifecycle.character_id`；未见同一 scope 的 active plan。
- **skeleton**：查 `threat memory`、`cross player`、`nearest player`、`UUID`；无同一根因骨架。
- **reminder.md**：查 `threat`、`memory`、`player_uuid`、`NPC`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`NpcBlackboard.nearest_player`、目标玩家 `Lifecycle.character_id`、`Cultivation`/`Wounds`、NPC `NpcMemoryComponent`。
- **Outputs**：`ThreatAssessment`、`SelfInterestDecision` 写回 blackboard，供 NPC brain scorer/action 使用。
- **共享类型或事件**：复用 `NpcMemoryComponent`、`NpcMemoryEntry`、`NpcInteractionType`、`Lifecycle`；不新增 schema。
- **server 符号**：`npc::brain::threat::compute_threat_assessments`、`NpcThreatQueryItem`、`npc::interaction_memory::NpcMemoryComponent::{has_been_attacked_by,has_traded_with,has_been_robbed_by}`、`record_player_npc_interaction`。
- **agent**：无变更；threat assessment 是 server 内部 AI blackboard 值，不通过 Redis 发给 agent。
- **client**：无变更；NPC 行为/目标筛选不改变 CustomPayload 结构，客户端只看到最终实体行为。
- **worldview 锚点**：`docs/worldview.md §七` 的散修威胁评估和 §十一 的身份/信誉关联。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 为 threat system 提供目标稳定身份并调用按 UUID 记忆 helper |
| P1 | ⬜ | 多玩家切换、缺身份 fail-closed 和记忆类型回归测试 |

## P0：目标身份绑定

- 在目标玩家 query 中读取 `Lifecycle`（或同等稳定 canonical id），只把该 id 传给 helper；禁止回退到“任意 interaction”语义。
- 若目标缺稳定身份，攻击/盗窃等危险偏置默认 false；不使用 Entity bits 伪造跨 tick 记忆 key。

## P1：行为边界

- 覆盖 A/B 交替成为最近玩家、同一玩家不同 interaction 类型、无 memory、缺 Lifecycle。
- 保持 `decide_self_interest_with_memory` 的纯函数契约不变，测试系统只负责正确传入三项 bool。

## 验收测试计划

- A 的 Attack 只提高面对 A 的 threat，面对 B 不提高。
- B 的 Trade/Theft 只影响对应类型；同一目标的组合仍按既有优先级（Theft→Flee）。
- 目标缺 canonical identity 时不误用其他玩家记忆，且不会 panic。

## 来源 issue

- #1774 `[flash-review][major] NPC 威胁记忆偏置跨玩家泄漏（非最近玩家也生效）`
