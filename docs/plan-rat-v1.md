# Bong · plan-rat-v1 · 骨架

**噬元鼠 · 从零落地 + 密度相变蝗潮**。把 worldview §七 + 馆藏《异兽三形考》里的"压差网 / 末法淋巴系统 / 修炼苍蝇"完整实装：

1. **基线（散居态）** —— 给 BeastKind::Rat 配真本体（EntityKind + 自有 spawn）+ 专属 big-brain AI（PressureSeek + Regroup + 攻击只扣真元），打通 击杀→shu_gu→G 拾取→inventory 全链路
2. **相变** —— 借 locust phase polyphenism（蝗虫密度依赖相变）做"灵蝗潮"：噬元鼠在 chunk 内密度 + 周围灵气压差陡度超阈值 → `Solitary → Transitioning → Gregarious`，单只视觉/速度/AI 全切
3. **蝗潮天灾** —— `Gregarious` 鼠群跨 zone 行进，路径吸光 zone qi、扫掉 ItemEntity、围住修士不停扣真元；扩展现有 `EVENT_BEAST_TIDE` 加 `tide_kind = "wandering" | "locust_swarm"` 子类型
4. **天道 agent 接入** —— chunk 局部相变 server 自动 detect；跨 zone 升级为大潮天灾必须 agent 拍板（agent 收到 `bong:rat_phase_event` → 决策 → 发 `spawn locust_swarm` 命令）

**世界观锚点**：
- `worldview.md §七 动态生物生态 / 噬元鼠群`（"修炼苍蝇" + 灵气波动吸引）
- `worldview.md §七 / 生态联动 / 大迁徙`（现有"逃逸式兽潮"——本 plan 补"主动侵略式"灵蝗潮）
- `worldview.md §八 天道行为准则 / 激烈手段 / 域崩`（蝗潮 = 天道激烈手段的生物形态变体）
- `worldview.md §八 / 灵物密度阈值`（"分仓逼迫"——本 plan 把它从"装备/丹药密度"扩到"活体灵气密度"）
- `worldview.md §十 资源与匮乏`（被 G 拾取的 shu_gu 走标准骨币原料链路）

**library 锚点**：
- `docs/library/ecology/异兽三形考.json`（藏荒散修九年观鼠：压差网 / 不回已采地点 / 鼠群是末法淋巴系统）—— **物理逻辑必读**
- 后续待写 `peoples-XXXX 灵蝗见闻录`（蝗潮亲历散修视角，anchor §七 + §八）

**交叉引用**：
- `plan-fauna-v1`（✅ 归档；前置——本 plan 复用 `FaunaTag::Rat` / `BeastKind::Rat` / `shu_gu` drop / `roll_fauna_drops`，不重定义）
- `plan-npc-ai-v1`（✅ 归档；前置——big-brain Utility AI 框架；本 plan 加 Rat 专属 Scorer/Action 入 `npc::brain` plugin）
- `plan-inventory-v1` / `plan-input-binding-v1`（✅ 归档；前置——`DroppedLootRegistry` + `InteractionKeybindings.G` + `dropped_loot_sync_emit`，本 plan P1 写 e2e 验闭环）
- `plan-cultivation-v1`（✅ 归档；前置——攻击 Cultivator 时只扣 `qi_current` 不扣 hp，复用现有 qi pool API）
- `plan-tribulation-v1` / `plan-niche-defense-v1`（同源——本 plan §3 灵蝗潮按 worldview §八 是 thunder_tribulation / realm_collapse 平级的"天道激烈手段"）
- `plan-agent-v2`（✅ 归档；前置——agent tool call + arbiter，本 plan P4 加 `locust-swarm-narration.ts`）
- `plan-multi-style-v1`（旁路；爆脉流 / 截脉流跟蝗潮的伤害模型互动留 P5 验收）
- `plan-spawn-tutorial-v1`（✅ 归档；**前置 + 收敛**——已实装教学占位 `dynamic_rat_swarm_spawner` + `TutorialRatSwarmNpc` + `tutorial_rat_qi_drain_tick`（`server/src/world/spawn_tutorial.rs:498/137/173`），spawn 的是 zombie。本 plan P1 必须把教学触发口接到 `spawn_rat_npc_at`、把 `tutorial_rat_qi_drain_tick` 收敛到本 plan 统一的 `apply_rat_bite_qi_drain`，避免两套"鼠扣真元"并行）
- 待立 `plan-fauna-mimic-spider-v1` / `plan-fauna-stitched-beast-v1`（同源 sibling；本 plan 不动 Spider/HybridBeast 行为，但 RatPhase enum / pressure 模块设计要给那两 plan 留延伸口）

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | Rat 实体本体 + spawn 函数 + EntityKind 选定 | ⬜ |
| P1 | Rat 散居态 big-brain AI + 击杀→shu_gu→G 拾取 e2e 验证 | ⬜ |
| P2 | RatPhase 三态机（Solitary / Transitioning / Gregarious）+ 相变触发器 | ⬜ |
| P3 | 灵蝗潮：扩展 EVENT_BEAST_TIDE 加 tide_kind + 跨 zone 行进 + 路径吸 qi | ⬜ |
| P4 | 天道 agent 接入（rat_phase_event Redis 推送 + locust-swarm-narration 决策） | ⬜ |
| P5 | 客户端表现（HUD warning / Phase 视觉切换 / 蝗潮预兆 audio）+ 饱和测试收口 | ⬜ |

---

## 接入面 checklist（防孤岛）

| 维度 | 内容 |
|------|------|
| **进料** | `world::karma::QiDensityHeatmap`（chunk qi 浓度，决定 PressureGradient + 蝗潮目标方向） · `inventory::DroppedLootRegistry.entries`（离体真元源——骨币堆吸引压差） · `cultivation::Cultivation`（修士打坐时 qi 外泄强度——"修炼苍蝇"判定） · `npc::lifecycle::NpcRegistry`（chunk 内 Rat 头数计数） |
| **出料** | `inventory::DroppedLootRegistry`（鼠死后 `shu_gu` drop，复用 `fauna::drop::fauna_drop_system`） · `cultivation::Cultivation::qi_current`（鼠攻击只扣真元，不走 `combat::DamageEvent`） · `world::karma::QiDensityHeatmap` mutate（蝗潮经过的 chunk qi 减少） · agent 推送 `bong:rat_phase_event`（chunk 相变事件） |
| **共享 event** | 复用 `EVENT_BEAST_TIDE`（`world::events.rs:40`）扩展 `tide_kind` 字段，**不另立** `EVENT_RAT_SWARM`；复用 `BeastTideRuntimeState`（`world::events.rs:91`）扩成 enum `RuntimeState::Wandering(...) \| LocustSwarm(...)`；新增 `RatPhaseChangeEvent`（仅 server 内部 + agent 推送，不与既有 event 同名） |
| **跨仓库契约** | **server**：`RatPhase`（enum） · `RatGroupId`（component） · `PressureSensor`（component+system） · `RatBlackboard`（component） · `LocustSwarmRuntimeState`（struct） · `EVENT_BEAST_TIDE` 扩 `tide_kind` 参数 · `RatPhaseChangeEvent`（Bevy event） · `bong:rat_phase_event`（Redis pub）<br>**agent**：`locust-swarm-narration.ts`（新文件，参考 `tribulation-runtime.ts` 模式） · `world_state.rat_density_heatmap`（新字段，TypeBox） · 新 tool `query_rat_density(zone)` （供 agent 主动查询）<br>**client**：P0–P3 沿用 vanilla silverfish skin（无新协议） · P5 加 `bong:locust_swarm_warning` CustomPayload（HUD 警示，参考 `realm_collapse_boundary` VFX） |
| **worldview 锚点** | §七 噬元鼠（修炼苍蝇语义） + §七 大迁徙（兽潮基础） + §八 天道激烈手段 + §八.1 灵物密度阈值（活体灵气密度变体） + §十 资源与匮乏（shu_gu 走骨币原料链） |
| **红旗自查** | ❌ 自产自消（接 inventory / cultivation / agent / worldgen / combat） · ❌ 近义重名（复用 BeastKind::Rat / FaunaTag / EVENT_BEAST_TIDE / DroppedLootEntry，新增名称无碰撞） · ❌ 无 worldview 锚（§七 §八 §十 三处） · ❌ skeleton 同主题未合（skeleton 无 fauna behavior 骨架） · ❌ 跨仓库缺面（server + agent 都改；client 沿用 vanilla skin 至 P5） |

---

## §0 设计轴心

- [ ] **Rat 不掉血只掉真元**：噬元鼠攻击 Cultivator 时直接扣 `qi_current`，不走 `combat::DamageEvent` 路径——单只伤害极小（每次 1 qi），但成群围住会让修士打坐瞬间被吸干。匹配 worldview §七 "修炼苍蝇"语义
- [ ] **从不回已采地点**：Rat 的 PressureSensor 维护 `recently_drained_chunks: BTreeSet<ChunkPos>`（窗口 N tick），Scorer 对这些 chunk 给 0 分。匹配《异兽三形考》"鼠群从不回已采地点"
- [ ] **群体感应不是通讯是物理**：不实装 messaging / broadcast；改成"每只 Rat 独立感应周围 Rat 的 qi 信号 + zone qi 梯度"，群行为是 emergent 的（同 chunk 鼠看到同一 PressureField → 自然走向同一目标）
- [ ] **相变不可逆是设计**：Solitary→Gregarious 触发后**不在本 plan 范围内提供回退**——蝗潮经过 zone qi 归 0 + 群规模降阈值下 → 大批饿死 drop shu_gu 雨。死亡 = 解散，符合"灵气尽则群亡"
- [ ] **不做 spawner**：Rat 头数仍由 mob_spawn / 天道 agent 控制；本 plan 只做"已 spawn 的 Rat 怎么行为"
- [ ] **EntityKind 占位**：vanilla silverfish 作 P0 视觉占位（小、灰、贴地、群居感），自有 model/skin 留 plan-mob-skin 后续重做

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·压差网**：每只 Rat 携微量 qi（< 1）形成局部压差场，修士的 qi pool 在场中是"山"——鼠不是"想吃你"，是"被压差物理推向你"
- **噬论·末法淋巴**：鼠群把局部淤积的 qi 吸散到贫瘠区（鼠到达低 qi 区消化散气）；蝗潮是淋巴系统**应激反应**——一处 qi 浓度爆表导致鼠群"发炎"
- **音论·相变是共振**：单只鼠的"压差网震荡"在密度低时随机相消；密度过阈值时所有鼠的震荡相位锁定 → 相变（与 §四 真元过载撕裂同源，赌命式集体涨潮）
- **影论·蝗潮是天道分身**：蝗潮经过的轨迹是"天道意志的可见化"——天道不点穴位，它推蝗潮去碾压灵气淤积处

---

## §2 P0 — Rat 实体本体

- [ ] **EntityKind 选定**：`EntityKind::Silverfish`（vanilla 1.20.1 已有，体型贴近"鼠"，先行占位）。spawn bundle 用 `valence::entity::silverfish::SilverfishEntityBundle`
- [ ] **新文件 `server/src/npc/spawn_rat.rs`**：`spawn_rat_npc_at(commands, position, zone_name) -> Entity`，包含 `NpcMarker + FaunaTag::Rat + RatBlackboard + Navigator + MovementController + npc_runtime_bundle`
- [ ] **替换现有 `spawn_beast_tide_zombie`** 中 `BeastKind::Rat` 分支：原本统一 spawn zombie，改为按 `BeastKind` dispatch（Rat → spawn_rat_npc_at；其他先保持 zombie，留 sibling plan 重做）
- [ ] **`RatBlackboard` component**（`server/src/npc/spawn_rat.rs`）：`{ home_chunk: ChunkPos, group_id: RatGroupId, last_pressure_target: Option<DVec3>, recently_drained: ArrayVec<ChunkPos, 8> }`
- [ ] **测试**：
  - `spawn_rat_npc_at_attaches_fauna_tag_and_blackboard`（FaunaTag/RatBlackboard 都挂上）
  - `spawn_rat_npc_at_uses_silverfish_entity_kind`
  - `beast_tide_event_spawns_rats_via_spawn_rat_when_kind_is_rat`（兼容老 zombie path）
  - `mob_spawn_dispatches_to_spawn_rat_for_rat_kind`

---

## §3 P1 — 散居态 AI + G 拾取闭环 e2e

### big-brain Scorer/Action（在 `server/src/npc/brain.rs` 注册新 plugin module 或拆 `npc/brain_rat.rs`）

| Component | 作用 | Score 计算 |
|---|---|---|
| `QiSourceProximityScorer` | 距离最近 qi 源（修士打坐 / DroppedLootEntry { item: bone_coin\|qi_dense } / zone qi 高点）越近越高 | `1.0 - clamp(dist/32, 0, 1)` |
| `GroupCohesionScorer` | 离群中心（同 RatGroupId 鼠的位置均值）越远越高（拉回群） | `clamp(dist_to_centroid / 16, 0, 1)` |
| `DrainedChunkAvoidScorer` | 当前所在 chunk 在 recently_drained 集合时，所有 seek 评分压到 0 | bool |
| `WanderScorer`（复用现有） | 兜底散步 | baseline 0.08 |

| Action | 作用 | Success 条件 |
|---|---|---|
| `SeekQiSourceAction` | 走向最近 qi 源；到达后触发 `RatBiteEvent`（消费 1 qi）然后退到 4 格外消化 | qi 源消失 / 距离 < 0.8 / tick > 600 |
| `RegroupAction` | 走向群中心 | 距离 < 4 / tick > 200 |
| `RatWanderAction` | 复用 `WanderAction` | (现成) |

### 攻击只扣真元

- [ ] 新 event `RatBiteEvent { rat: Entity, target: Entity, qi_steal: u32 }`（`server/src/combat/events.rs`）
- [ ] 新 system `apply_rat_bite_qi_drain`：消费 RatBiteEvent → 找 target 的 `Cultivation` → `qi_current = qi_current.saturating_sub(qi_steal)` → 不 emit DamageEvent
- [ ] **不走 hp**：玩家 hp 不掉；UI 上需要 client 后续提示"被噬元鼠吸真元"，P5 处理

### 收敛 spawn-tutorial 教学占位（**避免双实现**）

- [ ] **`server/src/world/spawn_tutorial.rs::dynamic_rat_swarm_spawner`** 改写：原本直接 spawn zombie，改为调用本 plan 的 `spawn_rat_npc_at`（带 `TutorialRatSwarmNpc` 标记区分教学群 vs 自然群）
- [ ] **`server/src/world/spawn_tutorial.rs::tutorial_rat_qi_drain_tick`** 删除：教学鼠群通过本 plan `RatBiteEvent → apply_rat_bite_qi_drain` 链路扣 qi（行为完全一致，但实现唯一）。教学专属差异（如固定 qi_steal=1 / 强制目标=新手玩家）通过 `TutorialRatSwarmNpc` component 在 `apply_rat_bite_qi_drain` 里分支即可
- [ ] 跑 `plan-spawn-tutorial-v1` 既有教学集成测试确保零回归（教学 30min 钩子表 / RatSwarmEncounter hook 触发不变）
- [ ] 测试 `tutorial_rat_swarm_uses_spawn_rat_npc_at_not_zombie`（验 EntityKind 是 silverfish 不是 zombie）

### G 拾取 e2e（**用户点名要验**）

- [ ] **新集成测试 `server/src/fauna/integration_rat_pickup.rs`**：
  ```
  spawn_rat → Cultivator player nearby → trigger SeekQiSource → 玩家击杀 rat
    → fauna_drop_system 注册 DroppedLootEntry { item: shu_gu, ... }
    → 模拟 client PickupDroppedLoot 请求
    → assert: DroppedLootRegistry 移除该 entry
    → assert: player inventory 里出现 shu_gu × N
    → assert: dropped_loot_sync_emit 推送给所有 client 的 snapshot 不再包含
  ```
- [ ] 测试名要明示：`rat_kill_to_g_pickup_round_trip_creates_inventory_shu_gu`

### 测试

- [ ] `qi_source_proximity_scorer_ranks_nearest_meditator_first`
- [ ] `group_cohesion_pulls_lone_rat_back_to_centroid`
- [ ] `drained_chunk_avoid_blocks_seek`
- [ ] `seek_qi_source_action_triggers_rat_bite_at_close_range`
- [ ] `rat_bite_drains_only_qi_no_hp_damage`
- [ ] `rat_kill_to_g_pickup_round_trip_creates_inventory_shu_gu`（e2e）

---

## §4 P2 — RatPhase 三态相变

### 类型定义（`server/src/fauna/rat_phase.rs` 新文件）

```rust
pub enum RatPhase {
    Solitary,                        // 散居态：默认；走 §3 散居 AI
    Transitioning { progress: u16 }, // 过渡态：视觉变深红，速度 1.2×；progress 0..MAX_TRANSITION
    Gregarious,                      // 群居态：视觉黑红，速度 1.5×；走蝗潮 AI（§5）
}

#[derive(Component)]
pub struct PressureSensor {
    pub local_density: f32,    // chunk 内同 RatGroupId 鼠头数 / 阈值
    pub qi_pressure_grad: f32, // 周围 zone qi 梯度陡度
    pub surge_intensity: f32,  // 累积"潮气强度"，过 SURGE_TRIGGER_THRESHOLD → 全 chunk 同步升级
}
```

### 相变触发器 system

- [ ] `pressure_sensor_tick_system`（PreUpdate，Rat LOD-gated）：
  - 每 N tick 重算 `local_density` = chunk 内 RatGroupId 计数 / `RAT_PHASE_DENSITY_THRESHOLD`（默认 8 只 / 16x16 chunk）
  - 重算 `qi_pressure_grad` = `QiDensityHeatmap` 在 chunk 周围 3x3 的 max - min
  - `surge_intensity += local_density * qi_pressure_grad * dt`
  - 当 chunk 内任一 Rat surge_intensity > `SURGE_TRIGGER_THRESHOLD` → emit `RatPhaseChangeEvent { chunk, from: Solitary, to: Transitioning }`
  - Transitioning 持续 `TRANSITION_DURATION_TICKS`（默认 600 tick = 30s 真实时；后续可调到几分钟）→ 升级到 Gregarious
- [ ] `apply_rat_phase_change_system`：消费 RatPhaseChangeEvent → 把整 chunk 内同 RatGroupId 的 Rat 全部更新 phase（同步相位）
- [ ] **跨 zone 信号**：每次 `Solitary → Transitioning` 转换 emit `RatPhaseChangeEvent`，同时通过 `redis_outbox` push 一条 `bong:rat_phase_event` payload（agent 端 P4 消费）

### 不可逆但可终结

- [ ] Phase 不在本 plan 范围内提供 Gregarious → Solitary 回退；蝗潮自然终结靠 §5 群规模降至阈值下 → 死亡解散

### 测试

- [ ] `rat_phase_default_is_solitary`
- [ ] `pressure_sensor_density_threshold_triggers_transition`
- [ ] `pressure_sensor_low_qi_gradient_does_not_transition_alone`（密度高但 qi 平坦不触发）
- [ ] `pressure_sensor_high_qi_gradient_alone_does_not_transition`（qi 陡但鼠少不触发）
- [ ] `transitioning_phase_promotes_to_gregarious_after_duration`
- [ ] `apply_rat_phase_change_synchronizes_full_chunk_group`
- [ ] `rat_phase_change_pushes_redis_event_for_agent`
- [ ] `drained_chunk_avoid_still_works_in_gregarious_phase`（群居态仍然不回旧地）

---

## §5 P3 — 灵蝗潮天灾

### Event 扩展

- [ ] `ActiveEvent.beast_tide` 改为 enum：
  ```rust
  enum BeastTideRuntimeState {
      Wandering(WanderingTideState),    // 现有逻辑
      LocustSwarm(LocustSwarmState),    // 本 plan 新增
  }
  ```
- [ ] `LocustSwarmState`（`server/src/world/events.rs`）：
  ```rust
  struct LocustSwarmState {
      spawned_rats: Vec<Entity>,
      origin_zone: String,
      target_zone: String,        // agent 命令时塞，server fallback 用 QiDensityHeatmap argmax
      front_position: DVec3,      // 蝗锋当前位置
      front_velocity: DVec3,      // 朝 target_zone 推进
      drained_chunks: HashSet<ChunkPos>,
      group_alive: u32,
  }
  ```
- [ ] `EVENT_BEAST_TIDE` 命令 params 加 `tide_kind: "wandering" | "locust_swarm"`，缺省值 `"wandering"`（向后兼容旧 agent 命令）
- [ ] `tide_kind = "locust_swarm"` 时 spawn 数量 × 5（不是 intensity × 10）；spawn 用 `spawn_rat_npc_at`；初始 phase 直接置 `Gregarious`

### 蝗锋行进 system

- [ ] `locust_swarm_advance_system`（Update tick）：
  - 计算 `target_direction = (target_zone center - front_position).normalize()`
  - `front_position += front_velocity * dt`
  - 每个 Rat 的 `Navigator.target = front_position + jitter`（群在锋线附近 ±N 格分布）
  - 蝗锋每进入新 chunk 一次：
    - `QiDensityHeatmap[chunk] -= LOCUST_QI_DRAIN_PER_CHUNK`（默认 0.05，可调）
    - 该 chunk 内所有 `DroppedLootEntry { item: bone_coin | shu_gu | ling_cao }` despawn（worldview §八.2 "天地交易税"的极端形态）
    - 加入 `drained_chunks`
  - 蝗锋经过修士周围 N 格时，对每个 Cultivator emit RatBiteEvent × ~M（每秒扣 M qi，量小但持续）
- [ ] **解散条件**：`group_alive < DISBAND_THRESHOLD`（默认 5）或 `target_zone` qi 已 < 0.05 → emit `LocustSwarmDispersedEvent` → 全部 Rat 进入 `Hunger::starving` → 在 N tick 内陆续 die → drop shu_gu 雨

### 死亡释放（"末法淋巴"闭环）

- [ ] Rat 死亡时把吸到的 qi 等量回归到 **死亡地点的 zone qi**（不是来源 zone）——把 qi 从灵气淤积区扩散到稀薄区，物理实现"末法淋巴系统"
- [ ] 实装位：`fauna::drop::fauna_drop_system` hook 之前加 `release_drained_qi_on_death_system`

### 测试

- [ ] `beast_tide_with_tide_kind_locust_swarm_uses_locust_state`
- [ ] `beast_tide_default_tide_kind_is_wandering_for_backward_compat`
- [ ] `locust_swarm_advance_drains_qi_from_chunks_on_path`
- [ ] `locust_swarm_advance_despawns_dropped_bone_coin_in_path`
- [ ] `locust_swarm_advance_drains_cultivator_qi_via_rat_bite_when_in_radius`
- [ ] `locust_swarm_disperses_when_target_zone_qi_below_threshold`
- [ ] `locust_swarm_disperses_when_group_alive_below_threshold`
- [ ] `dispersed_locust_dies_and_drops_shu_gu_rain`
- [ ] `rat_death_releases_drained_qi_to_death_zone_not_source_zone`（"末法淋巴"闭环）

---

## §6 P4 — 天道 agent 接入

### Server → agent

- [ ] `RatPhaseChangeEvent` 序列化推 `bong:rat_phase_event` Redis 频道：
  ```json
  {
    "event": "rat_phase_change",
    "chunk": [x, z],
    "zone": "spirit_marsh",
    "from": "solitary",
    "to": "transitioning",
    "rat_count": 12,
    "local_qi": 0.42,
    "qi_gradient": 0.31,
    "tick": 12345
  }
  ```
- [ ] `world_state` 新字段 `rat_density_heatmap: Map<ZoneName, RatDensitySnapshot>`（每 zone 头数 + Phase 分布），定期推送
- [ ] schema 在 `agent/packages/schema/src/world-state.ts` TypeBox 加字段；server 端 `bong_world_state` payload 同步加

### Agent 决策（`agent/packages/tiandao/src/locust-swarm-narration.ts` 新文件）

- [ ] 参考 `tribulation-runtime.ts` 模式：
  ```ts
  // input: rat_phase_event + world_state.rat_density_heatmap
  // tools: query_zone_qi, query_player_density, query_recent_calamities
  // decision: 是否升级为跨 zone locust_swarm 天灾？
  //   - 是：emit narration ("某区域突现灵蝗大潮 ...") + spawn agent_cmd EVENT_BEAST_TIDE { tide_kind: "locust_swarm", target: <high_qi_zone> }
  //   - 否：仅记录到 world_model 等下次累积
  ```
- [ ] decision logic（worldview §八 决策模型）：
  - 高 qi 区附近的 chunk 触发 transition + 该 zone 玩家活跃 → 升级概率高
  - 蝗潮 cooldown：同一 target_zone 24 game-hour 内不重复触发
  - 全服已有进行中 calamity（thunder_tribulation / realm_collapse）→ 优先级降，不并发
- [ ] 加进 arbiter pipeline（`agent/packages/tiandao/src/arbiter.ts`）：本 narration 与现有几个 narration 平级（不互排）

### 测试

- [ ] agent 端 vitest：
  - `parses_rat_phase_event_from_redis`
  - `escalates_to_locust_swarm_when_qi_zone_and_player_density_high`
  - `skips_escalation_when_calamity_in_progress`
  - `respects_24h_cooldown_per_target_zone`
- [ ] server 端：`rat_phase_event_serializes_to_redis_payload_correctly`

---

## §7 P5 — 客户端表现 + 饱和测试收口

### 客户端

- [ ] **Phase 视觉切换**：silverfish vanilla skin 不动；用 client mod hook（`SilverfishRenderer` mixin？）按 server 推送的 `rat_phase` 给 silverfish 染色（Solitary 灰 / Transitioning 暗红 / Gregarious 黑红）
  - 暂用 entity custom name color 占位（最小代价，无需 model swap）
- [ ] **蝗潮预兆**：`bong:locust_swarm_warning` CustomPayload（参考 `realm_collapse_boundary` VFX），HUD 中央闪烁红字"灵蝗潮逼近 · 朝 <方向>" + 远处地面震动粒子（仿 §八 "天象有异"）
- [ ] **被吸真元提示**：被 RatBiteEvent 命中时 HUD 真元条闪红 + 短促"沙沙"音效

### 饱和测试

- [ ] **happy path**：单 chunk 鼠群密度爆 → transition → gregarious → 跨 zone → 解散 → drop 雨
- [ ] **边界**：
  - 0 只 Rat 的 chunk surge_intensity 恒为 0 不 NaN
  - SURGE_TRIGGER_THRESHOLD 边界 ±1 行为正确
  - target_zone 与 origin_zone 相同时（agent 误指）回退到 QiDensityHeatmap argmax
  - 蝗潮 spawn 数被 NpcRegistry budget clamp 时仍能正常推进 / 解散
- [ ] **错误分支**：
  - rat_phase_event redis push 失败时 server 不 panic（应 log + 缓存重试）
  - tide_kind 传入未知字符串时 fallback `wandering`（向后兼容）
  - LocustSwarmState 在 ActiveEvent expire 之前 group_alive 已 = 0 时正确清理
- [ ] **状态转换**：Solitary→Transitioning→Gregarious 三态切换在 phase_change event 上对拍 sample
- [ ] **e2e 集成**：spawn dense rat zone → server detect → push redis → agent decide → spawn locust_swarm → 蝗锋推进 → drain qi → 解散 → drop 雨 → 玩家 G 拾取 shu_gu 入袋

---

## §8 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|---|
| `spawn_rat_npc_at` | `server/src/npc/spawn_rat.rs`（新文件） |
| `RatBlackboard` | `server/src/npc/spawn_rat.rs` |
| `RatPhase` enum | `server/src/fauna/rat_phase.rs`（新文件） |
| `RatGroupId` component | `server/src/fauna/rat_phase.rs` |
| `PressureSensor` component | `server/src/fauna/rat_phase.rs` |
| `RatPhaseChangeEvent` Bevy event | `server/src/fauna/rat_phase.rs` |
| `LocustSwarmState` struct | `server/src/world/events.rs`（扩展 BeastTideRuntimeState 为 enum） |
| `BeastTideRuntimeState::LocustSwarm` variant | `server/src/world/events.rs` |
| `EVENT_BEAST_TIDE` `tide_kind` 参数 | `server/src/world/events.rs` `ActiveEvent::from_spawn_command` |
| `RatBiteEvent` + `apply_rat_bite_qi_drain` | `server/src/combat/events.rs` + `server/src/combat/rat_bite.rs`（新） |
| `release_drained_qi_on_death_system` | `server/src/fauna/rat_phase.rs` |
| `QiSourceProximityScorer` / `GroupCohesionScorer` / `DrainedChunkAvoidScorer` / `SeekQiSourceAction` / `RegroupAction` | `server/src/npc/brain.rs` 或新拆 `server/src/npc/brain_rat.rs` |
| Rat e2e 集成测试 | `server/src/fauna/integration_rat_pickup.rs`（新文件） |
| `bong:rat_phase_event` Redis key + payload | `server/src/redis_outbox.rs` + `agent/packages/tiandao/src/redis-ipc.ts` |
| `rat_density_heatmap` world_state 字段 | `agent/packages/schema/src/world-state.ts` + server `bong_world_state` 序列化 |
| `LocustSwarmNarration` 决策器 | `agent/packages/tiandao/src/locust-swarm-narration.ts`（新文件） |
| `bong:locust_swarm_warning` CustomPayload | `agent/packages/schema/src/client-payload.ts` + client `LocustSwarmWarningHandler` |

---

## §9 决议（2026-05-04 一次性闭环）

调研锚点：`plan-npc-skin-v1`（finished）· `cultivation/tick.rs:7`（"P1 简化无静坐区分"注释）· `npc/lod.rs NpcLodConfig`（distance-tiered LOD）· `cultivation/death_hooks.rs CultivationDeathCause` enum · `shelflife/types.rs Freshness` + `TrackState` · `agent/packages/tiandao/src/skills/calamity.md`（已含 `beast_tide` spawn_event）· `world/karma.rs` 现有 calamity 概率模型（无显式 cooldown 表）· `npc/lifecycle.rs NpcRegistry::max_npc_count = 512`。

| # | 问题 | 决议 | 落地点 |
|---|------|------|--------|
| **Q-RT-1** | EntityKind 长期方案 | ✅ **silverfish 占位 + ResourcePack 重 texture/model**。Rat 不是"假玩家"（不走 `plan-npc-skin-v1` 的 PlayerEntity 路线，那是给散修 / 弟子用的），走 client-side resource pack 替换 silverfish 渲染。无需新协议、无需 client mod 加 entity kind。Phase 视觉切换（Solitary/Transitioning/Gregarious）在 P5 通过 silverfish 上挂 ScoreboardTeam 染色（vanilla 现成机制） | P0 §2（EntityKind 选定 silverfish 不变）+ P5 §7（resource pack + ScoreboardTeam color） |
| **Q-RT-2** | 修炼苍蝇 IsMeditating | ✅ **本 plan 加 `MeditatingState` component**，但 P0 用代理信号兜底。`MeditatingState { since_tick: u64 }` 挂在玩家/NPC entity 上：① P1 代理触发——玩家位移 < 0.5 格 / sec 持续 3s + qi_current < qi_max 时自动挂；② P5 client 加 "V 键静坐" 显式触发（参考 `CultivationScreenBootstrap` 已有 K 键模式）。`QiSourceProximityScorer` 把 `MeditatingState` 命中权重 × 3.0（"修炼苍蝇" 语义） | P0 §2 加 component；P1 §3 加代理触发 system；P5 §7 客户端 V 键 |
| **Q-RT-3** | 跨 zone LOD / chunk loading | ✅ **不 pre-warm chunk，用滚动 spawn**。`LocustSwarmState.front_position` 是逻辑标量，每 tick 推进；只在锋面 ±32 格 + 玩家可见范围内维护活体 Rat 实体（`active_window_size = 80`，FIFO 滚动 despawn 锋面后方）。蝗锋穿越无玩家 zone 时不 spawn 任何实体，仅 mutate `QiDensityHeatmap`（"远方蝗潮"对玩家是看不见的环境效应，符合 worldview §八 "天象有异"叙事）。LOD 走现有 `NpcLodConfig`（按距离玩家自动 tier 降级），不改全局 | P3 §5 加 `active_window_size` + `roll_swarm_window_system`；§8 抓手补 `LocustSwarmActiveWindow` |
| **Q-RT-4** | 蝗潮 vs 其他生物 | ✅ 三档处理：**（a）遇 Cultivator NPC** — 当作 qi 源同玩家处理（NPC 也有 `Cultivation` component），RatBiteEvent 同样消费 qi；**（b）遇 HybridBeast / VoidDistorted** — 在 PressureSensor 加 `negative_pressure_avoidance`（worldview §七 "鼠畏缝合兽" + 馆藏《异兽三形考》"鼠群在缝合兽进入视野前四分之一炷香便知道了"）：HybridBeast / VoidDistorted entity 范围 24 格内的 Rat，所有 SeekScorer 评分压成 0 + 给 `FleeAction` baseline 0.7；**（c）遇散居 Rat 群** — Gregarious 蝗锋经过 Solitary 群时直接同步相位（Solitary→Gregarious 强升），群规模合并 | P1 §3 加 `negative_pressure_avoidance` 字段；P3 §5 加 `swarm_engulfs_solitary_groups_in_path` 测试 |
| **Q-RT-5** | 离体真元 / 骨币堆 | ✅ **bone_coin 不构成压差输入**（已封印 = qi 不散逸）。`PressureSensor::scan_qi_sources` 仅纳入：① `MeditatingState` entity（最高权重 × 3.0）② DroppedLootRegistry 中 `Freshness.track ∈ {Declining, Dead, Spoiled, AgePostPeakSpoiled}` 的 entry（散逸中的灵草 / 灵石碎渣，权重 × 1.0）③ `QiDensityHeatmap` chunk argmax（baseline 散源，权重 × 0.3）。Fresh 状态的 ling_cao 不算（还在主人锁区里）。这同时贴合 worldview §七 "灵气波动" + §十 "灵气散失"双意 | P1 §3 `QiSourceProximityScorer` 实现细节定义；§8 抓手补 `freshness_filter_for_qi_source` |
| **Q-RT-6** | multi_life 互动（蝗潮死法） | ✅ **加 `CultivationDeathCause::SwarmQiDrain` variant**（修改既有 `cultivation/death_hooks.rs:CultivationDeathCause` enum）。RatBiteEvent 把 player qi 扣到 0 → emit `CultivationDeathTrigger { cause: SwarmQiDrain, context: { zone, swarm_origin, swarm_kill_count } }` → 走 plan-multi-life-v1 既有 lifecycle（运数 -1 / 重生灵龛 / 寿元归零角色终结）。死亡扣寿引 `plan-lifespan-v1 §2`（被杀 5% 当前境界寿元上限），**不做特殊蝗潮死法奖惩**（worldview "末土残忍"原则）。死亡 narration 留 P4 给 agent locust-swarm-narration 处理 | P1 §3 加 SwarmQiDrain enum variant；P3 §5 加 `qi_drain_to_zero_emits_swarm_death_trigger` 测试 |
| **Q-RT-7** | Agent prompt template | ✅ **不新建 skill**（蝗潮仍是 calamity skill 下的 `beast_tide` spawn_event 子类型）。修改 `agent/packages/tiandao/src/skills/calamity.md`：①§权限 加 `beast_tide.params: { tide_kind: "wandering" \| "locust_swarm", target_zone: "zone_name" }` 字段说明 ②§决策偏好 加一行 "灵蝗潮（locust_swarm）：仅在 zone qi > 0.6 + 玩家活跃 + 同 zone 24 game-hour cooldown 已过 时考虑；与 thunder_tribulation / realm_collapse 互斥不并发"。**反向流（server 报相变事件→agent 决策升级）走 `locust-swarm-narration.ts` event handler**（不是 prompt skill），`bong:rat_phase_event` redis 消息触发 → handler 拼 calamity 上下文 → 调 calamity skill | P4 §6 文件清单更新：calamity.md edit + locust-swarm-narration.ts 新建（不新建 skill md） |
| **Q-RT-8** | 蝗潮 cooldown | ✅ **server 硬阻 + agent 软自律 双层**。新建 `LocustSwarmCooldownStore { last_swarm_by_zone: HashMap<ZoneName, u64>, cooldown_ticks: u64 = 24*3600*20 }`（默认 24 game-hour）。`ActiveEvent::from_spawn_command` 在 tide_kind=locust_swarm 时检查 cooldown 未过则拒绝命令（log warn + 不入队），允许同 zone 仍可 spawn wandering tide。配置字段允许 dev_command 覆盖。calamity.md §决策偏好 同步声明 cooldown（让 agent 别白下命令） | P3 §5 加 `LocustSwarmCooldownStore` resource + `from_spawn_command` 拒绝路径 + `locust_swarm_within_cooldown_command_rejected` 测试；P4 §6 calamity.md 同步 |
| **Q-RT-9** | 饱和度 vs FPS | ✅ **不改全局 NpcRegistry::max_npc_count: 512**（避免影响其他 NPC）。`LocustSwarmState.active_window_size: u32 = 80`（合 Q-RT-3）。spawn 时如 budget 余额不足，临时降 active_window_size 到 budget 剩余值（log info）+ 蝗锋速度 × 0.7（生态降级）。复用 `BeastTideRuntimeState` 已有 budget exhausted log 模式（`world/events.rs:1029`）。**性能压测**：P5 加 `dev_command spawn_locust_swarm` 命令 + 文档化压测步骤（200 只 spawn 下 server tick rate / client FPS）；目标 server tick > 18 / client > 50fps | P3 §5 加 `active_window_size` 字段 + `budget_exhaust_clamps_active_window` 测试；P5 §7 加 dev_command + 压测验收脚本 |

> **本 plan 无未拍开放问题**——P0 可立刻起。P5 的 cooldown 数值 / 蝗锋速度 / window size 是配置项可后期调，不阻塞落地。

---

## §10 进度日志

- **2026-05-04 立项**：骨架立项。来源：用户灵感 = locust phase polyphenism 映射噬元鼠。调研：worldview §七 §八 §十 + 馆藏《异兽三形考》（藏荒散修九年观鼠）+ plan-fauna-v1（已归档，drop 链路完备）+ 现有 EVENT_BEAST_TIDE 框架（`world/events.rs:40,684-730`）+ G 拾取链路（`InteractionKeybindings.G` + `DroppedLootRegistry`）已成熟。**关键缺口**：FaunaTag::Rat 仅用于 drop loot table，没有任何 Rat 专属 entity 本体 / AI / 行为。本 plan 从零落地。
- **2026-05-04 决议闭环**：§9 9 条开放问题一次性 [x]：Q-RT-1 EntityKind 走 ResourcePack 重 silverfish texture（不走 npc-skin PlayerEntity） · Q-RT-2 加 `MeditatingState` component（P0 代理触发 + P5 V 键显式） · Q-RT-3 滚动 spawn `active_window_size=80` 不 pre-warm chunk · Q-RT-4 蝗潮三档互动（NPC 当 qi 源 / HybridBeast 强避让 / 散居 Rat 群被卷入升相） · Q-RT-5 PressureSensor 仅纳入 MeditatingState + freshness 散逸态 + zone qi argmax，**bone_coin 不构成压差源** · Q-RT-6 加 `CultivationDeathCause::SwarmQiDrain` variant 走 plan-multi-life-v1 既有 lifecycle · Q-RT-7 不新建 calamity skill，改 `calamity.md` 加 tide_kind 字段 + 反向流走 `locust-swarm-narration.ts` event handler · Q-RT-8 双层 cooldown（server `LocustSwarmCooldownStore` 24 game-hour 硬阻 + agent calamity.md 软自律） · Q-RT-9 不改全局 NpcRegistry budget，蝗潮窗口 budget 不足时临时降 window size + 锋速 ×0.7。本 plan 无未拍开放问题，P0 可起。
