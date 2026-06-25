use serde::{Deserialize, Serialize};

use crate::schema::inventory::EquipSlotV1;

pub(crate) const ARMOR_DURABILITY_RATIO_MIN: f64 = 0.0;
pub(crate) const ARMOR_DURABILITY_RATIO_MAX: f64 = 1.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ArmorDurabilityChangedV1 {
    pub v: u8,
    pub entity_id: String,
    pub slot: EquipSlotV1,
    pub instance_id: u64,
    pub template_id: String,
    pub cur: f64,
    pub max: f64,
    pub durability_ratio: f64,
    pub broken: bool,
}

impl ArmorDurabilityChangedV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.v != 1 {
            return Err(format!(
                "ArmorDurabilityChangedV1.v must be 1, got {}",
                self.v
            ));
        }
        if self.entity_id.is_empty() {
            return Err("ArmorDurabilityChangedV1.entity_id must be non-empty".to_string());
        }
        if self.template_id.is_empty() {
            return Err("ArmorDurabilityChangedV1.template_id must be non-empty".to_string());
        }
        if self.cur < 0.0 {
            return Err(format!(
                "ArmorDurabilityChangedV1.cur must be >= 0, got {}",
                self.cur
            ));
        }
        if self.max < 0.0 {
            return Err(format!(
                "ArmorDurabilityChangedV1.max must be >= 0, got {}",
                self.max
            ));
        }
        if !(ARMOR_DURABILITY_RATIO_MIN..=ARMOR_DURABILITY_RATIO_MAX)
            .contains(&self.durability_ratio)
        {
            return Err(format!(
                "ArmorDurabilityChangedV1.durability_ratio must be {ARMOR_DURABILITY_RATIO_MIN}..={ARMOR_DURABILITY_RATIO_MAX}, got {}",
                self.durability_ratio
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_durability_changed_roundtrip() {
        let event = ArmorDurabilityChangedV1 {
            v: 1,
            entity_id: "offline:Crimson".to_string(),
            slot: EquipSlotV1::Chest,
            instance_id: 88,
            template_id: "fake_spirit_hide".to_string(),
            cur: 0.0,
            max: 100.0,
            durability_ratio: 0.0,
            broken: true,
        };

        let json = serde_json::to_string(&event).expect("serialize armor durability event");
        let back: ArmorDurabilityChangedV1 =
            serde_json::from_str(json.as_str()).expect("deserialize armor durability event");

        assert_eq!(back.v, 1);
        assert_eq!(back.entity_id, "offline:Crimson");
        assert_eq!(back.slot, EquipSlotV1::Chest);
        assert_eq!(back.template_id, "fake_spirit_hide");
        assert!(back.broken);
    }
}
