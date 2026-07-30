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

- [ ] 在 `server/src/zhenfa/mod.rs` 增加 focused typed helper，例如 `ScatterSourceAccountId` / `ScatterSourceTerminal` 与 `retire_scatter_source_account`（名称可等义）。helper 只接受 `QiAccountKind::Container` 且 ID 精确属于 `qi_scatter:` 或 `qi_scatter_buried:` namespace；zone、overflow、其他 container 账户一律稳定拒绝。
- [ ] retirement 必须区分 `present-zero`、`absent` 与 `present-positive`：只有**存在且 balance 精确为 0.0**的 owned source 可以 `remove_balance`；absent 返回 `SCATTER_ACCOUNT_MISSING`，任意正余额（包括 `next_down(QI_EPSILON)`、`QI_EPSILON`、`next_up(QI_EPSILON)`）返回 `SCATTER_ACCOUNT_RESIDUAL_QI` 并保留原 key/余额。禁止以 epsilon 为借口直接丢弃真元。
- [ ] terminal drain 必须先完成可核验的 preflight，再以现有 canonical `QiTransfer` 将有限正额完整转入 accepted + overflow，使 source 由真实 transfer 到达精确 0，再调用 retirement；若现有 `release_scatter_qi_to_zone` 会先写 zone/source/event、再因 overflow 失败返回，则实现必须在本 plan 内定义并测试可观察的 preflight/rollback/commit barrier，使失败时 source、burial、zone、overflow、transfer history 零新增副作用；不得只新增断言，也不得复制释放公式或直接改 zone/overflow 余额。对已消费即时 item 的失败例外只能是同一 runtime terminal recovery marker 的创建/更新（不改 qi 状态），该 marker 记录 owner/source/位置/overflow key 并允许同一进程重试；它不是 inventory rollback、burial persistence、hydrate/replay 或通用事务框架。
- [ ] `release_scatter_qi_to_zone` 不得接受 caller-supplied balance 作为无条件 `set_balance` 覆盖，也不得兼任 source establishment。terminal path 必须先读取 ledger 当前 source 三态：missing fail-closed、present-positive 只能按实际余额 release、present-zero 只有在无需转账时允许 retirement；已有 residual 与参数不一致、source missing 被“现造”或任何中途 transfer 失败均由 `SCATTER_ACCOUNT_MISSING` / `SCATTER_ACCOUNT_RELEASE_FAILED` 稳定 diagnostic 拒绝，并保留可重试 owner state。即时 source 的首次 absent→capacity 建立必须走 P1-A 独立 typed seam；预埋 source 仍由 burial 创建链建立。
- [ ] `scatter_source_retirement_contract_is_pinned`（名称可等义）覆盖两个合法 namespace、其他 account kind/prefix、present-zero、absent、正余额与 epsilon 相邻可表示值；每个失败 case 同时断言 account map、余额、transfer history、zone/overflow 与 burial 均不变。
- [ ] 若采用 negative fixture / mutation gate，fixture 必须先证明仍可编译并加载 production schedule，再断言上述 guard **自身的稳定 diagnostic**；普通 rustc/解析/链接/无关测试失败不算 guard 命中。

## P1 — 三条 production terminal path 接线

### P1-A — 即时主动破裂

- [ ] `handle_scatter_bead_use` 在既有 item 消费成功后，必须先经独立、typed、只允许 absent→`QI_SCATTER_BEAD_CAPACITY` 的 source-establishment seam 建立 `qi_scatter:{owner_player_id}:{item_instance_id}`，再调用不得合成账户的 terminal drain；release 成功且 accepted + overflow 已真实落账后才 retire source。source establishment 失败属于既有 inventory transaction 缺口，不得伪造 source 或 cleanup；source 已建立后的 release/retirement 失败则必须创建或更新上述 runtime terminal recovery marker（marker 本身不改 qi 状态），不得留下无 retry owner state 的 orphan source。inventory rollback 不是本 plan 的实现路径，不能以隐式旁路替代 marker。
- [ ] **P1-A recovery marker contract**：marker 只允许 owner 已消费且 source 已成功建立的 terminal；保存 owner/source/pos/overflow key/失败 diagnostic，并由既有 `ZhenfaSystemSet::Runtime` 在有限重试窗口重放 terminal drain。marker 不改 inventory、burial persistence、hydrate/replay、ledger balance 或 transfer history；重试成功后按同一 release → zero proof → retirement → marker removal 顺序提交，耗尽窗口则保留 marker 并以 `SCATTER_ACCOUNT_RELEASE_FAILED` 可观测失败，不得静默丢 source。
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
- [ ] `release_scatter_qi_to_zone`、source-establishment seam、retirement guard 与 runtime terminal recovery marker 的失败表必须统一稳定 diagnostic：`SCATTER_ACCOUNT_MISSING`（terminal drain source absent/未 hydrate）、`SCATTER_ACCOUNT_ALREADY_EXISTS`（source establishment 发现同 ID 既有账户，禁止覆盖/重复注资）、`SCATTER_ACCOUNT_RESIDUAL_QI`（正余额不可删）、`SCATTER_ACCOUNT_NAMESPACE`（非 owned container/prefix）、`SCATTER_ACCOUNT_RELEASE_FAILED`（preflight 或 canonical transfer 拒绝）；terminal wiring 缺失必须由 `SCATTER_ACCOUNT_TERMINAL_LEAK` guard 自身报错。每个失败 fixture 先证明可编译、可加载并注册真实 `ZhenfaSystemSet::Runtime`，再断言 diagnostic 与 source/burial/zone/overflow/transfers 零新增副作用；runtime marker 只允许记录可重试 terminal metadata，不得改变这些 ledger 状态。
- [ ] long-run `scatter_source_accounts_do_not_accumulate`（名称可等义）在同一 App 驱动足量即时与预埋生命周期，针对 `qi_scatter:*` 与 `qi_scatter_buried:*` 两个临时 source namespace，断言每个已终结 source key 不在 `iter_balances`，且这两个 namespace 的 live/terminal key 数不随已终结珠子增长；不得把 `QiAccountId::overflow(...)` 或 `WorldQiAccount::transfers` audit history 计入 source cardinality。tracked zone/overflow 余额与 transfer history 保留。
- [ ] telemetry pin：`build_qi_ledger_hash_fields` 不再含已 retire source 的 `account:container:qi_scatter*` 字段，活跃未耗尽 burial 与 overflow 字段仍存在。总账 cleanup 前后（只删除 zero key）`WorldQiAccount::total()` 精确不变。
- [ ] production loading test 从 `zhenfa::register` 建 App 并驱动 `Update`，不得只直接调用 private helper；若删掉任一 terminal-path retirement wiring，fixture 保持可编译且必须由 `SCATTER_ACCOUNT_TERMINAL_LEAK`（名称可等义）的 guard diagnostic 失败，普通 compile failure 不算。
- [ ] 可核验 symbol：`establish_scatter_source_account`、`retire_scatter_source_account`、`ScatterSourceAccountId`、`ScatterSourceTerminal`、`ScatterTerminalRecovery`、`retry_scatter_terminal_recovery`、`scatter_source_establishment_is_absent_only`、`scatter_source_retirement_contract_is_pinned`、`scatter_source_accounts_do_not_accumulate`、`scatter_terminal_paths_remove_zero_source_accounts`、`scatter_terminal_residual_is_transferred_before_retirement`、`scatter_terminal_failure_records_retry_owner_without_qi_mutation`、`scatter_cleanup_preserves_overflow_and_transfer_audit`、`scatter_ledger_hash_omits_retired_source_fields`（名称可等义）。

**测试声明**：focused `cd server && cargo test zhenfa::tests::scatter`（或实际列出上述测试的过滤器，禁止零测试假绿）；最终 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。本地严禁运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite；关停覆盖只留给 GitHub e2e。

## 范围边界与相邻 owner

- `docs/plans-skeleton/plan-bughunt-scatter-bead-burial-restart-loss-v1.md` 独占 burial persistence table/slice、owner 跨重启 identity、`next_id`、hydrate/replay、dirty save 与 shutdown flush；本 plan 不读取/写入该 persistence schema，也不把动态 scatter account 加入 runtime whitelist。
- 两 plan 的接缝固定为：未来 hydrate 先恢复 burial/source；本 plan 的 cleanup 只在恢复后的 terminal release 成功且 source 精确归零时发生。不得用 cleanup 删除“尚未 hydrate 的未知账户”或替 persistence plan 消项。
- 本 plan 为已消费即时 item 增加的 `ScatterTerminalRecovery`（名称可等义）仅是同一 server 进程内的 runtime retry owner：不持久化、不 hydrate/replay、不回补 inventory，也不替代 restart-loss plan。若进程在 marker 未结算时退出，跨重启恢复仍属于 `plan-bughunt-scatter-bead-burial-restart-loss-v1` 或独立 transaction finding；本 plan 不冒充 durable guarantee。
- 不修 `handle_scatter_bead_use` 已消费 item 后 release/init 失败的 inventory rollback；该事务问题若成立需独立 canonical finding。
- 不重构 `WorldQiAccount`、`summarize_world_qi`、per-account telemetry schema、transfer history retention 或所有 ephemeral account；只处理两个散灵珠 source namespace。
- 不改散灵珠 capacity、`EmbeddedTrap` 逸散公式、zone cap、disturbance tag、VFX/audio/narration、owner 权限或 client/proto。

## §8 开放问题（P0 决策门前需收口）

1. **即时 source 是否纳入？** 推荐纳入：`qi_scatter:*` 与 buried source 由同一 helper 创建并具有同一 terminal-only ephemeral ownership；PR body 与本 plan 必须都写明包含两 namespace，不能只在实现中顺手扩大。
2. **epsilon residual 如何终结？** 推荐先经 canonical release 转走全部有限正额，再只删除 exact-zero key；禁止把 `<= QI_EPSILON` 当可销毁额度。
3. **absent source 是否幂等成功？** 推荐 fail-closed `SCATTER_ACCOUNT_MISSING` 并保留 burial；只有更上层已经确认 terminal owner state 不存在的重复请求才 no-op，不能让 absent 掩盖未 hydrate/漏建账户。
4. **成功提交顺序？** 推荐 release/overflow 全成功 → source-zero guard → remove source → remove burial；任一步失败保留可重试 owner state。本 plan 不为既有 release helper 新建通用事务框架。
5. **即时 source 如何建立且失败后由谁重试？** 推荐把 source establishment 与 terminal drain 分开：前者只允许 absent→capacity，existing/mismatch fail-closed 且不覆盖；source 成功建立后若 terminal 失败，增加只在同一 server 进程内存在的 `ScatterTerminalRecovery` runtime marker，保存 owner/source/pos/overflow key/diagnostic 并由既有 Runtime chain 有限重试。marker 不改 inventory/qi、不持久化、不 hydrate/replay。跨重启未结算 marker 不由本 plan 保证。
6. **如何与 restart-loss plan 串行？** 推荐 cleanup 可先实现，但 persistence plan 实施时必须复用本 helper，并以 hydrate 完成作为 cleanup schedule 前置；两 plan 不得各自实现不同 source retirement。

> 六项须在 active plan 的 `§8.1 决议` 以 current `file:line + plan 章节` 双锚点收口后才能进入 P0。PR body 的 delivery scope 必须逐字语义对齐本文件的 Canonical Finding Mapping 与范围边界；body 不得宣称交付 durable persistence、inventory rollback 或通用 ledger cleanup。
