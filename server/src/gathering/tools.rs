use crate::cultivation::components::Realm;
use crate::inventory::{
    set_item_instance_durability, InventoryDurabilityChangedEvent, InventoryDurabilityUpdate,
    ItemInstance, PlayerInventory, EQUIP_SLOT_MAIN_HAND, EQUIP_SLOT_TWO_HAND,
};
use valence::prelude::{Entity, EventWriter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatheringToolKind {
    Axe,
    Pickaxe,
    Hoe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatheringMaterial {
    Bone,
    Iron,
    Copper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatheringTargetKind {
    Herb,
    Ore,
    Wood,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GatheringToolSpec {
    pub item_id: &'static str,
    pub display_name: &'static str,
    pub kind: GatheringToolKind,
    pub material: GatheringMaterial,
    pub speed_multiplier: f32,
    pub durability_uses: u32,
    pub target: GatheringTargetKind,
}

impl GatheringMaterial {
    pub const fn speed_multiplier(self) -> f32 {
        match self {
            Self::Bone => 1.2,
            Self::Iron => 1.0,
            Self::Copper => 0.8,
        }
    }

    pub const fn durability_uses(self) -> u32 {
        match self {
            Self::Bone => 60,
            Self::Iron => 120,
            Self::Copper => 90,
        }
    }

    pub fn quality_bonus(self) -> QualityBonus {
        match self {
            Self::Bone => QualityBonus {
                fine: 0.0,
                perfect: 0.0,
            },
            Self::Iron => QualityBonus {
                fine: 0.05,
                perfect: 0.01,
            },
            Self::Copper => QualityBonus {
                fine: 0.10,
                perfect: 0.03,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityBonus {
    pub fine: f32,
    pub perfect: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatheringToolDurabilityUseOutcome {
    pub spec: GatheringToolSpec,
    pub instance_id: u64,
    pub update: InventoryDurabilityUpdate,
}

impl GatheringToolSpec {
    pub const fn new(
        item_id: &'static str,
        display_name: &'static str,
        kind: GatheringToolKind,
        material: GatheringMaterial,
        target: GatheringTargetKind,
    ) -> Self {
        Self {
            item_id,
            display_name,
            kind,
            material,
            speed_multiplier: material.speed_multiplier(),
            durability_uses: material.durability_uses(),
            target,
        }
    }

    pub fn matches_target(self, target: GatheringTargetKind) -> bool {
        self.target == target
    }

    pub fn durability_cost_ratio(self) -> f64 {
        1.0 / f64::from(self.durability_uses.max(1))
    }
}

pub const GATHERING_TOOL_SPECS: [GatheringToolSpec; 9] = [
    GatheringToolSpec::new(
        "axe_bone",
        "骨斧",
        GatheringToolKind::Axe,
        GatheringMaterial::Bone,
        GatheringTargetKind::Wood,
    ),
    GatheringToolSpec::new(
        "axe_iron",
        "铁斧",
        GatheringToolKind::Axe,
        GatheringMaterial::Iron,
        GatheringTargetKind::Wood,
    ),
    GatheringToolSpec::new(
        "axe_copper",
        "铜斧",
        GatheringToolKind::Axe,
        GatheringMaterial::Copper,
        GatheringTargetKind::Wood,
    ),
    GatheringToolSpec::new(
        "pickaxe_bone",
        "骨镐",
        GatheringToolKind::Pickaxe,
        GatheringMaterial::Bone,
        GatheringTargetKind::Ore,
    ),
    GatheringToolSpec::new(
        "pickaxe_iron",
        "铁镐",
        GatheringToolKind::Pickaxe,
        GatheringMaterial::Iron,
        GatheringTargetKind::Ore,
    ),
    GatheringToolSpec::new(
        "pickaxe_copper",
        "铜镐",
        GatheringToolKind::Pickaxe,
        GatheringMaterial::Copper,
        GatheringTargetKind::Ore,
    ),
    GatheringToolSpec::new(
        "bao_chu",
        "刨锄",
        GatheringToolKind::Hoe,
        GatheringMaterial::Bone,
        GatheringTargetKind::Herb,
    ),
    GatheringToolSpec::new(
        "hoe_iron",
        "铁锄",
        GatheringToolKind::Hoe,
        GatheringMaterial::Iron,
        GatheringTargetKind::Herb,
    ),
    GatheringToolSpec::new(
        "hoe_lingtie",
        "灵铁锄",
        GatheringToolKind::Hoe,
        GatheringMaterial::Copper,
        GatheringTargetKind::Herb,
    ),
];

pub fn spec_for_item_id(item_id: &str) -> Option<GatheringToolSpec> {
    GATHERING_TOOL_SPECS
        .iter()
        .copied()
        .find(|spec| spec.item_id == item_id)
        .or_else(|| {
            // 玄铁锄是 lingtian 既有高档锄头；采集系统按轻快高质档处理。
            (item_id == "hoe_xuantie").then_some(GatheringToolSpec::new(
                "hoe_xuantie",
                "玄铁锄",
                GatheringToolKind::Hoe,
                GatheringMaterial::Copper,
                GatheringTargetKind::Herb,
            ))
        })
}

pub fn item_to_spec(item: &ItemInstance) -> Option<GatheringToolSpec> {
    if item.durability <= 0.0 {
        return None;
    }
    spec_for_item_id(item.template_id.as_str())
}

pub fn equipped_gathering_tool(inventory: &PlayerInventory) -> Option<GatheringToolSpec> {
    inventory
        .equipped
        .get(EQUIP_SLOT_MAIN_HAND)
        .and_then(item_to_spec)
        .or_else(|| {
            inventory
                .equipped
                .get(EQUIP_SLOT_TWO_HAND)
                .and_then(item_to_spec)
        })
}

fn equipped_gathering_tool_instance(
    inventory: &PlayerInventory,
    expected: GatheringToolSpec,
) -> Option<(GatheringToolSpec, u64, f64)> {
    [EQUIP_SLOT_MAIN_HAND, EQUIP_SLOT_TWO_HAND]
        .into_iter()
        .filter_map(|slot| inventory.equipped.get(slot))
        .find_map(|item| {
            let spec = item_to_spec(item)?;
            (spec.item_id == expected.item_id).then_some((spec, item.instance_id, item.durability))
        })
}

fn damage_equipped_gathering_tool_in_inventory(
    inventory: &mut PlayerInventory,
    expected: GatheringToolSpec,
) -> Option<GatheringToolDurabilityUseOutcome> {
    let (spec, instance_id, current) = equipped_gathering_tool_instance(inventory, expected)?;
    let next = (current - spec.durability_cost_ratio()).clamp(0.0, 1.0);
    if next >= current {
        return None;
    }
    match set_item_instance_durability(inventory, instance_id, next) {
        Ok(update) => Some(GatheringToolDurabilityUseOutcome {
            spec,
            instance_id,
            update,
        }),
        Err(error) => {
            tracing::warn!(
                "[bong][gathering] failed to persist durability for tool instance {}: {}",
                instance_id,
                error
            );
            None
        }
    }
}

pub fn damage_equipped_gathering_tool(
    entity: Entity,
    inventory: &mut PlayerInventory,
    expected: GatheringToolSpec,
    durability_events: &mut EventWriter<InventoryDurabilityChangedEvent>,
) -> Option<GatheringToolDurabilityUseOutcome> {
    let outcome = damage_equipped_gathering_tool_in_inventory(inventory, expected)?;
    durability_events.send(InventoryDurabilityChangedEvent {
        entity,
        revision: outcome.update.revision,
        instance_id: outcome.update.instance_id,
        durability: outcome.update.durability,
    });
    Some(outcome)
}

pub fn base_time_ticks(target: GatheringTargetKind) -> u64 {
    match target {
        GatheringTargetKind::Herb => 40,
        GatheringTargetKind::Ore => 60,
        GatheringTargetKind::Wood => 50,
    }
}

pub fn realm_speed_multiplier(realm: Realm) -> f32 {
    let rank = match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    };
    (1.0_f32 - rank as f32 * 0.05).clamp(0.75, 1.0)
}

pub fn gather_time_ticks(
    target: GatheringTargetKind,
    tool: Option<GatheringToolSpec>,
    realm: Realm,
) -> u64 {
    let tool_multiplier = match tool {
        Some(spec) if spec.matches_target(target) => spec.speed_multiplier,
        Some(_) | None => 3.0,
    };
    let ticks = base_time_ticks(target) as f32 * tool_multiplier * realm_speed_multiplier(realm);
    ticks.ceil().max(1.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryRevision, ItemRarity};
    use std::collections::HashMap;

    fn item(template_id: &str, durability: f64) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability,
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

    fn inventory_with_main(template_id: &str) -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert(EQUIP_SLOT_MAIN_HAND.to_string(), item(template_id, 1.0));
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: Vec::new(),
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 10.0,
        }
    }

    fn inventory_with_two_hand(template_id: &str) -> PlayerInventory {
        let mut equipped = HashMap::new();
        equipped.insert(EQUIP_SLOT_TWO_HAND.to_string(), item(template_id, 1.0));
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: Vec::new(),
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 10.0,
        }
    }

    #[test]
    fn tool_specs_cover_three_targets_and_materials() {
        for target in [
            GatheringTargetKind::Herb,
            GatheringTargetKind::Ore,
            GatheringTargetKind::Wood,
        ] {
            for material in [
                GatheringMaterial::Bone,
                GatheringMaterial::Iron,
                GatheringMaterial::Copper,
            ] {
                assert!(
                    GATHERING_TOOL_SPECS
                        .iter()
                        .any(|spec| spec.target == target && spec.material == material),
                    "missing {target:?}/{material:?} gathering tool spec"
                );
            }
        }
    }

    #[test]
    fn equipped_tool_reads_main_hand_and_ignores_broken() {
        assert_eq!(
            equipped_gathering_tool(&inventory_with_main("pickaxe_copper"))
                .expect("pickaxe should map")
                .kind,
            GatheringToolKind::Pickaxe
        );

        let mut inventory = inventory_with_main("axe_iron");
        inventory
            .equipped
            .get_mut(EQUIP_SLOT_MAIN_HAND)
            .expect("main hand")
            .durability = 0.0;
        assert_eq!(equipped_gathering_tool(&inventory), None);
    }

    #[test]
    fn equipped_tool_falls_back_to_two_hand_when_main_hand_is_not_gathering_tool() {
        let mut inventory = inventory_with_main("iron_sword");
        inventory
            .equipped
            .insert(EQUIP_SLOT_TWO_HAND.to_string(), item("pickaxe_iron", 1.0));

        let actual = equipped_gathering_tool(&inventory).map(|spec| spec.kind);

        assert_eq!(
            actual,
            Some(GatheringToolKind::Pickaxe),
            "expected equipped_gathering_tool to inspect two-hand when main-hand is not a gathering tool, got {actual:?}"
        );
    }

    #[test]
    fn spec_for_item_id_supports_hoe_xuantie_alias() {
        let spec = spec_for_item_id("hoe_xuantie")
            .expect("expected hoe_xuantie alias to resolve to a gathering spec");
        assert_eq!(
            spec.item_id, "hoe_xuantie",
            "expected hoe_xuantie alias to preserve the registered item id, actual {}",
            spec.item_id
        );
        assert_eq!(
            spec.kind,
            GatheringToolKind::Hoe,
            "expected hoe_xuantie to map to Hoe because it is a herb tool, actual {:?}",
            spec.kind
        );
        assert_eq!(
            spec.material,
            GatheringMaterial::Copper,
            "expected hoe_xuantie compatibility mapping to use Copper tier, actual {:?}",
            spec.material
        );
        assert_eq!(
            spec.target,
            GatheringTargetKind::Herb,
            "expected hoe_xuantie compatibility mapping to target Herb, actual {:?}",
            spec.target
        );
    }

    #[test]
    fn successful_gathering_ticks_tool_durability() {
        let mut inventory = inventory_with_two_hand("pickaxe_copper");
        let spec = spec_for_item_id("pickaxe_copper").expect("fixture tool exists");

        let outcome = damage_equipped_gathering_tool_in_inventory(&mut inventory, spec)
            .expect("matching equipped tool should be damaged");

        assert_eq!(outcome.spec.item_id, "pickaxe_copper");
        assert_eq!(
            inventory.equipped[EQUIP_SLOT_TWO_HAND].durability,
            1.0 - 1.0 / 90.0
        );
        assert_eq!(outcome.update.instance_id, 1);
    }

    #[test]
    fn wrong_equipped_tool_does_not_tick_gathering_durability() {
        let mut inventory = inventory_with_main("axe_bone");
        let expected = spec_for_item_id("pickaxe_bone").expect("fixture tool exists");

        assert_eq!(
            damage_equipped_gathering_tool_in_inventory(&mut inventory, expected),
            None
        );
        assert_eq!(inventory.equipped[EQUIP_SLOT_MAIN_HAND].durability, 1.0);
    }

    #[test]
    fn tool_and_realm_reduce_gather_time() {
        let no_tool = gather_time_ticks(GatheringTargetKind::Herb, None, Realm::Awaken);
        let iron_hoe = gather_time_ticks(
            GatheringTargetKind::Herb,
            spec_for_item_id("hoe_iron"),
            Realm::Awaken,
        );
        let copper_hoe_void = gather_time_ticks(
            GatheringTargetKind::Herb,
            spec_for_item_id("hoe_lingtie"),
            Realm::Void,
        );

        assert_eq!(no_tool, 120);
        assert_eq!(iron_hoe, 40);
        assert_eq!(copper_hoe_void, 24);
    }
}
