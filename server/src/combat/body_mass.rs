//! plan-knockback-physics-v1 — body mass and stance inputs for knockback.

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Query, Res};

use crate::combat::armor::ArmorProfileRegistry;
use crate::combat::components::{CombatState, Stamina, StaminaState};
use crate::inventory::{
    calculate_current_weight, PlayerInventory, EQUIP_SLOT_CHEST, EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD,
    EQUIP_SLOT_LEGS,
};
use crate::npc::lifecycle::NpcArchetype;

pub const HUMAN_BASE_MASS: f64 = 70.0;
pub const DEFAULT_NPC_BASE_MASS: f64 = 70.0;
pub const SKULL_FIEND_BASE_MASS: f64 = 30.0;
pub const DAOXIANG_BASE_MASS: f64 = 60.0;
pub const BEAST_BASE_MASS: f64 = 120.0;
pub const HEAVY_BEAST_BASE_MASS: f64 = 500.0;
pub const MAX_INVENTORY_MASS: f64 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Component)]
pub struct BodyMass {
    pub base_mass: f64,
    pub armor_mass: f64,
    pub inventory_mass: f64,
}

impl BodyMass {
    pub const fn human() -> Self {
        Self {
            base_mass: HUMAN_BASE_MASS,
            armor_mass: 0.0,
            inventory_mass: 0.0,
        }
    }

    pub const fn npc(base_mass: f64) -> Self {
        Self {
            base_mass,
            armor_mass: 0.0,
            inventory_mass: 0.0,
        }
    }

    pub const fn for_npc_archetype(archetype: NpcArchetype) -> Self {
        Self::npc(npc_base_mass(archetype))
    }

    pub fn total_mass(self) -> f64 {
        (self.base_mass + self.armor_mass + self.inventory_mass).max(1.0)
    }

    pub fn from_inventory(
        inventory: &PlayerInventory,
        armor_profiles: &ArmorProfileRegistry,
    ) -> Self {
        let armor_mass = equipped_armor_mass(inventory, armor_profiles);
        let carried_mass = (calculate_current_weight(inventory) - armor_mass).max(0.0);
        Self {
            base_mass: HUMAN_BASE_MASS,
            armor_mass,
            inventory_mass: carried_mass.min(MAX_INVENTORY_MASS),
        }
    }
}

pub const fn npc_base_mass(archetype: NpcArchetype) -> f64 {
    match archetype {
        NpcArchetype::SkullFiend => SKULL_FIEND_BASE_MASS,
        NpcArchetype::Daoxiang => DAOXIANG_BASE_MASS,
        NpcArchetype::Beast => BEAST_BASE_MASS,
        NpcArchetype::Fuya => HEAVY_BEAST_BASE_MASS,
        _ => DEFAULT_NPC_BASE_MASS,
    }
}

impl Default for BodyMass {
    fn default() -> Self {
        Self::human()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Component)]
pub enum Stance {
    Rooted,
    Braced,
    Standing,
    Moving,
    Casting,
    Sprinting,
    Exhausted,
    Airborne,
}

impl Stance {
    pub const fn factor(self) -> f64 {
        match self {
            Self::Rooted => 2.5,
            Self::Braced => 1.5,
            Self::Standing => 1.0,
            Self::Moving => 0.85,
            Self::Casting => 0.7,
            Self::Sprinting => 0.5,
            Self::Exhausted => 0.4,
            Self::Airborne => 0.2,
        }
    }

    pub fn from_runtime(stamina: &Stamina, combat_state: Option<&CombatState>) -> Self {
        if combat_state.is_some_and(|state| state.incoming_window.is_some()) {
            return Self::Braced;
        }
        match stamina.state {
            StaminaState::Exhausted => Self::Exhausted,
            StaminaState::Sprinting => Self::Sprinting,
            StaminaState::Walking | StaminaState::Jogging => Self::Moving,
            // plan-shield-block-v1 P2 — 举盾视为站立（未冲刺/行进）
            StaminaState::Idle | StaminaState::Combat | StaminaState::ShieldBlocking => {
                Self::Standing
            }
        }
    }
}

impl Default for Stance {
    fn default() -> Self {
        Self::Standing
    }
}

pub fn sync_body_mass_from_inventory(
    mut players: Query<(&PlayerInventory, &mut BodyMass)>,
    armor_profiles: Res<ArmorProfileRegistry>,
) {
    for (inventory, mut body_mass) in &mut players {
        let next = BodyMass::from_inventory(inventory, armor_profiles.as_ref());
        if *body_mass != next {
            *body_mass = next;
        }
    }
}

pub fn sync_stance_from_runtime(
    mut combatants: Query<(&mut Stance, &Stamina, Option<&CombatState>)>,
) {
    for (mut stance, stamina, combat_state) in &mut combatants {
        let next = Stance::from_runtime(stamina, combat_state);
        if *stance != next {
            *stance = next;
        }
    }
}

fn equipped_armor_mass(inventory: &PlayerInventory, armor_profiles: &ArmorProfileRegistry) -> f64 {
    // plan-layered-equip-v1 P3 公式6（gap#13）— 遍历四护甲身体槽 worn 全层，
    // **仅计有 ArmorProfile 的件**（排除同槽 worn 的背包 / 伪皮自重，否则误算进击退 total_mass）。
    // 与 calculate_current_weight 一致按 worn 全层求和，但 mass 须排非护甲件——
    // 背包件自重仍计入 current_weight（公式7），只是不计入 armor_mass / 击退。
    [
        EQUIP_SLOT_HEAD,
        EQUIP_SLOT_CHEST,
        EQUIP_SLOT_LEGS,
        EQUIP_SLOT_FEET,
    ]
    .into_iter()
    .filter_map(|slot| inventory.equipped.get(slot))
    .flat_map(|contents| contents.worn.iter())
    .filter(|item| armor_profiles.get(item.template_id.as_str()).is_some())
    .map(|item| item.weight * f64::from(item.stack_count.max(1)))
    .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::armor::ArmorProfile;
    use crate::combat::components::{BodyPart, WoundKind};
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity, SlotContents};
    use crate::schema::inventory::EquipSlotV1;
    use std::collections::HashMap;

    /// 构造一个把给定 template_id 标记为有 ArmorProfile 的注册表（公式6 过滤用）。
    /// 未列入的 template_id（如背包 / 伪皮）视作无护甲 → 不计入 armor_mass。
    fn armor_registry(armor_template_ids: &[&str]) -> ArmorProfileRegistry {
        let map = armor_template_ids
            .iter()
            .map(|id| {
                (
                    id.to_string(),
                    ArmorProfile {
                        slot: EquipSlotV1::Chest,
                        body_coverage: vec![BodyPart::Chest],
                        kind_mitigation: HashMap::from([(WoundKind::Cut, 0.1)]),
                        durability_max: 10,
                        broken_multiplier: 0.3,
                    },
                )
            })
            .collect();
        ArmorProfileRegistry::from_map(map)
    }

    fn item(id: u64, weight: f64) -> ItemInstance {
        ItemInstance {
            instance_id: id,
            template_id: format!("item_{id}"),
            display_name: format!("Item {id}"),
            grid_w: 1,
            grid_h: 1,
            weight,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: Vec::new(),
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    #[test]
    fn body_mass_splits_armor_from_carried_weight() {
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents::worn_single(item(1, 12.0)),
        );
        inventory.hotbar[0] = Some(item(2, 4.0));

        // item_1 有 ArmorProfile → 计入 armor_mass；item_2 是 hotbar 件（不参与 armor_mass 槽扫描）。
        let registry = armor_registry(&["item_1"]);
        let mass = BodyMass::from_inventory(&inventory, &registry);

        assert_eq!(mass.base_mass, HUMAN_BASE_MASS);
        assert_eq!(
            mass.armor_mass, 12.0,
            "穿戴的护甲件(item_1, 12kg)应计入 armor_mass"
        );
        assert_eq!(
            mass.inventory_mass, 4.0,
            "hotbar 件(item_2, 4kg)是携带重量、不是护甲重量"
        );
        assert_eq!(mass.total_mass(), 86.0);
    }

    #[test]
    fn equipped_armor_mass_filters_non_armor_worn_items() {
        // plan-layered-equip-v1 P3 公式6（gap#13）：chest worn=[铁甲 12kg, 背包 2kg]
        // → armor_mass = 12（仅有 ArmorProfile 的甲，排背包自重）；
        //   current_weight = 14（甲+背包自重均计入负重，公式7）；
        //   carried(inventory_mass) = current_weight - armor_mass = 2（背包自重落携带侧，不计入击退护甲）。
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            SlotContents {
                worn: vec![item(1, 12.0), item(2, 2.0)],
                held: None,
            },
        );

        // 只有铁甲(item_1)有 ArmorProfile；背包(item_2)无 → 不计入 armor_mass。
        let registry = armor_registry(&["item_1"]);

        assert_eq!(
            equipped_armor_mass(&inventory, &registry),
            12.0,
            "只有有 ArmorProfile 的铁甲计入 armor_mass，背包自重排除（公式6）"
        );
        assert_eq!(
            calculate_current_weight(&inventory),
            14.0,
            "current_weight 含甲(12)+背包自重(2)（公式7）"
        );

        let mass = BodyMass::from_inventory(&inventory, &registry);
        assert_eq!(mass.armor_mass, 12.0, "armor_mass 仅护甲");
        assert_eq!(
            mass.inventory_mass, 2.0,
            "背包自重落 inventory_mass(carried = current_weight - armor_mass)"
        );
    }

    #[test]
    fn inventory_mass_is_capped() {
        let mut inventory = empty_inventory();
        inventory.hotbar[0] = Some(item(1, 100.0));

        let registry = armor_registry(&[]);
        let mass = BodyMass::from_inventory(&inventory, &registry);

        assert_eq!(mass.inventory_mass, MAX_INVENTORY_MASS);
    }

    #[test]
    fn npc_archetype_mass_matches_plan_table() {
        assert_eq!(
            BodyMass::for_npc_archetype(NpcArchetype::SkullFiend).base_mass,
            SKULL_FIEND_BASE_MASS
        );
        assert_eq!(
            BodyMass::for_npc_archetype(NpcArchetype::Daoxiang).base_mass,
            DAOXIANG_BASE_MASS
        );
        assert_eq!(
            BodyMass::for_npc_archetype(NpcArchetype::Beast).base_mass,
            BEAST_BASE_MASS
        );
        assert_eq!(
            BodyMass::for_npc_archetype(NpcArchetype::Fuya).base_mass,
            HEAVY_BEAST_BASE_MASS
        );
    }

    #[test]
    fn stance_prefers_braced_defense_window_over_stamina_state() {
        let stamina = Stamina {
            state: StaminaState::Sprinting,
            ..Default::default()
        };
        let combat_state = CombatState {
            incoming_window: Some(crate::combat::components::DefenseWindow {
                opened_at_tick: 0,
                duration_ms: 100,
            }),
            ..Default::default()
        };

        assert_eq!(
            Stance::from_runtime(&stamina, Some(&combat_state)),
            Stance::Braced
        );
    }

    #[test]
    fn stance_maps_exhausted_to_low_resistance() {
        let stamina = Stamina {
            state: StaminaState::Exhausted,
            ..Default::default()
        };

        assert_eq!(Stance::from_runtime(&stamina, None), Stance::Exhausted);
        assert!(Stance::Exhausted.factor() < Stance::Standing.factor());
    }
}
