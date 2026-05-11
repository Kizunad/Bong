//! 顿悟 fallback 生成器：按当前真元色谱动态生成 CONVERGE/NEUTRAL/DIVERGE 三轨。

use super::color::PracticeLog;
use super::color_affinity::select_aligned_choices;
use super::components::{QiColor, Realm};
use super::generic_talent::GenericTalentRegistry;
use super::insight::{InsightChoice, InsightQuota};

const KNOWN_TRIGGERS: &[&str] = &[
    "first_breakthrough_to_Induce",
    "first_breakthrough_to_Condense",
    "first_breakthrough_to_Solidify",
    "first_breakthrough_to_Spirit",
    "first_breakthrough_to_Void",
    "breakthrough_failed_recovered",
    "meridian_forge_tier_milestone",
    "first_tribulation_survived",
    "survived_negative_zone",
    "wind_candle_lifespan_extension",
    "practice_dedication_milestone",
    "chaotic_to_hunyuan_pivot",
    "witnessed_xuhua_tribulation",
    "killed_higher_realm",
    "killed_by_higher_realm_survived",
    "post_rebirth_clarity",
];

/// 旧调用点的无上下文兜底：保留签名，默认按 Mellow/空 PracticeLog 生成三轨。
pub fn fallback_for(trigger_id: &str) -> Vec<InsightChoice> {
    fallback_for_context(
        trigger_id,
        &QiColor::default(),
        &PracticeLog::default(),
        &InsightQuota::default(),
        Realm::Induce,
    )
}

pub fn fallback_for_context(
    trigger_id: &str,
    qi_color: &QiColor,
    practice_log: &PracticeLog,
    quota: &InsightQuota,
    realm: Realm,
) -> Vec<InsightChoice> {
    if !KNOWN_TRIGGERS.contains(&trigger_id) {
        return Vec::new();
    }
    let registry = GenericTalentRegistry::builtin()
        .expect("server/assets/insight/generic_talents.json must remain valid");
    select_aligned_choices(trigger_id, qi_color, practice_log, quota, realm, &registry)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::cultivation::components::{ColorKind, QiColor};
    use crate::cultivation::insight::{validate_offer, InsightAlignment, InsightCost};

    #[test]
    fn every_known_trigger_has_three_aligned_options() {
        for id in KNOWN_TRIGGERS {
            let opts = fallback_for(id);
            assert_eq!(opts.len(), 3, "{id} had {} options", opts.len());
            let alignments: HashSet<_> = opts.iter().map(|choice| choice.alignment).collect();
            assert!(
                alignments.contains(&InsightAlignment::Converge),
                "{id} missing converge"
            );
            assert!(
                alignments.contains(&InsightAlignment::Neutral),
                "{id} missing neutral"
            );
            assert!(
                alignments.contains(&InsightAlignment::Diverge),
                "{id} missing diverge"
            );
        }
    }

    #[test]
    fn all_fallback_options_pass_arbiter() {
        let quota = InsightQuota::default();
        for id in KNOWN_TRIGGERS {
            for choice in fallback_for(id) {
                validate_offer(&quota, &choice, Realm::Induce)
                    .unwrap_or_else(|e| panic!("fallback for {id} failed arbiter: {e:?}"));
            }
        }
    }

    #[test]
    fn unknown_trigger_yields_empty() {
        assert!(fallback_for("unknown_thing_xyz").is_empty());
    }

    #[test]
    fn fallback_produces_three_alignments() {
        let opts = fallback_for("first_breakthrough_to_Induce");
        assert_eq!(
            opts.iter()
                .map(|choice| choice.alignment)
                .collect::<Vec<_>>(),
            vec![
                InsightAlignment::Converge,
                InsightAlignment::Neutral,
                InsightAlignment::Diverge
            ]
        );
    }

    #[test]
    fn every_tradeoff_has_cost() {
        let opts = fallback_for("first_breakthrough_to_Induce");
        assert!(opts.iter().all(|choice| choice.cost_magnitude > 0.0));
    }

    #[test]
    fn converge_cost_is_opposite_or_narrowing_cost() {
        let opts = fallback_for("first_breakthrough_to_Induce");
        let converge = opts
            .iter()
            .find(|choice| choice.alignment == InsightAlignment::Converge)
            .unwrap();
        assert!(matches!(
            converge.cost,
            InsightCost::OppositeColorPenalty { .. } | InsightCost::ChaoticToleranceLoss { .. }
        ));
    }

    #[test]
    fn diverge_cost_is_main_color_penalty() {
        let opts = fallback_for("first_breakthrough_to_Induce");
        let diverge = opts
            .iter()
            .find(|choice| choice.alignment == InsightAlignment::Diverge)
            .unwrap();
        assert!(matches!(
            diverge.cost,
            InsightCost::MainColorPenalty {
                color: ColorKind::Mellow,
                ..
            }
        ));
    }

    #[test]
    fn neutral_cost_matches_pair_axis() {
        let opts = fallback_for("first_breakthrough_to_Induce");
        let neutral = opts
            .iter()
            .find(|choice| choice.alignment == InsightAlignment::Neutral)
            .unwrap();
        assert!(matches!(
            neutral.cost,
            InsightCost::ShockSensitivity { .. }
                | InsightCost::BreakthroughFailurePenalty { .. }
                | InsightCost::QiVolatility { .. }
                | InsightCost::SenseExposure { .. }
        ));
    }

    #[test]
    fn diverge_injects_target_color_metadata() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 10.0);
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let opts = fallback_for_context(
            "first_breakthrough_to_Induce",
            &qi,
            &log,
            &InsightQuota::default(),
            Realm::Induce,
        );
        let diverge = opts
            .iter()
            .find(|choice| choice.alignment == InsightAlignment::Diverge)
            .unwrap();
        assert!(diverge.target_color.is_some());
    }

    #[test]
    fn cost_flavor_not_empty() {
        let opts = fallback_for("first_breakthrough_to_Induce");
        assert!(opts
            .iter()
            .all(|choice| !choice.cost_flavor.trim().is_empty()));
    }

    #[test]
    fn gain_flavor_contains_color_name() {
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let opts = fallback_for_context(
            "first_breakthrough_to_Induce",
            &qi,
            &PracticeLog::default(),
            &InsightQuota::default(),
            Realm::Induce,
        );
        assert!(opts.iter().any(|choice| choice.flavor.contains("锋锐")));
    }
}
