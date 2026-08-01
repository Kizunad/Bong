# plan-bughunt-scatter-bead-ledger-account-cleanup-v1（骨架）

> **骨架（草案）**。一句话主题：只关闭 bughunt r9 #1——散灵珠 terminal 成功后，按既有 `QiTransfer` 守恒释放两个 `qi_scatter:*` / `qi_scatter_buried:*` 临时 source 的实际余额，并仅在其精确归零后删除该 source key，阻止 `WorldQiAccount` 与 `bong:qi/ledger` 长跑积累僵尸键。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | typed source、full-preflight terminal commit 与 exact-zero retirement 合同 | ⬜ |
| P1 | 主动使用、owner trigger、自然终结三条 production terminal path 接线 | ⬜ |
| P2 | 真实 Runtime 路径、提交前拒绝、分流审计与长跑 cardinality 回归 | ⬜ |

## 接入面

- **进料**：现有 `ScatterBeadUseRequest` 经 `server/src/network/client_request_handler.rs:1687-1715` 进入 `server/src/zhenfa/mod.rs`；`handle_scatter_bead_use` 产生即时 source 或 `ScatterBeadBurials`。`handle_scatter_bead_trigger_requests` 当前只由 `server/src/zhenfa/mod.rs:5562,5576` 的测试直接注入 `ScatterBeadTriggerRequest`，而现有 client dispatch 只有 `ClientRequestV1::QiScatterBeadUse → ScatterBeadUseRequest`；因此 P1-B 明确把真实 client/protocol producer 与 dispatch 缺口纳入本 plan。`tick_scatter_bead_excretion` 处理自然逸散与 early-depleted。
- **出料**：只复用 `release_scatter_qi_to_zone` / `qi_release_to_zone` / `WorldQiAccount::transfer` 向 zone 与 `QiAccountId::overflow` 转移真元；本 plan 唯一新增的账户操作是 terminal 成功后对**已存在且精确为 `0.0`**的临时 source 调用 `WorldQiAccount::remove_balance`。不删 zone、overflow 或 `WorldQiAccount::transfers` audit。
- **共享类型 / event**：复用 `WorldQiAccount`、`QiAccountId`、`QiTransfer`、`QiTransferReason::ReleaseToZone`、`QI_EPSILON`、`QI_SCATTER_BEAD_CAPACITY`、`ScatterBeadBurials`、`ScatterBeadUseRequest`、`ScatterBeadTriggerRequest` 与既有 `CombatClock`/`ZhenfaSystemSet::Runtime`；P1-B 只补 owner-trigger 所需的真实 client/protocol producer 与 dispatch，不新增 ledger、epsilon、transfer event、tick resource、receipt、diagnostic bus 或平行账户表。
- **跨仓库契约**：以 server runtime lifecycle cleanup 为主；P1-B 的唯一跨层例外限于「范围边界与相邻 owner」中逐项列明的 owner-trigger client/protocol C2S producer surface。除该例外外，不改 agent Redis、VFX/audio/narration、client/proto 或无关请求形状。
- **worldview 锚点**：`worldview.md §五 L417-L421`（环境诡雷无人触发仍会随载体逸散）、`§五 L457-L465`（阵法主轴是真元逆逸散效率）、`§二 L30-L46`（灵压与环境交换）。
- **qi_physics 锚点**：`server/src/qi_physics/ledger.rs:390-480` 是账户唯一权威，`server/src/qi_physics/constants.rs:52,122` 提供 capacity/epsilon。对每个 source，`ledger.balance(source) + Σ(source 发出的 ReleaseToZone transfer.amount) == QI_SCATTER_BEAD_CAPACITY`；terminal commit 仅转出其当时实际余额，成功后 `ledger.balance(source) == 0.0` 且 `!ledger.has_account(source)`。禁止用 `set_balance(source, 0.0)` 擦账或把 cleanup 冒充 transfer。

## Canonical Finding Mapping（本 plan 的全部 delivery scope）

| Canonical finding | 本 plan 覆盖 | 明确不覆盖 |
|---|---|---|
| `docs/finished_plans/plan-bughunt-r9-findings-v1.md` r9 #1 / Finding Mapping `#1 scatter-bead ledger zombie` | 即时 `qi_scatter:*` 与预埋 `qi_scatter_buried:*` source 在 active-use、owner-trigger、自然终结及 early-depleted 成功 terminal 后的守恒转出、exact-zero retirement 与直接回归 | active-use inventory rollback、失败后 runtime retry/recovery、attempt/completion receipt、anti-orphan guard、mutation fixture/manifest、持久化/重启恢复、通用 ledger transaction/diagnostic redesign、transfer history compaction、其他账户 namespace |

计数固定为 **1 条 canonical finding**。两个 namespace 是同一散灵珠 source 生命周期的两种形态；自然终结与 early-depleted 是同一自然路径的两个 terminal branch，不增加 finding。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-30）

1. `zhenfa::register` 将 `handle_scatter_bead_use → handle_scatter_bead_trigger_requests → tick_scatter_bead_excretion` 加入同一 `Update` / `ZhenfaSystemSet::Runtime` chain（`server/src/zhenfa/mod.rs:597-635`）。
2. `release_scatter_qi_to_zone`（`:2366-2449`）可对 zone 与 overflow 分别创建 `QiTransfer`；`WorldQiAccount::transfer`（`server/src/qi_physics/ledger.rs:416-454`）把 source 写为零但不删除 key。
3. 即时成功链（`zhenfa/mod.rs:2547-2586`）、owner-trigger 成功链（`:2614-2647`）和自然耗尽移除链（`:2663-2733`）均未调用 `remove_balance`；自然 tick 开头 `remaining_qi <= QI_EPSILON` 还会直接删除 burial。
4. `WorldQiAccount::iter_balances` / `total`（`server/src/qi_physics/ledger.rs:457-480`）与 `build_qi_ledger_hash_fields`（`server/src/qi_physics/ledger.rs:668-704`）保留并发布每个 zero source key；item instance / bead ID 单调增长，故临时 source cardinality 无界增长。
5. 现有生产测试只断言 source `balance <= QI_EPSILON` 或 burial 删除，没有断言 `!ledger.has_account(source)`；zero-key 回归可静默通过。

## P0 — typed source 与 terminal commit 合同

- [ ] 在 `server/src/zhenfa/mod.rs` 增加 focused local type，例如 `ScatterSourceAccountId`（名称可等义），以 `canonical_player_id`、active item instance ID 或 buried bead ID 构造唯一的 `QiAccountId::container`：即时形状为 `qi_scatter:<canonical_player_id>:<canonical_u64_item_instance>`，预埋形状为 `qi_scatter_buried:<canonical_player_id>:<canonical_u64_bead_id>`。terminal path 只能从已验证的 request 或 burial 记录构造该 type，不接受裸字符串、其他 account kind 或其他 namespace。
- [ ] 增加 focused `retire_scatter_source_account`（名称可等义）：仅当 `ledger.has_account(source)` 且余额为可接受的 exact `0.0` 时调用 `remove_balance`；missing 或任意正余额均保持 account map 不变并拒绝 retirement。若 source balance 为 `NaN`、`+∞` 或 `-∞`，也必须 fail closed、保留 source key、不得删除或把它改写成零；该状态可由 `WorldQiAccount::transfer` 的 `to_balance + amount` 溢出形成（`server/src/qi_physics/ledger.rs:438,450-452`），因此属于本 retirement contract 的可达边界。
- [ ] **冻结唯一提交策略：full preflight → infallible commit，不采用 rollback。** terminal helper 在第一个写操作前必须：读取实际 source balance、验证 source 存在及有限非负、解析 zone、计算 accepted/overflow、构造所有非零 `QiTransfer`，并验证其金额之和恰等于 source balance。preflight 任一拒绝必须在任何 source、burial、zone、overflow 或 transfer history 写入前返回。通过后 commit 不得再保留可恢复的失败分支：按预先验证的数据写入现有 canonical transfer、更新已计算的 zone 值、证明 source 精确为零、再 retirement；若既有 API 不能在此约束下形成无失败 commit，实施必须停止并另立 transaction finding，不得在本 plan 引入 rollback journal 或 recovery framework。
- [ ] transfer/audit 计数固定按**非零目标**而非按 terminal 次数：accepted `> 0` 时恰有一条 `source → zone` 的 `ReleaseToZone` transfer/audit；overflow `> 0` 时恰有一条 `source → overflow` 的 `ReleaseToZone` transfer/audit；零额不建 transfer/audit。accepted-only、overflow-only、accepted+overflow 分流分别为 1、1、2 条，成功 retirement 不新增 audit；同一 terminal commit 不得重复任一 source-target transfer。
- [ ] `scatter_source_retirement_contract_is_pinned`（名称可等义）覆盖两种 typed constructor 的 canonical key、exact-zero 删除、missing 保留、正余额（含 `next_down(QI_EPSILON)`、`QI_EPSILON`、`next_up(QI_EPSILON)`）保留，以及 `NaN`、`+∞`、`-∞` 等由 `WorldQiAccount::transfer` 溢出可达的非有限余额 fail-closed 保留；同时锁定 `set_balance(source, 0.0)` 不是合法 retirement 替代。每条拒绝只断言可观察账户、burial、zone、overflow 与 transfer history 不变，不要求新的跨路径 diagnostic 协议。
- [ ] `scatter_terminal_preflight_is_atomic`（名称可等义）只在第一个写操作**之前**注入 source missing、非有限 source、zone missing、余额不足、transfer 构造拒绝等真实 preflight failure；每个 case 断言 source、burial、zone、overflow、完整 transfer history 均零差分。该测试不得要求 accepted/overflow 已写入后的失败回滚；另以 accepted-only、overflow-only、split 三种成功 case 锁定 P0 的精确 transfer/audit 数量与 exact-zero retirement。

## P1 — 三条 production terminal path 接线

### P1-A — 即时主动破裂

- [ ] `handle_scatter_bead_use` 在既有 `consume_item_instance_once` 与既有 source establishment 成功后，从请求 owner / item instance 构造 typed active source，并调用 P0 terminal helper。只有 full-preflight + commit + exact-zero retirement 都完成时才视为 active terminal 成功；不得删除正余额或未存在 key。
- [ ] 本 finding 不改已消费 item 的 rollback，也不增加 `ScatterTerminalRecovery` 或 retry owner。若 active-use preflight/commit 不能被现有局部状态安全表示，保留现有 owner state 并把事务问题作为独立 finding，不得把 receipt、marker 或 background retry 混入本 lifecycle cleanup。

### P1-B — owner trigger

- [ ] 先补齐真实 owner-trigger producer：为现有 client/protocol 请求链新增与 `ScatterBeadTriggerRequest` 对应的明确 client dispatch、wire/schema 映射和 server `ClientRequestV1` handler，使生产代码能够从 owner 的触发动作构造该 request；不能继续把 `server/src/zhenfa/mod.rs:5562,5576` 的测试注入当作 production entry point。保持既有请求校验、Entity owner gate 与无裸字符串 source 构造约束，不扩展到无关协议。
- [ ] 真实 producer 接线后，`handle_scatter_bead_trigger_requests` 在既有 `ScatterBeadBurials::trigger_buried` Entity owner gate 成功后，从取出的 burial 的 canonical owner / bead ID 构造 typed buried source；只在 P0 terminal helper 成功 retirement 后删除 burial。client/protocol producer 缺失、zone/source preflight 拒绝或 terminal commit 失败时必须保留 burial/source，不能把测试 fixture 视为生产成功。

### P1-C — 自然逸散与 early-depleted

- [ ] `tick_scatter_bead_excretion` 的非 terminal natural tick 继续只按 `qi_excretion` 得到的 `leaked` 走既有 canonical release，并保留正余额 source / burial；不得把它误当 cleanup sweep。
- [ ] 当 natural tick 算出 `remaining_after <= QI_EPSILON` 时，不得先完成 ordinary partial release 再删除 burial；必须以 typed buried source 的**实际 ledger balance**走一次 P0 terminal helper，成功 zero proof + retirement 后才移除 burial。这样 `(0, QI_EPSILON]` residual 也会经 canonical transfer 而非静默丢弃。
- [ ] tick 开始即 `remaining_qi <= QI_EPSILON` 的 early-depleted branch 不得直接 push `depleted`：actual source balance 为正时走 P0 terminal helper，为 exact zero 时走 P0 retirement；source missing 或 terminal preflight 拒绝时保留 burial/source，只有 retirement 成功后才删除 burial。
- [ ] 不新增平行 cleanup sweep；terminal retirement 紧邻上述 owner path，仍由 `zhenfa::register` 的既有 `ZhenfaSystemSet::Runtime` chain 驱动。

## P2 — 饱和验收

- [ ] production App 从真实 `zhenfa::register` 驱动 `Update`，覆盖 active-use、真实 client/protocol owner-trigger、natural final 与 early-depleted 四个成功 terminal branch；owner-trigger 测试必须经过 production producer/dispatch，而非直接注入 `ScatterBeadTriggerRequest`。每个 branch 断言 `!ledger.has_account(source)`、burial 仅在其对应成功路径删除、zone/overflow 与 `WorldQiAccount::transfers` 保留，且 source 生命周期守恒式闭合。
- [ ] 将 accepted-only、overflow-only、accepted+overflow 三种 terminal commit 分开断言：分别精确新增 1、1、2 条 source-originated `ReleaseToZone` audit，目标与金额正确，source exact-zero 后才从 `iter_balances` 消失；失败 preflight 追加 0 条 audit。
- [ ] 覆盖 source absence、exact zero、`next_down(QI_EPSILON)`、`QI_EPSILON`、`next_up(QI_EPSILON)`、partial residual、full capacity，以及 `transfer` 的 `to_balance + amount` 溢出形成 `NaN` / `+∞` / `-∞` source balance；任意正额或非有限额均 fail closed，不删除 key、不伪造 zero transfer，任意正额先 canonical transfer 后删除 key，直接 retirement 一律保留 key/余额。
- [ ] `scatter_source_accounts_do_not_accumulate`（名称可等义）在同一 App 执行足量即时与预埋完整生命周期，针对 `qi_scatter:*` / `qi_scatter_buried:*` 断言每个已终结 key 不在 `iter_balances`，两个 namespace 的 live key 数不随已终结珠子增长；overflow account 与 transfer history 属守恒审计，不计入 source cardinality。
- [ ] telemetry pin：`server/src/qi_physics/ledger.rs:668-704` 的 `build_qi_ledger_hash_fields` 不再含已 retire source 的 `account:container:qi_scatter*` 字段，活跃 source 与 overflow 字段仍存在；只删除 zero key 前后 `WorldQiAccount::total()` 精确不变。
- [ ] 可核验 symbol：`ScatterSourceAccountId`、`retire_scatter_source_account`、`scatter_source_retirement_contract_is_pinned`、`scatter_terminal_preflight_is_atomic`、`scatter_terminal_paths_remove_zero_source_accounts`、`scatter_terminal_residual_is_transferred_before_retirement`、`scatter_source_accounts_do_not_accumulate`、`scatter_cleanup_preserves_overflow_and_transfer_audit`、`scatter_ledger_hash_omits_retired_source_fields`（名称可等义）。

**测试声明**：focused `cd server && cargo test zhenfa::tests::scatter`（或实际列出上述测试的过滤器，禁止零测试假绿）；最终 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。本地严禁运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite；关停覆盖只留给 GitHub e2e。

## 范围边界与相邻 owner

- `docs/plans-skeleton/plan-bughunt-scatter-bead-burial-restart-loss-v1.md` 独占 burial persistence、跨重启 identity、hydrate/replay、`next_id` 与 shutdown flush；本 plan 不读写其 persistence schema。
- 不修 active-use 已消费 item 后的 inventory rollback，也不为 failed terminal 加 runtime recovery、retry budget、attempt/completion receipt、anti-orphan guard、diagnostic event/resource、mutation fixture 或 terminal manifest。它们不是 r9 #1 的 zero-key retirement 交付；如第一性验真证明局部 full-preflight 无法保证提交前拒绝，必须另立 finding，而非扩张本 plan。
- 不重构 `WorldQiAccount`、`summarize_world_qi`、per-account telemetry schema、transfer history retention 或所有 ephemeral account；只处理两个散灵珠 source namespace。
- P1-B 是本 plan 唯一允许改变的 client/proto surface，且仅为 owner-trigger 的 C2S producer 闭环：`client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java` 的 owner 触发交互入口、`client/src/main/java/com/bong/client/network/ClientRequestProtocol.java` 的编码、`ClientRequestSender.java` 的发送、`proto/bong/envelope.proto` 的 request envelope/message、`server/src/schema/client_request.rs` 的 `ClientRequestV1` variant、`server/src/schema/proto_convert.rs` 的双向 payload conversion，以及 `server/src/network/client_request_handler.rs` 的 `DispatchResources` event writer 与 dispatch arm。实现必须让该 C2S 链实际发出 `ScatterBeadTriggerRequest`，并以 client/protocol → handler → `zhenfa::register` Runtime 的真实路径覆盖 P2；不得借此改其他请求、agent Redis、VFX/audio/narration。
- 不改散灵珠 capacity、`EmbeddedTrap` 逸散公式、zone cap、disturbance tag、无关的 client/proto 请求、owner 权限或 client/proto 的任何其他 surface。

## §8.1 决议（骨架实施合同，`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`）

1. **两 namespace 均纳入**：`server/src/zhenfa/mod.rs:2521-2530,2547-2559` 证明 buried 与 active source 均由散灵珠生命周期生产；对应 P1-A/P1-B/P1-C，同属 r9 #1。
2. **exact-zero 是唯一删除门**：`WorldQiAccount::transfer` 只写 zero（`server/src/qi_physics/ledger.rs:448-453`），`remove_balance` 是独立删除 API（`:404-405`）；对应 P0，任何非零 residual 先 transfer，不能 epsilon 擦账。
3. **full-preflight 唯一策略**：现有 helper 在 `server/src/zhenfa/mod.rs:2400-2440` 有部分提交窗口；对应 P0/P2，实施必须把全部可失败验证移到首个写前，并让 commit 无 recoverable failure，不实现 rollback 双轨。
4. **audit 按目标计数**：同一 source 的 zone 与 overflow 是不同 target；对应 P0/P2，非零 zone / overflow 各一条 audit，split 恰两条，零额零条，retirement 无 audit。
5. **terminal 与非 terminal 分开**：`tick_scatter_bead_excretion` 的 ordinary leak 不做 cleanup；对应 P1-C，只有最终 residual / early-depleted 进入 terminal helper，成功 retirement 后才删除 burial。
6. **失败事务不借题扩张**：r9 #1 的证据是 terminal 后遗留 zero key（`docs/finished_plans/plan-bughunt-r9-findings-v1.md:28-30,61,71`）；对应范围边界，recovery、receipt、guard、retry 与 inventory rollback 另属独立 transaction/observability 问题，不在本 skeleton 交付。

以上六项是本 skeleton 的确定实施合同；active promotion 时仅允许补 current `file:line` 漂移，不得重新扩大 delivery scope。PR body 必须保持相同语义：不得宣称 durable persistence、inventory rollback、runtime recovery 或通用 ledger cleanup。
