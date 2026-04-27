use serde::{Deserialize, Serialize};

use crate::schema::inventory::EquipSlotV1;

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
