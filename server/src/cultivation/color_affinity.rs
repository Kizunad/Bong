//! 真元色谱与顿悟三轨选取。

use std::collections::HashSet;

use super::color::PracticeLog;
use super::components::{ColorKind, QiColor, Realm};
use super::generic_talent::{GenericTalentRegistry, ALL_COLORS};
use super::insight::{
    validate_offer, InsightAlignment, InsightChoice, InsightCost, InsightEffect, InsightQuota,
    InsightTradeoff,
};
use super::insight_flavor::flavor_for;
use super::special_talent::{
    special_converge_pool, special_diverge_pool, special_neutral_pool, tradeoff,
};

pub fn opposite_color(color: ColorKind) -> ColorKind {
    match color {
        ColorKind::Sharp => ColorKind::Heavy,
        ColorKind::Heavy => ColorKind::Light,
        ColorKind::Mellow => ColorKind::Violent,
        ColorKind::Solid => ColorKind::Light,
        ColorKind::Light => ColorKind::Heavy,
        ColorKind::Intricate => ColorKind::Violent,
        ColorKind::Gentle => ColorKind::Insidious,
        ColorKind::Insidious => ColorKind::Gentle,
        ColorKind::Violent => ColorKind::Intricate,
        ColorKind::Turbid => ColorKind::Mellow,
    }
}

pub fn diverge_target(log: &PracticeLog, current_main: ColorKind) -> ColorKind {
    ALL_COLORS
        .iter()
        .copied()
        .filter(|color| *color != current_main)
        .min_by(|a, b| {
            let aw = log.weights.get(a).copied().unwrap_or(0.0);
            let bw = log.weights.get(b).copied().unwrap_or(0.0);
            aw.partial_cmp(&bw)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| color_rank(*a).cmp(&color_rank(*b)))
        })
        .unwrap_or(ColorKind::Sharp)
}

pub fn select_aligned_choices(
    trigger_id: &str,
    qi_color: &QiColor,
    practice_log: &PracticeLog,
    quota: &InsightQuota,
    realm: Realm,
    registry: &GenericTalentRegistry,
) -> Vec<InsightChoice> {
    select_aligned_tradeoffs(trigger_id, qi_color, practice_log, quota, realm, registry)
        .into_iter()
        .map(InsightChoice::from_tradeoff)
        .collect()
}

pub fn select_aligned_tradeoffs(
    trigger_id: &str,
    qi_color: &QiColor,
    practice_log: &PracticeLog,
    quota: &InsightQuota,
    realm: Realm,
    registry: &GenericTalentRegistry,
) -> Vec<InsightTradeoff> {
    if qi_color.is_hunyuan {
        return hunyuan_tradeoffs(trigger_id, qi_color, practice_log, quota, realm);
    }
    if qi_color.is_chaotic {
        return chaotic_tradeoffs(trigger_id, qi_color, practice_log, quota, realm);
    }

    let diverge = diverge_target(practice_log, qi_color.main);
    let mut selected = Vec::new();
    let mut used = HashSet::new();
    for alignment in [
        InsightAlignment::Converge,
        InsightAlignment::Neutral,
        InsightAlignment::Diverge,
    ] {
        let mut candidates = candidates_for(alignment, qi_color.main, diverge, registry);
        if let Some(choice) = select_first_valid(
            trigger_id,
            alignment,
            qi_color.main,
            &mut candidates,
            quota,
            realm,
            &mut used,
        ) {
            selected.push(choice);
        } else if let Some(choice) = neutral_degrade_tradeoff(
            trigger_id,
            alignment,
            qi_color.main,
            quota,
            realm,
            &mut used,
        ) {
            selected.push(choice);
        }
    }
    selected
}

fn candidates_for(
    alignment: InsightAlignment,
    main_color: ColorKind,
    diverge_color: ColorKind,
    registry: &GenericTalentRegistry,
) -> Vec<InsightTradeoff> {
    let affinity = match alignment {
        InsightAlignment::Converge | InsightAlignment::Neutral => main_color,
        InsightAlignment::Diverge => diverge_color,
    };
    let mut candidates: Vec<_> = registry
        .query(affinity, alignment)
        .into_iter()
        .filter_map(|def| {
            registry
                .to_insight_tradeoff(def, alignment, main_color, diverge_color)
                .ok()
        })
        .collect();
    match alignment {
        InsightAlignment::Converge => candidates.extend(special_converge_pool(main_color)),
        InsightAlignment::Neutral => candidates.extend(special_neutral_pool()),
        InsightAlignment::Diverge => {
            candidates.extend(special_diverge_pool(main_color, diverge_color))
        }
    }
    candidates
}

fn select_first_valid(
    trigger_id: &str,
    alignment: InsightAlignment,
    main_color: ColorKind,
    candidates: &mut [InsightTradeoff],
    quota: &InsightQuota,
    realm: Realm,
    used: &mut HashSet<String>,
) -> Option<InsightTradeoff> {
    for candidate in candidates.iter_mut() {
        candidate.gain_flavor = flavor_for(
            trigger_id,
            alignment,
            main_color,
            candidate.target_color,
            candidate.gain_flavor.as_str(),
        );
        let key = format!("{:?}", candidate.gain);
        if used.contains(&key) {
            continue;
        }
        let choice = InsightChoice::from_tradeoff(candidate.clone());
        if validate_offer(quota, &choice, realm).is_ok() {
            used.insert(key);
            return Some(candidate.clone());
        }
    }
    None
}

fn neutral_degrade_tradeoff(
    trigger_id: &str,
    saturated_alignment: InsightAlignment,
    main_color: ColorKind,
    quota: &InsightQuota,
    realm: Realm,
    used: &mut HashSet<String>,
) -> Option<InsightTradeoff> {
    let mut candidate = tradeoff(
        InsightAlignment::Neutral,
        InsightEffect::UnlockPerception {
            kind: format!("alignment_saturated_{}", saturated_alignment.code()),
        },
        InsightCost::SenseExposure { add: 0.03 },
        format!(
            "此道已臻顶，{}之意暂不再添；你转而看见真元走向的细痕。",
            super::generic_talent::color_kind_to_chinese(main_color)
        ),
        "灵识外放——被感知暴露度 +3%".to_string(),
        None,
    );
    candidate.gain_flavor = flavor_for(
        trigger_id,
        InsightAlignment::Neutral,
        main_color,
        None,
        candidate.gain_flavor.as_str(),
    );
    let key = format!("{:?}", candidate.gain);
    if used.contains(&key) {
        return None;
    }
    let choice = InsightChoice::from_tradeoff(candidate.clone());
    if validate_offer(quota, &choice, realm).is_err() {
        return None;
    }
    used.insert(key);
    Some(candidate)
}

fn hunyuan_tradeoffs(
    trigger_id: &str,
    qi_color: &QiColor,
    practice_log: &PracticeLog,
    quota: &InsightQuota,
    realm: Realm,
) -> Vec<InsightTradeoff> {
    let target = strongest_color(practice_log).unwrap_or(qi_color.main);
    let mut candidates = vec![
        tradeoff(
            InsightAlignment::Converge,
            InsightEffect::HunyuanThreshold { mul: 0.964 },
            InsightCost::ChaoticToleranceLoss { sub: 0.02 },
            "混元之息自相抱守——混元阈值降低 3.6%".to_string(),
            "越守归一，专精越钝——杂色容忍 -2%".to_string(),
            None,
        ),
        tradeoff(
            InsightAlignment::Neutral,
            InsightEffect::ComposureRecover { mul: 1.06 },
            InsightCost::ShockSensitivity { add: 0.03 },
            "心湖归平——心境恢复 +6%".to_string(),
            "水面更易起波——心境冲击敏感 +3%".to_string(),
            None,
        ),
        tradeoff(
            InsightAlignment::Diverge,
            InsightEffect::ColorCapAdd {
                color: target,
                add: 0.05,
            },
            InsightCost::MainColorPenalty {
                color: qi_color.main,
                penalty: 0.10,
            },
            "打破混元，择一色为锋——目标色染色上限 +5%".to_string(),
            "混元旧稳被割开——主色效率 -10%".to_string(),
            Some(target),
        ),
    ];
    validate_specials(trigger_id, qi_color.main, quota, realm, &mut candidates)
}

fn chaotic_tradeoffs(
    trigger_id: &str,
    qi_color: &QiColor,
    practice_log: &PracticeLog,
    quota: &InsightQuota,
    realm: Realm,
) -> Vec<InsightTradeoff> {
    let lowest = diverge_target(practice_log, qi_color.main);
    let highest = strongest_color(practice_log).unwrap_or(qi_color.main);
    let mut candidates = vec![
        tradeoff(
            InsightAlignment::Converge,
            InsightEffect::ChaoticTolerance { add: 0.036 },
            InsightCost::MainColorPenalty {
                color: qi_color.main,
                penalty: 0.10,
            },
            "杂色不再互噬，开始向混元合拢——杂色容忍 +3.6%".to_string(),
            "旧主色被压低声量——主色效率 -10%".to_string(),
            Some(lowest),
        ),
        tradeoff(
            InsightAlignment::Neutral,
            InsightEffect::ComposureRecover { mul: 1.06 },
            InsightCost::ShockSensitivity { add: 0.03 },
            "心湖归平——心境恢复 +6%".to_string(),
            "水面更易起波——心境冲击敏感 +3%".to_string(),
            None,
        ),
        tradeoff(
            InsightAlignment::Diverge,
            InsightEffect::ColorCapAdd {
                color: highest,
                add: 0.04,
            },
            InsightCost::MainColorPenalty {
                color: qi_color.main,
                penalty: 0.10,
            },
            "从杂色里抓回主线——最高权重色染色上限 +4%".to_string(),
            "其余杂色被压入暗处——主色效率 -10%".to_string(),
            Some(highest),
        ),
    ];
    validate_specials(trigger_id, qi_color.main, quota, realm, &mut candidates)
}

fn validate_specials(
    trigger_id: &str,
    main_color: ColorKind,
    quota: &InsightQuota,
    realm: Realm,
    candidates: &mut [InsightTradeoff],
) -> Vec<InsightTradeoff> {
    let mut used = HashSet::new();
    let mut selected = Vec::new();
    for candidate in candidates {
        candidate.gain_flavor = flavor_for(
            trigger_id,
            candidate.alignment,
            main_color,
            candidate.target_color,
            candidate.gain_flavor.as_str(),
        );
        let key = format!("{:?}", candidate.gain);
        let choice = InsightChoice::from_tradeoff(candidate.clone());
        if !used.contains(&key) && validate_offer(quota, &choice, realm).is_ok() {
            used.insert(key);
            selected.push(candidate.clone());
        }
    }
    selected
}

fn strongest_color(log: &PracticeLog) -> Option<ColorKind> {
    ALL_COLORS.iter().copied().max_by(|a, b| {
        let aw = log.weights.get(a).copied().unwrap_or(0.0);
        let bw = log.weights.get(b).copied().unwrap_or(0.0);
        aw.partial_cmp(&bw)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| color_rank(*b).cmp(&color_rank(*a)))
    })
}

fn color_rank(color: ColorKind) -> usize {
    ALL_COLORS
        .iter()
        .position(|candidate| *candidate == color)
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::generic_talent::GenericTalentRegistry;

    fn registry() -> GenericTalentRegistry {
        GenericTalentRegistry::builtin().unwrap()
    }

    #[test]
    fn converge_pool_all_10_colors() {
        let registry = registry();
        for color in ALL_COLORS {
            let candidates = candidates_for(
                InsightAlignment::Converge,
                color,
                ColorKind::Light,
                &registry,
            );
            assert!(
                candidates.len() >= 3,
                "{color:?} only had {}",
                candidates.len()
            );
        }
    }

    #[test]
    fn diverge_target_picks_lowest_weight() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 10.0);
        log.add(ColorKind::Heavy, 3.0);
        log.add(ColorKind::Mellow, 1.0);
        assert_eq!(diverge_target(&log, ColorKind::Sharp), ColorKind::Solid);
    }

    #[test]
    fn diverge_target_new_player_random() {
        let log = PracticeLog::default();
        assert_ne!(diverge_target(&log, ColorKind::Mellow), ColorKind::Mellow);
    }

    #[test]
    fn select_aligned_no_duplicate_effects() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 50.0);
        log.add(ColorKind::Heavy, 5.0);
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let choices = select_aligned_choices(
            "first_breakthrough_to_Induce",
            &qi,
            &log,
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        let keys: HashSet<_> = choices
            .iter()
            .map(|choice| format!("{:?}", choice.effect))
            .collect();
        assert_eq!(choices.len(), 3);
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn converge_magnitude_1_2x() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 10.0);
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let choices = select_aligned_choices(
            "first_breakthrough_to_Induce",
            &qi,
            &log,
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        let converge = choices
            .iter()
            .find(|choice| choice.alignment == InsightAlignment::Converge)
            .unwrap();
        assert!((converge.effect.magnitude() - 0.036).abs() < 1e-9);
    }

    #[test]
    fn diverge_magnitude_0_9x() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 10.0);
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let choices = select_aligned_choices(
            "first_breakthrough_to_Induce",
            &qi,
            &log,
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        let diverge = choices
            .iter()
            .find(|choice| choice.alignment == InsightAlignment::Diverge)
            .unwrap();
        assert!((diverge.effect.magnitude() - 0.036).abs() < 1e-9);
    }

    #[test]
    fn hunyuan_special_converge_maintains() {
        let qi = QiColor {
            is_hunyuan: true,
            ..QiColor::default()
        };
        let choices = select_aligned_choices(
            "chaotic_to_hunyuan_pivot",
            &qi,
            &PracticeLog::default(),
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        assert!(choices
            .iter()
            .any(|choice| matches!(choice.effect, InsightEffect::HunyuanThreshold { .. })));
    }

    #[test]
    fn chaotic_special_converge_to_hunyuan() {
        let qi = QiColor {
            is_chaotic: true,
            ..QiColor::default()
        };
        let choices = select_aligned_choices(
            "chaotic_to_hunyuan_pivot",
            &qi,
            &PracticeLog::default(),
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        assert!(choices
            .iter()
            .any(|choice| matches!(choice.effect, InsightEffect::ChaoticTolerance { .. })));
    }

    #[test]
    fn every_choice_has_nonzero_cost() {
        let qi = QiColor::default();
        let choices = select_aligned_choices(
            "first_breakthrough_to_Induce",
            &qi,
            &PracticeLog::default(),
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        assert!(choices.iter().all(|choice| choice.cost_magnitude > 0.0));
    }

    #[test]
    fn cost_magnitude_gte_half_gain() {
        let qi = QiColor::default();
        let choices = select_aligned_choices(
            "first_breakthrough_to_Induce",
            &qi,
            &PracticeLog::default(),
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        for choice in choices {
            assert!(choice.cost_magnitude + 1e-9 >= choice.effect.magnitude() * 0.5);
        }
    }

    #[test]
    fn converge_cost_targets_opposite_color() {
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let choices = select_aligned_choices(
            "first_breakthrough_to_Induce",
            &qi,
            &PracticeLog::default(),
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        let converge = choices
            .iter()
            .find(|choice| choice.alignment == InsightAlignment::Converge)
            .unwrap();
        assert!(matches!(
            converge.cost,
            InsightCost::ChaoticToleranceLoss { .. }
                | InsightCost::OppositeColorPenalty {
                    color: ColorKind::Heavy,
                    ..
                }
        ));
    }

    #[test]
    fn diverge_cost_targets_main_color() {
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let choices = select_aligned_choices(
            "first_breakthrough_to_Induce",
            &qi,
            &PracticeLog::default(),
            &InsightQuota::default(),
            Realm::Induce,
            &registry(),
        );
        let diverge = choices
            .iter()
            .find(|choice| choice.alignment == InsightAlignment::Diverge)
            .unwrap();
        assert!(matches!(
            diverge.cost,
            InsightCost::MainColorPenalty {
                color: ColorKind::Sharp,
                ..
            }
        ));
    }

    #[test]
    fn opposite_color_map_covers_all_colors() {
        for color in ALL_COLORS {
            assert_ne!(opposite_color(color), color);
        }
    }

    #[test]
    fn missing_track_degrades_to_neutral_hint() {
        let mut used = HashSet::new();
        let degraded = neutral_degrade_tradeoff(
            "first_breakthrough_to_Induce",
            InsightAlignment::Converge,
            ColorKind::Sharp,
            &InsightQuota::default(),
            Realm::Induce,
            &mut used,
        )
        .unwrap();
        assert_eq!(degraded.alignment, InsightAlignment::Neutral);
        assert!(degraded.gain_flavor.contains("此道已臻顶"));
        assert!(matches!(degraded.cost, InsightCost::SenseExposure { .. }));
    }
}
