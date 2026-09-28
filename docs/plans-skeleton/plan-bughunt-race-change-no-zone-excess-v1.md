# plan-bughunt-race-change-no-zone-excess-v1

> 来源 Issue：#1625。RaceChange 在没有 zone 时把 excess 预检为 accepted，但 commit 没有把它落入 overflow。

## §0 摘要

`precheck_race_change` 在找不到 zone 时把 `to_capacity` 设为 `f64::MAX`，因此 `prepare_transfer` 返回 `accepted = excess、overflow = 0`。`apply_qi_excess_release` 随后只有 `zone_name` 分支和 `overflow > epsilon` 分支；`zone_name = None` 且 overflow 为零时两个分支都跳过，commit 仍把 `qi_current` 钳到新上限，excess 没有任何落点。

## §1 实际游玩体验影响

- 玩家在 zone 外换种族时会凭空损失超出新上限的真元。
- 同一次 RaceChange 在 zone 内和 zone 外的守恒结果不同，且没有失败提示。
- 这条路径直接影响 `SPIRIT_QI_TOTAL`，属于高优先级 ledger 缺口。

## §2 复现路径

1. 让玩家 `qi_current > new_qi_max`，并移除 `Position`/`ZoneRegistry` 命中，使 `zone_lookup = None`。
2. 运行 `precheck_race_change`，观察 `QiExcessReleasePlan { zone_name: None, transfer.accepted: excess, transfer.overflow: 0 }`。
3. 运行 `commit_race_change`，`apply_qi_excess_release` 不发 transfer、不写 overflow，但 current 已降到 new max。
4. 用 `Cultivation::release_to_zone(None, ...)` 的现有测试先例对照正确行为：缺 zone 时应进入持久化 overflow。

## §3 根因证据

- `server/src/cultivation/race_change.rs:196-224` 把缺 zone 情况建模为无限容量 accepted，而不是 overflow。
- `server/src/cultivation/race_change.rs:306-347` 仅在 `Some(zone_name)` 或 `transfer.overflow > epsilon` 时写入；`None + accepted` 没有分支。
- `server/src/cultivation/components/qi_flow.rs:359-377` 的 `Cultivation::release_to_zone` 明确支持 `zone = None`，并把完整请求写进 `qi_flow_overflow`。
- 在线玩家真元权威是 `Cultivation.qi_current`；不能只发一条 emit-only `QiTransfer`，必须让 `transfer_external_qi_to_ledger` 或对应 release helper 真实落账。

## §4 非重复比对

- `docs/finished_plans/plan-bughunt-qimax-shrink-clamp-leak-v1.md` 已覆盖其他缩容入口，但没有覆盖 RaceChange 的 no-zone custom plan/apply 分支。
- `plan-bughunt-dugu-baomai-qi-max-shrink-ledger-v1.md` 关注 Dugu/Baomai；本 issue 的根因是 RaceChange 自己的 TransferPlan 分流逻辑。
- #1498 只修 `qi_max_frozen` 元数据，不会使 excess 获得落点，两个 issue 仍需分开。

## 接入面与跨仓契约

- **Inputs**：`RaceChangeCommitPlan`、`QiExcessReleasePlan`、玩家 `Cultivation.qi_current`、可选 `ZoneRegistry`、`WorldQiAccount`、`Events<QiTransfer>`。
- **Outputs**：zone 存在时 accepted 更新 signed `Zone.spirit_qi`，zone 缺失/满时 overflow 余额增加；current 减少量与两者严格相等。
- **共享类型 / 事件**：`TransferPlan`、`QiTransfer`、`QiTransferReason::ReleaseToZone`、`Cultivation::release_to_zone`、`transfer_external_qi_to_ledger`、`qi_flow_overflow` 账户；不改 agent/client payload。
- **server 契约符号**：`precheck_race_change`、`commit_race_change`、`apply_qi_excess_release`、`prepare_transfer`、`Cultivation::release_to_zone`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更。RaceChange 与 qi release 在 server 内完成，agent 不持有玩家 current。
- **client**：无变更。已有 cultivation detail 会显示 commit 后 current/max，overflow 只用于服务端审计。
- **worldview 锚点**：`docs/worldview.md §二 L18-L22`（守恒）；`§三 L65-L77`（真元池与上限）。
- **qi_physics**：在线 current 必须作为外部源处理。zone 路径可用 `Cultivation::release_to_zone(Some(zone), ledger, actor, excess, ReleaseToZone)`；无 zone 直接传 `None`，由 helper 真实写入 overflow。若手写事务，核对 `transfer_external_qi_to_ledger(&mut WorldQiAccount, from, to, amount, reason)` 签名并在成功后才提交 current；不要把 `accepted` 当作已落账。

## §5 修复骨架

### P0 no-zone 分流

- 取消 `None -> f64::MAX/accepted` 的假容量语义，预检结果明确区分 zone accepted 与 overflow。
- `apply_qi_excess_release` 对缺 zone 的全部 excess 执行真实 overflow transfer；任何 ledger 失败都阻止 current/max commit，保持 precheck/commit 原子性。
- 复核 zone 满、负 zone 和 Position 缺失三条路径，不能通过后置 clamp 兜底。

### P1 回归与守恒

- 覆盖 zone 内有空间、zone 满、zone 缺失、Position 缺失和 ledger 失败。
- `assert_conservation(before, after, era_decay)` 引用 `SPIRIT_QI_TOTAL`，断言 accepted + overflow = current 减少量。
