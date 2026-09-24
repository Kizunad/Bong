//! plan-sword-path-v1 — 剑道五招的行为参数。
//!
//! 境界、消耗、施法、冷却与射程统一由 `TechniqueRegistry` 提供；本模块只保留不属于
//! 通用功法 metadata 的伤害、状态效果与剑意染色参数。

pub mod effects {
    pub const CONDENSE_EDGE_DAMAGE_MULT: f32 = 1.8;
    pub const CONDENSE_EDGE_ARMOR_PIERCE: f32 = 0.30;
    pub const CONDENSE_EDGE_DURATION_TICKS: u32 = 5 * 20;

    pub const QI_SLASH_ATTENUATION_PER_BLOCK: f64 = 0.03;

    pub const RESONANCE_SLOW_MIN_SECS: f32 = 3.0;
    pub const RESONANCE_SLOW_MAX_SECS: f32 = 5.0;

    pub const MANIFEST_ATTACK_MULT: f32 = 2.0;
    pub const MANIFEST_DURATION_TICKS: u32 = 5 * 20;
    pub const MANIFEST_BOND_PENALTY: f32 = 0.1;

    pub const HEAVEN_GATE_DEFENSE_IGNORE: f32 = 0.50;
    pub const HEAVEN_GATE_BLIND_ZONE_TTL_TICKS: u64 = 5 * 60 * 20;
    pub const HEAVEN_GATE_QI_MAX_RETAIN: f64 = 0.1;
}

pub mod coloring {
    pub struct ColorWeight {
        pub solid: f32,
        pub keen: f32,
    }

    pub fn practice_weight(technique_id: &str) -> Option<ColorWeight> {
        match technique_id {
            "sword_path.condense_edge" => Some(ColorWeight {
                solid: 1.0,
                keen: 0.0,
            }),
            "sword_path.qi_slash" => Some(ColorWeight {
                solid: 2.0,
                keen: 1.0,
            }),
            "sword_path.resonance" => Some(ColorWeight {
                solid: 3.0,
                keen: 0.0,
            }),
            "sword_path.manifest" => Some(ColorWeight {
                solid: 4.0,
                keen: 2.0,
            }),
            "sword_path.heaven_gate" => Some(ColorWeight {
                solid: 0.0,
                keen: 50.0,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_condense_edge_damage_mult() {
        let v = effects::CONDENSE_EDGE_DAMAGE_MULT;
        assert!(
            v > 1.0,
            "CONDENSE_EDGE_DAMAGE_MULT should be > 1.0, got {v}"
        );
    }

    #[test]
    fn effects_armor_pierce_in_zero_one() {
        let v = effects::CONDENSE_EDGE_ARMOR_PIERCE;
        assert!(
            v > 0.0 && v < 1.0,
            "ARMOR_PIERCE should be in (0,1), got {v}"
        );
    }

    #[test]
    fn effects_qi_slash_attenuation_positive() {
        let v = effects::QI_SLASH_ATTENUATION_PER_BLOCK;
        assert!(v > 0.0, "QI_SLASH_ATTENUATION should be > 0, got {v}");
    }

    #[test]
    fn effects_resonance_slow_range_valid() {
        let lo = effects::RESONANCE_SLOW_MIN_SECS;
        let hi = effects::RESONANCE_SLOW_MAX_SECS;
        assert!(lo < hi, "RESONANCE_SLOW min({lo}) should be < max({hi})");
    }

    #[test]
    fn effects_manifest_attack_mult() {
        let v = effects::MANIFEST_ATTACK_MULT;
        assert!(v > 1.0, "MANIFEST_ATTACK_MULT should be > 1.0, got {v}");
    }

    #[test]
    fn effects_heaven_gate_defense_ignore_in_zero_one() {
        let v = effects::HEAVEN_GATE_DEFENSE_IGNORE;
        assert!(
            v > 0.0 && v < 1.0,
            "DEFENSE_IGNORE should be in (0,1), got {v}"
        );
    }

    #[test]
    fn color_weights_all_resolver_ids_covered() {
        let registry = crate::cultivation::known_techniques::TechniqueRegistry::load_for_tests();
        let ids: Vec<&str> = registry
            .iter()
            .filter(|definition| definition.id.starts_with("sword_path."))
            .map(|definition| definition.id.as_str())
            .collect();
        assert!(
            !ids.is_empty(),
            "registry must contain sword_path techniques"
        );
        for id in ids {
            assert!(
                coloring::practice_weight(id).is_some(),
                "technique {id} should have color weight"
            );
        }
    }

    #[test]
    fn color_weights_condense_no_keen() {
        let w = coloring::practice_weight("sword_path.condense_edge").unwrap();
        assert!(w.keen.abs() < 1e-6, "凝锋 should have 0 keen weight");
    }

    #[test]
    fn color_weights_heaven_gate_heavy_keen() {
        let w = coloring::practice_weight("sword_path.heaven_gate").unwrap();
        assert!(
            (w.keen - 50.0).abs() < 1e-6,
            "一剑开天 should have 50 keen weight"
        );
    }

    #[test]
    fn unknown_technique_returns_none() {
        assert!(coloring::practice_weight("nonexistent").is_none());
    }
}
