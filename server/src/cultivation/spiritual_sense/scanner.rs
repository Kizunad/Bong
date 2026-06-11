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
    /// plan-fauna-mimic-spider-v1 P2：伪装蛛 — 只有神识足够的观察者才能看穿。
    ///
    /// 识破条件：observer realm >= Condense（凝脉期，realm_rank ≥ 2）。
    /// 低于此境界的观察者感知结果被过滤掉，蛛在神识雷达上不可见。
    DisguisedSpider,
    /// plan-daozhan-v1 P3：Mimicry 态道伥 — 伪装为"无名玩家"，神识较难识破。
    ///
    /// 识破条件：observer realm >= Solidify（固元期，realm_rank ≥ 3）。
    /// 道伥伪装比蛛更接近真实玩家，需更高境界才能识破。
    DisguisedDaoZhang,
    /// plan-dying-elder-v1 P3：垂死大能 Plea 态真元 — 神识感知大能的残余真元波动。
    ///
    /// 感知条件：observer realm >= Solidify（固元期，realm_rank ≥ 3）。
    /// SpiritEye 激活时：显示具体 betray_probability%（明确数值）。
    /// SpiritEye 未激活时：仅显示"气息有异"（模糊提示，不暴露概率）。
    /// intensity 映射为大能剩余真元比例（0.0 = 濒死 / 1.0 = 尚有余力）。
    DyingElderQi,
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
        // plan-fauna-mimic-spider-v1 P2：Condense（凝脉期）及以上境界才能识破伪装蛛。
        // realm_rank: Awaken=0, Induce=1, Condense=2, Solidify=3, Spirit=4, Void=5
        SpiritualSenseTargetKind::DisguisedSpider => {
            if realm_rank(observer_realm) >= 2 {
                SenseKindV1::DisguisedSpider
            } else {
                return None;
            }
        }
        // plan-daozhan-v1 P3：Solidify（固元期）及以上境界才能识破 Mimicry 态道伥。
        // 道伥伪装更接近真实玩家，需 realm_rank ≥ 3（Solidify）才能看穿。
        SpiritualSenseTargetKind::DisguisedDaoZhang => {
            if realm_rank(observer_realm) >= 3 {
                SenseKindV1::DisguisedDaoZhang
            } else {
                return None;
            }
        }
        // plan-dying-elder-v1 P3：Solidify（固元期）及以上境界感知垂死大能真元波动。
        // intensity 映射大能剩余真元比（0.0 = 濒死 / 1.0 = 余力尚足）。
        SpiritualSenseTargetKind::DyingElderQi => {
            if realm_rank(observer_realm) >= 3 {
                SenseKindV1::DyingElderQi
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

pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
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

    // ── P2 DisguisedSpider 测试 ─────────────────────────────────────────────

    #[test]
    fn disguised_spider_visible_to_condense_and_above() {
        // DisguisedSpider 只有 Condense 及以上境界（realm_rank ≥ 2）可见
        let targets = vec![SpiritualSenseTarget {
            position: [30.0, 64.0, 0.0],
            kind: SpiritualSenseTargetKind::DisguisedSpider,
            intensity: 0.7,
            stealth: None,
        }];

        // Awaken / Induce 看不见（rank 0/1 < 2）
        assert!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Awaken, &targets).is_empty(),
            "Awaken 境界不应识破伪装蛛"
        );
        assert!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Induce, &targets).is_empty(),
            "Induce 境界不应识破伪装蛛"
        );

        // Condense 及以上可见
        let condense_entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Condense, &targets);
        assert_eq!(
            condense_entries.len(),
            1,
            "Condense 境界应识破伪装蛛（实际 {}）",
            condense_entries.len()
        );
        assert_eq!(
            condense_entries[0].kind,
            SenseKindV1::DisguisedSpider,
            "识破后 sense kind 应为 DisguisedSpider"
        );

        let solidify_entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Solidify, &targets);
        assert_eq!(solidify_entries.len(), 1, "Solidify 境界应识破伪装蛛");

        let spirit_entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Spirit, &targets);
        assert_eq!(spirit_entries.len(), 1, "Spirit 境界应识破伪装蛛");
    }

    #[test]
    fn disguised_spider_sense_kind_serializes_correctly() {
        // SenseKindV1::DisguisedSpider 序列化 wire name
        let entry = SenseEntryV1 {
            kind: SenseKindV1::DisguisedSpider,
            x: 10.0,
            y: 64.0,
            z: 5.0,
            intensity: 0.7,
        };
        let json = serde_json::to_string(&entry).expect("DisguisedSpider must serialize");
        assert!(
            json.contains("\"DisguisedSpider\""),
            "DisguisedSpider wire name 必须为 DisguisedSpider（PascalCase），实际 {json}"
        );

        // 往返
        let decoded: SenseEntryV1 = serde_json::from_str(&json).expect("must deserialize");
        assert_eq!(decoded.kind, SenseKindV1::DisguisedSpider);
        assert_eq!(decoded.intensity, 0.7_f64);
    }

    #[test]
    fn disguised_spider_out_of_realm_scan_radius_not_visible() {
        // Condense 扫描半径 200，目标距离 300 > 200，不可见
        let targets = vec![SpiritualSenseTarget {
            position: [300.0, 64.0, 0.0],
            kind: SpiritualSenseTargetKind::DisguisedSpider,
            intensity: 0.7,
            stealth: None,
        }];
        let entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Condense, &targets);
        assert!(
            entries.is_empty(),
            "超出扫描半径的 DisguisedSpider 不应出现在神识结果中"
        );
    }

    #[test]
    fn distance_fn_is_correct() {
        // 验证 pub distance 函数正确性（3D 勾股）
        let result = distance([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]);
        assert!(
            (result - 5.0).abs() < 1e-9,
            "distance([0,0,0], [3,4,0]) 应为 5.0，实际 {result}"
        );
    }

    // ── P3 DisguisedDaoZhang 测试 ───────────────────────────────────────────

    #[test]
    fn disguised_daozhan_requires_solidify_realm_or_above() {
        // DisguisedDaoZhang 只有 Solidify（固元期）及以上（realm_rank ≥ 3）可见
        let targets = vec![SpiritualSenseTarget {
            position: [30.0, 64.0, 0.0],
            kind: SpiritualSenseTargetKind::DisguisedDaoZhang,
            intensity: 0.6,
            stealth: None,
        }];

        // Awaken / Induce / Condense 看不见（rank 0/1/2 < 3）
        assert!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Awaken, &targets).is_empty(),
            "Awaken 境界不应识破 Mimicry 态道伥（rank=0 < 3）"
        );
        assert!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Induce, &targets).is_empty(),
            "Induce 境界不应识破 Mimicry 态道伥（rank=1 < 3）"
        );
        assert!(
            scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Condense, &targets).is_empty(),
            "Condense 境界不应识破 Mimicry 态道伥（rank=2 < 3，比伪装蛛更难识破）"
        );

        // Solidify 及以上可见（rank ≥ 3）
        let solidify_entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Solidify, &targets);
        assert_eq!(
            solidify_entries.len(),
            1,
            "Solidify 境界（固元）应识破 Mimicry 态道伥（实际 {}）",
            solidify_entries.len()
        );
        assert_eq!(
            solidify_entries[0].kind,
            SenseKindV1::DisguisedDaoZhang,
            "识破后 sense kind 应为 DisguisedDaoZhang"
        );

        let spirit_entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Spirit, &targets);
        assert_eq!(spirit_entries.len(), 1, "Spirit 境界应识破 Mimicry 态道伥");

        let void_entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Void, &targets);
        assert_eq!(void_entries.len(), 1, "Void 境界应识破 Mimicry 态道伥");
    }

    #[test]
    fn disguised_daozhan_harder_than_spider_to_detect() {
        // 道伥识破要求 Solidify（rank≥3），蛛识破只需 Condense（rank≥2）
        // Condense 境界能看到蛛，但看不到道伥
        let spider_target = SpiritualSenseTarget {
            position: [30.0, 64.0, 0.0],
            kind: SpiritualSenseTargetKind::DisguisedSpider,
            intensity: 0.7,
            stealth: None,
        };
        let daozhan_target = SpiritualSenseTarget {
            position: [30.0, 64.0, 5.0],
            kind: SpiritualSenseTargetKind::DisguisedDaoZhang,
            intensity: 0.6,
            stealth: None,
        };
        let targets = vec![spider_target, daozhan_target];

        // Condense：看到蛛（rank=2），但看不到道伥（需 rank≥3）
        let condense_entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Condense, &targets);
        assert_eq!(
            condense_entries.len(),
            1,
            "Condense 境界应只看到蛛（1个），看不到道伥（需更高境界），实际={}",
            condense_entries.len()
        );
        assert_eq!(
            condense_entries[0].kind,
            SenseKindV1::DisguisedSpider,
            "Condense 境界看到的应是 DisguisedSpider，而非道伥"
        );

        // Solidify：两者都看到（rank=3）
        let solidify_entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Solidify, &targets);
        assert_eq!(
            solidify_entries.len(),
            2,
            "Solidify 境界应同时看到蛛和道伥（共2个），实际={}",
            solidify_entries.len()
        );
    }

    #[test]
    fn disguised_daozhan_sense_kind_serializes_correctly() {
        // SenseKindV1::DisguisedDaoZhang 序列化 wire name（PascalCase）
        let entry = SenseEntryV1 {
            kind: SenseKindV1::DisguisedDaoZhang,
            x: 10.0,
            y: 64.0,
            z: 5.0,
            intensity: 0.6,
        };
        let json = serde_json::to_string(&entry).expect("DisguisedDaoZhang must serialize");
        assert!(
            json.contains("\"DisguisedDaoZhang\""),
            "DisguisedDaoZhang wire name 必须为 DisguisedDaoZhang（PascalCase），实际 {json}"
        );

        // 往返
        let decoded: SenseEntryV1 = serde_json::from_str(&json).expect("must deserialize");
        assert_eq!(decoded.kind, SenseKindV1::DisguisedDaoZhang);
        assert!(
            (decoded.intensity - 0.6).abs() < 1e-9,
            "intensity 往返应保持 0.6，实际={}",
            decoded.intensity
        );
    }

    #[test]
    fn disguised_daozhan_out_of_range_not_visible() {
        // Solidify 扫描半径 500，目标在 600 > 500，不可见
        let targets = vec![SpiritualSenseTarget {
            position: [600.0, 64.0, 0.0],
            kind: SpiritualSenseTargetKind::DisguisedDaoZhang,
            intensity: 0.6,
            stealth: None,
        }];
        let entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Solidify, &targets);
        assert!(
            entries.is_empty(),
            "超出扫描半径（600 > 500）的道伥不应出现在神识结果中"
        );
    }

    #[test]
    fn disguised_daozhan_intensity_within_range() {
        // intensity 应在 [0, 1] 范围内（保证 clamp 后不越界）
        let targets = vec![SpiritualSenseTarget {
            position: [30.0, 64.0, 0.0],
            kind: SpiritualSenseTargetKind::DisguisedDaoZhang,
            intensity: 0.6,
            stealth: None,
        }];
        let entries = scan_targets_inner_ring([0.0, 64.0, 0.0], Realm::Solidify, &targets);
        assert_eq!(entries.len(), 1, "应有 1 条道伥感知记录");
        let intensity = entries[0].intensity;
        assert!(
            (0.0..=1.0).contains(&intensity),
            "道伥感知 intensity={intensity:.3} 应在 [0, 1] 范围内"
        );
    }
}
