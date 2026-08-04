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

2026-08-04 对当前 source 的核对结果：`craft.ts` 有 standalone `CraftStartReqV1` 与 `CraftSessionStateV1`，但 `ClientRequestV1`、`ServerDataV1`、`SCHEMA_REGISTRY` 对 `craft_start`、`craft_cancel`、`workbench_open`、craft session state 均无 membership。Rust/proto 的生产变体数不是 TypeBox baseline，不能写成 `113→116`。

| Row ID | production chain 所需 contract | 当前 TypeBox 状态 | A-CS 终态 |
|---|---|---|---|
| A-01 | `CraftOpen` | absent；取代 standalone/legacy `CraftStartReqV1` 的 lifecycle intent | required `target = Handcraft | Workbench { workbench_key }` |
| A-02 | `CraftPause` | absent | required `session_key + generation` |
| A-03 | `CraftResume` | absent | required `session_key + generation` |
| A-04 | `CraftCancel` | server/proto 已 live，TypeBox envelope absent | 纳入 authoritative C2S union；不由 Pause 替代 |
| A-05 | `WorkbenchOpen` | server/proto 已 live（`entity_id + x + y + z`），TypeBox S2C union absent | 纳入 authoritative S2C union；完整保留四字段并由 `entity_id` 产生 request-local `workbench_key` |
| A-06 | `CraftSessionStateV2` | 只有 standalone V1，S2C union absent | V2 纳入 union；删除 V1 production export/membership |

P0 必须枚举每个 row 的 source→export→`ClientRequestV1`/`ServerDataV1`→`SCHEMA_REGISTRY`→generated→dist/runtime import 状态，并计算**当时真实** C2S/S2C type set。P1/P2 完成后的目标 count 是该集合实际去重后的派生值；主线 drift 先由其 owner 修复或显式登记，不把 Rust count 冒充 TypeBox count，也不为凑常量越界修无关 contract。

## 2. 冻结 shape

- A-01 `CraftOpen.target` required：`Handcraft | Workbench { workbench_key }`。key 是 unsigned `u64` decimal string；缺失、负数、小数、科学计数法、空白、`>u64::MAX` 均拒绝；它不是 durable identity 或 capability。
- A-02/A-03 只含 required `session_key + generation`，不得夹带 target 或替代 Cancel。
- A-04 保留显式取消语义与既有 production discriminant；字段 inventory 必须与 proto/Rust live contract 对拍后冻结。
- A-05 `WorkbenchOpen` 完整镜像 live proto：required `entity_id + x + y + z`；`entity_id` 是 A-01 `workbench_key` 的 producer，TypeBox/JSON 使用 unsigned `u64` decimal string，并锁 `0/1/u64::MAX`。`x/y/z` 是 required signed `sint32` integer，锁 `i32::MIN/0/i32::MAX`；坐标只供显示/上下文，不是 authorization，R4 仍须按 authoritative ECS 重验实体、维度、距离和 facility。
- A-06 的五个 phase 都 required `session_key + generation`，且禁止 delivery obligation 字段。`Paused` 是唯一 Resume-eligible phase；`Running` 表示已活动且重复 Resume 不得重启；`Suspended` 必须等待 guarded restore 后由新 `Paused` projection 开放 Resume；`HandoffPreparing` 与 `Ended` 均 terminal/non-resumable。

## 3. 阶段交付物

### P1 — TypeBox domain content

在 `agent/packages/schema/src/{craft.ts,client-request.ts,server-data.ts,schema-registry.ts,index.ts}` 落 A-01..A-06，注册/export 到相应 envelope 与 registry。正反样本覆盖 target/key、Pause/Resume identity、Cancel、WorkbenchOpen 四字段（坐标 min/zero/max、缺字段/错类型），并按下表逐行 pin StateV2：

| phase | identity rule | client intent rule |
|---|---|---|
| `Running` | required matching `session_key + generation` | non-resumable；重复 Resume typed reject/no restart |
| `Paused` | required matching `session_key + generation` | 唯一 Resume-eligible；只允许同 identity/generation 恰一次 Resume |
| `Suspended` | required matching `session_key + generation` | non-resumable；等待 guarded restore 后新的 `Paused` projection |
| `HandoffPreparing` | required matching `session_key + generation` | terminalizing；Resume/Pause/Cancel 均不得重新打开 session |
| `Ended` | required matching `session_key + generation` | terminal；所有 gameplay intent stale reject |

每个 phase 各有 structural valid sample，并分别覆盖缺 `session_key`、缺 `generation`、字段类型/范围错误；含 obligation phase/字段或未知 phase 必须拒绝。TypeBox 只证明 shape，不能判定 structurally valid identity 是否匹配当前 runtime session；stale generation、mismatched identity 与 phase-specific intent allow/reject 分别由 R1 S-16/S-19/S-23、R4 `gate_matrix_sweep` 和 R7 producer tests 执行。P1 只可声明 contract-first，不宣称 producer→consumer production 可达。

### P2 — generated / dist

更新 `GENERATED_SCHEMA_FILES`、六个单项 schema、envelope schemas 与 committed dist。source/registry/generated/dist 同一提交；删除或篡改任一层时 freshness test 失败，clean checkout 可 runtime import A-01..A-06。

### P3 — R6 handoff

记录 merge SHA、schema digest、A-row source/generated/dist/runtime-import 对拍，以及从最终 registry **程序化计算**的 C2S/S2C counts/type sets。R6 只接受该冻结版本，并按 PR 1902 负责 proto/Rust/client machinery；production activation 由 master cutover row 管理，不是 A-CS P3 的下游实现验收。

## 4. 验收与边界

- 必跑 `cd agent/packages/schema && npm test` 与 `cd agent && npm run build -w @bong/schema`。
- acceptance 逐 A-01..A-06 证明正反 sample、union membership、registry membership、freshness、generated/dist/runtime import；A-05 对拍 live `entity_id/x/y/z` 全字段，A-06 必须逐 `Running/Paused/Suspended/HandoffPreparing/Ended` 执行 §3 structural phase/identity presence 矩阵，不能用通用样本代替；stateful stale/mismatch/intent 规则必须引用 R1/R4/R7 runtime traces，不能伪称 TypeBox 可判断。count 断言从 registry 派生，不出现手写 113/116。
- 不改 proto、Rust、Java、gameplay handler/session；不吸收全量 schema drift plan。若无关 drift 阻断 envelope freshness，记录真实 owner/prerequisite，不擅自扩 scope。
- 跨轨 owner/order/cutover 仅引用 master §3/§4.1 与 PR 1902，不在本 plan 复制依赖箭头。

## Finish Evidence

> 迁入 `finished_plans/` 前填写 A-01..A-06 的落地路径、commit SHA/日期、测试结果、registry-derived counts/type sets、source/generated/dist/runtime-import 对拍及遗留 drift owner。
