use valence::prelude::{Commands, Entity, EventWriter, Query, Res};

use crate::combat::components::DerivedAttrs;
use crate::combat::CombatClock;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::Cultivation;
use crate::inventory::{PlayerInventory, EQUIP_SLOT_FALSE_SKIN};

use super::events::FalseSkinDecayedToAshEvent;
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
    clock: Option<Res<CombatClock>>,
    mut query: Query<(
        Entity,
        &mut Cultivation,
        &StackedFalseSkins,
        Option<&PracticeLog>,
    )>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    if tick % crate::combat::components::TICKS_PER_SECOND != 0 {
        return;
    }
    for (_entity, mut cultivation, stack, practice) in &mut query {
        let cost = maintenance_qi_per_sec(stack, practice);
        if cost <= f64::EPSILON {
            continue;
        }
        cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);
    }
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
