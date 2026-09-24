//! NPC LOD（plan-npc-ai-v1 §7 Phase 9 / plan-offscreen-war-v1 §10 P7）。
//!
//! 按最近玩家距离把 NPC 分**四**档（Near < Mid < Far < Dormant）：
//! - **Near**（默认 0..=80 格）：每 tick 正常跑 scorer / action
//! - **Mid**（80..=256，"Drowsy"）：每 `mid_skip_interval` tick 才跑一次（默认 4）；
//!   仍是 hydrated live ECS entity，不触发 dehydrate；qi 账户在 ECS CultivationState
//!   上，LOD 切换**绝不丢/造真元**
//! - **Far**（256..=512）：每 `far_skip_interval` tick 才跑一次（默认 10）
//! - **Dormant**（>512）：scorer 阶段直接置 0，停止新行为决策；lifespan
//!   继续 tick，方便老化/寿命清理
//!
//! **LOD ↔ hydrate 正交性**：`NpcLodTier` 是纯可见性/降频组件，不驱动
//! `dehydrate_far_npcs_system` / `hydrate_dormant_near_players_system`；
//! 进出 `NpcDormantStore` 由距离阈值决定，与 LOD tier 无关。
//! 因此 Near↔Mid↔Far 之间切换**无任何 qi 账本操作**（E1–E4 守恒边）。
//!
//! 真正"卸载到 agent 代管"（plan §7 Phase 9 第 2 项）需要跨进程协作，属
//! 后续 PR 范围；本 commit 只提供 ECS 层降频 infra。

#![allow(dead_code)]

use std::collections::HashMap;

use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, Component, DVec3, Despawned, Entity, IntoSystemConfigs, Position, PreUpdate,
    Query, Res, ResMut, Resource, Update, With, Without,
};

use crate::cultivation::components::{Cultivation, MeridianSystem};
use crate::npc::dormant::{current_tick, should_run_interval};
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::{
    constants::QI_ZONE_UNIT_CAPACITY, regen_from_zone, QiAccountId, QiTransfer, QiTransferReason,
    WorldQiAccount,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

/// 四档 LOD（Near < Mid < Far < Dormant）。
///
/// GuardianRelic 强制 Near（考验需要实时响应）。
///
/// **守恒红线**：Near↔Mid↔Far↔Dormant(ECS) 之间切换只改此 Component；
/// qi 账户（`CultivationState.qi_current`）不受任何 LOD tier 切换影响。
/// hydrate ↔ dehydrate（进出 `NpcDormantStore`）由独立距离阈值驱动，
/// 与 `NpcLodTier` 完全正交。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Component)]
pub enum NpcLodTier {
    #[default]
    Near,
    /// Drowsy（中间态）：hydrated live entity，降频到 `mid_skip_interval`。
    /// 不触发 dehydrate，不做任何 qi 账本操作。
    Mid,
    Far,
    Dormant,
}

/// 6 条有向转换边（供文档/单测参考）。
///
/// - **E1** Near → Mid：只改 tier，无 qi 操作
/// - **E2** Mid → Near：只改 tier，无 qi 操作
/// - **E3** Mid → Far：只改 tier，无 qi 操作
/// - **E4** Far → Mid：只改 tier，无 qi 操作
/// - **E5** Live ECS → NpcDormantStore（dehydrate）：qi 随快照原值保存，无 ledger 操作
/// - **E6** NpcDormantStore → Live ECS（hydrate）：qi 随快照原值恢复，无 ledger 操作
///
/// E1–E4 完全由 `update_npc_lod_tier_system` 驱动（距离分档）。
/// E5/E6 由 `dehydrate_far_npcs_system` / `hydrate_dormant_near_players_system` 驱动
/// （距离阈值，与 LOD tier 无关）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodTransitionEdge {
    /// E1：Near → Mid（降频，qi 不变）
    NearToMid,
    /// E2：Mid → Near（升频，qi 不变）
    MidToNear,
    /// E3：Mid → Far（降频，qi 不变）
    MidToFar,
    /// E4：Far → Mid（升频，qi 不变）
    FarToMid,
    /// E5：Live ECS → NpcDormantStore（dehydrate，qi 随快照保存）
    LiveToDormantStore,
    /// E6：NpcDormantStore → Live ECS（hydrate，qi 随快照恢复）
    DormantStoreToLive,
}

impl LodTransitionEdge {
    /// 判断此转换边是否涉及 qi 账本操作。
    /// E1–E4 均为 `false`（LOD 纯频率门）。
    /// E5/E6 由 hydrate 模块负责，账本本身也不做 ledger 操作——qi 原值拷入/拷出快照。
    pub fn has_qi_ledger_operation(self) -> bool {
        false // 所有 6 条边均不触发 ledger.transfer；守恒红线
    }

    /// 给定 from/to tier，若属于 E1–E4 返回对应边，否则 None。
    pub fn classify(from: NpcLodTier, to: NpcLodTier) -> Option<Self> {
        match (from, to) {
            (NpcLodTier::Near, NpcLodTier::Mid) => Some(Self::NearToMid),
            (NpcLodTier::Mid, NpcLodTier::Near) => Some(Self::MidToNear),
            (NpcLodTier::Mid, NpcLodTier::Far) => Some(Self::MidToFar),
            (NpcLodTier::Far, NpcLodTier::Mid) => Some(Self::FarToMid),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScorerKind {
    /// 必须保持实时响应的评分，例如会影响玩家近身安全的强制逻辑。
    Critical,
    /// 常规评分：Near 每 tick，Mid/Far 按各自 interval 降频，Dormant 跳过。
    #[default]
    Standard,
    /// 远处可延迟的评分：仅 Near 计算，Mid / Far / Dormant 跳过。
    Cosmetic,
}

#[derive(Clone, Copy, Debug, Resource)]
pub struct NpcLodConfig {
    pub near_radius: f64,
    /// Mid（Drowsy）带半径上界；默认 256.0（与 dehydrate_radius_blocks 对齐）。
    pub mid_radius: f64,
    pub far_radius: f64,
    /// Mid 档降频间隔，默认 4（介于 Near=1 和 Far=10 之间）。
    pub mid_skip_interval: u32,
    pub far_skip_interval: u32,
    #[allow(dead_code)]
    pub dormant_skip_interval: u32,
    /// 每 N tick 重新评估一次 tier（避免每 tick O(npc × player)）。
    pub reassess_interval: u32,
    /// `BONG_NPC_NO_DORMANT=1` → 所有 NPC 最低降到 Far，不进 Dormant。
    pub no_dormant: bool,
}

impl Default for NpcLodConfig {
    fn default() -> Self {
        Self {
            near_radius: 80.0,
            mid_radius: 256.0,
            far_radius: 512.0,
            mid_skip_interval: 4,
            far_skip_interval: 10,
            dormant_skip_interval: 60,
            reassess_interval: 20,
            no_dormant: std::env::var("BONG_NPC_NO_DORMANT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        }
    }
}

/// LOD scheduler tick。仅本模块和 `should_skip_scorer_tick` 用。
#[derive(Clone, Copy, Debug, Default, Resource)]
pub struct NpcLodTick(pub u32);

/// Drowsy（Mid tier）降频 qi regen 的每 NPC 状态组件。
///
/// 记录上次 drowsy regen tick 的时间戳（与 `GameTick` 对齐）；
/// `drowsy_tick_system` 每 `DROWSY_TICK_INTERVAL` tick 对 Mid 档 NPC 做一次
/// zone→NPC qi regen（走双录簿路径，与 `apply_dormant_regen_with_multiplier` 语义一致）。
///
/// **守恒保证**：`NpcDrowsyState` 不持有任何 qi 账本——regen 走
/// `ledger.set_balance` 对齐 + `ledger.transfer` 搬运，NPC 的 `Cultivation.qi_current`
/// 在 ECS 上与 ECS 账本对齐更新；LOD tier 切换（E1–E4）**不触碰**此组件内的 qi 字段，
/// 更不触碰 `ledger`——守恒红线完全隔离。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct NpcDrowsyState {
    /// 上次 drowsy regen 成功执行的 game tick（u64，与 `GameTick` 对齐）。
    /// `0` = 尚未执行过。
    pub last_drowsy_tick: u64,
}

/// Drowsy regen 间隔：20 tick = 1 秒（20TPS 下 1Hz）。
///
/// 与 `DORMANT_LIFECYCLE_TICK_INTERVAL`（1200 tick = 60s）区分——Drowsy 态 NPC 仍是
/// hydrated live entity，regen 频率远高于 dormant batch（每秒 vs 每分钟）。
pub const DROWSY_TICK_INTERVAL: u32 = 20;

pub fn register(app: &mut App) {
    // LOD gate：接入 brain.rs 3 个核心 scorer（player_proximity / hunger / wander）
    // 的 Dormant skip。seed 100 rogue 在 test area 无玩家连接时全部分类为 Dormant，
    // scorer early return → CI e2e 无玩家路径上 TPS 不塌。
    //
    // ccfbb458 曾把这一套 add_systems 和 brain.rs gate 整体撤回，误诊为 TPS 回归
    // 源；真正根因是 `seed_initial_rogue_population_on_startup` 默认 target=100
    // 让 brain.rs 20+ scorer × 100 actor 在 CI 单核上跑不动。LOD gate 是正解。
    let lod_config = NpcLodConfig::default();
    if lod_config.no_dormant {
        tracing::warn!(
            "[bong][npc] BONG_NPC_NO_DORMANT=1 — dormant tier disabled, all NPCs stay >= Far"
        );
    }
    app.insert_resource(lod_config)
        .insert_resource(NpcLodTick::default())
        .add_systems(
            PreUpdate,
            (tick_lod_counter, update_npc_lod_tier_system)
                .chain()
                .before(big_brain::prelude::BigBrainSet::Scorers),
        )
        // drowsy_tick_system：在 Update 阶段对 Mid 档 live NPC 做 1Hz zone→NPC qi regen。
        // 独立于 PreUpdate 的 LOD tier 分类系统；不接触 dehydrate/hydrate 路径。
        .add_systems(Update, drowsy_tick_system);
}

fn tick_lod_counter(mut counter: ResMut<NpcLodTick>) {
    counter.0 = counter.0.wrapping_add(1);
}

type NpcLodQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, Option<&'static NpcLodTier>),
    (With<NpcMarker>, Without<Despawned>),
>;

type PlayerPosQuery<'w, 's> = Query<'w, 's, &'static Position, With<ClientMarker>>;

#[allow(clippy::type_complexity)]
fn update_npc_lod_tier_system(
    mut commands: valence::prelude::Commands<'_, '_>,
    counter: Res<NpcLodTick>,
    config: Res<NpcLodConfig>,
    npcs: NpcLodQuery<'_, '_>,
    players: PlayerPosQuery<'_, '_>,
) {
    let should_reassess_existing =
        counter.0 == 1 || counter.0.is_multiple_of(config.reassess_interval.max(1));
    let player_positions: Vec<DVec3> = players.iter().map(|p| p.get()).collect();
    let mut transitions = [0u32; 4]; // near, mid, far, dormant
    for (entity, pos, current) in &npcs {
        if current.is_some() && !should_reassess_existing {
            continue;
        }

        let desired = classify_tier(pos.get(), &player_positions, &config);
        match (current.copied(), desired) {
            (Some(c), d) if c == d => {}
            _ => {
                let mut ec = commands.entity(entity);
                ec.insert(desired);
                // Mid 档（Drowsy）需要 NpcDrowsyState 组件供 drowsy_tick_system 处理。
                // Bevy `insert` 是**覆盖**语义（非幂等保留）。但此分支仅在 tier 发生变化时
                // 执行（同档 same-tier 走上方 `(Some(c), d) if c == d => {}` short-circuit 跳过），
                // 所以 NPC 稳定停在 Mid 时不会反复执行此插入，`last_drowsy_tick` 不会被意外重置。
                // Mid 重入时（从 Near/Far 进入 Mid）`default()` 重置是正确行为——开启新一轮 drowsy 时钟。
                if desired == NpcLodTier::Mid {
                    ec.insert(NpcDrowsyState::default());
                }
                match desired {
                    NpcLodTier::Near => transitions[0] += 1,
                    NpcLodTier::Mid => transitions[1] += 1,
                    NpcLodTier::Far => transitions[2] += 1,
                    NpcLodTier::Dormant => transitions[3] += 1,
                }
            }
        }
    }
}

/// Drowsy（Mid tier）降频 qi regen 系统。
///
/// 每 `DROWSY_TICK_INTERVAL`（20 tick = 1Hz）对所有 `NpcLodTier::Mid` 的 live ECS NPC
/// 执行一次 zone→NPC qi regen，走与 `apply_dormant_regen_with_multiplier` 相同的双录簿路径：
/// 1. `ledger.set_balance` 把 zone / npc 账户对齐到当前状态量
/// 2. 计算 `regen_from_zone(zone.spirit_qi, rate, integrity, room)` → (gain, drain)
/// 3. `ledger.transfer(zone_account → npc_account, CultivationRegen)` 真实搬运
/// 4. 更新 `cultivation.qi_current` = ledger 余额；`zone.spirit_qi -= drain`
///
/// **守恒红线保证**：
/// - LOD tier 切换（E1–E4）**不触发**此函数——只有 `NpcLodTier::Mid` 才跑 regen
/// - 本系统完全不影响 hydrate / dehydrate 路径（E5/E6 由独立距离阈值驱动）
/// - regen 每次均走 `set_balance` + `transfer`，ledger 总量严格守恒（无凭空增减）
/// - 若 zone 无灵气 / NPC 真元已满 / 无经脉，跳过（不做零 transfer）
#[allow(clippy::type_complexity)]
pub fn drowsy_tick_system(
    game_tick: Option<Res<GameTick>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    mut npcs: Query<
        (
            Entity,
            &Position,
            Option<&CurrentDimension>,
            &NpcLodTier,
            &mut Cultivation,
            &MeridianSystem,
            &mut NpcDrowsyState,
        ),
        (With<NpcMarker>, Without<Despawned>),
    >,
) {
    let tick = current_tick(game_tick.as_deref());
    if !should_run_interval(tick, DROWSY_TICK_INTERVAL) {
        return;
    }
    let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) else {
        return;
    };
    for (entity, pos, dim, tier, mut cultivation, meridian_system, mut drowsy_state) in &mut npcs {
        if *tier != NpcLodTier::Mid {
            continue;
        }
        // 避免同 tick 重复处理（理论上 should_run_interval 已防，防御性保留）。
        if drowsy_state.last_drowsy_tick == tick && tick > 0 {
            continue;
        }
        drowsy_state.last_drowsy_tick = tick;

        let dim_kind = dim.map(|d| d.0).unwrap_or(DimensionKind::Overworld);
        // find_zone（不可变）取 zone 名称，再 find_zone_mut 获取可变引用。
        let Some(zone_name) = zones.find_zone(dim_kind, pos.get()).map(|z| z.name.clone()) else {
            continue;
        };
        let Some(zone) = zones.find_zone_mut(&zone_name) else {
            continue;
        };
        if zone.spirit_qi <= 0.0 {
            continue;
        }
        let rate = meridian_system.sum_rate();
        if rate <= 0.0 {
            continue;
        }
        let integrity_count = meridian_system.iter().count() as f64;
        let avg_integrity = if integrity_count > 0.0 {
            meridian_system.iter().map(|m| m.integrity).sum::<f64>() / integrity_count
        } else {
            1.0
        };
        let room = (cultivation.qi_max - cultivation.qi_current).max(0.0);
        let (gain, drain) = regen_from_zone(zone.spirit_qi, rate, avg_integrity, room);
        if gain <= 0.0 || drain <= 0.0 {
            continue;
        }
        // 双录簿：先 set_balance 对齐，再 transfer 真实搬运。
        // NPC 账户用 entity ID 稳定标识（live ECS entity 无 CharId 组件，用 entity index）。
        let zone_account = QiAccountId::zone(zone.name.clone());
        let npc_char_id = format!("drowsy_live:{}", entity.index());
        let npc_account = QiAccountId::npc(npc_char_id);
        if ledger
            .set_balance(
                zone_account.clone(),
                zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY,
            )
            .is_err()
        {
            continue;
        }
        if ledger
            .set_balance(npc_account.clone(), cultivation.qi_current.max(0.0))
            .is_err()
        {
            continue;
        }
        let Ok(transfer) = QiTransfer::new(
            zone_account,
            npc_account.clone(),
            gain,
            QiTransferReason::CultivationRegen,
        ) else {
            continue;
        };
        if ledger.transfer(transfer).is_ok() {
            cultivation.qi_current = ledger.balance(&npc_account);
            zone.spirit_qi = (zone.spirit_qi - drain).max(0.0);
        }
    }
}

#[allow(clippy::if_same_then_else)]
pub fn classify_tier(npc_pos: DVec3, players: &[DVec3], config: &NpcLodConfig) -> NpcLodTier {
    if players.is_empty() {
        return if config.no_dormant {
            NpcLodTier::Far
        } else {
            NpcLodTier::Dormant
        };
    }
    let min_d = players
        .iter()
        .map(|p| {
            let dx = p.x - npc_pos.x;
            let dz = p.z - npc_pos.z;
            (dx * dx + dz * dz).sqrt()
        })
        .fold(f64::INFINITY, f64::min);
    if min_d <= config.near_radius {
        NpcLodTier::Near
    } else if min_d <= config.mid_radius {
        NpcLodTier::Mid
    } else if min_d <= config.far_radius {
        NpcLodTier::Far
    } else if config.no_dormant {
        NpcLodTier::Far
    } else {
        NpcLodTier::Dormant
    }
}

/// Scorer 系统用：给定当前 tick + entity 的 tier，返回 true 表示**本 tick
/// 应跳过**（分数保持不变，或在想强制 0 的语境下置 0）。
/// - Near 永远 false（不跳过）
/// - Mid 每 `mid_skip_interval` tick 才算"非跳过"
/// - Far 每 `far_skip_interval` tick 才算"非跳过"
/// - Dormant 总是 true
pub fn should_skip_scorer_tick(tier: NpcLodTier, tick: u32, config: &NpcLodConfig) -> bool {
    should_skip_scorer_tick_for(tier, ScorerKind::Standard, tick, config)
}

pub fn should_skip_scorer_tick_for(
    tier: NpcLodTier,
    scorer_kind: ScorerKind,
    tick: u32,
    config: &NpcLodConfig,
) -> bool {
    if matches!(scorer_kind, ScorerKind::Critical) {
        return false;
    }

    match tier {
        NpcLodTier::Near => false,
        NpcLodTier::Mid => {
            // Cosmetic 在 Mid 也跳过（同 Far 语义）。
            // Standard/Critical：按 mid_skip_interval 降频。
            matches!(scorer_kind, ScorerKind::Cosmetic)
                || !tick.is_multiple_of(config.mid_skip_interval.max(1))
        }
        NpcLodTier::Far => {
            matches!(scorer_kind, ScorerKind::Cosmetic)
                || !tick.is_multiple_of(config.far_skip_interval.max(1))
        }
        NpcLodTier::Dormant => true,
    }
}

pub fn lod_gated_score(
    tier: Option<&NpcLodTier>,
    tick: u32,
    config: &NpcLodConfig,
    compute: impl FnOnce() -> f32,
) -> Option<f32> {
    lod_gated_score_by_kind(tier, tick, config, ScorerKind::Standard, compute)
}

pub fn lod_gated_score_by_kind(
    tier: Option<&NpcLodTier>,
    tick: u32,
    config: &NpcLodConfig,
    scorer_kind: ScorerKind,
    compute: impl FnOnce() -> f32,
) -> Option<f32> {
    if is_dormant(tier) {
        Some(0.0)
    } else if tier
        .copied()
        .map(|tier| should_skip_scorer_tick_for(tier, scorer_kind, tick, config))
        .unwrap_or(false)
    {
        None
    } else {
        Some(compute())
    }
}

/// Dormant 判断的便捷版（不需要 config）。
pub fn is_dormant(tier: Option<&NpcLodTier>) -> bool {
    matches!(tier, Some(NpcLodTier::Dormant))
}

/// Mid（Drowsy）判断的便捷版。
pub fn is_mid(tier: Option<&NpcLodTier>) -> bool {
    matches!(tier, Some(NpcLodTier::Mid))
}

/// 与 scorer 系统配合：给 `Actor(npc)` 上挂的 scorer 查 actor 的 LOD tier。
/// 供 brain.rs / territory.rs 等共享使用的极简 helper。
#[allow(dead_code)]
pub fn actor_lod_tier<'a>(
    npc_tiers: &'a Query<'_, '_, &'a NpcLodTier, With<NpcMarker>>,
    actor: Entity,
) -> Option<NpcLodTier> {
    npc_tiers.get(actor).ok().copied()
}

/// 统计每个 tier 的 NPC 数量（debug / 监控用）。
#[allow(dead_code)]
pub fn count_by_tier(
    npcs: &Query<Option<&NpcLodTier>, With<NpcMarker>>,
) -> HashMap<NpcLodTier, usize> {
    let mut counts = HashMap::new();
    for tier_opt in npcs.iter() {
        let t = tier_opt.copied().unwrap_or(NpcLodTier::Near);
        *counts.entry(t).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
#[path = "lod_tests.rs"]
mod tests;
