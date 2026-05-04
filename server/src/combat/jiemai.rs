use valence::entity::Look;
use valence::prelude::{bevy_ecs, DVec3, Entity, Events};

use crate::combat::components::{
    DefenseWindow, SkillBarBindings, JIEMAI_CONCUSSION_BASE_SEVERITY, JIEMAI_CONTAM_MULTIPLIER,
    JIEMAI_PREP_WINDOW_MS,
};
use crate::combat::events::{DefenseIntent, StatusEffectKind};
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::inventory::{
    PlayerInventory, EQUIP_SLOT_CHEST, EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS,
};

pub const ZHENMAI_PARRY_SKILL_ID: &str = "zhenmai.parry";
pub const JIEMAI_PARRY_RECOVERY_TICKS: u64 = 10;
pub const JIEMAI_PARRY_RECOVERY_MOVE_SPEED_MULTIPLIER: f32 = 0.7;

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(ZHENMAI_PARRY_SKILL_ID, resolve_zhenmai_parry_skill);
}

pub fn resolve_zhenmai_parry_skill(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default();

    if world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
    {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    if jiemai_qi_cost_for_realm(cultivation.realm).is_none() {
        return rejected(CastRejectReason::RealmTooLow);
    }
    if world
        .get::<crate::combat::components::StatusEffects>(caster)
        .is_some_and(|statuses| {
            crate::combat::status::has_active_status(statuses, StatusEffectKind::ParryRecovery)
        })
    {
        return rejected(CastRejectReason::InRecovery);
    }

    let Some(mut defense_events) = world.get_resource_mut::<Events<DefenseIntent>>() else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    defense_events.send(DefenseIntent {
        defender: caster,
        issued_at_tick: now_tick,
    });

    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, now_tick.saturating_add(JIEMAI_PARRY_RECOVERY_TICKS));
    }
    CastResult::Started {
        cooldown_ticks: JIEMAI_PARRY_RECOVERY_TICKS,
        anim_duration_ticks: 1,
    }
}

pub fn jiemai_qi_cost_for_realm(realm: Realm) -> Option<f64> {
    match realm {
        Realm::Awaken => None,
        Realm::Induce => Some(5.0),
        Realm::Condense => Some(6.0),
        Realm::Solidify => Some(8.0),
        Realm::Spirit => Some(10.0),
        Realm::Void => None,
    }
}

pub fn jiemai_prep_window(
    inventory: Option<&PlayerInventory>,
    opened_at_tick: u64,
) -> DefenseWindow {
    DefenseWindow {
        opened_at_tick,
        duration_ms: jiemai_prep_window_ms(inventory),
    }
}

pub fn jiemai_prep_window_ms(inventory: Option<&PlayerInventory>) -> u32 {
    let modifier = inventory
        .map(jiemai_armor_modifier_from_inventory)
        .unwrap_or(1.0);
    ((JIEMAI_PREP_WINDOW_MS as f32 * modifier).round() as u32).max(1)
}

pub fn jiemai_armor_modifier_from_inventory(inventory: &PlayerInventory) -> f32 {
    let heaviest = [
        EQUIP_SLOT_HEAD,
        EQUIP_SLOT_CHEST,
        EQUIP_SLOT_LEGS,
        EQUIP_SLOT_FEET,
    ]
    .into_iter()
    .filter_map(|slot| inventory.equipped.get(slot))
    .map(|item| item.weight)
    .fold(0.0_f64, f64::max);
    jiemai_armor_modifier_for_item_weight(heaviest)
}

pub fn jiemai_armor_modifier_for_item_weight(weight: f64) -> f32 {
    if weight >= 5.0 {
        0.6
    } else if weight >= 2.0 {
        0.9
    } else {
        1.0
    }
}

pub fn jiemai_effectiveness(hit_distance: f32) -> f32 {
    if hit_distance >= 2.0 {
        1.0
    } else if hit_distance <= 0.9 {
        0.3
    } else {
        0.3 + (hit_distance - 0.9) / (2.0 - 0.9) * 0.7
    }
}

pub fn jiemai_contam_multiplier_for_effectiveness(effectiveness: f32) -> f64 {
    let effectiveness = effectiveness.clamp(0.3, 1.0) as f64;
    1.0 - (1.0 - JIEMAI_CONTAM_MULTIPLIER) * effectiveness
}

pub fn jiemai_concussion_severity_for_effectiveness(effectiveness: f32) -> f32 {
    JIEMAI_CONCUSSION_BASE_SEVERITY / effectiveness.clamp(0.3, 1.0)
}

pub fn jiemai_apply_effects(
    effectiveness: f32,
    contam_amount: &mut f64,
    concussion_severity: &mut f32,
) {
    *contam_amount *= jiemai_contam_multiplier_for_effectiveness(effectiveness);
    *concussion_severity = jiemai_concussion_severity_for_effectiveness(effectiveness);
}

pub fn jiemai_fov_dot_threshold(realm: Realm) -> f64 {
    match realm {
        Realm::Induce => 0.0,
        Realm::Condense => -0.17,
        Realm::Solidify => -0.71,
        Realm::Spirit | Realm::Void => -1.0,
        Realm::Awaken => 0.0,
    }
}

pub fn jiemai_fov_check(
    attacker_pos: DVec3,
    defender_pos: DVec3,
    defender_look: Option<&Look>,
    realm: Realm,
) -> bool {
    if matches!(realm, Realm::Spirit | Realm::Void) {
        return true;
    }
    let Some(look) = defender_look else {
        return true;
    };
    let to_attacker = DVec3::new(
        attacker_pos.x - defender_pos.x,
        0.0,
        attacker_pos.z - defender_pos.z,
    );
    let len_sq = to_attacker.length_squared();
    if len_sq <= f64::EPSILON {
        return true;
    }
    let yaw = f64::from(look.yaw).to_radians();
    let facing = DVec3::new(-yaw.sin(), 0.0, yaw.cos());
    facing.dot(to_attacker / len_sq.sqrt()) >= jiemai_fov_dot_threshold(realm)
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity};
    use std::collections::HashMap;

    fn armor_item(weight: f64) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: "armor".to_string(),
            display_name: "armor".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight,
            rarity: ItemRarity::Common,
            description: String::new(),
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
            alchemy: None,
        }
    }

    #[test]
    fn realm_qi_cost_rejects_awaken_and_scales_v1_realms() {
        assert_eq!(jiemai_qi_cost_for_realm(Realm::Awaken), None);
        assert_eq!(jiemai_qi_cost_for_realm(Realm::Induce), Some(5.0));
        assert_eq!(jiemai_qi_cost_for_realm(Realm::Condense), Some(6.0));
        assert_eq!(jiemai_qi_cost_for_realm(Realm::Solidify), Some(8.0));
        assert_eq!(jiemai_qi_cost_for_realm(Realm::Spirit), Some(10.0));
        assert_eq!(jiemai_qi_cost_for_realm(Realm::Void), None);
    }

    #[test]
    fn distance_effectiveness_clamps_close_and_far_with_linear_middle() {
        assert!((jiemai_effectiveness(0.5) - 0.3).abs() < 1e-6);
        assert!((jiemai_effectiveness(2.5) - 1.0).abs() < 1e-6);
        assert!((jiemai_effectiveness(1.45) - 0.65).abs() < 1e-5);
    }

    #[test]
    fn effectiveness_applies_dual_axis_contam_and_concussion() {
        let mut contam = 10.0;
        let mut severity = JIEMAI_CONCUSSION_BASE_SEVERITY;
        jiemai_apply_effects(0.3, &mut contam, &mut severity);
        assert!((contam - 7.6).abs() < 1e-6);
        assert!((severity - 1.0).abs() < 1e-6);

        let mut full_contam = 10.0;
        let mut full_severity = JIEMAI_CONCUSSION_BASE_SEVERITY;
        jiemai_apply_effects(1.0, &mut full_contam, &mut full_severity);
        assert!((full_contam - 2.0).abs() < 1e-9);
        assert!((full_severity - 0.3).abs() < 1e-6);
    }

    #[test]
    fn armor_weight_modifies_prep_window() {
        let mut equipped = HashMap::new();
        equipped.insert(EQUIP_SLOT_CHEST.to_string(), armor_item(2.5));
        let inventory = PlayerInventory {
            revision: InventoryRevision(0),
            containers: Vec::new(),
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        };
        assert_eq!(jiemai_prep_window_ms(Some(&inventory)), 900);

        let mut heavy = inventory.clone();
        heavy
            .equipped
            .insert(EQUIP_SLOT_LEGS.to_string(), armor_item(7.0));
        assert_eq!(jiemai_prep_window_ms(Some(&heavy)), 600);
        assert_eq!(jiemai_prep_window_ms(None), 1000);
    }

    #[test]
    fn fov_check_uses_realm_thresholds() {
        let defender = DVec3::ZERO;
        let front = DVec3::new(0.0, 0.0, 1.0);
        let back = DVec3::new(0.0, 0.0, -1.0);
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };

        assert!(jiemai_fov_check(
            front,
            defender,
            Some(&look),
            Realm::Induce
        ));
        assert!(!jiemai_fov_check(
            back,
            defender,
            Some(&look),
            Realm::Induce
        ));
        assert!(jiemai_fov_check(back, defender, Some(&look), Realm::Spirit));
    }
}
