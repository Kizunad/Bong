# plan-bughunt-forge-minor-cleanups-v1

> **来源 issue**：#1446、#1443。
> 一句话主题：锻造淬炼费用没有从玩家真元权威提交，法器时间戳使用 epoch。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 淬炼的 Cultivation 与 ledger 原子扣费、法器创建 tick 接线 | ⬜ |
| P1 | forge session、artifact maintenance 与守恒回归 | ⬜ |

## §0 摘要

`apply_tempering_hit` 在 `server/src/forge/steps.rs:118-138` 只把 `profile.qi_per_hit` 加到 `TemperingState.qi_spent`；生产 `handle_tempering_hits`（`forge/mod.rs:500-551`）拿到的是不可变 `Cultivation`，没有扣费或 ledger 转移。锻造结算在 `forge/inventory_bridge.rs:105-115` 把 `artifact_state_for_outcome` 的 `created_at_tick` 固定传 `0`，而当前 tick 直到 `:128` 才取得。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` 的锻造、法器养护和真元守恒关键词；不改变 `qi_per_hit`、养护周期或法器品阶规则，只补事务边界。
- **finished_plans**：查 forge outcome、qi ledger 和 inventory persistence 的已归档计划；确认它们没有同时覆盖淬炼命中费用与结算创建 tick。
- **active plan**：查 `docs/plan-*.md` 的 `TemperingHit`、`ArtifactState`、`WorldQiAccount`；未见统一的两处修复入口。
- **skeleton**：查 `plan-bughunt-forge-outcome-full-inventory-loss-v1.md`、forge inscription/repair 骨架和 `plan-bughunt-qi-physics-minor-cleanups-v1.md`；它们不拥有本次 forge 费用与时间戳。
- **reminder.md**：查 `docs/plans-skeleton/reminder.md` 的 `qi_per_hit`、`created_at_tick`；仓内有该文件，但没有这两条的合并登记。

## §2 接入面与跨仓契约

- **Inputs**：C2S `TemperingHit`/`ForgeSession`、`TemperingProfile.qi_per_hit`、玩家权威 `Cultivation.qi_current`、`WorldQiAccount`、`CombatClock.tick`、`ForgeOutcomeEvent` 和 `ArtifactState`。
- **Outputs**：每次接受的淬炼命中真实扣费并转入对应 zone/overflow；锻造法器的 `created_at_tick` 等于结算时的 `CombatClock.tick`。
- **共享类型/事件**：复用 `TemperingHit`、`TemperingState`、`ForgeOutcomeEvent`、`ArtifactState`、`CombatClock`、`QiTransfer`、`QiTransferReason::MeridianForge`。玩家真元权威是 `Cultivation.qi_current`；跨到 ledger 时先用真实签名 `qi_physics::ledger::transfer_external_qi_to_ledger(account, from, to, amount, reason)`，其内部走 `ledger.transfer(QiTransfer { from, to, amount, reason })`，成功后才提交外部字段，不能用 `set_balance` 加审计事件冒充原子转账。
- **三端契约符号**：server `forge::{handle_tempering_hits,forge_outcome_to_inventory}`；client **无变更**，依据是已有 `TemperingHit`/forge snapshot payload 只承载命中和结果，修复应在 server 权威扣费与 item state 写回；agent **无变更**，forge 不经过 Redis IPC。
- **worldview/qi**：守恒回归使用 `qi_physics::ledger::assert_conservation`，`era_decay` 保持 `0.0`，预算快照引用 `schema::common::SPIRIT_QI_TOTAL`；不得写 `DEFAULT_SPIRIT_QI_TOTAL` 或字面量代替测试总量。

## §3 游玩影响与复现

1. 玩家进入有淬炼步骤的 forge session，连续发送合法 `TemperingHit`，`TemperingState.qi_spent` 增长但 `Cultivation.qi_current` 不变，玩家免费淬炼。
2. 服务器运行超过三天后新锻造法器的 `created_at_tick=0`，每日养护比较 `now_tick - last_maintenance_tick` 时被当成长期未养护物品。

## §4 `origin/main` 根因证据

- `server/src/forge/steps.rs:118-138` 只累加 `qi_spent`；`server/src/forge/mod.rs:500-531` 的 `casters` query 是 `&Cultivation`，调用链没有 `WorldQiAccount` 或 `QiTransfer`。
- `server/src/forge/inventory_bridge.rs:105-115` 传入 `0`，`server/src/forge/inventory_bridge.rs:128-153` 才读取 `CombatClock.tick` 作为物品时间戳。

## §5 非重复比对

forge outcome inventory-loss 骨架处理产物发放失败；inscription/repair 骨架处理材料和工作站门禁；qi-physics minor 骨架处理账本通用审计容量。本骨架只覆盖 forge 淬炼费用与创建时间的两处局部根因。

## §6 修复计划骨架

- **P0**：把淬炼扣费放在接受命中的权威 system，先用 `transfer_external_qi_to_ledger` 以 `QiTransferReason::MeridianForge` 验证/记账，成功后才提交 `Cultivation.qi_current` 与 `TemperingState`；缺 ledger、zone 或余额时 fail closed，不消费命中。
- **P0**：在构造 `artifact_state_for_outcome` 前取得 `CombatClock.tick` 并传入 `created_at_tick`，让 `last_maintenance_tick` 与实际结算 tick 同源。
- **P1**：覆盖命中不足、缺账本、zone 满走稳定 overflow、服务重启 tick 与 ledger 写入失败的测试；守恒断言调用 `assert_conservation(before, after, 0.0)` 并引用 `SPIRIT_QI_TOTAL`。

## §7 验证计划

实现后只跑 server 栈 fmt、clippy、cargo test，重点 `forge::{steps,mod,inventory_bridge}` 与 qi ledger 守恒测试；agent/client 无代码改动，不跑其门禁。
