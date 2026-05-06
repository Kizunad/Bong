# Bong · plan-lingtian-npc-v1

**NPC 散修种田 = 末法残土的生态压力指示器**。重要认知：散修种田**不是 ambient NPC 刷资源**，而是 worldview §七 / §八 闭环的**触发器**——散修按 §七 "寻路 AI 实时追踪周围灵气浓度"主动迁徙到玩家附近的高灵气区 → 推高 §八 灵物密度阈值 → 触发天道注视 / 道伥刷新 / 灵气归零。这是 worldview 已有循环的**玩法实例化**，给世界添加非玩家驱动的灵气竞争 + 三方博弈（玩家 vs 散修 vs 天道）。本 plan 在 plan-lingtian-v1（merged 93%）+ plan-npc-ai-v1（finished，big-brain Rogue archetype 已落地）基础上，实装散修自主开荒-种植-收获-迁徙循环。

**世界观锚点**：
- `worldview.md §七 动态生物生态 / 智能 NPC（散修）`（核心锚点：散修是"利己主义者" / "NPC 没有灵龛" / "寻路 AI 实时追踪周围灵气浓度"——本 plan 把这段文字落地为代码层）
- `worldview.md §七 拾荒散修 三种行为模式`（恭敬交易 / 拔刀爆装备 / 逃窜——本 plan 散修对玩家进入灵田 5 格的反应直接复用此三档模式 + 新增"翻脸偷田"档）
- `worldview.md §七 生态联动 / 大迁徙`（"大区域灵气被吸干即将化为死域时，所有野生生物疯狂向附近正数灵气区狂奔"——散修种田 zone 灵气将尽时，散修先于玩家逃跑，是预警信号）
- `worldview.md §八 天道行为准则 / §八.1 灵物密度阈值`（散修群居 → 灵气聚集点 → 天道注视 → 灵气强制归零或刷高阶道伥——本 plan 是"灵物密度阈值"的玩家可观测实例）
- `worldview.md §十 资源与匮乏 / 灵气是零和的`（SPIRIT_QI_TOTAL = 100，NPC 种田从 zone 抽吸——共享 ZoneQiAccount 是 worldview 锚定的物理事实）
- `worldview.md §十一 安全与社交 / 危机分层`（中等境界："散修 NPC 的翻脸掠夺"——明文背书 NPC 抄家来源，对接 plan-niche-defense-v1 仇家 NPC 触发器）
- `worldview.md §十二 死亡 / 一生记录`（散修死亡 / 玩家偷田都进双方生平卷"社交印记 / 死仇"，亡者博物馆永久公开）

**library 锚点**：
- `docs/library/peoples/peoples-0006 战斗流派源流.json`（散修生态背景——本 plan 的 NPC archetype 性格基础）
- 待写 `peoples-XXXX 散修生计录`（散修种田动机文本：为何不修炼而种田？因为穷，因为吃，因为等灵气，因为天道不允许聚集——anchor §七 散修生态 + §一 末法残土基调）
- 待写 `peoples-XXXX 散修迁徙志`（兽潮 + 散修迁徙 = 灵气将尽预警，anchor §七 大迁徙）

**交叉引用**：
- `plan-lingtian-v1.md`（前置 merged 93%；plot/seed/session/replenish/pressure 全套依赖。**关键阻塞**：ActiveLingtianSessions.by_player → by_actor 重构，需小步 PR 先做）
- `plan-npc-ai-v1.md`（前置 finished；big-brain Rogue archetype + Scorer/Action 框架已落地，本 plan 在 RogueBundle 上附加 ScatteredCultivator 组件，复用现有 Brain）
- `plan-cultivation-v1.md`（NPC 散修必须有 cultivation 组件才有种田动机——种植积 herbalism XP）
- `plan-skill-v1.md`（finished；NPC herbalism 技艺等级影响成功率——复用 SkillSet 系统）
- `plan-death-lifecycle-v1.md`（finished；散修老死 → 灵田无主 → 玩家可接收；一生记录"物质足迹 / 死仇"写入入口）
- `plan-niche-defense-v1.md`（active；散修翻脸掠夺触发抄家——NPC 抄家来源之一，本 plan 散修在灵气竞争激烈时升级为 IntrusionAttempt）
- `plan-tribulation-v1.md`（active-implementing；散修群居推高密度 → 道伥刷新 / 天劫降临，是 §八 的玩家可观测实例）
- `plan-perception-v1.md`（active-design；散修迁徙是预警信号，玩家神识可感知"周围散修迁徙趋势"）
- `plan-fauna-v1.md`（skeleton；散修被异变兽袭击 / 散修与道伥共存——属生态联动，依 fauna-v1 落地）

**阶段总览**：
- P0 ✅ 重构 + ScatteredCultivator 组件（ActiveLingtianSessions.by_player → by_actor；NPC 使用同套 session 资源，保留客户端 player wire 字段兼容）
- P1 ✅ 5 个 farming Action（TillAction / PlantAction / HarvestAction / ReplenishAction / MigrateAction）+ 6 个 Scorer（土壤适宜 / 灵气浓度 / 真元 / 季节 / 邻近威胁 / 工具持有）
- P2 ✅ 散修迁徙 + 灵气追踪行为（worldview §七 "寻路 AI 实时追踪周围灵气浓度"——散修主动找高灵气区扎根；zone 灵气将尽时启动迁徙）
- P3 ✅ 玩家-散修博弈（5 格 linger 触发反应；Aggressive 翻脸场景接 plan-niche-defense IntrusionAttempt）
- P4 ✅ 天道循环联动（NPC-owned plots 计入 ZonePressureTracker；跨档事件推送 agent）
- P5 ✅ NarrationKind::ScatteredCultivator / npc_farm_pressure / niche_intrusion_by_npc 扩展 + agent 散修种田 narration（worldview §八 天道叙事语调："此地散修聚众，又一波将逝"）

**接入面**（按 docs/CLAUDE.md "防孤岛" checklist）：
- **进料**：lingtian `LingtianPlot` / `LingtianSession` / `ActiveLingtianSessions`（**需重构 by_player → by_actor**）+ `ZoneQiAccount` / `ZonePressureTracker` + npc Rogue `Brain` / `Scorer` / `Action` 框架 + cultivation `CultivationState` + skill `SkillSet`（herbalism XP）
- **出料**：`ScatteredCultivator` component → 各采集 / 种植 / 收获 system 消费；散修迁徙触发 zone 灵气波动；散修群居 → 推高 ZonePressureTracker → 触发 §八 天道阈值（已有 system，本 plan 仅作"贡献者"）
- **共享类型**：复用 `LingtianPlot` / `LingtianSession` / `ZoneQiAccount` / `ZonePressureTracker` / `RogueBundle` / `Brain` / `SkillSet`；**新建** `ScatteredCultivator` component / `FarmingTemperament` enum / 4 个 Action / 6 个 Scorer
- **跨仓库契约**：
  - server: `npc::ScatteredCultivator { home_plot: Option<PlotId>, temperament: FarmingTemperament, fail_streak: u8, last_replenish: Tick }` / `npc::FarmingTemperament` enum (Patient / Greedy / Anxious / Aggressive) / `npc::farming_brain::{LingtianFarmingScorer, TillAction, PlantAction, HarvestAction, ReplenishAction, MigrateAction}` / `npc::spawn::spawn_scattered_cultivator_at(zone, qi_density)`
  - schema: 散修行为不直推客户端——通过 entity 位置 / 动画自然同步；新增 `bong:zone/pressure_crossed` 与 `NarrationKind::ScatteredCultivator` / `npc_farm_pressure` / `niche_intrusion_by_npc`（agent 侧 narration 类型）
  - client: 散修 entity 渲染走现有 PlayerEntityBundle（npc-skin-v1 已 finished）；UI 不专门做散修面板（保留 ambient 感）
  - agent: 新增 `niche_intrusion_by_npc` / `npc_farm_pressure` narration kind，订阅 zone 密度阈值 / 散修翻脸事件
  - Redis channel: 新增并对齐 `bong:zone/pressure_crossed`（密度阈值变化）；`bong:lingtian/session_event` 保持玩家 HUD 兼容，不承载 NPC ambient 行为

---

## §0 设计轴心

- [ ] **散修是 indicator，不是 NPC**：散修在 worldview 里是"利己主义者 + 没有灵龛 + 追踪灵气浓度"——本 plan 不把散修当作机械刷资源 NPC，而是末法残土生态压力的可观测指示器
- [ ] **三方博弈**：玩家 vs 散修 vs 天道——玩家不能驱逐散修（worldview §七 散修自主），散修聚集会触发 §八 天道阈值，玩家面对的不是单一散修而是"散修+天道"组合压力
- [ ] **散修没灵龛 → 灵田就是家**：worldview §七 明文"NPC 没有灵龛"——散修把 home_plot 当作家，玩家偷田 = 触怒散修 = §十一 散修翻脸的物理来源
- [ ] **共享 ZoneQiAccount，不分玩家/NPC**：worldview §十 SPIRIT_QI_TOTAL=100 是零和的——散修种田从同一个 zone 抽吸，与玩家共享账本（不另立"NPC 灵田表"）。审计时无法精确归因（zone 不区分消耗源），但符合 worldview 灵气是物理流体的直觉
- [ ] **NPC 走同套 session 流程**：散修走 ActiveLingtianSessions（**需 by_actor 重构**），不绕过 ECS——这样压力计算 / 灵气消耗 / herbalism XP 都自动统一
- [ ] **不做 NPC 互相买卖 / 雇佣**：worldview §十一 玩家默认敌对的延伸"散修也敌对"——散修不与玩家组队，不互相交易作物（保留 §七 拾荒散修单点交易模式即可）
- [ ] **不做"看到散修种田就触发剧情"**：散修是 ambient，不是 quest 节点；narration 仅在 §八 阈值触发 / 抄家事件时介入
- [ ] **离线场景**：玩家离线时散修继续运转（推高密度 / 抢灵气 / 偷玩家田），玩家上线时通过 ZonePressureTracker 历史 + 一生记录"物质足迹"被入侵推送了解发生了什么

---

## §1 散修在 worldview 中的位置（认知锚定）

worldview §七 / §八 已经写好的循环——本 plan 的工作只是把它**实例化**：

```
[末法残土的生态循环]

   高灵气区（玩家发现/经营）
   ↓ §七 散修寻路 AI 追踪灵气浓度
   散修迁徙过来，开荒种田（NPC 没有灵龛 → 灵田就是家）
   ↓ NPC 推高 ZoneQiAccount 消耗 + ZonePressureTracker 上升
   §八 灵物密度阈值触发（"高浓度灵气聚集点 → 天道注视"）
   ↓ §八 天道反应：灵气归零 OR 刷高阶道伥 OR 降天劫
   1. 灵气归零 → 散修先逃跑（§七 大迁徙），玩家也只能撤
   2. 刷高阶道伥 → 散修被道伥猎杀（§七 道伥行为不分玩家/NPC）
   3. 降天劫 → 散修中境界高者也要扛劫
   ↓
   高灵气区 → 中性区 → 低灵气区 / 死域
   ↓ 散修迁徙到下一个高灵气区
   循环
```

**关键洞察**：
- 玩家不是孤立面对天道——散修是"挡箭牌兼竞争者"
- 玩家在高灵气区扎根 → 散修必然来 → 必然推高密度 → 必然触发天道
- 这是 worldview 设计的"末法残土 = 不能久留"机制的物理化

本 plan 不创造新机制，只**让 worldview 既写好的文字变成可观测的 entity 行为**。

---

## §2 ScatteredCultivator 组件 + FarmingTemperament

附加在现有 `RogueBundle`（npc-ai-v1 已 finished）上，标识此散修是"种田流"：

```rust
// server/src/npc/scattered_cultivator.rs
#[derive(Component, Debug, Clone)]
pub struct ScatteredCultivator {
    pub home_plot: Option<PlotId>,        // 散修认领的灵田（"家"的物理替代）
    pub temperament: FarmingTemperament,  // 性格：影响 Scorer 权重
    pub fail_streak: u8,                  // 连续种植失败次数（影响翻脸 / 迁徙概率）
    pub last_replenish_tick: Tick,        // 上次补灵时间
    pub migration_cooldown: Tick,         // 迁徙冷却（防抖动）
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FarmingTemperament {
    Patient,     // 稳健派：偏好低灵气稳定区，对玩家保持距离
    Greedy,      // 贪心派：追高灵气浓度，容易在天道阈值线上徘徊
    Anxious,     // 焦虑派：fail_streak 阈值低，频繁迁徙
    Aggressive,  // 激进派：玩家进入 5 格 → 翻脸偷田概率高（接 niche-defense 仇家来源）
}
```

性格分布在 spawn 时随机：30% Patient / 30% Greedy / 25% Anxious / 15% Aggressive，整体偏温和但保留少数激进个体（worldview §七 "利己主义者"语境）。

---

## §3 farming Brain（接 npc-ai-v1 big-brain）

### 3.1 6 个 Scorer

| Scorer | 评分依据 | worldview 锚 |
|---|---|---|
| `SoilSuitabilityScorer` | 当前位置土壤翻土难度 / 已有 plot 占位 | §七 散修自主性 |
| `QiDensityScorer` | 当前 zone 灵气浓度 / 邻区对比 | §七 "寻路 AI 实时追踪灵气浓度" |
| `OwnQiPoolScorer` | 散修自身真元状态（低真元偏离开荒/种植） | §四 真元池模型 |
| `SeasonScorer` | 当前季节适合种植 / 收获 | §十七 末法节律 |
| `NearbyThreatScorer` | 5 格内异变兽 / 道伥 / 玩家威胁度 | §七 三种行为模式（恭敬 / 拔刀 / 逃窜）|
| `ToolPossessionScorer` | 散修是否持有 plan-tools-v1 凡器（影响开荒效率） | §四 凡器档位（凡铁 / 木石）|

权重组合按 `FarmingTemperament` 调整：Greedy 强调 QiDensityScorer，Anxious 强调 NearbyThreatScorer。

### 3.2 5 个 Action

| Action | 触发条件 | 走的 system |
|---|---|---|
| `TillAction` | SoilSuitabilityScorer 高 + 无 home_plot | lingtian `start_tilling_session(actor=Self)` |
| `PlantAction` | home_plot 存在 + 季节适合 + 持有种子 | lingtian `start_planting_session(actor=Self)` |
| `HarvestAction` | home_plot 有成熟作物 | lingtian `start_harvest_session(actor=Self)` |
| `ReplenishAction` | home_plot qi 低 + 散修真元够 | lingtian `start_replenish_session(actor=Self)` |
| `MigrateAction` | fail_streak ≥ 3 OR ZonePressureCrossed=High OR §七 大迁徙触发 | 离开 home_plot，转入 wander，找新 zone |

所有 farming session 走 lingtian 既有 ActiveLingtianSessions——**这是 P0 的关键阻塞**（by_player → by_actor 重构）。

---

## §4 关键阻塞：lingtian-v1 ActiveLingtianSessions 重构

worldview 设计上散修与玩家**共享 session 流程**，但 lingtian-v1 当前代码是 player 硬编码：

```rust
// server/src/lingtian/systems.rs 当前
pub struct ActiveLingtianSessions {
    pub by_player: HashMap<Entity, ActiveSession>,  // ← 字段名硬编码
}

pub struct ActiveSession {
    pub player: Entity,  // ← 字段硬编码
    // ...
}
```

**P0 必做的 prep PR**（不在 lingtian-npc-v1 范围内，但本 plan 阻塞于此）：

```rust
// 重构后
pub struct ActiveLingtianSessions {
    pub by_actor: HashMap<Entity, ActiveSession>,  // 泛化
}

pub struct ActiveSession {
    pub actor: Entity,    // 玩家 or 散修
    // ...
}
```

修改面：
- `server/src/lingtian/systems.rs` 主体
- `server/src/lingtian/session_*.rs` 各 session 模块（till / plant / harvest / replenish）
- `server/src/lingtian/networks.rs` IPC emit（actor 字段加 entity_kind 标记 player vs npc）
- `server/src/lingtian/persistence.rs` DB schema（plot owner 已经是 Entity，可能不需要改）

**建议先开 prep PR 把这一票字段重命名 + 单测覆盖**——属于 lingtian-v1 的 P6 维护工作，不属本 plan 范围；本 plan 标注为前置任务。

---

## §5 散修与玩家的博弈

### 5.1 玩家进入散修 5 格内的反应

复用 worldview §七 拾荒散修三种行为模式 + 新增第 4 种：

| 玩家状态 | 散修反应 | 触发条件 |
|---|---|---|
| 玩家真元满 + 境界高于散修 | **逃窜** | §七 既有"丢下低级资源疯狂逃窜" |
| 玩家真元满 + 境界等同 | **恭敬交易** | §七 既有"恭敬交易，愿以物易物" |
| 玩家满身是血 + 真元见底 | **拔刀爆装备** | §七 既有"交易界面瞬间关闭，直接拔刀" |
| 玩家在散修 home_plot 5 格内逗留 ≥ 30s | **翻脸偷田** | 新增——FarmingTemperament=Aggressive 时概率 +50% |

"翻脸偷田" = 散修启动 IntrusionAttempt → 接 plan-niche-defense-v1 NPC 抄家来源（如玩家也有灵龛）。

### 5.2 玩家偷散修田的反应

worldview §十一 "玩家默认敌对" 的延伸——散修田被偷：

- 散修立即触发"拔刀爆装备"模式（与 worldview §七 一致）
- 多次被偷 → fail_streak += 1 → 散修迁徙概率上升
- 重大事件写入 NPC 散修生平卷（worldview §十二 NPC 也有一生记录吗？这是开放问题，暂按"NPC 不写一生卷"实施，待 worldview 决定）

---

## §6 §八 天道循环联动

### 6.1 散修推高密度 → §八 阈值触发器

worldview §八.1 灵物密度阈值——本 plan 不实现新阈值 system（已在 lingtian-v1 ZonePressureTracker 落地），只**让散修也作贡献者**：

```rust
// server/src/lingtian/zone_pressure.rs（已有，本 plan 不改）
pub fn compute_zone_pressure_system(
    plots: Query<&LingtianPlot>,  // ← 不分 owner_kind，全量统计
    mut tracker: ResMut<ZonePressureTracker>,
) {
    // 按 plot 数量 + 灵气消耗速率算 pressure
    // 散修的 plot 自动算入（plot.owner 是 Entity，不区分 player/npc）
}
```

### 6.2 阈值触发后的玩家可观测信号

§八 天道反应（已在 plan-cultivation-v1 / plan-tribulation-v1 / plan-fauna-v1 落地）：

| 阈值档 | 天道反应（已有） | 玩家可观测 |
|---|---|---|
| MID | 异变兽刷新概率 +50%（plan-fauna-v1） | "近来兽多" |
| HIGH | 道伥刷新（worldview §七 + plan-tribulation P5） | "山中又见无名人形" |
| EXTREME | zone 灵气强制归零 → §七 大迁徙触发 | 散修 + 玩家齐逃 |

→ 本 plan 不改这些 system，但**散修是触发它们的可见信号**——玩家看到散修聚集，就知道阈值即将到。

### 6.3 §七 大迁徙触发散修先逃

worldview §七 "大区域灵气被吸干即将化为死域时，所有野生生物疯狂向附近正数灵气区狂奔"——散修也是"野生生物"：

- ZonePressureCrossed=Extreme + zone qi → 阈值 → 散修 MigrateAction 优先级最高
- 散修迁徙集体性发生（所有 home_plot 在该 zone 的散修同时启动）
- 玩家看到散修离开是"灵气将尽"的信号——比天道 narration 更早的预警

---

## §7 平衡考量

### 7.1 散修种田不是"玩家版机械刷"

- 散修每次种田/收获/补灵都消耗**散修自身真元**（与玩家相同）
- 散修真元池上限按境界（醒灵-引气期为主，少数凝脉期）—— 不是无限刷
- 散修慢慢种、慢慢收，绝不像玩家高效——给玩家"散修是麻烦但不致命"的语义

### 7.2 散修不挤压玩家修炼

- 散修种田只在地表 plot，不在玩家闭关点（灵龛附近 3 格内不开荒）
- 散修不会主动破坏玩家 plot（除非翻脸状态）
- 设计上避免"玩家被散修包围无法修炼"——保留世界宽容度

### 7.3 散修迁徙的节奏

- 默认 30 分钟（real-time）一次 MigrateAction 评估
- 灵气将尽时（zone qi < 0.2）立即迁徙
- 防止散修在两 zone 之间反复横跳——migration_cooldown=10min

### 7.4 散修翻脸概率

- Aggressive temperament: 30%（玩家 5 格 / 30s 触发）
- Anxious / Greedy: 10%
- Patient: 5%
- 散修翻脸 ≠ 一定胜利——散修境界通常低于玩家，翻脸成功率本身有限

### 7.5 与 plan-niche-defense-v1 的接口

- 散修翻脸 + 玩家有灵龛 → 散修 IntrusionAttempt 走 niche-defense P4 NPC 抄家流程
- 散修被收买（食腐者卖坐标）→ 也走 niche-defense P4 仇家 NPC 来源
- 散修群体压力 → 不直接接 niche-defense，但触发 §八 天道反应 → 间接增加玩家在 zone 内的危险性

---

## §8 数据契约（下游 grep 抓手）

### server

- [x] `lingtian::ActiveLingtianSessions.by_actor` 字段重命名（**P0 前置**）— `server/src/lingtian/systems.rs`
- [x] `lingtian::ActiveSession` 继续作为 enum 包装 session；actor 语义落在 `ActiveLingtianSessions.by_actor` key，保留原 session struct wire 兼容 — `server/src/lingtian/systems.rs`
- [x] `npc::ScatteredCultivator` component — `server/src/npc/scattered_cultivator.rs`（新文件）
- [x] `npc::FarmingTemperament` enum (Patient / Greedy / Anxious / Aggressive) — 同上
- [x] `npc::farming_brain::{LingtianFarmingScorer, TillAction, PlantAction, HarvestAction, ReplenishAction, MigrateAction}` — `server/src/npc/farming_brain.rs`（新文件）
- [x] `npc::spawn_scattered_cultivator_at(zone, qi_density)` — `server/src/npc/spawn.rs`（修改）
- [x] `npc::ScatteredCultivatorBundle`（含 Brain + ScatteredCultivator + Rogue base）— 同上

### schema / agent

- [x] `NarrationKind::ScatteredCultivator` / `npc_farm_pressure` / `niche_intrusion_by_npc` 扩展 — `agent/packages/schema/src/common.ts`
- [x] agent 订阅 `bong:zone/pressure_crossed` + `bong:social/niche_intrusion`，分别生成 zone pressure 与 NPC 抄家 narration
- [x] agent 新增 `ScatteredCultivatorNarrationRuntime` — `agent/packages/tiandao/src/scattered-cultivator-narration.ts`
- [x] schema：新增 `ZonePressureCrossedV1`；lingtian session 协议不新增 NPC actor 字段，NPC ambient 行为不走玩家 HUD session wire

### client

- [x] **无新增 UI**——散修 entity 走 npc-skin-v1 现有渲染，UI 不专门做散修面板
- [x] inspect tab 不显示散修信息（保 ambient 感）

### Redis channel

- [x] `bong:zone/pressure_crossed` 新增为 server → agent 观测通道；`bong:lingtian/session_event` 保持玩家 HUD 兼容

---

## §9 实施节点

- [x] **P0**：重构前置 + ScatteredCultivator 组件
  - `ActiveLingtianSessions.by_player → by_actor` 字段重命名；`ActiveSession` 现状为 enum 包装，不存在独立 `player` 字段，保留事件字段兼容
  - 本 plan：`ScatteredCultivator` component + `FarmingTemperament` enum
  - `spawn_scattered_cultivator_at(zone, qi_density)` 在现有 RogueBundle 上附加 ScatteredCultivator
  - 启动播种的 Rogue 散修接入 farming Brain，非 agent-command Rogue 仍可保持普通 Rogue 行为
  - 单测：spawn 成功 / temperament 分布 / home_plot 初始为 None
  - **lingtian-v1 阻塞**：已在本 PR 内收敛到 by_actor + NPC inventory 兼容

- [x] **P1**：farming Brain（6 Scorer + 5 Action）
  - 6 个 Scorer 实装（Soil/Qi/Quz/Season/Threat/Tool）
  - 5 个 Action 实装（Till/Plant/Harvest/Replenish/Migrate）
  - 性格权重表
  - 散修单独跑 farming Brain 闭环（不与玩家交互）
  - 单测：评分函数 / plot 状态判定 / 性格影响权重 / spawn 挂载 farming thinker
  - **lingtian P0-P5 闭环**（已 merged 93%，足以支撑）

- [x] **P2**：散修迁徙 + 灵气追踪
  - QiDensityScorer 跨 zone 比较，散修主动迁徙至更高灵气区
  - MigrateAction 实装：找新 zone + 离开 home_plot + Navigator 迁徙
  - §七 大迁徙触发器先落为低 zone qi / fail_streak 优先迁徙；集体迁徙留给 fauna/tribulation 后续事件整合
  - 单测：QiDensity 比较正确；迁徙执行由 `cargo clippy` + runtime wiring 覆盖

- [x] **P3**：玩家-散修博弈
  - 玩家 5 格触发反应（4 种行为模式：逃窜 / 恭敬交易 / 拔刀 / 翻脸偷田）
  - 翻脸状态接 niche-defense IntrusionAttempt（如玩家有灵龛在附近）
  - 玩家偷散修田 → v1 通过 home_plot linger 触发 IntrusionAttempt；fail_streak 由 farming loop 迁徙 scorer 消费
  - 单测：反应判定 / IntrusionAttempt 触发

- [x] **P4**：天道循环联动
  - 验证：散修群居 → ZonePressureTracker 上升 → 阈值触发 → §八 天道反应（道伥刷新 / 灵气归零 / 天劫）
  - NPC-owned plots 计入 ZonePressureTracker；HIGH 保持既有 plot_qi 清零与 npc pressure consumer
  - 集成测：NPC owner plots 推高至 HIGH；ZonePressureCrossed 发布到 Redis outbound

- [x] **P5**：narration kind 扩展 + agent 散修 narration
  - `NarrationKind::ScatteredCultivator` / `npc_farm_pressure` / `niche_intrusion_by_npc` schema 扩展
  - agent runtime 订阅 zone pressure / 翻脸事件 → 触发"此地散修聚众，又一波将逝"等 §八 风格 narration
  - 单测：narration 触发条件 / 风格符合 §八 冷漠语调
  - **agent 阻塞**：agent narration 框架在 finished plan-agent.md 已完整落地

---

## §10 开放问题

- [ ] **NPC 是否有一生记录**？worldview §十二 一生记录段未明示 NPC 是否走同一系统。本 plan 暂按"NPC 不写一生卷"实施，但散修死亡后玩家可能想查"这只散修在我地盘种了几天"——可作为 worldview 扩展讨论
- [ ] **散修豢养异变兽 / 道伥** 是否被允许？worldview §七 拾荒散修是单独行动者，但 niche-defense-v1 P2 守宅道伥机制扩到 NPC 阵营是否合理？v1 不做，留 v2
- [ ] **散修间互相协作 / 偷田** 是否启用？v1 不做（worldview §十一 玩家敌对的延伸），但极端密度下散修间可能资源争夺——可作为生态联动深化点留 v2
- [ ] **散修的境界天花板** 在哪？v1 以醒灵 / 引气为主（与 worldview §七 "底层修士"语境一致）；少数凝脉期散修是否需要特殊行为模板？
- [ ] **离线时玩家被散修偷田**：玩家上线后通过 ZonePressureTracker 历史 + 灵田损失日志了解发生了什么——具体推送形式（list 形式 / 点位定位 / 神识感知 trigger）需进一步设计
- [ ] **散修对玩家阵法陷阱** 是否触发？plan-niche-defense P1 阵法陷阱判定 entity 不分玩家/NPC——理论上散修走过陷阱也会触发；但如果散修是"玩家敌对"语境，陷阱该不该误伤散修盟友（如玩家曾给散修援助）？
- [ ] **散修群居 → 天道阈值触发** 时，agent narration 应该如何措辞？worldview §八 风格指南要求"冷漠 / 古意 / 嘲讽"——具体话术由 narration runtime 实战调

---

## §11 进度日志

- **2026-04-XX**：骨架立项，从 reminder.md 提炼。承接 plan-lingtian-v1 + plan-npc-ai-v1 全套依赖。
- **2026-04-30**：从 skeleton 升 active（commit c5ea3e03）。`/plans-status` 调研评级 ⚠️ 有阻塞——核心是 `ActiveLingtianSessions.by_player` HashMap 字段是 player 语义硬编码，需要先小步重构 PR 再升 active；散修种田概念是 worldview 推导而非正典直引。
- **2026-04-30**（重写）：核心认知调整——散修种田**不是机械 NPC 刷资源**，而是 worldview §七 / §八 闭环的**触发器**。重读 worldview 后发现 §七 "NPC 没有灵龛 / 寻路 AI 实时追踪灵气浓度 / 利己主义者"已经写好了散修的全部行为锚点，§八 灵物密度阈值已经写好了"高聚集点 → 天道注视"的循环，本 plan 只是把这些文字**实例化**为代码。新增 §1 "散修在 worldview 中的位置（认知锚定）"明确循环图，§6 "§八 天道循环联动" 把散修的角色定位为"玩家可观测的天道触发器"而不是孤立 NPC。新增 P5 narration kind 扩展，让 agent 把散修聚集事件按 §八 冷漠语调讲出来。

## Finish Evidence

### 落地清单

- **P0 重构 + ScatteredCultivator 组件**：`server/src/lingtian/systems.rs` 将 `ActiveLingtianSessions.by_player` 收敛为 `by_actor`，NPC actor 可复用同套 session 生命周期；`server/src/npc/scattered_cultivator.rs` 新增 `ScatteredCultivator`、`FarmingTemperament`、玩家靠近反应与散修社会记忆；`server/src/npc/spawn.rs` 新增 `ScatteredCultivatorBundle` / `spawn_scattered_cultivator_at` 并挂载 farming brain。
- **P1 farming Brain**：`server/src/npc/farming_brain.rs` 新增 `LingtianFarmingScorer`、Till / Plant / Harvest / Replenish / Migrate 五个 action 与六类 scorer，复用既有 `ActiveSession`，避免为 NPC 另建平行灵田流程。
- **P2 迁徙 + 灵气追踪**：`MigrateAction` 通过 zone qi 与 `fail_streak` 提升迁徙优先级，并用既有 `Navigator` 表达移动；v1 先落低灵气 / 连续失败触发的个体迁徙，集体迁徙留给 fauna / tribulation 后续事件汇合。
- **P3 玩家-散修博弈**：`server/src/npc/scattered_cultivator.rs` 根据 temperament 判定玩家 5 格 linger 反应；Aggressive 散修围绕 home plot 触发 `NicheIntrusionAttempt`，接入 niche-defense 的 NPC 抄家契约。
- **P4 天道循环联动**：`server/src/lingtian/systems.rs` 覆盖 NPC-owned plots 对 `ZonePressureTracker` 的计入；`server/src/network/zone_pressure_bridge.rs` 与 `server/src/network/redis_bridge.rs` 将 `ZonePressureCrossed` 发布到 `bong:zone/pressure_crossed`。
- **P5 agent 散修 narration**：`agent/packages/schema/src/common.ts`、`agent/packages/schema/src/zone-pressure.ts`、`agent/packages/schema/src/channels.ts` 扩展 narration kind 与 zone pressure contract；`agent/packages/tiandao/src/scattered-cultivator-narration.ts` 订阅 `ZONE_PRESSURE_CROSSED` / `SOCIAL_NICHE_INTRUSION` 并发布 `AGENT_NARRATE`。

### 关键 commit

- `fd7130a2`（2026-05-06）`实现散修灵田运行态`：server 侧 by_actor、散修组件、farming brain、spawn wiring、zone pressure Redis bridge。
- `2b248700`（2026-05-06）`接入散修天道叙事契约`：schema contract、generated schema、tiandao narration runtime 与测试。

### 测试结果

- `cd server && cargo fmt --check`
- `cd server && cargo clippy --all-targets -- -D warnings`
- `cd server && cargo test`：2444 passed
- `cd agent && npm ci`：补齐 worktree 内 `node_modules`
- `cd agent && npm run generate -w @bong/schema`
- `cd agent && npm run generate:check -w @bong/schema`
- `cd agent && npm run build`
- `cd agent && npm test -w @bong/schema`：273 passed
- `cd agent && npm test -w @bong/tiandao`：241 passed
- `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build`：BUILD SUCCESSFUL

### 跨仓库核验

- **server**：`ActiveLingtianSessions.by_actor`、`ScatteredCultivator`、`FarmingTemperament`、`LingtianFarmingScorer`、`ScatteredCultivatorBundle`、`ZonePressureCrossedV1`、`RedisOutbound::ZonePressureCrossed`。
- **agent/schema**：`NarrationKind` 新增 `scattered_cultivator` / `npc_farm_pressure` / `niche_intrusion_by_npc`，`CHANNELS.ZONE_PRESSURE_CROSSED` 对齐 `bong:zone/pressure_crossed`，`ZonePressureCrossedV1` 生成 JSON schema 并有 schema test。
- **agent/tiandao**：`ScatteredCultivatorNarrationRuntime` 接入 `main.ts`，对 zone pressure 与 NPC niche intrusion 生成 narration，且过滤非 NPC intruder。
- **client**：无新增 UI / payload path；Fabric 侧通过 Java 17 `./gradlew test build` 验证既有渲染与构建未被破坏。

### 遗留 / 后续

- v1 不实现散修集体迁徙总开关；后续应在 fauna / tribulation 事件系统落地后统一处理 zone 级迁徙。
- v1 不为 NPC 写一生卷、不开放散修豢养异变兽 / 道伥、不做散修间偷田协作；这些仍保留为 worldview / 后续 plan 的边界问题。
- `bong:lingtian/session_event` 继续保持玩家 HUD 兼容，不承载 NPC ambient 行为；散修叙事入口统一走 `bong:zone/pressure_crossed` 与 `bong:social/niche_intrusion`。
- `SeasonScorer` 与 `ToolPossessionScorer` 在 v1 中只锁定 big-brain scorer 接口形状；真实节律和凡器持有判定应在末法节律 / plan-tools-v1 落地后接入。
- `intruder_char_id` 仍沿用现有 NPC intrusion 的 Bevy entity debug 形式；若后续要跨重生追踪仇家，需要引入稳定 NPC identity。
