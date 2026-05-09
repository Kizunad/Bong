use valence::prelude::{Commands, DVec3, Entity, EventWriter, Position, Query, Res, With};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
use crate::world::dimension::{CurrentDimension, DimensionKind};

use super::backfire::apply_backfire_to_hand_meridians;
use super::events::{
    BackfireCauseV2, BackfireLevel, TurbulenceFieldDecayed, VortexBackfireEventV2, WoliuSkillId,
};
use super::physics::turbulence_decay_step;
use super::state::{TurbulenceExposure, TurbulenceField, VortexV2State};

const VOID_HEART_TRIBULATION_TICKS: u64 = 30 * TICKS_PER_SECOND;

pub fn turbulence_decay_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut fields: Query<(valence::prelude::Entity, &mut TurbulenceField)>,
    mut decayed_events: EventWriter<TurbulenceFieldDecayed>,
) {
    for (entity, mut field) in &mut fields {
        let elapsed_ticks = clock.tick.saturating_sub(field.last_decay_tick);
        if elapsed_ticks == 0 {
            continue;
        }
        field.last_decay_tick = clock.tick;
        let elapsed_seconds = elapsed_ticks as f64 / TICKS_PER_SECOND as f64;
        let (decayed, remaining) = turbulence_decay_step(
            f64::from(field.remaining_swirl_qi),
            f64::from(field.decay_rate_per_second),
            elapsed_seconds,
        );
        field.remaining_swirl_qi = remaining as f32;
        if decayed > f64::EPSILON {
            decayed_events.send(TurbulenceFieldDecayed {
                caster: field.caster,
                radius: field.radius,
                decayed_qi: decayed as f32,
                remaining_swirl_qi: field.remaining_swirl_qi,
                tick: clock.tick,
            });
        }
        if field.remaining_swirl_qi <= f32::EPSILON {
            commands.entity(entity).remove::<TurbulenceField>();
        }
    }
}

type TurbulenceTargetItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a CurrentDimension>,
    Option<&'a TurbulenceExposure>,
);

type TurbulenceFieldItem<'a> = (Entity, &'a TurbulenceField, Option<&'a CurrentDimension>);

pub fn update_turbulence_exposure_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    fields: Query<TurbulenceFieldItem<'_>>,
    targets: Query<TurbulenceTargetItem<'_>, With<Cultivation>>,
) {
    for (target, position, target_dim, current_exposure) in &targets {
        let target_dim = dimension_kind(target_dim);
        let mut strongest: Option<(Entity, f32)> = None;
        for (field_entity, field, field_dim) in &fields {
            if target == field.caster || target == field_entity {
                continue;
            }
            if dimension_kind(field_dim) != target_dim {
                continue;
            }
            if !within_radius(position.get(), field.center, field.radius) {
                continue;
            }
            if strongest
                .map(|(_, intensity)| field.intensity > intensity)
                .unwrap_or(true)
            {
                strongest = Some((field.caster, field.intensity));
            }
        }

        match (strongest, current_exposure) {
            (Some((source, intensity)), _) if intensity > f32::EPSILON => {
                commands.entity(target).insert(TurbulenceExposure::new(
                    source,
                    intensity,
                    clock.tick.saturating_add(1),
                ));
            }
            (None, Some(_)) => {
                commands.entity(target).remove::<TurbulenceExposure>();
            }
            _ => {}
        }
    }
}

pub fn heart_active_backfire_tick(
    clock: Res<CombatClock>,
    mut states: Query<(
        Entity,
        &mut VortexV2State,
        &Cultivation,
        &mut MeridianSystem,
        Option<&CurrentDimension>,
    )>,
    mut events: EventWriter<VortexBackfireEventV2>,
) {
    for (entity, mut state, cultivation, mut meridians, dimension) in &mut states {
        if state.active_skill_kind != WoliuSkillId::Heart
            || cultivation.realm != Realm::Void
            || state.backfire_level.is_some()
            || dimension_kind(dimension) == DimensionKind::Tsy
        {
            continue;
        }
        if clock.tick.saturating_sub(state.started_at_tick) < VOID_HEART_TRIBULATION_TICKS {
            continue;
        }
        apply_backfire_to_hand_meridians(&mut meridians, BackfireLevel::Severed);
        state.backfire_level = Some(BackfireLevel::Severed);
        events.send(VortexBackfireEventV2 {
            caster: entity,
            skill: WoliuSkillId::Heart,
            level: BackfireLevel::Severed,
            cause: BackfireCauseV2::VoidHeartTribulation,
            overflow_qi: 0.0,
            tick: clock.tick,
        });
    }
}

fn within_radius(target: DVec3, center: DVec3, radius: f32) -> bool {
    let radius = f64::from(radius.max(0.0));
    target.distance_squared(center) <= radius * radius
}

fn dimension_kind(dimension: Option<&CurrentDimension>) -> DimensionKind {
    dimension.map(|dimension| dimension.0).unwrap_or_default()
}
