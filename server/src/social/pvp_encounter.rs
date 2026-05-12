//! PvP encounter choreography hooks.
//!
//! This module does not change combat rules. It records the before/after social facts that let
//! anonymous player encounters become later intel, renown, and jianghu narration material.

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Client, Event, EventReader, EventWriter, IntoSystemConfigs, Query, Res, Update,
    With,
};

use crate::combat::components::Lifecycle;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::identity::PlayerIdentities;
use crate::persistence::{identity as identity_db, PersistenceSettings};
use crate::schema::social::{RelationshipKindV1, RenownTagV1};
use crate::social::events::{SocialRelationshipEvent, SocialRenownDeltaEvent};

pub const BETRAYAL_REPUTATION_DELTA: i32 = 30;

type EncounterPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Lifecycle,
        Option<&'static mut LifeRecord>,
        Option<&'static mut PlayerIdentities>,
    ),
    With<Client>,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterPhase {
    FarAssessment,
    MidProbe,
    CloseContact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterOutcome {
    Bypass,
    PeacefulSeparation,
    ProbeFight,
    DeathFight,
    TemporaryCooperation,
    Betrayal,
}

impl EncounterOutcome {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Bypass => "bypass",
            Self::PeacefulSeparation => "peaceful_separation",
            Self::ProbeFight => "probe_fight",
            Self::DeathFight => "death_fight",
            Self::TemporaryCooperation => "temporary_cooperation",
            Self::Betrayal => "betrayal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterContext {
    Wilderness,
    ResourcePoint,
    TsyEntrance,
    TsyShallow,
    TsyMid,
    TsyDeep,
    TsyExtract,
    TsyRaceOut,
}

impl EncounterContext {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Wilderness => "wilderness",
            Self::ResourcePoint => "resource_point",
            Self::TsyEntrance => "tsy_entrance",
            Self::TsyShallow => "tsy_shallow",
            Self::TsyMid => "tsy_mid",
            Self::TsyDeep => "tsy_deep",
            Self::TsyExtract => "tsy_extract",
            Self::TsyRaceOut => "tsy_race_out",
        }
    }
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct PvpEncounterEvent {
    pub left: String,
    pub right: String,
    pub outcome: EncounterOutcome,
    pub context: EncounterContext,
    pub zone: String,
    pub tick: u64,
    #[serde(default)]
    pub observed_style: Option<String>,
    #[serde(default)]
    pub appearance_hint: Option<String>,
    #[serde(default)]
    pub qi_color_hint: Option<String>,
    #[serde(default)]
    pub betrayer: Option<String>,
    #[serde(default)]
    pub npc_witnessed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // P0 design weights are consumed by tests and future encounter telemetry.
pub struct EncounterDecisionWeights {
    pub gear: u8,
    pub own_condition: u8,
    pub resource_pressure: u8,
    pub carried_value: u8,
    pub tsy_pressure: u8,
}

impl EncounterDecisionWeights {
    #[allow(dead_code)] // See EncounterDecisionWeights.
    pub const fn baseline_for(context: EncounterContext) -> Self {
        match context {
            EncounterContext::TsyEntrance
            | EncounterContext::TsyShallow
            | EncounterContext::TsyMid
            | EncounterContext::TsyDeep
            | EncounterContext::TsyExtract
            | EncounterContext::TsyRaceOut => Self {
                gear: 3,
                own_condition: 5,
                resource_pressure: 4,
                carried_value: 4,
                tsy_pressure: 5,
            },
            EncounterContext::ResourcePoint => Self {
                gear: 3,
                own_condition: 5,
                resource_pressure: 4,
                carried_value: 4,
                tsy_pressure: 0,
            },
            EncounterContext::Wilderness => Self {
                gear: 3,
                own_condition: 5,
                resource_pressure: 1,
                carried_value: 4,
                tsy_pressure: 0,
            },
        }
    }
}

#[allow(dead_code)] // P0 distance bands are a documented design contract for client/server telemetry.
pub fn phase_for_distance(distance_blocks: f64) -> EncounterPhase {
    if distance_blocks <= 8.0 {
        EncounterPhase::CloseContact
    } else if distance_blocks <= 20.0 {
        EncounterPhase::MidProbe
    } else {
        EncounterPhase::FarAssessment
    }
}

pub fn register(app: &mut App) {
    app.add_event::<PvpEncounterEvent>().add_systems(
        Update,
        handle_pvp_encounter_events.after(crate::combat::CombatSystemSet::Resolve),
    );
}

pub fn handle_pvp_encounter_events(
    mut events: EventReader<PvpEncounterEvent>,
    mut players: EncounterPlayerQuery<'_, '_>,
    persistence: Option<Res<PersistenceSettings>>,
    mut relationships: EventWriter<SocialRelationshipEvent>,
    mut renown_deltas: EventWriter<SocialRenownDeltaEvent>,
) {
    for event in events.read() {
        if event.left == event.right {
            continue;
        }
        let validated_betrayer = validated_betrayer(event);

        record_life_entries(
            event,
            validated_betrayer,
            &mut players,
            persistence.as_deref(),
        );
        emit_social_edges(
            event,
            validated_betrayer,
            &mut relationships,
            &mut renown_deltas,
        );
    }
}

fn record_life_entries(
    event: &PvpEncounterEvent,
    validated_betrayer: Option<&str>,
    players: &mut EncounterPlayerQuery<'_, '_>,
    persistence: Option<&PersistenceSettings>,
) {
    for (lifecycle, life_record, identities) in players.iter_mut() {
        let Some(counterparty_id) = counterparty(event, lifecycle.character_id.as_str()) else {
            continue;
        };

        if let Some(mut life_record) = life_record {
            life_record.push(encounter_entry(event, counterparty_id));
            if let Some(betrayer) = validated_betrayer {
                if betrayer != lifecycle.character_id {
                    life_record.push(BiographyEntry::PvpBetrayal {
                        betrayer_id: betrayer.to_string(),
                        victim_id: lifecycle.character_id.clone(),
                        scene: event.context.wire_name().to_string(),
                        npc_witnessed: event.npc_witnessed,
                        tick: event.tick,
                    });
                }
            }
        }

        if event.npc_witnessed
            && validated_betrayer.is_some_and(|betrayer| betrayer == lifecycle.character_id)
        {
            if let Some(mut identities) = identities {
                apply_betrayal_reputation_to_active_identity(&mut identities, event.tick);
                if let Some(persistence) = persistence {
                    if let Err(error) = identity_db::save_player_identities(
                        persistence,
                        lifecycle.character_id.as_str(),
                        &identities,
                    ) {
                        tracing::warn!(
                            ?error,
                            char_id = lifecycle.character_id.as_str(),
                            "[bong][pvp-encounter] failed to persist betrayal reputation"
                        );
                    }
                }
            }
        }
    }
}

fn encounter_entry(event: &PvpEncounterEvent, counterparty_id: &str) -> BiographyEntry {
    BiographyEntry::PvpEncounter {
        counterparty_id: counterparty_id.to_string(),
        outcome: event.outcome.wire_name().to_string(),
        zone: event.zone.clone(),
        context: event.context.wire_name().to_string(),
        observed_style: event.observed_style.clone(),
        appearance_hint: event.appearance_hint.clone(),
        qi_color_hint: event.qi_color_hint.clone(),
        tick: event.tick,
    }
}

fn apply_betrayal_reputation_to_active_identity(identities: &mut PlayerIdentities, tick: u64) {
    let Some(active) = identities.active_mut() else {
        return;
    };
    active
        .renown
        .apply_delta(0, BETRAYAL_REPUTATION_DELTA, vec![betrayal_tag(tick)]);
}

fn emit_social_edges(
    event: &PvpEncounterEvent,
    validated_betrayer: Option<&str>,
    relationships: &mut EventWriter<SocialRelationshipEvent>,
    renown_deltas: &mut EventWriter<SocialRenownDeltaEvent>,
) {
    if matches!(
        event.outcome,
        EncounterOutcome::ProbeFight | EncounterOutcome::DeathFight | EncounterOutcome::Betrayal
    ) {
        relationships.send(SocialRelationshipEvent {
            left: event.left.clone(),
            right: event.right.clone(),
            left_kind: RelationshipKindV1::Feud,
            right_kind: RelationshipKindV1::Feud,
            tick: event.tick,
            metadata: serde_json::json!({
                "cause": event.outcome.wire_name(),
                "place": event.zone,
                "context": event.context.wire_name(),
            }),
        });
    }

    if event.npc_witnessed {
        if let Some(betrayer) = validated_betrayer {
            renown_deltas.send(SocialRenownDeltaEvent {
                char_id: betrayer.to_string(),
                fame_delta: 0,
                notoriety_delta: BETRAYAL_REPUTATION_DELTA,
                tags_added: vec![betrayal_tag(event.tick)],
                tick: event.tick,
                reason: "pvp_betrayal".to_string(),
            });
        }
    }
}

fn betrayal_tag(tick: u64) -> RenownTagV1 {
    RenownTagV1 {
        tag: "背信者".to_string(),
        weight: f64::from(BETRAYAL_REPUTATION_DELTA),
        last_seen_tick: tick,
        permanent: false,
    }
}

fn counterparty<'a>(event: &'a PvpEncounterEvent, char_id: &str) -> Option<&'a str> {
    if event.left == char_id {
        Some(event.right.as_str())
    } else if event.right == char_id {
        Some(event.left.as_str())
    } else {
        None
    }
}

fn validated_betrayer(event: &PvpEncounterEvent) -> Option<&str> {
    if event.outcome != EncounterOutcome::Betrayal {
        return None;
    }
    event
        .betrayer
        .as_deref()
        .filter(|betrayer| *betrayer == event.left || *betrayer == event.right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::Lifecycle;
    use crate::cultivation::life_record::LifeRecord;
    use crate::identity::{IdentityId, IdentityProfile, PlayerIdentities};
    use crate::social::components::Renown;
    use valence::prelude::{App, Entity, Events};
    use valence::testing::create_mock_client;

    fn spawn_player(app: &mut App, username: &str, char_id: &str) -> Entity {
        let (bundle, _helper) = create_mock_client(username);
        let entity = app.world_mut().spawn(bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Lifecycle {
                character_id: char_id.to_string(),
                ..Default::default()
            },
            LifeRecord::new(char_id),
            PlayerIdentities::with_default(username, 0),
            Renown::default(),
        ));
        entity
    }

    fn betrayal_event() -> PvpEncounterEvent {
        PvpEncounterEvent {
            left: "char:alice".to_string(),
            right: "char:bob".to_string(),
            outcome: EncounterOutcome::Betrayal,
            context: EncounterContext::TsyExtract,
            zone: "blood_valley".to_string(),
            tick: 77,
            observed_style: Some("woliu".to_string()),
            appearance_hint: Some("右手持骨刺".to_string()),
            qi_color_hint: Some("青白".to_string()),
            betrayer: Some("char:bob".to_string()),
            npc_witnessed: true,
        }
    }

    fn app_with_pvp_encounter_system() -> App {
        let mut app = App::new();
        app.add_event::<PvpEncounterEvent>();
        app.add_event::<SocialRelationshipEvent>();
        app.add_event::<SocialRenownDeltaEvent>();
        app.add_systems(Update, handle_pvp_encounter_events);
        app
    }

    fn app_with_pvp_and_social_renown_systems() -> App {
        let mut app = App::new();
        app.add_event::<PvpEncounterEvent>();
        app.add_event::<SocialRelationshipEvent>();
        app.add_event::<SocialRenownDeltaEvent>();
        app.add_systems(
            Update,
            (
                handle_pvp_encounter_events,
                crate::social::apply_social_renown_deltas.after(handle_pvp_encounter_events),
            ),
        );
        app
    }

    #[test]
    fn encounter_phase_follows_plan_distance_bands() {
        assert_eq!(phase_for_distance(50.0), EncounterPhase::FarAssessment);
        assert_eq!(phase_for_distance(20.0), EncounterPhase::MidProbe);
        assert_eq!(phase_for_distance(8.0), EncounterPhase::CloseContact);
    }

    #[test]
    fn encounter_phase_boundary_precision() {
        assert_eq!(phase_for_distance(8.0), EncounterPhase::CloseContact);
        assert_eq!(phase_for_distance(8.01), EncounterPhase::MidProbe);
        assert_eq!(phase_for_distance(20.0), EncounterPhase::MidProbe);
        assert_eq!(phase_for_distance(20.01), EncounterPhase::FarAssessment);
    }

    #[test]
    fn encounter_enums_serialize_to_wire_names() {
        for variant in [
            EncounterPhase::FarAssessment,
            EncounterPhase::MidProbe,
            EncounterPhase::CloseContact,
        ] {
            let serialized =
                serde_json::to_string(&variant).expect("encounter phase should serialize");
            assert_eq!(serialized, format!("\"{}\"", phase_wire_name(variant)));
            let decoded: EncounterPhase =
                serde_json::from_str(&serialized).expect("encounter phase should deserialize");
            assert_eq!(decoded, variant);
        }

        for variant in [
            EncounterOutcome::Bypass,
            EncounterOutcome::PeacefulSeparation,
            EncounterOutcome::ProbeFight,
            EncounterOutcome::DeathFight,
            EncounterOutcome::TemporaryCooperation,
            EncounterOutcome::Betrayal,
        ] {
            let serialized =
                serde_json::to_string(&variant).expect("encounter outcome should serialize");
            assert_eq!(serialized, format!("\"{}\"", variant.wire_name()));
            let decoded: EncounterOutcome =
                serde_json::from_str(&serialized).expect("encounter outcome should deserialize");
            assert_eq!(decoded, variant);
        }

        for variant in [
            EncounterContext::Wilderness,
            EncounterContext::ResourcePoint,
            EncounterContext::TsyEntrance,
            EncounterContext::TsyShallow,
            EncounterContext::TsyMid,
            EncounterContext::TsyDeep,
            EncounterContext::TsyExtract,
            EncounterContext::TsyRaceOut,
        ] {
            let serialized =
                serde_json::to_string(&variant).expect("encounter context should serialize");
            assert_eq!(serialized, format!("\"{}\"", variant.wire_name()));
            let decoded: EncounterContext =
                serde_json::from_str(&serialized).expect("encounter context should deserialize");
            assert_eq!(decoded, variant);
        }
    }

    #[test]
    fn encounter_enums_reject_invalid_wire_values() {
        for bad_json in ["\"closeContact\"", "\"close-contact\"", "\"unrelated\""] {
            assert!(
                serde_json::from_str::<EncounterPhase>(bad_json).is_err(),
                "expected EncounterPhase to reject non-snake_case or unknown wire value {bad_json}"
            );
        }
        for bad_json in ["\"peacefulSeparation\"", "\"death-fight\"", "\"unrelated\""] {
            assert!(
                serde_json::from_str::<EncounterOutcome>(bad_json).is_err(),
                "expected EncounterOutcome to reject non-snake_case or unknown wire value {bad_json}"
            );
        }
        for bad_json in ["\"tsyExtract\"", "\"tsy-extract\"", "\"unrelated\""] {
            assert!(
                serde_json::from_str::<EncounterContext>(bad_json).is_err(),
                "expected EncounterContext to reject non-snake_case or unknown wire value {bad_json}"
            );
        }
    }

    const fn phase_wire_name(phase: EncounterPhase) -> &'static str {
        match phase {
            EncounterPhase::FarAssessment => "far_assessment",
            EncounterPhase::MidProbe => "mid_probe",
            EncounterPhase::CloseContact => "close_contact",
        }
    }

    #[test]
    fn betrayal_reputation_impact() {
        let mut app = app_with_pvp_encounter_system();
        spawn_player(&mut app, "Alice", "char:alice");
        let bob = spawn_player(&mut app, "Bob", "char:bob");

        app.world_mut().send_event(betrayal_event());
        app.update();

        let identities = app.world().get::<PlayerIdentities>(bob).unwrap();
        let active = identities.active().unwrap();
        assert_eq!(active.renown.notoriety, BETRAYAL_REPUTATION_DELTA);
        assert_eq!(active.renown.tags[0].tag, "背信者");

        let renown_events = app.world().resource::<Events<SocialRenownDeltaEvent>>();
        let emitted = renown_events
            .iter_current_update_events()
            .next()
            .expect("betrayal should emit a social renown delta");
        assert_eq!(emitted.reason, "pvp_betrayal");
        assert_eq!(emitted.char_id, "char:bob");
    }

    #[test]
    fn non_betrayal_outcome_never_applies_betrayal_renown() {
        let mut app = app_with_pvp_encounter_system();
        let alice = spawn_player(&mut app, "Alice", "char:alice");
        let bob = spawn_player(&mut app, "Bob", "char:bob");
        let mut event = betrayal_event();
        event.outcome = EncounterOutcome::TemporaryCooperation;

        app.world_mut().send_event(event);
        app.update();

        let identities = app.world().get::<PlayerIdentities>(bob).unwrap();
        assert_eq!(
            identities.active().unwrap().renown.notoriety,
            0,
            "non-betrayal encounter should not write betrayal notoriety"
        );
        let renown_events = app.world().resource::<Events<SocialRenownDeltaEvent>>();
        assert!(
            renown_events.iter_current_update_events().next().is_none(),
            "non-betrayal encounter should not emit pvp_betrayal renown event"
        );
        let record = app.world().get::<LifeRecord>(alice).unwrap();
        assert!(
            record
                .biography
                .iter()
                .all(|entry| !matches!(entry, BiographyEntry::PvpBetrayal { .. })),
            "non-betrayal encounter should not record PvpBetrayal"
        );
    }

    #[test]
    fn outsider_betrayer_is_rejected_before_renown_writes() {
        let mut app = app_with_pvp_encounter_system();
        let alice = spawn_player(&mut app, "Alice", "char:alice");
        let bob = spawn_player(&mut app, "Bob", "char:bob");
        let mut event = betrayal_event();
        event.betrayer = Some("char:outsider".to_string());

        app.world_mut().send_event(event);
        app.update();

        let identities = app.world().get::<PlayerIdentities>(bob).unwrap();
        assert_eq!(
            identities.active().unwrap().renown.notoriety,
            0,
            "outsider betrayer should not write reputation to encounter participants"
        );
        let renown_events = app.world().resource::<Events<SocialRenownDeltaEvent>>();
        assert!(
            renown_events.iter_current_update_events().next().is_none(),
            "outsider betrayer should not emit pvp_betrayal renown event"
        );
        let record = app.world().get::<LifeRecord>(alice).unwrap();
        assert!(
            record
                .biography
                .iter()
                .all(|entry| !matches!(entry, BiographyEntry::PvpBetrayal { .. })),
            "outsider betrayer should not be recorded as a valid betrayal"
        );
    }

    #[test]
    fn unwitnessed_betrayal_does_not_emit_renown_delta() {
        let mut app = app_with_pvp_encounter_system();
        let alice = spawn_player(&mut app, "Alice", "char:alice");
        let bob = spawn_player(&mut app, "Bob", "char:bob");
        let mut event = betrayal_event();
        event.npc_witnessed = false;

        app.world_mut().send_event(event);
        app.update();

        let identities = app.world().get::<PlayerIdentities>(bob).unwrap();
        assert_eq!(
            identities.active().unwrap().renown.notoriety,
            0,
            "unwitnessed betrayal should not write identity notoriety"
        );
        let renown_events = app.world().resource::<Events<SocialRenownDeltaEvent>>();
        assert!(
            renown_events.iter_current_update_events().next().is_none(),
            "unwitnessed betrayal should not emit pvp_betrayal renown event"
        );
        let record = app.world().get::<LifeRecord>(alice).unwrap();
        assert!(
            record.biography.iter().any(|entry| matches!(
                entry,
                BiographyEntry::PvpBetrayal {
                    npc_witnessed: false,
                    ..
                }
            )),
            "unwitnessed betrayal should still record private victim biography"
        );
    }

    #[test]
    fn witnessed_betrayal_full_frame_keeps_identity_delta_single() {
        let mut app = app_with_pvp_and_social_renown_systems();
        spawn_player(&mut app, "Alice", "char:alice");
        let bob = spawn_player(&mut app, "Bob", "char:bob");

        app.world_mut().send_event(betrayal_event());
        app.update();

        let identities = app.world().get::<PlayerIdentities>(bob).unwrap();
        assert_eq!(
            identities.active().unwrap().renown.notoriety,
            BETRAYAL_REPUTATION_DELTA,
            "identity notoriety should be applied once by the pvp encounter handler"
        );
        let social_renown = app.world().get::<Renown>(bob).unwrap();
        assert_eq!(
            social_renown.notoriety, BETRAYAL_REPUTATION_DELTA,
            "social renown delta should apply once through the downstream renown system"
        );
    }

    #[test]
    fn identity_switch_clears_betrayal() {
        let mut identities = PlayerIdentities::with_default("Bob", 0);
        apply_betrayal_reputation_to_active_identity(&mut identities, 10);
        identities
            .identities
            .push(IdentityProfile::new(IdentityId(1), "Bob-alt", 20));
        identities.active_identity_id = IdentityId(1);

        assert_eq!(identities.active().unwrap().renown.notoriety, 0);

        identities.active_identity_id = IdentityId(0);
        assert_eq!(
            identities.active().unwrap().renown.notoriety,
            BETRAYAL_REPUTATION_DELTA
        );
    }

    #[test]
    fn life_record_encounter_entry() {
        let mut app = app_with_pvp_encounter_system();
        let alice = spawn_player(&mut app, "Alice", "char:alice");
        spawn_player(&mut app, "Bob", "char:bob");

        app.world_mut().send_event(betrayal_event());
        app.update();

        let record = app.world().get::<LifeRecord>(alice).unwrap();
        assert!(matches!(
            record.biography.first(),
            Some(BiographyEntry::PvpEncounter {
                counterparty_id,
                outcome,
                zone,
                observed_style: Some(style),
                ..
            }) if counterparty_id == "char:bob"
                && outcome == "betrayal"
                && zone == "blood_valley"
                && style == "woliu"
        ));
        assert!(matches!(
            record.biography.get(1),
            Some(BiographyEntry::PvpBetrayal {
                betrayer_id,
                victim_id,
                scene,
                npc_witnessed: true,
                ..
            }) if betrayer_id == "char:bob" && victim_id == "char:alice" && scene == "tsy_extract"
        ));
    }

    #[test]
    fn pvp_extract_context_has_max_tsy_pressure() {
        let weights = EncounterDecisionWeights::baseline_for(EncounterContext::TsyExtract);
        assert_eq!(weights.own_condition, 5);
        assert_eq!(weights.tsy_pressure, 5);
    }

    #[test]
    fn five_player_encounter_matrix_produces_trackable_outcomes() {
        let realms = ["awaken", "induce", "condense", "solidify", "spirit"];
        let mut encounters = Vec::new();
        for left in 0..realms.len() {
            for right in (left + 1)..realms.len() {
                let context = if left % 2 == 0 {
                    EncounterContext::ResourcePoint
                } else {
                    EncounterContext::TsyExtract
                };
                let outcome = if right == realms.len() - 1 {
                    EncounterOutcome::TemporaryCooperation
                } else if left == 0 && right == 1 {
                    EncounterOutcome::Betrayal
                } else {
                    EncounterOutcome::Bypass
                };
                encounters.push((left, right, context, outcome));
            }
        }

        let trackable = encounters
            .iter()
            .filter(|(_, _, _, outcome)| {
                matches!(
                    outcome,
                    EncounterOutcome::TemporaryCooperation | EncounterOutcome::Betrayal
                )
            })
            .count();

        assert_eq!(encounters.len(), 10);
        assert!(
            trackable >= 2,
            "5 玩家交叉矩阵应至少产生合作/背叛这类可追踪故事"
        );
        assert!(encounters
            .iter()
            .any(|(_, _, context, _)| *context == EncounterContext::TsyExtract));
    }
}
