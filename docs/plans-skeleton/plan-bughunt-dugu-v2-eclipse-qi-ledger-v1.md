# plan-bughunt-dugu-v2-eclipse-qi-ledger-v1

## §0 摘要

**来源 Issue：#1864、#1373。** Dugu v2 的 `Eclipse` 以名义 `effect.qi_loss` 计算脏真元碰撞与 taint intensity，却可能只从目标实际扣除较小的余额；低真元目标因此留下超过实际扣减的标记，之后倒蚀按标记强度返还会铸造真元。Immediate tier 又不创建 `TaintMark`，实际扣下的脏真元没有任何 zone/ledger 去向。两种 tier 是同一 Eclipse 账本边界的两个漏口，本骨架不改生产代码。

接入面：进料为 `Cultivation.qi_current`、`eclipse_effect`、`dirty_qi_collision` 与 `TaintMark`；出料为目标余额、taint residue、zone/overflow 与 `QiTransfer`/事件审计。共享 `QiPhysics` 的 release helper、`EclipseNeedleEvent`、`ReverseTriggeredEvent` 和 `DuguReverseVictimQiEvent`；agent/client 不消费这些内部事件，wire 契约不变。依据 `docs/worldview.md` §二和 §十，目标实际扣掉的每一点真元只能流向另一个 owner 或 zone。

## §1 游玩影响

- 对当前真元低于 `effect.qi_loss` 的玩家施放 Eclipse 后，后续 Reverse 会按未曾拥有的 intensity 释放真元，世界总量增加。
- Awaken/Induce/Condense 等 Immediate tier 的实际扣减则没有 TaintMark 或 zone credit，真元直接消失。
- 玩家只看到中毒/倒蚀效果，无法从技能反馈判断账本是否多付或少付；重复施法会累积偏差。

## §2 复现路径

1. 令目标 `qi_current` 小于 Eclipse 的名义 `effect.qi_loss`，或令其为零；施放 `DuguSkillId::Eclipse`。
2. 当前代码用 `.max(0.0)` 扣减余额，但把 `collision.effective_hit.max(1.0)` 写入 Temporary/Permanent `TaintMark.intensity`。
3. 对前一种 tier 再施放 `Reverse`；`reverse_burst_all_marks` 与 `reverse_zone_credit_tick` 按 intensity 返还，金额可超过目标实际扣减。
4. 对 Immediate tier 施法后检查 `EclipseNeedleEvent.returned_zone_qi`、zone 与 ledger；当前没有对应的释放或标记。

## §3 根因证据

- `server/src/combat/dugu_v2/skills.rs:214-258` 以 `effect.qi_loss` 创建 `dirty_qi_collision`，但只执行 `target.qi_current = (before - effect.qi_loss).max(0.0)`；实际扣减应是 `min(before, effect.qi_loss)`。
- `server/src/combat/dugu_v2/skills.rs:255-273` 在 Temporary/Permanent 分支把 `collision.effective_hit.max(1.0)` 作为 `TaintMark.intensity`，没有用实际扣除量/实际 residue 进行封顶；Immediate 分支则根本不进入 `mark_to_insert`。
- `server/src/combat/dugu_v2/skills.rs:294-315` 把名义 `collision.returned_zone_qi` 直接塞进 `EclipseNeedleEvent`。该观察事件本身不会改变 zone balance，不能替代真实 `QiTransfer`。
- `server/src/combat/dugu_v2/skills.rs:500-590` 的 Reverse 已单独累计并发出被清零目标的实际 `victim_qi_total`，但这只能修 Reverse 清零阶段，不能修 Eclipse 先前产生的过大 intensity 或 Immediate 遗漏。

## §4 非重复比对

- `docs/finished_plans/plan-bughunt-dugu-v2-qi-release-v1.md`/commit `c65c8de7f` 处理 Penetrate/Reverse 的目标清零归还；本骨架针对 Eclipse 的实际 debit、taint intensity 和 Immediate tier，触发点不同。
- `plan-bughunt-dugu-v2-taint-decay-ledger-v1` 只处理临时 mark 到期时释放 `TaintMark` intensity，本骨架处理 mark 创建和一次性扣减，二者按生命周期前后相接但不是同一根因。
- `plan-bughunt-combat-qi-max-shrink-ledger-v1` 处理 `qi_max` 收缩后的 clamp，不处理 Eclipse 的 `qi_loss`/taint residue。

## §5 修复计划骨架

### P0：Eclipse debit 与 residue 对齐

- 计算并保存实际扣除量，限制 taint intensity、returned residue 和后续 Reverse 返还不超过该量；明确 collision 只是公式结果，真实余额变更必须先提交。
- 为 Immediate tier 增加明确的 zone/overflow `ReleaseToZone` 路径，或建立能承载 residue 的状态；真实账户间提交 `ledger.transfer(QiTransfer { from, to, amount, reason })`，不能让 `EclipseNeedleEvent` 成为唯一 consumer。
- Temporary/Permanent 的 `qi_max` 缩容若产生 current excess，沿统一缩容释放合同处理，避免在 Eclipse 修复中引入第二套账本。

### P1：回归契约

- 低于名义 qi_loss、恰好等于名义 qi_loss、余额充足三组均断言 Reverse 总返还不超过实际扣减。
- Immediate、Temporary、Permanent 各保留一个代表 case：目标余额、zone/overflow 和 `QiTransfer` 合计守恒；重复施法不重复释放。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，重点覆盖 `dugu_v2::skills`、`dugu_v2::tick` 与 ledger helper。守恒测试引用 `SPIRIT_QI_TOTAL`/`QI_ZONE_UNIT_CAPACITY` 等生产常量，不写裸总量。本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Server：** producer 是 `dugu_v2::skills::resolve_dugu_v2_skill` 中的 `apply_eclipse`；消费者/对照是 `dugu_v2::tick::eclipse_zone_credit_tick`、`reverse_zone_credit_tick` 与 `apply_reverse`。测试应直接观察目标 `Cultivation.qi_current`、`TaintMark.intensity`、`EclipseNeedleEvent` 和 zone/overflow。
- **Qi：** Immediate 的实际扣除必须由真实余额结算承载；zone 回灌使用 `qi_release_to_zone` 与 `QI_ZONE_UNIT_CAPACITY`，overflow 使用 `QiAccountId::overflow`。任何真正的 ledger 账户搬运都明确调用 `ledger.transfer(QiTransfer { from, to, amount, reason })`，其中 `QiTransferReason::Healing`/`ReleaseToZone` 按去向选择；`QiTransferReason::DuguReturnToZone` 与 `DuguReverseVictimQi` 是当前枚举标记的 audit-only reason，只能 `push_transfer_audit`，不得误调 `WorldQiAccount::transfer`。守恒断言引用 `QI_EPSILON`、`DEFAULT_SPIRIT_QI_TOTAL`（生产预算；现有 combat fixture 的 `SPIRIT_QI_TOTAL` 来自 `schema::common`）和 `assert_conservation`。
- **Agent：无变更。** 证据是 `EclipseNeedleEvent`、`ReverseTriggeredEvent` 与 `DuguReverseVictimQiEvent` 都是 server 内部事件，agent 不消费其字段。
- **Client：无变更。** 证据是 Eclipse/Reverse 的现有技能与视觉事件 wire 保持原 payload；本修复只校正 server 余额和账本去向。
