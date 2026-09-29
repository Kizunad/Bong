# plan-bughunt-yidao-treatment-qi-destination-v1

## §0 摘要

**来源 Issue：#1341。** 当前仍可施放的医道净化 `ContamPurge` 会从施术者 `Cultivation.qi_current` 扣除 `calc.qi_cost`，然后只调用 `emit_qi_transfer` 发一条 `Healing` 事件；患者余额没有增加，zone/overflow 也没有收到真实 ledger transfer。该事件没有全局 consumer，因此扣下的真元凭空消失。本骨架只记录现状，不修改生产代码。

接入面：进料是 `YidaoSkillId::ContamPurge`、施术者与患者 `Cultivation`、患者 `Contamination`；出料必须是患者 credit 或目标 zone/overflow 的唯一去向，并与 `YidaoEventV1` 审计一致。复用 `QiTransfer`、`qi_release_to_zone`、`WorldQiAccount` 和现有 yidao 施法/事件路径；agent/client schema 不需要新增字段。依据 `docs/worldview.md` §二、§十，治疗转移不能通过只写事件的假账完成。

## §1 游玩影响

- 玩家使用净化技能时，施术者真元下降但患者真元与环境均不增加；总量会随每次治疗下降。
- `Healing` 事件可能让 UI/日志看起来像完成了转移，掩盖了真实余额没有变化的事实。
- 这是正常 `ContamPurge` 施法路径，满足污染存在且真元足够即可复现。

## §2 复现路径

1. 让患者拥有正的 `Contamination`，施术者拥有足够 `qi_current`，并通过 yidao 的经脉/境界/范围门禁。
2. 完成 `resolve_contam_purge_skill` 的蓄力结算。
3. `apply_contam_purge` 调 `debit_caster_qi`，缩减施术者余额；随后缩放患者污染并 `emit_qi_transfer`。
4. 检查患者 `qi_current`、所在 zone 和 `WorldQiAccount`：当前均未收到 `calc.qi_cost`。

## §3 根因证据

- `server/src/combat/yidao.rs:735-770` 的 `apply_contam_purge` 在污染存在时执行 `debit_caster_qi(world, caster, calc.qi_cost)`，随后只修改 `Contamination` 并调用 `emit_qi_transfer(world, caster, patient, calc.qi_cost)`。
- `server/src/combat/yidao.rs:1070-1103` 的 `debit_caster_qi` 只直接覆盖施术者 `qi_current`；`emit_qi_transfer` 仅把 `QiTransfer` 放入 `Events<QiTransfer>`，没有 `WorldQiAccount::transfer` 或患者 `credit_patient_qi`。
- 同文件 `apply_meridian_repair` 成功分支（约 `690-716`）明确执行 `credit_patient_qi`，失败分支还调用 `release_failed_repair_qi_to_zone`；这说明 ContamPurge 的“只 emit 不提交”不是有意的统一语义。
- 旧 Issue 中的 `apply_emergency_resuscitate`/`apply_life_extension` 已由主线提交 `da1b79673`（PR #2207）随濒死阶段删除，当前 registry 只保留三招。它们不再纳入本 skeleton；本 skeleton 的唯一仍可达根因是 `apply_contam_purge`。

## §4 非重复比对

- `docs/finished_plans/plan-qi-conservation-leaks-v1.md` 已覆盖若干战斗技能的 zone credit tick，但没有为 yidao `Healing` transfer 提供余额 consumer。
- 现有 `plan-bughunt-yidao-healing-cap-leak-v1.md`（来源 #1648/#1504/#1343）处理患者超 `qi_max` 截断与群体预扣份额，不处理 ContamPurge 的全额去向。
- `plan-bughunt-yidao-treatment-qi-destination-v1` 不重新打开 #1487/#1569：两条旧技能由 PR #2207 删除，若未来复活则应另立恢复后的具体 plan。

## §5 修复计划骨架

### P0：治疗真元结算

- 选定明确的治疗合同：成功净化的 qi 是患者可吸收的 credit，超出患者 `qi_max` 的部分通过 `qi_release_to_zone`/overflow 回灌；或在设计确认后将全部成本释放到患者所在 zone，但必须有唯一物理去向。
- 让余额变更与 `QiTransfer` 审计原子提交：施术者是 ECS `Cultivation.qi_current`，若目的地是 ledger 账户则先用 `transfer_external_qi_to_ledger` 验证/记账，成功后才扣施术者；若患者也是 ECS，则在同一原子服务内直接 credit 患者并追加审计；只有两端都是 ledger 账户时才调用 `ledger.transfer(QiTransfer { from, to, amount, reason: QiTransferReason::Healing })`，禁止把 `Events<QiTransfer>` 当作自动 consumer；拒绝/污染为零的路径不扣真元。

### P1：回归契约

- 成功、患者接近上限、无污染、真元不足四类覆盖 `Cultivation`、zone/overflow 与 transfer reason。
- 患者/zone 缺资源时按既有 overflow/fail-closed 规则处理，不重复扣除；`YidaoEventV1.qi_transferred` 与实际落点一致。

## §6 验证计划

实现后运行 server 栈 fmt、clippy、cargo test，重点覆盖 `combat::yidao` 和 qi ledger。守恒断言使用生产常量与 `assert_conservation`；本 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Inputs：** `resolve_contam_purge_skill`/`complete_yidao_casts` 输入 caster/patient `Cultivation`、`Contamination`、`calc.qi_cost`、位置维度和 zone。
- **Outputs：** `apply_contam_purge` 成功时施术者真实扣减、患者或其 zone/overflow 得到同额去向；失败不改污染或 qi。
- **共享类型/事件：** `YidaoSkillId::ContamPurge`、`YidaoEventV1`、`Cultivation`、`Contamination`、`QiTransfer`、`QiTransferReason::Healing`；server 符号为 `apply_contam_purge`、`debit_caster_qi`、`credit_patient_qi`、`emit_qi_transfer`。
- **三端契约符号：** Server 负责外部 ECS 余额事务；Agent：无变更，理由是 `YidaoEventV1` 字段不变；Client：无变更，理由是治疗动画和事件 payload 不增字段。
- **Qi：** 在线 caster 不是 ledger player balance；目的地是 ledger 时调用 `qi_physics::ledger::transfer_external_qi_to_ledger(&mut ledger, from, to, amount, QiTransferReason::Healing)`，目的地是患者 ECS 时由同一原子服务扣/加两个 `Cultivation` 并追加审计，成功后才提交 caster 扣减。zone/overflow 用 `qi_release_to_zone`、`ReleaseToZone`、`QI_ZONE_UNIT_CAPACITY`；断言调用 `qi_physics::ledger::assert_conservation`、`QI_EPSILON` 与 `crate::schema::common::SPIRIT_QI_TOTAL`。
- **worldview 锚点：** `docs/worldview.md` §二、§十的治疗真元转移和零和约束；事件不能代替患者余额。
