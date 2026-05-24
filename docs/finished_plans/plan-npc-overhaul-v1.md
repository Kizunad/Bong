# Bong · plan-npc-overhaul-v1

NPC 重质不重量——大幅裁减数量（200→50）、散布生成（Poisson 间距替代 patrol_anchor 聚堆）、升级利己决策智能（composite 威胁评估 + 动态攻守切换）、完善交易经济（动态定价 + 信誉门控 + 信息商品化）、整理 34K 行 NPC 代码（brain.rs 4834 行拆分 + 死代码清理）。

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-05-24 | 代码拆分（brain.rs / spawn.rs 模块化） | brain.rs 4834→8 个子模块各 ≤800 行；cargo test 全绿；功能零变更 |
| **P1** | ✅ 2026-05-24 | 数量裁剪 + 散布生成 | max_npc_count 200→50；同 archetype 间距 ≥ zone 自适应阈值；启动后视野内不出现 NPC 扎堆 |
| **P2** | ✅ 2026-05-24 | 威胁评估 + 利己决策升级 | NPC 根据玩家 realm/qi%/伤口三维评估做出攻/守/逃/交易/伏击五态切换 |
| **P3** | ✅ 2026-05-24 | 交易系统完善 | 动态定价公式（含价格下限）、NpcPlayerReputation 信誉门控、信息商品、NPC 翻脸掠夺链路 |
| **P4** | ✅ 2026-05-24 | 集成校准 + e2e | 完整遭遇链路 e2e 测试；50 NPC TPS≥18；数值平衡校准 |

---

## 接入面 Checklist

### 进料

| 来源模块 | 消费内容 | 用途 |
|---------|---------|------|
| `npc::lifecycle::NpcRegistry` | `max_npc_count` / `per_zone_caps` / hysteresis | P1 下调上限 + 区域预算 |
| `npc::spawn::RoguePopulationSeedConfig` | `target_count` / `resource_fraction` | P1 下调种子数 |
| `npc::spawn::spawn_rogue_npc_at()` | patrol_anchors + jitter 逻辑 | P1 替换为 Poisson 采样 |
| `npc::brain` 全部 Scorer/Action | 25+ 评分器/行为 | P0 拆分 + P2 重组升级 |
| `npc::trade::NpcTradeInventory` | 静态目录 + splitmix64 生成 | P3 扩展动态定价 |
| `npc::interaction_memory::NpcMemoryComponent` | 玩家交互记忆 (8 slot) | P2/P3 驱动信誉 + 行为 |
| `combat::components::Wounds` | `health_current / health_max` | P2 威胁评估读取玩家伤口 |
| `cultivation::components::Cultivation` | `realm / qi_current / qi_max` | P2 威胁评估读取玩家境界 |
| `npc::equipment::NpcEquipment` | 6 slot 装备 | P2 评估玩家装备威胁 |
| `npc::scattered_cultivator::CultivatorPlayerReaction` | 旧 4 态决策（Flee/RespectfulTrade/RobPlayer/StealPlot） | P2 迁移替换为 SelfInterestDecision |
| `npc::scattered_cultivator::choose_player_reaction()` | 旧 reputation→反应逻辑 | P2 删除，调用方迁移到 SelfInterestScorer |
| `npc::scattered_cultivator::trade_price_multiplier_for_reputation()` | 旧 i32 reputation 定价 | P3 废弃，替换为 NpcPlayerReputation + DynamicPricing |

### 出料

| 产出 | 消费方 | 形式 |
|------|--------|------|
| `npc::spawn::PoissonSpawnSampler` | spawn 系统 | 新 struct：候选位置生成 + 间距校验 |
| `npc::brain::ThreatAssessment` | 所有战斗/交易/逃跑 scorer | 新 struct（`#[derive(Copy, Clone)]`）：composite 威胁分 (0.0-1.0) |
| `npc::brain::SelfInterestDecision` | thinker 决策层 | 新 enum：Attack / Guard / Flee / Trade / Ambush |
| `npc::trade::DynamicPricing` | 交易系统 | 新 struct：动态定价引擎（含 `floor_ratio: 0.3` 价格下限） |
| `npc::trade::InformationOffer` | 交易 GUI | 新 struct：信息商品（坐标/情报/警告） |
| `npc::trade::NpcPlayerReputation` | 交易 + 决策系统 | 新 component：per-NPC per-player 信誉度（与 `faction::Reputation` 无关） |
| schema `NpcTradeOfferV2` | client `bong:npc_metadata` | S2C 扩展：含动态价格 + 信息类型 + NPC 信誉等级标签 |

### 共享类型 / event

- **复用** `NpcRetireRequest` / `NpcReproductionRequest` / `NpcDeathNotice`（不新建）
- **复用** `NpcMemoryComponent`（扩展 slot 上限 8→16 + 新增 `NpcInteractionType::TradeRefused` / `Ambushed` / `FledFrom`）
- **淘汰策略变更**：`NpcMemoryComponent::trim_to_limit` 从 FIFO 改为权重淘汰——`Attack`/`Theft`/`Ambushed` 标记 `pinned=true` 不被淘汰，其余按时间淘汰
- **复用** `NpcBlackboard`（新增字段 `threat_assessment: Option<ThreatAssessment>`）——`ThreatAssessment` 为 `Copy` 类型，不影响 blackboard 的 `Copy` derive
- **新增** `NpcPlayerReputation` component（挂在 NPC entity 上，`HashMap<Uuid, f32>` 存储 per-player 信誉度 0.0-1.0）。**与 `npc::faction::Reputation`（NPC 对派系的忠诚度）完全无关**——后者是 NPC-to-faction 的 loyalty，前者是 NPC-to-player 的评价
- **新增** `ReputationGossipEvent { source_npc, target_player_uuid, delta: f32 }` —— NPC 间传话降低玩家信誉
- **不新建** CultivationDeathTrigger 等——威胁评估是 per-tick 计算写入 blackboard，不走事件

### 跨仓库契约

| 层 | symbol / key | 变更 |
|----|-------------|------|
| **server** | `NpcRegistry::max_npc_count` | 200→50 |
| **server** | `PoissonSpawnSampler` | 新增 |
| **server** | `ThreatAssessment` | 新增（`#[derive(Copy, Clone)]`） |
| **server** | `NpcPlayerReputation` | 新增 component |
| **server** | `DynamicPricing` / `InformationOffer` | 新增 |
| **server→client** | `bong:npc_metadata` | 扩展 `trade_offers` 含 `dynamic_price` + `info_type` + `npc_rep_tier` |
| **client** | `NpcMetadata.java` | 扩展字段解析（`npc_rep_tier` 显示为信誉等级标签） |
| **agent** | 无变更 | NPC 行为变化通过现有 world_state 推送自然反映 |

### worldview 锚点

| worldview 章节 | 对应本 plan 内容 |
|----------------|-----------------|
| §七:730-740 散修 NPC 行为 | P2 利己决策：威胁评估→攻守切换 |
| §七:733-740 散修评估玩家 | P2 三维评估：qi 满→恭敬交易 / 见底→翻脸掠夺 / 境界高→丢买命钱逃跑 |
| §九:839-858 经济与交易 | P3 动态定价 + 信息商品 |
| §十一:949-953 NPC 反应分级 | P3 信誉门控（NpcPlayerReputation） |
| §十:866-872 灵气零和 | P1 高 qi 区多 NPC 低 qi 区少 NPC |
| §十一:976-982 危机分层 | P2 新手保护：NPC 不翻脸境界低 ≥2 级的玩家 |

### qi_physics 锚点

本 plan 不引入新物理常数。NPC 交易定价读取 `zone.spirit_qi_normalized()`（只读），不产生/消耗灵气。

### 与现有 plan 的关系

| plan | 关系 | 边界 |
|------|------|------|
| `plan-npc-combat-gear-v1`（active ⬜） | **互补** | combat-gear 拥有交易 GUI 最终形态（P2 owo-lib NpcTradeScreen）+ 装备模型/功法调用。**本 plan 只提供 server 端数据**（DynamicPricing / NpcPlayerReputation / InformationOffer），不触碰 GUI 布局和 owo-lib 代码。client 扩展仅限 `NpcMetadata.java` 字段解析（`dynamic_price` / `info_type` / `npc_rep_tier`），GUI 渲染归 combat-gear |
| `plan-npc-virtualize-v2`（skeleton） | **受益** | NPC 数量减少后 virtualize 压力大幅降低，Drowsy 态设计空间更充裕 |
| `plan-npc-virtualize-v3`（skeleton） | **无冲突** | dormant 派系战争是独立模块 |

---

## P0 — 代码拆分（前置重构）

> **P0 是纯重构阶段，功能零变更。** brain.rs 4834 行在后续 P2 要做 scorer 替换，不先拆就在巨文件上做结构性改动——review 痛苦、合并冲突风险高。

### P0.1 brain.rs → brain/ 目录拆分

| 新文件 | 内容 | 预估行数 |
|--------|------|---------|
| `brain/mod.rs` | thinker builder 函数 + 公共类型 + re-export | ~400 |
| `brain/scorers_combat.rs` | 战斗相关 scorer：PlayerProximity / ChaseTarget / MeleeRange / Dash | ~500 |
| `brain/scorers_survival.rs` | 生存相关 scorer：Fear / Hunger / ReturnHome / Ageing | ~400 |
| `brain/scorers_cultivation.rs` | 修炼相关 scorer：CultivationDrive / TribulationReady / Seclusion / Curiosity | ~400 |
| `brain/scorers_social.rs` | 社交/交易 scorer：TradeStall / FactionDuel / Socialize / Loyalty | ~500 |
| `brain/actions_combat.rs` | 战斗 action：Chase / MeleeAttack / Dash / Flee / FleeCultivator | ~600 |
| `brain/actions_life.rs` | 生活 action：Wander / GoToPoi / Rest / Farm / Stall / Cultivate / Retire | ~600 |
| `brain/threat.rs` | 预留空文件（P2 填充 ThreatAssessment + SelfInterestDecision） | ~10 |

**规则**：
- 每个子模块 ≤800 行
- scorer struct 定义和对应 system 在同一文件（就近原则）
- `brain/mod.rs` 只做 re-export + thinker builder，不含任何 scorer/action 实现
- 测试跟着代码走（原 brain.rs 中的 `#[cfg(test)]` 按归属拆到各子模块）

### P0.2 spawn.rs → spawn/ 目录拆分

spawn.rs 2500 行，大量是各 archetype 的 `spawn_*_npc_at()` 函数。

- `spawn/mod.rs`：公共类型 + 种子分布逻辑 + PoissonSpawnSampler（P1 填充）
- `spawn/common.rs`：`npc_runtime_bundle_with_age` 等公共 bundle 组装
- `spawn/rogue.rs`：`spawn_rogue_npc_at` / `spawn_rogue_commoner_base`
- `spawn/commoner.rs`：`spawn_commoner_npc_at`
- `spawn/beast.rs`：`spawn_beast_npc_at`
- `spawn/disciple.rs`：`spawn_disciple_npc_at`
- `spawn/zombie.rs`：`spawn_zombie_npc_at`（dev 命令仍引用，保留但标注 `#[cfg(feature = "dev")]`）

### P0.3 死代码清理

grep `#[allow(dead_code)]` + 未被引用的 struct/fn，标记并删除：

已知候选：
- `guanzhu_remnant.rs`（已标 dead code）
- `spawn_dragon.rs` 中未使用的 Dragon archetype 残留

### P0 测试要求

- `cargo test --all` 全绿（功能零变更的唯一验收标准）
- `cargo clippy --all-targets -- -D warnings` 零警告
- 各子模块内 `#[cfg(test)]` 测试与原始结果 1:1 对应

---

## P1 — 数量裁剪 + 散布生成

### P1.1 全局/区域上限下调

**目标**：max_npc_count 200→50，rogue seed 100→20。

| 参数 | 旧值 | 新值 | 文件 |
|------|------|------|------|
| `NpcRegistry::max_npc_count` | 200 | 50 | `npc/lifecycle.rs` |
| `NpcRegistry::resume_npc_count` | 180 | 40 | `npc/lifecycle.rs` |
| `RoguePopulationSeedConfig::target_count` | 100 | 20 | `npc/spawn/mod.rs` |
| `ROGUE_SEED_BATCH_SIZE` | 10 | 5 | `npc/spawn/mod.rs` |
| `BONG_ROGUE_SEED_COUNT` env | 覆盖 target | 保留，默认 20 | `npc/spawn/mod.rs` |

**区域预算**（`per_zone_caps` 显式写入）：

| 区域 | qi 基准 | 散修+凡人预算 | 散修:凡人 | 自适应间距 |
|------|---------|-------------|-----------|-----------|
| spawn | 0.5 | 6 | 3:3 | 48 格 |
| qingyun_peaks | 0.7 | 5 | 4:1 | 48 格 |
| spring_marsh | 0.4 | 4 | 3:1 | 40 格 |
| rift_valley | 0.6 | 5 | 4:1 | 48 格 |
| north_wastes | 0.2 | 2 | 2:0 | 32 格（小区域缩小） |
| lingquan_marsh | 0.5 | 4 | 3:1 | 40 格 |
| **小计** | — | **26** | — | — |

**NpcRegistry 分桶计数**：

| 桶 | 上限 | 说明 |
|----|------|------|
| `humanoid_budget` | 26 | 散修 + 凡人（上表总和） |
| `beast_budget` | 20 | 野兽 / 鼠群 / 异变兽 |
| `special_budget` | 4 | Whale / Dragon / Guardian / SkullFiend |
| **全局 max_npc_count** | **50** | 三桶总和上限（任一桶满不阻塞其他桶，但三桶总和不超 50） |

> `NpcRegistry` 新增 `counts_by_bucket: HashMap<NpcBudgetBucket, u32>`，spawn 系统按 archetype 映射到对应 bucket 检查余量。三桶独立计数避免 beast 挤压 rogue 配额。

**删除启动僵尸**：`spawn_single_zombie_npc_on_startup`（`spawn.rs:658-687`）移除——这是 MVP 遗物，与散修世界观不符。

### P1.2 Poisson 散布采样

**问题**：当前 `patrol_anchors[index % len] + 4-block jitter` 导致多个 NPC 叠在同一 anchor 点。

**方案**：新增 `PoissonSpawnSampler`，替代 anchor+jitter。

> `ZoneBounds` 从现有 zone 注册表中导出——当前 zone 系统用 `patrol_anchors` + zone 边界 AABB 定义区域范围。`ZoneBounds` 是这个 AABB 的类型别名（`type ZoneBounds = (DVec3, DVec3)`），如果现有代码没有合适类型则在 `spawn/mod.rs` 新建。

```rust
// npc/spawn/mod.rs 新增
pub struct PoissonSpawnSampler {
    min_same_archetype_dist: f64,  // per-zone 自适应（32-48 格）
    min_cross_archetype_dist: f64, // per-zone 自适应（16-24 格）
    max_candidates: u32,           // 30（Mitchell best-candidate）
}

impl PoissonSpawnSampler {
    pub fn sample_position(
        &self,
        zone_bounds: &ZoneBounds,
        existing_npcs: &NpcSpatialIndex,
        archetype: NpcArchetype,
        rng: &mut impl Rng,
    ) -> Option<DVec3>;

    /// 根据 zone 面积自适应计算间距阈值
    pub fn adaptive_for_zone(zone_bounds: &ZoneBounds) -> Self;
}
```

**自适应间距规则**（决议 §8.1 #2）：
- zone 面积 ≥ 500×500 格：间距 48（默认）
- zone 面积 300-500 格：间距 40
- zone 面积 < 300 格：间距 32（下限，不再缩小）

**算法**：Mitchell's best-candidate
1. 生成 `max_candidates` 个随机候选点（zone 边界内）
2. 对每个候选点计算到最近同 archetype NPC 的距离
3. 选距离最大的候选点
4. 若最大距离仍 < `min_same_archetype_dist`，返回 None（该区域已饱和）

**间距豁免**：
- `BeastKind::Rat`：群居设计，走原有 chunk-based grouping，不受间距约束
- Beast 亲子对（`ProtectYoungScorer` 关联）：允许共处
- Whale/Dragon：飞行实体，只检查 XZ 平面距离

### P1.3 种子分布重写

替换 `seed_initial_rogue_population_on_startup` 中的分配逻辑：

```
旧：distribute_counts_evenly() → 均分到 zone → 同 anchor 聚堆
新：per_zone_caps 预算 → PoissonSpawnSampler::adaptive_for_zone() 逐个采样 → 间距保证
```

每 tick 仍限 `ROGUE_SEED_BATCH_SIZE=5` 个（降低启动 burst），但采样方式从 anchor-jitter 改为 Poisson。

**年龄散布保留**：`index % 10` bucket 不变（防同步退休）。

### P1 测试要求

- `spawn::poisson_sampler_respects_min_distance`：生成 20 NPC 后全对检查间距 ≥ adaptive 阈值
- `spawn::poisson_sampler_returns_none_when_saturated`：zone 面积不够时返回 None
- `spawn::adaptive_distance_scales_with_area`：500×500 zone → 48 格；200×200 → 32 格
- `spawn::zone_budget_caps_respected`：per_zone_caps 不被突破
- `spawn::rat_exempted_from_spacing`：鼠群不受间距约束
- `spawn::startup_zombie_removed`：PostStartup 不再生成僵尸
- `lifecycle::registry_defaults_50`：NpcRegistry 默认 max=50 resume=40
- `spawn::seed_count_default_20`：种子数默认 20
- `lifecycle::bucket_independence`：beast 桶满不阻塞 humanoid 桶
- `lifecycle::total_cap_50`：三桶总和不超 50

---

## P2 — 威胁评估 + 利己决策升级

### P2.1 Composite 威胁评估

当前每个 Scorer 各自判断玩家距离/境界，没有统一的"这个玩家对我有多危险"评估。

**新增 `ThreatAssessment` struct**（`npc/brain/threat.rs`）：

```rust
#[derive(Copy, Clone, Debug)]
pub struct ThreatAssessment {
    pub score: f32,             // 0.0（无害）→ 1.0（致命威胁）
    pub realm_delta: i8,        // 玩家境界 - NPC 境界（正=玩家更强）
    pub player_qi_ratio: f32,   // 玩家 qi_current / qi_max
    pub player_wound_ratio: f32,// 聚合伤口严重度（见下方公式）
    pub player_distance: f32,   // 距离
    pub has_weapon_visible: bool,// 玩家手持武器
}
```

> `ThreatAssessment` 为 `Copy` 类型——所有字段均为原始类型，写入 `NpcBlackboard.threat_assessment: Option<ThreatAssessment>` 不破坏 blackboard 的 `Copy` derive。

**评分公式**（对标 worldview §七:733-740）：

```
threat_score = clamp(0, 1,
    realm_factor(realm_delta)       // 境界碾压 = 高威胁
  + qi_factor(player_qi_ratio)      // 玩家 qi 满 = 高威胁
  - wound_factor(player_wound_ratio)// 玩家满身伤 = 低威胁
  + weapon_factor(has_weapon)       // 持械 = +0.1
  - distance_factor(distance)       // 远 = 低威胁
)
```

各因子权重用 `NpcThreatConfig` resource 注入（可热调），默认值：
- `realm_weight`: 0.4（境界差是最大因素）
- `qi_weight`: 0.25
- `wound_weight`: 0.15
- `weapon_weight`: 0.1
- `distance_weight`: 0.1

**`player_wound_ratio` 聚合公式**：`Wounds` component 含多部位多 severity 伤口。聚合为标量：`wound_ratio = 1.0 - (health_current / health_max)`。这里用 hp 比例代替伤口细节——NPC 不需要知道玩家具体哪个部位受伤，只需要感知"这个人看起来多虚弱"。

**计算频率**：仅 LOD Near/Far 时计算，写入 `NpcBlackboard.threat_assessment`。Dormant 跳过。

### P2.2 利己决策模型（SelfInterestDecision）

worldview §七 定义了散修"利己主义者"的行为模式。将其形式化为五态决策：

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SelfInterestDecision {
    Trade,    // 恭敬交易（玩家强 + qi 满 + 非通缉）
    Flee,     // 丢买命钱逃跑（玩家境界远高于自己）
    Guard,    // 警戒观望（威胁中等，保持距离）
    Ambush,   // 翻脸掠夺（玩家 qi 见底 / 满身伤 + NPC 有胜算）
    Ignore,   // 无视（玩家太远或无利可图）
}
```

**决策映射**（对标 worldview §七:735-740）：

| 条件 | 决策 | worldview 对应 |
|------|------|---------------|
| threat ≥ 0.8 | Flee | "你境界远高于它→丢买命钱逃窜" |
| threat 0.5-0.8 + player qi > 0.5 | Trade | "你气息绵长→恭敬交易" |
| threat 0.5-0.8 + player qi ≤ 0.5 | Guard | 观望，等待时机 |
| threat < 0.3 + player wounds > 0.4 **+ realm_delta ≥ -1** | Ambush | "你满身血→翻脸爆装备" |
| threat < 0.3 + player qi < 0.2 **+ realm_delta ≥ -1** | Ambush | "你真元见底→翻脸" |
| distance > 32 | Ignore | 视野外不反应 |

**新手保护**（决议 §8.1 #3）：`Ambush` 决策**附加条件** `realm_delta >= -1`（NPC 只翻脸同境或比自己低 1 级的玩家）。若 `realm_delta < -1`（玩家境界比 NPC 低 2 级以上），NPC 不翻脸——worldview §十一:976 明确醒灵/引气的危机来源是"NPC 野兽"和"灵气不足"，不是散修翻脸。

**迁移旧决策系统**：`scattered_cultivator::CultivatorPlayerReaction`（4 态：Flee/RespectfulTrade/RobPlayer/StealPlot）及 `choose_player_reaction()` 函数全部废弃，其调用方（farming brain 的 scorer）迁移到 `SelfInterestScorer`。旧的 4 态映射到新 5 态：`RobPlayer` → `Ambush`，`StealPlot` → `Ambush`，`Flee` → `Flee`，`RespectfulTrade` → `Trade`。

**`detect_scattered_cultivator_plot_trespass` 系统迁移**：该系统（`scattered_cultivator.rs:211-278`）依赖 `choose_player_reaction()` 检测 `StealPlot` 来触发 `NicheIntrusionAttempt` 事件。迁移方案：改为读取 `NpcBlackboard.self_interest_decision == Ambush` + 玩家在 NPC home_plot 范围内 → emit `NicheIntrusionAttempt`。plot trespass 检测逻辑独立保留（不合并到 SelfInterestScorer），只替换其决策来源。

**集成到 thinker**：

新增 `SelfInterestScorer`（替代 `PlayerProximityScorer` / `ChaseTargetScorer` 等碎片判断 + `CultivatorPlayerReaction`），输出 `SelfInterestDecision` 写入 blackboard → 下游 scorer 读取 decision 而非各自重算威胁。

```rust
// 旧：每个 scorer 独立判断
.when(PlayerProximityScorer, FleeAction)      // 近了就逃
.when(ChaseTargetScorer, ChaseAction)          // 近了就追

// 新：统一评估后分发
.when(SelfInterestScorer::flee(), FleeAction)
.when(SelfInterestScorer::ambush(), AmbushAction)
.when(SelfInterestScorer::trade(), ApproachForTradeAction)
.when(SelfInterestScorer::guard(), GuardAction)
```

### P2.3 记忆驱动行为偏移

利用现有 `NpcMemoryComponent`（`interaction_memory.rs`），让过去交互影响未来决策：

- **被攻击过的玩家**：threat_score 永久 +0.2（在该 NPC 存活期间）
- **成功交易过的玩家**：trade 决策阈值放宽（threat 0.3-0.8 都愿交易）
- **被抢劫过**：对该玩家直接进入 Flee（不再判断）

**记忆淘汰策略变更**：`NpcMemoryComponent::trim_to_limit` 从 FIFO 改为权重淘汰：
- `Attack` / `Theft` / `Ambushed` 类型记忆标记 `pinned = true`，不参与淘汰
- 其余记忆（`Trade` / `Help` / `TradeRefused` / `FledFrom`）按时间 FIFO 淘汰
- slot 上限从 8→16（50 NPC × 16 slot × ~64 bytes ≈ 50KB，可忽略）
- pinned 记忆占满上限时（极端情况：NPC 被 16 个不同玩家攻击），最旧的 pinned 记忆降级为 unpinned 可淘汰

扩展现有 `NpcInteractionType`（`interaction_memory.rs:32`）新增 3 个变体：
```rust
// interaction_memory.rs — 现有枚举，新增标 ← NEW
pub enum NpcInteractionType {
    Trade,       // 已有
    Attack,      // 已有（pinned）
    Theft,       // 已有（pinned）
    Help,        // 已有
    TradeRefused,// ← NEW
    Ambushed,    // ← NEW（pinned）
    FledFrom,    // ← NEW
}
```

### P2 测试要求

- `threat::assessment_realm_dominant`：境界差 2 级 → threat ≥ 0.7
- `threat::assessment_wounded_player_low_threat`：玩家 hp < 30% → threat < 0.3
- `threat::assessment_qi_depleted_triggers_ambush`：玩家 qi < 20% + NPC 同境 → decision = Ambush
- `threat::assessment_high_realm_triggers_flee`：玩家境界 +3 → decision = Flee
- `threat::assessment_healthy_player_triggers_trade`：玩家 qi > 80% + 同境 → decision = Trade
- `threat::newbie_protection_no_ambush`：玩家境界比 NPC 低 2 级 + qi 见底 → decision ≠ Ambush（Guard instead）
- `threat::memory_attack_bias`：被攻击后 threat +0.2
- `threat::memory_trade_broadens_threshold`：成功交易后 trade 阈值放宽
- `decision::all_variants_reachable`：5 种决策全部有对应测试路径
- `decision::guard_to_ambush_transition`：Guard 状态下玩家 qi 持续下降 + realm_delta ≥ -1 → 切换为 Ambush
- `memory::pinned_not_evicted`：pinned 记忆在 slot 满时不被 FIFO 淘汰
- `memory::oldest_pinned_downgrades_when_full`：16 个 pinned slot 全满 → 最旧 pinned 降级

---

## P3 — 交易系统完善

### P3.1 NpcPlayerReputation component

**问题**：现有 `npc::faction::Reputation` 是 NPC 对**派系**的忠诚度（单 `loyalty: f64` 字段），不是 NPC 对**玩家**的评价。worldview §十一:949-953 要求"NPC 反应分级按 identity 信誉度"。代码中不存在 per-player 信誉度接口。

**方案**：新增 `NpcPlayerReputation` component，挂在每个可交互 NPC entity 上。**同时废弃旧接口**：`scattered_cultivator::trade_price_multiplier_for_reputation(i32)` / `should_attack_for_reputation(i32)` / `trade_price_for_reputation(i32)` 全部删除，调用方迁移到 `NpcPlayerReputation::tier()` + `DynamicPricing::compute_price()`。

```rust
// npc/trade.rs 新增
#[derive(Component, Default)]
pub struct NpcPlayerReputation {
    scores: HashMap<Uuid, f32>,  // player UUID → reputation 0.0-1.0
}

impl NpcPlayerReputation {
    pub fn get(&self, player: Uuid) -> f32;      // 默认 0.5（中立）
    pub fn adjust(&mut self, player: Uuid, delta: f32);
    pub fn tier(&self, player: Uuid) -> RepTier;  // High/Mid/Low/Hostile
}

pub enum RepTier { High, Mid, Low, Hostile }
```

**信誉来源**：
- 初始值 0.5（中立）
- 成功交易 +0.05
- 被攻击 -0.3
- 收到 `ReputationGossipEvent` -0.05
- 帮助 NPC（如治疗）+0.1

### P3.2 动态定价引擎

当前交易是静态目录。扩展为 `DynamicPricing`：

```rust
// npc/trade.rs
pub struct DynamicPricing;

impl DynamicPricing {
    pub fn compute_price(
        base_price: u32,
        zone_qi: f32,
        npc_qi_need: f32,
        reputation: f32,
        supply_scarcity: f32,
        config: &TradePricingConfig,
    ) -> u32;
}
```

**定价公式**：
```
raw_price = base_price
    × zone_modifier(zone_qi)        // qi<0.3: ×1.5 / qi>0.6: ×0.8
    × relationship_modifier(rep)    // rep>0.7: ×0.85 / rep<0.3: ×1.3
    × scarcity_modifier(scarcity)   // 稀有物品不打折
    × npc_urgency(npc_qi_need)      // NPC 急需 qi → 愿意低价出

final_price = max(base_price × floor_ratio, raw_price)  // floor_ratio=0.3，防止定价降到 0
```

> **价格下限**（决议 §8.1 #10）：`floor_ratio = 0.3`——再怎么叠乘，最终价格不低于 base_price 的 30%。

所有乘数用 `TradePricingConfig` resource 注入，支持热调。

### P3.3 信誉门控（worldview §十一:949-953）

交易前检查 `NpcPlayerReputation::tier(player_uuid)`：

| 信誉 tier | 行为 | worldview 对应 |
|-----------|------|---------------|
| High (> 0.7) | 折扣 15% + 主动给情报 | "高=主动给情报/折扣" |
| Mid (0.3-0.7) | 正常交易 | "中=正常交易" |
| Low (0.1-0.3) | 加价 30% + 拒绝稀有物品 | "低=加价/拒绝服务" |
| Hostile (< 0.1) | 拒绝交易 + NPC 间传话 | "极低=通缉" |

**传话机制**：NPC A 与 Hostile 信誉玩家交互 → 120 tick 后向 48 格内其他 NPC emit `ReputationGossipEvent` → 接收方对该玩家信誉 -0.05。最多传播 3 跳（每跳衰减 delta ×0.5），防止全图扩散。

### P3.4 信息商品

worldview §九:845 "真正保值的只有信息"。NPC 可出售信息类商品：

```rust
pub struct InformationOffer {
    pub info_kind: InfoKind,
    pub price_bone_coins: u32,
    pub accuracy: f32,        // 0.0-1.0 信息准确度（NPC 可能说谎）
    pub expiry_ticks: u64,    // 信息有效期
}

pub enum InfoKind {
    ZoneQiLevel { zone_name: String, qi_value: f32 },
    DangerWarning { zone_name: String, threat_desc: String },
    ResourceLocation { zone_name: String, resource: String },
    NpcSighting { target_desc: String, last_zone: String },
}
```

**生成逻辑**：NPC 根据自己的巡逻历史生成信息——曾经路过高 qi 区、见过异变兽、知道某区资源点。信息来源是 NPC 的 `NpcPatrol.home_zone` 及 `NpcMemoryComponent` 记录。

**准确度与玩家感知**（决议 §8.1 #4）：
- 高信誉 NPC → accuracy 0.8-1.0
- 低信誉 NPC → accuracy 0.3-0.6（可能卖假情报骗骨币）
- **client 端显示 NPC 信誉等级标签**（`npc_rep_tier` 字段）：`[可靠] / [中立] / [狡诈] / [敌意]`，让玩家根据标签自行判断信息可信度
- **不显示 accuracy 数值**——worldview 设计哲学是"信息差"，玩家靠经验判断而非 UI 提示

### P3.5 翻脸掠夺链路

P2 的 `SelfInterestDecision::Ambush` 在交易场景的具体表现：

1. NPC 开启交易界面（正常流程）
2. 玩家打开背包 → NPC 看到玩家真元低/满身伤
3. NPC 关闭交易界面 + 立即转入 `AmbushAction`
4. `AmbushAction`：发出战斗意图 + 优先攻击 + 战斗结束后拾取掉落

**server 端**：emit `TradeAbortedByNpc { reason: Ambush }` → client 收到后关闭交易屏幕。

**翻脸场景视听规格**：

| 维度 | 规格 |
|------|------|
| **narration** | scope=player, style=perception。模板 3 条：<br>① "对面散修的目光落在你空虚的灵脉上，嘴角微微上扬。"<br>② "散修收起笑脸，手已经按上了腰间的刀柄。"<br>③ "交易？不，这从来就不是一场交易。" |
| **音效** | audio_recipe：`[{sound: "entity.villager.trade", pitch: 0.5, volume: 0.8, delay: 0}]` → `[{sound: "entity.player.attack.strong", pitch: 0.9, volume: 1.0, delay: 8}]`（先交易声降调变阴沉，8 tick 后切换为攻击音效） |
| **HUD** | HudRenderLayer::OVERLAY_WARNING，vignette 红色 `#8B0000` opacity 0.3，fade_in 5 tick / hold 20 tick / fade_out 10 tick，仅受害玩家可见 |
| **粒子** | 无（翻脸是一瞬间的事，粒子反而预警过度） |

### P3 测试要求

- `reputation::new_npc_default_mid`：新 NPC 对未见过的玩家 tier = Mid
- `reputation::trade_increases`：成功交易后 rep +0.05
- `reputation::attack_decreases`：被攻击后 rep -0.3
- `reputation::gossip_propagates`：NPC A 传话 → NPC B 信誉度降低
- `reputation::gossip_decays_over_hops`：3 跳后 delta 衰减到 ×0.125
- `reputation::extreme_low_refuses_all`：rep<0.1 → 拒绝一切交易
- `pricing::zone_qi_low_increases_price`：qi=0.2 → price ×1.5
- `pricing::high_reputation_discount`：rep=0.9 → price ×0.85
- `pricing::npc_urgent_sells_cheap`：NPC qi<20% → price ×0.7
- `pricing::price_floor_enforced`：叠乘后不低于 base×0.3
- `pricing::price_floor_boundary`：base=10, 所有乘数最低 → final=3（不是 0）
- `info::accuracy_correlates_reputation`：高信誉 NPC accuracy ≥ 0.8
- `info::info_expires`：过期信息标记 invalid
- `ambush::trade_to_combat_transition`：交易中玩家 qi 降到阈值 → NPC 翻脸
- `ambush::abort_event_emitted`：翻脸时 emit TradeAbortedByNpc
- `ambush::newbie_protected`：玩家比 NPC 低 2 级 → 不翻脸（即使 qi=0）

---

## P4 — 集成校准 + e2e

### P4.1 完整遭遇 e2e 链路

模拟一次完整 NPC 遭遇：

```
玩家进入 zone → 发现散修 NPC（间距 ≥ adaptive 阈值验证）
→ NPC 评估威胁（ThreatAssessment 输出验证）
→ 决策 Trade（SelfInterestDecision 验证）
→ 打开交易（DynamicPricing 输出验证 + NPC 信誉标签显示验证）
→ 玩家 qi 被外部消耗降到 <20%（且 realm_delta ≥ -1）
→ NPC 翻脸（TradeAbortedByNpc 验证）
→ AmbushAction 执行（战斗流程验证）
→ narration + HUD vignette + SFX 验证
```

### P4.2 性能验证

50 NPC 场景下 TPS ≥ 18（当前 200 NPC + LOD 已通过 perf plan，50 应当轻松达标）。

验证点：
- `ThreatAssessment` 每 NPC 每 tick 计算开销 < 0.1ms
- `PoissonSpawnSampler` 启动采样 20 个 NPC < 50ms
- `DynamicPricing` 计算 < 0.01ms/次

### P4.3 数值平衡

- 各区域 NPC 密度体感测试：spawn 区走 5 分钟应遭遇 1-2 个 NPC
- 交易定价体感：引气期玩家在 spawn 区买一瓶灵草 ≈ 3-5 骨币（合理范围）
- 翻脸概率：qi<20% + 同境 NPC（realm_delta ≥ -1）时 `SelfInterestScorer` 输出 Ambush 分 ≈ 0.6-0.8（通过 `NpcThreatConfig` 调节因子权重实现概率化——得分低于 FirstToScore 阈值 0.05 时不翻脸，留悬念空间）

---

## §8 开放问题（P0 决策门前需收口）

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

### #1 NPC 总数 50 是否太少？

50 上限含 beast/whale/special。如果 6 个 zone 各 3-4 散修 + 若干凡人 + beast，可能只有 ~25 个"可交互 NPC"。是否需要 dormant NPC 作为"远方有人"的视觉暗示？（与 plan-npc-virtualize-v2 Drowsy 态联动）

### #2 Poisson 间距 48 格是否太大？

48 格 = 3 chunk，在小 zone（如 north_wastes）中可能只放得下 1-2 个 NPC。是否需要 per-zone 自适应间距？

### #3 翻脸掠夺的 PvE 体验

翻脸机制对新手是否太 harsh？醒灵/引气期玩家 qi 本来就少，容易被 NPC 翻脸杀。是否需要"NPC 不会翻脸低于自己 2 个境界的玩家"保护？

### #4 信息商品准确度如何验证？

玩家买到假情报后如何感知？需要 UI 提示还是纯靠经验？

### #5 brain.rs 拆分是否需要先于 P0/P1？

P3 代码重构放在 P1 之后，但 P1 要改 scorer → 在旧 brain.rs 上改。是否应该先做 P3 拆分再做 P1 智能升级？

---

## §8.1 决议（pre-P0 收口，2026-05-23）

### #1 NPC 总数 50 够用

**决议**：
1. 50 上限维持不变。~25 个可交互 NPC 在末法残土世界观下是合理的——末法灵气稀薄，散修本就稀少，每一个遭遇应当有重量感。
2. "远方有人"的视觉暗示交给 plan-npc-virtualize-v2 的 Drowsy 态解决——Drowsy NPC 是远视野低成本占位，不在本 plan 范围内。
3. 如果 playtesting 发现 50 太少，`max_npc_count` 是 resource 参数，一行改动即可上调。

**落点**：`npc/lifecycle.rs` `NpcRegistry::default()` / plan P1.1

### #2 per-zone 自适应间距

**决议**：
1. 间距不再固定 48 格，改为 `PoissonSpawnSampler::adaptive_for_zone()` 按 zone 面积自适应。
2. 面积 ≥500×500 → 48 格；300-500 → 40 格；<300 → 32 格（下限）。
3. north_wastes（预算 2 NPC）如果面积 <300 格，用 32 格间距，2 个 NPC 仍能放下。

**落点**：`npc/spawn/mod.rs` `PoissonSpawnSampler::adaptive_for_zone()` / plan P1.2

### #3 新手保护——Ambush realm_delta 门槛

**决议**：
1. `SelfInterestDecision::Ambush` 附加条件 `realm_delta >= -1`。NPC 只翻脸同境或比自己低 1 级的玩家。
2. 对 realm_delta < -1 的玩家（新手），即使 qi=0%、满身伤，NPC 也不翻脸——最多 Guard（保持距离）。
3. 理由：worldview §十一:976 危机分层明确醒灵/引气的危机来源是"NPC 野兽"和"灵气不足"，不是散修翻脸。散修翻脸是中期（凝脉/固元）才面对的社交风险。

**落点**：`npc/brain/threat.rs` `decide_self_interest()` / plan P2.2 决策映射表

### #4 信息准确度——NPC 信誉等级标签

**决议**：
1. 不显示 accuracy 数值——worldview 设计哲学是"信息差"，玩家靠经验判断。
2. client 端交易 GUI 显示 NPC 信誉等级标签：`[可靠] / [中立] / [狡诈] / [敌意]`（通过 `bong:npc_metadata` 的 `npc_rep_tier` 字段传递）。
3. 标签是间接提示——"狡诈"的 NPC 不一定说谎，但概率高。玩家通过多次交易建立信任/不信任。

**落点**：`bong:npc_metadata` 扩展 `npc_rep_tier` 字段 / `NpcMetadata.java` 解析 / plan P3.4

### #5 brain.rs 拆分提前到 P0

**决议**：
1. brain.rs 拆分从原 P3 提前到 P0（新的阶段编号），作为前置纯重构阶段。
2. PR-1 先做代码拆分（功能零变更）→ PR-2 再做数量裁剪 → PR-3 威胁评估 → PR-4 交易 → PR-5 集成。
3. 理由：P2 要替换多个 scorer 为 SelfInterestScorer，在拆分后的子模块上改比在 4834 行的单文件上改更安全，review 更聚焦。

**落点**：plan 阶段总览已重编号 / §10.1 PR 拆分已更新

---

## §10 实施工作流

### §10.1 PR 拆分计划

| PR | 范围 | 依赖 |
|----|------|------|
| PR-1 | P0 代码拆分（brain.rs + spawn.rs + 死代码，纯重构无功能变更） | 无 |
| PR-2 | P1 数量裁剪 + 散布（server only） | PR-1 merge |
| PR-3 | P2 威胁评估 + 利己决策（server only） | PR-2 merge |
| PR-4 | P3 交易系统 + NpcPlayerReputation（server + schema 扩展） | PR-3 merge |
| PR-5 | P4 集成 + client 扩展 + e2e + 数值校准 | PR-4 merge |

### §10.2 PR 实施用独立 subagent（context 隔离）

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...本 PR 范围...\n\nultrathink"
)
```

主线只做 merge + ScheduleWakeup 等 CR review。

### §10.3 CodeRabbit ScheduleWakeup 等待协议

按 docs/CLAUDE.md §6.5 执行：1200s 间隔，最多 3 回合。

### §10.4 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan` 后即可离开，醒来检查 plan 是否在 finished_plans/。

---

## Finish Evidence

### 落地清单

| 阶段 | 交付物 | 实际文件路径 |
|------|--------|------------|
| **P0** | brain.rs 拆分为 8 个子模块 | `server/src/npc/brain/mod.rs`, `brain/scorers_combat.rs`, `brain/scorers_survival.rs`, `brain/scorers_cultivation.rs`, `brain/scorers_social.rs`, `brain/actions_combat.rs`, `brain/actions_life.rs`, `brain/threat.rs` |
| **P0** | spawn.rs 拆分为 6 个子模块 | `server/src/npc/spawn/mod.rs`, `spawn/common.rs`, `spawn/rogue.rs`, `spawn/commoner.rs`, `spawn/beast.rs`, `spawn/disciple.rs`, `spawn/zombie.rs` |
| **P1** | NpcBudgetBucket 三桶预算 | `server/src/npc/lifecycle.rs` (`NpcBudgetBucket` enum, `budget_bucket()` method) |
| **P1** | PoissonSpawnSampler | `server/src/npc/spawn/mod.rs` (`PoissonSpawnSampler` struct, `adaptive_for_zone()`, `sample_position()`) |
| **P1** | max_npc_count 200→50, seed 100→20 | `server/src/npc/lifecycle.rs` (`NpcRegistry`), `server/src/npc/spawn/mod.rs` (`RoguePopulationSeedConfig`) |
| **P2** | ThreatAssessment + 评分因子 | `server/src/npc/brain/threat.rs` (`ThreatAssessment`, `build_threat_assessment`, `compute_threat_score`, factor functions) |
| **P2** | SelfInterestDecision 五态 | `server/src/npc/brain/threat.rs` (`SelfInterestDecision` enum: Trade/Flee/Guard/Ambush/Ignore, `decide_self_interest`, `decide_self_interest_with_memory`) |
| **P2** | NpcThreatConfig resource | `server/src/npc/brain/threat.rs` (`NpcThreatConfig` with default weights) |
| **P2** | NpcBlackboard 扩展 | `server/src/npc/spawn/common.rs` (`threat_assessment: Option<ThreatAssessment>`, `self_interest_decision: Option<SelfInterestDecision>`) |
| **P2** | compute_threat_assessments system | `server/src/npc/brain/threat.rs` (PreUpdate system, registered in `threat::register()`) |
| **P3** | NpcPlayerReputation | `server/src/npc/trade.rs` (`NpcPlayerReputation` component, `RepTier` enum, `adjust()`, `get()`, `tier()`) |
| **P3** | DynamicPricing | `server/src/npc/trade.rs` (`DynamicPricing::compute_price()`, `TradePricingConfig` resource) |
| **P3** | TradeEligibility | `server/src/npc/trade.rs` (`TradeEligibility` enum, `check_trade_eligibility()`) |
| **P3** | InformationOffer | `server/src/npc/trade.rs` (`InformationOffer` struct, `InfoKind` enum, `generate_info_offers()`) |
| **P3** | ReputationGossipEvent + 传话系统 | `server/src/npc/trade.rs` (`ReputationGossipEvent`, `PendingGossip`, `tick_pending_gossips`, `process_gossip_events`) |
| **P3** | TradeAbortedByNpc | `server/src/npc/trade.rs` (`TradeAbortedByNpc` event, `TradeAbortReason` enum) |
| **P3** | NpcMemoryComponent 扩展 | `server/src/npc/interaction_memory.rs` (slot 8→16, weighted eviction, `NpcInteractionType::{TradeRefused,Ambushed,FledFrom}`) |
| **P4** | 完整 NPC 遭遇 e2e 测试 | `server/src/npc/integration_tests.rs` (22 tests) |
| **P4** | 性能基准测试 | `server/src/npc/integration_tests.rs` (threat 50 NPC < 5ms, Poisson 20 NPC < 50ms, pricing 1000x < 10ms) |
| **P4** | 数值平衡断言 | `server/src/npc/integration_tests.rs` (定价范围、翻脸阈值、新手保护边界、信誉→定价联动) |

### 关键 commit

| PR | merge commit | 日期 | 说明 |
|----|-------------|------|------|
| PR-1 (#314) | `56ac12675` | 2026-05-24 | P0 代码拆分（brain.rs + spawn.rs 模块化） |
| PR-2 (#317) | `f6a6cb89c` | 2026-05-24 | P1 数量裁剪 + Poisson 散布生成 |
| PR-3 (#320) | `361ea3769` | 2026-05-24 | P2 威胁评估 + 利己决策升级 |
| PR-4 (#322) | `75d5bcba4` | 2026-05-24 | P3 交易系统完善 |
| PR-5 | (本 PR) | 2026-05-24 | P4 集成校准 + e2e + 归档 |

### 测试结果

```
cargo test: 6426 passed; 0 failed; 0 ignored
cargo clippy --all-targets -- -D warnings: 0 warnings
cargo fmt --check: ok

P4 新增测试 (integration_tests.rs): 22 tests
- encounter_e2e_threat_to_decision_to_trade_to_ambush
- encounter_e2e_memory_biases_affect_decision
- encounter_e2e_npc_memory_drives_trade_broadening
- threat_assessment_50_npcs_under_5ms
- poisson_sampler_20_npcs_under_50ms
- dynamic_pricing_1000_calls_under_10ms
- spawn_zone_lingcao_price_reasonable_range
- ambush_threshold_matches_plan
- ambush_requires_low_threat_score
- newbie_protection_exact_boundary
- reputation_tiers_affect_pricing
- pricing_floor_ratio_enforced
- budget_bucket_mapping_exhaustive
- poisson_adaptive_distance_scales_with_area
- all_decision_variants_reachable_from_e2e_scenarios
- threat_scores_always_clamped_0_1
- threat_score_weights_sum_correctly
- pricing_combined_worst_case
- pricing_combined_best_case
- poisson_sampler_returns_none_when_saturated
- reputation_accumulates_to_high_tier
- reputation_attack_hard_to_recover
```

### 跨仓库核验

| 层 | symbol | 状态 |
|----|--------|------|
| **server** | `ThreatAssessment` | ✅ `npc/brain/threat.rs` |
| **server** | `SelfInterestDecision` | ✅ `npc/brain/threat.rs` (5 variants: Trade/Flee/Guard/Ambush/Ignore) |
| **server** | `NpcPlayerReputation` | ✅ `npc/trade.rs` (component, per-NPC per-player) |
| **server** | `DynamicPricing` | ✅ `npc/trade.rs` (`compute_price()` + `TradePricingConfig`) |
| **server** | `PoissonSpawnSampler` | ✅ `npc/spawn/mod.rs` (Mitchell's best-candidate) |
| **server** | `NpcBudgetBucket` | ✅ `npc/lifecycle.rs` (Humanoid/Beast/Special 三桶) |
| **server** | `InformationOffer` / `InfoKind` | ✅ `npc/trade.rs` |
| **server** | `ReputationGossipEvent` | ✅ `npc/trade.rs` (event + pending gossip system) |
| **server** | `TradeAbortedByNpc` | ✅ `npc/trade.rs` (event) |
| **server** | `NpcThreatConfig` | ✅ `npc/brain/threat.rs` (resource, default weights) |
| **server** | `NpcMemoryComponent` slot 16 + weighted eviction | ✅ `npc/interaction_memory.rs` |
| **agent** | 无变更 | ✅ NPC 行为变化通过现有 world_state 推送自然反映 |

### 遗留 / 后续

- **client `NpcMetadata.java` 扩展 (`npc_rep_tier` field)**：deferred to `plan-npc-combat-gear-v1` P2 owo-lib NpcTradeScreen，该 plan 拥有交易 GUI 最终形态
- **per-player memory precision**：当前 `compute_threat_assessments` 系统遍历 NPC 全部记忆条目而非精确匹配当前 player UUID（需 Lifecycle 组件 on player query），后续优化时可加 player UUID→Entity 映射
- **has_weapon_visible**：当前 `compute_threat_assessments` 中硬编码 `false`，需要 inventory access 检测玩家手持物品类型；后续可接入 `equipment` 模块
