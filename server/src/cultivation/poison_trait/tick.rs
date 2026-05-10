use valence::prelude::{Entity, EventReader, EventWriter, Query, Res};

use super::components::{
    DigestionLoad, PoisonToxicity, DIGESTION_DECAY_PER_HOUR, POISON_DECAY_PER_HOUR_HEAVY,
    POISON_DECAY_PER_HOUR_LIGHT,
};
use super::events::{
    ConsumePoisonPillIntent, DigestionOverloadEvent, PoisonDoseEvent, PoisonOverdoseEvent,
};
use super::handlers::consume_poison_pill_now;
use crate::cultivation::components::{CrackCause, Cultivation, Realm};
use crate::cultivation::lifespan::LifespanComponent;
use crate::cultivation::overload::MeridianCrackEvent;
use crate::cultivation::tick::CultivationClock;

pub const TICKS_PER_HOUR: u64 = 60 * 60 * 20;
pub const HEAVY_TOXICITY_THRESHOLD: f32 = 70.0;

pub fn decay_poison_toxicity(toxicity: &mut PoisonToxicity, elapsed_ticks: u64) -> f32 {
    if elapsed_ticks == 0 || toxicity.level <= 0.0 {
        return 0.0;
    }
    let hours = elapsed_ticks as f32 / TICKS_PER_HOUR as f32;
    let per_hour = if toxicity.level > HEAVY_TOXICITY_THRESHOLD {
        POISON_DECAY_PER_HOUR_HEAVY
    } else {
        POISON_DECAY_PER_HOUR_LIGHT
    };
    let amount = (hours * per_hour).min(toxicity.level);
    toxicity.level = (toxicity.level - amount).max(0.0);
    amount
}

pub fn decay_digestion_load(
    digestion: &mut DigestionLoad,
    now_tick: u64,
    elapsed_ticks: u64,
) -> f32 {
    if elapsed_ticks == 0 || digestion.current <= 0.0 {
        return 0.0;
    }
    let hours = elapsed_ticks as f32 / TICKS_PER_HOUR as f32;
    let locked = digestion
        .digest_lock_until_tick
        .is_some_and(|until| now_tick < until);
    let rate = if locked {
        digestion.decay_rate * 0.5
    } else {
        digestion.decay_rate
    }
    .max(0.0);
    let amount = (hours * rate).min(digestion.current);
    digestion.current = (digestion.current - amount).max(0.0);
    if digestion
        .digest_lock_until_tick
        .is_some_and(|until| now_tick >= until)
    {
        digestion.digest_lock_until_tick = None;
    }
    amount
}

pub fn consume_poison_pill_system(
    clock: Res<CultivationClock>,
    mut intents: EventReader<ConsumePoisonPillIntent>,
    mut dose_events: EventWriter<PoisonDoseEvent>,
    mut overdose_events: EventWriter<PoisonOverdoseEvent>,
    mut digestion_events: EventWriter<DigestionOverloadEvent>,
    mut players: Query<(
        Entity,
        &mut PoisonToxicity,
        &mut DigestionLoad,
        Option<&Cultivation>,
    )>,
) {
    for intent in intents.read() {
        let Ok((_entity, mut toxicity, mut digestion, cultivation)) =
            players.get_mut(intent.entity)
        else {
            tracing::warn!(
                "[bong][poison_trait] dropped consume intent for {:?}: missing poison components",
                intent.entity
            );
            continue;
        };
        let realm = cultivation.map_or(Realm::Awaken, |c| c.realm);
        let at_tick = intent.issued_at_tick.max(clock.tick);
        let outcome = consume_poison_pill_now(
            intent.entity,
            intent.pill,
            realm,
            &mut toxicity,
            &mut digestion,
            at_tick,
        );
        if let Some(overdose) = outcome.overdose_event {
            digestion_events.send(DigestionOverloadEvent {
                player: overdose.player,
                current: digestion.current,
                capacity: digestion.capacity,
                overflow: overdose.overflow,
                at_tick: overdose.at_tick,
            });
            overdose_events.send(overdose);
        }
        dose_events.send(outcome.dose_event);
    }
}

pub fn poison_toxicity_decay_tick(
    clock: Res<CultivationClock>,
    mut players: Query<&mut PoisonToxicity>,
) {
    let now = clock.tick;
    for mut toxicity in players.iter_mut() {
        let elapsed = now.saturating_sub(toxicity.last_decay_tick);
        decay_poison_toxicity(&mut toxicity, elapsed);
        toxicity.last_decay_tick = now;
    }
}

pub fn digestion_load_decay_tick(
    clock: Res<CultivationClock>,
    mut players: Query<&mut DigestionLoad>,
) {
    let now = clock.tick;
    for mut digestion in players.iter_mut() {
        let elapsed = now.saturating_sub(digestion.last_decay_tick);
        if digestion.decay_rate <= 0.0 {
            digestion.decay_rate = DIGESTION_DECAY_PER_HOUR;
        }
        decay_digestion_load(&mut digestion, now, elapsed);
        digestion.last_decay_tick = now;
    }
}

pub fn apply_poison_overdose_costs(
    mut overdose_events: EventReader<PoisonOverdoseEvent>,
    mut lifespans: Query<&mut LifespanComponent>,
    mut crack_events: EventWriter<MeridianCrackEvent>,
) {
    for event in overdose_events.read() {
        if let Ok(mut lifespan) = lifespans.get_mut(event.player) {
            lifespan.years_lived =
                (lifespan.years_lived + f64::from(event.lifespan_penalty_years)).max(0.0);
        }
        if poison_micro_tear_roll(event.player, event.at_tick) < event.micro_tear_probability {
            crack_events.send(MeridianCrackEvent {
                target: event.player,
                severity: event.severity.micro_tear_severity(),
                cause: CrackCause::Backfire,
                created_at: event.at_tick,
            });
        }
    }
}

pub fn poison_micro_tear_roll(entity: Entity, tick: u64) -> f32 {
    let mut x = entity.to_bits() ^ tick.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x as f64 / u64::MAX as f64) as f32
}
