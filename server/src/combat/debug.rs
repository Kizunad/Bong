use valence::prelude::{EventReader, EventWriter, ResMut};

use crate::combat::events::{AttackIntent, CombatEvent};
use crate::combat::CombatClock;

pub fn tick_combat_clock(mut clock: ResMut<CombatClock>) {
    clock.tick = clock.tick.saturating_add(1);
}

pub fn enqueue_debug_attack_intent(intents: &mut EventWriter<AttackIntent>, intent: AttackIntent) {
    intents.send(intent);
}

pub fn drain_combat_events_for_debug(mut events: EventReader<CombatEvent>) {
    for event in events.read() {
        tracing::debug!(
            "[bong][combat][debug] event attacker={:?} target={:?} tick={} desc={}",
            event.attacker,
            event.target,
            event.resolved_at_tick,
            event.description
        );
    }
}
