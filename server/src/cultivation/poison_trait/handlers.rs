use super::components::{
    digestion_capacity_for_realm, DigestionLoad, PoisonDoseRecord, PoisonPillKind,
    PoisonSideEffectTag, PoisonToxicity,
};
use super::events::{PoisonDoseEvent, PoisonOverdoseEvent, PoisonOverdoseSeverity};
use crate::cultivation::components::Realm;
use valence::prelude::Entity;

#[derive(Debug, Clone, PartialEq)]
pub struct PoisonConsumeOutcome {
    pub dose_event: PoisonDoseEvent,
    pub overdose_event: Option<PoisonOverdoseEvent>,
    pub base_lifespan_cost_years: f32,
}

pub fn consume_poison_pill_now(
    player: Entity,
    pill: PoisonPillKind,
    realm: Realm,
    toxicity: &mut PoisonToxicity,
    digestion: &mut DigestionLoad,
    at_tick: u64,
) -> PoisonConsumeOutcome {
    let spec = pill.spec();
    digestion.capacity = digestion.capacity.max(digestion_capacity_for_realm(realm));

    toxicity.level = (toxicity.level + spec.poison_amount).clamp(0.0, 100.0);
    toxicity.last_dose_tick = at_tick;
    toxicity.last_decay_tick = at_tick;
    toxicity.source_history.push(PoisonDoseRecord {
        tick: at_tick,
        dose_amount: spec.poison_amount,
        side_effect_tag: spec.side_effect_tag,
    });
    if spec.side_effect_tag == PoisonSideEffectTag::ToxicityTierUnlock {
        toxicity.toxicity_tier_unlocked = true;
    }

    if spec.side_effect_tag == PoisonSideEffectTag::DigestLock6h {
        digestion.digest_lock_until_tick = Some(at_tick + spec.side_effect_tag.duration_ticks());
    }
    let projected_digestion = digestion.current + spec.digestion_load;
    let overflow = (projected_digestion - digestion.capacity).max(0.0);
    digestion.current = projected_digestion.clamp(0.0, digestion.capacity);
    digestion.last_decay_tick = at_tick;

    let dose_event = PoisonDoseEvent {
        player,
        dose_amount: spec.poison_amount,
        side_effect_tag: spec.side_effect_tag,
        poison_level_after: toxicity.level,
        digestion_after: digestion.current,
        at_tick,
    };

    let overdose_event =
        calculate_overdose_severity(overflow, digestion.capacity).map(|severity| {
            PoisonOverdoseEvent {
                player,
                severity,
                overflow,
                lifespan_penalty_years: severity.lifespan_penalty_years(),
                micro_tear_probability: severity
                    .micro_tear_probability()
                    .max(spec.micro_tear_probability),
                at_tick,
            }
        });

    PoisonConsumeOutcome {
        dose_event,
        overdose_event,
        base_lifespan_cost_years: spec.lifespan_years,
    }
}

pub fn calculate_overdose_severity(overflow: f32, capacity: f32) -> Option<PoisonOverdoseSeverity> {
    if overflow <= 0.0 || capacity <= 0.0 || !overflow.is_finite() || !capacity.is_finite() {
        return None;
    }
    let ratio = overflow / capacity;
    Some(if ratio <= 0.20 {
        PoisonOverdoseSeverity::Mild
    } else if ratio <= 0.60 {
        PoisonOverdoseSeverity::Moderate
    } else {
        PoisonOverdoseSeverity::Severe
    })
}

pub fn poison_side_effect_tag_for_item(item_id: &str) -> Option<PoisonSideEffectTag> {
    PoisonPillKind::from_item_id(item_id).map(|pill| pill.spec().side_effect_tag)
}
