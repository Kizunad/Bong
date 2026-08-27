//! C2S Forge 请求的编译期 typed dispatch。
//!
//! 顶层 ingress 负责 channel、decode、版本、预算与 live gate；本模块只负责
//! Forge 八个请求的静态提取和领域分发。Forge 业务校验仍复用原有 helper，
//! 同一 `handle_client_request_payloads` 调用内的 projected step 由调用方持有，
//! 不跨 tick 或跨请求生命周期泄漏。

use std::collections::HashMap;

use valence::message::SendMessage;
use valence::prelude::{BlockPos, Client, Commands, Entity, Events, Query, ResMut, Username};

use crate::cultivation::components::Cultivation;
use crate::forge::blueprint::BlueprintRegistry;
use crate::forge::events::{
    ConsecrationInject, InscriptionScrollSubmit, StartForgeRequest, StepAdvance, TemperingHit,
};
use crate::forge::learned::LearnedBlueprints;
use crate::forge::session::{ForgeSessionId, ForgeSessions, ForgeStep};
use crate::forge::station::{PlaceForgeStationRequest, StationTier, WeaponForgeStation};
use crate::forge::steps::next_step_after;
use crate::inventory::{consume_item_instance_once, ItemRegistry, PlayerInventory};
use crate::network::client_request_handler::{
    resync_snapshot, ClientRequestDispatchParams, SkillScrollRequestParams,
};
use crate::network::forge_snapshot_emit;
use crate::player::state::PlayerState;
use crate::schema::client_request::ClientRequestV1;

/// 已通过 schema/version/live gate 的 Forge 请求。
#[derive(Debug, PartialEq)]
pub enum ForgeRequest {
    StationPlace {
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
        station_tier: StationTier,
    },
    InscriptionScroll {
        session_id: u64,
        inscription_id: String,
    },
    TemperingHit {
        session_id: u64,
        beat: String,
        ticks_remaining: u32,
    },
    ConsecrationInject {
        session_id: u64,
        qi_amount: f64,
    },
    StepAdvance {
        session_id: u64,
    },
    LearnBlueprint {
        blueprint_id: String,
    },
    StartSession {
        station_pos: (i32, i32, i32),
        blueprint_id: String,
        materials: Vec<(String, u32)>,
    },
    BlueprintTurnPage {
        delta: i32,
    },
}

/// 从总 C2S enum 提取 Forge 域；非 Forge 请求原样交还顶层 handler。
pub fn try_into_forge_request(request: ClientRequestV1) -> Result<ForgeRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::ForgeStationPlace {
            x,
            y,
            z,
            item_instance_id,
            station_tier,
            ..
        } => Ok(ForgeRequest::StationPlace {
            x,
            y,
            z,
            item_instance_id,
            station_tier,
        }),
        ClientRequestV1::ForgeInscriptionScroll {
            session_id,
            inscription_id,
            ..
        } => Ok(ForgeRequest::InscriptionScroll {
            session_id,
            inscription_id,
        }),
        ClientRequestV1::ForgeTemperingHit {
            session_id,
            beat,
            ticks_remaining,
            ..
        } => Ok(ForgeRequest::TemperingHit {
            session_id,
            beat,
            ticks_remaining,
        }),
        ClientRequestV1::ForgeConsecrationInject {
            session_id,
            qi_amount,
            ..
        } => Ok(ForgeRequest::ConsecrationInject {
            session_id,
            qi_amount,
        }),
        ClientRequestV1::ForgeStepAdvance { session_id, .. } => {
            Ok(ForgeRequest::StepAdvance { session_id })
        }
        ClientRequestV1::ForgeLearnBlueprint { blueprint_id, .. } => {
            Ok(ForgeRequest::LearnBlueprint { blueprint_id })
        }
        ClientRequestV1::ForgeStartSession {
            station_pos,
            blueprint_id,
            materials,
            ..
        } => Ok(ForgeRequest::StartSession {
            station_pos,
            blueprint_id,
            materials,
        }),
        ClientRequestV1::ForgeBlueprintTurnPage { delta, .. } => {
            Ok(ForgeRequest::BlueprintTurnPage { delta })
        }
        request => Err(request),
    }
}

/// 将一个 typed Forge 请求交给原有 Forge helper。
///
/// `pending_forge_steps` 由顶层 handler 在一次 payload batch 中创建并传入。只有
/// `StepAdvance` 已成功发出事件后才写入 next step，因此同 batch 后续三个步骤
/// 读取到的 projected state 与拆分前一致。
#[allow(clippy::too_many_arguments)]
pub fn dispatch_forge_request(
    request: ForgeRequest,
    player: Entity,
    pending_forge_steps: &mut HashMap<(u64, ForgeSessionId), ForgeStep>,
    dispatch: &mut ClientRequestDispatchParams<'_>,
    skill_scroll: &mut SkillScrollRequestParams<'_, '_>,
    commands: &mut Commands,
    clients: &mut Query<(&Username, &mut Client)>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
) {
    match request {
        ForgeRequest::StationPlace {
            x,
            y,
            z,
            item_instance_id,
            station_tier,
        } => {
            tracing::info!(
                "[bong][network][forge] station_place entity={:?} pos=[{x},{y},{z}] instance={item_instance_id} tier={station_tier}",
                player
            );
            if let Some(place_forge_station_tx) = dispatch.place_forge_station_tx.as_deref_mut() {
                place_forge_station_tx.send(PlaceForgeStationRequest {
                    player,
                    pos: BlockPos::new(x, y, z),
                    item_instance_id,
                    station_tier,
                });
            }
        }
        ForgeRequest::InscriptionScroll {
            session_id,
            inscription_id,
        } => {
            let session = ForgeSessionId(session_id);
            let pending_step = pending_forge_steps
                .get(&(player.to_bits(), session))
                .copied();
            handle_forge_inscription_scroll(
                player,
                session_id,
                &inscription_id,
                inventories,
                &skill_scroll.item_registry,
                clients,
                player_states,
                &skill_scroll.cultivations,
                &mut skill_scroll.inscription_scroll_tx,
                skill_scroll.forge_sessions.as_deref(),
                pending_step,
            );
        }
        ForgeRequest::TemperingHit {
            session_id,
            beat,
            ticks_remaining,
        } => {
            let session = ForgeSessionId(session_id);
            let pending_step = pending_forge_steps
                .get(&(player.to_bits(), session))
                .copied();
            handle_forge_tempering_hit(
                player,
                session_id,
                &beat,
                ticks_remaining,
                &mut dispatch.tempering_hit_tx,
                skill_scroll.forge_sessions.as_deref(),
                pending_step,
            );
        }
        ForgeRequest::ConsecrationInject {
            session_id,
            qi_amount,
        } => {
            let session = ForgeSessionId(session_id);
            let pending_step = pending_forge_steps
                .get(&(player.to_bits(), session))
                .copied();
            handle_forge_consecration_inject(
                player,
                session_id,
                qi_amount,
                &mut dispatch.consecration_inject_tx,
                skill_scroll.forge_sessions.as_deref(),
                pending_step,
            );
        }
        ForgeRequest::StepAdvance { session_id } => {
            if let Some((session, next_step)) = handle_forge_step_advance(
                player,
                session_id,
                &mut dispatch.step_advance_tx,
                skill_scroll.forge_sessions.as_deref(),
                skill_scroll.blueprint_registry.as_deref(),
            ) {
                pending_forge_steps.insert((player.to_bits(), session), next_step);
            }
        }
        ForgeRequest::LearnBlueprint { blueprint_id } => {
            handle_forge_learn_blueprint(
                player,
                &blueprint_id,
                commands,
                inventories,
                &skill_scroll.item_registry,
                clients,
                player_states,
                &skill_scroll.cultivations,
                &mut skill_scroll.learned_blueprints,
            );
        }
        ForgeRequest::StartSession {
            station_pos,
            blueprint_id,
            materials,
        } => {
            handle_forge_start_session(
                player,
                station_pos,
                blueprint_id,
                materials,
                clients,
                &skill_scroll.forge_stations,
                &mut dispatch.start_forge_tx,
            );
        }
        ForgeRequest::BlueprintTurnPage { delta } => {
            handle_forge_blueprint_turn_page(
                player,
                delta,
                clients,
                &mut skill_scroll.learned_blueprints,
                skill_scroll.blueprint_registry.as_deref(),
            );
        }
    }
}

/// plan-forge-session-entry-wiring-v1 §4.1#3 — station_pos → 站台实体寻址结果。
#[derive(Debug, PartialEq, Eq)]
enum ForgeStationRouteError {
    Missing,
    Forbidden { owner: Option<Entity> },
}

/// 按 `station_pos` 在 `WeaponForgeStation` 里查实体，并校验 owner。
fn find_owned_forge_station(
    player: Entity,
    station_pos: (i32, i32, i32),
    stations: &Query<(Entity, &WeaponForgeStation)>,
) -> Result<Entity, ForgeStationRouteError> {
    let Some((station_entity, station)) = stations
        .iter()
        .find(|(_, station)| station.pos == Some(station_pos))
    else {
        return Err(ForgeStationRouteError::Missing);
    };
    let owner_ok = match station.owner {
        None => true,
        Some(owner) => owner == player,
    };
    if owner_ok {
        Ok(station_entity)
    } else {
        Err(ForgeStationRouteError::Forbidden {
            owner: station.owner,
        })
    }
}

fn send_forge_error(client: &mut Client, player_id: &str, message: String) {
    client.send_chat_message(format!("§c[炼器] {message}"));
    tracing::warn!("[bong][network][forge] error for `{player_id}`: {message}");
}

/// plan-forge-session-entry-wiring-v1 §4.1#3 — `ForgeStartSession` C2S 真分发。
#[allow(clippy::too_many_arguments)]
fn handle_forge_start_session(
    entity: Entity,
    station_pos: (i32, i32, i32),
    blueprint_id: String,
    materials: Vec<(String, u32)>,
    clients: &mut Query<(&Username, &mut Client)>,
    stations: &Query<(Entity, &WeaponForgeStation)>,
    start_forge_tx: &mut Option<ResMut<Events<StartForgeRequest>>>,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    let player_id = crate::player::state::canonical_player_id(username.0.as_str());
    match find_owned_forge_station(entity, station_pos, stations) {
        Ok(station_entity) => {
            let Some(start_forge_tx) = start_forge_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network][forge] start_session dropped: StartForgeRequest events resource missing"
                );
                return;
            };
            tracing::info!(
                "[bong][network][forge] start_session pos={station_pos:?} blueprint={blueprint_id} \
                 materials={materials:?} for `{player_id}`"
            );
            start_forge_tx.send(StartForgeRequest {
                station: station_entity,
                caster: entity,
                blueprint: blueprint_id,
                materials,
            });
        }
        Err(ForgeStationRouteError::Missing) => {
            tracing::warn!(
                "[bong][network][forge] `{player_id}` start_session rejected: missing station pos={station_pos:?}"
            );
            send_forge_error(
                &mut client,
                &player_id,
                format!("锻炉不存在：{station_pos:?}"),
            );
        }
        Err(ForgeStationRouteError::Forbidden { owner }) => {
            tracing::warn!(
                "[bong][network][forge] `{player_id}` tried to start_session at pos={station_pos:?} owned by {owner:?}"
            );
            send_forge_error(&mut client, &player_id, "这座炼器炉不是你的".to_string());
        }
    }
}

/// plan-forge-session-entry-wiring-v1 §4.1#2 — `ForgeBlueprintTurnPage` C2S 真分发。
fn handle_forge_blueprint_turn_page(
    entity: Entity,
    delta: i32,
    clients: &mut Query<(&Username, &mut Client)>,
    learned_blueprints: &mut Query<&mut LearnedBlueprints>,
    registry: Option<&BlueprintRegistry>,
) {
    let Ok(mut learned) = learned_blueprints.get_mut(entity) else {
        return;
    };
    if delta == 0 || learned.ids.is_empty() {
        return;
    }
    let steps = delta.unsigned_abs() % (learned.ids.len() as u32);
    for _ in 0..steps {
        if delta > 0 {
            learned.next_page();
        } else {
            learned.prev_page();
        }
    }

    let Ok((_, mut client)) = clients.get_mut(entity) else {
        return;
    };
    tracing::info!(
        "[bong][network][forge] blueprint_turn_page delta={delta} entity={entity:?} new_index={}",
        learned.current_index
    );
    let Some(registry) = registry else {
        tracing::warn!(
            "[bong][network][forge] blueprint_turn_page: BlueprintRegistry resource missing, S2C echo skipped"
        );
        return;
    };
    forge_snapshot_emit::send_blueprint_book_to_player(&mut client, &learned, registry);
}

#[allow(clippy::too_many_arguments)]
fn handle_forge_learn_blueprint(
    entity: Entity,
    blueprint_id: &str,
    commands: &mut Commands,
    inventories: &mut Query<&mut PlayerInventory>,
    registry: &ItemRegistry,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    learned_blueprints: &mut Query<&mut LearnedBlueprints>,
) {
    let blueprint_id = blueprint_id.trim();
    if blueprint_id.is_empty() {
        return;
    }

    if let Ok(learned) = learned_blueprints.get_mut(entity) {
        if learned.knows(blueprint_id) {
            if let Ok(inventory) = inventories.get(entity) {
                resync_snapshot(
                    entity,
                    inventory,
                    clients,
                    player_states,
                    cultivations,
                    "forge_blueprint_already_known",
                );
            }
            return;
        }
    }

    let Some(instance_id) = inventories
        .get(entity)
        .ok()
        .and_then(|inventory| find_blueprint_scroll_instance_id(inventory, registry, blueprint_id))
    else {
        if let Ok(inventory) = inventories.get(entity) {
            resync_snapshot(
                entity,
                inventory,
                clients,
                player_states,
                cultivations,
                "forge_blueprint_scroll_missing",
            );
        }
        tracing::warn!(
            "[bong][network][forge] learn_blueprint rejected: no scroll for blueprint_id={blueprint_id} on entity={entity:?}"
        );
        return;
    };

    {
        let Ok(mut inventory) = inventories.get_mut(entity) else {
            return;
        };
        if let Err(err) = consume_item_instance_once(&mut inventory, instance_id) {
            tracing::warn!(
                "[bong][network][forge] learn_blueprint consume failed for instance_id={instance_id}: {err}"
            );
            return;
        }
        resync_snapshot(
            entity,
            &inventory,
            clients,
            player_states,
            cultivations,
            "forge_blueprint_learned",
        );
    }

    if let Ok(mut learned) = learned_blueprints.get_mut(entity) {
        learned.learn(blueprint_id.to_string());
    } else {
        let mut learned = LearnedBlueprints::new();
        learned.learn(blueprint_id.to_string());
        commands.entity(entity).insert(learned);
    }
}

fn require_owned_active_step(
    forge_sessions: Option<&ForgeSessions>,
    session: ForgeSessionId,
    entity: Entity,
    expected: ForgeStep,
    pending_step: Option<ForgeStep>,
    request_label: &str,
) -> bool {
    let Some(forge_sessions) = forge_sessions else {
        tracing::warn!(
            "[bong][network][forge] {request_label} rejected: ForgeSessions unavailable"
        );
        return false;
    };
    let Some(session_state) = forge_sessions.get(session) else {
        tracing::warn!(
            "[bong][network][forge] {request_label} rejected: missing session_id={}",
            session.0
        );
        return false;
    };
    if session_state.caster != entity {
        tracing::warn!(
            "[bong][network][forge] {request_label} rejected: session_id={} caster mismatch entity={entity:?} session_caster={:?}",
            session.0,
            session_state.caster
        );
        return false;
    }
    if session_state.current_step != expected && pending_step != Some(expected) {
        tracing::warn!(
            "[bong][network][forge] {request_label} rejected: session_id={} step={:?}, pending={pending_step:?}, expected={expected:?}",
            session.0,
            session_state.current_step
        );
        return false;
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn handle_forge_inscription_scroll(
    entity: Entity,
    session_id: u64,
    inscription_id: &str,
    inventories: &mut Query<&mut PlayerInventory>,
    registry: &ItemRegistry,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    inscription_scroll_tx: &mut Option<ResMut<Events<InscriptionScrollSubmit>>>,
    forge_sessions: Option<&ForgeSessions>,
    pending_step: Option<ForgeStep>,
) {
    let inscription_id = inscription_id.trim();
    if inscription_id.is_empty() {
        return;
    }
    let session = ForgeSessionId(session_id);
    if !require_owned_active_step(
        forge_sessions,
        session,
        entity,
        ForgeStep::Inscription,
        pending_step,
        "inscription_scroll",
    ) {
        return;
    }
    let Some(inscription_scroll_tx) = inscription_scroll_tx.as_deref_mut() else {
        tracing::warn!(
            "[bong][network][forge] inscription_scroll rejected: ForgePlugin events unavailable"
        );
        return;
    };

    let Some(instance_id) = inventories.get(entity).ok().and_then(|inventory| {
        find_inscription_scroll_instance_id(inventory, registry, inscription_id)
    }) else {
        if let Ok(inventory) = inventories.get(entity) {
            resync_snapshot(
                entity,
                inventory,
                clients,
                player_states,
                cultivations,
                "forge_inscription_scroll_missing",
            );
        }
        tracing::warn!(
            "[bong][network][forge] inscription_scroll rejected: no scroll for inscription_id={inscription_id} on entity={entity:?}"
        );
        return;
    };

    inscription_scroll_tx.send(InscriptionScrollSubmit {
        session,
        caster: entity,
        item_instance_id: instance_id,
        inscription_id: inscription_id.to_string(),
    });
}

fn handle_forge_tempering_hit(
    entity: Entity,
    session_id: u64,
    beat: &str,
    ticks_remaining: u32,
    tempering_hit_tx: &mut Option<ResMut<Events<TemperingHit>>>,
    forge_sessions: Option<&ForgeSessions>,
    pending_step: Option<ForgeStep>,
) {
    let Some(beat) = parse_temper_beat(beat) else {
        tracing::warn!("[bong][network][forge] tempering_hit rejected: unknown beat `{beat}`");
        return;
    };
    let session = ForgeSessionId(session_id);
    if !require_owned_active_step(
        forge_sessions,
        session,
        entity,
        ForgeStep::Tempering,
        pending_step,
        "tempering_hit",
    ) {
        return;
    }
    let Some(tempering_hit_tx) = tempering_hit_tx.as_deref_mut() else {
        tracing::warn!(
            "[bong][network][forge] tempering_hit rejected: ForgePlugin events unavailable"
        );
        return;
    };
    tempering_hit_tx.send(TemperingHit {
        session,
        beat,
        ticks_remaining,
    });
}

fn handle_forge_consecration_inject(
    entity: Entity,
    session_id: u64,
    qi_amount: f64,
    consecration_inject_tx: &mut Option<ResMut<Events<ConsecrationInject>>>,
    forge_sessions: Option<&ForgeSessions>,
    pending_step: Option<ForgeStep>,
) {
    if !qi_amount.is_finite() || qi_amount < 0.0 {
        tracing::warn!(
            "[bong][network][forge] consecration_inject rejected: invalid qi_amount={qi_amount}"
        );
        return;
    }
    let session = ForgeSessionId(session_id);
    if !require_owned_active_step(
        forge_sessions,
        session,
        entity,
        ForgeStep::Consecration,
        pending_step,
        "consecration_inject",
    ) {
        return;
    }
    let Some(consecration_inject_tx) = consecration_inject_tx.as_deref_mut() else {
        tracing::warn!(
            "[bong][network][forge] consecration_inject rejected: ForgePlugin events unavailable"
        );
        return;
    };
    consecration_inject_tx.send(ConsecrationInject { session, qi_amount });
}

fn handle_forge_step_advance(
    entity: Entity,
    session_id: u64,
    step_advance_tx: &mut Option<ResMut<Events<StepAdvance>>>,
    forge_sessions: Option<&ForgeSessions>,
    blueprint_registry: Option<&BlueprintRegistry>,
) -> Option<(ForgeSessionId, ForgeStep)> {
    let session = ForgeSessionId(session_id);
    let Some(forge_sessions) = forge_sessions else {
        tracing::warn!("[bong][network][forge] step_advance rejected: ForgeSessions unavailable");
        return None;
    };
    let Some(session_state) = forge_sessions.get(session) else {
        tracing::warn!(
            "[bong][network][forge] step_advance rejected: missing session_id={session_id}"
        );
        return None;
    };
    if session_state.caster != entity {
        tracing::warn!(
            "[bong][network][forge] step_advance rejected: session_id={session_id} caster mismatch entity={entity:?} session_caster={:?}",
            session_state.caster
        );
        return None;
    }
    if matches!(session_state.current_step, ForgeStep::Done) {
        tracing::warn!(
            "[bong][network][forge] step_advance rejected: session_id={session_id} already done"
        );
        return None;
    }
    let Some(step_advance_tx) = step_advance_tx.as_deref_mut() else {
        tracing::warn!(
            "[bong][network][forge] step_advance rejected: ForgePlugin events unavailable"
        );
        return None;
    };
    let from_step = session_state.current_step;
    step_advance_tx.send(StepAdvance { session, from_step });
    let next_step = blueprint_registry
        .and_then(|registry| registry.get(session_state.blueprint.as_str()))
        .map(|blueprint| next_step_after(blueprint, session_state.step_index))
        .unwrap_or(ForgeStep::Done);
    Some((session, next_step))
}

fn parse_temper_beat(raw: &str) -> Option<crate::forge::blueprint::TemperBeat> {
    match raw {
        "L" => Some(crate::forge::blueprint::TemperBeat::Light),
        "H" => Some(crate::forge::blueprint::TemperBeat::Heavy),
        "F" => Some(crate::forge::blueprint::TemperBeat::Fold),
        _ => None,
    }
}

fn find_blueprint_scroll_instance_id(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    blueprint_id: &str,
) -> Option<u64> {
    find_inventory_instance_id_matching(inventory, |template_id| {
        registry
            .get(template_id)
            .and_then(|template| template.blueprint_scroll_spec.as_ref())
            .is_some_and(|spec| spec.blueprint_id == blueprint_id)
    })
}

fn find_inscription_scroll_instance_id(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    inscription_id: &str,
) -> Option<u64> {
    find_inventory_instance_id_matching(inventory, |template_id| {
        registry
            .get(template_id)
            .and_then(|template| template.inscription_scroll_spec.as_ref())
            .is_some_and(|spec| spec.inscription_id == inscription_id)
    })
}

fn find_inventory_instance_id_matching(
    inventory: &PlayerInventory,
    mut predicate: impl FnMut(&str) -> bool,
) -> Option<u64> {
    for item in inventory.hotbar.iter().flatten() {
        if predicate(item.template_id.as_str()) {
            return Some(item.instance_id);
        }
    }
    for container in &inventory.containers {
        for placed in &container.items {
            if predicate(placed.instance.template_id.as_str()) {
                return Some(placed.instance.instance_id);
            }
        }
    }
    for item in inventory.equipped.values().flat_map(|s| s.iter_all()) {
        if predicate(item.template_id.as_str()) {
            return Some(item.instance_id);
        }
    }
    None
}
