use valence::prelude::{Commands, Entity, EventWriter, Query, Res};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;

use super::events::PermanentQiMaxDecayApplied;
use super::state::{ReverseAftermathCloud, ShroudActive, TaintMark};

pub fn taint_decay_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut targets: Query<(Entity, &mut Cultivation, &TaintMark)>,
) {
    for (entity, mut cultivation, mark) in &mut targets {
        if mark
            .expires_at_tick
            .is_some_and(|expires| clock.tick >= expires)
        {
            if mark.temporary_qi_max_loss > 0.0 {
                cultivation.qi_max += f64::from(mark.temporary_qi_max_loss);
            }
            commands.entity(entity).remove::<TaintMark>();
        }
    }
}

pub fn permanent_qi_max_decay_tick(
    clock: Res<CombatClock>,
    mut targets: Query<(Entity, &mut Cultivation, &TaintMark)>,
    mut events: EventWriter<PermanentQiMaxDecayApplied>,
) {
    for (entity, mut cultivation, mark) in &mut targets {
        if !mark.is_permanent() || mark.permanent_decay_rate_per_min <= 0.0 {
            continue;
        }
        let per_tick =
            f64::from(mark.permanent_decay_rate_per_min) / 60.0 / TICKS_PER_SECOND as f64;
        let loss = (cultivation.qi_max * per_tick).max(0.0);
        if loss <= f64::EPSILON {
            continue;
        }
        cultivation.qi_max = (cultivation.qi_max - loss).max(0.0);
        cultivation.qi_current = cultivation.qi_current.min(cultivation.qi_max);
        events.send(PermanentQiMaxDecayApplied {
            target: entity,
            caster: mark.caster,
            loss: loss as f32,
            qi_max_after: cultivation.qi_max as f32,
            tick: clock.tick,
        });
    }
}

pub fn shroud_maintain_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut actors: Query<(Entity, &mut Cultivation, &ShroudActive)>,
) {
    for (entity, mut cultivation, shroud) in &mut actors {
        if !shroud.permanent_until_cancelled && clock.tick >= shroud.expires_at_tick {
            commands.entity(entity).remove::<ShroudActive>();
            continue;
        }
        if cultivation.qi_current <= shroud.maintain_qi_per_tick {
            commands.entity(entity).remove::<ShroudActive>();
            continue;
        }
        cultivation.qi_current =
            (cultivation.qi_current - shroud.maintain_qi_per_tick).clamp(0.0, cultivation.qi_max);
    }
}

pub fn reverse_aftermath_decay_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    clouds: Query<(Entity, &ReverseAftermathCloud)>,
) {
    for (entity, cloud) in &clouds {
        if clock.tick >= cloud.expires_at_tick {
            commands.entity(entity).remove::<ReverseAftermathCloud>();
        }
    }
}
