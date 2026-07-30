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

- [ ] marker 仅可由当前 accepted over-quota DuXu 或其直接追加的 quota-origin JueBi 拥有。
- [ ] 以进程内单调且不复用的 `attempt_id`（等义命名可）作为 accepted attempt 的 identity：仅在 candidate 通过 rejected/duplicate 门并成功 persist 后分配，随同 accepted `TribulationState` 与 marker 提交；被拒绝或 persist-failed 的 candidate 不获得 identity，也不得触发 marker cleanup。ECS entity 被复用时不得复用旧 identity。
- [ ] marker 的权威 owner token 必须是 `(entity, attempt_id, owner_phase)`，其中至少区分 `DuXu` 与 `QuotaJueBi`；accepted over-quota marker 初始为 `DuXu` phase。producer、consumer、terminal cleanup 与 deferred cleanup 都必须携带/捕获完整 token，不能只按 entity + attempt_id 或 entity 操作。
- [ ] accepted over-quota DuXu 以当前 identity + quota snapshot 的 marker 完整替换旧 marker；accepted non-over-quota DuXu 清除旧 marker。cleanup 必须以完整 token compare-and-remove；旧 DuXu deferred cleanup 不能匹配已转交为 `QuotaJueBi` phase 的同 identity marker。
- [ ] identity mismatch 只相对于**当前已 accepted 且正在运行**的 `TribulationState` 与其期望 owner phase 判定：consumer 清除该 stale marker 并拒绝追加 JueBi；尚未 accepted 的候选 start 不参与判定。
- [ ] 最终 wave 的 quota-origin 分支须在同一 atomic transition 内先把匹配 identity 的 `TribulationState`/`JueBiRuntimeContext` 切为 JueBi owner，并把 marker 的 owner phase 从 `DuXu` 原子更新为 `QuotaJueBi`，保留同一 identity + snapshot，再退出普通 DuXu success cleanup；普通 success、failure、fled/disconnect、intercept death 仅 compare-remove 匹配的 `DuXu` token，JueBi settlement 最终只清一次 `QuotaJueBi` token。
- [ ] marker cleanup 不得重复触发 DB 删除、惩罚、Qi 释放或 settlement/lifecycle event。

## P1 — Regression closure

- [ ] `accepted_last_available_slot_clears_stale_quota_marker`
- [ ] `accepted_first_over_quota_start_replaces_stale_quota_marker`
- [ ] `rejected_duplicate_and_persist_failed_start_do_not_allocate_identity_or_touch_marker`
- [ ] `accepted_attempt_identity_is_monotonic_across_entity_reuse`
- [ ] `mismatched_current_attempt_marker_is_cleared_without_quota_juebi`
- [ ] `old_terminal_cleanup_compare_remove_cannot_remove_new_attempt_marker`
- [ ] `terminal_quota_marker_cleanup_and_retry_matrix`
- [ ] `quota_juebi_transition_transfers_owner_before_duxu_cleanup`
- [ ] `quota_juebi_settlement_clears_follow_up_marker_once`
- [ ] 回归须从 `cultivation::register` 的生产注册路径驱动，并覆盖同帧/跨帧 deferred cleanup 与 owner transfer；边界样例显式固定为 start 前 `occupied_slots + 1 == quota_limit`（本次占最后一个合法名额，不得留 marker）与 `occupied_slots == quota_limit`（第一个超额 attempt，必须创建/替换当前 identity marker）。新旧 marker 使用可区分的 quota snapshot，逐字段对拍当前 marker，并断言 quota-origin `JueBiTriggeredEvent.source/intensity` 与 `JueBiRuntimeContext` 均从当前 snapshot 派生；专门回归 `QuotaJueBi owner transfer → 旧 DuXu cleanup → JueBi settlement` 顺序与最终 marker 只清一次。
- [ ] success、failure、fled/disconnect、intercept death 与 JueBi settlement 各路径须把修复前既有副作用固定为精确基线（不适用为 0、适用为 1），并在修复后严格相等：DB active-row 删除、惩罚、settlement/lifecycle event 对拍关键 identity/payload；Qi ledger 对拍账户、reason、方向与 amount，并断言 marker-only cleanup 为零新交易、余额不变且守恒。

## 可核验 symbols

- `JueBiAfterDuXuQuota`、`start_tribulation_system`
- `tribulation_wave_system`、`tribulation_failure_system`、`abort_du_xu_on_client_removed`
- `tribulation_escape_boundary_system`、`settle_fled_tribulation`
- `tribulation_intercept_death_system`、`juebi_settlement_system`

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
