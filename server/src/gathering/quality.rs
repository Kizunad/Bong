use super::tools::{GatheringMaterial, QualityBonus};
use crate::cultivation::components::Realm;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatheringQuality {
    Normal,
    Fine,
    Perfect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityChances {
    pub normal: f32,
    pub fine: f32,
    pub perfect: f32,
}

impl GatheringQuality {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Fine => "fine",
            Self::Perfect => "perfect",
        }
    }

    pub fn yield_multiplier(self) -> f32 {
        match self {
            Self::Normal => 1.0,
            Self::Fine => 1.5,
            Self::Perfect => 2.0,
        }
    }

    pub fn has_rare_bonus(self) -> bool {
        self == Self::Perfect
    }
}

pub fn quality_chances(material: Option<GatheringMaterial>, realm: Realm) -> QualityChances {
    let QualityBonus { fine, perfect } =
        material
            .map(GatheringMaterial::quality_bonus)
            .unwrap_or(QualityBonus {
                fine: 0.0,
                perfect: 0.0,
            });
    let rank = realm_rank(realm);
    let perfect = (0.05 + perfect + rank as f32 * 0.005).clamp(0.0, 0.40);
    let fine = (0.25 + fine + rank as f32 * 0.02).clamp(0.0, 0.85 - perfect);
    let normal = (1.0 - fine - perfect).clamp(0.0, 1.0);
    QualityChances {
        normal,
        fine,
        perfect,
    }
}

pub fn roll_quality(
    seed: u64,
    material: Option<GatheringMaterial>,
    realm: Realm,
) -> GatheringQuality {
    let chances = quality_chances(material, realm);
    let roll = unit_interval(seed);
    if roll < chances.perfect {
        GatheringQuality::Perfect
    } else if roll < chances.perfect + chances.fine {
        GatheringQuality::Fine
    } else {
        GatheringQuality::Normal
    }
}

pub fn quality_hint(material: Option<GatheringMaterial>, realm: Realm) -> &'static str {
    let chances = quality_chances(material, realm);
    if chances.perfect >= 0.08 {
        "perfect_possible"
    } else if chances.fine >= 0.30 {
        "fine_likely"
    } else {
        "normal"
    }
}

fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

fn unit_interval(seed: u64) -> f32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z as f64 / u64::MAX as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copper_and_realm_raise_quality_chances() {
        let bare = quality_chances(None, Realm::Awaken);
        let copper_void = quality_chances(Some(GatheringMaterial::Copper), Realm::Void);

        assert!((bare.normal - 0.70).abs() < 0.0001);
        assert!((bare.fine - 0.25).abs() < 0.0001);
        assert!((bare.perfect - 0.05).abs() < 0.0001);
        assert!(copper_void.fine > bare.fine);
        assert!(copper_void.perfect > bare.perfect);
        assert!((copper_void.normal + copper_void.fine + copper_void.perfect - 1.0).abs() < 0.0001);
    }

    #[test]
    fn quality_yield_rules_match_plan() {
        assert_eq!(GatheringQuality::Normal.yield_multiplier(), 1.0);
        assert_eq!(GatheringQuality::Fine.yield_multiplier(), 1.5);
        assert_eq!(GatheringQuality::Perfect.yield_multiplier(), 2.0);
        assert!(GatheringQuality::Perfect.has_rare_bonus());
        assert!(!GatheringQuality::Fine.has_rare_bonus());
    }

    #[test]
    fn roll_quality_is_deterministic() {
        let first = roll_quality(42, Some(GatheringMaterial::Copper), Realm::Condense);
        let second = roll_quality(42, Some(GatheringMaterial::Copper), Realm::Condense);
        assert_eq!(first, second);
    }
}
