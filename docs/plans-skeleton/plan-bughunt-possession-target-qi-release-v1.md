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

## 接入面与跨仓契约

- **Inputs**：`DuoSheRequestEvent`、目标 `Cultivation.qi_current`/`LifeRecord`、目标 `Position`/`CurrentDimension`、`ZoneRegistry`、`WorldQiAccount`、`Events<QiTransfer>`。
- **Outputs**：目标终结记录、`Despawned` 标记，以及目标余额全部进入 zone/稳定 overflow 的 `QiFlowOutcome`/审计转账。
- **共享类型 / 事件**：复用 `ActorQiIdentity::from_life_record`、`Cultivation::release_to_zone`、`QiTransferReason::ReleaseToZone`、`DuoSheEventEmitted`；不改 `DuoSheEventV1`。
- **server 契约符号**：`process_duo_she_requests`、`DuoSheTargetReadItem`、`resolve_target_snapshot`、`Cultivation::release_to_zone`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更；agent 继续消费已有 `DuoSheEventV1`，真元落点是 server 内部审计。
- **client**：无变更；现有夺舍结果 payload 不增加字段。
- **worldview 锚点**：`docs/worldview.md §十二 L1058-L1060`（夺舍是有代价的续命路径）；`§十二 L330-L332`（死亡/重生真元归零是结算的一部分）。
- **qi_physics**：目标真元权威是 `Cultivation.qi_current`，不是长期 player ledger balance。不能直接对不存在的 player 余额 `ledger.transfer`；须以 `ActorQiIdentity` 调 `Cultivation::release_to_zone(zone, &mut WorldQiAccount, actor, amount, ReleaseToZone)`，底层使用 `transfer_external_qi_to_ledger`，zone 字段需要真实更新，满 zone 走持久化 overflow。测试守恒用 `SPIRIT_QI_TOTAL` 与 `assert_conservation`。

## §5 修复骨架

### P0 目标终结释放

- 在插入 `Despawned` 前，对带 `Cultivation` 的目标执行一次失败原子的全额 `release_to_zone`；缺身份/账本/zone 时走稳定 overflow，无法证明入账则拒绝标记 `Despawned`。
- NPC 与玩家目标共用同一 release helper，避免依赖某个单独的 NPC notice。

### P1 回归

- 醒灵玩家、醒灵 NPC、无 zone overflow、账本失败四个行为类；断言目标 current、zone/overflow 账户、Despawned 标记和事件一致。
- 宿主 qi_max 缩容与目标释放同时发生时，分别只产生各自 `ReleaseToZone` 转账，不双计。
