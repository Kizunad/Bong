# plan-bughunt-tsy-collapse-hostile-cleanup-v1（骨架）

> 一句话主题：TSY family 塌缩完成时按 `TsyHostileMarker.family_id` 清理当前被漏掉的 Zhinian/Fuya/SkullFiend/Sentinel 四类 non-Daoxiang hostile，消除 family 已 Dead 但 NPC 仍 tick 的 ghost entity；既有 Daoxiang 50% 喷出/50% despawn 分支不属本 finding。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 冻结唯一终态：同 family 四类 non-Daoxiang hostile 安全 despawn（不喷出、不转移）及 qi/掉落/audio/VFX 规则 | ⬜ |
| P1 | family-scoped cleanup + qi fail-closed release + Valence 安全 despawn | ⬜ |
| P2 | 全变体/跨 family/重复 collapse 矩阵 | ⬜ |
| P3 | server gate + TSY bot 回归 | ⬜ |

## 接入面

- **进料**：`server/src/world/tsy_lifecycle.rs::tsy_collapse_completed_cleanup`、`TsyHostileMarker`、`TsyFamilyId`、Zhinian/Fuya/SkullFiend/Sentinel spawn；Daoxiang 仅作为既有独立 collapse 分支的回归隔离对象。
- **出料**：塌缩 family 的实体终态、音频/VFX 清理与 layer projection；其他 family 不受影响。
- **共享类型 / event**：复用 `TsyHostileMarker` 与 `Despawned`；禁止仅按 `NpcArchetype` 猜 family。
- **跨仓库契约**：四类 non-Daoxiang hostile 的终态固定为 server-authoritative despawn；不新增它们的喷出/转移 payload，不新增 vanilla entity hack。既有 Daoxiang 50% `DimensionTransferRequest` / 50% `Despawned` 是独立正典分支，本 plan 不改其概率、转移或反馈。
- **worldview 锚点**：坍缩渊塌缩后该 family 生命周期终结；hostile 不能脱离已死亡 family 继续存在。
- **qi_physics 锚点**：zone 移除后 drain 已跳过；本 plan 不另做 qi 修复，也不得在清理时吞/铸造 NPC 私池 qi。

## 当前证据（origin/main @ c625d5a5）

`server/src/world/tsy_lifecycle.rs:667` 的 cleanup 仍显式 `continue` 所有非 `NpcArchetype::Daoxiang`。Zhinian/Fuya/SkullFiend/Sentinel 在 `server/src/npc/tsy_hostile.rs:922,981,1021,1091` 挂 `TsyHostileMarker { family_id }`，但 collapse cleanup 未按 marker/family 查询它们。`TsyPresence` 与 Fuya stop-audio 已由 PR #1139 修复，不属本 plan。

## P0 冻结终态：family-scoped safe despawn

1. **唯一终态（仅 finding 命中的四类）**：收到 `TsyCollapseCompleted` 后，`TsyHostileMarker.family_id` 等于该 family 且 archetype 为 Zhinian/Fuya/SkullFiend/Sentinel 的实体一律安全 despawn；它们不得进入 Daoxiang 的 50% ejection roll，不喷出主世界、不迁到其他 family，也不留下 detached hostile。Daoxiang 即使同样携 `TsyHostileMarker`，仍由既有 Daoxiang/corpse 分支独立处理，必须从新 four-variant query/分支中显式排除。
2. **Valence / layer projection**：上述四类客户端可见实体必须 `commands.entity(entity).insert(Despawned)`，并让同 tick 及后续 AI scorer/action、drain、碰撞、掉落与 audio query 显式排除 `Despawned`；禁止裸 `.despawn()`。`Despawned` 是这四类 entity 与 layer projection 的唯一终态标志。
3. **qi fail-closed**：`npc_runtime_bundle` 当前以 `qi_current = 0.0` 生成这四类 hostile，但 cleanup 不得据此假设永久为零。插入 `Despawned` 前读取真实 qi；正余额必须消费 R5 冻结的 `release_dormant_qi_to_zone`/等价 ledger API 完整释放到明确 zone/overflow 并以 source 严格归零为成功条件。现有 lifecycle tick 会在发出 `TsyCollapseCompleted` 前把 family 标为 `Dead`，因此任一 entity 的 release/zone lookup 失败时必须原子恢复该 family 为 `Collapsing`、清除 `dead_at_tick` 并保留 entity/qi，供下一次可重试 cleanup；不得让状态停在 `Dead` 且留下 ghost。零余额走无 transfer 的快路径。
4. **掉落与 lifecycle 语义**：collapse cleanup 不是战斗击杀，不发送 `DeathEvent`，不生成普通死亡掉落、奖励或重复 collapse loot；已有死亡/`Despawned` 实体幂等跳过。测试必须证明 cleanup 不触发 death-only consumer。
5. **audio/VFX**：Fuya hum 等 loop 在插入 `Despawned` 的同一 cleanup 事务中显式发 stop（不能依赖未发送的 `DeathEvent`）；只保留既有 collapse/despawn 可见反馈，不新增 death VFX/SFX。stop 失败不得复活实体，但要有可观测日志/幂等重发策略。

## 验收

1. Zhinian/Fuya/SkullFiend/Sentinel 每个变体单独覆盖；同 family 四类全部 `Despawned`，邻近不同 family 保留，且这四类都不进入 Daoxiang ejection/transfer 路径。另有回归 fixture 证明同 family Daoxiang 仍按既有 50% transfer / 50% despawn 分支处理，不被 four-variant cleanup 重复消费。
2. 重复 collapse 幂等；已 `Despawned`/死亡实体不 panic、不重复 stop audio、不生成 death loot/reward/`DeathEvent`。
3. 客户端可见 Valence entity 使用 `insert(Despawned)`，并让 AI scorer/action、Fuya hum、drain、碰撞与掉落 query 在同 tick 起排除；禁止裸 `.despawn()`。
4. qi=0 时无 transfer；qi>0 时逐腿断言 source→zone/overflow 的 balance、reason 与 audit 后 source 严格归零再 despawn；zone lookup/release 失败时实体和 qi 原样保留，family 从事件发送前的 `Dead` 原子恢复为 `Collapsing`、清除 `dead_at_tick` 且 cleanup 可重试，守恒取 `SPIRIT_QI_TOTAL`/既有 R5 helper 而非字面量。
5. 断言 Fuya hum 显式 stop、layer projection 消失且无新增 death VFX/SFX；其他 family 的 entity/audio 不受影响。
6. 完整 server gate + TSY collapse bot 场景，bot 同时证明四类 non-Daoxiang hostile 不再可见/可攻击，且主世界没有被喷出的这四类实体；Daoxiang 的既有喷出分支单独回归，不把其主世界实体误判为失败。

## 边界

- 不重新修 `TsyPresence` 或 Fuya death stop-audio；collapse cleanup 仍须为 non-death despawn 显式 stop Fuya hum。
- 不改变 Daoxiang/corpse 的既有 50% 喷出/50% despawn 正典分支，不改变 collapse 触发条件、zone qi redistribution 或 hostile 战斗数值；唯一 lifecycle 调整是 qi release 失败时把提前写入的 `Dead` 原子回滚为可重试 `Collapsing`。
