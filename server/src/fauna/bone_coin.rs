//! 封灵骨币制作 session（plan-fauna-v1 §4 / §7 P1）。

use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Query, Res, ResMut};

use crate::cultivation::components::Cultivation;
use crate::inventory::{
    bump_revision, consume_item_instance_once, force_attach_item_to_inventory,
    inventory_item_by_instance_borrow, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::shelflife::DecayProfileRegistry;

use super::drop::{
    build_fauna_item_instance, FENG_HE_GU, SHU_GU, YI_SHOU_GU, ZHEN_SHI_CHU, ZHU_GU,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoneGrade {
    Rat,
    Spider,
    Hybrid,
    General,
}

impl BoneGrade {
    pub const fn qi_cap(self) -> f64 {
        match self {
            Self::Rat => 5.0,
            Self::Spider => 15.0,
            Self::Hybrid => 40.0,
            Self::General => 20.0,
        }
    }

    pub const fn output_template(self, sealed_qi: f64) -> &'static str {
        if sealed_qi <= 5.0 {
            "bone_coin_5"
        } else if sealed_qi <= 15.0 {
            "bone_coin_15"
        } else {
            "bone_coin_40"
        }
    }
}

pub fn bone_grade_for_template(template_id: &str) -> Option<BoneGrade> {
    match template_id {
        SHU_GU => Some(BoneGrade::Rat),
        ZHU_GU => Some(BoneGrade::Spider),
        FENG_HE_GU => Some(BoneGrade::Hybrid),
        YI_SHOU_GU => Some(BoneGrade::General),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneCoinCraftSession {
    pub player: Entity,
    pub bone_instance_id: u64,
    pub catalyst_instance_id: Option<u64>,
    pub qi_invest: f64,
    pub issued_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoneCoinCraftPlan {
    pub bone_grade: BoneGrade,
    pub sealed_qi: f64,
    pub total_qi_cost: f64,
    pub output_template: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoneCoinCraftError {
    BoneMissing,
    NotFaunaBone,
    InvalidQi,
    InsufficientQi,
    CatalystMissing,
    CatalystWrongItem,
    RegistryMissing,
    ItemBuildFailed,
    InventoryConsumeFailed,
}

#[derive(Debug, Clone, Event)]
pub struct BoneCoinCraftRequest {
    pub session: BoneCoinCraftSession,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct BoneCoinCrafted {
    pub player: Entity,
    pub bone_instance_id: u64,
    pub catalyst_instance_id: Option<u64>,
    pub output_instance_id: u64,
    pub output_template: String,
    pub sealed_qi: f64,
}

pub fn plan_bone_coin_craft(
    bone_template_id: &str,
    qi_invest: f64,
    has_catalyst: bool,
) -> Result<BoneCoinCraftPlan, BoneCoinCraftError> {
    if !qi_invest.is_finite() || qi_invest <= 0.0 {
        return Err(BoneCoinCraftError::InvalidQi);
    }
    let bone_grade =
        bone_grade_for_template(bone_template_id).ok_or(BoneCoinCraftError::NotFaunaBone)?;
    let sealed_qi = qi_invest.min(bone_grade.qi_cap());
    let seal_cost = if has_catalyst { 0.0 } else { sealed_qi * 0.2 };
    let total_qi_cost = sealed_qi + seal_cost;
    Ok(BoneCoinCraftPlan {
        bone_grade,
        sealed_qi,
        total_qi_cost,
        output_template: bone_grade.output_template(sealed_qi),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn handle_bone_coin_craft_requests(
    mut requests: EventReader<BoneCoinCraftRequest>,
    mut crafted: EventWriter<BoneCoinCrafted>,
    mut players: Query<(&mut PlayerInventory, &mut Cultivation)>,
    item_registry: Option<Res<ItemRegistry>>,
    decay_profiles: Option<Res<DecayProfileRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
) {
    let (Some(item_registry), Some(decay_profiles), Some(allocator)) = (
        item_registry.as_deref(),
        decay_profiles.as_deref(),
        allocator.as_deref_mut(),
    ) else {
        return;
    };

    for request in requests.read() {
        let Ok((mut inventory, mut cultivation)) = players.get_mut(request.session.player) else {
            continue;
        };
        let Ok(outcome) = apply_bone_coin_craft_session(
            &request.session,
            &mut inventory,
            &mut cultivation,
            item_registry,
            decay_profiles,
            allocator,
        ) else {
            continue;
        };
        crafted.send(outcome);
    }
}

pub fn apply_bone_coin_craft_session(
    session: &BoneCoinCraftSession,
    inventory: &mut PlayerInventory,
    cultivation: &mut Cultivation,
    item_registry: &ItemRegistry,
    decay_profiles: &DecayProfileRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Result<BoneCoinCrafted, BoneCoinCraftError> {
    let bone = inventory_item_by_instance_borrow(inventory, session.bone_instance_id)
        .ok_or(BoneCoinCraftError::BoneMissing)?;
    let has_catalyst = session.catalyst_instance_id.is_some();
    let plan = plan_bone_coin_craft(&bone.template_id, session.qi_invest, has_catalyst)?;
    if cultivation.qi_current + f64::EPSILON < plan.total_qi_cost {
        return Err(BoneCoinCraftError::InsufficientQi);
    }

    if let Some(catalyst_id) = session.catalyst_instance_id {
        let catalyst = inventory_item_by_instance_borrow(inventory, catalyst_id)
            .ok_or(BoneCoinCraftError::CatalystMissing)?;
        if catalyst.template_id != ZHEN_SHI_CHU {
            return Err(BoneCoinCraftError::CatalystWrongItem);
        }
    }

    let mut output = build_fauna_item_instance(
        plan.output_template,
        1,
        session.issued_at_tick,
        item_registry,
        Some(decay_profiles),
        allocator,
    )
    .map_err(|_| BoneCoinCraftError::ItemBuildFailed)?;
    output.spirit_quality = (plan.sealed_qi / plan.bone_grade.qi_cap()).clamp(0.0, 1.0);
    let output_instance_id = output.instance_id;
    let output_template = output.template_id.clone();

    consume_item_instance_once(inventory, session.bone_instance_id)
        .map_err(|_| BoneCoinCraftError::InventoryConsumeFailed)?;
    if let Some(catalyst_id) = session.catalyst_instance_id {
        consume_item_instance_once(inventory, catalyst_id)
            .map_err(|_| BoneCoinCraftError::InventoryConsumeFailed)?;
    }
    cultivation.qi_current = (cultivation.qi_current - plan.total_qi_cost).max(0.0);
    force_attach_item_to_inventory(inventory, output);
    bump_revision(inventory);

    Ok(BoneCoinCrafted {
        player: session.player,
        bone_instance_id: session.bone_instance_id,
        catalyst_instance_id: session.catalyst_instance_id,
        output_instance_id,
        output_template,
        sealed_qi: plan.sealed_qi,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemTemplate,
        PlacedItemState, MAIN_PACK_CONTAINER_ID,
    };

    fn template(id: &str, category: ItemCategory) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: id.to_string(),
            effect: None,
            cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
            cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
        }
    }

    fn registry() -> ItemRegistry {
        ItemRegistry::from_map(HashMap::from([
            (SHU_GU.to_string(), template(SHU_GU, ItemCategory::Misc)),
            (ZHU_GU.to_string(), template(ZHU_GU, ItemCategory::Misc)),
            (
                FENG_HE_GU.to_string(),
                template(FENG_HE_GU, ItemCategory::Misc),
            ),
            (
                YI_SHOU_GU.to_string(),
                template(YI_SHOU_GU, ItemCategory::Misc),
            ),
            (
                ZHEN_SHI_CHU.to_string(),
                template(ZHEN_SHI_CHU, ItemCategory::Misc),
            ),
            (
                "bone_coin_5".to_string(),
                template("bone_coin_5", ItemCategory::BoneCoin),
            ),
            (
                "bone_coin_15".to_string(),
                template("bone_coin_15", ItemCategory::BoneCoin),
            ),
            (
                "bone_coin_40".to_string(),
                template("bone_coin_40", ItemCategory::BoneCoin),
            ),
        ]))
    }

    fn item(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: template_id.to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        }
    }

    fn inventory_with(items: Vec<ItemInstance>) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main".to_string(),
                rows: 5,
                cols: 7,
                items: items
                    .into_iter()
                    .enumerate()
                    .map(|(idx, instance)| PlacedItemState {
                        row: 0,
                        col: idx as u8,
                        instance,
                    })
                    .collect(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    #[test]
    fn craft_plan_caps_by_bone_grade_and_uses_catalyst_to_waive_seal_cost() {
        let plan = plan_bone_coin_craft(FENG_HE_GU, 80.0, true).unwrap();
        assert_eq!(plan.bone_grade, BoneGrade::Hybrid);
        assert_eq!(plan.sealed_qi, 40.0);
        assert_eq!(plan.total_qi_cost, 40.0);
        assert_eq!(plan.output_template, "bone_coin_40");

        let without_catalyst = plan_bone_coin_craft(ZHU_GU, 10.0, false).unwrap();
        assert_eq!(without_catalyst.sealed_qi, 10.0);
        assert_eq!(without_catalyst.total_qi_cost, 12.0);
        assert_eq!(without_catalyst.output_template, "bone_coin_15");
    }

    #[test]
    fn craft_rejects_non_bone_or_invalid_qi() {
        assert_eq!(
            plan_bone_coin_craft("spirit_grass", 5.0, true),
            Err(BoneCoinCraftError::NotFaunaBone)
        );
        assert_eq!(
            plan_bone_coin_craft(SHU_GU, 0.0, true),
            Err(BoneCoinCraftError::InvalidQi)
        );
    }

    #[test]
    fn craft_session_consumes_bone_catalyst_and_adds_fresh_bone_coin() {
        let item_registry = registry();
        let decay_profiles = crate::shelflife::build_default_registry();
        let mut allocator = InventoryInstanceIdAllocator::new(1000);
        let mut inventory = inventory_with(vec![item(1, FENG_HE_GU), item(2, ZHEN_SHI_CHU)]);
        let mut cultivation = Cultivation {
            qi_current: 45.0,
            ..Default::default()
        };
        let player = Entity::from_raw(9);

        let crafted = apply_bone_coin_craft_session(
            &BoneCoinCraftSession {
                player,
                bone_instance_id: 1,
                catalyst_instance_id: Some(2),
                qi_invest: 50.0,
                issued_at_tick: 88,
            },
            &mut inventory,
            &mut cultivation,
            &item_registry,
            &decay_profiles,
            &mut allocator,
        )
        .expect("craft should succeed");

        assert_eq!(crafted.output_template, "bone_coin_40");
        assert_eq!(crafted.sealed_qi, 40.0);
        assert_eq!(cultivation.qi_current, 5.0);
        let container = &inventory.containers[0];
        assert!(!container
            .items
            .iter()
            .any(|placed| placed.instance.instance_id == 1 || placed.instance.instance_id == 2));
        let coin = container
            .items
            .iter()
            .find(|placed| placed.instance.template_id == "bone_coin_40")
            .expect("crafted coin should be attached");
        assert_eq!(
            coin.instance
                .freshness
                .as_ref()
                .expect("coin should decay")
                .profile
                .as_str(),
            "bone_coin_40_v1"
        );
    }

    #[test]
    fn craft_session_rejects_insufficient_qi_before_consuming_items() {
        let item_registry = registry();
        let decay_profiles = crate::shelflife::build_default_registry();
        let mut allocator = InventoryInstanceIdAllocator::new(1000);
        let mut inventory = inventory_with(vec![item(1, ZHU_GU)]);
        let mut cultivation = Cultivation {
            qi_current: 1.0,
            ..Default::default()
        };

        let err = apply_bone_coin_craft_session(
            &BoneCoinCraftSession {
                player: Entity::from_raw(1),
                bone_instance_id: 1,
                catalyst_instance_id: None,
                qi_invest: 15.0,
                issued_at_tick: 1,
            },
            &mut inventory,
            &mut cultivation,
            &item_registry,
            &decay_profiles,
            &mut allocator,
        )
        .unwrap_err();

        assert_eq!(err, BoneCoinCraftError::InsufficientQi);
        assert!(inventory.containers[0]
            .items
            .iter()
            .any(|placed| placed.instance.instance_id == 1));
    }
}
