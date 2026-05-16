//! plan-sword-path-v1 P0 — 剑道五招定义与注册。

use crate::cultivation::components::Realm;

#[derive(Debug, Clone)]
pub struct SwordTechniqueDef {
    pub id: &'static str,
    pub display_name: &'static str,
    pub required_realm: Realm,
    pub qi_cost: f64,
    pub stamina_cost: f32,
    pub cast_ticks: u32,
    pub cooldown_ticks: u32,
    pub range: f32,
}

pub const CONDENSE_EDGE: SwordTechniqueDef = SwordTechniqueDef {
    id: "sword_path.condense_edge",
    display_name: "剑意·凝锋",
    required_realm: Realm::Induce,
    qi_cost: 0.0,
    stamina_cost: 8.0,
    cast_ticks: 12,
    cooldown_ticks: 40,
    range: 4.0,
};

pub const QI_SLASH: SwordTechniqueDef = SwordTechniqueDef {
    id: "sword_path.qi_slash",
    display_name: "剑气·斩",
    required_realm: Realm::Condense,
    qi_cost: 3.0,
    stamina_cost: 12.0,
    cast_ticks: 20,
    cooldown_ticks: 60,
    range: 8.0,
};

pub const RESONANCE: SwordTechniqueDef = SwordTechniqueDef {
    id: "sword_path.resonance",
    display_name: "共鸣·剑鸣",
    required_realm: Realm::Solidify,
    qi_cost: 20.0,
    stamina_cost: 15.0,
    cast_ticks: 30,
    cooldown_ticks: 120,
    range: 6.0,
};

pub const MANIFEST: SwordTechniqueDef = SwordTechniqueDef {
    id: "sword_path.manifest",
    display_name: "归一·剑意化形",
    required_realm: Realm::Spirit,
    qi_cost: 40.0,
    stamina_cost: 20.0,
    cast_ticks: 40,
    cooldown_ticks: 200,
    range: 5.0,
};

pub const HEAVEN_GATE: SwordTechniqueDef = SwordTechniqueDef {
    id: "sword_path.heaven_gate",
    display_name: "天门·一剑开天",
    required_realm: Realm::Void,
    qi_cost: f64::INFINITY,
    stamina_cost: 0.0,
    cast_ticks: 80,
    cooldown_ticks: u32::MAX,
    range: 100.0,
};

pub const ALL_TECHNIQUES: [&SwordTechniqueDef; 5] = [
    &CONDENSE_EDGE,
    &QI_SLASH,
    &RESONANCE,
    &MANIFEST,
    &HEAVEN_GATE,
];

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

    pub const HEAVEN_GATE_RADIUS: f64 = 100.0;
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
    fn all_techniques_count() {
        assert_eq!(ALL_TECHNIQUES.len(), 5);
    }

    #[test]
    fn technique_ids_unique() {
        let ids: Vec<&str> = ALL_TECHNIQUES.iter().map(|t| t.id).collect();
        let mut dedup = ids.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(ids.len(), dedup.len(), "technique ids must be unique");
    }

    #[test]
    fn realm_gates_ascending() {
        let realms: Vec<u8> = ALL_TECHNIQUES
            .iter()
            .map(|t| match t.required_realm {
                Realm::Awaken => 0,
                Realm::Induce => 1,
                Realm::Condense => 2,
                Realm::Solidify => 3,
                Realm::Spirit => 4,
                Realm::Void => 5,
            })
            .collect();
        for pair in realms.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "techniques should require ascending realms"
            );
        }
    }

    #[test]
    fn condense_edge_no_qi() {
        assert!(
            CONDENSE_EDGE.qi_cost.abs() < 1e-6,
            "凝锋 should cost 0 qi, got {}",
            CONDENSE_EDGE.qi_cost
        );
    }

    #[test]
    fn qi_slash_low_qi() {
        assert!(
            (QI_SLASH.qi_cost - 3.0).abs() < 1e-6,
            "剑气斩 should cost 3 qi, got {}",
            QI_SLASH.qi_cost
        );
    }

    #[test]
    fn resonance_qi_for_solidify() {
        assert!(
            (RESONANCE.qi_cost - 20.0).abs() < 1e-6,
            "剑鸣 should cost 20 qi, got {}",
            RESONANCE.qi_cost
        );
        assert_eq!(RESONANCE.required_realm, Realm::Solidify);
    }

    #[test]
    fn manifest_qi_for_spirit() {
        assert!(
            (MANIFEST.qi_cost - 40.0).abs() < 1e-6,
            "剑意化形 should cost 40 qi, got {}",
            MANIFEST.qi_cost
        );
        assert_eq!(MANIFEST.required_realm, Realm::Spirit);
    }

    #[test]
    fn heaven_gate_costs_all_qi() {
        assert!(
            HEAVEN_GATE.qi_cost.is_infinite(),
            "一剑开天 should cost ALL qi (infinity sentinel)"
        );
        assert_eq!(HEAVEN_GATE.required_realm, Realm::Void);
    }

    #[test]
    fn heaven_gate_one_shot() {
        assert_eq!(
            HEAVEN_GATE.cooldown_ticks,
            u32::MAX,
            "一剑开天 should be one-shot (u32::MAX cooldown)"
        );
    }

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
    fn effects_heaven_gate_radius_positive() {
        let v = effects::HEAVEN_GATE_RADIUS;
        assert!(v > 0.0, "HEAVEN_GATE_RADIUS should be > 0, got {v}");
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
    fn color_weights_all_techniques_covered() {
        for tech in ALL_TECHNIQUES {
            assert!(
                coloring::practice_weight(tech.id).is_some(),
                "technique {} should have color weight",
                tech.id
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
