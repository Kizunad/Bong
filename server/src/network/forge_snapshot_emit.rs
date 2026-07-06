//! 玩家加入时一次性推送 forge 快照（plan-forge-v1 §4 vertical slice）。
//!
//! 从真实 ECS `BlueprintRegistry` / `WeaponForgeStation` / `ForgeSessions`
//! 读取数据构建 snapshot（非 mock）。

#![allow(dead_code)]

use valence::prelude::{Added, Client, Entity, Query, Res, Username, With};

use crate::forge::blueprint::{Blueprint, BlueprintRegistry, StepSpec};
use crate::forge::learned::LearnedBlueprints;
use crate::forge::session::{ForgeSession, ForgeSessions, ForgeStep, StepState};
use crate::forge::station::WeaponForgeStation;
use crate::inventory::PlayerInventory;
use crate::network::send_server_data_payload;
use crate::schema::forge::{
    ForgeBlueprintBookDataV1, ForgeBlueprintEntryV1, ForgeSessionDataV1, ForgeStepStateDataV1,
    ForgeStepV1, WeaponForgeStationDataV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::skill::components::SkillSet;

type JoinedClientQueryItem<'a> = (Entity, &'a mut Client, &'a Username);

pub fn emit_join_forge_snapshots(
    #[allow(unused)] mut joined_clients: Query<
        JoinedClientQueryItem<'_>,
        (With<Client>, Added<PlayerInventory>),
    >,
    _registry: Res<BlueprintRegistry>,
    _stations: Query<&WeaponForgeStation>,
    _sessions: Res<ForgeSessions>,
    _learned_q: Query<&LearnedBlueprints>,
    _caster_names: Query<&Username>,
    _skill_q: Query<&SkillSet>,
) {
    // join hydration placeholder — real snapshots sent via send_forge_snapshots_to_player
    // when the player opens the forge screen.
}

/// Send forge snapshots for a specific player when they open a forge screen.
pub fn send_forge_snapshots_to_player(
    client: &mut Client,
    station: &WeaponForgeStation,
    owner_name: &str,
    session: Option<(&ForgeSession, &str)>, // (session, blueprint_name)
    learned: Option<(&LearnedBlueprints, &BlueprintRegistry)>,
) {
    // ── station ──
    {
        let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeStation(Box::new(
            build_station_data(station, owner_name),
        )));
        let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload)
        else {
            return;
        };
        send_server_data_payload(client, bytes.as_slice());
    }

    // ── session ──
    if let Some((session, bp_name)) = session {
        let blueprint = learned.and_then(|(_, registry)| registry.get(session.blueprint.as_str()));
        let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeSession(Box::new(
            build_session_data(session, bp_name, blueprint),
        )));
        let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload)
        else {
            return;
        };
        send_server_data_payload(client, bytes.as_slice());
    }

    // ── blueprint book ──
    if let Some((lb, registry)) = learned {
        let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeBlueprintBook(Box::new(
            build_blueprint_book(lb, registry),
        )));
        let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload)
        else {
            return;
        };
        send_server_data_payload(client, bytes.as_slice());
    }
}

/// 锻造结算后推 outcome payload 给对应 player。
pub fn send_forge_outcome_to_player(
    client: &mut Client,
    outcome: &crate::forge::events::ForgeOutcomeEvent,
    flawed_path: bool,
) {
    use crate::schema::forge::{ForgeOutcomeBucketV1, ForgeOutcomeDataV1};
    let data = ForgeOutcomeDataV1 {
        session_id: outcome.session.0,
        blueprint_id: outcome.blueprint.clone(),
        bucket: ForgeOutcomeBucketV1::from(outcome.bucket),
        weapon_item: outcome.weapon_item.clone(),
        quality: outcome.quality,
        color: outcome.color,
        side_effects: outcome.side_effects.clone(),
        achieved_tier: outcome.achieved_tier as u32,
        flawed_path,
    };
    let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeOutcome(Box::new(data)));
    let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
        return;
    };
    send_server_data_payload(client, bytes.as_slice());
}

fn build_station_data(station: &WeaponForgeStation, owner_name: &str) -> WeaponForgeStationDataV1 {
    WeaponForgeStationDataV1 {
        station_id: format!("forge_station_{}", owner_name),
        tier: station.tier,
        integrity: station.integrity,
        owner_name: owner_name.to_string(),
        has_session: station.session.is_some(),
    }
}

fn build_session_data(
    session: &ForgeSession,
    bp_name: &str,
    blueprint: Option<&Blueprint>,
) -> ForgeSessionDataV1 {
    ForgeSessionDataV1 {
        session_id: session.id.0,
        blueprint_id: session.blueprint.clone(),
        blueprint_name: bp_name.to_string(),
        active: !session.is_done(),
        current_step: forge_step_to_v1(session.current_step),
        step_index: session.step_index as u32,
        achieved_tier: session.achieved_tier as u32,
        step_state: build_step_state(session, blueprint),
    }
}

fn forge_step_to_v1(step: ForgeStep) -> ForgeStepV1 {
    match step {
        ForgeStep::Billet => ForgeStepV1::Billet,
        ForgeStep::Tempering => ForgeStepV1::Tempering,
        ForgeStep::Inscription => ForgeStepV1::Inscription,
        ForgeStep::Consecration => ForgeStepV1::Consecration,
        ForgeStep::Done => ForgeStepV1::Done,
    }
}

fn build_step_state(session: &ForgeSession, blueprint: Option<&Blueprint>) -> ForgeStepStateDataV1 {
    let step_spec = blueprint.and_then(|bp| bp.steps.get(session.step_index));
    match &session.step_state {
        StepState::Billet(state) => ForgeStepStateDataV1::Billet {
            materials_in: state
                .materials_in
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            active_carrier: state.active_carrier.clone(),
            resolved_tier_cap: state.resolved_tier_cap as u32,
        },
        StepState::Tempering(state) => ForgeStepStateDataV1::Tempering {
            pattern: match step_spec {
                Some(StepSpec::Tempering { profile }) => profile
                    .pattern
                    .iter()
                    .copied()
                    .map(crate::schema::forge::TemperBeatV1::from)
                    .collect(),
                _ => vec![],
            },
            beat_cursor: state.beat_cursor as u32,
            hits: state.hits,
            misses: state.misses,
            deviation: state.deviation,
            qi_spent: state.qi_spent,
        },
        StepState::Inscription(state) => ForgeStepStateDataV1::Inscription {
            filled_slots: state.filled_slots as u32,
            max_slots: match step_spec {
                Some(StepSpec::Inscription { profile }) => profile.slots as u32,
                _ => state.filled_slots as u32,
            },
            failed: state.failed,
        },
        StepState::Consecration(state) => ForgeStepStateDataV1::Consecration {
            qi_injected: state.qi_injected,
            qi_required: state.qi_required,
            color_imprint: state.color_imprint,
            min_realm: match step_spec {
                Some(StepSpec::Consecration { profile }) => Some(profile.min_realm),
                _ => None,
            },
        },
        StepState::None => ForgeStepStateDataV1::None,
    }
}

fn build_blueprint_book(
    learned: &LearnedBlueprints,
    registry: &BlueprintRegistry,
) -> ForgeBlueprintBookDataV1 {
    let entries: Vec<ForgeBlueprintEntryV1> = learned
        .ids
        .iter()
        .filter_map(|id| {
            registry.get(id).map(|bp| ForgeBlueprintEntryV1 {
                id: bp.id.clone(),
                display_name: bp.name.clone(),
                tier_cap: bp.tier_cap,
                step_count: bp.steps.len() as u32,
            })
        })
        .collect();
    ForgeBlueprintBookDataV1 {
        learned: entries,
        current_index: learned.current_index as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::forge::blueprint::StepKind;
    use crate::forge::session::{
        ConsecrationState, ForgeSessionId, InscriptionState, TemperingState,
    };
    use valence::prelude::Entity;

    fn qing_feng() -> Blueprint {
        serde_json::from_str(include_str!(
            "../../assets/forge/blueprints/qing_feng_v0.json"
        ))
        .expect("qing_feng_v0 blueprint should parse")
    }

    fn ling_feng() -> Blueprint {
        serde_json::from_str(include_str!(
            "../../assets/forge/blueprints/ling_feng_v0.json"
        ))
        .expect("ling_feng_v0 blueprint should parse")
    }

    fn session_at(blueprint: &Blueprint, kind: StepKind, state: StepState) -> ForgeSession {
        let step_index = blueprint
            .step_index(kind)
            .expect("blueprint should contain requested step");
        let mut session = ForgeSession::new(
            ForgeSessionId(7),
            blueprint.id.clone(),
            Entity::from_raw(1),
            Entity::from_raw(2),
        );
        session.step_index = step_index;
        session.current_step = ForgeStep::from_kind(kind);
        session.step_state = state;
        session
    }

    #[test]
    fn tempering_snapshot_includes_blueprint_pattern() {
        let blueprint = qing_feng();
        let session = session_at(
            &blueprint,
            StepKind::Tempering,
            StepState::Tempering(TemperingState {
                beat_cursor: 1,
                hits: 1,
                ..Default::default()
            }),
        );

        let data = build_session_data(&session, blueprint.name.as_str(), Some(&blueprint));

        match data.step_state {
            ForgeStepStateDataV1::Tempering {
                pattern,
                beat_cursor,
                ..
            } => {
                assert_eq!(beat_cursor, 1);
                assert_eq!(pattern.len(), 10);
                assert_eq!(
                    &pattern[0..3],
                    &[
                        crate::schema::forge::TemperBeatV1::Light,
                        crate::schema::forge::TemperBeatV1::Light,
                        crate::schema::forge::TemperBeatV1::Heavy,
                    ]
                );
            }
            other => panic!("expected tempering state, got {other:?}"),
        }
    }

    #[test]
    fn inscription_snapshot_uses_blueprint_max_slots() {
        let blueprint = ling_feng();
        let session = session_at(
            &blueprint,
            StepKind::Inscription,
            StepState::Inscription(InscriptionState {
                scrolls_in: vec!["frost_edge".to_string()],
                filled_slots: 1,
                failed: false,
            }),
        );

        let data = build_session_data(&session, blueprint.name.as_str(), Some(&blueprint));

        match data.step_state {
            ForgeStepStateDataV1::Inscription {
                filled_slots,
                max_slots,
                failed,
            } => {
                assert_eq!(filled_slots, 1);
                assert_eq!(max_slots, 2);
                assert!(!failed);
            }
            other => panic!("expected inscription state, got {other:?}"),
        }
    }

    #[test]
    fn consecration_snapshot_includes_blueprint_min_realm() {
        let blueprint = ling_feng();
        let session = session_at(
            &blueprint,
            StepKind::Consecration,
            StepState::Consecration(ConsecrationState {
                qi_injected: 12.5,
                qi_required: 80.0,
                color_imprint: None,
            }),
        );

        let data = build_session_data(&session, blueprint.name.as_str(), Some(&blueprint));

        match data.step_state {
            ForgeStepStateDataV1::Consecration {
                qi_injected,
                qi_required,
                min_realm,
                ..
            } => {
                assert!((qi_injected - 12.5).abs() < f64::EPSILON);
                assert!((qi_required - 80.0).abs() < f64::EPSILON);
                assert_eq!(min_realm, Some(Realm::Spirit));
            }
            other => panic!("expected consecration state, got {other:?}"),
        }
    }
}
