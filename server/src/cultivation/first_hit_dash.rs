//! plan-onboarding-loop-v1 P1.2 — 首战自学闪避。
//!
//! 当玩家被 CombatEvent 或 RatBiteEvent 攻击（首次受伤）时，如果不会
//! `movement.dash`，自动学会闪身步。
//!
//! 系统调度在 `CombatSystemSet::Emit`（读 CombatEvent resolve 结果之后）。

use valence::prelude::{Client, Entity, EventReader, EventWriter, Query, ResMut, With};

use crate::combat::events::CombatEvent;
use crate::combat::rat_bite::RatBiteEvent;
use crate::cultivation::components::{Cultivation, MeridianSystem};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::technique_scroll::{
    learn_technique_if_allowed, LearnSource, ScrollReadOutcome, TechniqueLearnedEvent,
};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;

use valence::prelude::Username;

const DASH_SKILL_ID: &str = "movement.dash";
const DASH_INSIGHT_NARRATION: &str = "皮肉记住了。比脑子快。";

#[allow(clippy::type_complexity)]
pub fn first_hit_dash_insight(
    mut combat_events: EventReader<CombatEvent>,
    mut rat_events: EventReader<RatBiteEvent>,
    mut players: Query<
        (
            &mut KnownTechniques,
            &Cultivation,
            &MeridianSystem,
            Option<&MeridianSeveredPermanent>,
            &Username,
        ),
        With<Client>,
    >,
    mut learned_events: EventWriter<TechniqueLearnedEvent>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    // Collect player entities that were damaged
    let mut damaged_players: Vec<Entity> = Vec::new();

    for event in combat_events.read() {
        // Only care about events where target is a player and damage > 0
        if (event.damage > 0.0 || event.physical_damage > 0.0)
            && players.get(event.target).is_ok()
            && !damaged_players.contains(&event.target)
        {
            damaged_players.push(event.target);
        }
    }

    for event in rat_events.read() {
        // RatBiteEvent always deals qi_steal damage to target
        if players.get(event.target).is_ok() && !damaged_players.contains(&event.target) {
            damaged_players.push(event.target);
        }
    }

    for entity in damaged_players {
        let Ok((mut known, cultivation, meridians, severed, username)) = players.get_mut(entity)
        else {
            continue;
        };

        // Already knows dash — skip
        if known.entries.iter().any(|e| e.id == DASH_SKILL_ID) {
            continue;
        }

        let outcome = learn_technique_if_allowed(
            &mut known,
            cultivation,
            meridians,
            severed,
            DASH_SKILL_ID,
            0.0,
        );

        if outcome == ScrollReadOutcome::Learned {
            learned_events.send(TechniqueLearnedEvent {
                player: entity,
                technique_id: DASH_SKILL_ID.to_string(),
                source: LearnSource::CombatInsight,
            });

            if let Some(ref mut narr) = narrations {
                narr.push_player(
                    username.0.as_str(),
                    DASH_INSIGHT_NARRATION,
                    NarrationStyle::Perception,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{BodyPart, WoundKind};
    use crate::combat::events::AttackSource;
    use crate::cultivation::components::Realm;
    use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
    use valence::prelude::{App, Events, Update};
    use valence::testing::create_mock_client;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<RatBiteEvent>();
        app.add_event::<TechniqueLearnedEvent>();
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, first_hit_dash_insight);
        app
    }

    fn spawn_player(app: &mut App, known: KnownTechniques) -> Entity {
        let (client_bundle, _helper) = create_mock_client("TestPlayer");
        app.world_mut()
            .spawn((
                client_bundle,
                known,
                Cultivation {
                    realm: Realm::Induce,
                    ..Default::default()
                },
                MeridianSystem::default(),
            ))
            .id()
    }

    fn combat_event(attacker: Entity, target: Entity) -> CombatEvent {
        CombatEvent {
            attacker,
            target,
            resolved_at_tick: 100,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::default(),
            debug_command: false,
            physical_damage: 5.0,
            damage: 0.0,
            contam_delta: 0.0,
            description: "test hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        }
    }

    #[test]
    fn first_hit_learns_dash_on_combat_event() {
        let mut app = setup_app();
        let attacker = app.world_mut().spawn_empty().id();
        let player = spawn_player(&mut app, KnownTechniques::default());

        app.world_mut().send_event(combat_event(attacker, player));
        app.update();

        let known = app.world().get::<KnownTechniques>(player).unwrap();
        assert!(
            known.entries.iter().any(|e| e.id == "movement.dash"),
            "player should have learned movement.dash after being hit; known: {:?}",
            known.entries.iter().map(|e| &e.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn first_hit_learns_dash_on_rat_bite() {
        let mut app = setup_app();
        let rat = app.world_mut().spawn_empty().id();
        let player = spawn_player(&mut app, KnownTechniques::default());

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target: player,
            qi_steal: 3,
        });
        app.update();

        let known = app.world().get::<KnownTechniques>(player).unwrap();
        assert!(
            known.entries.iter().any(|e| e.id == "movement.dash"),
            "player should have learned movement.dash after rat bite; known: {:?}",
            known.entries.iter().map(|e| &e.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn first_hit_idempotent_if_dash_already_known() {
        let mut app = setup_app();
        let attacker = app.world_mut().spawn_empty().id();
        let player = spawn_player(
            &mut app,
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: "movement.dash".to_string(),
                    proficiency: 0.5,
                    active: true,
                }],
            },
        );

        app.world_mut().send_event(combat_event(attacker, player));
        app.update();

        let known = app.world().get::<KnownTechniques>(player).unwrap();
        assert_eq!(
            known.entries.len(),
            1,
            "should not have added duplicate movement.dash; entries: {:?}",
            known.entries
        );
        assert_eq!(
            known.entries[0].proficiency, 0.5,
            "original proficiency should be preserved"
        );

        // No TechniqueLearnedEvent should have been emitted
        let events = app.world().resource::<Events<TechniqueLearnedEvent>>();
        assert_eq!(
            events.len(),
            0,
            "no TechniqueLearnedEvent should be emitted when dash is already known"
        );
    }

    #[test]
    fn first_hit_ignores_zero_damage() {
        let mut app = setup_app();
        let attacker = app.world_mut().spawn_empty().id();
        let player = spawn_player(&mut app, KnownTechniques::default());

        let mut e = combat_event(attacker, player);
        e.damage = 0.0;
        e.physical_damage = 0.0;
        app.world_mut().send_event(e);
        app.update();

        let known = app.world().get::<KnownTechniques>(player).unwrap();
        assert!(
            !known.entries.iter().any(|t| t.id == "movement.dash"),
            "zero-damage hit should not trigger dash learning; entries: {:?}",
            known.entries.iter().map(|t| &t.id).collect::<Vec<_>>()
        );
        let events = app.world().resource::<Events<TechniqueLearnedEvent>>();
        assert_eq!(
            events.len(),
            0,
            "no TechniqueLearnedEvent should be emitted for zero-damage hit"
        );
        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            0,
            "no narration should be emitted for zero-damage hit"
        );
    }

    #[test]
    fn first_hit_narration_emitted() {
        let mut app = setup_app();
        let attacker = app.world_mut().spawn_empty().id();
        let player = spawn_player(&mut app, KnownTechniques::default());

        app.world_mut().send_event(combat_event(attacker, player));
        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            1,
            "exactly one narration should be emitted; got {}",
            narrations.len()
        );
        assert_eq!(
            narrations[0].text, DASH_INSIGHT_NARRATION,
            "narration text should match"
        );
        assert_eq!(
            narrations[0].style,
            NarrationStyle::Perception,
            "narration style should be Perception"
        );
    }
}
