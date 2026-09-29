# plan-bughunt-npc-tribulation-interception-tick-gate-v1

> 严重级别：Major。来源 #1431。天道 NPC producer 对同一渡虚劫信号逐 tick 生成新的截胡 mission，server 队列没有重复任务门。

## §0 摘要

`agent/packages/tiandao/src/npc-producer.ts:125-156` 的 `produceTribulationInterceptionIntents` 只检查 karma、渡劫信号和可用弟子，没有 tick 间隔或事件生命周期门控；`mission_id` 还把当前 `state.tick` 拼进去，所以相邻 tick 会得到不同 ID。server `server/src/npc/faction.rs:286-300` 的 `FactionEventKind::EnqueueMission` 直接把 `MissionId` push 到 `pending`，没有按 mission identity 去重。一个持续存在的 `active_events` 或 `recent_events` 信号因此会持续增长派系任务队列。

修复必须同时覆盖 producer 和 consumer：producer 要按稳定的渡劫事件/目标身份设置明确、可核验的 tick gate，重试同一事件不得靠当前 tick 生成新身份；server enqueue 入口还要按稳定 mission identity 幂等，防止重复 command 或多个 agent 实例绕过 producer gate。若今天的 world-state event 没有可复用的稳定事件 ID，先在 P0 明确可审计的 `(tribulation event, subject_id, faction_id)` identity 语义，再决定最小共享字段扩展，不能把 tick 当事件 ID。

## §1 游玩影响

同一场渡虚劫会让敌对派系任务队列快速膨胀，重复派出的弟子可能重复赶路、重复攻击或占满任务容量；世界状态里的 `pending_count/top_mission_id` 也会脱离实际事件数量。正常间隔的新渡劫事件仍应能产生一条新任务，不能用全局禁用截胡来掩盖重复入队。

## §2 复现路径

1. 准备一个 karma 达到 `TRIBULATION_INTERCEPT_MIN_KARMA` 的玩家，在其 zone 的 `active_events` 放入渡虚劫信号，并准备 loyalty 达标的 attack/defend disciple。
2. 以 tick `T` 调用 `produceDeterministicNpcDecisions`，得到 `faction_event`/`enqueue_mission`，其 ID 为 `mission:intercept_duxu:T:<subject>`。
3. 将同一持续信号的 world state tick 改为 `T+1` 再调用 producer；当前实现再次产出命令，ID 变成 `mission:intercept_duxu:T+1:<subject>`。
4. 依次送入 server `FactionEventKind::EnqueueMission`；`pending.push` 两次，`pending_count` 增长，而不是同一渡劫事件保持一个任务。
5. 对照验收：同一事件在 gate 窗口内不增长队列；稳定间隔后的新事件（或事件 identity 变化）仍能入队一次。

## §3 今天 `origin/main` 证据

- `agent/packages/tiandao/src/npc-producer.ts:125-156` 的 producer 没有像 rogue spawn/faction era 路径那样使用 tick gap；`missionId` 在 `:144` 直接包含 `state.tick`。
- `hasTribulationSignal`（`:185-211`）只要 zone active event 或 recent event 命中即返回 true，没有提供“已为该事件派过任务”的状态；`selectInterceptionFaction`（`:213-223`）选择 loyalty 达标的 attack/defend disciple。
- `agent/packages/schema/src/agent-command.ts:19-25,122-130` 允许 `faction_event` 的 `enqueue_mission`、`mission_id` 与 `subject_id`，但当前语义校验没有稳定 identity 或重复约束；`agent/packages/schema/src/world-state.ts:36-43` 只向 agent 暴露 `pending_count/top_mission_id` 摘要。
- `server/src/npc/faction.rs:286-300` 的 `EnqueueMission` 无条件 `pending.push(MissionId(mission_id))`；没有基于 mission ID 或 subject/event identity 的幂等检查。
- 现有 `agent/packages/tiandao/tests/npc-producer.test.ts:139-185` 只有 tick=3000 的单次正例，没有相邻 tick、同一 event 不重复和正常间隔新事件的回归。

## §4 非重复比对

- `docs/finished_plans/plan-tribulation-v1.md` 与 `plan-npc-ai-v1.md` 已把渡劫广播、观战/截胡和 NPC 截胡作为玩法闭环，但未实现 agent mission 的重复门控；本骨架只修任务触发频率与入队幂等，不改变截胡资格、战斗或掉落规则。
- `docs/plans-skeleton/plan-bughunt-duxu-juebi-quota-marker-lifecycle-v1.md` 处理的是 JueBi marker 的生命周期清理；它不涉及 `npc-producer` 的 `enqueue_mission`，也不提供 faction queue 去重。
- `plan-agent-narration-pipeline-v1` 的 dedupe 针对 narration publish，不覆盖 NPC faction command；本骨架要求在 command producer 和 server consumer 两端各自明确边界。

## §5 立项检查记录

本记录按任务卡检查了世界观、归档计划、active plan、骨架和 reminder 入口，并以今天的 `origin/main` 代码为准。

- **worldview**：检索“智能 NPC、派系、渡虚劫、截胡、散修”；命中 `docs/worldview.md §七 L739-L748`（智能 NPC/散修行为）及 `§十一 L999-L1005`（渡劫广播和截胡窗口）。
- **finished_plans**：检索 `produceTribulationInterceptionIntents`、`enqueue_mission`、`mission_queue`、`pending_count`；确认既有渡劫/NPC 计划描述玩法，但没有同一事件任务幂等交付物。
- **active plan**：检索 `npc-producer`、`faction queue`、`tribulation interception`、`tick gate`；没有锁定本 producer/consumer 重复入队问题的 active plan。
- **skeleton**：检索 `interception mission`、`enqueue_mission`、`mission_id`、`tick gap`；没有同主题骨架，只有渡劫 marker、物品转移等相邻问题。
- **reminder.md**：检索 `npc`、`mission`、`tribulation`、`dedupe`；仓内无 `docs/plans-skeleton/reminder.md`。

## §6 接入面与跨仓契约

- **Inputs**：`WorldStateV1.tick`、玩家 `karma/uuid/name/zone`、zone `active_events/recent_events`、NPC disciple 的 faction/loyalty、派系 mission queue 摘要、`TickPublishMetadata`。
- **Outputs**：agent 发出的 `AgentCommandV1.commands[].type = faction_event`，参数含 `kind=enqueue_mission`、`faction_id`、`subject_id` 和稳定 `mission_id`；server 只接受一次同一 mission identity，并在 `WorldStateV1` 中反映真实 `pending_count/top_mission_id`。
- **共享类型或事件**：`AgentCommandV1`、`Command`、`FactionEventKind::EnqueueMission`、`MissionId`、`MissionQueueSummaryV1`、`WorldStateV1`；若稳定渡劫事件 ID 当前不存在，P0 必须先冻结最小 schema/事件字段，再让两端按同一 identity 去重。
- **server 符号**：`npc::faction::{FactionEventKind::EnqueueMission, MissionId}`、`FactionState::mission_queue.pending`、接收 `faction_event` 的 command executor；enqueue 入口负责 fail-safe 幂等。
- **agent 符号**：`tiandao::npc_producer::{produceDeterministicNpcDecisions,produceTribulationInterceptionIntents,hasTribulationSignal,selectInterceptionFaction}`、`schema::{AgentCommandV1,validateAgentCommandV1Contract,WorldStateV1}`；producer 负责稳定 gate，schema 负责字段形状而非用开放字符串掩盖 identity 缺口。
- **client**：无变更；客户端不生产或消费派系 mission command，本问题的重复发生在 agent→server 队列，client 只会被动看到既有 world-state/UI 摘要。
- **worldview 锚点**：`docs/worldview.md §七 L739-L748`、`§十一 L999-L1005`；本计划保持“渡劫可被派系截胡”的玩法，只阻止同一事件被重复派遣。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 核实并冻结可复用的渡劫事件 identity；在 producer 增加明确 tick/事件生命周期 gate，mission ID 不再以当前 tick 伪造新事件 |
| P1 | ⬜ | server `EnqueueMission` 按稳定 identity 幂等；补 agent 相邻 tick、同事件重复、正常间隔新事件和 server queue 不增长回归测试 |

## 来源 issue

- #1431 `[flash-review][major] 渡虚劫截胡任务缺 tick 间隔门控，逐 tick 重复入队`
