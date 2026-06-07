//! 死域自然刷怪过滤规则与生产生成调度。
//!
//! P1 plan-era-state-v1：[`era_beast_spawn_gate`] 根据 WorldEraState 的 `beast_density_mul`
//! 决定本次 spawn 事件是否放行（概率门控，Calamity×1.5 / Deduction×0.8 / Unknown×1.0）。

use valence::prelude::{Commands, DVec3, Entity};

use crate::cultivation::dead_zone::is_dead_zone;
use crate::world::zone::Zone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalMobKind {
    Zombie,
    Skeleton,
    Creeper,
    Rogue,
    AshSpider,
    Daoxiang,
}

pub const DEFAULT_MOB_SPAWN_CANDIDATES: [NaturalMobKind; 6] = [
    NaturalMobKind::Zombie,
    NaturalMobKind::Skeleton,
    NaturalMobKind::Creeper,
    NaturalMobKind::Rogue,
    NaturalMobKind::AshSpider,
    NaturalMobKind::Daoxiang,
];

pub const DEAD_ZONE_MOB_WHITELIST: [NaturalMobKind; 2] =
    [NaturalMobKind::AshSpider, NaturalMobKind::Daoxiang];

pub struct MobSpawnFilter;

impl MobSpawnFilter {
    pub fn ban_in_dead_zone(zone: &Zone, mob: NaturalMobKind) -> bool {
        is_dead_zone(zone) && !DEAD_ZONE_MOB_WHITELIST.contains(&mob)
    }

    pub fn default_candidates_for_zone(zone: &Zone) -> Vec<NaturalMobKind> {
        DEFAULT_MOB_SPAWN_CANDIDATES
            .into_iter()
            .filter(|mob| !Self::ban_in_dead_zone(zone, *mob))
            .collect()
    }
}

/// 按 `NaturalMobKind` 调度生产生成。
///
/// `NaturalMobKind::AshSpider` 走 `spawn_ash_spider_npc_at`（附带完整拟态组件）；
/// 其它种类走 `spawn_beast_npc_at`（通用妖兽路径）。
///
/// 返回生成的 `Entity`，`None` 表示该种类尚未实装（不应发生，触发时 warn 日志）。
pub fn spawn_natural_mob_at(
    commands: &mut Commands,
    layer: Entity,
    kind: NaturalMobKind,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Option<Entity> {
    use crate::npc::spawn::spawn_beast_npc_at;
    use crate::npc::spawn_spider::spawn_ash_spider_npc_at;
    use crate::npc::territory::Territory;

    match kind {
        NaturalMobKind::AshSpider => {
            // 拟态灰烬蛛：附带 MimicSpiderBlackboard / SpiderDisguiseState / spider_thinker
            Some(spawn_ash_spider_npc_at(
                commands,
                layer,
                home_zone,
                spawn_position,
                patrol_target,
            ))
        }
        NaturalMobKind::Zombie
        | NaturalMobKind::Skeleton
        | NaturalMobKind::Creeper
        | NaturalMobKind::Rogue
        | NaturalMobKind::Daoxiang => {
            // 通用妖兽 / 僵尸路径（不带拟态组件）
            Some(spawn_beast_npc_at(
                commands,
                layer,
                home_zone,
                spawn_position,
                Territory::new(spawn_position, 24.0),
                0.0,
            ))
        }
    }
}

// ─── P1 Era 异变兽 spawn 密度门控 ─────────────────────────────────────────────

/// era 时代对兽 spawn 密度的 baseline clamp。
/// beast_density_mul > 1.0 时允许超过 1.0 的有效概率（冗余 spawn 机会），
/// < 1.0 时有一定概率抑制 spawn。
// NOTE: 已集成到 spawn 决策层，binary 直接用于 era_beast_spawn_gate，暂保留 allow。
#[allow(dead_code)]
pub const ERA_BEAST_SPAWN_DENSITY_CLAMP_MAX: f64 = 2.0;

/// P1 era beast spawn 密度门控（纯函数，方便测试）。
// NOTE: 调用侧（botany/hazard.rs spawn 调度）在 spawn 决策前调用此函数；
//       binary 中目前未直接调用，allow 保留接口供集成测试/botany 调用。
#[allow(dead_code)]
///
/// 根据 `beast_density_mul`（来自 `WorldEraState::current_modifiers().beast_density_mul`）
/// 以及伪随机种子 `spawn_seed`（通常为 tick ^ zone_hash 等轻量信息），决定本次 spawn 是否放行。
///
/// 算法：
/// - 有效概率 = clamp(beast_density_mul, 0.0, ERA_BEAST_SPAWN_DENSITY_CLAMP_MAX)
/// - 若有效概率 >= 1.0，通过（Calamity 时代始终允许）
/// - 若有效概率 < 1.0，以 (spawn_seed % 1000) / 1000.0 作为随机数，小于概率则允许
///
/// 注意：`spawn_seed` 只提供粗粒度随机性，不追求密码学强度。
/// 调用侧应在 spawn 事件触发处（如 botany/hazard.rs 或 spawn scheduler）检查此函数。
pub fn era_beast_spawn_gate(beast_density_mul: f64, spawn_seed: u64) -> bool {
    let effective_prob = beast_density_mul.clamp(0.0, ERA_BEAST_SPAWN_DENSITY_CLAMP_MAX);
    if effective_prob >= 1.0 {
        return true;
    }
    // 以 spawn_seed mod 1000 / 1000.0 作为 [0, 1) 的伪随机数
    let random_val = (spawn_seed % 1000) as f64 / 1000.0;
    random_val < effective_prob
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fauna::mimic_spider::{MimicSpiderBlackboard, SpiderDisguiseState};
    use crate::npc::spawn::NpcMarker;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::DVec3;
    use valence::testing::ScenarioSingleClient;

    fn zone(spirit_qi: f64) -> Zone {
        Zone {
            name: "south_ash_dead_zone".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::ZERO, DVec3::new(100.0, 100.0, 100.0)),
            spirit_qi,
            danger_level: 5,
            active_events: vec!["no_cadence".to_string()],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    // ── P1 era beast spawn gate ────────────────────────────────────────────

    #[test]
    fn era_beast_spawn_gate_calamity_always_allows() {
        use crate::world::era::{current_modifiers, EraType, CALAMITY_BEAST_DENSITY_MUL};
        let mul = current_modifiers(EraType::Calamity).beast_density_mul;
        assert_eq!(
            mul, CALAMITY_BEAST_DENSITY_MUL,
            "灾劫时代兽密度系数应等于常数（不写字面值）"
        );
        // 灾劫时代 mul=1.5 >= 1.0，任何 seed 都应通过
        assert!(era_beast_spawn_gate(mul, 0), "seed=0 灾劫时代应允许");
        assert!(era_beast_spawn_gate(mul, 999), "seed=999 灾劫时代应允许");
        assert!(era_beast_spawn_gate(mul, 500), "seed=500 灾劫时代应允许");
    }

    #[test]
    fn era_beast_spawn_gate_unknown_era_always_allows() {
        // Unknown 时代 mul=1.0 >= 1.0，始终通过
        assert!(era_beast_spawn_gate(1.0, 0), "Unknown 时代 seed=0 应允许");
        assert!(
            era_beast_spawn_gate(1.0, 999),
            "Unknown 时代 seed=999 应允许"
        );
    }

    #[test]
    fn era_beast_spawn_gate_deduction_era_reduces_spawn_probability() {
        use crate::world::era::{current_modifiers, EraType, DEDUCTION_BEAST_DENSITY_MUL};
        let mul = current_modifiers(EraType::Deduction).beast_density_mul;
        assert_eq!(
            mul, DEDUCTION_BEAST_DENSITY_MUL,
            "演绎时代兽密度系数应等于常数（不写字面值）"
        );
        // mul=0.8，seed % 1000 / 1000.0 < 0.8 才通过
        // seed=700: 700/1000.0 = 0.7 < 0.8 → 通过
        assert!(
            era_beast_spawn_gate(mul, 700),
            "演绎时代 seed=700 (0.7 < 0.8) 应通过"
        );
        // seed=850: 850/1000.0 = 0.85 >= 0.8 → 拒绝
        assert!(
            !era_beast_spawn_gate(mul, 850),
            "演绎时代 seed=850 (0.85 >= 0.8) 应被拒"
        );
    }

    #[test]
    fn era_beast_spawn_gate_zero_density_blocks_all() {
        // mul=0.0 → 所有 seed 都被拒绝（0/1000 = 0.0 不 < 0.0）
        for seed in [0u64, 1, 100, 499, 999] {
            assert!(
                !era_beast_spawn_gate(0.0, seed),
                "mul=0.0 任何 seed={seed} 应被拒绝"
            );
        }
    }

    #[test]
    fn era_beast_spawn_gate_above_1_always_allows() {
        // mul=1.5 clamp 到不超过 ERA_BEAST_SPAWN_DENSITY_CLAMP_MAX，但仍 >= 1.0 → 始终通过
        assert!(era_beast_spawn_gate(1.5, 0));
        assert!(era_beast_spawn_gate(1.5, 999));
        assert!(
            era_beast_spawn_gate(99.0, 0),
            "超高系数 clamp 后仍 >= 1.0，应通过"
        );
    }

    #[test]
    fn dead_zone_bans_common_natural_mobs_but_keeps_whitelist() {
        let zone = zone(0.0);
        let allowed = MobSpawnFilter::default_candidates_for_zone(&zone);

        assert_eq!(
            allowed,
            vec![NaturalMobKind::AshSpider, NaturalMobKind::Daoxiang]
        );
    }

    #[test]
    fn normal_zone_keeps_common_mobs() {
        let zone = zone(0.2);
        assert!(!MobSpawnFilter::ban_in_dead_zone(
            &zone,
            NaturalMobKind::Zombie
        ));
        assert!(!MobSpawnFilter::ban_in_dead_zone(
            &zone,
            NaturalMobKind::Skeleton
        ));
        assert!(!MobSpawnFilter::ban_in_dead_zone(
            &zone,
            NaturalMobKind::Creeper
        ));
        assert!(MobSpawnFilter::default_candidates_for_zone(&zone).contains(&NaturalMobKind::Rogue));
    }

    // ── B1 生产路径验证：spawn_natural_mob_at(AshSpider) 必须附加拟态组件 ──────────

    /// 通过生产路径 spawn_natural_mob_at(AshSpider) 生成蛛，验证其携带完整拟态组件。
    ///
    /// 这是断路修复的 e2e 测试：任何回归（蛛走了 spawn_beast_npc_at 路径）都会让此测试失败，
    /// 因为 spawn_beast_npc_at 不附加 MimicSpiderBlackboard / SpiderDisguiseState。
    #[test]
    fn spawn_natural_mob_ash_spider_attaches_mimic_components_via_production_path() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spawn_pos = DVec3::new(0.0, 64.0, 0.0);
        let patrol_target = DVec3::new(5.0, 64.0, 5.0);

        let entity = spawn_natural_mob_at(
            &mut app.world_mut().commands(),
            layer,
            NaturalMobKind::AshSpider,
            "spawn",
            spawn_pos,
            patrol_target,
        );
        app.world_mut().flush();

        let entity = entity.expect("spawn_natural_mob_at(AshSpider) 不应返回 None");

        // 验证 NpcMarker（NPC 标记）
        assert!(
            app.world().get::<NpcMarker>(entity).is_some(),
            "经生产路径生成的拟态蛛必须带 NpcMarker（实际缺失 — 说明走错了 spawn 路径）"
        );

        // 验证 SpiderDisguiseState（拟态状态机）
        assert_eq!(
            app.world().get::<SpiderDisguiseState>(entity),
            Some(&SpiderDisguiseState::Disguised),
            "经生产路径生成的拟态蛛初始状态必须是 Disguised（实际缺失或状态错误）\n\
             — 若 SpiderDisguiseState 缺失，说明走了 spawn_beast_npc_at 而非 spawn_ash_spider_npc_at"
        );

        // 验证 MimicSpiderBlackboard（感知 / drained_qi 追踪）
        let blackboard = app
            .world()
            .get::<MimicSpiderBlackboard>(entity)
            .expect("经生产路径生成的拟态蛛必须带 MimicSpiderBlackboard（缺失说明走错路径）");
        assert_eq!(
            blackboard.home_zone, "spawn",
            "blackboard.home_zone 应与传入 zone 一致（期望 spawn，实际 {}）",
            blackboard.home_zone
        );
    }

    /// 验证生产路径对非 AshSpider 种类也能正常返回 Some(entity)（不 panic）。
    #[test]
    fn spawn_natural_mob_non_spider_returns_some_entity() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spawn_pos = DVec3::new(10.0, 64.0, 10.0);
        let patrol_target = DVec3::new(15.0, 64.0, 15.0);

        // 非 AshSpider 应走 spawn_beast_npc_at，不带拟态组件
        let entity = spawn_natural_mob_at(
            &mut app.world_mut().commands(),
            layer,
            NaturalMobKind::Zombie,
            "spawn",
            spawn_pos,
            patrol_target,
        );
        app.world_mut().flush();

        assert!(
            entity.is_some(),
            "spawn_natural_mob_at(Zombie) 应返回 Some(entity)，实际为 None"
        );

        // 非 AshSpider 不应带 SpiderDisguiseState（防止误挂）
        assert!(
            app.world()
                .get::<SpiderDisguiseState>(entity.unwrap())
                .is_none(),
            "非 AshSpider 种类不应携带 SpiderDisguiseState（误挂会导致 tick 系统空转）"
        );
    }
}
