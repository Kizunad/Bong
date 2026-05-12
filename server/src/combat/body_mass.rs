//! plan-knockback-physics-v1 — body mass and stance inputs for knockback.

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Query};

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

    pub fn from_inventory(inventory: &PlayerInventory) -> Self {
        let armor_mass = equipped_armor_mass(inventory);
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
            StaminaState::Idle | StaminaState::Combat => Self::Standing,
        }
    }
}

impl Default for Stance {
    fn default() -> Self {
        Self::Standing
    }
}

pub fn sync_body_mass_from_inventory(mut players: Query<(&PlayerInventory, &mut BodyMass)>) {
    for (inventory, mut body_mass) in &mut players {
        let next = BodyMass::from_inventory(inventory);
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

fn equipped_armor_mass(inventory: &PlayerInventory) -> f64 {
    [
        EQUIP_SLOT_HEAD,
        EQUIP_SLOT_CHEST,
        EQUIP_SLOT_LEGS,
        EQUIP_SLOT_FEET,
    ]
    .into_iter()
    .filter_map(|slot| inventory.equipped.get(slot))
    .map(|item| item.weight * f64::from(item.stack_count.max(1)))
    .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity};
    use std::collections::HashMap;

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
        inventory
            .equipped
            .insert(EQUIP_SLOT_CHEST.to_string(), item(1, 12.0));
        inventory.hotbar[0] = Some(item(2, 4.0));

        let mass = BodyMass::from_inventory(&inventory);

        assert_eq!(mass.base_mass, HUMAN_BASE_MASS);
        assert_eq!(mass.armor_mass, 12.0);
        assert_eq!(mass.inventory_mass, 4.0);
        assert_eq!(mass.total_mass(), 86.0);
    }

    #[test]
    fn inventory_mass_is_capped() {
        let mut inventory = empty_inventory();
        inventory.hotbar[0] = Some(item(1, 100.0));

        let mass = BodyMass::from_inventory(&inventory);

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
