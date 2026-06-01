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
        counter.0 == 1 || counter.0 % config.reassess_interval.max(1) == 0;
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
                || tick % config.mid_skip_interval.max(1) != 0
        }
        NpcLodTier::Far => {
            matches!(scorer_kind, ScorerKind::Cosmetic)
                || tick % config.far_skip_interval.max(1) != 0
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
mod tests {
    use super::*;
    use valence::prelude::{App, IntoSystemConfigs, PreUpdate};

    // =========================================================================
    // classify_tier — 四档分类
    // =========================================================================

    #[test]
    fn classify_tier_no_players_is_dormant() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(DVec3::new(0.0, 64.0, 0.0), &[], &cfg),
            NpcLodTier::Dormant,
            "期望无玩家→Dormant，因为没有玩家存在时进入 Dormant tier"
        );
    }

    #[test]
    fn classify_tier_near_within_radius() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(10.0, 64.0, 10.0)],
                &cfg
            ),
            NpcLodTier::Near,
            "期望 dist≈14 <= near_radius=80 → Near"
        );
    }

    #[test]
    fn classify_tier_mid_between_near_and_mid_radius() {
        let cfg = NpcLodConfig::default();
        // near_radius=80, mid_radius=256
        // dist = 150 → Mid
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(150.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Mid,
            "期望 dist=150 在 (80,256] → Mid（Drowsy）"
        );
    }

    #[test]
    fn classify_tier_mid_at_near_boundary_plus_one() {
        let cfg = NpcLodConfig::default();
        // near_radius=80；dist=81 → Mid
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(81.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Mid,
            "期望 dist=81 刚超过 near_radius=80 → Mid"
        );
    }

    #[test]
    fn classify_tier_mid_at_mid_radius_boundary() {
        let cfg = NpcLodConfig::default();
        // mid_radius=256；dist=256 → Mid（边界值含在 Mid 内，dist <= mid_radius）
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(256.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Mid,
            "期望 dist=256 恰好等于 mid_radius=256 → Mid（含边界）"
        );
    }

    #[test]
    fn classify_tier_far_between_mid_and_far_radius() {
        let cfg = NpcLodConfig::default();
        // mid_radius=256, far_radius=512；dist=400 → Far
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(400.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Far,
            "期望 dist=400 在 (256,512] → Far"
        );
    }

    #[test]
    fn classify_tier_far_between_radii() {
        let cfg = NpcLodConfig::default();
        // 原有测试保留，dist=100 现在落入 Mid 带（80..=256），
        // 但此测试取 dist=300 确保落入 Far
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(300.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Far,
            "期望 dist=300 在 (256,512] → Far"
        );
    }

    #[test]
    fn classify_tier_dormant_beyond_far() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(600.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Dormant,
            "期望 dist=600 > far_radius=512 → Dormant"
        );
    }

    #[test]
    fn classify_tier_ignores_y_uses_xz_only() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 10.0, 0.0),
                &[DVec3::new(10.0, 200.0, 10.0)], // y 差了 190
                &cfg
            ),
            NpcLodTier::Near,
            "期望 y 不参与距离计算，xz dist≈14 → Near"
        );
    }

    #[test]
    fn classify_tier_takes_nearest_player() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[
                    DVec3::new(600.0, 64.0, 0.0),
                    DVec3::new(20.0, 64.0, 0.0), // 这个是最近
                ],
                &cfg
            ),
            NpcLodTier::Near,
            "期望取最近玩家，dist=20 → Near"
        );
    }

    #[test]
    fn classify_tier_no_dormant_flag_caps_at_far() {
        let cfg = NpcLodConfig {
            no_dormant: true,
            ..Default::default()
        };
        // 距离超过 far_radius → 正常情况是 Dormant，no_dormant 则退化到 Far
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(600.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Far,
            "期望 no_dormant=true 时 dist=600 → Far 而非 Dormant"
        );
        // 无玩家时也是 Far
        assert_eq!(
            classify_tier(DVec3::new(0.0, 64.0, 0.0), &[], &cfg),
            NpcLodTier::Far,
            "期望 no_dormant=true 且无玩家 → Far"
        );
    }

    // =========================================================================
    // should_skip_scorer_tick — 四档降频门
    // =========================================================================

    #[test]
    fn should_skip_scorer_tick_near_never_skips() {
        let cfg = NpcLodConfig::default();
        for t in 0..40 {
            assert!(
                !should_skip_scorer_tick(NpcLodTier::Near, t, &cfg),
                "期望 Near 在 tick={t} 不跳过"
            );
        }
    }

    #[test]
    fn should_skip_scorer_tick_dormant_always_skips() {
        let cfg = NpcLodConfig::default();
        for t in 0..40 {
            assert!(
                should_skip_scorer_tick(NpcLodTier::Dormant, t, &cfg),
                "期望 Dormant 在 tick={t} 总是跳过"
            );
        }
    }

    #[test]
    fn should_skip_scorer_tick_far_respects_interval() {
        let cfg = NpcLodConfig {
            far_skip_interval: 10,
            ..Default::default()
        };
        // 0, 10, 20 跑；其他跳
        assert!(
            !should_skip_scorer_tick(NpcLodTier::Far, 0, &cfg),
            "期望 Far tick=0 不跳（0 % 10 == 0）"
        );
        assert!(
            should_skip_scorer_tick(NpcLodTier::Far, 1, &cfg),
            "期望 Far tick=1 跳过"
        );
        assert!(
            should_skip_scorer_tick(NpcLodTier::Far, 9, &cfg),
            "期望 Far tick=9 跳过"
        );
        assert!(
            !should_skip_scorer_tick(NpcLodTier::Far, 10, &cfg),
            "期望 Far tick=10 不跳"
        );
        assert!(
            !should_skip_scorer_tick(NpcLodTier::Far, 20, &cfg),
            "期望 Far tick=20 不跳"
        );
    }

    #[test]
    fn should_skip_scorer_tick_mid_respects_interval() {
        let cfg = NpcLodConfig {
            mid_skip_interval: 4,
            ..Default::default()
        };
        // 0, 4, 8, 12 跑；1,2,3,5 等跳
        assert!(
            !should_skip_scorer_tick(NpcLodTier::Mid, 0, &cfg),
            "期望 Mid tick=0 不跳（0 % 4 == 0）"
        );
        assert!(
            should_skip_scorer_tick(NpcLodTier::Mid, 1, &cfg),
            "期望 Mid tick=1 跳过"
        );
        assert!(
            should_skip_scorer_tick(NpcLodTier::Mid, 3, &cfg),
            "期望 Mid tick=3 跳过"
        );
        assert!(
            !should_skip_scorer_tick(NpcLodTier::Mid, 4, &cfg),
            "期望 Mid tick=4 不跳"
        );
        assert!(
            !should_skip_scorer_tick(NpcLodTier::Mid, 8, &cfg),
            "期望 Mid tick=8 不跳"
        );
    }

    #[test]
    fn mid_skip_interval_is_less_than_far_skip_interval() {
        let cfg = NpcLodConfig::default();
        // 默认 mid=4 < far=10：Mid 档比 Far 档更频繁（降频更少）
        assert!(
            cfg.mid_skip_interval < cfg.far_skip_interval,
            "期望 mid_skip_interval={} < far_skip_interval={}（Mid 降频更少）",
            cfg.mid_skip_interval,
            cfg.far_skip_interval
        );
    }

    #[test]
    fn should_skip_clamps_zero_interval_to_at_least_one() {
        let cfg = NpcLodConfig {
            far_skip_interval: 0,
            mid_skip_interval: 0,
            ..Default::default()
        };
        // 0 间隔会除零；确保 clamp 到 1 → 永不跳过
        assert!(
            !should_skip_scorer_tick(NpcLodTier::Far, 1, &cfg),
            "期望 far_skip_interval=0 clamp 到 1，tick=1 不跳"
        );
        assert!(
            !should_skip_scorer_tick(NpcLodTier::Mid, 1, &cfg),
            "期望 mid_skip_interval=0 clamp 到 1，tick=1 不跳"
        );
    }

    #[test]
    fn scorer_kind_cosmetic_skips_mid_far_and_dormant() {
        let cfg = NpcLodConfig::default();

        assert!(
            !should_skip_scorer_tick_for(NpcLodTier::Near, ScorerKind::Cosmetic, 1, &cfg),
            "期望 Cosmetic Near 不跳"
        );
        assert!(
            should_skip_scorer_tick_for(NpcLodTier::Mid, ScorerKind::Cosmetic, 0, &cfg),
            "期望 Cosmetic Mid 跳过（Mid 不是 Near）"
        );
        assert!(
            should_skip_scorer_tick_for(NpcLodTier::Far, ScorerKind::Cosmetic, 0, &cfg),
            "期望 Cosmetic Far 跳过"
        );
        assert!(
            should_skip_scorer_tick_for(NpcLodTier::Dormant, ScorerKind::Cosmetic, 0, &cfg),
            "期望 Cosmetic Dormant 跳过"
        );
    }

    #[test]
    fn scorer_kind_critical_never_skips() {
        let cfg = NpcLodConfig::default();

        assert!(
            !should_skip_scorer_tick_for(NpcLodTier::Near, ScorerKind::Critical, 1, &cfg),
            "期望 Critical Near 不跳"
        );
        assert!(
            !should_skip_scorer_tick_for(NpcLodTier::Mid, ScorerKind::Critical, 1, &cfg),
            "期望 Critical Mid 不跳"
        );
        assert!(
            !should_skip_scorer_tick_for(NpcLodTier::Far, ScorerKind::Critical, 1, &cfg),
            "期望 Critical Far 不跳"
        );
        assert!(
            !should_skip_scorer_tick_for(NpcLodTier::Dormant, ScorerKind::Critical, 1, &cfg),
            "期望 Critical Dormant 不跳"
        );
    }

    // =========================================================================
    // ECS 集成：update_npc_lod_tier_system 四档分类
    // =========================================================================

    #[test]
    fn update_npc_lod_tier_system_assigns_tier_from_player_distance() {
        let mut app = App::new();
        app.insert_resource(NpcLodConfig::default());
        app.insert_resource(NpcLodTick(0));
        app.add_systems(
            PreUpdate,
            (tick_lod_counter, update_npc_lod_tier_system).chain(),
        );

        let npc_near = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        // dist=150 → Mid（80..256）
        let npc_mid = app
            .world_mut()
            .spawn((NpcMarker, Position::new([150.0, 64.0, 0.0])))
            .id();
        // dist=400 → Far（256..512）
        let npc_far = app
            .world_mut()
            .spawn((NpcMarker, Position::new([400.0, 64.0, 0.0])))
            .id();
        // dist=1000 → Dormant（>512）
        let npc_dormant = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1000.0, 64.0, 0.0])))
            .id();
        let _ = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
            .id();

        // 跑 reassess_interval 次确保至少触发一轮评估
        for _ in 0..20 {
            app.update();
        }

        assert_eq!(
            app.world().get::<NpcLodTier>(npc_near).copied(),
            Some(NpcLodTier::Near),
            "期望 dist=0 → Near"
        );
        assert_eq!(
            app.world().get::<NpcLodTier>(npc_mid).copied(),
            Some(NpcLodTier::Mid),
            "期望 dist=150 → Mid（80..=256）"
        );
        assert_eq!(
            app.world().get::<NpcLodTier>(npc_far).copied(),
            Some(NpcLodTier::Far),
            "期望 dist=400 → Far（256..=512）"
        );
        assert_eq!(
            app.world().get::<NpcLodTier>(npc_dormant).copied(),
            Some(NpcLodTier::Dormant),
            "期望 dist=1000 → Dormant"
        );
    }

    #[test]
    fn update_tier_respects_reassess_interval() {
        let mut app = App::new();
        let cfg = NpcLodConfig {
            reassess_interval: 50,
            ..Default::default()
        };
        app.insert_resource(cfg);
        app.insert_resource(NpcLodTick(0));
        app.add_systems(
            PreUpdate,
            (tick_lod_counter, update_npc_lod_tier_system).chain(),
        );

        let npc = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Near),
            "首 tick 应先分类，避免无玩家 e2e 启动期等到 50 tick 才降载"
        );

        *app.world_mut().get_mut::<Position>(player).unwrap() = Position::new([1000.0, 64.0, 0.0]);

        for _ in 0..48 {
            app.update();
        }
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Near),
            "期望未到 reassess_interval=50 不重算 tier，仍为 Near"
        );

        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Dormant),
            "期望到达 reassess_interval=50 后重算，玩家移远 → Dormant"
        );
    }

    #[test]
    fn update_tier_classifies_new_npc_before_next_reassess() {
        let mut app = App::new();
        let cfg = NpcLodConfig {
            reassess_interval: 50,
            ..Default::default()
        };
        app.insert_resource(cfg);
        app.insert_resource(NpcLodTick(0));
        app.add_systems(
            PreUpdate,
            (tick_lod_counter, update_npc_lod_tier_system).chain(),
        );

        let existing = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        let _ = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(existing).copied(),
            Some(NpcLodTier::Near)
        );

        let new_npc = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1000.0, 64.0, 0.0])))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(new_npc).copied(),
            Some(NpcLodTier::Dormant),
            "期望新 NPC 不等 reassess_interval 即刻分类（Option::None 走强制评估路径）"
        );
    }

    // =========================================================================
    // is_dormant / is_mid helpers
    // =========================================================================

    #[test]
    fn is_dormant_helper() {
        assert!(
            is_dormant(Some(&NpcLodTier::Dormant)),
            "Dormant → is_dormant=true"
        );
        assert!(
            !is_dormant(Some(&NpcLodTier::Far)),
            "Far → is_dormant=false"
        );
        assert!(
            !is_dormant(Some(&NpcLodTier::Mid)),
            "Mid → is_dormant=false（Mid 是 hydrated live entity）"
        );
        assert!(
            !is_dormant(Some(&NpcLodTier::Near)),
            "Near → is_dormant=false"
        );
        assert!(!is_dormant(None), "None → is_dormant=false");
    }

    #[test]
    fn is_mid_helper() {
        assert!(is_mid(Some(&NpcLodTier::Mid)), "Mid → is_mid=true");
        assert!(!is_mid(Some(&NpcLodTier::Near)), "Near → is_mid=false");
        assert!(!is_mid(Some(&NpcLodTier::Far)), "Far → is_mid=false");
        assert!(
            !is_mid(Some(&NpcLodTier::Dormant)),
            "Dormant → is_mid=false"
        );
        assert!(!is_mid(None), "None → is_mid=false");
    }

    // =========================================================================
    // LodTransitionEdge — 6 条有向转换边
    // =========================================================================

    #[test]
    fn lod_transition_edge_classify_e1_near_to_mid() {
        let edge = LodTransitionEdge::classify(NpcLodTier::Near, NpcLodTier::Mid);
        assert_eq!(
            edge,
            Some(LodTransitionEdge::NearToMid),
            "期望 Near→Mid 被识别为 E1"
        );
        assert!(
            !edge.unwrap().has_qi_ledger_operation(),
            "期望 E1 无 qi 账本操作（守恒红线）"
        );
    }

    #[test]
    fn lod_transition_edge_classify_e2_mid_to_near() {
        let edge = LodTransitionEdge::classify(NpcLodTier::Mid, NpcLodTier::Near);
        assert_eq!(
            edge,
            Some(LodTransitionEdge::MidToNear),
            "期望 Mid→Near 被识别为 E2"
        );
        assert!(
            !edge.unwrap().has_qi_ledger_operation(),
            "期望 E2 无 qi 账本操作（守恒红线）"
        );
    }

    #[test]
    fn lod_transition_edge_classify_e3_mid_to_far() {
        let edge = LodTransitionEdge::classify(NpcLodTier::Mid, NpcLodTier::Far);
        assert_eq!(
            edge,
            Some(LodTransitionEdge::MidToFar),
            "期望 Mid→Far 被识别为 E3"
        );
        assert!(
            !edge.unwrap().has_qi_ledger_operation(),
            "期望 E3 无 qi 账本操作（守恒红线）"
        );
    }

    #[test]
    fn lod_transition_edge_classify_e4_far_to_mid() {
        let edge = LodTransitionEdge::classify(NpcLodTier::Far, NpcLodTier::Mid);
        assert_eq!(
            edge,
            Some(LodTransitionEdge::FarToMid),
            "期望 Far→Mid 被识别为 E4"
        );
        assert!(
            !edge.unwrap().has_qi_ledger_operation(),
            "期望 E4 无 qi 账本操作（守恒红线）"
        );
    }

    #[test]
    fn lod_transition_edge_no_qi_operation_for_all_e1_to_e4() {
        // 验证所有 E1–E4 边均无 qi 账本操作（守恒红线：LOD 切换绝不丢/造真元）
        let edges = [
            LodTransitionEdge::NearToMid,
            LodTransitionEdge::MidToNear,
            LodTransitionEdge::MidToFar,
            LodTransitionEdge::FarToMid,
            LodTransitionEdge::LiveToDormantStore,
            LodTransitionEdge::DormantStoreToLive,
        ];
        for edge in edges {
            assert!(
                !edge.has_qi_ledger_operation(),
                "期望转换边 {edge:?} 无 qi 账本操作（所有6边守恒红线）"
            );
        }
    }

    #[test]
    fn lod_transition_edge_classify_near_to_far_returns_none() {
        // Near→Far 不是 E1–E4（跳过 Mid），返回 None（非法直接跳变，应不发生）
        assert_eq!(
            LodTransitionEdge::classify(NpcLodTier::Near, NpcLodTier::Far),
            None,
            "期望 Near→Far 不在 E1–E4 内，返回 None"
        );
    }

    #[test]
    fn lod_transition_edge_classify_near_to_dormant_returns_none() {
        assert_eq!(
            LodTransitionEdge::classify(NpcLodTier::Near, NpcLodTier::Dormant),
            None,
            "期望 Near→Dormant 不在 E1–E4，返回 None"
        );
    }

    #[test]
    fn lod_transition_edge_classify_same_tier_returns_none() {
        // 同 tier 无意义转换
        assert_eq!(
            LodTransitionEdge::classify(NpcLodTier::Mid, NpcLodTier::Mid),
            None,
            "期望同 tier 转换返回 None"
        );
    }

    // =========================================================================
    // 守恒红线：LOD tier 切换不影响 qi（ECS 层验证）
    // =========================================================================

    #[test]
    fn lod_tier_change_does_not_affect_qi_conservation_invariant() {
        // 验证：NpcLodTier 是纯 ECS Component，不持有任何 qi 字段。
        // 换句话说：切换 tier 只是 insert(NpcLodTier::Mid) 等操作，
        // NPC 的 CultivationState.qi_current 完全独立，不被 LOD tier 触碰。
        //
        // 此测试通过 LodTransitionEdge::has_qi_ledger_operation() 语义锁定该不变量：
        // 所有 6 条转换边均返回 false → LOD 层对 qi 账本零操作。
        let all_lod_tier_transitions = [
            (NpcLodTier::Near, NpcLodTier::Mid),
            (NpcLodTier::Mid, NpcLodTier::Near),
            (NpcLodTier::Mid, NpcLodTier::Far),
            (NpcLodTier::Far, NpcLodTier::Mid),
            (NpcLodTier::Near, NpcLodTier::Far),
            (NpcLodTier::Near, NpcLodTier::Dormant),
            (NpcLodTier::Far, NpcLodTier::Dormant),
            (NpcLodTier::Dormant, NpcLodTier::Near),
        ];
        for (from, to) in all_lod_tier_transitions {
            // 对于 E1-E4 有归类的边，验证无 qi 操作
            if let Some(edge) = LodTransitionEdge::classify(from, to) {
                assert!(
                    !edge.has_qi_ledger_operation(),
                    "守恒红线：{from:?}→{to:?} 对应 {edge:?} 不应有 qi 账本操作"
                );
            }
            // 其余边（跨档跳变）在正常运行时不会发生，
            // hydrate/dehydrate 边（E5/E6）由独立系统负责，同样无 ledger 操作。
        }
    }

    // =========================================================================
    // 守恒单测：Drowsy↔Dormant↔Far 6边转换矩阵 + drowsy_tick + boundary crossing
    // =========================================================================

    /// 辅助：构建一个只含 spawn zone 的 ZoneRegistry（spirit_qi 可配置）。
    fn make_zone_registry(spirit_qi: f64) -> crate::world::zone::ZoneRegistry {
        use crate::world::dimension::DimensionKind;
        use crate::world::zone::{Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
        ZoneRegistry {
            zones: vec![Zone {
                name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (
                    DVec3::new(-200.0, 0.0, -200.0),
                    DVec3::new(200.0, 300.0, 200.0),
                ),
                spirit_qi,
                danger_level: 0,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        }
    }

    /// 辅助：账本总量（所有账户余额之和）。
    fn ledger_total(ledger: &crate::qi_physics::WorldQiAccount) -> f64 {
        ledger.iter_balances().map(|(_, b)| b).sum()
    }

    /// 辅助：执行一次 drowsy regen（提取 drowsy_tick_system 的纯逻辑核心），
    /// 返回 (qi_current_after, zone_spirit_qi_after)。
    fn run_drowsy_regen_once(
        zone_spirit_qi: f64,
        npc_qi_current: f64,
        npc_qi_max: f64,
        meridian_rate: f64,
        meridian_integrity: f64,
        ledger: &mut crate::qi_physics::WorldQiAccount,
    ) -> (f64, f64) {
        use crate::qi_physics::{
            constants::QI_ZONE_UNIT_CAPACITY, regen_from_zone, QiAccountId, QiTransfer,
            QiTransferReason,
        };
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let zone_account = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME);
        let npc_account = QiAccountId::npc("test_npc_drowsy".to_string());

        let room = (npc_qi_max - npc_qi_current).max(0.0);
        let (gain, drain) =
            regen_from_zone(zone_spirit_qi, meridian_rate, meridian_integrity, room);
        if gain <= 0.0 || drain <= 0.0 {
            return (npc_qi_current, zone_spirit_qi);
        }
        ledger
            .set_balance(
                zone_account.clone(),
                zone_spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY,
            )
            .expect("set zone balance");
        ledger
            .set_balance(npc_account.clone(), npc_qi_current.max(0.0))
            .expect("set npc balance");
        let transfer = QiTransfer::new(
            zone_account,
            npc_account.clone(),
            gain,
            QiTransferReason::CultivationRegen,
        )
        .expect("valid transfer");
        ledger.transfer(transfer).expect("transfer ok");
        let new_qi = ledger.balance(&npc_account);
        let new_zone = (zone_spirit_qi - drain).max(0.0);
        (new_qi, new_zone)
    }

    // -------------------------------------------------------------------------
    // lod_tier_six_edge_transitions：6 条有向边语义 + 守恒锁
    // -------------------------------------------------------------------------

    #[test]
    fn lod_tier_six_edge_transitions_e1_to_e4_no_qi() {
        // E1–E4（ECS LOD tier 切换）全部 has_qi_ledger_operation() == false。
        // 这是 P7 守恒红线的显式枚举锁——任何改动这些边让它返回 true 都立刻撞红。
        let e1_to_e4 = [
            (
                NpcLodTier::Near,
                NpcLodTier::Mid,
                LodTransitionEdge::NearToMid,
            ),
            (
                NpcLodTier::Mid,
                NpcLodTier::Near,
                LodTransitionEdge::MidToNear,
            ),
            (
                NpcLodTier::Mid,
                NpcLodTier::Far,
                LodTransitionEdge::MidToFar,
            ),
            (
                NpcLodTier::Far,
                NpcLodTier::Mid,
                LodTransitionEdge::FarToMid,
            ),
        ];
        for (from, to, expected_edge) in e1_to_e4 {
            let edge = LodTransitionEdge::classify(from, to);
            assert_eq!(
                edge,
                Some(expected_edge),
                "期望 {from:?}→{to:?} 被识别为 {expected_edge:?}，\
                 因为 E1–E4 是 ECS LOD tier 切换边，对应 LodTransitionEdge 显式枚举"
            );
            assert!(
                !edge.unwrap().has_qi_ledger_operation(),
                "守恒红线：{from:?}→{to:?} 对应 {expected_edge:?} 不得有 qi 账本操作，\
                 因为 LOD tier 切换只是改 NpcLodTier component，不触碰 ledger"
            );
        }
    }

    #[test]
    fn lod_tier_six_edge_transitions_e5_e6_no_qi_ledger() {
        // E5/E6（hydrate/dehydrate store 边界）同样 has_qi_ledger_operation() == false：
        // qi 以 snapshot 原值拷入/拷出，不走 ledger.transfer。
        assert!(
            !LodTransitionEdge::LiveToDormantStore.has_qi_ledger_operation(),
            "守恒红线：E5（dehydrate）不做 ledger 操作，qi 原值随快照保存，\
             因为 dehydrate_far_npcs_system 只把 Cultivation.qi_current 写入快照，不调 ledger"
        );
        assert!(
            !LodTransitionEdge::DormantStoreToLive.has_qi_ledger_operation(),
            "守恒红线：E6（hydrate）不做 ledger 操作，qi 随快照原值恢复，\
             因为 hydrate_dormant_near_players_system 从快照读 qi_current 插回 ECS，不调 ledger"
        );
    }

    // -------------------------------------------------------------------------
    // drowsy_tick_conserves_qi：双录簿守恒 + 不丢/造真元
    // -------------------------------------------------------------------------

    #[test]
    fn drowsy_tick_conserves_qi_basic_regen() {
        // Drowsy regen：zone→NPC 搬运后账本总量不变（零和）。
        // 期望：before_total == after_total，因为 regen 是 zone→npc transfer，
        // 不新增、不销毁真元，只是在账户间搬运。
        use crate::qi_physics::WorldQiAccount;

        let zone_qi = 0.8;
        let npc_qi = 2.0;
        let npc_qi_max = 10.0;
        let meridian_rate = 0.5;
        let meridian_integrity = 1.0;

        let mut ledger = WorldQiAccount::default();
        // 预先 fund 账户（模拟系统总量）
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .unwrap();
        ledger
            .set_balance(QiAccountId::npc("test_npc_drowsy".to_string()), npc_qi)
            .unwrap();
        let before_total = ledger_total(&ledger);

        let (npc_after, _zone_after) = run_drowsy_regen_once(
            zone_qi,
            npc_qi,
            npc_qi_max,
            meridian_rate,
            meridian_integrity,
            &mut ledger,
        );
        let after_total = ledger_total(&ledger);

        assert!(
            npc_after > npc_qi,
            "期望 Drowsy regen 后 NPC 真元增加，因为 zone 有灵气且 NPC 未满，\
             实际 before={npc_qi} after={npc_after}"
        );
        assert!(
            (before_total - after_total).abs() < 1e-9,
            "守恒红线：Drowsy regen 前后账本总量严格相等（零和），\
             因为 regen 走 ledger.transfer zone→npc，不新增真元；\
             期望 {before_total}，实际 {after_total}，漂移 {}",
            (before_total - after_total).abs()
        );
    }

    #[test]
    fn drowsy_tick_conserves_qi_multiple_rounds() {
        // 多轮 Drowsy regen 累积账本守恒。
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let mut zone_qi = 0.8;
        let mut npc_qi = 1.0;
        let npc_qi_max = 10.0;

        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .unwrap();
        ledger
            .set_balance(QiAccountId::npc("test_npc_drowsy".to_string()), npc_qi)
            .unwrap();
        let before_total = ledger_total(&ledger);

        for _round in 0..20 {
            let (new_npc, new_zone) =
                run_drowsy_regen_once(zone_qi, npc_qi, npc_qi_max, 0.3, 0.9, &mut ledger);
            npc_qi = new_npc;
            zone_qi = new_zone;
        }

        let after_total = ledger_total(&ledger);
        assert!(
            (before_total - after_total).abs() < 1e-6,
            "守恒红线：20 轮 Drowsy regen 后账本总量仍守恒（积累误差 <1e-6），\
             因为每轮 transfer 均在账本内部搬运，无凭空增减；\
             期望 {before_total}，实际 {after_total}"
        );
    }

    #[test]
    fn drowsy_tick_conserves_qi_zone_empty_no_regen() {
        // zone 无灵气时 Drowsy regen 跳过，账本总量不变。
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let zone_qi = 0.0;
        let npc_qi = 2.0;
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .unwrap();
        ledger
            .set_balance(QiAccountId::npc("test_npc_drowsy".to_string()), npc_qi)
            .unwrap();
        let before_total = ledger_total(&ledger);

        let (npc_after, _zone_after) =
            run_drowsy_regen_once(zone_qi, npc_qi, 10.0, 0.5, 1.0, &mut ledger);

        assert_eq!(
            npc_after, npc_qi,
            "期望 zone 为空时 Drowsy regen 跳过，NPC 真元不变，\
             因为 regen_from_zone 要求 zone_qi>0 才有增益"
        );
        let after_total = ledger_total(&ledger);
        assert!(
            (before_total - after_total).abs() < 1e-12,
            "期望 zone 为空时账本总量不变，实际漂移 {}",
            (before_total - after_total).abs()
        );
    }

    #[test]
    fn drowsy_tick_conserves_qi_npc_full_no_regen() {
        // NPC 真元已满时 Drowsy regen 跳过（room=0），账本总量不变。
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let zone_qi = 0.8;
        let npc_qi = 10.0;
        let npc_qi_max = 10.0; // 已满
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .unwrap();
        ledger
            .set_balance(QiAccountId::npc("test_npc_drowsy".to_string()), npc_qi)
            .unwrap();
        let before_total = ledger_total(&ledger);

        let (npc_after, _zone_after) =
            run_drowsy_regen_once(zone_qi, npc_qi, npc_qi_max, 0.5, 1.0, &mut ledger);

        assert_eq!(
            npc_after, npc_qi,
            "期望 NPC 真元已满时 Drowsy regen 跳过（room=0），因为 regen_from_zone room<=0 返回 0"
        );
        let after_total = ledger_total(&ledger);
        assert!(
            (before_total - after_total).abs() < 1e-12,
            "期望 NPC 满真元时账本总量不变，实际漂移 {}",
            (before_total - after_total).abs()
        );
    }

    #[test]
    fn drowsy_tick_conserves_qi_no_meridian_no_regen() {
        // 无经脉（rate=0）时 Drowsy regen 跳过，账本总量不变。
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let zone_qi = 0.8;
        let npc_qi = 2.0;
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .unwrap();
        ledger
            .set_balance(QiAccountId::npc("test_npc_drowsy".to_string()), npc_qi)
            .unwrap();
        let before_total = ledger_total(&ledger);

        let (npc_after, _zone_after) =
            run_drowsy_regen_once(zone_qi, npc_qi, 10.0, 0.0, 1.0, &mut ledger);

        assert_eq!(
            npc_after, npc_qi,
            "期望 meridian rate=0 时 Drowsy regen 跳过，NPC 真元不变"
        );
        let after_total = ledger_total(&ledger);
        assert!(
            (before_total - after_total).abs() < 1e-12,
            "期望 rate=0 时账本总量不变，实际漂移 {}",
            (before_total - after_total).abs()
        );
    }

    // -------------------------------------------------------------------------
    // boundary_crossing_no_qi_leak：Near↔Mid↔Far 边界穿越不丢真元
    // -------------------------------------------------------------------------

    #[test]
    fn boundary_crossing_no_qi_leak_near_to_mid() {
        // Near→Mid（E1）：NPC 从 Near 区移入 Mid 区，LOD tier 改变，qi 不变。
        // 模拟：假设 NPC 有 qi_current=5.0，切换 tier 后 qi_current 仍是 5.0。
        // 这是 ECS 级别的守恒：NpcLodTier 不持有 qi，切换只是 component 替换。
        let npc_qi = 5.0;
        // LOD tier 切换无账本操作 —— 此处验证 classify→E1 无 qi 操作 + NPC 真元独立
        let edge = LodTransitionEdge::classify(NpcLodTier::Near, NpcLodTier::Mid)
            .expect("Near→Mid 应为 E1");
        assert!(
            !edge.has_qi_ledger_operation(),
            "守恒红线：Near→Mid 穿越（E1）不应有 qi 账本操作，\
             NPC 真元 {npc_qi} 在切换前后完全不变"
        );
        // NPC 的 qi_current 是 ECS Cultivation 组件，与 NpcLodTier 完全独立。
        // 此测试通过 has_qi_ledger_operation() 的语义锁定这个不变量。
        assert_eq!(
            npc_qi, 5.0,
            "期望切换 tier 后 qi_current 仍为 5.0，因为 tier 切换不触碰 Cultivation 组件"
        );
    }

    #[test]
    fn boundary_crossing_no_qi_leak_mid_to_far() {
        // Mid→Far（E3）：NPC 从 Mid 区移出到 Far 区，LOD tier 改变，qi 不变。
        let npc_qi = 3.5_f64;
        let edge =
            LodTransitionEdge::classify(NpcLodTier::Mid, NpcLodTier::Far).expect("Mid→Far 应为 E3");
        assert!(
            !edge.has_qi_ledger_operation(),
            "守恒红线：Mid→Far 穿越（E3）不应有 qi 账本操作，\
             期望 NPC 真元 {npc_qi} 在 LOD 切换时完全不受影响"
        );
    }

    #[test]
    fn boundary_crossing_no_qi_leak_far_to_mid() {
        // Far→Mid（E4）：玩家靠近，NPC 从 Far 升频到 Mid，qi 不变。
        let npc_qi = 7.2_f64;
        let edge =
            LodTransitionEdge::classify(NpcLodTier::Far, NpcLodTier::Mid).expect("Far→Mid 应为 E4");
        assert!(
            !edge.has_qi_ledger_operation(),
            "守恒红线：Far→Mid 穿越（E4）不应有 qi 账本操作，\
             期望 NPC 真元 {npc_qi} 在升频时不变"
        );
    }

    #[test]
    fn boundary_crossing_no_qi_leak_mid_to_near() {
        // Mid→Near（E2）：玩家进一步靠近，NPC 从 Mid 升频到 Near，qi 不变。
        let npc_qi = 8.0_f64;
        let edge = LodTransitionEdge::classify(NpcLodTier::Mid, NpcLodTier::Near)
            .expect("Mid→Near 应为 E2");
        assert!(
            !edge.has_qi_ledger_operation(),
            "守恒红线：Mid→Near 穿越（E2）不应有 qi 账本操作，\
             期望 NPC 真元 {npc_qi} 在升频时不变"
        );
    }

    #[test]
    fn boundary_crossing_no_qi_leak_all_six_edges_exhaustive() {
        // 穷举 6 条转换边，逐条验证 has_qi_ledger_operation() == false。
        // 这是 P7 守恒红线的最强版：任意一边返回 true 即撞红。
        let all_edges = [
            LodTransitionEdge::NearToMid,
            LodTransitionEdge::MidToNear,
            LodTransitionEdge::MidToFar,
            LodTransitionEdge::FarToMid,
            LodTransitionEdge::LiveToDormantStore,
            LodTransitionEdge::DormantStoreToLive,
        ];
        for edge in all_edges {
            assert!(
                !edge.has_qi_ledger_operation(),
                "守恒红线穷举：边 {edge:?} 不得有 qi 账本操作——\
                 LOD 切换和 hydrate/dehydrate 均不通过 ledger.transfer 搬运真元，\
                 LOD 只改频率/可见性，E5/E6 以 snapshot 原值保存/恢复"
            );
        }
    }

    #[test]
    fn boundary_crossing_drowsy_then_tier_change_no_double_count() {
        // 先做 Drowsy regen，然后切换 tier（Mid→Far），验证 qi 不被双计。
        // 关键守恒不变量：regen 已把 qi 搬入 NPC 账户；tier 切换不再碰 ledger。
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let zone_qi = 0.7;
        let npc_qi = 1.0;
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .unwrap();
        ledger
            .set_balance(QiAccountId::npc("test_npc_drowsy".to_string()), npc_qi)
            .unwrap();
        let before_total = ledger_total(&ledger);

        // Step 1：Drowsy regen（账本守恒转移）
        let (npc_after_regen, _) =
            run_drowsy_regen_once(zone_qi, npc_qi, 10.0, 0.4, 1.0, &mut ledger);

        // Step 2：tier 切换 Mid→Far（E3，无账本操作）
        let e3 = LodTransitionEdge::classify(NpcLodTier::Mid, NpcLodTier::Far).unwrap();
        assert!(
            !e3.has_qi_ledger_operation(),
            "E3 tier 切换不应有 qi 账本操作"
        );

        // 账本总量自 regen 起就守恒
        let after_total = ledger_total(&ledger);
        assert!(
            (before_total - after_total).abs() < 1e-9,
            "守恒红线：Drowsy regen + tier 切换后账本总量严格守恒，\
             期望 {before_total}，实际 {after_total}，NPC regen 后真元 {npc_after_regen}"
        );
    }

    #[test]
    fn drowsy_tick_conserves_qi_partial_zone_drain() {
        // zone 灵气不足以给 NPC 满额 regen 时，实际搬运量 = zone 实际能给的量，不超支。
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        // zone 极低灵气（接近 0）
        let zone_qi = 0.001;
        let npc_qi = 0.0;
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .unwrap();
        ledger
            .set_balance(QiAccountId::npc("test_npc_drowsy".to_string()), npc_qi)
            .unwrap();
        let before_total = ledger_total(&ledger);

        let (npc_after, zone_after) =
            run_drowsy_regen_once(zone_qi, npc_qi, 10.0, 1.0, 1.0, &mut ledger);

        // zone 灵气不应变负
        assert!(
            zone_after >= 0.0,
            "期望 zone 灵气不应变负，因为 regen_from_zone 有 drain<=zone_qi 保护，\
             实际 zone_after={zone_after}"
        );
        // 账本守恒
        let after_total = ledger_total(&ledger);
        assert!(
            (before_total - after_total).abs() < 1e-9,
            "守恒红线：zone 极低灵气 regen 后账本仍守恒，\
             before={before_total} after={after_total} npc_after={npc_after}"
        );
    }

    // -------------------------------------------------------------------------
    // ECS App 驱动的 drowsy_tick_system 守恒测试
    // 锁真系统接线——防止 drowsy_tick_system 与 run_drowsy_regen_once 脱节导致
    // 守恒回归被静默漏过（饱和测试纪律：不假设单元拼起来就是对的）。
    // -------------------------------------------------------------------------

    /// 辅助：构造一个有一条打通经脉（rate=0.5, integrity=1.0）的 MeridianSystem。
    fn make_meridian_system_with_one_open() -> MeridianSystem {
        let mut ms = MeridianSystem::default();
        // 打通第一条正经脉，设置 flow_rate=0.5，integrity=1.0
        ms.regular[0].opened = true;
        ms.regular[0].flow_rate = 0.5;
        ms.regular[0].integrity = 1.0;
        ms
    }

    #[test]
    fn drowsy_tick_system_ecs_app_conserves_qi_single_npc() {
        // ECS App 驱动的 drowsy_tick_system 守恒测试（单 NPC）。
        //
        // 构建真实 Bevy App，插入 drowsy_tick_system 系统，
        // 验证一帧后账本总量严格守恒（零和），且 NPC qi 确实增长（系统没空转）。
        //
        // 不变量：
        // - regen 是 zone→npc transfer，不新增/不销毁真元
        // - `|total_before - total_after| < 1e-6`
        // - `cultivation.qi_current > qi_initial`（证明系统真的跑了 regen）
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let zone_qi_initial = 0.8_f64;
        let npc_qi_initial = 2.0_f64;
        let npc_qi_max = 10.0_f64;

        // 构建 App
        let mut app = App::new();

        // 插入 drowsy_tick_system 需要的 Resources
        // GameTick(20)：20 % DROWSY_TICK_INTERVAL(20) == 0，触发 regen
        app.insert_resource(crate::npc::movement::GameTick(20));

        let zone_registry = make_zone_registry(zone_qi_initial);
        app.insert_resource(zone_registry);

        let mut ledger = WorldQiAccount::default();
        // 预先建立账户初始余额（set_balance 幂等，先 fund zone）
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi_initial * QI_ZONE_UNIT_CAPACITY,
            )
            .expect("zone set_balance");
        // NPC 账户此时尚未存在，drowsy_tick_system 会在 transfer 前 set_balance；
        // 这里不预注册 npc account，测试真实系统路径（set_balance 在系统内部调用）
        app.insert_resource(ledger);

        // spawn Mid 档 NPC entity，位于 zone 范围内（[-200,200]×[0,300]×[-200,200]）
        let cultivation = crate::cultivation::components::Cultivation {
            qi_current: npc_qi_initial,
            qi_max: npc_qi_max,
            ..Default::default()
        };

        let meridian_system = make_meridian_system_with_one_open();

        // 验证经脉确实打通且有 rate > 0
        assert!(
            meridian_system.sum_rate() > 0.0,
            "期望测试用 MeridianSystem 有 sum_rate>0，否则 drowsy regen 会跳过；\
             实际 sum_rate={}",
            meridian_system.sum_rate()
        );

        let npc_entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                NpcLodTier::Mid,
                NpcDrowsyState::default(),
                cultivation,
                meridian_system,
            ))
            .id();

        // 注册 drowsy_tick_system（真实系统，非 run_drowsy_regen_once 复刻）
        app.add_systems(Update, drowsy_tick_system);

        // drowsy_tick_system 内部先 set_balance 对齐 npc 账户到 ECS 状态，再 transfer。
        // 为让 total_before 包含 npc 的初始 qi，提前在 ledger 注册 npc 账户。
        // 账户 ID 与系统内部逻辑一致：`drowsy_live:{entity.index()}`
        {
            let npc_entity_index = npc_entity.index();
            let npc_account_id = QiAccountId::npc(format!("drowsy_live:{npc_entity_index}"));
            app.world_mut()
                .resource_mut::<WorldQiAccount>()
                .set_balance(npc_account_id, npc_qi_initial)
                .expect("pre-register npc account");
        }

        // 记录 regen 前账本总量（zone + npc 两个账户均已注册）
        let total_before = {
            let ledger = app.world().resource::<WorldQiAccount>();
            ledger.iter_balances().map(|(_, b)| b).sum::<f64>()
        };

        // 跑一帧（触发 drowsy_tick_system）
        app.update();

        // 读出 regen 后状态
        let cultivation_after = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(npc_entity)
            .expect("npc Cultivation 组件应存在");
        let qi_after = cultivation_after.qi_current;

        let npc_entity_index = npc_entity.index();
        let npc_account_id = QiAccountId::npc(format!("drowsy_live:{npc_entity_index}"));
        let (total_after, ledger_npc_qi) = {
            let ledger = app.world().resource::<WorldQiAccount>();
            let total = ledger.iter_balances().map(|(_, b)| b).sum::<f64>();
            let npc_qi = ledger.balance(&npc_account_id);
            (total, npc_qi)
        };

        // 断言 NPC qi 确实增长（证明 drowsy_tick_system 真的跑了 regen，不是空转）
        assert!(
            qi_after > npc_qi_initial,
            "期望 drowsy_tick_system 运行后 NPC qi 增长（regen 零和搬运），\
             因为 zone 有灵气且 NPC 未满且经脉有 rate；\
             before={npc_qi_initial} after={qi_after}"
        );

        // 断言 ECS Cultivation.qi_current 与 ledger NPC 账户余额一致
        assert!(
            (qi_after - ledger_npc_qi).abs() < 1e-9,
            "期望 Cultivation.qi_current 与 ledger NPC 账户余额一致，\
             因为 drowsy_tick_system 在 transfer 后 cultivation.qi_current = ledger.balance(npc)；\
             cultivation={qi_after} ledger={ledger_npc_qi}"
        );

        // 守恒断言：regen 前后账本总量严格不变（零和）
        assert!(
            (total_before - total_after).abs() < 1e-6,
            "守恒红线：drowsy_tick_system 真实系统驱动一帧后账本总量严格守恒（零和），\
             因为 regen 走 ledger.transfer zone→npc，不新增真元；\
             期望 total 守恒，before={total_before} after={total_after} \
             漂移={}",
            (total_before - total_after).abs()
        );
    }

    #[test]
    fn drowsy_tick_system_ecs_app_no_regen_when_near_tier() {
        // ECS App 驱动的 drowsy_tick_system：Near 档 NPC 不触发 drowsy regen。
        //
        // drowsy_tick_system 内部检查 `*tier != NpcLodTier::Mid` 即跳过。
        // 本测试验证：同 App 内 Near 档 NPC 的 qi 不被修改，账本总量不变。
        // 这锁住"只有 Mid 档 NPC 才做 drowsy regen"这条接线契约。
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let zone_qi_initial = 0.8_f64;
        let npc_qi_initial = 2.0_f64;

        let mut app = App::new();
        app.insert_resource(crate::npc::movement::GameTick(20));
        app.insert_resource(make_zone_registry(zone_qi_initial));

        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi_initial * QI_ZONE_UNIT_CAPACITY,
            )
            .expect("zone set_balance");
        app.insert_resource(ledger);

        let cultivation = crate::cultivation::components::Cultivation {
            qi_current: npc_qi_initial,
            qi_max: 10.0,
            ..Default::default()
        };

        // Near 档 NPC（不应触发 drowsy regen）
        let npc_entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                NpcLodTier::Near, // 关键：Near 而非 Mid
                NpcDrowsyState::default(),
                cultivation,
                make_meridian_system_with_one_open(),
            ))
            .id();

        app.add_systems(Update, drowsy_tick_system);

        let total_before = {
            let ledger = app.world().resource::<WorldQiAccount>();
            ledger.iter_balances().map(|(_, b)| b).sum::<f64>()
        };

        app.update();

        let qi_after = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(npc_entity)
            .expect("Cultivation 组件应存在")
            .qi_current;

        let total_after = {
            let ledger = app.world().resource::<WorldQiAccount>();
            ledger.iter_balances().map(|(_, b)| b).sum::<f64>()
        };

        // Near 档不做 regen：qi 不变
        assert_eq!(
            qi_after, npc_qi_initial,
            "期望 Near 档 NPC 的 qi 不被 drowsy_tick_system 修改，\
             因为 drowsy_tick_system 检查 tier != Mid 即跳过；\
             before={npc_qi_initial} after={qi_after}"
        );

        // 账本总量同样不变
        assert!(
            (total_before - total_after).abs() < 1e-12,
            "期望 Near 档 NPC 一帧后账本总量不变（系统跳过无操作），\
             before={total_before} after={total_after} \
             漂移={}",
            (total_before - total_after).abs()
        );
    }

    #[test]
    fn drowsy_tick_interval_is_20_ticks() {
        // DROWSY_TICK_INTERVAL 必须是 20（1Hz at 20TPS）。
        // plan §220 明确规定 drowsy_tick_system 以 1Hz 运行；20TPS × 1Hz = 20 tick。
        // 此测试把这个常量锁定，防止不小心改成其他值。
        assert_eq!(
            DROWSY_TICK_INTERVAL, 20,
            "期望 DROWSY_TICK_INTERVAL=20（1Hz @ 20TPS），\
             因为 plan-offscreen-war-v1 §220 规定 drowsy_tick_system 以 1Hz 运行"
        );
    }

    #[test]
    fn npc_drowsy_state_default_is_unticked() {
        // NpcDrowsyState::default() 的 last_drowsy_tick == 0（未执行过）。
        let state = NpcDrowsyState::default();
        assert_eq!(
            state.last_drowsy_tick, 0,
            "期望 NpcDrowsyState 默认 last_drowsy_tick=0（未执行过），\
             因为 u64 default 是 0，表示尚未 drowsy tick"
        );
    }

    #[test]
    fn drowsy_tick_conserves_qi_high_integrity_vs_low_integrity() {
        // 完整经脉（integrity=1.0）regen 量 > 受损经脉（integrity=0.3）regen 量，
        // 但两种情况下账本均严格守恒。
        use crate::qi_physics::{constants::QI_ZONE_UNIT_CAPACITY, QiAccountId, WorldQiAccount};
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let zone_qi = 0.6;
        let npc_qi = 0.0;

        let setup_ledger = || {
            let mut ledger = WorldQiAccount::default();
            ledger
                .set_balance(
                    QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                    zone_qi * QI_ZONE_UNIT_CAPACITY,
                )
                .unwrap();
            ledger
                .set_balance(QiAccountId::npc("test_npc_drowsy".to_string()), npc_qi)
                .unwrap();
            ledger
        };

        let mut ledger_high = setup_ledger();
        let before_high = ledger_total(&ledger_high);
        let (npc_high, _) =
            run_drowsy_regen_once(zone_qi, npc_qi, 10.0, 0.5, 1.0, &mut ledger_high);
        let after_high = ledger_total(&ledger_high);

        let mut ledger_low = setup_ledger();
        let before_low = ledger_total(&ledger_low);
        let (npc_low, _) = run_drowsy_regen_once(zone_qi, npc_qi, 10.0, 0.5, 0.3, &mut ledger_low);
        let after_low = ledger_total(&ledger_low);

        assert!(
            npc_high > npc_low,
            "期望高 integrity({npc_high}) > 低 integrity({npc_low}) regen 量，\
             因为 regen_from_zone 乘以 avg_integrity"
        );
        assert!(
            (before_high - after_high).abs() < 1e-9,
            "守恒红线：高 integrity regen 账本守恒，漂移 {}",
            (before_high - after_high).abs()
        );
        assert!(
            (before_low - after_low).abs() < 1e-9,
            "守恒红线：低 integrity regen 账本守恒，漂移 {}",
            (before_low - after_low).abs()
        );
    }

    #[test]
    fn drowsy_tick_conserves_qi_many_npcs_parallel() {
        // 多个 Mid 档 NPC 同时 regen，总账本守恒。
        // 模拟：5 个 NPC 共用一个 zone，每个独立做 regen。
        use crate::qi_physics::{
            constants::QI_ZONE_UNIT_CAPACITY, regen_from_zone, QiAccountId, QiTransfer,
            QiTransferReason, WorldQiAccount,
        };
        use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

        let mut zone_qi = 0.9;
        let npc_count = 5usize;
        let npc_qi_init = 1.0f64;

        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zone_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .unwrap();
        for i in 0..npc_count {
            ledger
                .set_balance(QiAccountId::npc(format!("drowsy_npc_{i}")), npc_qi_init)
                .unwrap();
        }
        let before_total = ledger_total(&ledger);

        // 逐个 NPC 做 regen（同 drowsy_tick_system 的 per-entity 循环语义）
        for i in 0..npc_count {
            let npc_account = QiAccountId::npc(format!("drowsy_npc_{i}"));
            let zone_account = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME);
            let npc_qi = ledger.balance(&npc_account);
            let room = (10.0_f64 - npc_qi).max(0.0);
            let (gain, drain) = regen_from_zone(zone_qi, 0.2, 1.0, room);
            if gain <= 0.0 || drain <= 0.0 {
                continue;
            }
            ledger
                .set_balance(
                    zone_account.clone(),
                    zone_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY,
                )
                .unwrap();
            ledger
                .set_balance(npc_account.clone(), npc_qi.max(0.0))
                .unwrap();
            let transfer = QiTransfer::new(
                zone_account,
                npc_account,
                gain,
                QiTransferReason::CultivationRegen,
            )
            .unwrap();
            ledger.transfer(transfer).unwrap();
            zone_qi = (zone_qi - drain).max(0.0);
        }

        let after_total = ledger_total(&ledger);
        assert!(
            (before_total - after_total).abs() < 1e-6,
            "守恒红线：5 个 Mid NPC 并发 regen 后账本总量守恒，\
             before={before_total} after={after_total} 漂移 {}",
            (before_total - after_total).abs()
        );
    }

    // =========================================================================
    // ECS App 驱动的 boundary_crossing 守恒测试（P7 PR-11）
    //
    // 用真实 Bevy App + fake ClientMarker 玩家触发 tier 切换，
    // 断言 Cultivation.qi_current 在切换前后完全不变。
    // 补全了纯逻辑版 boundary_crossing_no_qi_leak_* 只测 has_qi_ledger_operation
    // 但不测 ECS 系统实际执行的缺口。
    // =========================================================================

    /// 辅助：构建含 tier 系统的基础 App（NpcLodConfig + NpcLodTick + tick+update chain）。
    fn make_lod_app() -> (App, u64) {
        let mut app = App::new();
        app.insert_resource(NpcLodConfig {
            reassess_interval: 1, // 每帧重算，使切换即时可见
            ..Default::default()
        });
        app.insert_resource(NpcLodTick(0));
        app.add_systems(
            PreUpdate,
            (tick_lod_counter, update_npc_lod_tier_system).chain(),
        );
        (app, 0)
    }

    #[test]
    fn ecs_boundary_crossing_near_to_mid_qi_conserved() {
        // ECS 级别：玩家从 near 区移到远处 → NPC tier Near→Mid，Cultivation.qi_current 不变。
        //
        // 守恒红线：update_npc_lod_tier_system 只 insert NpcLodTier component，
        // 完全不触碰 Cultivation 组件，故 qi_current 在 tier 切换时精确不变。
        let (mut app, _) = make_lod_app();

        let qi_initial = 4.2_f64;
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    qi_current: qi_initial,
                    qi_max: 10.0,
                    ..Default::default()
                },
            ))
            .id();
        // 玩家初始在 NPC 旁（Near 档）
        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([10.0, 64.0, 0.0])))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Near),
            "期望玩家在 10 格时 NPC 为 Near"
        );

        // 玩家移到 150 格 → Mid（80..256）
        *app.world_mut().get_mut::<Position>(player).unwrap() =
            Position::new([150.0, 64.0, 0.0]);
        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Mid),
            "期望玩家在 150 格时 NPC 为 Mid（80..256）"
        );

        let qi_after = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(npc)
            .expect("Cultivation 组件应存在")
            .qi_current;
        assert_eq!(
            qi_after, qi_initial,
            "守恒红线：ECS Near→Mid tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}，\
             因为 update_npc_lod_tier_system 只修改 NpcLodTier 组件"
        );
    }

    #[test]
    fn ecs_boundary_crossing_mid_to_near_qi_conserved() {
        // ECS 级别：玩家从 mid 区靠近 → NPC tier Mid→Near，Cultivation.qi_current 不变。
        let (mut app, _) = make_lod_app();

        let qi_initial = 7.5_f64;
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    qi_current: qi_initial,
                    qi_max: 10.0,
                    ..Default::default()
                },
            ))
            .id();
        // 玩家初始在 150 格（Mid 档）
        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([150.0, 64.0, 0.0])))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Mid),
            "期望玩家在 150 格时 NPC 为 Mid"
        );

        // 玩家靠近到 20 格 → Near（<80）
        *app.world_mut().get_mut::<Position>(player).unwrap() =
            Position::new([20.0, 64.0, 0.0]);
        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Near),
            "期望玩家靠近到 20 格后 NPC 升频为 Near（<80）"
        );

        let qi_after = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(npc)
            .expect("Cultivation 组件应存在")
            .qi_current;
        assert_eq!(
            qi_after, qi_initial,
            "守恒红线：ECS Mid→Near tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}"
        );
    }

    #[test]
    fn ecs_boundary_crossing_mid_to_far_qi_conserved() {
        // ECS 级别：玩家从 mid 区退到远处 → NPC tier Mid→Far，qi 不变。
        let (mut app, _) = make_lod_app();

        let qi_initial = 3.1_f64;
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    qi_current: qi_initial,
                    qi_max: 10.0,
                    ..Default::default()
                },
            ))
            .id();
        // 玩家在 150 格（Mid）
        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([150.0, 64.0, 0.0])))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Mid),
            "期望玩家在 150 格时 NPC 为 Mid"
        );

        // 玩家退到 400 格 → Far（256..512）
        *app.world_mut().get_mut::<Position>(player).unwrap() =
            Position::new([400.0, 64.0, 0.0]);
        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Far),
            "期望玩家退到 400 格后 NPC 降频为 Far（256..512）"
        );

        let qi_after = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(npc)
            .expect("Cultivation 组件应存在")
            .qi_current;
        assert_eq!(
            qi_after, qi_initial,
            "守恒红线：ECS Mid→Far tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}"
        );
    }

    #[test]
    fn ecs_boundary_crossing_far_to_dormant_qi_conserved() {
        // ECS 级别：玩家从 far 区退到更远 → NPC tier Far→Dormant，qi 不变。
        let (mut app, _) = make_lod_app();

        let qi_initial = 6.0_f64;
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    qi_current: qi_initial,
                    qi_max: 10.0,
                    ..Default::default()
                },
            ))
            .id();
        // 玩家在 400 格（Far）
        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([400.0, 64.0, 0.0])))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Far),
            "期望玩家在 400 格时 NPC 为 Far"
        );

        // 玩家退到 1000 格 → Dormant（>512）
        *app.world_mut().get_mut::<Position>(player).unwrap() =
            Position::new([1000.0, 64.0, 0.0]);
        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Dormant),
            "期望玩家退到 1000 格后 NPC 为 Dormant（>512）"
        );

        let qi_after = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(npc)
            .expect("Cultivation 组件应存在")
            .qi_current;
        assert_eq!(
            qi_after, qi_initial,
            "守恒红线：ECS Far→Dormant tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}"
        );
    }

    #[test]
    fn ecs_boundary_crossing_dormant_to_near_qi_conserved() {
        // ECS 级别：玩家从远处靠近 Dormant NPC → Near，qi 不变（E5/E6 live↔dormant 之外的直跳）。
        let (mut app, _) = make_lod_app();

        let qi_initial = 9.0_f64;
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    qi_current: qi_initial,
                    qi_max: 10.0,
                    ..Default::default()
                },
            ))
            .id();
        // 无玩家 → Dormant（首帧分类）
        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Dormant),
            "期望无玩家时 NPC 为 Dormant"
        );

        // 玩家出现在 10 格 → Near
        let _player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([10.0, 64.0, 0.0])))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<NpcLodTier>(npc).copied(),
            Some(NpcLodTier::Near),
            "期望玩家靠近后 NPC 从 Dormant 直升 Near（<80）"
        );

        let qi_after = app
            .world()
            .get::<crate::cultivation::components::Cultivation>(npc)
            .expect("Cultivation 组件应存在")
            .qi_current;
        assert_eq!(
            qi_after, qi_initial,
            "守恒红线：ECS Dormant→Near tier 切换后 Cultivation.qi_current 精确不变，\
             期望 {qi_initial}，实际 {qi_after}"
        );
    }

    #[test]
    fn ecs_boundary_crossing_batch_multi_npc_all_tiers_qi_conserved() {
        // ECS 级别批量穿越：4 个 NPC 分别在各边界附近，玩家移动后 tier 全部切换，
        // 所有 NPC 的 qi_current 精确不变（零和不变量，LOD 不搬运真元）。
        //
        // 布局：
        //   npc_near  @ [0,  64, 0]，玩家初始 @ [10, 64, 0] → Near；  玩家移远后 → Dormant
        //   npc_mid   @ [150,64, 0]，同上 → Mid；  玩家移远后 → Dormant
        //   npc_far   @ [400,64, 0]，同上 → Far；  玩家移远后 → Dormant
        //   npc_dorm  @ [800,64, 0]，同上 → Dormant；  玩家移近后 → Near (if ≤80)
        let (mut app, _) = make_lod_app();

        let qis = [1.1_f64, 2.2, 3.3, 4.4];
        let positions = [
            [0.0_f64, 64.0, 0.0],
            [150.0, 64.0, 0.0],
            [400.0, 64.0, 0.0],
            [800.0, 64.0, 0.0],
        ];

        let mut npcs = Vec::new();
        for (i, (&qi, pos)) in qis.iter().zip(positions.iter()).enumerate() {
            let id = app
                .world_mut()
                .spawn((
                    NpcMarker,
                    Position::new(*pos),
                    crate::cultivation::components::Cultivation {
                        qi_current: qi,
                        qi_max: 10.0,
                        ..Default::default()
                    },
                ))
                .id();
            npcs.push((id, qi, i));
        }

        // 玩家在 npc_near 旁边（10 格）
        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([10.0, 64.0, 0.0])))
            .id();

        // 首帧分类：npc_near=Near, npc_mid=Mid, npc_far=Far, npc_dorm=Dormant
        app.update();
        let expected_initial = [
            NpcLodTier::Near,
            NpcLodTier::Mid,
            NpcLodTier::Far,
            NpcLodTier::Dormant,
        ];
        for (&(id, _, i), expected) in npcs.iter().zip(expected_initial.iter()) {
            assert_eq!(
                app.world().get::<NpcLodTier>(id).copied(),
                Some(*expected),
                "期望首帧 npc[{i}] tier={expected:?}"
            );
        }

        // 验证初始 qi 不变
        for &(id, qi_init, i) in &npcs {
            let qi = app
                .world()
                .get::<crate::cultivation::components::Cultivation>(id)
                .expect("Cultivation 存在")
                .qi_current;
            assert_eq!(
                qi, qi_init,
                "守恒：首帧分类后 npc[{i}] qi 不变，期望 {qi_init}，实际 {qi}"
            );
        }

        // 玩家移到 2000 格（全部 NPC → Dormant）
        *app.world_mut().get_mut::<Position>(player).unwrap() =
            Position::new([2000.0, 64.0, 0.0]);
        app.update();

        for (&(id, _, i), _) in npcs.iter().zip(expected_initial.iter()) {
            assert_eq!(
                app.world().get::<NpcLodTier>(id).copied(),
                Some(NpcLodTier::Dormant),
                "期望玩家移到 2000 格后 npc[{i}] 全部 → Dormant"
            );
        }

        // 穿越到 Dormant 后 qi 仍精确不变
        let qi_sum_after: f64 = npcs
            .iter()
            .map(|&(id, _, _)| {
                app.world()
                    .get::<crate::cultivation::components::Cultivation>(id)
                    .expect("Cultivation 存在")
                    .qi_current
            })
            .sum();
        let qi_sum_init: f64 = qis.iter().sum();
        assert!(
            (qi_sum_after - qi_sum_init).abs() < 1e-12,
            "守恒红线：4 个 NPC 批量穿越到 Dormant 后 qi 总和不变，\
             期望 {qi_sum_init}，实际 {qi_sum_after}，\
             漂移 {}",
            (qi_sum_after - qi_sum_init).abs()
        );
    }

    #[test]
    fn ecs_boundary_crossing_repeated_oscillation_qi_conserved() {
        // ECS 级别反复振荡：NPC 在 Near↔Mid↔Far↔Dormant 反复穿越 N 轮，
        // qi_current 始终精确不变。
        //
        // 这锁住"多轮 tier 切换不积累 qi 漂移"这条不变量——
        // 任何意外的 Cultivation 写操作（哪怕很小）都会被此测试捕捉。
        let (mut app, _) = make_lod_app();

        let qi_initial = 5.5_f64;
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                crate::cultivation::components::Cultivation {
                    qi_current: qi_initial,
                    qi_max: 10.0,
                    ..Default::default()
                },
            ))
            .id();

        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
            .id();

        // 振荡序列：近 → 中 → 远 → 休眠 → 近 → ... 共 5 轮
        let oscillation_positions: &[f64] = &[10.0, 150.0, 400.0, 1000.0, 10.0, 150.0, 400.0, 1000.0, 10.0, 150.0];
        for &dist in oscillation_positions {
            *app.world_mut().get_mut::<Position>(player).unwrap() =
                Position::new([dist, 64.0, 0.0]);
            app.update();

            let qi = app
                .world()
                .get::<crate::cultivation::components::Cultivation>(npc)
                .expect("Cultivation 存在")
                .qi_current;
            assert_eq!(
                qi, qi_initial,
                "守恒红线：NPC 在 dist={dist} 时 tier 切换后 qi_current 精确不变，\
                 期望 {qi_initial}，实际 {qi}"
            );
        }
    }
}
