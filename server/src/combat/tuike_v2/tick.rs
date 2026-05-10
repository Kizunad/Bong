use valence::prelude::{Commands, Entity, EventWriter, Query, Res};

use crate::combat::components::DerivedAttrs;
use crate::combat::CombatClock;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::Cultivation;
use crate::inventory::{consume_item_instance_once, PlayerInventory, EQUIP_SLOT_FALSE_SKIN};

use super::events::{
    FalseSkinDecayedToAshEvent, FalseSkinSheddedEvent, TuikeSkillId, TuikeSkillVisual,
};
use super::physics::maintenance_qi_per_sec;
use super::state::{
    false_skin_tier_for_item, FalseSkinLayer, FalseSkinResidue, StackedFalseSkins, WornFalseSkin,
};

type SyncFalseSkinItem<'a> = (
    Entity,
    &'a PlayerInventory,
    Option<&'a mut StackedFalseSkins>,
    Option<&'a WornFalseSkin>,
    Option<&'a mut DerivedAttrs>,
);
type MaintenanceFalseSkinItem<'a> = (
    Entity,
    &'a mut Cultivation,
    &'a mut StackedFalseSkins,
    Option<&'a PracticeLog>,
    Option<&'a mut DerivedAttrs>,
    Option<&'a mut PlayerInventory>,
);

pub fn sync_false_skin_stack_from_inventory(
    mut commands: Commands,
    clock: Option<Res<CombatClock>>,
    mut query: Query<SyncFalseSkinItem<'_>>,
) {
    let now_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (entity, inventory, stack, worn, attrs) in &mut query {
        let equipped = inventory.equipped.get(EQUIP_SLOT_FALSE_SKIN);
        let next = equipped.and_then(|item| {
            false_skin_tier_for_item(item.template_id.as_str())
                .map(|tier| (item.instance_id, tier, item.spirit_quality.max(0.1)))
        });
        let layers_after = match (next, stack, worn) {
            (Some((instance_id, _, _)), Some(stack), Some(worn))
                if worn.instance_id == instance_id
                    && stack
                        .outer()
                        .is_some_and(|outer| outer.instance_id == instance_id) =>
            {
                stack.layer_count() as u8
            }
            (Some((instance_id, tier, quality)), Some(mut stack), _) => {
                stack.layers.clear();
                let layer = FalseSkinLayer::new(instance_id, tier, quality, now_tick);
                stack.layers.push(layer.clone());
                commands.entity(entity).insert(WornFalseSkin::from(&layer));
                stack.layer_count() as u8
            }
            (Some((instance_id, tier, quality)), None, _) => {
                let layer = FalseSkinLayer::new(instance_id, tier, quality, now_tick);
                commands.entity(entity).insert((
                    StackedFalseSkins::with_layer(layer.clone()),
                    WornFalseSkin::from(&layer),
                ));
                1
            }
            (None, Some(_), _) => {
                commands
                    .entity(entity)
                    .remove::<StackedFalseSkins>()
                    .remove::<WornFalseSkin>();
                0
            }
            (None, None, Some(_)) => {
                commands.entity(entity).remove::<WornFalseSkin>();
                0
            }
            (None, None, None) => 0,
        };
        if let Some(mut attrs) = attrs {
            attrs.tuike_layers = layers_after;
        }
    }
}

pub fn false_skin_maintenance_tick(
    mut commands: Commands,
    clock: Option<Res<CombatClock>>,
    mut query: Query<MaintenanceFalseSkinItem<'_>>,
    mut shed_events: EventWriter<FalseSkinSheddedEvent>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    if tick % crate::combat::components::TICKS_PER_SECOND != 0 {
        return;
    }
    for (entity, mut cultivation, mut stack, practice, attrs, inventory) in &mut query {
        let cost = maintenance_qi_per_sec(&stack, practice);
        if cost <= f64::EPSILON {
            continue;
        }
        if cultivation.qi_current + f64::EPSILON < cost {
            shed_outer_layer_for_maintenance(
                &mut commands,
                entity,
                &mut stack,
                attrs,
                inventory,
                &mut shed_events,
                tick,
            );
            continue;
        }
        cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);
    }
}

fn shed_outer_layer_for_maintenance(
    commands: &mut Commands,
    owner: Entity,
    stack: &mut StackedFalseSkins,
    attrs: Option<valence::prelude::Mut<'_, DerivedAttrs>>,
    inventory: Option<valence::prelude::Mut<'_, PlayerInventory>>,
    shed_events: &mut EventWriter<FalseSkinSheddedEvent>,
    tick: u64,
) {
    let Some(layer) = stack.shed_outer(tick) else {
        return;
    };
    let layers_after = stack.layer_count() as u8;
    if let Some(mut attrs) = attrs {
        attrs.tuike_layers = layers_after;
    }
    if stack.is_empty() {
        commands.entity(owner).remove::<WornFalseSkin>();
    } else if let Some(outer) = stack.outer().map(WornFalseSkin::from) {
        commands.entity(owner).insert(outer);
    }
    if let Some(mut inventory) = inventory {
        let _ = consume_item_instance_once(&mut inventory, layer.instance_id);
    }

    let residue_decay = super::physics::residue_decay_ticks_for_tier(layer.tier);
    commands.spawn(FalseSkinResidue {
        owner,
        tier: layer.tier,
        contam_load: layer.contam_load,
        permanent_taint_load: layer.permanent_taint_load,
        dropped_at_tick: tick,
        decay_at_tick: tick.saturating_add(residue_decay),
        picked_up: false,
    });
    shed_events.send(FalseSkinSheddedEvent {
        owner,
        attacker: None,
        tier: layer.tier,
        damage_absorbed: 0.0,
        damage_overflow: 0.0,
        contam_load: layer.contam_load,
        permanent_taint_load: layer.permanent_taint_load,
        layers_after,
        active: false,
        tick,
        visual: TuikeSkillVisual::for_skill(
            TuikeSkillId::Shed,
            layer.tier == super::state::FalseSkinTier::Ancient,
        )
        .into(),
    });
}

pub fn false_skin_residue_decay_tick(
    mut commands: Commands,
    clock: Option<Res<CombatClock>>,
    mut residues: Query<(Entity, &FalseSkinResidue)>,
    mut events: EventWriter<FalseSkinDecayedToAshEvent>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (entity, residue) in &mut residues {
        if residue.picked_up || tick < residue.decay_at_tick {
            continue;
        }
        events.send(FalseSkinDecayedToAshEvent {
            owner: residue.owner,
            tier: residue.tier,
            output_item_id: residue.tier.residue_output_item_id().to_string(),
            tick,
        });
        commands.entity(entity).remove::<FalseSkinResidue>();
    }
}
