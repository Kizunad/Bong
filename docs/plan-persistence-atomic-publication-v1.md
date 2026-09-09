# plan-persistence-atomic-publication-v1 — persistence 归档原子发布与并发恢复加固

> 一句话主题：在 R3 P1 persistence 按域拆分之后，单独定义并实现归档文件发布、批次回滚与生命周期并发的可证明契约；本 plan 按阶段承接设计与实施边界，不宣称尚未完成的生产接入已经安全。
>
> 来源：#2180（`plan-refactor-persistence-slices-v1`）出口 D。当前基线 `origin/main=60f21e6ab36bd78fdfd2606dd766b09939d3fac5`；拆分后的生产落点是 `server/src/persistence/{helpers,npc,player,social,tribulation,void_actions,world,world_qi}.rs`，不再以旧的巨型 `persistence/mod.rs` 行号作为落点。

## 阶段总览

| 阶段 | 交付物 | 状态 | 验收日期 |
|---|---|---|---|
| P0 | 归档发布三条不变式、owner 模型与决策门定稿；每条都有可撞红的验证方式 | ⬜ | 待验收 |
| P1 | `helpers.rs:330` 临时清理失败可观察、两处生产不变式 `expect` 评估 | ✅ 2026-09-09 | 2026-09-09 |
| P2 | 归档身份证明、no-replace 发布与 CAS 批次回滚按 P0 契约落地 | ⬜ | 待验收 |
| P3 | `published_by_sweep` 发布后替换竞态收口，不误删 successor | ⬜ | 待验收 |
| P4 | 饱和回归、并发/失败矩阵、完整 persistence 与 server 验收证据 | ⬜ | 待验收 |

## 1. 立 plan 来历与边界

### 1.1 从 #2180 拆出的原因

#2180 的宪章是 R3 P1「按域拆分 persistence」，目标应当是机械搬迁与既有行为保持不变；但在该 PR 中混入了一整套原子发布并发设计。连续五轮 review 的新发现都落在这套 PR 新加的归档机械里，修复一个局部路径又暴露下一个并发/所有权路径，review 面因此没有收敛。最终以 `169a70872` 为纯搬迁边界，用 `git revert` 将 `7fb558e91` 起的加固提交整体翻回；#2180 已回到纯机械拆分并合入主线。

本 plan 专门承接被 revert 的原子发布加固与其未闭合问题，使「persistence 按域拆分」和「归档并发设计」各自拥有独立的契约、测试、review 与验收面。后来者不得把本 plan 的实现偷偷塞回机械拆分 PR，也不得把纯搬迁证据表述成原子发布设计已经安全。

### 1.2 硬边界

- pre-P0 决策阶段只写文档；进入 P1 后仅按阶段交付物修改 `server/src/persistence/**` 及其回归测试，不改迁移链、表结构、生产事务边界或其他 plan；P1 不归档本 plan。
- 实施阶段只允许在本 plan 明确的 persistence 归档/恢复范围内工作；不得借机修改 TypeBox、protobuf、proto conversion、client router、schema、wire、R7、dropped-loot、craft production 或 inventory receipt。
- 必须原样保留 R3 P0 已安装的生产接入点：canonical persistence registry、`AppExit → Last` dispatcher、zone-runtime shutdown descriptor，以及 KnownTechniques 的 reconnect、load guard、dirty snapshot、durable fence adapter。
- `world_qi.rs` 属于 persistence 切片，但任何真元恢复仍必须遵循 `qi_physics` 受控接口；本 plan 不新增真元物理公式、不创建第二套 ledger、不把 event/audit 当作余额状态消费者。
- 归档文件与 SQLite hot-row 状态的失败语义必须在 P0 决策门收口后实现；没有决议不得以局部 helper 或临时清理逻辑推进 P1 之后的代码。

## 2. 接入面 checklist（docs/CLAUDE.md §二）

### 2.1 进料

- `server/src/persistence/npc.rs`：NPC active/deceased 记录、digest stale rows、sweep 候选与现有保存入口。
- `server/src/persistence/helpers.rs`：归档 bundle 序列化/压缩、临时文件与最终路径操作的既有辅助边界；具体加固符号在当前主线尚不存在，实施时须先由 P0 决定其最小形态。
- SQLite hot rows、既有 `npc_deceased_index`/digest 记录、归档 payload，以及固定 `.npc.lifecycle.lock` 所代表的生产生命周期协调范围。
- `server/src/persistence/migrations.rs` 与 `world_qi.rs`：只作为既有 schema/恢复契约的输入与核验落点，不改迁移版本链或表结构。

### 2.2 出料

- NPC deceased archive 与 digest archive 文件：发布成功、冲突、回滚和 orphan 恢复的可观察文件状态。
- SQLite transaction：hot rows、deceased index 与 digest 状态的原子提交/回滚结果。
- 恢复与 sweep：崩溃后孤立归档的处理、过期行清理、批次失败的诊断信息。
- `combine_persistence_failure` 计划中的聚合错误输出：主错误和 ownership/rollback 清理错误必须同时可观测；它不是当前 `origin/main` 已存在的 API。

### 2.3 共享类型与事件

- 复用现有 persistence models/records、NPC identity、归档 payload 和 SQLite transaction 语义。
- 归档身份若经 P0 选择引入，`ArchiveFileIdentity` / `ensure_archive_identity` 只能作为本 plan 的拟议契约；当前 `origin/main` 不提供它们。
- 真元恢复如触及 `world_qi.rs`，只调用 `qi_physics::ledger` 的受控 durable-owner restore/validation 接口；不得伪造 `QiTransfer` 或绕过 ledger。

### 2.4 跨仓库契约

本 plan 是 server persistence 内部 plan，不新增或改变 server↔agent↔client 的 TypeBox、protobuf、CustomPayload、Redis key 或 wire shape；agent/client 没有本 plan 的生产 import。若实现需要跨仓库可观察契约，必须另立并明确 owner，不得隐式扩大本 plan。

### 2.5 worldview 锚点

- 归档保存的是末法残土中的生灵生平/死亡与持久化状态；不得用归档失败掩盖状态丢失或制造重复生灵。
- 涉及真元状态时遵循 `docs/worldview.md §二 L30-L50` 的灵压/正域、死域、负灵域语义与 `§十 L870-L880` 的零和重分配约束；持久化恢复是恢复既有 owner，不是生成真元。
- 新命名、资源语义和 gameplay 规则仍以 `docs/worldview.md` 为唯一正典，本 plan 不回写 worldview。

### 2.6 qi_physics 锚点

- 归档/恢复不得把携带真元的状态直接 `store.remove` 丢弃，也不得用裸 `qi_current`/`zone.spirit_qi` 加减补偿文件失败；离屏或终局释放必须走 `qi_physics::ledger`/受控 `release_dormant_qi_to_zone` 等既有入口。
- `world_qi.rs` 的恢复须验证固定 durable owner 集合、非法/缺失记录和守恒边界；不新增 `*_DECAY`、`*_DRAIN`、`*_ATTEN` 或任何本 plan 私有物理常数。
- 验收测试引用 `qi_physics` 的 canonical constants/API，不写死 `SPIRIT_QI_TOTAL` 的历史字面值；若发现既有入口不足，先停在依赖/决策记录，不在本 plan 偷建旁路 ledger。

#### 2.6.1 当前 `world_qi.rs` 恢复与验证调用链（基线核验）

以下是当前 `origin/main=e52a991fd` 已存在的真实符号与 owner 输入；实施时必须以这条链为边界，不能把抽象描述误当成 API，也不能再造一条旁路恢复链：

- `persistent_runtime_qi_accounts() -> [QiAccountId; 5]`（`server/src/qi_physics/ledger.rs`）：无运行时参数，返回必须完整持久化/恢复的固定 durable owner 白名单；每个 `QiAccountId` 是余额的 owner identity，不能由持久化行名动态扩展。
- `load_runtime_qi_account_balances(settings: &PersistenceSettings) -> io::Result<Vec<(QiAccountId, f64)>>`（`server/src/persistence/world_qi.rs`）：以 `&PersistenceSettings` 提供 SQLite 路径/连接上下文，按上述白名单读取 `qi_runtime_accounts`；当前已对每个 owner 检查行存在、余额 finite 且非负，缺行或非法值 fail-closed，返回带 owner 的已验证 `(QiAccountId, balance)` 集合。
- `hydrate_runtime_qi_accounts(settings: &PersistenceSettings, qi_ledger: &mut WorldQiAccount) -> io::Result<usize>`（`server/src/persistence/world_qi.rs`）：由 `bootstrap_persistence_system(settings: Res<PersistenceSettings>, mut qi_ledger: ResMut<WorldQiAccount>)` 在启动恢复时提供 `&PersistenceSettings` 与唯一可变 `&mut WorldQiAccount` owner；当前实现逐项调用 `WorldQiAccount::set_balance(account: QiAccountId, amount: f64) -> Result<(), QiPhysicsError>` 写入恢复余额。`set_balance` 是当前通用写入原语，不是已经存在的专用 restore API；P0 必须明确其恢复边界/验证责任，禁止在 persistence 另建等价 setter、隐式 transfer 或 event-only 恢复。
- `upsert_runtime_qi_account_balances(transaction: &rusqlite::Transaction<'_>, qi_ledger: &WorldQiAccount, wall_clock: i64) -> io::Result<()>`（`server/src/persistence/world_qi.rs`）：以 `&Transaction` 作为 SQLite 写入 owner、以 `&WorldQiAccount` 作为余额读取 owner、以 `wall_clock` 作为持久化时间输入；当前对固定白名单逐项写回，底层 `upsert_runtime_qi_account_balance` 校验余额 finite 且非负。它是写回链，不得被当作恢复链或新 ledger。
- `assert_conservation(before: &WorldQiSnapshot, after: &WorldQiSnapshot, era_decay: f64) -> Result<(), QiPhysicsError>`（`server/src/qi_physics/ledger.rs`）：以 before/after world snapshot 和 canonical `era_decay` 作为验证输入，验证观察总量与允许的时代衰减一致；P4 用它验证恢复/失败前后没有吞真元，不用字面常数代替。`WorldQiAccount::iter_balances(&self)` 只读暴露各 durable owner 的余额，可用于审计对拍，不提供 mutation capability。

上述调用链是“当前事实”，不是本 plan 的实现承诺；尤其不能把当前 `WorldQiAccount::set_balance` 包装成未经 P0 决议的新 persistence restore helper。若某个阶段触及受控恢复入口，必须先在 `qi_physics` owner 边界内明确其输入、失败原子性与审计语义，并同步更新本节；P1 不触及该依赖。没有带日期的 `pre-P0 决议` 写清选定入口、owner 输入、失败原子性和 audit 语义，就不得开始涉及该恢复链的阶段，也不得把当前 `set_balance` 的现状写成已收口契约。

## 3. 设计基础：三条归档发布不变式（原样承接）

以下三条文字原样承接自被 revert 的 `bb1d3b6f1` / `3611a07f7` 设计证据，是本 plan 的 P0 决策基础。实施前只能在 P0 的正式决议中澄清边界，不能用代码行为反推未写下的契约。

### 不变式一：删除必须有明确 ownership 证明

> 最终归档只能在对应生产调用方持有固定 `.npc.lifecycle.lock`、且 `ensure_archive_identity`（NPC 归档同时要求 payload 校验）仍证明该路径是本次发布者创建的文件时删除；身份不匹配、目标消失或无法证明 ownership 均不删除，并用 `combine_persistence_failure` 保留 ownership 诊断。

**为什么必须成立：** 归档路径是跨进程共享状态；路径名相同不等于文件仍由本次发布者拥有。没有生命周期锁、稳定身份或 NPC payload 证明时删除，可能删掉既有归档、并发 writer 的 successor 或不可恢复的唯一历史。fail-closed 是避免数据丢失的最低安全线，清理失败也必须保留原始错误上下文。

**可撞红的验证方式：** 在同一归档路径预置既有文件，并分别模拟目标消失、inode/身份变化、payload 变化、锁外 writer 替换和清理失败；断言既有文件/successor 不被删除，返回结果包含 ownership/rollback 诊断。正例要求固定 `.npc.lifecycle.lock` + identity（NPC 还要 payload）匹配时才允许删除本次发布者文件。

### 不变式二：批次失败的 hot rows 与归档文件必须按所有权回滚

> 批次中途失败时，SQLite transaction 回滚本批次已删除的 hot rows；本次 sweep 自己发布的文件只按保存的 `ArchiveFileIdentity` 回滚，既有文件或 successor 不得触碰；回滚错误与主错误一起上报。单条准备失败仍按既有契约隔离该条、继续处理健康 stale row，并在最终返回中保留首个准备错误。

**为什么必须成立：** sweep 同时改变数据库行和文件系统；只回滚其中一侧会产生 hot row 丢失、重复恢复或孤立归档。批次的回滚边界必须只覆盖本次发布且身份仍匹配的文件，不能把其他 writer 的成功发布当作 orphan；单条坏数据也不能阻塞所有健康 stale row。

**可撞红的验证方式：** 构造至少两个 stale row，在第一个已发布/删除后让后续准备或 CAS 失败；断言事务回滚先前 hot rows、只删除本批次且 identity 匹配的文件，既有文件和 successor 保留，回滚失败与主错误均可观察；另测单条准备失败继续处理健康 row且最终保留首个错误。对 prepared archive 列表做混合身份/消失/清理失败饱和测试。

### 不变式三：同一路径竞争必须串行且 no-replace

> 同一路径采用同一生命周期锁串行化，锁外/未协调 writer 只能在 `hard_link` no-replace 竞争中由先发布者获胜；失败方看到 `AlreadyExists` 或身份变化即 fail-closed，不覆盖、不删除、不把别人的成功发布当成自己的 orphan。该不变式冻结本 PR 的已有身份/回滚抽象，不再扩展新的 helper 类型或 ownership 概念。

**为什么必须成立：** rename/overwrite 或“按内容相同即复用”无法区分本进程和 successor，TOCTOU 会把并发 writer 的成功结果误删。固定锁覆盖观察、发布、identity/payload 校验、DB transaction 与失败清理，no-replace 再提供锁外竞争的最后防线；输家必须保守退出，不覆盖赢家也不夺取其所有权。

**可撞红的验证方式：** 让两个 writer 在同一目标上同时准备、发布、采样 identity 并在各自 DB 分支失败；断言只有先发布者建立目标，后发布者收到 `AlreadyExists`/身份变化并 fail-closed，任一方都不会删除另一方的文件。另测发布后到 identity 采样前被未协调 writer 替换（包括相同 payload），确认回滚按身份失败而不误删 successor；锁粒度测试需证明同路径操作不会在关键窗口交错。

### 3.1 当前主线 API 事实

以下名称是出口 D 移交的拟议实现面，不是当前 `origin/main=60f21e6ab36bd78fdfd2606dd766b09939d3fac5` 已存在的 API；本 plan 不得把它们写成现状：

`ensure_archive_identity`、`ArchiveFileIdentity`、`prepared_archives`、`combine_persistence_failure`、`published_by_sweep`、`archive_file_identity` 均在当前主线核验为零命中。P0 需先确定是否保留这些概念、各自的最小职责和测试可观察面；在开放问题未收口前不能通过便利性继续扩张 helper/type/public visibility。

## 4. 阶段交付物

### P0 — 不变式定稿与决策门

- 将第 3 节三条不变式定为实现契约，明确 owner、锁持有者、锁粒度、临时文件生命周期、归档身份采样时点和失败归属。
- 对现有 `server/src/persistence/{helpers,npc,world_qi}.rs` 逐条核验输入、输出、文件系统副作用、SQLite transaction 边界和 R3 P0 接入点；不引用旧 `mod.rs` 巨型文件行号。
- 决定发布/回滚语义后，写出状态转移表与每个错误分支的外部可观察结果；P0 未完成不得开始实现。
- 每条不变式至少先有一个可独立失败的 contract/regression harness，验证命令、目标 SHA 和预期红/绿行为写入 evidence。

### P1 — 最小安全网

- `server/src/persistence/helpers.rs:330` 的 `write_zstd_bundle_with_writer` 在临时文件写入失败后保留主错误；若 `fs::remove_file(&temp_path)` 也失败，则在不引入 P2 身份/CAS 抽象的前提下返回同时包含主错误与清理错误的诊断，禁止静默丢弃清理结果。
- 按 §8.1 #5（2026-09-09）决议评估并故意保留 `server/src/persistence/helpers.rs:8` 与 `server/src/persistence/mod.rs:172` 的 `expect`；P1 不改动这两处系统不变量断言。
- 为 `helpers.rs:330` 补能撞红的回归测试：测试注入在返回主错误前删除临时路径，使生产清理稳定得到 `NotFound`，并断言主错误、cleanup 诊断与 source 链均可观察；该用例明确不覆盖“清理失败留下可恢复临时文件”的另一性质，避免把互斥前提混在同一测试中。
- `npc.rs` 的 `first_error.expect` 不属于当前 P1 的真实落点：在 `origin/main@60f21e6ab36bd78fdfd2606dd766b09939d3fac5` 上执行 `git grep -n 'first_error' origin/main -- server/src` 为 0 命中，且 `git grep -nE '\.expect\(|let _ =' origin/main -- server/src/persistence/npc.rs` 也为 0 命中。该项是 #2180 未合入加固分支的条件性后续：只有 P2 真正引入 CAS 批次错误聚合路径时，才在该新落点采用 `unwrap_or_else` + `io::ErrorKind::InvalidData` 并配回归测试；P1 不伪造不存在的代码落点。

**P1 证据（2026-09-09）**：实现修复提交 `e8609830c2f8bef8c970e0f54f632a507149c919`，格式化收尾提交 `d6b27ab9242d45bd55fd081bfe00ab2bf24c2fe1`。后者为当前证据 HEAD；其定向命令 `../scripts/build-token.sh cargo test write_zstd_bundle_surfaces_primary_and_cleanup_failures -- --nocapture` 实际执行 `persistence::tests::write_zstd_bundle_surfaces_primary_and_cleanup_failures`，结果 `1 passed; 0 failed; 11977 filtered out`。在精确代码 HEAD `d6b27ab9242d45bd55fd081bfe00ab2bf24c2fe1` 上依次执行 `../scripts/build-token.sh cargo fmt --check`、`../scripts/build-token.sh cargo clippy --all-targets -- -D warnings`、`../scripts/build-token.sh cargo test`，三条退出码均为 `0`；完整测试主库为 `11977 passed; 0 failed; 1 ignored`，main 为 `18 passed; 0 failed`，doctest 为 `3 passed; 0 failed; 5 ignored`，各登记 unit/integration target 均无失败。`write_zstd_bundle_with_writer` 现在用私有 `ArchiveCleanupError` 保留原有两段 Display 文案并经 `Error::source` 暴露 primary error；回归注入在返回 primary error 前删除临时路径，确定性触发 cleanup `NotFound`，明确不覆盖“清理失败留下可恢复临时文件”的另一性质。本轮仍未修改迁移链、schema、R3 P0 生产接入点或跨仓库契约，也未引入任何 P2/P3 身份、CAS、no-replace 发布抽象；最终 HEAD 的无上下文 validator 尚待本轮重新验证。

### P2 — 归档身份与 CAS 批次回滚

- 按 P0 决议在 `helpers.rs`/`npc.rs` 落最小的 `ArchiveFileIdentity` 与 `ensure_archive_identity` 责任（若决议保留这些命名）：从已写入并同步的临时文件句柄在 `hard_link` 前捕获预期 identity，再证明最终目标与之匹配且 NPC payload 未被改换。
- 发布采用 no-replace 语义；`prepared_archives` 只登记本批次已成功发布、且使用 `hard_link` 前捕获的预期 identity 可证明的文件，并保留临时文件句柄到该批次事务提交或回滚结束。批次大小由当前进程的文件描述符预算推导，不得用无界 stale-row 列表累积句柄。
- CAS 失败时在 SQLite transaction 返回前回滚本批次已发布文件；每个 `rollback_file` 只在 identity 匹配时操作，回滚失败通过 `combine_persistence_failure` 与 primary error 聚合。
- 若 P2 实际引入 CAS 批次错误聚合并因此产生 `first_error` 路径，才在该真实新落点用 `unwrap_or_else` 构造 `io::ErrorKind::InvalidData` 并补回归；`origin/main` 当前没有该代码，P1 不提前添加。
- 不改变迁移链、表结构、事务边界或 R3 P0 生产接入点；为既有文件、目标消失、identity mismatch、混合批次和回滚失败分别补回归。

### P3 — 发布后替换竞态

- 收口 `published_by_sweep` 仅依据发布前状态授予删除权的问题：预期 identity 必须在 `hard_link` 前从本次临时句柄捕获；若未协调 writer 在发布后至最终路径核验前替换同 payload 目标，核验与失败回滚不得误删后继文件。最终路径采样只是与预期 identity 的核验窗口。
- 设计必须复用 P0 已决策的生命周期锁、no-replace 与身份/ownership 机制；不得通过“内容相同所以仍归本方”放宽删除条件。
- 回归测试要在 identity 采样窗口注入 successor replacement，断言 successor 保留、数据库批次按契约回滚、ownership 错误可观察；并覆盖 deceased 与 digest 两条归档路径。

### P4 — 饱和回归与验收

- 为三条不变式建立完整矩阵：happy path、空/缺失 metadata、目标不存在、既有文件、锁竞争、同 payload successor、不同 payload successor、CAS 失败、主错误+回滚错误、单条准备失败和多条混合批次。
- 断言外部可观察文件/SQLite/error/report 行为，不把测试绑定到 helper 调用次数或私有中间字段；并发测试必须说明调度控制、锁范围和失败方观测结果。
- 另测文件描述符预算的 `B == 0`、`B == 1`、多批次提交、句柄保持到提交/回滚结束，以及失败后 attempt 闸门不在同一 tick 重复扫描；批次上限必须由 `RLIMIT_NOFILE`、实时 FD 占用和 helper 峰值推导，不能用固定条数冒充资源安全。
- 运行 persistence 定向回归、完整 server fmt/clippy/test；每次修复或 merge 后证据必须绑定精确 HEAD，不得用早于文档/代码的 gate 或 validator 结果背书。
- 对 `world_qi.rs` 做守恒与恢复核验：固定 durable owner、非法快照 fail-closed、失败无部分 hydrate、无伪造 transfer；必要的 bot/e2e 跨栈依赖另行登记，不将 server-only 测试冒充端到端链路。

## 5. 移交清单（来自 #2180 出口 D）

以下事项全部从 #2180 移交，后续实施必须逐项验真、逐项有测试和证据；名称仅描述拟议职责，不能当作当前主线符号：

| 项目 | 来源/失效场景 | 本 plan 处理要求 |
|---|---|---|
| `ensure_archive_identity` / `ArchiveFileIdentity` 归档身份校验 | 按路径或仅按内容判断 ownership，可能误删既有文件或 successor | 定义稳定身份与 payload 校验时点；identity 不匹配 fail-closed |
| `prepared_archives` + `combine_persistence_failure` CAS 批次回滚 | CAS 失败只回滚数据库或吞掉文件回滚错误，留下孤立文件/丢诊断 | 只登记本批次、自有且可验证的发布；聚合 primary 与 rollback/ownership 错误 |
| NPC 生命周期锁文件写入语义 | 锁粒度不足导致观察、发布、校验、DB transaction 之间出现 TOCTOU | 明确固定 `.npc.lifecycle.lock` 的持有范围、失败释放与跨 writer 行为 |
| `published_by_sweep` sweep 归属判定 | 发布前状态被用作删除权；后继 writer 可在 identity 采样前替换目标 | 以 P0 ownership 证明收口，禁止仅凭“本轮曾发布”删除 |
| `helpers.rs` 回滚错误被静默吞掉 | `let _ = fs::remove_file(&temp_path)` 丢失清理失败上下文 | 保留原始错误并 surface 清理/回滚失败 |
| `npc.rs` TOCTOU 竞态 | 读取/校验/发布/回滚不在同一生命周期语义下，successor 可被误复用或误删 | 在既定抽象面内覆盖锁、身份、payload 和 no-replace 交互 |
| `npc.rs`（P2 条件项）`first_error.expect` panic | #2180 未合入加固分支的历史落点；`git grep -n 'first_error' origin/main -- server/src` 当前为 0 命中 | 若 P2 引入该 CAS 错误路径，再用 `unwrap_or_else` 构造 `InvalidData` 并锁定回归；P1 不伪造主线落点 |
| 发布后至 `archive_file_identity` 采样前的同 payload 替换 | 未协调 writer 替换目标后，旧 `published_by_sweep` 删除权会误删 successor；截至本骨架建立时尚未修复 | P3 必须在身份采样与回滚窗口建立可证明的 ownership 判据，并用 successor 饱和测试锁住 |

## 6. 非目标与生产接入保留清单

- 非目标：把原子发布加固重新并入 R3 P1 机械拆分；扩大 `pub`/`pub(crate)` seam；重写迁移；重做 SQLite schema；改变现有 wire 行为；把 snapshot restore 伪装成 gameplay transfer。
- 生产接入保留：`PersistenceSliceRegistry` canonical registry、`AppExit` 到 `Last` 的 shutdown dispatcher、zone-runtime shutdown descriptor，以及 KnownTechniques reconnect/load guard/dirty snapshot/durable fence adapter。
- 任何实现 PR 都必须在 plan evidence 中说明上述接入点未被移除或旁路，并说明 `world_qi.rs` 的持久化恢复仍由 `qi_physics` 受控接口负责。

## 7. 验收证据约定

- 每次 validator、定向测试、完整 server gate 和主线 merge 复验都记录精确 commit SHA；测试/validator 在文档提交后若 SHA 改变，必须重跑或明确证据过期。
- 失败证据不能用“疑似既有问题”代替第一性结论；应写出复现输入、实际文件/数据库/error 结果、预期契约以及 owner 判断。
- pre-P0 决议阶段为 docs-only；P1 及以后必须按精确 SHA 记录真实代码、测试和 gate 证据，且在对应阶段完成前不宣称原子发布已 production reachable。

## 8. 开放问题（P0 决策门前需收口）

以下原为只提问、不在骨架阶段拍板的 P0 阻塞项；所有问题均须在 P0 追加带日期的 `## 8.1 决议（pre-P0 收口，YYYY-MM-DD）`，逐项记录选型、owner、失败原子性和可观察证据后，才能进入实现：

1. 归档发布语义选择**原子重命名**还是**两阶段提交**？在跨平台文件系统与 no-replace 要求下，哪个操作是可验证的原子边界？
2. 批次中途失败选择**全回滚**，还是允许**部分发布 + 幂等重放**？两种语义如何分别与 SQLite hot-row transaction、恢复扫描和重复执行相容？
3. 文件身份采用 **inode**、内容 **digest**，还是两者兼备？在 successor 使用同 payload 时，什么组合仍能证明 ownership 而不误删？
4. 是否把这套机制抽成供 persistence 各切片共用的通用 helper？`npc.rs`、`helpers.rs` 与其他切片之间的共用边界是什么；若抽象会扩大 public/seam，如何冻结在最小可见性？

全部已在 §8.1 收口。原表保留以备追溯，实施时以 §8.1 决议为准。

## 8.1 决议（pre-P0 收口，2026-09-09）

> 证据基线：以下代码行号由 `git show origin/main:<path> | nl -ba` 与 `git grep origin/main` 在 `origin/main@3748d52a77882f8352b408d2d83b3e70ed89a36c` 上实读得到。本文中的 `ArchiveFileIdentity`、`ensure_archive_identity`、`prepared_archives`、`combine_persistence_failure` 等名称仍是本 plan 的拟议实现面，不是当前主线 API；§3 三条不变式保持原样不动。

### #1 发布语义：同目录 `link(2)` + `unlink(2)` 的 no-replace 发布

**决议**：

1. 选用“同一目标目录内创建临时文件 → 写入完整压缩 bytes 并 `sync_all` → 从仍打开的临时文件句柄取得 `(st_dev, st_ino)`，保存实际写入 bytes 的 digest 作为预期 identity → `std::fs::hard_link(temp, final)` → 仅将最终路径与预期 identity 比对核验”的序列；Linux 上 `std::fs::hard_link` 使用 `link(2)`，`link(2)` 是原子 no-replace 边界：目标已存在时返回 `AlreadyExists`，失败方不得覆盖或删除现有目标。临时文件继续用 `OpenOptions::create_new(true)`，因此临时名字也不会互相覆盖。
2. `link(2)` 成功建立最终目录项且最终路径匹配预期 identity 后，最终文件视为已发布；临时文件句柄必须保持到 SQLite transaction 提交或回滚结束，之后才用 `unlink(2)` 清理临时名字。临时名字清理失败必须作为可观察的 cleanup/rollback 诊断返回并交给恢复扫描处理，但不能因此按路径删除已发布最终文件；必要的目录同步随该发布/收尾序列完成，以保留 Linux 崩溃后的目录项可恢复性。
3. 拒绝普通 `fs::rename`/`rename(2)`：它虽在同一文件系统内原子，却会覆盖已有目标；“`O_EXCL` 临时文件后再 `rename`”仍然保留覆盖窗口。也不以没有现有实现/依赖的跨平台 rename fallback 掩盖 no-replace 约束；本 plan 的生产运行面限定为 Linux，同文件系统由临时文件与最终文件位于同一目标目录保证，其他平台应 fail-closed 并另行决策。

**落点**：代码锚点为 `server/src/persistence/helpers.rs:297-334`（当前压缩、`create_new` 临时文件和会覆盖目标的 `fs::rename`）以及 `server/src/persistence/npc.rs:176-226,252-290`（deceased/digest 两条发布调用链）；plan 锚点为 §3「不变式三」、§4 P2「no-replace」、§4 P4「失败矩阵」和本节 #1。

### #2 批次失败语义：数据库全回滚，文件按 ownership 全回滚，残留只能作为可诊断恢复状态

**决议**：

1. 选用“每个受文件描述符预算约束的批次全回滚”，不采用无界 sweep 的“部分发布 + 幂等重放”作为正常成功语义。每个批次在读取 bounded stale rows 之前开启一个 SQLite transaction，并将该批次的文件发布、最终路径核验、hot-row 删除和 index 变更纳入同一事务生命周期；transaction 未提交时 hot rows/index 不得留下本批次的删除或推进。已发布文件由 `prepared_archives` 逐项登记其路径、`hard_link` 前从临时句柄捕获的预期 identity 以及保持中的句柄；只有发布后最终路径与该预期 identity 匹配才进入登记，失败时仅按这份预期 identity 回滚本批次文件，并用 `combine_persistence_failure` 同时保留主错误与回滚/ownership 错误；单条预处理失败仍按既有逐条隔离语义跳过该条、继续健康条目，未发布条目不进入回滚清单。
2. 正常错误路径的逻辑结果必须是“事务未提交 + 本批次自有归档已撤销”。进程崩溃、目录同步失败或清理失败造成的已发布但未索引文件/临时名字，是允许存在但不代表部分成功的恢复残留：启动恢复扫描必须把它识别为 orphan，只有能重新证明 ownership 才能清理，否则保留文件并报告，绝不按路径猜测删除。
3. 重复执行只在恢复扫描已安全清理残留或已存在的 index/path 与保存的、来源于发布前临时句柄的归档身份能够明确对拍时幂等；没有 index 的 `AlreadyExists` 不得静默复用，必须 fail-closed 并保留诊断。多个成功批次可以各自提交，但每个批次内部仍是全回滚原子单位；某批次失败时不回滚此前已经提交的独立批次，下一次重试从仍存在的 stale rows 重新按确定性顺序取 bounded batch。
4. 批次上限不使用固定条数魔数：每次批次开始在 Linux 读取当前进程的 `RLIMIT_NOFILE` soft limit `L`，在打开该批次 SQLite transaction 后统计当前已占用的进程 FD 数 `F`（`/proc/self/fd`），并由 helper 的固定 syscall 序列计算一次最多同时需要的非归档临时 FD 数 `T`（最终路径核验、目录同步等操作的峰值）。每个待发布归档保留一个临时文件句柄直到该批次 transaction 提交或回滚结束，因此安全批量上限是 `B = max(0, L - F - T)`；实现必须保持 `F + B + T ≤ L`，`B == 0` 时 fail-closed/延后本批次，不强行塞入一条记录。批次查询必须使用该 `B` 做 `LIMIT`（并沿现有 `last_referenced_wall, char_id` 顺序分页），不能让 `load_stale_npc_digests` 返回无界 Vec。每批申请句柄前重新核对预算；若其他同进程活动令预算变小或实际打开返回 `EMFILE`，立即按本批次契约回滚并进入 attempt 闸门，禁止临时扩大 `B` 或退回发布后采样。若 soft limit 或实时 FD 计数不可得，则 fail-closed，不猜一个条数上限。这样句柄仍覆盖整个批内事务，但数量由 OS 预算、实时 FD 占用和实际 helper 峰值推导。
5. 当前 `NpcDigestSweepState::last_sweep_wall` 只在成功分支更新会导致失败后每个 tick 重做扫描；将 sweep 闸门改为记录每次尝试的墙钟时间（成功与失败均更新，或等价地使用私有 `last_attempt_wall`），失败批次的 rows 因 transaction/文件全回滚而保留，下一次只在既有 `NPC_DIGEST_SWEEP_INTERVAL_SECS` 间隔后重试。成功批次已删除的 rows 不会再次 I/O；进程重启后的首次尝试属于正常恢复，不通过 schema 偷加持久化游标。

**落点**：代码锚点为 `server/src/persistence/npc.rs:12-15,252-290,413-431,1250-1281`（当前 sweep 状态、无界 stale-row 查询、文件发布后才开 transaction 及失败不推进 attempt 时间）、`server/src/persistence/mod.rs:112-113`（既有 sweep 间隔）和 `server/src/persistence/helpers.rs:372-422`（当前 orphan 枚举/扫描入口）；plan 锚点为 §3「不变式二」、§4 P1/P2/P4、§5 移交清单和本节 #2。

### #3 文件身份：`st_dev + st_ino` 与精确 bytes digest 两者兼备

**决议**：

> 对原样保留的 §3 不变式中“identity 采样窗口”作明确解释：它是 `hard_link` 之后对最终路径进行采样并与发布前预期 identity 比对的窗口；该采样结果只能用于核验/拒绝，不能成为 ownership 的来源。ownership 永远来自 `hard_link` 前的临时文件句柄。

1. `ArchiveFileIdentity`（拟引入）必须在 `hard_link` 前从本次 `create_new` 临时文件的仍打开句柄捕获 Linux 文件身份 `(st_dev, st_ino)`，并保存已成功写入、`sync_all` 完成的完整 compressed bytes 的固定 digest；NPC 归档额外解压并校验 payload 的 `char_id`、归档记录语义与预期输入。digest 必须覆盖实际发布的完整 compressed bytes，而不是仅覆盖路径或未经发布的对象表示；`NpcDeceasedArchiveRecord` 的字段边界以当前模型为准。
2. `hard_link` 发布成功后立即用最终路径读取 metadata/bytes/payload 与这份发布前预期 identity 比对；最终路径采样只用于核验，绝不用于直接授予 ownership。每次撤销前在同一 `.npc.lifecycle.lock` 保护下再次执行该匹配，且临时文件句柄保持到 transaction 提交或回滚结束；只有 `(dev, ino)`、digest 和所需 payload 条件全部匹配才允许删除/恢复。目标不存在、读取失败、任一字段不匹配均 fail-closed，并通过拟议的 `combine_persistence_failure` 保留 ownership 诊断。
3. 独立创建的 successor 即使使用完全相同 payload，也会是新的 `(st_dev, st_ino)`，所以不能因 digest 相同而复用或删除；若同一 inode 被原地改写，digest/payload 校验仍会拒绝；只有同一 inode 且 bytes 与 payload 均匹配时，才证明仍是同一个文件对象。所有遵守本协议的 writer 必须持同一生命周期锁，锁外 writer 只允许以 `link(2)` no-replace 竞争、不得替换/删除已有目标；因此最终校验与撤销在锁内不会被协议 writer 插入 successor。对任意绕过协议直接 unlink/recreate 的外部操作者不作不可能的 pathname 原子比较承诺，检测到身份变化一律不删并交恢复处理。

**落点**：代码锚点为 `server/src/persistence/helpers.rs:32-49`（当前文件读取/回滚边界）、`server/src/persistence/helpers.rs:297-334`（发布 bytes 的临时文件生命周期）、`server/src/persistence/npc.rs:176-226,252-290`（两条归档 payload/transaction 路径）和 `server/src/persistence/models.rs:420-439`（NPC 归档模型字段）；plan 锚点为 §3「不变式一/二/三」、§4 P1/P2/P3 和本节 #3。

### #4 通用 helper：只共享文件系统原语，保持 persistence 内部最小可见性

**决议**：

1. 采用窄边界共享 helper，但只覆盖 deceased 与 digest 两条确实相同的文件系统机制：临时文件写入/同步、从临时文件句柄在 `hard_link` 前捕获预期 identity、Linux no-replace 发布、发布后用最终路径做 `ensure_archive_identity` 校验、按预期 identity 回滚，以及 `combine_persistence_failure` 错误聚合。它们落在 `helpers.rs`，通过 persistence 父模块现有的内部 re-export 提供给切片；不再新增 helper 类型、ownership 概念或第二套发布抽象。
2. helper 只返回/消费 persistence 内部所需的拟议身份与错误边界，保持 `pub(super)`（父模块 re-export 也不得扩大到 `pub`/`pub(crate)`）；不新增 `#[doc(hidden)]`、测试专用 seam 或跨 crate API。发布方必须持有临时文件句柄直到 transaction 提交或回滚结束；`npc.rs` 负责 `RLIMIT_NOFILE`/实时 FD 占用推导、bounded stale-row 分页、批次 transaction 与失败重试闸门，保留 char_id/归档 payload 解码、stale-row 选择、sweep 编排和 SQLite transaction 策略，helper 不携带批量调度或 NPC 业务决策。
3. `player.rs`、`social.rs`、`tribulation.rs`、`void_actions.rs`、`world.rs`、`world_qi.rs` 不因“看起来可复用”接入该 helper；只有出现同一归档发布契约且另有 plan/P0 明确 owner 时才扩展。当前模块拆分与内部导出边界以 `mod.rs` 的七切片声明和现有 re-export 为准，迁移链、schema、事务边界与 R3 P0 接入点保持不变。

**落点**：代码锚点为 `server/src/persistence/mod.rs:132-162`（七切片与现有内部 re-export）、`server/src/persistence/helpers.rs:297-334`（可共享文件发布边界）及 `server/src/persistence/npc.rs:12-15,176-226,252-290,413-431,1250-1281`（两个消费者、当前 sweep 状态、无界查询和事务/重试边界）；plan 锚点为 §2.1/§2.3、§3「不变式三」、§4 P2/P4、§5 移交清单和本节 #4。

### #5 P1 系统不变量 `expect` 评估：故意保留（2026-09-09）

**决议**：

1. `server/src/persistence/helpers.rs:8` 的 `current_unix_seconds` 对 `SystemTime::duration_since(UNIX_EPOCH)` 使用 `expect`。依据是 `origin/main@60f21e6ab36bd78fdfd2606dd766b09939d3fac5` 的实读代码：运行时系统时钟早于 Unix epoch 时无法得到合法的持久化墙钟值；这是基础运行时前提失败，不是恢复路径可降级的输入。故意保留 fail-fast，不以静默零值或伪造时间继续写入。
2. `server/src/persistence/mod.rs:172` 的生产 `PersistenceSliceRegistry` 注册链对 descriptor 校验使用 `expect`。同一 `origin/main@60f21e6ab36bd78fdfd2606dd766b09939d3fac5` 的实读代码表明该错误意味着生产 persistence 接入描述符违反静态注册不变量；没有安全的部分注册或恢复回退路径。故意保留 fail-fast，阻止带坏 descriptor 的生产启动。
3. 这两处都不属于归档发布、临时文件 cleanup 或 P2/P3 身份/CAS/no-replace 机制；P1 只记录判断，不改变它们的行为。

**双锚点**：代码锚点为 `server/src/persistence/helpers.rs:8` 与 `server/src/persistence/mod.rs:172`，依据为上述 `origin/main` 精确 SHA 的 `git show ... | nl -ba` 实读；plan 锚点为本节 §8.1 #5、§3 不变式与 §4 P1 最小安全网。

## Finish Evidence

> 本 plan 尚未完成。后续完成 P0–P4 后，按根 `CLAUDE.md` 要求填写真实落地文件、关键 commit/日期、定向与完整测试结果、server/agent/client 跨仓库核验（若无跨仓库变更须明确写明）及遗留/后续，再由独立流程归档。
