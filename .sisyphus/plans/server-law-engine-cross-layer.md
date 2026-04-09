# Server Law Engine Cross-Layer Closure

## TL;DR
> **Summary**: 在新 worktree 中完成 `docs/plan-server.md` 的 server 主线，并把为端到端闭环所必需的 agent/client/schema 收口任务纳入同一执行计划。以 fallback-first 地图策略、TypeBox 契约真源、Bevy 主线程执行边界和混合测试策略为硬约束，交付一个可从 Redis 指令到游戏可见效果、再到客户端可见 UI/叙事的完整修仙法则闭环。
> **Deliverables**:
> - 新 worktree `bong-server-law-engine` 与原子提交节奏
> - `server/` M1-M3 路线：zone/world/events/player state/combat/cultivation 全链路
> - `agent/packages/schema` 契约加固与 `agent/packages/tiandao` M1-M3 路线收口
> - `client/` narration/zone/player_state/UI 路由与显示链路
> - 自动化 smoke / integration / evidence 产物与兼容清理
> **Effort**: XL
> **Parallel**: YES - 5 waves
> **Critical Path**: 1 → 2 → 3 → 4 → 7 → 8 → 9 → 11 → 12 → 13 → 14 → 15 → 16 → 20 → 23 → 24 → 25 → 26

## Context
### Original Request
- 用户起初要求编写 server 路线 Redis IPC + schema 对齐计划。
- 随后将范围扩大为：`docs/plan-server.md` 的**完整实现计划**，执行于新 worktree，测试采用 **TDD + tests-after 混合**。
- 后续再次明确：凡 `docs/plan-server.md` 牵出的 **agent/client 相关工作，全量纳入同一总计划**。
- 用户已明确要求：**自动执行 Momus Review，不需要二次询问**。

### Interview Summary
- worktree 默认固定为：分支 `bong-server-law-engine`，目录 `/workspace/worktrees/bong-server-law-engine`。
- `.mca` 地图资产采用 fallback-first：缺失或损坏时不阻塞 M1/M3，继续使用当前平坦测试世界。
- 测试策略固定为：纯逻辑/契约/调度 TDD；Valence/Redis/Anvil/client smoke 与端到端联调 tests-after。
- `@bong/schema` 继续作为契约真源；server Rust mirror、agent runtime validate、client payload parser 全部围绕同一真源对齐。
- M1 的 zone 不等待 M2 地图：先引入最小 `ZoneRegistry` 与单区 `spawn` fallback，后续再切入 `zones.json` 与 Anvil。

### Metis Review (gaps addressed)
- 已固定 zone authoritative source：**`zones.json` + fallback 单区 `spawn`**；Anvil 只负责地形，不负责 zone 元数据真源。
- 已消解 event 重复实现风险：M1 先落最小事件调度骨架，M2/M3 在同一调度器上扩展，不允许 S1/S8 各写一套。
- 已固定身份语义：玩家 canonical id 默认 `offline:{username}`，显示名保留 `username`；NPC id 统一为 `npc_{entity.index()}`。
- 已纳入 server 侧重复校验：不能仅信任 agent parse；server 必须再次执行版本、数量、范围、target 存在性与 budget 检查。
- 已纳入 Redis/帧预算护栏：bridge 线程只做 I/O，主线程 drain 和执行必须有每帧上限，避免抢占 Update。

## Work Objectives
### Core Objective
- 交付一个决策完整的跨层实施蓝图，使执行代理能够在单一 worktree 中完成 server 主线、agent 收口、client 渲染与 schema 对齐，并在无人工介入的情况下验证“世界状态 → 天道决策 → 服务端执行 → 客户端表现”的完整闭环。

### Deliverables
- `agent/packages/schema/`：Redis IPC + client payload 契约、generated freshness gate、共享 samples/negative tests、runtime validation exports。
- `server/src/world/{mod,zone,events}.rs`、`server/src/player/{mod,state}.rs`、`server/src/network/{mod,redis_bridge,command_executor,chat_collector}.rs`、`server/src/npc/{spawn,brain,patrol,sync}.rs`。
- `agent/packages/tiandao/src/{arbiter,chat-processor,world-model,balance}.ts` 与 `main.ts/context.ts/parse.ts/redis-ipc.ts/skills/*.md` 升级。
- `client/src/main/java/com/bong/client/{network,hud,ui,visual}/**` typed payload router、narration/zone/player_state UI、optional dynamic UI gateway。
- `server/zones.json`（或 `zones.json.example` + runtime fallback）、`scripts/` smoke/integration 脚本、测试与 evidence 产物规范。

### Definition of Done (verifiable conditions with commands)
- `git worktree list` 显示 `/workspace/worktrees/bong-server-law-engine` 与分支 `bong-server-law-engine`。
- `cd agent/packages/schema && npm run check && npm test && npm run generate`
- `cd agent/packages/tiandao && npm run check && npm test && npm run start:mock`（`npm test` 由 Task 5 新增，`start:mock` 必须支持无 `.env` 运行）
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `cd client && ./gradlew test build`
- `bash scripts/smoke-test.sh`
- `bash scripts/smoke-law-engine.sh`（由 Task 26 新增）

### Must Have
- 所有源码改动只发生在新 worktree 中，主工作树保持只读基线。
- `@bong/schema` 为唯一契约真源；Redis IPC 与 `bong:server_data` custom payload 都有可测试契约。
- Server 在 Redis 不可用、`.mca` 缺失或 zones 配置缺失时仍可降级运行。
- M1 至少完成：真实 world_state、chat 采集、command executor、scoped narration、agent merged publish、client narration 渲染。
- M2 至少完成：optional Anvil + authoritative zones.json + patrol/beast_tide + zone HUD。
- M3 至少完成：player persistence、player_state payload、cultivation UI、基础战斗/采集/境界推进。
- 每个实现任务必须包含自动化 happy path 与 failure/edge QA。

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- 不引入第二套 IPC 传输范式（本计划不把 Redis Pub/Sub 全面改成 Streams）。
- 不在主工作树直接改代码，不把“merge 后再修”当成放弃边界的理由。
- 不把 `.mca` 资产缺失作为阻塞 M1/M3 的借口；必须保留平坦 fallback 世界。
- 不新增与现有 schema 无关的 ad-hoc JSON 格式；server/client payload 也必须契约化。
- 不依赖人工观察作为唯一验收；所有 acceptance criteria 必须可由代理执行。
- 不提前引入多余系统：无脚本 DSL、无自建 LLM、无 Docker 依赖、无额外 MC 版本升级。

## Verification Strategy
> ZERO HUMAN INTERVENTION — all verification is agent-executed.
- Test decision: **混合策略**
  - **TDD**：schema validation、zone lookup、queue budget、param parsing/clamp、event scheduler、player-state serde、arbiter/balance/parse 等纯逻辑。
  - **tests-after**：Valence/Redis bridge、Anvil loader、client build/runtime smoke、server-agent-client 联调。
- QA policy: 每个任务都必须包含一条 happy path 和一条 failure/edge path；Minecraft/Fabric 不使用浏览器验证，统一使用 Bash / interactive_bash / JUnit / cargo/vitest 测试。
- Evidence: `.sisyphus/evidence/task-{N}-{slug}.{ext}`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave. <3 per wave (except final) = under-splitting.
> Extract shared dependencies as Wave-1 tasks for max parallelism.

Wave 1: foundation + contracts + harnesses (Tasks 1-5)
Wave 2: server M1 substrate + world bootstrap (Tasks 6-10)
Wave 3: server/agent/client M1 closure (Tasks 11-15)
Wave 4: M2 world + routing + client zone UX (Tasks 16-20)
Wave 5: M3 progression + cross-layer closure (Tasks 21-26)

### Dependency Matrix (full, all tasks)
| Task | Depends On | Blocks |
|---|---|---|
| 1 | - | 2,3,4,5,6 |
| 2 | 1 | 7,9,10,11,14,15,21,22,26 |
| 3 | 1,2 | 6,13,16,20,24,26 |
| 4 | 1 | 7,8,9,10,11,12,13,17,18,19,23,25 |
| 5 | 1,2 | 14,15,21,22 |
| 6 | 1,3 | 16,20,24,26 |
| 7 | 2,4 | 8,9,10,11,12,13,17,18,19,23,25 |
| 8 | 4,7 | 9,10,12,17,18,19,23,25 |
| 9 | 2,4,7,8 | 12,14,15,21,23,26 |
| 10 | 2,4,7 | 15,26 |
| 11 | 2,4,7,9 | 12,18,19,25,26 |
| 12 | 4,7,8,9,11 | 13,17,19,25,26 |
| 13 | 3,4,6,7,9,12 | 14,16,20,24,26 |
| 14 | 2,5,9,12 | 15,21,22,26 |
| 15 | 2,5,10,14 | 21,22,26 |
| 16 | 3,6,13 | 20,24,26 |
| 17 | 4,7,8,12 | 18,19,25,26 |
| 18 | 4,7,8,12,17 | 19,25,26 |
| 19 | 4,7,8,12,17,18 | 23,25,26 |
| 20 | 3,6,13,16 | 24,26 |
| 21 | 2,5,9,14,15 | 22,26 |
| 22 | 2,5,14,15,21 | 26 |
| 23 | 4,7,8,9,19 | 24,25,26 |
| 24 | 3,6,13,20,23 | 26 |
| 25 | 4,7,8,17,18,19,23 | 26 |
| 26 | 2,3,10,13,15,20,22,24,25 | Final Verification Wave |

### Agent Dispatch Summary (wave → task count → categories)
- Wave 1 → 5 tasks → `git` (1), `ultrabrain` (2,3), `deep` (4,5)
- Wave 2 → 5 tasks → `ultrabrain` (6,7,9), `deep` (8,10)
- Wave 3 → 5 tasks → `deep` (11,12,13,14,15)
- Wave 4 → 5 tasks → `deep` (16), `visual-engineering` (17,18,20), `unspecified-high` (19)
- Wave 5 → 6 tasks → `ultrabrain` (21,23,25), `artistry` (22), `visual-engineering` (24), `deep` (26)

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task MUST have: Agent Profile + Parallelization + QA Scenarios.

- [x] 1. 创建 worktree 基线与 evidence 约定

  **What to do**: 在执行开始前创建 `/workspace/worktrees/bong-server-law-engine` 对应的独立 worktree 与分支 `bong-server-law-engine`，记录基线 `git status`、`git branch --show-current`、子项目可用命令，以及 `.sisyphus/evidence/` 命名约定。所有后续实现、测试、evidence 产物都必须在该 worktree 中完成。
  **Must NOT do**: 不得在 `/workspace/Bong` 主工作树直接修改代码；不得在未确认 worktree 路径时启动实现。

  **Recommended Agent Profile**:
  - Category: `git` — Reason: 需要安全创建/验证 worktree 并固定后续执行位置。
  - Skills: `[]` — 仅依赖 git 与 shell 预检，不需要额外技能。
  - Omitted: `["playwright"]` — 不是浏览器工作流。

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 2,3,4,5,6 | Blocked By: -

  **References**:
  - Pattern: `docs/local-test-env.md:63-142` — 当前 server/client 本地启动与 smoke 基线。
  - Pattern: `scripts/smoke-test.sh:1-48` — 现有 smoke 脚本的阶段划分与输出风格。
  - Pattern: `docs/plan-server.md:328-346` — 任务排序与先做基础设施的原则。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `git worktree list | grep "/workspace/worktrees/bong-server-law-engine"` 返回目标 worktree。
  - [ ] `git -C /workspace/worktrees/bong-server-law-engine branch --show-current` 输出 `bong-server-law-engine`。
  - [ ] `.sisyphus/evidence/task-1-worktree.txt` 记录 worktree 路径、当前分支、基线 `git status --short` 输出。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path worktree bootstrap
    Tool: Bash
    Steps: run `git worktree add -b bong-server-law-engine /workspace/worktrees/bong-server-law-engine main`; run `git -C /workspace/worktrees/bong-server-law-engine status --short`; save output to `.sisyphus/evidence/task-1-worktree.txt`
    Expected: worktree 创建成功；目标分支为 `bong-server-law-engine`；基线状态为空或仅包含计划要求的辅助文件
    Evidence: .sisyphus/evidence/task-1-worktree.txt

  Scenario: Duplicate path is rejected
    Tool: Bash
    Steps: rerun the same `git worktree add ...` command after the first creation
    Expected: 命令非零退出且不会覆盖已有 worktree；stderr 写入 `.sisyphus/evidence/task-1-worktree-error.txt`
    Evidence: .sisyphus/evidence/task-1-worktree-error.txt
  ```

  **Commit**: NO | Message: `chore(repo): create law-engine worktree baseline` | Files: `[]`

- [x] 2. 加固 `@bong/schema` 真源与 generated freshness gate

  **What to do**: 以 `agent/packages/schema` 为唯一契约真源，补齐对 Redis IPC 基础消息的 rejection tests、generated JSON Schema freshness gate、负样例样本与运行时 validate 导出稳定接口；确保后续 server/agent/client 只能消费该包的导出，不得各自复制 schema。此任务只处理真源与测试护栏，不引入 client payload union，后者由 Task 4 单独负责。
  **Must NOT do**: 不得把 Rust mirror 或 Java parser 变成真源；不得跳过 generated 文件的新鲜度校验。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 需要把类型、生成物与测试门禁严格对齐。
  - Skills: `[]` — 依赖现有 TypeScript/Vitest 栈即可。
  - Omitted: `["playwright"]` — 无 UI/浏览器验证。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 7,9,10,11,14,15,21,22,26 | Blocked By: 1

  **References**:
  - Pattern: `agent/packages/schema/src/index.ts:1-11` — 当前 schema 导出入口。
  - Test: `agent/packages/schema/tests/schema.test.ts:1-122` — sample + rejection test 组织方式。
  - Pattern: `agent/packages/schema/package.json:1-23` — `check/test/generate` 脚本基线。
  - Pattern: `docs/plan-agent.md:356-360` — schema/agent 侧测试策略期望。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/schema && npm run check && npm test && npm run generate` 全绿。
  - [ ] 生成物 freshness test 在 generated 文件过期时会失败，在重新 `npm run generate` 后恢复通过。
  - [ ] schema 包导出运行时 `validate(...)` 路径稳定，可被 server/agent/client 直接复用。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Schema source of truth stays green
    Tool: Bash
    Steps: run `cd agent/packages/schema && npm run check && npm test && npm run generate`; save stdout/stderr to `.sisyphus/evidence/task-2-schema.log`
    Expected: TypeScript 检查、Vitest、JSON Schema 生成全部成功
    Evidence: .sisyphus/evidence/task-2-schema.log

  Scenario: Stale generated schema is caught
    Tool: Bash
    Steps: modify one generated file timestamp/content in a temporary copy; run the freshness test target; save failure output
    Expected: freshness gate 非零退出并明确指出 generated 文件已漂移
    Evidence: .sisyphus/evidence/task-2-schema-error.txt
  ```

  **Commit**: YES | Message: `chore(schema): add freshness gate and contract guards` | Files: `agent/packages/schema/src/**`, `agent/packages/schema/tests/**`, `agent/packages/schema/generated/**`, `agent/packages/schema/package.json`

- [x] 3. 拆分 `server` 的 `world/player` 模块并建立 spawn-zone 基线

  **What to do**: 按 `docs/plan-server.md` 的文件规划把平铺的 `server/src/world.rs` 与 `server/src/player.rs` 迁移为 `world/mod.rs`、`world/zone.rs`、`player/mod.rs` 等目录结构，同时保留现有出生点、欢迎消息与 Adventure 模式初始化。该任务必须引入最小 `ZoneRegistry` 基线：在没有 `zones.json` 时自动提供单区 `spawn` AABB，供 Task 8/9/10/11 立即使用。
  **Must NOT do**: 不得在此任务中引入 Anvil 依赖或把 `zones.json` 变成启动前置条件。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 涉及模块重构与后续依赖的稳定边界。
  - Skills: `[]` — 现有 Rust/Bevy 代码已足够提供模式。
  - Omitted: `["playwright"]` — 无浏览器内容。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 6,13,16,20,24,26 | Blocked By: 1,2

  **References**:
  - Pattern: `server/src/world.rs:1-48` — 当前平坦世界创建逻辑。
  - Pattern: `server/src/player.rs:1-67` — 当前玩家初始化/清理逻辑。
  - Pattern: `docs/plan-server.md:181-209` — `ZoneRegistry` 目标形态。
  - Pattern: `docs/plan-server.md:299-323` — server 目录重构目标。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test world::tests::fallback_spawn_zone_exists player::tests::spawn_defaults_are_preserved` 通过。
  - [ ] `cd server && cargo test` 中不再引用已删除的 `world.rs`/`player.rs` 平铺入口。
  - [ ] 在无 `zones.json` 的情况下，`ZoneRegistry` 仍返回名为 `spawn` 的默认区域。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Fallback spawn zone exists after module split
    Tool: Bash
    Steps: run `cd server && cargo test world::tests::fallback_spawn_zone_exists player::tests::spawn_defaults_are_preserved -- --nocapture`; save output to `.sisyphus/evidence/task-3-server-modules.log`
    Expected: 测试确认 spawn zone 存在，出生坐标仍为 `[8.0, 66.0, 8.0]`
    Evidence: .sisyphus/evidence/task-3-server-modules.log

  Scenario: Missing zones config does not block boot
    Tool: Bash
    Steps: run `cd server && cargo test world::tests::missing_zones_file_uses_spawn_fallback -- --nocapture`
    Expected: 测试通过并明确记录 fallback 行为；不得 panic
    Evidence: .sisyphus/evidence/task-3-server-modules-error.txt
  ```

  **Commit**: YES | Message: `refactor(server): split world and player modules` | Files: `server/src/world/**`, `server/src/player/**`, `server/src/main.rs`

- [x] 4. 定义 typed `bong:server_data` envelope 与 shared fixtures

  **What to do**: 在 schema 真源中新增 `bong:server_data` 的 typed envelope，统一描述 `welcome`、`heartbeat`、`narration`、`zone_info`、`event_alert`、`player_state`、`ui_open` 七类 payload；并提供 sample/negative fixtures，作为 server 发送与 client 解析的唯一契约。保留现有 channel `bong:server_data`，通过 `type` 字段扩展，不新增第二条客户端自定义 payload channel。
  **Must NOT do**: 不得继续扩散 ad-hoc JSON；不得把 welcome/heartbeat 留在契约之外。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要同时照顾 schema、server legacy payload、client parser 三边兼容。
  - Skills: `[]` — 依赖现有 schema 与 JUnit 解析模式。
  - Omitted: `["playwright"]` — 该任务是契约与解析，不是交互 UI。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 7,8,9,10,11,12,13,17,18,19,23,25 | Blocked By: 1

  **References**:
  - Pattern: `client/src/main/java/com/bong/client/BongNetworkHandler.java:9-247` — 当前 `bong:server_data` 解析器入口。
  - Pattern: `server/src/network/mod.rs:177-243` — welcome/heartbeat 现有发送方式。
  - Pattern: `docs/plan-client.md:25-80` — narration payload 目标结构。
  - Pattern: `docs/plan-client.md:223-258` — typed router 方向。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/schema && npm test` 覆盖 `server_data` sample 与 rejection cases。
  - [ ] 生成物中存在 `server_data` 对应 schema，且从 `@bong/schema` 入口导出。
  - [ ] `welcome`/`heartbeat` 样例与 `narration`/`zone_info`/`event_alert`/`player_state`/`ui_open` 使用同一 envelope 约束。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Typed envelope accepts known payload kinds
    Tool: Bash
    Steps: run `cd agent/packages/schema && npm test`; capture the `server_data` schema suite output
    Expected: 所有已定义 payload type 均通过 sample validation
    Evidence: .sisyphus/evidence/task-4-server-data.log

  Scenario: Unknown payload kind is rejected
    Tool: Bash
    Steps: run the rejection test for a payload with `type: "unknown"`; save failure output
    Expected: 校验失败并指出非法 `type`
    Evidence: .sisyphus/evidence/task-4-server-data-error.txt
  ```

  **Commit**: YES | Message: `feat(schema): add typed server_data envelope` | Files: `agent/packages/schema/src/**`, `agent/packages/schema/tests/**`, `agent/packages/schema/samples/**`, `client/src/test/**`

- [x] 5. 为 `tiandao` 建立可运行测试脚手架与 `npm test`

  **What to do**: 给 `agent/packages/tiandao` 增加可离线运行的单元测试脚手架（推荐 Vitest），新增 `npm test` 脚本、测试目录、mock LLM/Redis 注入点，并修正 `start:mock`：`--mock` 模式必须在没有 `.env` 的情况下直接运行，不得在进入 mock 流程前因缺少 `LLM_BASE_URL/LLM_API_KEY` 退出。
  **Must NOT do**: 不得让单元测试依赖真实 Redis 或真实 LLM；不得要求 `.env` 才能运行 mock smoke。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要调整入口依赖注入与测试执行方式。
  - Skills: `[]` — 直接使用 TypeScript/Vitest 即可。
  - Omitted: `["playwright"]` — 无浏览器测试。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 14,15,21,22 | Blocked By: 1,2

  **References**:
  - Pattern: `agent/packages/tiandao/package.json:1-25` — 当前缺少 `test` script。
  - Pattern: `agent/packages/tiandao/src/main.ts:22-35` — 当前 `.env` 前置校验会阻断 mock。
  - Pattern: `agent/packages/tiandao/src/main.ts:99-107` — mock mode 现有入口。
  - Pattern: `docs/plan-agent.md:356-360` — tiandao 测试策略。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm run check && npm test` 通过。
  - [ ] `cd agent/packages/tiandao && env -u LLM_BASE_URL -u LLM_API_KEY npm run start:mock` 以 mock 数据成功退出。
  - [ ] 单元测试支持在无 Redis、无网络、无 `.env` 的环境运行。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Offline tiandao test harness works
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm run check && npm test`; save output to `.sisyphus/evidence/task-5-tiandao-tests.log`
    Expected: 类型检查与单测全部通过，`npm test` 已成为有效脚本
    Evidence: .sisyphus/evidence/task-5-tiandao-tests.log

  Scenario: Mock mode no longer requires .env
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && env -u LLM_BASE_URL -u LLM_API_KEY npm run start:mock`; capture output
    Expected: 进程以 mock world state 正常执行一轮并退出 0
    Evidence: .sisyphus/evidence/task-5-tiandao-tests-error.txt
  ```

  **Commit**: YES | Message: `test(agent): add tiandao offline test harness` | Files: `agent/packages/tiandao/package.json`, `agent/packages/tiandao/src/**`, `agent/packages/tiandao/tests/**`

- [x] 6. 实现 fallback-first world bootstrap 配置资源

  **What to do**: 在 `server/src/world/mod.rs` 中抽出统一的 world bootstrap 入口，定义 `FallbackFlat` / `AnvilIfPresent` 选择逻辑与日志约定，使 server 在 `.mca` 缺失、目录为空、未配置 world path 时始终回退到当前平坦测试世界。该任务只实现启动配置与 fallback 选择，不处理真正的 Anvil 读取；真实 Anvil 载入留给 Task 16。
  **Must NOT do**: 不得让 world bootstrap 同时承担 zone 加载或事件初始化；不得因为没有 region 文件而让 server 启动失败。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 需要把世界初始化入口与未来 Anvil 路径分离清楚。
  - Skills: `[]` — 现有 world.rs 即是直接参考。
  - Omitted: `["playwright"]` — 非 UI 工作。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 16,20,24,26 | Blocked By: 1,3

  **References**:
  - Pattern: `server/src/world.rs:1-48` — 当前平坦世界构造代码。
  - Pattern: `docs/plan-server.md:155-177` — Anvil/fallback 世界目标。
  - Pattern: `docs/plan-server.md:305-307` — `world/mod.rs` / `zone.rs` / `events.rs` 切分。
  - Pattern: `docs/local-test-env.md:63-79` — server 启动期望日志。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test world::tests::selects_fallback_without_region_dir` 通过。
  - [ ] `cd server && timeout 15s cargo run` 在无 region 目录时仍成功进入主循环并输出 fallback world 日志。
  - [ ] fallback 启动不影响当前 16x16 chunk 草地测试世界可用性。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Missing region directory still boots fallback world
    Tool: Bash
    Steps: run `cd server && cargo test world::tests::selects_fallback_without_region_dir -- --nocapture`; then run `timeout 15s cargo run` and capture logs
    Expected: 测试与 smoke 都确认进入 fallback 路径，而不是退出报错
    Evidence: .sisyphus/evidence/task-6-world-bootstrap.log

  Scenario: Invalid configured path does not panic
    Tool: Bash
    Steps: start server with a nonexistent world path env/config; capture stderr/stdout
    Expected: server 记录 warning 并继续使用平坦世界，退出码为 0/timeout
    Evidence: .sisyphus/evidence/task-6-world-bootstrap-error.txt
  ```

  **Commit**: YES | Message: `refactor(server): add fallback-first world bootstrap` | Files: `server/src/world/**`, `server/src/main.rs`, `server/tests/**`

- [x] 7. 加固 Redis bridge：runtime validate、chat list、I/O 预算

  **What to do**: 扩展 `server/src/network/redis_bridge.rs` 与配套资源，使 bridge 同时支持 `WorldState` 发布、`ChatMessageV1` 的 `RPUSH bong:player_chat`、入站 `AgentCommandV1` / `NarrationV1` 的 runtime validation、以及桥接线程的非阻塞预算控制。明确规则：Redis I/O 只在 bridge 线程，ECS 修改只在主线程；bridge 遇到坏 payload 只能丢弃并告警，不能拖垮主循环。
  **Must NOT do**: 不得把 Redis 命令执行移到 Bevy 主线程；不得把 `bong:player_chat` 改成 Pub/Sub；不得因 Redis 暂时不可用而 panic。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 涉及线程边界、runtime validation 与 budget 护栏。
  - Skills: `[]` — 现有 Rust/Redis 代码足够提供模式。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 8,9,10,11,12,13,17,18,19,23,25 | Blocked By: 2,4

  **References**:
  - Pattern: `server/src/network/redis_bridge.rs:1-197` — 当前 pub/sub bridge 基线。
  - Pattern: `server/src/network/mod.rs:17-57` — bridge resource 注册方式。
  - API/Type: `agent/packages/schema/src/agent-command.ts:20-32` — `AgentCommandV1` 约束。
  - API/Type: `agent/packages/schema/src/narration.ts:12-16` — `NarrationV1` 顶层结构。
  - API/Type: `agent/packages/schema/src/chat-message.ts:6-13` — `ChatMessageV1` List payload 结构。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test redis_bridge_tests::publishes_world_state redis_bridge_tests::pushes_chat_messages redis_bridge_tests::rejects_invalid_inbound_payloads` 通过。
  - [ ] Redis 断连/坏 JSON 只产生日志警告，不会导致 bridge thread panic。
  - [ ] `RedisOutbound` 至少支持 `WorldState` 与 `PlayerChat` 两类消息。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Bridge handles world_state and chat list traffic
    Tool: Bash
    Steps: run `cd server && cargo test redis_bridge_tests::publishes_world_state redis_bridge_tests::pushes_chat_messages -- --nocapture`; save output
    Expected: 测试确认 PUBLISH 与 RPUSH 路径都可用，且消息体符合 schema
    Evidence: .sisyphus/evidence/task-7-redis-bridge.log

  Scenario: Invalid inbound payload is dropped safely
    Tool: Bash
    Steps: run `cd server && cargo test redis_bridge_tests::rejects_invalid_inbound_payloads -- --nocapture`
    Expected: 非法 JSON 或 schema 不匹配 payload 被拒绝，测试通过且无 panic
    Evidence: .sisyphus/evidence/task-7-redis-bridge-error.txt
  ```

  **Commit**: YES | Message: `feat(server): harden redis bridge and chat queue` | Files: `server/src/network/redis_bridge.rs`, `server/src/network/mod.rs`, `server/tests/**`

- [x] 8. 抽象 `bong:server_data` 发送器与定向投递辅助层

  **What to do**: 在 server 侧建立 typed `bong:server_data` payload builder/dispatcher，统一 welcome、heartbeat、narration、zone_info、event_alert、player_state、ui_open 的序列化、大小检查与投递。保留 legacy `bong:server_data` channel，不新增第二条 client custom payload channel；同时引入可复用的收件人过滤辅助层（broadcast / zone / player / offline-id alias），供后续 narration、zone_info、player_state 共用。
  **Must NOT do**: 不得继续在各系统手写裸 JSON 字符串；不得把 player routing 只绑死在 `username`，必须兼容 `offline:{username}`。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 牵涉 server payload 发送入口与后续多类消息共用基础设施。
  - Skills: `[]` — 依赖已有 custom payload 发送模式。
  - Omitted: `["playwright"]` — 非浏览器任务。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 9,10,12,17,18,19,23,25 | Blocked By: 4,7

  **References**:
  - Pattern: `server/src/network/mod.rs:177-243` — 当前 welcome/heartbeat 发送逻辑。
  - Pattern: `client/src/main/java/com/bong/client/BongNetworkHandler.java:12-51` — 当前 `bong:server_data` 解析入口。
  - API/Type: `agent/packages/schema/src/common.ts:24-25` — `MAX_PAYLOAD_BYTES` 上限。
  - Pattern: `docs/plan-client.md:223-258` — 统一 payload 路由器目标。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test server_data_tests::serializes_known_payloads server_data_tests::routes_player_and_zone_targets server_data_tests::rejects_oversize_payloads` 通过。
  - [ ] welcome/heartbeat 改为通过 typed envelope 发送，client 旧测试仍可调整后通过。
  - [ ] 后续系统不再直接调用裸 `send_custom_payload` 拼 JSON，而是通过统一 helper。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Typed server_data payloads serialize and route correctly
    Tool: Bash
    Steps: run `cd server && cargo test server_data_tests::serializes_known_payloads server_data_tests::routes_player_and_zone_targets -- --nocapture`
    Expected: 已知 payload type 序列化成功，玩家/区域过滤逻辑输出正确目标集合
    Evidence: .sisyphus/evidence/task-8-server-data.log

  Scenario: Oversize payload is rejected before send
    Tool: Bash
    Steps: run `cd server && cargo test server_data_tests::rejects_oversize_payloads -- --nocapture`
    Expected: 超过 `MAX_PAYLOAD_BYTES` 的 payload 被拒绝并返回可断言错误
    Evidence: .sisyphus/evidence/task-8-server-data-error.txt
  ```

  **Commit**: YES | Message: `feat(server): add typed server_data dispatcher` | Files: `server/src/network/**`, `server/tests/**`

- [x] 9. 用真实 ECS 数据替换占位 `world_state`

  **What to do**: 重写 `publish_world_state_to_redis` 的快照构造，读取真实玩家位置/名字、ZoneRegistry、NPC 状态与近期事件，输出稳定的 `WorldStateV1`。固定 canonical ids：玩家 `offline:{username}`，NPC `npc_{entity.index()}`；在 M1 没有 `PlayerState` 时为 realm/power/trend 提供明确默认值，到 Task 21 再切换真实持久化数据。
  **Must NOT do**: 不得继续发布 `Player0`/`offline:player_0` 这类占位名；不得在没有玩家时省略 `zones` 数组。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 需要把 ECS 查询与 schema 输出严格对齐。
  - Skills: `[]` — 当前 world_state 与 schema 已提供足够上下文。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 12,14,15,21,23,26 | Blocked By: 2,4,7,8

  **References**:
  - Pattern: `server/src/network/mod.rs:59-118` — 当前占位 `world_state` 构造逻辑。
  - API/Type: `agent/packages/schema/src/world-state.ts:18-72` — `PlayerProfile`/`NpcSnapshot`/`ZoneSnapshot` 结构。
  - Pattern: `docs/plan-server.md:75-105` — S2 真实 world_state 目标。
  - Pattern: `.sisyphus/drafts/server-law-engine-cross-layer.md:12-20` — canonical id 默认决策。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test world_state_tests::uses_real_player_names_and_positions world_state_tests::emits_spawn_zone_without_players world_state_tests::uses_canonical_ids` 通过。
  - [ ] `timeout 15s cargo run` + Redis 订阅可以看到真实 `username` 与位置，而非占位值。
  - [ ] `recent_events` 在无事件时为空数组而非缺失字段。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Snapshot uses real ECS data
    Tool: Bash
    Steps: run `cd server && cargo test world_state_tests::uses_real_player_names_and_positions world_state_tests::uses_canonical_ids -- --nocapture`
    Expected: 测试断言 world_state 使用真实用户名、位置与 canonical ids
    Evidence: .sisyphus/evidence/task-9-world-state.log

  Scenario: Empty-world snapshot still stays schema-valid
    Tool: Bash
    Steps: run `cd server && cargo test world_state_tests::emits_spawn_zone_without_players -- --nocapture`
    Expected: 在 0 玩家/0 NPC 下仍输出合法 `WorldStateV1`
    Evidence: .sisyphus/evidence/task-9-world-state-error.txt
  ```

  **Commit**: YES | Message: `feat(server): publish real world state snapshots` | Files: `server/src/network/mod.rs`, `server/src/world/**`, `server/src/player/**`, `server/tests/**`

- [x] 10. 采集玩家聊天并写入 `bong:player_chat`

  **What to do**: 新增 `server/src/network/chat_collector.rs`，监听玩家聊天事件，构造 `ChatMessageV1`，根据玩家当前位置解析 zone，然后通过 Task 7 的 List 能力写入 `bong:player_chat`。为积压保护加入每 tick/每玩家消息长度与速率上限，并明确 command/chat 区分：slash command 不进入 `player_chat`，普通聊天才进入队列。
  **Must NOT do**: 不得把 chat queue 实现成阻塞式 BLPOP；不得把 slash command 与普通聊天混在一起；不得信任客户端自报 zone。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 涉及事件拦截、ZoneRegistry 查询与 Redis List 对接。
  - Skills: `[]` — 当前 player/network/world 基础已足够。
  - Omitted: `["playwright"]` — 非 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 15,26 | Blocked By: 2,4,7,8

  **References**:
  - Pattern: `docs/plan-server.md:108-124` — S3 Chat 采集目标与 Redis List 说明。
  - API/Type: `agent/packages/schema/src/chat-message.ts:6-13` — `ChatMessageV1` 结构。
  - Pattern: `server/src/player.rs:24-55` — 当前玩家初始化与 username 获取上下文。
  - Pattern: `server/src/world.rs:16-47` — 当前 spawn 世界坐标基线。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test chat_collector_tests::captures_plain_chat chat_collector_tests::skips_commands chat_collector_tests::adds_zone_context` 通过。
  - [ ] 聊天消息写入 `bong:player_chat` 时符合 `ChatMessageV1` schema。
  - [ ] 过长或过频消息按预算被截断/丢弃并产生可断言日志。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Plain chat becomes ChatMessageV1 on Redis list
    Tool: Bash
    Steps: run `cd server && cargo test chat_collector_tests::captures_plain_chat chat_collector_tests::adds_zone_context -- --nocapture`
    Expected: 普通聊天被序列化为 `ChatMessageV1`，zone 字段来自 `ZoneRegistry`
    Evidence: .sisyphus/evidence/task-10-chat.log

  Scenario: Commands are not enqueued as player chat
    Tool: Bash
    Steps: run `cd server && cargo test chat_collector_tests::skips_commands chat_collector_tests::drops_oversize_messages -- --nocapture`
    Expected: slash command 与超长消息不会进入 `bong:player_chat`
    Evidence: .sisyphus/evidence/task-10-chat-error.txt
  ```

  **Commit**: YES | Message: `feat(server): collect player chat to redis list` | Files: `server/src/network/chat_collector.rs`, `server/src/network/mod.rs`, `server/tests/**`

- [x] 11. 落地主线程 command executor 与执行预算

  **What to do**: 新增 `server/src/network/command_executor.rs`，把入站 `AgentCommandV1` 从 network drain 推入 `CommandExecutorResource`，由 `execute_agent_commands` 在 Update 中按 `MAX_COMMANDS_PER_TICK=5` 执行。M1 必须支持：`spawn_event(thunder_tribulation)`、`modify_zone`、`npc_behavior.flee_threshold`；`beast_tide` 允许先接到 Task 13 的事件调度骨架；`realm_collapse` / `karma_backlash` 先记录 `not implemented` warning，但必须走统一 dispatcher。
  **Must NOT do**: 不得在接收 Redis 消息时直接改 ECS；不得跳过 target 存在性、数值 clamp、zone existence 二次校验。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 是 server M1 真正的执行入口，涉及 ECS 修改与预算控制。
  - Skills: `[]` — 依赖现有 schema/world/npc 基线。
  - Omitted: `["playwright"]` — 非 UI。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 12,18,19,25,26 | Blocked By: 2,4,7,9

  **References**:
  - Pattern: `server/src/network/mod.rs:120-175` — 当前 agent command 仅日志输出。
  - API/Type: `agent/packages/schema/src/agent-command.ts:6-32` — command batch 结构。
  - API/Type: `agent/packages/schema/src/common.ts:18-20` — `MAX_COMMANDS_PER_TICK` 常量。
  - Pattern: `docs/plan-server.md:31-72` — S1 指令执行器目标。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test command_executor_tests::applies_modify_zone command_executor_tests::caps_commands_per_tick command_executor_tests::updates_flee_threshold` 通过。
  - [ ] 对不存在 zone / NPC / 非法 intensity 的命令会被拒绝并留下 warning，不会 panic。
  - [ ] thunder_tribulation 通过统一事件入口排入 `ActiveEventsResource` 或直接生成最小效果，不绕过 dispatcher。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Supported commands mutate ECS through the executor queue
    Tool: Bash
    Steps: run `cd server && cargo test command_executor_tests::applies_modify_zone command_executor_tests::updates_flee_threshold -- --nocapture`
    Expected: zone/npc 变化仅在 executor drain 时生效，并通过测试断言
    Evidence: .sisyphus/evidence/task-11-command-executor.log

  Scenario: Invalid commands are clamped or rejected safely
    Tool: Bash
    Steps: run `cd server && cargo test command_executor_tests::caps_commands_per_tick command_executor_tests::rejects_unknown_targets -- --nocapture`
    Expected: 超预算/坏 target/坏参数的命令被安全处理，无 panic
    Evidence: .sisyphus/evidence/task-11-command-executor-error.txt
  ```

  **Commit**: YES | Message: `feat(server): add command executor queue` | Files: `server/src/network/command_executor.rs`, `server/src/network/mod.rs`, `server/src/world/**`, `server/src/npc/**`, `server/tests/**`

- [x] 12. 实现 scoped narration 与 typed narration payload 下发

  **What to do**: 将 `RedisInbound::AgentNarration` 从“全部广播 chat message”升级为统一 scoped delivery：`Broadcast` 发给全部在线玩家；`Zone` 只发给当前 zone 内玩家；`Player` 支持按 `username` 或 `offline:{username}` 精确定位。发送载体采用 Task 8 的 typed `bong:server_data` narration payload；如需兼容 vanilla 体验，可同时保留格式化聊天栏镜像，但 typed payload 是客户端主通道。
  **Must NOT do**: 不得继续把 zone/player narration 广播给所有 client；不得把 scoped logic 写死在 client 侧。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要整合 zone lookup、player routing 与 payload dispatcher。
  - Skills: `[]` — 现有 server/client/schema 上下文足够。
  - Omitted: `["playwright"]` — 非浏览器任务。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 13,17,19,25,26 | Blocked By: 4,7,8,9,11

  **References**:
  - Pattern: `server/src/network/mod.rs:144-171` — 当前 narration 全广播逻辑。
  - API/Type: `agent/packages/schema/src/narration.ts:4-16` — narration scope/style 结构。
  - Pattern: `docs/plan-server.md:128-149` — S4 narration 精准下发目标。
  - Pattern: `docs/plan-client.md:25-80` — client narration 渲染预期。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test narration_tests::broadcast_hits_all_clients narration_tests::zone_scope_filters_by_zone narration_tests::player_scope_matches_username_and_offline_id` 通过。
  - [ ] typed narration payload 能被序列化到 `bong:server_data`，并包含 style/text/scope 需要的字段。
  - [ ] 未匹配到目标玩家时安全忽略，不向无关玩家泄露内容。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Scoped narration targets only intended recipients
    Tool: Bash
    Steps: run `cd server && cargo test narration_tests::broadcast_hits_all_clients narration_tests::zone_scope_filters_by_zone narration_tests::player_scope_matches_username_and_offline_id -- --nocapture`
    Expected: 三种 scope 的收件人集合与测试断言完全一致
    Evidence: .sisyphus/evidence/task-12-narration.log

  Scenario: Missing target does not leak narration
    Tool: Bash
    Steps: run `cd server && cargo test narration_tests::missing_player_target_is_ignored -- --nocapture`
    Expected: 未命中 target 时不发送 payload，测试通过且无 panic
    Evidence: .sisyphus/evidence/task-12-narration-error.txt
  ```

  **Commit**: YES | Message: `feat(server): deliver scoped narration payloads` | Files: `server/src/network/**`, `server/src/player/**`, `server/src/world/**`, `server/tests/**`

- [x] 13. 建立统一事件调度骨架与最小 thunder/beast hooks

  **What to do**: 新增 `server/src/world/events.rs` 与 `ActiveEventsResource`，把 thunder_tribulation 与 beast_tide 的最小 M1 版本统一纳入调度器：事件具备创建、逐 tick 推进、到期清理三阶段。此骨架必须可被 M2/M3 继续扩展到 realm_collapse、karma_backlash，不允许再出现第二套事件执行路径。
  **Must NOT do**: 不得让 `spawn_event` 一部分走 command executor 直接生成、一部分走 events resource；不得把事件状态散落在多个临时 resource。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 是 M1/M2/M3 事件一致性的核心骨架。
  - Skills: `[]` — 可直接复用 command executor 与 ZoneRegistry。
  - Omitted: `["playwright"]` — 非 UI。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 14,16,20,24,26 | Blocked By: 3,4,6,7,9,12

  **References**:
  - Pattern: `docs/plan-server.md:238-265` — S8 事件系统目标。
  - Pattern: `docs/plan-server.md:31-72` — thunder/beast 的最小行为要求。
  - API/Type: `agent/packages/schema/src/common.ts:36-42` — `EventKind` 枚举。
  - Pattern: `.sisyphus/drafts/server-law-engine-cross-layer.md:17-18` — “事件实现避免 S1/S8 重复”的已定决策。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test events_tests::thunder_event_ticks_until_expiry events_tests::beast_tide_event_spawns_and_cleans_up` 通过。
  - [ ] `spawn_event` 最终只通过 `ActiveEventsResource` 进入执行路径。
  - [ ] 已有事件会出现在 `world_state.recent_events` 或 zone.active_events 中的稳定字段里。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Event scheduler advances and expires thunder/beast events
    Tool: Bash
    Steps: run `cd server && cargo test events_tests::thunder_event_ticks_until_expiry events_tests::beast_tide_event_spawns_and_cleans_up -- --nocapture`
    Expected: 测试确认事件从创建到过期完整经过统一调度器
    Evidence: .sisyphus/evidence/task-13-events.log

  Scenario: Duplicate event execution path is impossible
    Tool: Bash
    Steps: run `cd server && cargo test events_tests::spawn_event_only_enters_scheduler_once -- --nocapture`
    Expected: 同一命令不会触发双重执行或双重资源登记
    Evidence: .sisyphus/evidence/task-13-events-error.txt
  ```

  **Commit**: YES | Message: `feat(server): add unified active event scheduler` | Files: `server/src/world/events.rs`, `server/src/world/mod.rs`, `server/src/network/command_executor.rs`, `server/tests/**`

- [x] 14. 构建 `tiandao` arbiter 并修复 `source` 兼容性

  **What to do**: 按 `docs/plan-agent.md` 实现 `arbiter.ts`，把三个子 agent 的 commands/narrations 合并为一个 merged result，执行硬约束校验、同 zone 冲突消解、灵气守恒缩放与 `MAX_COMMANDS_PER_TICK` 截断。同时修复 `publishCommands("arbiter", ...)` 与当前 schema `source` union 不兼容问题：要么扩展 schema 明确允许 `arbiter`，要么在 batch 顶层保留原 agent source 的合法表示，但必须做出单一确定决策并在代码与 schema 同步。
  **Must NOT do**: 不得继续每个子 agent 各自 publish command batch；不得留下 `source as AgentCommandV1["source"]` 这种仅靠类型断言掩盖的不一致。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 涉及核心 agent 合并逻辑与跨层契约修复。
  - Skills: `[]` — 现有 schema/common/context 足够支撑。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 15,21,22,26 | Blocked By: 2,5,9,12

  **References**:
  - Pattern: `agent/packages/tiandao/src/main.ts:64-92` — 当前 per-agent publish 逻辑。
  - Pattern: `agent/packages/schema/src/agent-command.ts:20-32` — 当前 `source` union。
  - Pattern: `agent/packages/schema/src/common.ts:18-20` — 指令数量上限。
  - Pattern: `docs/plan-agent.md:33-87` — A1 arbiter 目标规则。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- arbiter` 通过，覆盖 merge/scale/truncate/conflict cases。
  - [ ] merged publish 路径不再向 Redis 发送 3 份独立 command batch。
  - [ ] `source` 兼容性在 schema、runtime、tests 三处一致，无断言逃逸。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Arbiter merges and limits command batches correctly
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- arbiter`; save output
    Expected: 同 zone modify_zone 合并、总量缩放、预算截断都通过测试
    Evidence: .sisyphus/evidence/task-14-arbiter.log

  Scenario: Invalid source contract is no longer possible
    Tool: Bash
    Steps: run the contract test covering merged publish serialization
    Expected: `source` 与 `AgentCommandV1` 完全兼容，测试不会依赖 TS 类型断言蒙混过关
    Evidence: .sisyphus/evidence/task-14-arbiter-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): add arbiter merged publish flow` | Files: `agent/packages/tiandao/src/main.ts`, `agent/packages/tiandao/src/arbiter.ts`, `agent/packages/schema/src/**`, `agent/packages/tiandao/tests/**`

- [x] 15. 实现 chat processor、安全 drain 与循环稳定化

  **What to do**: 在 `tiandao` 中新增 `chat-processor.ts`，提供安全的 Redis chat drain 方案，禁止使用 `LRANGE + DEL` 这种会丢消息的竞态模式；采用 `MULTI` + `LRANGE/LTRIM` 或等效原子窗口 drain。把 chat signals 注入 `WorldModel` / `context.ts`，并同时完成循环模式稳定化：LLM 超时、连续失败退避、Redis 断连恢复、tick 级日志指标。
  **Must NOT do**: 不得继续使用 `LRANGE + DEL`；不得让一次 LLM/Redis 异常导致主循环退出。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 涉及 Redis 语义、上下文注入与长期运行稳定性。
  - Skills: `[]` — 依赖现有 RedisIpc/main/context 即可。
  - Omitted: `["playwright"]` — 非 UI。

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 21,22,26 | Blocked By: 2,5,10,14

  **References**:
  - Pattern: `agent/packages/tiandao/src/redis-ipc.ts:21-100` — 当前 Redis IPC 能力。
  - Pattern: `agent/packages/tiandao/src/main.ts:109-137` — 当前 loop/shutdown 逻辑。
  - Pattern: `agent/packages/tiandao/src/context.ts:1-136` — 当前 context block 结构。
  - Pattern: `docs/plan-agent.md:90-163` — A2/A3 目标与约束。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- chat-processor redis-ipc main-loop` 通过。
  - [ ] chat drain 使用原子/幂等方案，不会在并发写入下丢失消息。
  - [ ] Redis 断连、LLM 500、JSON parse 失败时主循环继续存活并在日志记录退避/恢复。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Chat drain is race-safe and feeds context
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- chat-processor redis-ipc`; capture output
    Expected: 测试证明并发写入场景下消息不丢失，chat signals 成功进入 context 数据结构
    Evidence: .sisyphus/evidence/task-15-chat-processor.log

  Scenario: Loop survives Redis/LLM failures
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- main-loop`; capture failure-handling assertions
    Expected: 断连/超时/500 情况仅触发 warning/backoff，不会崩溃退出
    Evidence: .sisyphus/evidence/task-15-chat-processor-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): add safe chat drain and resilient loop` | Files: `agent/packages/tiandao/src/chat-processor.ts`, `agent/packages/tiandao/src/redis-ipc.ts`, `agent/packages/tiandao/src/main.ts`, `agent/packages/tiandao/src/context.ts`, `agent/packages/tiandao/tests/**`

- [x] 16. 接入 optional Anvil 地形且保持 fallback 可用

  **What to do**: 在 Task 6 的 bootstrap 基础上实现真正的 Anvil 加载路径：若 `server/world/region/*.mca` 存在且可读，则用 Valence/Anvil 读取真实地形；否则继续使用平坦 fallback。该实现必须严格 optional：有资产就用、没有就降级，且日志明确记录所选路径。
  **Must NOT do**: 不得让 `.mca` 成为 M1/M3 前置；不得把 region 解析失败视为 fatal error。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 涉及 Valence Anvil 接入与 fallback 保底。
  - Skills: `[]` — 现有 docs/tech-audit 与 server world bootstrap 可直接参考。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 20,24,26 | Blocked By: 3,6,13

  **References**:
  - Pattern: `docs/plan-server.md:155-177` — S5 Anvil 地形加载目标。
  - Pattern: `server/src/world.rs:16-47` — 当前 fallback 地形实现。
  - Pattern: `docs/tech-audit.md` — Valence/Anvil 可行性结论（执行者需在仓库中引用具体章节）。
  - External: `https://github.com/valence-rs/valence/blob/main/examples/anvil_loading.rs` — Anvil 接入示例。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test world::tests::uses_anvil_when_region_exists world::tests::falls_back_when_anvil_missing` 通过。
  - [ ] `timeout 15s cargo run` 在没有 region 目录时仍进入 fallback 世界。
  - [ ] 使用有效 `.mca` 资产时，日志明确表明进入 Anvil 路径。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Server prefers Anvil only when region assets exist
    Tool: Bash
    Steps: run `cd server && cargo test world::tests::uses_anvil_when_region_exists world::tests::falls_back_when_anvil_missing -- --nocapture`
    Expected: 两条路径都被测试覆盖，行为与日志可断言
    Evidence: .sisyphus/evidence/task-16-anvil.log

  Scenario: Broken/missing Anvil assets do not block startup
    Tool: Bash
    Steps: run a smoke case with missing or unreadable region path; capture logs
    Expected: server 记录 warning 后继续使用 fallback flat world
    Evidence: .sisyphus/evidence/task-16-anvil-error.txt
  ```

  **Commit**: YES | Message: `feat(server): load anvil worlds opportunistically` | Files: `server/src/world/**`, `server/tests/**`

- [x] 17. 将客户端网络层重构为 typed payload router

  **What to do**: 把当前单文件 `BongNetworkHandler.java` 重构为 `network/` 子包与 handler 注册表，统一路由 `welcome`、`heartbeat`、`narration`、`zone_info`、`event_alert`、`player_state`、`ui_open`。保留现有自定义轻量 JSON 解析风格或改为库化解析均可，但必须基于 Task 4 的 typed envelope；未知 payload type 要安全忽略并记录错误。
  **Must NOT do**: 不得继续把所有 payload 都降格成 `[Bong] type: message` 字符串；不得让未知/坏 payload 直接崩 client。

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: 虽是网络层，但直接决定后续 HUD/UI 展示能力与 handler 结构。
  - Skills: `[]` — 依赖现有 JUnit/Fabric networking 即可。
  - Omitted: `["playwright"]` — Minecraft client 非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 18,19,25,26 | Blocked By: 4,7,8,12

  **References**:
  - Pattern: `client/src/main/java/com/bong/client/BongNetworkHandler.java:9-247` — 当前解析器与测试边界。
  - Test: `client/src/test/java/com/bong/client/BongNetworkHandlerTest.java:1-64` — 现有 JUnit 测试风格。
  - Pattern: `docs/plan-client.md:223-258` — C5 payload router 目标。
  - Pattern: `client/build.gradle:19-42` — JUnit 依赖与 Java 17 基线。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd client && ./gradlew test` 通过，覆盖 known types、unknown type、malformed JSON。
  - [ ] `BongNetworkHandler` 重构后仍只注册 `bong:server_data` 一个 channel。
  - [ ] handler 注册表能够安全分发到 narration/zone/player_state 等独立 handler 类。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Typed router dispatches known payload kinds
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "*BongNetworkHandlerTest"`; save output
    Expected: 已知 type 被路由到对应 handler，JUnit 全绿
    Evidence: .sisyphus/evidence/task-17-client-router.log

  Scenario: Unknown or malformed payload is ignored safely
    Tool: Bash
    Steps: run tests covering malformed JSON and unknown type handling
    Expected: parser 返回 error/ignore，不会抛出未捕获异常
    Evidence: .sisyphus/evidence/task-17-client-router-error.txt
  ```

  **Commit**: YES | Message: `refactor(client): add typed server_data router` | Files: `client/src/main/java/com/bong/client/network/**`, `client/src/test/java/com/bong/client/**`

- [x] 18. 实现 narration/chat 渲染与关键 toast

  **What to do**: 基于 Task 17 的 typed router 完成 client narration 渲染：普通 narration/perception/system_warning/era_decree 进入聊天栏，`system_warning`/`era_decree` 额外触发中央 Toast。优先完成无需 mixin 的聊天栏 + HUD 路径；视觉增强（天象摇晃/fog）仍留到后续可选项。
  **Must NOT do**: 不得在 M1 就引入复杂 mixin 作为必需路径；不得让 narration 只存在于 server chat message，而 client 无 typed handler。

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: 直接负责玩家可见叙事体验。
  - Skills: `[]` — 依赖 Fabric HUD/render API 与既有 HUD 模块。
  - Omitted: `["playwright"]` — 非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 19,25,26 | Blocked By: 4,7,8,12,17

  **References**:
  - Pattern: `docs/plan-client.md:25-80` — C1 narration 渲染目标。
  - Pattern: `docs/plan-client.md:88-129` — C2 toast 规则。
  - Pattern: `client/src/main/java/com/bong/client/BongHud.java:1-17` — 当前 HUD 入口。
  - Pattern: `agent/packages/schema/src/narration.ts:4-16` — narration style 结构。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd client && ./gradlew test` 通过，包括 narration formatting / toast trigger 单测。
  - [ ] `system_warning` 与 `era_decree` 会触发 toast；`perception` / `narration` 不触发 toast。
  - [ ] narration handler 仍支持 unknown style 的安全降级显示。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Narration styles render to chat and toast as intended
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "*Narration*"`; capture output
    Expected: 样式映射、toast 触发规则、默认 fallback 全部通过测试
    Evidence: .sisyphus/evidence/task-18-client-narration.log

  Scenario: Unknown narration style degrades safely
    Tool: Bash
    Steps: run the JUnit case for an unsupported narration style
    Expected: 客户端显示默认文本或忽略，不会崩溃
    Evidence: .sisyphus/evidence/task-18-client-narration-error.txt
  ```

  **Commit**: YES | Message: `feat(client): render narration and key toasts` | Files: `client/src/main/java/com/bong/client/network/**`, `client/src/main/java/com/bong/client/hud/**`, `client/src/test/java/com/bong/client/**`

- [x] 19. 完成 zone 信息与事件警报的客户端展示链路

  **What to do**: 在 client 侧实现 `zone_info` 与 `event_alert` handlers、`BongZoneHud` 常驻信息栏与进入区域时的大字提示；server 侧在玩家跨 zone 与重大事件发生时通过 Task 8 的 typed payload 主动下发这两类消息。danger/spirit_qi 的展示必须直接来自 server payload，不允许 client 侧自算。
  **Must NOT do**: 不得让 zone HUD 依赖客户端本地推导区域边界；不得把 zone/event 信息继续混入 narration 文本中代替结构化 payload。

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: 需要同时收口 server 触发逻辑与 client 展示逻辑，但不属于纯视觉装饰。
  - Skills: `[]` — 现有 ZoneRegistry、router、HUD 模块足够。
  - Omitted: `["playwright"]` — Minecraft 验证非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 23,25,26 | Blocked By: 4,7,8,12,17,18

  **References**:
  - Pattern: `docs/plan-client.md:161-220` — C4 区域 HUD 目标。
  - Pattern: `docs/plan-client.md:223-258` — 事件/区域 handler 注册方式。
  - Pattern: `docs/plan-server.md:181-209` — zone 基础字段来源。
  - Pattern: `agent/packages/schema/src/world-state.ts:42-49` — `ZoneSnapshot` 字段约束。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd client && ./gradlew test --tests "*Zone*" --tests "*EventAlert*"` 通过。
  - [ ] `cd server && cargo test zone_payload_tests::emits_zone_info_on_transition event_payload_tests::emits_event_alert_on_major_event` 通过。
  - [ ] zone HUD 显示的数据直接来自 server payload，客户端无本地区域推导分支。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Server emits typed zone/event payloads and client renders them
    Tool: Bash
    Steps: run both `cd server && cargo test zone_payload_tests::emits_zone_info_on_transition event_payload_tests::emits_event_alert_on_major_event -- --nocapture` and `cd client && ./gradlew test --tests "*Zone*" --tests "*EventAlert*"`
    Expected: zone/event payload 发送与 HUD handler 解析全部通过
    Evidence: .sisyphus/evidence/task-19-zone-event.log

  Scenario: Malformed zone/event payload is ignored safely
    Tool: Bash
    Steps: run JUnit/rejection cases with missing spirit_qi or invalid danger level
    Expected: 客户端忽略坏 payload，不污染 HUD 状态
    Evidence: .sisyphus/evidence/task-19-zone-event-error.txt
  ```

  **Commit**: YES | Message: `feat(client): add zone hud and event alerts` | Files: `client/src/main/java/com/bong/client/network/**`, `client/src/main/java/com/bong/client/hud/**`, `client/src/test/java/com/bong/client/**`, `server/src/player/**`, `server/src/world/**`, `server/tests/**`

- [x] 20. 从 `zones.json` 装载 authoritative zones 并接入 patrol/pathing

  **What to do**: 在 `server/src/world/zone.rs` 里实现 `zones.json` 读取与 fallback `spawn` 共存，定义 AABB、`spirit_qi`、`danger_level`、`active_events`、patrol anchors 等最小字段；再在 `npc/patrol.rs` 中接入基于 zone 的巡逻/寻路骨架，让 skeleton/zombie 等可依据 zone 边界与中心执行 patrol/beast_tide 路线。若 `zones.json` 不存在或解析失败，继续回退单区 `spawn`。
  **Must NOT do**: 不得把 zone 元数据绑到 `.mca` 文件本身；不得让坏 `zones.json` 使 server 启动失败。

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: 虽属 server 逻辑，但直接决定世界区域体验与 NPC 路线表现。
  - Skills: `[]` — 基于 pathfinding crate 与现有 NPC 基线实现。
  - Omitted: `["playwright"]` — 非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 24,26 | Blocked By: 3,6,13,16

  **References**:
  - Pattern: `docs/plan-server.md:181-234` — S6/S7 区域系统与 patrol 目标。
  - Pattern: `server/Cargo.toml:6-19` — 已包含 `pathfinding` 依赖。
  - Pattern: `server/src/npc/{spawn.rs,brain.rs,sync.rs}` — 现有 NPC 基线（执行者需按文件引用具体实现）。
  - Pattern: `.sisyphus/drafts/server-law-engine-cross-layer.md:15-18` — `zones.json` authoritative 与 Anvil 解耦决策。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test zone_tests::loads_zones_json_with_fallback patrol_tests::npc_patrol_stays_within_zone patrol_tests::invalid_zones_file_uses_spawn_fallback` 通过。
  - [ ] `world_state.zones` 读取真实 `zones.json` 内容，而非硬编码单区。
  - [ ] beast_tide / patrol 相关 NPC 不会离开所属 zone 的合法边界。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Zones load from config and patrol stays in bounds
    Tool: Bash
    Steps: run `cd server && cargo test zone_tests::loads_zones_json_with_fallback patrol_tests::npc_patrol_stays_within_zone -- --nocapture`
    Expected: zones.json 成功装载，patrol 目标点与移动路径均留在 zone AABB 内
    Evidence: .sisyphus/evidence/task-20-zones-patrol.log

  Scenario: Bad zones config falls back to spawn zone
    Tool: Bash
    Steps: run `cd server && cargo test patrol_tests::invalid_zones_file_uses_spawn_fallback -- --nocapture`
    Expected: 配置损坏时仅记录 warning，并继续使用 fallback spawn zone
    Evidence: .sisyphus/evidence/task-20-zones-patrol-error.txt
  ```

  **Commit**: YES | Message: `feat(server): load authoritative zones and patrol paths` | Files: `server/src/world/zone.rs`, `server/src/npc/patrol.rs`, `server/src/npc/**`, `server/zones.json`, `server/tests/**`

- [x] 21. 引入 world model、peer decisions、balance 与 key-player blocks

  **What to do**: 在 `tiandao` 中完成 `world-model.ts`、peer decisions 记忆、Gini/balance 计算、key-player block，把 `latestState` 扩展成可观察趋势与关键人物的长期上下文。此任务同时要求 `main.ts` 在每轮后记录上轮 decisions，并为后续 M3 的个体关注、时代效果提供统一数据源。
  **Must NOT do**: 不得把趋势/平衡计算直接散落到 prompt 字符串拼接里；不得让 `WorldModel` 与 `ContextRecipe` 分叉出多套事实来源。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 涉及状态建模、趋势分析与长期上下文组织。
  - Skills: `[]` — 现有 context/world_state/schema 足够支撑。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 22,26 | Blocked By: 2,5,9,14,15

  **References**:
  - Pattern: `agent/packages/tiandao/src/context.ts:1-136` — 当前 block/recipe 架构。
  - Pattern: `docs/plan-agent.md:166-273` — A4/A5/A6 peer decisions、world model、balance 目标。
  - API/Type: `agent/packages/schema/src/world-state.ts:18-72` — 输入 world_state 结构。
  - API/Type: `agent/packages/schema/src/chat-message.ts:17-25` — `ChatSignal` 结构。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- world-model balance context` 通过。
  - [ ] recipe 渲染中可包含 peer decisions、zone trend、balance advice、key player block，且有 token budget 裁剪测试。
  - [ ] `WorldModel` 成为这些上下文块的唯一事实来源。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: World model computes trends, balance, and peer summaries
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- world-model balance context`; capture output
    Expected: 历史趋势、Gini、关键人物与 peer decisions 均有稳定测试覆盖
    Evidence: .sisyphus/evidence/task-21-world-model.log

  Scenario: Context budget trimming preserves required blocks
    Tool: Bash
    Steps: run the context budget unit tests with oversized state input
    Expected: 非必需 block 被裁剪，必需 block 保留，且不抛异常
    Evidence: .sisyphus/evidence/task-21-world-model-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): add world model and balance context` | Files: `agent/packages/tiandao/src/world-model.ts`, `agent/packages/tiandao/src/balance.ts`, `agent/packages/tiandao/src/context.ts`, `agent/packages/tiandao/src/main.ts`, `agent/packages/tiandao/tests/**`

- [x] 22. 强化叙事质量与时代实质化效果

  **What to do**: 更新 `skills/*.md` prompt，使 narration 采用半文言半白话、100–200 字、带预兆；并把 Era 的宣告从“只有文本”升级为“文本 + 结构化全局效果”：当 Era 决策生成时代宣告时，arbiter/WorldModel 必须落地 `currentEra` 与全局 `modify_zone` 影响的单一规则。该规则需有测试，并且不允许绕过 Task 14 的 arbiter。
  **Must NOT do**: 不得把时代效果散在多个 agent 各自拼接；不得让 prompt 文本质量优化影响结构化 JSON 解析稳定性。

  **Recommended Agent Profile**:
  - Category: `artistry` — Reason: 同时包含 prompt 风格质量与结构化时代效果设计。
  - Skills: `[]` — 以现有 prompt/context/arbiter 为基础。
  - Omitted: `["playwright"]` — 非 UI。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 26 | Blocked By: 2,5,14,15,21

  **References**:
  - Pattern: `docs/plan-agent.md:277-309` — A7/A8/A9 目标。
  - Pattern: `agent/packages/tiandao/src/main.ts:57-92` — 当前 tick publish 逻辑。
  - Pattern: `agent/packages/tiandao/src/context.ts:108-136` — recipe 定义基线。
  - Pattern: `agent/packages/schema/src/common.ts:29-57` — 指令/叙事通用枚举。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd agent/packages/tiandao && npm test -- era arbiter prompts` 通过。
  - [ ] 时代宣告可产生可测试的 `currentEra` 状态与对应全局 effect，而非仅有 narration 文本。
  - [ ] prompt 改动后 `parse.ts` 相关 JSON 解析测试仍然全绿。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Era declarations create both narration and structured global effect
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- era arbiter`; save output
    Expected: 测试确认时代宣告会更新 `currentEra` 并附带统一的全局 zone effect
    Evidence: .sisyphus/evidence/task-22-era.log

  Scenario: Prompt refinement does not break JSON parsing
    Tool: Bash
    Steps: run `cd agent/packages/tiandao && npm test -- parse prompts`
    Expected: 改进后的 prompts 仍保持结构化输出可解析，坏输出有 fallback
    Evidence: .sisyphus/evidence/task-22-era-error.txt
  ```

  **Commit**: YES | Message: `feat(agent): deepen era effects and narration quality` | Files: `agent/packages/tiandao/src/**`, `agent/packages/tiandao/tests/**`, `agent/packages/tiandao/src/skills/*.md`

- [x] 23. 实现 PlayerState 持久化与 server→client `player_state` payload

  **What to do**: 在 `server/src/player/state.rs` 定义 `PlayerState`、自动保存/加载、断连保存、重连附加与 `composite_power` 计算；同时在玩家连接后与状态变化后，通过 typed `player_state` payload 定期向 client 下发 realm/spirit_qi/karma/power breakdown/current zone 等字段。player persistence 的 canonical key 必须使用 `offline:{username}`。
  **Must NOT do**: 不得把 `player_state` 仅存在于 server 内部而不下发 client；不得在没有持久化文件时崩溃。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 涉及持久化模型、ECS 组件与对外 payload 对齐。
  - Skills: `[]` — 现有 world_state/player 基线足够。
  - Omitted: `["playwright"]` — 非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 24,25,26 | Blocked By: 4,7,8,9,19

  **References**:
  - Pattern: `docs/plan-server.md:271-289` — S9 PlayerState 目标。
  - Pattern: `docs/plan-client.md:266-316` — C6 修仙 UI 所需字段。
  - API/Type: `agent/packages/schema/src/world-state.ts:18-30` — `PlayerProfile` 可复用字段基线。
  - Pattern: `.sisyphus/drafts/server-law-engine-cross-layer.md:19-20` — canonical player id 决策。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test player_state_tests::loads_and_saves_offline_player_state player_state_tests::computes_composite_power player_state_tests::serializes_player_state_payload` 通过。
  - [ ] 首次登录无存档文件时会生成默认 `PlayerState`，重连后可加载上次值。
  - [ ] `player_state` payload 由 server 主动下发且可被 client router 识别。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: PlayerState persists and emits typed payloads
    Tool: Bash
    Steps: run `cd server && cargo test player_state_tests::loads_and_saves_offline_player_state player_state_tests::serializes_player_state_payload -- --nocapture`
    Expected: 持久化文件按 canonical player id 命名，payload 序列化与测试断言通过
    Evidence: .sisyphus/evidence/task-23-player-state.log

  Scenario: Missing or corrupted save file falls back safely
    Tool: Bash
    Steps: run `cd server && cargo test player_state_tests::corrupt_save_uses_default_state -- --nocapture`
    Expected: 坏存档只触发 warning，并回退默认 PlayerState
    Evidence: .sisyphus/evidence/task-23-player-state-error.txt
  ```

  **Commit**: YES | Message: `feat(server): persist player state and publish client payload` | Files: `server/src/player/state.rs`, `server/src/player/mod.rs`, `server/src/network/**`, `server/tests/**`

- [x] 24. 构建 cultivation UI 面板与 player_state 客户端消费

  **What to do**: 在 client 侧接入 `player_state` handler、状态缓存与 `ui/CultivationScreen.java`（owo-ui），支持按键打开修仙面板显示境界、真元、karma、综合实力分解与当前区域。M3 阶段 `ui_open` 可以先只保留解析/路由能力，真正动态 XML UI 仍视为 optional；但静态 cultivation screen 必须完成。
  **Must NOT do**: 不得把 player_state 仅显示成聊天栏文本；不得把 `ui_open` 当成本任务的阻塞前提。

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: 直接实现 M3 关键玩家界面。
  - Skills: `[]` — 现有 owo/Fabric 配置已就绪。
  - Omitted: `["playwright"]` — Minecraft UI 非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 26 | Blocked By: 3,6,13,20,23

  **References**:
  - Pattern: `docs/plan-client.md:266-316` — C6 修仙 UI 目标。
  - Pattern: `client/build.gradle:19-29` — owo-lib 依赖已存在。
  - Pattern: `client/src/main/java/com/bong/client/BongHud.java:1-17` — 当前 HUD 模块入口。
  - Pattern: `docs/plan-client.md:321-340` — `ui_open` optional 安全边界。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd client && ./gradlew test build` 通过，包含 player_state parser/cache/screen tests。
  - [ ] 客户端在收到 `player_state` payload 后可更新缓存，并能通过 `K` 键打开 cultivation screen。
  - [ ] `ui_open` 未实现动态 XML 时也不会阻塞 `player_state` UI 路径。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Player state updates feed cultivation screen
    Tool: Bash
    Steps: run `cd client && ./gradlew test --tests "*PlayerState*" --tests "*CultivationScreen*"`; save output
    Expected: player_state handler、缓存与 screen 组件测试全部通过
    Evidence: .sisyphus/evidence/task-24-cultivation-ui.log

  Scenario: Missing ui_open support does not block static cultivation UI
    Tool: Bash
    Steps: run the JUnit case where `ui_open` payload is ignored but `player_state` still renders
    Expected: 静态 cultivation screen 仍可通过缓存数据打开
    Evidence: .sisyphus/evidence/task-24-cultivation-ui-error.txt
  ```

  **Commit**: YES | Message: `feat(client): add cultivation screen and player state cache` | Files: `client/src/main/java/com/bong/client/ui/**`, `client/src/main/java/com/bong/client/network/**`, `client/src/test/java/com/bong/client/**`

- [x] 25. 完成战斗/采集/境界推进与客户端提示收口

  **What to do**: 在 server 侧实现最小可验证的战斗伤害、采集收益、经验累积与境界突破规则，并在状态变化时更新 `PlayerState`、`world_state.recent_events`、必要的 `event_alert` / narration。client 侧补齐对应提示：突破、掉血、采集成功、重大事件警报可通过已有 narration/toast/HUD 能力呈现，无需额外动态 UI。
  **Must NOT do**: 不得在未完成 `PlayerState` 的情况下单独发临时文本提示；不得把境界规则写成不可测试的散乱常量。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 涉及玩法数值、事件联动与状态推进。
  - Skills: `[]` — 基于已建立的 PlayerState/Zone/Event/Narration 基础实现。
  - Omitted: `["playwright"]` — 非浏览器。

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 26 | Blocked By: 4,7,8,17,18,19,23

  **References**:
  - Pattern: `docs/plan-server.md:290-295` — S10 战斗/采集/境界目标。
  - Pattern: `docs/plan-client.md:25-129` — narration/toast 可复用显示路径。
  - Pattern: `agent/packages/schema/src/world-state.ts:51-59` — `recent_events` 结构。
  - Pattern: `agent/packages/schema/src/common.ts:84-94` — `GameEventType` 可扩展基线。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo test gameplay_tests::combat_updates_player_state gameplay_tests::gathering_grants_experience gameplay_tests::realm_breakthrough_updates_payloads` 通过。
  - [ ] `cd client && ./gradlew test --tests "*EventAlert*" --tests "*Narration*"` 继续通过，能消费来自 gameplay 的状态提示。
  - [ ] 战斗/采集/突破至少各有一条 `recent_events` 可被 agent 观测。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Gameplay actions advance player progression and emit signals
    Tool: Bash
    Steps: run `cd server && cargo test gameplay_tests::combat_updates_player_state gameplay_tests::gathering_grants_experience gameplay_tests::realm_breakthrough_updates_payloads -- --nocapture`
    Expected: 战斗、采集、突破都能驱动 PlayerState、recent_events 与 payload 更新
    Evidence: .sisyphus/evidence/task-25-gameplay.log

  Scenario: Invalid progression state is rejected safely
    Tool: Bash
    Steps: run tests for insufficient experience / invalid karma / dead target edge cases
    Expected: 边界条件下不发生非法突破或负经验，测试可断言失败路径
    Evidence: .sisyphus/evidence/task-25-gameplay-error.txt
  ```

  **Commit**: YES | Message: `feat(server): add progression gameplay loop` | Files: `server/src/player/**`, `server/src/world/**`, `server/src/network/**`, `client/src/main/java/com/bong/client/**`, `server/tests/**`, `client/src/test/**`

- [x] 26. 创建跨层 smoke harness 并跑通最终闭环

  **What to do**: 新增 `scripts/smoke-law-engine.sh`，把 schema / tiandao / server / client 的关键测试与最小端到端联调串起来：创建或验证 worktree、启动 Redis（使用环境已存在 Redis，不引入 Docker）、启动 server 与 tiandao mock/real loop、运行 client build/tests、执行至少一组从 `world_state` → agent command/narration → server execution → client payload parsing 的自动化 smoke，并输出 evidence 文件列表。该脚本必须成为 DoD 中真实可执行的一部分。
  **Must NOT do**: 不得继续在 DoD 引用不存在的脚本；不得依赖人工盯屏作为唯一闭环验收；不得引入 Docker。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要整合全仓多子项目验证链路与最终证据产物。
  - Skills: `[]` — 复用已有 `scripts/smoke-test.sh` 风格即可。
  - Omitted: `["playwright"]` — 本仓当前闭环以 Rust/TS/JUnit/shell 为主，不需浏览器。

  **Parallelization**: Can Parallel: NO | Wave 5 | Blocks: Final Verification Wave | Blocked By: 2,3,10,13,15,20,22,24,25

  **References**:
  - Pattern: `scripts/smoke-test.sh:1-48` — 现有 smoke 脚本组织风格。
  - Pattern: `docs/local-test-env.md:111-142` — 联机验证链路与 smoke 基线。
  - Pattern: `agent/packages/tiandao/package.json:7-13` — `check/build/start:mock` 现有入口。
  - Pattern: `client/build.gradle:19-42` — client build/test 命令基线。

  **Acceptance Criteria** (agent-executable only):
  - [ ] 仓库中存在 `scripts/smoke-law-engine.sh`，且 `bash scripts/smoke-law-engine.sh` 可执行。
  - [ ] 脚本会运行 schema/server/client/tiandao 的最小测试集合，并在关键失败时返回非零退出码。
  - [ ] 脚本产出 `.sisyphus/evidence/task-26-smoke.log` 与至少一份失败路径 evidence。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Cross-layer smoke script validates the full closure
    Tool: Bash
    Steps: run `bash scripts/smoke-law-engine.sh`; save output and referenced evidence manifests
    Expected: 脚本成功完成 schema → tiandao → server → client 的最小闭环验证，并以 0 退出
    Evidence: .sisyphus/evidence/task-26-smoke.log

  Scenario: Missing dependency causes clear non-zero failure
    Tool: Bash
    Steps: run the script with Redis unavailable or a required build step intentionally skipped in CI-style mode
    Expected: 脚本快速失败，给出明确阶段名与错误说明，退出非零
    Evidence: .sisyphus/evidence/task-26-smoke-error.txt
  ```

  **Commit**: YES | Message: `test(integration): add law-engine cross-layer smoke` | Files: `scripts/smoke-law-engine.sh`, `.sisyphus/evidence/**`, `scripts/**`

## Final Verification Wave (4 parallel agents, ALL must APPROVE)
- [x] F1. Plan Compliance Audit — oracle
- [x] F2. Code Quality Review — unspecified-high
- [x] F3. Real Manual QA — unspecified-high (+ playwright if UI)
- [x] F4. Scope Fidelity Check — deep

## Commit Strategy
- 在 `bong-server-law-engine` worktree 中执行；禁止在主工作树提交实现代码。
- 原子提交默认按**任务号**切分：单任务完成、测试通过、evidence 生成后再提交。
- 允许将“纯脚手架 + 其直接单测”合并为一个提交，但不得跨波次混合 server/agent/client 不相关修改。
- 推荐提交前缀：
  - `chore(schema): ...`
  - `feat(server): ...`
  - `feat(agent): ...`
  - `feat(client): ...`
  - `test(integration): ...`
- Wave 2 之后，每个提交必须至少通过其子项目的最小测试集合；Wave 5 结束前必须通过全量 smoke。

## Success Criteria
- Redis IPC 契约、client payload 契约、Rust mirror、agent parser/runtime validate、client parser 五者一致，无 sample 漂移。
- Server 在 Redis 可用时能发布真实 world_state、消费 agent command/narration，并对 chat / zone / player_state 做定向处理。
- Agent 从“每个子 agent 各自 publish”升级为 merged publish，并具备 chat signals、peer decisions、world model、balance、era context。
- Client 不再只会显示 legacy welcome/heartbeat，而能消费 narration / zone_info / event_alert / player_state / ui_open。
- 没有 `.mca` 也能完成 M1/M3 全链路；有 `.mca` 时能切换到 Anvil + zones.json 模式。
- 玩家进入世界后，能够在无人工介入的 smoke 流程中观察到：天道叙事、zone HUD、player_state UI、指令可见效果、基础修仙进度。
