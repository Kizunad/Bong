# plan-bughunt-distance-decay-calibration-v1（骨架）

> 一句话主题：按 worldview 与暗器 plan 的双锚点重校统一的**无量纲远程效果保留系数**，使普通攻击 10 格保留 40% 效果、异变兽骨+凝实色 50 格约保留 80% 效果；该系数绝不表示或改变真实 qi 余额。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 冻结无量纲 coefficient、退役 amount API、全调用方量纲/ledger 矩阵 | ⬜ |
| P1 | R5-owned coefficient 类型/helper + focused gameplay consumer 迁移 | ⬜ |
| P2 | 双锚点/单调性/非法输入 + 账户零变化/static misuse pins | ⬜ |
| P3 | bot 远程伤害/污染效果校准回归 | ⬜ |

## 接入面

- **进料**：`server/src/qi_physics/distance.rs:1-27` 当前 amount-shaped `qi_distance_atten`、`MediumKind` color/carrier loss、`server/src/combat/decay.rs:8-24` 的 `hit_qi_ratio`、`server/src/combat/carrier.rs:1144-1145,1260-1324` 与 `server/src/qi_physics/collision.rs:70-147`。
- **出料**：只输出 `[0,1]` 的 `DistanceEffectCoefficient`，供 wound/health damage 与纯展示命中反馈等**不会进入任何 qi/污染账户生命周期**的 gameplay 效果量相乘；不输出“到达真元”、不计算账户余量或转账金额。现有 `ContamSource.amount` 会在 `contamination_tick` 中决定 `release_qi_amount_to_zone` 与玩家 qi 扣减，因此明确属于禁用 consumer，不得乘 coefficient。
- **共享类型 / event**：复用 `MediumKind`、`CarrierGrade`、`ColorKind`；R5 提供 `DistanceEffectCoefficient` 与 canonical helper；禁止另写 combat 私有 decay。
- **跨仓库契约**：无 payload 形状变化；玩家可感知效果数值必须补 bot/playtest。
- **worldview 锚点**：`worldview.md §四 L332-L340`（0 格 100%、普通 10 格 40%）与 §五 L405-L413（异变兽骨 50 格约 80%）；本 plan 把百分比明确解释为远程攻击的**效果保留率**，不是 qi account 的余额保留率；`docs/finished_plans/plan-anqi-v1.md` Q41 同锚。
- **qi_physics 锚点**：R5 是 `server/src/qi_physics/**` 与真实 qi 账户流动的唯一 owner；focused plan 只拥有校准规格、combat effect integration 与 bot 验收。

## 当前证据（origin/main @ c625d5a5）

- `server/src/qi_physics/constants.rs:3-4` 仍是 `QI_DECAY_PER_BLOCK = 0.03`；`server/src/qi_physics/distance.rs:1-27` 的 `qi_distance_atten(initial, ...)` 接受 amount 并返回 amount，命名和签名都没有冻结“效果系数”边界。
- `server/src/combat/decay.rs:8-24` 把 `CarrierGrade::Beast` 映射为 `MediumKind::SpiritWeapon`；`:40-79` 的 regression test 锁 Mellow+BareQi@10 约 `0.737` 与 Solid+AncientRelic@50 约 `0.494`，后者并非 Beast 锚点。按当前 bonus，Solid+SpiritWeapon@50 约 `0.364`。
- `server/src/combat/carrier.rs:1144-1145,1260-1324` 把 ratio 乘入名为 `qi_payload`/`hit_qi` 的字段，`server/src/qi_physics/collision.rs:70-147` 又以 attenuated amount 派生 transfer；这正是 P0 必须消除的量纲歧义，当前路径不能作为“纯系数已安全”的证据。
- `server/src/combat/needle.rs:95-223,292-392` 当前不调用相关 helper；needle 若未来接入属于新增 gameplay integration，不得伪称既有调用方迁移。

## P0 冻结契约：只允许无量纲效果系数

1. **唯一语义**：距离函数只表达“同一攻击在该距离/介质下保留多少 gameplay 效果”，量纲为 `1`。R5 必须以 ratio-only API 取代 amount API，例如 `distance_effect_coefficient(distance_blocks, medium, env) -> DistanceEffectCoefficient`；参数不得包含 initial qi amount、`QiAccountId`、`WorldQiAccount`、ledger 或 transfer writer，返回值必须有限且钳在 `[0,1]`。
2. **旧 API 必须退役**：`qi_distance_atten(initial, ...)`、`qi_distance_atten_in_env(initial, ...)` 以及“arrived/lost/evaporated qi”式返回语义不得与新 helper 并存。调用方变量/结构字段若只是效果基数，必须改成 `effect_basis` / `effect_retained` 等非余额命名；不能继续让 `initial - arrived` 看起来像未入账的真实 qi。
3. **允许/禁止的 consumer**：coefficient 只能乘入 wound/health damage 与不参与任何资源结算的视觉/命中反馈。它不得用于污染账本或其输入（包括 `ContamSource.amount`、污染排异量、污染导致的 qi 扣减）、`QiTransfer::amount`、`WorldQiAccount::{set_balance,transfer,remove_balance}`、projectile/residual qi 余额、source debit、target credit、zone/overflow release 或任何持久化 qi/污染字段。若需要距离改变“污染表现”，必须另用不进入 `Contamination.entries`/排异公式的纯展示字段；本 plan 不新建该字段。
4. **真实 qi 独立结算**：发射扣费、projectile source、命中/落空、zone/overflow 释放等若承载真实 qi，必须继续由现有 ledger 或 R5 冻结的账户 API 以**未乘 distance coefficient 的实际余额**完整结算。coefficient 的变化不得改变 source/target/zone/overflow 任一账户 delta，也不得改变 `QiTransferReason`/audit 数量。
5. **当前 amount caller 是阻塞项**：`carrier`/`collision` 中任何把 coefficient 结果当成余额、residual、transfer amount 的路径，必须先由 R5 分离“真实账户腿”与“效果腿”才可进入 P1。若产品决定距离确实要减少真实 qi 余额，则本路线立即失效：必须先改由 R5 另立 source→明确 target/zone/overflow、reason、audit、失败原子性及逐腿守恒契约；禁止 focused plan 临时恢复 amount API。
6. **双锚点反解**：只反解 coefficient 的 base/color/carrier 三部分：`Mellow + BareQi @10 ≈ 0.40`、`Solid + SpiritWeapon @50 ≈ 0.80`（`SpiritWeapon` 是 `CarrierGrade::Beast` 当前映射）。若现有指数模型不能同时满足，P0 明确改 coefficient 模型并列全 effect caller 影响，不得改 ledger 公式。

## 可机械验收

1. **API/量纲 pin**：编译测试只允许 ratio-only helper；其签名无 initial amount/account/ledger 参数，返回强类型 `DistanceEffectCoefficient`，不存在可调用的 amount-shaped `qi_distance_atten*`。
2. **数值 pin**：0 格=1.0；Mellow+BareQi@10≈0.40；Solid+SpiritWeapon@50≈0.80；距离单调不增、高级载体不劣于普通载体；NaN/负值按 P0 冻结契约处理。
3. **账户/污染全生命周期零变化 runtime pin**：在同一 fixture 记录 source、target、zone、overflow、玩家 `qi_current`、`Contamination.entries`/排异结果与 audit 快照，分别以 0/10/50 格 coefficient 运行完整命中→污染 tick→排异结算；断言距离变化只改变 wound/health/纯展示 feedback，所有 qi account delta、transfer legs、reason/audit 数量、污染 amount 与后续排异 qi delta 逐项完全相同。
4. **misuse static gate**：扫描所有 `DistanceEffectCoefficient` consumer，若其值流入 `QiTransfer::new`/ledger mutation/projectile qi balance/residual/release amount、`ContamSource.amount`/污染排异输入或任何会改变 qi/污染资源生命周期的字段则测试必红；所有生产 caller 必须登记为 effect-only 或由 R5 ledger owner 显式豁免并给出账户 API。
5. `combat::decay`/carrier 与 `qi_physics::collision` 只有在完成 effect/account 分腿后才能消费 canonical helper；当前未接入的 needle 不计为迁移完成。
6. R5 coefficient 与量纲 pins 合入后，focused plan 跑完整 server gate，并以 bot 场景比较近距/10 格/50 格的真实伤害/污染效果；bot 不得用账户余额变化来证明 coefficient。

## 边界

- 不改真实 qi 总量、账户归属、污染资源量/排异 qi 代价、技能基础扣费、瞄准、弹道速度或射程；只校准 wound/health damage 与纯展示 feedback 等非账户 gameplay effect。
- 本 plan 不直接修改 `server/src/qi_physics/**`；P0 可先完成反解/验收冻结，implementation 等 R5 coefficient API 合入后再消费，禁止与 R5 并行写文件。
- 不把候选但非 `origin/main` 祖先的历史提交当作已修证据。
