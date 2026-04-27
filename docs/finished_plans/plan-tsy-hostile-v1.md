# TSY 敌对 NPC 分层 · plan-tsy-hostile-v1

> 坍缩渊内 PVE 四档威胁：**道伥**（基础）/ **执念**（精英）/ **秘境守灵**（Boss）/ **负压畸变体**（环境）。本 plan 扩展 `NpcArchetype` + 建四套 big-brain AI tree + 按 TSY 起源分布的 spawn pool + 对应 drop table + `Fuya` 的耗真元光环。P4 在 P2 (lifecycle 已加 `Daoxiang` variant) + P3 (container 提供钥匙消耗对象) 就位后开工。
> 交叉引用：`plan-tsy-v1.md §0/§2`（公理 / 横切）· `plan-tsy-dimension-v1`（位面基础设施前置）· `plan-tsy-zone-v1.md §1`（TSY zone 识别 + `TsyPresence`）· `plan-tsy-loot-v1.md §1-§2`（`ItemRarity::Ancient` + `AncientRelicTemplate`）· `plan-tsy-lifecycle-v1.md §4`（`NpcArchetype::Daoxiang` + `DaoxiangOrigin`）· `plan-tsy-container-v1.md §3`（钥匙 template 约定）· `worldview.md §十六.五 敌对 NPC 分层`（4 档威胁表 + 起源倾向） · `worldview.md §七`（道伥 lore）

> **2026-04-24 架构反转备忘**：TSY 实现为独立位面。本 plan 所有 NPC（道伥/执念/守灵/畸变体）spawn 在 **TSY layer** 下（挂到 `DimensionLayers.tsy`）；"zone AABB 内分布"全部在 TSY dim 内部坐标下计算。P2 lifecycle 中"道伥喷出主世界" = 跨位面传送到 Overworld layer，已在 P2 plan §6 更新。

---

## §-1 现状（已实装 / 上游 plan 已锁）

| 层 | 能力 | 位置 |
|----|------|------|
| `NpcArchetype` | `Zombie` / `Commoner` / `Rogue` / `Beast` / `Disciple` / `GuardianRelic` | `server/src/npc/lifecycle.rs:40-72` |
| `NpcRuntimeBundle` | archetype + lifespan + combat / cultivation components | `server/src/npc/lifecycle.rs:218-231` |
| big-brain Scorer / Action | `PlayerProximityScorer` / `ChaseTargetScorer` / `MeleeRangeScorer` / `DashScorer` / `FleeAction` / `ChaseAction` / `MeleeAttackAction` / `DashAction` | `server/src/npc/brain.rs:47-80` |
| `NpcPatrol` / `Navigator` / `MovementController` | 巡逻 / 寻路 / 移动 | `server/src/npc/{patrol,navigator,movement}.rs` |
| `AttackIntent` | 战斗事件 | `server/src/combat/events.rs` |
| `NpcMarker` / `NpcMeleeProfile` | 标签 / 近战属性 | `server/src/npc/spawn.rs` |
| `DroppedLootRegistry.ownerless` | NPC drop 的载体 | P1 plan §3 扩展 |
| `DeathEvent.attacker_player_id` | drop 归属字段 | P1 plan §2（横切） |
| `NpcArchetype::Daoxiang` | P2 已加 variant + `DaoxiangOrigin` component | P2 plan §4.2-4.4 |
| 基础 Daoxiang brain | P2 plan §4.3 描述的 chase + attack nearest | P2 plan（实施后落地在 `npc/brain.rs`） |
| `LootContainer` + `KeyKind` | 钥匙 template 约定 + 查找 helper | P3 plan §1 |
| `Cultivation.spirit_qi` | 真元池（Fuya aura 作用对象） | `server/src/cultivation/components.rs` |
| `TsyPresence` | 玩家秘境会话状态（Fuya aura 作用读 TSY presence） | P0 plan §1.3 |

**本 plan 要新增**：`NpcArchetype` 扩展（`Zhinian`、`Fuya`）+ `TsySentinel` tag 复用 `GuardianRelic` + 4 类 archetype 的 big-brain 差异化 tree + `TsyOrigin` enum + `TsySpawnPool` 配置 + `NpcDropTable` 配置 + `FuyaAura` Component + aura 叠加到玩家 drain rate + 死亡 drop 生成（接 `DroppedLootRegistry.ownerless`）+ IPC schema `tsy-hostile-v1` 可选。

---

## §0 设计轴心（不可违反）

1. **4 档威胁结构**：基础 / 精英 / Boss / 环境威胁 — 对应 Tarkov 的 Scav / Raider / Boss / Cultist，但语义锚定到末法残土 lore（§十六.五 敌对 NPC）
2. **起源决定 spawn 倾向** — 单个 TSY 以一种起源为主；每种起源有 PVE 倾向 pool（§十六.五 起源 → 敌人倾向性表）
3. **道伥 = 过去玩家回声** — 道伥生前境界决定当前强度（P2 `DaoxiangOrigin.from_corpse.realm`）；drop 是生前装备磨损版（见 §十六.五 表格）
4. **执念有智能 = 世界最危险的非 Boss PVE** — 会伪装成道伥靠近；玩家不能用"所有僵尸都笨"的经验
5. **秘境守灵是**上古宗门遗迹的**物理守护装置**（不是活物）— 不怕真元抽、不老死、不能移动位置；阶段性 boss（§十六.五 第 3 行）
6. **负压畸变体是环境灾难 + 少量 PVE** — 其核心价值不是战斗挑战，而是"**在场逼你加速抽真元**"的 AOE debuff（§十六.五 第 4 行）
7. **所有 TSY NPC 死后 drop 进 `DroppedLootRegistry.ownerless`** — 不归属玩家 corpse，放在 NPC 死亡地点；谁捡归谁
8. **NPC 死亡不计入**秘境 race-out **倒计时驱动**（仅玩家取 RelicCore 才触发塌缩，NPC 死与否无关）— 见 §十六.一 "遗物 = 骨架" 铁律
9. **起源不影响 boss 密度** — 守灵数量由 zone 元数据固定（宗门遗迹类 1-3 个，大能陨落 0-1，战场 / 近代高手 0），与 spawn pool 随机无关
10. **不引入新的 movement 模式** — 复用现有 `MovementController` + `Navigator`；Fuya 慢速用 `MovementCapabilities.max_speed = 0.6`，守灵不可移动用 `Stationary` marker

---

## §1 `NpcArchetype` 扩展

### 1.1 新增两个 variant

**位置**：`server/src/npc/lifecycle.rs:40-72` 扩展

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
pub enum NpcArchetype {
    #[default]
    Zombie,
    Commoner,
    Rogue,
    Beast,
    Disciple,
    GuardianRelic,
    Daoxiang,   // ← P2 已加
    Zhinian,    // ← P4 新加：执念
    Fuya,       // ← P4 新加：负压畸变体
}

impl NpcArchetype {
    pub const fn as_str(self) -> &'static str {
        match self {
            // ... 已有 variant 省略
            Self::Daoxiang => "daoxiang",
            Self::Zhinian  => "zhinian",
            Self::Fuya     => "fuya",
        }
    }

    pub const fn default_max_age_ticks(self) -> f64 {
        match self {
            // ... 已有 variant 省略
            Self::Daoxiang => 120_000.0,      // = Zombie（道伥是 Zombie 的 lore 镜像）
            Self::Zhinian  => 180_000.0,      // 比道伥长——残念存留更久
            Self::Fuya     => 240_000.0,      // 畸变体稳定性高
            // GuardianRelic 保持 1_000_000.0
        }
    }
}
```

### 1.2 `TsySentinel` tag（复用 `GuardianRelic`）

**不新增 archetype variant**。秘境守灵语义上已贴合 `GuardianRelic`（护主物、不死、固定点位），只加一个 tag Component 区别 overworld 守灵和 TSY 守灵：

```rust
// 位置：server/src/npc/tsy_hostile.rs（新建）

#[derive(Component, Debug)]
pub struct TsySentinelMarker {
    pub family_id: String,           // 所属 TSY
    pub guarding_container: Option<Entity>,  // 守护的传说档容器（None = 通道守灵）
    pub phase: u8,                   // 当前阶段（0 开始）
    pub max_phase: u8,               // 总阶段数（2-3）
}
```

**spawn 时**：`NpcArchetype::GuardianRelic` + `TsySentinelMarker`。`brain` 看到 `TsySentinelMarker` 存在就走守灵 AI 分支；不存在（普通 `GuardianRelic`）走 overworld 护主分支。

### 1.3 起源 enum（zone metadata）

**位置**：`server/src/world/tsy_origin.rs`（新建）

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TsyOrigin {
    DanengLuoluo,     // 大能陨落
    ZongmenYiji,      // 上古宗门遗迹
    ZhanchangChendian,// 上古战场沉淀
    GaoshouShichu,    // 近代高手死处
}

impl TsyOrigin {
    /// 从 zone name 提取起源
    /// e.g. "tsy_zongmen_lingxu_01_deep" → ZongmenYiji
    pub fn from_zone_name(name: &str) -> Option<Self> {
        if !name.starts_with("tsy_") { return None; }
        let body = &name[4..];
        if body.starts_with("daneng_")    { Some(Self::DanengLuoluo) }
        else if body.starts_with("zongmen_")   { Some(Self::ZongmenYiji) }
        else if body.starts_with("zhanchang_") { Some(Self::ZhanchangChendian) }
        else if body.starts_with("gaoshou_")   { Some(Self::GaoshouShichu) }
        else { None }
    }
}
```

**集成点**：P3 plan `TsyOriginModifier` 共用此 enum（P3 / P4 都读 zone name 前缀），确保容器 spawn 和 NPC spawn 对同一 zone 的 origin 判断一致。Meta plan §2 横切追加 `2.5 TsyOrigin 共享 enum`（由**本 P4 plan 接纳**，因为 P3 plan 仅需要一个映射表而 P4 需要 enum 本身做 pattern match）。

---

## §2 AI Trees

### 2.1 道伥（Daoxiang）— 基础 PvE 扩展

**P2 plan §4.3 已定义基础 brain**：`ChaseTargetScorer → ChaseAction`（向玩家移动）+ `MeleeRangeScorer → MeleeAttackAction`（近战）。

**P4 扩展**：加入"生前战斗本能触发"——当玩家背对 / 真元低时，短时间爆发生前招式片段：

```rust
// 新 Scorer
#[derive(Clone, Copy, Debug, Component)]
pub struct DaoxiangInstinctScorer;

// 触发条件：
// - 玩家在 MELEE_RANGE 内
// - 玩家背对（玩家朝向和道伥朝向夹角 > 90°）
//   OR 玩家 spirit_qi / spirit_qi_max < 0.2
// - 道伥的 instinct_cooldown = 0

// 新 Action
#[derive(Clone, Copy, Debug, Component)]
pub struct DaoxiangInstinctAction;

// 效果：
// - 播放生前招式（依 DaoxiangOrigin.from_corpse.realm / tags）
// - 爆发一次 AttackIntent 伤害 = 正常近战的 2.5x
// - 触发后设 instinct_cooldown = 600 tick (30s)
```

**BigBrain thinker**（道伥）：

```
Thinker::build()
    .picker(Highest)
    .when(DaoxiangInstinctScorer, DaoxiangInstinctAction)
    .when(MeleeRangeScorer,       MeleeAttackAction)
    .when(ChaseTargetScorer,      ChaseAction)
    .otherwise(WanderAction)
```

**寻路 cap**：道伥 `Navigator` 有限 range (~16 block)，超出就停下游荡——符合"笨 + 本能" lore。

### 2.2 执念（Zhinian）— 精英 PvE

**差异化**：能假装道伥靠近（慢速 pace 模仿道伥巡逻），进入近战范围瞬间切换到爆发模式。

```rust
#[derive(Component, Debug)]
pub struct ZhinianMind {
    pub phase: ZhinianPhase,       // Masquerade / Aggressive
    pub phase_entered_at_tick: u64,
    pub combat_memory: CombatCombo, // 生前招式序列（从 NPC init config 读）
}

#[derive(Debug, Clone, Copy)]
pub enum ZhinianPhase {
    Masquerade,   // 伪装态
    Aggressive,   // 战斗态
}

#[derive(Debug, Clone)]
pub struct CombatCombo {
    pub steps: Vec<ComboStep>,
    pub current_step: usize,
}

#[derive(Debug, Clone)]
pub struct ComboStep {
    pub kind: ComboKind,           // Melee / Dash / Projectile
    pub cooldown_ticks: u32,
    pub damage_mul: f32,
}

#[derive(Debug, Clone)]
pub enum ComboKind {
    Melee,
    Dash,
    Projectile,
}
```

**新 Scorer / Action**：

```rust
#[derive(Component)]
pub struct ZhinianAmbushScorer;       // 玩家首次进入 8 block 内时触发

#[derive(Component)]
pub struct ZhinianComboStepAction;    // 依 CombatCombo.current_step 发起对应攻击
```

**Thinker**：

```
Thinker::build()
    .picker(FirstToScore)
    .when(ZhinianAmbushScorer,    BurstAttackAction)       // 首次进近 → 连击起手
    .when(MeleeRangeScorer,       ZhinianComboStepAction)  // 打 combo
    .when(ChaseTargetScorer,      ChaseAction)
    .otherwise(ZhinianPatrolAction)                        // Masquerade 时假装道伥
```

**伪装态行为**：
- `max_speed = 0.5`（和道伥一致）
- 不主动 chase，只巡逻
- 发现玩家 → phase 切到 Aggressive，max_speed 恢复 1.0

### 2.3 秘境守灵（TsySentinel）— Boss 级

**行为要点**：
- 固定位置（不移动）
- 守护传说档容器（`TsySentinelMarker.guarding_container`）
- 多阶段血量：血条被打到阈值（P0 plan 读 `Wounds.total_damage()`）→ 切阶段
- 每阶段有独占技能：

| 阶段 | 血量范围 | 技能 |
|------|---------|------|
| 0 | 100% - 67% | 单体远程投射（`ProjectileAction`） |
| 1 | 67% - 33% | 多体法阵地刺（`ArrayBurstAction`） |
| 2 | 33% - 0%  | 自爆冲击波 + 持续伤害区（`SelfDetonateAction`） |

**核心 Scorer**：

```rust
#[derive(Component)]
pub struct SentinelAggroScorer;       // 玩家进入 16 block 即 1.0

#[derive(Component)]
pub struct SentinelPhaseAction;       // 依 phase 分派技能
```

**Thinker**（守灵是 Stationary，不需要 chase）：

```
Thinker::build()
    .picker(Highest)
    .when(SentinelAggroScorer, SentinelPhaseAction)
    .otherwise(NoOpAction)
```

**阶段切换**：
- `Wounds` 监听 system，按血量百分比更新 `TsySentinelMarker.phase`
- 切阶段时触发一次"屏幕震动 + 尸气冲击波"（client 视觉 polish，非本 plan 范围）

**不可移动**：
- 在 `MovementCapabilities` 里设 `max_speed = 0.0`
- 或更显式：加 `Stationary` marker，`Navigator` 看到跳过

### 2.4 负压畸变体（Fuya）— 环境威胁

**核心特征**：耗真元光环 + 慢速追击 + 狂暴态。

**Components**：

```rust
#[derive(Component, Debug)]
pub struct FuyaAura {
    pub radius_blocks: f32,          // 默认 8.0
    pub drain_boost_multiplier: f32, // 默认 1.5（玩家 drain rate × 1.5）
}

#[derive(Component, Debug)]
pub struct FuyaEnragedMarker;        // 血量 < 30% 时附加
```

**movement**：`MovementCapabilities.max_speed = 0.6`（比道伥还慢）；狂暴时 × 1.5。

**Scorer**：

```rust
#[derive(Component)]
pub struct FuyaEnrageScorer;  // 血量 < 30% 且未 enraged → 评分 1.0

#[derive(Component)]
pub struct FuyaChargeScorer;  // enraged + 玩家在 12 block 内 → 冲刺

#[derive(Component)]
pub struct FuyaEnrageAction;  // insert FuyaEnragedMarker，buff stats
```

**Thinker**：

```
Thinker::build()
    .picker(Highest)
    .when(FuyaEnrageScorer,     FuyaEnrageAction)
    .when(FuyaChargeScorer,     DashAction)
    .when(MeleeRangeScorer,     MeleeAttackAction)
    .when(ChaseTargetScorer,    ChaseAction)
    .otherwise(WanderAction)
```

---

## §3 Fuya 耗真元光环（跨 plan 联动）

### 3.1 `apply_fuya_aura_drain` system

**输入**：所有带 `FuyaAura` 的 Fuya NPC 的位置；所有带 `TsyPresence` 的玩家的位置。

**逻辑**：每 tick 遍历玩家，检查是否在任何 `FuyaAura` 半径内，若是 → 叠加 drain multiplier。

**接入 P0 drain 公式**（同 P3 容器加速的做法）：

```rust
fn compute_drain_per_tick_p4(
    zone: &Zone,
    player: &PlayerState,
    presence: &TsyPresence,
    searching: Option<&SearchProgress>,      // P3 接入
    fuya_aura_stack: f32,                    // P4 接入（1.0 = 不受 aura，1.5 = 一个 aura，1.5^2 = 两个叠加）
) -> f64 {
    let base = compute_drain_per_tick(zone, player, presence);
    let mut mul = 1.0;
    if searching.is_some() { mul *= 1.5; }  // P3
    mul *= fuya_aura_stack as f64;          // P4
    base * mul
}
```

**叠加规则**：多个 Fuya aura 覆盖同一玩家 → **相乘叠加**（1.5 × 1.5 = 2.25）。设计意图：玩家误入多个畸变体的 aura 交叠区 → 真元哗啦啦掉，强制速撤。

### 3.2 client 可视化提示

- Fuya NPC 脚下显示**紫黑色雾气**粒子（client 视觉 polish）
- 玩家进入 aura 半径时 HUD 闪"真元加速流失"红字警报（1 秒）
- 离开后红字消失

---

## §4 Spawn Rule

### 4.1 `TsySpawnPool` 配置（`server/tsy_spawn_pools.json`）

```json
{
  "by_origin": {
    "daneng_luoluo": {
      "shallow": { "daoxiang": 2, "zhinian":  0, "fuya": 0 },
      "mid":     { "daoxiang": 4, "zhinian":  2, "fuya": 0 },
      "deep":    { "daoxiang": 6, "zhinian":  4, "fuya": 1 }
    },
    "zongmen_yiji": {
      "shallow": { "daoxiang": 3, "zhinian":  0, "fuya": 0 },
      "mid":     { "daoxiang": 5, "zhinian":  1, "fuya": 0 },
      "deep":    { "daoxiang": 8, "zhinian":  2, "fuya": 0 }
    },
    "zhanchang_chendian": {
      "shallow": { "daoxiang": 4, "zhinian":  0, "fuya": 0 },
      "mid":     { "daoxiang": 6, "zhinian":  0, "fuya": 1 },
      "deep":    { "daoxiang": 10, "zhinian": 1, "fuya": 3 }
    },
    "gaoshou_shichu": {
      "shallow": { "daoxiang": 2, "zhinian":  0, "fuya": 0 },
      "mid":     { "daoxiang": 4, "zhinian":  0, "fuya": 0 },
      "deep":    { "daoxiang": 6, "zhinian":  1, "fuya": 0 }
    }
  },
  "sentinel_count_by_origin": {
    "daneng_luoluo":      1,
    "zongmen_yiji":       3,
    "zhanchang_chendian": 0,
    "gaoshou_shichu":     0
  }
}
```

**读逻辑**：`load_tsy_spawn_pools() -> TsySpawnPoolRegistry`（启动加载）。

### 4.2 spawn 触发：`/tsy-spawn` 命令拓展

P3 plan 已有 `/tsy-spawn <family_id>` 命令（spawn 容器）；P4 plan 扩展同一命令，让它在 spawn 容器后也调用 `spawn_tsy_hostiles(family_id)`：

```rust
fn spawn_tsy_hostiles(
    family_id: &str,
    registry: &TsySpawnPoolRegistry,
    zone_registry: &ZoneRegistry,
    container_query: Query<(Entity, &LootContainer)>,
    mut commands: Commands,
) {
    let origin = TsyOrigin::from_zone_name(family_id).unwrap();
    let pool = registry.by_origin.get(&origin).unwrap();

    for (layer, pool_layer) in [("shallow", &pool.shallow), ("mid", &pool.mid), ("deep", &pool.deep)] {
        let zone_name = format!("{family_id}_{layer}");
        let zone = zone_registry.find_zone_by_name(&zone_name).unwrap();

        for _ in 0..pool_layer.daoxiang { spawn_npc(commands, &zone, NpcArchetype::Daoxiang); }
        for _ in 0..pool_layer.zhinian  { spawn_npc(commands, &zone, NpcArchetype::Zhinian); }
        for _ in 0..pool_layer.fuya     { spawn_npc(commands, &zone, NpcArchetype::Fuya); }
    }

    // Sentinel 独立逻辑：每个 deep layer 的 relic_core 容器绑定一个守灵
    let sentinel_count = registry.sentinel_count_by_origin.get(&origin).copied().unwrap_or(0);
    let relic_cores: Vec<_> = container_query.iter()
        .filter(|(_, c)| c.kind == ContainerKind::RelicCore && c.family_id == family_id)
        .take(sentinel_count as usize)
        .collect();

    for (container_ent, container) in relic_cores {
        spawn_sentinel(commands, container, Some(container_ent));
    }
}
```

### 4.3 Daoxiang spawn 特例：道伥来源

P2 plan §4 已规划"玩家干尸 / 历史 corpse → 道伥"自动转化。P4 plan 的道伥 spawn 有两条路径：

- **路径 A（P2 提供）**：玩家死在 TSY → corpse 躺 5 分钟 → 转化为道伥。这条路径受塌缩影响（zone 塌缩则道伥 50% 被"喷出"到主世界，见 P2 §5）
- **路径 B（本 P4 plan 提供）**：zone 初始化时按 spawn pool 预置道伥，模拟"历代死在这里的修士累积"。这些道伥的 `DaoxiangOrigin.from_corpse = None`（合成道伥，非来自具体玩家）

两路并行——玩家进 TSY 一眼看到的道伥大部分来自路径 B（预置），战斗死亡后几分钟看到的道伥来自路径 A（本场玩家 corpse 转化）。

### 4.4 spawn 位置分布策略

- **Daoxiang**：zone AABB 内均匀分布（每 16 block 至少一个），避开 `blocked_tiles`
- **Zhinian**：放在"通道汇点"（patrol_anchors 附近），离玩家入口距离 > 24 block 避免无脑自杀式突击
- **Fuya**：放在深层的狭窄腔体（模拟其守在咽喉要道）；每个 Fuya 的 aura 半径 8 block，保证玩家有至少一条不过 aura 的路径
- **TsySentinel**：绑定具体 RelicCore 容器位置（紧挨容器 2 block 内站立）

---

## §5 Drop Table

### 5.1 `NpcDropTable` 配置（`server/tsy_drops.json`）

```json
{
  "daoxiang": {
    "guaranteed": [],
    "rolls": [
      { "template_id": "iron_sword_worn",     "chance": 0.50, "count": [1, 1] },
      { "template_id": "bone_coin_dead",      "chance": 0.70, "count": [1, 3] },
      { "template_id": "manual_fragment",     "chance": 0.10, "count": [1, 1] },
      { "template_id": "broken_artifact",     "chance": 0.05, "count": [1, 1] },
      { "template_id": "key_stone_casket",    "chance": 0.05, "count": [1, 1] }
    ],
    "max_rolls": 4
  },

  "zhinian": {
    "guaranteed": [
      { "template_id": "tattered_robe_elite", "count": [1, 1] }
    ],
    "rolls": [
      { "template_id": "mid_tier_sword",       "chance": 0.40, "count": [1, 1] },
      { "template_id": "ancient_relic_shard",  "chance": 0.15, "count": [1, 1] },
      { "template_id": "manual_fragment",      "chance": 0.30, "count": [1, 2] },
      { "template_id": "key_jade_coffin",      "chance": 0.08, "count": [1, 1] }
    ],
    "max_rolls": 3
  },

  "tsy_sentinel": {
    "guaranteed": [
      { "template_id": "__ancient_relic_random__", "count": [1, 2] },
      { "template_id": "__origin_keyed_key__",     "count": [1, 1] }
    ],
    "rolls": [
      { "template_id": "sect_token",               "chance": 0.50, "count": [1, 1] },
      { "template_id": "rare_spirit_herb",         "chance": 0.30, "count": [1, 2] }
    ],
    "max_rolls": 2
  },

  "fuya": {
    "guaranteed": [
      { "template_id": "beast_bone_chunk",         "count": [2, 5] }
    ],
    "rolls": [
      { "template_id": "mutated_core",             "chance": 0.70, "count": [1, 1] },
      { "template_id": "dense_fur_tough",          "chance": 0.40, "count": [1, 1] }
    ],
    "max_rolls": 2
  }
}
```

### 5.2 特殊 sentinel 模板

- `__ancient_relic_random__` — 和 P3 plan 的 `LootContainer` loot pool 共用 sentinel；由 `AncientRelicTemplate::random_roll()` 解析
- `__origin_keyed_key__` — **新增**，基于守灵所在 zone 的 origin 决定 drop 哪把钥匙：
  - `daneng_luoluo` / `zhanchang_chendian` → `key_array_core`（守的是 relic_core）
  - `zongmen_yiji` → `key_stone_casket` 或 `key_array_core` 按绑定的容器类型决定
  - `gaoshou_shichu` → 无守灵（sentinel_count = 0）

**resolver**：

```rust
fn resolve_drop_template(template_id: &str, ctx: &DropContext) -> Option<ItemInstance> {
    match template_id {
        "__ancient_relic_random__" => ancient_relic_random_roll(),
        "__origin_keyed_key__"     => {
            let guarding_kind = ctx.guarding_container_kind?;
            match guarding_kind {
                ContainerKind::RelicCore => template_as_item("key_array_core"),
                ContainerKind::StoneCasket => template_as_item("key_stone_casket"),
                _ => None,
            }
        }
        _ => template_as_item(template_id),
    }
}
```

### 5.3 Drop 事件：`handle_npc_death_drop`

```rust
fn handle_npc_death_drop(
    mut events: EventReader<DeathEvent>,
    npcs: Query<(&NpcArchetype, &Transform, Option<&TsySentinelMarker>, Option<&DaoxiangOrigin>), With<NpcMarker>>,
    drops: Res<NpcDropTable>,
    mut loot_registry: ResMut<DroppedLootRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
) {
    for e in events.read() {
        let Ok((archetype, tf, sentinel, daoxiang_origin)) = npcs.get(e.target) else { continue };

        let drop_key = match archetype {
            NpcArchetype::Daoxiang => "daoxiang",
            NpcArchetype::Zhinian  => "zhinian",
            NpcArchetype::GuardianRelic if sentinel.is_some() => "tsy_sentinel",
            NpcArchetype::Fuya     => "fuya",
            _ => continue,  // 非 TSY archetype 不走本 drop
        };

        let Some(entry) = drops.0.get(drop_key) else { continue };
        let ctx = DropContext {
            guarding_container_kind: sentinel.and_then(|s| s.guarding_container.and_then(|c| lookup_container_kind(c))),
            daoxiang_source_realm: daoxiang_origin.and_then(|o| o.from_corpse.as_ref().map(|c| c.realm)),
        };

        let items = roll_drop_entry(entry, &ctx, &mut allocator);
        let world_pos = tf.translation.as_array_f64();

        for item in items {
            loot_registry.ownerless.push(DroppedLootEntry {
                instance_id: item.instance_id,
                source_container_id: String::from("npc_drop"),
                source_row: 0,
                source_col: 0,
                world_pos,
                item,
            });
        }
    }
}
```

**挂钩时机**：放在 `combat::events::DeathEvent` 消费链末尾，保证 wounds / bleed_out / revive 都处理完后再 drop。

### 5.4 Daoxiang 装备传承

当 `DaoxiangOrigin.from_corpse = Some(PreviousCorpse { realm, equipment_snapshot })` 存在时：

- `daoxiang` drop 表的 `iron_sword_worn` 等条目被 **override 为** `equipment_snapshot` 的对应物件（durability × 0.5 磨损）
- `from_corpse = None` 时走默认 drop 表

```rust
// 伪代码
let items = if let Some(corpse) = ctx.daoxiang_source_corpse {
    // 路径 A: 玩家转化的道伥 → drop 生前装备的磨损版本
    apply_wear_to_snapshot(&corpse.equipment_snapshot, 0.5)
} else {
    // 路径 B: 预置道伥 → 走 drop 表
    roll_drop_entry(entry, &ctx, &mut allocator)
};
```

---

## §6 IPC schema（非必须，可延后）

如果 agent 层需要感知秘境 NPC 动态（给 narration 用）：

```typescript
// agent/packages/schema/src/tsy-hostile-v1.ts

export const TsyNpcSpawnedV1 = Type.Object({
  family_id: Type.String(),
  archetype: Type.Union([
    Type.Literal('daoxiang'),
    Type.Literal('zhinian'),
    Type.Literal('guardian_relic_sentinel'),  // TsySentinelMarker 存在的 GuardianRelic
    Type.Literal('fuya'),
  ]),
  count: Type.Number(),
  at_tick: Type.Number(),
});

export const TsySentinelPhaseChangedV1 = Type.Object({
  family_id: Type.String(),
  container_entity_id: Type.Number(),
  phase: Type.Number(),
  max_phase: Type.Number(),
  at_tick: Type.Number(),
});
```

**本 plan 范围**：实装 schema 定义，但 narration 消费侧由独立 plan（agent 层）做；P4 plan 只负责 publish。

---

## §7 验收 demo

**E2E 场景**（P4 单阶段）：

1. `/tsy-spawn tsy_zongmen_lingxu_01` 启动一个**宗门遗迹类** TSY
2. 检查 spawn pool：浅 3 道伥、中 5 道伥 + 1 执念、深 8 道伥 + 2 执念；3 个守灵绑 3 个 RelicCore（因为 origin = zongmen_yiji 的 sentinel_count = 3）
3. 玩家 A 进 TSY 浅层，遇 3 个道伥，击杀 → 每个道伥 drop 1-2 件凡铁 + 骨币，**5% 掉钥匙**：20 个道伥平均 1 把钥匙
4. A 下中层遇执念 1 个，贴近发现 `ZhinianPhase::Masquerade` 行为（慢速巡逻）→ A 靠近到 6 block → 执念切 `Aggressive` → A 措手不及被击中（ambush 机制生效）
5. A 躲开执念（或杀了），下深层
6. 深层入口有一个 Fuya，A 靠近 → HUD 闪"真元加速流失"红字 → spirit_qi drain 从 baseline 0.3/s 飙到 0.45/s（× 1.5）
7. A 绕开 Fuya，走到深层中央一个 RelicCore 前 → 旁边是一个守灵（GuardianRelic + TsySentinelMarker）→ 守灵 aggro → 打 3 段血量，每段有不同技能
8. 守灵死 → drop 1-2 件上古遗物（ancient_relic_random）+ 1 把阵核钤（key_array_core）+ 宗门 token
9. A 用阵核钤搜 RelicCore（P3 plan 验收）→ 拿上古遗物 → 触发 P2 `relics_remaining -= 1`
10. A 试图撤出（P5 plan 验收），途中遇被 P2 系统 spawn 的新道伥（来自之前死的执念转化？—— 执念不转化，只有玩家死才转）

**自动化测试**：

- `server/src/npc/tsy_hostile.rs::tests`：
  - `TsyOrigin::from_zone_name` 解析
  - spawn pool by origin 对齐（数量 / 种类）
  - sentinel_count 对齐
  - `DaoxiangInstinctScorer` 触发条件（玩家背对 / 真元低）
  - `FuyaAura` drain stack 叠加（1.5 / 1.5^2）
  - `resolve_drop_template` 特殊 sentinel 解析

- 集成：`cargo test tsy_hostile` 通过

---

## §8 非目标（推迟 / 独立 plan）

| 功能 | 状态 | 说明 |
|------|------|------|
| 守灵独占视觉 / 特效 | P4 后续 | 法阵地刺、自爆特效交 client 视觉 plan |
| Zhinian 用**玩家录像**的招式片段 | 独立 plan | MVP 用 hardcoded combo；后续接 `LifeRecord` 系统的"死者招式回放" |
| Fuya 种族变种（战场蛇 / 古兽 / 妖蝠） | 独立 plan | MVP 所有 Fuya 外观 / stat 相同 |
| Daoxiang 出坍缩渊后在主世界的行为 | 扩展 plan-npc-ai | 见 P2 lifecycle §5 "道伥喷出" 的主世界行为 |
| NPC 智能协同（道伥群 AI / 执念呼叫支援） | 不做 | 保持各自独立，避免 overengineering |
| 玩家**伪造道伥**潜入 | 不做 | 修仙 lore 不支持玩家变道伥 |
| 守灵被杀后"解除"绑定容器的锁 | 不做 | 守灵死不死都要用钥匙 / 令牌开容器 —— 钥匙从守灵身上 drop |

---

## §9 风险 / 未决

| 风险 | 级别 | 缓解 |
|------|------|------|
| big-brain Thinker 新 Scorer 数量多 → 性能开销 | 中 | 每 tick 只有 10-30 NPC 跑 Scorer，可承受；若 Zhinian Thinker 复杂到开销明显再拆 |
| `DaoxiangInstinctScorer` 误触（玩家背对队友时） | 中 | 明确 Scorer 读"玩家朝向 vs 道伥朝向 > 90°"，不读"是否看向道伥" |
| Fuya aura 半径重叠导致玩家一刀被抽 | **高** | `drain_boost_multiplier` 相乘叠加设计即预期；只需确保 Fuya spawn 距离 > aura 半径 × 1.5，避免自然重叠 |
| `GuardianRelic` + `TsySentinelMarker` 和 overworld 的 `GuardianRelic` 语义冲突 | 低 | 布尔 check 明确：有 marker 走 TSY 分支，无 marker 走原分支 |
| 守灵 phase 切换时旧技能未打完 → buggy state | 中 | system 强制打断旧 `Action`，`ActionState::Cancelled` → 立即进入新 phase |
| `NpcArchetype::Zhinian` 扩展破坏现有 test | 中 | 新 variant 不改 default；`default_max_age_ticks` 补新 arm；runtime_bundle 已泛化 |
| `apply_fuya_aura_drain` 和 P3 `apply_search_drain_multiplier` 合并时签名冲突 | 高 | P4 开工前和 P3 对齐 drain 函数签名——最终版本是 P0 定义的"多 multiplier 注入点"设计；若 P3 实装先于 P4，P4 打 patch 即可 |
| NPC drop 溢出：多个 Daoxiang 同时在同一格死 → Drop 堆叠冲突 | 低 | `DroppedLootEntry.world_pos` 加 ±0.5 block jitter |
| 预置道伥数量过多 → zone 初始化时 entity spawn spike | 中 | `/tsy-spawn` 命令分帧 spawn（每帧最多 5 个），避免一次 spawn 30+ NPC |

---

## §10 命名与版本

- 本 plan 文件：`plan-tsy-hostile-v1.md`
- 实施后归档：`docs/finished_plans/plan-tsy-hostile-v1.md`
- v2 触发条件：Zhinian 接入 LifeRecord 招式录像 / 守灵视觉 plan / Fuya 变种扩展

---

**下一步**：P0/P1/P2/P3 全部 demoable 后，`/consume-plan tsy-hostile` 启动 P4。

---

## §11 进度日志

- 2026-04-25：P4 仍为纯设计骨架，零代码落地。`server/src/npc/lifecycle.rs:41-49` 的 `NpcArchetype` 只有 6 个 variant（`Zombie/Commoner/Rogue/Beast/Disciple/GuardianRelic`），尚未加入 P2 占位的 `Daoxiang` 或 P4 的 `Zhinian/Fuya`；`server/src/npc/` 下无 `tsy_hostile.rs`，`server/src/world/` 下无 `tsy_origin.rs`，仓库根也无 `tsy_spawn_pools.json` / `tsy_drops.json`。整个 TSY 位面（P0 `TsyPresence` / P3 容器 / P2 lifecycle Daoxiang）前置链未启动，须等 `/consume-plan tsy-dimension`→`tsy-zone`→`tsy-lifecycle`→`tsy-container` 串行落地后再开 P4。
- **2026-04-26**：**P-1 解冻** — `plan-tsy-dimension-v1` 已 PR #47（merge 579fc67e）合并；串行链中 `tsy-dimension` 已划掉，剩 `tsy-zone`→`tsy-lifecycle`→`tsy-container` 三档串行前置仍未启动。本 plan 仍 ⬜。
- **2026-04-27**：**前置全部就位 + 主体落地** — `tsy-zone` (#49 `bd349286`) / `tsy-zone-followup` (#50 `29f8033c`) / `tsy-lifecycle` (#54 `99c29ebd`) / `tsy-container` (#55 `d6e84e37`) 全部 merged，串行前置链清空。本 plan 主体 PR `9d05e622` 于 23:50 merged：
  - §1 `NpcArchetype` ✅：`Daoxiang` / `Zhinian` / `Fuya` / `TsySentinelMarker` 三 variant + 守灵标记全部接入 `npc/lifecycle.rs`
  - §2 AI Trees ✅：`server/src/npc/tsy_hostile.rs` 1936 行，4 套差异化 thinker（`DaoxiangInstinctScorer` / `ZhinianAmbush` / `SentinelPhase` / `FuyaEnrage`）+ 9 单测
  - §3 Fuya aura ✅：`FuyaAura` component + `compute_fuya_aura_drain_multiplier()` 接入 `world/tsy_drain.rs`
  - §4 Spawn Pool ✅：`tsy_spawn_pools.json` + `DEFAULT_TSY_SPAWN_POOLS_PATH`；`TsyOrigin` enum @ `world/tsy_origin.rs`（4 值：DanengLuoluo/ZongmenYiji/ZhanchangChendian/GaoshouShichu）
  - §5 Drop Table ✅：`tsy_drops.json` + `DEFAULT_TSY_DROPS_PATH`，`npc/loot.rs` 接入 archetype 查询
  - §6 IPC schema ✅：`TsyNpcSpawnedV1` / `TsySentinelPhaseChangedV1` 在 `server/schema/tsy_hostile.rs`
- **2026-04-28**：审核反馈修复 `6e964407` (07:45)，schema/network/loot 同步 4 个 follow-up commit。本 plan 全 P 落地，剩归档准备。

---

## Finish Evidence

### 落地清单

- **§1 NpcArchetype 扩展**：`server/src/npc/lifecycle.rs:51-95` — `NpcArchetype` 加 `Daoxiang` / `Zhinian` / `Fuya` 三 variant + `as_str()` + `default_max_age_ticks()`（Daoxiang 实际取 1_000_000.0 承接 lifecycle "不老" 语义而非 plan 草稿的 120_000.0；Zhinian 180_000 / Fuya 240_000）
- **§1.2 TsySentinelMarker tag**：`server/src/npc/tsy_hostile.rs:62`（`family_id` / `guarding_container` / `phase` / `max_phase`）
- **§1.3 TsyOrigin enum**：`server/src/world/tsy_origin.rs:6-10` — 4 variant（`DanengLuoluo` / `ZongmenYiji` / `ZhanchangChendian` / `GaoshouShichu`）+ `from_zone_name` + `from_origin_key`
- **§2 AI Trees**：`server/src/npc/tsy_hostile.rs`（1936 行）— 4 套差异化 thinker：
  - 道伥：`DaoxiangInstinctScorer` (line 161) → `DaoxiangInstinctAction` (line 893 thinker)
  - 执念：`ZhinianMind` (line 99) + `ZhinianAmbushScorer` (line 167) → `ZhinianComboStepAction` (line 903 thinker)
  - 守灵：`SentinelAggroScorer` + `SentinelPhaseAction` (line 179) → 阶段切换 system (line 1195) + `TsySentinelPhaseChanged` event (line 304)
  - 畸变体：`FuyaEnrageScorer` (line 182) + `FuyaEnrageAction` (line 188) + `FuyaEnragedMarker` (line 85)
- **§3 Fuya aura**：`compute_fuya_aura_drain_multiplier` @ `server/src/npc/tsy_hostile.rs:926`，接入 `server/src/world/tsy_drain.rs:96` 与 `compute_drain_per_tick` 相乘叠加
- **§4 Spawn Pool**：`server/tsy_spawn_pools.json` + `DEFAULT_TSY_SPAWN_POOLS_PATH` (`tsy_hostile.rs:53`)；`TsySpawnPoolRegistry` (line 231) + `load_tsy_spawn_pool_registry` (line 396)；接入 `server/src/world/tsy_dev_command.rs:165` (`hostile_specs: Option<Res<TsySpawnPoolRegistry>>`)
- **§5 Drop Table**：`server/tsy_drops.json` + `DEFAULT_TSY_DROPS_PATH` (`tsy_hostile.rs:54`)；`server/src/npc/loot.rs:93/98/102` 按 `Daoxiang` / `Zhinian` / `Fuya` archetype 派生 drop entries
- **§6 IPC schema**：
  - server `server/src/schema/tsy_hostile.rs:18` `TsyNpcSpawnedV1` / line 29 `TsySentinelPhaseChangedV1`
  - agent `agent/packages/schema/src/tsy-hostile-v1.ts:18/39` TypeBox 双端契约 + `validateTsyNpcSpawnedV1Contract` / `validateTsySentinelPhaseChangedV1Contract`
  - registry 接入 `agent/packages/schema/src/schema-registry.ts:142-279`
  - 运行时投递 `server/src/network/redis_bridge.rs:64/65` (`RedisOutbound::TsyNpcSpawned` / `TsySentinelPhaseChanged`)

### 关键 commit

- `3fe36222` (2026-04-27) — 定义敌对 NPC IPC schema（双端 contract 起手）
- `ed2d63f5` (2026-04-27) — 接入 TSY 敌对 NPC（首次落地）
- `ca0374bc` (2026-04-27) — 补齐敌对 NPC schema 产物
- `9d05e622` (2026-04-27) — 接入 TSY 敌对 NPC（主体 PR 合并提交）
- `a3d7e127` (2026-04-27) — 监听敌对 NPC 事件
- `6be3cf06` (2026-04-27) — 接线敌对 NPC Redis 事件
- `6e964407` (2026-04-27) — 修复敌对 NPC 审核反馈

### 测试结果

- `grep -c '#\[test\]' server/src/npc/tsy_hostile.rs` → **9** 单测（覆盖 Fuya aura drain 叠加、零半径、空集合、距离衰减、`TsyOrigin::from_zone_name` 等）
- `server/src/world/tsy_origin.rs:46-67` — `from_zone_name` 5 case 测试（4 origin 命中 + spawn 落空）
- `server/src/schema/tsy_hostile.rs:47-85` — sample JSON 双向序列化测试（`TsyNpcSpawnedV1` / `TsySentinelPhaseChangedV1`）
- `server/src/network/redis_bridge.rs:1384-1420` — Redis outbound 序列化 + `kind` 字段断言（`tsy_npc_spawned` / `tsy_sentinel_phase_changed`）

### 跨仓库核验

- **server**：
  - `NpcArchetype::{Daoxiang,Zhinian,Fuya}` @ `server/src/npc/lifecycle.rs:60-64`
  - `TsySentinelMarker` / `FuyaAura` / `ZhinianMind` / `FuyaEnragedMarker` @ `server/src/npc/tsy_hostile.rs:62/70/85/99`
  - 4 套 Scorer / Action（`DaoxiangInstinctScorer` / `ZhinianAmbushScorer` / `SentinelPhaseAction` / `FuyaEnrageScorer`）@ `server/src/npc/tsy_hostile.rs:161/167/179/182`
  - `compute_fuya_aura_drain_multiplier` @ `server/src/npc/tsy_hostile.rs:926`，drain 链接入 `server/src/world/tsy_drain.rs:96`
  - `TsyOrigin` 4 variant @ `server/src/world/tsy_origin.rs:6-10`
  - schema `TsyNpcSpawnedV1` / `TsySentinelPhaseChangedV1` @ `server/src/schema/tsy_hostile.rs:18/29`
  - Redis outbound 接线 @ `server/src/network/redis_bridge.rs:27/64/65`
- **agent**：
  - `TsyNpcSpawnedV1` / `TsySentinelPhaseChangedV1` TypeBox @ `agent/packages/schema/src/tsy-hostile-v1.ts:18/39`
  - schema registry 注入 @ `agent/packages/schema/src/schema-registry.ts:142-279`
- **client**：不涉及（client 视觉 polish 列入 §8 非目标，独立 plan）
- **worldgen**：不涉及

### 遗留 / 后续

- §6 schema 已 publish；agent 层 narration 消费侧由独立 plan 处理（plan 自报范围内不消费）。其他 §8 列出的视觉特效 / Zhinian LifeRecord 招式录像 / Fuya 种族变种 / 道伥喷出主世界行为均归独立 plan，本 plan 不再扩展。
