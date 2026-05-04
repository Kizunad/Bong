use crate::cultivation::components::Realm;
use crate::schema::realm_vision::{SenseEntryV1, SenseKindV1};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpiritualSenseTargetKind {
    Living,
    Leyline,
    Cultivator(Realm),
    HeavenlyGaze,
    Crisis,
    SpiritEye,
    NicheIntrusionTrace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpiritualSenseTarget {
    pub position: [f64; 3],
    pub kind: SpiritualSenseTargetKind,
    pub intensity: f64,
    pub stealth: Option<StealthState>,
}

pub fn scan_radius_for_realm(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 50.0,
        Realm::Condense => 200.0,
        Realm::Solidify => 500.0,
        Realm::Spirit => 1000.0,
        Realm::Void => 500.0,
    }
}

pub fn scan_targets_inner_ring(
    observer_pos: [f64; 3],
    observer_realm: Realm,
    targets: &[SpiritualSenseTarget],
) -> Vec<SenseEntryV1> {
    let radius = scan_radius_for_realm(observer_realm);
    if radius <= 0.0 {
        return Vec::new();
    }
    targets
        .iter()
        .filter(|target| distance(observer_pos, target.position) <= radius)
        .filter_map(|target| target_to_entry(observer_realm, target))
        .collect()
}

pub fn scan_targets_mid_ring_void(
    observer_pos: [f64; 3],
    targets: &[SpiritualSenseTarget],
) -> Vec<SenseEntryV1> {
    targets
        .iter()
        .filter(|target| {
            let d = distance(observer_pos, target.position);
            d > 500.0 && d <= 2000.0
        })
        .filter(|target| {
            matches!(
                target.kind,
                SpiritualSenseTargetKind::Cultivator(_)
                    | SpiritualSenseTargetKind::Leyline
                    | SpiritualSenseTargetKind::Crisis
            )
        })
        .filter_map(|target| target_to_entry(Realm::Void, target))
        .collect()
}

fn target_to_entry(observer_realm: Realm, target: &SpiritualSenseTarget) -> Option<SenseEntryV1> {
    let kind = match target.kind {
        SpiritualSenseTargetKind::Living => {
            if realm_rank(observer_realm) >= 1 {
                SenseKindV1::LivingQi
            } else {
                return None;
            }
        }
        SpiritualSenseTargetKind::Leyline => {
            if realm_rank(observer_realm) >= 2 {
                SenseKindV1::AmbientLeyline
            } else {
                return None;
            }
        }
        SpiritualSenseTargetKind::Cultivator(target_realm) => {
            if realm_rank(observer_realm) >= 3 {
                obfuscate_sense_kind(
                    SenseKindV1::CultivatorRealm,
                    observer_realm,
                    target_realm,
                    target.stealth.as_ref(),
                )?
            } else if realm_rank(observer_realm) >= 1 {
                SenseKindV1::LivingQi
            } else {
                return None;
            }
        }
        SpiritualSenseTargetKind::HeavenlyGaze => {
            if realm_rank(observer_realm) >= 4 {
                SenseKindV1::HeavenlyGaze
            } else {
                return None;
            }
        }
        SpiritualSenseTargetKind::Crisis => {
            if realm_rank(observer_realm) >= 4 {
                SenseKindV1::CrisisPremonition
            } else {
                return None;
            }
        }
        SpiritualSenseTargetKind::SpiritEye => SenseKindV1::SpiritEye,
        SpiritualSenseTargetKind::NicheIntrusionTrace => {
            if realm_rank(observer_realm) >= 3 {
                SenseKindV1::NicheIntrusionTrace
            } else {
                return None;
            }
        }
    };
    Some(SenseEntryV1 {
        kind,
        x: target.position[0],
        y: target.position[1],
        z: target.position[2],
        intensity: target.intensity.clamp(0.0, 1.0),
    })
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StealthState {
    pub active: bool,
    pub disrupted: bool,
}

fn realm_rank(realm: Realm) -> u8 {
    crate::cultivation::realm_vision::planner::realm_rank(realm)
}

pub fn obfuscate_sense_kind(
    original_kind: SenseKindV1,
    observer_realm: Realm,
    target_realm: Realm,
    target_stealth: Option<&StealthState>,
) -> Option<SenseKindV1> {
    let Some(stealth) = target_stealth else {
        return Some(original_kind);
    };
    if !stealth.active || stealth.disrupted || original_kind != SenseKindV1::CultivatorRealm {
        return Some(original_kind);
    }

    if realm_rank(observer_realm).saturating_sub(realm_rank(target_realm)) >= 2 {
        Some(original_kind)
    } else {
        Some(SenseKindV1::AmbientLeyline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obfuscate_hook_default_passthrough() {
        assert_eq!(
            obfuscate_sense_kind(
                SenseKindV1::CultivatorRealm,
                Realm::Solidify,
                Realm::Induce,
                None,
            ),
            Some(SenseKindV1::CultivatorRealm)
        );
    }

    #[test]
    fn dugu_stealth_masks_same_band_cultivator_as_leyline() {
        assert_eq!(
            obfuscate_sense_kind(
                SenseKindV1::CultivatorRealm,
                Realm::Solidify,
                Realm::Condense,
                Some(&StealthState {
                    active: true,
                    disrupted: false,
                }),
            ),
            Some(SenseKindV1::AmbientLeyline)
        );
    }

    #[test]
    fn dugu_exposure_or_two_realm_gap_reveals_cultivator() {
        assert_eq!(
            obfuscate_sense_kind(
                SenseKindV1::CultivatorRealm,
                Realm::Spirit,
                Realm::Induce,
                Some(&StealthState {
                    active: true,
                    disrupted: false,
                }),
            ),
            Some(SenseKindV1::CultivatorRealm)
        );
        assert_eq!(
            obfuscate_sense_kind(
                SenseKindV1::CultivatorRealm,
                Realm::Solidify,
                Realm::Condense,
                Some(&StealthState {
                    active: true,
                    disrupted: true,
                }),
            ),
            Some(SenseKindV1::CultivatorRealm)
        );
    }

    #[test]
    fn scan_targets_per_realm() {
        let targets = vec![
            SpiritualSenseTarget {
                position: [30.0, 64.0, 0.0],
                kind: SpiritualSenseTargetKind::Living,
                intensity: 0.7,
                stealth: None,
            },
            SpiritualSenseTarget {
                position: [100.0, 64.0, 0.0],
                kind: SpiritualSenseTargetKind::Leyline,
                intensity: 0.6,
                stealth: None,
            },
            SpiritualSenseTarget {
                position: [300.0, 64.0, 0.0],
                kind: SpiritualSenseTargetKind::Cultivator(Realm::Induce),
                intensity: 0.8,
                stealth: None,
            },
            SpiritualSenseTarget {
                position: [800.0, 64.0, 0.0],
                kind: SpiritualSenseTargetKind::Crisis,
                intensity: 1.0,
                stealth: None,
            },
            SpiritualSenseTarget {
                position: [20.0, 64.0, 10.0],
                kind: SpiritualSenseTargetKind::SpiritEye,
                intensity: 1.0,
                stealth: None,
            },
        ];
        assert!(scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Awaken, &targets).is_empty());
        assert_eq!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Induce, &targets).len(),
            2
        );
        assert_eq!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Condense, &targets).len(),
            3
        );
        assert_eq!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Solidify, &targets).len(),
            4
        );
        assert_eq!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Spirit, &targets).len(),
            5
        );
        assert_eq!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Void, &targets).len(),
            4
        );
    }

    #[test]
    fn three_ring_scan_for_void() {
        let targets = vec![
            SpiritualSenseTarget {
                position: [600.0, 64.0, 0.0],
                kind: SpiritualSenseTargetKind::Living,
                intensity: 0.2,
                stealth: None,
            },
            SpiritualSenseTarget {
                position: [700.0, 64.0, 0.0],
                kind: SpiritualSenseTargetKind::Cultivator(Realm::Spirit),
                intensity: 0.8,
                stealth: None,
            },
            SpiritualSenseTarget {
                position: [1500.0, 64.0, 0.0],
                kind: SpiritualSenseTargetKind::Leyline,
                intensity: 0.4,
                stealth: None,
            },
            SpiritualSenseTarget {
                position: [2100.0, 64.0, 0.0],
                kind: SpiritualSenseTargetKind::Crisis,
                intensity: 1.0,
                stealth: None,
            },
        ];
        let entries = scan_targets_mid_ring_void([0.0, 64.0, 0.0], &targets);
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.kind == SenseKindV1::CultivatorRealm));
        assert!(entries
            .iter()
            .any(|entry| entry.kind == SenseKindV1::AmbientLeyline));
    }

    #[test]
    fn niche_intrusion_trace_requires_solidify_sense() {
        let targets = vec![SpiritualSenseTarget {
            position: [40.0, 64.0, 0.0],
            kind: SpiritualSenseTargetKind::NicheIntrusionTrace,
            intensity: 0.8,
            stealth: None,
        }];

        assert!(scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Condense, &targets).is_empty());
        assert_eq!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Solidify, &targets)[0].kind,
            SenseKindV1::NicheIntrusionTrace
        );
    }

    #[test]
    fn obfuscate_hook_three_state_schema() {
        for kind in [
            SenseKindV1::LivingQi,
            SenseKindV1::AmbientLeyline,
            SenseKindV1::CultivatorRealm,
        ] {
            let entry = SenseEntryV1 {
                kind,
                x: 0.0,
                y: 64.0,
                z: 0.0,
                intensity: 1.0,
            };
            serde_json::to_string(&entry).expect("sense entry variant should serialize");
        }
    }
}
