# plan-combat-qi-invest-conservation-v1：战斗 qi_invest 守恒修复

一句话主题：修复 `server/src/combat/resolve.rs` 普通战斗的 `qi_invest` 从攻击者活体真元扣除后，必须经 `qi_physics::qi_release_to_zone` 归还当前 zone（zone 饱和时进入既有 `qi_flow_overflow` 稳定账本），不把 `Meridian::throughput_current` 统计量误当真元终点。

## 阶段总览

- P0 ⏳（2026-09-07）：第一性核验 qi_invest 的输入、扣除、经脉统计与 canonical ledger 出料路径。
- P1 ⬜：在合法性校验完成后接入既有 `qi_physics` 释放事务，保持伤害/命中/倍率语义不变。
- P2 ⬜：补齐命中、未命中、超距/跨维拒绝、真元不足、prepaid 源与守恒回归。
- P3 ⬜：完成无上下文只读对抗验证、locked server gate、主线合并复验与 CI/Kody 审查。
- P4 ⬜：补 Finish Evidence，独立归档本 plan 并开 PR；不在本任务内合并 PR。

## P0：语义与接入面核验

- 入口：`server/src/combat/resolve.rs::resolve_attack_intents`，`AttackIntent.qi_invest` 由命中结算消费。
- 进料：非负 `qi_invest` 从攻击者 `Cultivation.qi_current` 取得；`QiInvestExceeded`、目标生命周期、反作弊与 raycast 合法性失败时不得改变任何 qi/world owner。
- 统计：`Meridian::throughput_current` 只供 `server/src/cultivation/overload.rs` 当前 tick 检测后清零，不能作为物理真元接收账户。
- 出料：普通非 prepaid 攻击使用 `release_external_qi_to_zone`，由其调用 `qi_release_to_zone` 并把 zone 饱和余量写入 `qi_flow_overflow`；`QiTransferReason::ReleaseToZone` 是唯一真实流转审计。
- 例外：`source_uses_prepaid_qi` 的 cast 已扣源不得重复扣/放；`AttackSource::NpcMelee` 是服务端权威 NPC 攻击，不纳入玩家 qi 账本。
- 规范锚点：`docs/CLAUDE.md` 真元守恒律；`docs/worldview.md` 真元总量恒定与释放/吸收路径约束。

## P1：最小生产修复

- 仅调整 `server/src/combat/resolve.rs` 的普通非 prepaid qi 结算。
- 扣费/释放位于 raycast 命中及其它攻击合法性校验之后；任意超距、跨维、反作弊、目标无效或真元不足拒绝路径零 qi mutation。
- 复用现有 `qi_physics` 常量、账户类型与失败原子性；不新增物理常数，不直写 zone/ledger，不改伤害公式、部位倍率、命中判定、暴击或护甲减免。

## P2：饱和回归

- 命中：攻击者 `qi_current` 减少请求量，当前 zone 或既有 overflow 增加同量，产生 `QiTransfer(reason=ReleaseToZone)`。
- 未命中：按现有 raycast 语义不发生扣费或释放，并锁定该边界。
- 超距/跨维/无效目标、反作弊拒绝：攻击者 qi、zone、ledger、transfer audit 均保持不变。
- `QiInvestExceeded`：不足时拒绝且零 qi mutation。
- prepaid 与 `NpcMelee`：仍不由 resolver 重复结算。
- 使用 `qi_physics::constants::SPIRIT_QI_TOTAL`（或仓库当前同一 canonical 常量导出）作为预算锚点，并使用 `qi_physics::ledger::assert_conservation`；测试不写总量字面量。

## P3：验证门

- 以新 HEAD 启动无上下文、read-only、绑定 SHA 的 validator，结论必须携带该 SHA。
- 通过后在 `/tmp/bong-cargo.lock` 下完成 server fmt、clippy `-D warnings`、完整 cargo test。
- 紧邻 `git fetch origin && git merge origin/main`；若合并影响修复路径，重新 validator 与完整 server gate。
- push 任务分支、创建 PR，等待当前 HEAD 的 Kody 主动无 findings 结论、全部 CI/e2e，并检查 review/inline discussion。

## P4：归档交付

- 全部阶段完成后补 `## Finish Evidence`：真实文件、关键 commit、测试命令/结果、跨仓库核验与后续事项。
- 独立中文归档 commit 将 active plan 移入 `docs/finished_plans/`；PR 开出后回报调度会话，不自行 merge。

## Finish Evidence

待 P0–P4 完成后填写。
