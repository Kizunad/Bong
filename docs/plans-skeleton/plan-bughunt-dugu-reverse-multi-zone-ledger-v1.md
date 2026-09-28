# plan-bughunt-dugu-reverse-multi-zone-ledger-v1

> **来源 issue**：#1635。`dugu.reverse` 清零全图受害者的 `Cultivation.qi_current` 后，把总额按施法中心单一 zone 入账，跨 zone 时只保全局数量而丢失来源 zone 归属。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 每个 victim 的 dimension/zone 与 qi release 绑定 | ⬜ |
| P1 | 多 zone、无 zone、overflow 和全局守恒回归 | ⬜ |

## §0 摘要

`apply_reverse` 在 `target=None` 时通过 `all_permanent_taint_targets` 收集跨地图目标（`skills.rs:518-521`），循环把每个目标的 qi 清零并聚合成 `victim_qi_total`（`:538-553`），事件只携带 `center`。消费者 `reverse_victim_qi_zone_credit_tick`（`tick.rs:379-430`）因此只能用施法者维度与中心坐标选择一个 zone；受害者所在 zone 的账本归属被改写。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` §二/§十的真元零和和区域灵气语义；本骨架不改变 Dugu 技能规则。
- **finished_plans**：查 `plan-dugu-v2`、`plan-qi-physics-v1`、Dugu qi ledger 归档；已有测试只覆盖单中心/overflow，不覆盖 per-victim zone。
- **active plan**：查 `docs/plan-*.md` 的 Dugu reverse、qi ledger 和 combat event；没有同一 multi-zone 归属修复。
- **skeleton**：查 `plan-bughunt-dugu-v2-eclipse-qi-ledger-v1`、`plan-bughunt-dugu-v2-taint-decay-ledger-v1` 与 `plan-bughunt-qi-ledger-asymmetry-v1`；它们不处理 Reverse victim 聚合。
- **reminder.md**：查 Dugu、Reverse、victim qi、zone 关键词；仓内有 reminder.md，但无本 issue 登记。

## §2 接入面与跨仓契约

- **Inputs**：`DuguReverseVictimQiEvent`、每个 victim 的 `Position`/`CurrentDimension`/`Cultivation.qi_current`、`ZoneRegistry`、`WorldQiAccount`。
- **Outputs**：每个 victim 的 raw qi 只归还其真实 zone，缺 zone 稳定进入 overflow；`ReverseTriggeredEvent` 的 taint residue 与 victim qi 仍是两条独立路径。
- **共享类型/事件**：复用 `DuguReverseVictimQiEvent`、`QiFlowOutcome`、`QiTransfer`、`QiTransferReason::{DuguReverseVictimQi,ReleaseToZone}`、`qi_release_to_zone`/`transfer_external_qi_to_ledger`、`assert_conservation`；若修改 event，必须同步 server schema mirror 与测试，不另造第二个 Reverse event。
- **三端契约符号**：server `dugu_v2::skills::apply_reverse`、`events::DuguReverseVictimQiEvent`、`tick::reverse_victim_qi_zone_credit_tick`；agent **无变更**，事件不出 Redis；client **无变更**，VFX/audio 仍消费 `ReverseTriggeredEvent`，zone 归属是 server 内部。
- **worldview/qi**：`Cultivation.qi_current` 是 victim 的外部真元权威；先按真实 API 扣除，再以 `QiTransfer { from, to, amount, reason }` 记账。ledger 账户之间调用 `ledger.transfer`，外部玩家来源调用 `transfer_external_qi_to_ledger`，释放到 zone 使用 `qi_release_to_zone`/`ReleaseToZone`。守恒断言引用 `SPIRIT_QI_TOTAL`。

## §3 游玩影响与复现

在两个不同维度/zone 各放一个带永久 taint 的目标，Void 玩家施放无显式 target 的 Reverse。两个目标 qi 都被清零，但当前实现把总额写进中心 zone；目标所在 zone 的灵气不会回正，中心 zone 产生不应有的 credit。

## §4 `origin/main` 根因证据

- `server/src/combat/dugu_v2/skills.rs:518-553` 的 `target=None` 收集全图目标并清零，只累计一个 `victim_qi_total`。
- `server/src/combat/dugu_v2/skills.rs:556-595` 只把 `center` 放入事件；没有 victim 列表、位置或维度。
- `server/src/combat/dugu_v2/tick.rs:379-416` 以 caster dimension + `event.center` 找单一 zone，随后 `qi_release_to_zone` 处理总额。
- 现有 `tick.rs:417-465` 的 overflow/审计分支只能保证数量落点，不会恢复被聚合掉的来源 zone。

## §5 非重复比对

Eclipse/taint decay 骨架处理脏气残留和 qi_max 缩容；`plan-bughunt-combat-qi-max-shrink-ledger-v1` 不触及 Reverse victim 清零；`plan-bughunt-qi-ledger-asymmetry-v1` 的道伥 owner 也不共用该 event。这里唯一根因是多目标事件丢失 per-victim 空间身份。

## §6 修复计划骨架

- **P0**：扩展内部事件携带每个受害者的稳定 entity/character identity、位置和 dimension，或在清零前立即按 victim 调用统一 release helper；禁止用中心 zone 代替来源 zone。缺身份/zone 时 fail closed 或进入稳定 overflow，不得丢弃。
- **P1**：多 zone、跨维、同 zone、多 victim qi=0、zone 满载 overflow、无 zone 六组测试；用 `qi_physics::ledger::assert_conservation` 对比前后 `SPIRIT_QI_TOTAL`，`era_decay=0.0`。

## §7 验证计划

实现后跑 server fmt/clippy/cargo test，重点 `combat::dugu_v2` 与 qi ledger；本骨架阶段不编译、不改代码。
