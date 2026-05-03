//! plan-poi-novice-v1 — 异变兽巢 zombie 占位配置。

use valence::prelude::{bevy_ecs, Component};

pub const MUTANT_NEST_ZOMBIE_COUNT: u8 = 3;
pub const MUTANT_NEST_ZOMBIE_HP: f32 = 60.0;
pub const MUTANT_NEST_RECOMMENDED_REALM: &str = "condense";

#[derive(Debug, Clone, Copy, Component, PartialEq)]
pub struct MutantNestSpawnConfig {
    pub zombie_count: u8,
    pub zombie_hp: f32,
    pub damage_multiplier: f32,
}

impl Default for MutantNestSpawnConfig {
    fn default() -> Self {
        Self {
            zombie_count: MUTANT_NEST_ZOMBIE_COUNT,
            zombie_hp: MUTANT_NEST_ZOMBIE_HP,
            damage_multiplier: 1.6,
        }
    }
}

impl MutantNestSpawnConfig {
    pub fn total_hp(self) -> f32 {
        self.zombie_hp * f32::from(self.zombie_count)
    }

    pub fn is_induce_group_content(self, induce_qi_max: f32) -> bool {
        self.total_hp() > induce_qi_max * 3.0
    }
}

pub fn log_mutant_nest_contract() {
    let config = MutantNestSpawnConfig::default();
    tracing::debug!(
        "[bong][poi-novice] mutant nest contract zombies={} hp={} total_hp={} recommended_realm={} induce_group_content={}",
        config.zombie_count,
        config.zombie_hp,
        config.total_hp(),
        MUTANT_NEST_RECOMMENDED_REALM,
        config.is_induce_group_content(40.0)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mutant_nest_matches_q117_high_difficulty() {
        let cfg = MutantNestSpawnConfig::default();
        assert_eq!(cfg.zombie_count, 3);
        assert_eq!(cfg.zombie_hp, 60.0);
        assert_eq!(cfg.total_hp(), 180.0);
        assert!(cfg.is_induce_group_content(40.0));
    }
}
