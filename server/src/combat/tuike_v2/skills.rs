use valence::prelude::{bevy_ecs, DVec3, Entity, Events, Position, UniqueId};

use crate::combat::components::{DerivedAttrs, SkillBarBindings};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::combat::CombatClock;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation};
use crate::cultivation::meridian::severed::SkillMeridianDependencies;
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::inventory::{consume_item_instance_once, PlayerInventory, EQUIP_SLOT_FALSE_SKIN};
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::{qi_release_to_zone, QiAccountId, QiTransfer, QiTransferReason};
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

use super::events::{
    ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent, PermanentTaintAbsorbedEvent,
    TuikeSkillId, TuikeSkillVisual,
};
use super::physics::{
    max_layers_for_realm, shed_start_cost, transfer_cooldown_ticks, transfer_taint_to_outer_skin,
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

pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    dependencies.declare(TUIKE_DON_SKILL_ID, Vec::new());
    dependencies.declare(TUIKE_SHED_SKILL_ID, Vec::new());
    dependencies.declare(TUIKE_TRANSFER_TAINT_SKILL_ID, Vec::new());
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
        let duplicate_outer = stack
            .outer()
            .is_some_and(|outer| outer.instance_id == instance_id);
        if duplicate_outer || !stack.push_outer(layer, max_layers) {
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

    if let Some(pos) = world.get::<Position>(caster).map(|p| p.get()) {
        emit_vfx(world, pos, "bong:false_skin_don_dust", "#D8C08A", 0.75, 10, 34);
        emit_audio(world, "don_skin_low_thud", pos);
        emit_anim(world, caster, "bong:tuike_don_skin");
    }

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

    if let Some(pos) = world.get::<Position>(caster).map(|p| p.get()) {
        emit_vfx(world, pos, "bong:false_skin_shed_burst", "#B58B5A", 0.9, 18, 34);
        emit_audio(world, "shed_skin_burst", pos);
        emit_anim(world, caster, "bong:tuike_shed_burst");
    }

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

    let cooldown_ticks = transfer_cooldown_ticks(outcome.permanent_absorbed);
    set_cooldown(world, caster, slot, now_tick, cooldown_ticks);
    record_practice(world, caster, TuikeSkillId::TransferTaint, 1);

    if let Some(pos) = world.get::<Position>(caster).map(|p| p.get()) {
        let vfx_id = if outcome.permanent_absorbed > 0.0 { "bong:ancient_skin_glow" } else { "bong:false_skin_don_dust" };
        let color = if outcome.permanent_absorbed > 0.0 { "#BFD8FF" } else { "#D8C08A" };
        emit_vfx(world, pos, vfx_id, color, 0.8, 12, 40);
        emit_audio(world, "contam_transfer_hum", pos);
        emit_anim(world, caster, "bong:tuike_taint_transfer");
    }

    CastResult::Started {
        cooldown_ticks,
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
    if let Some(mut inventory) = world.get_mut::<PlayerInventory>(owner) {
        let _ = consume_item_instance_once(&mut inventory, layer.instance_id);
    }
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
    if !amount.is_finite() {
        return false;
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
    emit_spent_qi_release(world, caster, amount, sink);
    true
}

fn emit_spent_qi_release(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    amount: f64,
    sink: &'static str,
) {
    let from = QiAccountId::player(format!("entity:{}", caster.to_bits()));
    let position = world.get::<Position>(caster).map(|position| position.get());
    let dimension = world
        .get::<CurrentDimension>(caster)
        .map(|dimension| dimension.0)
        .unwrap_or(DimensionKind::Overworld);

    let mut transfers = Vec::new();
    if let (Some(position), Some(mut zones)) = (position, world.get_resource_mut::<ZoneRegistry>())
    {
        let zone_name = zones
            .find_zone(dimension, position)
            .map(|zone| zone.name.clone());
        if let Some(zone_name) = zone_name {
            if let Some(zone) = zones.find_zone_mut(zone_name.as_str()) {
                let to = QiAccountId::zone(zone.name.clone());
                let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
                match qi_release_to_zone(
                    amount,
                    from.clone(),
                    to,
                    zone_current,
                    QI_ZONE_UNIT_CAPACITY,
                ) {
                    Ok(outcome) => {
                        zone.spirit_qi =
                            (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                        if let Some(transfer) = outcome.transfer {
                            transfers.push(transfer);
                        }
                        if outcome.overflow > QI_EPSILON {
                            push_spent_qi_overflow(
                                &mut transfers,
                                from.clone(),
                                outcome.overflow,
                                sink,
                                caster,
                            );
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "[bong][tuike_v2] invalid spent qi release for {:?}; route to overflow",
                            caster
                        );
                        push_spent_qi_overflow(&mut transfers, from.clone(), amount, sink, caster);
                    }
                }
            } else {
                push_spent_qi_overflow(&mut transfers, from.clone(), amount, sink, caster);
            }
        } else {
            push_spent_qi_overflow(&mut transfers, from.clone(), amount, sink, caster);
        }
    } else {
        push_spent_qi_overflow(&mut transfers, from.clone(), amount, sink, caster);
    }

    for transfer in transfers {
        emit_if_present(world, transfer);
    }
}

fn push_spent_qi_overflow(
    transfers: &mut Vec<QiTransfer>,
    from: QiAccountId,
    amount: f64,
    sink: &'static str,
    caster: Entity,
) {
    if amount <= QI_EPSILON {
        return;
    }
    match QiTransfer::new(
        from,
        QiAccountId::overflow(format!("{sink}:{}", caster.to_bits())),
        amount,
        QiTransferReason::ReleaseToZone,
    ) {
        Ok(transfer) => transfers.push(transfer),
        Err(error) => tracing::warn!(
            ?error,
            sink,
            ?caster,
            amount,
            "[bong][tuike_v2] failed to build spent qi overflow transfer"
        ),
    }
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

fn emit_vfx(
    world: &mut bevy_ecs::world::World,
    origin: DVec3,
    event_id: &str,
    color: &str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) {
    if let Some(mut events) = world.get_resource_mut::<Events<VfxEventRequest>>() {
        events.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::SpawnParticle {
                event_id: event_id.to_string(),
                origin: [origin.x, origin.y + 1.0, origin.z],
                direction: None,
                color: Some(color.to_string()),
                strength: Some(strength.clamp(0.0, 1.0)),
                count: Some(count),
                duration_ticks: Some(duration_ticks),
            },
        ));
    }
}

fn emit_audio(world: &mut bevy_ecs::world::World, recipe: &str, origin: DVec3) {
    if let Some(mut events) = world.get_resource_mut::<Events<PlaySoundRecipeRequest>>() {
        events.send(PlaySoundRecipeRequest {
            recipe_id: recipe.to_string(),
            instance_id: 0,
            pos: None,
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin,
                radius: AUDIO_BROADCAST_RADIUS,
            },
        });
    }
}

fn emit_anim(world: &mut bevy_ecs::world::World, entity: Entity, anim_id: &str) {
    let origin = world
        .get::<Position>(entity)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);
    let unique_id = world.get::<UniqueId>(entity).map(|id| id.0.to_string());
    if let (Some(target_player), Some(mut events)) = (
        unique_id,
        world.get_resource_mut::<Events<VfxEventRequest>>(),
    ) {
        events.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::PlayAnim {
                target_player,
                anim_id: anim_id.to_string(),
                priority: 1200,
                fade_in_ticks: Some(2),
            },
        ));
    }
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

fn emit_if_present<T: valence::prelude::Event>(world: &mut bevy_ecs::world::World, event: T) {
    if let Some(mut events) = world.get_resource_mut::<Events<T>>() {
        events.send(event);
    }
}
