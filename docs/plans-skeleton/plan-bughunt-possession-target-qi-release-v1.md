# plan-bughunt-possession-target-qi-release-v1

> 来源 Issue：#1371。夺舍成功时目标被直接标记 `Despawned`，目标 `Cultivation.qi_current` 没有经过终结释放事务。

## §0 摘要

`process_duo_she_requests` 为宿主执行 `resize_qi_max_and_release_excess`，但对目标只写 `LifecycleState::Terminated`、生平记录，然后 `insert((PossessedVictim, Despawned))`。目标实体上的 `Cultivation.qi_current`（在线玩家或醒灵 NPC）没有调用死亡/释放路径；NPC 生命周期 notice 也不覆盖玩家目标。目标携带的真元随实体软删除而失去物理落点。

## §1 实际游玩体验影响

- 对醒灵目标反复夺舍会从世界总量中吞掉目标当前真元，宿主只继承年龄/身体位置而不是一笔可审计的 qi 流。
- 目标可能是玩家；标记 `Despawned` 后正常死亡 hook 不会替它补回 zone，造成跨玩家可感知的守恒漏洞。

## §2 复现路径

1. 准备一个符合 `duo_she_target_eligibility` 的醒灵玩家/NPC，令其 `Cultivation.qi_current > 0`，并准备有 zone、`WorldQiAccount` 和 `Events<QiTransfer>` 的正常 server 资源。
2. 通过公开 `DuoSheRequest` 触发 `process_duo_she_requests`。
3. 宿主侧完成 qi_max 缩容事务；目标侧只更新 `LifeRecord`/`Lifecycle`，随后在 `possession.rs:303-307` 插入 `Despawned`。
4. 目标 `qi_current` 没有 `ReleaseToZone`/overflow 转账，实体进入软删除队列，世界观察到的 qi 少了目标余额。

## §3 根因证据

- `server/src/cultivation/possession.rs:95-106` 的目标读查询带 `Option<&Cultivation>`，证明目标真元可读；`:281-307` 的写入只处理生平、生命周期和 `Despawned`，没有目标真元 release。
- `server/src/cultivation/possession.rs:216-240` 仅对宿主调用 `resize_qi_max_and_release_excess`；该 release 不能替代目标当前余额的终结结算。
- `server/src/npc/lifecycle.rs:1004-1015` 的 `Added<Despawned>` notice 是 NPC 终结路径，夺舍目标可能是玩家且 possession 已预先写 `LifecycleState::Terminated`，不能把它当通用兜底。

## §4 非重复比对

- `docs/plan-bughunt-duoshe-scope-gate-v1.md` 只处理夺舍请求的距离/维度门禁；本 issue 在成功结算后的目标 qi 去向，门禁 plan 不覆盖。
- `docs/finished_plans/plan-bughunt-qimax-shrink-clamp-leak-v1.md` 处理宿主/丹药缩容差额；这里是被夺舍目标实体即将消失时的完整余额释放。
- 垂死大能自身的夺舍释放已有独立 plan；本目标是通用 `cultivation::possession` 的 player/NPC target despawn。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“夺舍、转世、死亡、真元归零”，核对 §十二 L1058-L1060 与 §十二 L330-L332。
- `docs/finished_plans/`：检索 `process_duo_she_requests`、`PossessedVictim`、`Despawned`；命中夺舍范围和死亡生命周期文档，但没有目标 qi 释放闭环。
- active plan：检索 `DuoShe`、`Despawned`、`release_to_zone`；`docs/plan-container-filter-and-completion-v1.md` 与持久化计划未覆盖该终结事务。
- `docs/plans-skeleton/`：检索 `DuoShe`、`PossessedVictim`、`target qi`；除本文件外未发现同主题 skeleton。
- `docs/plans-skeleton/reminder.md`：已查阅并检索 `DuoShe`、`PossessedVictim`、`qi_flow_overflow`，未发现同主题延后事项。

## 接入面与跨仓契约

- **Inputs**：`DuoSheRequestEvent`、目标 `Cultivation.qi_current`/`LifeRecord`、目标 `Position`/`CurrentDimension`、`ZoneRegistry`、`WorldQiAccount`、`Events<QiTransfer>`。
- **Outputs**：目标终结记录，以及 `Cultivation::release_to_zone` 返回的 `QiFlowOutcome`/已提交审计转账；缺少 canonical 身份或 ledger 时 fail closed，目标不得标记 `Despawned`。今天主线的字段名是 `QiFlowOutcome::overflow_credited`（即 overflow 部分）；zone 缺失或容量不足时，release helper 已将该金额写入稳定 `qi_flow_overflow`，调用方只读取它做审计和断言，不得再次入账。
- **共享类型 / 事件**：复用 `ActorQiIdentity::from_life_record`、`Cultivation::release_to_zone`、`QiTransferReason::ReleaseToZone`、`DuoSheEventEmitted`；不改 `DuoSheEventV1`。
- **server 契约符号**：`process_duo_she_requests`、`DuoSheTargetReadItem`、`resolve_target_snapshot`、`Cultivation::release_to_zone`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更；agent 继续消费已有 `DuoSheEventV1`，真元落点是 server 内部审计。
- **client**：无变更；现有夺舍结果 payload 不增加字段。
- **worldview 锚点**：`docs/worldview.md §十二 L1058-L1060`（夺舍是有代价的续命路径）；`§十二 L330-L332`（死亡/重生真元归零是结算的一部分）。
- **qi_physics**：目标真元权威是 `Cultivation.qi_current`，不是长期 player ledger balance。玩家可用 `death_hooks::release_qi_amount_to_zone`（`origin/main` `death_hooks.rs:440-505`），它要求有效 `LifeRecord`，缺身份返回 `InvalidActorIdentity`；NPC 不能复用这个固定 `ActorQiKind::Player` 的 facade，必须调用现有 `Cultivation::release_to_zone`，用 `ActorQiIdentity::from_life_record(..., ActorQiKind::Npc)` 或 `ActorQiIdentity::from_canonical_npc_id` 构造 NPC 来源。若调用上下文无法取得这些入口，骨架需新增一个接受 `ActorQiKind`/NPC identity 的 facade，而不是伪造 player 账户。今天主线 `QiFlowOutcome` 的 overflow 字段实际名为 `overflow_credited`（`qi_flow.rs:39-46`）；zone 查找失败或容量不足时，`Cultivation::release_to_zone` 已在 `qi_flow.rs:683-707,718-735` 通过 `transfer_external_qi_to_ledger` 完成 `qi_flow_overflow` 审计转账并返回该字段，调用方只能读取它做审计和断言，不得再次交给 `qi_flow_overflow` 入账。缺身份或 ledger/context 时必须 fail closed，不能强行标记 `Despawned`。测试守恒用 `SPIRIT_QI_TOTAL` 与 `assert_conservation`。

## §5 修复骨架

### P0 目标终结释放

- 在插入 `Despawned` 前，对带 `Cultivation` 的目标执行一次失败原子的全额 `release_to_zone`；该 helper 已负责 zone 接收和 overflow 的账本转账。调用方只读取 `QiFlowOutcome::overflow_credited` 做审计和守恒断言，不得再次把这笔金额交给 `qi_flow_overflow`，避免重复入账和凭空造元。缺 canonical 身份或 ledger 时 fail closed，并保持目标未 `Despawned`；无法证明 release 已入账则拒绝标记 `Despawned`。
- NPC 与玩家目标共用同一失败原子 release 事务；玩家 facade 固定为 `ActorQiKind::Player`，NPC 走现有 `Cultivation::release_to_zone` + `ActorQiIdentity::from_life_record(..., ActorQiKind::Npc)`/`from_canonical_npc_id`，避免依赖某个单独的 NPC notice 或伪造 player 来源。

### P1 回归

- 醒灵玩家、醒灵 NPC、无 zone overflow、账本失败四个行为类；断言目标 current、zone/overflow 账户、Despawned 标记和事件一致。
- 宿主 qi_max 缩容与目标释放同时发生时，分别只产生各自 `ReleaseToZone` 转账，不双计。
