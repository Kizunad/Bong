use serde::{Deserialize, Serialize};
use valence::prelude::Entity;

use super::components::{PoisonPowderKind, PoisonToxicity};
use super::events::PoisonPowderConsumedEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoisonAttackKind {
    Anqi,
    Zhenfa,
    Baomai,
    Dugu,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoisonDebuffTier {
    None,
    Mild,
    Moderate,
    Severe,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PoisonDebuff {
    pub tier: PoisonDebuffTier,
    pub damage_per_second: f32,
    pub duration_ticks: u64,
}

impl PoisonDebuff {
    pub const NONE: Self = Self {
        tier: PoisonDebuffTier::None,
        damage_per_second: 0.0,
        duration_ticks: 0,
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoisonAttackResolution {
    pub final_damage: f32,
    pub toxicity_debuff: Option<PoisonDebuff>,
    pub powder_debuff: Option<PoisonDebuff>,
    pub consumed_powder: Option<PoisonPowderConsumedEvent>,
}

pub trait PoisonAttackModifier {
    fn apply_modifier(
        &self,
        caster: Entity,
        target: Option<Entity>,
        base_damage: f32,
        kind: PoisonAttackKind,
        powder: Option<PoisonPowderKind>,
        at_tick: u64,
    ) -> PoisonAttackResolution;
}

pub struct PoisonToxicityModifier<'a> {
    pub toxicity: Option<&'a PoisonToxicity>,
}

impl<'a> PoisonAttackModifier for PoisonToxicityModifier<'a> {
    fn apply_modifier(
        &self,
        caster: Entity,
        target: Option<Entity>,
        base_damage: f32,
        kind: PoisonAttackKind,
        powder: Option<PoisonPowderKind>,
        at_tick: u64,
    ) -> PoisonAttackResolution {
        apply_poison_attack_modifier(
            caster,
            target,
            self.toxicity,
            base_damage,
            kind,
            powder,
            at_tick,
        )
    }
}

pub fn apply_poison_attack_modifier(
    caster: Entity,
    target: Option<Entity>,
    toxicity: Option<&PoisonToxicity>,
    base_damage: f32,
    kind: PoisonAttackKind,
    powder: Option<PoisonPowderKind>,
    at_tick: u64,
) -> PoisonAttackResolution {
    let base_damage = base_damage.max(0.0);
    let toxicity_debuff = toxicity
        .filter(|_| kind != PoisonAttackKind::Dugu)
        .and_then(|toxicity| poison_debuff_for_toxicity(toxicity.level));
    let powder_debuff = powder.map(poison_debuff_for_powder);
    let toxicity_multiplier = toxicity_debuff
        .map(|debuff| match debuff.tier {
            PoisonDebuffTier::Mild => 1.05,
            PoisonDebuffTier::Moderate => 1.10,
            PoisonDebuffTier::Severe => 1.20,
            PoisonDebuffTier::None => 1.0,
        })
        .unwrap_or(1.0);
    let powder_add = powder_debuff
        .map(|debuff| debuff.damage_per_second * (debuff.duration_ticks as f32 / 20.0))
        .unwrap_or(0.0);
    PoisonAttackResolution {
        final_damage: base_damage * toxicity_multiplier + powder_add,
        toxicity_debuff,
        powder_debuff,
        consumed_powder: powder.map(|powder| PoisonPowderConsumedEvent {
            player: caster,
            powder,
            target,
            at_tick,
        }),
    }
}

pub fn poison_debuff_for_toxicity(level: f32) -> Option<PoisonDebuff> {
    if level < 30.0 {
        None
    } else if level <= 70.0 {
        Some(PoisonDebuff {
            tier: PoisonDebuffTier::Mild,
            damage_per_second: 2.0,
            duration_ticks: 5 * 20,
        })
    } else {
        Some(PoisonDebuff {
            tier: PoisonDebuffTier::Severe,
            damage_per_second: 5.0,
            duration_ticks: 8 * 20,
        })
    }
}

pub fn poison_debuff_for_powder(powder: PoisonPowderKind) -> PoisonDebuff {
    let spec = powder.spec();
    let tier = if spec.damage_per_second < 4.0 {
        PoisonDebuffTier::Mild
    } else if spec.damage_per_second < 8.0 {
        PoisonDebuffTier::Moderate
    } else {
        PoisonDebuffTier::Severe
    };
    PoisonDebuff {
        tier,
        damage_per_second: spec.damage_per_second,
        duration_ticks: u64::from(spec.duration_seconds) * 20,
    }
}
