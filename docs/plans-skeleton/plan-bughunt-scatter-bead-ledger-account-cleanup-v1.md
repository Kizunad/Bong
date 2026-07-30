# plan-bughunt-scatter-bead-ledger-account-cleanup-v1（骨架）

> **骨架（草案）**。一句话主题：只关闭 bughunt r9 #1——散灵珠完成主动破裂、owner 触发或自然耗尽后，先把最后一分真元守恒转入 zone/overflow，再删除已精确归零的 `qi_scatter:*` / `qi_scatter_buried:*` 临时 source account，避免 `WorldQiAccount` 与 `bong:qi/ledger` 长跑积累僵尸键。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 临时 source 生命周期合同、守恒终结器与 fail-closed retirement guard | ⬜ |
| P1 | 主动使用、owner 触发、自然耗尽三条 production terminal path 接线 | ⬜ |
| P2 | 边界/失败/长跑/telemetry 饱和回归与真实 schedule 证明 | ⬜ |

## 接入面

- **进料**：client 既有 `qi_scatter_bead_use` 请求经 `server/src/network/client_request_handler.rs:1687-1715` 进入 `ScatterBeadUseRequest`；`server/src/zhenfa/mod.rs:2452-2587` 的 `handle_scatter_bead_use` 生产即时 source 或 `ScatterBeadBurials`，`:2589-2648` owner trigger、`:2651-2734` `qi_excretion(ContainerKind::EmbeddedTrap)` tick 生产终结条件。
- **出料**：释放仍只走 `release_scatter_qi_to_zone` → `qi_release_to_zone` → `WorldQiAccount::transfer`，zone 满时继续转入 `QiAccountId::overflow`；本 plan 新增的唯一出料是成功 terminal 后从 `WorldQiAccount.balances` 删除**零余额临时 source key**，从而让 `iter_balances` / `build_qi_ledger_hash_fields` 不再发布已终结珠子的 per-account 字段。
- **共享类型 / event**：复用 `WorldQiAccount`、`QiAccountId`、`QiTransfer`、`QiTransferReason::ReleaseToZone`、`QI_EPSILON`、`QI_SCATTER_BEAD_CAPACITY`、`ScatterBeadBurials`、`ScatterBeadUseRequest`、`ScatterBeadTriggerRequest`；禁止另造 ledger、epsilon、transfer event 或并行账户表。
- **跨仓库契约**：纯 server runtime lifecycle cleanup；不改 client 请求形状、proto/TypeBox、agent Redis、VFX/audio/narration。现有玩家反馈完全保留。
- **worldview 锚点**：`worldview.md §五 L417-L421`（地师把真元封入环境诡雷且无人触发会随载体逸散）、`§五 L457-L465`（阵法主轴是真元逆逸散效率）、`§二 L30-L46`（灵压与环境交换）。
- **qi_physics 锚点**：`server/src/qi_physics/ledger.rs:390-480` 是账户唯一权威；`server/src/qi_physics/constants.rs:52,122` 提供 capacity/epsilon。终结前必须保持 `bead_remaining + Σ(zone accepted) + Σ(overflow) == QI_SCATTER_BEAD_CAPACITY`；禁止删除任何正余额、禁止用 `set_balance(source, 0.0)` 擦账、禁止把 cleanup 冒充 transfer。

## Canonical Finding Mapping（本 plan 的全部 delivery scope）

| Canonical finding | 本 plan 覆盖 | 明确不覆盖 |
|---|---|---|
| `docs/finished_plans/plan-bughunt-r9-findings-v1.md` r9 #1 / Finding Mapping `#1 scatter-bead ledger zombie` | `qi_scatter:*` 即时 source 与 `qi_scatter_buried:*` 预埋 source；主动破裂、owner trigger、自然耗尽（含 tick 开始即 `remaining_qi <= QI_EPSILON`）三类成功 terminal | burial 持久化/重启恢复、库存扣除回滚、通用 ledger redesign、transfer history compaction、其他账户 namespace |

计数固定为 **1 条 canonical finding**。即时与预埋 namespace 是同一散灵珠临时 source 生命周期的两个形态，不增 finding；P0/P1/P2 也不是额外 finding。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-30）

1. `server/src/zhenfa/mod.rs:597-635` 把 `handle_scatter_bead_use → handle_scatter_bead_trigger_requests → tick_scatter_bead_excretion` 真实加载进 `Update` 的 `ZhenfaSystemSet::Runtime` chain。
2. `release_scatter_qi_to_zone`（`:2366-2449`）先将 source 写入 `WorldQiAccount`，再把 accepted/overflow 守恒转出；`WorldQiAccount::transfer`（`server/src/qi_physics/ledger.rs:416-454`）只把 source 余额写成 0，不删除 key。
3. 即时使用成功链（`zhenfa/mod.rs:2547-2586`）、owner trigger 成功链（`:2614-2647`）和自然耗尽移除链（`:2663-2733`）均未调用 `remove_balance`；自然 tick 开头 `remaining_qi <= QI_EPSILON` 还会不做当 tick release 就直接移除 burial。
4. `WorldQiAccount::iter_balances` / `total`（`ledger.rs:457-480`）与 `build_qi_ledger_hash_fields`（`:650-705`）保留并发布每个零余额 key；散灵珠 item instance / bead ID 持续增长，因此长跑 account/telemetry cardinality 无界增长。
5. 现有生产测试 `scatter_bead_active_use_consumes_item_and_applies_ledger_transfer` 只断言 `balance(source) <= QI_EPSILON`，没有断言 `!has_account(source)`；buried owner-trigger 测试也只断言 burial 被删和 zone 收到 qi，故僵尸 key 回归不会撞红。
6. `server/src/lingtian/qi_account.rs:62-80` 已有 owner 在生命周期结束后调用 `remove_balance` 的模式，但它不证明散灵珠 terminal sequencing、namespace ownership 或正余额 fail-closed。

## P0 — 临时 source 终结合同

- [ ] 在 `server/src/zhenfa/mod.rs` 增加 focused typed helper，例如 `ScatterSourceAccountId` / `ScatterSourceTerminal` 与 `retire_scatter_source_account`（名称可等义）。helper 只接受由 `ScatterSourceAccountId::parse`/typed constructor 构造的 `QiAccountKind::Container` source；不能按裸 `:` 分段，因为 `canonical_player_id` 当前自身是 `offline:<username>`。规范语法固定为 `qi_scatter:<canonical_player_id>:<canonical_u64_item_instance>` 或 `qi_scatter_buried:<canonical_player_id>:<canonical_u64_bead_id>`：parser 必须先剥离精确 namespace、再从余串最后一个 `:` 右切数值段；owner 必须非空、严格等于 `canonical_player_id(username)` 的 `offline:<non-empty username>` 形状，username 不得含 `:`；数值段必须是无符号十进制 `u64` 的 canonical `to_string()`（无正负号、空白、前导零或额外段）。zone、overflow、其他 container 账户、相似前缀及即时/预埋形状混用一律稳定拒绝。
- [ ] retirement 必须区分 `present-zero`、`absent` 与 `present-positive`：只有**存在且 balance 精确为 0.0**的 owned source 可以 `remove_balance`；absent 返回 `SCATTER_ACCOUNT_MISSING`，任意正余额（包括 `next_down(QI_EPSILON)`、`QI_EPSILON`、`next_up(QI_EPSILON)`）返回 `SCATTER_ACCOUNT_RESIDUAL_QI` 并保留原 key/余额。禁止以 epsilon 为借口直接丢弃真元。
- [ ] terminal drain 必须先完成可核验的 preflight，再以现有 canonical `QiTransfer` 将有限正额完整转入 accepted + overflow，使 source 由真实 transfer 到达精确 0，再调用 retirement；若现有 `release_scatter_qi_to_zone` 会先写 zone/source/event、再因 overflow 失败返回，则实现必须在本 plan 内定义并测试可观察的 preflight/rollback/commit barrier，使失败时 source、burial、zone、overflow、transfer history 零新增副作用；不得只新增断言，也不得复制释放公式或直接改 zone/overflow 余额。对已消费即时 item 的失败例外只能是同一 runtime terminal recovery marker 的创建/更新（不改 qi 状态），该 marker 记录 owner/source/位置/overflow key 并允许同一进程重试；它不是 inventory rollback、burial persistence、hydrate/replay 或通用事务框架。
- [ ] `release_scatter_qi_to_zone` 不得接受 caller-supplied balance 作为无条件 `set_balance` 覆盖，也不得兼任 source establishment。terminal path 必须先读取 ledger 当前 source 三态：missing fail-closed、present-positive 只能按实际余额 release、present-zero 只有在无需转账时允许 retirement；已有 residual 与参数不一致、source missing 被“现造”或任何中途 transfer 失败均由 `SCATTER_ACCOUNT_MISSING` / `SCATTER_ACCOUNT_RELEASE_FAILED` 稳定 diagnostic 拒绝，并保留可重试 owner state。即时 source 的首次 absent→capacity 建立必须走 P1-A 独立 typed seam；预埋 source 仍由 burial 创建链建立。
- [ ] `scatter_source_retirement_contract_is_pinned`（名称可等义）覆盖两个合法 grammar 的 canonical constructor/parser、其他 account kind/prefix、缺失/空 `offline:<username>` owner、username 内嵌 `:`、缺失/空 item/bead 段、前导零/正负号/空白数值、额外分隔段、相似前缀、即时与 buried 形状混用、present-zero、absent、正余额与 epsilon 相邻可表示值；每个失败 case 同时断言 parser/guard diagnostic、account map、余额、transfer history、zone/overflow 与 burial 均不变。
- [ ] **terminal recovery 承载与注册**：`ScatterTerminalRecovery` 必须是 `Resource`（或等价唯一 world-local store），以规范 `ScatterSourceAccountId` 为唯一 key，字段固定为 owner、source、pos、overflow key、last diagnostic、remaining retry budget、`next_retry_tick`、terminal kind 与 first/last failed tick；`handle_scatter_bead_use` 是唯一即时 marker 写入方，owner trigger/natural/early-depleted 失败保留其既有 burial/source owner state，不创建即时 marker。budget 以命名常量 `SCATTER_TERMINAL_RETRY_BUDGET = TICKS_PER_SECOND`（当前 20）初始化，每个 eligible `Update` 最多消耗一次；首次 marker 的 `next_retry_tick = failed_tick + 1`，禁止同一 Update 内立即重放。`retry_scatter_terminal_recovery` 必须在 `zhenfa::register` 的同一 `ZhenfaSystemSet::Runtime` chain 中注册，精确位于 `tick_scatter_bead_excretion` 之后、`tick_scatter_disturbance_zones` 之前；按 source key 幂等 upsert marker、每次失败只递减一次预算、成功执行 release→zero proof→retirement 后原子移除 marker，预算耗尽保留 marker/source 并稳定报告 `SCATTER_ACCOUNT_RELEASE_FAILED`。production loading test 必须从 `zhenfa::register` 驱动真实 `Update` 验证这条顺序和全部状态转换。
- [ ] **提交边界故障矩阵**：terminal drain 必须用 deterministic fault injection 覆盖 preflight rejection、accepted zone transfer 前、accepted zone transfer 已应用但 overflow transfer 前、overflow transfer rejection、overflow transfer 已应用但 retirement 前；若最终采用全量 preflight，则仍需证明进入 commit 后每个 canonical transfer/retirement 不存在可返回错误的分支。每个 case 对比调用前后 source、burial、zone、overflow、完整 transfer history、recovery marker 与 allowed inventory state：失败必须完全相同，重试成功只追加一次对应 transfer、只删除一次 source/marker/burial，禁止重复入账或重复 audit。
- [ ] negative fixture / mutation gate 为强制交付物：分别对 active-use、owner-trigger、natural leaked、early-depleted 四条 terminal wiring 做“删除 retirement/marker consumer 或替换为 no-op”的 mutation；fixture 必须仍可编译、由 `zhenfa::register` 加载并至少运行一个真实 `Update`，由独立 anti-orphan guard 读取 production registration/terminal-path manifest 并自身产生稳定 `SCATTER_ACCOUNT_TERMINAL_LEAK`（或明确子码）diagnostic。普通 rustc/解析/链接、零测试、测试进程 panic 或无关失败一律不算命中。

## P1 — 三条 production terminal path 接线

### P1-A — 即时主动破裂

- [ ] `handle_scatter_bead_use` 在既有 item 消费成功后，必须先经独立、typed、只允许 absent→`QI_SCATTER_BEAD_CAPACITY` 的 source-establishment seam 建立 `qi_scatter:{owner_player_id}:{item_instance_id}`，再调用不得合成账户的 terminal drain；release 成功且 accepted + overflow 已真实落账后才 retire source。source establishment 失败属于既有 inventory transaction 缺口，不得伪造 source 或 cleanup；source 已建立后的 release/retirement 失败则必须创建或更新上述 runtime terminal recovery marker（marker 本身不改 qi 状态），不得留下无 retry owner state 的 orphan source。inventory rollback 不是本 plan 的实现路径，不能以隐式旁路替代 marker。
- [ ] **P1-A recovery marker contract**：marker 只允许 owner 已消费且 source 已成功建立的 terminal；按 `ScatterSourceAccountId` 唯一 key 幂等 upsert，保存 P0 冻结的字段并由 `retry_scatter_terminal_recovery` 通过既有 `ZhenfaSystemSet::Runtime` 在有限预算内重放 terminal drain。首次失败创建一个 marker；同 source 重复失败只更新同一 marker、推进 last-failed tick/diagnostic 且每个 Update 至多递减一次预算。marker 不改 inventory、burial persistence、hydrate/replay、ledger balance 或 transfer history；重试成功后按同一 release → zero proof → retirement → marker removal 顺序提交且全生命周期只落一次 terminal transfer/audit，预算耗尽则停止自动重试、保留 marker/source 并以 `SCATTER_ACCOUNT_RELEASE_FAILED` 可观测失败，不得静默删除或丢 source。
- [ ] full-zone case 仍把全部 capacity 留在 tracked overflow account；cleanup 只删 source，不删 overflow，也不删 `WorldQiAccount::transfers` audit history。long-run cardinality 断言只量化 `qi_scatter:*` / `qi_scatter_buried:*` source namespace；tracked overflow accounts/history 的增长是守恒审计产物，不算本 finding 的 source zombie。

### P1-B — owner trigger

- [ ] `handle_scatter_bead_trigger_requests` 对 `qi_scatter_buried:{owner_player_id}:{bead_id}` 完成 release → source-zero 证明 → retirement → burial removal 的有序提交。ZoneRegistry 缺失、release/overflow transfer 失败、missing/residual/namespace guard 失败时必须保留 burial 与 source，允许后续重试。
- [ ] 非 owner、重复/陈旧 trigger 不得触发 release 或 cleanup；成功重试恰好产生一次终结转账与一次 key removal。

### P1-C — 自然逸散与早已耗尽

- [ ] `tick_scatter_bead_excretion` 正常 leaked path 在 `remaining_after <= QI_EPSILON` 时把末尾 residual 合并进最后一次守恒 release，使 `remaining_qi` 与 source balance 都精确归零，再 retire source 和 burial；不得把 `(0, QI_EPSILON]` residual 静默抹掉。
- [ ] tick 开始即 `remaining_qi <= QI_EPSILON` 的 early-depleted branch 不得直接 push `depleted`：账户/remaining 任一仍为正时先做 terminal drain；zone 不存在或 release/retirement 失败时保留 burial/source，只有成功后才删除。
- [ ] production schedule 不新增平行 cleanup sweep；cleanup 紧邻 owner terminal path，仍由 `zhenfa::register` 的既有 `ZhenfaSystemSet::Runtime` chain 驱动。

## P2 — 饱和验收

- [ ] 强化 production App 测试：即时使用、owner trigger、自然 excretion、early-depleted 四条成功链在 terminal 后同时断言 `!ledger.has_account(source)`、burial 状态正确、accepted + overflow + remaining 精确闭合 capacity、对应 transfer audit 仍存在。
- [ ] 边界表覆盖余额/remaining 的 exact 0、`next_down(QI_EPSILON)`、`QI_EPSILON`、`next_up(QI_EPSILON)`、partial residual、full capacity；正额必须先 transfer 后删 key，直接调用 retirement 时则稳定 fail-closed。
- [ ] `release_scatter_qi_to_zone`、source-establishment seam、retirement guard 与 runtime terminal recovery marker 的失败表必须统一稳定 diagnostic：`SCATTER_ACCOUNT_MISSING`（terminal drain source absent/未 hydrate）、`SCATTER_ACCOUNT_ALREADY_EXISTS`（source establishment 发现同 ID 既有账户，禁止覆盖/重复注资）、`SCATTER_ACCOUNT_RESIDUAL_QI`（正余额不可删）、`SCATTER_ACCOUNT_NAMESPACE`（account kind 或完整 grammar 非法）、`SCATTER_ACCOUNT_RELEASE_FAILED`（preflight、canonical transfer、retirement 或 recovery budget 失败）；terminal wiring 缺失必须由 `SCATTER_ACCOUNT_TERMINAL_LEAK` guard 自身报错。每个失败 fixture 先证明可编译、可加载并注册真实 `ZhenfaSystemSet::Runtime`，再断言对应 diagnostic 与允许状态快照；除首次即时失败允许幂等创建/更新唯一 recovery marker 外，source/burial/zone/overflow/transfers/inventory 必须零新增副作用。
- [ ] recovery state machine `scatter_terminal_recovery_state_machine_is_pinned`（名称可等义）必须用 production App 从真实 `zhenfa::register` 连续驱动 `Update`：首次 terminal 失败创建唯一 marker；第二次失败只更新同 marker 且预算精确递减一次；后续成功只追加一次 accepted/overflow terminal transfer/audit，精确删除 source 和 marker；预算耗尽后停止自动重试并保留 marker/source、稳定产生 `SCATTER_ACCOUNT_RELEASE_FAILED`。每一步同时断言 inventory 不再变化、burial 按 owner 类型保持、source/zone/overflow/完整 transfer history 的允许差分及同 source 不重复入账。
- [ ] deterministic commit-boundary `scatter_terminal_commit_boundary_is_atomic`（名称可等义）按 P0 故障矩阵注入 accepted transfer 前/后、history append 后、overflow transfer 前/拒绝/后及 retirement 前失败；失败后 source、burial、zone、overflow、完整 transfer history 与 marker 除允许首次幂等写入外均与快照一致。若 implementation 选择全量 preflight，则额外证明 commit phase 无 fallible branch，并 mutation 掉该证明时由 `SCATTER_ACCOUNT_RELEASE_FAILED` fixture 撞红。
- [ ] namespace grammar `scatter_source_account_id_grammar_is_pinned`（名称可等义）覆盖两个 canonical typed constructor/parser、合法 `offline:<username>` owner、空/缺失 owner 或 username、username 内嵌 `:`、空/缺失/额外 item/bead 段、相似前缀、前导零/符号/空白/overflow 数值、即时/预埋形状混用；每个非法 case 稳定返回 `SCATTER_ACCOUNT_NAMESPACE`，不得删除或改变任一账户、burial、zone、overflow 与 transfer history。
- [ ] long-run `scatter_source_accounts_do_not_accumulate`（名称可等义）在同一 App 驱动足量即时与预埋生命周期，针对 `qi_scatter:*` 与 `qi_scatter_buried:*` 两个临时 source namespace，断言每个已终结 source key 不在 `iter_balances`，且这两个 namespace 的 live/terminal key 数不随已终结珠子增长；不得把 `QiAccountId::overflow(...)` 或 `WorldQiAccount::transfers` audit history 计入 source cardinality。tracked zone/overflow 余额与 transfer history 保留。
- [ ] telemetry pin：`build_qi_ledger_hash_fields` 不再含已 retire source 的 `account:container:qi_scatter*` 字段，活跃未耗尽 burial 与 overflow 字段仍存在。总账 cleanup 前后（只删除 zero key）`WorldQiAccount::total()` 精确不变。
- [ ] production loading / anti-orphan test 从 `zhenfa::register` 建 App 并驱动真实 `Update`，机械对拍 terminal-path manifest 与实际 `ZhenfaSystemSet::Runtime` registration：active use 必须 `handle_scatter_bead_use → establish/drain/retire 或 ScatterTerminalRecovery upsert`，recovery 必须在三条现有 terminal producer 后消费 marker，owner trigger/natural/early-depleted 必须各自 drain→zero proof→retire→owner removal。fixture 分别把四条 retirement seam 或 recovery consumer 替换为 typed no-op（保留函数签名、事件/resource 与 schedule 可编译可加载），再要求 guard 自身产生对应 `SCATTER_ACCOUNT_TERMINAL_LEAK` 子码；普通 compile failure、零测试或无关 panic 不算。
- [ ] 可核验 symbol：`establish_scatter_source_account`、`retire_scatter_source_account`、`ScatterSourceAccountId`、`ScatterSourceTerminal`、`ScatterTerminalRecovery`、`SCATTER_TERMINAL_PATH_MANIFEST`、`retry_scatter_terminal_recovery`、`scatter_source_establishment_is_absent_only`、`scatter_source_account_id_grammar_is_pinned`、`scatter_source_retirement_contract_is_pinned`、`scatter_terminal_recovery_state_machine_is_pinned`、`scatter_terminal_commit_boundary_is_atomic`、`scatter_terminal_anti_orphan_guard_is_loaded`、`scatter_source_accounts_do_not_accumulate`、`scatter_terminal_paths_remove_zero_source_accounts`、`scatter_terminal_residual_is_transferred_before_retirement`、`scatter_terminal_failure_records_retry_owner_without_qi_mutation`、`scatter_cleanup_preserves_overflow_and_transfer_audit`、`scatter_ledger_hash_omits_retired_source_fields`（名称可等义）。

**测试声明**：focused `cd server && cargo test zhenfa::tests::scatter`（或实际列出上述测试的过滤器，禁止零测试假绿）；最终 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。本地严禁运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite；关停覆盖只留给 GitHub e2e。

## 范围边界与相邻 owner

- `docs/plans-skeleton/plan-bughunt-scatter-bead-burial-restart-loss-v1.md` 独占 burial persistence table/slice、owner 跨重启 identity、`next_id`、hydrate/replay、dirty save 与 shutdown flush；本 plan 不读取/写入该 persistence schema，也不把动态 scatter account 加入 runtime whitelist。
- 两 plan 的接缝固定为：未来 hydrate 先恢复 burial/source；本 plan 的 cleanup 只在恢复后的 terminal release 成功且 source 精确归零时发生。不得用 cleanup 删除“尚未 hydrate 的未知账户”或替 persistence plan 消项。
- 本 plan 为已消费即时 item 增加的 `ScatterTerminalRecovery`（名称可等义）仅是同一 server 进程内的 runtime retry owner：不持久化、不 hydrate/replay、不回补 inventory，也不替代 restart-loss plan。若进程在 marker 未结算时退出，跨重启恢复仍属于 `plan-bughunt-scatter-bead-burial-restart-loss-v1` 或独立 transaction finding；本 plan 不冒充 durable guarantee。
- 不修 `handle_scatter_bead_use` 已消费 item 后 release/init 失败的 inventory rollback；该事务问题若成立需独立 canonical finding。
- 不重构 `WorldQiAccount`、`summarize_world_qi`、per-account telemetry schema、transfer history retention 或所有 ephemeral account；只处理两个散灵珠 source namespace。
- 不改散灵珠 capacity、`EmbeddedTrap` 逸散公式、zone cap、disturbance tag、VFX/audio/narration、owner 权限或 client/proto。

## §8.1 决议（骨架实施合同，`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`）

1. **两 namespace 均纳入**：`server/src/zhenfa/mod.rs:2521-2530,2547-2559` 证明 buried 与 active source 由同一散灵珠生命周期生产；对应 Canonical Finding Mapping / P1-A/P1-B/P1-C，两个 namespace 同属 r9 #1，不增加 finding。
2. **epsilon 正额不销毁**：`server/src/qi_physics/constants.rs:52,122` 与 `server/src/qi_physics/ledger.rs:398-454` 只提供 finite balance/transfer，不授权 epsilon 擦账；对应 P0 retirement/P2 boundary，任意 `(0,+∞)` finite residual 均先经 canonical transfer，只有 exact `0.0` 可删。
3. **absent fail-closed**：`WorldQiAccount::balance` 当前把 absent 映射成 `0.0`，但 `has_account` 可区分（`server/src/qi_physics/ledger.rs:408-414`）；对应 P0 source 三态，terminal absent 稳定报 `SCATTER_ACCOUNT_MISSING`，只有更上层确认 owner state 本就不存在的重复请求才能在进入 terminal drain 前 no-op。
4. **提交顺序固定**：现有 helper 在 `server/src/zhenfa/mod.rs:2400-2448` 存在 zone/accepted/overflow 部分提交窗口；对应 P0 commit barrier/P2 fault matrix，最终序列固定为 full preflight → accepted/overflow canonical transfer commit → source exact-zero proof → retirement → burial/marker removal。实现可选择全量 preflight 使 commit 后无 fallible branch，或显式 rollback；两者都必须通过相同快照矩阵，不新建通用 ledger transaction framework。
5. **active establishment + runtime retry owner**：`handle_scatter_bead_use` 当前先消费 item（`server/src/zhenfa/mod.rs:2508-2517`）再调用会合成 source 的 helper（`:2547-2565`）；对应 P1-A/P2，拆成 absent-only typed source establishment 与不合成 source 的 terminal drain。source 成功建立后的失败由唯一 `ScatterTerminalRecovery` Resource 按 source key 持有，`handle_scatter_bead_use` 写入，`retry_scatter_terminal_recovery` 在 `zhenfa::register` Runtime chain 中位于 active/owner/natural 三个 producer 之后消费；状态机、预算、成功清除与耗尽保留按 P0/P2 固定。marker 不持久化、不 hydrate/replay、不回补 inventory。
6. **与 restart-loss 串行**：`ScatterBeadBurials` 当前仅 runtime resource（`server/src/zhenfa/mod.rs:597-603`），重启恢复由 sibling `plan-bughunt-scatter-bead-burial-restart-loss-v1` 独占；对应范围边界。本 cleanup 可先实现，但 future hydrate 必须先恢复 owner/source，再复用同一 typed terminal helper；本 plan 不把 marker/source 加入 persistence whitelist，也不把未 hydrate unknown account 当 zombie 删除。

以上六项是本 skeleton 的确定实施合同，不再保留为推荐分支；active promotion 时仅允许补 current `file:line` 漂移，不得重新扩大 delivery scope。PR body 必须保持相同语义：不得宣称 durable persistence、inventory rollback 或通用 ledger cleanup。
