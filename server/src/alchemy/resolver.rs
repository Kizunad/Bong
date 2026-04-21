//! 炼丹结算 — session 结束时的完整流水线。
//!
//! 整合：
//!   * 精确 vs 残缺 匹配（plan §1.3）
//!   * DeviationSummary → OutcomeBucket
//!   * flawed_fallback 副作用抽取
//!   * LifeRecord 记录（plan §1.3 "试药史"）
//!
//! 输出 `ResolvedOutcome` 供调用侧应用（产丹到背包 / 炸炉扣 integrity / 施加 meridian_crack）。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::ColorKind;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};

use super::outcome::{build_flawed_result, classify_precise, OutcomeBucket};
use super::recipe::{Recipe, RecipeRegistry, SideEffect};
use super::session::AlchemySession;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolvedOutcome {
    /// 成丹（精确匹配 perfect/good/flawed，或残缺匹配 flawed）。
    Pill {
        recipe_id: String,
        pill: String,
        quality: f64,
        toxin_amount: f64,
        toxin_color: ColorKind,
        qi_gain: Option<f64>,
        /// 残缺路径抽中的副作用（精确路径为 None）。
        side_effect: Option<SideEffect>,
        /// 路径标记：true = 残缺匹配路径。
        flawed_path: bool,
    },
    /// 废丹，无产出。
    Waste { recipe_id: Option<String> },
    /// 炸炉。
    Explode { damage: f64, meridian_crack: f64 },
    /// 不匹配任何配方（投错料且无 flawed_fallback 命中）。
    Mismatch,
}

/// plan §1.3 残缺匹配用 seed 来源 — session 本身的确定性哈希（不依赖 rand）。
fn session_seed(session: &AlchemySession) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    mix(session.elapsed_ticks as u64);
    for (t, temp) in &session.temp_track {
        mix(*t as u64);
        mix(temp.to_bits());
    }
    mix(session.qi_injected.to_bits());
    for (k, v) in &session.staged.materials {
        for b in k.as_bytes() {
            mix(*b as u64);
        }
        mix(*v as u64);
    }
    h
}

/// 主入口：session 结束时调用。`recipe` 是 session 绑定的配方（learned 翻到这张）。
///
/// 流程：
/// 1. 先看 staged 材料是否与 recipe.stage0 精确匹配
/// 2. 精确：走 DeviationSummary → 桶
/// 3. 否则去 RecipeRegistry 找 flawed_fallback
/// 4. 若有匹配：构造 FlawedResult → ResolvedOutcome::Pill(flawed_path=true)
/// 5. 否则根据温度/qi 严重度返回 Waste 或 Explode
pub fn resolve(
    session: &AlchemySession,
    recipe: &Recipe,
    registry: &RecipeRegistry,
) -> ResolvedOutcome {
    let outcome = resolve_raw(session, recipe, registry);
    // plan-shelflife-v1 §5.1 M5c — 按 staged.quality_factor 折算 qi_gain。
    // Bucket / quality / pill ID 不变（freshness 不影响"炼丹技艺档位"，只影响"产出 qi 量"）。
    apply_quality_factor(outcome, session.staged.quality_factor)
}

fn resolve_raw(
    session: &AlchemySession,
    recipe: &Recipe,
    registry: &RecipeRegistry,
) -> ResolvedOutcome {
    let staged = &session.staged.materials;
    let need = recipe.stage0_ingredients();

    // 精确匹配判定（仅首 stage；多 stage 的中途料是否齐通过 summary.missed_stage 体现）
    let is_exact_stage0 = staged == &need;

    if is_exact_stage0 {
        let summary = session.summarize(recipe);
        let bucket = classify_precise(&summary);
        return map_exact_bucket(recipe, bucket);
    }

    // 残缺匹配：在 registry 找 subset 命中
    if let Some((hit_recipe, missing_ratio)) = registry.match_flawed(staged) {
        // 仍然要看温度/qi：qi_deficit → 废，severe_overheat → 炸，其余走 flawed
        let summary = session.summarize(hit_recipe);
        if summary.severe_overheat {
            if let Some(ex) = &hit_recipe.outcomes.explode {
                return ResolvedOutcome::Explode {
                    damage: ex.damage,
                    meridian_crack: ex.meridian_crack,
                };
            }
            return ResolvedOutcome::Explode {
                damage: 10.0,
                meridian_crack: 0.1,
            };
        }
        if summary.qi_deficit {
            return ResolvedOutcome::Waste {
                recipe_id: Some(hit_recipe.id.clone()),
            };
        }
        let toxin_color = hit_recipe
            .outcomes
            .flawed
            .as_ref()
            .map(|o| o.toxin_color)
            .or_else(|| hit_recipe.outcomes.perfect.as_ref().map(|o| o.toxin_color))
            .unwrap_or(ColorKind::Turbid);
        if let Some(result) = build_flawed_result(
            hit_recipe,
            toxin_color,
            missing_ratio,
            session_seed(session),
        ) {
            return ResolvedOutcome::Pill {
                recipe_id: hit_recipe.id.clone(),
                pill: result.pill,
                quality: result.quality,
                toxin_amount: result.toxin_amount,
                toxin_color: result.toxin_color,
                qi_gain: None,
                side_effect: result.side_effect,
                flawed_path: true,
            };
        }
    }

    // 乱投
    let summary = session.summarize(recipe);
    if summary.severe_overheat {
        if let Some(ex) = &recipe.outcomes.explode {
            return ResolvedOutcome::Explode {
                damage: ex.damage,
                meridian_crack: ex.meridian_crack,
            };
        }
        return ResolvedOutcome::Explode {
            damage: 10.0,
            meridian_crack: 0.1,
        };
    }
    ResolvedOutcome::Mismatch
}

/// plan-shelflife-v1 §5.1 M5c — 把 staged.quality_factor 应用到 Pill.qi_gain。
/// 其它 outcome 变体（Waste / Explode / Mismatch）不受影响。
/// factor ≈ 1.0 时短路，避免浮点漂移破坏现有 exact-equality 测试。
fn apply_quality_factor(outcome: ResolvedOutcome, factor: f32) -> ResolvedOutcome {
    if (factor - 1.0).abs() < f32::EPSILON {
        return outcome;
    }
    match outcome {
        ResolvedOutcome::Pill {
            recipe_id,
            pill,
            quality,
            toxin_amount,
            toxin_color,
            qi_gain,
            side_effect,
            flawed_path,
        } => ResolvedOutcome::Pill {
            recipe_id,
            pill,
            quality,
            toxin_amount,
            toxin_color,
            qi_gain: qi_gain.map(|q| q * factor as f64),
            side_effect,
            flawed_path,
        },
        other => other,
    }
}

fn map_exact_bucket(recipe: &Recipe, bucket: OutcomeBucket) -> ResolvedOutcome {
    match bucket {
        OutcomeBucket::Perfect | OutcomeBucket::Good | OutcomeBucket::Flawed => {
            let outcome = match bucket {
                OutcomeBucket::Perfect => recipe.outcomes.perfect.as_ref(),
                OutcomeBucket::Good => recipe.outcomes.good.as_ref(),
                OutcomeBucket::Flawed => recipe.outcomes.flawed.as_ref(),
                _ => None,
            };
            match outcome {
                Some(o) => ResolvedOutcome::Pill {
                    recipe_id: recipe.id.clone(),
                    pill: o.pill.clone(),
                    quality: o.quality,
                    toxin_amount: o.toxin_amount,
                    toxin_color: o.toxin_color,
                    qi_gain: o.qi_gain,
                    side_effect: None,
                    flawed_path: false,
                },
                None => ResolvedOutcome::Waste {
                    recipe_id: Some(recipe.id.clone()),
                },
            }
        }
        OutcomeBucket::Waste => ResolvedOutcome::Waste {
            recipe_id: Some(recipe.id.clone()),
        },
        OutcomeBucket::Explode => {
            if let Some(ex) = &recipe.outcomes.explode {
                ResolvedOutcome::Explode {
                    damage: ex.damage,
                    meridian_crack: ex.meridian_crack,
                }
            } else {
                ResolvedOutcome::Explode {
                    damage: 10.0,
                    meridian_crack: 0.1,
                }
            }
        }
    }
}

/// plan §1.3 "试药史" — 把一次 resolve 结果写入 LifeRecord。
pub fn record_attempt_in_life(record: &mut LifeRecord, outcome: &ResolvedOutcome, tick: u64) {
    match outcome {
        ResolvedOutcome::Pill {
            recipe_id,
            flawed_path,
            side_effect,
            pill,
            ..
        } => {
            let side_tag = side_effect.as_ref().map(|s| s.tag.clone());
            record.push(BiographyEntry::AlchemyAttempt {
                recipe_id: recipe_id.clone(),
                pill: Some(pill.clone()),
                flawed_path: *flawed_path,
                side_effect_tag: side_tag,
                tick,
            });
        }
        ResolvedOutcome::Waste { recipe_id } => {
            record.push(BiographyEntry::AlchemyAttempt {
                recipe_id: recipe_id.clone().unwrap_or_else(|| "<mismatch>".into()),
                pill: None,
                flawed_path: false,
                side_effect_tag: None,
                tick,
            });
        }
        ResolvedOutcome::Explode { .. } => {
            record.push(BiographyEntry::AlchemyAttempt {
                recipe_id: "<explode>".into(),
                pill: None,
                flawed_path: false,
                side_effect_tag: None,
                tick,
            });
        }
        ResolvedOutcome::Mismatch => {
            record.push(BiographyEntry::AlchemyAttempt {
                recipe_id: "<mismatch>".into(),
                pill: None,
                flawed_path: false,
                side_effect_tag: None,
                tick,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::recipe::load_recipe_registry;
    use crate::alchemy::session::{AlchemySession, Intervention};

    fn drive_to_finish(session: &mut AlchemySession, recipe: &Recipe) {
        for _ in 0..recipe.fire_profile.target_duration_ticks {
            session.tick();
        }
    }

    #[test]
    fn exact_match_perfect_hui_yuan() {
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("hui_yuan_pill_v0").unwrap().clone();
        let mut s = AlchemySession::new(recipe.id.clone(), "alice".into());
        s.feed_stage(
            &recipe,
            0,
            &[("bai_cao".into(), 2, 1.0), ("ling_shui".into(), 1, 1.0)],
        )
        .unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.45));
        s.apply_intervention(Intervention::InjectQi(8.0));
        drive_to_finish(&mut s, &recipe);
        let out = resolve(&s, &recipe, &registry);
        match out {
            ResolvedOutcome::Pill {
                pill,
                flawed_path,
                qi_gain,
                ..
            } => {
                assert_eq!(pill, "hui_yuan_pill");
                assert!(!flawed_path);
                assert_eq!(qi_gain, Some(24.0));
            }
            other => panic!("expected perfect pill, got {other:?}"),
        }
    }

    #[test]
    fn flawed_subset_hits_fallback_for_kai_mai() {
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("kai_mai_pill_v0").unwrap().clone();
        let mut s = AlchemySession::new(recipe.id.clone(), "alice".into());
        // 缺 ling_shui
        s.feed_stage(&recipe, 0, &[("kai_mai_cao".into(), 3, 1.0)])
            .unwrap_or_default();
        s.apply_intervention(Intervention::AdjustTemp(0.60));
        s.apply_intervention(Intervention::InjectQi(15.0));
        drive_to_finish(&mut s, &recipe);
        let out = resolve(&s, &recipe, &registry);
        match out {
            ResolvedOutcome::Pill {
                pill,
                flawed_path,
                side_effect,
                ..
            } => {
                assert_eq!(pill, "kai_mai_pill_flawed");
                assert!(flawed_path);
                assert!(side_effect.is_some());
            }
            other => panic!("expected flawed pill, got {other:?}"),
        }
    }

    #[test]
    fn exact_match_explode_on_overheat() {
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("hui_yuan_pill_v0").unwrap().clone();
        let mut s = AlchemySession::new(recipe.id.clone(), "alice".into());
        s.feed_stage(
            &recipe,
            0,
            &[("bai_cao".into(), 2, 1.0), ("ling_shui".into(), 1, 1.0)],
        )
        .unwrap();
        s.apply_intervention(Intervention::AdjustTemp(1.0));
        s.apply_intervention(Intervention::InjectQi(8.0));
        drive_to_finish(&mut s, &recipe);
        let out = resolve(&s, &recipe, &registry);
        matches!(out, ResolvedOutcome::Explode { .. });
    }

    #[test]
    fn pure_mismatch_returns_mismatch_not_pill() {
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("hui_yuan_pill_v0").unwrap().clone();
        let mut s = AlchemySession::new(recipe.id.clone(), "alice".into());
        // unknown material only
        s.staged.materials.insert("garbage_herb".into(), 1);
        s.apply_intervention(Intervention::AdjustTemp(0.45));
        s.apply_intervention(Intervention::InjectQi(8.0));
        drive_to_finish(&mut s, &recipe);
        let out = resolve(&s, &recipe, &registry);
        assert!(matches!(
            out,
            ResolvedOutcome::Mismatch | ResolvedOutcome::Explode { .. }
        ));
    }

    #[test]
    fn record_attempt_appends_biography() {
        let mut lr = LifeRecord::new("alice".to_string());
        let out = ResolvedOutcome::Pill {
            recipe_id: "hui_yuan_pill_v0".into(),
            pill: "hui_yuan_pill".into(),
            quality: 1.0,
            toxin_amount: 0.2,
            toxin_color: ColorKind::Mellow,
            qi_gain: Some(24.0),
            side_effect: None,
            flawed_path: false,
        };
        record_attempt_in_life(&mut lr, &out, 99);
        assert_eq!(lr.biography.len(), 1);
        match &lr.biography[0] {
            BiographyEntry::AlchemyAttempt {
                recipe_id,
                pill,
                flawed_path,
                tick,
                ..
            } => {
                assert_eq!(recipe_id, "hui_yuan_pill_v0");
                assert_eq!(pill.as_deref(), Some("hui_yuan_pill"));
                assert!(!*flawed_path);
                assert_eq!(*tick, 99);
            }
            other => panic!("unexpected biography: {other:?}"),
        }
    }

    // ============== M5c — quality_factor 折算 qi_gain ==============

    #[test]
    fn quality_factor_one_preserves_qi_gain() {
        // factor=1.0 短路，qi_gain 与原 perfect 值一致（24.0）。
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("hui_yuan_pill_v0").unwrap().clone();
        let mut s = AlchemySession::new(recipe.id.clone(), "alice".into());
        s.feed_stage(
            &recipe,
            0,
            &[("bai_cao".into(), 2, 1.0), ("ling_shui".into(), 1, 1.0)],
        )
        .unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.45));
        s.apply_intervention(Intervention::InjectQi(8.0));
        drive_to_finish(&mut s, &recipe);
        assert!((s.staged.quality_factor - 1.0).abs() < 1e-6);
        let out = resolve(&s, &recipe, &registry);
        match out {
            ResolvedOutcome::Pill { qi_gain, .. } => assert_eq!(qi_gain, Some(24.0)),
            other => panic!("expected pill, got {other:?}"),
        }
    }

    #[test]
    fn quality_factor_half_halves_qi_gain() {
        // factor=0.5 → qi_gain = 24 × 0.5 = 12
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("hui_yuan_pill_v0").unwrap().clone();
        let mut s = AlchemySession::new(recipe.id.clone(), "alice".into());
        s.feed_stage(
            &recipe,
            0,
            &[("bai_cao".into(), 2, 0.5), ("ling_shui".into(), 1, 0.5)],
        )
        .unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.45));
        s.apply_intervention(Intervention::InjectQi(8.0));
        drive_to_finish(&mut s, &recipe);
        assert!((s.staged.quality_factor - 0.5).abs() < 1e-6);
        let out = resolve(&s, &recipe, &registry);
        match out {
            ResolvedOutcome::Pill { qi_gain, .. } => {
                let q = qi_gain.expect("qi_gain");
                assert!((q - 12.0).abs() < 1e-6, "expected ~12.0, got {q}");
            }
            other => panic!("expected pill, got {other:?}"),
        }
    }

    #[test]
    fn quality_factor_mixed_fresh_and_half_weighted_average() {
        // 2 个 bai_cao factor=1.0 + 1 个 ling_shui factor=0.5 → acc = (2×1.0 + 1×0.5) / 3 = 0.833...
        // qi_gain = 24 × 0.833... = 20.0
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("hui_yuan_pill_v0").unwrap().clone();
        let mut s = AlchemySession::new(recipe.id.clone(), "alice".into());
        s.feed_stage(
            &recipe,
            0,
            &[("bai_cao".into(), 2, 1.0), ("ling_shui".into(), 1, 0.5)],
        )
        .unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.45));
        s.apply_intervention(Intervention::InjectQi(8.0));
        drive_to_finish(&mut s, &recipe);
        assert!(
            (s.staged.quality_factor - 0.8333).abs() < 1e-3,
            "running avg got {}",
            s.staged.quality_factor
        );
        let out = resolve(&s, &recipe, &registry);
        match out {
            ResolvedOutcome::Pill { qi_gain, .. } => {
                let q = qi_gain.expect("qi_gain");
                assert!((q - 20.0).abs() < 0.01, "expected ~20.0, got {q}");
            }
            other => panic!("expected pill, got {other:?}"),
        }
    }

    #[test]
    fn quality_factor_zero_dead_materials_give_zero_qi() {
        // factor=0.0 (全死料) → qi_gain=0
        let registry = load_recipe_registry().unwrap();
        let recipe = registry.get("hui_yuan_pill_v0").unwrap().clone();
        let mut s = AlchemySession::new(recipe.id.clone(), "alice".into());
        s.feed_stage(
            &recipe,
            0,
            &[("bai_cao".into(), 2, 0.0), ("ling_shui".into(), 1, 0.0)],
        )
        .unwrap();
        s.apply_intervention(Intervention::AdjustTemp(0.45));
        s.apply_intervention(Intervention::InjectQi(8.0));
        drive_to_finish(&mut s, &recipe);
        let out = resolve(&s, &recipe, &registry);
        match out {
            ResolvedOutcome::Pill { qi_gain, .. } => assert_eq!(qi_gain, Some(0.0)),
            other => {
                panic!("expected pill (even dead materials still produce pill), got {other:?}")
            }
        }
    }

    #[test]
    fn quality_factor_default_is_one_for_legacy_sessions() {
        // 无 Freshness 传 factor=1.0 的 test 路径：running avg 应保持 1.0。
        let mut s = AlchemySession::new("r".into(), "alice".into());
        assert_eq!(s.staged.quality_factor, 1.0);
        assert_eq!(s.staged.quality_total_count, 0);
    }

    #[test]
    fn session_seed_deterministic_across_same_state() {
        let mut s1 = AlchemySession::new("r".into(), "alice".into());
        let mut s2 = AlchemySession::new("r".into(), "alice".into());
        for _ in 0..5 {
            s1.apply_intervention(Intervention::AdjustTemp(0.5));
            s2.apply_intervention(Intervention::AdjustTemp(0.5));
            s1.tick();
            s2.tick();
        }
        assert_eq!(session_seed(&s1), session_seed(&s2));
    }
}
