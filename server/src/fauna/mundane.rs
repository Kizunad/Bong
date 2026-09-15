//! plan-mundane-fauna-v1 P0 — 凡兽底盘：9 种 MC 1.20.1 原版被动生物，走 Valence 原生
//! entity bundle（`npc/spawn/zombie.rs:52` 的 `ZombieEntityBundle { kind: EntityKind::ZOMBIE, .. }`
//! Rail A 范式，**不走** `beast.rs:64` 的 `MarkerEntityBundle` custom visual 路数）。
//! client 零改动，vanilla renderer 免费渲染原版模型/贴图/音效。
//!
//! **威胁谱系**（[[feedback_threat_spectrum]]）：凡兽最低档也能反抗，见
//! `npc/spawn/mundane.rs` 的 4-thinker 组合（`CorneredScorer` 排在 `FleeThreatScorer` 前）。
//!
//! **qi_physics 锚点**：凡兽无灵——不吸灵气、不放灵气、死亡无 qi 释放（本模块引入零个
//! qi 常数）。**豁免机制**：凡兽经 `npc_runtime_bundle_with_age` 拿到 live
//! `Cultivation`（qi_max>0，仅为满足 hunt 猎物契约的 realm 门）+ `MeridianSystem`，
//! 但 `cultivation::tick::qi_regen_and_zone_drain_tick` 用 `Without<MundaneFaunaSpecies>`
//! 把凡兽整体挡在真元吐纳外（spawn 与 hydrate 两路均挂此标记）——凡兽 `qi_current`
//! 恒 0，绝不抽 zone 灵气，死亡/超距回收裸 `insert(Despawned)` 无 qi 蒸发，守恒安全。

use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Despawned, Entity, EntityKind, EventWriter,
    IntoSystemConfigs, Position, Query, Res, Update, Without,
};

use crate::cultivation::dead_zone::is_dead_zone;
use crate::fauna::components::{fauna_spawn_seed, BeastKind};
use crate::movement::{movement_zone_kind, MovementZoneKind};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::spawn::ambient_scheduler::{
    ambient_scheduler_system, AmbientMarkerData, AmbientSchedulerConfig, AmbientSchedulerState,
    ThreatBudget,
};
use crate::npc::spawn::spawn_mundane_fauna_at;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::calamity::EVENT_REALM_COLLAPSE;
use crate::world::season::Season;
use crate::world::zone::{Zone, ZoneRegistry};

// ---------------------------------------------------------------------------
// MundaneFaunaKind — 9 变体终表（§8.1 #1 锁定，不增删）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MundaneFaunaKind {
    Cow,
    Pig,
    Sheep,
    Chicken,
    Rabbit,
    Goat,
    Frog,
    Fox,
    Wolf,
}

impl MundaneFaunaKind {
    /// 所有变体，用于遍历和穷尽性测试。
    pub const ALL: [MundaneFaunaKind; 9] = [
        Self::Cow,
        Self::Pig,
        Self::Sheep,
        Self::Chicken,
        Self::Rabbit,
        Self::Goat,
        Self::Frog,
        Self::Fox,
        Self::Wolf,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cow => "cow",
            Self::Pig => "pig",
            Self::Sheep => "sheep",
            Self::Chicken => "chicken",
            Self::Rabbit => "rabbit",
            Self::Goat => "goat",
            Self::Frog => "frog",
            Self::Fox => "fox",
            Self::Wolf => "wolf",
        }
    }

    /// 威胁谱系 health_max（§8.1 决议数值）：T0 惊扰反抗档（鸡/兔/蛙）最低 4-6，
    /// T1 被击反抗档（牛/猪/羊）10-14，T1.5 主动冲撞（山羊）与 T1 同档，
    /// T2 掠食骚扰（狐）16，T2.5 群体掠食（狼）最高 26——覆盖"鸡 < 狼"威胁谱系差异化，
    /// 不共享全局 DEFAULT_HEALTH_MAX（照 `beast.rs:111` 覆盖 wounds.health_current/max 范式）。
    pub const fn health_max(self) -> f32 {
        match self {
            Self::Chicken => 4.0,
            Self::Rabbit => 5.0,
            Self::Frog => 6.0,
            Self::Sheep => 10.0,
            Self::Pig => 12.0,
            Self::Goat => 12.0,
            Self::Cow => 14.0,
            Self::Fox => 16.0,
            Self::Wolf => 26.0,
        }
    }

    /// P2 威胁谱系 T2/T2.5 掠食者判定（狐/狼）——`preys_on` 与
    /// `npc/spawn/mundane.rs` 的 Hunt thinker 分支挂载共用同一口径。
    pub const fn is_predator(self) -> bool {
        matches!(self, Self::Fox | Self::Wolf)
    }

    /// T0 惊扰反抗档（鸡/兔/蛙）——[[feedback_threat_spectrum]] 的最低威胁档，只避不猎，
    /// 是狐（T2）唯一的猎物子集（狼猎 T0∪T1，狐只猎 T0）。
    pub const fn is_t0(self) -> bool {
        matches!(self, Self::Chicken | Self::Rabbit | Self::Frog)
    }
}

/// `MundaneFaunaKind` → Valence 原生 `EntityKind`（Rail A 头部，照
/// `fauna::visual::entity_kind_for_beast` 范式）。实际 spawn 时每个 kind 对应一个**不同的**
/// `<X>EntityBundle` 具体类型（`CowEntityBundle`/`PigEntityBundle`/...），Rust 类型系统不允许
/// 从单个函数返回异构 Bundle 类型，故真正的 bundle 构造 match 落在
/// `npc/spawn/mundane.rs::spawn_mundane_fauna_at`（同 `spawn/common.rs::spawn_rogue_commoner_base`
/// 按 `EntityKind` match 后各自 `insert` 具体 bundle 的先例）。本函数是该 match 的分支依据。
pub const fn entity_kind_for_mundane(kind: MundaneFaunaKind) -> EntityKind {
    match kind {
        MundaneFaunaKind::Cow => EntityKind::COW,
        MundaneFaunaKind::Pig => EntityKind::PIG,
        MundaneFaunaKind::Sheep => EntityKind::SHEEP,
        MundaneFaunaKind::Chicken => EntityKind::CHICKEN,
        MundaneFaunaKind::Rabbit => EntityKind::RABBIT,
        MundaneFaunaKind::Goat => EntityKind::GOAT,
        MundaneFaunaKind::Frog => EntityKind::FROG,
        MundaneFaunaKind::Fox => EntityKind::FOX,
        MundaneFaunaKind::Wolf => EntityKind::WOLF,
    }
}

/// 凡兽物种 tag component——记录一个已生成实体是哪个 `MundaneFaunaKind`。**不能**塞进
/// [`MundaneFaunaMarker`]（见该类型文档：`ambient_scheduler_system` 会在 pool_fn 产出实体后
/// 用 `M::new(now, zone.name.clone())` 覆盖式 insert marker，`AmbientMarkerData::new` 签名
/// 只有 `(spawned_at, home_zone)` 两个参数，装不下 `kind`），故独立成组件，`spawn_mundane_fauna_at`
/// 内直接 insert。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct MundaneFaunaSpecies(pub MundaneFaunaKind);

// ---------------------------------------------------------------------------
// MundaneFaunaMarker — AmbientMarkerData 实现（§8.1 #2 3 步接入第一步）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Component)]
pub struct MundaneFaunaMarker {
    pub spawned_at: u64,
    pub home_zone: String,
}

impl AmbientMarkerData for MundaneFaunaMarker {
    fn new(spawned_at: u64, home_zone: String) -> Self {
        Self {
            spawned_at,
            home_zone,
        }
    }

    fn home_zone(&self) -> &str {
        &self.home_zone
    }
}

// ---------------------------------------------------------------------------
// biome 分池（§8.1 #2 决议：AmbientPoolFn 签名只给 `&Zone`，没有 `Res<TerrainProvider>`
// 可用——「零改调度核」约束下不能给 AmbientPoolFn 类型加参数改造调用点。改用 `Zone::name`
// 判定，同 `Zone::botany_tags`（world/zone.rs:475）既有先例：zone 名字面量与地形 profile
// 一一对应，见 CLAUDE.md「Terrain profiles」——qingyun_peaks≈峰区/lingquan_marsh≈沼泽/
// north_wastes≈荒原，其余（含 spawn）≈平原，语义等价 TerrainProvider 的
// is_peaks_biome/is_marsh_biome/is_wastes_biome 谓词。
// ---------------------------------------------------------------------------

/// 平原/spawn 池：鸡/兔/猪/羊（默认兜底——覆盖 spawn、blood_valley/rift_valley、
/// youan_depths 等未特别标注的 zone）。
const PLAINS_POOL: &[MundaneFaunaKind] = &[
    MundaneFaunaKind::Cow,
    MundaneFaunaKind::Pig,
    MundaneFaunaKind::Sheep,
    MundaneFaunaKind::Chicken,
    MundaneFaunaKind::Rabbit,
];

/// 沼泽池：蛙/兔。
const MARSH_POOL: &[MundaneFaunaKind] = &[MundaneFaunaKind::Frog, MundaneFaunaKind::Rabbit];

/// 峰区池：山羊/羊。
const PEAKS_POOL: &[MundaneFaunaKind] = &[MundaneFaunaKind::Goat, MundaneFaunaKind::Sheep];

/// 荒原池：兔/狐/狼——plan §P0 原文列了兔/狐但漏收狼（9 变体终表里狼在任何 biome 池均未
/// 出现，无处可刷），本次实施把 T2.5 狼补进荒原池（贫瘠地带=狼群猎场，语义自洽，且
/// 补足"9 variant 全覆盖"测试要求——若后续人工判定狼该独立成另一 biome，归 P2 调整）。
const WASTES_POOL: &[MundaneFaunaKind] = &[
    MundaneFaunaKind::Rabbit,
    MundaneFaunaKind::Fox,
    MundaneFaunaKind::Wolf,
];

/// 按 zone 名判定 biome 分池（见模块顶部关于 `AmbientPoolFn` 签名约束的说明）。取
/// `zone_name: &str`（而非 `&Zone`）——本函数只读 zone 名字这一个字段，接口收窄成纯字符串
/// 判定后，`npc/hydrate/mod.rs` 复活快照（无 `Zone` 对象、只有持久化的 `zone_name`）也能
/// 调用同一份口径重新派生物种，不需要新增持久化字段（同 `spawn_beast_npc_at` 用
/// `fauna_tag_for_beast_spawn(home_zone, seed)` 从 home_zone+位置重新派生 `BeastKind`
/// 而不持久化它的先例）。
pub fn mundane_biome_pool(zone_name: &str) -> &'static [MundaneFaunaKind] {
    if zone_name.eq_ignore_ascii_case("lingquan_marsh") {
        MARSH_POOL
    } else if zone_name.eq_ignore_ascii_case("qingyun_peaks") {
        PEAKS_POOL
    } else if zone_name.eq_ignore_ascii_case("north_wastes") {
        WASTES_POOL
    } else {
        PLAINS_POOL
    }
}

/// 从 pool 按权重相等（均权 1）抽样，`seed % len` 确定性选取。空池返回 `None`
/// （理论上四张池均非空，双重防御同 `select_threat_species` 范式）。
pub fn select_mundane_species(pool: &[MundaneFaunaKind], seed: u64) -> Option<MundaneFaunaKind> {
    if pool.is_empty() {
        return None;
    }
    let idx = (seed % pool.len() as u64) as usize;
    Some(pool[idx])
}

/// 从 `(zone_name, position)` 确定性派生物种——biome 池 + [`fauna_spawn_seed`] 组合，
/// 供 [`mundane_pool_fn`]（新 spawn）与 `npc::hydrate`（dormant 复活重新派生，不持久化
/// `MundaneFaunaKind` 字段）共用同一口径。四张 biome 池恒非空，`unwrap_or` 兜底仅为
/// 防御（理论不可达）。
///
/// **不接季节权重**：hydrate 复活路径要求"同 (zone_name, position) 必复现同一物种"
/// （§8.1 #2 决议锚点），若混入随时间变化的 `Season` 会打破这条确定性契约。季节权重只在
/// [`mundane_species_for_position_seasonal`]（新 spawn 专属）生效，两者刻意分离。
pub fn mundane_species_for_position(zone_name: &str, position: DVec3) -> MundaneFaunaKind {
    let pool = mundane_biome_pool(zone_name);
    let seed = fauna_spawn_seed(zone_name, position.x, position.z);
    select_mundane_species(pool, seed).unwrap_or(MundaneFaunaKind::Cow)
}

// ---------------------------------------------------------------------------
// 季节权重（P2，plan §P2「季节权重」）——夏耐热物种权重升、冬耐寒物种权重升。
// ---------------------------------------------------------------------------

/// 季节权重倍率：夏季偏好蛙（喜湿热，marsh 池常驻种）；冬季偏好兔/山羊（plan 原文明确
/// 点名"冬耐寒（兔/山羊）升"）。汐转期（`Season::is_xizhuan`）中性 1.0——同
/// `Season::natural_supply_modifier` 等既有季节修饰函数在汐转期返回中性基线的先例，
/// 不引入 v1 未定义的过渡态偏权。
pub const fn mundane_season_weight_multiplier(kind: MundaneFaunaKind, season: Season) -> u32 {
    match season {
        Season::Summer => match kind {
            MundaneFaunaKind::Frog => 3,
            _ => 1,
        },
        Season::Winter => match kind {
            MundaneFaunaKind::Rabbit | MundaneFaunaKind::Goat => 3,
            _ => 1,
        },
        Season::SummerToWinter | Season::WinterToSummer => 1,
    }
}

/// 按季节权重从 pool 中抽样（`seed % total_weight` 确定性选取，同
/// [`select_threat_species`](crate::npc::spawn::ambient_scheduler::select_threat_species)
/// 的权重抽样手法）。空池返回 `None`；`total_weight` 理论恒 > 0（每条目权重 >= 1）。
pub fn select_mundane_species_weighted(
    pool: &[MundaneFaunaKind],
    season: Season,
    seed: u64,
) -> Option<MundaneFaunaKind> {
    if pool.is_empty() {
        return None;
    }
    let weights: Vec<u32> = pool
        .iter()
        .map(|kind| mundane_season_weight_multiplier(*kind, season))
        .collect();
    let total_weight: u32 = weights.iter().sum();
    if total_weight == 0 {
        // 防御分支（理论不可达：每条目权重恒 >= 1）——退回等权兜底而非 panic。
        return select_mundane_species(pool, seed);
    }
    let roll = (seed % total_weight as u64) as u32;
    let mut cumulative = 0u32;
    for (kind, weight) in pool.iter().zip(weights.iter()) {
        cumulative += weight;
        if roll < cumulative {
            return Some(*kind);
        }
    }
    pool.last().copied()
}

/// 新 spawn 专属：biome 池 + 季节权重抽样。**不供 hydrate 复活使用**（见
/// [`mundane_species_for_position`] 文档——复活要求确定性不随季节漂移）。
pub fn mundane_species_for_position_seasonal(
    zone_name: &str,
    position: DVec3,
    season: Season,
) -> MundaneFaunaKind {
    let pool = mundane_biome_pool(zone_name);
    let seed = fauna_spawn_seed(zone_name, position.x, position.z);
    select_mundane_species_weighted(pool, season, seed).unwrap_or(MundaneFaunaKind::Cow)
}

// ---------------------------------------------------------------------------
// 栖息门槛（P2，plan §P2「栖息门槛」）——死域不刷凡兽 + 塌缩 zone 跳过。
// ---------------------------------------------------------------------------

/// 死域/塌缩 zone 栖息门槛：**复用既有 `is_dead_zone`**（`cultivation::dead_zone`，
/// `spirit_qi ∈ [0, 0.01)`，与 `world::mob_spawn::MobSpawnFilter::ban_in_dead_zone`
/// 同一口径）——**不新造第二/三套死域阈值**（本次核验发现的收口点：plan 原文草案的
/// `spirit_qi <= 0` 会漏掉 `[0, 0.01)` 这个正典意义上的死域窄带，与自引用的 §一:22
/// 正典冲突，故改用既有判定函数）。`REALM_COLLAPSE` zone 单独跳过（塌缩期地表本身在
/// 剧烈变动，凡兽不应在此时段刷新，同 `movement_zone_kind` 的 `Dead` 分支判据之一）。
///
/// 负灵域（`spirit_qi < -0.2`）**不**在此处过滤——见 [`mundane_fauna_negative_zone_wither_system`]
/// 文档：负灵域内凡兽走「入即枯萎」而非「禁止生成」，两者是正典描述的不同现象
/// （§一:22 死域 vs §七:759 负灵域），刻意不合并成一套判据。
pub fn mundane_habitat_allows_spawn(zone: &Zone) -> bool {
    if zone
        .active_events
        .iter()
        .any(|event| event == EVENT_REALM_COLLAPSE)
    {
        return false;
    }
    !is_dead_zone(zone)
}

// ---------------------------------------------------------------------------
// mundane_passive_budget_fn / mundane_pool_fn（§8.1 #2/#4 决议数值）
// ---------------------------------------------------------------------------

/// 凡兽 passive 预算：v1 保守拍小，不看 `danger`（凡兽不分 danger 分级，恒同一档）。
/// `max_alive=3`（§8.1 #4），`pack_size_range=(1,1)`（调度核每次巡检产 1 个，多产未消费），
/// `spawn_interval_ticks=400`（复用 `threat_budget(3)` 同档 stride，§8.1 #4）。
pub fn mundane_passive_budget_fn(_danger: u8) -> ThreatBudget {
    ThreatBudget {
        max_alive: 3,
        spawn_interval_ticks: 400,
        pack_size_range: (1, 1),
    }
}

/// 真实 `AmbientPoolFn`：按 biome 分池 + 季节权重选物种 → `spawn_mundane_fauna_at`。
/// **P2 落地**：死域/塌缩 zone 栖息门槛（[`mundane_habitat_allows_spawn`]）先行拦截——
/// 命中即返回 `None`（调用方 `ambient_scheduler_system` 把 `None` 当"本次不刷"处理，
/// 不 panic，同 `ambient_threat_pool_fn` 死域过滤后池为空的既有分支）。
pub fn mundane_pool_fn(
    commands: &mut Commands,
    layer: Entity,
    zone: &Zone,
    spawn_position: DVec3,
    patrol_target: DVec3,
    season: Season,
) -> Option<Entity> {
    if !mundane_habitat_allows_spawn(zone) {
        return None;
    }
    let kind = mundane_species_for_position_seasonal(&zone.name, spawn_position, season);
    Some(spawn_mundane_fauna_at(
        commands,
        layer,
        &zone.name,
        spawn_position,
        patrol_target,
        kind,
    ))
}

// ---------------------------------------------------------------------------
// register — 3 步纯复用 ambient_scheduler（§8.1 #2 决议，零改调度核）
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// preys_on — 跨层捕食关系表（P2，plan §P2「跨层食物网落地」）
//
// **净新增，不扩展 `is_prey_of`**（`fauna/components.rs:121`，签名锁定
// `(BeastKind, BeastKind) -> bool` 且内部用 `realm_tier()` 比较，凡兽无境界阶维度，
// 无法直接扩展给跨层用，§8.1 #2 定案）。
// ---------------------------------------------------------------------------

/// 捕食者身份——横跨凡兽（`MundaneFaunaKind`，狼/狐）与妖兽（`BeastKind`，全档陆生非鼠）
/// 两个类目，`preys_on` 借这层包装统一处理两套键空间。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaunaPredatorId {
    Mundane(MundaneFaunaKind),
    Beast(BeastKind),
}

/// 猎物身份——凡兽任意物种，或噬元鼠（`BeastKind::Rat`，鼠患的天敌来自凡兽层这条边
/// 单独建模，见模块级设计原则表「狼(T2.5)/狐(T2) 猎鼠」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaunaPreyId {
    Mundane(MundaneFaunaKind),
    Rat,
}

/// 跨层捕食关系总账。边清单（对应 plan 设计原则表 ASCII 图）：
/// - 狼(T2.5) ──猎──→ T0∪T1 凡兽（不含狐/狼自身）+ 噬元鼠
/// - 狐(T2)   ──猎──→ T0 凡兽（鸡/兔/蛙）+ 噬元鼠
/// - 妖兽（陆生、非鼠）──猎──→ 凡兽全档（"凡兽=妖兽的口粮"，plan 原文明确"全档"非按
///   tier 子集）——**此边当前无需运行时门控**：`territory.rs` 的既有
///   `consider_hunt_candidate` 对非 `NpcArchetype::Beast` 目标（凡兽走 `Mundane` 档）
///   本就无差别放行（只对 Beast-vs-Beast 目标才查 `is_prey_of`），凡兽自 P0 起就带
///   `Cultivation::default()`（`npc_runtime_bundle_with_age` 一并插入，满足
///   `HuntCandidateQuery` 契约）——这条边已是**既有生产代码的自然结果**，此处仅做关系
///   总账的文档化 + 测试断言，非新增运行时门控点（运行时回归护栏见
///   `npc::territory::tests::beast_hunt_action_targets_mundane_fauna_via_existing_territory_pipeline`，
///   纯函数真值表见本模块 `preys_on` 及 `npc::brain::predation::tests`）。
/// - T0（鸡/兔/蛙）──避──→ 一切（永不作为 predator 出现，走 `_ => false` 兜底）。
/// - **反向关闭**（§8.1 #8 定案）：`Rat` 不腐食/不捕食凡兽尸体或活体——鼠患的"吃"
///   （噬元，吃灵气）与凡兽的"死"（无灵）不构成边。
pub fn preys_on(predator: FaunaPredatorId, prey: FaunaPreyId) -> bool {
    match (predator, prey) {
        (FaunaPredatorId::Mundane(MundaneFaunaKind::Wolf), FaunaPreyId::Rat) => true,
        (FaunaPredatorId::Mundane(MundaneFaunaKind::Wolf), FaunaPreyId::Mundane(prey_kind)) => {
            !prey_kind.is_predator()
        }
        (FaunaPredatorId::Mundane(MundaneFaunaKind::Fox), FaunaPreyId::Rat) => true,
        (FaunaPredatorId::Mundane(MundaneFaunaKind::Fox), FaunaPreyId::Mundane(prey_kind)) => {
            prey_kind.is_t0()
        }
        // T0（鸡/兔/蛙）从不作为 predator：既不匹配上面两支，也不匹配下面的 Beast 分支，
        // 落到 `_ => false`。
        (FaunaPredatorId::Beast(predator_kind), FaunaPreyId::Mundane(_)) => {
            predator_kind.is_terrestrial() && predator_kind != BeastKind::Rat
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// 负灵域灭杀（P2，plan §P2「负灵域灭杀」）——§七:759 正典明文。
// ---------------------------------------------------------------------------

/// 枯萎倒计时：`spirit_qi < -0.2`（对齐 `movement::movement_zone_kind` 的 `Negative`
/// 分支阈值，不新造第二套负灵域判据）后开始计时，`NEGATIVE_ZONE_WITHER_TICKS` 到期即
/// despawn。
pub const NEGATIVE_ZONE_WITHER_TICKS: u32 = 60; // 3s @ 20 TPS
/// 枯萎期间粒子 continuous 发射间隔（tick）。
pub const NEGATIVE_ZONE_WITHER_PARTICLE_INTERVAL_TICKS: u32 = 4;
/// 枯萎起始 burst 粒子数。
pub const NEGATIVE_ZONE_WITHER_BURST_COUNT: u16 = 8;
/// 枯萎期间 continuous 粒子数（每次发射）。
pub const NEGATIVE_ZONE_WITHER_CONTINUOUS_COUNT: u16 = 1;
/// 粒子生命周期（tick）。
pub const NEGATIVE_ZONE_WITHER_PARTICLE_DURATION_TICKS: u16 = 12;
/// 灰烬色残灰（复用既有 `bong:fauna_spawn_dust` 通用 sprite burst 原语，色由 payload 驱动，
/// client 零改动——见 `client/.../FaunaSpawnDustPlayer.java` 的 `payload.colorRgb()`）。
pub const NEGATIVE_ZONE_WITHER_COLOR_HEX: &str = "#6E6A5E";
/// 消亡瞬间音效 recipe id（`assets/audio/recipes/fauna_mundane_wither.json` 新建注册，
/// `entity.wither.hurt` pitch 1.6 vol 0.4，client 零改动——data-driven 通用音频管线）。
pub const NEGATIVE_ZONE_WITHER_SOUND_RECIPE_ID: &str = "fauna_mundane_wither";

/// 枯萎中间态：挂在进入负灵域的凡兽实体上，逐 tick 计时；离开负灵域（qi 回升）则移除
/// （恢复语义合理——负灵域本身可能随 zone qi 波动短暂进出）。
#[derive(Debug, Clone, Copy, Default, Component)]
pub struct MundaneFaunaWithering {
    pub elapsed_ticks: u32,
}

/// 负灵域灭杀 tick 系统：查所有凡兽实体，按 `Position` 定位其所在 zone，用既有
/// `movement_zone_kind` 判定是否处于负灵域——命中则计时枯萎，到期 despawn +
/// 消亡音效；未命中且此前在枯萎中则移除计时器（恢复）。
///
/// 位置→zone 查找沿用 `ambient_scheduler.rs` 既有 NPC 场景先例（`find_zone(Overworld, pos)`，
/// 不依赖 `CurrentDimension`——凡兽实体不挂该组件，全部 Overworld 地形 profile 亦无需要）。
type MundaneFaunaWitherQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static mut MundaneFaunaWithering>,
    ),
    (
        bevy_ecs::query::With<MundaneFaunaSpecies>,
        Without<Despawned>,
    ),
>;

#[allow(clippy::too_many_arguments)]
pub fn mundane_fauna_negative_zone_wither_system(
    mut commands: Commands,
    mut fauna: MundaneFaunaWitherQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    let Some(zones) = zones else { return };
    for (entity, position, withering) in &mut fauna {
        let pos = position.get();
        let zone = zones.find_zone(crate::world::dimension::DimensionKind::Overworld, pos);
        let zone_kind = movement_zone_kind(zone, false);

        if zone_kind != MovementZoneKind::Negative {
            if withering.is_some() {
                commands.entity(entity).remove::<MundaneFaunaWithering>();
            }
            continue;
        }

        match withering {
            None => {
                commands
                    .entity(entity)
                    .insert(MundaneFaunaWithering::default());
                vfx_events.send(wither_particle_event(pos, NEGATIVE_ZONE_WITHER_BURST_COUNT));
            }
            Some(mut state) => {
                state.elapsed_ticks = state.elapsed_ticks.saturating_add(1);
                let elapsed = state.elapsed_ticks;
                if elapsed >= NEGATIVE_ZONE_WITHER_TICKS {
                    vfx_events.send(wither_particle_event(pos, NEGATIVE_ZONE_WITHER_BURST_COUNT));
                    audio_events.send(PlaySoundRecipeRequest {
                        recipe_id: NEGATIVE_ZONE_WITHER_SOUND_RECIPE_ID.to_string(),
                        instance_id: 0,
                        pos: Some([
                            pos.x.floor() as i32,
                            pos.y.floor() as i32,
                            pos.z.floor() as i32,
                        ]),
                        flag: None,
                        volume_mul: 1.0,
                        pitch_shift: 0.0,
                        recipient: AudioRecipient::Radius {
                            origin: pos,
                            radius: 32.0,
                        },
                    });
                    commands.entity(entity).insert(Despawned);
                } else if elapsed % NEGATIVE_ZONE_WITHER_PARTICLE_INTERVAL_TICKS == 0 {
                    vfx_events.send(wither_particle_event(
                        pos,
                        NEGATIVE_ZONE_WITHER_CONTINUOUS_COUNT,
                    ));
                }
            }
        }
    }
}

fn wither_particle_event(pos: DVec3, count: u16) -> VfxEventRequest {
    VfxEventRequest::new(
        pos,
        VfxEventPayloadV1::SpawnParticle {
            event_id: "bong:fauna_spawn_dust".to_string(),
            origin: [pos.x, pos.y + 0.5, pos.z],
            direction: Some([0.0, -0.02, 0.0]),
            color: Some(NEGATIVE_ZONE_WITHER_COLOR_HEX.to_string()),
            strength: Some(0.6),
            count: Some(count),
            duration_ticks: Some(NEGATIVE_ZONE_WITHER_PARTICLE_DURATION_TICKS),
        },
    )
}

pub fn register(app: &mut App) {
    app.insert_resource(AmbientSchedulerState::<MundaneFaunaMarker>::default())
        .insert_resource(AmbientSchedulerConfig::<MundaneFaunaMarker>::new(
            mundane_passive_budget_fn,
            mundane_pool_fn,
            // counts_against_threat_budget=false（§8.1 #3）：凡兽全档独立 passive 预算，
            // 不进 plan-ambient-threat-v1 的 zone 威胁密度统计。
            false,
        ))
        .add_systems(
            Update,
            ambient_scheduler_system::<MundaneFaunaMarker>
                .in_set(crate::npc::spawn::ambient_scheduler::AmbientTerminalSystemSet::Recycle),
        )
        // P2 — 负灵域灭杀（§七:759）。防御性 add_event（可能已由其它模块全局注册，
        // Bevy 允许重复调用，同 `fauna::experience`/`fauna::migration` 既有先例）。
        .add_event::<VfxEventRequest>()
        .add_event::<PlaySoundRecipeRequest>()
        .add_systems(Update, mundane_fauna_negative_zone_wither_system);
}

#[cfg(test)]
#[path = "mundane_tests.rs"]
mod tests;
