//! plan-alchemy-v2 P0 — `SideEffect.tag` 到真实战斗状态 / 顿悟触发的唯一映射入口。

use valence::prelude::Entity;

use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::cultivation::components::Realm;
use crate::cultivation::insight::InsightRequest;

use super::recipe::SideEffect;

const TICKS_PER_SECOND: u64 = 20;
const DEFAULT_SIDE_EFFECT_TICKS: u64 = 30 * TICKS_PER_SECOND;

#[derive(Debug, Clone)]
pub struct SideEffectApplication {
    pub status_intent: ApplyStatusEffectIntent,
    pub insight_request: Option<InsightRequest>,
}

pub fn status_kind_for_side_effect_tag(tag: &str) -> StatusEffectKind {
    match tag {
        "minor_qi_regen_boost" | "qi_regen_boost" => StatusEffectKind::QiRegenBoost,
        "rare_insight_flash" => StatusEffectKind::InsightFlash,
        "qi_cap_perm_minus_1" => StatusEffectKind::QiCapPermMinus,
        "contam_boost" => StatusEffectKind::ContaminationBoost,
        other => {
            tracing::warn!(
                "[bong][alchemy] unknown side_effect tag `{other}`; preserving as AlchemyBuff"
            );
            StatusEffectKind::AlchemyBuff(other.to_string())
        }
    }
}

pub fn side_effect_duration_ticks(effect: &SideEffect) -> u64 {
    if effect.perm {
        return u64::MAX;
    }
    if effect.duration_s == 0 {
        return DEFAULT_SIDE_EFFECT_TICKS;
    }
    u64::from(effect.duration_s).saturating_mul(TICKS_PER_SECOND)
}

pub fn side_effect_magnitude(effect: &SideEffect) -> f32 {
    let explicit = effect.amount.unwrap_or_default() as f32;
    if explicit > 0.0 {
        return explicit;
    }

    match effect.tag.as_str() {
        "minor_qi_regen_boost" => 0.10,
        "qi_regen_boost" => 0.25,
        "rare_insight_flash" => 0.05,
        "qi_cap_perm_minus_1" => 0.01,
        "contam_boost" => 0.15,
        _ => 0.05,
    }
}

pub fn build_side_effect_application(
    target: Entity,
    effect: &SideEffect,
    issued_at_tick: u64,
    realm: Realm,
) -> SideEffectApplication {
    let kind = status_kind_for_side_effect_tag(effect.tag.as_str());
    let insight_request = (kind == StatusEffectKind::InsightFlash).then(|| InsightRequest {
        entity: target,
        trigger_id: format!("alchemy_side_effect:{}", effect.tag),
        realm,
    });

    SideEffectApplication {
        status_intent: ApplyStatusEffectIntent {
            target,
            kind,
            magnitude: side_effect_magnitude(effect),
            duration_ticks: side_effect_duration_ticks(effect),
            issued_at_tick,
        },
        insight_request,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::recipe::SideEffect;

    fn effect(tag: &str) -> SideEffect {
        SideEffect {
            tag: tag.to_string(),
            duration_s: 7,
            weight: 1,
            perm: false,
            color: None,
            amount: None,
        }
    }

    #[test]
    fn known_tags_map_to_status_effect_variants() {
        assert_eq!(
            status_kind_for_side_effect_tag("minor_qi_regen_boost"),
            StatusEffectKind::QiRegenBoost
        );
        assert_eq!(
            status_kind_for_side_effect_tag("qi_regen_boost"),
            StatusEffectKind::QiRegenBoost
        );
        assert_eq!(
            status_kind_for_side_effect_tag("rare_insight_flash"),
            StatusEffectKind::InsightFlash
        );
        assert_eq!(
            status_kind_for_side_effect_tag("qi_cap_perm_minus_1"),
            StatusEffectKind::QiCapPermMinus
        );
        assert_eq!(
            status_kind_for_side_effect_tag("contam_boost"),
            StatusEffectKind::ContaminationBoost
        );
    }

    #[test]
    fn unknown_tag_is_preserved_as_alchemy_buff() {
        assert_eq!(
            status_kind_for_side_effect_tag("strange_cold_sweat"),
            StatusEffectKind::AlchemyBuff("strange_cold_sweat".to_string())
        );
    }

    #[test]
    fn permanent_side_effect_uses_never_expiring_status_duration() {
        let mut effect = effect("qi_cap_perm_minus_1");
        effect.perm = true;

        assert_eq!(side_effect_duration_ticks(&effect), u64::MAX);
    }

    #[test]
    fn application_emits_status_intent_and_insight_request_for_flash() {
        let target = Entity::from_raw(7);
        let app =
            build_side_effect_application(target, &effect("rare_insight_flash"), 42, Realm::Awaken);

        assert_eq!(app.status_intent.target, target);
        assert_eq!(app.status_intent.kind, StatusEffectKind::InsightFlash);
        assert_eq!(app.status_intent.duration_ticks, 7 * TICKS_PER_SECOND);
        let insight = app
            .insight_request
            .expect("rare insight flash should request insight");
        assert_eq!(insight.entity, target);
        assert_eq!(insight.trigger_id, "alchemy_side_effect:rare_insight_flash");
        assert_eq!(insight.realm, Realm::Awaken);
    }
}
