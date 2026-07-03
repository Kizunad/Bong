//! plan-ambient-threat-v1 P0/P1 — 通用 ambient 调度核 + 物种池分层。
//!
//! fork 自 `heiwushi_natural_spawn_system`（`npc/heiwushi_spawn.rs:42-115`）的结构：
//! state resource + `last_check_tick` 节流 + marker 活体计数 query + 玩家在场半径门 +
//! `DimensionKind::Overworld` 维度过滤全套骨架直接复用，套
//! `PoissonSpawnSampler::adaptive_for_zone`（`npc/spawn/mod.rs:73`）的自适应间距参数
//! 做距离环采样，节流驱动改用 `should_run_interval`（`npc/dormant/mod.rs:712`）。
//!
//! **泛型可注入**：调度核按 `(pool_fn, budget_fn, marker_type, counts_against_threat_budget)`
//! 四个维度参数化——`marker_type` 通过 Rust 泛型（`M: AmbientMarkerData`）区分，
//! `budget_fn`/`pool_fn`/`counts_against_threat_budget` 通过 `AmbientSchedulerConfig<M>`
//! 资源里的函数指针注入。`plan-mundane-fauna-v1` 只需给自己的 marker 类型实现
//! [`AmbientMarkerData`]、注册一份独立的 `AmbientSchedulerConfig<其 marker>` +
//! `ambient_scheduler_system::<其 marker>`，即可零改复用本模块的距离环采样 / 密度上限 /
//! 超距 `Despawned` 回收基建，且把 `counts_against_threat_budget` 设为 `false` 时完全不
//! 计入本 plan 的威胁预算计数（P0/P1 各自独立的 `AmbientSchedulerState<M>` / 活体 query
//! 因泛型单态化天然互不干扰）。
//!
//! **P1 收口**：[`threat_pool`] 提供 danger 1~7 分层物种表（§8.1 #2），
//! [`ambient_threat_pool_fn`] 是 `AmbientThreatMarker` 的真实 `pool_fn` 实现——
//! 唯一相对 P0 设想有出入的地方：`AmbientPoolFn` 签名从 `(u8, &str)` 改成了 `&Zone`
//! （死域白名单过滤 `MobSpawnFilter::ban_in_dead_zone` 需要 `zone.spirit_qi`，仅
//! `danger_level` 不够），调用点 `ambient_scheduler_system` 本就持有 `&Zone`，属零成本改动，
//! 不影响"泛型调度核不关心具体物种"的设计初衷。TSY 自然涌现**不**接入本调度核/本表——
//! 直调 `spawn_tsy_hostiles_for_family`（`npc/tsy_hostile.rs:561`），见 §8.1 #3。
//!
//! **P3 收口**（§8.1 #4/#5，生态联动）：[`danger_tide_weight`] 是兽潮双因子门槛改造的
//! danger 加权因子——`world/heartbeat.rs` 的 `maybe_queue_beast_tide`（主入口）与
//! `chain_reaction_tick` 内 `PseudoVeinDissipated` 分支（次入口）各自读取本函数返回值去
//! 缩放"有效阈值/时长/强度"，两条入口内 `BEAST_TIDE_LOW_QI_THRESHOLD`/
//! `BEAST_TIDE_LOW_QI_REQUIRED_TICKS` 常数本身原位不动。[`dead_zone_threat_budget`] 直接
//! 复用 `movement::movement_zone_kind` 的既有死域/负灵域判定口径（不新造第二套定义）放大
//! [`ThreatBudget`]，已接入 [`decide_ambient_check`] 新增的 `zone_kind` 参数。

use std::marker::PhantomData;

use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Despawned, Entity, Position, Query, Res, ResMut,
    Resource, Update, With, Without,
};

use crate::movement::{movement_zone_kind, MovementZoneKind};
use crate::npc::dormant::{planar_distance, should_run_interval};
use crate::npc::movement::GameTick;
use crate::npc::spawn::PoissonSpawnSampler;
use crate::npc::spawn_rat::spawn_rat_npc_at;
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::era::WorldEraState;
use crate::world::mob_spawn::{
    era_beast_spawn_gate, spawn_natural_mob_at, MobSpawnFilter, NaturalMobKind,
};
use crate::world::zone::{Zone, ZoneRegistry};

/// 调度核每次巡检的粗节流步长（对齐 heiwushi 的 `last_check_tick` 早退模式）。
/// 必须整除下方 [`threat_budget`] 表里所有 `spawn_interval_ticks`，否则粗节流会把
/// 某些恰好落在间隔倍数上的 tick 漏检——见模块内 `spawn_interval 全为 50 的倍数` 注释。
pub const AMBIENT_SCHEDULER_STRIDE_TICKS: u64 = 50;

/// 距离环内环半径（格）——低于此距离视为"贴脸刷"，禁止。
pub const AMBIENT_RING_MIN_RADIUS: f64 = 24.0;
/// 距离环外环半径（格）——超出此距离视为"玩家看不到"，不在此处刷新。
pub const AMBIENT_RING_MAX_RADIUS: f64 = 64.0;
/// 存活 ambient 实体距所有 Overworld 玩家超过此距离即回收（`insert(Despawned)`）。
pub const AMBIENT_DESPAWN_RADIUS: f64 = 96.0;

// ---------------------------------------------------------------------------
// ThreatBudget — 按 zone danger_level 查表
// ---------------------------------------------------------------------------

/// 单个 danger 档位的威胁预算：zone 内活体上限 / 巡检间隔 / 单次刷新群体大小范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreatBudget {
    pub max_alive: u32,
    pub spawn_interval_ticks: u32,
    pub pack_size_range: (u32, u32),
}

/// danger 1~7 预算表（§8.1 #1 收口数值，danger1→max_alive 1-2 / danger4→3-5 /
/// danger7→8-10；`spawn_interval_ticks` danger1→~600 / danger7→~150，用
/// `should_run_interval` 驱动）。
///
/// `danger_level == 0`（`Zone::spawn()` 硬编码兜底 fallback 值——zones.json 缺失/
/// 校验失败时才会出现，正常 zones.json 全部 1~7）与 `danger_level > 7`（未来枚举扩展/
/// 脏数据）均钳到最近的合法边界档，不 panic、不返回"空预算"。
pub fn threat_budget(danger: u8) -> ThreatBudget {
    let clamped = danger.clamp(1, 7);
    match clamped {
        1 => ThreatBudget {
            max_alive: 2,
            spawn_interval_ticks: 600,
            pack_size_range: (1, 2),
        },
        2 => ThreatBudget {
            max_alive: 2,
            spawn_interval_ticks: 500,
            pack_size_range: (1, 2),
        },
        3 => ThreatBudget {
            max_alive: 3,
            spawn_interval_ticks: 400,
            pack_size_range: (2, 3),
        },
        4 => ThreatBudget {
            max_alive: 4,
            spawn_interval_ticks: 300,
            pack_size_range: (3, 5),
        },
        5 => ThreatBudget {
            max_alive: 6,
            spawn_interval_ticks: 250,
            pack_size_range: (4, 6),
        },
        6 => ThreatBudget {
            max_alive: 8,
            spawn_interval_ticks: 200,
            pack_size_range: (6, 8),
        },
        _ => ThreatBudget {
            max_alive: 10,
            spawn_interval_ticks: 150,
            pack_size_range: (8, 10),
        },
    }
}

// ---------------------------------------------------------------------------
// P3 生态联动 — 兽潮门槛双因子 danger 加权 + 死域/负灵域威胁乘区（§8.1 #4/#5）
// ---------------------------------------------------------------------------

/// 兽潮双因子门槛改造的 danger 加权因子（§8.1 #5）。**不修改** `world/heartbeat.rs` 内
/// `BEAST_TIDE_LOW_QI_THRESHOLD`（值 0.15）/`BEAST_TIDE_LOW_QI_REQUIRED_TICKS`
/// 常数本身——两条既有兽潮入口（主入口 `maybe_queue_beast_tide` 按
/// `low_qi_ticks_by_zone` 累计满时长独立触发；次入口 `PseudoVeinDissipated` 邻域扩散
/// 分支）各自在调用点读取本函数返回值去缩放"有效阈值/时长/强度"，常数定义原位不动。
///
/// danger 越高，危险度地理对应的生态失衡越剧烈——兽潮应当越容易触发、触发时越猛烈：
/// danger1 → `1.0`（无收紧，行为与本 plan P3 落地前完全一致，覆盖全部沿用
/// `danger_level: 0`/`1` 兜底默认值的既有测试 fixture）；danger7 → `1.6`（+60%）。
/// 线性插值，`danger_level` 钳到 `[1, 7]`（`0` 兜底值与 `>7` 脏数据/未来扩展均钳到
/// 最近合法边界，不 panic，语义对齐 [`threat_budget`] 的钳位处理）。
pub fn danger_tide_weight(danger_level: u8) -> f64 {
    let clamped = danger_level.clamp(1, 7);
    1.0 + (f64::from(clamped) - 1.0) * 0.1
}

/// 主入口 `maybe_queue_beast_tide` 用：把 `BEAST_TIDE_LOW_QI_REQUIRED_TICKS` 按 danger
/// 权重缩短为"有效所需 tick 数"的缩放系数——[`danger_tide_weight`] 的倒数，权重越大
/// （危险度越高）达到触发所需的持续低灵气时长越短。danger1 → `1.0`（不变，5 分钟阈值
/// 原样保留）；danger7 → `1.0 / 1.6 ≈ 0.625`（缩至约 62.5%，5 分钟阈值降到约 3 分钟多）。
/// 调用点负责 `(BEAST_TIDE_LOW_QI_REQUIRED_TICKS as f64 * scale).round() as u64`，本函数
/// 只返回纯缩放系数，不接触常数本身。
pub fn danger_tide_required_ticks_scale(danger_level: u8) -> f64 {
    1.0 / danger_tide_weight(danger_level)
}

/// 死域/负灵域威胁预算乘区（§8.1 #4）：直接复用 `movement::movement_zone_kind` 已编码的
/// 死域判定口径（`zone.danger_level >= 5 && zone.spirit_qi <= 0.1` 或 `REALM_COLLAPSE`
/// 活跃事件），**不新造第二套"死域"定义**。正典依据 worldview §一:22"死域连野兽都活不了"+
/// §七:759"负灵域野兽材质枯萎化飞灰"——死域/负灵域里没有寻常生灵却有更浓的凶险（游荡的
/// 死物/邪祟），故这里放大的是**威胁 spawn 预算**而非"允许更多活体野兽"。
///
/// `MovementZoneKind::Dead` → `max_alive`/`pack_size_range` 上限 ×1.5、`spawn_interval_ticks`
/// 缩至约 2/3（更频繁刷新）；`MovementZoneKind::Negative` → ×1.2/缩至约 5/6；
/// `Normal`/`ResidueAsh` → 原样返回，不放大（`ResidueAsh` 是灰烬地表微观判定，非本 plan
/// 危险度语义覆盖范围，保守不动）。所有取整用 `.round()`，`max_alive`/`pack_size_range`
/// 上界用 `.max(原值)` 保证乘区只增不减，`spawn_interval_ticks` 用 `.min(原值)` 保证只
/// 缩短不延长，且钳 `>= 1` 防止除零/零间隔。
pub fn dead_zone_threat_budget(budget: ThreatBudget, zone_kind: MovementZoneKind) -> ThreatBudget {
    let multiplier: f64 = match zone_kind {
        MovementZoneKind::Dead => 1.5,
        MovementZoneKind::Negative => 1.2,
        MovementZoneKind::Normal | MovementZoneKind::ResidueAsh => 1.0,
    };
    if (multiplier - 1.0).abs() < f64::EPSILON {
        return budget;
    }
    let scaled_max_alive =
        ((f64::from(budget.max_alive) * multiplier).round() as u32).max(budget.max_alive);
    let scaled_interval = ((f64::from(budget.spawn_interval_ticks) / multiplier).round() as u32)
        .max(1)
        .min(budget.spawn_interval_ticks);
    let scaled_pack_max = ((f64::from(budget.pack_size_range.1) * multiplier).round() as u32)
        .max(budget.pack_size_range.1);
    ThreatBudget {
        max_alive: scaled_max_alive,
        spawn_interval_ticks: scaled_interval,
        pack_size_range: (budget.pack_size_range.0, scaled_pack_max),
    }
}

// ---------------------------------------------------------------------------
// ThreatPool — P1 物种池分层（§8.1 #2 收口）
// ---------------------------------------------------------------------------

/// P1 物种池的两大类：`Rat`（danger1-2 中立袭扰档，走 [`spawn_rat_npc_at`]，`NaturalMobKind`
/// 无 Rat 变体故独立枚举）与 `Mob`（danger3-7 通用 beast/AshSpider，走
/// [`spawn_natural_mob_at`]）。**不新增任何变体、不加任何 buff 组件**（§8.1 #2/#3 拒绝
/// "danger7 精英" buff 路线）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreatSpecies {
    Rat,
    Mob(NaturalMobKind),
}

/// 物种池单条目：物种 + 权重（同档内均权重 1，无"稀有度"分层——§8.1 #2 已订正
/// "逐种可辨"假设不成立，权重差异化留给未来若真的接了 `kind` 透传参数再回填）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreatPoolEntry {
    pub species: ThreatSpecies,
    pub weight: u32,
}

/// danger 1~7 物种池终表（§8.1 #2 收口，已按 `spawn_natural_mob_at` 实际调度路径订正）：
/// - danger 1~2 → `Rat` 小群（2~4 只，见 P2 袭扰行为）
/// - danger 3~4 → 通用 beast 五变体（`Zombie/Skeleton/Creeper/Rogue/Daoxiang`，`mob_spawn.rs:77-91`
///   统一走 `spawn_beast_npc_at`，落地实体外观由 `fauna_tag_for_beast_spawn` 按 zone 名 / seed
///   派生，**不可逐种可辨**，靠 pack_size/interval/danger 梯度而非视觉表达强度差异）
/// - danger 5~7 → 同上五变体 + `AshSpider`（死域白名单物种，danger7 只调
///   [`ThreatBudget`] 的 pack/interval，不额外加物种/buff）
///
/// `weight_hook`：§8.1 #6 昼夜/天气权重占位钩子，默认 `None` 效果等价 1.0 倍率，当前
/// **不接入任何实际昼夜/天气读取逻辑**——本 plan 明确拒绝首创昼夜系统，留空钩子只为不阻塞
/// 未来独立 plan 回填。
///
/// `dimension != Overworld`（即 TSY）恒返回空池——TSY 自然涌现走独立直调
/// `spawn_tsy_hostiles_for_family`（`npc/tsy_hostile.rs:561`）路径（§8.1 #3），不复用本表、
/// 不进 `AmbientPoolFn` 泛型通用调度核。
pub fn threat_pool(
    danger: u8,
    dimension: DimensionKind,
    weight_hook: Option<f32>,
) -> Vec<ThreatPoolEntry> {
    let _ = weight_hook; // 占位钩子，见函数文档 §8.1 #6，当前恒无操作
    if dimension != DimensionKind::Overworld {
        return Vec::new();
    }
    match danger.clamp(1, 7) {
        1 | 2 => vec![ThreatPoolEntry {
            species: ThreatSpecies::Rat,
            weight: 1,
        }],
        3 | 4 => generic_beast_entries(),
        _ => {
            // 5, 6, 7 — 同一份物种池，danger7 只在 ThreatBudget 层面加密 pack/interval。
            let mut entries = generic_beast_entries();
            entries.push(ThreatPoolEntry {
                species: ThreatSpecies::Mob(NaturalMobKind::AshSpider),
                weight: 1,
            });
            entries
        }
    }
}

fn generic_beast_entries() -> Vec<ThreatPoolEntry> {
    [
        NaturalMobKind::Zombie,
        NaturalMobKind::Skeleton,
        NaturalMobKind::Creeper,
        NaturalMobKind::Rogue,
        NaturalMobKind::Daoxiang,
    ]
    .into_iter()
    .map(|kind| ThreatPoolEntry {
        species: ThreatSpecies::Mob(kind),
        weight: 1,
    })
    .collect()
}

/// 从物种池中按权重选取一个物种，先过滤死域禁入物种（`MobSpawnFilter::ban_in_dead_zone`，
/// 复用既有判定不新写；`ThreatSpecies::Rat` 不受 `NaturalMobKind` 死域白名单约束，恒放行）。
/// `seed % total_weight` 做确定性抽样（非密码学强度，同 [`sample_ambient_ring_position`]
/// 的取舍）。过滤后池为空（如死域内 danger3-4 全部 5 变体皆非白名单成员且池恰好只有它们时）
/// 或 `total_weight == 0`（理论不可能出现，池非空则每条目 weight>=1，双重防御）时返回
/// `None`——调用方应把 `None` 当"本次不刷"处理，不能 panic。
pub fn select_threat_species(
    pool: &[ThreatPoolEntry],
    zone: &Zone,
    seed: u64,
) -> Option<ThreatSpecies> {
    let filtered: Vec<&ThreatPoolEntry> = pool
        .iter()
        .filter(|entry| match entry.species {
            ThreatSpecies::Rat => true,
            ThreatSpecies::Mob(kind) => !MobSpawnFilter::ban_in_dead_zone(zone, kind),
        })
        .collect();
    let total_weight: u32 = filtered.iter().map(|entry| entry.weight).sum();
    if total_weight == 0 {
        return None;
    }
    let roll = (seed % total_weight as u64) as u32;
    let mut cumulative = 0u32;
    for entry in &filtered {
        cumulative += entry.weight;
        if roll < cumulative {
            return Some(entry.species);
        }
    }
    // 浮点/整数取整误差兜底：理论上循环必在 roll < total_weight 内命中，走到这里说明
    // cumulative 求和有 bug——保守返回最后一个条目而非 panic，行为可观察便于测试抓回归。
    filtered.last().map(|entry| entry.species)
}

/// 调度核真正物种池 `pool_fn`：接住 `AmbientPoolFn` 契约，从 [`threat_pool`] 选物种后调对应
/// 生成函数。**不透传 `kind` 给 `spawn_natural_mob_at`**——已按 §8.1 #2 订正记录在案：该函数
/// 落地实体外观不受 `kind` 影响（外观由 `fauna_tag_for_beast_spawn` 派生），故此处即便逐一
/// match 枚举、传参数值也只是让调用点契约完整，不是"假装能控外观"。
pub fn ambient_threat_pool_fn(
    commands: &mut Commands,
    layer: Entity,
    zone: &Zone,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Option<Entity> {
    let pool = threat_pool(zone.danger_level, zone.dimension, None);
    let seed = threat_species_seed(&zone.name, spawn_position, zone.danger_level);
    let species = select_threat_species(&pool, zone, seed)?;
    match species {
        ThreatSpecies::Rat => Some(spawn_rat_npc_at(
            commands,
            layer,
            &zone.name,
            spawn_position,
            patrol_target,
        )),
        ThreatSpecies::Mob(kind) => spawn_natural_mob_at(
            commands,
            layer,
            kind,
            &zone.name,
            spawn_position,
            patrol_target,
        ),
    }
}

/// 确定性物种选取种子——混合 zone 名字节 + 生成点坐标位模式 + danger，与
/// [`sample_ambient_ring_position`] 的种子混合手法一致（FNV-1a 起手 + `wrapping_mul`
/// 雪崩常数），不追求密码学强度，只求"同输入同输出"可复现。
fn threat_species_seed(zone_name: &str, spawn_position: DVec3, danger: u8) -> u64 {
    let mut acc = zone_name
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |acc, byte| {
            (acc ^ byte as u64).wrapping_mul(0x0000_0100_0000_01B3)
        });
    acc ^= spawn_position.x.to_bits();
    acc = acc.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    acc ^= spawn_position.z.to_bits();
    acc = acc.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    acc ^= danger as u64;
    acc
}

// ---------------------------------------------------------------------------
// AmbientCheckOutcome — 纯判定核心（不依赖 Bevy World，饱和单测的主要抓手）
// ---------------------------------------------------------------------------

/// 单个 (zone, 最近玩家) 组合的调度判定结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmbientCheckOutcome {
    /// 该 danger 档位巡检间隔未到（`should_run_interval` 未命中）。
    Throttled,
    /// zone 内 ambient 活体已达该 danger 档位的 `max_alive` 上限。
    BudgetSaturated,
    /// era 密度门未放行（随机门控 miss，非"永久拒绝"）。
    EraGateBlocked,
    /// 判定通过，应当刷新；附带命中的预算表（供调用方取 `pack_size_range`）。
    ShouldSpawn { budget: ThreatBudget },
}

/// 纯函数：给定 zone danger、当前 tick、zone 内活体计数、era 密度门参数，判定本次是否应该
/// 刷新。**不做玩家在场判定**——调用方需先过滤出"确有 Overworld 玩家在场"的 zone 才调用
/// 本函数（玩家在场门在 ECS 系统层用位置查询完成，此处保持纯粹便于饱和单测）。
///
/// `zone_kind`（P3 §8.1 #4）：调用方用 `movement::movement_zone_kind(Some(zone), false)`
/// 算出后传入，本函数只负责把它喂给 [`dead_zone_threat_budget`] 放大预算，不重算判定。
pub fn decide_ambient_check(
    now_tick: u64,
    danger_level: u8,
    zone_kind: MovementZoneKind,
    alive_count: u32,
    counts_against_threat_budget: bool,
    era_beast_density_mul: f64,
    era_spawn_seed: u64,
) -> AmbientCheckOutcome {
    let budget = dead_zone_threat_budget(threat_budget(danger_level), zone_kind);
    if !should_run_interval(now_tick, budget.spawn_interval_ticks as u32) {
        return AmbientCheckOutcome::Throttled;
    }
    if counts_against_threat_budget && alive_count >= budget.max_alive {
        return AmbientCheckOutcome::BudgetSaturated;
    }
    if !era_beast_spawn_gate(era_beast_density_mul, era_spawn_seed) {
        return AmbientCheckOutcome::EraGateBlocked;
    }
    AmbientCheckOutcome::ShouldSpawn { budget }
}

/// 超距回收判定：ambient 实体距最近 Overworld 玩家的平面距离 > [`AMBIENT_DESPAWN_RADIUS`]
/// 即应回收。边界取 `>`（严格大于）——恰好 96.0 格不回收，与距离环采样上限 64 留出缓冲带,
/// 防止玩家在环外沿反复横跳时把自己召唤出的威胁瞬间抖动回收。
pub fn should_recycle_ambient(nearest_player_planar_dist: f64) -> bool {
    nearest_player_planar_dist > AMBIENT_DESPAWN_RADIUS
}

// ---------------------------------------------------------------------------
// 距离环采样 —— 复用 PoissonSpawnSampler 的自适应间距参数，环带范围为自研逻辑
// ---------------------------------------------------------------------------

/// 以 `anchor`（通常是触发巡检的玩家位置）为圆心，在
/// [`AMBIENT_RING_MIN_RADIUS`, `AMBIENT_RING_MAX_RADIUS`] 环带内做 Mitchell's
/// best-candidate 采样：`PoissonSpawnSampler::adaptive_for_zone` 只用来取自适应间距参数
/// （`min_same_archetype_dist`/`max_candidates`），环带本身（角度+半径）是本模块新写的
/// 逻辑——`PoissonSpawnSampler::sample_position` 采的是整个 zone AABB 均匀候选点，不是
/// "距玩家 24~64 格环带"，两者用途不同不可直接复用。
///
/// 候选点会被钳制在 `zone_bounds` 内（超出 zone 边界的候选点丢弃）；若 `max_candidates`
/// 次尝试全部越界或与既有点距离不足，返回 `None`（zone 太小/已饱和，本次跳过不刷）。
pub fn sample_ambient_ring_position(
    zone_bounds: (DVec3, DVec3),
    anchor: DVec3,
    existing_positions: &[DVec3],
    rng_seed: u64,
) -> Option<DVec3> {
    let sampler = PoissonSpawnSampler::adaptive_for_zone(zone_bounds);
    let (min, max) = zone_bounds;

    let mut best: Option<DVec3> = None;
    let mut best_score = f64::NEG_INFINITY;

    for i in 0..sampler.max_candidates {
        let seed = rng_seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(i as u64)
            .wrapping_mul(0xbf58_476d_1ce4_e5b9)
            .wrapping_add(rng_seed.rotate_left(17));
        let angle_frac = ((seed & 0xFFFF_FFFF) as f64) / (0xFFFF_FFFF_u64 as f64);
        let radius_frac = (((seed >> 32) & 0xFFFF_FFFF) as f64) / (0xFFFF_FFFF_u64 as f64);

        let angle = angle_frac * std::f64::consts::TAU;
        let radius = AMBIENT_RING_MIN_RADIUS
            + radius_frac * (AMBIENT_RING_MAX_RADIUS - AMBIENT_RING_MIN_RADIUS);

        let cx = anchor.x + angle.cos() * radius;
        let cz = anchor.z + angle.sin() * radius;
        if cx < min.x || cx > max.x || cz < min.z || cz > max.z {
            continue;
        }
        let candidate = DVec3::new(cx, anchor.y, cz);

        let min_dist = existing_positions
            .iter()
            .map(|p| {
                let dx = candidate.x - p.x;
                let dz = candidate.z - p.z;
                (dx * dx + dz * dz).sqrt()
            })
            .fold(f64::INFINITY, f64::min);
        // existing_positions 为空时 min_dist == f64::INFINITY，score 恒最大，首个越界通过的
        // 候选点即被采用——与 PoissonSpawnSampler 首个 NPC 直接落点的语义一致。
        let score = min_dist - sampler.min_same_archetype_dist;
        if score > best_score {
            best_score = score;
            best = Some(candidate);
        }
    }

    best
}

// ---------------------------------------------------------------------------
// AmbientMarkerData —— marker_type 泛型契约
// ---------------------------------------------------------------------------

/// 任意 ambient marker 类型需实现的最小契约：调度核靠这两个方法按 zone 分组统计活体、
/// 构造新实体的 marker 组件，全程不需要知道具体是哪个 plan 的哪种物种。
/// `plan-mundane-fauna-v1` 复用时只需给自己的 marker struct 实现本 trait。
pub trait AmbientMarkerData: Component {
    fn new(spawned_at: u64, home_zone: String) -> Self;
    fn home_zone(&self) -> &str;
}

/// P0 定义的威胁 marker（P1 起挂在真实生成的妖兽/鼠群实体上）。
#[derive(Debug, Clone, Component)]
pub struct AmbientThreatMarker {
    pub spawned_at: u64,
    pub home_zone: String,
}

impl AmbientMarkerData for AmbientThreatMarker {
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
// AmbientSchedulerState / AmbientSchedulerConfig —— 每 marker_type 独立一份
// ---------------------------------------------------------------------------

/// 调度核状态资源，按 `M` 泛型单态化——`AmbientThreatMarker` 与未来
/// `plan-mundane-fauna-v1` 的 marker 类型各自持有独立实例，互不干扰节流计时。
#[derive(Resource)]
pub struct AmbientSchedulerState<M> {
    last_check_tick: u64,
    _marker: PhantomData<fn() -> M>,
}

impl<M> Default for AmbientSchedulerState<M> {
    fn default() -> Self {
        Self {
            last_check_tick: 0,
            _marker: PhantomData,
        }
    }
}

/// `pool_fn` 接 `(commands, overworld layer, zone, spawn_pos, patrol_target)`，返回
/// `Some(实体)` 表示真的刷出了什么，`None` 表示本次物种池未命中（死域过滤后池为空等）。
/// 调度核负责在 pool_fn 产出实体后追加挂载 marker 组件，pool_fn 自身不需要关心 marker 类型。
///
/// **P1 收口**：签名用 `&Zone`（而非 P0 初版的 `u8, &str`）——`AmbientThreatMarker` 的真实
/// `pool_fn`（[`ambient_threat_pool_fn`]）需要 `zone.spirit_qi` 驱动
/// `MobSpawnFilter::ban_in_dead_zone` 死域白名单过滤（§8.1 #2），仅 `danger_level` 不够；
/// `Zone` 本就同时携带 `danger_level`/`name`/`spirit_qi`，改传引用比拆两个标量参数更干净，
/// 调度核调用点（`ambient_scheduler_system`）本就持有 `&Zone`，改动零成本。
pub type AmbientPoolFn = fn(&mut Commands, Entity, &Zone, DVec3, DVec3) -> Option<Entity>;

/// 每 marker_type 独立注入的调度配置：`budget_fn` 决定"刷多少/多久"，`pool_fn` 决定
/// "刷什么"，`counts_against_threat_budget` 决定该 marker 类型的活体数是否计入
/// `budget_fn` 返回的 `max_alive`（`plan-mundane-fauna-v1` 的被动 pool 传 `false`，
/// 不占本 plan 的威胁预算）。
#[derive(Resource)]
pub struct AmbientSchedulerConfig<M> {
    pub budget_fn: fn(u8) -> ThreatBudget,
    pub pool_fn: AmbientPoolFn,
    pub counts_against_threat_budget: bool,
    _marker: PhantomData<fn() -> M>,
}

impl<M> AmbientSchedulerConfig<M> {
    pub fn new(
        budget_fn: fn(u8) -> ThreatBudget,
        pool_fn: AmbientPoolFn,
        counts_against_threat_budget: bool,
    ) -> Self {
        Self {
            budget_fn,
            pool_fn,
            counts_against_threat_budget,
            _marker: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// System: register
// ---------------------------------------------------------------------------

pub fn register(app: &mut App) {
    app.insert_resource(AmbientSchedulerState::<AmbientThreatMarker>::default())
        .insert_resource(AmbientSchedulerConfig::<AmbientThreatMarker>::new(
            threat_budget,
            ambient_threat_pool_fn,
            true,
        ))
        .add_systems(Update, ambient_scheduler_system::<AmbientThreatMarker>);
}

/// 通用 ambient 调度系统，按 `M: AmbientMarkerData` 单态化——每个 marker 类型注册一次
/// 独立实例（各自持有独立的 `AmbientSchedulerState<M>`/`AmbientSchedulerConfig<M>`
/// 资源与独立的 `Query<&M, ...>`），互不干扰。
#[allow(clippy::too_many_arguments)]
pub fn ambient_scheduler_system<M: AmbientMarkerData>(
    tick: Option<Res<GameTick>>,
    mut state: ResMut<AmbientSchedulerState<M>>,
    config: Res<AmbientSchedulerConfig<M>>,
    alive: Query<(Entity, &M, &Position), Without<Despawned>>,
    players: Query<(&Position, Option<&CurrentDimension>), With<ClientMarker>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    era_state: Option<Res<WorldEraState>>,
    mut commands: Commands,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);

    // 粗节流：与 heiwushi 一致，避免每 tick 都做全量 zone/query 扫描。
    // `now == 0` 时必须放行（对齐 `should_run_interval` 的 tick==0 分支）——否则
    // `state.last_check_tick` 默认值也是 0，`0.saturating_sub(0) == 0 < STRIDE` 会让世界
    // 起服后第一次巡检被粗节流吞掉，永远进不到任何真实判定分支。
    if now != 0 && now.saturating_sub(state.last_check_tick) < AMBIENT_SCHEDULER_STRIDE_TICKS {
        return;
    }
    state.last_check_tick = now;

    let Some(registry) = zone_registry.as_deref() else {
        return;
    };
    let Some(layers) = dimension_layers.as_deref() else {
        return;
    };
    let density_mul = era_state
        .as_deref()
        .map(|e| e.current_modifiers().beast_density_mul)
        .unwrap_or(1.0);

    // 只看 Overworld 玩家——TSY 维度接活是 P1 §"TSY 接活"独立小节，不走本调度核。
    let overworld_players: Vec<DVec3> = players
        .iter()
        .filter(|(_, dim)| {
            dim.map(|d| d.0).unwrap_or(DimensionKind::Overworld) == DimensionKind::Overworld
        })
        .map(|(pos, _)| pos.get())
        .collect();

    // 1) 超距回收：任意存活 ambient marker 距最近 Overworld 玩家 > 96 格 → Despawned。
    //    Valence 层实体裸 despawn 会在下一次 send_entity_update_messages 崩服，必须走
    //    insert(Despawned) 软删除模式（对齐 heiwushi/scenario/dying_master 现有回收路径）。
    for (entity, _marker, pos) in &alive {
        let nearest = overworld_players
            .iter()
            .map(|p| planar_distance(pos.get(), *p))
            .fold(f64::INFINITY, f64::min);
        if should_recycle_ambient(nearest) {
            commands.entity(entity).insert(Despawned);
        }
    }

    if overworld_players.is_empty() {
        // 无玩家在场：不做任何刷新判定（对齐 heiwushi 步骤 5 的玩家在场门）。
        return;
    }

    // 2) 逐玩家找 zone，判定预算 + 间隔 + era 密度门，命中就在距该玩家 24~64 格环带内刷新。
    for player_pos in &overworld_players {
        let Some(zone) = registry.find_zone(DimensionKind::Overworld, *player_pos) else {
            continue;
        };

        let alive_in_zone: Vec<DVec3> = alive
            .iter()
            .filter(|(_, marker, _)| marker.home_zone() == zone.name)
            .map(|(_, _, pos)| pos.get())
            .collect();
        let alive_count = alive_in_zone.len() as u32;

        let spawn_seed =
            now.wrapping_add((zone.name.len() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));

        // P3 §8.1 #4 — 死域/负灵域预算乘区：复用既有 `movement_zone_kind` 判定口径
        // （`on_residue_ash=false`，ambient 调度核只关心危险度地理，不关心灰烬地表微观判定）。
        let zone_kind = movement_zone_kind(Some(zone), false);
        let outcome = decide_ambient_check(
            now,
            zone.danger_level,
            zone_kind,
            alive_count,
            config.counts_against_threat_budget,
            density_mul,
            spawn_seed,
        );
        let AmbientCheckOutcome::ShouldSpawn { budget } = outcome else {
            continue;
        };

        let Some(spawn_pos) =
            sample_ambient_ring_position(zone.bounds, *player_pos, &alive_in_zone, spawn_seed)
        else {
            continue;
        };

        let Some(spawned) =
            (config.pool_fn)(&mut commands, layers.overworld, zone, spawn_pos, spawn_pos)
        else {
            // 死域过滤后池为空 / 未来其它 pool_fn 实现判定本次不刷都会走这支——非 panic 分支。
            // `budget.pack_size_range`（多只群体刷新）不在本 plan 范围内消费，留给后续若立项
            // "群体刷新"再回填，当前调度核每次巡检命中只产 1 个实体。
            let _ = budget.pack_size_range;
            continue;
        };
        commands
            .entity(spawned)
            .insert(M::new(now, zone.name.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::spawn::common::NpcMarker;
    use crate::npc::spawn_rat::RatBlackboard;
    use valence::prelude::App;

    // -----------------------------------------------------------------
    // threat_budget —— 预算表边界
    // -----------------------------------------------------------------

    #[test]
    fn threat_budget_danger_zero_clamps_to_danger_one() {
        // Zone::spawn() 硬编码 fallback danger_level=0（zones.json 缺失时才出现）；
        // 必须钳到 danger=1 档，绝不 panic 或返回零预算（否则 fallback zone 世界永远
        // 一只都刷不出）。
        assert_eq!(
            threat_budget(0),
            threat_budget(1),
            "danger=0 应钳到 danger=1 同档预算"
        );
    }

    #[test]
    fn threat_budget_danger_one_is_sparse_nonzero() {
        let budget = threat_budget(1);
        assert!(
            budget.max_alive >= 1 && budget.max_alive <= 2,
            "danger=1 max_alive 应落在 1~2 档（零星非包围），实际 {}",
            budget.max_alive
        );
        assert_eq!(
            budget.spawn_interval_ticks, 600,
            "danger=1 间隔应 ~600 tick"
        );
    }

    #[test]
    fn threat_budget_danger_seven_is_dense() {
        let budget = threat_budget(7);
        assert!(
            budget.max_alive >= 8 && budget.max_alive <= 10,
            "danger=7 max_alive 应落在 8~10 档，实际 {}",
            budget.max_alive
        );
        assert_eq!(
            budget.spawn_interval_ticks, 150,
            "danger=7 间隔应 ~150 tick"
        );
    }

    #[test]
    fn threat_budget_unknown_high_danger_clamps_to_danger_seven() {
        // danger_level 是 u8，理论上可以是 8~255（脏数据/未来枚举扩展）——必须钳到
        // danger=7 的合法上限档，不能因为超出表而 panic 或产出无意义的更大预算。
        assert_eq!(
            threat_budget(200),
            threat_budget(7),
            "danger>7 的未知 zone 应钳到 danger=7 同档预算"
        );
    }

    #[test]
    fn threat_budget_monotonic_max_alive_across_all_dangers() {
        // 预算表必须整体单调不减：danger 越高威胁越密集，不能出现"danger=5 比 danger=3
        // 活体上限还低"这种反直觉数值倒挂。
        let mut prev = threat_budget(1).max_alive;
        for danger in 2..=7u8 {
            let cur = threat_budget(danger).max_alive;
            assert!(
                cur >= prev,
                "danger={danger} max_alive={cur} 不应低于前一档 {prev}"
            );
            prev = cur;
        }
    }

    #[test]
    fn threat_budget_spawn_intervals_are_multiples_of_scheduler_stride() {
        // AMBIENT_SCHEDULER_STRIDE_TICKS 是调度核的粗节流步长；若某档 spawn_interval_ticks
        // 不是它的整数倍，粗节流会漏检该档在 should_run_interval 命中的 tick（见模块头注释），
        // 静默丢失该档所有刷新——这是能直接崩坏生产行为的回归，必须锁死。
        for danger in 1..=7u8 {
            let interval = threat_budget(danger).spawn_interval_ticks as u64;
            assert_eq!(
                interval % AMBIENT_SCHEDULER_STRIDE_TICKS,
                0,
                "danger={danger} 的 spawn_interval_ticks={interval} 必须是粗节流步长 {} 的整数倍",
                AMBIENT_SCHEDULER_STRIDE_TICKS
            );
        }
    }

    // -----------------------------------------------------------------
    // threat_pool —— danger 1~7 物种池分层（§8.1 #2）
    // -----------------------------------------------------------------

    fn zone_with(danger_level: u8, spirit_qi: f64, dimension: DimensionKind) -> Zone {
        Zone {
            name: "test_zone".to_string(),
            dimension,
            bounds: (
                DVec3::new(-500.0, 0.0, -500.0),
                DVec3::new(500.0, 200.0, 500.0),
            ),
            spirit_qi,
            danger_level,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    #[test]
    fn threat_pool_danger_one_and_two_are_rat_only() {
        for danger in [1u8, 2u8] {
            let pool = threat_pool(danger, DimensionKind::Overworld, None);
            assert_eq!(
                pool,
                vec![ThreatPoolEntry {
                    species: ThreatSpecies::Rat,
                    weight: 1
                }],
                "danger={danger} 应恰好是 Rat 单条目池（§8.1 #2 danger1-2 中立袭扰档）"
            );
        }
    }

    #[test]
    fn threat_pool_danger_three_and_four_are_five_generic_beasts_no_ash_spider() {
        for danger in [3u8, 4u8] {
            let pool = threat_pool(danger, DimensionKind::Overworld, None);
            assert_eq!(
                pool.len(),
                5,
                "danger={danger} 应是 5 变体通用 beast 池（不含 AshSpider）"
            );
            assert!(
                !pool
                    .iter()
                    .any(|e| e.species == ThreatSpecies::Mob(NaturalMobKind::AshSpider)),
                "danger={danger} 不应包含 AshSpider（该物种只在 danger>=5 档加入）"
            );
            for kind in [
                NaturalMobKind::Zombie,
                NaturalMobKind::Skeleton,
                NaturalMobKind::Creeper,
                NaturalMobKind::Rogue,
                NaturalMobKind::Daoxiang,
            ] {
                assert!(
                    pool.iter().any(|e| e.species == ThreatSpecies::Mob(kind)),
                    "danger={danger} 池应含 {kind:?}"
                );
            }
        }
    }

    #[test]
    fn threat_pool_danger_five_six_seven_add_ash_spider_same_pool() {
        let pools: Vec<_> = [5u8, 6u8, 7u8]
            .into_iter()
            .map(|danger| threat_pool(danger, DimensionKind::Overworld, None))
            .collect();
        for (idx, pool) in pools.iter().enumerate() {
            assert_eq!(
                pool.len(),
                6,
                "danger={} 应是 5 通用 beast + AshSpider = 6 条目",
                idx + 5
            );
            assert!(
                pool.iter()
                    .any(|e| e.species == ThreatSpecies::Mob(NaturalMobKind::AshSpider)),
                "danger={} 应含 AshSpider（死域白名单物种）",
                idx + 5
            );
        }
        assert_eq!(
            pools[0], pools[1],
            "danger5 与 danger6 必须是同一份池（§8.1 #2 未区分 5/6）"
        );
        assert_eq!(
            pools[1], pools[2],
            "danger7 必须与 5~6 档同一份池——§8.1 #2/#3 明令 danger7 只调 pack/interval，\
             不新增变体/buff，本测试锁死禁止悄悄给 danger7 加新物种"
        );
    }

    #[test]
    fn threat_pool_danger_zero_clamps_to_one_matches_budget_clamp_semantics() {
        // 与 threat_budget 的 danger=0 兜底钳位口径对齐（Zone::spawn() fallback）。
        assert_eq!(
            threat_pool(0, DimensionKind::Overworld, None),
            threat_pool(1, DimensionKind::Overworld, None),
            "danger=0 应钳到 danger=1 同档物种池，不能 panic 或返回空池"
        );
    }

    #[test]
    fn threat_pool_danger_above_seven_clamps_to_seven() {
        assert_eq!(
            threat_pool(200, DimensionKind::Overworld, None),
            threat_pool(7, DimensionKind::Overworld, None),
            "danger>7 的脏数据/未来扩展应钳到 danger=7 同档物种池"
        );
    }

    #[test]
    fn threat_pool_non_overworld_dimension_is_always_empty() {
        // TSY 自然涌现走独立直调 spawn_tsy_hostiles_for_family 路径（§8.1 #3），
        // 不复用本表——任何 danger 档在非 Overworld 维度都必须返回空池。
        for danger in 1..=7u8 {
            assert_eq!(
                threat_pool(danger, DimensionKind::Tsy, None),
                Vec::new(),
                "danger={danger} 在 DimensionKind::Tsy 下必须是空池（TSY 不走本表）"
            );
        }
    }

    #[test]
    fn threat_pool_weight_hook_is_inert_placeholder() {
        // §8.1 #6：weight_hook 当前恒无操作，任意取值不应改变产出池。
        for hook in [None, Some(0.0f32), Some(0.5), Some(2.0), Some(-1.0)] {
            assert_eq!(
                threat_pool(4, DimensionKind::Overworld, hook),
                threat_pool(4, DimensionKind::Overworld, None),
                "weight_hook={hook:?} 不应影响 danger=4 池内容（§8.1 #6 昼夜/天气权重占位钩子）"
            );
        }
    }

    // -----------------------------------------------------------------
    // select_threat_species —— 权重抽样 + 死域过滤
    // -----------------------------------------------------------------

    #[test]
    fn select_threat_species_pins_rat_regardless_of_seed() {
        let pool = threat_pool(1, DimensionKind::Overworld, None);
        let zone = zone_with(1, 0.5, DimensionKind::Overworld);
        for seed in [0u64, 1, 999, u64::MAX] {
            assert_eq!(
                select_threat_species(&pool, &zone, seed),
                Some(ThreatSpecies::Rat),
                "seed={seed} 单条目 Rat 池应恒选中 Rat"
            );
        }
    }

    #[test]
    fn select_threat_species_exhaustive_weight_distribution_pin() {
        // danger3-4 池 5 条目均权重 1，total_weight=5——seed 0..5 应恰好遍历全部 5 个物种
        // 各一次（roll = seed % 5），锁死"权重抽样按 cumulative 累加正确落位"这条契约。
        let pool = threat_pool(3, DimensionKind::Overworld, None);
        let zone = zone_with(3, 0.5, DimensionKind::Overworld);
        let mut hit: Vec<ThreatSpecies> = (0u64..5)
            .map(|seed| select_threat_species(&pool, &zone, seed).expect("非死域池非空必命中"))
            .collect();
        hit.sort_by_key(|s| format!("{s:?}"));
        let mut expected: Vec<ThreatSpecies> = pool.iter().map(|entry| entry.species).collect();
        expected.sort_by_key(|s| format!("{s:?}"));
        assert_eq!(
            hit, expected,
            "seed 0..5 遍历 5 权重相等条目应恰好各命中一次，缺失/重复说明 cumulative 累加有 off-by-one"
        );
    }

    #[test]
    fn select_threat_species_dead_zone_filters_non_whitelisted_generic_beasts() {
        // is_dead_zone 阈值是 spirit_qi < 0.01（cultivation::dead_zone::DEAD_ZONE_QI_THRESHOLD）。
        let dead_zone = zone_with(5, 0.0, DimensionKind::Overworld);
        let pool = threat_pool(5, DimensionKind::Overworld, None);
        for seed in 0u64..6 {
            let species = select_threat_species(&pool, &dead_zone, seed);
            assert!(
                matches!(
                    species,
                    Some(ThreatSpecies::Mob(NaturalMobKind::AshSpider))
                        | Some(ThreatSpecies::Mob(NaturalMobKind::Daoxiang))
                ),
                "seed={seed} 死域(spirit_qi=0.0)只应选中 AshSpider/Daoxiang（死域白名单），\
                 实际选中 {species:?}——Zombie/Skeleton/Creeper/Rogue 必须被 ban_in_dead_zone 挡下"
            );
        }
    }

    #[test]
    fn select_threat_species_non_dead_zone_allows_all_generic_beasts() {
        let live_zone = zone_with(5, 0.5, DimensionKind::Overworld);
        let pool = threat_pool(5, DimensionKind::Overworld, None);
        // 6 条目全权重 1，seed 0..6 应恰好遍历全部（非死域不过滤任何条目）。
        let mut hit: Vec<ThreatSpecies> = (0u64..6)
            .map(|seed| select_threat_species(&pool, &live_zone, seed).expect("非死域必命中"))
            .collect();
        hit.sort_by_key(|s| format!("{s:?}"));
        let mut expected: Vec<ThreatSpecies> = pool.iter().map(|e| e.species).collect();
        expected.sort_by_key(|s| format!("{s:?}"));
        assert_eq!(hit, expected, "非死域 zone 不应过滤任何 danger=5 池条目");
    }

    #[test]
    fn select_threat_species_returns_none_when_pool_empty() {
        let zone = zone_with(1, 0.5, DimensionKind::Overworld);
        assert_eq!(
            select_threat_species(&[], &zone, 42),
            None,
            "空池应返回 None，不能 panic 或凭空造一个物种"
        );
    }

    #[test]
    fn select_threat_species_deterministic_for_same_seed() {
        let pool = threat_pool(6, DimensionKind::Overworld, None);
        let zone = zone_with(6, 0.5, DimensionKind::Overworld);
        let a = select_threat_species(&pool, &zone, 777);
        let b = select_threat_species(&pool, &zone, 777);
        assert_eq!(a, b, "同一 seed 必须产出同一物种（可复现，非真随机）");
    }

    // -----------------------------------------------------------------
    // ambient_threat_pool_fn —— 真实 pool_fn 端到端（替换 P0 stub）
    // -----------------------------------------------------------------

    #[test]
    fn ambient_threat_pool_fn_spawns_rat_marker_in_danger_one_zone() {
        let mut app = App::new();
        let layer = app.world_mut().spawn_empty().id();
        let zone = zone_with(1, 0.5, DimensionKind::Overworld);
        let spawned = {
            let mut commands = app.world_mut().commands();
            ambient_threat_pool_fn(
                &mut commands,
                layer,
                &zone,
                DVec3::new(10.0, 64.0, 10.0),
                DVec3::new(10.0, 64.0, 10.0),
            )
        };
        app.world_mut().flush();
        let entity = spawned.expect("danger=1 非死域池非空，必须刷出实体");
        assert!(
            app.world().get::<RatBlackboard>(entity).is_some(),
            "danger=1 物种池只有 Rat，产出实体必须带 RatBlackboard（spawn_rat_npc_at 契约）"
        );
    }

    #[test]
    fn ambient_threat_pool_fn_spawns_beast_marker_in_danger_four_zone() {
        let mut app = App::new();
        let layer = app.world_mut().spawn_empty().id();
        let zone = zone_with(4, 0.5, DimensionKind::Overworld);
        let spawned = {
            let mut commands = app.world_mut().commands();
            ambient_threat_pool_fn(
                &mut commands,
                layer,
                &zone,
                DVec3::new(10.0, 64.0, 10.0),
                DVec3::new(10.0, 64.0, 10.0),
            )
        };
        app.world_mut().flush();
        let entity = spawned.expect("danger=4 通用 beast 池非空，必须刷出实体");
        assert!(
            app.world().get::<RatBlackboard>(entity).is_none(),
            "danger=4 通用 beast 路径不应带 RatBlackboard（会误判成 rat 分支）"
        );
        assert!(
            app.world().get::<NpcMarker>(entity).is_some(),
            "danger=4 spawn_beast_npc_at 产出实体必须带通用 NpcMarker"
        );
    }

    #[test]
    fn ambient_threat_pool_fn_returns_none_for_tsy_dimension() {
        let mut app = App::new();
        let layer = app.world_mut().spawn_empty().id();
        let zone = zone_with(5, 0.5, DimensionKind::Tsy);
        let spawned = {
            let mut commands = app.world_mut().commands();
            ambient_threat_pool_fn(
                &mut commands,
                layer,
                &zone,
                DVec3::new(10.0, 64.0, 10.0),
                DVec3::new(10.0, 64.0, 10.0),
            )
        };
        assert_eq!(
            spawned, None,
            "TSY 维度必须走独立直调 spawn_tsy_hostiles_for_family 路径（§8.1 #3），\
             本通用 pool_fn 对 TSY zone 必须恒 None，不能顺手刷出主世界物种"
        );
    }

    // -----------------------------------------------------------------
    // decide_ambient_check —— 纯判定核心
    // -----------------------------------------------------------------

    #[test]
    fn decide_throttled_when_interval_not_elapsed() {
        // danger=1 → interval 600；tick=599 不是 600 的倍数也不是 0 → Throttled。
        let outcome = decide_ambient_check(599, 1, MovementZoneKind::Normal, 0, true, 1.0, 0);
        assert_eq!(outcome, AmbientCheckOutcome::Throttled);
    }

    #[test]
    fn decide_runs_at_tick_zero_regardless_of_interval() {
        // should_run_interval 的 `tick == 0` 分支：世界刚起服第一帧就该有判定机会。
        let outcome = decide_ambient_check(0, 7, MovementZoneKind::Normal, 0, true, 1.0, 0);
        assert_eq!(
            outcome,
            AmbientCheckOutcome::ShouldSpawn {
                budget: threat_budget(7)
            }
        );
    }

    #[test]
    fn decide_budget_saturated_when_alive_count_at_max() {
        let budget = threat_budget(1);
        let outcome = decide_ambient_check(
            600,
            1,
            MovementZoneKind::Normal,
            budget.max_alive,
            true,
            1.0,
            0,
        );
        assert_eq!(outcome, AmbientCheckOutcome::BudgetSaturated);
    }

    #[test]
    fn decide_budget_saturated_boundary_one_below_max_passes() {
        let budget = threat_budget(1);
        let outcome = decide_ambient_check(
            600,
            1,
            MovementZoneKind::Normal,
            budget.max_alive - 1,
            true,
            1.0,
            0,
        );
        assert_eq!(
            outcome,
            AmbientCheckOutcome::ShouldSpawn { budget },
            "活体数恰好比上限少 1 时应放行，off-by-one 出错会导致 zone 永远刷不满或提前锁死"
        );
    }

    #[test]
    fn decide_ignores_budget_when_counts_against_threat_budget_false() {
        // plan-mundane-fauna-v1 的被动 pool 用 counts_against_threat_budget=false：
        // 即使"活体数"远超 max_alive 也不应被威胁预算拦截（它压根不算威胁预算的一部分）。
        let budget = threat_budget(1);
        let outcome = decide_ambient_check(
            600,
            1,
            MovementZoneKind::Normal,
            budget.max_alive * 100,
            false,
            1.0,
            0,
        );
        assert_eq!(outcome, AmbientCheckOutcome::ShouldSpawn { budget });
    }

    #[test]
    fn decide_era_gate_blocks_when_density_mul_zero() {
        // beast_density_mul=0.0 时 era_beast_spawn_gate 恒 false（有效概率钳到 0）。
        let outcome = decide_ambient_check(600, 1, MovementZoneKind::Normal, 0, true, 0.0, 42);
        assert_eq!(outcome, AmbientCheckOutcome::EraGateBlocked);
    }

    #[test]
    fn decide_era_gate_passes_when_density_mul_at_or_above_one() {
        // Calamity 时代 beast_density_mul > 1.0 时应始终放行（clamp 到
        // ERA_BEAST_SPAWN_DENSITY_CLAMP_MAX=2.0 后仍 >= 1.0）。
        let outcome = decide_ambient_check(600, 1, MovementZoneKind::Normal, 0, true, 1.5, 999);
        assert_eq!(
            outcome,
            AmbientCheckOutcome::ShouldSpawn {
                budget: threat_budget(1)
            }
        );
    }

    #[test]
    fn decide_era_gate_clamp_extreme_density_still_passes() {
        // beast_density_mul 远超 ERA_BEAST_SPAWN_DENSITY_CLAMP_MAX(2.0) 时应被钳到 2.0
        // 而非无脑当成"必过"外的其他分支——不管 seed 取什么都应放行。
        let outcome = decide_ambient_check(600, 1, MovementZoneKind::Normal, 0, true, 99.0, 0);
        assert_eq!(
            outcome,
            AmbientCheckOutcome::ShouldSpawn {
                budget: threat_budget(1)
            }
        );
    }

    #[test]
    fn decide_ambient_check_dead_zone_widens_budget_over_normal() {
        // 同一 danger=7 档下，Dead zone_kind 应比 Normal 命中放大后的预算（§8.1 #4），
        // 而不是"zone_kind 参数被悄悄忽略"的回归。
        let outcome = decide_ambient_check(0, 7, MovementZoneKind::Dead, 0, true, 1.0, 0);
        assert_eq!(
            outcome,
            AmbientCheckOutcome::ShouldSpawn {
                budget: dead_zone_threat_budget(threat_budget(7), MovementZoneKind::Dead)
            },
            "Dead zone_kind 必须命中 dead_zone_threat_budget 放大后的预算，不是原始 threat_budget(7)"
        );
        assert_ne!(
            threat_budget(7),
            dead_zone_threat_budget(threat_budget(7), MovementZoneKind::Dead),
            "测试前置条件：danger=7 的死域乘区必须真的放大了预算，否则本用例测不出回归"
        );
    }

    #[test]
    fn decide_ambient_check_dead_zone_budget_saturation_uses_widened_max_alive() {
        // 死域放大后的 max_alive 更宽——原本会 BudgetSaturated 的活体数在死域里应放行。
        let normal_budget = threat_budget(7);
        let dead_budget = dead_zone_threat_budget(normal_budget, MovementZoneKind::Dead);
        assert!(
            dead_budget.max_alive > normal_budget.max_alive,
            "前置条件：死域 max_alive 必须严格大于常态，否则本用例的活体数选取无意义"
        );
        let alive_count = normal_budget.max_alive; // 常态下已饱和，死域里应仍未饱和
        let outcome = decide_ambient_check(0, 7, MovementZoneKind::Dead, alive_count, true, 1.0, 0);
        assert_eq!(
            outcome,
            AmbientCheckOutcome::ShouldSpawn {
                budget: dead_budget
            },
            "活体数={alive_count} 常态下已达上限={}，死域放宽后的上限={} 应仍放行",
            normal_budget.max_alive,
            dead_budget.max_alive
        );
    }

    // -----------------------------------------------------------------
    // dead_zone_threat_budget / danger_tide_weight —— P3 生态联动（§8.1 #4/#5）
    // -----------------------------------------------------------------

    #[test]
    fn dead_zone_threat_budget_normal_is_identity() {
        // Normal zone_kind 必须原样返回预算——不放大不缩小，否则常态世界的预算表全部
        // 静默漂移，回归极难察觉。
        for danger in 1..=7u8 {
            let budget = threat_budget(danger);
            assert_eq!(
                dead_zone_threat_budget(budget, MovementZoneKind::Normal),
                budget,
                "danger={danger} 的 Normal zone_kind 必须是 identity"
            );
        }
    }

    #[test]
    fn dead_zone_threat_budget_residue_ash_is_identity() {
        // ResidueAsh 是灰烬地表微观判定，非本 plan 危险度语义覆盖范围——保守不放大。
        let budget = threat_budget(4);
        assert_eq!(
            dead_zone_threat_budget(budget, MovementZoneKind::ResidueAsh),
            budget
        );
    }

    #[test]
    fn dead_zone_threat_budget_dead_amplifies_max_alive_and_shortens_interval() {
        let budget = threat_budget(5);
        let scaled = dead_zone_threat_budget(budget, MovementZoneKind::Dead);
        assert!(
            scaled.max_alive > budget.max_alive,
            "Dead zone_kind 必须放大 max_alive：原始 {}，放大后 {}",
            budget.max_alive,
            scaled.max_alive
        );
        assert!(
            scaled.spawn_interval_ticks < budget.spawn_interval_ticks,
            "Dead zone_kind 必须缩短 spawn_interval_ticks（更频繁刷新）：原始 {}，缩短后 {}",
            budget.spawn_interval_ticks,
            scaled.spawn_interval_ticks
        );
        assert!(
            scaled.pack_size_range.1 >= budget.pack_size_range.1,
            "Dead zone_kind 的 pack_size 上限不应低于原始值"
        );
    }

    #[test]
    fn dead_zone_threat_budget_negative_amplifies_less_than_dead() {
        let budget = threat_budget(5);
        let dead = dead_zone_threat_budget(budget, MovementZoneKind::Dead);
        let negative = dead_zone_threat_budget(budget, MovementZoneKind::Negative);
        assert!(
            negative.max_alive >= budget.max_alive,
            "Negative zone_kind 也应放大（不小于原始值）"
        );
        assert!(
            negative.max_alive <= dead.max_alive,
            "Negative 乘区({:?})不应超过 Dead 乘区({:?})——死域凶险程度高于负灵域",
            negative.max_alive,
            dead.max_alive
        );
    }

    #[test]
    fn dead_zone_threat_budget_never_zeroes_interval() {
        // 极端情况下缩放不能把 spawn_interval_ticks 缩到 0——0 会让 should_run_interval
        // 出现除零/无限刷新的边界灾难。
        for danger in 1..=7u8 {
            let scaled = dead_zone_threat_budget(threat_budget(danger), MovementZoneKind::Dead);
            assert!(
                scaled.spawn_interval_ticks >= 1,
                "danger={danger} 死域缩放后 spawn_interval_ticks 不能为 0"
            );
        }
    }

    #[test]
    fn danger_tide_weight_danger_one_is_identity() {
        // danger=0（zones.json 缺失兜底）与 danger=1 都应钳到权重 1.0——覆盖所有沿用
        // `danger_level: 0` 兜底 fixture 的既有 heartbeat.rs 测试，保证它们行为不变。
        assert_eq!(danger_tide_weight(0), 1.0);
        assert_eq!(danger_tide_weight(1), 1.0);
    }

    #[test]
    fn danger_tide_weight_danger_seven_is_max() {
        assert!(
            (danger_tide_weight(7) - 1.6).abs() < 1e-9,
            "danger=7 权重应为 1.6（+60%），实际 {}",
            danger_tide_weight(7)
        );
    }

    #[test]
    fn danger_tide_weight_monotonic_non_decreasing() {
        let mut prev = danger_tide_weight(1);
        for danger in 2..=7u8 {
            let cur = danger_tide_weight(danger);
            assert!(
                cur >= prev,
                "danger={danger} 权重 {cur} 不应低于前一档 {prev}——danger 越高兽潮应越容易触发"
            );
            prev = cur;
        }
    }

    #[test]
    fn danger_tide_weight_above_seven_clamps_to_seven() {
        assert_eq!(danger_tide_weight(200), danger_tide_weight(7));
    }

    #[test]
    fn danger_tide_required_ticks_scale_is_reciprocal_of_weight() {
        for danger in 0..=7u8 {
            let scale = danger_tide_required_ticks_scale(danger);
            let weight = danger_tide_weight(danger);
            assert!(
                (scale * weight - 1.0).abs() < 1e-9,
                "danger={danger}: scale({scale}) * weight({weight}) 应恒为 1.0"
            );
        }
    }

    #[test]
    fn danger_tide_required_ticks_scale_danger_one_is_identity() {
        assert_eq!(danger_tide_required_ticks_scale(1), 1.0);
    }

    #[test]
    fn danger_tide_required_ticks_scale_danger_seven_shortens_duration() {
        let scale = danger_tide_required_ticks_scale(7);
        assert!(
            scale < 1.0,
            "danger=7 的 required_ticks 缩放系数必须 < 1.0（缩短所需时长），实际 {scale}"
        );
        assert!(
            (scale - 0.625).abs() < 1e-9,
            "danger=7 应缩至 1/1.6=0.625，实际 {scale}"
        );
    }

    // -----------------------------------------------------------------
    // should_recycle_ambient —— 超距回收 off-by-one
    // -----------------------------------------------------------------

    #[test]
    fn should_recycle_false_at_exact_boundary() {
        assert!(
            !should_recycle_ambient(AMBIENT_DESPAWN_RADIUS),
            "恰好 96.0 格不应回收（严格大于才回收）"
        );
    }

    #[test]
    fn should_recycle_true_just_past_boundary() {
        assert!(
            should_recycle_ambient(AMBIENT_DESPAWN_RADIUS + 0.001),
            "超过 96.0 格哪怕一点点也应回收"
        );
    }

    #[test]
    fn should_recycle_false_well_within_radius() {
        assert!(!should_recycle_ambient(10.0));
    }

    // -----------------------------------------------------------------
    // sample_ambient_ring_position —— 距离环 off-by-one + 确定性
    // -----------------------------------------------------------------

    fn wide_open_bounds() -> (DVec3, DVec3) {
        (
            DVec3::new(-10_000.0, 0.0, -10_000.0),
            DVec3::new(10_000.0, 200.0, 10_000.0),
        )
    }

    #[test]
    fn ring_sample_always_within_ring_radius_bounds() {
        let anchor = DVec3::new(0.0, 64.0, 0.0);
        for seed in 0..200u64 {
            let Some(pos) = sample_ambient_ring_position(wide_open_bounds(), anchor, &[], seed)
            else {
                panic!("seed={seed} 在无穷大 bounds + 无既有点时不应返回 None");
            };
            let dx = pos.x - anchor.x;
            let dz = pos.z - anchor.z;
            let dist = (dx * dx + dz * dz).sqrt();
            assert!(
                (AMBIENT_RING_MIN_RADIUS - 1e-6..=AMBIENT_RING_MAX_RADIUS + 1e-6).contains(&dist),
                "seed={seed} 采样距离 {dist} 超出环带 [{AMBIENT_RING_MIN_RADIUS}, {AMBIENT_RING_MAX_RADIUS}]"
            );
        }
    }

    #[test]
    fn ring_sample_deterministic_for_same_seed() {
        let anchor = DVec3::new(100.0, 64.0, 100.0);
        let a = sample_ambient_ring_position(wide_open_bounds(), anchor, &[], 12345);
        let b = sample_ambient_ring_position(wide_open_bounds(), anchor, &[], 12345);
        assert_eq!(a, b, "同一 seed 必须产出同一候选点（可复现，非真随机）");
    }

    #[test]
    fn ring_sample_none_when_zone_bounds_exclude_entire_ring() {
        // zone 边界比环带内环还小 → 所有候选点必然越界 → None。
        let tiny_bounds = (DVec3::new(-1.0, 0.0, -1.0), DVec3::new(1.0, 200.0, 1.0));
        let anchor = DVec3::new(0.0, 64.0, 0.0);
        let result = sample_ambient_ring_position(tiny_bounds, anchor, &[], 7);
        assert_eq!(
            result, None,
            "zone 边界完全排除 24~64 格环带时应返回 None，不能越界刷出 zone 外"
        );
    }

    #[test]
    fn ring_sample_respects_existing_position_spacing_preference() {
        // 存在既有点时，采样器应更偏向远离既有点的候选（best_score 逻辑）——
        // 用两个不同 existing_positions 集合验证输出不同，证明 existing_positions 确实
        // 参与了打分（而非被忽略的死参数）。
        let anchor = DVec3::new(0.0, 64.0, 0.0);
        let existing_a = vec![DVec3::new(30.0, 64.0, 0.0)];
        let existing_b = vec![DVec3::new(-30.0, 64.0, 0.0)];
        let a = sample_ambient_ring_position(wide_open_bounds(), anchor, &existing_a, 42);
        let b = sample_ambient_ring_position(wide_open_bounds(), anchor, &existing_b, 42);
        assert_ne!(
            a, b,
            "existing_positions 改变时同 seed 下应选出不同候选点，证明间距打分生效"
        );
    }

    // -----------------------------------------------------------------
    // ambient_scheduler_system —— 泛型 ECS 集成测试
    // -----------------------------------------------------------------

    /// 独立于 `AmbientThreatMarker` 的第二 marker 类型，验证调度核对不同 `M` 单态化后
    /// 状态/配置/query 完全互不干扰（`plan-mundane-fauna-v1` 复用场景的最小复现）。
    #[derive(Debug, Clone, Component)]
    struct TestFaunaMarker {
        spawned_at: u64,
        home_zone: String,
    }

    impl AmbientMarkerData for TestFaunaMarker {
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

    fn test_pool_fn(
        commands: &mut Commands,
        _layer: Entity,
        _zone: &Zone,
        spawn_position: DVec3,
        _patrol_target: DVec3,
    ) -> Option<Entity> {
        Some(
            commands
                .spawn((
                    Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                    crate::npc::spawn::common::NpcMarker,
                ))
                .id(),
        )
    }

    fn make_app() -> App {
        let mut app = App::new();
        app.insert_resource(AmbientSchedulerState::<AmbientThreatMarker>::default())
            .insert_resource(AmbientSchedulerConfig::<AmbientThreatMarker>::new(
                threat_budget,
                test_pool_fn,
                true,
            ))
            .add_systems(Update, ambient_scheduler_system::<AmbientThreatMarker>);
        app
    }

    fn install_layers(app: &mut App) -> Entity {
        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        overworld
    }

    fn install_zone_registry(app: &mut App, danger_level: u8) {
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "test_zone".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (
                    DVec3::new(-500.0, 0.0, -500.0),
                    DVec3::new(500.0, 200.0, 500.0),
                ),
                spirit_qi: 0.5,
                danger_level,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            }],
        });
    }

    #[test]
    fn no_spawn_when_no_player_present() {
        let mut app = make_app();
        install_layers(&mut app);
        install_zone_registry(&mut app, 7);
        app.insert_resource(GameTick(1_000_000));
        app.update();

        let mut q = app
            .world_mut()
            .query_filtered::<(), With<AmbientThreatMarker>>();
        assert_eq!(
            q.iter(app.world()).count(),
            0,
            "无玩家在场时不应刷新任何 ambient 威胁"
        );
    }

    #[test]
    fn no_spawn_when_no_zone_registry() {
        let mut app = make_app();
        install_layers(&mut app);
        // 故意不插入 ZoneRegistry。
        app.insert_resource(GameTick(1_000_000));
        app.world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])));
        app.update();

        let mut q = app
            .world_mut()
            .query_filtered::<(), With<AmbientThreatMarker>>();
        assert_eq!(
            q.iter(app.world()).count(),
            0,
            "缺 ZoneRegistry 时应安全跳过"
        );
    }

    #[test]
    fn spawns_and_tags_marker_when_player_in_high_danger_zone() {
        let mut app = make_app();
        install_layers(&mut app);
        install_zone_registry(&mut app, 7);
        app.insert_resource(GameTick(0)); // tick=0 → should_run_interval 恒真
        app.world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])));
        app.update();

        let mut q = app
            .world_mut()
            .query_filtered::<&AmbientThreatMarker, With<AmbientThreatMarker>>();
        let markers: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(
            markers.len(),
            1,
            "danger=7 + 玩家在场 + tick=0 应恰好刷出 1 个 ambient 威胁（test_pool_fn 每次产 1 个）"
        );
        assert_eq!(markers[0].home_zone, "test_zone");
        assert_eq!(markers[0].spawned_at, 0);
    }

    #[test]
    fn does_not_spawn_beyond_budget_saturation() {
        let mut app = make_app();
        install_layers(&mut app);
        install_zone_registry(&mut app, 1); // danger=1 → max_alive=2
        app.insert_resource(GameTick(0));
        app.world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])));

        // 预先塞 2 个活体 ambient 威胁（已达 danger=1 的上限）。
        for _ in 0..2 {
            app.world_mut().spawn((
                Position::new([10.0, 64.0, 10.0]),
                AmbientThreatMarker {
                    spawned_at: 0,
                    home_zone: "test_zone".to_string(),
                },
            ));
        }
        app.update();

        let mut q = app
            .world_mut()
            .query_filtered::<(), With<AmbientThreatMarker>>();
        assert_eq!(
            q.iter(app.world()).count(),
            2,
            "danger=1 已有 2 个活体（=max_alive）时不应再刷新第 3 个"
        );
    }

    #[test]
    fn recycles_marker_beyond_despawn_radius_via_insert_despawned() {
        let mut app = make_app();
        install_layers(&mut app);
        install_zone_registry(&mut app, 1);
        app.insert_resource(GameTick(0));
        // 玩家远在天边，ambient 威胁距其 > 96 格。
        app.world_mut()
            .spawn((ClientMarker, Position::new([10_000.0, 64.0, 10_000.0])));
        let stray = app
            .world_mut()
            .spawn((
                Position::new([0.0, 64.0, 0.0]),
                AmbientThreatMarker {
                    spawned_at: 0,
                    home_zone: "test_zone".to_string(),
                },
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<Despawned>(stray).is_some(),
            "超距 ambient 威胁必须通过 insert(Despawned) 回收（裸 despawn 会崩服）"
        );
    }

    #[test]
    fn generic_marker_type_is_independent_from_threat_marker() {
        // 泛型参数注入独立生效：第二套 marker_type（TestFaunaMarker）用不同的
        // budget_fn（恒返回 max_alive=1，逼近饱和）+ counts_against_threat_budget=false，
        // 与 AmbientThreatMarker 的调度状态/资源/query 完全独立。
        fn tiny_budget(_danger: u8) -> ThreatBudget {
            ThreatBudget {
                max_alive: 1,
                spawn_interval_ticks: 50,
                pack_size_range: (1, 1),
            }
        }

        let mut app = make_app();
        install_layers(&mut app);
        install_zone_registry(&mut app, 1);
        app.insert_resource(GameTick(0));
        app.world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])));

        // 第二套调度实例：counts_against_threat_budget=false，即使已有 100 个"活体"
        // TestFaunaMarker 也不应被 tiny_budget 的 max_alive=1 拦下。
        for _ in 0..100 {
            app.world_mut().spawn((
                Position::new([5.0, 64.0, 5.0]),
                TestFaunaMarker {
                    spawned_at: 0,
                    home_zone: "test_zone".to_string(),
                },
            ));
        }
        app.insert_resource(AmbientSchedulerState::<TestFaunaMarker>::default())
            .insert_resource(AmbientSchedulerConfig::<TestFaunaMarker>::new(
                tiny_budget,
                test_pool_fn,
                false,
            ))
            .add_systems(Update, ambient_scheduler_system::<TestFaunaMarker>);

        app.update();

        let mut threat_q = app
            .world_mut()
            .query_filtered::<(), With<AmbientThreatMarker>>();
        let mut fauna_q = app.world_mut().query::<&TestFaunaMarker>();
        assert_eq!(
            threat_q.iter(app.world()).count(),
            1,
            "AmbientThreatMarker 调度实例应正常独立刷出 1 个（danger=1, max_alive=2, 0 在场）"
        );
        let fauna_markers: Vec<_> = fauna_q.iter(app.world()).collect();
        assert_eq!(
            fauna_markers.len(),
            101,
            "TestFaunaMarker 调度实例 counts_against_threat_budget=false，\
             100 个既有活体不拦截新刷新，应变成 101（不与 AmbientThreatMarker 预算互相干扰）"
        );
        assert!(
            fauna_markers.iter().all(|m| m.spawned_at == 0),
            "本轮巡检 tick=0，所有 TestFaunaMarker（既有 100 个预置 spawned_at=0 + 新刷 1 个\
             由 M::new(now, ..) 构造）都应记录 spawned_at=0，验证 marker 构造契约按 tick 落账"
        );
    }

    // -----------------------------------------------------------------
    // P3 §8.1 — ambient 存量 beast 衔接 beast_horde_detect_system 集成 case
    // -----------------------------------------------------------------

    #[test]
    fn ambient_spawned_beast_feeds_beast_horde_detect_system_beast_count() {
        // 声明性集成 case（plan §P3「horde 衔接」）：ambient 调度核刷出的常驻 beast 走
        // `spawn_natural_mob_at` → `spawn_beast_npc_at`，本就挂 `NpcArchetype::Beast`
        // （`npc/spawn/beast.rs:82`），天然被 `fauna::migration::is_horde_beast` 识别——
        // **不需要在 `fauna/migration.rs` 里改一行代码**（本 plan 只声明衔接，不吞
        // `plan-beast-horde-v1` P2 领地争夺 scope）。本用例把这条衔接坐实成一条真实跑通
        // 的断言，防止未来任一侧重构悄悄断开这条链路。
        use crate::fauna::migration::{
            beast_horde_detect_system, BeastHordeEvent, BeastHordeState, FlowFieldComputeTask,
            FlowFieldPrototype, ZoneDepletionEvent,
        };
        use valence::prelude::Events;

        let mut app = App::new();
        app.insert_resource(AmbientSchedulerState::<AmbientThreatMarker>::default())
            .insert_resource(AmbientSchedulerConfig::<AmbientThreatMarker>::new(
                threat_budget,
                ambient_threat_pool_fn,
                true,
            ))
            .add_systems(Update, ambient_scheduler_system::<AmbientThreatMarker>);
        install_layers(&mut app);
        app.insert_resource(ZoneRegistry {
            zones: vec![
                Zone {
                    name: "test_zone".to_string(),
                    dimension: DimensionKind::Overworld,
                    bounds: (
                        DVec3::new(-500.0, 0.0, -500.0),
                        DVec3::new(500.0, 200.0, 500.0),
                    ),
                    spirit_qi: 0.05,
                    danger_level: 7,
                    active_events: Vec::new(),
                    patrol_anchors: Vec::new(),
                    blocked_tiles: Vec::new(),
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                },
                // select_migration_target_zone 需要一个 spirit_qi 更高的邻域才能选出
                // migration target；无 ZoneGraph 时 fallback 到全体 zones。
                Zone {
                    name: "test_zone_refuge".to_string(),
                    dimension: DimensionKind::Overworld,
                    bounds: (
                        DVec3::new(1000.0, 0.0, 1000.0),
                        DVec3::new(1500.0, 200.0, 1500.0),
                    ),
                    spirit_qi: 0.8,
                    danger_level: 1,
                    active_events: Vec::new(),
                    patrol_anchors: Vec::new(),
                    blocked_tiles: Vec::new(),
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                },
            ],
        });
        app.insert_resource(GameTick(0));
        app.world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])));
        app.update(); // 用真实 pool_fn 刷出 1 只 ambient beast（携带 NpcArchetype::Beast）。

        let mut marker_q = app
            .world_mut()
            .query_filtered::<(), With<AmbientThreatMarker>>();
        assert_eq!(
            marker_q.iter(app.world()).count(),
            1,
            "前置条件：ambient 调度核必须先真的刷出 1 个威胁实体，否则本用例测不出衔接"
        );

        // 接上 beast_horde_detect_system：同一 App 里追加 migration 系统链路，喂一条低
        // 灵气 ZoneDepletionEvent 触发兽潮检测——不改 fauna/migration.rs 任何判定逻辑。
        app.insert_resource(BeastHordeState::default());
        app.add_event::<ZoneDepletionEvent>();
        app.add_event::<BeastHordeEvent>();
        app.add_event::<FlowFieldPrototype>();
        app.add_event::<FlowFieldComputeTask>();
        app.add_systems(Update, beast_horde_detect_system);
        app.world_mut()
            .resource_mut::<Events<ZoneDepletionEvent>>()
            .send(ZoneDepletionEvent {
                zone: "test_zone".to_string(),
                spirit_qi: 0.05,
                spirit_qi_rate_of_change: -0.01,
                tick: 0,
            });
        app.update();

        let hordes: Vec<BeastHordeEvent> = {
            let events = app.world().resource::<Events<BeastHordeEvent>>();
            events.get_reader().read(events).cloned().collect()
        };
        assert_eq!(
            hordes.len(),
            1,
            "beast_horde_detect_system 应识别出 ambient 已刷出的存量 beast 并触发迁徙，\
             不应因 beast_count==0 短路——若为空数组，说明 ambient marker 挂的组件没被\
             migration 的 is_horde_beast 识别到，horde 衔接已断"
        );
        assert!(
            hordes[0].beast_count >= 1,
            "beast_count 应 >= 1（至少数到 ambient 刷出的那只），实际 {}",
            hordes[0].beast_count
        );
        assert_eq!(hordes[0].source_zone, "test_zone");
    }
}
