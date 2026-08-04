# plan-agent-craft-schema-v1 — Craft lifecycle Agent schema 生产批次（总纲 A-CS）

> 所属总纲：`plan-refactor-master-v1.md` §3/§4/§4.1/§6.11。唯一职责：原子生产 craft lifecycle 的 TypeBox source、generated JSON Schema 与 committed `@bong/schema` dist，供 R6 消费。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | inventory、shape 与 variant baseline | ⬜ |
| P1 | TypeBox source、registry/exports、contract tests | ⬜ |
| P2 | generated schema、dist、freshness gate | ⬜ |
| P3 | R6 handoff SHA/digest/evidence | ⬜ |

## 接入面与冻结合同

- **进料**：总纲 §4 canonical shape、R1 phase/identity/generation、`agent/packages/schema/src/{craft.ts,client-request.ts,server-data.ts,schema-registry.ts}`。
- **出料/owner**：本 plan 唯一修改 `agent/packages/schema/**`，原子交付 `CraftOpen`、`CraftPause`、`CraftResume`、`CraftSessionStateV2` 的 source/generated/dist；R6 只消费记录 SHA，不回改 Agent artifacts。
- `CraftOpen.target` required：`Handcraft | Workbench { workbench_key }`。key 为 unsigned `u64` decimal string；缺失、负数、小数、科学计数法、空白、`>u64::MAX` 均拒绝，且不是 durable identity。
- Pause/Resume 只含 required `session_key + generation`；不得夹带 target 或替代 Cancel。StateV2 覆盖 `Running | Paused | Suspended | AwaitingDelivery | DeliveryPending | Terminal`，逐 phase 锁定 identity/generation required/forbidden 组合；production union 删除 V1。
- P0 从 registry 计算实时基线；新增三种 C2S 后目标 113→116。若主线计数变化，先同步总纲/R6 pins，禁止硬编码旧数假绿。

## 阶段交付物

### P0 — inventory / shape

枚举 source→registry→generated→dist/export 全路径，冻结四合同字段矩阵、C2S/S2C type-set baseline；本 plan 只拥有 craft 四合同，全量 drift guard 仍归既有 drift plans。

### P1 — TypeBox / tests

实现四合同并注册/export 到 `ClientRequestV1`、`ServerDataV1`、`SCHEMA_REGISTRY`。正例覆盖 Handcraft、Workbench key `0/1/u64::MAX`、Pause/Resume identity 与 StateV2 全 phase；反例覆盖 malformed target/key、identity 缺失或多余字段、非法 phase 组合。

### P2 — generated / dist

更新 `GENERATED_SCHEMA_FILES`、四单项 schema、`client-request-v1.json`、`server-data-v1.json` 与 committed dist。source/registry/generated/dist 必须同一提交；删除或篡改任一层时 freshness test 失败，clean checkout 可 import 新 symbols。

### P3 — R6 handoff

记录 merge SHA、schema digest、最终 C2S/S2C count 与 exports。R6 只接受该 SHA并做 TypeBox↔proto/Rust/client roundtrip；不一致退回对应 owner。A-CS 未归档时所有 craft production gate 阻塞。

## 验收与边界

必跑 `cd agent/packages/schema && npm test`、`cd agent && npm run build -w @bong/schema`；必验四合同正反样本、union membership、freshness、runtime import 与 variant count。不改 proto、Rust、Java、gameplay handler/session，也不替代 `tiandao-schema-dist-start`。

## Finish Evidence

> 迁入 `finished_plans/` 前填写落地路径、commit SHA/日期、测试结果、四 symbols 的 source/generated/dist/runtime-import 对拍，以及遗留项。
