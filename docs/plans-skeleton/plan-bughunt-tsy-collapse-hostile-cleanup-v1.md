# plan-bughunt-tsy-collapse-hostile-cleanup-v1（骨架）

> 一句话主题：TSY family 塌缩完成时按 `TsyHostileMarker.family_id` 清理全部 hostile 变体，消除 family 已 Dead 但 NPC 仍 tick 的 ghost entity。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 拍板 hostile 终态：despawn、喷出或转移 | ⬜ |
| P1 | family-scoped cleanup + Valence 安全 despawn | ⬜ |
| P2 | 全变体/跨 family/重复 collapse 矩阵 | ⬜ |
| P3 | server gate + TSY bot 回归 | ⬜ |

## 接入面

- **进料**：`server/src/world/tsy_lifecycle.rs::tsy_collapse_completed_cleanup`、`TsyHostileMarker`、`TsyFamilyId`、Zhinian/Fuya/SkullFiend/Sentinel/Daoxiang spawn。
- **出料**：塌缩 family 的实体终态、音频/VFX 清理与 layer projection；其他 family 不受影响。
- **共享类型 / event**：复用 `TsyHostileMarker` 与 `Despawned`；禁止仅按 `NpcArchetype` 猜 family。
- **跨仓库契约**：若选择可见喷出/转移，沿既有 entity/audio payload；不新增 vanilla entity hack。
- **worldview 锚点**：坍缩渊塌缩后该 family 生命周期终结；hostile 不能脱离已死亡 family 继续存在。
- **qi_physics 锚点**：zone 移除后 drain 已跳过；本 plan 不另做 qi 修复，也不得在清理时吞/铸造 NPC 私池 qi。

## 当前证据（origin/main @ c625d5a5）

`server/src/world/tsy_lifecycle.rs:667` 的 cleanup 仍显式 `continue` 所有非 `NpcArchetype::Daoxiang`。Zhinian/Fuya/SkullFiend/Sentinel 在 `server/src/npc/tsy_hostile.rs:922,981,1021,1091` 挂 `TsyHostileMarker { family_id }`，但 collapse cleanup 未按 marker/family 查询它们。`TsyPresence` 与 Fuya stop-audio 已由 PR #1139 修复，不属本 plan。

## 验收

1. 每个 hostile 变体单独覆盖；同 family 全清，邻近不同 family 保留。
2. 重复 collapse 幂等；已 `Despawned`/死亡实体不触发 panic或重复掉落。
3. 客户端可见 Valence entity 使用 `insert(Despawned)` 并让后续 query 排除，禁止裸 `.despawn()`。
4. 断言 AI scorer/action、Fuya hum、drain、掉落不会在终态后继续；qi 守恒按既有路径。
5. 完整 server gate + TSY collapse bot 场景。

## 边界

- 不重新修 `TsyPresence` 或 Fuya stop-audio。
- 不改变 collapse 触发条件、family 状态机、zone qi redistribution 或 hostile 战斗数值。
