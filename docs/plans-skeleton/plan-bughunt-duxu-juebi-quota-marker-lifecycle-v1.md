# plan-bughunt-duxu-juebi-quota-marker-lifecycle-v1（骨架）

> **GOAL**：修复 r6 #6 的同实体 ECS stale-positive：`JueBiAfterDuXuQuota` 不得从已结束的 DuXu attempt 遗留并改变后续 attempt；合法的 DuXu → quota-origin JueBi 连续过渡必须保留该 marker。
>
> **Canonical owner**：`docs/finished_plans/plan-bughunt-r6-findings-v1.md:53-61` Finding Mapping #6。当前 `origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14` 仍可复现；PR #1304 只完成 finding 分流，未修代码。
>
> **Delivery**：按根 `CLAUDE.md` BugFix 工作流，一个 skeleton = 一个修复 subagent = 一个常驻 slot = 一个 PR；不由 `/consume-plan` 消费。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 当前 accepted attempt 的 marker ownership 与 live terminal cleanup | ⬜ |
| P1 | r6 #6 的回归矩阵与完整 server gate | ⬜ |

## 接入面

- **进料**：`server/src/cultivation/tribulation.rs:860-1023` `start_tribulation_system` 计算 quota，并在 `:939-957,983-987` 为 accepted over-quota DuXu 插入 marker。
- **出料**：`tribulation_wave_system` 在 `:3166-3258` 读取 marker 并追加 quota-origin JueBi；`juebi_settlement_system` 在 `:1900-2080` 使用并清理 marker。
- **生产调度**：`server/src/main.rs:112` 调用 `cultivation::register`；`server/src/cultivation/mod.rs:216,428-469` 注册 producer、wave consumer、settlement 与 terminal cleanup 的顺序。竞态回归必须走该生产注册入口（或从它抽出的同一 production-owned helper），不得在测试里手拼另一套顺序。
- **共享类型 / event**：复用 `JueBiAfterDuXuQuota`、`TribulationState`、`JueBiRuntimeContext`、`JueBiTriggerSource::VoidQuotaExceeded`；不另造状态机。
- **跨仓库契约**：纯 server ECS 生命周期修复；不改 public IPC、agent/client 或 SQLite shape。
- **worldview / qi_physics**：维持现有“超额 DuXu 后追加绝壁”与 qi ledger 语义；不改配额公式、绝壁强度或任何真元流。

## 第一性验真

- `JueBiAfterDuXuQuota` 定义于 `tribulation.rs:305-310`，没有 attempt identity。
- `start_tribulation_system` 仅在本次判定为 `Some(marker)` 时 insert；accepted non-over-quota DuXu 不清旧 marker（`:939-957,983-987`）。
- 配额检查接收的是**新 attempt 占位前**的 `occupied_slots`：`check_void_quota` 以 `occupied_slots >= quota_limit` 表示已无剩余名额（`:179-192`）。因此精确边界是 `occupied_slots + 1 == quota_limit` 仍合法且清旧 marker，`occupied_slots == quota_limit` 的下一次 accepted DuXu 才创建/替换 marker；不得误改为严格 `>`。
- failure（`:3334-3427`）、disconnect/fled（`:3433-3670`）与 intercept death（`:3673-3749`）终止当前 attempt，但 cleanup tuple 未移除 marker。
- 下一次 DuXu 完成最终 wave 时，`tribulation_wave_system` 无 attempt 对拍地读取残留 marker、按旧 snapshot 计算强度并追加 JueBi（`:3177,3216-3258`），因此错误行为可达。
- 现有 marker 测试只 pin producer presence（如 `tribulation.rs:4477,4572,7128`），没有“终止 → 同实体重试”回归。

## P0 — Accepted-attempt marker ownership

- [ ] 本 successor 只处理 **DuXu accepted attempt**；本域没有其他 accepted attempt kind 能拥有该 marker。`TribulationKind::DuXu` 是 producer 的唯一合法 owner kind；`JueBi` 仅是该 DuXu 的 quota-origin continuation，不是新的 accepted start。P1 必须 pin：`TribulationKind::{ZoneCollapse,Targeted,JueBi}` 既不分配也不清除该 DuXu marker，只有 quota-origin `JueBi` settlement 按完整 token 清理它。
- [ ] 以进程内单调且不复用的 `attempt_id`（等义命名可）作为 accepted attempt 的 identity：这是易失 ECS identity，不写入 `ActiveTribulationRecord`、SQLite 或其他跨进程持久化；重启后不承诺复原，属于本 plan 明确排除的 durability。一次 start 的唯一事务状态机固定为：

  ```text
  Candidate → entity duplicate gate ──→ DuplicateRejected
                                   └─→ Admitted → reserve
                                                    ├─→ ReservationRejected(Contention | Exhaustion) → release admission
                                                    └─→ Reserved(id) → persist
                                                                         ├─→ PersistFailedDiscarded(id) → release admission
                                                                         └─→ Persisted(id) → infallible ECS commit → Accepted(id)
  ```

  entity duplicate gate 必须按 entity 原子持有从 `Admitted` 到 `Accepted` 或失败释放为止的 admission；它同时检查 live accepted attempt 与 in-flight admission。因此同一 entity 的第二次同帧请求也必须在该 gate 被拒绝，不能落到 allocator；reserve/persist 失败则只释放 admission，不留下 ECS、DB 或 marker 状态。通过 gate 的 candidate 在任何 DB 写入前由进程级 `AttemptIdAllocator` 原子 reserve 下一个唯一、单调递增、永不复用的 ID；allocator 对并发 reserve 线性化，两个不同 entity 的同帧请求必须各自取得不同 ID，不能互相当作 duplicate 或丢失一方。reserve 只允许返回 `Contention` 或 `Exhaustion`，此阶段不触碰 ECS、DB 或 marker。`persist_active_state` 是 all-or-nothing persistence boundary：失败返回时不得存在该 candidate 的 DB active row；此时丢弃 reservation 的 candidate、烧掉已取出的序号而不回收，允许 monotonic sequence 出现 holes，且不提交 ECS state 或 marker。persist 成功后才接受 identity，并调用结构上不可失败的 commit helper 一次性提交 accepted `TribulationState` 与 marker，同时将 admission 交接为 live accepted attempt；该 helper 不执行可失败查找、分配或外部 IO，因此不存在需要补偿的 ECS half-commit 分支。ECS entity 被复用时不得复用旧 identity。
- [ ] `AttemptIdAllocator` 是由 `cultivation::register` 注册的进程级 Resource/helper；reserve 的唯一可观察失败是 `Contention` 或 `Exhaustion`，两者都发生在 DB 写入前并 fail-closed，不能把 allocator rejection 与 duplicate gate 混同。reserve 成功后必须恰好调用一次 `persist_active_state`；persist-failed 的 ID 保持已消耗状态，后续 allocator 只能继续向前。persist 成功后只能进入结构上 infallible 的 accepted ECS commit；该 commit 不执行可失败查找、分配或外部 IO，因此不存在需要隐瞒的半提交。P1 必须分别 pin duplicate-gate rejection、persist failure、reservation contention、reservation exhaustion，以及两个不同 entity 同帧请求各自完成唯一 reservation 的 transition。
- [ ] marker 的完整权威类型由实现 PR 具体落地为 `JueBiAfterDuXuQuota { attempt_id, owner_phase, occupied_slots, quota_limit, total_world_qi, quota_k }`（字段可用等义命名但不得缺失）；`owner_phase` 为 `JueBiMarkerOwnerPhase::{DuXu, QuotaJueBi}`。`TribulationState` 必须携带同一 `attempt_id` 字段；owner token 是 `(entity, attempt_id, owner_phase)`。producer、consumer、terminal cleanup 与 deferred cleanup 都必须携带/捕获完整 token，不能只按 entity + attempt_id 或 entity 操作。
- [ ] accepted over-quota DuXu 以当前 identity + quota snapshot 的 marker 完整替换旧 marker；accepted non-over-quota DuXu 清除旧 marker。cleanup 必须以完整 token compare-and-remove；旧 DuXu deferred cleanup 不能匹配已转交为 `QuotaJueBi` phase 的同 identity marker。marker replace、compare-remove、phase-transfer 必须分别由明确 helper/API 承载；只有 quota-origin transition helper 可以修改 `owner_phase`，其他 producer/cleanup 只能 replace 或 compare-remove。
- [ ] identity mismatch 只相对于**当前已 accepted 且正在运行**的 `TribulationState` 与其期望 owner phase 判定：consumer 清除该 stale marker 并拒绝追加 JueBi；尚未 accepted 的候选 start 不参与判定。
- [ ] 最终 wave 的 quota-origin 分支须在同一 atomic transition 内先把匹配 identity 的 `TribulationState`/`JueBiRuntimeContext` 切为 JueBi owner，并调用唯一 phase-transfer helper 把 marker 从 `DuXu` 原子更新为 `QuotaJueBi`，保留同一 identity + snapshot，再退出普通 DuXu success cleanup；普通 success、failure、fled/disconnect、intercept death 仅 compare-remove 匹配的 `DuXu` token，JueBi settlement 最终只清一次 `QuotaJueBi` token。
- [ ] marker cleanup 不得重复触发 DB 删除、惩罚、Qi 释放或 settlement/lifecycle event。

## P1 — Regression closure

- [ ] P1 的 schedule 回归落在 `server/src/cultivation/mod.rs:1577` 的 `#[cfg(test)] mod tests`（必要时由该模块调用 `tribulation.rs:4104` 的 `#[cfg(test)] mod tests` helper）；测试必须以 `App::new(); cultivation::register(&mut app)` 构建 production-owned schedule，不得手工 add 一套替代顺序。断言 registration order 包含 `start_tribulation_system → tribulation_wave_system → terminal cleanup → juebi_settlement_system` 的现有 `.after/.before` 关系，并逐帧推进每个状态转换。每个下列回归均须先固定可区分的 precondition，再断言明确的 `before → after` transition；凡涉及 start/reserve/persist，必须分别断言 allocator state、persist call count、candidate ECS components、candidate DB active row，不能只看最终 marker。
- [ ] `accepted_last_available_slot_clears_stale_quota_marker`：前置为 entity 无 live/in-flight attempt、保有旧 snapshot marker、`occupied_slots + 1 == quota_limit`；断言 `Candidate → Admitted → Reserved(id) → Persisted → Accepted(id)`，当前 attempt 取得新 ID，旧 marker 被清除而非保留/替换，persist call count 为 1、current ECS state 与 current DB active row 存在。
- [ ] `accepted_first_over_quota_start_replaces_stale_quota_marker`：前置为 entity 无 live/in-flight attempt、保有与本次可区分的旧 snapshot marker、`occupied_slots == quota_limit`；断言同一 accepted transition 后 marker 被当前 `id` 与当前 quota snapshot 完整替换，persist call count 为 1、current ECS state 与 current DB active row 存在。
- [ ] `duplicate_rejection_and_persist_failure_do_not_commit_identity_or_touch_marker`：分别建立 duplicate gate 已命中和 reserve 已成功的前置条件。前者断言 transition 为 `Candidate → DuplicateRejected`，allocator next/reserved state 不变、persist call count 为 0、既有 ECS components / marker / DB active row 均不变；后者让 `persist_active_state` 明确失败，断言 `Reserved(id) → PersistFailedDiscarded(id)`，allocator 已越过 `id` 且下一个成功 reserve 不复用它、persist call count 恰为 1、candidate 没有 `TribulationState` 或 marker，且 DB 中没有该 candidate 的 active row。该测试不得只看最终 marker。
- [ ] `reservation_contention_exhaustion_and_concurrent_request_fail_closed_before_persist`：为每个子例分别建立可区分 precondition。contention/exhaustion 以前置 allocator state 使 reserve 确定返回相应错误，断言 `Admitted → ReservationRejected(...) → admission released`，allocator 除预期 contention bookkeeping 外不前进、persist call count 为 0、ECS components / marker / DB active row 均未改变；两个不同 entity 同帧候选以前置为均不重复且 allocator 有至少两个 slot，断言各自 `Candidate → Admitted → Reserved(id_a | id_b) → Persisted → Accepted`，`id_a != id_b` 且按 reserve 线性化顺序单调、每 entity persist call count 为 1、各自 ECS state 与 DB active row 都存在。另以同一 entity 的第一请求停在 `Admitted` 的前置条件，断言第二请求为 `Candidate → DuplicateRejected`、不调用 allocator 或 persist，证明 duplicate gate 而非 allocator 拒绝重入。不得以单个最终 marker 代替这些 transition 断言。
- [ ] `accepted_attempt_identity_is_monotonic_across_entity_reuse`：前置为 entity 的旧 accepted attempt 已带 `id_old` 完整终止、ECS entity 被回收后复用且 allocator next 大于 `id_old`；断言新 candidate 经 `Reserved(id_new) → Accepted(id_new)`，`id_new > id_old`，persist call count 为 1，新 ECS state / DB active row 只携带 `id_new`，不得因 entity reuse 复活旧 identity 或旧 marker。
- [ ] `non_duxu_attempt_kinds_do_not_allocate_or_clear_duxu_marker`：分别以前置 `ZoneCollapse`、`Targeted`、非 quota-origin `JueBi` start，以及同 entity 的可区分 DuXu marker；断言每条 non-DuXu transition 前后 allocator state 与 persist call count 不因 marker ownership 改变，既有 DuXu marker 不被分配、清除或 phase-transfer，且只有后续匹配 token 的 quota-origin settlement 可清理它。
- [ ] `mismatched_current_attempt_marker_is_cleared_without_quota_juebi`：前置为 live current `TribulationState(id_current, DuXu)` 与可区分的 stale marker token；断言最终 wave 的 `DuXu active → stale marker removed` transition，不追加 `VoidQuotaExceeded` JueBi、无新的 allocation/persist，current DB active row 与当前 ECS state 仍保持当前 attempt。
- [ ] `old_terminal_cleanup_compare_remove_cannot_remove_new_attempt_marker`：前置为旧 terminal/deferred cleanup 捕获 `(entity, id_old, DuXu)`，同 entity 已完成 `Candidate → Accepted(id_new)` 并拥有 current marker；断言旧 cleanup 执行后 `current marker(id_new) → current marker(id_new)`，不增加 persist call、不删除 `id_new` ECS state 或 DB active row。
- [ ] `terminal_quota_marker_cleanup_and_retry_matrix`：分别以前置 success、failure、fled/disconnect、intercept death 的 live matching DuXu token，再在 cleanup 后对同 entity 发起新 candidate；断言每条先从 `live matching marker → terminal marker removed`，随后新 attempt 只通过自身 `Reserved(id_new) → Accepted(id_new)` 创建/清理 marker，旧 token 不得影响 retry 的 ECS state、DB active row 或 marker。
- [ ] `quota_juebi_transition_transfers_owner_before_duxu_cleanup`：前置为最终 DuXu wave 拥有匹配 `(entity, id, DuXu)` marker 和 quota snapshot；断言单一 atomic transition `DuXu owner → QuotaJueBi owner` 先更新 current `TribulationState` / `JueBiRuntimeContext` 与 marker phase，后续普通 DuXu cleanup compare-remove 不命中，且无额外 allocation/persist。
- [ ] `quota_juebi_settlement_clears_follow_up_marker_once`：前置为 live quota-origin JueBi 持有匹配 `(entity, id, QuotaJueBi)` marker；断言 `live QuotaJueBi marker → removed` 只发生一次，重复 settlement/cleanup 保持 removed 并且不重复 DB delete、惩罚、Qi 释放或 settlement/lifecycle event。
- [ ] 回归须覆盖同帧/跨帧 deferred cleanup 与 owner transfer；边界样例显式固定为 start 前 `occupied_slots + 1 == quota_limit`（本次占最后一个合法名额，不得留 marker）与 `occupied_slots == quota_limit`（第一个超额 attempt，必须创建/替换当前 identity marker）。新旧 marker 使用可区分的 quota snapshot，逐字段对拍当前 marker，并断言 quota-origin `JueBiTriggeredEvent.source/intensity` 与 `JueBiRuntimeContext` 均从当前 snapshot 派生；专门回归 `QuotaJueBi owner transfer → 旧 DuXu cleanup → JueBi settlement` 顺序与最终 marker 只清一次。
- [ ] success、failure、fled/disconnect、intercept death 与 JueBi settlement 各路径须把修复前既有副作用固定为精确基线（不适用为 0、适用为 1），并在修复后严格相等：DB active-row 删除、惩罚、settlement/lifecycle event 对拍关键 identity/payload；Qi ledger 对拍账户、reason、方向与 amount，并断言 marker-only cleanup 为零新交易、余额不变且守恒。

## 可核验 symbols

- `JueBiAfterDuXuQuota`、`JueBiMarkerOwnerPhase`、`AttemptIdAllocator`、`start_tribulation_system`
- `tribulation_wave_system`、`tribulation_failure_system`、`abort_du_xu_on_client_removed`
- `tribulation_escape_boundary_system`、`settle_fled_tribulation`
- `tribulation_intercept_death_system`、`juebi_settlement_system`
- `replace_quota_marker`、`compare_remove_quota_marker`、`transfer_quota_marker_owner`

## 非本 plan 交付物

以下是第一性复核发现的邻接风险，但不属于 PR #1304 Mapping 分配给本 successor 的 r6 #6；不得在本 plan 的实现 PR 顺手扩大范围：

- `JueBiAfterDuXuQuota` 的 cross-process durability，包括 `ActiveTribulationRecord`、migration 与 hydration；合法 over-quota marker 当前无法跨进程恢复。
- terminal DB delete failure 后的 tombstone/retry/reconciliation 与“重启不得复活旧 attempt”。
- fresh/dev reset、通用 despawn、NPC dormancy/rehydration 的统一 lifecycle 重构。
- restart 后 start-time intensity / settlement-time quota 的双时点持久化契约。
- `scripts/build-token.sh` 的创建及 V 轨交付；该脚本尚未存在于当前 `origin/main`。

## 验收与安全边界

- Server gate：若实现时 `scripts/build-token.sh` 已由 V 轨合入，按其真实 CLI 运行；否则遵守本轮调度授权，用 `flock /tmp/bong-cargo.lock -c 'cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test'`。
- 严禁本地运行 `scripts/test-tmux-shutdown-order.sh`、`scripts/test-server-lifecycle.sh` 或任何调用它们的 suite；GitHub e2e 保留该覆盖。
- push 前 `git fetch origin && git merge origin/main`；exact-HEAD fresh-context read-only validator PASS 后才能 push。每次 push 后在同一 PR 发新的 `/review` 评论。
- P0/P1 全部 ✅ 后补 `## Finish Evidence` 并归档；实现与归档仍保持唯一 BugFix PR。
