# Bong · plan-npc-perf-v1 · 骨架

NPC 系统性能恢复专项 —— 100 rogue seed 让单核 WSL2 TPS 跌至 **0.7（30× 慢）**，所有玩家 packet 卡几秒。`plan-npc-ai-v1` Phase 9 已实装 `NpcLodConfig.reassess_interval = 20` + 3 核 scorer 的 `should_skip_scorer_tick` gate + Dormant tier，但下列**四类没被 LOD 覆盖的热点**仍把单 tick 阻塞拉到 1380ms：(1) `faction::assign_hostile_encounters` 真 O(N²) 10000 距离比较/tick，无任何 spatial index；(2) `socialize_scorer_system` / `territory_intruder_scorer_system` Near tier 内仍 O(N²) 全扫 peer / candidate；(3) `navigator_tick_system` 共享 `repath_countdown=20`，所有 NPC 同 tick 归零 → 单 tick 同时跑 100 次 A*（每次 ≤400 迭代 × ChunkLayer 查询）；(4) `qi_regen_and_zone_drain_tick` / `patrol_npcs` / `update_npc_blackboard` / `tribulation_ready_scorer` 每 tick 全 NPC 跑、每 NPC 双 zone hashmap lookup + O(NPC×Player) 双层循环。**短期权宜 = `BONG_ROGUE_SEED_COUNT=0`**（CI e2e + 默认 dev），本 plan = 让默认 100 重新跑得动。

**交叉引用**：`plan-npc-ai-v1.md §0 性能优先 / Phase 9 LOD` ✅（前置）· `plan-server.md` 基建 · `plan-agent-v2.md`（NpcDigest 远方压缩，已实装）· `worldview.md §三:124 NPC 与玩家平等`（性能恢复后 NPC 起渡虚劫 / 派系战争才能正常推演，否则 agent 长期决策无 NPC 数量基础）

**worldview 锚点**：本 plan 是**基建恢复**，不引入新玩法，不写新真元 / 灵气常数。仅恢复 plan-npc-ai-v1 已设计的 `max_npc_count = 512` 路径所必需的算力——目前 100 NPC 已不可承载，512 完全无望。worldview §三 「NPC 与玩家同规则」依赖足够多 NPC 同时 active 才能体现派系/师承/截胡的世界感。

**qi_physics 锚点**：无（基建 plan，不动真元 / 守恒律 / 衰减常数）。

**前置依赖**：

- `plan-npc-ai-v1` ✅ → NpcLodTier / NpcLodConfig / `should_skip_scorer_tick` gate / 6 archetype Bundle / 13 Scorer + 15 Action 注册 / NpcRegistry / FactionStore / `bong:npc/spawn,death` Redis 通道 / `BONG_ROGUE_SEED_COUNT` env 占位
- `plan-server.md` ✅ → ScheduleRunnerPlugin / Bevy 0.14 Update + FixedUpdate 调度 / Position↔Transform sync 桥
- `plan-agent-v2.md` ✅ → NpcDigest 远方压缩 / publish_world_state_to_redis 每 200 tick

**反向被依赖**：

- `plan-npc-ai-v1.md` 后续工作 / Phase 9 完成 → 1000 NPC × 50 玩家压测留待本 plan
- `plan-fauna-v1` ✅ / `plan-lingtian-npc-v1` ✅（活跃 NPC 密度提高的玩法 plan）
- 任何后续派系/师承/任务玩法（plan-quest-v1 等占位）—— 都依赖 100+ NPC 可同时 active

---

## 接入面 Checklist

- **进料**：
  - `Position`（所有 NPC 当前坐标）+ `NpcMarker` filter（Bevy ECS Query）
  - `NpcLodTier` ✅（plan-npc-ai-v1 Phase 9，Active/Near/Dormant 三档）
  - 现有 13 Scorer + 15 Action 注册表（`brain.rs` / `social.rs` / `territory.rs` / `faction.rs` / `relic.rs` / `farming_brain.rs`）
  - `navigator_tick_system` 内部状态 `repath_countdown` / `path` Vec
  - `qi_regen_and_zone_drain_tick` / `patrol_npcs` / `update_npc_blackboard` 现 schedule = `Update`
- **出料**：
  - `NpcSpatialIndex` Resource（按 32 格 cell 散列 NPC entity，每 tick 重建一次或增量更新；O(N) 重建 / O(k) 邻居查询）
  - 改造后的 `assign_hostile_encounters` / `socialize_scorer_system` / `territory_intruder_scorer_system` / `relic::guard*` 全部走 SpatialIndex 邻居查询
  - `navigator_tick_system` 分桶逻辑：`if (entity_index + tick) % BUCKET_COUNT == 0 { repath }`，BUCKET_COUNT = 20
  - `qi_regen_and_zone_drain_tick` / `patrol_npcs` / `update_npc_blackboard` 迁 `FixedUpdate`（5-10Hz）
  - 已遗漏 LOD gate 的 scorer（ChaseTargetScorer / MeleeRangeScorer / DashScorer / CultivationDriveScorer / TribulationReadyScorer）补 `should_skip_scorer_tick` 接入
  - `sync_position_to_transform` 冗余写删除（navigator 已写 Transform，无需 PostUpdate 再覆盖）
  - **新增 telemetry**：`NpcPerfProbe` Resource 记录每个热点 system 的 µs/tick，便于回归监控（`tracing::info!` 每 200 tick 一行，跟现有 `TickRateProbe` 同节奏）
- **共享类型 / event**：
  - 复用 `NpcLodTier` / `NpcLodConfig`（不新建 LOD 概念）
  - 复用 `NpcSpatialIndex` 于所有空间查询 system（**禁止各 scorer 自己造一份**——孤岛红旗）
  - 不新增 Schema / Component / Event（纯内部优化）
- **跨仓库契约**：
  - server: `npc::spatial::*` 新模块 / 现有 system 改造，无对外 schema 变化
  - agent: 无（NpcDigest / world_state 仍走原通道，本 plan 不动 publish 逻辑）
  - client: 无（行为可见结果不变；玩家观感 = 之前 0.7 TPS → 18+ TPS）
- **worldview 锚点**：见头部，基建 plan
- **qi_physics 锚点**：无

---

## §0 设计轴心

- [ ] **本 plan 不引入新玩法 / 不改 NPC 行为表达**：所有 scorer / action 的语义保持不变。仅改"如何高效执行同一行为"。如果需要砍掉某个 scorer，**先回 plan-npc-ai-v1 改设计**，本 plan 不做行为侧裁剪
- [ ] **空间索引是单一来源**：所有 O(N²) 热点共用同一个 `NpcSpatialIndex` Resource。**禁止各 scorer 自建一份 KdTree / Grid**（孤岛红旗）。每 tick 由专用 system 重建（`PreUpdate` 阶段，在所有 scorer 之前）
- [ ] **节流系统改 FixedUpdate 而非"加 if tick % N"**：cultivation tick / patrol / blackboard 改 FixedUpdate(5-10Hz) 而不是手写 `if tick % 4 == 0`，是因为 Bevy FixedUpdate 调度器已处理累积时间不丢 tick + 测试断言更清晰。**例外**：navigator 分桶因要保留 entity-id 错峰，仍走 Update + 内部分桶
- [ ] **LOD gate 不一刀切**：保留 `Near` tier 的 scorer 全跑（玩家附近的 NPC 行为不能延迟），仅`Far` / `Dormant` tier 跳过或降频。但**所有 scorer 都必须挂 `should_skip_scorer_tick` 接口**（即使 Far/Dormant 不跳，也要显式调一次返回 false，防止"漏挂 LOD"红旗）
- [ ] **可重入压测基线**：本 plan 的"完成"定义 = 100 NPC + 1 玩家在 spawn 区域附近 5min，TPS ≥ 18。**测试矩阵 §7 必须含此压测脚本**，不允许仅靠"本地跑了一下"验收
- [ ] **回归保护**：plan-npc-ai-v1 Phase 9 已经因为 e2e CI TPS 回归撤回过 beast/disciple/relic 的 thinker 注册。本 plan 必须**先恢复 100 NPC 默认 + 跑通 e2e**，再考虑 stretch goal 1000 NPC

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | **决策门 + 度量基线**：NpcSpatialIndex 数据结构定稿（HashMap<(i32,i32), SmallVec<[Entity; 8]>> vs flat Vec<bucket>）+ FixedUpdate 频率定值（5Hz vs 10Hz）+ navigator BUCKET_COUNT 定值（10/20/30）+ NpcPerfProbe telemetry 落地（per-system µs/tick）+ 回归基线录档（`BONG_ROGUE_SEED_COUNT=100` 当前 TPS / 各热点 µs，写入 plan §9 进度日志） | 数据结构 + 常数 + telemetry + baseline 全落 plan §2 / §9 |
| **P1** ⬜ | **空间索引底盘**：`server/src/npc/spatial.rs` 新模块（NpcSpatialIndex + rebuild system + neighbor query API）+ 改造 4 个 O(N²) 热点（faction::assign_hostile_encounters / socialize_scorer_system / territory_intruder_scorer_system / relic::guardian_duty_scorer_system）走邻居查询 + ≥30 单测（rebuild 正确性 / 邻居查询正确性 / 4 个 scorer 行为不变 vs 暴力对照） | `cargo test npc::spatial` 全过 / 4 scorer 行为对照测试 / 100 NPC TPS 实测 ≥ 12 |
| **P2** ⬜ | **navigator A\* 分桶**：`navigator_tick_system` 改造（按 `(entity_index + tick) % BUCKET_COUNT == 0` 错峰 repath）+ stuck 检测保留（不影响分桶）+ 删除 `sync_position_to_transform` 冗余写 + ≥15 单测（分桶正确性 / 单 NPC 路径行为不变 / stuck 强制 repath 仍生效）| 100 NPC TPS 实测 ≥ 16 / 单 NPC 行为对照（pathfinding 测试矩阵原有用例全过）|
| **P3** ⬜ | **per-NPC 系统降频 + LOD 补漏**：qi_regen_and_zone_drain_tick / patrol_npcs / update_npc_blackboard / lifespan_aging_tick 迁 FixedUpdate(5Hz) + ChaseTargetScorer / MeleeRangeScorer / DashScorer / CultivationDriveScorer / TribulationReadyScorer 接 `should_skip_scorer_tick`（Far/Dormant 跳过）+ NpcPerfProbe 验证 µs/tick 下降 ≥ 70% | 100 NPC TPS 实测 ≥ 18 / 5Hz 节流不影响 NPC 老化 / 突破 / patrol 流畅性（人工验收 5min 录像）|
| **P4** ⬜ | **回归保护 + 默认值恢复**：`scripts/start.sh` 默认 `BONG_ROGUE_SEED_COUNT=100`（不再是 0）+ CI e2e 100 NPC 跑通 ≥ 18 TPS + 1000 NPC stretch goal 实测（不强制达标，记录到 plan §9）+ 回归测试加入 CI（每 PR 跑 100 NPC 30s 压测，TPS < 15 阻塞 merge）| start.sh 默认 100 / CI e2e green / 回归门禁 active |

---

## §2 数据模型

```rust
// server/src/npc/spatial.rs（新模块）

#[derive(Resource, Default)]
pub struct NpcSpatialIndex {
    /// 按 cell_size=32 散列的 NPC entity 列表
    cells: HashMap<(i32, i32), SmallVec<[Entity; 8]>>,
    cell_size: f64,
}

impl NpcSpatialIndex {
    pub const DEFAULT_CELL_SIZE: f64 = 32.0;

    /// O(N) 全量重建（PreUpdate 每 tick 跑一次；增量更新留 P1 决策）
    pub fn rebuild(&mut self, npcs: &Query<(Entity, &Position), With<NpcMarker>>);

    /// O(k) 邻居查询（k = 周围 9 cell 内 NPC 数；半径 ≤ cell_size 时仅 4-9 cell）
    pub fn neighbors_within(&self, center: DVec3, radius: f64) -> impl Iterator<Item = Entity>;
}

// server/src/npc/perf.rs（新模块）

#[derive(Resource, Default)]
pub struct NpcPerfProbe {
    /// 每 system 的累计 µs，每 200 tick log 一次
    samples: HashMap<&'static str, (u64 /* total_us */, u64 /* count */)>,
    last_log_tick: i64,
}

impl NpcPerfProbe {
    pub fn record(&mut self, system_name: &'static str, dur_us: u64);
    pub fn flush_if_due(&mut self, current_tick: i64);
}

// server/src/npc/lod.rs 扩展（plan-npc-ai-v1 已存在，仅补接口）

pub fn should_skip_scorer_tick(
    tier: &NpcLodTier,
    scorer_kind: ScorerKind, // 新增 enum：Critical / Standard / Cosmetic
    tick: u64,
) -> bool;
```

**FixedUpdate 频率**：暂定 5Hz（200ms / 跨 4 个 Update tick）。P0 决策门可调到 10Hz。

**navigator BUCKET_COUNT**：暂定 20（同 `REPATH_INTERVAL_TICKS`，每 tick 5 NPC repath，平均延迟 10 tick = 0.5s）。P0 决策门可调。

---

## §3 空间索引强约束（CLAUDE.md 风格规则）

> **本节是后续所有 NPC scorer / action plan 必守的底盘约束**。任何新增 scorer / action 涉及"找附近的 X"必须用 `NpcSpatialIndex::neighbors_within`，**禁止自己写 query iter 全扫**。

### 强约束规则

1. **新增 scorer / action 涉及空间查询 → 必须用 NpcSpatialIndex**（不允许 `npcs.iter().filter(|n| dist(n) < r)`）
2. **不允许 plan 内自建 KdTree / R-tree / Grid**（孤岛红旗，docs/CLAUDE.md §四 应加一条）
3. **若需查"非 NPC 的 entity"**（如玩家、灵田、ore）→ 留待派生 plan-spatial-index-v2 通用化（本 plan 仅覆盖 NPC↔NPC 查询）
4. **rebuild 频率不可调高**：固定每 tick rebuild（O(N) 不贵，N=512 时约 0.05ms）。增量更新 hold 到 1000+ NPC 阶段再做

### 已知豁免（本 plan 不动）

- `cultivation::tick::qi_regen_and_zone_drain_tick`：用 zone 哈希查询而非空间索引（zone 是预先生成的 region，非动态点）
- `update_npc_blackboard`：要扫所有玩家（玩家数 ≤ 50，O(NPC × Player) 与 P3 FixedUpdate 节流足以应对）

---

## §4 热点系统改造清单

> 5 路 sonnet agent 探查（2026-05-07）输出，按改造优先级排序。

| # | 模块 | 文件:行 | 当前复杂度 | 改造方案 | 阶段 |
|---|---|---|---|---|---|
| 1 | `faction::assign_hostile_encounters` | npc/faction.rs:527 | O(N²)，每 tick 10000 距离比较，无 LOD | 走 NpcSpatialIndex::neighbors_within（半径 16 格）→ O(N×k)，k≈5 | P1 |
| 2 | `socialize_scorer_system` | npc/social.rs:150 | O(N²)，Near tier 仍全扫 peer | 走 SpatialIndex + Near tier 内仍仅查 16 格邻居 | P1 |
| 3 | `territory_intruder_scorer_system` | npc/territory.rs:416 | O(N²)，Beast 扫所有候选 | 走 SpatialIndex + Territory.radius 上限 | P1 |
| 4 | `relic::guardian_duty_scorer_system` | npc/relic.rs:176 | O(G×N)，G 小但仍全扫 NPC | 走 SpatialIndex + GuardianDuty.alarm_radius | P1 |
| 5 | `navigator_tick_system` | npc/navigator.rs:237 | 共享 repath_countdown=20，同 tick 100 A* | `(entity_index + tick) % 20 == 0` 分桶 | P2 |
| 6 | `sync_position_to_transform` | npc/sync.rs:7 | navigator 已写 Transform，PostUpdate 冗余写 | 删除，navigator 内部直接写 | P2 |
| 7 | `qi_regen_and_zone_drain_tick` | cultivation/tick.rs:70 | 每 NPC 双 zone hashmap lookup × 每 tick | 迁 FixedUpdate(5Hz)；逻辑乘 4 倍速 | P3 |
| 8 | `patrol_npcs` | npc/patrol.rs:64 | 每 tick 无节流 + zone lookup × 100 | 迁 FixedUpdate(5Hz) | P3 |
| 9 | `update_npc_blackboard` | npc/brain.rs:615 | O(NPC × Player) 双层嵌套，每 tick | 迁 FixedUpdate(10Hz)（blackboard 是 scorer 上游，不能太慢）| P3 |
| 10 | `tribulation_ready_scorer_system` | npc/brain.rs:1709 | 6 组件 + 玩家迭代，无 LOD | 接 should_skip_scorer_tick + Far/Dormant 跳过 | P3 |
| 11 | `lifespan_aging_tick` | cultivation/lifespan.rs:388 | 每 NPC zone lookup + 季节修正 + 死亡判断 | 迁 FixedUpdate(1Hz)（寿元秒级精度足够）| P3 |
| 12 | `update_npc_registry` | npc/lifecycle.rs:482 | PreUpdate 全扫重建 HashMap | 迁 FixedUpdate(1Hz） | P3 |
| 13 | ChaseTargetScorer / MeleeRangeScorer / DashScorer / CultivationDriveScorer | brain.rs 多处 | 无 LOD gate | 接 should_skip_scorer_tick（Cosmetic 类，Far 跳过）| P3 |

---

## §5 telemetry 与 baseline 录档

P0 决策门必须先录基线：

```
2026-05-07 baseline（BONG_ROGUE_SEED_COUNT=100, 1 player at spawn, WSL2 单核）：
  TPS = 0.7 (target 20.0)
  per-system µs/tick（top 10）:
    [filled in P0]
  per-tick alloc count: [filled in P0]
```

NpcPerfProbe 用 `std::time::Instant::now()` + `Duration::as_micros()` 包住每个热点 system fn 入口/出口，每 200 tick `tracing::info!` 一次：

```
[npc-perf] tick 200: faction_hostile=12450µs (62 calls) social_scorer=8230µs (200 calls) navigator=15600µs (1 spike) ...
```

**P3 验收门禁**：每个 P1/P2/P3 改造完成时，比对 baseline 看 µs/tick 下降幅度。降幅 < 50% 视为该项失败 → 回设计。

---

## §6 客户端新建资产

无（纯 server 内部优化，client 无可见变化）。

---

## §7 测试矩阵（饱和化）

下限 **60 单测 + 1 e2e 压测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `NpcSpatialIndex` | rebuild 正确性（empty / 1 / N） / cell 边界 / 邻居查询包含半径 / 不漏不重 / 跨 cell 半径 | 12 |
| `assign_hostile_encounters 改造` | 暴力 O(N²) vs 空间索引结果完全一致（多种 NPC 分布）| 8 |
| `socialize_scorer_system 改造` | 同上对照 + Near/Far/Dormant tier 行为表 | 8 |
| `territory_intruder_scorer_system 改造` | 同上对照 + Territory.radius 上限正确 | 6 |
| `relic::guardian_duty_scorer_system 改造` | 同上对照 + alarm_radius 正确 | 4 |
| `navigator 分桶` | 分桶正确性 / 单 NPC 路径行为对照不变 / stuck 强制 repath 跳过分桶 | 8 |
| `FixedUpdate 节流` | qi_regen 5Hz 下灵气总量收支平衡（每 5 tick 收支 == 之前 5 tick 累积）/ patrol 5Hz 下不卡死 / blackboard 10Hz 下 scorer 不受影响 | 8 |
| `LOD gate 补漏` | 5 个新接入 scorer 的 Far/Dormant 跳过行为 | 6 |
| **e2e 压测** | `BONG_ROGUE_SEED_COUNT=100` + 1 player 5min，TPS ≥ 18 / 操作（drop/pickup/cmd/chat）延迟 < 200ms | 1（重） |

**P1 验收**：`grep -rcE '#\[test\]' server/src/npc/spatial.rs server/src/npc/faction.rs server/src/npc/social.rs server/src/npc/territory.rs server/src/npc/relic.rs` ≥ 38。

**P4 验收**：CI e2e 跑 100 NPC 30s，TPS ≥ 15，否则 PR 阻塞。

---

## §8 开放问题 / 决策门

### #1 cell_size 选 16 / 32 / 64？

- **A** = 16：邻居查询命中率高（faction.rs hostile 半径 16 刚好对齐），但 cell 数多 = HashMap 重建成本略升
- **B** = 32：折中；neighbor 通常需查 4-9 cell；rebuild O(N) 平均 0.05ms（N=512）
- **C** = 64：cell 数最少 rebuild 最快，但邻居查询要扫更多 NPC（半径 16 时仍要看 4-9 cell，浪费）

**默认推 B（32）** —— 跟 worldgen chunk 对齐 1/2，邻居半径 ≤16 时只扫 4 cell。

### #2 NpcSpatialIndex 是否要支持非 NPC entity（玩家 / 灵田 / ore）？

- **A**：仅 NPC（本 plan 范围）
- **B**：扩展为通用 SpatialIndex<E: Component>（派生 plan-spatial-index-v2）
- **C**：本 plan 用泛型，但仅注册 NPC 一种使用

**默认推 A** —— YAGNI；玩家数 ≤ 50 不需要索引；灵田/ore 是静态可用 BTreeMap。需要时再 v2。

### #3 FixedUpdate 频率：5Hz / 10Hz？

- **A**：5Hz（qi_regen/patrol/blackboard 全 5Hz）
- **B**：10Hz（blackboard 单独 10Hz，其他 5Hz；blackboard 是 scorer 上游不能太慢）
- **C**：跟 NpcLodTier 联动（Active=10Hz / Near=5Hz / Far=1Hz）

**默认推 B** —— blackboard 决定 PlayerProximityScorer 触发延迟，10Hz 上限 100ms 玩家可接受；其他 200ms 可。

### #4 navigator BUCKET_COUNT：10 / 20 / 30？

- **A** = 10：每 tick 10 NPC repath，平均延迟 5 tick = 250ms（最坏 500ms）
- **B** = 20：每 tick 5 NPC，平均 500ms / 最坏 1s
- **C** = 30：每 tick 3-4 NPC，平均 750ms / 最坏 1.5s

**默认推 A（10）** —— 平均 250ms 玩家几乎不感知；TPS 单 tick A* 成本从 100× 降至 10×。

### #5 是否在 docs/CLAUDE.md §四 红旗加一条「scorer 内 npcs.iter() 全扫」？

- **A**：加（强约束化，跟 qi_physics / meridian_severed 一致格调）
- **B**：仅在本 plan §3 内强约束

**默认推 A** —— 防止后续派生 plan 又造一个 O(N²) 热点；底盘约束应升级到项目级。

---

## §9 进度日志

- **2026-05-07** 骨架立项。源自 100 rogue seed 让 TPS 跌至 0.7 的实测（用户操作如 drop/pickup/cmd/chat 全部延迟数秒）+ 5 路 sonnet agent 并行探查（big-brain 拓扑 / pathfinding / social-faction O(N²) / cultivation-lifespan / redis bridge）输出热点清单：
  - 4 个真 O(N²)：faction::assign_hostile_encounters / socialize_scorer / territory_intruder_scorer / relic::guardian_duty_scorer
  - 1 个 navigator A* 风暴：共享 repath_countdown=20 → 同 tick 100 A*
  - 5+ 个 per-NPC tick lookup：qi_regen / patrol / blackboard / tribulation_ready / lifespan_aging
  - 1 个冗余 sync：sync_position_to_transform PostUpdate 重写 navigator 已写过的 Transform
  - 已洗清非瓶颈：Redis bridge（独立 OS 线程 + crossbeam channel，主线程零 IO）/ big-brain 调度本身（FirstToScore 短路 2-5ms）/ lifecycle 多数 system / contamination / overload / tribulation auto wave
- 现状对齐：plan-npc-ai-v1 ✅ Phase 9 LOD 已实装（reassess_interval=20 + 3 核 scorer gate + Dormant），但仅覆盖 PlayerProximityScorer / FearCultivatorScorer / HungerScorer / WanderScorer 4 个，且 faction/social/territory 的 O(N²) 完全在 LOD gate 外
- 短期权宜：`scripts/start.sh` 默认 `BONG_ROGUE_SEED_COUNT=0`（CI e2e 同），本 plan 验收 = 恢复默认 100 + TPS ≥ 18

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：

- **落地清单**：`server/src/npc/spatial.rs` 新模块 + `server/src/npc/perf.rs` telemetry + 13 个热点 system 改造（§4 表格）+ FixedUpdate 节流 6 个 + LOD gate 补漏 5 个 + scripts/start.sh 默认 100 恢复
- **关键 commit**：P0/P1/P2/P3/P4 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test npc::spatial npc::perf cultivation::tick` 数量 / e2e 100 NPC 30s ≥ 18 TPS / per-system µs/tick 降幅 ≥ 70%
- **跨仓库核验**：server `npc::spatial::*` + 13 system 改造 / agent 无变化（NpcDigest / world_state 通道 unchanged）/ client 无变化
- **遗留 / 后续**：
  - 1000 NPC stretch goal（本 plan §1 P4 不强制达标，记录基线）→ 派生 `plan-npc-perf-v2`（增量索引 / Bevy 0.15 Schedule API / SIMD 距离比较）
  - 通用 SpatialIndex<E: Component>（玩家 / 灵田 / ore） → 派生 `plan-spatial-index-v2`
  - docs/CLAUDE.md §四 红旗加「scorer 内 npcs.iter() 全扫」一条（决策门 #5 = A 时）
  - reminder.md 登记本骨架（已转为独立骨架 2026-05-07 列表项）
