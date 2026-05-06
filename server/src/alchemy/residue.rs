//! plan-alchemy-recycle-v1 — 炼丹废料反哺灵田的废料规格与库存 helper。

use serde::{Deserialize, Serialize};

use crate::inventory::{AlchemyItemData, ItemInstance, PlayerInventory};

pub const FAILED_PILL_RESIDUE_TEMPLATE_ID: &str = "alchemy_residue_failed_pill";
pub const FLAWED_PILL_RESIDUE_TEMPLATE_ID: &str = "alchemy_residue_flawed_pill";
pub const PROCESSING_DREGS_TEMPLATE_ID: &str = "alchemy_residue_processing_dregs";
pub const AGING_SCRAPS_TEMPLATE_ID: &str = "alchemy_residue_aging_scraps";

/// 72h 保鲜期，按 server tick 计（20 ticks/s）。
pub const PILL_RESIDUE_TTL_TICKS: u64 = 72 * 60 * 60 * 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PillResidueKind {
    FailedPill,
    FlawedPill,
    ProcessingDregs,
    AgingScraps,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillResidueSpec {
    pub template_id: &'static str,
    pub plot_qi_amount: f32,
    pub duration_ticks: u32,
    pub contamination_chance: f32,
    pub contamination_delta: f32,
}

impl PillResidueKind {
    pub fn spec(self) -> PillResidueSpec {
        match self {
            Self::FailedPill => PillResidueSpec {
                template_id: FAILED_PILL_RESIDUE_TEMPLATE_ID,
                plot_qi_amount: 0.4,
                duration_ticks: 100,
                contamination_chance: 0.30,
                contamination_delta: 0.10,
            },
            Self::FlawedPill => PillResidueSpec {
                template_id: FLAWED_PILL_RESIDUE_TEMPLATE_ID,
                plot_qi_amount: 0.6,
                duration_ticks: 80,
                contamination_chance: 0.10,
                contamination_delta: 0.05,
            },
            Self::ProcessingDregs => PillResidueSpec {
                template_id: PROCESSING_DREGS_TEMPLATE_ID,
                plot_qi_amount: 0.3,
                duration_ticks: 60,
                contamination_chance: 0.03,
                contamination_delta: 0.02,
            },
            Self::AgingScraps => PillResidueSpec {
                template_id: AGING_SCRAPS_TEMPLATE_ID,
                plot_qi_amount: 0.2,
                duration_ticks: 60,
                contamination_chance: 0.005,
                contamination_delta: 0.01,
            },
        }
    }
}

pub fn kind_for_template_id(template_id: &str) -> Option<PillResidueKind> {
    match template_id {
        FAILED_PILL_RESIDUE_TEMPLATE_ID => Some(PillResidueKind::FailedPill),
        FLAWED_PILL_RESIDUE_TEMPLATE_ID => Some(PillResidueKind::FlawedPill),
        PROCESSING_DREGS_TEMPLATE_ID
        | "withered_processed_ci_she_hao"
        | "withered_processed_ning_mai_cao" => Some(PillResidueKind::ProcessingDregs),
        AGING_SCRAPS_TEMPLATE_ID | "withered_dry_ci_she_hao" | "withered_dry_ning_mai_cao" => {
            Some(PillResidueKind::AgingScraps)
        }
        _ => None,
    }
}

pub fn residue_alchemy_data(kind: PillResidueKind, produced_at_tick: u64) -> AlchemyItemData {
    AlchemyItemData::PillResidue {
        residue_kind: kind,
        produced_at_tick,
        expires_at_tick: produced_at_tick.saturating_add(PILL_RESIDUE_TTL_TICKS),
    }
}

pub fn residue_kind_for_recyclable_outcome(
    outcome: &crate::alchemy::ResolvedOutcome,
) -> Option<PillResidueKind> {
    match outcome {
        crate::alchemy::ResolvedOutcome::Pill {
            flawed_path: true, ..
        } => Some(PillResidueKind::FlawedPill),
        crate::alchemy::ResolvedOutcome::Waste { .. }
        | crate::alchemy::ResolvedOutcome::Mismatch
        | crate::alchemy::ResolvedOutcome::Explode { .. } => Some(PillResidueKind::FailedPill),
        crate::alchemy::ResolvedOutcome::Pill {
            flawed_path: false, ..
        } => None,
    }
}

pub fn item_residue_kind(item: &ItemInstance) -> Option<PillResidueKind> {
    match item.alchemy.as_ref() {
        Some(AlchemyItemData::PillResidue { residue_kind, .. }) => Some(*residue_kind),
        _ => kind_for_template_id(item.template_id.as_str()),
    }
}

pub fn item_is_usable_residue(item: &ItemInstance, kind: PillResidueKind, now_tick: u64) -> bool {
    if item.stack_count == 0 || item_residue_kind(item) != Some(kind) {
        return false;
    }
    match item.alchemy.as_ref() {
        Some(AlchemyItemData::PillResidue {
            expires_at_tick, ..
        }) => now_tick < *expires_at_tick,
        _ => true,
    }
}

pub fn inventory_has_usable_residue(
    inventory: &PlayerInventory,
    kind: PillResidueKind,
    now_tick: u64,
) -> bool {
    inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter().map(|placed| &placed.instance))
        .chain(inventory.hotbar.iter().flatten())
        .any(|item| item_is_usable_residue(item, kind, now_tick))
}

pub fn consume_one_residue(
    inventory: &mut PlayerInventory,
    kind: PillResidueKind,
    now_tick: u64,
) -> bool {
    for container in &mut inventory.containers {
        if let Some(idx) = container
            .items
            .iter()
            .position(|placed| item_is_usable_residue(&placed.instance, kind, now_tick))
        {
            let placed = &mut container.items[idx];
            if placed.instance.stack_count > 1 {
                placed.instance.stack_count -= 1;
            } else {
                container.items.remove(idx);
            }
            inventory.revision.0 = inventory.revision.0.saturating_add(1);
            return true;
        }
    }

    for slot in inventory.hotbar.iter_mut() {
        if slot
            .as_ref()
            .is_some_and(|item| item_is_usable_residue(item, kind, now_tick))
        {
            let item = slot.as_mut().expect("checked above");
            if item.stack_count > 1 {
                item.stack_count -= 1;
            } else {
                *slot = None;
            }
            inventory.revision.0 = inventory.revision.0.saturating_add(1);
            return true;
        }
    }

    false
}

pub fn contamination_triggers(kind: PillResidueKind, roll: f32) -> bool {
    let roll = if roll.is_finite() { roll } else { 1.0 };
    roll.clamp(0.0, 1.0) < kind.spec().contamination_chance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ContainerState, InventoryRevision, ItemRarity, PlacedItemState};

    fn residue_item(kind: PillResidueKind, now_tick: u64, stack_count: u32) -> ItemInstance {
        let spec = kind.spec();
        ItemInstance {
            instance_id: 10,
            template_id: spec.template_id.to_string(),
            display_name: spec.template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.04,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count,
            spirit_quality: 0.3,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: Some(residue_alchemy_data(kind, now_tick)),
            lingering_owner_qi: None,
        }
    }

    fn inventory_with(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "主背包".to_string(),
                rows: 4,
                cols: 4,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    #[test]
    fn residue_rejects_expired_metadata() {
        let mut inventory = inventory_with(residue_item(PillResidueKind::FailedPill, 10, 1));
        let expired_tick = 10 + PILL_RESIDUE_TTL_TICKS;
        assert!(!inventory_has_usable_residue(
            &inventory,
            PillResidueKind::FailedPill,
            expired_tick
        ));
        assert!(!consume_one_residue(
            &mut inventory,
            PillResidueKind::FailedPill,
            expired_tick
        ));
    }

    #[test]
    fn consume_residue_decrements_stack_and_revision() {
        let mut inventory = inventory_with(residue_item(PillResidueKind::FailedPill, 10, 2));
        assert!(consume_one_residue(
            &mut inventory,
            PillResidueKind::FailedPill,
            11
        ));
        let item = &inventory.containers[0].items[0].instance;
        assert_eq!(item.stack_count, 1);
        assert_eq!(inventory.revision.0, 1);
    }

    #[test]
    fn contamination_rolls_are_kind_specific() {
        assert!(contamination_triggers(PillResidueKind::FailedPill, 0.29));
        assert!(!contamination_triggers(PillResidueKind::FailedPill, 0.30));
        assert!(contamination_triggers(
            PillResidueKind::ProcessingDregs,
            0.02
        ));
        assert!(!contamination_triggers(
            PillResidueKind::ProcessingDregs,
            0.03
        ));
    }

    #[test]
    fn recyclable_outcomes_map_to_residue_kinds() {
        assert_eq!(
            residue_kind_for_recyclable_outcome(&crate::alchemy::ResolvedOutcome::Waste {
                recipe_id: Some("hui_yuan_pill_v0".to_string()),
            }),
            Some(PillResidueKind::FailedPill)
        );
        assert_eq!(
            residue_kind_for_recyclable_outcome(&crate::alchemy::ResolvedOutcome::Mismatch),
            Some(PillResidueKind::FailedPill)
        );
        assert_eq!(
            residue_kind_for_recyclable_outcome(&crate::alchemy::ResolvedOutcome::Pill {
                recipe_id: "hui_yuan_pill_v0".to_string(),
                pill: "hui_yuan_pill".to_string(),
                quality: 0.4,
                toxin_amount: 0.3,
                toxin_color: crate::cultivation::components::ColorKind::Mellow,
                qi_gain: None,
                quality_tier: 3,
                effect_multiplier: 0.6,
                consecrated: false,
                side_effect: None,
                flawed_path: true,
            }),
            Some(PillResidueKind::FlawedPill)
        );
        assert_eq!(
            residue_kind_for_recyclable_outcome(&crate::alchemy::ResolvedOutcome::Pill {
                recipe_id: "hui_yuan_pill_v0".to_string(),
                pill: "hui_yuan_pill".to_string(),
                quality: 1.0,
                toxin_amount: 0.1,
                toxin_color: crate::cultivation::components::ColorKind::Mellow,
                qi_gain: Some(1.0),
                quality_tier: 5,
                effect_multiplier: 1.0,
                consecrated: false,
                side_effect: None,
                flawed_path: false,
            }),
            None
        );
    }
}
