# plan-refactor-server-session-v1 — Server 交互 Session 生命周期与终态交付（重构轨 R1）

> 所属总纲：`plan-refactor-master-v1.md`。本轨唯一负责 gameplay session；终态交付的 durable 语义由本文件的 canonical contract 定义，R3/R10 只实现其存储与消费投影。跨轨排序引用总纲 §3 及 PR 1902 五项裁决，不在本文件另建依赖图。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 现状/吸收验真；canonical session + obligation reducer；row-ID acceptance index | ⏳ |
| P1 | `server/src/session/` 框架与 craft contract/adapter 接缝 | ⬜ |
| P2 | alchemy、forge、lingtian adapter | ⬜ |
| P3 | gathering、mineral、spiritwood、世界交互 adapter | ⬜ |
| P4 | derived bot e2e、吸收结案、归档 | ⬜ |

## 0. 外部决议与边界

- 总纲 `plan-refactor-master-v1.md §3/§4` 是跨轨 start/order/cutover 的唯一 authority。本计划引用 master artifact/phase ID，不复制 A-CS→R6→R4→R7→R3→R10 的箭头。
- PR 1902（`origin/docs/master-r9-r6-ownership-adjudication`，五项 settled rulings）作为外部已定上下文：TypeBox 是全仓 schema source of truth；domain content 与 generation machinery 分权；contract-first 可先合入但不可宣称 live；production activation 必须原子切换；track plan 不得创建第二套 sequencing authority。
- R1 独占 gameplay session registry、session reducer 和七域 adapter；R3 独占 SQL/checkpoint/outbox storage；R10 独占 inventory/spill transaction 与 worker；R4/R6/R7/R2 各自拥有其 gate/wire/UI/store。任何轨只消费冻结 API，不复制另一轨的状态语义。
- durable checkpoint 不以 Bevy `Entity` 为主键。`workbench_key` 是当前进程 `WorkbenchOpen.entity_id` 的 runtime locator；checkpoint 只保存 R3 P4 提供的 stable `placed_id`。

## 1. 现状与吸收边界

### 1.1 七域现状

| 域 | 当前权威状态 | R1 结论 |
|---|---|---|
| alchemy | `server/src/alchemy/session.rs:68` `AlchemySession`，挂在 furnace | P2 checkpointed adapter；炉与 session 同一 checkpoint |
| craft | `server/src/craft/session.rs:40` ECS `CraftSession` | P1 首宿主；迁入 registry 后保留既有 hydration 语义 |
| forge | `server/src/forge/session.rs:157` `ForgeSessions` Resource | P2 checkpointed adapter；station/material/session 同一快照 |
| gathering | `server/src/gathering/session.rs:57` store | P3 volatile adapter；中断确定性 teardown |
| lingtian | `server/src/lingtian/systems.rs:196` `ActiveLingtianSessions` | P2 volatile actor adapter；断线停止 tick |
| mineral | `server/src/mineral/session.rs:16` `MiningSession` | P3 volatile adapter；释放矿点/工具 claim |
| spiritwood | `server/src/spiritwood/session.rs:59` store | P3 volatile adapter；`settling` 与 teardown 同步 |

已确认的接线缺口包括保存前未统一 teardown、跨维残留 owner、TSY Search/Extract 双向 busy 缺失、PendingInsightOffer 无 deadline，以及取回路径先 `end_session` 后 delivery 失败。P0 吸收验真以本节七域表（§1.1）为域清单，逐项对照总纲 `plan-refactor-master-v1.md §6` 所列各轨吸收清单；被 R2/R3/R4/R10 明确拥有的实现不在 R1 重复登记，并在本轨 P0/Finish Evidence 记录验真结论。

## 2. 术语与两个 ownership domain

R1 不再用一组 `SessionPhase` 同时表示 gameplay 和 delivery。两者通过一次 handoff 连接：

- `GameplaySession`：`SessionKey`、`PlayerKey`、durability、runtime `Entity` binding、facility/target/busy claim、checkpoint/escrow 和 termination cause。
- `TerminalObligation`：reservation、quota `Q`、canonical payload、payload digest、delivery lease/retry/dead-letter、receipt/disposition 和 retention metadata。
- `Q = (1 row, reserved_bytes)`；`quota_used = sum(Q)` 对所有占用 obligation 成立。
- `workbench_key` 只做当前进程 lookup，不是 authorization、durable identity 或 checkpoint 字段；`placed_id` 才是 restore/rebind identity。
- `SessionKey`、terminal `generation`、`delivery_id` 和 canonical payload bytes 是 exactly-once 的不同概念：前者定位 session，generation 防旧写入，delivery id 防重复 handoff，payload digest 绑定交付内容。

### 2.1 R1 提供的 session 类型

```rust
pub trait InteractionSession {
    fn session_key(&self) -> SessionKey;
    fn owner_key(&self) -> &PlayerKey;
    fn durability(&self) -> SessionDurability;
    fn busy_claim(&self) -> BusyClaim;
    fn reduce(&mut self, event: SessionEvent, ctx: &mut SessionLifecycleCtx)
        -> SessionDecision;
}
```

固定类型：

- `SessionDurability::{Checkpointed, Volatile}`；adapter 必须显式注册。
- `SessionState::{Running, Paused, Suspended, HandoffPreparing, Ended}`。
- `TerminationCause::{Completed, VoluntaryCancel, Disconnect, DimensionChange, Shutdown, InvalidRestore, SuspensionExpired, AuthorizedAdministratorClosure}`。
- `BusyClaim`：player-exclusive、target-exclusive、facility-exclusive 三类集中冲突矩阵。
- `SuspensionPolicy`：`SESSION_SUSPENSION_TTL_TICKS = 1_728_000`、扫描 cadence `1_200` ticks；R3 tick rebase 只保留真实剩余 TTL，不刷新租约。
- `TerminalDeliveryPolicy`：`SESSION_DELIVERY_MAX_PAYLOAD_BYTES = 1_048_576`（1 MiB）、`MAX_ACTIVE_SESSION_DELIVERY_ROWS = 4_096`、`MAX_ACTIVE_SESSION_DELIVERY_BYTES = 4_294_967_296`（4 GiB）、`SESSION_DELIVERY_MAX_ATTEMPTS = 8`、`SESSION_DELIVERY_MAX_RETRY_AGE_TICKS = 1_728_000`（24 小时）；O-01 在同一事务内同时检查 row/byte aggregate limit，任一超界走 O-02；每个 obligation 固定预留 1 MiB capacity，但实际 canonical payload 只要求 `serialized_bytes <= SESSION_DELIVERY_MAX_PAYLOAD_BYTES`；O-12/O-13 在 attempts 或 age 任一达到边界时进 `DeadLetter`，否则回 `Pending`，`created_at_tick`/attempts 在重启时不刷新。
- `SessionMaintenancePermissions`：server console，或已认证且绑定当前 executor 的 principal/capability；Username、owner 字符串、offline player 和跨 executor capability 均无权。

## 3. Canonical session reducer（唯一 session 规范）

`reduce_session(state, event) -> {next_state, handoff?, claim_effect, checkpoint_effect, audit_effect}` 是 gameplay 唯一规范。以下 row ID 同时是 acceptance index 的引用；任何 phase prose 必须展开为这些 trace，不得另写 transition。

| Row ID | 当前状态 + event | 结果 | claim/checkpoint/delivery effect |
|---|---|---|---|
| S-01 | Absent + admission validation fail | Absent | 无 runtime claim/obligation；若 envelope 已解码且 `request_id` 合法，返回与 `CraftOpen.request_id` 关联的 typed `CraftOpenRejected`；缺失/非法 `request_id` 或 envelope decode failure 只记录不可关联 parse rejection/metric，不伪造 A-08 且不改变 session/claim/obligation |
| S-02 | Absent + `reserve_new_terminal_obligation` 成功且 claim win | Running | obligation O-01→O-03，`+Q`；绑定 owner/target |
| S-03 | Absent + reservation 成功但 busy race-loss | Absent | 执行 O-05→O-07/O-06；只有 O-06 commit 才 `-Q` |
| S-04 | Running + valid pause/close | Paused | claim 保留；不退款、不创建 terminal obligation |
| S-05 | Running/Paused + matching `session_key/generation` explicit `VoluntaryCancel` | HandoffPreparing | 按域 policy 生成 refund payload；进入 O-08 handoff |
| S-06 | Running/Paused + `Completed` | HandoffPreparing | 产物 payload；不 refund inputs；沿 S-14 handoff，不恢复为 Running |
| S-07 | Running/Paused + `Disconnect`/`Shutdown` checkpointed | Suspended only after durable commit | snapshot + state/generation + a newly generated one-time `ReconnectGuard { owner_key, session_key, generation, phase_revision, restore_token }`以单事务 commit 为线性化点；commit 前失败回 exact prior Running 或 Paused 并保留 binding/可重试，commit 后 authoritative state 禁止 tick、解绑 runtime Entity并保留逻辑 facility claim；R3 与 checkpoint 同事务持久化 guard，后续仅允许 S-10/S-11 guarded restore |
| S-08 | Running/Paused + `Disconnect`/`Shutdown` volatile | HandoffPreparing | 停止 intake，按 volatile policy 全退未消费 escrow；进入 O-08，不留下可恢复 claim |
| S-09 | Running/Paused + `DimensionChange` | HandoffPreparing | 停 intake；全退未消费 escrow；不得以旧 Running/Paused checkpoint 跨维恢复，进入 O-08 |
| S-10 | Suspended + guarded reconnect restore with matching `restore_token`, owner/session identity, generation and strictly higher `phase_revision` | Paused | R3 首先加载与 Suspended checkpoint 同事务保存的 `ReconnectGuard`；server restore producer 先向新连接发送独立 `CraftRestoreGuard { owner_key, session_key, generation, phase_revision, restore_token }` control frame，随后发送独立 A-06 `Restore { restore_token }` variant 的 authoritative Paused projection。R1 只接受逐字段匹配 guard、owner/session identity、generation 和严格更高 `phase_revision` 的 Restore，成功后在同一 server-side CAS 中消费 guard 并 rebind 当前 `placed_id`/runtime facility；不依赖已清除的 OpenPending，不发送普通 Pause；stale/mismatched token、owner、identity、generation 或 revision 均 no-op，旧 guard 不得重放或跨连接复用 |
| S-11 | Suspended + missing/conflicting checkpoint/owner/placed_id | HandoffPreparing | `InvalidRestore`；不得 attach 或覆盖另一 session；进入 O-08 |
| S-12 | Suspended + TTL expiry | HandoffPreparing | `SuspensionExpired`；保留已完成产物交付，未消费 escrow 全退；进入 O-08 |
| S-13 | Suspended + authorized admin closure | HandoffPreparing | `AuthorizedAdministratorClosure`；审计 principal/reason；进入 O-08 |
| S-14 | HandoffPreparing + handoff transaction committed | Ended | 释放 runtime/facility/target claim；obligation 获得唯一 durable payload owner；不可 reopen |
| S-15 | HandoffPreparing + transaction rollback before commit | prior live/checkpoint state | claim/session 保持可重试；不得删除 checkpoint 或扣 quota |
| S-16 | Ended + late gameplay request/reconnect | Ended | stale generation reject；不 attach、不恢复 claim |
| S-17 | any + maintenance authorization failure | same state | no mutation；audit reject reason，不产生 delivery |
| S-18 | any + same-tick timeout/reconnect/admin CAS loser | winner state | loser 重读 generation；不二次 handoff、不重复释放 claim |
| S-19 | Paused + resume with matching owner/generation | Running | claim 保留；继续原 checkpoint/escrow，不增 Q |
| S-20 | Paused + `Disconnect`/`Shutdown`/`DimensionChange`/`Completed` | by S-06/S-07/S-08/S-09 | 每个 paused event 只命中对应 cause row；checkpointed disconnect/shutdown 失败回 Paused，成功后仅经 S-10/S-11 restore；volatile/dimension/complete 均按各自 handoff policy 终结，不激活为 Running |
| S-21 | Suspended + duplicate disconnect/shutdown/pause | Suspended | 幂等 no-op；不刷新 TTL、不增 Q |
| S-22 | HandoffPreparing + duplicate gameplay/terminal event | HandoffPreparing | 重试同一 generation/payload；禁止生成第二 delivery_id |
| S-23 | nonterminal + stale/invalid gameplay identity or event not admitted by state/cause matrix | same state | typed reject + audit；无 claim/checkpoint/quota effect；generation `< current` 为 stale、`> current` 为 future-invalid，key/owner mismatch 为 invalid |
| S-24 | Suspended + matching `session_key/generation` explicit `VoluntaryCancel` | HandoffPreparing | 取消 retained logical claim；按 cancel policy 固定 payload 后进入 O-08，不先 rebind runtime Entity |
| S-25 | persisted Running/Paused + startup detects new process epoch/no runtime binding | Suspended after durable fence | startup transaction 递增 generation/fence 并把 process-local live checkpoint normalize 为 Suspended；不得伪造旧 Entity 或直接恢复 tick，随后只允许 S-10/S-11 guarded restore |
| S-26 | Running + matching `session_key/generation` valid `CraftStart { recipe_id, quantity }` | Running | 选择并启动 recipe execution；不创建新 session/Q、不改 target；Paused/Suspended/terminal 或 invalid recipe/quantity 走 S-23 |

**唯一 teardown linearization point 是 S-14 的 durable handoff commit。** 从 S-14 起，worker 的 retry、lease expiry、malformed payload 或 inventory failure 只改变 obligation，不重新 attach gameplay session/claim。R1 不允许 direct terminal-delivery alternative；R10 的同步 `deliver` 只能是 obligation worker 内部的 transaction primitive。

### 3.1 Domain policy projection

- `Checkpointed` 的 `Disconnect`/`Shutdown` 是可恢复暂停，不退款；`DimensionChange`、`InvalidRestore`、`SuspensionExpired`、管理员结案才进入 terminal refund/delivery。
- `Volatile` 遇 disconnect/dimension/shutdown 立即进入 terminal handoff；不留下 owner、claim 或 settling。
- `Completed` 只交付完整产物，不退 inputs；`VoluntaryCancel` 才执行域既有规则（craft 未完成部分 70% 返还）。
- 所有 refund/release 通过 canonical obligation payload 和 R10 worker；qi 变化仍必须走 `qi_physics::ledger::QiTransfer`。
- maintenance allow 必须分别覆盖 console positive 与 bound capability positive；wrong executor、offline username、伪造 owner 和普通玩家均拒绝。
- retention 常量固定：`SESSION_DELIVERY_RESULT_REPLAY_TTL_TICKS = 12_096_000`（7 日）、`SESSION_DELIVERY_TOMBSTONE_TTL_TICKS = 51_840_000`（30 日）、`MAX_SESSION_DELIVERY_HISTORY_ROWS = 65_536`、`MAX_SESSION_DELIVERY_HISTORY_BYTES = 134_217_728`。R3 tick rebase 保留真实 age，不刷新 horizon；达到 row 或 byte 上限时 O-26 fail-before-deliver。

## 4. Canonical terminal-obligation reducer（R1/R3/R10 共用）

`reduce_obligation(state, event) -> {next_state, quota_effect, worker_effect, retention_effect, audit_effect}` 是终态交付唯一规范。R3 提供 durable CAS/transaction；R10 提供唯一生产 worker；R1 只发出 handoff command 和消费结果。

| Row ID | obligation state + event | next state | `ΔQ` / effect |
|---|---|---|---|
| O-01 | Absent + new admission reservation commit | ReservedAwaitingClaim | `+Q`；quota update + unique reservation 同事务；`reserved_bytes = SESSION_DELIVERY_MAX_PAYLOAD_BYTES`，按最坏 payload 固定预留 |
| O-02 | Absent + quota full / unique conflict | Absent | `0`；fail closed |
| O-03 | ReservedAwaitingClaim + runtime busy claim win | ReservedLive | `0`；绑定 live session/generation，不重复计量 |
| O-04 | ReservedLive + matching suspended restore/retry | ReservedLive | `0`；复用 owner/bytes/generation，禁止 insert/+Q |
| O-05 | ReservedAwaitingClaim + busy race-loss/validation failure | CancelPending | `0`；runtime claim先释放，但 durable cleanup owner仍存在 |
| O-06 | CancelPending + cancel CAS commit | Absent | `-Q`；删除 reservation；重复/CAS loser `0` |
| O-07 | CancelPending + persistence failure | CancelPending | `0`；写入/保留 `next_retry`, attempts, generation；live reconciliation 必须重试 |
| O-08 | ReservedLive + terminal handoff transaction commit | Pending | `0`；reservation→outbox，不重复计量；payload bytes/digest 固定 |
| O-09 | ReservedLive + handoff transaction failure before commit | ReservedLive | `0`；checkpoint/claim/reservation 可重试，不能部分删除 |
| O-10 | Pending + worker claim CAS win | InFlight | `0`；写 lease/generation |
| O-11 | Pending + duplicate claim/CAS loser | authoritative state | `0`；no-write，重读 winner row（通常 InFlight + winner lease），不得写回 Pending |
| O-12 | InFlight + lease expiry | Pending 或 DeadLetter | `0`；O-10 已原子递增 attempts；attempts `< 8` 且 age `< 1_728_000` ticks 回 Pending，否则 DeadLetter；不 attach gameplay |
| O-13 | InFlight + retryable inventory/spill failure | Pending 或 DeadLetter | `0`；沿用 O-12 attempts/age 边界，未达界时更新 backoff，达界即 DeadLetter |
| O-14 | InFlight + malformed payload/digest mismatch | DeadLetter | `0`；fail closed、保留完整 payload、告警；禁止调用 deliver |
| O-15 | InFlight + claimed payload decode/validation success | InFlight | `0`；`DeliveryRequest` 只能由 canonical payload decode 派生 |
| O-16 | InFlight + inventory/spill + receipt transaction commit | ReceiptRetained | `-Q`；inventory/spill、receipt、obligation delete、quota release 同事务 |
| O-17 | ReceiptRetained + stale worker completion/claim for same digest | ReceiptRetained | `0`；authoritative receipt 证明 O-16 已原子 `-Q`；返回既有 receipt，不二次 deliver/release |
| O-18 | DeadLetter + authorized operator retry | Pending | `0`；CAS loser 不改变 quota |
| O-19 | DeadLetter + authorized resolve with complete disposition | DispositionRetained | `-Q`；完整 disposition、obligation delete、quota release 同事务 |
| O-20 | DeadLetter + resolve missing payload/disposition | DeadLetter | `0`；fail closed，继续占 Q |
| O-21 | Receipt/Disposition retained + replay before horizon | same | `0`；返回既有 receipt/disposition |
| O-22 | Receipt/Disposition retained + compaction watermark reached | CompactedTombstone | `0`；保留 bounded digest/idempotency tombstone，不保留无限 payload |
| O-23 | CompactedTombstone + old replay | CompactedTombstone | `0`；按 tombstone reject/ack，不重新交付 |
| O-24 | CompactedTombstone + GC watermark/replay horizon confirmed | GarbageCollected | `0`；只允许在 producer/outbox watermark 证明旧 obligation 不可重现后删除 |
| O-25 | ReservedAwaitingClaim + cancel-mark persistence failure | ReservedAwaitingClaim | `0`；reservation 本身仍是 durable cleanup owner；live retry + startup stale-reservation scanner，禁止超时即 `-Q` |
| O-26 | InFlight + receipt/history quota unavailable before deliver | Pending 或 DeadLetter | `0`；在 inventory mutation 前 fail closed；保留 payload/Q 并 backoff/告警 |
| O-27 | any nonterminal + stale generation/invalid event/CAS loser | authoritative state | `0`；typed reject/audit，重读 row；不得隐式 insert、release、deliver |

### 4.1 Obligation invariants

1. 每个 `session_key` 恰有零或一个 durable obligation owner；`quota_used` 等于 `ReservedAwaitingClaim`、`ReservedLive`、`CancelPending`、`Pending`、`InFlight`、`DeadLetter` 的 Q 之和，且始终满足 rows `<= 4_096`、bytes `<= 4_294_967_296`。每个 active Q 固定为 `(1 row, SESSION_DELIVERY_MAX_PAYLOAD_BYTES)`，不按当前 payload 小值预留；payload envelope 固定含 `payload_schema_version`、`delivery_id`、`session_key`、terminal `generation`、recipient `PlayerKey`、cause、item/refund entries；O-08 首次写入的 exact bytes 是唯一权威身份，后续不得重序列化替换。
2. `reserve_new` 只产生 O-01 的 `+Q`，并固定预留完整 1 MiB；claim win O-03、matching restore/retry O-04、handoff O-08、retry/lease O-12/O-13、所有 CAS loser 均 `0`，因此 terminal payload 增长不需要未建模 resize。
3. 只有 O-06 成功取消或 O-16/O-19 成功持久化终结结果才 `-Q`。取消标记或删除失败不能把 quota 留成无主 reservation：O-25 由原 reservation 继续担当 durable owner，O-07 由 `CancelPending` row 担当；scanner/retry 均不得凭 age 直接释放。
4. `payload` 是 canonical serialized bytes；`payload_digest = SHA-256(payload)`。worker 的 `DeliveryRequest` 必须从 claimed payload decode，并在 O-16 transaction 中以 digest/semantic item set 对拍；不存在独立可替换的 caller payload。
5. `payload` 是 canonical serialized bytes，实际长度必须满足 `serialized_bytes <= SESSION_DELIVERY_MAX_PAYLOAD_BYTES`；`payload_digest = SHA-256(payload)`。固定 1 MiB 只属于每个 active Q 的 reservation capacity，不要求小 payload padding 到上限；`1_048_577` 在新增 escrow/output 被接受前 fail closed。payload 一旦进入 O-08 不可增长。
6. `ReceiptRetained`/`DispositionRetained` 不是无限历史：完整结果按 §3.1 的 replay TTL 保留，达到 producer/outbox GC watermark 后转 O-22 bounded tombstone，再按 §3.1 的 tombstone TTL 满足 O-24 才 GC。R1 是 retention 数值与边界的唯一 contract owner；R3 只实现 table/index/compaction scheduling 与 O-26 deliver 前容量预留，不得另定 horizon、row/byte limit 或重启时刷新 age。
7. O-08 成功后 gameplay claim 已由 S-14 释放；O-12/O-13/O-14/O-18 不得恢复 session。DeadLetter 无 runtime claim 但继续占 Q。

## 5. 接缝与 artifact ledger

下表只说明 R1 提供/消费的 artifact；跨轨 owner/order 引用 master §4.1 和 PR 1902，不在 R1 复制 sequencing：

| Artifact / row | R1 提供或消费 | canonical evidence |
|---|---|---|
| `InteractionSession` / S-01..S-26 | R1 producer；domain adapters consumer | session reducer trace |
| `SessionKey`/`PlayerKey`/generation | R1 producer；R3/R4/R6/R7/R10 consumer | stale generation S-16/S-23 |
| reservation/quota/outbox / O-01..O-27 | R3 durable producer；R1 semantic consumer；R10 worker consumer | quota invariant 1-3 |
| payload/digest/receipt/retention | R3 storage + R10 transaction；R1 consumes result | O-14..O-27 |
| stable `placed_id` | R3 P4 provider；R4 lookup；R1 checkpoint/restore consumer | S-10/S-11 |
| `CraftRestoreGuard` control frame | R1 producer；R3 与 Suspended checkpoint 同事务持久化；R6 proto/generated/converter/transport/router registration owner；R2 bridge/`CraftStore.armReconnectGuard(...)` consumer | owner/session/generation/revision/token 全字段对拍；M-10 producer→frame→A-06 Restore→store atomic cutover |
| craft intent/state wire | A-CS/R6/R4/R7 按 master owner；R1 消费 admitted Open/Start/Pause/Resume/Cancel，生产 authoritative `CraftSessionStateV2`（含 `open_request_id`、ordinary `session_transition=Initial | Rollover { previous_session_key }`、`phase_revision`）；每次同 identity 的 phase projection 递增 `phase_revision`，S-10 guarded reconnect restore 使用独立 A-06 `Restore { restore_token }` variant，不要求已清除的 `open_request_id`/OpenPending；并把 S-01 correlated admission decision 交 R4 的 A-08 producer | M-10 real state/rejection producer→emit→store activation |
| coupled `TsyPresence` snapshot | R3 provider；R1 reconnect/gate consumer | full-old/full-new snapshot trace |

## 6. Derived acceptance index

验收只引用 row ID，不重新定义状态：

1. `session_trace_matrix`：S-01..S-26 覆盖 admission/rejection、matching Running recipe start、Paused/invalid recipe start reject、pause/resume、matching/stale/future Cancel、complete、disconnect、dimension、shutdown、restore、startup normalization、TTL、admin、duplicate、invalid、stale/CAS loser；每条 trace 逐步执行 `reduce_session`。S-07 另在 quiesce 后/commit 前、durable commit 后/runtime cleanup 前、cleanup 后注入 failure/crash：同进程 commit 前失败回 exact prior Running/Paused；进程 crash 后旧 process-local binding 一律经 S-25 normalize 为 Suspended，再走 S-10/S-11；commit 后只能恢复 Suspended，禁止出现 Paused→Running、伪造旧 Entity 或 lease 刷新。
2. `obligation_trace_matrix`：O-01..O-27 覆盖 quota-full、claim win、busy loser、cancel-mark/delete persistence failure 与 live/restart retry、restore reuse、handoff crash、lease expiry、retry/dead-letter（attempts 7/8 与 age boundary-1/exact/after）、malformed/mismatched payload、history quota full、receipt replay、resolve fail-closed、retention/GC、invalid/stale event。
3. `payload_identity`：claimed bytes、digest、derived request、inventory receipt 四者对拍；注入 payload A/request B 必须命中 O-14，不能调用 deliver 或 ack 任一 payload。
4. `quota_conservation`：以 `MAX_ACTIVE_SESSION_DELIVERY_ROWS = 4_096` / `MAX_ACTIVE_SESSION_DELIVERY_BYTES = 4_294_967_296` 为 aggregate boundary，两个 SQLite connection 竞争第 4,096 个/4 GiB 最后 slot；恰一个 O-01 成功并固定取得 `(1 row, 1_048_576 bytes)`，其余 O-02。固定 Q 使 row/bytes 只能成对到达边界，因此 acceptance 以 `(rows, bytes)` 的 lockstep limit-1、exact-limit、limit+1 三组 paired cases 对拍，不声称 row-full/bytes-not-full 或 bytes-full/rows-not-full 可独立构造。race loser 的 cancel-mark 成功走 O-05→O-07/O-06，标记失败走 O-25 并由 live/startup reconciliation 收敛；terminal payload 只要求 `serialized_bytes <= SESSION_DELIVERY_MAX_PAYLOAD_BYTES`，从空到 exact-max 均不 resize，`1_048_577` 在接收新 escrow/output 前拒绝，任何路径都不泄漏或双扣 Q。
5. `handoff_crash_atomicity`：outbox insert 前、terminal checkpoint 前、commit 后 ack 前强杀；结果只能 O-09 可重试或 O-08/Pending 可重放，S-14 不 reopen。
6. `delivery_history_bound`：对两种 retention class 各执行 `horizon-1 / exact-horizon / horizon+1` 矩阵：`ReceiptRetained`/`DispositionRetained` 在 replay TTL `12_096_000` 的前一 tick 与 exact tick 仍返回既有结果，刚过 TTL 才允许在 watermark 已满足时转 O-22、不得重新 deliver；`CompactedTombstone` 在 tombstone TTL `51_840_000` 的前一 tick 与 exact tick 仍保留 digest/idempotency reject/ack，刚过 TTL 才允许 O-24 GC，GC 前后的 replay 均不得重新交付。另覆盖 stale worker 对已由 O-16 释放 Q 的 receipt 走 O-17 且不二次 `-Q`、history quota 满命中 O-26、inventory 不 mutation、storage 不无限增长。
7. `maintenance_auth`：对 S-13、O-18 与 O-19 分别执行 console allow、current-executor capability allow、wrong-executor capability deny、offline/username/owner spoof deny、普通玩家 deny；拒绝时命中 S-17/O-27，DeadLetter payload/Q/state 不变。
8. `workbench_restore`：`workbench_key` 0/1/u64::MAX 与 malformed/stale/despawned/cross-dimension/out-of-range；成功后只保存/恢复 `placed_id`，新 runtime Entity 可 rebind。
9. `tsy_presence_snapshot`：routine autosave、disconnect、shutdown 每个写边界 crash 后 presence/position/dimension 只能全旧或全新。
10. named bot scenarios（按总纲 `plan-refactor-master-v1.md §0` 的每轨 3-8 场景上限固定为以下八项）：`session_disconnect_cleanup`、`session_dimension_transfer`、`session_restart_recovery`、`session_busy_mutex`、`session_full_inventory_delivery`、`session_suspension_reclamation`、`session_delivery_crash_atomicity`、`session_craft_pause_resume_wire` 均只引用上述 row/trace ID。`session_craft_generation_cancel`、`session_tsy_presence_relog`、`session_pending_insight_offer_deadline` 属跨轨/共享证据，不计入 R1 的 bot 场景交付数。

## 7. 本轨实施阶段

### P1 — framework + craft adapter

只落 `server/src/session/{mod.rs,registry.rs,lifecycle.rs}`、S reducer、registry/busy API 和 contract pins。craft adapter 消费 A-01/A-07/A-02..A-04 intent，生产 A-06 authoritative state，并为 admitted Open 的 S-01 failure 返回 correlated rejection decision；R4 统一生产覆盖 decode/gate/admission fail 的 A-08。framework 可在 master Wave 允许时先合入，但 production adapter 只有 master 的真实 artifact/cutover rows 全部满足后才启用，不以 mock、fixture 或未存在的 R6/R4/R7 consumer 宣称可达。

### P2 — alchemy / forge / lingtian

checkpointed furnace/station 使用 S-07/S-14 与 O-08/O-16；delivery failure 只推进 O-13，不重新 attach。lingtian 六类 actor 使用 volatile S-08；不在线 tick。

### P3 — gathering / mineral / spiritwood / world interaction

volatile adapter 统一释放 target claim；TSY Search/Extract 双向 busy；TsyPresence 仅在 R3 coupled snapshot guarded restore 通过后 attach。

### P4 — bot/e2e/归档

执行 §6 derived index，确认所有吸收项已有唯一 owner，再补 Finish Evidence；不修改其他轨文档的 authority 表述。

## 8. 开放问题

1. retention 数值与边界由本 canonical contract §3.1 唯一冻结；R3 P0 只选择 table/index/compaction scheduling 并实现这些常量，不得扩大 horizon/上限或在重启时刷新 age。
2. R3 需选择 `CancelPending` 独立表或 reservation row 内嵌 retry metadata；两者必须实现相同 O-07/O-25 语义。
3. 具体 domain escrow/refund 数值沿用各域既有 plan；不得在本 canonical protocol 新增经济规则。

## Finish Evidence

> 迁入 `finished_plans/` 前填写各 S/O row 的实现路径、关键 commit、测试命令与数量、真实 producer→consumer→cutover evidence、receipt retention evidence，以及未完成的跨轨依赖。
