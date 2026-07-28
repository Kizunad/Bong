# plan-bughunt-scatter-bead-ledger-account-cleanup-v1（骨架）

> 一句话主题：散灵珠 source 生命周期终止时，在真实余额经 ledger 完整释放且严格归零后删除 `WorldQiAccount` key，避免长跑 server 留下无界僵尸账户与 telemetry 膨胀。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 枚举直接使用、自然耗尽、主动触发、重复终止四条账户生命周期并冻结 strict-zero 合约 | ⬜ |
| P1 | R5 P3-B handoff 后：zhenfa-domain release + burial/account 同 tick cleanup | ⬜ |
| P2 | 守恒、失败补偿、重启/telemetry 回归 + server gate | ⬜ |

## 接入面

- **进料**：`server/src/zhenfa/mod.rs:215-229,2366-2449,2452-2734` 的直接使用、burial trigger、自然耗尽与既有 `release_scatter_qi_to_zone` 语义；`ScatterBeadBurials`；`WorldQiAccount::remove_balance`。
- **出料**：任意正余额先按既有 ledger 路径完整转入 zone/overflow；仅当 source 实际余额严格为 `0.0` 时删除 key；burial 与对应 account 同时终止。
- **共享类型 / event**：复用 `QiAccountId::{zone,overflow}`、`QiTransferReason::ReleaseToZone` 与 `remove_balance`；不另造墓地账本或私有 qi sink。
- **跨仓库契约**：纯 server，无 wire 改动。
- **worldview 锚点**：散灵珠是 qi 中转载体，不是永久账户。
- **qi_physics 锚点**：`QI_EPSILON` 只作比较/断言容差，不是可销毁余额；不得用 `remove_balance` 代替 qi 释放。

## 当前证据（origin/main @ c625d5a5）

- `server/src/zhenfa/mod.rs:2452-2587` 的非 burial 直接使用会创建并清空 source account，但成功后不删 key。
- `server/src/zhenfa/mod.rs:215-229,2589-2647` 的 burial 主动触发先移除记录并释放余额，成功后同样不删 source account；`:2651-2734` 的自然耗尽也只删除 burial。
- `server/src/zhenfa/mod.rs:2366-2379` 对 `amount <= QI_EPSILON` 直接返回，不能作为终止时销毁 epsilon-positive 余额的依据；`:2366-2449` 已定义 zone 可接收部分与 overflow 部分的 canonical release 语义。
- `server/src/qi_physics/ledger.rs:398-405` 的 `remove_balance` 只删除 map key，不做 transfer/守恒；`:1343-1356` 证明 epsilon-positive qi 仍按真实余额处理。

## 终止账本契约（P0 冻结）

1. 终止前读取实际 `WorldQiAccount` source balance；任意有限正余额（包括 `0 < balance <= QI_EPSILON`）均须沿既有 release 语义完整转移：zone 可接收部分进入 `QiAccountId::zone`，剩余部分进入该珠对应的 `QiAccountId::overflow`，全部使用 `QiTransferReason::ReleaseToZone`。
2. 只有 source 实际余额严格为 `0.0` 后才允许 `remove_balance`；burial 为零且 source key 不存在时可 idempotent no-op。
3. source 缺失但 burial 为正、source/burial 余额不一致、输入非有限、zone lookup/release/transfer 任一步失败时必须 fail closed：保留或恢复 burial/source，不得用 `set_balance` 覆盖真实余额后删除，也不得吞掉 epsilon-positive qi。
4. 主动 trigger 已先从 map 取走 burial；失败分支必须原样 reinstate。领域实现只消费 R5 P3-B 冻结的 `release_all_to_zone_then_close`（最终 canonical 命名以 R5 为准）事务 API，不在 zhenfa 另造账本实现；在 R5 P3-B 合入并满足 strict-zero/失败原子性/misuse 放行门前，P1 保持 BLOCKED。

## 验收

1. 非 burial 直接使用、burial 主动触发、自然耗尽、初始已空、重复终止、未知 bead ID 全覆盖。
2. 普通 zone 与满 zone/overflow 两条成功路径逐笔断言 source→target 的 ledger delta 与 audit；成功后 burial（若有）和 source key 均不存在。
3. 专门注入 `0 < balance <= QI_EPSILON`，断言残量被真实 transfer 后 source 严格归零再删 key；不得只靠 epsilon 容差让总量测试通过。
4. source 缺失/余额不一致、zone lookup、release 或 transfer 失败均 fail closed，burial 被恢复、未转移余额仍可见且其他 bead/account 不受影响。
5. restart 与 telemetry 证明已终止 key 不复活、不出现在 `iter_balances`/hash fields；现有历史 orphan 告警语义不变。
6. 完整 server gate，并以真实 transfer legs、target delta 与 audit 锁定守恒；只有 fixture 明确处理 zone/ledger mirror 口径时才追加全局 `assert_conservation`。

## 边界

- 不替代 `plan-bughunt-scatter-bead-burial-restart-loss-v1` 的 persistence owner；本 plan 只修运行时 source-account 终止。
- R5 唯一拥有 `server/src/qi_physics/**`：本 plan 只改 zhenfa-domain 调用方/测试，并只消费已合入的 R5 P3-B 原子 release+strict-close API；不得并行修改 ledger 实现，也不得以裸 `remove_balance` 临时绕过 handoff。
- 不改变珠容量、释放速率、距离或阵法效果。
