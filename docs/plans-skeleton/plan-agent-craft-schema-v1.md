# plan-agent-craft-schema-v1 — Craft lifecycle Agent schema 生产批次（总纲 A-CS）

> 所属总纲：`plan-refactor-master-v1.md` §3/§4/§4.1/§6.11。一句话：由 Agent 轨一次性冻结 craft lifecycle 的 TypeBox source、generated JSON Schema 与 committed `@bong/schema` dist，给 R6 提供唯一、可调度、可验收的 agent-side schema 输入。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 对拍现有 craft/client-request/server-data schema，冻结 V2 shape 与计数基线 | ⬜ |
| P1 | TypeBox source + registry/exports + 正反 contract tests | ⬜ |
| P2 | generated JSON Schema + committed dist 原子生成与 freshness gate | ⬜ |
| P3 | R6 handoff manifest、跨轨对拍与归档证据 | ⬜ |

## 接入面

- **进料**：总纲 §4 的 canonical `CraftOpen.target`，R1 的 `SessionPhase`/session identity/generation 合同，现有 `agent/packages/schema/src/{craft.ts,client-request.ts,server-data.ts,schema-registry.ts}` 与 `CraftSessionStateV1` 基线。
- **出料**：冻结的 `CraftOpen`、`CraftPause`、`CraftResume`、`CraftSessionStateV2` TypeBox exports；对应 generated schemas；可由 agent runtime/import consumer 加载的 committed `@bong/schema` dist；记录 commit SHA、schema digest 与变体计数的 R6 handoff manifest/evidence。
- **共享类型**：复用 canonical unsigned decimal-string helper、session key 与 generation schema；不新造与 proto/Rust 不同义的 identity。
- **跨仓库契约**：本 plan 只改 `agent/packages/schema/**`。R6 P1 消费冻结结果并实现 proto/Rust/client wire；R4/R7/R1 不直接修改 Agent artifacts。无 gameplay/worldview/qi 语义。

## 冻结合同

1. `CraftOpen.target` 是 required discriminated union：`Handcraft` 或 `Workbench { workbench_key }`。`workbench_key` 是无符号 `u64` 十进制字符串；缺失、负数、小数、科学计数法、前后空白、超过 `u64::MAX` 均拒绝。它只用于当前进程初次请求，不是 durable identity。
2. `CraftPause` 与 `CraftResume` 只携带 required `session_key` + `generation`；不得复带 target 或用 Pause/Resume 替代 `CraftCancel`。
3. `CraftSessionStateV2` 覆盖 R1 的 `Running | Paused | Suspended | AwaitingDelivery | DeliveryPending | Terminal`，锁定每个 phase 对 identity/generation 的 required/forbidden 组合；删除 production V1 分支由 R6 执行，本轨只提供 V2 source-of-truth。
4. P0 必须从当前 schema registry 计算真实 C2S/S2C 基线；P1 完成后把新增三种 C2S intent 体现为总纲要求的 113→116，并记录 craft S2C V2 对总 S2C variant count 的影响。若合入前主线计数变化，先更新基线与总纲/R6 pins，不得硬写陈旧数字让 freshness 假绿。

## 阶段交付物

### P0 — schema inventory 与 shape 冻结

- 枚举 `ClientRequestV1`、`ServerDataV1`、`CraftSessionStateV1`、`SCHEMA_REGISTRY`、`GENERATED_SCHEMA_FILES`、package exports/dist 入口；记录 source→registry→generated→dist 完整路径。
- 对拍总纲 §4 与 R1/R6 contracts，形成 `CraftOpen`/Pause/Resume/StateV2 字段矩阵与 C2S/S2C type-set baseline。
- 明确与 `plan-bughunt-client-request-schema-drift.md`、`plan-bughunt-server-data-s2c-schema-union-drift-v1.md` 的边界：本 plan 只交付四个 craft lifecycle artifacts；全量 parity guard 仍由各自 drift plan 负责，但本 plan 的新增变体必须进入其现有 union/registry。

### P1 — TypeBox source 与 contract tests

- 在 `agent/packages/schema/src/` 实现并 export 四个合同，注册到 `ClientRequestV1`/`ServerDataV1` 与 `SCHEMA_REGISTRY`；旧 V1 不得继续作为 production union 分支。
- 正例：Handcraft、Workbench key `0`/`1`/`u64::MAX`，Pause/Resume identity，StateV2 每个 phase。
- 反例：target 缺失/未知 discriminator、malformed/out-of-u64 key、Pause/Resume 缺 identity/generation 或夹带 target、StateV2 phase/identity/generation 非法组合、额外字段。
- 测试必须验证对外 observable schema，不绑定私有 helper 调用次数。

### P2 — generated/dist 原子交付

- 更新 `GENERATED_SCHEMA_FILES`，生成四个单项 schema及命中的 `client-request-v1.json`/`server-data-v1.json`；`npm run build -w @bong/schema` 生成 committed dist。
- source、registry、generated、dist 必须同一提交；freshness 测试删除/篡改任一层都会失败。禁止让 R6 后补 dist 或手改 generated JSON。
- 运行 `cd agent/packages/schema && npm test` 与 `cd agent && npm run build -w @bong/schema`，并验证 clean checkout 的 package exports 可 import 新 symbols。

### P3 — R6 handoff 与跨轨验收

- 在 Finish Evidence 记录合入 SHA、generated schema digest、C2S/S2C 最终计数和导出 symbol 清单；该证据是 R6 P1 唯一可接受输入。
- R6 对同一 SHA 做 TypeBox↔proto/Rust/client samples roundtrip；字段或计数不一致退回本 plan/R6 对应 owner，不在 R6 越权修改 Agent source。
- A-CS 未归档前，总纲 Wave 1 的 R6 P1 与所有 craft production gate保持阻塞。

## 验收与边界

- 必跑：`cd agent/packages/schema && npm test`；`cd agent && npm run build -w @bong/schema`。
- 必验：source/generated/dist freshness、package runtime import、四合同全部正反样本、ClientRequest/ServerData union membership、最终变体计数。
- 不改 proto、Rust、Java、R1/R4/R6/R7 文件；不实现 client producer、server handler、session registry 或 workbench `placed_id`。
- 这是总纲具名 production batch，不吸收或替代 `tiandao-schema-dist-start` 的 clean-start 构建链修复；后者解决启动前 dist 缺失，本 plan 解决具体 craft schema 内容及其原子产物。
