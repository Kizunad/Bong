use valence::prelude::{bevy_ecs, Entity, Events};

use crate::combat::components::{DerivedAttrs, SkillBarBindings};
use crate::combat::CombatClock;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::inventory::{PlayerInventory, EQUIP_SLOT_FALSE_SKIN};
use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason};
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};

use super::events::{
    ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent, PermanentTaintAbsorbedEvent,
    TuikeSkillId, TuikeSkillVisual,
};
use super::physics::{
    max_layers_for_realm, shed_start_cost, transfer_taint_to_outer_skin,
    ACTIVE_SHED_COOLDOWN_TICKS, TRANSFER_PERMANENT_COOLDOWN_TICKS,
};
use super::state::{
    false_skin_tier_for_item, FalseSkinLayer, FalseSkinResidue, PermanentQiMaxDecay,
    StackedFalseSkins, WornFalseSkin,
};

pub const TUIKE_DON_SKILL_ID: &str = "tuike.don";
pub const TUIKE_SHED_SKILL_ID: &str = "tuike.shed";
pub const TUIKE_TRANSFER_TAINT_SKILL_ID: &str = "tuike.transfer_taint";

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(TUIKE_DON_SKILL_ID, cast_don);
    registry.register(TUIKE_SHED_SKILL_ID, cast_shed);
    registry.register(TUIKE_TRANSFER_TAINT_SKILL_ID, cast_transfer_taint);
}

pub fn cast_don(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = now_tick(world);
    if on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::QiInsufficient);
    };
    let Some((instance_id, tier, spirit_quality)) = equipped_false_skin(world, caster) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    if !super::physics::can_wear_tier(cultivation.realm, tier) {
        return rejected(CastRejectReason::RealmTooLow);
    }

    let max_layers = max_layers_for_realm(cultivation.realm);
    let layer = FalseSkinLayer::new(instance_id, tier, spirit_quality, now_tick);
    let layers_after = {
        let mut entity = world.entity_mut(caster);
        let mut stack = entity.take::<StackedFalseSkins>().unwrap_or_default();
        if stack
            .outer()
            .is_some_and(|outer| outer.instance_id == instance_id)
        {
            let layers_after = stack.layer_count() as u8;
            entity.insert(stack);
            layers_after
        } else if !stack.push_outer(layer, max_layers) {
            entity.insert(stack);
            return rejected(CastRejectReason::InvalidTarget);
        } else {
            let layers_after = stack.layer_count() as u8;
            let outer = stack.outer().map(WornFalseSkin::from);
            entity.insert(stack);
            if let Some(outer) = outer {
                entity.insert(outer);
            }
            layers_after
        }
    };
    if let Some(mut attrs) = world.get_mut::<DerivedAttrs>(caster) {
        attrs.tuike_layers = layers_after;
    }

    set_cooldown(world, caster, slot, now_tick, 20);
    emit_if_present(
        world,
        DonFalseSkinEvent {
            caster,
            tier,
            layers_after,
            tick: now_tick,
            visual: TuikeSkillVisual::for_skill(TuikeSkillId::Don, false).into(),
        },
    );
    record_practice(world, caster, TuikeSkillId::Don, 1);

    CastResult::Started {
        cooldown_ticks: 20,
        anim_duration_ticks: 12,
    }
}

pub fn cast_shed(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = now_tick(world);
    if on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let has_stack = world
        .get::<StackedFalseSkins>(caster)
        .is_some_and(|stack| !stack.is_empty());
    if !has_stack {
        return rejected(CastRejectReason::InvalidTarget);
    }
    let cost = {
        let Some(cultivation) = world.get::<Cultivation>(caster) else {
            return rejected(CastRejectReason::QiInsufficient);
        };
        shed_start_cost(cultivation.qi_current)
    };
    if !spend_qi(world, caster, cost, "tuike_shed") {
        return rejected(CastRejectReason::QiInsufficient);
    }
    let Some(_shed) = shed_outer_layer(world, caster, None, 0.0, 0.0, true, now_tick) else {
        return rejected(CastRejectReason::InvalidTarget);
    };

    set_cooldown(world, caster, slot, now_tick, ACTIVE_SHED_COOLDOWN_TICKS);
    record_practice(world, caster, TuikeSkillId::Shed, 2);
    CastResult::Started {
        cooldown_ticks: ACTIVE_SHED_COOLDOWN_TICKS,
        anim_duration_ticks: 8,
    }
}

pub fn cast_transfer_taint(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = now_tick(world);
    if on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::QiInsufficient);
    };
    let available_contam = contamination_total_percent(world.get::<Contamination>(caster));
    if available_contam <= f64::EPSILON && world.get::<PermanentQiMaxDecay>(caster).is_none() {
        return rejected(CastRejectReason::InvalidTarget);
    }
    let permanent = world
        .get::<PermanentQiMaxDecay>(caster)
        .map(|marker| marker.amount);

    let outcome = {
        let Some(mut stack) = world.entity_mut(caster).take::<StackedFalseSkins>() else {
            return rejected(CastRejectReason::InvalidTarget);
        };
        let outcome = transfer_taint_to_outer_skin(
            &mut stack,
            cultivation.realm,
            available_contam,
            cultivation.qi_current,
            permanent,
        );
        let outer = stack.outer().map(WornFalseSkin::from);
        world.entity_mut(caster).insert(stack);
        if let Some(outer) = outer {
            world.entity_mut(caster).insert(outer);
        }
        let Some(outcome) = outcome else {
            return rejected(CastRejectReason::InvalidTarget);
        };
        outcome
    };

    if outcome.qi_cost > 0.0 && !spend_qi(world, caster, outcome.qi_cost, "tuike_transfer_taint") {
        return rejected(CastRejectReason::QiInsufficient);
    }
    drain_contamination(world, caster, outcome.contam_moved_percent, now_tick);
    if outcome.backflow_percent > 0.0 {
        add_backflow_contamination(world, caster, outcome.backflow_percent, now_tick);
    }
    if outcome.permanent_absorbed > 0.0 {
        world.entity_mut(caster).remove::<PermanentQiMaxDecay>();
        if let Some(mut stack) = world.get_mut::<StackedFalseSkins>(caster) {
            stack.transfer_permanent_cooldown_until_tick =
                now_tick.saturating_add(TRANSFER_PERMANENT_COOLDOWN_TICKS);
        }
        let tier = world
            .get::<WornFalseSkin>(caster)
            .map(|skin| skin.tier)
            .expect("outer skin was present for permanent absorb");
        emit_if_present(
            world,
            PermanentTaintAbsorbedEvent {
                caster,
                amount: outcome.permanent_absorbed,
                tier,
                tick: now_tick,
            },
        );
    }
    let Some(tier) = world.get::<WornFalseSkin>(caster).map(|skin| skin.tier) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    emit_if_present(
        world,
        ContamTransferredEvent {
            caster,
            tier,
            contam_moved_percent: outcome.contam_moved_percent,
            backflow_percent: outcome.backflow_percent,
            permanent_absorbed: outcome.permanent_absorbed,
            qi_cost: outcome.qi_cost,
            tick: now_tick,
            visual: TuikeSkillVisual::for_skill(
                TuikeSkillId::TransferTaint,
                outcome.permanent_absorbed > 0.0,
            )
            .into(),
        },
    );

    set_cooldown(
        world,
        caster,
        slot,
        now_tick,
        TRANSFER_PERMANENT_COOLDOWN_TICKS,
    );
    record_practice(world, caster, TuikeSkillId::TransferTaint, 1);
    CastResult::Started {
        cooldown_ticks: TRANSFER_PERMANENT_COOLDOWN_TICKS,
        anim_duration_ticks: 10,
    }
}

pub fn shed_outer_layer(
    world: &mut bevy_ecs::world::World,
    owner: Entity,
    attacker: Option<Entity>,
    damage_absorbed: f64,
    damage_overflow: f64,
    active: bool,
    now_tick: u64,
) -> Option<FalseSkinSheddedEvent> {
    let mut stack = world.entity_mut(owner).take::<StackedFalseSkins>()?;
    let layer = stack.shed_outer(now_tick)?;
    let layers_after = stack.layer_count() as u8;
    let outer = stack.outer().map(WornFalseSkin::from);
    if stack.is_empty() {
        world.entity_mut(owner).remove::<WornFalseSkin>();
    } else if let Some(outer) = outer {
        world.entity_mut(owner).insert(outer);
    }
    if let Some(mut attrs) = world.get_mut::<DerivedAttrs>(owner) {
        attrs.tuike_layers = layers_after;
    }
    world.entity_mut(owner).insert(stack);
    let residue_decay = super::physics::residue_decay_ticks_for_tier(layer.tier);
    world.spawn(FalseSkinResidue {
        owner,
        tier: layer.tier,
        contam_load: layer.contam_load,
        permanent_taint_load: layer.permanent_taint_load,
        dropped_at_tick: now_tick,
        decay_at_tick: now_tick.saturating_add(residue_decay),
        picked_up: false,
    });
    let event = FalseSkinSheddedEvent {
        owner,
        attacker,
        tier: layer.tier,
        damage_absorbed,
        damage_overflow,
        contam_load: layer.contam_load,
        permanent_taint_load: layer.permanent_taint_load,
        layers_after,
        active,
        tick: now_tick,
        visual: TuikeSkillVisual::for_skill(
            TuikeSkillId::Shed,
            layer.tier == super::state::FalseSkinTier::Ancient,
        )
        .into(),
    };
    emit_if_present(world, event.clone());
    Some(event)
}

fn equipped_false_skin(
    world: &bevy_ecs::world::World,
    caster: Entity,
) -> Option<(u64, super::state::FalseSkinTier, f64)> {
    let inventory = world.get::<PlayerInventory>(caster)?;
    let item = inventory.equipped.get(EQUIP_SLOT_FALSE_SKIN)?;
    let tier = false_skin_tier_for_item(item.template_id.as_str())?;
    Some((item.instance_id, tier, item.spirit_quality.max(0.1)))
}

fn now_tick(world: &bevy_ecs::world::World) -> u64 {
    world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default()
}

fn on_cooldown(world: &bevy_ecs::world::World, caster: Entity, slot: u8, now_tick: u64) -> bool {
    world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
}

fn set_cooldown(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    now_tick: u64,
    duration: u64,
) {
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, now_tick.saturating_add(duration));
    }
}

fn spend_qi(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    amount: f64,
    sink: &'static str,
) -> bool {
    if amount <= f64::EPSILON {
        return true;
    }
    {
        let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) else {
            return false;
        };
        if cultivation.qi_current + f64::EPSILON < amount {
            return false;
        }
        cultivation.qi_current = (cultivation.qi_current - amount).clamp(0.0, cultivation.qi_max);
    }
    if let Ok(transfer) = QiTransfer::new(
        QiAccountId::player(format!("entity:{}", caster.to_bits())),
        QiAccountId::container(format!("{sink}:{}", caster.to_bits())),
        amount,
        QiTransferReason::Channeling,
    ) {
        emit_if_present(world, transfer);
    }
    true
}

fn contamination_total_percent(contamination: Option<&Contamination>) -> f64 {
    contamination
        .map(|contamination| contamination.entries.iter().map(|entry| entry.amount).sum())
        .unwrap_or(0.0)
}

fn drain_contamination(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    mut amount: f64,
    _now_tick: u64,
) {
    let Some(mut contamination) = world.get_mut::<Contamination>(caster) else {
        return;
    };
    for entry in &mut contamination.entries {
        if amount <= f64::EPSILON {
            break;
        }
        let take = entry.amount.min(amount);
        entry.amount -= take;
        amount -= take;
    }
    contamination
        .entries
        .retain(|entry| entry.amount > f64::EPSILON);
}

fn add_backflow_contamination(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    amount: f64,
    now_tick: u64,
) {
    if amount <= f64::EPSILON {
        return;
    }
    let source = ContamSource {
        amount,
        color: ColorKind::Insidious,
        meridian_id: None,
        attacker_id: None,
        introduced_at: now_tick,
    };
    if let Some(mut contamination) = world.get_mut::<Contamination>(caster) {
        contamination.entries.push(source);
    } else {
        world.entity_mut(caster).insert(Contamination {
            entries: vec![source],
        });
    }
}

fn record_practice(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    skill: TuikeSkillId,
    amount: u32,
) {
    if let Some(mut log) = world.get_mut::<PracticeLog>(caster) {
        for _ in 0..amount {
            record_style_practice(&mut log, ColorKind::Solid);
        }
    }
    emit_if_present(
        world,
        SkillXpGain {
            char_entity: caster,
            skill: SkillId::Combat,
            amount,
            source: XpGainSource::Action {
                plan_id: "tuike_v2",
                action: skill.payload_kind(),
            },
        },
    );
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

fn emit_if_present<T: valence::prelude::Event>(world: &mut bevy_ecs::world::World, event: T) {
    if let Some(mut events) = world.get_resource_mut::<Events<T>>() {
        events.send(event);
    }
}
