//! 玩家加入时一次性推送 forge 快照（plan-forge-v1 §4 vertical slice）。
//!
//! 从真实 ECS `BlueprintRegistry` / `WeaponForgeStation` / `ForgeSessions`
//! 读取数据构建 snapshot（非 mock）。

#![allow(dead_code)]

use valence::prelude::{Added, Client, Entity, Query, Res, Username, With};

use crate::forge::blueprint::{BlueprintRegistry};
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
    #[allow(unused)] mut joined_clients: Query<JoinedClientQueryItem<'_>, (With<Client>, Added<PlayerInventory>)>,
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
        let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
            return;
        };
        send_server_data_payload(client, bytes.as_slice());
    }

    // ── session ──
    if let Some((session, bp_name)) = session {
        let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeSession(Box::new(
            build_session_data(session, bp_name),
        )));
        let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
            return;
        };
        send_server_data_payload(client, bytes.as_slice());
    }

    // ── blueprint book ──
    if let Some((lb, registry)) = learned {
        let payload = ServerDataV1::new(ServerDataPayloadV1::ForgeBlueprintBook(Box::new(
            build_blueprint_book(lb, registry),
        )));
        let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
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

fn build_session_data(session: &ForgeSession, bp_name: &str) -> ForgeSessionDataV1 {
    ForgeSessionDataV1 {
        session_id: session.id.0,
        blueprint_id: session.blueprint.clone(),
        blueprint_name: bp_name.to_string(),
        active: !session.is_done(),
        current_step: forge_step_to_v1(session.current_step),
        step_index: session.step_index as u32,
        achieved_tier: session.achieved_tier as u32,
        step_state: build_step_state(session),
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

fn build_step_state(session: &ForgeSession) -> ForgeStepStateDataV1 {
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
        StepState::Tempering(state) => {
            // Pattern is not stored in session state; filled by incremental updates.
            ForgeStepStateDataV1::Tempering {
                pattern: vec![],
                beat_cursor: state.beat_cursor as u32,
                hits: state.hits,
                misses: state.misses,
                deviation: state.deviation,
                qi_spent: state.qi_spent,
            }
        },
        StepState::Inscription(state) => ForgeStepStateDataV1::Inscription {
            filled_slots: state.filled_slots as u32,
            max_slots: state.filled_slots as u32,
            failed: state.failed,
        },
        StepState::Consecration(state) => ForgeStepStateDataV1::Consecration {
            qi_injected: state.qi_injected,
            qi_required: state.qi_required,
            color_imprint: state.color_imprint,
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
