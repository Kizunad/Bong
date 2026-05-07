# Bong · plan-npc-virtualize-v1 · 骨架

NPC 隐式更新框架 **二态 MVP** —— 把"远方 NPC"完全移出 ECS 改为纯数据态 SoA，全局后台批量推演（移动 / 灵气 / 寿元 / 突破），所有灵气流动**强制走 `qi_physics::ledger::QiTransfer` 守恒账本**（worldview §二）。当前架构所有 NPC 都是常驻 ECS entity（即使做完 plan-npc-perf-v1 也只能撑 100-200 hydrated），本 plan 把单服 NPC 总上限从 `max_npc_count = 512` 提升到 5000+，让 worldview §三「NPC 与玩家平等」+ §十一「散修江湖人来人往」+ 派系战争 / 师承代际更替有真实 NPC 数量基础。**v1 是二态 MVP**：玩家 ≤ 64 格 = Hydrated（ECS entity 全 system 跑），玩家 > 256 格 = Dormant（SoA 数据 + 全局 1/分钟推演），中间过渡态 Drowsy 留 v2（见 reminder.md）。

**交叉引用**：`plan-npc-ai-v1.md` ✅（hydrated NPC 的所有 Bundle / Scorer / Action / Lifespan / FactionStore）· `plan-npc-perf-v1.md` ⏳（hydrated NPC 跑得动）· `plan-qi-physics-v1.md` P1 ✅（ledger::QiTransfer + excretion + release）· `plan-agent-v2.md` ✅（NpcDigest 已是 dormant 友好的压缩表示，本 plan 复用）· `plan-tribulation-v1.md`（dormant NPC 渡虚劫强制 hydrate）· `plan-death-lifecycle-v1.md` §4b（dormant 老死走善终路径）

**worldview 锚点**：

- **§二 真元守恒**：dormant NPC 修炼吸收 zone qi / 释放回 zone / 老死归还，全部走 `qi_physics::ledger` 账本。SPIRIT_QI_TOTAL 全服恒定（const 当前 100.0）—— dormant NPC 不允许灵气凭空消失或生成，**这是底线红旗**
- **§三:124-187 NPC 与玩家平等**：dormant NPC 仍按 `rate_multiplier=0.3` 老化、按 cultivation 推境界、满足条件强制 hydrate 渡劫。规则不因"玩家不在场"而豁免
- **§十一:947-970 散修江湖**：5000+ NPC 总量是「人来人往」物理化身，512 上限不够撑起 worldview 设定
- **§十二:1043 寿元代际更替**：dormant NPC 的代际继承（凡人邻居生子 / 散修偶发批量 spawn）走 dormant 内部，不触发 hydrate 风暴
- **§P 真元浓度场**：dormant NPC 修炼受所在 zone EnvField 限制（远 zone 中心 = 距离衰减 distance.rs / 多 NPC 共 zone = 异体排斥 collision.rs）

**qi_physics 锚点**（所有 dormant 灵气调用走以下函数，**禁止 plan 内自定**）：

- `qi_physics::ledger::QiTransfer { from, to, amount }` —— 所有灵气转移记账
- `qi_physics::excretion::container_intake(npc.cultivation, zone, dt)` —— dormant NPC 修炼吸收量计算
- `qi_physics::release::release_to_zone(amount, zone)` —— dormant NPC 老死 / 战斗释放灵气回 zone
- `qi_physics::distance::attenuation(npc.position, zone.center)` —— 距 zone 中心衰减
- `qi_physics::collision::repulsion(npc.style_rho, others_in_zone)` —— 同 zone 多 NPC 异体排斥（worldview §P ρ 矩阵）
- `qi_physics::env::EnvField` —— zone 浓度场边界（dormant NPC 距 zone > 64 格不可吸收，对应 worldview §二「真元极易挥发」）
- `qi_physics::tiandao::era_decay(...)` —— 时代衰减不影响 dormant NPC 个体（已是全服级减项）

**前置依赖**：

- `plan-npc-ai-v1` ✅ → 所有 archetype Bundle / Cultivation / Lifespan / FactionMembership / Lineage / Reputation / NpcLootTable / NpcDigest / `bong:npc/{spawn,death}` 通道 / `max_npc_count` config
- `plan-npc-perf-v1` ⏳ → spatial index + navigator 分桶 + per-NPC FixedUpdate（hydrated NPC 性能基础；virtualize 前必须先把 hydrated 100 跑通）
- `plan-qi-physics-v1` P1 ✅ → ledger::QiTransfer + excretion + release + distance + collision API 冻结
- `plan-qi-physics-patch-v1` P0/P1/P2 ✅（守恒律 hot zones 完成迁移）
- `plan-agent-v2` ✅ → NpcDigest 已是远方 NPC 压缩表示，本 plan 把 dormant 内部状态原样塞进 NpcDigest 给天道 agent 推演

**反向被依赖**：

- `plan-npc-ai-v1.md §3.3` 代际更替 1000 NPC stretch goal → 本 plan 实装
- `plan-tribulation-v1.md` NPC 化虚名额 4 名 / 半步化虚等 → dormant NPC 触发渡劫由本 plan 强制 hydrate
- `plan-narrative-political-v1.md` ✅（agent 长期推演叙事）→ 5000+ NPC 数量基础是叙事丰度的物理前提
- `plan-quest-v1` 占位 → dormant NPC 派任务 / 拜师走 hydrate-on-demand
- `plan-multi-life-v1` ⏳ → dormant 老死走 plan-death §4b 善终路径，与玩家死亡同通道

---

## 接入面 Checklist

- **进料**：
  - 所有 hydrated NPC ECS Components（Position / NpcMarker / NpcArchetype / Cultivation / Lifespan / FactionMembership / Lineage / Reputation / NpcPatrol / MeridianSystem / Contamination / NpcLootTable / NpcBlackboard）
  - `qi_physics::env::EnvField`（zone 浓度场快照）
  - `ZoneRegistry`（zone 中心 / 半径 / spirit_qi 当前值）
  - 玩家 Position（每秒采样一次，决定哪些 NPC 该 hydrate / dehydrate）
  - 现有 `bong:npc/death` Redis 通道（dormant 老死复用）
- **出料**：
  - **`NpcDormantStore` Resource**（HashMap<CharId, NpcDormantSnapshot>，server 启动时从持久化加载，shutdown 持久化）
  - **`NpcDormantSnapshot` struct**（dehydrate 时 collect 所有持久化字段；hydrate 时反向 spawn ECS entity）
  - **`hydrate_npc_system`**（FixedUpdate 1Hz：检查每个 dormant NPC 距最近玩家，≤ 64 格 → 触发 hydrate）
  - **`dehydrate_npc_system`**（FixedUpdate 1Hz：检查每个 hydrated NPC 距最近玩家，> 256 格 → 触发 dehydrate）
  - **`dormant_global_tick_system`**（FixedUpdate 自定义频率：每 in-game 60s 跑一次，对所有 dormant NPC 推演移动 / 灵气 / 寿元 / 突破）
  - **`DormantBehaviorIntent` enum**（每 dormant NPC 当前意图：Wander / PatrolToward(BlockPos) / FleeFrom(BlockPos)）
  - 复用 `bong:npc/death`（dormant 老死同通道）
  - 扩展 `bong:npc/spawn` 标注 `from_dormant: bool`（hydrate 复活）
- **共享类型 / event**：
  - 复用 `NpcArchetype` / `Cultivation` / `Lifespan` / `FactionMembership` / `Lineage` / `Reputation`（不另造 dormant 专属）
  - 复用 `NpcDigest` 给天道 agent 推演（dormant NPC 直接出 NpcDigest）
  - **新增** `DormantSeveredAt(CharId, MeridianId)` event 桥接（dormant 期 SEVERED 由 plan-meridian-severed-v1 通用 event 触发，本 plan 仅扣 NpcDormantSnapshot.meridian_severed 字段）
  - **禁止新建** Cultivation / Lifespan / Faction 的 dormant 副本（孤岛红旗）
- **跨仓库契约**：
  - server: `npc::dormant::*` 主模块 + `npc::hydrate::*` 转换层
  - agent: 无 schema 变化（NpcDigest 通道 unchanged）；天道 agent 推演时同时看 hydrated + dormant，不区分
  - client: 无可见变化（hydrate 时 spawn ECS entity 自然走 Valence 实体协议；dehydrate 时 despawn 自然走）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：见头部

---

## §0 设计轴心

- [ ] **二态 MVP，跳过 Drowsy**：v1 仅 Hydrated（ECS）↔ Dormant（SoA），中间过渡态 Drowsy（ECS entity 但仅核心 system 1Hz tick）留 **plan-npc-virtualize-v2**。决策门 #1 验收：v1 完成后实测玩家穿越 64-256 格边界 hydrate/dehydrate 是否撕裂；如开销 > 5ms/NPC 或视觉撕裂明显 → v2 补 Drowsy
- [ ] **守恒律是底线，不是优化目标**：所有 dormant 灵气流动**必须**走 `qi_physics::ledger::QiTransfer`，不允许 `dormant.cultivation.qi_current += X`、不允许老死时灵气"凭空消失"。**这是 §3 强约束源头，违反 = 阻塞 merge**
- [ ] **NPC 与玩家平等不因 dormant 豁免**：dormant 老化按 `rate_multiplier=0.3`、cultivation 自动推境界（无 UI 自动选默认）、寿元到期老死走 plan-death §4b、满足条件渡虚劫强制 hydrate 走 plan-tribulation 完整流程。worldview §三:124-187 不因为"玩家看不见"就特殊对待 NPC
- [ ] **dormant 推演频率 = in-game 60s**（real-time 约 3s @ 20×时间倍率，可调）：足够感知"代际"流动，又不让 telemetry 爆炸。worldview §十二 1 real hour = 1 year 锚点 → 60s in-game = 1 day 衰老 ≈ 0.0027 year × rate_multiplier=0.3 ≈ 4.4 min real-time / 寿元年
- [ ] **Hysteresis 防抖**：hydrate 阈值 64 格 / dehydrate 阈值 256 格不对称（玩家在 64-256 之间徘徊不会反复转换）。决策门 #2 可调
- [ ] **dormant NPC 渡虚劫强制 hydrate**：不允许 dormant 期渡劫（叙事关键事件必须可见 + plan-tribulation 全服广播 + 截胡机制依赖 ECS）。条件触发 → 立刻 hydrate（即使 256 格外）→ broadcast tribulation event → 玩家可远程观摩 / 截胡
- [ ] **dormant NPC 间互动 v1 极简**：v1 不实装 dormant↔dormant 战斗 / 社交 / 师承演变。所有 dormant 互动**全权交天道 agent 演绎**（agent 已能看 NpcDigest 推演长期决策）。决策门 #3 拍板首版边界
- [ ] **dormant NPC 受玩家攻击 = 强制 hydrate**：玩家朝 dormant NPC 方向开火 / 投石 / 范围伤害命中 dormant 区域 → 立刻 hydrate 该 NPC（让伤害走正常 ECS 战斗链）。**dormant NPC 不可在 SoA 态被扣 HP**（避免守恒律绕过 + 战斗逻辑双份）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | **决策门 + 数据模型**：NpcDormantSnapshot 字段定稿（持久化字段全列）+ Hysteresis 阈值定值（64/256 vs 64/128 vs 32/128）+ dormant tick 频率定值（in-game 30s / 60s / 5min）+ 5 个开放问题（§8）收口 + 守恒律强约束清单（§3）落 plan + 持久化路径定（Redis HASH `bong:npc/dormant` vs sled DB） | 数据模型 + 5 决策 + 守恒律红线 全落 plan §2-§3 / §8 |
| **P1** ⬜ | **dehydrate / hydrate 双向通路**：`NpcDormantStore` Resource + `NpcDormantSnapshot` struct + `hydrate_npc_system` + `dehydrate_npc_system` + Hysteresis 防抖 + roundtrip 完整性测试（≥ 25 单测：所有持久化字段经 dehydrate→hydrate 不丢；Cultivation / MeridianSystem / FactionMembership / Lineage / Reputation 各自 roundtrip 验证；Hysteresis 64-256 反复进出不抖动） | `cargo test npc::dormant::hydrate` 全过 / 玩家走入 64 格 → ECS entity spawn / 玩家走出 256 格 → ECS entity despawn + SoA snapshot 写入 |
| **P2** ⬜ | **dormant 全局推演**：`dormant_global_tick_system`（in-game 60s 频率）+ 移动（按 archetype 默认行为：Wander 随机方向 / Patrol 巡逻点 / Flee 远离最近修士）+ 灵气消耗（**走 qi_physics::excretion + ledger::QiTransfer**）+ 寿元衰减 + 自动境界推进（plan-cultivation 自动选默认）+ ≥ 30 单测（各 archetype 移动正确性 / 灵气守恒：emit QiTransfer 总量 == zone 扣减总量 / 寿元到期触发 → §P3 老死路径 / 自动突破不触发 UI） | 守恒律全过：1000 dormant NPC 推演 1 小时 in-game / 所有 zone.spirit_qi 收支 == ledger 累计 emit 量 |
| **P3** ⬜ | **dormant 渡虚劫强制 hydrate + 老死通道 + agent 整合**：dormant NPC 满足渡劫条件 → 立刻 hydrate（即使 > 256 格）→ broadcast `bong:tribulation/start` → 走 plan-tribulation 完整流程（玩家可截胡）；dormant NPC 老死 → emit `bong:npc/death`（含 cause / archetype / faction / lifespan / 生平卷 snapshot）+ release 灵气回 zone 走 ledger（守恒）；天道 agent 推演时 NpcDigest 流同时含 hydrated + dormant（dormant 内部直接出 NpcDigest）| dormant 渡劫 e2e：玩家在 spawn 区域观察远方"灵脉波动"narration → 远方散修起劫 → hydrate → 全服广播 → 玩家可飞过去截胡 |
| **P4** ⬜ | **5000+ NPC 总量上限恢复 + e2e 压测**：`max_npc_count` config 拆为 `max_hydrated_count = 200` / `max_dormant_count = 5000`（默认值）+ scripts/start.sh `BONG_ROGUE_SEED_COUNT` 默认 100 hydrated + 1000 dormant（远方散修批量 spawn）+ CI e2e 跑 100 hydrated + 1000 dormant 30s ≥ 18 TPS + 单服 5000 dormant 推演基线录档（in-game 1 小时 = real-time 3min）+ 持久化测试（server restart dormant 不丢） | start.sh 默认 100/1000 / CI e2e green / 5000 dormant 1h in-game 推演基线写入 plan §9 |

---

## §2 数据模型

```rust
// server/src/npc/dormant/mod.rs（新模块）

#[derive(Resource, Default)]
pub struct NpcDormantStore {
    /// SoA 形式（按字段分 Vec 而非 Vec<Snapshot>，cache-friendly 且批量推演快）
    /// MVP 先用 HashMap<CharId, NpcDormantSnapshot>，P4 决策门考虑改 SoA
    snapshots: HashMap<CharId, NpcDormantSnapshot>,
    /// 索引：archetype → CharIds（批量推演时按 archetype 分桶）
    by_archetype: HashMap<ArchetypeId, Vec<CharId>>,
    /// 索引：所属 zone → CharIds（zone 灵气推演时批量处理）
    by_zone: HashMap<ZoneId, Vec<CharId>>,
}

#[derive(Clone)]
pub struct NpcDormantSnapshot {
    // 身份
    pub char_id: CharId,
    pub archetype: ArchetypeId,
    pub position: DVec3,                       // 虚拟坐标，dormant 期由 dormant_global_tick_system 推演
    // 修炼（必持久化，灵气流动入 ledger）
    pub cultivation: Cultivation,              // realm + qi_current + qi_max + xp
    pub meridian_system: MeridianSystem,       // 12 正经 + 8 奇经状态
    pub meridian_severed: MeridianSeveredPermanent, // 永久 SEVERED 列表（plan-meridian-severed-v1）
    pub contamination: Contamination,
    // 寿元
    pub lifespan: NpcLifespan,
    pub age_ticks: u64,
    // 派系 / 师承 / 声望
    pub faction: Option<FactionMembership>,
    pub lineage: Option<Lineage>,
    pub reputation: Reputation,
    // 行为意图（dormant 期持续，hydrate 后由 big-brain 接管）
    pub intent: DormantBehaviorIntent,
    pub patrol: Option<NpcPatrol>,
    // dormant 元数据
    pub dormant_since_tick: u64,
    pub last_dormant_tick_processed: u64,
    /// dormant 期累计 ledger 净额（debug + 守恒律审计）
    pub qi_ledger_net: f64,
}

pub enum DormantBehaviorIntent {
    /// 随机漂移（Commoner / Rogue idle）
    Wander { drift_radius: f64 },
    /// 朝目标点巡逻（Disciple / Beast Patrol）
    PatrolToward { target: BlockPos },
    /// 远离修士（Commoner Fear）
    FleeFrom { source: BlockPos, until_tick: u64 },
    /// 静坐修炼（Rogue / Disciple Cultivate）
    Cultivate { in_zone: ZoneId },
    /// 风烛归隐（Retire）
    Retire { destination: BlockPos },
}

// server/src/npc/hydrate/mod.rs（新模块）

pub fn dehydrate_npc(
    entity: Entity,
    components: DehydrateQuery, // 系统 query：所有需 collect 的 Component
    store: &mut NpcDormantStore,
    spatial: &mut NpcSpatialIndex,
    commands: &mut Commands,
);

pub fn hydrate_npc(
    snapshot: NpcDormantSnapshot,
    commands: &mut Commands,
    spatial: &mut NpcSpatialIndex,
) -> Entity;

// FixedUpdate(1Hz) 双向触发
pub fn hydrate_dormant_near_players(
    players: Query<&Position, With<Player>>,
    store: Res<NpcDormantStore>,
    // ...
);

pub fn dehydrate_far_npcs(
    npcs: Query<(Entity, &Position), With<NpcMarker>>,
    players: Query<&Position, With<Player>>,
    // ...
);

// FixedUpdate(自定义频率，默认 in-game 60s) 全局推演
pub fn dormant_global_tick(
    mut store: ResMut<NpcDormantStore>,
    zones: Res<ZoneRegistry>,
    mut ledger: ResMut<QiLedger>, // qi_physics::ledger
    // ...
);
```

**Hysteresis 阈值**：暂定 64 / 256，决策门 #2 可调。

**dormant tick 频率**：暂定 in-game 60s（real-time 约 3s @ 20×），决策门 #3 可调。

**持久化路径**：暂定 Redis HASH `bong:npc/dormant`，决策门 #5 可调（vs sled DB / SQLite）。

---

## §3 守恒律强约束（CLAUDE.md 风格规则）

> **本节是 dormant 推演必守的底盘约束**。任何对 dormant NPC 灵气 / qi 字段的写入必须走 `qi_physics::ledger::QiTransfer`。违反 = 阻塞 merge，docs/CLAUDE.md §四 应加一条「dormant 灵气未走 ledger」红旗。

### 强约束规则

1. **所有 dormant cultivation.qi_current 写入必须有对应 ledger 项**。直接 `snapshot.cultivation.qi_current += X` = 红旗
2. **所有 dormant 老死 / 渡劫失败 / 战斗死亡必须 release 灵气回 zone**（worldview §二「修炼消耗 = 别人少掉」+ §十「真元守恒」）。死亡时 `qi_physics::release::release_to_zone(snapshot.cultivation.qi_current, snapshot.zone)` → emit `QiTransfer { from: npc, to: zone, amount }`
3. **所有 dormant 修炼吸收必须走 zone EnvField 边界**：dormant NPC 距 zone 中心 > 64 格（zone EnvField 边界，对应 worldview §二「真元极易挥发」）→ 不可吸收（`excretion::container_intake` 自动 clamp 到 0）
4. **多 dormant NPC 同 zone 必须走 collision::repulsion**（worldview §P 异体排斥 ρ 矩阵）：3 个散修同时在 spirit_marsh 修炼，每人吸收量按 ρ 矩阵衰减
5. **不允许 dormant 内部"自循环"灵气**（如 dormant NPC A 给 B 传功）：必须先 hydrate 双方走正常战斗 / 社交链。dormant 期 SoA 状态全员独立
6. **审计字段 `qi_ledger_net`**：dormant 期累计 emit 的 ledger 净额（应等于 cultivation.qi_current 增量），P3 验收 e2e 检查全 dormant NPC `qi_ledger_net == cultivation.qi_current - dormant_initial_qi`，差值 > 1e-6 = 红旗

### docs/CLAUDE.md §四 红旗候选（决策门 #6 决定是否升级到项目级）

```
- **dormant NPC 灵气未走 ledger 账本**：dormant_global_tick 内 grep 出 `snapshot.cultivation.qi_current
  [+\-]=` 模式 → 必查 plan-npc-virtualize-v1 §3。所有 dormant 灵气流动必须 emit
  `qi_physics::ledger::QiTransfer { from, to, amount }` 并走 zone.spirit_qi 同步扣减
```

---

## §4 dormant 全局推演详情

每 in-game 60s 跑一次（FixedUpdate 自定义频率）。按 archetype 分桶处理：

### 4.1 移动推演

| Archetype | DormantBehaviorIntent | 推演逻辑 |
|---|---|---|
| Commoner | Wander / FleeFrom | 50-200 格随机漂移；FleeFrom 优先（30 格内修士触发，远离 5 min）|
| Rogue | Cultivate / Wander / Curiosity | 60% 静坐 cultivate / 30% wander / 10% 朝最近未探索 POI 漂移 |
| Disciple | PatrolToward / Cultivate | 70% 巡逻派系 hq_zone 周围 200 格 / 30% cultivate |
| Beast | PatrolToward (territory.center) | 在 Territory.radius 内随机走 |
| GuardianRelic | 不动 | 守遗迹，永远 PatrolToward(relic.position, radius=10) |

漂移距离：`per_tick_distance = base_speed * dt_in_game_seconds`（default 1 m/s × 60s = 60 格 / tick）。

### 4.2 灵气消耗（**走 qi_physics**）

```rust
for snapshot in store.snapshots.values_mut() {
    let zone = zones.find(&snapshot.position);
    if zone.is_none() { continue; } // 远 zone 中心 > 64 格不可吸收

    let intake = qi_physics::excretion::container_intake(
        &snapshot.cultivation,
        zone.unwrap(),
        Duration::from_secs(60), // in-game 60s
    );
    if intake > 0.0 {
        // emit ledger 转移
        ledger.emit(QiTransfer {
            from: QiSource::Zone(zone.unwrap().id),
            to: QiSource::Npc(snapshot.char_id),
            amount: intake,
        });
        snapshot.cultivation.qi_current += intake;
        snapshot.qi_ledger_net += intake;
        zone.spirit_qi -= intake; // ledger 同步扣减
    }
}
```

### 4.3 寿元衰减

```rust
snapshot.age_ticks += 60 * 20; // in-game 60s = 1200 ECS tick
let years_aged = (snapshot.age_ticks / TICKS_PER_YEAR) as f64 * lifespan.rate_multiplier;
if snapshot.lifespan.years_remaining < years_aged {
    // 触发 §4.5 dormant 老死路径
}
```

### 4.4 自动境界推进

满足突破条件 → `Cultivation::auto_breakthrough()`（plan-cultivation 已正典「NPC 无 UI 自动选默认」）。**注意**：突破成功后 qi_current 重置走 release（凡破境多余灵气返还 zone，worldview §三）。

### 4.5 dormant 老死

```rust
fn dormant_die(snapshot: &NpcDormantSnapshot, ledger: &mut QiLedger, zones: &mut ZoneRegistry) {
    let zone = zones.find(&snapshot.position).unwrap_or(&zones.default_zone);
    // release 灵气回 zone（守恒）
    qi_physics::release::release_to_zone(snapshot.cultivation.qi_current, zone, ledger);
    // emit npc death channel
    redis.publish("bong:npc/death", DeathSnapshot {
        char_id: snapshot.char_id,
        archetype: snapshot.archetype,
        cause: DeathCause::Aging,
        faction: snapshot.faction.as_ref().map(|f| f.faction),
        lifespan_used: snapshot.age_ticks,
        ..
    });
    // 走 plan-death §4b 善终路径
}
```

### 4.6 dormant 渡虚劫强制 hydrate

```rust
fn check_tribulation_ready(snapshot: &NpcDormantSnapshot) -> bool {
    snapshot.cultivation.realm == Realm::Tonglian
        && snapshot.cultivation.xp_to_next_realm() > THRESHOLD
        && snapshot.meridian_system.all_circulating()
        // dormant 期不能查"100 格无敌意"（无 spatial），简化为 ZoneRegistry zone-level 检查
}

if check_tribulation_ready(&snapshot) {
    let entity = hydrate_npc(snapshot.clone(), commands, spatial);
    // 立刻 emit tribulation event
    commands.entity(entity).insert(StartDuXuMarker);
    // 玩家收到 narration「远方有人起劫」
}
```

---

## §5 hydrate / dehydrate 触发

### 5.1 hydrate（dormant → ECS）

```rust
// FixedUpdate(1Hz)
fn hydrate_dormant_near_players(
    players: Query<&Position, With<Player>>,
    store: Res<NpcDormantStore>,
) {
    for snapshot in store.snapshots.values() {
        for player_pos in players.iter() {
            if (snapshot.position - player_pos).length() <= HYDRATE_RADIUS_64 {
                hydrate_npc(snapshot.clone(), &mut commands, &mut spatial);
                store.snapshots.remove(&snapshot.char_id);
                break;
            }
        }
    }
}
```

### 5.2 dehydrate（ECS → SoA）

```rust
// FixedUpdate(1Hz)
fn dehydrate_far_npcs(
    npcs: Query<(Entity, DehydrateQuery), With<NpcMarker>>,
    players: Query<&Position, With<Player>>,
) {
    for (entity, query) in npcs.iter() {
        let nearest = players.iter().map(|p| (p - query.position).length()).min();
        if nearest.unwrap_or(f64::MAX) > DEHYDRATE_RADIUS_256 {
            let snapshot = build_snapshot_from_components(query);
            store.snapshots.insert(snapshot.char_id, snapshot);
            commands.entity(entity).despawn();
        }
    }
}
```

### 5.3 Hysteresis 防抖

64 格触发 hydrate / 256 格触发 dehydrate（不对称）。玩家在 64-256 之间徘徊不会反复转换。

---

## §6 客户端可见变化

| 现象 | 描述 | P0 决策门 |
|---|---|---|
| **远视野无 NPC** | 玩家 256 格外完全看不到 NPC（即使有 dormant 在那里）| 默认接受；如果"远眺空旷"违和 → v2 加 Drowsy ECS entity 远视野可见 |
| **NPC pop-in** | 玩家走近 64 格 NPC "突然出现" | 决策门 #4：A 直接 spawn / B 渐入粒子 / C 雾遮罩 |
| **NPC pop-out** | 玩家走出 256 格 NPC "突然消失" | 同上 |
| **远方 narration** | dormant NPC 渡劫 / 老死时 agent 触发"远方传闻"narration（玩家可能正看不见但听得到）| ✅ 复用 plan-narrative-political-v1，本 plan 仅触发事件 |

---

## §7 测试矩阵（饱和化）

下限 **80 单测 + 2 e2e 压测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `NpcDormantStore` | CRUD / by_archetype / by_zone 索引同步 / 持久化 roundtrip（Redis HASH 序列化反序列化）| 12 |
| `dehydrate_npc + hydrate_npc roundtrip` | 所有持久化字段不丢 / Cultivation / MeridianSystem / FactionMembership / Lineage / Reputation / NpcLootTable / DormantBehaviorIntent 各自 roundtrip | 16 |
| `Hysteresis 防抖` | 64-256 格反复进出（同一玩家来回走 100 次）/ 多玩家叠加（A 在 64 内 / B 在 300 外，不 dehydrate）| 8 |
| `dormant_global_tick` | 各 archetype 移动正确性 / DormantBehaviorIntent 切换 / 漂移距离正确 | 10 |
| **守恒律 §3** | dormant 修炼吸收 emit ledger 总量 == zone 扣减总量（10 个 dormant × 100 tick）/ 老死 release 灵气回 zone（10 死亡）/ 渡劫强制 hydrate 后灵气经 ECS 战斗链消耗仍守恒 / `qi_ledger_net == cultivation.qi_current - initial` 全 NPC 检查 | **20**（守恒律是底线，必须饱和）|
| `dormant 老死路径` | 寿元到期 → emit `bong:npc/death` 含完整字段 / release 灵气回 zone 走 ledger / NpcDormantStore 移除 | 6 |
| `dormant 渡劫强制 hydrate` | TribulationReady 满足 → 立刻 hydrate（即使 > 256 格）/ broadcast tribulation event / 玩家可远程截胡 | 6 |
| **e2e 压测 #1**：100 hydrated + 1000 dormant + 1 player 5min | TPS ≥ 18 / dormant 推演不阻塞主 tick / qi_ledger_net 全员守恒 | 1（重） |
| **e2e 压测 #2**：5000 dormant + 0 hydrated + agent 推演 1 hour in-game（real-time 3min）| dormant 全员推演完成 / 5000 个 NpcDigest 出 agent 流 / zone.spirit_qi 全服收支等于 ledger emit 总量（守恒律 e2e）| 1（重） |

**P1 验收**：`grep -rcE '#\[test\]' server/src/npc/dormant/ server/src/npc/hydrate/` ≥ 50。

**P3 验收**：守恒律饱和测试 ≥ 20 全过 / `qi_ledger_net` 审计全 NPC == 0 差值。

---

## §8 开放问题 / 决策门

### #1 二态 vs 三态（Drowsy 是否进 v1）

- **A**：v1 二态（Hydrated + Dormant），Drowsy 留 v2
- **B**：v1 三态（含 Drowsy 中间态）
- **C**：v1 二态，但加 "soft hydrate"（hydrate 时不立刻全 component 重建，仅核心组件 + 帧间 amortize）

**默认推 A** —— MVP 先把守恒律 + 基本 roundtrip 跑通，撕裂感留实测后再决定是否补 Drowsy。Drowsy 已写入 reminder.md 待办

### #2 Hysteresis 阈值

- **A**：64 / 256（默认）
- **B**：64 / 128（更紧凑，dehydrate 更激进，节省 hydrated 数量）
- **C**：32 / 128（视野半径 32 格 = MC 默认渲染距离 / 4，更激进）

**默认推 A** —— 64 = 玩家近距能看到 NPC 行为细节；256 = 玩家远视野（远超 MC 默认渲染距离 16 chunks）反复进出概率低

### #3 dormant tick 频率

- **A**：in-game 60s（real-time 约 3s @ 20×时间倍率）
- **B**：in-game 30s（更平滑，但推演开销翻倍）
- **C**：in-game 5min（开销极低，但 dormant 漂移 / 老化跳变明显）

**默认推 A** —— 60s 是寿元粒度的 1/1440 day，足够"代际感"；3s real-time 推演 5000 NPC 单核单次约 5-15ms，可吸收

### #4 hydrate / dehydrate 视觉过渡

- **A**：直接 spawn / despawn（最简）
- **B**：渐入粒子 + 0.5s 渐显
- **C**：雾遮罩 + 256 格外 client 自带"远雾"覆盖

**默认推 A** —— MVP 接受 pop-in；如玩家反馈撕裂明显再升级（决策门 #1 联动）

### #5 dormant 持久化路径

- **A**：Redis HASH `bong:npc/dormant`（每 NPC 一个 hash entry）
- **B**：sled DB 嵌入式 KV（server 启动加载）
- **C**：SQLite

**默认推 A** —— Redis 已是 IPC backbone，复用；server 重启 dormant 自然恢复；agent 可 subscribe 变更

### #6 dormant NPC 间互动 v1 边界

- **A**：v1 完全不实装 dormant↔dormant，全权交 agent 推演
- **B**：v1 实装 dormant↔dormant 同 zone 修炼互相 collision::repulsion（已被 §3 强约束 #4 覆盖）
- **C**：v1 实装 dormant↔dormant faction 战争（批量推演敌对派系 dormant NPC 互殴 → 部分死亡 → release 灵气）

**默认推 A + B** —— A 边界清晰 + B 守恒律强约束 #4 已要求；C 留 v2

### #7 docs/CLAUDE.md §四 是否加「dormant 灵气未走 ledger」红旗

- **A**：加（强约束化）
- **B**：仅 plan §3 内强约束

**默认推 A** —— 跟 qi_physics / meridian_severed 一致格调；底盘约束应升级到项目级。同 plan-npc-perf-v1 决策门 #5

---

## §9 进度日志

- **2026-05-07** 骨架立项。源自 plan-npc-perf-v1 5 路探查后用户提议「隐式更新框架」+ qi_physics 模块（constants/env/excretion/release/ledger/...）已实装为底盘 API：
  - 所有 dormant 灵气流动接入 `qi_physics::ledger::QiTransfer`，对齐 worldview §二 守恒律
  - 二态 MVP（Hydrated + Dormant），Drowsy 中间态 v2 候补（写入 reminder.md）
  - 单服 NPC 总上限目标：512 → 5000+（worldview §三 + §十一 散修江湖物理基础）
  - dormant 渡虚劫强制 hydrate（叙事关键事件必须可见 + 截胡机制依赖 ECS）
  - dormant NPC 间互动 v1 极简（全权交天道 agent 推演）
  - 7 个开放决策门待 P0 收口

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：

- **落地清单**：`server/src/npc/dormant/*` 主模块 + `server/src/npc/hydrate/*` 转换层 + `dormant_global_tick_system` + `hydrate_dormant_near_players` + `dehydrate_far_npcs` + DormantBehaviorIntent 5 类 + qi_physics ledger 集成（守恒律审计）+ 持久化路径（Redis HASH）+ scripts/start.sh 100 hydrated + 1000 dormant 默认
- **关键 commit**：P0/P1/P2/P3/P4 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test npc::dormant npc::hydrate` 数量 / 守恒律饱和 ≥ 20 全过 / e2e #1 100 hydrated + 1000 dormant 5min ≥ 18 TPS / e2e #2 5000 dormant 1h in-game qi 守恒
- **跨仓库核验**：server `npc::dormant::*` + `npc::hydrate::*` / agent NpcDigest 通道 unchanged 但 dormant 数据流入 / client 无变化（hydrate spawn / dehydrate despawn 自然走 Valence）
- **遗留 / 后续**：
  - `plan-npc-virtualize-v2`（Drowsy 中间态）—— v1 实测玩家穿越 64-256 格边界撕裂 / hydrate 开销超阈值时启动
  - `plan-npc-virtualize-v3`（dormant↔dormant 战争 / faction 兴衰批量推演）—— 决策门 #6 选 C 时启动
  - docs/CLAUDE.md §四 红旗加「dormant 灵气未走 ledger」（决策门 #7 = A 时）
  - reminder.md 登记本骨架（已转为独立骨架 2026-05-07）
