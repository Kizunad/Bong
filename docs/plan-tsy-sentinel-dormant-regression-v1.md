# plan-tsy-sentinel-dormant-regression-v1

> 一句话主题：TSY 守灵 `TsySentinelMarker` 没有穿过 dormant → hydrate 链路，玩家离开秘境或拉远后再回来，会把本该是秘境守灵的 boss 洗成普通 `GuardianRelic` 守卫（overworld villager 风格），直接丢失 phase/Boss HUD/专属掉落/外观。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | 复现链路 + 回归 pin：sentinel 脱水/回水后仍保持守灵身份 | fix_pr | ⬜ 升 active 2026-07-04 |
| P1 | dormant snapshot 补齐 sentinel 身份载荷（不再只记 `TsyHostileMarker`） | fix_pr | ⬜ |
| P2 | hydrate 按 sentinel 专属 spawn 路径重建 AI/外观/HUD/掉落语义 | fix_pr | ⬜ |
| P3 | 玩家可见回归：离开 TSY/回头后二次遭遇仍是同一类 boss | fix_pr | ⬜ |

## 接入面

- **进料**：
  - `server/src/npc/tsy_hostile.rs::spawn_tsy_sentinel_at`（守灵真实语义来源：`NpcArchetype::GuardianRelic` + `FaunaVisualKind::TsySentinel` + `TsyHostileMarker` + `TsySentinelMarker` + `sentinel_thinker()`）
  - `server/src/npc/hydrate/mod.rs::dehydrate_far_npcs_system`（脱水扫描，`With<NpcMarker>` 全量 NPC，line 272-459）+ `dormant_tsy_hostile_snapshot`（line 485-507）+ `DormantExtraComponentQueries`（line 59-67，`SystemParam`）
  - `server/src/world/tsy_container.rs::LootContainer`（`guarding_container` 指向的容器实体，`Position` 已知且容器本身从不 dehydrate/despawn——见 §8.1 #2 决议核实结论）
- **出料**：
  - sentinel rehydrate 后仍保留 `TsySentinelMarker`、`FaunaVisualKind::TsySentinel`、sentinel thinker（`sentinel_aggro_scorer_system` / `update_sentinel_phase_system` / `sentinel_phase_action_system`）
  - `server/src/network/tsy_polish.rs::emit_tsy_boss_health_payloads`（boss 血条 payload，只查询 `TsySentinelMarker`，line 95-153）
  - `server/src/npc/tsy_hostile.rs::handle_npc_death_drop`（`tsy_sentinel` 掉落键，line 1739-1847）+ `TsySentinelPhaseChanged` phase 事件（line 1556-1581）
- **共享类型 / event**：`TsySentinelMarker`（`server/src/npc/tsy_hostile.rs:84-89`）、`TsyHostileMarker`、`DormantTsyHostileSnapshot`（`server/src/npc/dormant/mod.rs:231-241`）、`DormantGuardianRelicSnapshot`（`server/src/npc/dormant/mod.rs:199-207`，precedent：`alarm_center: [f64;3]` 已经是"存位置不存 Entity"的先例）、`TsySentinelPhaseChanged`、`TsyBossHealthS2c`
- **跨模块核验**：`network/tsy_polish.rs:90` 的 boss HUD 只看 `TsySentinelMarker`（不依赖 `guarding_container`——已核实，见 §8.1 #2）；`npc/tsy_hostile.rs:1842` 的 `drop_key_for_npc` 掉落键分流只认 `TsySentinelMarker` 是否存在；`npc/tsy_hostile.rs:1805-1811` 的 `guarding_container_kind`（仅影响 `__origin_keyed_key__` 模板的钥匙掉落，`guarding_container=None` 时该子分支静默跳过，不阻塞整体掉落）
- **worldview 锚点**：`docs/worldview.md` §十六「秘境：活坍缩渊」，§十六.五「秘境守灵必掉」（line 1507）——秘境守灵是秘境专属 boss 身份，其"必掉"承诺与外观/HUD 身份直接对应正典设定，身份洗平即违背 §十六.五
- **qi_physics 锚点**：不涉及。本 plan 是纯 ECS component/snapshot 往返 bug 修复（marker 丢失 → archetype 分流错误），不产生/消耗/转移真元，不引入衰减常数，`Wounds`（生命值）本就不在 dormant snapshot 范围内（现有系统性行为，见 §8.1 #2），本 plan 不改变这一点

## P0 — 复现链路 + pin 测试

- [ ] 复现并锁定当前坏链路（文档化，无需新代码，仅测试）：
  - `spawn_tsy_sentinel_at` 生成 sentinel 时同时挂 `NpcArchetype::GuardianRelic` + `FaunaVisualKind::TsySentinel` + `TsyHostileMarker{family_id}` + `TsySentinelMarker{family_id, guarding_container:Some(Entity), phase:0, max_phase:3}` + `sentinel_thinker()`（`server/src/npc/tsy_hostile.rs:1036-1096`）
  - `dehydrate_far_npcs_system` 处理所有 `With<NpcMarker>` 的远离 NPC，未把 sentinel 排除在 dormant 之外（`server/src/npc/hydrate/mod.rs:272-459`）
  - `dormant_tsy_hostile_snapshot` 只读 `TsyHostileMarker`/`ZhinianMind`/`FuyaAura`/`DaoxiangOrigin`，**完全不读** `TsySentinelMarker`（`server/src/npc/hydrate/mod.rs:485-507`，`DormantExtraComponentQueries` 缺字段：`server/src/npc/hydrate/mod.rs:59-67`）
  - hydrate 时 `NpcArchetype::GuardianRelic` **无条件**走 `spawn_relic_guard_npc_at`（`server/src/npc/hydrate/mod.rs:641-656`），随后只补回 `TsyHostileMarker`，不会补回 `TsySentinelMarker`（`server/src/npc/hydrate/mod.rs:764-791`）
  - `spawn_relic_guard_npc_at` 本体是普通 overworld relic guard：`EntityKind::VILLAGER` + `GuardianDuty`/`TrialEval` + `relic_guard_thinker()`（`server/src/npc/spawn/disciple.rs:167-236`）
- [ ] 新增回归测试，落在 `server/src/npc/hydrate/mod.rs` `#[cfg(test)] mod tests`（复用既有 `zone_registry()` / `snapshot()` / `App::new()` 测试 harness 模式，line 818+）：
  - `tsy_sentinel_dehydrates_with_sentinel_identity_payload`——构造带 `TsySentinelMarker` 的活体 sentinel entity，跑 `dehydrate_far_npcs_system`，断言 `NpcDormantStore` 里对应 snapshot 的 `tsy_sentinel`（P1 新字段，见下）字段 `Some`，且携带 `guarding_container_pos` / `phase` / `max_phase`
  - `hydrated_tsy_sentinel_uses_spawn_tsy_sentinel_path_not_spawn_relic_guard`——构造带 sentinel 载荷的 dormant snapshot，跑 `hydrate_dormant_near_players_system`，断言 hydrate 后实体带 `TsySentinelMarker` + `FaunaVisualKind::TsySentinel`（**不是** `GuardianDuty`+`TrialEval`+villager `EntityKind`）
  - `rehydrated_tsy_sentinel_keeps_marker_visual_and_phase_state`——同上，额外断言 `max_phase` 精确回填、`guarding_container` 在容器仍存在时重绑成功指向正确 entity
  - `guardian_relic_dual_identity_invariant_partitioned_by_sentinel_marker`（§8.1 #3 决议新增）——覆盖 `NpcArchetype::GuardianRelic` 的两条分支互斥：有 sentinel 载荷 → 必须产出 `TsySentinelMarker` 且不产出 `GuardianDuty`/`TrialEval`；无 sentinel 载荷 → 必须产出 `GuardianDuty`/`TrialEval` 且不产出 `TsySentinelMarker`，杜绝未来"两者都长出来"或"两者都没有"的漂移
  - `hydrated_tsy_sentinel_container_rebind_ignores_same_position_different_family`（§8.1 #1 补漏新增，博弈 blocker 直接锁定项）——构造两个 `LootContainer`：一个 `family_id="tsy_lingxu_01"`（sentinel 真正所属），一个 `family_id="spawn_tutorial"`（模拟 `spawn_tutorial.rs:497` `tutorial_chest` 同类型同 Overworld 容器），两者坐标落在同一 epsilon（≤0.5 格）范围内；跑 hydrate 重绑，断言 sentinel 的 `guarding_container` 精确指向 `tsy_lingxu_01` 那个容器实体，**绝不**误绑到 `spawn_tutorial` 容器——即使后者坐标更早出现在 `relic_containers` 切片里也不能被匹配到

## P1 — snapshot 补齐 sentinel 身份载荷

- [ ] 新增 `DormantTsySentinelSnapshot`（`server/src/npc/dormant/mod.rs`，紧邻 `DormantTsyHostileSnapshot` line 231 之后），字段：
  - `guarding_container_pos: Option<[f64; 3]>`——**稳定重绑键 = `family_id` + 容器世界坐标**（不存 `Entity`，见 §8.1 #1 决议；`Entity` 本身不可 serde，且 `NpcDormantStore` 经 Redis 持久化跨重启，裸 `Entity` 索引在长期存续场景下有 generation 复用风险）。**`family_id` 不在本结构体内另开字段**——`spawn_tsy_sentinel_at`（`server/src/npc/tsy_hostile.rs:1036-1096`）为同一实体同时插入 `TsyHostileMarker{family_id}` 与 `TsySentinelMarker{family_id}`（两值恒相等），且 `dormant_tsy_hostile_snapshot` 只在 `TsyHostileMarker` 存在时才返回 `Some`（`server/src/npc/hydrate/mod.rs:485-490`，`marker?` 短路返回）——因此任意实体只要 `snapshot.tsy_sentinel.is_some()`，`snapshot.tsy_hostile` 必为 `Some`。P2 重绑直接读 `snapshot.tsy_hostile.family_id` 做 family 过滤键，不新增冗余字段、不产生两处 family_id 可能不同步的风险
  - `phase: u8`
  - `max_phase: u8`
- [ ] `NpcDormantSnapshot`（`server/src/npc/dormant/mod.rs:276+`）新增字段 `tsy_sentinel: Option<DormantTsySentinelSnapshot>`（`#[serde(skip_serializing_if = "Option::is_none", default)]`，非破坏迁移——旧快照反序列化为 `None`）
- [ ] `DormantExtraComponentQueries`（`server/src/npc/hydrate/mod.rs:59-67`）补两个 query 字段：
  - `tsy_sentinel_markers: Query<'w, 's, Option<&'static TsySentinelMarker>, With<NpcMarker>>`
  - `containers: Query<'w, 's, &'static Position, (With<LootContainer>, Without<NpcMarker>)>`（dehydrate 侧 `guarding_container: Option<Entity>` 是精确已知的单个 `Entity`，`.get(entity)` 直接拿 `Position` 写快照即可，此处不存在多容器歧义、无需过滤 family_id；family_id 过滤只在 P2 hydrate 反查阶段——从坐标反推 Entity——才需要，见下）
- [ ] 新增 `dormant_tsy_sentinel_snapshot(marker: Option<&TsySentinelMarker>, containers: &Query<...>) -> Option<DormantTsySentinelSnapshot>`（`server/src/npc/hydrate/mod.rs`，紧邻 `dormant_tsy_hostile_snapshot` line 485 之后），在 `dehydrate_far_npcs_system` 的 candidate 构造处（`server/src/npc/hydrate/mod.rs:425-430` 附近）接入，写进 `NpcDormantSnapshot.tsy_sentinel`
- [ ] 明确"普通 `GuardianRelic`"与"TSY sentinel"在 dormant 载荷层的判别位：`snapshot.tsy_sentinel.is_some()` 即为 sentinel 分支判据（P2 hydrate 路由用它），`snapshot.guardian_relic`（`DormantGuardianRelicSnapshot`）继续只服务纯 overworld relic guard——两者字段互斥不复用（对齐 §8.1 #3 不变量测试）

## P2 — hydrate 重建 sentinel 专属语义

- [ ] `spawn_tsy_sentinel_at` 签名收紧化：`guarding_container: Entity` → `guarding_container: Option<Entity>`（`server/src/npc/tsy_hostile.rs:1036-1043`），内部 `TsySentinelMarker.guarding_container` 直接透传（本来就是 `Option<Entity>`，line 86）。同步改 3 个既有调用点：
  - `spawn_tsy_hostiles_for_family`（`server/src/npc/tsy_hostile.rs:623-630`）：`guard.entity` → `Some(guard.entity)`
  - 两处测试调用（`server/src/npc/tsy_hostile.rs:2245`、`2327`）：`Some(...)` 包裹
- [ ] `spawn_from_snapshot`（`server/src/npc/hydrate/mod.rs:553-560`）签名新增参数 `relic_containers: &[(Entity, String, DVec3)]`（三元组 = entity / `LootContainer.family_id` / 世界坐标，由两个调用方各自的新增 `Query<(Entity, &Position, &LootContainer), With<LootContainer>>` 现算出的 `Vec` 传入：`hydrate_dormant_near_players_system` line 83-172、`hydrate_dormant_on_rechallenge_trigger` line 190-260。**比原决议多带一个 `family_id`**——博弈 blocker 指出仅坐标做重绑键在存在异 family 同类型容器时有偶合误绑风险，见 §8.1 #1 补漏；`LootContainer`（`server/src/world/tsy_container.rs:126`）本就自带 `family_id` 字段，零成本随手多取）
- [ ] `spawn_from_snapshot` 内 `NpcArchetype::GuardianRelic` 分支（`server/src/npc/hydrate/mod.rs:641-656`）改为按 `snapshot.tsy_sentinel` 是否 `Some` 二选一：
  - `Some(sentinel_snapshot)` → 调 `spawn_tsy_sentinel_at`，`guarding_container` 由**两段式**匹配得到的 `Option<Entity>`：**先按 `family_id == snapshot.tsy_hostile.family_id`（§P1 已论证 sentinel 快照必然伴随同 family_id 的 `tsy_hostile` 快照）过滤 `relic_containers` 子集，再仅在该子集内按 `sentinel_snapshot.guarding_container_pos` 做坐标 epsilon（≤ 0.5 格）精确匹配**——不允许跨 family 对全体 `relic_containers` 裸坐标匹配（会被 `spawn_tutorial.rs:497` 一类异 family 同 Overworld 容器偶合误绑，见 §8.1 #1 补漏）；family 内确无匹配坐标（容器确已消失，理论上现状不会发生，见 §8.1 #2）→ 传 `None`，`tracing::warn!` 记录（日志带上尝试匹配的 `family_id` + 坐标，便于事后排查是否真的触发了跨 family 偶合），sentinel 仍然按秘境守灵身份 spawn（只是不再绑定具体容器 alarm，行为退化为纯 aggro，不阻塞外观/HUD/掉落）
  - `None` → 保持现状调 `spawn_relic_guard_npc_at`（纯 overworld relic guard 路径不变）
- [ ] hydrate 后重新接好（`server/src/npc/hydrate/mod.rs:764-791` 对应位置扩展 `if let Some(tsy) = snapshot.tsy_hostile` 块旁新增 `if let Some(sentinel) = snapshot.tsy_sentinel` 分支，或直接在 `spawn_tsy_sentinel_at` 调用后 `entity_commands.insert(...)` 补 `max_phase`）：
  - `TsySentinelMarker`（含 `max_phase` 精确回填；`phase` 字段回填但明确其为 best-effort——见 §8.1 #2，`Wounds` 本轮 hydrate 恒为满血，`update_sentinel_phase_system` 会在下一次该系统运行时按真实血量重新计算 `phase`，快照里的 `phase` 值不产生用户可见的持久错位）
  - `FaunaVisualKind::TsySentinel`（`spawn_tsy_sentinel_at` 内部已带，无需额外 insert）
  - `sentinel_thinker()`（同上，`spawn_tsy_sentinel_at` 内部已带）
  - 守护容器绑定（`guarding_container` 解析结果，见上）
- [ ] 确认不会误把普通 overworld `GuardianRelic` 提升成 TSY sentinel：`snapshot.tsy_sentinel` 只有在原实体确实带 `TsySentinelMarker` 时才会在 P1 dehydrate 阶段被写入非 `None`，纯 overworld relic guard 从不携带该组件 → `snapshot.tsy_sentinel` 恒为 `None` → 路由天然落到 `spawn_relic_guard_npc_at` 分支，无需额外判别逻辑

## P3 — 玩家可见回归

- [ ] 走玩家真实链路验收（本地 `cargo run` + `/qi`/`/realm`/`/time advance` 等 dev 命令辅助触发，非自动化测试，人工核对）：
  - 玩家进入 TSY 深层，见到 sentinel（`spawn_tsy_hostiles_for_family` 正常路径，`server/src/npc/tsy_hostile.rs:561-641`）
  - 玩家离开 TSY 或拉远到 dehydrate 条件成立（`config.dehydrate_radius_blocks`，`NpcVirtualizationConfig`）
  - 玩家返回后，原 sentinel 仍显示为秘境守灵，不变成 villager 风格 relic guard（视觉核对 `FaunaVisualKind::TsySentinel` 渲染 + `EntityKind` 非 `VILLAGER`）
  - boss 血条继续出现（`network/tsy_polish.rs:95-153` 只查询 `TsySentinelMarker`——P2 落地后自动满足，无需额外改动）
  - phase 继续推进（`npc/tsy_hostile.rs:1556-1581` `update_sentinel_phase_system`——`max_phase` 精确回填后自动满足）
  - 死亡掉落仍走 `tsy_sentinel` 分支（`npc/tsy_hostile.rs:1739-1847` `handle_npc_death_drop` + `drop_key_for_npc:1835-1847`——`TsySentinelMarker` 正确回填后自动满足；`guarding_container_kind` 相关的 `__origin_keyed_key__` 钥匙掉落若容器重绑成功则精确保留，重绑失败也只丢这一个子分支，不影响其余掉落）
- [ ] 补一条端到端集成测试（`server/src/npc/hydrate/mod.rs` 测试模块）：`sentinel_survives_full_dehydrate_hydrate_cycle_with_container_still_present`——完整跑 dehydrate → 建 dormant snapshot → hydrate，断言容器仍在（未被上文任何系统触碰）时重绑 100% 成功，同时验证掉落键路径（直接调 `drop_key_for_npc` 或 `handle_npc_death_drop` 断言产出 `tsy_sentinel` 键而非其它）

## 玩家影响

- 玩家正常游玩可达：进入 TSY 深层摸完 relic core、撤出秘境、稍后回来二刷，就是最自然的触发方式
- 当前结果不是"后台状态少一位"这种无感 bug，而是**直接把 boss 洗成另一种 NPC**
- 具体体感退化：
  - 守灵外观/实体种类错
  - boss 血条消失
  - sentinel 三段 phase 技能链失效
  - 专属掉落分流失效
  - 设计上"守灵看守 relic core"的身份被破坏

## §8 开放问题（原表，历史回溯用）

1. `guarding_container` 跨 dormant 往返要用什么稳定键重绑：容器实体 `Entity` 不能直接进长期 snapshot，需拍定逻辑 ID / source key / family 内索引
2. sentinel 的 `phase` 是否应精确保留，还是允许 hydrate 后重置到 phase 0；若允许重置，需要确认是否与 boss HUD / 掉落 / 伤害档位设计冲突
3. 是否顺手为"复用 `NpcArchetype::GuardianRelic` 的 TSY/overworld 双身份"加一条统一不变量测试，避免未来再发生 marker 丢失后语义洗平

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-04）

### #1 `guarding_container` 跨 dormant 稳定键

**决议（2026-07-04 补丁：坐标单独作键在异 family 容器偶合场景下会静默误绑，改为 `family_id` + 坐标复合键，原决议 #1/#2/#3 坐标结论保留，新增 #4 补漏）**：
1. 用 **`family_id` + 容器世界坐标**（`Position` → `[f64; 3]`）的复合键作稳定重绑键，不引入新的逻辑 ID / family 内索引体系。**不再是坐标单独作键**——见 #4。
2. 理由（坐标部分，原决议保留）：实地核查 `server/src/world/tsy_dev_command.rs:394-411` 确认 `LootContainer` 实体从 spawn 起就没有任何超出 `family_id + depth + kind + Position` 的持久身份字段；`TsyContainerSpawnRef { entity, pos }`（`server/src/npc/tsy_hostile.rs:385-388`）本来就随手带着 `pos`。容器一旦放置永不移动（无任何系统对 `LootContainer` 实体做位置更新），坐标精确匹配（epsilon ≤ 0.5 格）足够可靠地找回同一个容器实体。`DormantGuardianRelicSnapshot`（`server/src/npc/dormant/mod.rs:199-207`）里 `alarm_center: [f64;3]` 已经是"snapshot 层存位置不存 Entity"的既有先例，本决议延续同一模式，不另造一套 ID 分配器。
3. 拒绝方案：给 `LootContainer` 加自增 `container_id: u64` 字段——需要一个新的全局分配器 + 迁移现有存档格式，成本远高于复用已知稳定的 `Position` + 已存在的 `family_id`；且 `Entity` 直接塞进 snapshot 的路线在 §8.1 #2 已被否决（不可 serde + generation 复用风险），此处一并说明。
4. **补漏（博弈 blocker，2026-07-04）**：实地核查 `LootContainer::new` 全仓共 7 处调用点——`server/src/world/tsy_dev_command.rs:397`（真实 TSY 秘境生成路径，family_id 为实际 zone family，如 `tsy_lingxu_01`）之外，还有 6 处：`server/src/world/spawn_tutorial.rs:497`（**生产路径**，`tutorial_chest` POI，`ContainerKind::StoragePouch`，family_id 硬编码字面量 `"spawn_tutorial"`，同样落在 `EntityLayerId(layers.overworld)`）、`server/src/network/tsy_container_search_emit.rs:278`、`server/src/world/tsy_container.rs:352/363/372`、`server/src/world/tsy_container_search.rs:1050`（后 5 处均为 `#[cfg(test)]` 测试构造，不在生产 spawn 路径上，但代码里确实存在同类型 `LootContainer` 实例）。若只用坐标做重绑键，`tutorial_chest` 与某个真实 TSY 容器一旦坐标落入同一 epsilon（≤0.5 格）——两者同为 Overworld layer，教程区块与某个 TSY 秘境入口在 worldgen 摆位上理论并非彼此隔离到不可能靠近——`relic_containers` 的坐标匹配会把 sentinel 误绑到 `family_id="spawn_tutorial"` 的教程箱子上，产生携带错误 `container_entity_id` 的 `TsySentinelPhaseChanged` 广播——这正是本 plan 要消灭的"身份错配"缺陷的一个变种，绝不能被本 plan 自己的修复引入。`LootContainer`（`server/src/world/tsy_container.rs:126`）与 `TsyHostileMarker`/`TsySentinelMarker`（`server/src/npc/tsy_hostile.rs:80/85`）三者都已经自带 `family_id` 字段，零成本引入一次 `family_id` 相等性前置过滤，即可把重绑检索范围从"全体 `LootContainer`"收窄到"同一 TSY family 的容器"，结构性排除跨 family（`spawn_tutorial` vs `tsy_lingxu_01`）与跨维度的偶合，不再依赖"坐标数值碰巧不重叠"的运气。

**落点**：`server/src/npc/dormant/mod.rs:231`（`DormantTsyHostileSnapshot` 定义处新增 `DormantTsySentinelSnapshot` 结构体，**不新增独立 family_id 字段，复用同一实体上必然同时存在的 `snapshot.tsy_hostile.family_id`**，见 §P1）/ `server/src/npc/hydrate/mod.rs:59-67`（`DormantExtraComponentQueries.containers` query，dehydrate 侧不变；hydrate 侧改为携带 `&LootContainer` 一并取 family_id，见 §P2）/ plan §P1（"新增 `DormantTsySentinelSnapshot`"条目）/ plan §P2（"relic_containers 查找"条目，改为 family_id 前置过滤 + 坐标精确匹配两段式）/ plan §P0（新增 `hydrated_tsy_sentinel_container_rebind_ignores_same_position_different_family` pin 测试）

### #2 sentinel `phase` 精确保留 vs 重置 + 容器 dehydrate/despawn 容错回退（合并回答，两条互相印证）

**决议（phase）**：
1. `max_phase`（design 常量，当前恒为 3）精确回填快照与恢复，这是稳定值，无成本无风险。
2. `phase`（当前伤害档位）**照抄写入快照并在 hydrate 时回填**，但明确其为 best-effort 展示值：实地核查确认 `NpcDormantSnapshot`（`server/src/npc/dormant/mod.rs:276+`）全局没有 `Wounds`/health 字段（`grep Wounds server/src/npc/dormant/mod.rs server/src/npc/hydrate/mod.rs` 零命中）——**所有** NPC archetype（不只 sentinel）在 hydrate 时都通过 `npc_runtime_bundle` 拿到满血 `Wounds`，这是本 plan 范围之外的既有系统行为。`update_sentinel_phase_system`（`server/src/npc/tsy_hostile.rs:1556-1581`）每次 `Update` 都会按*当前*（满血）`Wounds` 重算 `next_phase` 并在不等于 `marker.phase` 时立即纠正——也就是说 hydrate 后哪怕写入了"stale"的 `phase=2`，下一次该 system 运行（同 tick 或次 tick，Update 内每帧跑）就会被真实血量覆盖成 `phase=0`。不存在"精确保留 phase 与满血展示冲突"的设计矛盾，两者会在一帧内自洽收敛。
3. 结论：不需要为 sentinel 单独引入血量持久化（那是另一个更大范围的"dormant 保留伤势"课题，不在本 plan scope），`phase`/`max_phase` 都照抄进快照只是为了字段完整性和未来若某天引入伤势持久化时不用再改一次 schema。

**决议（容器容错回退）**：
1. 实地核查确认**当前代码库里容器实体从未被 dehydrate 或 despawn**：`dehydrate_far_npcs_system`（`server/src/npc/hydrate/mod.rs:279-295`）query 条件是 `With<NpcMarker>`，`LootContainer` 实体从不携带 `NpcMarker`（`server/src/world/tsy_dev_command.rs:394-405` spawn bundle 里没有）；`tsy_collapse_completed_cleanup`（`server/src/world/tsy_lifecycle.rs:548-690`）在秘境塌缩时只处理 `DroppedLootRegistry` 条目 / `Daoxiang` NPC / `CorpseEmbalmed`，**完全不触碰 `LootContainer` 实体本身**（grep `LootContainer` 全文零命中于该 cleanup 函数）。所以"容器被 dehydrate/despawn 导致重绑失败"目前是一个**不会发生的场景**，不是要修的 bug。
2. 但坐标匹配仍是运行时查找（`relic_containers` 切片先按 `family_id == snapshot.tsy_hostile.family_id` 过滤，再 `.iter().find(|(_, _, pos)| ...)` 坐标匹配，两段式见 §8.1 #1 补漏 / §P2），必须写容错分支而非 `.unwrap()`/`.expect()`——万一未来某个新系统（比如某个道具"摧毁容器"机制）改变了这个不变量，family 内找不到匹配坐标时优雅退化为 `guarding_container: None`，只影响：a) `TsySentinelPhaseChanged` 事件不再带 `container_entity_id`（`server/src/npc/tsy_hostile.rs:1571-1579`，已有 `if let Some(container) = ...` 判空）；b) `handle_npc_death_drop` 的 `guarding_container_kind`（`server/src/npc/tsy_hostile.rs:1805-1811`）为 `None`，只丢 `__origin_keyed_key__` 模板对应的钥匙掉落这一个子分支（`server/src/npc/tsy_hostile.rs:1908-1917`），不影响 boss HUD、phase 推进本体、其余掉落。这是可接受的优雅降级，不是需要额外补偿的红线。
3. 不引入"容器丢失时报警/告警系统"或"容器持久化 ID 注册表"这类额外基建——现状验证不需要，过度设计。

**落点**：`server/src/npc/tsy_hostile.rs:1556-1581`（`update_sentinel_phase_system`，phase 自洽收敛机制不变）/ `server/src/npc/hydrate/mod.rs:641-656`（P2 新分支里 `.find(...)` 找不到匹配坐标时的 `None` 回退 + `tracing::warn!`）/ plan §P2（"匹配不到...传 `None`"条目）

### #3 `GuardianRelic` 双身份统一不变量测试

**决议**：
1. 加。新增 `guardian_relic_dual_identity_invariant_partitioned_by_sentinel_marker` 测试，断言 hydrate 产出的 `NpcArchetype::GuardianRelic` 实体在任意时刻只落在两个互斥分支之一：（a）带 `TsySentinelMarker` + `FaunaVisualKind::TsySentinel`，不带 `GuardianDuty`/`TrialEval`；（b）带 `GuardianDuty`/`TrialEval`，不带 `TsySentinelMarker`。
2. 判别源头钉死在 snapshot 层：`snapshot.tsy_sentinel.is_some()`（P1 新增字段）是唯一路由判据，不允许未来有代码路径绕过这个字段直接查 `archetype == GuardianRelic` 就假定是某一种身份。
3. 测试直接放进 P0 pin 测试清单（不单独起新阶段），因为它锁的正是本 plan 要修的那条分流逻辑本身，属于同一批回归测试的自然收尾，拆开只会增加交叉引用成本。

**落点**：`server/src/npc/hydrate/mod.rs:641-656`（新路由分支落点）/ plan §P0（pin 测试清单第 4 条）
