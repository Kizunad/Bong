use crate::combat::components::TICKS_PER_SECOND;
use crate::cultivation::components::Realm;

use super::events::BaomaiSkillId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountainShakeProfile {
    pub qi_cost: f64,
    pub radius_blocks: f32,
    pub shock_damage: f32,
    pub cooldown_ticks: u64,
    pub cast_ticks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloodBurnProfile {
    pub hp_burn: f32,
    pub qi_multiplier: f32,
    pub duration_ticks: u64,
    pub cooldown_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisperseProfile {
    pub qi_max_loss_ratio: f64,
    pub flow_rate_multiplier: f64,
    pub duration_ticks: u64,
    pub has_transcendence: bool,
}

pub fn skill_factor(skill_lv: u8) -> f64 {
    f64::from(skill_lv.min(100)) / 100.0
}

pub fn cooldown_ticks_linear(start_seconds: f64, end_seconds: f64, skill_lv: u8) -> u64 {
    let seconds = start_seconds + (end_seconds - start_seconds) * skill_factor(skill_lv);
    (seconds.max(0.05) * TICKS_PER_SECOND as f64).round() as u64
}

pub fn beng_quan_cooldown_ticks(skill_lv: u8) -> u64 {
    cooldown_ticks_linear(3.0, 0.5, skill_lv)
}

pub fn full_power_charge_rate_per_tick(skill_lv: u8) -> f64 {
    50.0 + 150.0 * skill_factor(skill_lv)
}

pub fn full_power_exhausted_duration_multiplier(skill_lv: u8) -> f64 {
    1.0 - skill_factor(skill_lv) * 0.30
}

pub fn beng_quan_overload_multiplier(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 1.0,
        Realm::Induce => 1.2,
        Realm::Condense | Realm::Solidify | Realm::Spirit => 1.5,
        Realm::Void => 1.6,
    }
}

pub fn beng_quan_qi_cost(qi_max: f64) -> f64 {
    qi_max.max(0.0) * 0.40
}

pub fn mountain_shake_profile(realm: Realm, skill_lv: u8) -> MountainShakeProfile {
    let (qi_cost, radius, damage) = match realm {
        Realm::Awaken => (25.0, 3.0, 5.0),
        Realm::Induce => (25.0, 4.0, 12.0),
        Realm::Condense => (30.0, 5.0, 35.0),
        Realm::Solidify => (35.0, 6.0, 90.0),
        Realm::Spirit => (40.0, 7.0, 220.0),
        Realm::Void => (50.0, 10.0, 850.0),
    };
    MountainShakeProfile {
        qi_cost,
        radius_blocks: radius,
        shock_damage: damage,
        cooldown_ticks: cooldown_ticks_linear(30.0, 10.0, skill_lv),
        cast_ticks: 12,
    }
}

pub fn blood_burn_profile(realm: Realm, skill_lv: u8) -> BloodBurnProfile {
    let (hp_burn, base_multiplier, duration_seconds) = match realm {
        Realm::Awaken => (10.0, 1.2, 10.0),
        Realm::Induce => (20.0, 1.5, 15.0),
        Realm::Condense => (50.0, 2.0, 20.0),
        Realm::Solidify => (100.0, 2.5, 25.0),
        Realm::Spirit => (200.0, 3.0, 30.0),
        Realm::Void => (300.0, 4.0, 30.0),
    };
    let mastery_bonus = 1.0 + 0.05 * skill_factor(skill_lv);
    BloodBurnProfile {
        hp_burn,
        qi_multiplier: (base_multiplier * mastery_bonus) as f32,
        duration_ticks: (duration_seconds * TICKS_PER_SECOND as f64) as u64,
        cooldown_ticks: cooldown_ticks_linear(60.0, 20.0, skill_lv),
    }
}

pub fn disperse_profile(realm: Realm, skill_lv: u8) -> DisperseProfile {
    match realm {
        Realm::Void => DisperseProfile {
            qi_max_loss_ratio: 0.50,
            flow_rate_multiplier: 10.0,
            duration_ticks: cooldown_ticks_linear(5.0, 8.0, skill_lv),
            has_transcendence: true,
        },
        _ => DisperseProfile {
            qi_max_loss_ratio: 0.05,
            flow_rate_multiplier: 1.0,
            duration_ticks: 0,
            has_transcendence: false,
        },
    }
}

pub fn skill_qi_multiplier(
    blood_burn_multiplier: f32,
    flow_rate_multiplier: f64,
    heavy_color_multiplier: f64,
) -> f64 {
    f64::from(blood_burn_multiplier.max(1.0))
        * flow_rate_multiplier.max(1.0)
        * heavy_color_multiplier.max(1.0)
}

pub fn overload_severity(skill: BaomaiSkillId, flow_rate_multiplier: f64) -> f64 {
    let base = match skill {
        BaomaiSkillId::BengQuan => 0.05,
        BaomaiSkillId::FullPowerCharge | BaomaiSkillId::FullPowerRelease => 0.08,
        BaomaiSkillId::MountainShake => 0.04,
        BaomaiSkillId::BloodBurn => 0.02,
        BaomaiSkillId::Disperse => 0.0,
    };
    if flow_rate_multiplier > 1.0 {
        0.0
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beng_quan_cooldown_matches_mastery_bounds() {
        assert_eq!(beng_quan_cooldown_ticks(0), 60);
        assert_eq!(beng_quan_cooldown_ticks(100), 10);
    }

    #[test]
    fn full_power_charge_rate_scales_to_plan_bound() {
        assert_eq!(full_power_charge_rate_per_tick(0), 50.0);
        assert_eq!(full_power_charge_rate_per_tick(100), 200.0);
    }

    #[test]
    fn mountain_shake_void_profile_uses_ten_block_radius() {
        let profile = mountain_shake_profile(Realm::Void, 0);
        assert_eq!(profile.qi_cost, 50.0);
        assert_eq!(profile.radius_blocks, 10.0);
        assert_eq!(profile.shock_damage, 850.0);
    }

    #[test]
    fn blood_burn_mastery_adds_small_efficiency_bonus() {
        assert!(blood_burn_profile(Realm::Void, 100).qi_multiplier > 4.0);
        assert_eq!(blood_burn_profile(Realm::Void, 0).duration_ticks, 600);
    }

    #[test]
    fn disperse_void_burns_half_pool_for_transcendence() {
        let profile = disperse_profile(Realm::Void, 0);
        assert!(profile.has_transcendence);
        assert_eq!(profile.qi_max_loss_ratio, 0.5);
        assert_eq!(profile.flow_rate_multiplier, 10.0);
    }

    #[test]
    fn disperse_sub_spirit_only_punishes_without_window() {
        let profile = disperse_profile(Realm::Condense, 0);
        assert!(!profile.has_transcendence);
        assert_eq!(profile.qi_max_loss_ratio, 0.05);
    }
}
