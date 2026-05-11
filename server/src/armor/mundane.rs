//! plan-armor-visual-v1 — 6 套凡物盔甲规格、profile 注册与 hand-craft 配方。

use std::collections::HashMap;

use crate::combat::armor::{ArmorProfile, ArmorProfileRegistry, ARMOR_MITIGATION_CAP};
use crate::combat::components::{BodyPart, WoundKind};
use crate::craft::{
    CraftCategory, CraftRecipe, CraftRegistry, CraftRequirements, RecipeId, RegistryError,
    UnlockSource,
};
use crate::schema::inventory::EquipSlotV1;

const CRAFT_TIME_TICKS: u64 = 40 * 20;
const BONE_COIN_TEMPLATE: &str = "bone_coin_5";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MundaneArmorMaterial {
    Bone,
    Hide,
    Iron,
    Copper,
    SpiritCloth,
    ScrollWrap,
}

impl MundaneArmorMaterial {
    pub const ALL: [Self; 6] = [
        Self::Bone,
        Self::Hide,
        Self::Iron,
        Self::Copper,
        Self::SpiritCloth,
        Self::ScrollWrap,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Bone => "bone",
            Self::Hide => "hide",
            Self::Iron => "iron",
            Self::Copper => "copper",
            Self::SpiritCloth => "spirit_cloth",
            Self::ScrollWrap => "scroll_wrap",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Bone => "骨甲",
            Self::Hide => "兽皮甲",
            Self::Iron => "铁甲",
            Self::Copper => "铜甲",
            Self::SpiritCloth => "灵布衫",
            Self::ScrollWrap => "残卷缠甲",
        }
    }

    pub fn defense(self) -> f32 {
        match self {
            Self::Bone => 3.0,
            Self::Hide => 5.0,
            Self::Iron => 8.0,
            Self::Copper => 7.0,
            Self::SpiritCloth => 4.0,
            Self::ScrollWrap => 6.0,
        }
    }

    pub fn durability_max(self) -> u32 {
        match self {
            Self::Bone => 80,
            Self::Hide => 120,
            Self::Iron => 200,
            Self::Copper => 160,
            Self::SpiritCloth => 100,
            Self::ScrollWrap => 140,
        }
    }

    fn base_materials(self) -> &'static [(&'static str, u32)] {
        match self {
            Self::Bone => &[(BONE_COIN_TEMPLATE, 6)],
            Self::Hide => &[("raw_beast_hide", 4), (BONE_COIN_TEMPLATE, 2)],
            Self::Iron => &[("iron_ore", 5), (BONE_COIN_TEMPLATE, 3)],
            Self::Copper => &[("copper_ore", 4), ("raw_beast_hide", 2)],
            Self::SpiritCloth => &[("spirit_cloth", 3), ("spirit_grass", 2)],
            Self::ScrollWrap => &[("scroll_fragment", 4), (BONE_COIN_TEMPLATE, 4)],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MundaneArmorSlot {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

impl MundaneArmorSlot {
    pub const ALL: [Self; 4] = [Self::Helmet, Self::Chestplate, Self::Leggings, Self::Boots];

    pub fn item_suffix(self) -> &'static str {
        match self {
            Self::Helmet => "helmet",
            Self::Chestplate => "chestplate",
            Self::Leggings => "leggings",
            Self::Boots => "boots",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Helmet => "盔",
            Self::Chestplate => "胸甲",
            Self::Leggings => "腿甲",
            Self::Boots => "靴",
        }
    }

    pub fn equip_slot(self) -> EquipSlotV1 {
        match self {
            Self::Helmet => EquipSlotV1::Head,
            Self::Chestplate => EquipSlotV1::Chest,
            Self::Leggings => EquipSlotV1::Legs,
            Self::Boots => EquipSlotV1::Feet,
        }
    }

    pub fn defense_ratio(self) -> f32 {
        match self {
            Self::Chestplate => 0.40,
            Self::Leggings => 0.30,
            Self::Helmet | Self::Boots => 0.15,
        }
    }

    fn material_ratio(self) -> u32 {
        match self {
            Self::Helmet => 2,
            Self::Chestplate => 3,
            Self::Leggings => 2,
            Self::Boots => 2,
        }
    }

    fn body_coverage(self) -> Vec<BodyPart> {
        match self {
            Self::Helmet => vec![BodyPart::Head],
            Self::Chestplate => vec![
                BodyPart::Chest,
                BodyPart::Abdomen,
                BodyPart::ArmL,
                BodyPart::ArmR,
            ],
            Self::Leggings => vec![BodyPart::Abdomen, BodyPart::LegL, BodyPart::LegR],
            Self::Boots => vec![BodyPart::LegL, BodyPart::LegR],
        }
    }

    pub fn from_item_suffix(suffix: &str) -> Option<Self> {
        match suffix {
            "helmet" => Some(Self::Helmet),
            "chestplate" => Some(Self::Chestplate),
            "leggings" => Some(Self::Leggings),
            "boots" => Some(Self::Boots),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MundaneArmorItem {
    pub material: MundaneArmorMaterial,
    pub slot: MundaneArmorSlot,
}

impl MundaneArmorItem {
    pub fn item_id(&self) -> String {
        format!("armor_{}_{}", self.material.id(), self.slot.item_suffix())
    }

    pub fn display_name(&self) -> String {
        format!(
            "{}{}",
            self.material.display_name(),
            self.slot.display_name()
        )
    }

    pub fn defense(&self) -> f32 {
        self.material.defense() * self.slot.defense_ratio()
    }

    pub fn armor_profile(&self) -> ArmorProfile {
        let physical = (self.defense() / 10.0).clamp(0.0, ARMOR_MITIGATION_CAP);
        let burn = (physical * 0.35).clamp(0.0, ARMOR_MITIGATION_CAP);
        ArmorProfile {
            slot: self.slot.equip_slot(),
            body_coverage: self.slot.body_coverage(),
            kind_mitigation: HashMap::from([
                (WoundKind::Cut, physical),
                (WoundKind::Blunt, physical),
                (WoundKind::Pierce, physical),
                (WoundKind::Burn, burn),
                (WoundKind::Concussion, physical),
            ]),
            durability_max: self.material.durability_max(),
            broken_multiplier: 0.0,
        }
    }
}

pub fn all_mundane_armor_items() -> Vec<MundaneArmorItem> {
    MundaneArmorMaterial::ALL
        .into_iter()
        .flat_map(|material| {
            MundaneArmorSlot::ALL
                .into_iter()
                .map(move |slot| MundaneArmorItem { material, slot })
        })
        .collect()
}

pub fn parse_mundane_armor_item_id(item_id: &str) -> Option<MundaneArmorItem> {
    let rest = item_id.strip_prefix("armor_")?;
    for material in MundaneArmorMaterial::ALL {
        let material_prefix = format!("{}_", material.id());
        let Some(slot_suffix) = rest.strip_prefix(material_prefix.as_str()) else {
            continue;
        };
        return MundaneArmorSlot::from_item_suffix(slot_suffix)
            .map(|slot| MundaneArmorItem { material, slot });
    }
    None
}

pub fn equip_slot_for_item_id(item_id: &str) -> Option<EquipSlotV1> {
    parse_mundane_armor_item_id(item_id).map(|item| item.slot.equip_slot())
}

pub fn register_mundane_armors(registry: &mut ArmorProfileRegistry) -> Result<(), String> {
    for item in all_mundane_armor_items() {
        registry.register_template(item.item_id(), item.armor_profile())?;
    }
    Ok(())
}

pub fn register_mundane_armor_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    for item in all_mundane_armor_items() {
        registry.register(craft_recipe_for(&item))?;
    }
    Ok(())
}

pub fn craft_recipe_for(item: &MundaneArmorItem) -> CraftRecipe {
    let factor = item.slot.material_ratio();
    let materials = item
        .material
        .base_materials()
        .iter()
        .map(|(template_id, count)| ((*template_id).to_string(), count.saturating_mul(factor)))
        .collect();

    CraftRecipe {
        id: RecipeId::new(format!(
            "armor.mundane.{}.{}",
            item.material.id(),
            item.slot.item_suffix()
        )),
        category: CraftCategory::ArmorCraft,
        display_name: item.display_name(),
        materials,
        qi_cost: 0.0,
        time_ticks: CRAFT_TIME_TICKS,
        output: (item.item_id(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![UnlockSource::Scroll {
            item_template: format!("scroll_armor_{}", item.material.id()),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mundane_bone_defense_3() {
        let total: f32 = MundaneArmorSlot::ALL
            .into_iter()
            .map(|slot| {
                MundaneArmorItem {
                    material: MundaneArmorMaterial::Bone,
                    slot,
                }
                .defense()
            })
            .sum();

        assert!((total - 3.0).abs() < 1e-6);
    }

    #[test]
    fn all_24_items_registered() {
        let mut registry = ArmorProfileRegistry::new();
        register_mundane_armors(&mut registry).expect("mundane armor profiles register");

        assert_eq!(registry.len(), 24);
        for item in all_mundane_armor_items() {
            assert!(
                registry.get(item.item_id().as_str()).is_some(),
                "{} should be registered",
                item.item_id()
            );
        }
    }

    #[test]
    fn craft_recipe_iron_chestplate() {
        let item = MundaneArmorItem {
            material: MundaneArmorMaterial::Iron,
            slot: MundaneArmorSlot::Chestplate,
        };
        let recipe = craft_recipe_for(&item);

        assert_eq!(recipe.id.as_str(), "armor.mundane.iron.chestplate");
        assert_eq!(recipe.category, CraftCategory::ArmorCraft);
        assert_eq!(recipe.output, ("armor_iron_chestplate".to_string(), 1));
        assert!(recipe.materials.contains(&("iron_ore".to_string(), 15)));
        assert!(recipe
            .materials
            .contains(&(BONE_COIN_TEMPLATE.to_string(), 9)));
    }
}
