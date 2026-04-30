use valence::prelude::{bevy_ecs, Entity, Event};

use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};
use crate::cultivation::components::{ColorKind, ContamSource, Contamination};
use crate::tools::ToolKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButcherDropKind {
    Bone,
    Meat,
    Hide,
}

impl ButcherDropKind {
    pub fn item_id(self) -> &'static str {
        match self {
            Self::Bone => "yi_beast_bone",
            Self::Meat => "raw_beast_meat",
            Self::Hide => "raw_beast_hide",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButcherSession {
    pub player: Entity,
    pub corpse: Entity,
    pub tool: Option<ToolKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButcherOutcome {
    pub drop: Option<ButcherDropKind>,
    pub wound: bool,
    pub contamination: bool,
}

#[derive(Debug, Clone, Event)]
pub struct ButcherRequest {
    pub player: Entity,
    pub corpse: Entity,
    pub tool: Option<ToolKind>,
}

pub fn start_butcher_session(
    player: Entity,
    corpse: Entity,
    tool: Option<ToolKind>,
) -> ButcherSession {
    ButcherSession {
        player,
        corpse,
        tool,
    }
}

pub fn resolve_butcher_session(session: &ButcherSession) -> ButcherOutcome {
    match session.tool {
        Some(ToolKind::GuHaiQian) => ButcherOutcome {
            drop: Some(ButcherDropKind::Bone),
            wound: false,
            contamination: false,
        },
        Some(ToolKind::CaiYaoDao) => ButcherOutcome {
            drop: Some(ButcherDropKind::Meat),
            wound: false,
            contamination: false,
        },
        Some(ToolKind::GuaDao) => ButcherOutcome {
            drop: Some(ButcherDropKind::Hide),
            wound: false,
            contamination: false,
        },
        _ => ButcherOutcome {
            drop: None,
            wound: true,
            contamination: true,
        },
    }
}

pub fn apply_bare_hand_butcher_hazard(
    wounds: &mut Wounds,
    contamination: &mut Contamination,
    now_tick: u64,
) {
    wounds.entries.push(Wound {
        location: BodyPart::ArmR,
        kind: WoundKind::Cut,
        severity: 0.35,
        bleeding_per_sec: 0.0,
        created_at_tick: now_tick,
        inflicted_by: Some("fauna_butcher_hazard".to_string()),
    });
    contamination.entries.push(ContamSource {
        amount: 0.4,
        color: ColorKind::Turbid,
        attacker_id: Some("fauna_butcher_hazard".to_string()),
        introduced_at: now_tick,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bone_pliers_extract_beast_bone() {
        let session = start_butcher_session(
            Entity::from_raw(1),
            Entity::from_raw(2),
            Some(ToolKind::GuHaiQian),
        );

        assert_eq!(
            resolve_butcher_session(&session),
            ButcherOutcome {
                drop: Some(ButcherDropKind::Bone),
                wound: false,
                contamination: false,
            }
        );
        assert_eq!(ButcherDropKind::Bone.item_id(), "yi_beast_bone");
    }

    #[test]
    fn cutting_tool_extracts_meat_and_scraper_extracts_hide() {
        let knife = start_butcher_session(
            Entity::from_raw(1),
            Entity::from_raw(2),
            Some(ToolKind::CaiYaoDao),
        );
        let scraper = start_butcher_session(
            Entity::from_raw(1),
            Entity::from_raw(2),
            Some(ToolKind::GuaDao),
        );

        assert_eq!(
            resolve_butcher_session(&knife).drop,
            Some(ButcherDropKind::Meat)
        );
        assert_eq!(
            resolve_butcher_session(&scraper).drop,
            Some(ButcherDropKind::Hide)
        );
    }

    #[test]
    fn bare_hand_butchery_causes_wound_and_contamination() {
        let session = start_butcher_session(Entity::from_raw(1), Entity::from_raw(2), None);

        assert_eq!(
            resolve_butcher_session(&session),
            ButcherOutcome {
                drop: None,
                wound: true,
                contamination: true,
            }
        );

        let mut wounds = Wounds::default();
        let mut contamination = Contamination::default();
        apply_bare_hand_butcher_hazard(&mut wounds, &mut contamination, 77);

        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].kind, WoundKind::Cut);
        assert_eq!(contamination.entries.len(), 1);
        assert_eq!(contamination.entries[0].amount, 0.4);
        assert_eq!(contamination.entries[0].introduced_at, 77);
    }
}
