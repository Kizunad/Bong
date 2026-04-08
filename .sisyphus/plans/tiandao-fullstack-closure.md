# Tiandao Fullstack Closure — Redis IPC、Schema 对齐与三层闭环总计划

## TL;DR
> **Summary**: 以 `docs/plan-agent.md` 为主轴，把为其闭环所必需的 `server/`、`client/`、`agent/packages/schema` 与 `agent/packages/tiandao` 任务统一收口到同一份执行计划中，先完成可见的 M1 闭环，再递进到 M2 世界语义与 M3 修仙体验。
> **Deliverables**:
> - 新 worktree：`/workspace/worktrees/Bong-tiandao-fullstack-closure`，分支：`atlas/tiandao-fullstack-closure`
> - `@bong/schema` 统一的 Redis + Client payload 契约、样例、生成物与 runtime validate 导出
> - `tiandao` 的 arbiter/chat/world-model/balance/era 全链路与 Vitest 测试基线
> - `server/` 的 world_state/chat/command executor/narration&payload bridge/zone&event/player_state 实现
> - `client/` 的 payload router、narration/toast、zone HUD、cultivation UI 与 JUnit 验证
> **Effort**: XL
> **Parallel**: YES - 5 waves
> **Critical Path**: 1 → 2 → 4 → 5 → 13 → 14 → 15 → 16 → 18 → 21 → 22 → 24

## Context
### Original Request
- 用户最初要求围绕 `docs/plan-agent.md` 编写 Redis IPC + schema 对齐路线，并要求新开 worktree。
- 后续明确测试策略采用 **TDD + tests-after 混合**。
- 随后用户将范围扩大为：**凡 `docs/plan-agent.md` 牵出的 server/client 相关工作，全部纳入同一计划，后续 merge 再统一修复。**
- 用户已明确：**Momus Review 必须自动执行，不需要再询问。**

### Interview Summary
- worktree 采用固定模式：目录 `/workspace/worktrees/Bong-tiandao-fullstack-closure`，分支 `atlas/tiandao-fullstack-closure`，沿用现有 `atlas/*` 命名风格与 `/workspace/worktrees/*` 路径风格。
- 测试策略固定为：**纯逻辑/契约/调度使用 TDD**；**Redis/Valence/Fabric 联线、运行时 smoke、端到端闭环使用 tests-after**。
- Agent 依赖的真实 Server 输入与真实 Client 展示仍纳入同一总计划，但每个跨层任务都必须先具备 **契约就绪 + mock/sample/manual Redis 注入** 的独立验收路径。
- Redis 聊天队列采用**单消费者 advisory 信号**语义；目标是**不丢并发新写入**，接受 **at-most-once** 处理，不引入 Streams 或 processing queue 复杂度。
- Client 传输统一收敛到 `bong:server_data` 单一 CustomPayload 通道，按 `type` 路由，不额外引入第二条 server→client 通道。

### Metis Review (gaps addressed)
- 已固定跨层热点决策：`bong:server_data` 使用**统一版本化 envelope**，client 不再停留在 `{v,type,message}` 简单模型，而是升级为 schema 驱动的 typed union。
- 已固定 Redis chat drain 护栏：禁止裸 `LRANGE + DEL` / 裸 `LRANGE + LTRIM`；采用 tiandao 侧原子 drain 命令，避免并发写入丢失。
- 已固定 `AgentCommandV1.source` 兼容策略：arbiter 合并后的批次**省略 `source`**，内部来源标签使用私有包装类型；不把 `"arbiter"` 强行写进现有公共契约。
- 已固定 M1 交付边界：先完成“一条可见闭环路径”——merged command + 最小 server 执行 + client narration 显示；Mixin 天象特效、动态 UI 等远期项不阻塞主线。
- 已固定 acceptance 策略：所有验收必须可由代理执行；“进入游戏看效果”只能作为补充 smoke，不得作为唯一完成判据。

## Work Objectives
### Core Objective
- 交付一份**决策完成型**全栈实施蓝图，使执行代理在单一 worktree 中完成 `schema → agent → server → client` 的 Redis IPC / CustomPayload / 世界状态 / 天道叙事 / 修仙状态闭环，并在无人工判断的前提下完成验证与证据沉淀。

### Deliverables
- `agent/packages/schema/`：`client-payload` 契约、`validate`/runtime export、samples、generated schemas、negative cases。
- `agent/packages/tiandao/src/`：`arbiter.ts`、`chat-processor.ts`、`world-model.ts`、`balance.ts` 与对 `main.ts/context.ts/agent.ts/redis-ipc.ts/parse.ts/skills/*.md` 的升级。
- `server/src/network/`：`command_executor.rs`、`chat_collector.rs`、升级后的 `redis_bridge.rs` / `mod.rs`；`server/src/world/`、`player/`、`npc/` 的配套扩展。
- `client/src/main/java/com/bong/client/`：分层 network router、narration handler、toast/zone HUD/cultivation UI。
- `scripts/`：新增全链路 smoke 脚本与 evidence 产出约定。

### Definition of Done (verifiable conditions with commands)
- `git worktree list` 显示 `/workspace/worktrees/Bong-tiandao-fullstack-closure` 与 `atlas/tiandao-fullstack-closure`
- `cd agent/packages/schema && npm run check && npm test && npm run generate`
- `cd agent/packages/tiandao && npm run check && npm test && npm run start:mock`
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `cd client && ./gradlew test build`
- `bash scripts/smoke-test.sh`
- `bash scripts/smoke-tiandao-fullstack.sh`

### Must Have
- `@bong/schema` 是**唯一**跨层契约真源；Redis IPC 与 `bong:server_data` payload 不允许绕开 schema 自说自话。
- Client 统一复用 `bong:server_data` 通道，并通过 `type` 路由 `welcome | heartbeat | narration | zone_info | event_alert | player_state`。
- Tiandao 所有 Redis ingress 在进入业务逻辑前都执行 runtime schema validate；失败消息要记录结构化错误并安全丢弃。
- Server 端对 agent command 再做一次防御式校验（数量、参数、target、范围、clamp），不能只信任 agent。
- M1 至少完成：真实 world_state、chat 采集、arbiter merged publish、最小 command executor、server→client narration payload、client narration 渲染。
- M2 至少完成：zone authoritative source、世界趋势/平衡态、基础事件调度、zone HUD。
- M3 至少完成：key player / era state、player persistence、player_state payload、cultivation UI、基础 progression。

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- 不引入 Redis Streams、消息 ack 队列、第二套 IPC 层、WebSocket 或 HTTP side-channel。
- 不为了让 arbiter 写入 `source="arbiter"` 而破坏现有 `AgentCommandV1` 契约兼容性。
- 不把 `Command.params` 在 M1-M3 中重构成全新复杂 DSL；仍用现有 Record 契约，但在 agent/server 两端补防御式 validator。
- 不把 optional/远期项当主线阻塞：`docs/plan-client.md` 的 `C3 天象视觉反馈` 与 `C7 动态 UI 下发` 默认排除在本计划之外。
- 不扩展到 `docs/roadmap.md` 的 M4 宗门/多人社交，也不升级 MC/Fabric/Valence 版本线。
- 不要求人工进入游戏做唯一验收；所有任务必须有可自动执行的 QA 场景与 evidence 路径。

## Verification Strategy
> ZERO HUMAN INTERVENTION — all verification is agent-executed.
- Test decision:
  - **TDD**：schema contract、TypeBox validate、arbiter merge rules、chat drain wrappers、world-model trend、balance Gini、payload parser/router、zone lookup、event scheduler、player-state serde。
  - **tests-after**：Redis bridge wiring、Valence ECS side effects、Anvil fallback、server→client payload flow、client build/runtime smoke、fullstack Redis E2E。
- QA policy: 每个任务都必须同时包含 happy path 和 failure/edge case；Minecraft/Fabric 不用浏览器验证，统一使用 Bash / cargo test / vitest / JUnit / scripted Redis publish。
- Evidence: `.sisyphus/evidence/task-{N}-{slug}.{ext}`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave. <3 per wave (except final) = under-splitting.
> Extract shared dependencies as Wave-1 tasks for max parallelism.

Wave 1: 基础契约、worktree、测试基线（Tasks 1-5）

Wave 2: M1 agent/server substrate（Tasks 6-11）

Wave 3: M1 端到端闭环（Tasks 12-15）

Wave 4: M2 世界语义与区域体验（Tasks 16-19）

Wave 5: M3 修仙体验与最终收口（Tasks 20-24）

### Dependency Matrix (full, all tasks)
| Task | Depends On | Blocks |
|---|---|---|
| 1 | - | 2,3,4,5,6 |
| 2 | 1 | 4,8,9,11,17,20,24 |
| 3 | 1 | 8,9,10,11,17,20,24 |
| 4 | 1,2 | 5,13,18,19,21,23,24 |
| 5 | 1,2,4 | 13,15,24 |
| 6 | 1 | 7,12,16,18,21,22,24 |
| 7 | 6 | 15,16,24 |
| 8 | 2,3 | 10,15,24 |
| 9 | 2,3 | 10,15,24 |
| 10 | 3,8,9 | 11,15,17,20,24 |
| 11 | 2,3,10 | 15,17,24 |
| 12 | 6 | 15,18,24 |
| 13 | 4,5 | 14,15,19,23,24 |
| 14 | 13 | 15,19,24 |
| 15 | 5,7,8,9,10,11,12,13,14 | 16,20,21,24 |
| 16 | 7,15 | 18,19,21,22,24 |
| 17 | 10,11,15 | 20,24 |
| 18 | 4,6,12,16 | 19,21,24 |
| 19 | 13,14,18 | 23,24 |
| 20 | 3,10,15,17 | 24 |
| 21 | 4,6,15,16,18 | 22,23,24 |
| 22 | 6,16,21 | 24 |
| 23 | 13,19,21 | 24 |
| 24 | 2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23 | Final Verification Wave |

### Agent Dispatch Summary (wave → task count → categories)
- Wave 1 → 5 tasks → `git` (1), `ultrabrain` (2,4), `deep` (3,5)
- Wave 2 → 6 tasks → `ultrabrain` (6,7,10), `deep` (8,9,11)
- Wave 3 → 4 tasks → `deep` (12,15), `ultrabrain` (13), `visual-engineering` (14)
- Wave 4 → 4 tasks → `deep` (16,17,18), `visual-engineering` (19)
- Wave 5 → 5 tasks → `artistry` (20), `deep` (21,22,24), `visual-engineering` (23)

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task MUST have: Agent Profile + Parallelization + QA Scenarios.

- [x] 1. 创建固定 worktree、分支与执行基线

  **What to do**: 在开始任何实现前创建独立 worktree：`/workspace/worktrees/Bong-tiandao-fullstack-closure`，分支固定为 `atlas/tiandao-fullstack-closure`，基于 `main`。把 worktree 路径、当前分支、主工作树保持只读规划态、`.sisyphus/evidence/` 命名规则、各子项目的基线命令统一写入 evidence。后续所有实现、测试、构建、smoke、提交都只能在该 worktree 中发生。
  **Must NOT do**: 不得在 `/workspace/Bong` 主工作树直接修改代码；不得改用临时 worktree 名或非 `atlas/*` 分支；不得在未确认 base branch 为 `main` 时开始实现。

  **Recommended Agent Profile**:
  - Category: `git` — Reason: 这是纯 git/worktree 预备任务，必须把执行位置固定死。
  - Skills: `[]` — 无额外技能依赖。
  - Omitted: `["playwright"]` — 无浏览器工作流。

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 2,3,4,5,6 | Blocked By: -

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `.sisyphus/plans/tiandao-fullstack-closure.md:4-13` — 已固定 worktree 路径、分支名与关键路径。
  - Pattern: `CLAUDE.md:1-34` — 仓库三层结构与常用命令基线。
  - Pattern: `scripts/start.sh:1-60` — 现有 Redis/Server/Agent 编排入口，后续 smoke 要复用其约定。
  - Pattern: `scripts/stop.sh:1-7` — 现有停止脚本与 Redis/tmux 清理路径。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `git worktree list | grep "/workspace/worktrees/Bong-tiandao-fullstack-closure"` 返回目标 worktree。
  - [ ] `git -C "/workspace/worktrees/Bong-tiandao-fullstack-closure" branch --show-current` 输出 `atlas/tiandao-fullstack-closure`。
  - [ ] `.sisyphus/evidence/task-1-worktree.txt` 记录 worktree 路径、当前分支、`git status --short`、base branch = `main`。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path worktree bootstrap
    Tool: Bash
    Steps: run `git worktree add -b atlas/tiandao-fullstack-closure /workspace/worktrees/Bong-tiandao-fullstack-closure main`; run `git -C /workspace/worktrees/Bong-tiandao-fullstack-closure status --short`; save outputs to `.sisyphus/evidence/task-1-worktree.txt`
    Expected: worktree 创建成功；目标分支为 `atlas/tiandao-fullstack-closure`；主工作树保持计划/证据专用
    Evidence: .sisyphus/evidence/task-1-worktree.txt

  Scenario: Duplicate worktree path is rejected
    Tool: Bash
    Steps: rerun the same `git worktree add ...` command after creation and capture stderr
    Expected: 命令非零退出且不会覆盖已有 worktree；错误输出写入 `.sisyphus/evidence/task-1-worktree-error.txt`
    Evidence: .sisyphus/evidence/task-1-worktree-error.txt
  ```

  **Commit**: NO | Message: `chore(repo): create tiandao fullstack worktree baseline` | Files: `[]`

- [x] 2. 加固 `@bong/schema` 真源、runtime validate 导出与 freshness gate

  **What to do**: 只在 `agent/packages/schema` 中加固基础契约层，不碰业务实现。必须完成三件事：
  1. 从 `src/index.ts` 直接导出 `validate` 与后续 client payload 所需的 shared constants/type helpers；
  2. 为 `generated/` 建立 freshness gate，要求 `npm run generate` 后 `generated/*.json` 不得漂移；
  3. 为现有 `world-state / agent-command / narration / chat-message` 增加 rejection coverage（错误版本、缺字段、非法枚举、超限长度/数量）。
  本任务不引入 `bong:server_data` 新 payload union；那部分由 Task 4 单独处理。
  **Must NOT do**: 不得把 Rust mirror 或 Java parser 变成真源；不得新增绕过 schema 包的 ad-hoc JSON 常量；不得修改 `MAX_PAYLOAD_BYTES`、`MAX_COMMANDS_PER_TICK` 等共享常量语义。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 需要把导出边界、生成物与测试门禁一次收紧。
  - Skills: `[]` — 依赖现有 TypeScript/Vitest 即可。
  - Omitted: `["playwright"]` — 无 UI 验证。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 4,8,9,11,17,20,24 | Blocked By: 1

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `agent/packages/schema/src/index.ts:1-11` — 当前导出入口尚未暴露 `validate`。
  - Pattern: `agent/packages/schema/src/validate.ts:1-22` — 现有 TypeBox runtime validate 包装器。
  - Pattern: `agent/packages/schema/src/common.ts:18-26` — `MAX_COMMANDS_PER_TICK`、`MAX_NARRATION_LENGTH`、`MAX_PAYLOAD_BYTES` 真源。
  - Test: `agent/packages/schema/tests/schema.test.ts:1-122` — 现有 sample/rejection 测试组织方式。
  - Pattern: `agent/packages/schema/src/generate.ts:1-34` — generated JSON Schema 导出入口。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/schema && npm run check && npm test && npm run generate` 全绿。
  - [ ] `cd agent/packages/schema && npm run generate && git diff --exit-code -- generated` 返回 0。
  - [ ] `@bong/schema` 包根可直接 `import { validate } from "@bong/schema"`，无需引用 `src/validate.ts` 私有路径。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Schema source of truth stays green
    Tool: Bash
    Steps: run `cd agent/packages/schema && npm run check && npm test && npm run generate`; save stdout/stderr to `.sisyphus/evidence/task-2-schema.log`
    Expected: 类型检查、Vitest、JSON Schema 生成全部成功
    Evidence: .sisyphus/evidence/task-2-schema.log

  Scenario: Stale generated artifacts are caught
    Tool: Bash
    Steps: intentionally alter one file under `agent/packages/schema/generated/`, then run `npm run generate && git diff --exit-code -- generated`; capture failure output before restoring the file
    Expected: freshness gate 明确报出 generated 漂移，不会静默通过
    Evidence: .sisyphus/evidence/task-2-schema-error.txt
  ```

  **Commit**: YES | Message: `chore(schema): export runtime validate and add freshness gate` | Files: `agent/packages/schema/src/index.ts`, `agent/packages/schema/src/validate.ts`, `agent/packages/schema/tests/**`, `agent/packages/schema/generated/**`, `agent/packages/schema/package.json`

- [x] 3. 为 `tiandao` 建立离线测试基线与真正可用的 `start:mock`

  **What to do**: 给 `agent/packages/tiandao` 增加可离线运行的测试脚手架（Vitest），并重构入口依赖注入，使 `--mock` 模式在没有 `.env`、没有 Redis、没有真实 LLM 的情况下也能单次跑通。必须完成：
  1. `package.json` 新增 `test` script；
  2. `main.ts` 中把 LLM client 初始化放到非 mock 分支内；
  3. 提供 fake LLM/fake clock/fake Redis publish sink 供单测与 mock smoke 复用；
  4. 输出稳定日志 marker，便于后续 smoke 用 `grep` 断言 merged tick 完成。
  **Must NOT do**: 不得让单测依赖真实网络、真实 Redis、真实 `.env`；不得在 mock 分支里读取缺失 env 后直接 `process.exit(1)`；不得把 `npm test` 做成只调用 `tsc --noEmit` 的伪测试。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要重构入口依赖与测试方式，但不改变对外运行语义。
  - Skills: `[]` — 直接使用 TypeScript/Vitest 即可。
  - Omitted: `["playwright"]` — 无浏览器内容。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 8,9,10,11,17,20,24 | Blocked By: 1

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `agent/packages/tiandao/package.json:1-25` — 当前没有 `npm test`。
  - Pattern: `agent/packages/tiandao/src/main.ts:22-35` — 当前 env 校验在 mock 分支之前执行。
  - Pattern: `agent/packages/tiandao/src/main.ts:99-107` — 当前 `--mock` 入口。
  - Pattern: `agent/packages/tiandao/src/mock-state.ts:1-73` — mock world state 现有样例。
  - Pattern: `docs/plan-agent.md:356-360` — tiandao 预期测试策略（单测 + mock mode + 集成）。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm run check && npm test` 通过。
  - [ ] `cd agent/packages/tiandao && env -u LLM_BASE_URL -u LLM_API_KEY npm run start:mock` 退出 0。
  - [ ] 单测可覆盖 fake LLM / fake publish sink，且不访问真实 Redis。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Offline tiandao test harness works
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm run check && npm test`; save output to `.sisyphus/evidence/task-3-tiandao-tests.log`
    Expected: 类型检查与单测全部通过，`npm test` 已成为有效脚本
    Evidence: .sisyphus/evidence/task-3-tiandao-tests.log

  Scenario: Mock mode no longer requires .env
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && env -u LLM_BASE_URL -u LLM_API_KEY npm run start:mock`; capture stdout/stderr
    Expected: 进程以 mock world state 正常执行一轮并退出 0，日志中出现稳定 tick-complete marker
    Evidence: .sisyphus/evidence/task-3-tiandao-mock.log
  ```

  **Commit**: YES | Message: `test(agent): add tiandao offline harness and env-free mock mode` | Files: `agent/packages/tiandao/package.json`, `agent/packages/tiandao/src/main.ts`, `agent/packages/tiandao/src/**`, `agent/packages/tiandao/tests/**`

- [x] 4. 定义 typed `bong:server_data` 契约并固定 1024B payload 策略
  **What to do**: 在 `@bong/schema` 中新增专门的 client payload schema（文件名固定为 `client-payload.ts`），作为 `bong:server_data` 的唯一 typed envelope。必须只包含六类 `type`：`welcome | heartbeat | narration | zone_info | event_alert | player_state`。同时把 payload size 策略固定死：
  1. 继续使用 `MAX_PAYLOAD_BYTES = 1024`；
  2. `welcome` / `heartbeat` 维持轻量 message 形态；
  3. `narration` payload 使用 `narrations` 字段，但 `maxItems = 1`，即 server 每个 payload 只发送一条 narration；
  4. `zone_info` / `event_alert` / `player_state` 采用嵌套对象，不再塞进平铺 `message`；
  5. unknown `type`、错误版本、缺少嵌套对象、序列化后超出 1024B 的样例必须有 rejection tests。
  本任务只定义 TypeBox、samples、generated 与 shared TS exports，不实现 Rust/Java 消费代码。
  **Must NOT do**: 不得引入 `ui_open` 或任何动态 UI payload；不得把 `narration` 继续定义成无限长数组；不得提升 `MAX_PAYLOAD_BYTES`；不得保留第二条 server→client channel。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是后续 server/client 全部实现的跨层硬契约。
  - Skills: `[]` — 依赖 TypeBox 与现有 sample/generate 流程即可。
  - Omitted: `["playwright"]` — 无浏览器验证。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 5,13,18,19,21,23,24 | Blocked By: 1,2

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `agent/packages/schema/src/common.ts:24-26` — 当前 `MAX_PAYLOAD_BYTES = 1024` 真源。
  - Pattern: `server/src/network/agent_bridge.rs:6-27` — 当前 legacy `{v,type,message}` payload 形态与 1024B 限制。
  - Pattern: `client/src/main/java/com/bong/client/BongNetworkHandler.java:33-91` — 当前 client 只接受 `{v,type,message}`。
  - Pattern: `docs/plan-client.md:25-80` — narration payload 的目标路由方向。
  - Pattern: `docs/plan-client.md:161-220` — `zone_info` 的目标字段集。
  - Pattern: `docs/plan-client.md:266-299` — `player_state` 目标字段范围。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/schema && npm test` 覆盖六类 payload 的 sample/rejection cases。
  - [ ] `cd agent/packages/schema && npm run generate && git diff --exit-code -- generated` 返回 0。
  - [ ] `@bong/schema` 根导出包含 `ClientPayloadV1`（或等价命名的 typed union）及其子 payload type。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Typed client payloads accept only the six supported kinds
    Tool: Bash
    Steps: run `cd agent/packages/schema && npm test`; capture the client-payload-specific suite output
    Expected: `welcome | heartbeat | narration | zone_info | event_alert | player_state` sample 全部通过
    Evidence: .sisyphus/evidence/task-4-client-payload.log

  Scenario: Oversize or unknown client payloads are rejected
    Tool: Bash
    Steps: run rejection tests for `type: "unknown"` and for a serialized payload over 1024 bytes; capture failure output
    Expected: 校验失败并明确指出非法 `type` 或 oversize
    Evidence: .sisyphus/evidence/task-4-client-payload-error.txt
  ```

  **Commit**: YES | Message: `feat(schema): add typed bong server data contract` | Files: `agent/packages/schema/src/client-payload.ts`, `agent/packages/schema/src/index.ts`, `agent/packages/schema/tests/**`, `agent/packages/schema/samples/**`, `agent/packages/schema/generated/**`

- [x] 5. 建立 Rust mirror 与 Java fixtures 对 `client-payload` 的跨层对齐

  **What to do**: 以 Task 4 新增的 TS client payload 契约为准，同时补齐 Rust serde mirror 与 Java fixture parsing 基线，但不在本任务实现真正的 server/client 行为。必须完成：
  1. `server/src/schema/` 新增与 `client-payload.ts` 1:1 对应的 Rust mirror 模块并接入 `schema/mod.rs`；
  2. 为 Rust mirror 增加 sample/roundtrip tests；
  3. 在 `client/src/test/` 新增基于 JSON fixture 的 envelope parse tests，覆盖所有六类 payload；
  4. 统一 fixtures 名称，确保 sample drift 能被同一批测试发现。
  **Must NOT do**: 不得在本任务里实现 network router、HUD、server payload builder；不得让 Java 测试重新发明独立 JSON 结构。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要把 TS 真源、Rust mirror、Java fixtures 同时拉齐但不提前做业务。
  - Skills: `[]` — 依赖 serde/JUnit 现有模式。
  - Omitted: `["playwright"]` — 无浏览器验证。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 13,15,24 | Blocked By: 1,2,4

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `server/src/schema/mod.rs:1-10` — Rust schema mirror 模块注册入口。
  - Pattern: `server/src/schema/world_state.rs:76-107` — Rust sample/roundtrip test 风格。
  - Pattern: `server/src/schema/agent_command.rs:23-51` — Rust include_str! sample mirror 模式。
  - Pattern: `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java:8-64` — Java 现有 payload parser 测试基线。
  - Pattern: `agent/packages/schema/samples/*.json` — 现有 shared sample 目录结构。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test schema::` 通过新增的 client payload mirror tests。
  - [ ] `cd client && ./gradlew test --tests "com.bong.client.*Payload*"` 通过六类 payload fixture parse tests。
  - [ ] TS sample 变更后，Rust/Java 对齐测试会同步暴露漂移。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Rust mirror matches TS client payload samples
    Tool: Bash
    Steps: run `cd server && cargo test schema:: -- --nocapture`; save output to `.sisyphus/evidence/task-5-rust-mirror.log`
    Expected: client payload sample 与 roundtrip mirror tests 全部通过
    Evidence: .sisyphus/evidence/task-5-rust-mirror.log

  Scenario: Java fixtures reject invalid typed payloads
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "com.bong.client.*Payload*"`; capture the invalid fixture assertions
    Expected: malformed/unknown/unsupported-version fixtures 被拒绝，不出现未捕获异常
    Evidence: .sisyphus/evidence/task-5-java-fixtures.log
  ```

  **Commit**: YES | Message: `test(crosslayer): align rust and java payload fixtures` | Files: `server/src/schema/**`, `client/src/test/**`, `agent/packages/schema/samples/**`

- [x] 6. 拆分 server `world/player` 模块并建立 fallback `spawn` zone 基线

  **What to do**: 把平铺的 `server/src/world.rs` 与 `server/src/player.rs` 重构为模块目录，至少形成：`world/mod.rs`、`world/zone.rs`、`player/mod.rs`。在这个过程中引入最小 `ZoneRegistry`：
  - 无配置文件时自动提供单区 `spawn` AABB；
  - `publish_world_state_to_redis`、后续 command executor、chat collector 都统一通过 `ZoneRegistry` 查找 zone；
  - 保持现有出生点 `[8.0, 66.0, 8.0]`、Adventure 模式、欢迎消息不变。
  本任务只做结构重组 + fallback zone，不实现 Anvil、事件系统、持久化。
  **Must NOT do**: 不得把 `zones.json` 变成启动前置条件；不得在本任务引入真正的 Anvil 读取；不得改变玩家出生点或移除当前欢迎消息。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是后续 server 多任务的模块基础与 dependency root。
  - Skills: `[]` — 现有 Rust/Valence 代码足够提供模式。
  - Omitted: `["playwright"]` — 无 UI 验证。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 7,12,16,18,21,22,24 | Blocked By: 1

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `server/src/world.rs:1-48` — 当前 world 初始化逻辑。
  - Pattern: `server/src/player.rs:1-67` — 当前玩家 init/cleanup 与 spawn 基线。
  - Pattern: `docs/plan-server.md:181-209` — `ZoneRegistry` 目标轮廓。
  - Pattern: `docs/plan-server.md:299-323` — server 目录规划。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test fallback_spawn_zone_exists && cargo test spawn_defaults_are_preserved` 通过。
  - [ ] `cd server && cargo test` 中不再引用旧平铺 `world.rs` / `player.rs` 入口。
  - [ ] 在没有任何 zone 配置的情况下，`ZoneRegistry` 仍返回名为 `spawn` 的默认区域。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Fallback spawn zone exists after module split
    Tool: Bash
    Steps: run `cd server && cargo test fallback_spawn_zone_exists -- --nocapture && cargo test spawn_defaults_are_preserved -- --nocapture`; save output to `.sisyphus/evidence/task-6-server-modules.log`
    Expected: 测试确认 spawn zone 存在，出生坐标仍为 `[8.0, 66.0, 8.0]`
    Evidence: .sisyphus/evidence/task-6-server-modules.log

  Scenario: Missing zone config does not block boot
    Tool: Bash
    Steps: run `cd server && cargo test world::tests::missing_zone_config_uses_spawn_fallback -- --nocapture`
    Expected: fallback 行为通过测试，不 panic，不要求额外文件
    Evidence: .sisyphus/evidence/task-6-server-modules-error.txt
  ```

  **Commit**: YES | Message: `refactor(server): split world and player modules with spawn zone fallback` | Files: `server/src/world/**`, `server/src/player/**`, `server/src/main.rs`

- [x] 7. 统一 server world bootstrap 为 fallback-first，并为未来 Anvil 预留单入口

  **What to do**: 在 `server/src/world/mod.rs` 中定义统一 bootstrap 入口与配置资源，明确两条路径：`FallbackFlat`（当前默认）与 `AnvilIfPresent`（未来 Task 16 启用）。当前实现只要求：
  - 当未配置 world path 或 region 目录不存在时，始终安全回退到当前 16x16 草地测试世界；
  - 启动日志必须明确打印选中的 bootstrap 模式；
  - 后续读取 zone/事件/玩家状态的系统只依赖统一 world bootstrap 完成后的 layer/resource，不直接依赖平坦世界假设。
  **Must NOT do**: 不得在本任务真正加载 `.mca`；不得因缺少 world 目录而启动失败；不得把 zone 初始化、事件初始化、Redis 桥接耦合进 bootstrap 模块。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 需要把世界初始化与后续功能解耦，避免未来 Anvil 替换时重写所有系统。
  - Skills: `[]` — 现有 world 初始化即可作为 fallback 参考。
  - Omitted: `["playwright"]` — 非 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 15,16,24 | Blocked By: 6

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `server/src/world.rs:1-48` — 当前平坦世界实现。
  - Pattern: `docs/plan-server.md:155-177` — Anvil/fallback 目标结构。
  - Pattern: `docs/local-test-env.md:63-79` — 现有 server 启动期望日志。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test world::tests::selects_fallback_without_region_dir` 通过。
  - [ ] `cd server && timeout 15s cargo run` 在无 region 目录时仍输出 fallback world 选择日志并进入主循环。
  - [ ] fallback world 行为不影响当前 smoke-test 的 world creation 断言。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Missing region directory still boots fallback world
    Tool: Bash
    Steps: run `cd server && cargo test world::tests::selects_fallback_without_region_dir -- --nocapture`; then run `timeout 15s cargo run` and capture logs
    Expected: 测试通过；运行日志明确包含 fallback bootstrap marker
    Evidence: .sisyphus/evidence/task-7-world-bootstrap.log

  Scenario: Invalid configured world path degrades gracefully
    Tool: Bash
    Steps: start server with a deliberately invalid world path config/env and capture stderr/stdout
    Expected: server 记录 warning 并继续使用 fallback flat world，而不是 panic
    Evidence: .sisyphus/evidence/task-7-world-bootstrap-error.log
  ```

  **Commit**: YES | Message: `refactor(server): add fallback-first world bootstrap` | Files: `server/src/world/**`, `server/src/main.rs`, `server/src/tests/**`

- [x] 8. 实现 `arbiter.ts`，替换 per-agent publish 为 merged publish

  **What to do**: 在 `agent/packages/tiandao/src/arbiter.ts` 中实现真正的合并层，并把 `main.ts` 改为：所有 sub-agent 决策先进入 arbiter，再统一 publish。仲裁规则固定为：
  1. `spawn_event` 同 zone 冲突按优先级 `era > mutation > calamity` 选 1 条；
  2. `modify_zone` 同 zone 聚合 delta；
  3. 灵气净变化若绝对值 > 0.01，则按比例缩放到接近 0；
  4. 对 `composite_power < NEWBIE_POWER_THRESHOLD` 的新手区不允许高强度 `spawn_event`；
  5. 最终 `commands.length <= MAX_COMMANDS_PER_TICK`；
  6. 最终 publish 到 Redis 的 `AgentCommandV1` 公共载荷必须省略 `source`，且绝不能包含私有 `_source`。
  narrations 全量保留，但每 tick 发布前必须裁到不会突破后续 server/client payload 策略的可消费上限。
  **Must NOT do**: 不得继续让每个 agent 单独调用 `publishCommands(agent.name, ...)`；不得把 `source: "arbiter"` 写进公共 schema；不得把 `_source` 泄漏到 Redis。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要同时重构 `agent.ts`、`main.ts`、`redis-ipc.ts` 的 publish 契约。
  - Skills: `[]` — 现有 parse/context/mock 数据足够驱动单测。
  - Omitted: `["playwright"]` — 无 UI 验证。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 10,15,24 | Blocked By: 2,3

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent.md:33-87` — arbiter 目标规则与 `main.ts` 集成方向。
  - Pattern: `agent/packages/tiandao/src/main.ts:57-93` — 当前 per-agent publish 位置。
  - Pattern: `agent/packages/tiandao/src/agent.ts:67-72` — 当前 `_source` 注入位置。
  - Pattern: `agent/packages/schema/src/agent-command.ts:20-31` — `source` 只能是三 agent literal union，不能写 `arbiter`。
  - Pattern: `agent/packages/schema/src/common.ts:15-25` — `NEWBIE_POWER_THRESHOLD` 与 `MAX_COMMANDS_PER_TICK`。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- arbiter` 通过冲突合并、净灵气缩放、命令截断、新手保护测试。
  - [ ] `cd agent/packages/tiandao && env -u LLM_BASE_URL -u LLM_API_KEY npm run start:mock` 的日志显示 merged publish，而非 per-agent publish。
  - [ ] `grep -R 'source: "arbiter"' agent/packages/tiandao/src` 无结果，发布载荷单测也断言不存在 `_source`。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Arbiter merges conflicting decisions deterministically
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- arbiter`; save output to `.sisyphus/evidence/task-8-arbiter.log`
    Expected: 同 zone 冲突、delta 合并、预算截断、新手保护规则全部通过
    Evidence: .sisyphus/evidence/task-8-arbiter.log

  Scenario: Public command payload never leaks arbiter/private source tags
    Tool: Bash
    Steps: run targeted unit tests asserting serialized publish payload omits `source` and `_source`; grep source tree for `source: "arbiter"`
    Expected: 单测通过且 grep 无匹配
    Evidence: .sisyphus/evidence/task-8-arbiter-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): add arbiter merged publish pipeline` | Files: `agent/packages/tiandao/src/arbiter.ts`, `agent/packages/tiandao/src/main.ts`, `agent/packages/tiandao/src/agent.ts`, `agent/packages/tiandao/src/redis-ipc.ts`, `agent/packages/tiandao/tests/**`

- [x] 9. 实现原子 `player_chat` drain 与 `chat-processor.ts`

  **What to do**: 在 `agent/packages/tiandao/src/redis-ipc.ts` 中实现原子 chat drain，并新增 `chat-processor.ts` 负责把原始聊天列表转成 `ChatSignal[]`。固定决策如下：
  1. `bong:player_chat` 仍用 Redis List；
  2. 禁止裸 `LRANGE + DEL` 或裸 `LRANGE + LTRIM`；必须使用 Lua `EVAL` / `defineCommand` 完成“读取当前 list + 一次性删空该批次”的原子操作；
  3. 并发写入在 drain 期间发生时，新写入必须保留在 list 中供下次消费；
  4. 非法 JSON / 不符合 `ChatMessageV1` 的记录要结构化记录后丢弃，不阻塞本轮；
  5. 廉价标注模型先用 fake annotator + deterministic heuristic 支持离线测试，真实 LLM annotator 仅在 runtime wiring 中替换。
  **Must NOT do**: 不得引入 Redis Streams、BRPOP worker、processing queue；不得让 chat processing 成为阻塞主 tick 的前提；不得把原始聊天直接塞进 context 而不做预处理。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 涉及 Redis 原子语义、runtime validate、annotator abstraction 三块联动。
  - Skills: `[]` — 现有 `validate`、`ChatMessageV1`、mock harness 足够支撑。
  - Omitted: `["playwright"]` — 无 UI 验证。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 10,15,24 | Blocked By: 2,3

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent.md:90-130` — chat processor 目标流程。
  - Pattern: `agent/packages/schema/src/chat-message.ts:6-25` — `ChatMessageV1` / `ChatSignal` 契约。
  - Pattern: `agent/packages/schema/src/validate.ts:1-22` — runtime validate 统一入口。
  - Pattern: `agent/packages/schema/src/channels.ts:5-17` — `PLAYER_CHAT` 真源常量。
  - Pattern: `agent/packages/tiandao/src/redis-ipc.ts:59-99` — 当前 Redis publish/subscribe 封装位置。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- chat` 通过 atomic drain、invalid JSON discard、validated ChatSignal 生成测试。
  - [ ] 并发写入测试证明“在 drain 中后写入的消息仍留在 list 中”。
  - [ ] `redis-ipc.ts` 中不再出现裸 `lrange(...); del(...)` 或 `lrange(...); ltrim(...)` 的 chat drain 实现。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Atomic chat drain preserves concurrent writes
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- chat`; save output to `.sisyphus/evidence/task-9-chat-drain.log`
    Expected: 并发写入测试通过，旧批次被 drain，而新写入保留到下一轮
    Evidence: .sisyphus/evidence/task-9-chat-drain.log

  Scenario: Invalid chat payloads are dropped safely
    Tool: Bash
    Steps: run targeted tests with malformed JSON and schema-invalid chat entries; capture stderr/stdout
    Expected: 非法消息被记录并丢弃，不会导致 tick 失败
    Evidence: .sisyphus/evidence/task-9-chat-drain-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): add atomic chat drain and chat processor` | Files: `agent/packages/tiandao/src/redis-ipc.ts`, `agent/packages/tiandao/src/chat-processor.ts`, `agent/packages/tiandao/tests/**`

- [x] 10. 引入 `world-model.ts`、`balance.ts` 与扩展后的 context blocks

  **What to do**: 在 `tiandao` 中新增长期状态层，把 `latestState`、`zoneHistory`、`chatSignals`、`lastDecisions`、`currentEra`、balance summary 统一沉淀到 `WorldModel`。同时扩展 `context.ts`，加入以下 blocks：
  - `chatSignalsBlock`
  - `peerDecisionsBlock`
  - `worldTrendBlock`
  - `balanceBlock`
  - `keyPlayerBlock`
  Recipes 固定为：`calamity` 最关注 `keyPlayer + playerProfiles`，`mutation` 最关注 `worldTrend + zoneSnapshot`，`era` 最关注 `worldTrend + balance + peerDecisions`。`balance.ts` 只实现 Gini 与结构化 advice，不做业务动作。
  **Must NOT do**: 不得在本任务修改 skills prompt 文案；不得让 context blocks 直接调用 Redis/LLM；不得把任何 block 写成依赖人工解释的 free-form string 拼凑而无测试。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是天道“有记忆、有平衡视角”的核心语义层。
  - Skills: `[]` — 依赖现有 `context.ts`、`mock-state.ts`、arbiter/chat processor 输出。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 11,15,17,20,24 | Blocked By: 3,8,9

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent.md:166-233` — peer decisions / world trend 目标结构。
  - Pattern: `docs/plan-agent.md:237-273` — balance Gini 目标结构。
  - Pattern: `docs/plan-agent.md:288-308` — key player / current era 目标方向。
  - Pattern: `agent/packages/tiandao/src/context.ts:8-136` — 现有 block/recipe 架构。
  - Pattern: `agent/packages/tiandao/src/mock-state.ts:7-73` — 测试数据可直接驱动趋势与 balance。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- world-model balance context` 通过世界趋势、Gini、recipe 裁剪测试。
  - [ ] `assembleContext(...)` 在扩展后仍可 deterministic 输出，不依赖真实 LLM。
  - [ ] `WorldModel` 可在连续多轮 mock state 更新后正确保留最近 N 轮 zone history。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: World model computes trend and balance deterministically
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- world-model balance context`; save output to `.sisyphus/evidence/task-10-world-model.log`
    Expected: zone trend、Gini、key player、recipe priority 裁剪测试全部通过
    Evidence: .sisyphus/evidence/task-10-world-model.log

  Scenario: Empty or sparse history degrades gracefully
    Tool: Bash
    Steps: run targeted tests where there is no chat, one tick only, or zero players; capture results
    Expected: 输出合理空块/默认值，不抛异常，不生成 NaN/Infinity
    Evidence: .sisyphus/evidence/task-10-world-model-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): add world model balance and contextual memory` | Files: `agent/packages/tiandao/src/world-model.ts`, `agent/packages/tiandao/src/balance.ts`, `agent/packages/tiandao/src/context.ts`, `agent/packages/tiandao/tests/**`

- [x] 11. 强化 `RedisIpc` ingress validation、重连日志与 tick runtime 护栏

  **What to do**: 只做 agent runtime 护栏，不扩业务范围。必须完成：
  1. `world_state` ingress 一律 `JSON.parse -> validate(WorldStateV1, payload)`，失败时记录结构化错误并丢弃；
  2. publish path 对 `AgentCommandV1` / `NarrationV1` 做发前 validate，禁止非法对象上 Redis；
  3. LLM 调用超时、连续失败退避、Redis 断连/重连日志统一；
  4. tick 级日志固定包含耗时、命令数、叙事数、chat signal 数、是否 skip。
  **Must NOT do**: 不得把 invalid payload 自动“修正后继续”；不得让某一 sub-agent LLM 失败拖垮整个主循环；不得在日志里打印 API key 或整段敏感 prompt。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 这一步是把 agent 从 demo 升级为可长时间运行的 runtime。
  - Skills: `[]` — 现有 mock harness 与 schema validate 已足够。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 15,17,24 | Blocked By: 2,3,10

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent.md:133-163` — 循环模式稳定化目标。
  - Pattern: `agent/packages/tiandao/src/redis-ipc.ts:32-49` — 当前 `world_state` 仅 `JSON.parse as WorldStateV1`。
  - Pattern: `agent/packages/tiandao/src/main.ts:57-136` — 当前 tick loop 与 shutdown 路径。
  - Pattern: `agent/packages/schema/src/validate.ts:1-22` — runtime validate 入口。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- redis-ipc runtime` 通过 ingress/publish validation 与 retry/backoff tests。
  - [ ] `env -u LLM_BASE_URL -u LLM_API_KEY npm run start:mock` 日志包含 tick markers 但不泄漏敏感信息。
  - [ ] invalid `world_state` 注入时 process 不崩溃，且日志中可 grep 到 validation warning。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Invalid world_state is rejected without crashing the loop
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- redis-ipc runtime`; save output to `.sisyphus/evidence/task-11-runtime-guards.log`
    Expected: invalid ingress 被丢弃；tick loop 继续；日志包含 validation marker
    Evidence: .sisyphus/evidence/task-11-runtime-guards.log

  Scenario: Repeated LLM failures trigger bounded backoff, not process exit
    Tool: Bash
    Steps: run targeted unit tests with fake client throwing repeatedly; capture results
    Expected: 连续失败会退避并 skip，但不会导致主进程退出
    Evidence: .sisyphus/evidence/task-11-runtime-guards-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): add runtime validation and loop guards` | Files: `agent/packages/tiandao/src/main.ts`, `agent/packages/tiandao/src/redis-ipc.ts`, `agent/packages/tiandao/src/llm.ts`, `agent/packages/tiandao/tests/**`

- [x] 12. 新建 `server/src/network/command_executor.rs` 并把 inbound command 改为排队执行

  **What to do**: 在 server 侧把 `RedisInbound::AgentCommand` 从“仅日志打印”升级为“入队 + system 执行”。必须实现：
  - `CommandExecutorResource` 或等价队列资源；
  - `process_redis_inbound` 只负责 validate 后 push 进队列；
  - `execute_agent_commands` system 在 Update 中逐帧消费固定上限；
  - M1 只支持三类最小行为：
    1. `modify_zone`：调整 `ZoneRegistry` 的 `spirit_qi` / `danger_level` 并 clamp；
    2. `spawn_event(event="thunder_tribulation")`：先以可测试的 server event resource 记录 active event，而不是直接把复杂特效硬编码到 inbound；
    3. `npc_behavior`：最小仅支持 `flee_threshold` 更新到 NPC runtime config。
  同时，server 必须在执行前做二次防御式校验：target zone 存在、delta/intensity/clamp 合法、参数 key/value 类型可接受。
  **Must NOT do**: 不得在 `process_redis_inbound` 中直接操作 ECS 世界；不得盲信 agent 已验证过 payload；不得在 M1 里实现 `realm_collapse` / `karma_backlash` 全量效果。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要把 inbound network、ZoneRegistry、NPC runtime config 串成可扩展执行管线。
  - Skills: `[]` — 现有 `npc/brain.rs` 测试模式与 `network/mod.rs` 足够参考。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 15,18,24 | Blocked By: 6

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-server.md:31-72` — command executor 目标结构。
  - Pattern: `server/src/network/mod.rs:120-175` — 当前 inbound command/narration 分支。
  - Pattern: `server/src/schema/agent_command.rs:6-21` — Rust command mirror。
  - Pattern: `server/src/npc/brain.rs:13-16` — 当前 flee threshold 基线，可为 runtime config 抽象提供常量参考。
  - Pattern: `server/src/world/zone.rs`（Task 6 产物） — zone 查找与 clamp 应基于该资源。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test network::command_executor::` 通过 zone modify、invalid target reject、flee-threshold update tests。
  - [ ] `cd server && cargo test` 中 `process_redis_inbound` 不再只有日志输出分支。
  - [ ] 任何非法 command batch 都不会 panic，且被记录为 warning/skip。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Command executor applies valid zone and npc updates
    Tool: Bash
    Steps: run `cd server && cargo test network::command_executor:: -- --nocapture`; save output to `.sisyphus/evidence/task-12-command-executor.log`
    Expected: `modify_zone` 与最小 `npc_behavior` 更新测试通过
    Evidence: .sisyphus/evidence/task-12-command-executor.log

  Scenario: Invalid command targets are rejected safely
    Tool: Bash
    Steps: run targeted tests with missing zone, wrong params, or excessive intensity; capture results
    Expected: 命令被跳过并记录 warning，不会 panic，不会写入非法状态
    Evidence: .sisyphus/evidence/task-12-command-executor-error.txt
  ```

  **Commit**: YES | Message: `feat(server): add queued agent command executor` | Files: `server/src/network/command_executor.rs`, `server/src/network/mod.rs`, `server/src/tests/**`, `server/src/npc/**`, `server/src/world/**`

- [x] 13. 用 typed `bong:server_data` 重写 client network router，淘汰 `JsonCursor`

  **What to do**: 在 client 侧把 `BongNetworkHandler` 重构成“receiver + router + payload models”三层，但保留 Fabric 1.20.1 的 raw-bytes receiver 方式。固定实现策略：
  1. `registerGlobalReceiver(new Identifier("bong", "server_data"), ...)` 保持不变；
  2. 接收端继续 `buf.readableBytes()` → `byte[]` → `UTF_8 String`；
  3. 使用 Gson / Minecraft 内置 JSON 解析 typed envelope；
  4. 支持六类 payload：`welcome | heartbeat | narration | zone_info | event_alert | player_state`；
  5. unknown type、wrong version、malformed JSON 一律 graceful no-op + log，而非抛异常；
  6. 删除或完全旁路当前 `JsonCursor` 简化解析器，不再扩展它。
  本任务只交付 router/parse/dispatch 骨架与 JUnit，不做 HUD/UI 状态变更。
  **Must NOT do**: 不得使用 `buf.readString(...)`；不得继续维护平行的 `JsonCursor` 嵌套 JSON 解析分支；不得在 router 中直接画 HUD 或打开 UI。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是 client 后续所有可视化功能的解析基座。
  - Skills: `[]` — 依赖现有 JUnit 与 Fabric receiver 即可。
  - Omitted: `["playwright"]` — 非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 14,15,19,23,24 | Blocked By: 4,5

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `client/src/main/java/com/bong/client/BongNetworkHandler.java:12-247` — 当前 raw bytes 读取与 `JsonCursor` 解析器。
  - Test: `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java:8-64` — 现有 JUnit 基线。
  - Pattern: `docs/plan-client.md:223-258` — 目标 router 注册表方向。
  - Pattern: `client/build.gradle:19-42` — 当前测试依赖，未引入额外 JSON 库。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd client && ./gradlew test --tests "com.bong.client.*Network*"` 通过六类 payload parse/router tests。
  - [ ] 代码中 active receiver path 不再调用 `JsonCursor`；`readString(` 不得出现在 `BongNetworkHandler` 接收路径。
  - [ ] malformed JSON / unsupported version / unknown type 都不会抛未捕获异常。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Typed payload router handles all supported kinds
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "com.bong.client.*Network*"`; save output to `.sisyphus/evidence/task-13-client-router.log`
    Expected: 六类 payload fixture 都能被正确 parse / dispatch
    Evidence: .sisyphus/evidence/task-13-client-router.log

  Scenario: Receiver still consumes raw bytes without readString
    Tool: Bash
    Steps: search the changed client tree for `readString(` and `JsonCursor`; save grep results
    Expected: active receiver path 不使用 `readString(`；`JsonCursor` 被删除或不再参与生产路径
    Evidence: .sisyphus/evidence/task-13-client-router-error.txt
  ```

  **Commit**: YES | Message: `feat(client-network): add typed bong server data router` | Files: `client/src/main/java/com/bong/client/**`, `client/src/test/java/com/bong/client/**`

- [x] 14. 实现 client 状态层、HUD 入口与 narration 渲染

  **What to do**: 在 client 侧建立最小渲染状态层，并把当前 `BongHud` 升级为复合 HUD 入口。M1 只要求：
  - `NarrationState`：保存最近 narration、style、过期时间；
  - `BongHud`：保留左上角 `Bong Client Connected` 基线，同时渲染 narration chat/toast 所需的轻量状态；
  - `NarrationHandler`：接收 `type = narration` payload，按 `style` 映射成聊天栏文本；
  - 仅 `system_warning` / `era_decree` 触发中央 toast，其余 narration 只入聊天栏。
  该任务不做 `zone_info` HUD、不做 `player_state` UI，不引入 mixin 特效。
  **Must NOT do**: 不得删除基线 HUD 文本；不得把 JSON parse 放进 render path；不得实现 `C3 天象视觉反馈` 或任何动态 UI。

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: 这是 M1 “看得见闭环”的唯一 client 可视输出。
  - Skills: `[]` — 依赖 typed router 与 HudRenderCallback 即可。
  - Omitted: `["playwright"]` — Minecraft client 不走浏览器。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 15,19,24 | Blocked By: 13

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `client/src/main/java/com/bong/client/BongHud.java:1-17` — 当前 baseline HUD。
  - Pattern: `client/src/main/java/com/bong/client/BongClient.java:11-17` — HUD 注册入口。
  - Pattern: `docs/plan-client.md:25-80` — narration 频道监听与渲染目标。
  - Pattern: `docs/plan-client.md:88-124` — toast 提示目标行为（仅作为 M1 子集参考）。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd client && ./gradlew test --tests "com.bong.client.*Narration*"` 通过 narration state/handler tests。
  - [ ] `cd client && ./gradlew test --tests "com.bong.client.*Hud*"` 通过 baseline + empty-state HUD tests。
  - [ ] `system_warning` / `era_decree` 与普通 narration 的渲染分流有明确单测覆盖。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Narration payload updates client chat/toast state correctly
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "com.bong.client.*Narration*"`; save output to `.sisyphus/evidence/task-14-narration-ui.log`
    Expected: style 分流、toast 触发与默认聊天输出全部通过测试
    Evidence: .sisyphus/evidence/task-14-narration-ui.log

  Scenario: HUD keeps baseline label with empty narration state
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "com.bong.client.*Hud*"`; capture empty-state assertions
    Expected: 无 payload 时仍显示 `Bong Client Connected`，不抛异常
    Evidence: .sisyphus/evidence/task-14-narration-ui-error.txt
  ```

  **Commit**: YES | Message: `feat(client): add narration rendering and baseline hud state` | Files: `client/src/main/java/com/bong/client/**`, `client/src/test/java/com/bong/client/**`

- [x] 15. 建立 M1 端到端闭环 smoke：world_state → arbiter → command queue → narration → client render

  **What to do**: 以 `scripts/smoke-tiandao-fullstack.sh` 为中心交付第一条“无人工判断”的全链路闭环。M1 smoke 必须完成以下步骤并留证：
  1. 启动 Redis、server、tiandao（允许使用 tmux 或后台进程，但脚本必须可重复执行/清理）；
  2. server 周期发布真实 `world_state`；
  3. tiandao 使用 mock/fake LLM 决策经 arbiter merged publish；
  4. server 接收 merged command 入队并处理最小 zone/event 变化；
  5. server 把 narration 转成 typed `bong:server_data` payload；
  6. client JUnit/fixture 层验证该 payload 可被 narration renderer 消费；
  7. 脚本输出统一 evidence：server log、agent log、Redis pub/sub snippet、client test output。
  **Must NOT do**: 不得把“进游戏看效果”当成唯一验收；不得依赖真实 LLM；不得把 smoke 建成只启动不校验的脚本。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 这是跨 schema/agent/server/client 的第一次真实闭环收口任务。
  - Skills: `[]` — 直接依赖前面 1-14 任务产物。
  - Omitted: `["playwright"]` — 非浏览器。

  **Parallelization**: Can Parallel: NO | Wave 3 | Blocks: 16,20,21,24 | Blocked By: 5,7,8,9,10,11,12,13,14

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `scripts/start.sh:1-60` — 当前 Redis+Server+Agent 编排入口。
  - Pattern: `scripts/stop.sh:1-7` — 当前清理路径。
  - Pattern: `scripts/smoke-test.sh:1-48` — 现有 smoke 脚本输出风格与证据模式。
  - Pattern: `.sisyphus/plans/tiandao-fullstack-closure.md:73-79` — agent-executed verification policy。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `bash scripts/smoke-tiandao-fullstack.sh` 退出 0。
  - [ ] smoke evidence 同时证明 world_state publish、merged command publish、server command queue 消费、narration payload build、client consume tests 通过。
  - [ ] `scripts/smoke-tiandao-fullstack.sh` 可重复执行两次且第二次不会因残留 Redis/tmux/session 状态失败。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Fullstack M1 smoke passes end-to-end
    Tool: Bash
    Steps: run `bash scripts/smoke-tiandao-fullstack.sh`; save combined output to `.sisyphus/evidence/task-15-fullstack-smoke.log`
    Expected: 脚本退出 0，并留下 world_state / arbiter / server executor / narration / client consumption 全部通过的证据
    Evidence: .sisyphus/evidence/task-15-fullstack-smoke.log

  Scenario: Smoke is idempotent after cleanup
    Tool: Bash
    Steps: run `bash scripts/smoke-tiandao-fullstack.sh` twice in succession with cleanup between runs; capture second-run output
    Expected: 第二次执行仍通过，不受残留 tmux/redis/session 状态影响
    Evidence: .sisyphus/evidence/task-15-fullstack-smoke-error.log
  ```

  **Commit**: YES | Message: `test(e2e): add tiandao m1 fullstack smoke` | Files: `scripts/smoke-tiandao-fullstack.sh`, `scripts/start.sh`, `scripts/stop.sh`, `.sisyphus/evidence/**`

- [x] 16. 接入 Anvil-if-present world bootstrap，但保留 fallback flat 作为默认退路

  **What to do**: 在 Task 7 的统一 world bootstrap 基础上，实现真正的 `AnvilIfPresent` 路径：
  - 当 `server/world/region/*.mca` 存在时尝试加载；
  - 加载失败时记录 warning 并自动回退到 fallback flat；
  - 不要求 Anvil 成功成为 CI/默认 smoke 前置；
  - zone registry 仍必须在 Anvil/flat 两种路径下工作。
  本任务只交付世界载入路径，不做 region-driven 事件/区域逻辑。
  **Must NOT do**: 不得让没有 `.mca` 文件的环境测试失败；不得把 Anvil 成为 `cargo test` 或默认 smoke 的强制依赖；不得把玩家 spawn/zone 初始化只绑定在 Anvil 世界。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 涉及 Valence world loading 与 fallback choreography。
  - Skills: `[]` — 以 Task 7 统一入口为基。
  - Omitted: `["playwright"]` — 非 UI。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 18,19,21,22,24 | Blocked By: 7,15

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-server.md:155-177` — Anvil 地形加载方向。
  - Pattern: `server/src/world/mod.rs`（Task 7 产物） — fallback/Anvil 统一入口。
  - Pattern: `CLAUDE.md:7-18` — 当前 server 版本线与 Valence pin。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test anvil_path_falls_back_on_error && cargo test flat_path_still_boots` 通过。
  - [ ] 在无 `server/world/region` 目录时，`timeout 15s cargo run` 仍成功进入主循环。
  - [ ] 在存在损坏/不可读 region 路径时，日志记录 warning 并回退 flat。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Broken or missing Anvil input falls back safely
    Tool: Bash
    Steps: run `cd server && cargo test anvil_path_falls_back_on_error -- --nocapture && cargo test flat_path_still_boots -- --nocapture`; save output to `.sisyphus/evidence/task-16-anvil.log`
    Expected: 缺失/损坏 Anvil 输入不会阻塞 server 启动
    Evidence: .sisyphus/evidence/task-16-anvil.log

  Scenario: Flat fallback still powers the smoke path
    Tool: Bash
    Steps: run `cd server && timeout 15s cargo run`; capture logs in a no-Anvil environment
    Expected: 服务器仍以 fallback world 启动并输出明确 marker
    Evidence: .sisyphus/evidence/task-16-anvil-error.log
  ```

  **Commit**: YES | Message: `feat(server-world): add anvil-if-present bootstrap path` | Files: `server/src/world/**`, `server/src/tests/**`

- [x] 17. 将 `currentEra` / key-player 语义接入 tiandao 持久上下文与 recipe 优先级

  **What to do**: 在 Task 10 的 world model 基础上，把 M3 所需的最小“时代 + 个体关注”语义接入 agent runtime，但仍保持行为最小化。必须完成：
  - `currentEra` 结构：`{ name, sinceTick, globalEffect }`；
  - `Era` agent 的 output 若宣布新纪元，可更新 `currentEra`；
  - `keyPlayerBlock` 使用 `composite_power`、`karma`、新加入玩家三类规则选出 1-3 位重点对象；
  - recipes 中固定 `calamity` 对 key player 优先级最高、`era` 对 `currentEra + worldTrend + balance` 最高。
  **Must NOT do**: 不得在此任务中自动向所有 zone 注入 era side-effect 命令；那属于 Task 20；不得改 skills prompt 风格文本。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 这是从 M2 语义层跨入 M3 叙事关注层的最小闭环。
  - Skills: `[]` — 依赖 Task 10/11 的 world model 与 runtime。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 20,24 | Blocked By: 10,11,15

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent.md:288-308` — key player / era 语义方向。
  - Pattern: `agent/packages/tiandao/src/context.ts:108-136` — recipe 现状。
  - Pattern: `agent/packages/tiandao/src/mock-state.ts:12-71` — key-player 选择的现成测试数据。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- era key-player` 通过 `currentEra` 更新与 key-player ranking tests。
  - [ ] recipes 的优先级变化有单测覆盖，不靠肉眼检查 prompt 文本。
  - [ ] 在无玩家或低信息状态下，`keyPlayerBlock` 退化为空但不报错。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Era and key-player memory behave deterministically
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- era key-player`; save output to `.sisyphus/evidence/task-17-era.log`
    Expected: `currentEra` 更新与 key-player 选择规则全部通过
    Evidence: .sisyphus/evidence/task-17-era.log

  Scenario: Sparse world data yields empty but valid key-player output
    Tool: Bash
    Steps: run targeted tests with zero players / no karma extremes / no era state
    Expected: 输出为空或默认态，不抛异常
    Evidence: .sisyphus/evidence/task-17-era-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): add era memory and key player prioritization` | Files: `agent/packages/tiandao/src/world-model.ts`, `agent/packages/tiandao/src/context.ts`, `agent/packages/tiandao/tests/**`

- [x] 18. 实现 `chat_collector.rs`、真实 world_state enrich 与 `zone_info` / `event_alert` payload builder

  **What to do**: 在 server 侧把世界感知与客户端可见语义补齐到 M2 级最小闭环。必须完成：
  1. 新建 `network/chat_collector.rs`，拦截玩家聊天并 `RPUSH bong:player_chat`；
  2. `publish_world_state_to_redis` 从真实客户端读取 `name/uuid/pos`，而不是 `Player{i}` 占位；
  3. 使用 `ZoneRegistry` 生成真实 `zone_info` payload；
  4. 从 active event resource 生成最小 `event_alert` payload；
  5. server 侧 payload builder 使用 Task 4 typed contract，并在 build 前检查 1024B 限制。
  关于聊天事件来源：执行代理必须以 pinned Valence 版本中**实际存在**的聊天/命令事件 API 为准实现；若无直接 chat 事件，则允许退化为命令/消息 packet interception，但必须在代码与测试中注明最终选用的真实 API 名称与原因。
  **Must NOT do**: 不得继续发布 `offline:player_{i}` / `Player{i}` 占位 world_state；不得用不存在的 Valence API 名称硬写代码；不得让 oversize payload 静默发送。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 这是 server 感知层与 client 可见语义层的关键桥梁。
  - Skills: `[]` — 依赖 Task 4/5/6/12/16 产物。
  - Omitted: `["playwright"]` — 无浏览器。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 19,21,24 | Blocked By: 4,6,12,16

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-server.md:75-125` — world_state enrich 与 chat collector 目标。
  - Pattern: `server/src/network/mod.rs:59-118` — 当前 world_state publish 占位逻辑。
  - Pattern: `server/src/schema/chat_message.rs:3-27` — `ChatMessageV1` Rust mirror。
  - Pattern: `server/src/player.rs:24-55` — 客户端初始化与 username 获取上下文。
  - Pattern: `agent/packages/schema/src/world-state.ts:18-72` — world_state 真源字段语义。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test chat_collector && cargo test payload_builder && cargo test world_state` 通过聊天采集、真实玩家名 world_state、`zone_info` / `event_alert` payload tests。
  - [ ] `timeout 15s cargo run` 的日志与 Redis evidence 能证明发布的 world_state 使用真实玩家标识而非 `Player{i}` 占位。
  - [ ] `zone_info` / `event_alert` payload builder 对 oversize 做 reject/log 而不是发送。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Server publishes real player identities and chat records
    Tool: Bash
    Steps: run `cd server && cargo test chat_collector -- --nocapture && cargo test world_state -- --nocapture`; save output to `.sisyphus/evidence/task-18-server-sense.log`
    Expected: world_state 使用真实 name/uuid；聊天消息可编码为 `ChatMessageV1`
    Evidence: .sisyphus/evidence/task-18-server-sense.log

  Scenario: Oversize zone/event payloads are rejected before send
    Tool: Bash
    Steps: run targeted payload-builder tests with deliberately oversized nested objects; capture results
    Expected: builder 返回 oversize error，server 只记录 warning 不发送
    Evidence: .sisyphus/evidence/task-18-server-sense-error.txt
  ```

  **Commit**: YES | Message: `feat(server): add chat collector and world payload builders` | Files: `server/src/network/chat_collector.rs`, `server/src/network/mod.rs`, `server/src/network/redis_bridge.rs`, `server/src/world/**`, `server/src/tests/**`

- [x] 19. 实现 `zone_info` / `event_alert` client handlers 与 Zone HUD

  **What to do**: 在 client 侧补齐 M2 可视反馈，但保持轻量。必须完成：
  - `ZoneState` / `EventAlertState` 只保存渲染必需字段；
  - `ZoneInfoHandler`、`EventAlertHandler` 接入 Task 13 router；
  - `BongZoneHud` 负责：区域名大字淡入、左上角常驻灵气条、危险等级标记；
  - `event_alert` 使用非阻塞 toast/banner 呈现，不引入 mixin 特效；
  - zone/event 显示逻辑的数值 clamp、alpha 退场、超长文本裁剪必须有纯 JUnit 覆盖。
  本任务是纯 client 可视化，不得回头修改 payload 契约。
  **Must NOT do**: 不得实现 `C3 天象视觉反馈` 或动态 XML UI；不得在渲染层直接解析 JSON；不得把 zone/event 逻辑塞回 `BongNetworkHandler` 巨类。

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: 这是 M2 世界语义在 client 上的直接可视结果。
  - Skills: `[]` — 依赖 Task 13 router 与 Task 18 payload builder。
  - Omitted: `["playwright"]` — Minecraft client 不走浏览器。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 23,24 | Blocked By: 13,14,18

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-client.md:161-220` — zone HUD 目标结构。
  - Pattern: `docs/plan-client.md:223-258` — payload router/handler 组织方向。
  - Pattern: `client/src/main/java/com/bong/client/BongHud.java:1-17` — HUD 总入口基线。
  - Pattern: `client/src/main/java/com/bong/client/BongClient.java:11-17` — HudRenderCallback 注册位置。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd client && ./gradlew test --tests "com.bong.client.*Zone*" --tests "com.bong.client.*EventAlert*"` 通过 zone/event handler 与 HUD tests。
  - [ ] zone 名、灵气值、危险等级越界时会在 state 层完成 clamp，不会让 HUD 崩溃。
  - [ ] `event_alert` 走独立 handler/state，而非复用 narration 文本硬塞聊天栏。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Zone payload updates HUD state and overlay text correctly
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "com.bong.client.*Zone*"`; save output to `.sisyphus/evidence/task-19-zone-hud.log`
    Expected: zone 信息更新、alpha 退场、灵气/危险等级显示测试全部通过
    Evidence: .sisyphus/evidence/task-19-zone-hud.log

  Scenario: Oversized or malformed event alerts degrade gracefully
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "com.bong.client.*EventAlert*"`; capture invalid payload assertions
    Expected: 非法/超长事件警报被裁剪或忽略，不会导致渲染异常
    Evidence: .sisyphus/evidence/task-19-zone-hud-error.txt
  ```

  **Commit**: YES | Message: `feat(client): add zone hud and event alert rendering` | Files: `client/src/main/java/com/bong/client/**`, `client/src/test/java/com/bong/client/**`

- [x] 20. 优化天道叙事提示词、半文言风格与时代宣告输出约束

  **What to do**: 把 `skills/*.md` 升级为适合长期运行的结构化叙事提示词，但不改变 JSON 输出协议。固定要求：
  - narration 语言风格为“半文言半白话”；
  - 单条 narration 文本长度控制在 100-200 中文字符以内，超出后仍受现有 `MAX_NARRATION_LENGTH` 二次裁剪；
  - 每条 narration 尽量包含“当前因果 + 预兆/下一轮暗示”；
  - `Era` agent 允许输出 `era_decree` narration，并把 `currentEra` 记忆纳入后续轮次上下文；
  - 提示词必须继续要求严格 JSON，不得回退成自由文本。
  同时为 prompt/output 加测试：用固定 fake LLM 响应校验 parse 后的 narration 风格约束、era_decree presence、命令 JSON 仍可被 parse。
  **Must NOT do**: 不得改变 `parseDecision` 的 JSON contract；不得把 style 约束下沉到 client/server；不得引入新的未在 schema 中定义的 narration style。

  **Recommended Agent Profile**:
  - Category: `artistry` — Reason: 这是少数以叙事品质为目标、但仍需严格受约束的任务。
  - Skills: `[]` — 依赖现有 skill markdown 与 parse tests。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 24 | Blocked By: 3,10,15,17

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-agent.md:279-308` — narration quality优化与 era 语义方向。
  - Pattern: `agent/packages/tiandao/src/parse.ts:21-49` — JSON 解析与长度裁剪边界。
  - Pattern: `agent/packages/schema/src/common.ts:21-22` — `MAX_NARRATION_LENGTH` 真源。
  - Pattern: `agent/packages/tiandao/src/skills/calamity.md`, `mutation.md`, `era.md` — 当前 system prompts。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- prompts narration era` 通过风格约束与 parse stability tests。
  - [ ] `parseDecision(...)` 仍能解析经更新 prompts 约束后的固定 JSON 样例。
  - [ ] `era_decree` narration 能通过 fake output tests 进入下游，而非仅存在于 prompt 文案里。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Prompt updates preserve strict JSON outputs
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- prompts narration era`; save output to `.sisyphus/evidence/task-20-prompts.log`
    Expected: 提示词更新后，固定 fake outputs 仍能稳定 parse 成 `AgentDecision`
    Evidence: .sisyphus/evidence/task-20-prompts.log

  Scenario: Invalid free-form narration responses are still rejected to empty decision
    Tool: Bash
    Steps: run targeted parse tests with deliberately non-JSON LLM outputs; capture results
    Expected: `parseDecision` 保持 graceful fallback，不因 prompt 升级而接受脏输出
    Evidence: .sisyphus/evidence/task-20-prompts-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): refine tiandao prompts and era decree narration` | Files: `agent/packages/tiandao/src/skills/*.md`, `agent/packages/tiandao/tests/**`

- [x] 21. 引入 `PlayerState`、持久化与 typed `player_state` server payload

  **What to do**: 在 server 侧建立最小可持续的修仙玩家状态层。必须完成：
  - 新增 `player/state.rs`（或等价文件）定义 `PlayerState` component：`realm`, `spirit_qi`, `spirit_qi_max`, `karma`, `experience`, `inventory_score`；
  - 玩家加入时按 uuid/load-or-init 挂载；断连和周期性 tick 落盘到 `server/data/players/{uuid}.json`；
  - `composite_power` 与 `world_state.players[*].breakdown` 从 `PlayerState` 投影；
  - server 周期下发 typed `player_state` payload 到对应 client；
  - payload builder 在序列化前做 size gate 与 field clamp。
  本任务只交付状态层与同步，不要求完整战斗/采集玩法。
  **Must NOT do**: 不得把 `PlayerState` 仅存于内存；不得让 `world_state` 继续硬编码 realm/composite_power 默认值；不得让 `player_state` payload 无目标路由或广播给全部玩家。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 涉及 ECS component、serde persistence、world_state 投影与 client payload 同步。
  - Skills: `[]` — 依赖 Task 4/6/12/15/16/18。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 22,23,24 | Blocked By: 4,6,15,16,18

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-server.md:271-289` — `PlayerState` 持久化目标字段。
  - Pattern: `server/src/player.rs:24-55` — 玩家 join/disconnect 生命周期入口。
  - Pattern: `agent/packages/schema/src/world-state.ts:18-30` — `PlayerProfile` 投影字段。
  - Pattern: `docs/plan-client.md:298-317` — `player_state` payload 最终消费方向。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test player::state && cargo test world_state` 通过 load/init/save/project tests。
  - [ ] `server/data/players/{uuid}.json` 可 roundtrip serde，不丢字段。
  - [ ] `player_state` payload 仅发给对应 client，且 oversize 时被拒绝并记录 warning。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Player state persists and projects into world_state correctly
    Tool: Bash
    Steps: run `cd server && cargo test player::state -- --nocapture && cargo test world_state -- --nocapture`; save output to `.sisyphus/evidence/task-21-player-state.log`
    Expected: load/init/save/project tests 全部通过，world_state 不再使用默认硬编码 realm/power
    Evidence: .sisyphus/evidence/task-21-player-state.log

  Scenario: Invalid or oversized player_state payload is not sent
    Tool: Bash
    Steps: run targeted payload-builder tests with oversized fields or missing uuid routing; capture results
    Expected: payload builder 返回错误并记录 warning，不会广播给所有 client
    Evidence: .sisyphus/evidence/task-21-player-state-error.txt
  ```

  **Commit**: YES | Message: `feat(server): add persisted player state and typed player payloads` | Files: `server/src/player/**`, `server/src/network/**`, `server/src/world/**`, `server/data/players/**`, `server/src/tests/**`

- [x] 22. 实现最小 progression engine：经验、境界晋升与 karma/qi clamp

  **What to do**: 在 `PlayerState` 基础上交付“可测试的基础 progression”，而不是完整战斗系统。固定实现边界：
  - 引入纯函数化 progression engine：realm thresholds、spirit_qi 上限变化、karma clamp、experience 累积；
  - server 侧提供最小 hook：来自 active event resolution、定时修炼 tick、以及可测试的 synthetic gain input；
  - 更新 `player_state` / `world_state` 投影，使 realm 与 qi 随 progression engine 变化；
  - 所有规则必须由 Rust 单测覆盖，而不是要求真人打怪/采集。
  该任务是“基础 progression engine”，不要求真正的物品采集、PVP、交易或技能树。
  **Must NOT do**: 不得把 M3 扩成完整 MMO 战斗系统；不得把 progression 仅写死在 UI 文本里；不得要求手动进入游戏触发境界突破来做唯一验收。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 这是数值/状态演化逻辑，需要强测试覆盖而非视觉效果。
  - Skills: `[]` — 依赖 Task 21 的 PlayerState persistence 与投影。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 24 | Blocked By: 6,16,21

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-server.md:290-295` — 经验/境界系统目标边界。
  - Pattern: `docs/plan-client.md:276-299` — client 最终展示需要的 realm/qi/karma/power 字段。
  - Pattern: `server/src/player/state.rs`（Task 21 产物） — PlayerState 真正存储结构。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test player::progression:: -- --nocapture` 通过 threshold、karma clamp、realm promotion tests。
  - [ ] progression engine 可在纯单测中驱动 `player_state` / `world_state` 投影变化，无需真人输入。
  - [ ] realm promotion 不会生成非法 qi 上限或负经验值。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Realm progression advances deterministically from synthetic gains
    Tool: Bash
    Steps: run `cd server && cargo test player::progression:: -- --nocapture`; save output to `.sisyphus/evidence/task-22-progression.log`
    Expected: 阈值、晋升、karma clamp、qi 上限更新测试全部通过
    Evidence: .sisyphus/evidence/task-22-progression.log

  Scenario: Invalid progression inputs are clamped safely
    Tool: Bash
    Steps: run tests with negative gains, oversized karma, or overflow-prone experience values; capture results
    Expected: 输入被 clamp/拒绝，不会产生 NaN、负值或 panic
    Evidence: .sisyphus/evidence/task-22-progression-error.txt
  ```

  **Commit**: YES | Message: `feat(server): add deterministic progression engine` | Files: `server/src/player/**`, `server/src/world/**`, `server/src/tests/**`

- [x] 23. 交付 `CultivationScreen`、`player_state` client handler 与只读修仙面板

  **What to do**: 在 client 侧把 `player_state` 转成真正可见的 M3 UI，但保持“只读、轻量、无动态 XML”。必须完成：
  - `PlayerStateViewModel` 与 `PlayerStateHandler` 接入 Task 13 router；
  - 注册 `K` 键打开 `CultivationScreen`；
  - `CultivationScreen` 使用 owo-ui 构建只读面板：realm、qi bar、karma、power breakdown、当前 zone；
  - UI 只消费本地 state，不直接依赖 network parse；
  - 保持 `ENABLE_DYNAMIC_XML_UI = false`，完全排除 Task C7 的远期动态 UI。
  **Must NOT do**: 不得实现服务端下发 XML；不得把 player state 展示写成聊天栏刷屏；不得引入额外 UI 框架。

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: 这是 M3 在 client 侧的核心呈现任务。
  - Skills: `[]` — 依赖 owo-lib 已在 `build.gradle` 中存在。
  - Omitted: `["playwright"]` — Minecraft client 非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 24 | Blocked By: 13,19,21

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-client.md:266-317` — cultivation UI 目标布局与字段范围。
  - Pattern: `client/build.gradle:19-42` — owo-lib 已在依赖中。
  - Pattern: `client/src/main/resources/fabric.mod.json:1-28` — 当前 entrypoint / resources 基线。
  - Pattern: `client/src/main/java/com/bong/client/BongClient.java:11-17` — client 初始化入口。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd client && ./gradlew test --tests "com.bong.client.*PlayerState*" --tests "com.bong.client.*Cultivation*"` 通过 state-to-viewmodel/UI tests。
  - [ ] `cd client && ./gradlew build` 通过，说明 owo UI 接线无编译错误。
  - [ ] `ENABLE_DYNAMIC_XML_UI`（或等价 feature flag）默认关闭并有测试覆盖。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Player state payload drives read-only cultivation screen correctly
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "com.bong.client.*PlayerState*" --tests "com.bong.client.*Cultivation*"`; save output to `.sisyphus/evidence/task-23-cultivation-ui.log`
    Expected: realm、qi、karma、power breakdown、zone 显示映射全部通过测试
    Evidence: .sisyphus/evidence/task-23-cultivation-ui.log

  Scenario: Dynamic XML UI remains disabled by default
    Tool: Bash
    Steps: run feature-flag tests asserting dynamic XML path is off and unknown ui payloads are ignored
    Expected: 动态 XML 仍为禁用态，不会意外开启远期功能
    Evidence: .sisyphus/evidence/task-23-cultivation-ui-error.txt
  ```

  **Commit**: YES | Message: `feat(client): add read-only cultivation screen` | Files: `client/src/main/java/com/bong/client/**`, `client/src/test/java/com/bong/client/**`, `client/src/main/resources/**`

- [x] 24. 扩展最终 M1-M3 全栈验证脚本与证据矩阵

  **What to do**: 在 Task 15 的 M1 smoke 基础上，把最终验证扩成覆盖 M1-M3 的单一自动化入口。允许升级现有 `scripts/smoke-tiandao-fullstack.sh`，也可新增 `scripts/smoke-tiandao-m123.sh`，但最终必须做到：
  - 顺序执行 `schema -> tiandao -> server -> client` 的最小检查与构建；
  - 验证 `world_state`、chat drain、merged command、typed narration、zone_info、event_alert、player_state` 六条主线证据；
  - 验证 player persistence 文件 roundtrip 与 progression engine 单测通过；
  - 汇总日志到 `.sisyphus/evidence/task-24-*.{log,txt}`；
  - 成功标准完全脚本化，不依赖人工进服观察。
  **Must NOT do**: 不得把最终验证拆成“需要人自己手动多跑几个命令”；不得遗漏 client 构建/测试；不得在脚本里吞掉失败退出码。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 最终收口需要跨四层整合并保证可重复执行。
  - Skills: `[]` — 依赖所有前置任务的构建/测试入口。
  - Omitted: `["playwright"]` — Minecraft 非浏览器。

  **Parallelization**: Can Parallel: NO | Wave 5 | Blocks: Final Verification Wave | Blocked By: 2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `.sisyphus/plans/tiandao-fullstack-closure.md:47-54` — Definition of Done 命令矩阵。
  - Pattern: `scripts/smoke-test.sh:1-48` — 当前 smoke-test 风格。
  - Pattern: `scripts/start.sh:1-60` / `scripts/stop.sh:1-7` — 启停编排基础。
  - Pattern: `docs/local-test-env.md:111-142` — 联机验证流程中可脚本化的部分。

  **Acceptance Criteria** (agent-executable only):
  - [ ] 最终 smoke 入口退出 0，并执行：`schema check/test/generate`、`tiandao check/test/start:mock`、`server fmt/clippy/test`、`client test/build`、全栈 Redis smoke。
  - [ ] evidence 目录下存在 task-24 对应的分层日志：schema、agent、server、client、fullstack。
  - [ ] 第二次执行同一最终 smoke 入口仍成功，证明脚本幂等且能正确清理残留状态。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Final M1-M3 verification matrix passes
    Tool: Bash
    Steps: run the final smoke entrypoint once end-to-end; save combined output to `.sisyphus/evidence/task-24-final-matrix.log`
    Expected: schema/agent/server/client/fullstack checks全部通过并留下分层 evidence
    Evidence: .sisyphus/evidence/task-24-final-matrix.log

  Scenario: Final smoke remains idempotent on second execution
    Tool: Bash
    Steps: rerun the same final smoke entrypoint after cleanup; capture second-run output
    Expected: 第二次执行仍通过，不受 Redis/tmux/build/cache 残留影响
    Evidence: .sisyphus/evidence/task-24-final-matrix-error.log
  ```

  **Commit**: YES | Message: `test(e2e): finalize tiandao fullstack verification matrix` | Files: `scripts/smoke-tiandao-fullstack.sh`, `scripts/**`, `.sisyphus/evidence/**`

## Final Verification Wave (4 parallel agents, ALL must APPROVE)
- [x] F1. Plan Compliance Audit — oracle
- [x] F2. Code Quality Review — unspecified-high
- [x] F3. Agent-Executed Runtime QA — unspecified-high (+ interactive_bash for MC/Fabric runtime)
- [x] F4. Scope Fidelity Check — deep

## Commit Strategy
- 所有实现代码只在 `/workspace/worktrees/Bong-tiandao-fullstack-closure` 中完成；主工作树只保留计划与证据。
- 原子提交按任务号或紧邻同依赖任务组合提交；每个提交必须通过对应子项目的最小测试集。
- 推荐提交前缀：`chore(schema): ...`、`feat(agent): ...`、`feat(server): ...`、`feat(client): ...`、`test(e2e): ...`。
- 任何跨层提交都必须同时包含契约变更、样例更新、对应 Rust/JUnit/Vitest 测试，禁止“先改一层，等 merge 再修另一层”。

## Success Criteria
- `schema`、`tiandao`、`server`、`client` 对同一消息形状的理解一致，没有 sample drift 或 envelope 分裂。
- Agent 不再逐个 sub-agent 直接 publish，而是统一经 arbiter 合并后输出；chat/peer/world/balance/era 信息进入上下文链路。
- Server 能消费 merged command，执行最小事件/zone/player_state 逻辑，并把 narration/zone_info/event_alert/player_state 通过 `bong:server_data` 下发。
- Client 能按 `type` 路由并显示 narration、zone HUD、cultivation UI，不再只处理 legacy welcome/heartbeat。
- 无人工判断即可跑通 M1-M3 既定 smoke 脚本并留下 evidence。
