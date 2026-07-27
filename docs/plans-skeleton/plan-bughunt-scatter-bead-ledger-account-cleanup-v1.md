# plan-bughunt-scatter-bead-ledger-account-cleanup-v1（骨架）

> 一句话主题：散灵珠 burial record 终止时同步删除 `WorldQiAccount` 账户，避免长跑 server 留下无界零余额/epsilon 僵尸账户与 telemetry 膨胀。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 枚举自然耗尽、主动触发、重复终止三条账户生命周期 | ⬜ |
| P1 | burial + ledger 原子/同 tick cleanup | ⬜ |
| P2 | 守恒/重启/telemetry 回归 + server gate | ⬜ |

## 接入面

- **进料**：`server/src/zhenfa/mod.rs::tick_scatter_bead_excretion`、主动 trigger handler、`ScatterBeadBurials`、`WorldQiAccount::remove_balance`。
- **出料**：珠内 qi 先按既有 ledger 路径释放；终止后 burial 与对应 account 同时不可见。
- **共享类型 / event**：复用 `QiAccountId` 与 `remove_balance`；不另造墓地账本。
- **跨仓库契约**：纯 server，无 wire 改动。
- **worldview 锚点**：散灵珠是 qi 中转载体，不是永久账户。
- **qi_physics 锚点**：先完成真实余额转移，再删除零/epsilon account；不得用删除账户代替 qi 释放。

## 当前证据（origin/main @ c625d5a5）

`server/src/zhenfa/mod.rs:215-229,2589-2647` 的主动触发成功链先从 `burials.beads` 取走记录并完成余额释放，却没有删除对应 account；`:2726-2732` 的自然耗尽链也只移除 burial。canonical 删除 API 是 `server/src/qi_physics/ledger.rs:404-405` 的 `WorldQiAccount::remove_balance`（调用先例在 `server/src/lingtian/qi_account.rs:78`），因此终止账户继续参与 balances map、`total()` 与 telemetry 枚举。`:2531` 只是 `set_balance` 初始化失败时回滚刚插入的 burial，此时 account 未创建，不属于本缺陷的成功终止路径。

## 验收

1. 自然耗尽、主动触发、初始已空、重复终止、未知 bead ID 全覆盖。
2. 非零余额必须先守恒释放；只有余额在 canonical epsilon 内才允许删除，异常非零余额 fail-closed/告警。
3. 终止后 burial 与 ledger key 均不存在，重复事件 no-op，不影响其他 bead。
4. restart 与 telemetry 测试证明不会复活/输出已终止账户；`assert_conservation` 前后成立。
5. 完整 server gate。

## 边界

- 不替代 `plan-bughunt-scatter-bead-burial-restart-loss-v1` 的 persistence owner；本 plan 只修运行时账户终止。
- 不改变珠容量、释放速率、距离或阵法效果。
