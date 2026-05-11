# Bong · plan-npc-daily-life-v1 · NPC 日常生活循环

NPC 从"反应式机器人"变成"有生活节奏的人"。两个核心改动：① 时段日程表（早/中/晚/夜 × 行为权重）② 目的地驱动漫游（去灵草点/水源/交易点/休息点，不是随机走）。**只在玩家附近时运行完整日程**——远处 NPC 简化为状态 tick，后台只维护数值一致性。

**当前问题**：
- NPC 行为全靠 Scorer 实时评分，无时间节奏——看起来像"无目的地在晃"
- WanderAction 是 4-10 格随机方向——不是"我要去那边看看"
- 无"回家"概念——散修永远在外面游荡，不会找地方休息
- 无"摆摊/等待"——NPC 不会主动在某处等玩家来交易

**世界观锚点**：
- `worldview.md §七` 散修状态机——"拾荒散修游荡在各大区域之间，靠近时评估你的威胁度"。但"游荡"不是"无目的随机走"——散修在灵草丛蹲守、在灵气高的地方打坐、在交通要道等着卖情报
- `worldview.md §十四` 一个虚构玩家的一天——NPC 也该有类似的日循环
- `worldview.md §七` 枯骨休眠——NPC 真元耗尽时自封为骸骨，有"进入休眠"行为

**前置依赖**：
- `plan-npc-ai-v1` ✅ → big-brain 行为树框架 + Scorer/Action 体系
- `plan-npc-perf-v1` ✅ → LOD 三层（Near/Far/Dormant）+ 100 NPC 性能基线
- `plan-npc-fixups-v3` ✅ → navigator idle 重力 / spawn Y snap
- `plan-lingtian-npc-v1` ✅ → 灵田 farming brain（日常行为参考实现）
- `plan-poi-novice-v1` ✅ → 兴趣点系统（POI = 日程目的地来源）

**反向被依赖**：
- `plan-npc-interaction-polish-v1` ⬜ active → NPC 在"摆摊"状态时才触发交易 UI
- `plan-sou-da-che-v1` ⬜ active → NPC 行为作为风险信号（散修突然蹲伏 = 附近有威胁）
- `plan-pvp-encounter-v1` ⬜ active → NPC 巡逻路径暴露区域情报

---

## 接入面 Checklist

- **进料**：`npc::NpcBlackboard`（大脑黑板）/ `npc::Navigator`（A* 寻路）/ `npc::Hunger`（饱食度）/ `npc::NpcLifespan`（寿元）/ `npc::NpcPatrol`（巡逻点）/ `cultivation::Cultivation`（真元/境界）/ `world::zone::ZoneRegistry`（区域灵气）/ `world::season::SeasonState`（季节）/ LOD 层级（`NpcLodLevel`）
- **出料**：`NpcDailySchedule` component（时段日程）/ `NpcHomeBase` component（居住点）/ `PointOfInterest` 扩展（日程目的地类型）/ 修改 `WanderAction` → `GoToPoiAction`（目的地驱动）/ 新增 `RestAction` / `StallAction` / `ReturnHomeAction` / 时段切换 event
- **共享类型 / event**：复用 `NpcArchetype`（不新建）/ 复用 `NpcBlackboard`（扩展字段）/ 新增 `DayPhase` enum / 新增 `NpcScheduleChangedEvent`
- **跨仓库契约**：server + agent/schema（POI wire literal 扩展与 TypeBox pin），client 无改动——NPC 动作沿用既有渲染管线。
- **worldview 锚点**：§七 散修状态机 + §十四 世界的一天
- **qi_physics 锚点**：不涉及（NPC 修炼吸收灵气已在 CultivateAction 中走 qi_physics）

---

## §0 设计轴心

- [ ] **性能第一：只在观众面前"演戏"**。完整日程只在 Near（≤64 格）运行。Far 层简化为每分钟状态 tick。Dormant 层纯数值推演。NPC 的"生活"是给玩家看的表演，不是后台仿真
- [ ] **时段不是时钟**：不引入真实 in-game 时钟。用 `CultivationClock.tick` 对 `DAY_TICKS`（一天 tick 数）取模，分 4 个时段。时段转换对 NPC 是渐变（±200 tick 随机偏移），不是全体同时切换
- [ ] **目的地驱动，不是随机走**：NPC 漫游时从附近 POI 中选一个目的地（灵草丛/水源/交易点/休息岩），走过去做对应行为。没有 POI 时才退化为随机漫游
- [ ] **散修有"家"**：每个散修 NPC 有一个 `HomeBase`（休息点），夜间或真元低时回家。家 = 某个隐蔽的方块位置（岩洞/树下/废墟角落），不是灵龛（NPC 没有灵龛）
- [ ] **行为可读**：玩家远距离观察 NPC 就能推断"这人在采草"/"这人在打坐"/"这人在摆摊"，不需要 UI 提示。行为本身就是信息

---

## §1 时段日程系统

### DayPhase（4 时段）

```rust
pub enum DayPhase {
    Dawn,      // 黎明（0-25% day_tick）：出门，去采集点
    Day,       // 白天（25-60% day_tick）：主要活动（采集/修炼/交易）
    Dusk,      // 黄昏（60-80% day_tick）：收尾，开始回家
    Night,     // 夜晚（80-100% day_tick）：休息/打坐/警戒
}
```

`DAY_TICKS` = `20 * 60 * 20` = 24000 tick（MC 一天 = 20 分钟真实时间）。

### NpcDailySchedule

```rust
#[derive(Component)]
pub struct NpcDailySchedule {
    pub phase_weights: HashMap<DayPhase, Vec<(ScheduleActivity, f64)>>,
    pub phase_offset_ticks: i32,  // ±200 随机偏移，防止全体同时切换
}

pub enum ScheduleActivity {
    Forage,       // 采集（去灵草/矿点）
    Cultivate,    // 修炼（去灵气高的地方打坐）
    Trade,        // 摆摊（在交通要道等待）
    Patrol,       // 巡逻（沿路径走）
    Rest,         // 休息（回家/找隐蔽处蹲着）
    Socialize,    // 社交（找其他 NPC 聊天）
    Wander,       // 漫游（无目的闲逛，兜底）
}
```

### 散修默认日程

| 时段 | 行为权重 | 叙事 |
|------|---------|------|
| Dawn | Forage 0.5 / Cultivate 0.3 / Wander 0.2 | 早起出门，趁灵气还没被别人吸干 |
| Day | Trade 0.3 / Forage 0.3 / Cultivate 0.2 / Socialize 0.1 / Wander 0.1 | 白天活动高峰 |
| Dusk | Rest 0.4 / Forage 0.3 / Cultivate 0.2 / Wander 0.1 | 开始回家，路上顺手采一把 |
| Night | Rest 0.6 / Cultivate 0.3 / Patrol 0.1 | 休息为主，警觉的散修会夜修 |

### 日程对 Scorer 的影响

不新建 Scorer——修改现有 Scorer 的评分，乘以时段权重：

```rust
// 原来
let wander_score = 0.08; // 固定低分兜底

// 改为
let activity = schedule.current_activity(clock.tick);
let wander_score = match activity {
    ScheduleActivity::Wander => 0.08 * schedule.weight(phase, Wander),
    _ => 0.02, // 不在漫游时段，降低漫游评分
};
```

已有 Scorer 的时段权重注入：
- `WanderScorer` → `Wander` 权重
- `CultivationDriveScorer` → `Cultivate` 权重
- `HungerScorer` → `Forage` 权重
- `CuriosityScorer` → 保持不变（好奇心不受时段影响）
- 新增 `TradeStallScorer` → `Trade` 权重
- 新增 `ReturnHomeScorer` → `Rest` 权重

---

## §2 目的地驱动漫游

### 改造 WanderAction → GoToPoiAction

现有 `WanderAction`：随机选 4-10 格内方向走。

改为 `GoToPoiAction`：
1. 查询当前 `ScheduleActivity` → 对应 POI 类型
2. 从 `PoiRegistry` 找最近的匹配 POI（64 格搜索半径）
3. 用 `Navigator` A* 走过去
4. 到达后执行对应行为（采集/打坐/摆摊/休息）
5. 无匹配 POI → 退化为原版随机漫游

### POI 类型扩展

现有 `plan-poi-novice-v1` ✅ 已有 POI 系统。扩展 POI 类型：

```rust
pub enum PoiKind {
    // 已有
    SpawnPortal,
    NoviceTutor,
    // 新增
    HerbPatch,        // 灵草丛（NPC 采集目的地）
    QiSpring,         // 灵气泉/高浓度点（NPC 修炼目的地）
    TradeSpot,        // 交通要道/路口（NPC 摆摊目的地）
    ShelterSpot,      // 隐蔽休息点（岩洞/树下/废墟，NPC 回家目的地）
    WaterSource,      // 水源（NPC 饮水/洗漱——纯装饰行为）
}
```

POI 注册来源：
- worldgen 结构生成时自动注册（灵草丛 = `HerbPatch` / 灵泉 = `QiSpring`）
- 手动标注（交易路口 / 休息点由 terrain profile 定义）

### 行为到达后的动作

| 到达 POI | 执行 Action | 时长 | 完成后 |
|---------|-----------|------|--------|
| HerbPatch | 弯腰采集动画（复用 farming TillAction 姿态）| 5-15s | 背包 +灵草（NPC 内部 inventory） |
| QiSpring | 打坐修炼（复用 CultivateAction）| 30-120s | qi_current 回复 |
| TradeSpot | 原地蹲守等待（新 StallAction：面朝路口方向站定）| 60-300s | 有玩家靠近 → 触发交易评估 |
| ShelterSpot | 靠墙/蹲下休息（新 RestAction：减速+hunger 回复加速）| 120-600s | hunger 满 → 结束休息 |
| WaterSource | 走到水边停顿 3s → 继续 | 3s | 纯装饰，无机制效果 |

---

## §3 NPC 居住点（HomeBase）

```rust
#[derive(Component)]
pub struct NpcHomeBase {
    pub pos: BlockPos,
    pub quality: f32,  // 0.0-1.0，影响休息效率
}
```

### 居住点选择

NPC 生成时自动选一个 `ShelterSpot` POI 作为 home。如果附近没有 → 在生成点 30 格内随机选一个靠墙/有遮挡的位置。

### 回家行为

`ReturnHomeScorer`：
- Night 时段 → 评分 0.6 × Rest 权重
- 真元 < 20% → 评分 0.8（紧急回家）
- 饥饿 < 30% → 评分 0.5（回家补充）

`ReturnHomeAction`：
- Navigator 走向 HomeBase.pos
- 到达后切换为 RestAction
- 休息中 hunger 回复 ×2 / 真元回复 ×1.5（家的安全感加成）

---

## §4 LOD 分层日程

### Near（≤64 格）—— 完整演出

- 完整 big-brain 行为树 + 日程权重注入
- Navigator A* 寻路到 POI
- 到达后播放对应动作（采集/打坐/摆摊/休息动画）
- 每 tick 更新

### Far（64-256 格）—— 状态 tick

不跑行为树和寻路。每 **1200 tick（1 分钟）** tick 一次：

```rust
fn far_npc_schedule_tick(schedule, cultivation, hunger, home, clock) {
    let phase = day_phase(clock.tick, schedule.phase_offset_ticks);
    let activity = schedule.weighted_random(phase);
    match activity {
        Forage => hunger.value += 0.05,  // 采到了一点
        Cultivate => cultivation.qi_current += zone_qi * 0.01,
        Rest => hunger.value += 0.02,
        Trade | Patrol | Socialize | Wander => {},  // 无状态变化
    }
    // 位置：根据 activity 微调（Forage 往 HerbPatch 方向偏移 5 格 / Rest 往 home 偏移）
    npc_position += activity_drift(activity, poi_direction);
}
```

不产生动画/寻路/碰撞——纯数值推演。

### Dormant（>256 格）—— 最小推演

每 **1200 tick（1 分钟）** tick 一次：

```rust
fn dormant_npc_tick(cultivation, hunger, lifespan, clock) {
    hunger.value -= 0.1;  // 10 分钟饿了一些
    if hunger.value < 0.0 { hunger.value = 0.3; }  // 假设自己找到了食物
    lifespan.advance(1200);   // 寿元推进
    // cultivation 不变（dormant 不修炼）
    // 位置不变（dormant 冻结位置）
}
```

### Hydrate 过渡（Dormant → Near）

玩家靠近 dormant NPC 时 hydrate 回 Near。此时：
1. 根据当前 `CultivationClock.tick` 计算 `DayPhase`
2. 按 phase 选一个 `ScheduleActivity`
3. 如果是 Rest + Night → NPC 在 HomeBase 位置出现（蹲着/靠墙）
4. 如果是 Forage + Dawn → NPC 在附近 HerbPatch POI 附近出现
5. 立即进入完整行为树

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `DayPhase` + `NpcDailySchedule` component + 时段权重注入到现有 Scorer | ✅ 2026-05-11 |
| P1 | `GoToPoiAction`（替换 WanderAction）+ POI 类型扩展（5 种新 POI） | ✅ 2026-05-11 |
| P2 | `NpcHomeBase` + `ReturnHomeAction` + `RestAction` + `StallAction` | ✅ 2026-05-11 |
| P3 | Far 层状态 tick + Dormant 层最小推演 + Hydrate 过渡逻辑 | ✅ 2026-05-11 |
| P4 | 饱和化测试（4 时段 × 7 活动 × 3 LOD 层 × hydrate 过渡） | ✅ 2026-05-11 |

---

## P0 — 时段日程 ✅ 2026-05-11

### 交付物

1. **`DayPhase` enum + `day_phase()` 函数**（`server/src/npc/schedule.rs`，新文件）

   ```rust
   pub fn day_phase(tick: u64, offset: i32) -> DayPhase {
       let day_tick = ((tick as i64 + offset as i64).rem_euclid(DAY_TICKS as i64)) as u64;
       match day_tick {
           0..=5999 => DayPhase::Dawn,
           6000..=14399 => DayPhase::Day,
           14400..=19199 => DayPhase::Dusk,
           _ => DayPhase::Night,
       }
   }
   pub const DAY_TICKS: u64 = 24000;
   ```

2. **`NpcDailySchedule` component**

   散修/凡人/兽各一套默认日程（§1 表格）。挂在 NPC spawn 时。

3. **时段权重注入**

   修改 `brain.rs` 中以下 Scorer 的评分逻辑：
   - `WanderScorer`：乘 `schedule.weight(phase, Wander)`
   - `CultivationDriveScorer`：乘 `schedule.weight(phase, Cultivate)`
   - `HungerScorer`：乘 `schedule.weight(phase, Forage)`

   **不改 Scorer 接口**——只在评分计算内部读 `NpcDailySchedule` component。

4. **LOD 守卫**

   所有日程逻辑加 `if lod == Near` 守卫。Far/Dormant 不跑日程 Scorer。

### 验收抓手

- 测试：`npc::schedule::tests::day_phase_boundaries`（4 时段边界 tick 值）
- 测试：`npc::schedule::tests::phase_offset_shifts_boundary`（偏移后时段正确）
- 测试：`npc::schedule::tests::wander_score_modulated_by_phase`（Night 时 Wander 评分低）
- 测试：`npc::schedule::tests::cultivate_score_high_at_night`（Night 时 Cultivate 权重 0.3）
- 手动：观察散修 NPC 在 Night 时段频繁打坐、Dawn 时段开始走动

---

## P1 — 目的地驱动漫游 ✅ 2026-05-11

### 交付物

1. **POI 类型扩展**

   在现有 `PoiKind` enum 中新增 5 种（§2 定义）。worldgen 结构生成时注册对应 POI。

2. **`GoToPoiAction`**（替换 `WanderAction`）

   ```rust
   pub struct GoToPoiAction {
       pub target_poi: Option<PoiId>,
       pub arrive_action: Option<ScheduleActivity>,
       pub timeout_ticks: u32,
   }
   ```

   逻辑：
   - 读当前 `ScheduleActivity` → 映射到 `PoiKind`
   - `PoiRegistry::nearest(pos, kind, 64)` 查最近匹配 POI
   - 有 → Navigator 走过去 → 到达后执行对应 sub-action
   - 无 → 退化为随机漫游（保留原 WanderAction 作为 fallback）

3. **到达后 sub-action 分发**

   到达 HerbPatch → 播采集动画 5-15s → hunger +0.1
   到达 QiSpring → 转入 CultivateAction
   到达 TradeSpot → 转入 StallAction（P2）
   到达 ShelterSpot → 转入 RestAction（P2）

### 验收抓手

- 测试：`npc::brain::tests::go_to_poi_selects_nearest_herb_patch`
- 测试：`npc::brain::tests::go_to_poi_fallback_to_wander_when_no_poi`
- 测试：`npc::brain::tests::go_to_poi_switches_to_cultivate_at_qi_spring`
- 手动：散修 NPC 在 Dawn 时走向灵草丛 → 到达后弯腰采集 → 采完走向下一个 POI

---

## P2 — HomeBase + 新 Action ✅ 2026-05-11

### 交付物

1. **`NpcHomeBase` component**（§3）

   NPC spawn 时选 ShelterSpot POI 或随机隐蔽位。

2. **`ReturnHomeAction`**

   Navigator 走向 home → 到达后切 RestAction。
   `ReturnHomeScorer`：Night×0.6 / qi<20%×0.8 / hunger<30%×0.5。

3. **`RestAction`**

   NPC 在 home 附近停留：hunger 回复 ×2 / qi 回复 ×1.5。
   视觉：蹲伏姿态 or 靠墙站定。

4. **`StallAction`**

   NPC 在 TradeSpot 站定，面朝最近路径方向。
   等待 60-300s → 有玩家 8 格内 → 触发交易评估（已有 ThreatAssessment）。
   无人来 → 超时离开。

### 验收抓手

- 测试：`npc::brain::tests::return_home_high_score_at_night`
- 测试：`npc::brain::tests::rest_at_home_doubles_hunger_recovery`
- 测试：`npc::brain::tests::stall_faces_nearest_path`
- 手动：Night 时散修走回岩洞 → 蹲下休息 → Dawn 起身出门

---

## P3 — LOD 分层推演 ✅ 2026-05-11

### 交付物

1. **Far 状态 tick**（§4 Far 部分）

   `far_npc_schedule_tick` system，每 1200 tick 一次。按当前时段加权随机选活动 → 纯数值更新。

2. **Dormant 最小推演**（§4 Dormant 部分）

   `dormant_npc_tick` system，每 1200 tick 一次。hunger 衰减 + 假设自养 + 寿元推进。

3. **Hydrate 过渡**（§4 Hydrate 部分）

   Dormant → Near 时：
   - 按当前 tick 计算 DayPhase
   - 按 phase 选 activity → 决定 NPC 出现位置（home / POI 附近）
   - 立即进入完整行为树

### 验收抓手

- 测试：`npc::schedule::tests::far_tick_updates_hunger`
- 测试：`npc::schedule::tests::dormant_tick_advances_lifespan`
- 测试：`npc::schedule::tests::hydrate_at_night_spawns_near_home`
- 测试：`npc::schedule::tests::hydrate_at_dawn_spawns_near_herb_poi`
- 性能：100 Near + 500 Far + 5000 Dormant = 仍维持 60 tps（Far/Dormant tick 开销 < 0.1ms）

---

## P4 — 饱和化测试 ✅ 2026-05-11

### 交付物

1. **日程矩阵**
   - 4 DayPhase × 7 ScheduleActivity × 散修/凡人/兽 3 archetype = 84 种权重组合 pin
   - phase 边界 tick 值精确（Dawn=0-5999 / Day=6000-14399 等）

2. **LOD 层级一致性**
   - NPC 在 Near 运行 1000 tick → 切到 Far 运行 10 次 tick → 切回 Near → 状态（hunger/qi）偏差 < 5%
   - Dormant 1 分钟推演 → hydrate 回 Near → 位置合理（在 home 或 POI 附近）

3. **端到端**
   - 跟踪一个散修 NPC 一整天（24000 tick）：Dawn 出门采集 → Day 在灵草丛蹲 → Dusk 回家 → Night 在家打坐 → Dawn 再出门
   - 观察 hunger / qi_current 曲线合理（白天消耗、夜晚回复）

---

## Finish Evidence

- **落地清单**：
  - P0：`server/src/npc/schedule.rs` 定义 `DayPhase`、`ScheduleActivity`、`NpcDailySchedule`、`NpcScheduleChangedEvent`，并在 `server/src/npc/brain.rs` 将日程权重注入 `HungerScorer`、`WanderScorer`、`CultivationDriveScorer`、`TradeStallScorer`、`ReturnHomeScorer`。
  - P1：`server/src/npc/brain.rs` 新增 `GoToPoiAction` / `GoToPoiState`，`server/src/world/poi_novice.rs`、`server/src/schema/poi_novice.rs`、`agent/packages/schema/src/poi-novice.ts` 扩展 `HerbPatch`、`QiSpring`、`TradeSpot`、`ShelterSpot`、`WaterSource`。
  - P2：`server/src/npc/schedule.rs` 新增 `NpcHomeBase` / `rest_tick()`，`server/src/npc/brain.rs` 新增 `ReturnHomeAction`、`RestAction`、`StallAction`，`server/src/npc/spawn.rs` 为 commoner / rogue / scattered cultivator / beast / disciple 初始化日程和 home。
  - P3：`server/src/npc/schedule.rs` 新增 Far 层 `far_npc_schedule_tick_system`、Dormant 最小推演测试钩子、`hydrate_position_for()`；`server/src/npc/dormant/mod.rs` 将 dormant 生命周期心跳收敛到 `DORMANT_LIFECYCLE_TICK_INTERVAL = 1200` tick 并持久化日程 seed；`server/src/npc/hydrate/mod.rs` hydrate 时沿用 snapshot seed 并按当前日程重定位。
  - P4：`server/src/npc/schedule.rs`、`server/src/npc/brain.rs`、`server/src/world/poi_novice.rs`、`agent/packages/schema/tests/poi-novice.test.ts` 覆盖时段边界、权重矩阵、POI 选择、回家/休息/摆摊、Far/Dormant/Hydrate、schema enum pin。
- **关键 commit**：
  - `6cd80afb1`（2026-05-11）`feat(npc): 扩展日常生活 POI 契约`
  - `d18a042e4`（2026-05-11）`feat(npc): 加入日程阶段与 LOD 状态 tick`
  - `6cc5d076d`（2026-05-11）`feat(npc): 接入目的地漫游与回家休息行为`
  - `3c7afd1ff`（2026-05-11）`fix(npc): 收紧日程 LOD 与回家休息语义`
  - `af61d7331`（2026-05-11）`fix(network): 放宽大型 NPC 快照 Redis timeout`
  - `348e57218`（2026-05-11）`fix(npc): 修复日常生活 review 边界`
  - `d790fb73b`（2026-05-11）`fix(npc): 收紧日常生活 review 后续边界`
- **测试结果**：
  - `server/ cargo fmt --check` ✅
  - `server/ cargo clippy --all-targets -- -D warnings` ✅
  - `server/ CARGO_BUILD_JOBS=1 cargo test` ✅ 4320 passed
  - `agent/ npm run build` ✅
  - `agent/packages/schema/ npm run check` ✅ generated schema artifacts are fresh（357 files）
  - `agent/packages/schema/ npm test` ✅ 19 files / 369 tests passed
  - `agent/packages/tiandao/ npm test` ✅ 52 files / 354 tests passed
  - `scripts/e2e-redis.sh` ✅ 15 passed, 0 failed（`.sisyphus/evidence/task-13-e2e-redis-run-20260511-224141-790788-default`）
  - `scripts/smoke-test-e2e.sh` ✅ 8 passed, 0 failed（`.sisyphus/evidence/task-13-smoke-test-e2e-run-20260511-224427-798009-default`）
- **跨仓库核验**：
  - server：`NpcDailySchedule`、`DayPhase`、`GoToPoiAction`、`NpcHomeBase`、`ReturnHomeAction`、`StallAction`、`PoiNoviceKind::HerbPatch`
  - agent/schema：`PoiNoviceKindV1` 含 `herb_patch` / `qi_spring` / `trade_spot` / `shelter_spot` / `water_source`
  - client：无 client 改动；NPC 渲染继续走既有服务端位置 / 动作同步链路
- **遗留 / 后续**：
  - NPC 季节行为差异（冬天少出门 / 夏天活动范围大）→ SeasonState 注入 Schedule。
  - NPC 社交 session 实质化（聊天 / 交易 / 结伴旅行）→ `plan-npc-interaction-polish-v1`。
  - NPC 间竞争（多个散修抢同一灵草丛）→ 需要 POI 占用计数。
  - NPC 记忆（"上次去那个灵草丛被打了，这次换一个"）→ NpcBlackboard 扩展。
  - 天道事件影响日程（伪灵脉升起 → 所有散修涌向该方向）→ agent 指令注入 Schedule。
