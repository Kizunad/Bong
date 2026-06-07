//! 死域自然刷怪过滤规则与生产生成调度。

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
