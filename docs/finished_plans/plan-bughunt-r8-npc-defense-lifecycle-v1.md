# plan-bughunt-r8-npc-defense-lifecycle-v1

> **已完成（2026-07-09，归档审计确认）**。一句话主题：从 round8 聚合骨架中拆出 `#7/#8 NPC defense scorer/action 缺 Lifecycle 门控`，独立修复 NPC 防御决策链，确保非 `Alive` NPC 不再参与防御评分、不开启格挡动作、也不会发出 `DefenseIntent`。

## 阶段总览

- **主题**：补齐 NPC defense scorer/action 的生命周期硬门，让 `LifecycleState != Alive` 的 NPC 不能进入主动防御链路。
- **验证日期**：2026-07-08。
- **P0 复核**：已确认 scorer 侧已有 `Option<&Lifecycle>` 门控，但缺 targeted lifecycle 回归；action 侧修复前缺 `Option<&Lifecycle>` 门控。
- **P1 实现**：`npc_defense_action_system` 查询已补入 `Option<&Lifecycle>`，`Requested` / `Executing` 两个分支统一使用生命周期 helper 阻断非 `Alive`。
- **P2 回归**：补齐 scorer/action targeted tests，覆盖 `NearDeath`、`AwaitingRevival`、`Terminated`、`Alive` happy path 与无 `Lifecycle` 兼容。
- **P3 收口**：本 PR 不新增招式、动画、VFX、SFX、icon，不改变真元/灵气流动路径。

## 结论

- **类型**：真实 bug，`fix_pr`
- **范围**：`server/src/npc/brain/scorers_combat.rs`、`server/src/npc/brain/actions_combat.rs`
- **修复前代码复核**：
  - `npc_defense_scorer_system` 在当前 `origin/main` 已经带 `Option<&Lifecycle>` 查询，并对非 `Alive` 设置 `Score=0.0`；round8 #7 的实现缺口已被先前变更补上，但缺 NearDeath/非 Alive targeted 回归测试。
  - `npc_defense_action_system` 仍只查询 `(&Cultivation, Option<&StatusEffects>)`，`Requested` 与 `Executing` 分支都没有 Lifecycle 门控；round8 #8 仍真实存在。
- **当前实现状态**：
  - `npc_defense_action_system` 已查询 `(&Cultivation, Option<&StatusEffects>, Option<&Lifecycle>)`。
  - `Requested` 与 `Executing` 分支已通过 `lifecycle_blocks_combat_action(...)` 统一阻断 `NearDeath`、`AwaitingRevival`、`Terminated`，并保留缺失 `Lifecycle` 的旧实体兼容行为。
- **一句话根因**：combat brain 的其他主动战斗行为已经把 `LifecycleState != Alive` 视为硬门，defense action 仍允许垂死 NPC 从 Requested 进入 Executing，并在 Executing 阶段继续发出格挡意图，破坏“只有活体 NPC 能进行主动防御”的规则。

## 真实规则 / 设计约束

1. `LifecycleState::Alive` 是 NPC 主动战斗行为的前置条件；`NearDeath`、`AwaitingRevival`、`Terminated` 等非活体状态不能参与战斗决策。
2. scorer 层应把非 `Alive` NPC 的战斗分数压到 `0.0`，避免行为树继续选择该动作。
3. action 层必须独立兜底：即使旧 ActionState 已经处于 `Requested` 或 `Executing`，非 `Alive` NPC 也要失败退出，不能发出 combat intent。
4. 同文件里 chase / melee / dash scorer 与 melee action 已经遵守该规则，defense 应与它们保持同一生命周期口径。

## 修复前违反链路

1. NPC 进入 `NearDeath` / `AwaitingRevival` / `Terminated` 后仍可能保留或获得 `NpcDefenseAction` 的 `Requested` / `Executing` 状态。
2. 修复前 `npc_defense_action_system` 查询不到 `Lifecycle`，无法区分 `Alive` 与非活体状态。
3. `Requested` 分支只检查境界、真元、`ParryRecovery`，非活体 NPC 仍可进入 `Executing`。
4. `Executing` 分支按间隔直接发送 `DefenseIntent { defender, issued_at_tick }`。
5. 下游防御解析会把该 `DefenseIntent` 当成真实 parry 窗口处理，导致非活体 NPC 仍能格挡。

## 修复方案

1. `npc_defense_scorer_system`：
   - 保持当前 `Lifecycle` 门控不放松。
   - 补 targeted test：`NearDeath` / 非 `Alive` NPC 即使有 nearest player 且境界满足，也必须得到 `Score=0.0`。
2. `npc_defense_action_system`：
   - 查询补入 `Option<&Lifecycle>`。
   - 对 `ActionState::Requested | ActionState::Executing` 的非 `Alive` NPC 直接置 `Failure` 并跳过。
   - 对缺失 `Lifecycle` 的旧测试实体保持兼容，按现有行为视为允许执行，避免扩大 blast radius。
3. 回归测试：
   - `Requested + NearDeath`：动作必须失败，不能进入 `Executing`。
   - `Executing + NearDeath`：动作必须失败，不能发 `DefenseIntent`。
   - `Executing + Alive`：保留原有可发 intent 的 happy path。

## 验收

- `cd server && cargo fmt --check`
- `cd server && cargo test npc_defense`
- 必要时追加 `cd server && cargo test npc::brain`
- 自启无上下文 gpt-5.5 xhigh read-only validator；只有 PASS 才 push 分支并开 PR。

## 非本 PR 范围

- 不处理 round8 聚合骨架里的 P0 modifier orphan、P1 距离衰减、P3 TSY 生命周期问题。
- 不移动 `docs/plans-skeleton/plan-bughunt-r8-findings-v1.md`，避免与其他 worker 冲突。
- 不改 combat intent 解析层；本 PR 只收紧 NPC defense scorer/action 的生命周期入口。

## Finish Evidence

### 落地清单

- **scorer 侧**：`server/src/npc/brain/scorers_combat.rs::npc_defense_scorer_system` 查询 `Option<&Lifecycle>`，非 `Alive` 一律 `Score=0.0`；新增 `npc_defense_scorer_alive_lifecycle_scores_normally` / `npc_defense_scorer_near_death_lifecycle_scores_zero` / `npc_defense_scorer_terminated_lifecycle_scores_zero` / `npc_defense_scorer_awaiting_revival_lifecycle_scores_zero` / `npc_defense_scorer_no_lifecycle_component_scores_normally` 五条 targeted 测试。
- **action 侧**：`server/src/npc/brain/actions_combat.rs::npc_defense_action_system` 查询扩为 `(&Cultivation, Option<&StatusEffects>, Option<&Lifecycle>)`，新增 `lifecycle_blocks_combat_action(...)` helper；`Requested` / `Executing` 两分支均调用该 helper 阻断非 `Alive`，缺失 `Lifecycle` 的旧实体保持兼容放行。新增 `npc_defense_action_requested_non_alive_states_fail_before_executing` / `npc_defense_action_executing_non_alive_states_fail_without_intent` / `npc_defense_action_executing_alive_lifecycle_emits_intent` / `npc_defense_action_no_lifecycle_emits_intent_on_first_fire` 四条 targeted 测试。

### 关键 commit

- `e5d7203f`（2026-07-08）：新增 r8 NPC 防御生命周期门控计划（骨架 promotion）。
- `79c4e025`（2026-07-09）：修复 NPC 防御生命周期门控（scorer/action 双侧门控 + 9 条 targeted 测试落地）。
- `72613e0e`（2026-07-09）：收口 NPC 防御生命周期 review 意见。

### 测试结果

- 归档审计时未重跑，以 plan 内既有记录为准：`cargo test npc_defense` 与 `cargo test npc::brain` 在 P0 实施时已 green（见各 commit message），审计时通过 grep 复核 `lifecycle_blocks_combat_action` 调用点与 9 条测试函数签名均存在于 `server/src/npc/brain/{scorers_combat.rs,actions_combat.rs}`，确认代码与测试均已落地在 `origin/main`（`79c4e025` 已是 `origin/main` 祖先）。

### 跨仓库核验

- **server**：`Lifecycle` 组件门控统一收敛在 `npc_defense_scorer_system` 与 `npc_defense_action_system` 两处，与同文件既有 chase/melee/dash scorer、melee action 的生命周期口径一致。
- **client / agent**：本修复不改变 combat intent 协议、不新增 schema 字段，纯 server 内部决策链修复，无需跨端改动。

### 遗留 / 后续

- round8 聚合骨架里的 P0 modifier orphan、P1 距离衰减、P3 TSY 生命周期问题不在本 plan 范围，由其他独立 plan 处理。
