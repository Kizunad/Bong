# plan-combat-qi-invest-conservation-v1：战斗 qi_invest 守恒修复

一句话主题：修复 `server/src/combat/resolve.rs` 普通战斗的 `qi_invest` 从攻击者活体真元扣除后，必须经 `qi_physics::qi_release_to_zone` 归还当前 zone（zone 饱和时进入既有 `qi_flow_overflow` 稳定账本），不把 `Meridian::throughput_current` 统计量误当真元终点。

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

### 落地清单

- `server/src/combat/resolve.rs::resolve_attack_intents`：普通非 prepaid `qi_invest` 在攻击合法性校验完成后扣除，并经既有 `release_external_qi_to_zone` → `qi_release_to_zone` 释放；zone 饱和余量进入 `qi_flow_overflow`。
- `server/src/combat/resolve_tests.rs`：覆盖命中释放、未命中、超距/跨维/无效目标/真元不足零 mutation、prepaid 与 `NpcMelee` 不重复结算、zone 饱和 overflow，以及 `SPIRIT_QI_TOTAL` + `assert_conservation` 守恒锚点。
- `docs/finished_plans/plan-combat-qi-invest-conservation-v1.md`：本归档文件。

### 关键 commit

- `e58994f4e`（2026-09-07）：建立 bugfix plan skeleton。
- `ce4f76152`（2026-09-07）：将 plan 晋级为 active。
- `9051bfbc3`（2026-09-07）：接入 canonical qi release 并补齐守恒/拒绝路径回归。
- `a7913c3f8`（2026-09-07）：修正既有测试对合法攻击者 `qi_invest` 释放的断言。
- `6b5ce511f`（2026-09-07）：合入最新 `origin/main`（`0a7ece3a6`）后的 merge commit。

### 测试结果

- 定向回归：`cargo test --lib combat::resolve::tests::jiemai_parry_no_qi_transfer_when_insufficient_qi`（1 passed）；`cargo test --lib combat::resolve::tests::dead_armor_block_is_drop_not_release`（1 passed）。
- 合并前与合并主线后的 locked server gate 均真实退出 0：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、全量 `cargo test`。
- 全量 server 结果：lib `12178 passed; 0 failed; 1 ignored`，main `18 passed`，各独立 integration/unit targets 通过，doc-tests `3 passed; 0 failed; 5 ignored`。
- validator：无上下文只读 validator 对 `9051bfbc3` 返回 PASS 并已关闭；之后仅修正测试断言，主线 merge 未触及 `server/src/combat/resolve.rs`，按流程未重复启动 validator。

### 跨仓库核验

- server ↔ `qi_physics`：生产路径使用既有 `release_external_qi_to_zone`、规范 `qi_release_to_zone`、`QiTransferReason::ReleaseToZone` 与 `qi_flow_overflow`，测试使用 `SPIRIT_QI_TOTAL` 和 `ledger::assert_conservation`；未新增物理常数或第二套 ledger。
- agent/client/schema：未修改 agent、client、schema、Redis 或 wire 协议，故无跨仓库契约变更；`Meridian::throughput_current` 仍只是统计量。

### 遗留 / 后续

- PR 创建后的 CI/e2e 与当前 HEAD 的 Kody 审查属于 PR 流程后续门禁；本任务不自行 merge。
- prepaid 攻击源与 `AttackSource::NpcMelee` 的既有权威扣费边界保持不变；其他 qi 物理缺陷不在本 plan 范围。
