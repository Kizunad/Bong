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

use std::collections::HashMap;
use std::marker::PhantomData;

use valence::client::ClientMarker;
use valence::prelude::{
    apply_deferred, bevy_ecs, App, Chunk, ChunkLayer, ChunkPos, Commands, Component, DVec3,
    Despawned, Entity, IntoSystemConfigs, IntoSystemSetConfigs, Position, Query, Res, ResMut,
    Resource, SystemSet, Update, With, Without,
};

use crate::cultivation::components::{ActorQiIdentity, ActorQiKind, Cultivation, QiFlowError};
use crate::cultivation::life_record::LifeRecord;
use crate::fauna::mimic_spider::{transfer_spider_qi_to_zone, MimicSpiderBlackboard};
use crate::fauna::mundane::{mundane_pool_fn, MundaneFaunaMarker};
use crate::fauna::rat_phase::transfer_rat_drained_qi_to_zone_or_overflow;
use crate::movement::{movement_zone_kind, MovementZoneKind};
use crate::npc::dormant::{planar_distance, should_run_interval};
use crate::npc::movement::GameTick;
use crate::npc::spawn::PoissonSpawnSampler;
use crate::npc::spawn_rat::{spawn_rat_npc_at, RatBlackboard};
use crate::qi_physics::WorldQiAccount;
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::era::WorldEraState;
use crate::world::mob_spawn::{
    era_beast_spawn_gate, spawn_natural_mob_at, MobSpawnFilter, NaturalMobKind,
};
use crate::world::season::{Season, WorldSeasonState};
use crate::world::terrain::{SurfaceProvider, TerrainProviders};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
pub enum AmbientTerminalSystemSet {
    Recycle,
    Flush,
    PostRecycle,
}

pub fn configure_terminal_schedule(app: &mut App) {
    app.configure_sets(
        Update,
        (
            AmbientTerminalSystemSet::Recycle,
            AmbientTerminalSystemSet::Flush,
            AmbientTerminalSystemSet::PostRecycle,
        )
            .chain(),
    )
    .add_systems(
        Update,
        apply_deferred.in_set(AmbientTerminalSystemSet::Flush),
    );
}

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
    // §Verify blocker① — 量化到 AMBIENT_SCHEDULER_STRIDE_TICKS 的整数倍，见 round_to_stride 文档。
    let scaled_interval = round_to_stride(
        ((f64::from(budget.spawn_interval_ticks) / multiplier).round() as u32)
            .max(1)
            .min(budget.spawn_interval_ticks),
    );
    let scaled_pack_max = ((f64::from(budget.pack_size_range.1) * multiplier).round() as u32)
        .max(budget.pack_size_range.1);
    ThreatBudget {
        max_alive: scaled_max_alive,
        spawn_interval_ticks: scaled_interval,
        pack_size_range: (budget.pack_size_range.0, scaled_pack_max),
    }
}

/// 把 spawn_interval_ticks 量化到 [`AMBIENT_SCHEDULER_STRIDE_TICKS`] 的整数倍（§Verify
/// blocker①——stride 混叠）：`ambient_scheduler_system` 的粗节流只在 `now_tick` 恰好是
/// stride(50) 整数倍时才真正跑到 `should_run_interval(now_tick, interval)` 判定；若
/// `interval` 本身不是 50 的整数倍，`now_tick % interval == 0` 的下一次命中会被推迟到
/// `lcm(50, interval)`——对于与 50 互质/低公因子的档位（如 10/14），这能从设计预期的
/// 十几秒~几十秒暴涨到 5200~20850 tick（约 4~17 分钟）。四舍五入到最近的 stride 倍数
/// （`(ticks + stride/2) / stride * stride`，与 blocker 给出的 `((iv + 25)/50)*50` 例子
/// 等价），并钳最低为一个 stride（不能是 0——0 会让 `should_run_interval` 的
/// `interval.max(1)` 钳到 1，退化成"每 tick 都刷"）。
fn round_to_stride(ticks: u32) -> u32 {
    let stride = AMBIENT_SCHEDULER_STRIDE_TICKS as u32;
    let rounded = ((ticks + stride / 2) / stride) * stride;
    rounded.max(stride)
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
    _season: Season,
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
    if !should_run_interval(now_tick, budget.spawn_interval_ticks) {
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

/// Ambient-only outcome of scanning a loaded runtime column for a landing.
///
/// `Unsafe` is deliberately narrow: a standable support with liquid in its feet
/// or head cell is authoritative runtime data that a stale raster must not
/// override. Other columns without a safe support remain `Miss` so the ambient
/// resolver may use its explicit fallback path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GroundLandingScan {
    Safe(i32),
    Unsafe,
    Miss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GroundLandingCheck {
    Safe,
    LiquidObstructed,
    Miss,
}

/// Scan the standard ambient runtime window for the topmost safe footing.
fn scan_ground_landing_from_chunk(
    wx: i32,
    wz: i32,
    ref_y: i32,
    layer: Option<&ChunkLayer>,
) -> GroundLandingScan {
    scan_ground_landing_from_chunk_range(wx, wz, ref_y - 16, ref_y + 4, layer)
}

fn scan_ground_landing_from_chunk_range(
    wx: i32,
    wz: i32,
    bottom: i32,
    top: i32,
    layer: Option<&ChunkLayer>,
) -> GroundLandingScan {
    let Some(layer) = layer else {
        return GroundLandingScan::Miss;
    };
    let min_y = layer.min_y();
    let max_y = min_y + layer.height() as i32 - 1;

    let chunk_pos = ChunkPos::new(wx.div_euclid(16), wz.div_euclid(16));
    if layer.chunk(chunk_pos).is_none() {
        return GroundLandingScan::Miss;
    }

    let scan_top = top.min(max_y);
    let scan_bottom = bottom.max(min_y);
    if scan_bottom > scan_top {
        return GroundLandingScan::Miss;
    }

    let mut liquid_obstructed = false;
    for ground_y in (scan_bottom..=scan_top).rev() {
        match classify_ground_landing_at(wx, wz, ground_y, layer) {
            GroundLandingCheck::Safe => return GroundLandingScan::Safe(ground_y),
            GroundLandingCheck::LiquidObstructed => liquid_obstructed = true,
            GroundLandingCheck::Miss => {}
        }
    }

    if liquid_obstructed {
        GroundLandingScan::Unsafe
    } else {
        GroundLandingScan::Miss
    }
}

/// Whether `ground_y` is a safe ambient landing in a loaded layer.
///
/// Support must block motion and be neither liquid/waterlogged, passthrough, nor leaves.
/// Feet and head must both be readable, clear, non-liquid/waterlogged, and non-leaves.
fn is_safe_ground_landing_at(wx: i32, wz: i32, ground_y: i32, layer: &ChunkLayer) -> bool {
    classify_ground_landing_at(wx, wz, ground_y, layer) == GroundLandingCheck::Safe
}

fn classify_ground_landing_at(
    wx: i32,
    wz: i32,
    ground_y: i32,
    layer: &ChunkLayer,
) -> GroundLandingCheck {
    let min_y = layer.min_y();
    let max_y = min_y + layer.height() as i32 - 1;
    let chunk_pos = ChunkPos::new(wx.div_euclid(16), wz.div_euclid(16));
    let Some(chunk) = layer.chunk(chunk_pos) else {
        return GroundLandingCheck::Miss;
    };
    let local_x = wx.rem_euclid(16) as u32;
    let local_z = wz.rem_euclid(16) as u32;
    let block_at = |world_y: i32| {
        (min_y..=max_y)
            .contains(&world_y)
            .then(|| chunk.block_state(local_x, (world_y - min_y) as u32, local_z))
    };
    let (Some(support), Some(feet), Some(head)) = (
        block_at(ground_y),
        block_at(ground_y + 1),
        block_at(ground_y + 2),
    ) else {
        return GroundLandingCheck::Miss;
    };

    // A liquid/waterlogged structural support is not merely an invalid foothold:
    // it is authoritative loaded data that must veto raster fallback. Preserve
    // `Miss` for non-motion blocks, passthrough, and leaves, which are not
    // structural landing candidates at all.
    if support.blocks_motion()
        && !is_ambient_passthrough_block(support)
        && !is_ambient_leaf_block(support)
        && contains_ambient_liquid(support)
    {
        return GroundLandingCheck::LiquidObstructed;
    }
    if !is_strict_ground_support(support) {
        return GroundLandingCheck::Miss;
    }
    if contains_ambient_liquid(feet) || contains_ambient_liquid(head) {
        return GroundLandingCheck::LiquidObstructed;
    }
    if is_clear_for_ground(feet) && is_clear_for_ground(head) {
        GroundLandingCheck::Safe
    } else {
        GroundLandingCheck::Miss
    }
}

/// Whether a block has a liquid kind or carries water through a waterlogged state.
fn contains_ambient_liquid(block: valence::prelude::BlockState) -> bool {
    use valence::prelude::{PropName, PropValue};

    block.is_liquid() || block.get(PropName::Waterlogged) == Some(PropValue::True)
}

fn is_strict_ground_support(block: valence::prelude::BlockState) -> bool {
    block.blocks_motion()
        && !contains_ambient_liquid(block)
        && !is_ambient_passthrough_block(block)
        && !is_ambient_leaf_block(block)
}

fn is_clear_for_ground(block: valence::prelude::BlockState) -> bool {
    !block.blocks_motion() && !contains_ambient_liquid(block) && !is_ambient_leaf_block(block)
}

// These explicit block sets intentionally mirror Navigator's legacy classifiers,
// but remain private because this is an ambient admission contract, not navigation.
fn is_ambient_passthrough_block(block: valence::prelude::BlockState) -> bool {
    use valence::prelude::BlockState;

    block == BlockState::GRASS
        || block == BlockState::TALL_GRASS
        || block == BlockState::FERN
        || block == BlockState::LARGE_FERN
        || block == BlockState::POPPY
        || block == BlockState::DANDELION
        || block == BlockState::DEAD_BUSH
        || block == BlockState::LILY_PAD
        || block == BlockState::SNOW
        || block == BlockState::VINE
        || block == BlockState::TORCH
        || block == BlockState::WALL_TORCH
        || block == BlockState::RAIL
        || block == BlockState::REDSTONE_WIRE
}

fn is_ambient_leaf_block(block: valence::prelude::BlockState) -> bool {
    use valence::prelude::BlockKind;

    matches!(
        block.to_kind(),
        BlockKind::OakLeaves
            | BlockKind::SpruceLeaves
            | BlockKind::BirchLeaves
            | BlockKind::JungleLeaves
            | BlockKind::AcaciaLeaves
            | BlockKind::DarkOakLeaves
            | BlockKind::AzaleaLeaves
            | BlockKind::FloweringAzaleaLeaves
            | BlockKind::CherryLeaves
            | BlockKind::MangroveLeaves
    )
}

/// Result of consulting the live runtime column for an ambient spawn.
///
/// `NeedsRaster { loaded_chunk: false }` means no runtime column is available, so a passable raster
/// may provide the fallback directly. A loaded chunk whose standard scan misses returns
/// `NeedsRaster { loaded_chunk: true }`: after obtaining the raster Y, ambient must re-check that
/// exact runtime landing before accepting it. `LoadedUnsafe` is an authoritative veto discovered
/// by the standard scan and never consults stale raster data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AmbientRuntimeGround {
    Safe(i32),
    LoadedUnsafe,
    NeedsRaster { loaded_chunk: bool },
}

fn loaded_ambient_landing_is_safe(
    world_x: i32,
    world_z: i32,
    ground_y: i32,
    layer: &ChunkLayer,
) -> bool {
    is_safe_ground_landing_at(world_x, world_z, ground_y, layer)
}

fn resolve_ambient_runtime_ground(
    world_x: i32,
    world_z: i32,
    reference_y: i32,
    layer: Option<&ChunkLayer>,
) -> AmbientRuntimeGround {
    let Some(layer) = layer else {
        return AmbientRuntimeGround::NeedsRaster {
            loaded_chunk: false,
        };
    };
    let chunk_pos = ChunkPos::new(world_x.div_euclid(16), world_z.div_euclid(16));
    if layer.chunk(chunk_pos).is_none() {
        return AmbientRuntimeGround::NeedsRaster {
            loaded_chunk: false,
        };
    }

    match scan_ground_landing_from_chunk(world_x, world_z, reference_y, Some(layer)) {
        GroundLandingScan::Safe(ground_y) => AmbientRuntimeGround::Safe(ground_y),
        GroundLandingScan::Unsafe => AmbientRuntimeGround::LoadedUnsafe,
        GroundLandingScan::Miss => AmbientRuntimeGround::NeedsRaster { loaded_chunk: true },
    }
}

/// Validate a raster fallback against the exact landing cells of an already-loaded runtime chunk.
///
/// Raster surface Y is a baked hint, not authority over authored/player blocks. Its exact
/// support/feet/head cells must be readable, standable, and non-liquid. Nearby supports do not
/// override the raster landing; an out-of-height or unsafe landing fails closed.
fn resolve_loaded_raster_landing(
    world_x: i32,
    world_z: i32,
    surface_y: i32,
    layer: &ChunkLayer,
) -> AmbientRuntimeGround {
    if !loaded_ambient_landing_is_safe(world_x, world_z, surface_y, layer) {
        return AmbientRuntimeGround::LoadedUnsafe;
    }

    AmbientRuntimeGround::Safe(surface_y)
}

/// Resolve an ambient ground candidate from the live world first, then the terrain raster.
///
/// A loaded [`ChunkLayer`] is authoritative because it contains caves, floating islands,
/// decorations, and player-built blocks that the baked raster cannot represent. The runtime
/// scan deliberately reuses Navigator's standard `ref_y - 16 .. ref_y + 4` support/headroom
/// contract. Missing runtime data may use a passable raster directly; a loaded standard-window
/// miss may use it only after the exact runtime raster landing passes support/feet/head validation.
/// A loaded support whose support/feet/head block is any water or lava state is an authoritative
/// veto. Both sources missing or unsafe rejects the candidate; the sampled/player Y is never
/// preserved.
fn resolve_ambient_ground_position<P: SurfaceProvider + ?Sized>(
    candidate: DVec3,
    layer: Option<&ChunkLayer>,
    terrain: Option<&P>,
) -> Option<DVec3> {
    let world_x = candidate.x.floor() as i32;
    let world_z = candidate.z.floor() as i32;
    let reference_y = candidate.y.floor() as i32;

    let loaded_chunk_needs_raster_validation =
        match resolve_ambient_runtime_ground(world_x, world_z, reference_y, layer) {
            AmbientRuntimeGround::Safe(ground_y) => {
                return Some(DVec3::new(
                    candidate.x,
                    f64::from(ground_y + 1),
                    candidate.z,
                ));
            }
            AmbientRuntimeGround::LoadedUnsafe => return None,
            AmbientRuntimeGround::NeedsRaster { loaded_chunk } => loaded_chunk,
        };

    let surface = terrain?.query_surface(world_x, world_z);
    if !surface.passable {
        return None;
    }
    if loaded_chunk_needs_raster_validation {
        match resolve_loaded_raster_landing(world_x, world_z, surface.y, layer?) {
            AmbientRuntimeGround::Safe(_) => {}
            AmbientRuntimeGround::LoadedUnsafe | AmbientRuntimeGround::NeedsRaster { .. } => {
                return None
            }
        }
    }

    Some(DVec3::new(
        candidate.x,
        f64::from(surface.y + 1),
        candidate.z,
    ))
}

/// 以 `anchor`（通常是触发巡检的玩家位置）为圆心，在
/// [`AMBIENT_RING_MIN_RADIUS`, `AMBIENT_RING_MAX_RADIUS`] 环带内做 Mitchell's
/// best-candidate 采样：`PoissonSpawnSampler::adaptive_for_zone` 只用来取自适应间距参数
/// （`min_same_archetype_dist`/`max_candidates`），环带本身（角度+半径）是本模块新写的
/// 逻辑——`PoissonSpawnSampler::sample_position` 采的是整个 zone AABB 均匀候选点，不是
/// "距玩家 24~64 格环带"，两者用途不同不可直接复用。
///
/// 候选点会被钳制在 `zone_bounds` 内（超出 zone 边界的候选点丢弃）。在合法候选中优先
/// 选择距既有点最远者；若它们都低于最小间距，仍保留最远候选作为既有 best-effort fallback。
/// 只有所有候选均越界时才返回 `None`。
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
        // Keep the pre-fix sampler's best-effort fallback: density is a scoring
        // preference, not an admission gate for an in-bounds ring candidate.
        if min_dist > best_score {
            best_score = min_dist;
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
///
/// **P2 追加尾参 `Season`**（plan-mundane-fauna-v1 P2「季节权重」）：`mundane_pool_fn`
/// 需要当前季节去偏权物种池；调度核本就在 `ambient_scheduler_system` 里读
/// `Res<WorldSeasonState>`（新增，`Option` 缺省 `Season::default()`），随调用一并透传。
/// 同 `threat_pool` 的 `weight_hook: Option<f32>` 占位钩子先例——`ambient_threat_pool_fn`
/// 直接忽略此参数（`_season`），不产生任何行为变化，避免跨 plan 共享 `AmbientPoolFn`
/// 类型时牵连另一侧实现。
pub type AmbientPoolFn = fn(&mut Commands, Entity, &Zone, DVec3, DVec3, Season) -> Option<Entity>;

/// Scheduler 候选提交所需的不可分割上下文。地表解析、真实 pool 调用、marker 挂载与
/// pending 记账必须共用这一条提交路径，避免测试只覆盖 resolver、生产调用点却再次漏接。
struct AmbientSpawnRequest<'a> {
    layer: Entity,
    zone: &'a Zone,
    candidate: DVec3,
    season: Season,
    now: u64,
}

/// 把一个已通过 scheduler 预算门的环带候选提交给真实 pool。
///
/// 提交严格 fail-closed：runtime 标准窗口和 raster fallback 都无法给出安全脚点、或 pool
/// 拒绝时均返回 `None`，既不挂 marker 也不占本 tick pending 预算；只有真实 pool 返回实体后
/// 才依次挂载 marker 并增加 pending。
fn submit_ambient_spawn_candidate<M, P>(
    commands: &mut Commands,
    layer: Option<&ChunkLayer>,
    terrain: Option<&P>,
    pool_fn: AmbientPoolFn,
    pending_spawns_by_zone: &mut HashMap<String, u32>,
    request: AmbientSpawnRequest<'_>,
) -> Option<Entity>
where
    M: AmbientMarkerData,
    P: SurfaceProvider + ?Sized,
{
    let spawn_position = resolve_ambient_ground_position(request.candidate, layer, terrain)?;
    let spawned = pool_fn(
        commands,
        request.layer,
        request.zone,
        spawn_position,
        spawn_position,
        request.season,
    )?;
    commands
        .entity(spawned)
        .insert(M::new(request.now, request.zone.name.clone()));
    *pending_spawns_by_zone
        .entry(request.zone.name.clone())
        .or_insert(0) += 1;
    Some(spawned)
}

/// Dev-only one-shot ambient pool selector. This is crate-visible so the command layer can exercise
/// the exact production submission boundary without exposing either concrete `spawn_*` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AmbientDevSpawnKind {
    Mundane,
    Threat,
}

impl AmbientDevSpawnKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Mundane => "mundane",
            Self::Threat => "threat",
        }
    }
}

/// One-shot dev ambient input. It preserves the production resolver inputs while keeping the
/// command seam from growing a flat argument list.
pub(crate) struct AmbientDevSpawnRequest<'a, P: SurfaceProvider + ?Sized> {
    pub kind: AmbientDevSpawnKind,
    pub layer: Entity,
    pub runtime_layer: Option<&'a ChunkLayer>,
    pub terrain: Option<&'a P>,
    pub zone: &'a Zone,
    pub candidate: DVec3,
    pub season: Season,
    pub now: u64,
}

/// Submit exactly one dev ambient candidate through the production resolver, real species pool,
/// marker insertion, and pending accounting path.
///
/// `request.layer` is the authoritative Overworld entity layer used by the real pool.
/// `request.runtime_layer` and `request.terrain` deliberately remain independently optional: a
/// loaded runtime layer works without a raster, while a passable raster can resolve an
/// unloaded/missing runtime layer. The local pending map exists only to preserve the production
/// submission contract; this one-shot bypasses scheduler cadence and budget by design and never
/// leaks its resolved Y as a command oracle.
pub(crate) fn submit_ambient_dev_spawn_once<P: SurfaceProvider + ?Sized>(
    commands: &mut Commands,
    request: AmbientDevSpawnRequest<'_, P>,
) -> Option<Entity> {
    let AmbientDevSpawnRequest {
        kind,
        layer,
        runtime_layer,
        terrain,
        zone,
        candidate,
        season,
        now,
    } = request;
    let request = AmbientSpawnRequest {
        layer,
        zone,
        candidate,
        season,
        now,
    };
    let mut pending_spawns_by_zone = HashMap::new();

    match kind {
        AmbientDevSpawnKind::Mundane => submit_ambient_spawn_candidate::<MundaneFaunaMarker, P>(
            commands,
            runtime_layer,
            terrain,
            mundane_pool_fn,
            &mut pending_spawns_by_zone,
            request,
        ),
        AmbientDevSpawnKind::Threat => submit_ambient_spawn_candidate::<AmbientThreatMarker, P>(
            commands,
            runtime_layer,
            terrain,
            ambient_threat_pool_fn,
            &mut pending_spawns_by_zone,
            request,
        ),
    }
}

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
        .add_systems(
            Update,
            ambient_scheduler_system::<AmbientThreatMarker>
                .in_set(AmbientTerminalSystemSet::Recycle),
        );
}

fn settle_rat_recycle(
    cultivation: &mut Cultivation,
    life_record: &LifeRecord,
    zone: Option<&mut Zone>,
    ledger: &mut WorldQiAccount,
) -> Result<(), QiFlowError> {
    let identity = ActorQiIdentity::from_life_record(life_record, ActorQiKind::Npc)?;
    let rat_account = identity.account();
    let mut staged_cultivation = cultivation.clone();
    let mut staged_zone = zone.as_deref().cloned();
    let mut staged_ledger = ledger.clone();
    let cultivation_qi = staged_cultivation.qi_current;
    staged_cultivation.release_to_zone(
        staged_zone.as_mut(),
        &mut staged_ledger,
        &identity,
        cultivation_qi,
        crate::qi_physics::ledger::QiTransferReason::ReleaseToZone,
    )?;
    transfer_rat_drained_qi_to_zone_or_overflow(
        &mut staged_ledger,
        staged_zone.as_mut(),
        &rat_account,
    )?;
    *cultivation = staged_cultivation;
    if let (Some(zone), Some(staged_zone)) = (zone, staged_zone) {
        *zone = staged_zone;
    }
    *ledger = staged_ledger;
    Ok(())
}

fn settle_spider_recycle(
    cultivation: &mut Cultivation,
    life_record: &LifeRecord,
    zone: Option<&mut Zone>,
    ledger: &mut WorldQiAccount,
) -> Result<(), QiFlowError> {
    let mut staged_cultivation = cultivation.clone();
    let mut staged_zone = zone.as_deref().cloned();
    let mut staged_ledger = ledger.clone();
    transfer_spider_qi_to_zone(
        &mut staged_cultivation,
        staged_zone.as_mut(),
        &mut staged_ledger,
        life_record,
    )?;
    *cultivation = staged_cultivation;
    if let (Some(zone), Some(staged_zone)) = (zone, staged_zone) {
        *zone = staged_zone;
    }
    *ledger = staged_ledger;
    Ok(())
}

/// 通用 ambient 调度系统，按 `M: AmbientMarkerData` 单态化——每个 marker 类型注册一次
/// 独立实例（各自持有独立的 `AmbientSchedulerState<M>`/`AmbientSchedulerConfig<M>`
/// 资源与独立的 `Query<&M, ...>`），互不干扰。
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn ambient_scheduler_system<M: AmbientMarkerData>(
    tick: Option<Res<GameTick>>,
    mut state: ResMut<AmbientSchedulerState<M>>,
    config: Res<AmbientSchedulerConfig<M>>,
    mut alive: Query<
        (
            Entity,
            &M,
            &Position,
            Option<&RatBlackboard>,
            Option<&MimicSpiderBlackboard>,
            Option<&LifeRecord>,
            Option<&mut Cultivation>,
        ),
        Without<Despawned>,
    >,
    players: Query<(&Position, Option<&CurrentDimension>), With<ClientMarker>>,
    zone_registry: Option<ResMut<ZoneRegistry>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    era_state: Option<Res<WorldEraState>>,
    world_season: Option<Res<WorldSeasonState>>,
    terrain_providers: Option<Res<TerrainProviders>>,
    chunk_layers: Query<&ChunkLayer, Without<Despawned>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut commands: Commands,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);
    // plan-mundane-fauna-v1 P2 — 季节权重：缺资源时退回 `Season::default()`
    // （`Season::Summer`，同 `WorldSeasonState::default()` 的隐含基线）。
    let season = world_season
        .as_deref()
        .map(|s| s.current.season)
        .unwrap_or_default();

    // 粗节流：与 heiwushi 一致，避免每 tick 都做全量 zone/query 扫描。
    // `now == 0` 时必须放行（对齐 `should_run_interval` 的 tick==0 分支）——否则
    // `state.last_check_tick` 默认值也是 0，`0.saturating_sub(0) == 0 < STRIDE` 会让世界
    // 起服后第一次巡检被粗节流吞掉，永远进不到任何真实判定分支。
    if now != 0 && now.saturating_sub(state.last_check_tick) < AMBIENT_SCHEDULER_STRIDE_TICKS {
        return;
    }
    state.last_check_tick = now;

    let mut zone_registry = zone_registry;
    let Some(registry) = zone_registry.as_deref_mut() else {
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
    for (entity, _marker, pos, rat_blackboard, spider_blackboard, life_record, cultivation) in
        &mut alive
    {
        let nearest = overworld_players
            .iter()
            .map(|p| planar_distance(pos.get(), *p))
            .fold(f64::INFINITY, f64::min);
        if should_recycle_ambient(nearest) {
            // §Verify blocker②（守恒蒸发红线）——鼠患 reserve 必须先完成 durable
            // 结算，实体才可进入 Despawned。缺 zone 时全额进入固定 stable overflow；缺
            // ledger/identity 或事务失败时保留实体，下一轮重试，不能先删 owner 的载体。
            let mut recycle_ready = true;
            if rat_blackboard.is_some() {
                recycle_ready = match (qi_account.as_deref_mut(), life_record, cultivation) {
                    (Some(account), Some(life_record), Some(mut cultivation)) => {
                        let zone_name = registry
                            .find_zone(DimensionKind::Overworld, pos.get())
                            .map(|zone| zone.name.clone());
                        let zone = zone_name
                            .as_deref()
                            .and_then(|zone_name| registry.find_zone_mut(zone_name));
                        match settle_rat_recycle(&mut cultivation, life_record, zone, account) {
                            Ok(()) => true,
                            Err(error) => {
                                tracing::debug!(
                                    "[bong][npc] ambient recycle rat qi settlement failed for \
                                     {:?}: {:?}",
                                    entity,
                                    error
                                );
                                false
                            }
                        }
                    }
                    (None, _, _) => {
                        tracing::debug!(
                            "[bong][npc] ambient recycle qi ledger unavailable; preserving rat \
                             owner {:?}",
                            entity
                        );
                        false
                    }
                    (_, None, _) | (_, _, None) => {
                        tracing::debug!(
                            "[bong][npc] ambient recycle rat owner identity unavailable; \
                             preserving {:?}",
                            entity
                        );
                        false
                    }
                };
            } else if spider_blackboard.is_some() {
                recycle_ready = match (qi_account.as_deref_mut(), life_record, cultivation) {
                    (Some(account), Some(life_record), Some(mut cultivation)) => {
                        let zone_name = registry
                            .find_zone(DimensionKind::Overworld, pos.get())
                            .map(|zone| zone.name.clone());
                        let zone = zone_name
                            .as_deref()
                            .and_then(|zone_name| registry.find_zone_mut(zone_name));
                        match settle_spider_recycle(&mut cultivation, life_record, zone, account) {
                            Ok(()) => true,
                            Err(error) => {
                                tracing::debug!(
                                    "[bong][npc] ambient recycle spider qi settlement failed for \
                                     {:?}: {:?}",
                                    entity,
                                    error
                                );
                                false
                            }
                        }
                    }
                    (None, _, _) => {
                        tracing::debug!(
                            "[bong][npc] ambient recycle qi ledger unavailable; preserving spider \
                             owner {:?}",
                            entity
                        );
                        false
                    }
                    (_, None, _) | (_, _, None) => {
                        tracing::debug!(
                            "[bong][npc] ambient recycle spider owner identity unavailable; \
                             preserving {:?}",
                            entity
                        );
                        false
                    }
                };
            }
            if recycle_ready {
                commands.entity(entity).insert(Despawned);
            }
        }
    }

    if overworld_players.is_empty() {
        // 无玩家在场：不做任何刷新判定（对齐 heiwushi 步骤 5 的玩家在场门）。
        return;
    }

    // `Commands::spawn` is deferred, so pending count—not same-tick spatial
    // occupancy—preserves the existing per-zone budget until the next tick.
    let mut pending_spawns_by_zone: HashMap<String, u32> = HashMap::new();

    // 2) 逐玩家找 zone，判定预算 + 间隔 + era 密度门，命中就在距该玩家 24~64 格环带内刷新。
    for player_pos in &overworld_players {
        let Some(zone) = registry.find_zone(DimensionKind::Overworld, *player_pos) else {
            continue;
        };

        let alive_in_zone: Vec<DVec3> = alive
            .iter()
            .filter(|(_, marker, _, _, _, _, _)| marker.home_zone() == zone.name)
            .map(|(_, _, pos, _, _, _, _)| pos.get())
            .collect();
        let pending_in_zone = pending_spawns_by_zone.get(&zone.name).copied().unwrap_or(0);
        let alive_count = alive_in_zone.len() as u32 + pending_in_zone;

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
        // `budget.pack_size_range`（多只群体刷新）不在本 plan 范围内消费，留给后续若立项；
        // baseline（origin/main）同样从未消费该字段，本 plan 只改地表落点门禁与提交边界，
        // 不改刷新数量语义。
        let _ = budget.pack_size_range;

        let Some(spawn_pos) =
            sample_ambient_ring_position(zone.bounds, *player_pos, &alive_in_zone, spawn_seed)
        else {
            continue;
        };
        // Raster 只能替代有效 live layer 内未加载 chunk 的 surface 数据。把 live-layer
        // 门禁限定在候选提交边界：stale、non-ChunkLayer 或 Despawned target 禁止新 spawn，
        // 但不能截断本轮前面已经执行的既有 ambient 回收与 qi 归还。
        let Ok(overworld_chunk_layer) = chunk_layers.get(layers.overworld) else {
            continue;
        };
        let spawned = submit_ambient_spawn_candidate::<M, _>(
            &mut commands,
            Some(overworld_chunk_layer),
            terrain_providers
                .as_deref()
                .map(|providers| &providers.overworld),
            config.pool_fn,
            &mut pending_spawns_by_zone,
            AmbientSpawnRequest {
                layer: layers.overworld,
                zone,
                candidate: spawn_pos,
                season,
                now,
            },
        );
        if spawned.is_none() {
            // runtime 标准窗口与 raster fallback 都无法给出安全脚点、或 pool 拒绝时，
            // 只丢弃本次候选；helper 保证失败分支不挂 marker、不占 pending，scheduler
            // 下一轮自然重试。
            continue;
        }
    }
}

#[cfg(test)]
#[path = "ambient_scheduler_tests.rs"]
mod tests;
