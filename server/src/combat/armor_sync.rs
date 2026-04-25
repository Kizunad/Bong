//! plan-armor-v1 §1.3 — 装备护甲 → DerivedAttrs.defense_profile 同步。

use std::collections::HashMap;

use valence::prelude::{Changed, Query, Res};

use crate::combat::armor::{ArmorProfileRegistry, ARMOR_MITIGATION_CAP};
use crate::combat::components::{BodyPart, DerivedAttrs, WoundKind};
use crate::inventory::{
    PlayerInventory, EQUIP_SLOT_CHEST, EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS,
};

/// plan-armor-v1 §1.3：每当装备变化，重新聚合护甲二维矩阵。
///
/// 聚合规则：同 `(BodyPart, WoundKind)` 多件覆盖时取最大，不叠加。
pub fn sync_armor_to_derived_attrs(
    mut query: Query<(&PlayerInventory, &mut DerivedAttrs), Changed<PlayerInventory>>,
    armor_profiles: Res<ArmorProfileRegistry>,
) {
    for (inv, mut derived) in &mut query {
        let mut profile: HashMap<(BodyPart, WoundKind), f32> = HashMap::new();

        // MVP 只读四个护甲槽。
        for slot in [
            EQUIP_SLOT_HEAD,
            EQUIP_SLOT_CHEST,
            EQUIP_SLOT_LEGS,
            EQUIP_SLOT_FEET,
        ] {
            let Some(item) = inv.equipped.get(slot) else {
                continue;
            };
            let Some(ap) = armor_profiles.get(item.template_id.as_str()) else {
                continue;
            };

            let effective_mul = ap.effective_multiplier_for_durability_ratio(item.durability);
            for body in &ap.body_coverage {
                for (kind, mitigation) in &ap.kind_mitigation {
                    let m = (mitigation * effective_mul).clamp(0.0, ARMOR_MITIGATION_CAP);
                    if m <= 0.0 {
                        continue;
                    }
                    profile
                        .entry((*body, *kind))
                        .and_modify(|existing| *existing = existing.max(m))
                        .or_insert(m);
                }
            }
        }

        derived.defense_profile = profile;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::armor::ArmorProfile;
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity};
    use crate::schema::inventory::EquipSlotV1;
    use valence::prelude::{App, Update};

    fn make_item(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "fake_spirit_hide".to_string(),
            display_name: "fake_spirit_hide".to_string(),
            grid_w: 2,
            grid_h: 2,
            weight: 1.8,
            rarity: ItemRarity::Rare,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.8,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
        }
    }

    #[test]
    fn sync_sets_defense_profile_for_equipped_armor() {
        let mut app = App::new();
        app.insert_resource(ArmorProfileRegistry::from_map(HashMap::from([(
            "fake_spirit_hide".to_string(),
            ArmorProfile {
                slot: EquipSlotV1::Chest,
                body_coverage: vec![BodyPart::Chest, BodyPart::Abdomen],
                kind_mitigation: HashMap::from([(WoundKind::Cut, 0.25)]),
                durability_max: 10,
                broken_multiplier: 0.3,
            },
        )])));
        app.add_systems(Update, sync_armor_to_derived_attrs);

        let mut equipped = HashMap::new();
        equipped.insert(EQUIP_SLOT_CHEST.to_string(), make_item(42));
        let entity = app
            .world_mut()
            .spawn((
                PlayerInventory {
                    revision: InventoryRevision(0),
                    containers: vec![],
                    equipped,
                    hotbar: Default::default(),
                    bone_coins: 0,
                    max_weight: 45.0,
                },
                DerivedAttrs::default(),
            ))
            .id();

        // Changed<PlayerInventory> 需要一次 mutation 才触发。
        {
            let world = app.world_mut();
            let mut entity_mut = world.entity_mut(entity);
            let mut inv = entity_mut.get_mut::<PlayerInventory>().unwrap();
            inv.revision = InventoryRevision(inv.revision.0.saturating_add(1));
        }
        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert_eq!(
            attrs
                .defense_profile
                .get(&(BodyPart::Chest, WoundKind::Cut)),
            Some(&0.25)
        );
        assert_eq!(
            attrs
                .defense_profile
                .get(&(BodyPart::Abdomen, WoundKind::Cut)),
            Some(&0.25)
        );
    }

    #[test]
    fn sync_applies_broken_multiplier_when_item_durability_zero() {
        let mut app = App::new();
        app.insert_resource(ArmorProfileRegistry::from_map(HashMap::from([(
            "fake_spirit_hide".to_string(),
            ArmorProfile {
                slot: EquipSlotV1::Chest,
                body_coverage: vec![BodyPart::Chest],
                kind_mitigation: HashMap::from([(WoundKind::Cut, 0.5)]),
                durability_max: 10,
                broken_multiplier: 0.3,
            },
        )])));
        app.add_systems(Update, sync_armor_to_derived_attrs);

        let mut item = make_item(7);
        item.durability = 0.0;
        let mut equipped = HashMap::new();
        equipped.insert(EQUIP_SLOT_CHEST.to_string(), item);
        let entity = app
            .world_mut()
            .spawn((
                PlayerInventory {
                    revision: InventoryRevision(0),
                    containers: vec![],
                    equipped,
                    hotbar: Default::default(),
                    bone_coins: 0,
                    max_weight: 45.0,
                },
                DerivedAttrs::default(),
            ))
            .id();

        // Changed<PlayerInventory> 需要一次 mutation 才触发。
        {
            let world = app.world_mut();
            let mut entity_mut = world.entity_mut(entity);
            let mut inv = entity_mut.get_mut::<PlayerInventory>().unwrap();
            inv.revision = InventoryRevision(inv.revision.0.saturating_add(1));
        }
        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        // 0.5 mitigation × 0.3 broken_multiplier
        assert_eq!(
            attrs
                .defense_profile
                .get(&(BodyPart::Chest, WoundKind::Cut)),
            Some(&0.15)
        );
    }
}
