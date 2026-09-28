# plan-bughunt-dugu-baomai-qi-max-shrink-ledger-v1

> 来源 Issue：#1385、#1801。两处都是 `Cultivation.qi_current` 在线真元在 `qi_max` 缩小时被 `.clamp/.min` 静默截断，差额没有释放到 zone 或真实 overflow 账户。

## §0 摘要

毒蛊经脉 tick（`cultivation/dugu.rs`）降低 `Meridian.flow_capacity` 后重算 `qi_max`，直接把 `Cultivation.qi_current` clamp 到新上限。爆脉散功（`combat/baomai_v3/skills.rs`）调用 `qi_physics::field::body_transcendence` 后同样缩小 `qi_max` 并 clamp。在线玩家的真元权威是 `Cultivation.qi_current`，不是 ledger 中的 player 账户；两条路径都没有为超额部分执行 `Cultivation::resize_qi_max_and_release_excess` 或等价的真实外部源转账，因此破坏 `SPIRIT_QI_TOTAL` 守恒。

本 plan 只写骨架，先由 BugFix agent 按真实资源可用性接线。

## §1 实际游玩体验影响

- 玩家在高真元时被毒蛊蚀脉或施放散功，超出新上限的真元会凭空消失，zone 没有得到回流。
- 爆脉是可重复战斗技能，长期使用会持续减少全服可观察真元；毒蛊则在每个 tick 产生隐性损耗。
- 失败/缺少 zone 或 ledger 资源时若继续 clamp，客户端只看到数值下降，无法审计损失。

## §2 复现路径

1. 令在线角色 `Cultivation.qi_current` 接近 `qi_max`，在正常玩法中附着毒蛊，等 `DUGU_POISON_TICK_INTERVAL` 到期；或在化虚角色上施放 `baomai_v3::cast_disperse`。
2. 毒蛊路径执行 `recompute_qi_max`，爆脉路径执行 `body_transcendence` + `apply_qi_max_loss`。
3. 新 `qi_max` 小于旧 `qi_current` 时，当前代码直接 `.clamp/.min`，没有 `QiTransfer` 的 `ReleaseToZone`、没有 zone 字段回写和 overflow 入账。
4. 对照前后 `Cultivation.qi_current`、zone `spirit_qi`、`WorldQiAccount` 与 `SPIRIT_QI_TOTAL`，差额无落点。

## §3 根因证据

- `server/src/cultivation/dugu.rs:350-375`：`dugu_poison_tick` 降低 flow capacity 后 `cultivation.qi_max = recompute_qi_max(&meridians)`，紧接着 `cultivation.qi_current = cultivation.qi_current.clamp(0.0, cultivation.qi_max)`。
- `server/src/cultivation/dugu.rs:500-523` 的解毒/失败同类路径也重算并 clamp，实施时须一并审计，不能只修一个 tick 分支。
- `server/src/qi_physics/field.rs:262-283` 的 `body_transcendence` 只返回 `qi_max_lost`；`server/src/combat/baomai_v3/skills.rs:542-570` 调用后进入 `apply_qi_max_loss`，`:776-785` 直接写新上限和 clamp。
- 已有正确底盘：`server/src/cultivation/components/qi_flow.rs:499-531` 的 `Cultivation::resize_qi_max_and_release_excess` 先释放 excess，成功后才写 `qi_max`，并收敛 `qi_max_frozen`。
- 账本 API 的真实签名：`qi_physics::ledger::transfer_external_qi_to_ledger(&mut WorldQiAccount, from: QiAccountId, to: QiAccountId, amount: f64, reason: QiTransferReason) -> Result<Option<QiTransfer>, QiPhysicsError>`；zone ledger→字段路径是 `transfer_ledger_qi_to_zone(&mut WorldQiAccount, from, zone_name: &str, zone_spirit_qi: &mut f64, requested: f64, zone_ceiling: f64, reason) -> Result<Option<QiTransfer>, QiPhysicsError>`。

## §4 非重复比对

- `docs/finished_plans/plan-bughunt-qimax-shrink-clamp-leak-v1.md` 已修延寿丹和断续散（commit `eb16b224d`），但其 Finish Evidence 未覆盖 `cultivation/dugu.rs` 或 `combat/baomai_v3/skills.rs` 的生产入口；本 finding 是同根因在另外两条路径的漏网点。
- `race_change.rs` 的 excess 释放是实现先例，不代表本 issue 已修：其 `apply_qi_excess_release` 对无 zone 的 `accepted` 仍有另一个独立缺口（见 #1625）。
- `dugu_v2::release_cast_cost_to_zone` 的正常 flat cost 释放不覆盖 `qi_max` 缩容；不能以“该模块有 ledger helper”作为已修证据。

## 接入面与跨仓契约

- **Inputs**：在线实体的 `Cultivation { qi_current, qi_max, qi_max_frozen }`（权威）、`MeridianSystem`、毒蛊/爆脉技能事件、`Position`、`CurrentDimension`、`ZoneRegistry`、`WorldQiAccount`、`Events<QiTransfer>`。
- **Outputs**：成功释放后更新 `Cultivation.qi_current/qi_max`、zone `spirit_qi` 或持久化 overflow；发出同一笔转账的审计事件。
- **共享类型 / 事件**：复用 `QiTransfer`、`QiTransferReason::ReleaseToZone`、`WorldQiAccount`、`ZoneRegistry`、`QiMaxShrinkReleaseContext`/`Cultivation::resize_qi_max_and_release_excess`；不新增跨仓 payload。
- **server 契约符号**：`dugu_poison_tick`、`recompute_qi_max`、`body_transcendence`、`cast_disperse`、`apply_qi_max_loss`、`qi_physics::ledger::assert_conservation`。
- **agent**：无变更。两条损耗都在 server 内部，agent 不拥有在线玩家真元。
- **client**：无变更。既有 cultivation snapshot 会自然反映最终值，不修改 payload。
- **worldview 锚点**：`docs/worldview.md §二 L18-L22`（总量恒定、流动守压强法则）；`§四 L357-L360`（爆脉过载的经脉代价）；`§五 L423-L426`（毒蛊造成 qi_max 永久下降）。
- **qi_physics 锚点**：活体权威始终是 `Cultivation.qi_current`；不要对不存在余额的 `QiAccountId::player` 直接 `ledger.transfer`。优先调用 `Cultivation::resize_qi_max_and_release_excess` / `release_to_zone`，其内部通过外部源 helper 入账；若需要手写字段镜像，必须先核对 `transfer_ledger_qi_to_zone` 的签名。测试守恒使用 `SPIRIT_QI_TOTAL`（`server/src/schema/common.rs:5`）和 `assert_conservation(before, after, era_decay)`，第三参数保持实际 era decay。

## §5 修复骨架

### P0 统一缩容事务

- 把 Dugu、Baomai 的 raw `qi_max` 修改改为预检 excess → 真实释放 → 成功后提交新上限；资源不足、身份缺失或 ledger 错误时 fail closed，不得 clamp。
- 释放到正/负 zone 时保留 signed `zone.spirit_qi`，禁止 `.max(0.0)`；zone 满时通过真实 overflow 账户入账，不能只 emit 事件。
- 审计 Dugu 的所有 `recompute_qi_max` callsite，保证任一缩容路径都走同一事务。

### P1 回归与守恒

- 覆盖毒蛊 tick、解毒失败/成功、爆脉散功：有 excess 时 zone/overflow 恰增差额，无 excess 时不生成转账。
- 覆盖 ledger/zone 缺失的拒绝原子性；用 `assert_conservation` 对拍 `SPIRIT_QI_TOTAL`，不写字面量 100.0。

