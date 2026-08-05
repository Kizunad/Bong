# plan-agent-craft-schema-v1 — Craft lifecycle Agent schema 生产批次（总纲 A-CS）

> 所属总纲：`plan-refactor-master-v1.md`。遵循 PR 1902 settled rulings：TypeBox 是 shape/validation source of truth；A-CS 拥有 craft domain contracts，R6 只拥有 generation/wire machinery 与 atomic activation。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 实际 source/union/registry/generated/dist inventory 与 row set | ⬜ |
| P1 | TypeBox domain contracts、union/registry/exports、contract tests | ⬜ |
| P2 | generated schema、dist、freshness gate | ⬜ |
| P3 | R6 handoff SHA/digest/derived counts | ⬜ |

## 1. P0 实际 inventory（禁止虚构 baseline）

2026-08-04 对当前 source 的核对结果：`craft.ts` 有 standalone `CraftStartReqV1` 与 `CraftSessionStateV1`，但 `ClientRequestV1`、`ServerDataV1`、`SCHEMA_REGISTRY` 对 `craft_start`、`craft_cancel`、`workbench_open`、craft session state 均无 membership。Rust/proto 的生产变体数不是 TypeBox baseline，不能写成 `113→116`。A-01 只接管 target admission，不删除 recipe execution；A-07 把既有 start command 升级为 session-identified command。

| Row ID | production chain 所需 contract | 当前 TypeBox 状态 | A-CS 终态 |
|---|---|---|---|
| A-01 | `CraftOpen` | absent；从 legacy `CraftStartReqV1` 拆出 target admission | required `request_id + target = Handcraft | Workbench { workbench_key }`；不含 recipe execution |
| A-02 | `CraftPause` | absent | required `session_key + generation` |
| A-03 | `CraftResume` | absent | required `session_key + generation` |
| A-04 | `CraftCancel` | server/proto 已 live但 identity-free，TypeBox envelope absent | breaking replacement 为 required `session_key + generation`；不由 Pause 替代 |
| A-05 | `WorkbenchOpen` | server/proto 已 live（`entity_id + x + y + z`），TypeBox S2C union absent | 纳入 authoritative S2C union；完整保留四字段并由 `entity_id` 产生 request-local `workbench_key` |
| A-06 | `CraftSessionStateV2` | 只有 standalone V1，S2C union absent | V2 纳入 union；普通 admission hydration required `open_request_id + session_key + generation + phase_revision + session_transition + phase`，guarded reconnect 使用独立 `Restore { restore_token }` variant；删除 V1 production export/membership |
| A-07 | `CraftStart` | standalone `CraftStartReqV1` 有 `recipe_id/quantity`，live Rust/proto 已可达 | required `session_key + generation + recipe_id + quantity(1..64)`；仅 matching Running 按 S-26 执行，其他 phase 走 S-23 |
| A-08 | `CraftOpenRejected` | absent | required `request_id + reason`；仅对可解析且 correlation 合法的 S-01/R4 admission reject producer，R2/R6 store 清理 matching OpenPending；不可关联 parse failure 不伪造 A-08 |

P0 必须枚举每个 row 的 source→export→`ClientRequestV1`/`ServerDataV1`→`SCHEMA_REGISTRY`→generated→dist/runtime import 状态，并计算**当时真实** C2S/S2C type set。P1/P2 完成后的目标 count 是该集合实际去重后的派生值；主线 drift 先由其 owner 修复或显式登记，不把 Rust count 冒充 TypeBox count，也不为凑常量越界修无关 contract。

## 2. 冻结 shape

跨语言标量先冻结，再定义 A-row 字段：

- `OpaqueId`（`request_id`、`session_key`、`previous_session_key`）：required canonical ASCII string，UTF-8 长度 `1..128`，regex `^[A-Za-z0-9][A-Za-z0-9._~-]{0,127}$`；禁止空串、空白、Unicode、控制字符与截断。JSON Schema/TypeBox 使用 bounded `Type.String`，Rust 使用 bounded string wrapper，proto 使用 `string`，Java bridge 按字符串比较，不转换为数值。它是 opaque identity/token，不代表 durable row 或 capability。
- `U64DecimalString`（`workbench_key`、`entity_id`、`generation`、`phase_revision`）：required canonical unsigned decimal string，regex `^(0|[1-9][0-9]*)$`，数值范围 `0..18446744073709551615`；禁止负数、小数、科学计数法、前导零、空白与超范围值。TypeBox/JSON 固定为 bounded `Type.String`，Rust/proto 语义为 `u64`/`uint64`，protobuf JSON 与 Java bridge 保持十进制字符串，禁止 JavaScript `Type.Integer` 或 signed Java `long` 造成精度/范围损失。所有 `0/1/u64::MAX` 边界共用此定义。
- `RestoreToken`（仅 A-06 `Restore.restore_token` 与 M-09 reconnect guard 使用）：required canonical opaque ASCII string，UTF-8 长度 `32..128`，regex `^[A-Za-z0-9][A-Za-z0-9._~-]{31,127}$`；服务端以密码学随机字节生成 unpadded base64url candidate，只有长度落在 `32..128` 且完整匹配该 regex 的 candidate 才能构造成 token，首字符为 `-`/`_` 或任一字符、长度不合 schema 时立即丢弃并重新生成，禁止把未通过 schema 校验的 candidate 写入 guard 或 wire。禁止空串、空白、Unicode、控制字符、前导/尾随空格、截断、数值化或重新规范化。TypeBox/JSON 是 bounded `Type.String`；proto `Restore` oneof 内是 `string restore_token`，空字符串或不符合 regex 在 converter 边界拒绝；Rust 生成字段先保持 `String`，转换为 bounded `RestoreToken` newtype 后才进入 R1 reducer；Java bridge/store 只保存并按字节相等比较 `String`，不转 UUID、`long` 或 JavaScript number。服务端 `ReconnectGuard` 与 A-06 `Restore` 必须逐字复用同一 token；token 只对绑定的 owner/session/generation/revision 有效，成功 S-10 后作废并由下一次 S-07 生成新 token。`restore_token` 是 capability，不是 `OpaqueId`，不写入 `open_request_id` 或 durable `placed_id`。

- A-01 `CraftOpen` required `request_id + target`：`Handcraft | Workbench { workbench_key }`。`workbench_key` 使用 `U64DecimalString`；它不是 durable identity 或 capability。`request_id` 只关联可关联的 A-08，不成为 session identity。缺失/非法 `request_id` 或 envelope decode failure 属于不可关联 parse rejection，不伪造 A-08，也不改变 session/claim/obligation。
- A-02/A-03 只含 required `session_key + generation`，不得夹带 target 或替代 Cancel。
- A-04 `CraftCancel` 保留显式取消 discriminant，但不保留 identity-free legacy shape：required `session_key + generation`。R1 对 `< current` stale reject、`= current` 按 S-05/S-24、`> current` future-invalid reject；key/owner mismatch reject，任何 reject 不得取消当前 session。
- A-05 `WorkbenchOpen` 完整镜像 live proto：required `entity_id + x + y + z`；`entity_id` 是 A-01 `workbench_key` 的 producer，TypeBox/JSON 使用 unsigned `u64` decimal string，并锁 `0/1/u64::MAX`。`x/y/z` 是 required signed `sint32` integer，锁 `i32::MIN/0/i32::MAX`；坐标只供显示/上下文，不是 authorization，R4 仍须按 authoritative ECS 重验实体、维度、距离和 facility。
- A-06 ordinary admission hydration is the `Initial | Rollover` variant and requires `open_request_id + session_key + generation + phase_revision + session_transition + phase`; `open_request_id` echoes the current successful A-01 request. Guarded S-10 reconnect is a separate `Restore { restore_token }` variant: it requires `restore_token` plus `session_key + generation + phase_revision + phase`, does not require or consume the cleared `open_request_id`/`OpenPending`, and still verifies current owner, session identity, generation, and monotonic `phase_revision`; `Initial` only applies to no-session Idle/OpenPending, while `Rollover { previous_session_key }` only authorizes an already-existing identity replacement. CraftStore accepts Restore only when the token matches the current reconnect guard; ordinary A-06 must not bypass the request latch. `phase_revision` is the authoritative monotonic sequence within one session identity; equal-generation snapshots are accepted only with a strictly higher revision, while lower/equal revisions are duplicate/replay no-ops and cannot regress phase. Delivery obligation fields remain forbidden in A-06. `Paused` is the only Resume-eligible phase; `Running` is active and duplicate Resume must not restart it; `Suspended` waits for guarded restore and a new `Paused` projection; `HandoffPreparing` and `Ended` are terminal/non-resumable.
- A-07 `CraftStart` required `session_key + generation + recipe_id + quantity`；quantity 锁 `1/64` accept、`0/65` reject。它选择/启动 recipe，不创建新 session、不携 target；只有 matching Running 进入 R1 S-26，Paused/Suspended/HandoffPreparing/Ended 或 identity/recipe/quantity invalid 均走 S-23。
- A-08 `CraftOpenRejected` required `request_id + reason`；reason 至少覆盖 malformed/stale/despawned/cross-dimension/out-of-range/busy/quota/persistence。仅对拥有合法 request correlation 的 malformed/业务 rejection 发出；缺失/非法 request_id 或 envelope decode failure 走不可关联 parse rejection，不伪造 A-08。A-08 只终结 matching OpenPending，不创建 Idle gameplay phase。

## 3. 阶段交付物

### P1 — TypeBox domain content

在 `agent/packages/schema/src/{craft.ts,client-request.ts,server-data.ts,schema-registry.ts,index.ts}` 落 A-01..A-08，注册/export 到相应 envelope 与 registry。A-CS 拥有 craft domain samples；R6 只消费 P3 冻结 SHA，不回写或重生成这些 Agent-owned samples。正反样本覆盖 Open request correlation/target/key、Pause/Resume identity、Cancel/Start identity与 generation shape、Start recipe/quantity、OpenRejected reasons、WorkbenchOpen 四字段（坐标 min/zero/max、缺字段/错类型），并按下表逐行 pin StateV2：

| phase | identity rule | client intent rule |
|---|---|---|
| `Running` | required matching `session_key + generation` | non-resumable；重复 Resume typed reject/no restart |
| `Paused` | required matching `session_key + generation` | 唯一 Resume-eligible；只允许同 identity/generation 恰一次 Resume |
| `Suspended` | required matching `session_key + generation` | non-resumable；等待 guarded restore 后新的 `Paused` projection |
| `HandoffPreparing` | required matching `session_key + generation` | terminalizing；Resume/Pause/Cancel 均不得重新打开 session |
| `Ended` | required matching `session_key + generation` | terminal；所有 gameplay intent stale reject |

- A-06 ordinary hydration covers `open_request_id + session_key + generation + phase_revision + session_transition + phase`; the guarded S-10 restore is a separate `Restore { restore_token }` variant that does not depend on the cleared request/latch. P1 samples must cover both variants, including missing/wrong restore token and stale restore generation/revision.

Each phase has a structural valid sample and covers missing `session_key`, missing `generation`, and field type/range errors; scalar samples must pin `OpaqueId` empty/whitespace/Unicode/overlength rejection and `U64DecimalString` negative/decimal/scientific/leading-zero/overflow rejection, plus accepted `0/1/u64::MAX` boundaries. Restore samples additionally cover missing/wrong token, owner mismatch, and stale generation/revision. Obligation phase/fields and unknown phases must be rejected. TypeBox proves shape only; runtime identity, restore authorization, and phase-specific intent rules are owned by R1/R4/R7 traces. P1 is contract-first and does not claim producer→consumer production reachability.

### P2 — generated / dist

更新 `GENERATED_SCHEMA_FILES`、八个单项 schema、envelope schemas 与 committed dist。source/registry/generated/dist 同一提交；删除或篡改任一层时 freshness test 失败，clean checkout 可 runtime import A-01..A-08。

### P3 — R6 handoff

记录 merge SHA、schema digest、A-row source/generated/dist/runtime-import 对拍，以及从最终 registry **程序化计算**的 C2S/S2C counts/type sets。R6 只接受该冻结版本，并按 PR 1902 负责 proto/Rust/client machinery；production activation 由 master cutover row 管理，不是 A-CS P3 的下游实现验收。

## 4. 验收与边界

- 必跑 `cd agent/packages/schema && npm test` 与 `cd agent && npm run build -w @bong/schema`。
- acceptance 逐 A-01..A-08 证明正反 sample、union membership、registry membership、freshness、generated/dist/runtime import；A-04/A-07 证明 identity/generation required 与 quantity boundary，A-05 对拍 live `entity_id/x/y/z` 全字段，A-06 必须逐 `Running/Paused/Suspended/HandoffPreparing/Ended` 执行 §3 structural phase/identity presence 矩阵，A-08 pin request correlation/reason；stateful generation comparison/intent 规则必须引用 R1/R4/R7 runtime traces，不能伪称 TypeBox 可判断。count 断言从 registry 派生，不出现手写 113/116。
- 不改 proto、Rust、Java、gameplay handler/session；不吸收全量 schema drift plan。若无关 drift 阻断 envelope freshness，记录真实 owner/prerequisite，不擅自扩 scope。
- 跨轨 owner/order/cutover 仅引用 master §3/§4.1 与 PR 1902，不在本 plan 复制依赖箭头。

## Finish Evidence

> 迁入 `finished_plans/` 前填写 A-01..A-08 的落地路径、commit SHA/日期、测试结果、registry-derived counts/type sets、source/generated/dist/runtime-import 对拍及遗留 drift owner。
