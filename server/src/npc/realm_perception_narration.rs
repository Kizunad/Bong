//! plan-npc-realm-distribution-v1 P3 感知面 — 境界识破 narration 示例。
//!
//! 境界感知本身（谁能看穿谁的境界）由 `spiritual_sense/scanner.rs`（§937 观察者侧门控，
//! 固元+ 才读得到对方大致段位）和 worldview §439/§554 伪灵皮藏拙机制承担，本模块**不**
//! 重实现感知门控，也**不**新增常驻广播式效果（§8.1 #5 已拍板拒绝气息粒子）。
//!
//! 本模块只提供 P3 §8.1 决议给定的两条示例文案 + 一个 zone-scope、`Perception` 风格
//! 的推送封装，供任何日后确实发生"境界识破"的具体事件（比如 P3 迁移重 roll 出的新
//! 高境界居民）复用，避免每个调用点各写一份文案硬编码。

use crate::cultivation::components::Realm;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;

/// 凝脉境示例：市井藏拙。
const CONDENSE_NARRATION: &str =
    "集市角落那个补锅匠收锤时指缝漏了一缕凝而不散的白气——凝脉，藏得很深";

/// 固元境示例：步态外露。
const SOLIDIFY_NARRATION: &str = "北荒来的独行客靴底不沾尘，固元境的横行是写在步子里的";

/// 给定 realm，返回对应的境界识破 narration 示例文案。
///
/// 只有凝脉 / 固元两档有文案（§8.1 决议原文两条示例，均取自这两档）；其余四档返回
/// `None`——醒灵 / 引气太寻常无谈资，通灵 / 化虚极稀有另有专属剧情（垂死大能等）承接，
/// 不该被这两条通用市井文案稀释。
pub fn realm_perception_narration_line(realm: Realm) -> Option<&'static str> {
    match realm {
        Realm::Condense => Some(CONDENSE_NARRATION),
        Realm::Solidify => Some(SOLIDIFY_NARRATION),
        Realm::Awaken | Realm::Induce | Realm::Spirit | Realm::Void => None,
    }
}

/// 若 `realm` 有对应文案，以 `NarrationScope::Zone` + `NarrationStyle::Perception` 推入
/// `narrations`（境界感知是观察者侧的门控能力，不是全服广播，§937）。返回是否实际推送。
pub fn push_realm_perception_narration(
    narrations: &mut PendingGameplayNarrations,
    zone: &str,
    realm: Realm,
) -> bool {
    match realm_perception_narration_line(realm) {
        Some(line) => {
            narrations.push_zone(zone, line, NarrationStyle::Perception);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::common::NarrationScope;

    #[test]
    fn condense_narration_pinned_verbatim() {
        assert_eq!(
            realm_perception_narration_line(Realm::Condense),
            Some("集市角落那个补锅匠收锤时指缝漏了一缕凝而不散的白气——凝脉，藏得很深")
        );
    }

    #[test]
    fn solidify_narration_pinned_verbatim() {
        assert_eq!(
            realm_perception_narration_line(Realm::Solidify),
            Some("北荒来的独行客靴底不沾尘，固元境的横行是写在步子里的")
        );
    }

    #[test]
    fn other_four_realms_have_no_narration_line() {
        for realm in [Realm::Awaken, Realm::Induce, Realm::Spirit, Realm::Void] {
            assert_eq!(
                realm_perception_narration_line(realm),
                None,
                "realm={realm:?} 不该有市井境界识破文案（醒灵/引气太寻常，通灵/化虚另有专属剧情）"
            );
        }
    }

    #[test]
    fn push_realm_perception_narration_uses_zone_scope_and_perception_style() {
        let mut narrations = PendingGameplayNarrations::default();

        let pushed =
            push_realm_perception_narration(&mut narrations, "青云峰外市集", Realm::Condense);
        assert!(pushed, "Condense 应有文案可推送");

        let drained = narrations.drain();
        assert_eq!(drained.len(), 1, "应恰好推送一条 narration");
        assert_eq!(drained[0].scope, NarrationScope::Zone);
        assert_eq!(drained[0].target.as_deref(), Some("青云峰外市集"));
        assert_eq!(drained[0].style, NarrationStyle::Perception);
        assert_eq!(
            drained[0].text,
            "集市角落那个补锅匠收锤时指缝漏了一缕凝而不散的白气——凝脉，藏得很深"
        );
    }

    #[test]
    fn push_realm_perception_narration_no_op_for_realms_without_a_line() {
        let mut narrations = PendingGameplayNarrations::default();

        let pushed = push_realm_perception_narration(&mut narrations, "北荒", Realm::Awaken);

        assert!(!pushed, "Awaken 没有文案，不应推送");
        assert!(
            narrations.drain().is_empty(),
            "没有文案时不该产生任何 pending narration"
        );
    }
}
