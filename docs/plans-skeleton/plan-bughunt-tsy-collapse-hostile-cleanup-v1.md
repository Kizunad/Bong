# plan-bughunt-tsy-collapse-hostile-cleanup-v1（骨架）

> **GOAL**：修复 r8 #10 的 TSY 塌缩 hostile ghost：collapse cleanup 不得因只处理 `NpcArchetype::Daoxiang` 而遗留带当前 `TsyHostileMarker.family_id` 的 Zhinian/Fuya/SkullFiend/GuardianRelic（由 `TsySentinelMarker` 标识的 TSY Sentinel）；塌缩完成后同一 family 的 hostile entity 必须按既定 ejection/despawn 结果离开 TSY，不能继续以 Dead family 运行。
>
> **Canonical owner**：`docs/finished_plans/plan-bughunt-r8-findings-v1.md:61-75` Finding Mapping #10。当前 `origin/main` 仍可复现；该 finding 是独立 domain fix，不与已修复的 #11 collapse death presence 或 #12 Fuya audio 混合。
>
> **Delivery**：按根 `CLAUDE.md` BugFix 工作流，一个 skeleton = 一个修复 subagent = 一个常驻 slot = 一个 PR；不由 `/consume-plan` 消费。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | collapse hostile family ownership 与完整清理 | ⬜ |
| P1 | 真实 world production schedule 回归与 server gate | ⬜ |

## 接入面

- **进料 / lifecycle owner**：`server/src/world/tsy_lifecycle.rs:548-702` 的 `tsy_collapse_completed_cleanup` 消费 `TsyCollapseCompleted`，并以 `TsyZoneStateRegistry.by_family[family_id]` 将 family 标记 `Dead`。
- **hostile producer**：`server/src/npc/tsy_hostile.rs:82-84` 的 `TsyHostileMarker { family_id }`；`spawn_tsy_hostiles_for_family` 及 `spawn_tsy_*_at` 在 `:564-807,897-1083` 为 Daoxiang/Zhinian/Fuya/SkullFiend/GuardianRelic(TSY Sentinel) 挂 marker；Sentinel 具体 `TsySentinelMarker` 为 `:87-92,1082-1099`。
- **生产调度**：`server/src/world/tsy_lifecycle.rs:953-970` 注册 lifecycle systems；`server/src/world/mod.rs:125` 是 world registration 入口，`server/src/main.rs` 的 production App 通过 `world::register` 接入。P1 必须从该入口构建 schedule，不得手拼孤立 cleanup system。
- **共享契约**：复用 `TsyCollapseCompleted`、`TsyZoneStateRegistry`、`TsyHostileMarker`、`DimensionTransferRequest` 与 Valence `Despawned`；不另造 TSY family 状态机，不修改 NPC schema/IPC。
- **worldview / qi_physics**：保持既有 collapse 后 50% deterministic ejection / 50% `Despawned` 语义；不改变 hostile drain、qi 数值或 `qi_physics` 公式。

## 第一性验真

- `tsy_collapse_completed_cleanup` 的 4b 循环在 `:665-685` 先 `if !matches!(archetype, NpcArchetype::Daoxiang) { continue; }`，所以已存在的 Zhinian/Fuya/SkullFiend/GuardianRelic(TSY Sentinel) 即使处于当前 collapse AABB 也不进入 ejection/despawn 分支。
- 这些变体在 spawn 时都携带 `TsyHostileMarker.family_id`；marker writer 仍在 `server/src/npc/tsy_hostile.rs`，但 cleanup 没有按 marker/family 查询。
- cleanup 随后在 `:688-701` 移除 family subzones 并标记 `TsyLifecycle::Dead`；ghost entity 因而继续被 NPC systems tick，且 family_id 指向已 Dead family。
- `TsyHostileMarker` 不是持久化 identity；本 finding 只要求 live ECS entity 按稳定 `family_id` 被处理，不扩展到 restart/durability。

## P0 — Complete hostile-family cleanup

- [ ] cleanup 的唯一 owner key 是 accepted `TsyCollapseCompleted.family_id` 与 live `TsyHostileMarker.family_id` 的精确相等；`TsySentinelMarker.family_id` 必须与同实体的 `TsyHostileMarker.family_id` 对拍，不得以 archetype 白名单代替 marker/family 对拍。只有当前 family 的 marker entity 进入清理，其他 family 与无 marker NPC 不受影响。
- [ ] 把现有 Daoxiang 的 deterministic decision 保持为 family cleanup 的统一行为：每个匹配 marker entity 只能走一次 ejection 或 `Despawned` 分支；ejection 使用现有 `DimensionTransferRequest` 和 `main_world_anchor`，despawn 使用 `Despawned`，不得裸 `.despawn()` 破坏 Valence entity layer。
- [ ] 完整清理覆盖 `NpcArchetype::{Daoxiang,Zhinian,Fuya,SkullFiend,GuardianRelic}` 中带 `TsyHostileMarker` 的 TSY Sentinel，并在 Commands deferred apply 后不得残留任一匹配 `TsyHostileMarker`；不得清理其他 family、TSY player、corpse 或无 marker entity。
- [ ] family cleanup 必须保持既有 ZoneRegistry subzone removal、`TsyLifecycle::Dead` transition、ejection/despawn side effects；不重复发送 transfer/despawn，不改变 qi ledger 或 hostile drain 语义。cleanup 是本 finding 唯一允许改变匹配 hostile entity 生命周期的 call site。

## P1 — Regression closure

- [ ] 回归 target 固定为 `server/src/world` 的现有 `#[cfg(test)]` world test module / server test target；测试通过 production `world::register(&mut app)`（`server/src/world/mod.rs:125`，由 production App 的 `main.rs` 注册链调用）构建 schedule，不得手工只 add `tsy_collapse_completed_cleanup`。
- [ ] `collapse_cleanup_removes_all_hostile_archetypes_for_family`
- [ ] `collapse_cleanup_ejects_or_despawns_each_matching_family_entity_once`
- [ ] `collapse_cleanup_preserves_other_family_and_unmarked_npcs`
- [ ] `collapse_cleanup_marks_dead_and_removes_subzones_after_deferred_commands`
- [ ] `collapse_cleanup_does_not_add_qi_transfer_or_drain_side_effects`
- [ ] 回归必须断言 production registration order：collapse event producer / lifecycle tick → `tsy_collapse_completed_cleanup` → deferred Commands apply → downstream NPC queries；按每个状态转换逐帧 `App::update()`，在同帧与下一帧分别断言 marker entity 的 transfer/despawn、family Dead、subzone removal 及无 ghost scorer/action 输入。

## 可核验 symbols

- `TsyHostileMarker`、`TsySentinelMarker`、`TsyCollapseCompleted`、`tsy_collapse_completed_cleanup`
- `spawn_tsy_hostiles_for_family`、`spawn_tsy_daoxiang_at`、`spawn_tsy_zhinian_at`、`spawn_tsy_fuya_at`、`spawn_tsy_skull_fiend_at`、`spawn_tsy_sentinel_at`
- `TsyZoneStateRegistry`、`TsyLifecycle::Dead`、`DimensionTransferRequest`
- `world::register`、`main.rs` production App registration

## 非本 plan 交付物

以下邻接风险不属于 PR #1304 Mapping #10，不得在本 plan 实现 PR 顺手扩大：

- collapse death 后 `TsyPresence`、revive/drop lifecycle（r8 #11 已由 PR #1139 修复并归档）。
- Fuya stop-audio recipient 门控（r8 #12 已由 PR #1139 修复并归档）。
- hostile AI 的一般 lifecycle gate、family reload、dormancy/rehydration 与 cross-process persistence。
- `TsyHostileMarker` 的跨进程 durability、migration 或 restart reconciliation。
- `scripts/build-token.sh` 的创建及 V 轨交付；当前 origin/main 尚无该脚本。

## 验收与安全边界

- Server gate：若实现时 `scripts/build-token.sh` 已由 V 轨合入，按真实 CLI 运行；否则使用 `flock /tmp/bong-cargo.lock -c 'cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test'`。
- 严禁本地运行 `scripts/test-tmux-shutdown-order.sh`、`scripts/test-server-lifecycle.sh` 或任何调用它们的 suite；GitHub e2e 保留该覆盖。
- 本次只 push，不触发 `/review`；待上游 relay 恢复后由调度方批量收集 verdict。push 前必须 `git fetch origin && git merge origin/main`，并以 fresh-context exact-HEAD validator 通过为前置。
- P0/P1 全部完成后补 `## Finish Evidence` 并归档；实施与归档保持唯一 BugFix PR。
