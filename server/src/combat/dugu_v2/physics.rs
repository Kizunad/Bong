use crate::combat::components::TICKS_PER_SECOND;
use crate::cultivation::components::{ColorKind, Cultivation, Realm};
use crate::qi_physics::constants::{DUGU_DIRTY_QI_ZONE_RETURN_RATIO, DUGU_RHO};
use crate::qi_physics::{
    qi_collision, EnvField, MediumKind, QiAccountId, StyleAttack, StyleDefense,
};

use super::events::{DuguSkillId, TaintTier};

pub const SELF_CURE_DAILY_CAP_HOURS: f32 = 6.0;
pub const SELF_CURE_REVEAL_THRESHOLD_PERCENT: f32 = 60.0;
pub const SELF_CURE_SOFT_CAP_PERCENT: f32 = 95.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirtyQiOutcome {
    pub injected_qi: f32,
    pub effective_hit: f32,
    pub rejected_qi: f32,
    pub returned_zone_qi: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EclipseEffect {
    pub hp_loss: f32,
    pub qi_loss: f32,
    pub tier: TaintTier,
    pub temporary_qi_max_loss_fraction: f32,
    pub permanent_decay_rate_per_min: f32,
    pub hud_hint: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenetrateSpec {
    pub multiplier: f32,
    pub extra_permanent_decay_rate_per_min: f32,
    pub radius_blocks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShroudSpec {
    pub strength: f32,
    pub duration_ticks: u64,
    pub permanent_until_cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuguSkillSpec {
    pub skill: DuguSkillId,
    pub qi_cost: f64,
    pub cooldown_ticks: u64,
    pub cast_ticks: u32,
}

pub fn skill_spec(skill: DuguSkillId) -> DuguSkillSpec {
    match skill {
        DuguSkillId::Eclipse => DuguSkillSpec {
            skill,
            qi_cost: 13.0,
            cooldown_ticks: 3 * TICKS_PER_SECOND,
            cast_ticks: 8,
        },
        DuguSkillId::SelfCure => DuguSkillSpec {
            skill,
            qi_cost: 10.0,
            cooldown_ticks: 6 * TICKS_PER_SECOND,
            cast_ticks: 20,
        },
        DuguSkillId::Penetrate => DuguSkillSpec {
            skill,
            qi_cost: 20.0,
            cooldown_ticks: 8 * TICKS_PER_SECOND,
            cast_ticks: 12,
        },
        DuguSkillId::Shroud => DuguSkillSpec {
            skill,
            qi_cost: 5.0,
            cooldown_ticks: 10 * TICKS_PER_SECOND,
            cast_ticks: 10,
        },
        DuguSkillId::Reverse => DuguSkillSpec {
            skill,
            qi_cost: 50.0,
            cooldown_ticks: 60 * TICKS_PER_SECOND,
            cast_ticks: 30,
        },
    }
}

pub fn eclipse_effect(victim_realm: Realm, self_cure_percent: f32) -> EclipseEffect {
    let multiplier = 1.0 + self_cure_percent.clamp(0.0, 100.0) / 100.0 * 2.0;
    let (hp, qi, tier, temp, decay, hint) = match victim_realm {
        Realm::Awaken => (2.0, 3.0, TaintTier::Immediate, 0.0, 0.0, "蚀针擦皮"),
        Realm::Induce => (5.0, 8.0, TaintTier::Immediate, 0.0, 0.0, "蚀针入肉"),
        Realm::Condense => (10.0, 15.0, TaintTier::Immediate, 0.0, 0.0, "蚀针深刺"),
        Realm::Solidify => (15.0, 25.0, TaintTier::Temporary, 0.03, 0.0, "蛊毒入脉"),
        Realm::Spirit => (20.0, 40.0, TaintTier::Permanent, 0.0, 0.0005, "蛊毒入髓"),
        Realm::Void => (40.0, 100.0, TaintTier::Permanent, 0.0, 0.001, "蛊毒入魂"),
    };
    EclipseEffect {
        hp_loss: hp * multiplier,
        qi_loss: qi * multiplier,
        tier,
        temporary_qi_max_loss_fraction: temp * multiplier,
        permanent_decay_rate_per_min: decay * multiplier,
        hud_hint: hint,
    }
}

pub fn self_cure_gain_percent(
    current_percent: f32,
    requested_hours: f32,
    already_today: f32,
) -> f32 {
    if !requested_hours.is_finite() || !already_today.is_finite() {
        return 0.0;
    }
    let remaining_today = (SELF_CURE_DAILY_CAP_HOURS - already_today.max(0.0)).max(0.0);
    let hours = requested_hours.clamp(0.0, remaining_today);
    let current = current_percent.clamp(0.0, SELF_CURE_SOFT_CAP_PERCENT);
    let diminishing = (1.0 - current / 90.0).max(0.0).powi(2);
    1.5 * diminishing * hours
}

pub fn reveal_probability(
    caster_realm: Realm,
    shroud_strength: f32,
    distance_blocks: f32,
    victim_realm: Realm,
) -> f32 {
    let base = match caster_realm {
        Realm::Awaken => 0.05,
        Realm::Induce => 0.04,
        Realm::Condense => 0.03,
        Realm::Solidify => 0.02,
        Realm::Spirit => 0.01,
        Realm::Void => 0.002,
    };
    let distance = if distance_blocks <= 5.0 {
        1.0
    } else if distance_blocks <= 15.0 {
        0.7
    } else {
        0.4
    };
    let victim_factor = if realm_rank(victim_realm) >= realm_rank(Realm::Solidify) {
        3.0
    } else {
        1.0
    };
    (base * (1.0 - shroud_strength.clamp(0.0, 0.95)) * distance * victim_factor).clamp(0.0, 1.0)
}

pub fn shroud_spec(realm: Realm) -> ShroudSpec {
    match realm {
        Realm::Awaken => ShroudSpec {
            strength: 0.20,
            duration_ticks: 60 * TICKS_PER_SECOND,
            permanent_until_cancelled: false,
        },
        Realm::Induce => ShroudSpec {
            strength: 0.30,
            duration_ticks: 3 * 60 * TICKS_PER_SECOND,
            permanent_until_cancelled: false,
        },
        Realm::Condense => ShroudSpec {
            strength: 0.50,
            duration_ticks: 5 * 60 * TICKS_PER_SECOND,
            permanent_until_cancelled: false,
        },
        Realm::Solidify => ShroudSpec {
            strength: 0.70,
            duration_ticks: 10 * 60 * TICKS_PER_SECOND,
            permanent_until_cancelled: false,
        },
        Realm::Spirit => ShroudSpec {
            strength: 0.85,
            duration_ticks: 30 * 60 * TICKS_PER_SECOND,
            permanent_until_cancelled: false,
        },
        Realm::Void => ShroudSpec {
            strength: 0.95,
            duration_ticks: u64::MAX / 4,
            permanent_until_cancelled: true,
        },
    }
}

pub fn penetrate_spec(realm: Realm) -> PenetrateSpec {
    match realm {
        Realm::Awaken => PenetrateSpec {
            multiplier: 1.5,
            extra_permanent_decay_rate_per_min: 0.0,
            radius_blocks: 0.0,
        },
        Realm::Induce => PenetrateSpec {
            multiplier: 1.8,
            extra_permanent_decay_rate_per_min: 0.0,
            radius_blocks: 0.0,
        },
        Realm::Condense => PenetrateSpec {
            multiplier: 2.0,
            extra_permanent_decay_rate_per_min: 0.0,
            radius_blocks: 0.0,
        },
        Realm::Solidify => PenetrateSpec {
            multiplier: 2.5,
            extra_permanent_decay_rate_per_min: 0.0,
            radius_blocks: 0.0,
        },
        Realm::Spirit => PenetrateSpec {
            multiplier: 3.0,
            extra_permanent_decay_rate_per_min: 0.001,
            radius_blocks: 0.0,
        },
        Realm::Void => PenetrateSpec {
            multiplier: 5.0,
            extra_permanent_decay_rate_per_min: 0.002,
            radius_blocks: f32::INFINITY,
        },
    }
}

pub fn dirty_qi_collision(
    injected_qi: f64,
    defender_resistance: f64,
    distance_blocks: f64,
) -> DirtyQiOutcome {
    let attack = DirtyQiAttack {
        injected_qi,
        purity: 1.0 - DUGU_RHO,
    };
    let defense = DirtyQiDefense {
        resistance: defender_resistance,
    };
    let out = qi_collision(
        &QiAccountId::player("dugu_attacker"),
        &QiAccountId::player("dugu_victim"),
        &QiAccountId::zone("dugu_zone"),
        &attack,
        &defense,
        distance_blocks,
        &EnvField::default().with_dugu_taint_residue(1.0),
    );
    DirtyQiOutcome {
        injected_qi: injected_qi.max(0.0) as f32,
        effective_hit: out.effective_hit as f32,
        rejected_qi: (out.attenuated_qi - out.effective_hit).max(0.0) as f32,
        returned_zone_qi: (injected_qi.max(0.0) * DUGU_DIRTY_QI_ZONE_RETURN_RATIO) as f32,
    }
}

pub fn fake_qi_color_for_realm(realm: Realm) -> crate::cultivation::components::QiColor {
    let mut color = crate::cultivation::components::QiColor::default();
    match realm {
        Realm::Awaken => {
            color.secondary = None;
            color.is_chaotic = false;
        }
        Realm::Induce => color.main = ColorKind::Heavy,
        Realm::Condense => color.main = ColorKind::Solid,
        Realm::Solidify => color.main = ColorKind::Sharp,
        Realm::Spirit | Realm::Void => {
            color.main = ColorKind::Heavy;
            color.secondary = Some(ColorKind::Solid);
            color.is_hunyuan = realm == Realm::Void;
        }
    }
    color
}

pub fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

pub fn defender_resistance(cultivation: &Cultivation) -> f64 {
    (0.05 + f64::from(realm_rank(cultivation.realm)) * 0.04).clamp(0.0, 0.35)
}

struct DirtyQiAttack {
    injected_qi: f64,
    purity: f64,
}

impl StyleAttack for DirtyQiAttack {
    fn style_color(&self) -> ColorKind {
        ColorKind::Insidious
    }

    fn injected_qi(&self) -> f64 {
        self.injected_qi
    }

    fn purity(&self) -> f64 {
        self.purity
    }

    fn medium(&self) -> MediumKind {
        MediumKind::bare(ColorKind::Insidious)
    }
}

struct DirtyQiDefense {
    resistance: f64,
}

impl StyleDefense for DirtyQiDefense {
    fn defense_color(&self) -> ColorKind {
        ColorKind::Mellow
    }

    fn resistance(&self) -> f64 {
        self.resistance
    }
}
