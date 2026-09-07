# plan-combat-qi-invest-conservation-v1：战斗 qi_invest 守恒修复

一句话主题：修复 `server/src/combat/resolve.rs` 普通战斗的 `qi_invest` 从攻击者活体真元扣除后，必须经 `qi_physics::qi_release_to_zone` 归还当前 zone（zone 饱和时进入既有 `qi_flow_overflow` 稳定账本），不把 `Meridian::throughput_current` 统计量误当真元终点。

## 接入面契约

- **范围与层**：这是纯 server 侧 combat/qi 守恒修复，落点为 `server/src/combat/resolve.rs` 与 `server/src/combat/resolve_tests.rs`；agent、client、schema、Redis key、CustomPayload/wire 契约均不变。
- **进料**：`resolve_attack_intents` 消费既有 `AttackIntent`（含 `attacker`、`target`、`qi_invest`、`source`、`reach`）；从攻击者 `Cultivation.qi_current` 取真实支出，复用 `CurrentDimension`、`Position`、`ZoneRegistry` 定位唯一归还 zone，并以 target lifecycle/game mode/完整组件查询/raycast 作为合法性前置。
- **出料**：普通非 prepaid `qi_invest` 经 `release_external_qi_to_zone` 调用 `qi_release_to_zone`，按 zone accepted/overflow 产生 `QiTransferReason::ReleaseToZone`；zone 饱和或缺失进入既有 `qi_flow_overflow_account`，`Meridian::throughput_current` 只接收统计量，不是物理终点。
- **共享类型与 API**：复用 `AttackIntent`、`AttackSource`、`Cultivation`、`CurrentDimension`、`ZoneRegistry`、`WorldQiAccount`、`QiTransfer`、`QiFlowError`；复用 `qi_physics::qi_release_to_zone`、`qi_physics::ledger::assert_conservation` 与 `schema::common::SPIRIT_QI_TOTAL`，不新增 seam、事件、物理常数或第二套 ledger。
- **worldview 锚点**：`worldview.md §二 L157-L169`（真元离体/战斗消耗）与 `worldview.md §十 L874-L883`（灵气零和、总量恒定/衰减）；工程硬约束为 `docs/CLAUDE.md §二` 接入面清单及 §四 真元守恒律。
- **行为边界**：`target_has_complete_query` 是命中后、扣真元前的最后完整目标组件合法性校验，属于“先验证合法性再扣费”的本 plan 范围；跨维拒绝则是本 PR 明确登记的 gameplay 行为变更，动机与历史行为见 Finish Evidence。

## 阶段总览

- P0 ✅ 2026-09-07：第一性核验 qi_invest 的输入、扣除、经脉统计与 canonical ledger 出料路径。
- P1 ✅ 2026-09-07：在合法性校验完成后接入既有 `qi_physics` 释放事务，保持伤害/命中/倍率语义不变。
- P2 ✅ 2026-09-07：补齐命中、未命中、超距/跨维拒绝、真元不足、prepaid 源与守恒回归。
- P3 ✅ 2026-09-07：完成无上下文只读对抗验证、locked server gate 与主线合并复验。
- P4 ✅ 2026-09-07：补 Finish Evidence，独立归档本 plan 并准备开 PR；不在本任务内合并 PR。

## P0：语义与接入面核验

- 入口：`server/src/combat/resolve.rs::resolve_attack_intents`，`AttackIntent.qi_invest` 由命中结算消费。
- 进料：非负 `qi_invest` 从攻击者 `Cultivation.qi_current` 取得；`QiInvestExceeded`、目标生命周期、反作弊与 raycast 合法性失败时不得改变任何 qi/world owner。
- 统计：`Meridian::throughput_current` 只供 `server/src/cultivation/overload.rs` 当前 tick 检测后清零，不能作为物理真元接收账户。
- 出料：普通非 prepaid 攻击使用 `release_external_qi_to_zone`，由其调用 `qi_release_to_zone` 并把 zone 饱和余量写入 `qi_flow_overflow`；`QiTransferReason::ReleaseToZone` 是唯一真实流转审计。
- 例外：`source_uses_prepaid_qi` 的 cast 已扣源不得重复扣/放；`AttackSource::NpcMelee` 是服务端权威 NPC 攻击，不纳入玩家 qi 账本。
- 规范锚点：`docs/CLAUDE.md` 真元守恒律；`docs/worldview.md` 真元总量恒定与释放/吸收路径约束。

## P1：最小生产修复

- 仅调整 `server/src/combat/resolve.rs` 的普通非 prepaid qi 结算。
- 扣费/释放位于 raycast 命中及其它攻击合法性校验之后；任意超距、反作弊、目标无效或真元不足拒绝路径零 qi mutation。跨维门及其动机、历史行为和回归证据见本 plan 末尾的显式行为变更登记。
- 复用现有 `qi_physics` 常量、账户类型与失败原子性；不新增物理常数，不直写 zone/ledger，不改伤害公式、部位倍率、命中判定、暴击或护甲减免。

## P2：饱和回归

- 命中：攻击者 `qi_current` 减少请求量，当前 zone 或既有 overflow 增加同量，产生 `QiTransfer(reason=ReleaseToZone)`。
- 未命中：按现有 raycast 语义不发生扣费或释放，并锁定该边界。
- 超距/跨维/无效目标、反作弊拒绝：攻击者 qi、zone、ledger、transfer audit 均保持不变。
- `QiInvestExceeded`：不足时拒绝且零 qi mutation。
- prepaid 与 `NpcMelee`：仍不由 resolver 重复结算。
- 释放事务返回 `UnrepresentableFlow` 时，释放按 no-op 处理但攻击继续结算；source、zone、ledger 与 audit 均不变，避免把不可表示的微量当作可扣除真元或凭空铸造 overflow。
- 释放事务返回其它错误时保持 fail-closed：事务先失败且不改 source/zone/ledger/audit，resolver 拒绝整次攻击，不产生伤害或伤口；由无效 zone 回归锁定该分支。
- 使用 `schema::common::SPIRIT_QI_TOTAL` 作为本测试 app 的预算锚点，并使用 `qi_physics::ledger::assert_conservation`；测试不写总量字面量。

## P3：验证门

- 以新 HEAD 启动无上下文、read-only、绑定 SHA 的 validator，结论必须携带该 SHA。
- 通过后在 `/tmp/bong-cargo.lock` 下完成 server fmt、clippy `-D warnings`、完整 cargo test。
- 紧邻 `git fetch origin && git merge origin/main`；若合并影响修复路径，重新 validator 与完整 server gate。
- push 任务分支、创建 PR，等待当前 HEAD 的 Kody 主动无 findings 结论、全部 CI/e2e，并检查 review/inline discussion。

## P4：归档交付

- 全部阶段完成后补 `## Finish Evidence`：真实文件、关键 commit、测试命令/结果、跨仓库核验与后续事项。
- 独立中文归档 commit 将 active plan 移入 `docs/finished_plans/`；PR 开出后回报调度会话，不自行 merge。

## Finish Evidence

### 落地清单

- `server/src/combat/resolve.rs::resolve_attack_intents`：普通非 prepaid `qi_invest` 在攻击合法性校验完成后扣除，并经既有 `release_external_qi_to_zone` → `qi_release_to_zone` 释放；zone 饱和余量进入 `qi_flow_overflow`。
- `server/src/combat/resolve_tests.rs`：覆盖命中释放、未命中、超距/跨维/无效目标/真元不足零 mutation、prepaid 与 `NpcMelee` 不重复结算、zone 饱和 overflow、`UnrepresentableFlow` no-op 继续攻击、其它释放错误 fail-closed，以及 `SPIRIT_QI_TOTAL` + `assert_conservation` 守恒锚点。
- `docs/finished_plans/plan-combat-qi-invest-conservation-v1.md`：本归档文件。

### 关键 commit

- `e58994f4e`（2026-09-07）：建立 bugfix plan skeleton。
- `ce4f76152`（2026-09-07）：将 plan 晋级为 active。
- `9051bfbc3`（2026-09-07）：接入 canonical qi release 并补齐守恒/拒绝路径回归。
- `a7913c3f8`（2026-09-07）：修正既有测试对合法攻击者 `qi_invest` 释放的断言。
- `2e53f4192`（2026-09-07）：补齐 `UnrepresentableFlow` no-op 与其它释放错误 fail-closed 回归，接入 `SPIRIT_QI_TOTAL` 权威快照及计划契约说明。
- `6b5ce511f`（2026-09-07）：合入最新 `origin/main`（`0a7ece3a6`）后的 merge commit。

### 测试结果

- 定向回归：`cargo test --lib combat::resolve::tests::jiemai_parry_no_qi_transfer_when_insufficient_qi`（1 passed）；`cargo test --lib combat::resolve::tests::dead_armor_block_is_drop_not_release`（1 passed）；`cargo test --lib combat::resolve::tests::qi_invest_unrepresentable_release_is_noop_but_attack_resolves`（1 passed）；`cargo test --lib combat::resolve::tests::qi_invest_release_error_fails_closed_without_resolving_attack`（1 passed）。
- 合并前与合并主线后的 locked server gate 均真实退出 0：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、全量 `cargo test`。
- 全量 server 结果：lib `12178 passed; 0 failed; 1 ignored`，main `18 passed`，各独立 integration/unit targets 通过，doc-tests `3 passed; 0 failed; 5 ignored`。
- validator：无上下文只读 validator 对 `9051bfbc3` 返回 PASS 并已关闭；本轮 review 后的 `2e53f4192` 通过两个新增定向回归及完整 locked server gate，按当前会话“validator 已 Closed、不要再查询/重开”的指示未重复启动 validator。

### 跨仓库核验

- server ↔ `qi_physics`：生产路径使用既有 `release_external_qi_to_zone`、规范 `qi_release_to_zone`、`QiTransferReason::ReleaseToZone` 与 `qi_flow_overflow`，测试使用 `SPIRIT_QI_TOTAL` 和 `ledger::assert_conservation`；未新增物理常数或第二套 ledger。
- agent/client/schema：未修改 agent、client、schema、Redis 或 wire 协议，故无跨仓库契约变更；`Meridian::throughput_current` 仍只是统计量。

### 遗留 / 后续

- PR 创建后的 CI/e2e 与当前 HEAD 的 Kody 审查属于 PR 流程后续门禁；本任务不自行 merge。
- prepaid 攻击源与 `AttackSource::NpcMelee` 的既有权威扣费边界保持不变；其他 qi 物理缺陷不在本 plan 范围。

### 返工补充：错误路径与跨维行为边界

- `QiFlowError::UnrepresentableFlow` 发生在释放事务的 source/zone 可表示性预检阶段，事务保证尚未修改 `qi_current`、zone 或 ledger。对这种低于当前 `f64` 表示精度的释放，本 PR 选择 no-op release 并继续攻击：不写 overflow，避免 source 未扣减时凭空铸造 ledger credit；其它释放错误仍 fail-closed 并拒绝整次攻击。回归测试为 `qi_invest_unrepresentable_release_is_noop_but_attack_resolves`；无效 zone 触发的通用错误分支由 `qi_invest_release_error_fails_closed_without_resolving_attack` 覆盖。
- `target_has_complete_query` 是 raycast 命中之后、普通 qi release 之前的最后一道完整目标组件合法性校验；它确保不完整/异常目标不会先扣真元，属于本 plan 的“先验证合法性再扣费”正当范围，不是额外 gameplay 平衡调整。
- 跨维拒绝是本 PR 明确引入的 gameplay 行为变更，不是单纯账本修复：此前 `resolve_attack_intents` 没有 attacker/target `CurrentDimension` 相等性门，几何命中即可继续结算；现在在任何 qi/world mutation 前拒绝跨维攻击。动机是 zone 归属必须绑定单一维度，避免跨维攻击无法定义真元归还目标，同时满足拒绝路径零 mutation；回归测试为 `qi_invest_cross_dimension_rejection_does_not_mutate_qi`。
