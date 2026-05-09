use crate::cultivation::components::Realm;

use super::constants::{
    QI_ACOUSTIC_THRESHOLD, QI_DRAIN_CLAMP, QI_EXCRETION_BASE, QI_NEGATIVE_FIELD_K,
    QI_NEGATIVE_FIELD_MIN_RADIUS_BLOCKS,
};
use super::distance::qi_distance_atten;
use super::env::EnvField;
use super::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use super::traits::{StyleAttack, StyleDefense};

pub const QI_ZHENMAI_BETA: f64 = 0.6;

#[derive(Debug, Clone, PartialEq)]
pub struct CollisionOutcome {
    pub attenuated_qi: f64,
    pub effective_hit: f64,
    pub attacker_spent: f64,
    pub defender_lost: f64,
    pub defender_absorbed: f64,
    pub transfers: Vec<QiTransfer>,
}

pub fn qi_woliu_vortex_field_strength_for_realm(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 0.10,
        Realm::Condense => 0.25,
        Realm::Solidify => 0.45,
        Realm::Spirit => 0.65,
        Realm::Void => 0.80,
    }
}

/// Returns the fraction of the current payload drained by a negative qi field.
/// The value is always clamped to `[0, 1]`.
pub fn qi_negative_field_drain_ratio(field_intensity: f64, distance_blocks: f64) -> f64 {
    if !field_intensity.is_finite() || field_intensity <= 0.0 {
        return 0.0;
    }
    if !distance_blocks.is_finite() {
        return 0.0;
    }

    let effective_radius = distance_blocks.max(QI_NEGATIVE_FIELD_MIN_RADIUS_BLOCKS);
    (field_intensity * QI_NEGATIVE_FIELD_K / effective_radius.powi(2)).clamp(0.0, 1.0)
}

/// 截脉音论反震基础算子：输入真元 × 反震效率 × 流派对位权重 × β。
pub fn reverse_clamp(incoming_qi: f64, k_drain: f64, style_weight: f64, beta: f64) -> f64 {
    if !incoming_qi.is_finite()
        || !k_drain.is_finite()
        || !style_weight.is_finite()
        || !beta.is_finite()
    {
        return 0.0;
    }
    (incoming_qi.max(0.0) * k_drain.max(0.0) * style_weight.max(0.0) * beta.max(0.0)).max(0.0)
}

/// 经脉硬化对 incoming damage flow 的修正算子。
pub fn flow_modifier(base_multiplier: f32, harden_multiplier: f32) -> f32 {
    if !base_multiplier.is_finite() || !harden_multiplier.is_finite() {
        return 1.0;
    }
    (base_multiplier.max(0.0) * harden_multiplier.max(0.0)).max(0.0)
}

pub fn qi_collision(
    attacker_id: &QiAccountId,
    defender_id: &QiAccountId,
    environment_id: &QiAccountId,
    atk: &dyn StyleAttack,
    def: &dyn StyleDefense,
    distance_blocks: f64,
    env: &EnvField,
) -> CollisionOutcome {
    let injected = atk.injected_qi().max(0.0);
    let attenuated = qi_distance_atten(injected, distance_blocks, atk.medium());
    let purity = atk.purity().clamp(0.0, 1.0);
    if purity < QI_ACOUSTIC_THRESHOLD {
        return CollisionOutcome {
            attenuated_qi: attenuated,
            effective_hit: 0.0,
            attacker_spent: injected,
            defender_lost: 0.0,
            defender_absorbed: 0.0,
            transfers: Vec::new(),
        };
    }

    let resistance = def.resistance().clamp(0.0, 1.0);
    let rejection = attenuated * QI_EXCRETION_BASE * (1.0 - purity + resistance * 0.5);
    let effective_hit = (attenuated - rejection).max(0.0) * env.rhythm_factor();
    let defender_lost = effective_hit * (1.0 - resistance);
    let defender_absorbed =
        (defender_lost * def.drain_affinity().clamp(0.0, 1.0)).min(injected * QI_DRAIN_CLAMP);

    let mut transfers = Vec::new();
    if defender_lost > 0.0 {
        if let Ok(transfer) = QiTransfer::new(
            defender_id.clone(),
            environment_id.clone(),
            defender_lost,
            QiTransferReason::Collision,
        ) {
            transfers.push(transfer);
        }
    }
    if defender_absorbed > 0.0 {
        if let Ok(transfer) = QiTransfer::new(
            attacker_id.clone(),
            defender_id.clone(),
            defender_absorbed,
            QiTransferReason::Collision,
        ) {
            transfers.push(transfer);
        }
    }

    CollisionOutcome {
        attenuated_qi: attenuated,
        effective_hit,
        attacker_spent: injected,
        defender_lost,
        defender_absorbed,
        transfers,
    }
}

#[cfg(test)]
mod tests {
    use crate::cultivation::components::ColorKind;

    use super::*;
    use crate::qi_physics::traits::{SimpleStyleAttack, SimpleStyleDefense};

    fn ids() -> (QiAccountId, QiAccountId, QiAccountId) {
        (
            QiAccountId::player("attacker-1"),
            QiAccountId::player("defender-1"),
            QiAccountId::zone("impact-zone"),
        )
    }

    #[test]
    fn collision_delivers_damage_after_distance() {
        let (attacker_id, defender_id, environment_id) = ids();
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.2);
        let outcome = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &def,
            3.0,
            &EnvField::default(),
        );
        assert!(outcome.attenuated_qi < 10.0);
        assert!(outcome.defender_lost > 0.0);
    }

    #[test]
    fn low_purity_fails_acoustic_threshold() {
        let (attacker_id, defender_id, environment_id) = ids();
        let mut atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        atk.purity = 0.1;
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        let outcome = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &def,
            1.0,
            &EnvField::default(),
        );
        assert_eq!(outcome.effective_hit, 0.0);
        assert!(outcome.transfers.is_empty());
    }

    #[test]
    fn strong_resistance_reduces_loss() {
        let (attacker_id, defender_id, environment_id) = ids();
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let weak = SimpleStyleDefense::new(ColorKind::Solid, 0.1);
        let strong = SimpleStyleDefense::new(ColorKind::Solid, 0.8);
        let weak_out = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &weak,
            1.0,
            &EnvField::default(),
        );
        let strong_out = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &strong,
            1.0,
            &EnvField::default(),
        );
        assert!(strong_out.defender_lost < weak_out.defender_lost);
    }

    #[test]
    fn drain_affinity_is_clamped_to_half_spend() {
        let (attacker_id, defender_id, environment_id) = ids();
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let mut def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        def.drain_affinity = 1.0;
        let outcome = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &def,
            0.0,
            &EnvField::default(),
        );
        assert!(outcome.defender_absorbed <= 5.0);
    }

    #[test]
    fn collision_records_bidirectional_transfers_when_absorbed() {
        let (attacker_id, defender_id, environment_id) = ids();
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let mut def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        def.drain_affinity = 0.5;
        let outcome = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &def,
            0.0,
            &EnvField::default(),
        );
        assert_eq!(outcome.transfers.len(), 2);
        assert_eq!(outcome.transfers[0].from, defender_id);
        assert_eq!(outcome.transfers[0].to, environment_id);
        assert_eq!(outcome.transfers[1].from, attacker_id);
        assert_eq!(outcome.transfers[1].to, defender_id);
    }

    #[test]
    fn active_rhythm_amplifies_effective_hit() {
        let (attacker_id, defender_id, environment_id) = ids();
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 10.0);
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        let base = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &def,
            0.0,
            &EnvField::default(),
        );
        let active = EnvField {
            rhythm_multiplier: 1.2,
            ..Default::default()
        };
        let boosted = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &def,
            0.0,
            &active,
        );
        assert!(boosted.effective_hit > base.effective_hit);
    }

    #[test]
    fn zero_injected_qi_has_no_effect() {
        let (attacker_id, defender_id, environment_id) = ids();
        let atk = SimpleStyleAttack::new(ColorKind::Sharp, 0.0);
        let def = SimpleStyleDefense::new(ColorKind::Solid, 0.0);
        let outcome = qi_collision(
            &attacker_id,
            &defender_id,
            &environment_id,
            &atk,
            &def,
            0.0,
            &EnvField::default(),
        );
        assert_eq!(outcome.defender_lost, 0.0);
    }

    #[test]
    fn negative_field_drain_uses_inverse_square_distance() {
        let near = qi_negative_field_drain_ratio(0.8, 1.0);
        let far = qi_negative_field_drain_ratio(0.8, 2.0);
        assert!((near - 0.8).abs() < 1e-9);
        assert!((far - 0.2).abs() < 1e-9);
    }

    #[test]
    fn negative_field_drain_clamps_invalid_and_overstrong_fields() {
        assert_eq!(qi_negative_field_drain_ratio(0.0, 1.0), 0.0);
        assert_eq!(qi_negative_field_drain_ratio(-0.1, 1.0), 0.0);
        assert_eq!(qi_negative_field_drain_ratio(f64::INFINITY, 1.0), 0.0);
        assert_eq!(qi_negative_field_drain_ratio(0.8, -1.0), 0.8);
        assert_eq!(qi_negative_field_drain_ratio(0.8, f64::NAN), 0.0);
        assert_eq!(qi_negative_field_drain_ratio(2.0, 0.0), 1.0);
    }

    #[test]
    fn reverse_clamp_applies_zhenmai_beta_and_weight() {
        assert!((reverse_clamp(100.0, 0.5, 0.7, QI_ZHENMAI_BETA) - 21.0).abs() < 1e-6);
    }

    #[test]
    fn reverse_clamp_rejects_invalid_inputs() {
        assert_eq!(reverse_clamp(f64::NAN, 0.5, 0.7, QI_ZHENMAI_BETA), 0.0);
        assert_eq!(reverse_clamp(100.0, -0.5, 0.7, QI_ZHENMAI_BETA), 0.0);
    }

    #[test]
    fn flow_modifier_multiplies_incoming_damage_factors() {
        assert_eq!(flow_modifier(0.8, 0.25), 0.2);
        assert_eq!(flow_modifier(f32::NAN, 0.25), 1.0);
    }

    #[test]
    fn woliu_vortex_strength_keeps_realm_progression_in_qi_physics() {
        assert_eq!(qi_woliu_vortex_field_strength_for_realm(Realm::Awaken), 0.0);
        assert_eq!(
            qi_woliu_vortex_field_strength_for_realm(Realm::Induce),
            0.10
        );
        assert_eq!(
            qi_woliu_vortex_field_strength_for_realm(Realm::Condense),
            0.25
        );
        assert_eq!(
            qi_woliu_vortex_field_strength_for_realm(Realm::Solidify),
            0.45
        );
        assert_eq!(
            qi_woliu_vortex_field_strength_for_realm(Realm::Spirit),
            0.65
        );
        assert_eq!(qi_woliu_vortex_field_strength_for_realm(Realm::Void), 0.80);
    }
}
