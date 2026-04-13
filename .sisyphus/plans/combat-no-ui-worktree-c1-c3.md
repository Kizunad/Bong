# Combat No-UI Worktree C1-C3

## TL;DR
> **Summary**: 以 `docs/plan-combat-no_ui.md` 最新顶部“实施边界”为唯一权威，在独立 worktree 中只落地 C1-C3 的 server/schema 子集：先修合同漂移与负灵域表示，再新增 `server/src/combat/` 骨架、统一玩家/NPC 调试攻击事务，并通过 Redis `bong:combat_realtime` / `bong:combat_summary` 暴露观测，不触碰 client UI、`ClientRequestV1`、`ClientPayloadV1`、CharacterRegistry 与 C4+ 终结/重生语义。
> **Deliverables**:
> - 新 worktree：`/workspace/worktrees/Bong-combat-no-ui-c1-c3`，分支：`atlas/combat-no-ui-c1-c3`
> - `server/src/combat/` 基础模块：components/events/system sets/raycast/debug ingress/attack resolver/death funnel
> - 合同修复：`ContamSource.attacker_id`、`LifeRecord.character_id`、`CultivationDetail` TS drift、负灵域范围放开到 `[-1.0, 1.0]`
> - Redis 观测合同：`bong:combat_realtime` / `bong:combat_summary` 的 Rust + TypeBox + generated schema + samples + tests
> **Effort**: Large
> **Parallel**: YES - 3 waves
> **Critical Path**: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8

## Context
### Original Request
- 用户先要求查看 `docs/plan-combat-no_ui.md` 并“计划一个 worktree 实现”。
- 用户随后更新了该文档，并明确要求“按照最新的来”。

### Interview Summary
- 本次执行以 `docs/plan-combat-no_ui.md:9-112` 的“实施边界（云端 worktree v1）”为唯一权威；正文后续与其冲突的 client/C4+/额外通道内容全部降为未来参考，不进入本 worktree。
- 本次 worktree **只做 C1-C3、只改 server + schema**；禁止修改 `client/`、`agent/packages/tiandao/`、`library-web/`、`agent/packages/schema/src/client-payload.ts`、`agent/packages/schema/src/client-request.ts`、`server/src/schema/client_payload.rs`、`server/src/schema/client_request.rs`。
- 身份锚点默认使用 `canonical_player_id(username)`；`Lifecycle.character_id` 与 `LifeRecord.character_id` 都落成 `String`，不引入 `CharacterRegistry`、不使用 `Uuid` 生命周期键。
- 传输边界固定：本 worktree 不新增 `bong:combat/*` Fabric CustomPayload，也不扩展 `ServerDataPayloadV1` 为 combat UI 服务；只新增 Redis `bong:combat_realtime` / `bong:combat_summary` 两条观测通道。
- `WorldStateV1` 在本 worktree **保持不扩展**；战斗观测完全通过上述两条 Redis 通道完成，避免 strict mirror/sample 大面积扩散。
- 调试入口固定为现有 `/bong combat <target> <health>` 命令的降级版 AttackIntent 注入；现有 NPC `MeleeAttackAction` 已存在，C2 只做“桥接到新 combat resolver”，不重做 big-brain 架构。

### Metis Review (gaps addressed)
- 已固定默认值：负灵域采用 **Option A**，直接把 `zone.spirit_qi` 语义扩展到 `[-1.0, 1.0]`；不新增 `is_negative_zone` 字段，避免 schema/zone/config 三层再拆一次。
- 已固定时钟表示：combat 新增的可持久化/可镜像时间字段统一使用 `u64 tick`，不使用 `Instant`，与 `ContamSource.introduced_at`、`LifeRecord.created_at`、`WorldStateV1.tick` 保持一致。
- 已固定 C2 死亡范围：本 worktree 只做 `DeathEvent` 统一收口与发布，不做终结归档、亡者博物馆、重生确认 UI、`deathInsight`/`终焉之言` 工具调用。
- 已固定 live ingress 范围：真实输入仅限 debug chat 与 NPC melee bridge；raycast 工具在 C1 落地并有单测，但不会在本 worktree 内接入客户端方向性攻击包。
- 已自动消解文档漂移：`docs/plan-combat-no_ui.md:53-57` 的“NPC 无 Attacking action”与实际仓库不符；实际代码已有 `MeleeRangeScorer` / `MeleeAttackAction`，因此本计划只桥接现有动作，不新增 scorer/action 家族。

## Work Objectives
### Core Objective
- 交付一份**决策完成型**执行蓝图，使实现代理能在单一 worktree 中完成 combat C1-C3 的 server/schema 子集：合同修复 → combat 模块骨架 → 调试/AI 共用攻击事务 → Redis 观测合同，全程不依赖人工判断，不越界到 UI、客户端协议扩展或 C4+ 生命周期语义。

### Deliverables
- `server/src/combat/` 新模块与注册接线：`mod.rs`、`components.rs`、`events.rs`、`raycast.rs`、`resolve.rs`、`debug.rs`（文件名可按任务内固定）。
- `server/src/main.rs` 注册顺序改为 `world -> player -> cultivation -> combat -> npc -> network`。
- `server/src/cultivation/components.rs`、`life_record.rs`、`cultivation/mod.rs` 的合同修复。
- `server/src/world/zone.rs` 与 `server/src/network/command_executor.rs` 的负灵域范围修复。
- `server/src/network/combat_bridge.rs` + `server/src/network/redis_bridge.rs` 的 combat Redis outbound。
- `server/src/schema/{mod.rs,channels.rs,combat_event.rs}` 与 `agent/packages/schema/src/{channels.ts,combat-event.ts,server-data.ts,index.ts,schema-registry.ts}` 的镜像更新。
- 对应 JSON samples、generated schema、Rust/TS 测试、以及 worktree evidence 约定。

### Definition of Done (verifiable conditions with commands)
- `git worktree list | grep "/workspace/worktrees/Bong-combat-no-ui-c1-c3"`
- `git -C "/workspace/worktrees/Bong-combat-no-ui-c1-c3" branch --show-current | grep "atlas/combat-no-ui-c1-c3"`
- `cd "/workspace/worktrees/Bong-combat-no-ui-c1-c3/server" && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `cd "/workspace/worktrees/Bong-combat-no-ui-c1-c3/agent/packages/schema" && npm test && npm run generate && git diff --exit-code -- generated`
- `cd "/workspace/worktrees/Bong-combat-no-ui-c1-c3" && bash scripts/smoke-test.sh`
- `git diff --name-only | grep -E '^(client/|agent/packages/tiandao/|library-web/|agent/packages/schema/src/client-(payload|request)\.ts|server/src/schema/client_(payload|request)\.rs)'` **无输出**

### Must Have
- 顶部“实施边界”优先于文档后续章节；本 worktree 只执行 C1-C3。
- `Lifecycle.character_id` / `LifeRecord.character_id` 均为 `String = canonical_player_id(username)`。
- 负灵域采用 `zone.spirit_qi ∈ [-1.0, 1.0]` 的单字段语义；`modify_zone` 与 zone config 验证同步放开。
- `ContamSource.attacker_id: Option<String>` 必落地；玩家用 `canonical_player_id`，NPC 用 `canonical_npc_id(entity)`。
- combat 新增时间字段统一为 `u64 tick`，不使用 `Instant`。
- `/bong combat` 与 NPC `MeleeAttackAction` 都必须走同一 `AttackIntent -> resolver -> CombatEvent/DeathEvent` 管线。
- Redis 只新增 `bong:combat_realtime` / `bong:combat_summary` 两条通道；`combat_summary` 与现有 world state 同 cadence（200 tick）。
- `WorldStateV1`、`ClientRequestV1`、`ClientPayloadV1`、`ServerDataPayloadV1` 不承载新的 combat UI/输入语义；`ServerDataV1` 仅补 `cultivation_detail` 漂移修复。

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- 不改 `client/`、`agent/packages/tiandao/`、`library-web/`。
- 不引入 `CharacterRegistry`、`PlayerIdentity` 实现、`Uuid` 生命周期 ID、同名玩家新档语义。
- 不实现 `bong:combat/*` Fabric CustomPayload，不扩展 `ClientRequestV1` / `ClientPayloadV1` / Rust 对应镜像。
- 不扩展 `WorldStateV1.players[].combat/lifecycle/stamina/status/derived_attrs`；这些留给后续阶段。
- 不新增 `bong:death_event`、`bong:tribulation_event`、`bong:status_effect`、`bong:anticheat` 等额外通道。
- 不实现 C3 之后的重生确认、终结归档、亡者博物馆、`deathInsight` tool、library-web 读取链路。
- 不在本 worktree 内做 weapon-specific reach 表、body-part live hitbox、多防御姿态、StatusEffect 全家桶；仅保留 skeleton 与未来扩展位。

## Verification Strategy
> ZERO HUMAN INTERVENTION — all verification is agent-executed.
- Test decision: **TDD + tests-after 混合**
  - **TDD**：合同修复、负灵域范围、combat 数据模型、attack decay/reach math、death routing、Redis schema roundtrip、channels frozen tests。
  - **tests-after**：module registration、`/bong combat` debug ingress、NPC melee bridge、Redis outbound wiring、`scripts/smoke-test.sh` 回归。
- QA policy: 每个任务都同时给 happy path 与 failure/edge case；无浏览器，无人工进游戏。
- Evidence: `.sisyphus/evidence/task-{N}-{slug}.{ext}`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave. <3 per wave (except final) = under-splitting.
> 先冻结合同与边界，再落 combat 骨架，再接攻击事务与 Redis 观测。

Wave 1: worktree 基线 + 合同修复 + 负灵域范围（Tasks 1-3）

Wave 2: combat 服务端骨架 + 攻击事务 + NPC bridge（Tasks 4-6）

Wave 3: Redis/schema 观测收口 + 全量验证（Tasks 7-8）

### Dependency Matrix (full, all tasks)
| Task | Depends On | Blocks |
|---|---|---|
| 1 | - | 2,3,4,5,6,7,8 |
| 2 | 1 | 4,5,7,8 |
| 3 | 1 | 4,5,7,8 |
| 4 | 1,2,3 | 5,6,7,8 |
| 5 | 1,2,3,4 | 6,7,8 |
| 6 | 1,2,3,4,5 | 7,8 |
| 7 | 1,2,3,4,5,6 | 8 |
| 8 | 1,2,3,4,5,6,7 | Final Verification Wave |

### Agent Dispatch Summary (wave → task count → categories)
- Wave 1 → 3 tasks → `git` (1), `ultrabrain` (2,3)
- Wave 2 → 3 tasks → `deep` (4,6), `ultrabrain` (5)
- Wave 3 → 2 tasks → `deep` (7), `unspecified-high` (8)

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task MUST have: Agent Profile + Parallelization + QA Scenarios.

- [x] 1. 创建固定 worktree、分支与 evidence 基线

  **What to do**: 创建独立 worktree：`/workspace/worktrees/Bong-combat-no-ui-c1-c3`，分支固定为 `atlas/combat-no-ui-c1-c3`，基于 `main`。记录 worktree 路径、当前分支、主工作树只读规划态、`.sisyphus/evidence/` 命名规则，以及 server/schema 的基线命令。后续所有实现、测试、证据、提交都只能在该 worktree 中完成。
  **Must NOT do**: 不得在 `/workspace/Bong` 主工作树直接改业务代码；不得使用其他 worktree 名或非 `atlas/*` 分支；不得在未验证 `main` 为基底时开始实现。

  **Recommended Agent Profile**:
  - Category: `git` — Reason: 这是纯 worktree/分支固定任务，必须先锁死执行位置。
  - Skills: `[]` — 无额外技能依赖。
  - Omitted: `["playwright"]` — 无浏览器工作流。

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 2,3,4,5,6,7,8 | Blocked By: -

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `.sisyphus/plans/tiandao-fullstack-closure.md:6,23,48,135-173` — 仓库现有 worktree 计划采用 `/workspace/worktrees/*` + 分支固定 + evidence 基线的模式。
  - Pattern: `docs/plan-combat-no_ui.md:13-22` — 本次只做 C1-C3，worktree 范围固定。
  - Pattern: `CLAUDE.md:1-34` — 仓库结构与常用测试命令基线。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `git worktree list | grep "/workspace/worktrees/Bong-combat-no-ui-c1-c3"` 返回目标 worktree。
  - [ ] `git -C "/workspace/worktrees/Bong-combat-no-ui-c1-c3" branch --show-current` 输出 `atlas/combat-no-ui-c1-c3`。
  - [ ] `.sisyphus/evidence/task-1-worktree.txt` 记录 worktree 路径、当前分支、`git status --short`、base branch = `main`。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Happy path worktree bootstrap
    Tool: Bash
    Steps: run `git worktree add -b atlas/combat-no-ui-c1-c3 /workspace/worktrees/Bong-combat-no-ui-c1-c3 main`; run `git -C /workspace/worktrees/Bong-combat-no-ui-c1-c3 status --short`; save output to `.sisyphus/evidence/task-1-worktree.txt`
    Expected: worktree 创建成功；目标分支为 `atlas/combat-no-ui-c1-c3`；主工作树不承载实现代码
    Evidence: .sisyphus/evidence/task-1-worktree.txt

  Scenario: Duplicate worktree path is rejected
    Tool: Bash
    Steps: rerun the same `git worktree add ...` command after creation and capture stderr
    Expected: 命令非零退出且不会覆盖已有 worktree；错误输出写入 `.sisyphus/evidence/task-1-worktree-error.txt`
    Evidence: .sisyphus/evidence/task-1-worktree-error.txt
  ```

  **Commit**: NO | Message: `chore(repo): create combat no-ui worktree baseline` | Files: `[]`

- [x] 2. 修复修炼侧合同锚点：`ContamSource.attacker_id` + `LifeRecord.character_id`

  **What to do**: 先修所有 combat 会依赖的共享合同，再开始 `server/src/combat`。在 `server/src/cultivation/components.rs` 为 `ContamSource` 增加 `attacker_id: Option<String>`，并明确玩家写 `canonical_player_id(username)`、NPC 写 `canonical_npc_id(entity)`；在 `server/src/cultivation/life_record.rs` 为 `LifeRecord` 增加 `character_id: String`。同步更新 `LifeRecord::default` 行为、客户端自动 attach 路径、必要的 serde/roundtrip/unit tests，确保这两个字段在后续 combat 事务和生平卷追溯中可直接使用。
  **Must NOT do**: 不得把 `character_id` 做成 `Uuid`；不得引入 `CharacterRegistry`、`PlayerIdentity` 新模块；不得修改 `client/`、`tiandao/`；不得把 `attacker_id` 设计成 Entity 临时 ID。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是跨模块合同修复，必须一次定死数据语义。
  - Skills: `[]` — 依赖现有 Rust 单元测试即可。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 4,5,7,8 | Blocked By: 1

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-combat-no_ui.md:28-38,84-87` — 2a/2b 前置修复与身份策略已固定。
  - API/Type: `server/src/cultivation/components.rs:183-195` — `ContamSource` / `Contamination` 当前结构。
  - API/Type: `server/src/cultivation/life_record.rs:71-98` — `LifeRecord` 当前结构与摘要 API。
  - Pattern: `server/src/player/state.rs:146-148` — `canonical_player_id(username)` 真源。
  - Pattern: `server/src/npc/brain.rs:92-94` — `canonical_npc_id(entity)` 真源。
  - Pattern: `server/src/cultivation/mod.rs:155-173` — 新客户端自动 attach cultivation bundle 的地方，若 `LifeRecord` 无默认值会在这里崩。
  - Test: `server/src/cultivation/death_hooks.rs:76-104` — revive penalty 已写 LifeRecord，可作为受影响单测入口。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `ContamSource` 序列化后含 `attacker_id` 可选字段，旧测试与新测试都通过。
  - [ ] `LifeRecord` 持有 `character_id: String`，新 join attach 路径不会 panic，且 `recent_summary_text()` 行为不变。
  - [ ] `cd server && cargo test cultivation::` 通过。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Contract anchors compile and serialize
    Tool: Bash
    Steps: run `cd server && cargo test cultivation:: -- --nocapture`; save output to `.sisyphus/evidence/task-2-cultivation-contracts.log`
    Expected: ContamSource/LifeRecord 相关测试通过，serde roundtrip 正常
    Evidence: .sisyphus/evidence/task-2-cultivation-contracts.log

  Scenario: Missing attacker/character identity is rejected by tests
    Tool: Bash
    Steps: add/adjust unit tests so a combat write without canonicalized identity fails the expected invariant; run targeted `cargo test`
    Expected: 测试能证明 `attacker_id` 和 `character_id` 语义已固定为字符串身份锚点，而非临时 Entity/Uuid
    Evidence: .sisyphus/evidence/task-2-cultivation-contracts-error.log
  ```

  **Commit**: YES | Message: `fix(cultivation): add combat identity anchors` | Files: `server/src/cultivation/components.rs`, `server/src/cultivation/life_record.rs`, `server/src/cultivation/mod.rs`, `server/src/cultivation/death_hooks.rs`, related tests

- [x] 3. 放开负灵域范围并补齐 `cultivation_detail` schema 漂移

  **What to do**: 完成 2c/2d 两个前置合同修复。第一，采用固定方案 A：把 `zone.spirit_qi` 范围从 `[0.0,1.0]` 放开到 `[-1.0,1.0]`，同步修改 `server/src/world/zone.rs` 的 config 验证、`server/src/network/command_executor.rs` 的 `modify_zone` clamp、必要样例与测试，保证 `negative_zone.rs` 终于能被真实触发。第二，只做允许范围内的 schema 例外修复：在 `agent/packages/schema/src/server-data.ts` 补充 `cultivation_detail`，同步 `schema-registry.ts` / generated artifacts / samples / tests，使 TS mirror 与 Rust `ServerDataV1` 对齐。
  **Must NOT do**: 不得新加 `is_negative_zone` 字段；不得扩 `ClientRequestV1` / `ClientPayloadV1`；不得在本任务里引入 combat server-data 类型；不得改 `world_state` 结构。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是 Rust/TS 双边合同一次性修复，出错会导致后续所有测试失真。
  - Skills: `[]` — 直接用 cargo/vitest/generate 即可。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 4,5,7,8 | Blocked By: 1

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-combat-no_ui.md:40-52` — 2c/2d 已说明现状与修复目标。
  - API/Type: `server/src/world/zone.rs:228-242` — 当前 `spirit_qi` 限定在 `[0.0, 1.0]`。
  - Pattern: `server/src/network/command_executor.rs:14-15` — `modify_zone` 当前 clamp 常量。
  - Pattern: `server/src/cultivation/negative_zone.rs` — 真实消费负值的下游系统。
  - API/Type: `server/src/schema/server_data.rs:71-86` — Rust `CultivationDetail` 已存在。
  - API/Type: `agent/packages/schema/src/server-data.ts:7-110` — TS 侧仍缺 `cultivation_detail`。
  - Pattern: `agent/packages/schema/src/schema-registry.ts:31-85` — generated schema 注册表。
  - Pattern: `agent/packages/schema/src/generate.ts:1-27` — freshness / generate 命令入口。
  - Test: `agent/packages/schema/tests/schema.test.ts:104-176` — server-data samples 验证入口。

  **Acceptance Criteria** (agent-executable only):
  - [ ] zone config 与 `modify_zone` 都允许 `spirit_qi = -1.0..=1.0`，并有 Rust 单测覆盖边界值。
  - [ ] `agent/packages/schema/src/server-data.ts` 接受 `cultivation_detail`，`npm test && npm run generate && git diff --exit-code -- generated` 全绿。
  - [ ] `cd server && cargo test schema::server_data && cargo test world::zone && cargo test network::command_executor && cargo test cultivation::negative_zone` 通过。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Negative spirit qi is now legal and usable
    Tool: Bash
    Steps: run targeted Rust tests for zone validation and command_executor clamp logic; save output to `.sisyphus/evidence/task-3-negative-zone.log`
    Expected: `spirit_qi=-1.0` 可通过验证且不会被 clamp 回 0.0
    Evidence: .sisyphus/evidence/task-3-negative-zone.log

  Scenario: TS mirror catches cultivation_detail drift
    Tool: Bash
    Steps: run `cd agent/packages/schema && npm test && npm run generate && git diff --exit-code -- generated`; save output
    Expected: `cultivation_detail` 被纳入 `ServerDataV1` union 且 generated artifacts 无漂移
    Evidence: .sisyphus/evidence/task-3-server-data-drift.log
  ```

  **Commit**: YES | Message: `fix(schema): align cultivation detail and negative zones` | Files: `server/src/world/zone.rs`, `server/src/network/command_executor.rs`, `server/src/schema/server_data.rs` (tests only if needed), `agent/packages/schema/src/server-data.ts`, `agent/packages/schema/src/schema-registry.ts`, `agent/packages/schema/samples/**`, generated artifacts, related tests

- [x] 4. 建立 `server/src/combat/` 模块骨架与统一系统顺序

  **What to do**: 在所有合同修复稳定后，新增 `server/src/combat/` 模块，并在 `server/src/main.rs` 注册。固定文件与职责：`mod.rs`（register + SystemSet + events registration）、`components.rs`（仅本 worktree 需要的最小 structs/components）、`events.rs`（`AttackIntent`/`CombatEvent`/`DeathEvent`）、`raycast.rs`（方案 A AABB+slab-test）、`debug.rs`（chat/debug ingress helper）、`resolve.rs`（空 resolver 占位）。同时把主 app 注册顺序改为 `world -> player -> cultivation -> combat -> npc -> network`。所有 combat 时间字段统一用 `u64 tick`，不得使用 `Instant`。C1 只实现最小数据模型：`Wounds`、`CombatState`、`Stamina`、`Lifecycle`、`DerivedAttrs`、`CombatClock` resource，以及 `IntentSet -> PhysicsSet -> ResolveSet -> EmitSet` 四段顺序。
  **Must NOT do**: 不得在这一任务里接客户端 payload；不得接完整 status effect；不得接 body-part 分类全家桶；不得把 `Lifecycle.character_id` 做成 `Uuid`；不得在未完成单测前往 `network/` 接线。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 新模块、新注册顺序和系统集需要端到端保持一致。
  - Skills: `[]` — 直接用 Rust 即可。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 5,6,7,8 | Blocked By: 1,2,3

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `server/src/main.rs:29-42` — 当前模块注册顺序。
  - Pattern: `server/src/cultivation/mod.rs:86-151` — `register(app)` + `add_event` + `add_systems(...after(...))` 的标准插件写法。
  - Pattern: `docs/plan-combat-no_ui.md:19-21,144-296,730-765,2696-2710` — C1 最小范围、组件草案、tick sets、阶段验收。
  - Pattern: `server/src/world/events.rs:108-220` — Resource + recent event 组织范式。
  - Pattern: `server/src/network/mod.rs:142-163` — 系统编排中枢，combat 后续要遵循同样的显式顺序约束。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `server/src/combat/` 可独立注册并编译，`cargo test` 通过。
  - [ ] `main.rs` 已接入 `combat::register(&mut app)`，且顺序位于 `cultivation` 与 `npc` 之间。
  - [ ] combat 单测能证明 `Lifecycle.character_id` 为 `String`、`Stamina` 状态机与 `raycast` 基础命中/未命中判定成立。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Combat module skeleton compiles and registers
    Tool: Bash
    Steps: run `cd server && cargo test combat:: -- --nocapture`; save output to `.sisyphus/evidence/task-4-combat-skeleton.log`
    Expected: combat 模块文件存在、事件/系统顺序注册成功、最小组件单测通过
    Evidence: .sisyphus/evidence/task-4-combat-skeleton.log

  Scenario: Raycast baseline rejects impossible hits
    Tool: Bash
    Steps: add unit tests for no-hit/out-of-range cases in `raycast.rs`; run targeted tests
    Expected: 超距/无交点时返回 miss，不会伪造命中
    Evidence: .sisyphus/evidence/task-4-combat-skeleton-error.log
  ```

  **Commit**: YES | Message: `feat(combat): add combat module skeleton` | Files: `server/src/main.rs`, `server/src/combat/**`, related tests

- [x] 5. 把 `/bong combat` 降级为 debug AttackIntent，并实现最小攻击事务

  **What to do**: 以现有 `/bong combat <target> <health>` 为唯一玩家输入入口，把 `server/src/network/chat_collector.rs` 的解析结果改为 enqueue 到新的 combat debug path，而不是直接走 `player/gameplay.rs::apply_combat_action`。在 `resolve.rs` 实现最小攻击事务：解析 target、基础 reach 校验、固定 decay 函数、扣减 health/stamina、向 `Contamination.entries` 追加带 `attacker_id` 的条目、向 `MeridianSystem.throughput_current` 累加、必要时 emit `DeathEvent`。旧 `GameplayAction::Combat` 保留但转成 debug-only 包装器，`apply_combat_action` 不再承载真实战斗语义。首版不做 body-part 分类和完整 style tree，只做统一单目标命中事务。
  **Must NOT do**: 不得接 `bong:combat/*` client 包；不得保留两套真实战斗语义；不得实现完整伤口部位倍率、防御窗口、StatusEffect 全量；不得把 decay/qi 写入放在 `player/gameplay.rs` 里继续分叉。

  **Recommended Agent Profile**:
  - Category: `ultrabrain` — Reason: 这是旧战斗路径向新事务管线的收口点，最容易产生双语义。
  - Skills: `[]` — Rust 逻辑和测试即可。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 6,7,8 | Blocked By: 1,2,3,4

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-combat-no_ui.md:89-97` — `/bong combat` 的收敛规则。
  - Pattern: `docs/plan-combat-no_ui.md:2696-2710,2715-2723` — C1/C2 最小验收目标。
  - Pattern: `server/src/network/chat_collector.rs:168-182` — `/bong combat` 当前解析入口。
  - Pattern: `server/src/player/gameplay.rs:54-59,69-73,165-260,271-326` — 旧 CombatAction/GameplayAction 与 `apply_combat_action` 的现状。
  - API/Type: `server/src/cultivation/components.rs:183-195,197-236` — `Contamination` / `MeridianSystem.throughput_current` 写入目标。
  - API/Type: `server/src/cultivation/death_hooks.rs:26-41` — 后续 `DeathEvent` 对接修炼 death hooks 的终点契约。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `/bong combat` 触发的是 combat resolver，不再直接改 `PlayerState` 假战斗数据。
  - [ ] 最小 resolver 能写 `Contamination.entries.attacker_id`、累加 `throughput_current`、在 health≤0 时 emit `DeathEvent`。
  - [ ] `cargo test` 中有 happy/failure 两类测试：合法 debug attack、越界/空目标拒绝。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Debug combat command flows through AttackIntent
    Tool: Bash
    Steps: run targeted Rust tests covering `parse_gameplay_action` → debug bridge → combat resolver; save output to `.sisyphus/evidence/task-5-debug-combat.log`
    Expected: `/bong combat` 不再直接调用旧 `apply_combat_action` 作为真实战斗实现，而是进入新 AttackIntent/resolver 路径
    Evidence: .sisyphus/evidence/task-5-debug-combat.log

  Scenario: Invalid attack is rejected without side effects
    Tool: Bash
    Steps: run tests for empty target/out-of-range/negative health hints
    Expected: resolver 发拒绝或直接丢弃，不写污染、不扣 health、不产生 death event
    Evidence: .sisyphus/evidence/task-5-debug-combat-error.log
  ```

  **Commit**: YES | Message: `feat(combat): route debug attacks through combat resolver` | Files: `server/src/network/chat_collector.rs`, `server/src/player/gameplay.rs`, `server/src/combat/**`, related tests

- [x] 6. 复用现有 NPC melee 行为，桥接到同一 combat resolver

  **What to do**: 明确按实际代码而不是文档旧描述执行：仓库已存在 `MeleeRangeScorer` 与 `MeleeAttackAction`，本任务只把 `server/src/npc/brain.rs::melee_attack_action_system` 的结果桥接到 combat AttackIntent/resolver。保留 big-brain scorer/action 拓扑，不新增 `AttackingScorer` / `AttackingAction` 家族。桥接完成后，玩家→NPC 和 NPC→玩家都通过同一 damage/contam/death 路径，且 `NpcStateKind::Attacking` 若已有显示语义则继续沿用。补充 dual-direction 单测与必要的 tracer log。
  **Must NOT do**: 不得重做 NPC AI 架构；不得扩展新 behavior tree family；不得把 NPC 攻击继续做成旧 knockback-only 旁路；不得在本任务里引入武器系统或 pathfinding 大改。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要跨 `npc/` 与 `combat/` 两个模块做真实接线，但范围应严格受控。
  - Skills: `[]` — 现有 big-brain + Rust 测试即可。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 7,8 | Blocked By: 1,2,3,4,5

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-combat-no_ui.md:53-57` — 文档认为 NPC attack gap 存在，但实际代码需以仓库为准。
  - API/Type: `server/src/npc/brain.rs:52-73,203-230,487-519` — `MeleeRangeScorer` / `MeleeAttackAction` / `melee_attack_action_system` 已存在。
  - Pattern: `server/src/network/mod.rs:226-280` — world state 收集 NPC action state 的现有方式，桥接时不要破坏。
  - Pattern: `server/src/player/gameplay.rs` — 旧 combat 分支已经收敛，NPC 必须接新 resolver。

  **Acceptance Criteria** (agent-executable only):
  - [ ] NPC melee 动作最终调用 combat resolver，而不是独立旧逻辑。
  - [ ] player→npc 与 npc→player 都能通过相同 combat event/death funnel 单测。
  - [ ] `cd server && cargo test npc::brain && cargo test combat::` 通过。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: NPC melee enters combat resolver
    Tool: Bash
    Steps: run targeted tests for `melee_attack_action_system` bridge; save output to `.sisyphus/evidence/task-6-npc-combat.log`
    Expected: NPC 在 melee range 内时经由统一 resolver 结算，而非旁路逻辑
    Evidence: .sisyphus/evidence/task-6-npc-combat.log

  Scenario: No duplicate damage path remains
    Tool: Bash
    Steps: add regression test ensuring the same NPC melee tick cannot both invoke old path and new resolver
    Expected: 单次动作只触发一次 combat resolution
    Evidence: .sisyphus/evidence/task-6-npc-combat-error.log
  ```

  **Commit**: YES | Message: `feat(npc): bridge melee ai into combat resolver` | Files: `server/src/npc/brain.rs`, `server/src/combat/**`, related tests

- [x] 7. 新增 combat Redis 观测合同并接入 network

  **What to do**: 只按顶部边界新增两条 combat Redis 通道：`bong:combat_realtime` 与 `bong:combat_summary`。在 Rust 侧新增 `server/src/schema/combat_event.rs`（含 realtime facts 与 summary payload）、扩展 `server/src/schema/channels.rs` 常量、把 `RedisOutbound` 增补对应变体、在 `redis_bridge.rs` 做 validation+publish、在 `network/` 新增 `combat_bridge.rs` 从 `CombatEvent`/`DeathEvent` 聚合发送。TS 侧新增 `agent/packages/schema/src/combat-event.ts`，更新 `channels.ts`、`index.ts`、`schema-registry.ts`、samples、generated artifacts 和 tests。`combat_summary` 固定为与 `WORLD_STATE_PUBLISH_INTERVAL_TICKS = 200` 同 cadence 发布的聚合摘要，不做 durable audit log；`combat_realtime` 固定为 per-attack/per-death pub/sub 事件。**本任务不扩展 world-state 结构。**
  **Must NOT do**: 不得新增 `death_event` / `tribulation_event` / `status_effect` / `anticheat` 通道；不得把 combat 信息塞回 `ServerDataV1` 或 `WorldStateV1`；不得给 `tiandao/` 增订阅实现。

  **Recommended Agent Profile**:
  - Category: `deep` — Reason: 需要 Rust/TS schema、network bridge、samples、generated 一次收口。
  - Skills: `[]` — 现有 schema/toolchain 足够。
  - Omitted: `["playwright"]` — 无 UI。

  **Parallelization**: Can Parallel: NO | Wave 3 | Blocks: 8 | Blocked By: 1,2,3,4,5,6

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `docs/plan-combat-no_ui.md:73-80,2600-2605` — 顶部边界只允许两条 Redis 观测通道；后文多余通道不进入本 worktree。
  - API/Type: `server/src/schema/channels.rs:1-25` — 当前 Rust channel 常量真源。
  - API/Type: `agent/packages/schema/src/channels.ts:1-46` — 当前 TS channel 常量真源。
  - Pattern: `server/src/network/redis_bridge.rs:26-42,237-299` — `RedisOutbound` + `prepare_outbound_command` 扩展点。
  - Pattern: `server/src/network/cultivation_bridge.rs` — 事件→Redis outbound 的现有桥接样板。
  - Pattern: `server/src/network/mod.rs:142-163,200-285` — network systems 注册与 world_state publish cadence。
  - Pattern: `agent/packages/schema/src/index.ts:1-24`、`schema-registry.ts:31-85` — schema 导出与 generated 注册表。
  - Test: `agent/packages/schema/tests/schema.test.ts:104-176` — sample acceptance 测试组织方式。

  **Acceptance Criteria** (agent-executable only):
  - [ ] Rust/TS 双端新增 `combat_realtime` / `combat_summary` 常量且 frozen tests 通过。
  - [ ] `combat-event.ts` / Rust mirror / samples / generated artifacts 全部齐备，`npm test && npm run generate && git diff --exit-code -- generated` 通过。
  - [ ] `combat_realtime` 与 `combat_summary` 均能通过 `RedisOutbound` 发布；summary cadence 明确绑定 200 tick。
  - [ ] `WorldStateV1` 与 `ServerDataV1` 无新增 combat 字段/变体（除先前的 `cultivation_detail` 修复之外）。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Combat channels publish valid payloads
    Tool: Bash
    Steps: run Rust tests covering `RedisOutbound` combat variants and TS schema tests for `combat-event.ts`; save outputs to `.sisyphus/evidence/task-7-combat-schema.log`
    Expected: realtime/summary payload 都通过双端校验并可序列化发布
    Evidence: .sisyphus/evidence/task-7-combat-schema.log

  Scenario: Scope guard rejects extra combat channels or world-state drift
    Tool: Bash
    Steps: run schema tests plus grep-based assertion over touched files; verify no `death_event|tribulation_event|status_effect|anticheat` channels and no WorldState combat fields were added
    Expected: 只存在 `combat_realtime` / `combat_summary` 两条新通道；WorldState 未扩展
    Evidence: .sisyphus/evidence/task-7-combat-schema-error.log
  ```

  **Commit**: YES | Message: `feat(schema): add combat redis observability contracts` | Files: `server/src/schema/channels.rs`, `server/src/schema/combat_event.rs`, `server/src/network/redis_bridge.rs`, `server/src/network/combat_bridge.rs`, `server/src/network/mod.rs`, `agent/packages/schema/src/channels.ts`, `agent/packages/schema/src/combat-event.ts`, `agent/packages/schema/src/index.ts`, `agent/packages/schema/src/schema-registry.ts`, samples, generated artifacts, related tests

- [x] 8. 运行全量验证、范围审计与 smoke 收口

  **What to do**: 在 worktree 内运行完整验证：Rust fmt/clippy/test、schema test/generate freshness、顶层 smoke。补充一个范围审计步骤，强制证明 forbidden path 未被触碰、未新增额外通道、未扩展 `WorldStateV1` 与 `ClientRequestV1`/`ClientPayloadV1`。若本阶段发现缺陷，只允许在已触及路径内修复且不得扩 scope。
  **Must NOT do**: 不得借验证名义新增功能；不得把失败解释为“等 UI 阶段再修”；不得跳过 generated freshness 或 forbidden path 审计。

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: 这是跨模块验证与范围稽核，强调严谨而不是新实现。
  - Skills: `[]` — 现有命令和测试足够。
  - Omitted: `["playwright"]` — 无浏览器。

  **Parallelization**: Can Parallel: NO | Wave 3 | Blocks: Final Verification Wave | Blocked By: 1,2,3,4,5,6,7

  **References** (executor has NO interview context — be exhaustive):
  - Pattern: `CLAUDE.md:7-43` — 仓库标准验证命令。
  - Pattern: `scripts/smoke-test.sh:1-48` — 现有 smoke 脚本。
  - Pattern: `docs/plan-combat-no_ui.md:59-80` — 禁碰目录与仅允许的观测通道。
  - Pattern: `.sisyphus/plans/combat-no-ui-worktree-c1-c3.md` — 本计划的 must have / must not have 全部条目。

  **Acceptance Criteria** (agent-executable only):
  - [ ] `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过。
  - [ ] `cd agent/packages/schema && npm test && npm run generate && git diff --exit-code -- generated` 通过。
  - [ ] `bash scripts/smoke-test.sh` 通过或仅出现与本任务无关且已存在的已知跳过项。
  - [ ] forbidden path 审计无命中；多余 combat channels 审计无命中；`WorldStateV1` 未扩展 combat 字段。

  **QA Scenarios** (MANDATORY — task incomplete without these):
  ```
  Scenario: Full validation stack stays green
    Tool: Bash
    Steps: run full Rust + schema + smoke command matrix; save output to `.sisyphus/evidence/task-8-full-validation.log`
    Expected: 格式、静态检查、单测、schema freshness、smoke 全部通过
    Evidence: .sisyphus/evidence/task-8-full-validation.log

  Scenario: Scope audit catches forbidden drift
    Tool: Bash
    Steps: run `git diff --name-only` and grep audits for forbidden paths, extra channels, and world-state/client-request/client-payload drift; save output
    Expected: 无 forbidden path 变更、无多余 channel、无 combat WorldState/client payload 扩散
    Evidence: .sisyphus/evidence/task-8-scope-audit.log
  ```

  **Commit**: NO | Message: `fix(combat): close verification regressions` | Files: restricted to already touched paths only

## Final Verification Wave (4 parallel agents, ALL must APPROVE)
- [x] F1. Plan Compliance Audit — oracle
- [x] F2. Code Quality Review — unspecified-high
- [x] F3. Real Manual QA — unspecified-high
- [x] F4. Scope Fidelity Check — deep

## Commit Strategy
- Commit 1: `chore(repo): create combat no-ui worktree baseline`
- Commit 2: `fix(cultivation): add combat identity anchors`
- Commit 3: `fix(schema): align cultivation detail and negative zones`
- Commit 4: `feat(combat): add combat module skeleton`
- Commit 5: `feat(combat): route debug attacks through combat resolver`
- Commit 6: `feat(npc): bridge melee ai into combat resolver`
- Commit 7: `feat(schema): add combat redis observability contracts`
- Task 8 只做验证/审计，不再开新功能提交；如验证暴露小缺陷，只允许修复已触及路径并单独提交 `fix(combat): close verification regressions`。

## Success Criteria
- worktree 与分支按固定命名落地，主工作树只保留计划/证据。
- `ContamSource.attacker_id`、`LifeRecord.character_id`、`cultivation_detail` drift、负灵域范围四类合同问题全部闭环，并有 Rust/TS 测试覆盖。
- `server/src/combat/` 能注册并编译，`/bong combat` 与 NPC melee 都能进入同一 attack resolver。
- `DeathEvent` 能统一吸收 combat kill 与 `CultivationDeathTrigger`，并输出到 `bong:combat_realtime`。
- `bong:combat_summary` 按 200 tick cadence 输出聚合摘要，且只走 Redis、不改 world state。
- 没有任何 forbidden path 改动，也没有额外通道或 client/custom-payload 扩散。
