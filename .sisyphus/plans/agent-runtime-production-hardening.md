# Tiandao Agent Runtime Production Hardening

## TL;DR
> **Summary**: 将 `docs/plan-agent-v2.md` 转化为一份单一、可执行、可验证的生产加固计划：在保留现有 Redis Pub/Sub 架构与既有 fullstack smoke 资产的前提下，补齐可观测性、恢复性、幂等、安全路由、工具调用、E2E 与 CI 守护网。  
> **Deliverables**:
> - `agent/packages/tiandao` 的 telemetry、严格 LLM 边界、多模型路由、WorldModel 持久化、tool-calling、narration 评估
> - `server/` 的 Redis 重连、命令/叙事去重、执行锚点日志、未实现事件占位能力
> - `scripts/e2e-redis.sh`、`scripts/smoke-test-e2e.sh`、`docker-compose.test.yml`、`.github/workflows/e2e.yml`
> - 覆盖 stale world_state、重连、重复投递、恢复、工具循环上限的自动化证明
> **Effort**: XL
> **Parallel**: YES - 3 waves
> **Critical Path**: 1 → 3 → 4 → 5 → 6 → 7 → 8 → 13 → 14 → 15

## Context
### Original Request
- 用户要求基于 `docs/plan-agent-v2.md` 指定一个 Sisyphus 计划。
- 计划不仅要覆盖文档列出的 B1-B7，还要预测“还需要额外包括哪些保证成功”。
- 当前模式是 Prometheus 规划模式：只产出执行计划，不直接实现代码。

### Interview Summary
- 本次按**架构级**任务处理，无阻塞性用户偏好需要额外追问。
- 已通过仓库探索确认：B1/B6 不是绿地；server Redis bridge、command executor、scoped narration、agent runtime、RedisIpc 与 fullstack smoke 资产均已存在。
- 计划默认保持现有 Redis Pub/Sub + List 架构，不把任务扩大成 Redis Streams / 消息系统重构。
- 计划默认用**确定性 fake LLM**完成 smoke/CI；真实远程模型不作为通过门槛。
- 文档中的“玩家进游戏看见 narration”被替换为**代理可执行**的 typed payload / Redis / 日志证据链验收。

### Metis Review (gaps addressed)
- 已收紧范围：不把 B1 演变成消息总线重写，不把 B4/B5 演变成自治代理平台。
- 已补额外成功条件：freshness gate、重连/重订阅、去重窗口、执行日志锚点、失败注入、CI 防抖、严格 schema 校验。
- 已固定关键默认值：保持 Redis 线协议 V1 兼容；幂等/恢复信息尽量以内存状态、日志和测试护栏实现，而不是扩张公共消息契约。
- 已将 B6 决策为**重构现有 smoke 脚本**，而不是从零建立独立流程。
- 已明确 B7 为非阻塞主线项：必须实现，但不得阻塞 B1-B6 收口。

## Work Objectives
### Core Objective
- 交付一条**可重复、可恢复、可观测、可自动验证**的 Tiandao 运行链路，使 agent 与 server 在 Redis 控制面下满足生产最小安全线，并通过本地脚本与 CI 自动证明其正确性。

### Deliverables
- `agent/packages/tiandao/src/telemetry.ts`
- `agent/packages/tiandao/src/tools/types.ts`
- `agent/packages/tiandao/src/tools/query-player.ts`
- `agent/packages/tiandao/src/tools/query-zone-history.ts`
- `agent/packages/tiandao/src/tools/list-active-events.ts`
- `agent/packages/tiandao/src/narration-eval.ts`
- 对 `agent/packages/tiandao/src/{runtime.ts,agent.ts,llm.ts,redis-ipc.ts,world-model.ts,parse.ts,main.ts}` 的生产加固修改
- 对 `server/src/network/{mod.rs,redis_bridge.rs,command_executor.rs}` 与 `server/src/world/events.rs` 的运行安全加固
- `scripts/e2e-redis.sh`
- `scripts/smoke-test-e2e.sh`
- `docker-compose.test.yml`
- `.github/workflows/e2e.yml`

### Definition of Done (verifiable conditions with commands)
- `cd agent/packages/schema && npm test`
- `cd agent/packages/tiandao && npm run check && npm test`
- `cd server && cargo test`
- `bash scripts/e2e-redis.sh`
- `bash scripts/smoke-test-e2e.sh`
- `test -f docker-compose.test.yml && test -f .github/workflows/e2e.yml`

### Must Have
- 保持 `bong:world_state` / `bong:player_chat` / `bong:agent_command` / `bong:agent_narrate` 现有 Redis 通道名不变。
- 保持 `AgentCommandV1` / `NarrationV1` / `WorldStateV1` 的跨语言 V1 契约兼容，并补齐 parity / rejection tests。
- agent 只处理**单调递增**的 `world_state.tick`；重复或陈旧状态必须跳过并留痕。
- server 必须对重复 command batch 与重复 narration 进行短窗去重，并把“收到 / 丢弃 / 执行 / 失败”分开记录。
- model routing 必须按角色隔离 backoff/timeout/telemetry；tool-calling 必须有严格 schema 校验与循环上限。
- E2E 与 CI 必须使用确定性 fake LLM / 本地可控输入，不依赖真实远程模型结果。

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- 不引入 Redis Streams、consumer groups、第二套控制面协议或全新消息系统。
- 不重写现有 typed `bong:server_data` 路由，不新增人工进游戏作为唯一验收手段。
- 不把公共 Redis V1 契约改成破坏性新版本；若需要额外护栏，优先放在内部状态、日志与测试层。
- 不让 B7 narration 评估阻塞 B1-B6 主线收口。
- 不让 CI 依赖真实 LLM API、人工 secrets 或非确定性外部服务。

## Verification Strategy
> ZERO HUMAN INTERVENTION — all verification is agent-executed.
- Test decision: **TDD + tests-after 混合**
  - TDD：schema parity、decision validation、telemetry、world-model snapshot、dedupe、tool loop limits、narration scoring
  - tests-after：Redis reconnect、server command execution、end-to-end smoke、CI wiring
- QA policy: 每个任务都必须包含 happy path 与 failure/edge case，且证据落到 `.sisyphus/evidence/`。
- Evidence: `.sisyphus/evidence/task-{N}-{slug}.{ext}`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave. <3 per wave (except final) = under-splitting.
> Shared contracts and runtime seams go first; E2E/CI only after runtime and server guarantees exist.

Wave 1: 协议冻结、LLM 边界、可观测性、持久化基础（Tasks 1-5）

Wave 2: 运行时/Server 恢复与幂等、安全工具框架（Tasks 6-10）

Wave 3: 具体工具、叙事评估、E2E 脚本、CI 与故障注入证明（Tasks 11-15）

### Dependency Matrix (full, all tasks)
| Task | Depends On | Blocks |
|---|---|---|
| 1 | - | 3,4,7,8,13,14,15 |
| 2 | - | 3,8,13,14,15 |
| 3 | 1,2 | 4,6,10,11,12,13,14,15 |
| 4 | 1,3 | 12,13,14,15 |
| 5 | 3 | 6,11,15 |
| 6 | 3,5 | 13,14,15 |
| 7 | 1 | 8,13,14,15 |
| 8 | 1,2,7 | 9,13,14,15 |
| 9 | 8 | 11,13,14,15 |
| 10 | 3 | 11,14,15 |
| 11 | 5,9,10 | 14,15 |
| 12 | 3,4 | 14,15 |
| 13 | 1,2,6,7,8,9 | 14,15 |
| 14 | 4,10,11,12,13 | 15 |
| 15 | 1,2,3,4,5,6,7,8,9,10,11,12,13,14 | Final Verification Wave |

### Agent Dispatch Summary (wave → task count → categories)
- Wave 1 → 5 tasks → `ultrabrain` (1,2,3), `deep` (4,5)
- Wave 2 → 5 tasks → `deep` (6,7,9,10), `ultrabrain` (8)
- Wave 3 → 5 tasks → `deep` (11,13,14,15), `artistry` (12)

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task MUST have: Agent Profile + Parallelization + QA Scenarios.

- [x] 1. 冻结跨语言 Redis V1 契约与 parity gate

  **What to do**: 以现有 V1 契约为基线，建立 TypeScript 与 Rust 两端**同一组正例/反例**门禁，覆盖 `CHANNELS`、`AgentCommandV1`、`NarrationV1`、`WorldStateV1`。必须补齐以下 rejection coverage：错误版本、未知顶层字段、未知嵌套字段、`commands` 超上限、`narration.scope != broadcast` 时缺少 `target`、非法 `type/style`、非法 `Command.params` 形态。若发现 TS 与 Rust 对同一 sample 的接受/拒绝不一致，优先修测试与校验逻辑，不改通道名、不引入 V2。

  **Must NOT do**: 不得重命名 `bong:world_state` / `bong:player_chat` / `bong:agent_command` / `bong:agent_narrate`；不得扩张公共 schema 字段；不得把本任务升级为 Streams/新协议设计。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是全部后续工作的跨语言真源护栏。
  - Skills: `[]` — 无额外技能依赖。
  - Omitted: `["playwright"]` — 无浏览器工作流。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 3,4,7,8,13,14,15 | Blocked By: -

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:56-60` — B1.1 明确要求 TS/Rust 双端 parse 一致。
  - Pattern: `agent/packages/schema/src/channels.ts:5-17` — TS 侧 Redis 通道单一真源。
  - Pattern: `server/src/schema/channels.rs:1-5` — Rust 侧通道常量必须与 TS 完全一致。
  - Pattern: `agent/packages/schema/src/agent-command.ts:23-39` — `AgentCommandV1` 与 `commands maxItems=5` 约束。
  - Pattern: `server/src/schema/agent_command.rs:14-21` — Rust 侧 `AgentCommandV1` mirror。
  - Pattern: `agent/packages/schema/src/narration.ts:15-22` — `NarrationV1` 约束。
  - Pattern: `server/src/schema/narration.rs:14-18` — Rust 侧 `NarrationV1` mirror。
  - Pattern: `agent/packages/schema/src/world-state.ts:78-90` — `WorldStateV1` 结构定义。
  - Pattern: `server/src/schema/world_state.rs:65-74` — Rust 侧 `WorldStateV1` mirror。
  - Test: `agent/packages/schema/tests/schema.test.ts:57-259` — 现有正例/反例组织方式。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/schema && npm test -- tests/schema.test.ts tests/generated-artifacts.test.ts` 通过。
  - [ ] `cd server && cargo test deserialize_agent_command_sample -- --nocapture && cargo test deserialize_narration_sample -- --nocapture && cargo test deserialize_world_state_sample -- --nocapture` 通过。
  - [ ] 新增 parity / rejection 测试可证明 TS 与 Rust 对同一非法 sample 给出一致拒绝结果。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path contract parity stays green
    Tool: Bash
    Steps: run `cd agent/packages/schema && npm test -- tests/schema.test.ts tests/generated-artifacts.test.ts`; then run `cd server && cargo test deserialize_agent_command_sample -- --nocapture && cargo test deserialize_narration_sample -- --nocapture && cargo test deserialize_world_state_sample -- --nocapture`; save combined output
    Expected: TS schema tests、generated freshness gate、Rust sample反序列化全部通过
    Evidence: .sisyphus/evidence/task-1-contract-parity.log

  Scenario: Failure cases are rejected symmetrically
    Tool: Bash
    Steps: run the new targeted negative tests for wrong version / extra field / missing target in both TS and Rust suites; capture suite names and output
    Expected: 反例测试全部明确失败于校验层，而不是在运行时 panic 或静默通过
    Evidence: .sisyphus/evidence/task-1-contract-parity-error.log
  ```

  **Commit**: YES | Message: `test(schema): harden redis contract parity gates` | Files: `agent/packages/schema/src/**`, `agent/packages/schema/tests/**`, `server/src/schema/**`

- [x] 2. 收紧 LLM client 表面与确定性 fake 响应接口

  **What to do**: 在 `agent/packages/tiandao/src/llm.ts` 中把 `LlmClient.chat()` 从“只返回 string”重构为“返回结构化结果”，统一包含 `content`、`durationMs`、`requestId`、`model`，并预留可选 `options` 参数位，供后续 telemetry、模型隔离和 tool-calling 复用。`createMockClient()` 必须支持稳定元数据输出；现有 timeout/backoff 逻辑与 `LlmTimeoutError` / `LlmBackoffError` 行为必须保持。更新所有调用方与测试，使 fake LLM 在单测、smoke 与未来 CI 中完全可控。

  **Must NOT do**: 不得在本任务中引入 tools 执行循环；不得让测试访问真实网络；不得破坏已有 timeout/backoff 语义。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是 B2/B4/B5 共用的接口底座，错误会放大全局返工。
  - Skills: `[]` — 无额外技能依赖。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 3,8,13,14,15 | Blocked By: -

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `agent/packages/tiandao/src/llm.ts:4-130` — 当前 `chat()` 仅返回 string，且已有 timeout/backoff seam。
  - Test: `agent/packages/tiandao/tests/llm.test.ts:45-162` — 现有 timeout/backoff 回归测试模板。
  - Pattern: `agent/packages/tiandao/src/runtime.ts:127-143` — `createRuntimeClient()` 当前调用面。
  - Pattern: `agent/packages/tiandao/src/main.ts:47-103` — `runMockTickForTest()` 需要稳定 fake LLM。
  - Test: `agent/packages/tiandao/tests/runtime.test.ts:50-106` — mock/non-mock client seam 已有覆盖。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm run check && npm test -- tests/llm.test.ts tests/runtime.test.ts` 通过。
  - [ ] `LlmClient.chat()` 的所有调用方都改为消费结构化结果，不再直接假设返回 string。
  - [ ] fake client 可稳定返回 `durationMs=0`、固定 `requestId`（或 `null`），供 smoke/CI 断言。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path LLM metadata is propagated
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/llm.test.ts tests/runtime.test.ts`; inspect the new assertions around `content/durationMs/requestId`
    Expected: 超时、backoff、mock metadata 三类测试全部通过
    Evidence: .sisyphus/evidence/task-2-llm-surface.log

  Scenario: Timeout/backoff behavior remains intact after refactor
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/llm.test.ts -t "times out a single chat call with bounded timeout" -t "enters backoff after reaching consecutive failure threshold"`
    Expected: 接口改造后仍抛出同类错误，且不会因为 metadata 包装吞掉异常
    Evidence: .sisyphus/evidence/task-2-llm-surface-error.log
  ```

  **Commit**: YES | Message: `refactor(tiandao): harden llm client result surface` | Files: `agent/packages/tiandao/src/llm.ts`, `agent/packages/tiandao/src/{runtime.ts,agent.ts,main.ts,chat-processor.ts}`, `agent/packages/tiandao/tests/{llm.test.ts,runtime.test.ts}`

- [x] 3. 加固 agent 决策边界：strict validation、stale tick skip 与 correlation seam

  **What to do**: 在 `runtime.ts`、`parse.ts`、`main.ts` 中建立 agent 运行最小安全线。必须完成：
  1. `parse.ts` 不再把 JSON 直接强转为 `Command[]/Narration[]`，而是逐项用 `@bong/schema validate` 校验并统计 parse failure；
  2. `runRuntime()` 维护 `lastProcessedTick`，当 `world_state.tick <= lastProcessedTick` 时跳过本轮并记录 `stale_state_skip`；
  3. `runTick()` 生成内部 `correlationId = tiandao-tick-{tick}`，并把 `sourceTick/correlationId` 通过内部 publish 接口传给 `RedisIpc`、日志与后续 telemetry；
  4. `AgentCommandV1.id` 生成规则改为包含 source tick（例如 `cmd_t{tick}_{source}_{ms}`），使 server 端可去重；
  5. `runMockTickForTest()` 同步采用新 seam，供后续 smoke 复用。

  **Must NOT do**: 不得修改公共 `NarrationV1` 结构；不得让 stale state 重新触发 command/narration publish；不得在 parse 失败时抛未捕获异常终止循环。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是运行正确性的第一道硬闸门。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 4,6,10,11,12,13,14,15 | Blocked By: 1,2

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `agent/packages/tiandao/src/runtime.ts:145-196` — 当前 `runTick()` 只 merge/publish，不做 freshness gate。
  - Pattern: `agent/packages/tiandao/src/runtime.ts:230-367` — 当前 runtime loop 对同一 `latestState` 可能重复运行。
  - Pattern: `agent/packages/tiandao/src/parse.ts:21-49` — 当前 parse 只 `JSON.parse + cast + slice`。
  - Pattern: `agent/packages/tiandao/src/main.ts:47-103` — deterministic mock seam 已存在。
  - Test: `agent/packages/tiandao/tests/runtime.test.ts:108-286` — `runTick()` / worldModel 注入已有测试模板。
  - Test: `agent/packages/tiandao/tests/main-loop.test.ts:68-173` — loop resilience 模板可扩展为 stale-state skip。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- tests/runtime.test.ts tests/main-loop.test.ts tests/parse.test.ts` 通过。
  - [ ] 新增测试证明：同一 `world_state.tick` 重复出现时不会再次 publish commands/narrations。
  - [ ] 新增测试证明：非法 command/narration 项被丢弃并计入 parse failure，而不是被直接透传。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path only newer ticks publish decisions
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/runtime.test.ts tests/main-loop.test.ts`; capture the new stale-tick and sourceTick/correlation tests
    Expected: 相同 tick 重放被 skip；更大 tick 才会继续 publish；batch id 含 tick 信息
    Evidence: .sisyphus/evidence/task-3-runtime-freshness.log

  Scenario: Failure path drops invalid decision rows safely
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/parse.test.ts -t "drops invalid command rows" -t "drops invalid narration rows"`
    Expected: 非法项被丢弃并留下 parse_fail 统计，不会导致 runtime 崩溃
    Evidence: .sisyphus/evidence/task-3-runtime-freshness-error.log
  ```

  **Commit**: YES | Message: `fix(tiandao): validate decisions and skip stale world state` | Files: `agent/packages/tiandao/src/{runtime.ts,parse.ts,main.ts,redis-ipc.ts}`, `agent/packages/tiandao/tests/{runtime.test.ts,main-loop.test.ts,parse.test.ts}`

- [x] 4. 实装 telemetry sink、结构化事件与 rolling summary

  **What to do**: 按 `docs/plan-agent-v2.md` B2 新增 `src/telemetry.ts`，并在 `runtime.ts` / `agent.ts` 中注入 telemetry。必须固定以下事件模型：
  - `TickMetrics` 至少包含 `tick/timestamp/durationMs/agentResults/mergedCommandCount/mergedNarrationCount/chatSignalCount/eraChanged/errorBreakdown/staleStateSkipped`
  - `agentResults` 至少记录 `name/status/durationMs/commandCount/narrationCount/tokensEstimated/model`
  - `errorBreakdown` 固定包含 `timeout/backoff/parseFail/reconnect/dedupeDrop`
  - 提供 `JsonLogSink`、`RollingSummarySink`、`NoopTelemetrySink`
  - 每 10 tick 打一条固定格式 summary；mock 模式与测试可注入 sink 验证调用次数与 payload 形态

  **Must NOT do**: 不得把 telemetry 写成不可 grep 的自由文本；不得让 `recordTick()` 失败拖垮主循环；不得在测试中依赖真实时间漂移。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要同时改 runtime、agent 调用面与新测试文件。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 12,13,14,15 | Blocked By: 1,3

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:96-154` — B2 的 `TickMetrics/TelemetrySink/errorBreakdown` 目标。
  - Pattern: `agent/packages/tiandao/src/runtime.ts:145-196` — merge/publish 计数采集点。
  - Pattern: `agent/packages/tiandao/src/runtime.ts:282-353` — loop error/backoff 计数采集点。
  - Pattern: `agent/packages/tiandao/src/agent.ts:59-89` — 单 agent tick timing/response length 采集点。
  - Pattern: `agent/packages/tiandao/src/llm.ts:47-121` — timeout/backoff 错误分类来源。
  - Test: `agent/packages/tiandao/tests/runtime.test.ts:108-286` — 可扩展为 mock sink 断言。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- tests/runtime.test.ts tests/telemetry.test.ts` 通过。
  - [ ] mock 模式跑 10 tick 时能输出固定格式 rolling summary。
  - [ ] telemetry sink 失败不会中断 `runRuntime()` 主循环。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path tick metrics are emitted deterministically
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/telemetry.test.ts tests/runtime.test.ts`; capture assertions for `recordTick()` payload and rolling summary format
    Expected: sink 收到完整 metrics，含 errorBreakdown 与 model 字段
    Evidence: .sisyphus/evidence/task-4-telemetry.log

  Scenario: Failure path telemetry sink does not crash runtime
    Tool: Bash
    Steps: run the new test where `recordTick()` throws once during runtime loop; capture output
    Expected: runtime 记录 warning 后继续工作，测试最终通过
    Evidence: .sisyphus/evidence/task-4-telemetry-error.log
  ```

  **Commit**: YES | Message: `feat(tiandao): add runtime telemetry sinks` | Files: `agent/packages/tiandao/src/{telemetry.ts,runtime.ts,agent.ts}`, `agent/packages/tiandao/tests/{telemetry.test.ts,runtime.test.ts}`

- [x] 5. 为 WorldModel 增加 Redis hash 持久化与文件快照恢复

  **What to do**: 以 `docs/plan-agent-v2.md` B3 为准，在 `world-model.ts`、`redis-ipc.ts`、`runtime.ts` 增加可恢复状态层。必须完成：
  1. `WorldModel.toJSON()` / `WorldModel.fromJSON()` / `WorldModelSnapshot`；
  2. Redis key 固定为 `bong:tiandao:state`，字段固定为 `current_era/zone_history/last_decisions/last_tick`；
  3. 启动时先做 `HGETALL` 恢复，恢复成功打印固定日志锚点；
  4. 每轮 tick 结束后以单次 pipeline/HSET 写入 Redis；
  5. 每 100 tick 写 `data/tiandao-snapshot-{tick}.json`，最多保留 5 个；
  6. 恢复时若 Redis 字段损坏或部分缺失，必须 fail-soft：跳过损坏字段、记录 warning、不中断 runtime。

  **Must NOT do**: 不得把 snapshot 写到 `data/` 之外的路径；不得要求手工恢复；不得让损坏快照导致 agent 启动失败。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要跨 `world-model`、Redis IPC、runtime 和测试一起改。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 6,11,15 | Blocked By: 3

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:158-197` — B3 的 Redis hash + snapshot 目标。
  - Pattern: `agent/packages/tiandao/src/world-model.ts:69-276` — 当前 `WorldModel` 内存结构与需要序列化的数据。
  - Pattern: `agent/packages/tiandao/src/runtime.ts:230-367` — 启动/循环/关闭生命周期切点。
  - Pattern: `agent/packages/tiandao/src/redis-ipc.ts:61-203` — 当前 RedisIpc 能力，可扩展 hash API。
  - Test: `agent/packages/tiandao/tests/world-model.test.ts:63-200` — world-model 现有语义测试模板。
  - Test: `agent/packages/tiandao/tests/runtime.test.ts:288-352` — mock mode 与依赖注入测试 seam。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- tests/world-model.test.ts tests/runtime.test.ts tests/redis-ipc.test.ts` 通过。
  - [ ] 新增 roundtrip 测试证明 `toJSON -> fromJSON` 保持 currentEra、zoneHistory、lastDecisions、lastTick 一致。
  - [ ] 新增集成测试证明：启动 → 跑若干 tick → 重建 runtime 后可从 Redis hash 恢复状态。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path state persists and restores across restarts
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/world-model.test.ts tests/runtime.test.ts`; capture the new restore-after-restart suite
    Expected: currentEra、zone history、last decisions 在重启后恢复；日志含固定 restore anchor
    Evidence: .sisyphus/evidence/task-5-worldmodel-persist.log

  Scenario: Corrupt persisted state degrades gracefully
    Tool: Bash
    Steps: run the new targeted test that injects malformed Redis hash fields / broken snapshot JSON and then boots runtime
    Expected: runtime 记录 warning 并继续运行，未损坏字段可恢复，进程不崩溃
    Evidence: .sisyphus/evidence/task-5-worldmodel-persist-error.log
  ```

  **Commit**: YES | Message: `feat(tiandao): persist and restore world model state` | Files: `agent/packages/tiandao/src/{world-model.ts,redis-ipc.ts,runtime.ts}`, `agent/packages/tiandao/tests/{world-model.test.ts,runtime.test.ts,redis-ipc.test.ts}`

- [x] 6. Server 侧补执行锚点日志、批次去重与未实现事件占位行为

  **What to do**: 在 `server/src/network/mod.rs`、`command_executor.rs`、`world/events.rs` 中把 B1 从“能跑”提升到“可证明正确”。必须完成：
  1. 为 inbound command batch 增加 seen-id 去重窗口（短 TTL/LRU 即可），重复 batch 直接丢弃并记录 `dedupe_drop`；
  2. 在每条命令真正执行前后打印统一结构化锚点日志，至少含 `batch_id/source/type/target/result`；
  3. `realm_collapse` / `karma_backlash` 不再只 warn no-op，至少实现可验证的占位行为（zone active event + narration/chat/message 反馈之一）；
  4. narration 侧对重复 payload 做短窗去重，防止重连/重发导致多次广播；
  5. 所有 drop/reject/execute 路径都纳入可 grep 日志。

  **Must NOT do**: 不得扩大为全局分布式去重系统；不得修改通道名；不得让去重导致合法新 batch 被长期吞掉。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要同时改 server 运行逻辑与测试。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 13,14,15 | Blocked By: 3,5

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:61-67` — B1.2 对 `spawn_event/modify_zone/npc_behavior` 的要求。
  - Pattern: `server/src/network/mod.rs:564-606` — 当前 inbound command/narration 处理点。
  - Pattern: `server/src/network/command_executor.rs:80-204` — 当前实际执行点。
  - Pattern: `server/src/world/events.rs:62-175` — unsupported event 当前仅 warn 不执行。
  - Test: `server/src/network/command_executor.rs:253-502` — 现有 command executor 测试模板。
  - Test: `server/src/network/mod.rs:1016-1235` — scoped narration 测试模板。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test command_executor_tests:: -- --nocapture` 通过。
  - [ ] 新增测试证明：重复 `AgentCommandV1.id` 只执行一次。
  - [ ] 新增测试证明：`realm_collapse` / `karma_backlash` 至少产生可验证占位副作用，而不是仅日志 warn。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path command execution emits auditable anchors
    Tool: Bash
    Steps: run `cd server && cargo test command_executor_tests:: -- --nocapture`; capture logs for execute begin/end and successful dedupe-free path
    Expected: 每条命令有统一锚点日志，且现有 modify_zone/npc_behavior/spawn_event 测试仍通过
    Evidence: .sisyphus/evidence/task-6-server-execution.log

  Scenario: Duplicate batch and placeholder events are handled safely
    Tool: Bash
    Steps: run the new tests for duplicate `AgentCommandV1.id` and unsupported-event placeholder behavior; capture output
    Expected: 重复 batch 被丢弃并计数；`realm_collapse/karma_backlash` 有可断言的占位效果
    Evidence: .sisyphus/evidence/task-6-server-execution-error.log
  ```

  **Commit**: YES | Message: `fix(server): add command dedupe and execution anchors` | Files: `server/src/network/{mod.rs,command_executor.rs}`, `server/src/world/events.rs`, `server/src/network/**/*test*`

- [x] 7. Server Redis bridge 配置化并建立 reconnect→resubscribe 状态机

  **What to do**: 在 server Redis bridge 侧补最低恢复能力。必须完成：
  1. `REDIS_URL` 从硬编码改为环境变量/配置读取，默认仍为 `redis://127.0.0.1:6379`；
  2. `redis_bridge.rs` 不再因为 subscriber task 结束就永久停桥，而是进入 backoff 重连并重新订阅；
  3. 明确记录 `connecting/subscribed/reconnect/backoff/subscriber_ended` 等状态锚点；
  4. reconnect 后不得丢失 outbound publish 能力，且不会导致重复订阅 handler 暴增；
  5. 为重连路径补 targeted tests 或可验证 harness。

  **Must NOT do**: 不得引入新的外部依赖编排器；不得让 bridge 重连逻辑阻塞主线程；不得把本任务做成“只打印 warning，无实际恢复”。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 这直接决定 B1/B6 的可恢复性与 CI 稳定度。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 8,13,14,15 | Blocked By: 1

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `server/src/network/mod.rs:33-34,70-77` — 当前 `REDIS_URL` 硬编码与 bridge 启动点。
  - Pattern: `server/src/network/redis_bridge.rs:58-199` — 当前 bridge 生命周期与 subscriber task 结束即停桥行为。
  - Pattern: `scripts/smoke-law-engine.sh:106-115,139-142` — 现有 redis reachability 与 server startup anchor 模式。
  - External: Redis Pub/Sub at-most-once / reconnect风险（已在研究中确认） — 本任务只做 compensating guardrails，不改架构。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test` 仍通过。
  - [ ] 新增测试/har ness 证明：bridge 连接失败或 subscriber 断开后会重连并重新订阅。
  - [ ] `cargo run` 日志可 grep 到 `connecting`、`subscribed`、`reconnect/backoff` 锚点。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path bridge startup remains observable
    Tool: Bash
    Steps: run `cd server && timeout 20s cargo run`; capture startup logs
    Expected: 日志出现配置化 redis URL、connecting、subscribed 锚点，server 正常启动
    Evidence: .sisyphus/evidence/task-7-server-redis.log

  Scenario: Failure path subscriber exit triggers reconnect
    Tool: Bash
    Steps: run the new targeted test/harness that forces the subscriber task to end or simulates connection loss; capture logs
    Expected: bridge 进入 backoff 并重新订阅，不是直接停止整个桥线程
    Evidence: .sisyphus/evidence/task-7-server-redis-error.log
  ```

  **Commit**: YES | Message: `fix(server): harden redis bridge reconnect flow` | Files: `server/src/network/{mod.rs,redis_bridge.rs}`, `server/src/network/**/*test*`, `server/src/main.rs`

- [x] 8. 多模型路由按角色隔离 client/backoff/telemetry 并增加 allowlist

  **What to do**: 按 B4 扩展 `RuntimeConfig`、`AgentConfig`、`createDefaultAgents()`、`chat-processor.ts`。必须完成：
  1. 增加 `modelOverrides: { default, annotate, calamity, mutation, era }` 等价结构；
  2. annotate 与每个 agent 使用独立 `LlmClient` 实例，避免共享 backoff 污染；
  3. 只允许从显式 allowlist 中选择 override model，未知值 fail fast；
  4. 每个 agent/tick/annotate telemetry 都记录实际使用 model；
  5. `createDefaultAgents()` 传入 per-agent model override；`chat-processor` 使用 `annotateModel`。

  **Must NOT do**: 不得让 LLM 自己选择模型；不得把 fallback 做成静默随机回退；不得共享一个 client 处理 annotate 与 era。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 配置、隔离、回退与 observability 必须同时正确。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 9,13,14,15 | Blocked By: 1,2,7

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:200-243` — B4 的 `LLM_MODEL_*` / `model override` 目标。
  - Pattern: `agent/packages/tiandao/src/runtime.ts:25-31,76-88,90-114` — 当前只有单一 `model` 与 `createDefaultAgents()`。
  - Pattern: `agent/packages/tiandao/src/agent.ts:18-24,59-89` — 当前 `TiandaoAgent` 没有 per-agent model。
  - Pattern: `agent/packages/tiandao/src/chat-processor.ts:23-28,92-155` — 当前 annotate 与 agent 共用 model 参数。
  - Test: `agent/packages/tiandao/tests/runtime.test.ts:34-47` — env 读取模板。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- tests/runtime.test.ts tests/chat-processor.test.ts` 通过。
  - [ ] 新增测试证明：`era`、`calamity`、`mutation`、`annotate` 分别调用预期 model 与独立 client。
  - [ ] 新增测试证明：未知 override model 会 fail fast，并留下清晰错误信息。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path per-role model routing works deterministically
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/runtime.test.ts tests/chat-processor.test.ts`; inspect new routing assertions
    Expected: era/annotate 等角色使用各自 model 与独立 client，不共享 backoff 状态
    Evidence: .sisyphus/evidence/task-8-model-routing.log

  Scenario: Invalid override is rejected before runtime loop
    Tool: Bash
    Steps: run the new targeted test with unsupported `LLM_MODEL_ERA` or runtime override; capture output
    Expected: 配置阶段直接失败，并输出 allowlist 校验错误
    Evidence: .sisyphus/evidence/task-8-model-routing-error.log
  ```

  **Commit**: YES | Message: `feat(tiandao): isolate model routing by role` | Files: `agent/packages/tiandao/src/{runtime.ts,agent.ts,chat-processor.ts,llm.ts}`, `agent/packages/tiandao/tests/{runtime.test.ts,chat-processor.test.ts}`

- [x] 9. 搭建 Agent Tools 框架并限制只读 tool-calling 循环

  **What to do**: 以 B5 为准实现 `tiandao/src/tools/` 与最小安全工具循环，但严格限制范围。必须完成：
  1. 新建 `tools/types.ts` 定义 `AgentTool`、`ToolContext`、工具 schema 与结果协议；
  2. `llm.ts` 支持传入 `tools`，并在模型请求返回 tool call 时执行“最多 3 轮”的只读循环；
  3. 同名同参 tool call 去重；总工具轮次、总耗时、单轮错误都要计入 telemetry；
  4. 工具执行前后都做 schema 校验，非法 args 直接拒绝并回填错误结果；
  5. 当前阶段仅允许只读查询工具，且绑定本轮固定 `latestState + worldModel` snapshot。

  **Must NOT do**: 不得实现会修改世界状态或 Redis 的工具；不得允许无限循环；不得让 tool call 绕过 `parse.ts` / schema validate。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 涉及协议、循环控制、验证与错误预算的硬约束。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 11,14,15 | Blocked By: 8

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:247-302` — B5 的 Tool 接口与 3 轮上限要求。
  - Pattern: `agent/packages/tiandao/src/llm.ts:47-121` — 现有请求与错误控制面。
  - Pattern: `agent/packages/tiandao/src/agent.ts:59-89` — 当前单 agent 调用入口。
  - Pattern: `agent/packages/tiandao/src/context.ts:156-260` — worldModel 可提供 peer/key player/world trend 等只读上下文。
  - Pattern: `agent/packages/tiandao/src/world-model.ts:128-275` — 可复用的只读查询面。
  - Test: `agent/packages/tiandao/tests/llm.test.ts:45-162` — loop/backoff 测试基础。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- tests/llm.test.ts tests/agent.test.ts tests/tool-calling.test.ts` 通过。
  - [ ] 新增测试证明：tool loop 最多执行 3 轮；超过即截断并产生可观测错误。
  - [ ] 新增测试证明：重复 toolName + args 的调用不会在同一轮里重复执行。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path read-only tool loop resolves within budget
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/tool-calling.test.ts tests/llm.test.ts`; capture the success path
    Expected: tools 执行后模型返回合法 JSON 决策；循环轮次 ≤ 3；telemetry 记录 tool usage
    Evidence: .sisyphus/evidence/task-9-tool-loop.log

  Scenario: Invalid args and repeated calls are safely rejected
    Tool: Bash
    Steps: run the new tests for malformed tool args, repeated identical tool calls, and loop overflow
    Expected: 非法 args 返回结构化错误；重复调用被去重；超过 3 轮强制终止
    Evidence: .sisyphus/evidence/task-9-tool-loop-error.log
  ```

  **Commit**: YES | Message: `feat(tiandao): add bounded read-only tool calling` | Files: `agent/packages/tiandao/src/{llm.ts,agent.ts,tools/types.ts}`, `agent/packages/tiandao/tests/{tool-calling.test.ts,llm.test.ts,agent.test.ts}`

- [x] 10. 更新 skill prompts 与 runtime wiring，使工具说明与模型安全边界一致

  **What to do**: 修改 `skills/calamity.md`、`mutation.md`、`era.md` 与 `agent.ts` 装配逻辑，把工具使用说明、模型名、只读限制、预算约束写入 prompt/runtime。必须完成：
  1. 在 calamity prompt 中声明 `query-player`、`list-active-events` 的使用时机；
  2. 在 mutation prompt 中声明 `query-zone-history` 的使用时机；
  3. 在 era prompt 中明确“默认无工具”；
  4. 统一写明“工具可选、只读、不得超过预算、最终仍必须输出单个合法 JSON 对象”；
  5. `agent.ts` 根据 agent config 装配对应 `tools[]` 与 effective model。

  **Must NOT do**: 不得放松现有 narration/JSON-only 约束；不得给 era 配无必要工具；不得把工具说明写成暗示必须每轮都调用。

  **Recommended Agent Profile**:
  - Category: `writing` — Reason: 这是 prompt 契约与 runtime 装配同步收口任务。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 11,14,15 | Blocked By: 3,9

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:286-295` — 不同 agent 的工具分配目标。
  - Pattern: `agent/packages/tiandao/src/skills/calamity.md:1-42` — 当前 calamity prompt。
  - Pattern: `agent/packages/tiandao/src/skills/mutation.md:1-42` — 当前 mutation prompt。
  - Pattern: `agent/packages/tiandao/src/skills/era.md:1-54` — 当前 era prompt。
  - Test: `agent/packages/tiandao/tests/prompts.test.ts:7-28` — prompt 文案锚点测试。
  - Test: `agent/packages/tiandao/tests/prompts-narration-era.test.ts:18-135` — narration/era prompt 回归测试。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- tests/prompts.test.ts tests/prompts-narration-era.test.ts tests/agent.test.ts` 通过。
  - [ ] 新增测试证明：prompt 中出现正确工具说明，且未破坏原有半文言/100-200 字/合法 JSON 约束。
  - [ ] `agent.ts` 会按 agent 配置装配对应工具集，不会把 era 错配为读工具调用者。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path prompt contracts remain explicit and valid
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/prompts.test.ts tests/prompts-narration-era.test.ts`
    Expected: 三个 skill prompt 仍满足 JSON-only 与 narration 契约，同时新增工具说明断言通过
    Evidence: .sisyphus/evidence/task-10-skill-prompts.log

  Scenario: Era agent remains tool-free by design
    Tool: Bash
    Steps: run the new targeted agent wiring test for era config and prompt inspection
    Expected: era 不会装配查询工具，且提示词未鼓励工具调用
    Evidence: .sisyphus/evidence/task-10-skill-prompts-error.log
  ```

  **Commit**: YES | Message: `docs(tiandao): wire prompt tool guidance safely` | Files: `agent/packages/tiandao/src/{agent.ts,skills/*.md}`, `agent/packages/tiandao/tests/{prompts.test.ts,prompts-narration-era.test.ts,agent.test.ts}`

- [x] 11. 实现三个只读内置工具并补齐 tool-level 测试

  **What to do**: 实现 `query-player`、`query-zone-history`、`list-active-events` 三个只读工具，并使其输出对 LLM 稳定友好。必须完成：
  1. `query-player`：按 player id/name 返回 breakdown、位置、zone、recent_kills/deaths、新手保护信号；
  2. `query-zone-history`：返回最近 N 轮 spirit_qi/danger_level 趋势与简单摘要；
  3. `list-active-events`：返回所有 zone 的 active events 与去重摘要；
  4. 工具结果统一为紧凑 JSON string，长度可控；
  5. 每个工具都补 happy path + not found/invalid param 测试。

  **Must NOT do**: 不得查询 Redis 或外部网络；不得返回不可预测大块文本；不得因为“找不到目标”抛出未捕获异常。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 是 B5 的具体实现收口层。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 14,15 | Blocked By: 5,9,10

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:267-295` — 三个工具的目标定义。
  - Pattern: `agent/packages/tiandao/src/world-model.ts:128-275` — `getZoneHistory/getZoneTrendSummary/getKeyPlayers/getPeerDecisions` 等只读数据源。
  - Pattern: `agent/packages/tiandao/src/context.ts:173-260` — 趋势/平衡/关键人物表述方式，可复用输出摘要语言。
  - Test: `agent/packages/tiandao/tests/world-model.test.ts:64-200` — world-model fixture 模板。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- tests/tools.test.ts tests/world-model.test.ts` 通过。
  - [ ] 每个工具都有 invalid params / not found 覆盖，且返回结构化错误结果。
  - [ ] 工具输出在 deterministic fixtures 上稳定，不依赖时间漂移。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path tools return compact deterministic JSON
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/tools.test.ts`; inspect snapshots / assertions for all three tools
    Expected: 三个工具均返回结构化 JSON string，字段稳定、长度可控
    Evidence: .sisyphus/evidence/task-11-tools.log

  Scenario: Invalid params and missing entities fail soft
    Tool: Bash
    Steps: run the new not-found / invalid-param tests for each tool
    Expected: 返回错误结果而不是 throw，测试通过
    Evidence: .sisyphus/evidence/task-11-tools-error.log
  ```

  **Commit**: YES | Message: `feat(tiandao): add readonly query tools` | Files: `agent/packages/tiandao/src/tools/{types.ts,query-player.ts,query-zone-history.ts,list-active-events.ts}`, `agent/packages/tiandao/tests/tools.test.ts`

- [x] 12. 增加 narration 规则评分与评估报告 CLI

  **What to do**: 按 B7 新增 `narration-eval.ts`，实现规则评分、tick 内打分与批量评估 CLI。必须完成：
  1. `scoreNarration(text, style)` 至少输出 `lengthOk/hasOmen/noModernSlang/styleMatch/score`；
  2. `runTick()` merge 后为每条 narration 评分，并写入 telemetry；
  3. 低于阈值的 narration 在日志中打固定 `⚠️`/`narration_low_score` 锚点；
  4. 增加 `npm run eval-narrations` 或等价 script，读取最近样本并输出 ASCII 分布；
  5. 使用 deterministic 样本测试“好/差 narration”评分。

  **Must NOT do**: 不得把 LLM 评审做成必需依赖；不得让评估逻辑修改业务输出；不得让 B7 阻塞 B1-B6 成功路径。

  **Recommended Agent Profile**:
  - Category: `artistry` — Reason: 需要兼顾文字规则与工程可验证性，但保持克制。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 14,15 | Blocked By: 3,4

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:348-385` — B7 的规则评分与报告目标。
  - Pattern: `agent/packages/tiandao/src/skills/{calamity.md,mutation.md,era.md}` — narration 风格约束真源。
  - Test: `agent/packages/tiandao/tests/prompts.test.ts:7-28` — “半文言半白话 / 100-200 / omen / JSON-only” 契约。
  - Test: `agent/packages/tiandao/tests/prompts-narration-era.test.ts:47-135` — narration 样例语义测试。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- tests/narration-eval.test.ts tests/prompts.test.ts` 通过。
  - [ ] `npm run eval-narrations` 能在无外部依赖下输出分布与问题模式。
  - [ ] 低分 narration 会进入 telemetry / warning，而不会改变原始业务 payload。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path narration samples score as expected
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- tests/narration-eval.test.ts tests/prompts.test.ts`
    Expected: 好样本得分高于差样本；字段断言与 style 规则全部通过
    Evidence: .sisyphus/evidence/task-12-narration-eval.log

  Scenario: Low-quality narration is flagged but not blocked
    Tool: Bash
    Steps: run the new test where poor narration is scored below threshold during tick processing
    Expected: 记录低分告警和 metrics，但 tick 不失败
    Evidence: .sisyphus/evidence/task-12-narration-eval-error.log
  ```

  **Commit**: YES | Message: `feat(tiandao): add narration quality evaluation` | Files: `agent/packages/tiandao/src/{narration-eval.ts,telemetry.ts,runtime.ts}`, `agent/packages/tiandao/tests/narration-eval.test.ts`, `agent/packages/tiandao/package.json`

- [x] 13. 重构现有 smoke 资产为 `scripts/e2e-redis.sh` 与 `scripts/smoke-test-e2e.sh`

  **What to do**: 以现有 `scripts/smoke-tiandao-fullstack.sh` 和 `scripts/smoke-law-engine.sh` 为基础，落地文档要求的两个脚本，不从零重写。必须完成：
  1. `scripts/e2e-redis.sh`：最小闭环，只验证 Redis + server + 非 mock agent 1 次 tick；
  2. `scripts/smoke-test-e2e.sh`：完整 smoke，包含 schema、agent、server、e2e、cleanup、证据输出；
  3. 将 deterministic fake LLM /固定 world_state 注入保留为默认路径，避免远程 LLM 不稳定；
  4. 把现有 world bootstrap、redis subscribed、world_state proof、typed narration proof、merged command proof 等 anchor 迁移成新脚本断言；
  5. 产出统一 evidence manifest。

  **Must NOT do**: 不得删掉现有 smoke 脚本里唯一的有价值 proof；不得让脚本依赖手工启动 Redis；不得要求人工看游戏画面。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要整合现有脚本资产与新的 runtime/redis 保证。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 当前不是浏览器任务。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 14,15 | Blocked By: 1,2,6,7,8,9

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:75-88,309-345` — B1.4 与 B6 的脚本目标。
  - Pattern: `scripts/smoke-tiandao-fullstack.sh:350-510` — 现有 fullstack runtime proof 和 evidence 输出骨架。
  - Pattern: `scripts/smoke-law-engine.sh:106-183` — 现有 contract/server/agent 分阶段 smoke 模式。
  - Pattern: `scripts/smoke-test.sh:1-48` — 基础 smoke 命名与汇总方式。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `bash scripts/e2e-redis.sh` 全程绿灯。
  - [ ] `bash scripts/smoke-test-e2e.sh` 全程绿灯。
  - [ ] 两个脚本都会输出 `.sisyphus/evidence/` 下的可复查日志路径。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path minimal redis e2e passes deterministically
    Tool: Bash
    Steps: run `bash scripts/e2e-redis.sh`; capture stdout and referenced logs
    Expected: world_state、agent_command、agent_narrate、server execution anchors 全部命中
    Evidence: .sisyphus/evidence/task-13-e2e-redis.log

  Scenario: Failure path produces actionable evidence manifest
    Tool: Bash
    Steps: intentionally break one required anchor in the harness test mode, rerun `bash scripts/smoke-test-e2e.sh`, capture failure bundle
    Expected: 脚本非零退出，并保留明确的阶段名、日志路径、失败上下文
    Evidence: .sisyphus/evidence/task-13-e2e-redis-error.log
  ```

  **Commit**: YES | Message: `test(e2e): add redis and fullstack smoke harnesses` | Files: `scripts/{e2e-redis.sh,smoke-test-e2e.sh,smoke-tiandao-fullstack.sh,smoke-law-engine.sh}`

- [x] 14. 新增 `docker-compose.test.yml` 与 GitHub Actions `e2e.yml`

  **What to do**: 为本地与 CI 统一提供 Redis test 环境和分层流水线。必须完成：
  1. 新增 `docker-compose.test.yml`，至少提供 `redis:7-alpine`；
  2. 新增 `.github/workflows/e2e.yml`，触发条件至少覆盖 `agent/**`、`server/**`、`scripts/**` 相关变更；
  3. workflow 采用分步执行：schema/test → agent/test → server/test → smoke-test-e2e；
  4. 缓存 Rust target、Node modules（若环境允许），并上传 smoke/e2e 日志工件；
  5. 把 deterministic mode 设为默认，不依赖真实 LLM secrets。

  **Must NOT do**: 不得让 CI workflow 只跑大一统黑盒脚本而无中间定位信息；不得要求 Docker 之外的额外服务；不得在 main 分支 push 时强制真实联网调用。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 涉及本地/CI 同构执行与产物上传策略。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 15 | Blocked By: 4,9,10,11,12,13

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent-v2.md:312-339` — B6 对 compose + workflow 的目标定义。
  - Pattern: `scripts/smoke-tiandao-fullstack.sh:350-510` — 现有 fullstack proof，可映射为 CI smoke 入口。
  - Pattern: `scripts/smoke-law-engine.sh:106-183` — 现有分阶段 smoke 结构，可复用为 workflow steps。
  - Pattern: `scripts/smoke-test.sh:1-48` — 基础 smoke 的汇总与退出风格。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `test -f docker-compose.test.yml && test -f .github/workflows/e2e.yml` 返回 0。
  - [ ] workflow YAML 通过基本语法检查，且引用的脚本路径全部存在。
  - [ ] 本地可用 `docker compose -f docker-compose.test.yml up -d redis` 启动 Redis 并被新脚本消费。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path CI config and compose file are internally consistent
    Tool: Bash
    Steps: run `docker compose -f docker-compose.test.yml up -d redis`; then run `bash scripts/smoke-test-e2e.sh`; finally bring compose down
    Expected: compose 服务可启动，workflow引用的脚本和路径在本地全部成立
    Evidence: .sisyphus/evidence/task-14-ci-wiring.log

  Scenario: Workflow fails loudly when a referenced script is missing
    Tool: Bash
    Steps: run a local validation script or targeted grep assertions against `.github/workflows/e2e.yml` to ensure referenced files exist; capture failure-mode output
    Expected: 缺失路径会被本地校验步骤及时发现，而不是在 CI 深处才暴露
    Evidence: .sisyphus/evidence/task-14-ci-wiring-error.log
  ```

  **Commit**: YES | Message: `ci(e2e): add redis compose and workflow gate` | Files: `docker-compose.test.yml`, `.github/workflows/e2e.yml`, `scripts/smoke-test-e2e.sh`

- [ ] 15. 最终收口：故障注入验证、回归矩阵与证据清单

  **What to do**: 在全部实现完成后，用单一回归矩阵证明整个方案达到“生产最小安全线”。必须完成：
  1. 执行 schema/agent/server/e2e 全套回归；
  2. 覆盖故障注入：stale world_state、Redis reconnect、重复 command batch、损坏 persisted state、tool loop overflow、低分 narration；
  3. 汇总 evidence manifest，按任务编号列出日志、关键锚点与退出码；
  4. 对照本计划逐项勾验收标准，任何未满足项不得进入最终 commit wave；
  5. 形成最终 smoke 结论文件，供 F1-F4 四路最终审查使用。

  **Must NOT do**: 不得跳过 failure path；不得只给“全部通过”的口头描述而无 evidence；不得把人工试玩替代为正式验证。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 这是全局收口与证据整理任务。
  - Skills: `[]` — 无额外技能。
  - Omitted: `["playwright"]` — 当前不是浏览器主任务。

  **Parallelization**: Can Parallel: NO | Wave 3 | Blocks: Final Verification Wave | Blocked By: 1,2,3,4,5,6,7,8,9,10,11,12,13,14

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `.sisyphus/plans/agent-runtime-production-hardening.md:1-839` — 以本文件所有 acceptance/QA 为最终检查清单。
  - Pattern: `scripts/smoke-tiandao-fullstack.sh:350-510` — 当前 fullstack evidence 汇总模板。
  - Pattern: `scripts/smoke-law-engine.sh:106-183` — 当前分阶段 smoke 证明模板。
  - Pattern: `scripts/smoke-test.sh:1-48` — 当前基础 smoke 汇总模板。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/schema && npm test && cd ../tiandao && npm run check && npm test && cd ../../../server && cargo test && cd .. && bash scripts/e2e-redis.sh && bash scripts/smoke-test-e2e.sh` 全绿。
  - [ ] `.sisyphus/evidence/task-15-final-matrix.log` 与 `.sisyphus/evidence/task-15-final-manifest.txt` 存在且完整。
  - [ ] 回归矩阵明确列出每类故障注入的通过证据。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path full matrix passes
    Tool: Bash
    Steps: run the full regression chain defined in Acceptance Criteria; save combined stdout/stderr and manifest
    Expected: 所有层级测试与两个 smoke 脚本全部通过，manifest 完整可追踪
    Evidence: .sisyphus/evidence/task-15-final-matrix.log

  Scenario: Failure paths are explicitly represented in final evidence
    Tool: Bash
    Steps: collect the outputs from the targeted failure-injection tests added in tasks 3/5/6/7/9/12/13/14 into a final manifest
    Expected: manifest 中能定位每类失败注入的证据文件，而不是只保留 happy path
    Evidence: .sisyphus/evidence/task-15-final-manifest.txt
  ```

  **Commit**: NO | Message: `test(runtime): add final production-hardening matrix` | Files: `[]`

## Final Verification Wave (4 parallel agents, ALL must APPROVE)
- [ ] F1. Plan Compliance Audit — oracle
- [ ] F2. Code Quality Review — unspecified-high
- [ ] F3. Real Manual QA — unspecified-high (+ playwright if UI)
- [ ] F4. Scope Fidelity Check — deep

## Commit Strategy
- 每个任务完成后单独提交一个原子 commit；禁止把多个任务混入同一提交。
- 只有在本任务 acceptance criteria 全绿后才允许提交。
- 推荐消息风格：`feat(tiandao): ...`、`fix(server): ...`、`test(e2e): ...`、`ci(e2e): ...`、`refactor(runtime): ...`
- 跨语言契约相关提交优先拆分为：schema/tests → agent/server consumer → scripts/CI，便于回滚。

## Success Criteria
- 现有 `agent/packages/tiandao` 与 `server/` 测试保持通过，且新增测试能证明 stale-state skip、恢复、去重、tool loop 上限与 narration scoring。
- `scripts/e2e-redis.sh` 与 `scripts/smoke-test-e2e.sh` 在无真实 LLM 的前提下稳定通过。
- server 不再对 `realm_collapse` / `karma_backlash` 打印 “not implemented” 并空跑；至少能提供可验证的占位行为。
- CI 对 `agent/**`、`server/**`、`scripts/**` 变更自动守护，并上传证据日志/工件。
- 整个方案在不更换 Redis 架构的前提下，具备生产最小安全线：可观测、可恢复、可诊断、可自动验收。
