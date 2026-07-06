# plan-bughunt-dying-elder-release-overflow-v1

> **Skeleton（bughunt）**。一句话主题：垂死大能在第 5 颗丹后的守信自裁 / 夺舍力竭死亡结算中，把 `bb.qi_current` 交给 `qi_release_to_zone` 时使用了 `spirit_qi` 比例单位和 `cap=1.0`，随后只审计 `accepted`、丢弃 `overflow`，再把大能真元清零，导致化虚级剩余真元大头蒸发。

## Bug 摘要

`server/src/fauna/dying_elder.rs` 的死亡释放路径声称“全额 qi release”，但实际调用：

- `zone_current_qi = zone.spirit_qi`，未乘 `QI_ZONE_UNIT_CAPACITY`。
- `DYING_ELDER_ZONE_RELEASE_CAP = 1.0` 被传入 `qi_release_to_zone` 的 `zone_cap`。
- `zone.spirit_qi = outcome.zone_after`，未按绝对真元量除回 `QI_ZONE_UNIT_CAPACITY`。
- 只发送 `outcome.transfer`（accepted 腿），没有处理 `outcome.overflow`。
- 最后 `bb.qi_current = 0.0`。

在典型 TSY zone `spirit_qi=-0.6`、大能 `qi_current≈500` 时，当前算法只把 zone 从 `-0.6` 推到 `1.0`，accepted 约 `1.6`，overflow 约 `498.4` 没有进入 overflow 账户，随后被清零。

## 对实际游玩体验的影响

玩家把垂死大能事件推进到第 5 颗丹后，最有戏剧性的两条结局都会踩中：守信自裁会掉传承，背叛路线会先夺走玩家当前真元再力竭死亡。当前实现让这些结局的“化虚级真元释放 / 负灵域回暖 / 末法守恒代价”变成只回灌很小一截，其余真元从世界账本里消失。

体感后果是：玩家付出多颗丹或被夺舍后，世界环境没有按设计获得对应的大能残响；后续 zone qi、ledger telemetry、天道预算审计都会低估这次事件遗留的真元。入口可达性 bug 修复后，这会从 dev / 测试路径变成正式玩法可触发的高价值遭遇结算错误。

## 证据定位

- `server/src/fauna/dying_elder.rs:574-578`：给丹先增加 `bb.qi_current`，最高可到 `qi_max_cache * 1.5`。
- `server/src/fauna/dying_elder.rs:608-642`：第 5 颗丹后，守信分支直接置 `Dead { dead_by_betrayal: false }`，此时 `bb.qi_current` 通常仍为正。
- `server/src/fauna/dying_elder.rs:705-708`：夺舍分支把玩家 `Cultivation.qi_current` 全额加到大能 `bb.qi_current`。
- `server/src/fauna/dying_elder.rs:792-795`：`DYING_ELDER_ZONE_RELEASE_CAP = 1.0` 注释称用于 `qi_release_to_zone` 的 `zone_cap`，但该 helper 的 canonical 调用使用绝对真元容量。
- `server/src/fauna/dying_elder.rs:969-1010`：死亡系统传入 `zone.spirit_qi` 和 `cap=1.0`，只处理 `outcome.transfer`，未处理 `outcome.overflow`，然后清零 `bb.qi_current`。
- `server/src/qi_physics/release.rs:27-45`：`qi_release_to_zone` 明确返回 `accepted` 和 `overflow`，且只为 accepted 创建 transfer。
- `server/src/cultivation/death_hooks.rs:333-371`：正典死亡释放用 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY`、`QI_ZONE_UNIT_CAPACITY` cap、写回 `/ QI_ZONE_UNIT_CAPACITY`，并把 overflow 路由到 overflow transfer。

## 触发路径

1. 垂死大能存在于 TSY / 负灵域，`DyingElderBlackboard.qi_current` 为正。
2. 玩家持续给回元丹，`dying_elder_give_dan_system` 每次增加大能真元并记录 `TradeDan` 审计。
3. 第 5 颗丹触发结局判定：
   - 守信：直接进入 `Dead { dead_by_betrayal: false }`。
   - 背叛：进入 `Betrayal`，`dying_elder_betray_system` 抽走玩家当前真元加入大能，再进入 `Dead { dead_by_betrayal: true }`。
4. `dying_elder_death_system` 处理 Dead 态，调用 `qi_release_to_zone`。
5. zone 只能接收约 `1.0 - spirit_qi` 的小比例量；大量 overflow 未落账，`bb.qi_current` 被清零。

自然力竭路线是例外：`dying_elder_drain_system` 在 `bb.qi_current <= 0.0` 时才置 Dead，通常没有剩余真元可释放。本 plan 不把自然耗尽作为主触发。

## 反方审查记录

Round 1 反方：

- 反驳点：`DYING_ELDER_ZONE_RELEASE_CAP = 1.0` 和现有测试都像是有意把 zone 限到 `spirit_qi=1.0`。
- 裁决：反驳失败。同文件注释反复写“全额释放 / 守恒”，测试只验证 `accepted + overflow == elder_qi`，没有验证系统处理 overflow；真实系统随后丢弃 overflow。
- 去重结论：不重复 #975 dormant 负灵域释放、#989 灵物磨损 overflow、#1000 heartbeat 伪灵脉、#1013 骨币面值、#988 垂死大能给丹输入断链。

Round 2 反方：

- 反驳点：自然死亡时 `qi_current=0`，是否实际影响很弱。
- 裁决：触发面收窄但仍成立。给丹第 5 颗后的守信自裁和夺舍力竭都会以正 `bb.qi_current` 进入 Dead，且夺舍会额外吞玩家当前真元。
- 边界裁决：不并入 `docs/plan-dying-elder-tsy-zones-unloaded-v1.md`。那个 plan 修入口可达性；本 plan 修实体已存在后的死亡结算守恒，两者根因和验收不同。

## Skeleton Fix Plan

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 垂死大能死亡释放单位统一到绝对真元量 | fix_pr | ⬜ |
| P1 | overflow 分流与 ledger / event 审计闭合 | fix_pr | ⬜ |
| P2 | 给丹结局端到端守恒回归 | fix_pr | ⬜ |

### P0 — 单位统一

- 在 `dying_elder_death_system` 中按正典死亡释放口径计算：
  - `zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY`
  - `zone_cap = QI_ZONE_UNIT_CAPACITY`
  - 写回 `zone.spirit_qi = outcome.zone_after / QI_ZONE_UNIT_CAPACITY`
- 删除或改名 `DYING_ELDER_ZONE_RELEASE_CAP`，避免继续把 `spirit_qi` 比例上限传给绝对真元接口。

### P1 — overflow 闭合

- 对 `outcome.overflow > QI_EPSILON` 创建 `QiAccountId::overflow("dying_elder_release:<entity>")` 的 `QiTransferReason::ReleaseToZone`。
- `accepted + overflow` 必须等于本次实际释放量。
- `qi_account.push_transfer_audit` 和 `qi_transfer_events.send` 要覆盖 accepted 与 overflow 两条腿；没有 `WorldQiAccount` 资源时仍至少发事件，避免审计完全缺失。

### P2 — 回归测试

- 用系统级测试驱动 `dying_elder_give_dan_system -> dying_elder_betray_system? -> dying_elder_death_system`。
- 分别覆盖守信自裁、夺舍力竭、zone 接近满、负灵域 zone、无 `WorldQiAccount` 降级。
- 保留自然力竭 `qi_current=0` 的 no-op 边界，防止误发 0 transfer。

## 验收测试计划

1. `death_system_release_uses_absolute_zone_capacity`：给 `spirit_qi=-0.6`、`release_amount=500`，断言 zone 写回不超过 `1.0`，accepted 为 `QI_ZONE_UNIT_CAPACITY - (-0.6 * QI_ZONE_UNIT_CAPACITY) = 80`，而不是 `1.6`。
2. `death_system_routes_overflow_to_overflow_account`：zone 容量不足时，断言 overflow transfer 金额等于 `release_amount - accepted`，且 `bb.qi_current` 清零前后的总释放量闭合。
3. `honorable_fifth_dan_death_preserves_qi`：跑到第 5 颗丹的守信结局，断言 `TradeDan` 加入的大能真元最终在 zone / overflow 中可审计。
4. `betrayal_death_releases_player_soul_seize_qi`：玩家被夺舍后，玩家 `qi_current` 减少量必须最终出现在大能死亡释放的 accepted + overflow 中。
5. `natural_exhaustion_zero_qi_no_transfer`：自然耗尽到 0 的死亡不产生假 transfer。

> 注：验收中的具体 accepted 数字应使用 `QI_ZONE_UNIT_CAPACITY` 常量推导，不写死全局真元预算字面值。

## 风险

- `WorldQiAccount` 当前在该路径多为 audit-only；修复 PR 要明确 accepted / overflow 是只留审计，还是也同步真实 ledger balance，避免和 `ZoneRegistry.spirit_qi` 双记。
- 如果同时修 `plan-dying-elder-tsy-zones-unloaded-v1`，该 bug 的可触发性会提高；两个 PR 需要在 e2e 顺序上协调，但不应合并成同一修复。
- 现有单测 pin 了 `DYING_ELDER_ZONE_RELEASE_CAP = 1.0`，修复时需要删除错误 pin 或改成“zone fraction 上限由写回 clamp 保证，release cap 使用绝对容量”的契约测试。
