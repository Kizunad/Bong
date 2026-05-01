//! 死域自然刷怪过滤规则。

use crate::cultivation::dead_zone::is_dead_zone;
use crate::world::zone::Zone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaturalMobKind {
    Zombie,
    Skeleton,
    Creeper,
    AshSpider,
    Daoxiang,
}

pub const DEFAULT_NATURAL_MOB_CANDIDATES: [NaturalMobKind; 3] = [
    NaturalMobKind::Zombie,
    NaturalMobKind::Skeleton,
    NaturalMobKind::Creeper,
];

pub const DEAD_ZONE_MOB_WHITELIST: [NaturalMobKind; 2] =
    [NaturalMobKind::AshSpider, NaturalMobKind::Daoxiang];

pub struct MobSpawnFilter;

impl MobSpawnFilter {
    pub fn ban_in_dead_zone(zone: &Zone, mob: NaturalMobKind) -> bool {
        is_dead_zone(zone) && !DEAD_ZONE_MOB_WHITELIST.contains(&mob)
    }

    pub fn default_candidates_for_zone(zone: &Zone) -> Vec<NaturalMobKind> {
        DEFAULT_NATURAL_MOB_CANDIDATES
            .into_iter()
            .chain(DEAD_ZONE_MOB_WHITELIST)
            .filter(|mob| !Self::ban_in_dead_zone(zone, *mob))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::DVec3;

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
    }
}
