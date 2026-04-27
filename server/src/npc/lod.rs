//! NPC LOD（plan-npc-ai-v1 §7 Phase 9）。
//!
//! 按最近玩家距离把 NPC 分三档：
//! - **Near**（默认 0..=50 格）：每 tick 正常跑 scorer / action
//! - **Far**（50..=150）：每 `far_skip_interval` tick 才跑一次（默认 10）
//! - **Dormant**（>150）：scorer 阶段直接置 0，停止新行为决策；lifespan
//!   继续 tick，方便老化/寿命清理
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

/// 三档 LOD。GuardianRelic 强制 Near（考验需要实时响应）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Component)]
pub enum NpcLodTier {
    #[default]
    Near,
    Far,
    Dormant,
}

#[derive(Clone, Copy, Debug, Resource)]
pub struct NpcLodConfig {
    pub near_radius: f64,
    pub far_radius: f64,
    pub far_skip_interval: u32,
    pub dormant_skip_interval: u32,
    /// 每 N tick 重新评估一次 tier（避免每 tick O(npc × player)）。
    pub reassess_interval: u32,
}

impl Default for NpcLodConfig {
    fn default() -> Self {
        Self {
            near_radius: 50.0,
            far_radius: 150.0,
            far_skip_interval: 10,
            dormant_skip_interval: 60,
            reassess_interval: 20,
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
    app.insert_resource(NpcLodConfig::default())
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
    if counter.0 % config.reassess_interval.max(1) != 0 {
        return;
    }
    let player_positions: Vec<DVec3> = players.iter().map(|p| p.get()).collect();
    for (entity, pos, current) in &npcs {
        let desired = classify_tier(pos.get(), &player_positions, &config);
        match (current.copied(), desired) {
            (Some(c), d) if c == d => {}
            _ => {
                commands.entity(entity).insert(desired);
            }
        }
    }
}

/// 纯函数：给定 NPC 坐标 + 所有玩家坐标 + config → 期望 tier。
/// 无玩家 → Dormant；距离取最近玩家。
pub fn classify_tier(npc_pos: DVec3, players: &[DVec3], config: &NpcLodConfig) -> NpcLodTier {
    if players.is_empty() {
        return NpcLodTier::Dormant;
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
    } else if min_d <= config.far_radius {
        NpcLodTier::Far
    } else {
        NpcLodTier::Dormant
    }
}

/// Scorer 系统用：给定当前 tick + entity 的 tier，返回 true 表示**本 tick
/// 应跳过**（分数保持不变，或在想强制 0 的语境下置 0）。
/// - Near 永远 false（不跳过）
/// - Far 每 `far_skip_interval` tick 才算"非跳过"
/// - Dormant 总是 true
pub fn should_skip_scorer_tick(tier: NpcLodTier, tick: u32, config: &NpcLodConfig) -> bool {
    match tier {
        NpcLodTier::Near => false,
        NpcLodTier::Far => tick % config.far_skip_interval.max(1) != 0,
        NpcLodTier::Dormant => true,
    }
}

/// Dormant 判断的便捷版（不需要 config）。
pub fn is_dormant(tier: Option<&NpcLodTier>) -> bool {
    matches!(tier, Some(NpcLodTier::Dormant))
}

/// 与 scorer 系统配合：给 `Actor(npc)` 上挂的 scorer 查 actor 的 LOD tier。
/// 供 brain.rs / territory.rs 等共享使用的极简 helper。
pub fn actor_lod_tier<'a>(
    npc_tiers: &'a Query<'_, '_, &'a NpcLodTier, With<NpcMarker>>,
    actor: Entity,
) -> Option<NpcLodTier> {
    npc_tiers.get(actor).ok().copied()
}

/// 统计每个 tier 的 NPC 数量（debug / 监控用）。
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

    #[test]
    fn classify_tier_no_players_is_dormant() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(DVec3::new(0.0, 64.0, 0.0), &[], &cfg),
            NpcLodTier::Dormant
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
            NpcLodTier::Near
        );
    }

    #[test]
    fn classify_tier_far_between_radii() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(100.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Far
        );
    }

    #[test]
    fn classify_tier_dormant_beyond_far() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[DVec3::new(500.0, 64.0, 0.0)],
                &cfg
            ),
            NpcLodTier::Dormant
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
            "y 不参与"
        );
    }

    #[test]
    fn classify_tier_takes_nearest_player() {
        let cfg = NpcLodConfig::default();
        assert_eq!(
            classify_tier(
                DVec3::new(0.0, 64.0, 0.0),
                &[
                    DVec3::new(500.0, 64.0, 0.0),
                    DVec3::new(20.0, 64.0, 0.0), // 这个是最近
                ],
                &cfg
            ),
            NpcLodTier::Near
        );
    }

    #[test]
    fn should_skip_scorer_tick_near_never_skips() {
        let cfg = NpcLodConfig::default();
        for t in 0..40 {
            assert!(!should_skip_scorer_tick(NpcLodTier::Near, t, &cfg));
        }
    }

    #[test]
    fn should_skip_scorer_tick_dormant_always_skips() {
        let cfg = NpcLodConfig::default();
        for t in 0..40 {
            assert!(should_skip_scorer_tick(NpcLodTier::Dormant, t, &cfg));
        }
    }

    #[test]
    fn should_skip_scorer_tick_far_respects_interval() {
        let cfg = NpcLodConfig {
            far_skip_interval: 10,
            ..Default::default()
        };
        // 0, 10, 20 跑；其他跳
        assert!(!should_skip_scorer_tick(NpcLodTier::Far, 0, &cfg));
        assert!(should_skip_scorer_tick(NpcLodTier::Far, 1, &cfg));
        assert!(should_skip_scorer_tick(NpcLodTier::Far, 9, &cfg));
        assert!(!should_skip_scorer_tick(NpcLodTier::Far, 10, &cfg));
        assert!(!should_skip_scorer_tick(NpcLodTier::Far, 20, &cfg));
    }

    #[test]
    fn should_skip_clamps_zero_interval_to_at_least_one() {
        let cfg = NpcLodConfig {
            far_skip_interval: 0,
            ..Default::default()
        };
        // 0 间隔会除零；确保 clamp 到 1 → 永不跳过
        assert!(!should_skip_scorer_tick(NpcLodTier::Far, 1, &cfg));
    }

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
        let npc_far = app
            .world_mut()
            .spawn((NpcMarker, Position::new([100.0, 64.0, 0.0])))
            .id();
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
            Some(NpcLodTier::Near)
        );
        assert_eq!(
            app.world().get::<NpcLodTier>(npc_far).copied(),
            Some(NpcLodTier::Far)
        );
        assert_eq!(
            app.world().get::<NpcLodTier>(npc_dormant).copied(),
            Some(NpcLodTier::Dormant)
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
        let _ = app
            .world_mut()
            .spawn((ClientMarker, Position::new([0.0, 64.0, 0.0])))
            .id();

        // tick_lod_counter 加到 10 < 50，应未评估
        for _ in 0..10 {
            app.update();
        }
        assert!(
            app.world().get::<NpcLodTier>(npc).is_none(),
            "未到 reassess_interval 不应设 tier"
        );

        // 继续推到 50
        for _ in 0..45 {
            app.update();
        }
        assert!(app.world().get::<NpcLodTier>(npc).is_some());
    }

    #[test]
    fn is_dormant_helper() {
        assert!(is_dormant(Some(&NpcLodTier::Dormant)));
        assert!(!is_dormant(Some(&NpcLodTier::Far)));
        assert!(!is_dormant(Some(&NpcLodTier::Near)));
        assert!(!is_dormant(None));
    }
}
