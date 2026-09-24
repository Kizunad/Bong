use crate::cultivation::components::MeridianId;
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::meridian::severed::SkillMeridianDependencies;

pub const DASH_TECHNIQUE_ID: &str = "movement.dash";

pub fn dash_stamina_cost(proficiency: f32) -> f32 {
    15.0 - normalized(proficiency) * 6.0
}

pub fn dash_cooldown_ticks(proficiency: f32) -> u64 {
    (40.0 - normalized(proficiency) * 20.0).round() as u64
}

pub fn dash_distance(proficiency: f32) -> f32 {
    2.8 + normalized(proficiency)
}

pub fn known_dash_proficiency(known: &KnownTechniques) -> Option<f32> {
    known
        .entries
        .iter()
        .find(|entry| entry.id == DASH_TECHNIQUE_ID && entry.active)
        .map(|entry| entry.proficiency.clamp(0.0, 1.0))
}

pub fn record_dash_use(known: &mut KnownTechniques, in_combat: bool, iframe_success: bool) {
    let Some(entry) = known
        .entries
        .iter_mut()
        .find(|entry| entry.id == DASH_TECHNIQUE_ID && entry.active)
    else {
        return;
    };
    let gain = dash_proficiency_gain(entry.proficiency, in_combat, iframe_success);
    entry.proficiency = (entry.proficiency + gain).clamp(0.0, 1.0);
}

pub fn dash_proficiency_gain(current: f32, in_combat: bool, iframe_success: bool) -> f32 {
    let base = if current < 0.5 { 0.005 } else { 0.0025 };
    let combat_bonus = if in_combat { 0.003 } else { 0.0 };
    let iframe_bonus = if iframe_success { 0.005 } else { 0.0 };
    base + combat_bonus + iframe_bonus
}

pub fn declare_dash_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    dependencies.declare(
        DASH_TECHNIQUE_ID,
        vec![
            MeridianId::Stomach,
            MeridianId::Bladder,
            MeridianId::Kidney,
            MeridianId::Gallbladder,
        ],
    );
}

fn normalized(proficiency: f32) -> f32 {
    proficiency.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dash_scalars_match_0_50_100_table() {
        assert_eq!(dash_stamina_cost(0.0), 15.0);
        assert_eq!(dash_stamina_cost(0.5), 12.0);
        assert_eq!(dash_stamina_cost(1.0), 9.0);
        assert_eq!(dash_cooldown_ticks(0.0), 40);
        assert_eq!(dash_cooldown_ticks(0.5), 30);
        assert_eq!(dash_cooldown_ticks(1.0), 20);
        assert!((dash_distance(0.0) - 2.8).abs() < 1e-6);
        assert!((dash_distance(0.5) - 3.3).abs() < 1e-6);
        assert!((dash_distance(1.0) - 3.8).abs() < 1e-6);
    }

    #[test]
    fn dash_use_gains_and_diminishes() {
        assert!((dash_proficiency_gain(0.0, false, false) - 0.005).abs() < 1e-6);
        assert!((dash_proficiency_gain(0.5, false, false) - 0.0025).abs() < 1e-6);
        assert!((dash_proficiency_gain(0.0, true, true) - 0.013).abs() < 1e-6);
    }

    #[test]
    fn practice_never_grants_or_reactivates_dash() {
        let mut known = KnownTechniques::default();
        record_dash_use(&mut known, false, false);
        assert!(known.entries.is_empty(), "使用不能绕过卷轴授予身法");
        known
            .entries
            .push(crate::cultivation::known_techniques::KnownTechnique {
                id: DASH_TECHNIQUE_ID.into(),
                proficiency: 0.0,
                active: false,
            });
        record_dash_use(&mut known, false, false);
        assert_eq!(known_dash_proficiency(&known), None);
        assert_eq!(known.entries[0].proficiency, 0.0);
        known.entries[0].active = true;
        record_dash_use(&mut known, false, false);
        assert_eq!(known_dash_proficiency(&known), Some(0.005));
    }
}
