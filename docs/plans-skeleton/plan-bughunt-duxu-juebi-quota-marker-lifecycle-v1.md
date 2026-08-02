# plan-bughunt-duxu-juebi-quota-marker-lifecycle-v1（骨架）

> **GOAL**：修复 r6 #6 的同实体 ECS stale-positive：`JueBiAfterDuXuQuota` 不得从已结束的 DuXu 遗留并改变后续 DuXu；合法的 DuXu → quota-origin JueBi 连续过渡必须保留该 marker。
>
> **Canonical owner**：`docs/finished_plans/plan-bughunt-r6-findings-v1.md:53-61` Finding Mapping #6。当前 `origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14` 仍可复现；PR #1304 只完成 finding 分流，未修代码。
>
> **Scope lock（2026-08-01）**：本 successor 只修 live ECS 上 marker 的 create / replace / retain / remove 生命周期、r6 #6 的四条 DuXu 终态路径，以及直接承接 marker 的 quota-origin JueBi 在既有 failure / fled / disconnect / escape 非-settlement 终态中的 cleanup；不新建 attempt admission、allocator、attempt identity、持久化事务、SQLite schema 或 restart/hydration 契约。
>
> **Delivery**：按根 `CLAUDE.md` BugFix 工作流，一个 skeleton = 一个修复 subagent = 一个常驻 slot = 一个 PR；不由 `/consume-plan` 消费。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | live `JueBiAfterDuXuQuota` ownership 与 terminal cleanup | ⬜ |
| P1 | r6 #6 生产调度回归矩阵与完整 server gate | ⬜ |

## 接入面

- **进料**：`server/src/cultivation/tribulation.rs:860-1023` 的 `start_tribulation_system` 保留既有 eligibility、same-tick dedupe、quota 与 `persist_active_state` 顺序；只在该既有 persistence guard 成功后变更 marker。
- **出料**：`tribulation_wave_system`（`:3166-3329`）读取 marker 追加 quota-origin JueBi；`juebi_settlement_system`（`:1900-2081`）清理直接承接的 marker；DuXu normal completion、failure、fled/disconnect、intercept death 分别在 `:3216-3310`、`:3334-3428`、`:3433-3670`、`:3673-3749` 终止 live state。quota-origin JueBi 的 generic wave failure 同样进入 `tribulation_failure_system`（`tribulation_aoe_system` 不筛 `state.kind`，`:1367-1479`），disconnect 进入 `abort_du_xu_on_client_removed`，而 boundary escape 须由 `tribulation_escape_boundary_system`（`:3498-3584`）明确放行该 source-aware JueBi 后才抵达 `settle_fled_tribulation`。
- **生产调度**：`server/src/main.rs:112` 调用 `cultivation::register`；`server/src/cultivation/mod.rs:428-469` 注册 start、wave、四条 terminal path 与 settlement 的真实顺序。P1 不得手拼另一套系统顺序。
- **共享类型 / event**：复用当前 `JueBiAfterDuXuQuota`、`TribulationState`、`JueBiRuntimeContext`、`JueBiTriggerSource::VoidQuotaExceeded`；不新增 marker token、owner phase、attempt ID 或状态机。
- **跨仓库契约**：纯 server live ECS 生命周期修复；不改 public IPC、agent/client、`ActiveTribulationRecord`、SQLite shape、migration 或 hydration。
- **worldview / qi_physics**：marker cleanup 是 ledger-neutral 的 component cleanup；不改配额公式、绝壁强度或任何真元流。

## 第一性验真

- `JueBiAfterDuXuQuota` 定义于 `server/src/cultivation/tribulation.rs:305-310`，当前只有 quota snapshot 字段，没有 attempt identity 或 owner phase。
- `start_tribulation_system` 仅在本次判定为 over-quota 时 insert marker；accepted non-over-quota DuXu 不清旧 marker（`tribulation.rs:939-987`）。
- 配额检查接收的是**新 attempt 占位前**的 `occupied_slots`：`check_void_quota` 以 `occupied_slots >= quota_limit` 表示已无剩余名额（`tribulation.rs:167-192`）。因此 `occupied_slots + 1 == quota_limit` 仍合法且必须清旧 marker；`occupied_slots == quota_limit` 的下一次 accepted DuXu 才创建/替换当前 snapshot；不得误改为严格 `>`。
- normal completion、failure、fled/disconnect 与 intercept death 都移除当前 `TribulationState`，但相应 cleanup tuple 未一致移除 marker；下一次**可再次从 `Realm::Spirit` 发起的** non-over-quota DuXu 完成最终 wave 时，`tribulation_wave_system` 可读到旧 snapshot 并伪造 JueBi（`tribulation.rs:3216-3258`）。normal ascension 会先将同一 entity 设为 `Realm::Void`（`:3277`），而 start gate 只接受 `Realm::Spirit`（`:902-908`），故 normal ascension 不存在同实体生产路径的 retry。
- 直接 quota-origin 转换是唯一允许的 retain：最终 DuXu wave 的 marker 分支把 state 改为 JueBi、写入 `JueBiRuntimeContext { source: VoidQuotaExceeded, .. }`，并让同一 component 留给 `juebi_settlement_system`（`tribulation.rs:3216-3258,2070-2080`）。但 JueBi wave 仍经未筛 kind 的 `tribulation_aoe_system` 发出 `TribulationFailed`（`:1367-1479`），随后 `tribulation_failure_system` 移除 live state（`:3334-3428`）；因此 quota-origin JueBi failure 是必须清 marker 的可达非-settlement terminal path。
- `tribulation_escape_boundary_system` 当前在 `state.kind != TribulationKind::DuXu` 时提前跳过（`:3533-3535`），故 quota-origin JueBi boundary escape 尚未抵达 shared `settle_fled_tribulation`；P0 必须只为带 `JueBiRuntimeContext { source: VoidQuotaExceeded }` 的 JueBi 放开这道 eligibility gate，保留 standalone/non-quota JueBi 的既有不处理行为。
- standalone JueBi 是既有独立生产路径：`start_due_juebi_triggers_system`（`tribulation.rs:1158-1244`）从 `JueBiTriggerEvent` 创建它；它不是 `start_tribulation_system` 的 accepted start，且自身不创建 quota marker。它不是本 successor 新增 marker policy 的对象；对“独立 JueBi 恰遇历史损坏 marker”的 source-aware settlement 分类保持现有行为并另列后续 owner，P0 不新增 generic mismatch consumer，也不把 standalone JueBi 纳入 marker cleanup assertion。
- 当前 marker 测试只 pin over-quota producer presence（如 `tribulation.rs:4405-4583,7128`），没有“终态 → 同实体 non-over-quota retry → 不追加 JueBi”的回归。

## P0 — Live marker ownership and cleanup

- [ ] 本 successor 的唯一对象是 entity 上 live `JueBiAfterDuXuQuota` component。在 P0 覆盖的正常路径中，它只在两种连续状态中合法存在：accepted over-quota DuXu，或该 DuXu 最终 wave 直接转入的 quota-origin JueBi；后者直到正常 `juebi_settlement_system` 或 quota-origin JueBi 的非-settlement terminal cleanup 才移除 marker。它不携带 attempt ID、owner phase 或跨进程身份。
- [ ] 保留 `start_tribulation_system` 既有 rejected / active / same-tick duplicate gate、quota calculation 与 `persist_active_state` guard。仅在该既有 persistence call 返回 `Ok` 后执行 marker branch：accepted over-quota DuXu 用本次 `check_void_quota` snapshot insert 或 replace；accepted non-over-quota DuXu remove 同 entity 上已有 marker。rejected、duplicate 或既有 persist-failed start 不得变更 marker；这是现有 start guard 的 marker-side consequence，不新增 admission、reservation、DB transaction 或 error taxonomy。
- [ ] quota snapshot 的唯一来源固定为 `WorldQiBudget.current_total`（`server/src/qi_physics/ledger.rs:15-57`）与 `VoidQuotaConfig::quota_k` 经 `check_void_quota`（`tribulation.rs:133-192`）生成的现有四字段。quota-origin JueBi 强度只继续调用 `juebi_intensity_for_quota_marker`（`tribulation.rs:1285-1292`），不得在本 successor 重算、复制常数或写入新的 qi_physics 公式。
- [ ] `tribulation_wave_system` 的直接 quota branch 在既有 JueBi persistence guard 成功后保留 marker 的原四字段，写入 `JueBiRuntimeContext { source: JueBiTriggerSource::VoidQuotaExceeded, .. }` 并替换 live `TribulationState` 为 JueBi；不得引入 owner-phase transfer、generic mismatch cleanup 或额外 persistence write。`juebi_settlement_system` 是这条直接 follow-up 的正常 settlement path 的 marker remover。
- [ ] 在正在终止的 live DuXu 分支的同一 component cleanup 中 remove marker：normal non-marker DuXu completion、`tribulation_failure_system`、`settle_fled_tribulation`（供 disconnect 与 escape 共用）及 `tribulation_intercept_death_system`。共享 flee helper 也可终止 JueBi，因此它必须额外识别 `state.kind == JueBi` 且现有 `JueBiRuntimeContext.source == VoidQuotaExceeded` 的 quota-origin 分支：该分支在移除 `TribulationState` 的同时 remove `JueBiAfterDuXuQuota`，覆盖 quota-origin JueBi 的 fled/disconnect/escape 非-settlement 终止。`tribulation_failure_system` 同样必须在其正在终止的 live JueBi 带该 runtime source 时 remove marker，覆盖 generic wave failure；不得只清理 DuXu failure。为使 escape 真正抵达 shared helper，`tribulation_escape_boundary_system` 的现有 DuXu-only eligibility 仅放宽为 `DuXu` 或带 `JueBiRuntimeContext.source == VoidQuotaExceeded` 的 JueBi，仍排除 standalone/non-quota-origin JueBi。每条只处理其正在终止的 live state，不为 `ZoneCollapse`、`Targeted` 或 generic mismatch consumer 新增 marker 语义。
- [ ] standalone JueBi 仍由 `start_due_juebi_triggers_system` 的既有独立 trigger path 管理，不是 `start_tribulation_system` 的 accepted start：它不 create、replace、transfer、consume 或以其他方式变更 quota marker，也不是 P0 新增 cleanup/mismatch policy 的对象。P0 的正常 live-state invariant 是 DuXu terminal path 与 quota-origin JueBi terminal path 都不会把 marker 留给后续独立 JueBi；若需修复 independent JueBi 与历史损坏 marker 的 source-aware settlement 分类，必须另立 successor，不得混入本 PR。
- [ ] component-only marker cleanup 不得新增或重复 `delete_active_tribulation`、quota mutation、penalty、Qi release、`QiTransfer`、`TribulationSettled` 或 `JueBiTriggeredEvent`。已有终态副作用保持既有一次性基线；本修复只能改变 marker presence。

## P1 — Regression closure

- [ ] P1 的跨 system lifecycle fixtures 与生产 registration 留在 `server/src/cultivation/mod.rs` 同一 module：实现抽出仅注册 `server/src/cultivation/mod.rs:428-469` tribulation tuple 的 private production helper，再令 `cultivation::register` 调用它；其 child `#[cfg(test)] mod tests` 以 `App::new()` 插入现有资源/事件后调用同一 helper，因而复用 production 的 `.after/.before` 关系，不能 duplicate 或手拼不同顺序。不得跨 module 调用 `tribulation.rs` 的 private test helper；需要成功 persistence fixture 时，在 `mod.rs` tests 使用既有 `PersistenceSettings::with_paths`。仅检查 start guard 不变性的 unit fixture 可留在 `tribulation.rs` tests：既有 private `unbootstrapped_persistence_settings` 只覆盖 persist-failed start 的 marker 不变性，不替代跨 system lifecycle 的 production-schedule regression。两处测试均不增加 allocator Resource、admission barrier、persistence fault-injection port、DB spy 或并发框架。
- [ ] `accepted_non_over_quota_start_clears_seeded_marker`：前置为 eligible、无 active DuXu 的 entity 带可区分旧 marker，且 `occupied_slots + 1 == quota_limit`；经现有成功 start path 后断言 `no TribulationState → live DuXu` 且 `old marker → absent`。此测试必须能在漏掉 remove 时失败。
- [ ] `accepted_over_quota_start_replaces_seeded_marker_with_current_snapshot`：前置为 eligible、无 active DuXu 的 entity 带可区分旧 marker，且 `occupied_slots == quota_limit`；经成功 start path 后断言 `old snapshot → check_void_quota` 的四字段当前 snapshot，并保留 live DuXu。此测试必须能区分错误保留旧 snapshot、错误清除与正确 replace。
- [ ] `rejected_duplicate_and_existing_persist_failure_leave_marker_unchanged`：分别前置已 active / same-tick duplicate 与既有 unbootstrapped persistence failure，且 entity 预先带 marker；断言 `marker → identical marker`、没有新的 `TribulationState` 或 JueBi event。该回归只锁定“既有 start guard 失败时不得误清 marker”，不要求新的 persist API、call counter 或 DB row identity。
- [ ] `du_xu_terminal_cleanup_removes_marker_on_all_r6_paths`：normal ascension 的子例以前置 **non-marker** live DuXu，断言 `TribulationState → terminal` 且 marker 仍 absent；failure、disconnect/fled、escape 与 intercept death 子例以前置 **marker-bearing DuXu**，各自断言 `live DuXu + marker → terminal without marker`，并固定该路径既有 settlement / penalty / Qi side effect 基线。这样 normal completion 不再把“marker-bearing DuXu 直接 terminal”与 P0 的 quota-origin JueBi transition 混为一谈；实现漏掉任一适用 remove 或破坏现有副作用必须使对应子例失败。
- [ ] `terminal_cleanup_then_non_over_quota_retry_does_not_append_juebi`：只覆盖 production 中同实体可重新通过 `Realm::Spirit` start gate 的 marker-bearing DuXu terminal path：failure、disconnect/fled、escape 与 intercept death。每个子例先完成 `marker-bearing DuXu → terminal without marker`，再以 non-over-quota 条件启动同 entity DuXu 并推进最终 wave；断言没有 `JueBiTriggeredEvent { source: VoidQuotaExceeded }`、没有 quota-origin `JueBiRuntimeContext`，从而锁定真实 stale-positive 触发链。normal ascension 不进入该 retry matrix：它的合法前置是 marker absent，且完成后 entity 为 `Realm::Void`，无法在不篡改生产 gate 的情况下同实体再启动 DuXu。
- [ ] `quota_origin_juebi_retains_then_all_terminal_paths_clear_marker_once`：前置为最终 wave 的 marker-bearing over-quota DuXu；断言 `DuXu + marker → JueBi + identical marker + VoidQuotaExceeded runtime`。正常 settlement 子例断言 `JueBi settle → marker absent`；failure 子例必须经真实 `tribulation_aoe_system → TribulationFailed → tribulation_failure_system` 生产链，断言 `JueBi + marker → terminal without marker`；fled/disconnect 子例走既有 shared `settle_fled_tribulation`，boundary escape 子例走 source-aware JueBi 经放宽后的 `tribulation_escape_boundary_system → settle_fled_tribulation`，各自断言相同终态。每条均断言重复 cleanup/settlement 不得生成额外事件或副作用，且不得把 marker 留给后续 independent JueBi；对 standalone/non-quota-origin JueBi 不增加 marker 断言。
- [ ] `non_quota_origin_juebi_boundary_escape_is_ignored`：前置为经既有 independent trigger path 启动、`JueBiRuntimeContext { source: JueBiTriggerSource::VoidActionExplodeZone, .. }` 的 standalone JueBi，且参与者已跨出既有 boundary；运行生产 `tribulation_escape_boundary_system` 后断言 live `TribulationState` 与 runtime source 保持、没有 `TribulationFled`/`TribulationSettled`、没有 penalty 或 Qi side effect，亦未抵达 `settle_fled_tribulation`。此负向用例锁定只有 `source == VoidQuotaExceeded` 的 JueBi 才能通过 escape eligibility；错误地把 gate 放宽到所有 JueBi 必须失败。它不对 marker presence 作断言，也不引入 standalone marker policy。
- [ ] 每个 fixture 同时记录已有 `Events<QiTransfer>` 与 `WorldQiBudget` snapshot：只由 marker cleanup 造成的 before/after 必须无新增 transfer、无 budget 变化，且已有终态 Qi path 继续通过 `qi_physics::ledger::assert_conservation`（`server/src/qi_physics/ledger.rs:771-789`）对拍。此项不改变 Qi 账户、reason、方向、amount 或 JueBi strength。

## 可核验 symbols

- `JueBiAfterDuXuQuota`、`TribulationState`、`TribulationOriginDimension`
- `start_tribulation_system`、`tribulation_wave_system`、`tribulation_failure_system`
- `abort_du_xu_on_client_removed`、`settle_fled_tribulation`、`tribulation_escape_boundary_system`、`tribulation_intercept_death_system`
- `juebi_settlement_system`、`JueBiRuntimeContext`、`JueBiTriggerSource::VoidQuotaExceeded`
- `check_void_quota`、`VoidQuotaConfig`、`WorldQiBudget`、`juebi_intensity_for_quota_marker`、`qi_physics::ledger::assert_conservation`
- `cultivation::register` 与 `server/src/cultivation/mod.rs` 的 production schedule tuple

## 非本 plan 交付物

以下是第一性复核发现的邻接风险，但不属于 PR #1304 Mapping 分配给本 successor 的 r6 #6；不得在本 plan 的实现 PR 顺手扩大范围：

- attempt ID、entity admission、allocator/reservation、并发 start linearization 或新的 error taxonomy。
- `ActiveTribulationRecord`、SQLite schema、migration、hydration/restart、DB reconciliation、tombstone/retry，以及“重启不得复活旧 attempt”的 durability 契约。
- independent JueBi 与历史损坏 marker 共存时的 source-aware `juebi_settlement_system` 分类；该问题保持当前行为并需独立 owner，不能用本 marker cleanup PR 改写 standalone JueBi 语义。quota-origin JueBi 的正常 settlement 与 fled/disconnect/escape 非-settlement marker cleanup 属于本 successor 的交付范围。
- `ZoneCollapse`、`Targeted`、generic mismatch consumer 或全局 tribulation lifecycle 重构。
- quota formula、`WorldQiBudget`、JueBi strength 常数、Qi ledger account/reason/direction/amount 或任何真元流。
- `scripts/build-token.sh` 的创建及 V 轨交付；该脚本尚未存在于当前 `origin/main`。

## 验收与安全边界

- Server gate：实现时遵守根 `CLAUDE.md` 的 server gate 与调度者分配的 build token；本 skeleton 不创建、绕过或改写 build-token 协议。
- 严禁本地运行 `scripts/test-tmux-shutdown-order.sh`、`scripts/test-server-lifecycle.sh` 或任何调用它们的 suite；GitHub e2e 保留该覆盖。
- push 前 `git fetch origin && git merge origin/main`；exact-HEAD fresh-context read-only validator PASS 后才能 push。PR 的 `/review` 触发由 orchestrator 串行控制，implementation subagent 不自行发送该评论。
- P0/P1 全部 ✅ 后补 `## Finish Evidence` 并归档；实现与归档仍保持唯一 BugFix PR。
