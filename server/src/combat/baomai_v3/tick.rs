use valence::prelude::{Commands, Entity, EventWriter, Query, Res};

use crate::combat::CombatClock;
use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation};

use super::events::BodyTranscendenceExpiredEvent;
use super::state::{BloodBurnActive, BodyTranscendence};

pub fn blood_burn_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &BloodBurnActive,
        Option<&Cultivation>,
        Option<&mut Contamination>,
    )>,
) {
    for (entity, active, cultivation, contamination) in &mut q {
        if active.is_active_at(clock.tick) {
            continue;
        }
        if let (Some(cultivation), Some(mut contamination)) = (cultivation, contamination) {
            contamination.entries.push(ContamSource {
                amount: cultivation.qi_max.max(0.0) * 0.05,
                color: ColorKind::Violent,
                meridian_id: None,
                attacker_id: Some("baomai:blood_burn".to_string()),
                introduced_at: clock.tick,
            });
        }
        commands.entity(entity).remove::<BloodBurnActive>();
    }
}

pub fn body_transcendence_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut expired: EventWriter<BodyTranscendenceExpiredEvent>,
    mut q: Query<(
        Entity,
        &BodyTranscendence,
        Option<&mut crate::cultivation::components::MeridianSystem>,
    )>,
) {
    for (entity, active, meridians) in &mut q {
        if active.is_active_at(clock.tick) {
            continue;
        }
        if let Some(mut meridians) = meridians {
            for (id, original_rate) in &active.original_flow_rates {
                meridians.get_mut(*id).flow_rate = *original_rate;
            }
        }
        commands.entity(entity).remove::<BodyTranscendence>();
        expired.send(BodyTranscendenceExpiredEvent {
            caster: entity,
            tick: clock.tick,
        });
    }
}
