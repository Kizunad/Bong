use valence::prelude::{bevy_ecs, Entity, Event};

use super::components::ButcherSession;
use super::ToolKind;
use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};

#[derive(Debug, Clone, Event)]
pub struct ButcherRequest {
    pub player: Entity,
    pub corpse: Entity,
    pub tool: Option<ToolKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButcherOutcome {
    pub drop_item: Option<&'static str>,
    pub wound: bool,
    pub contamination: bool,
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
            drop_item: Some("yi_beast_bone"),
            wound: false,
            contamination: false,
        },
        Some(ToolKind::CaiYaoDao) => ButcherOutcome {
            drop_item: Some("raw_beast_meat"),
            wound: false,
            contamination: false,
        },
        _ => ButcherOutcome {
            drop_item: None,
            wound: true,
            contamination: true,
        },
    }
}

pub fn apply_bare_hand_butcher_wound(
    wounds: &mut Wounds,
    now_tick: u64,
    inflicted_by: &'static str,
) {
    wounds.entries.push(Wound {
        location: BodyPart::ArmR,
        kind: WoundKind::Cut,
        severity: 0.35,
        bleeding_per_sec: 0.0,
        created_at_tick: now_tick,
        inflicted_by: Some(inflicted_by.to_string()),
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
                drop_item: Some("yi_beast_bone"),
                wound: false,
                contamination: false,
            }
        );
    }

    #[test]
    fn bare_hand_butchery_causes_wound_and_contamination() {
        let session = start_butcher_session(Entity::from_raw(1), Entity::from_raw(2), None);

        assert_eq!(
            resolve_butcher_session(&session),
            ButcherOutcome {
                drop_item: None,
                wound: true,
                contamination: true,
            }
        );
    }
}
