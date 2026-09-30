# plan-bughunt-qi-physics-minor-cleanups-v1

> **来源 issue**：#1514、#1497。`WorldQiAccount` 审计向量没有生命周期上限，`EnvField::new` 又把非有限 zone qi 直接送入 `f64::clamp`；两者都是 qi_physics 输入/观测边界问题。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 审计留痕容量策略与 EnvField 非有限输入策略 | ⬜ |
| P1 | 守恒、重启、NaN/Inf 边界测试 | ⬜ |

## §0 摘要

`WorldQiAccount::transfers`（`ledger.rs:494-498`）由 `transfer` 和 `push_transfer_audit` 只增不减；neg pressure、tsy drain、cultivation tick 等高频路径会让长服内存无界增长（#1514）。`EnvField::new`（`env.rs:108-112`）对 NaN 调 `clamp`，NaN 会继续污染阈值判断，而同文件其他 setter 已把非有限输入归零（#1497）。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` §二/§十的真元守恒和灵压输入；不改变任何衰减公式。
- **finished_plans**：查 `plan-qi-physics-v1`、patch、conservation leaks 和 runtime clock；均未给 transfers retention 或 `EnvField::new` 规定当前边界。
- **active plan**：查 qi ledger、zone economy、persistence slices；没有同一审计容量/NaN 处理实现。
- **skeleton**：查 `plan-bughunt-qi-ledger-asymmetry-v1`、botany growth ledger、negative-zone release；它们消费 ledger 但不拥有 audit vector 生命周期。
- **reminder.md**：查 qi、审计、NaN、overflow；仓内有该文件，无这两个 issue 的登记。

## §2 接入面与跨仓契约

- **Inputs**：`QiTransfer`/`QiTransferReason`、`WorldQiAccount::transfer`/`push_transfer_audit`、`EnvField::new(local_zone_qi)`、zone runtime qi。
- **Outputs**：有界且可审计的 transfer history/metrics；非有限环境输入 fail closed 或归零并保持阈值行为确定。
- **共享类型/事件**：复用 `QiTransfer`、`WorldQiAccount`、`QiPhysicsError`、`EnvField`、`QI_EPSILON`；不新增第二套 ledger 或环境字段。
- **三端契约符号**：server `qi_physics::{ledger::{WorldQiAccount,QiTransfer,assert_conservation},env::EnvField}`；agent **无变更**，现有 `bong:qi/ledger` telemetry 形状不扩展；client **无变更**，只消费既有视界/环境 payload。
- **worldview/qi**：所有余额改动仍明确写成 `ledger.transfer(QiTransfer { from, to, amount, reason })`；外部 `Cultivation.qi_current` 来源使用 `transfer_external_qi_to_ledger`。守恒测试引用 `SPIRIT_QI_TOTAL`，审计裁剪不能删除未结算余额。

## §3 游玩影响与复现

持续运行每 tick 的 qi drain/释放路径，观察 `WorldQiAccount::transfers().len()` 单调增加；构造 `EnvField::new(f64::NAN)`，后续 `is_finite`/阈值判断得到 NaN 并绕过正常比较。

## §4 `origin/main` 根因证据

- `server/src/qi_physics/ledger.rs:494-498` 保存无界 `transfers: Vec<QiTransfer>`；`:830-835` 的 `push_transfer_audit` 只 push；全仓无生产 clear/truncate/retain。
- `server/src/qi_physics/env.rs:108-112` 直接 `local_zone_qi.clamp(0.0, 1.0)`；同文件 `with_turbulence` `:123-128` 与 `with_law_disruption` `:132-137` 已有非有限归零对照。

## §5 非重复比对

`plan-bughunt-qi-ledger-asymmetry-v1` 处理 owner/单位/事务，不拥有 audit retention；`plan-bughunt-negative-zone-release-v1` 处理负 zone release，不处理 NaN；本骨架只收 qi_physics 的通用边界。

## §6 修复计划骨架

- **P0**：区分余额与审计：保留未结算余额不裁剪，给 audit history 采用按 tick/容量的有界 ring 或 durable metrics，并明确保留/落盘策略；任何 `QiTransfer` 仍先经过 `ledger.transfer`，不能为了限长丢掉 balance mutation。
- **P0**：`EnvField::new` 与其他 setter 统一 `is_finite` 策略（非法输入 fail closed/归零），禁止 NaN 进入 `clamp` 或下游物理公式。
- **P1**：用 `assert_conservation(before, after, 0.0)`、`SPIRIT_QI_TOTAL` pin 审计裁剪不改余额；NaN/±Inf/边界 0/1 测试覆盖环境输入。

## §7 验证计划

实现后跑 server fmt/clippy/cargo test，重点 `qi_physics::ledger` 和 `qi_physics::env`；不跑其他栈。
