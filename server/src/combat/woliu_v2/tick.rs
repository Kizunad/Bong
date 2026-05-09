use valence::prelude::{Commands, EventWriter, Query, Res};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;

use super::events::TurbulenceFieldDecayed;
use super::physics::turbulence_decay_step;
use super::state::TurbulenceField;

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
