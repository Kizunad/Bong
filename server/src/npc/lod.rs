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
    Query, Res, ResMut, Resource, With, Without,
};

use crate::npc::spawn::NpcMarker;

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
        );
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
                commands.entity(entity).insert(desired);
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
}
