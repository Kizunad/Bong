# plan-bughunt-heiwushi-dormant-identity-loss-v1

> Skeleton Plan（BugHunt persistence 20260708 r01）。仅记录真实 bug 与修复计划，不做实际修复。

## Bug 摘要

黑武士是特殊 BOSS，但它的 ECS 身份同时被标成通用 `NpcArchetype::Beast`。当黑武士满足 NPC virtualization 的通用脱水条件并进入 `NpcDormantStore` 后，快照只保留通用 NPC/Beast 字段；再次靠近玩家水化时，`NpcArchetype::Beast` 分支固定调用 `spawn_beast_npc_at`，不会恢复 `HeiwushiMarker`、`HeiwushiState`、`FaunaTag::Heiwushi`、`FaunaVisualKind::Heiwushi` 或 `heiwushi_thinker()`。

结果是：黑武士一旦经历 dormant 往返，就会被重建成普通野兽，丢失 BOSS 身份、专属 AI、视听触发和黑武士掉落语义。

边界：本 plan 不把 `HeiwushiSpawnState` 的自然刷新冷却重启重计作为新 bug。`docs/finished_plans/plan-sword-path-v3.md:181-182` 已把它列为可接受遗留。本 bug 只针对“已进入 `NpcDormantStore` 的黑武士水化后身份丢失”。

## 实际游玩体验影响

玩家在巨剑海附近触发黑武士后，如果离开到足够远、切服或长时间不在同一区域，使 NPC 进入 dormant，再回来时看到的可能不再是黑武士 BOSS，而是普通兽类行为实体。它不会走黑武士三相位 AI、不会触发 `heiwushi_*` VFX/SFX/动画事件，击杀后也不再保证 `star_iron`、`sword_embryo_shard` 等黑武士奖励。

玩家视角是“BOSS 被刷新/卸载后变成普通怪，专属战斗和奖励凭空消失”。这不是纯 HUD 问题，而是服务端权威身份在 state store 往返中被洗掉。

## 复现路径

1. 在 `giant_sword_sea` 通过自然刷新或 dev summon 生成黑武士，实体带 `HeiwushiMarker` / `HeiwushiState` / `FaunaTag::Heiwushi`。
2. 玩家离开到 NPC virtualization 脱水半径外，且不在该 zone，触发 `dehydrate_far_npcs_system` 把该 `NpcMarker` 写入 `NpcDormantStore`。
3. dormant snapshot 只保存 `NpcArchetype::Beast` 与通用字段，不保存黑武士专属组件。
4. 玩家再次靠近，`hydrate_dormant_near_players_system` 调 `spawn_from_snapshot`。
5. `NpcArchetype::Beast` 分支调用普通 `spawn_beast_npc_at`，按 zone/seed 派生普通 `FaunaTag`，然后只 tail-insert 通用组件。
6. 水化后的实体没有 `HeiwushiMarker` / `HeiwushiState`，也没有黑武士 thinker、视觉身份和掉落表。

## 根因证据

- `server/src/npc/heiwushi.rs:380-403`：黑武士专用 spawn 才插入 `HeiwushiMarker`、`HeiwushiState`、`FaunaTag::new(BeastKind::Heiwushi)`、`FaunaVisualKind::Heiwushi` 和 `heiwushi_thinker()`；同一实体也被标成 `NpcArchetype::Beast`。
- `server/src/npc/hydrate/mod.rs:299-315`：脱水查询是通用 `With<NpcMarker>, Without<Despawned>`，没有排除黑武士。
- `server/src/npc/hydrate/mod.rs:360-376`：脱水跳过条件只有渡劫、距离、玩家所在 zone；黑武士无专属保护。
- `server/src/npc/hydrate/mod.rs:388-475`：构造 `NpcDormantSnapshot` 时只写通用字段、loot、guardian/TSY sentinel 等，没有读取 `HeiwushiMarker` / `HeiwushiState` / `FaunaTag`。
- `server/src/npc/dormant/mod.rs:299-344`：`NpcDormantSnapshot` 没有黑武士字段，也没有 `BeastKind` / `FaunaVisualKind` / thinker / boss state 载荷。
- `server/src/npc/hydrate/mod.rs:721-750`：水化 `NpcArchetype::Beast` 固定调用 `spawn_beast_npc_at`。
- `server/src/npc/hydrate/mod.rs:892-930`：水化后的 tail insert 只恢复通用组件、loot、patrol 等，不补黑武士专属组件。
- `server/src/npc/spawn/beast.rs:52-70`：普通 Beast spawn 通过 `fauna_tag_for_beast_spawn` 派生兽类。
- `server/src/fauna/components.rs:146-283` 与 `server/src/fauna/components.rs:301-340`：普通 spawn pool/zone 分支不包含 `BeastKind::Heiwushi`。
- `server/src/npc/spawn/beast.rs:104-150`：普通 Beast 使用 brawler loadout 与 `beast_npc_thinker()`，不是黑武士剑系 loadout/thinker。
- `server/src/fauna/drop.rs:214-219` 与 `server/src/fauna/drop.rs:243-258`：黑武士专属掉落表只挂在 `BeastKind::Heiwushi`。

## 修复计划骨架

- [ ] 在 dormant 快照层新增黑武士专属身份载荷，例如 `heiwushi: Option<DormantHeiwushiSnapshot>`，至少保存 `HeiwushiState`、当前 `FaunaTag`/视觉身份、必要 cooldown/phase/growth 字段；或显式把黑武士排除出通用脱水，二者择一并写明取舍。
- [ ] `dehydrate_far_npcs_system` 增加 `HeiwushiMarker` / `HeiwushiState` / `FaunaTag` 查询，黑武士进入 dormant 时写专属 snapshot，避免只剩 `NpcArchetype::Beast`。
- [ ] `spawn_from_snapshot` 在 `NpcArchetype::Beast` 分支先检查黑武士载荷；有载荷时必须走 `spawn_heiwushi_at` 或等价专用 hydrate helper，再恢复 `HeiwushiState` 和黑武士视觉/AI组件。
- [ ] 确保普通 Beast 仍走原路径，不把所有 Beast 都改成黑武士，也不依赖 `home_zone == giant_sword_sea` 的脆弱推断。
- [ ] 对旧快照做非破坏迁移：缺少 `heiwushi` 字段的旧 Beast snapshot 继续按普通 Beast 恢复。
- [ ] 如果选择“黑武士禁止脱水”，需验证远离玩家后不被 `NpcDormantStore` 接管，且不会造成无限实体堆积；建议优先选择专属 snapshot，复用 TSY sentinel 的身份往返模式。

## 验证计划

- [ ] server 单测：构造带 `HeiwushiMarker` / `HeiwushiState` / `FaunaTag::Heiwushi` 的 NPC，触发 `dehydrate_far_npcs_system`，断言 snapshot 含黑武士载荷。
- [ ] server 单测：把黑武士 snapshot 灌入 `NpcDormantStore`，触发 hydrate，断言实体恢复 `HeiwushiMarker`、`HeiwushiState`、`FaunaTag::Heiwushi`、`FaunaVisualKind::Heiwushi` 和黑武士 thinker。
- [ ] server 单测：普通 Beast snapshot 仍恢复为普通 Beast，不误挂 `HeiwushiMarker`。
- [ ] 掉落回归：水化后的黑武士死亡仍走 `HEIWUSHI_DROPS`，保底 `star_iron` / `sword_embryo_shard`。
- [ ] 视听回归：水化后的黑武士发招仍 emit `heiwushi_*` VFX/SFX/动画事件。
- [ ] 跑 server 栈命令：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 去重

- 不重复 #1044：该 PR 处理 `workbench_item`、`trade_crate`、`herb_crate_placed`、`dead_drop_box` 等可放置纯 entity 重启丢失；本 bug 是 NPC dormant 快照中的黑武士身份往返丢失。
- 不重复 #1104：该 PR 处理 `ActiveEventsResource.active_events` 中长时世界事件运行态重启丢失；本 bug 不涉及 world event 队列。
- 不重复 `docs/finished_plans/plan-sword-path-v3.md`：该 plan 已实现黑武士 AI/视听/自然刷新，并承认自然刷新冷却非严格持久化；但没有覆盖黑武士进入 `NpcDormantStore` 后 hydrate 成普通 Beast 的身份 roundtrip 缺口。
- 同型先例：TSY sentinel 曾为 `TsySentinelMarker` 增加专属 snapshot 与 hydrate 路由，说明“特殊 NPC marker 被通用 archetype 水化洗掉”是需要显式处理的 persistence 问题。

## 对抗结论

- 第一轮反方裁决：`NEEDS_NARROWING`。候选中“普通 server 重启恢复成普通 Beast”证据不足；自然刷新冷却重启重计已是 sword-path v3 的可接受遗留。收窄为“黑武士进入 `NpcDormantStore` 后水化为普通 Beast”。
- 主 agent 回应：接受收窄，移除自然刷新冷却作为主张，只保留 dormant 往返身份丢失。
- 第二轮反方最终裁决：`ACCEPT`。前提限定为黑武士已经满足通用脱水条件并进入 `NpcDormantStore`；在此前提下，现有 hydrate 路径必然按普通 `NpcArchetype::Beast` 重建，无法恢复黑武士专属身份、AI、视听和掉落语义。

## 风险

- 直接按 zone 名推断黑武士会误伤普通巨剑海兽类；必须持久化专属身份或显式禁止脱水。
- 盲目恢复完整 `HeiwushiState` 可能带来 cooldown/phase 与当前血量不一致；实现时需定义状态与 `Wounds` 的同步顺序。
- 黑武士仍涉及真元/掉落/死亡链路，测试需要确认不引入新的守恒缺口；本 plan 只要求身份往返，不改真元物理。
