# BugHunt: 医道修复预扣真元未按实际落点结算

状态：Skeleton，仅记录候选 bug 与修复计划；本轮不消费、不归档、不改代码。

## Bug 摘要

医道接经术与群体接经在成功路径中先从医者 `Cultivation.qi_current` 预扣请求量，再调用 `credit_patient_qi` 给患者补真元。`credit_patient_qi` 会用患者 `qi_max` 截断实际接收量，但调用方仍按请求量写 `YidaoApplyOutcome.qi_transferred` 和 `QiTransferReason::Healing`，没有把截断差额回灌 zone 或 overflow。

同一类问题在群体接经中还有第二个入口：cast 开始时收集的患者在完成时仍在距离内但已经不可修复、缺少 `MeridianSeveredPermanent`，或只剩部分患者可修复时，`apply_mass_meridian_repair` 对该预分配份额直接 `continue`，没有成功转给患者，也没有回灌。

## 对实际游玩体验的影响

玩家作为医者救人时，会看到技能成功、经脉修复成功、事件显示转入了完整真元，但如果患者本来接近满真元，医者实际消耗的大量真元会无声蒸发。化虚群体接经尤其明显：一次群体手术会预扣整条真元池，战斗中患者状态稍有变化或患者接近满真元，就会让玩家付出完整代价却只得到少量恢复效果，破坏医道“医者付出，患者获得”的支援流派手感。

## 证据定位

- 守恒红线：`docs/CLAUDE.md:59` 要求所有真元/灵气流动走 `QiTransfer`，不能只扣一端或吞掉差额。
- 医道正典：`docs/finished_plans/plan-yidao-v1.md:41` 写明治疗真元守恒律为“医者付出，患者获得”；`docs/finished_plans/plan-yidao-v1.md:120-122` 再次规定真元从医者付出到患者获得；`docs/finished_plans/plan-yidao-v1.md:136` 与 `:146` 写接经术成功为 medic 真元转入 patient。
- 单体接经术：`server/src/combat/yidao.rs:789-814` 先 `debit_caster_qi` 扣 `calc.qi_cost`，成功后 `credit_patient_qi(..., calc.qi_cost)`，并按同一 `calc.qi_cost` emit Healing。
- 群体接经：`server/src/combat/yidao.rs:992-1026` 预扣 `cultivation.qi_max`，按 `qi_per_patient` 给每个成功患者补 qi 并 emit Healing；`server/src/combat/yidao.rs:1003-1008` 对不可修复/缺组件患者直接 `continue`。
- 截断点：`server/src/combat/yidao.rs:1251-1254` 的 `credit_patient_qi` 只做 `(qi_current + amount).min(qi_max)`，不返回实际接收量，也不处理 overflow。
- Healing 无补账消费者：`server/src/combat/yidao.rs:1274-1289` 是全仓唯一 `QiTransferReason::Healing` 发送点；`server/src/qi_physics/ledger.rs:641-672` 的守恒快照直接从 ECS `Cultivation.qi_current` 汇总真实余额。
- near-cap 可达：`server/src/cultivation/meridian/severed.rs:89-97` 写断经记录不碰 `qi_current`；`server/src/cultivation/meridian/severed.rs:318-331` 事件消费只写断经组件和经脉状态。

## 触发路径

最小单体反例：

1. 医者 `qi_current=300, qi_max=300`，接经术 mastery 100。
2. `qi_physics::healing::meridian_repair` 在 `server/src/qi_physics/healing.rs:67-75` 给出 `qi_cost = 300 * 0.5 = 150`。
3. 患者有一条可修 SEVERED，经脉可修，但 `qi_current=99, qi_max=100`。
4. 成功路径扣医者 150；患者实际只能从 99 到 100，接收 1；zone/overflow 无入账；净少 149。

群体反例：

1. 化虚医者群体接经开始时锁定 3 个患者，预扣 300，`qi_per_patient=100`。
2. 完成时其中 1 个患者已被其他医者修好或缺少可修组件，但仍在距离内。
3. 循环在该患者上 `continue`，对应 100 既未转入患者，也未通过失败路径回灌 zone。

## 反方审查记录

第一轮反方试图证明误报，检查了患者是否总是空真元、Healing 是否有消费者、现有测试是否覆盖 near-cap、群体份额是否恒等于容量。结论：真实。最强反驳点仅是测试夹具默认患者 `qi_current=0`，三人群体修复时 `300 / 3 == 100`，刚好掩盖截断。

第二轮反方继续攻击“是否值得立 plan”。结论：可立 Skeleton Plan。反方确认没有文档支持成功治疗允许溢出损耗；断经状态不保证清空真元；群体路径还有 stale patient / 不可修复患者份额未结算的相邻问题，建议归入同一个“预扣后按实际落点结算”修复。

## Skeleton Fix Plan

- [ ] 将 `credit_patient_qi` 改为返回实际接收量与 overflow，例如 `requested / accepted / overflow`。
- [ ] 单体接经成功路径按 `accepted` 写 `qi_transferred` 与 `QiTransferReason::Healing`；`requested - accepted` 走医者所在地 `release_failed_repair_qi_to_zone` 或等价 helper 回灌 zone/overflow。
- [ ] 群体接经成功路径逐患者按实际接收量记账；near-cap 差额回灌。
- [ ] 群体接经对 `first_repairable_meridian` 缺失、缺 `MeridianSeveredPermanent`、完成时不再可修复的患者份额回灌，而不是 `continue` 静默跳过。
- [ ] `YidaoApplyOutcome.qi_transferred` 改为“患者实际接收量”，HUD/event 不再显示请求量。

## 验收测试计划

- [ ] 单体接经 near-cap：患者 `qi_current=99, qi_max=100`，医者扣 150，断言患者只增 1，剩余 149 以 `ReleaseToZone` 或 overflow 入账，总量守恒。
- [ ] 单体接经 full patient：患者满真元但有 SEVERED，技能成功时请求量必须全额回灌，不得吞。
- [ ] 群体接经 near-cap：多个成功患者各自按实际容量接收，差额合计回灌。
- [ ] 群体接经 stale patient：cast 开始可修，完成前被修复或缺组件，预分配份额回灌。
- [ ] 保留现有失败路径测试：失败份额仍回灌，且不重复释放。

## 风险

- 医道事件/HUD 现有 `qi_transferred` 语义可能被客户端或 agent 当作请求量展示；改成实际接收量后需要检查 schema 消费方文案。
- 若选择把 near-cap 差额回灌医者脚下 zone，需确认无 `Position`/`ZoneRegistry` 时 overflow fallback 与失败路径一致。
- 群体接经的 `patients.len()` 分母和 `capacity` 关系要保持现有玩法节奏，修复不能让玩家通过混入 stale patient 降低每名成功患者成本。
