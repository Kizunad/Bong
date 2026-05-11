//! Audio implementation v1 recipe routing helpers.

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Entity, Resource};

use crate::cultivation::components::Realm;

pub const AUDIO_DEDUP_WINDOW_TICKS: u64 = 2;

#[derive(Debug, Default, Resource)]
pub struct AudioImplementationDedup {
    last_emit_tick: HashMap<(Entity, &'static str), u64>,
}

impl AudioImplementationDedup {
    pub fn should_emit(&mut self, entity: Entity, recipe_id: &'static str, tick: u64) -> bool {
        let key = (entity, recipe_id);
        if self
            .last_emit_tick
            .get(&key)
            .is_some_and(|last| tick.saturating_sub(*last) < AUDIO_DEDUP_WINDOW_TICKS)
        {
            return false;
        }
        self.last_emit_tick.insert(key, tick);
        true
    }
}

pub fn combat_hit_recipe(damage: f32, critical: bool) -> &'static str {
    if critical || damage >= 24.0 {
        "hit_critical"
    } else if damage >= 10.0 {
        "hit_heavy"
    } else {
        "hit_light"
    }
}

pub fn parry_recipe(effectiveness: f32) -> &'static str {
    if effectiveness >= 0.85 {
        "parry_perfect"
    } else {
        "parry_success"
    }
}

pub fn breakthrough_recipe(to: Realm) -> &'static str {
    match to {
        Realm::Induce | Realm::Awaken => "breakthrough_yinqi",
        Realm::Condense => "breakthrough_ningmai",
        Realm::Solidify | Realm::Spirit | Realm::Void => "breakthrough_guyuan",
    }
}

pub fn school_recipe_prefix(skill_id: &str) -> &'static str {
    let normalized = skill_id.to_ascii_lowercase();
    if normalized.contains("baomai")
        || normalized.contains("bao_mai")
        || normalized.contains("xue_beng")
    {
        "baomai"
    } else if normalized.contains("tuike") || normalized.contains("false_skin") {
        "tuike"
    } else if normalized.contains("woliu") || normalized.contains("vortex") {
        "woliu"
    } else if normalized.contains("zhenfa") || normalized.contains("formation") {
        "zhenfa"
    } else if normalized.contains("zhenmai") || normalized.contains("jiemai") {
        "zhenmai"
    } else if normalized.contains("poison") || normalized.contains("gu") {
        "dugu_poison"
    } else {
        "dugu"
    }
}

pub fn school_hit_recipe(skill_id: &str, damage: f32, critical: bool) -> &'static str {
    let tier = if critical || damage >= 24.0 {
        "critical"
    } else if damage >= 10.0 {
        "heavy"
    } else {
        "light"
    };
    match (school_recipe_prefix(skill_id), tier) {
        ("baomai", "light") => "baomai_hit_light",
        ("baomai", "heavy") => "baomai_hit_heavy",
        ("baomai", "critical") => "baomai_hit_critical",
        ("dugu", "light") => "dugu_hit_light",
        ("dugu", "heavy") => "dugu_hit_heavy",
        ("dugu", "critical") => "dugu_hit_critical",
        ("dugu_poison", "light") => "dugu_poison_hit_light",
        ("dugu_poison", "heavy") => "dugu_poison_hit_heavy",
        ("dugu_poison", "critical") => "dugu_poison_hit_critical",
        ("tuike", "light") => "tuike_hit_light",
        ("tuike", "heavy") => "tuike_hit_heavy",
        ("tuike", "critical") => "tuike_hit_critical",
        ("woliu", "light") => "woliu_hit_light",
        ("woliu", "heavy") => "woliu_hit_heavy",
        ("woliu", "critical") => "woliu_hit_critical",
        ("zhenfa", "light") => "zhenfa_hit_light",
        ("zhenfa", "heavy") => "zhenfa_hit_heavy",
        ("zhenfa", "critical") => "zhenfa_hit_critical",
        ("zhenmai", "light") => "zhenmai_hit_light",
        ("zhenmai", "heavy") => "zhenmai_hit_heavy",
        ("zhenmai", "critical") => "zhenmai_hit_critical",
        _ => combat_hit_recipe(damage, critical),
    }
}

pub fn forge_hammer_recipe(heavy: bool) -> &'static str {
    if heavy {
        "forge_hammer_heavy"
    } else {
        "forge_hammer_light"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;

    #[test]
    fn combat_damage_selects_expected_recipe() {
        assert_eq!(combat_hit_recipe(3.0, false), "hit_light");
        assert_eq!(combat_hit_recipe(12.0, false), "hit_heavy");
        assert_eq!(combat_hit_recipe(24.0, false), "hit_critical");
        assert_eq!(combat_hit_recipe(5.0, true), "hit_critical");
    }

    #[test]
    fn breakthrough_realm_selects_recipe_family() {
        assert_eq!(breakthrough_recipe(Realm::Induce), "breakthrough_yinqi");
        assert_eq!(breakthrough_recipe(Realm::Condense), "breakthrough_ningmai");
        assert_eq!(breakthrough_recipe(Realm::Solidify), "breakthrough_guyuan");
    }

    #[test]
    fn school_routes_to_correct_recipe_prefix() {
        assert_eq!(school_recipe_prefix("xue_beng_bu"), "baomai");
        assert_eq!(school_recipe_prefix("woliu_vortex_shield"), "woliu");
        assert_eq!(school_recipe_prefix("zhenmai_parry"), "zhenmai");
        assert_eq!(
            school_hit_recipe("poison_gu_bite", 30.0, false),
            "dugu_poison_hit_critical"
        );
    }

    #[test]
    fn dedup_blocks_same_entity_recipe_inside_window() {
        let mut dedup = AudioImplementationDedup::default();
        let entity = Entity::from_raw(1);

        assert!(dedup.should_emit(entity, "hit_light", 10));
        assert!(!dedup.should_emit(entity, "hit_light", 11));
        assert!(dedup.should_emit(entity, "hit_light", 12));
        assert!(dedup.should_emit(Entity::from_raw(2), "hit_light", 11));
    }
}
