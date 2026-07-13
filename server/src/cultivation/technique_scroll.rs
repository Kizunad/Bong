use valence::prelude::{bevy_ecs, Entity, Event};

use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
use crate::cultivation::known_techniques::{
    technique_definition, KnownTechnique, KnownTechniques, TechniqueDefinition,
};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::inventory::ItemTemplate;

#[derive(Debug, Clone, Event, PartialEq)]
pub struct TechniqueScrollReadEvent {
    pub player: Entity,
    pub technique_id: String,
    pub source_item: String,
    pub outcome: ScrollReadOutcome,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct TechniqueLearnedEvent {
    pub player: Entity,
    pub technique_id: String,
    pub source: LearnSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LearnSource {
    Scroll {
        item_id: String,
    },
    Observe {
        observed_entity: Entity,
    },
    Mentor {
        npc_entity: Entity,
    },
    DyingMaster {
        npc_entity: Entity,
    },
    DevCommand,
    /// plan-onboarding-loop-v1 P1.2 — 战斗中本能领悟（首次受击自学闪避）。
    CombatInsight,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScrollReadOutcome {
    Learned,
    AlreadyKnown,
    RealmTooLow {
        required: Realm,
        current: Realm,
    },
    /// plan-race-system-v1 P3a（决议 §8.1 #5/#6）——功法域 race gate 拒绝：本体 race_id /
    /// `intrinsic_is_humanoid` 未通过 `TechniqueDefinition.required_race`。境界门之后、
    /// 经脉门之前判定（见 `learn_technique_if_allowed`）。
    RaceMismatch,
    MeridianSevered {
        channel: MeridianId,
    },
    MeridianMissing {
        channel: MeridianId,
    },
    /// plan-race-system-v1 P4 —— 易形类技能（`morph.yixing`，见
    /// `body_plan::technique_requires_form_anchor`）专属前置门：本体
    /// `MeridianProfile` 内全部 `ChannelRole::FormAnchor` 经脉未全部打通/未断绝。
    /// `meridian_profile` 缺失（调用方未接入 body_plan registry）时同样判定本变体
    /// （fail-closed，不因资源缺失而放行易形）。
    FormAnchorClosed,
    InvalidScroll,
}

pub fn read_combat_technique_scroll(
    known: &mut KnownTechniques,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    template: &ItemTemplate,
    intrinsic_is_humanoid: bool,
    meridian_profile: Option<&crate::body_plan::MeridianProfile>,
) -> ScrollReadOutcome {
    let Some(spec) = template.technique_scroll_spec.as_ref() else {
        return ScrollReadOutcome::InvalidScroll;
    };
    if spec.kind != "combat_technique" {
        return ScrollReadOutcome::InvalidScroll;
    }
    learn_technique_if_allowed(
        known,
        cultivation,
        meridians,
        severed,
        spec.skill_id.as_str(),
        0.0,
        intrinsic_is_humanoid,
        meridian_profile,
    )
}

/// `intrinsic_is_humanoid`：调用方按 `BodyPlanPurpose::Intrinsic` 解析本体 BodyPlan 得到的
/// `is_humanoid`（见 `body_plan::resolve_body_plan_for_target` 文档）——本函数保持纯函数
/// 签名，不依赖 Bevy `Query`/`Res`，方便单测直接构造。
#[allow(clippy::too_many_arguments)]
pub fn learn_technique_if_allowed(
    known: &mut KnownTechniques,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    technique_id: &str,
    initial_proficiency: f32,
    intrinsic_is_humanoid: bool,
    // plan-race-system-v1 P4 —— 本体 `MeridianProfile`（`resolve_body_plan(Intrinsic)`
    // 的解析结果），供 `morph.yixing` 一类易形技能的 `form_anchors_open` 前置门使用；
    // 其余技能不消费本字段，既有调用点可安全传 `None`。
    meridian_profile: Option<&crate::body_plan::MeridianProfile>,
) -> ScrollReadOutcome {
    let Some(definition) = technique_definition(technique_id) else {
        return ScrollReadOutcome::InvalidScroll;
    };
    if known.entries.iter().any(|entry| entry.id == technique_id) {
        return ScrollReadOutcome::AlreadyKnown;
    }
    if let Some(required) = required_realm(definition) {
        if realm_rank(cultivation.realm) < realm_rank(required) {
            return ScrollReadOutcome::RealmTooLow {
                required,
                current: cultivation.realm,
            };
        }
    } else {
        return ScrollReadOutcome::InvalidScroll;
    }
    if !definition
        .required_race
        .allows(&cultivation.race, intrinsic_is_humanoid)
    {
        return ScrollReadOutcome::RaceMismatch;
    }
    if crate::body_plan::technique_requires_form_anchor(technique_id) {
        let anchors_ok = meridian_profile
            .map(|profile| crate::body_plan::form_anchors_open(profile, meridians, severed))
            .unwrap_or(false);
        if !anchors_ok {
            return ScrollReadOutcome::FormAnchorClosed;
        }
    }
    if let Err(outcome) = check_required_meridians(definition, meridians, severed) {
        return outcome;
    }

    known.entries.push(KnownTechnique {
        id: technique_id.to_string(),
        proficiency: initial_proficiency.clamp(0.0, 1.0),
        active: true,
    });
    ScrollReadOutcome::Learned
}

pub fn can_learn_technique(
    known: &KnownTechniques,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    technique_id: &str,
    intrinsic_is_humanoid: bool,
    meridian_profile: Option<&crate::body_plan::MeridianProfile>,
) -> ScrollReadOutcome {
    let mut probe = known.clone();
    learn_technique_if_allowed(
        &mut probe,
        cultivation,
        meridians,
        severed,
        technique_id,
        0.0,
        intrinsic_is_humanoid,
        meridian_profile,
    )
}

fn check_required_meridians(
    definition: &TechniqueDefinition,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
) -> Result<(), ScrollReadOutcome> {
    for required in definition.required_meridians {
        let Some(channel) = parse_meridian_id(required.channel) else {
            return Err(ScrollReadOutcome::InvalidScroll);
        };
        if severed.is_some_and(|severed| severed.is_severed(channel)) {
            return Err(ScrollReadOutcome::MeridianSevered { channel });
        }
        let state = meridians.get(channel);
        if !state.opened || state.integrity < f64::from(required.min_health) {
            return Err(ScrollReadOutcome::MeridianMissing { channel });
        }
    }
    Ok(())
}

fn required_realm(definition: &TechniqueDefinition) -> Option<Realm> {
    match definition.required_realm {
        "Awaken" => Some(Realm::Awaken),
        "Induce" => Some(Realm::Induce),
        "Condense" => Some(Realm::Condense),
        "Solidify" => Some(Realm::Solidify),
        "Spirit" => Some(Realm::Spirit),
        "Void" => Some(Realm::Void),
        _ => None,
    }
}

pub fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

pub fn parse_meridian_id(raw: &str) -> Option<MeridianId> {
    match raw {
        "Lung" => Some(MeridianId::Lung),
        "LargeIntestine" => Some(MeridianId::LargeIntestine),
        "Stomach" => Some(MeridianId::Stomach),
        "Spleen" => Some(MeridianId::Spleen),
        "Heart" => Some(MeridianId::Heart),
        "SmallIntestine" => Some(MeridianId::SmallIntestine),
        "Bladder" => Some(MeridianId::Bladder),
        "Kidney" => Some(MeridianId::Kidney),
        "Pericardium" => Some(MeridianId::Pericardium),
        "TripleEnergizer" => Some(MeridianId::TripleEnergizer),
        "GallBladder" | "Gallbladder" => Some(MeridianId::Gallbladder),
        "Liver" => Some(MeridianId::Liver),
        "Ren" => Some(MeridianId::Ren),
        "Du" => Some(MeridianId::Du),
        "Chong" => Some(MeridianId::Chong),
        "Dai" => Some(MeridianId::Dai),
        "YinQiao" => Some(MeridianId::YinQiao),
        "YangQiao" => Some(MeridianId::YangQiao),
        "YinWei" => Some(MeridianId::YinWei),
        "YangWei" => Some(MeridianId::YangWei),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ItemCategory, ItemRarity, TechniqueScrollSpec, DEFAULT_CAST_DURATION_MS,
        DEFAULT_COOLDOWN_MS,
    };

    fn template(id: &str, skill_id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: "涡流残卷".to_string(),
            category: ItemCategory::Scroll,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 0.05,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 0.5,
            description: "test".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: Some(TechniqueScrollSpec {
                kind: "combat_technique".to_string(),
                skill_id: skill_id.to_string(),
            }),
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        }
    }

    fn open_required_meridians(meridians: &mut MeridianSystem, skill_id: &str) {
        let definition = technique_definition(skill_id).unwrap();
        for required in definition.required_meridians {
            let id = parse_meridian_id(required.channel).unwrap();
            let channel = meridians.get_mut(id);
            channel.opened = true;
            channel.integrity = 1.0;
        }
    }

    #[test]
    fn read_scroll_success() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
            true,
            None,
        );

        assert_eq!(outcome, ScrollReadOutcome::Learned);
        assert_eq!(known.entries.len(), 1);
        assert_eq!(known.entries[0].id, "woliu.vortex");
        assert_eq!(known.entries[0].proficiency, 0.0);
        assert!(known.entries[0].active);
    }

    #[test]
    fn read_scroll_realm_too_low() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation::default();
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
            true,
            None,
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::RealmTooLow {
                required: Realm::Condense,
                current: Realm::Awaken
            }
        );
        assert!(known.entries.is_empty());
    }

    #[test]
    fn read_scroll_meridian_severed() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");
        let mut severed = MeridianSeveredPermanent::default();
        severed
            .severed_meridians
            .insert(MeridianId::Lung.channel_id());

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            Some(&severed),
            &template("scroll_woliu_vortex", "woliu.vortex"),
            true,
            None,
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::MeridianSevered {
                channel: MeridianId::Lung
            }
        );
        assert!(known.entries.is_empty());
    }

    #[test]
    fn read_scroll_meridian_missing() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &MeridianSystem::default(),
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
            true,
            None,
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::MeridianMissing {
                channel: MeridianId::Lung
            }
        );
        assert!(known.entries.is_empty());
    }

    #[test]
    fn read_scroll_already_known() {
        let mut known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "woliu.vortex".to_string(),
                proficiency: 0.2,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
            true,
            None,
        );

        assert_eq!(outcome, ScrollReadOutcome::AlreadyKnown);
        assert_eq!(known.entries.len(), 1);
        assert_eq!(known.entries[0].proficiency, 0.2);
    }

    #[test]
    fn read_scroll_invalid() {
        let mut known = KnownTechniques::default();
        let mut invalid = template("scroll_bad", "woliu.vortex");
        invalid.technique_scroll_spec = None;

        let outcome = read_combat_technique_scroll(
            &mut known,
            &Cultivation::default(),
            &MeridianSystem::default(),
            None,
            &invalid,
            true,
            None,
        );

        assert_eq!(outcome, ScrollReadOutcome::InvalidScroll);
        assert!(known.entries.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────
    // plan-race-system-v1 P3a —— 习得门 race gate（境界门后、经脉门前）。
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn learn_humanoid_gated_technique_rejects_non_humanoid_intrinsic() {
        // woliu.vortex 是 RaceGate::Humanoid；is_humanoid=false 必须拒绝，即便境界/经脉
        // 全部满足。
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = learn_technique_if_allowed(
            &mut known,
            &cultivation,
            &meridians,
            None,
            "woliu.vortex",
            0.0,
            false,
            None,
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::RaceMismatch,
            "is_humanoid=false 必须被 Humanoid 档拒绝"
        );
        assert!(known.entries.is_empty());
    }

    #[test]
    fn learn_humanoid_gated_technique_allows_humanoid_intrinsic() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = learn_technique_if_allowed(
            &mut known,
            &cultivation,
            &meridians,
            None,
            "woliu.vortex",
            0.0,
            true,
            None,
        );

        assert_eq!(outcome, ScrollReadOutcome::Learned);
    }

    #[test]
    fn learn_any_gated_technique_ignores_is_humanoid() {
        // movement.dash 是 RaceGate::Any——is_humanoid=false（如飞鲸种族）也必须放行。
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation::default();
        let meridians = MeridianSystem::default();

        let outcome = learn_technique_if_allowed(
            &mut known,
            &cultivation,
            &meridians,
            None,
            "movement.dash",
            0.0,
            false,
            None,
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::Learned,
            "Any 档不应因 is_humanoid=false 被拒绝"
        );
    }

    #[test]
    fn race_gate_checked_after_realm_gate_before_meridian_gate() {
        // 境界不足时优先报 RealmTooLow，即便种族也不合格——验证门的顺序（境界门先于
        // race gate）。
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation::default(); // Realm::Awaken < Condense
        let meridians = MeridianSystem::default();

        let outcome = learn_technique_if_allowed(
            &mut known,
            &cultivation,
            &meridians,
            None,
            "woliu.vortex",
            0.0,
            false,
            None,
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::RealmTooLow {
                required: Realm::Condense,
                current: Realm::Awaken,
            },
            "境界门必须先于 race gate 判定"
        );

        // 境界满足后，race gate 必须先于经脉门判定——meridians 全未开，若 race gate 未
        // 生效会误报 MeridianMissing 而非 RaceMismatch。
        let cultivation_ok_realm = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let outcome2 = learn_technique_if_allowed(
            &mut known,
            &cultivation_ok_realm,
            &meridians,
            None,
            "woliu.vortex",
            0.0,
            false,
            None,
        );
        assert_eq!(
            outcome2,
            ScrollReadOutcome::RaceMismatch,
            "race gate 必须先于经脉门判定，未开经脉不应掩盖 RaceMismatch"
        );
    }

    #[test]
    fn already_known_short_circuits_before_race_gate() {
        // AlreadyKnown 判定在 race gate 之前——已学会的功法不该因为易形/换种族后
        // is_humanoid 变化而报错误的 RaceMismatch（幂等：已知即返回 AlreadyKnown）。
        let mut known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "woliu.vortex".to_string(),
                proficiency: 0.2,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let meridians = MeridianSystem::default();

        let outcome = learn_technique_if_allowed(
            &mut known,
            &cultivation,
            &meridians,
            None,
            "woliu.vortex",
            0.0,
            false,
            None,
        );

        assert_eq!(outcome, ScrollReadOutcome::AlreadyKnown);
    }

    #[test]
    fn can_learn_technique_respects_race_gate_without_mutating_known() {
        let known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = can_learn_technique(
            &known,
            &cultivation,
            &meridians,
            None,
            "woliu.vortex",
            false,
            None,
        );
        assert_eq!(outcome, ScrollReadOutcome::RaceMismatch);
        assert!(
            known.entries.is_empty(),
            "can_learn_technique 只探测，不应写入 known"
        );

        let outcome_ok =
            can_learn_technique(&known, &cultivation, &meridians, None, "woliu.vortex", true, None);
        assert_eq!(outcome_ok, ScrollReadOutcome::Learned);
        assert!(known.entries.is_empty(), "探测不应留下副作用");
    }

    #[test]
    fn read_combat_technique_scroll_propagates_race_mismatch() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
            false,
            None,
        );

        assert_eq!(outcome, ScrollReadOutcome::RaceMismatch);
        assert!(known.entries.is_empty());
    }
}
