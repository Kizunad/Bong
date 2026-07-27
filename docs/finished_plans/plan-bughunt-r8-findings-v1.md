# plan-bughunt-r8-findings-v1（已归档）

> 一句话主题：round8 十一条 finding 已按 `origin/main @ c625d5a5` 拆散；五条有 merged 修复，modifier、距离衰减与 TSY hostile 三组 live 缺口各有唯一 successor。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| T0 | 十一条 finding current-code + ancestor commit/PR 复核 | ✅ 2026-07-28 |
| T1 | 三个 independent owner 去重登记 | ✅ 2026-07-28 |
| T2 | mapping table、Finish Evidence、归档 | ✅ 2026-07-28 |

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| #1 scar circuit reach/regen/purge | `player_attack.rs`、`cultivation/tick.rs:240-270`、`contamination.rs` 当前均有 runtime consumer | `already-fixed/invalid`（already-fixed） | `3e6981513` / PR #1143 | 仅归档 |
| #2 iron cocoon wound fields | `server/src/combat/baomai_v4/iron_cocoon.rs:110-139` 写入；伤口 resolve 不读 | `independent-domain-fix` | `plan-bughunt-modifier-effect-consumer-completion-v1.md` P2 | 统一 successor |
| #3 scar-forged flow | `iron_cocoon.rs:140` 写 flag；`MeridianSystem::sum_rate` 不读 | `independent-domain-fix` | 同上 P2 | effective-rate 设计门 |
| #5 Insight benefit/cost cluster | `server/src/cultivation/insight_apply.rs:24-254` 多字段仍无 production consumer | `independent-domain-fix` | 同上 P3 | 统一 successor |
| #4 `jump_height_multiplier` | `server/src/combat/status.rs:174` reset、body conditioning 写入；server/client 下游无 jump consumer | `independent-domain-fix` | 同上 P4 | 统一 successor |
| #6 distance decay calibration | `server/src/qi_physics/constants.rs:4` 仍 `0.03`；`combat/decay.rs:48,52` 仍锁 `0.737/0.494`，偏离 0.40/0.80 锚 | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-distance-decay-calibration-v1.md` | 新建唯一数值校准 owner |
| #7 NPC defense scorer lifecycle | `server/src/npc/brain/scorers_combat.rs:348-389` 当前在评分前按 `LifecycleState::Alive` gate | `already-fixed/invalid`（already-fixed） | `79c4e0258` / PR #1136 | 仅归档 |
| #8 NPC defense action lifecycle | `server/src/npc/brain/actions_combat.rs:447-448,468-500` 对 Requested/Executing 两态均应用 lifecycle gate | `already-fixed/invalid`（already-fixed） | 同 commit/PR #1136 | 仅归档 |
| #11 collapse death `TsyPresence` | `server/src/world/extract_system.rs:775-779` 插 `PendingTsyDeathDrop` 并移除 presence | `already-fixed/invalid`（already-fixed） | `5b477b453` / PR #1139 | 仅归档 |
| #10 TSY hostile ghost | `server/src/world/tsy_lifecycle.rs:667` 仍 skip 非 Daoxiang；hostiles 挂 `TsyHostileMarker.family_id` 却未 family-cleanup | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-tsy-collapse-hostile-cleanup-v1.md` | 新建唯一 focused owner |
| #12 Fuya death stop-audio 广播 | `server/src/npc/tsy_hostile.rs:1135-1153` 当前 stop query 以 `With<FuyaAura>` 对称门控；`:2414-2528` 覆盖非 aura/aura-family recipient | `already-fixed/invalid`（already-fixed） | `5b477b453` / PR #1139 | 仅归档 |

## Finish Evidence

- **落地清单**：modifier 四条归统一 successor；距离与 TSY hostile 各新建 focused skeleton；五条 merged 修复结案；bundle 迁入本路径。
- **关键 commit / PR**：`3e6981513`/#1143、`79c4e0258`/#1136、`5b477b453`/#1139 均为目标 HEAD 祖先且当前修复保留。
- **测试结果**：docs-only triage；最终 docs static gate + exact-HEAD validator。
- **跨仓库核验**：modifier/jump 的跨 server/schema/client 未实现部分已收进 successor；NPC/TSY 当前 finding 为 server lifecycle。
- **遗留 / 后续**：仅三个 canonical successor；本 bundle 禁止再消费。
