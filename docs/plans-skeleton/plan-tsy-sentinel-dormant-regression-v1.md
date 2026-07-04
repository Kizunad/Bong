# plan-tsy-sentinel-dormant-regression-v1（骨架）

> **骨架（草案）**。一句话主题：TSY 守灵 `TsySentinelMarker` 没有穿过 dormant → hydrate 链路，玩家离开秘境或拉远后再回来，会把本该是秘境守灵的 boss 洗成普通 `GuardianRelic` 守卫，直接丢失 phase/Boss HUD/专属掉落/外观。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | 复现链路 + 回归 pin：sentinel 脱水/回水后仍保持守灵身份 | fix_pr | ⬜ |
| P1 | dormant snapshot 补齐 sentinel 身份载荷（不再只记 `TsyHostileMarker`） | fix_pr | ⬜ |
| P2 | hydrate 按 sentinel 专属 spawn 路径重建 AI/外观/HUD/掉落语义 | fix_pr | ⬜ |
| P3 | 玩家可见回归：离开 TSY/回头后二次遭遇仍是同一类 boss | fix_pr | ⬜ |

## 接入面

- **进料**：`server/src/npc/tsy_hostile.rs` 的 `spawn_tsy_sentinel_at`（守灵真实语义来源） + `server/src/npc/hydrate/mod.rs` 的 `dehydrate_far_npcs_system` / `dormant_tsy_hostile_snapshot` / `spawn_from_snapshot`
- **出料**：sentinel rehydrate 后仍保留 `TsySentinelMarker`、sentinel thinker、`FaunaVisualKind::TsySentinel`、boss 血条 payload、`tsy_sentinel` 掉落键、phase 事件
- **共享类型 / event**：`TsySentinelMarker`、`TsyHostileMarker`、`DormantTsyHostileSnapshot`、`DormantGuardianRelicSnapshot`、`TsySentinelPhaseChanged`、`TsyBossHealthS2c`
- **跨模块核验**：`network/tsy_polish.rs` 的 boss HUD 只看 `TsySentinelMarker`；`npc/tsy_hostile.rs` 的相位 scorer/action、掉落键分流也只认 `TsySentinelMarker`
- **worldview / 既有 plan 锚点**：`docs/finished_plans/plan-tsy-hostile-v1.md` §1.2 明确规定“spawn 时：`NpcArchetype::GuardianRelic` + `TsySentinelMarker`；有 marker 走 TSY 守灵分支，无 marker 走 overworld 护主分支”

## P0 — 复现链路 + pin 测试

- [ ] 复现并锁定当前坏链路：
  - `spawn_tsy_sentinel_at` 生成 sentinel 时同时挂 `NpcArchetype::GuardianRelic` + `FaunaVisualKind::TsySentinel` + `TsySentinelMarker { guarding_container, phase, max_phase }` + `sentinel_thinker()`（`server/src/npc/tsy_hostile.rs:1030-1087`）
  - `dehydrate_far_npcs_system` 会处理所有 `With<NpcMarker>` 的远离 NPC，没有把 sentinel 排除在 dormant 之外（`server/src/npc/hydrate/mod.rs:278-458`）
  - snapshot 仅记录 `TsyHostileMarker` 的 family，以及 zhinian/fuya/daoxiang 分支数据；**完全不记录** `TsySentinelMarker`（`server/src/npc/hydrate/mod.rs:485-507`）
  - hydrate 时 `NpcArchetype::GuardianRelic` **无条件**走 `spawn_relic_guard_npc_at`（`server/src/npc/hydrate/mod.rs:641-656`），随后只补回 `TsyHostileMarker`，不会补回 `TsySentinelMarker`（`server/src/npc/hydrate/mod.rs:764-789`）
  - `spawn_relic_guard_npc_at` 本体是普通 overworld relic guard：`EntityKind::VILLAGER` + `GuardianDuty`/`TrialEval` + `relic_guard_thinker()`（`server/src/npc/spawn/disciple.rs:167-236`）
- [ ] 新增回归测试至少覆盖：
  - `tsy_sentinel_dehydrates_with_sentinel_identity_payload`
  - `hydrated_tsy_sentinel_uses_spawn_tsy_sentinel_path_not_spawn_relic_guard`
  - `rehydrated_tsy_sentinel_keeps_marker_visual_and_phase_state`

## P1 — snapshot 补齐 sentinel 身份载荷

- [ ] 扩 `DormantTsyHostileSnapshot` 或新增 sentinel 专属 snapshot，至少保存：
  - `guarding_container` 的**稳定重绑键**（不能继续依赖裸 `Entity`）
  - `phase`
  - `max_phase`
  - 需要的话补充 sentinel 专属外观/掉落分流所需字段
- [ ] `DormantExtraComponentQueries` 补 `TsySentinelMarker` 读取；dehydrate 时把 sentinel 专属字段写进 snapshot
- [ ] 明确“普通 `GuardianRelic`”与“TSY sentinel”在 dormant 载荷层的判别位，避免二者继续共用 `NpcArchetype::GuardianRelic` 后再被洗平

## P2 — hydrate 重建 sentinel 专属语义

- [ ] `spawn_from_snapshot` 对“`GuardianRelic` + sentinel payload”走 `spawn_tsy_sentinel_at`，而不是 `spawn_relic_guard_npc_at`
- [ ] hydrate 后重新接好：
  - `TsySentinelMarker`
  - `FaunaVisualKind::TsySentinel`
  - `sentinel_thinker()`
  - 守护容器绑定
  - phase/max_phase
- [ ] 确认不会误把普通 overworld `GuardianRelic` 提升成 TSY sentinel

## P3 — 玩家可见回归

- [ ] 走玩家真实链路验收：
  - 玩家进入 TSY 深层，见到 sentinel
  - 玩家离开 TSY 或拉远到 dehydrate 条件成立
  - 玩家返回后，原 sentinel 仍显示为秘境守灵，不变成 villager 风格 relic guard
  - boss 血条继续出现（`network/tsy_polish.rs:85-127` 只查询 `TsySentinelMarker`）
  - phase 继续推进（`npc/tsy_hostile.rs:1548-1616`）
  - 死亡掉落仍走 `tsy_sentinel` 分支（`npc/tsy_hostile.rs:1733-1835`）

## 玩家影响

- 玩家正常游玩可达：进入 TSY 深层摸完 relic core、撤出秘境、稍后回来二刷，就是最自然的触发方式
- 当前结果不是“后台状态少一位”这种无感 bug，而是**直接把 boss 洗成另一种 NPC**
- 具体体感退化：
  - 守灵外观/实体种类错
  - boss 血条消失
  - sentinel 三段 phase 技能链失效
  - 专属掉落分流失效
  - 设计上“守灵看守 relic core”的身份被破坏

## 开放问题

1. `guarding_container` 跨 dormant 往返要用什么稳定键重绑：容器实体 `Entity` 不能直接进长期 snapshot，需拍定逻辑 ID / source key / family 内索引
2. sentinel 的 `phase` 是否应精确保留，还是允许 hydrate 后重置到 phase 0；若允许重置，需要确认是否与 boss HUD / 掉落 / 伤害档位设计冲突
3. 是否顺手为“复用 `NpcArchetype::GuardianRelic` 的 TSY/overworld 双身份”加一条统一不变量测试，避免未来再发生 marker 丢失后语义洗平
