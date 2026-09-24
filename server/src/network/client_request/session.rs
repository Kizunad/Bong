//! C2S session-domain dispatch.
//!
//! The ingress system owns channel filtering, budget admission, decoding,
//! version checks, and live gates. This module only translates an admitted,
//! typed request into the existing domain events and session helpers.

use valence::message::SendMessage;
use valence::prelude::{Client, Commands, Entity, Events, Query, Username};

use crate::inventory::PlayerInventory;
use crate::schema::client_request::ClientRequestV1;
use crate::world::extract_system::{
    CancelExtractRequest as CancelExtractRequestEvent,
    StartExtractRequest as StartExtractRequestEvent,
};
use crate::world::tsy_container_search::{
    CancelSearchRequest as CancelSearchRequestEvent, StartSearchRequest as StartSearchRequestEvent,
};

use super::{ClientRequestDispatchParams, CombatRequestParams};

/// Dispatch one of the session-domain requests.
///
/// Returns `true` when `request` belongs to this domain, including when the
/// corresponding event resource is absent or the request is malformed. This
/// preserves the old branch-local drop/no-op behavior while keeping routing
/// decisions compile-time exhaustive.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn dispatch(
    request: &ClientRequestV1,
    player: Entity,
    dispatch: &mut ClientRequestDispatchParams,
    combat: &mut CombatRequestParams,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    commands: &mut Commands,
    vfx_events: Option<&mut Events<crate::network::vfx_event_emit::VfxEventRequest>>,
) -> bool {
    match request {
        ClientRequestV1::StartExtractRequest {
            portal_entity_id, ..
        } => {
            tracing::info!(
                "[bong][network] client_request start_extract entity={player:?} portal_bits={portal_entity_id}"
            );
            let Some(tx) = combat.start_extract_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped start_extract because StartExtractRequest event resource is missing"
                );
                return true;
            };
            let Ok(portal) = Entity::try_from_bits(*portal_entity_id) else {
                tracing::warn!(
                    "[bong][network] dropped start_extract: invalid portal_entity_id bits={portal_entity_id}"
                );
                return true;
            };
            tx.send(StartExtractRequestEvent { player, portal });
            true
        }
        ClientRequestV1::CancelExtractRequest { .. } => {
            tracing::info!("[bong][network] client_request cancel_extract entity={player:?}");
            let Some(tx) = combat.cancel_extract_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped cancel_extract because CancelExtractRequest event resource is missing"
                );
                return true;
            };
            tx.send(CancelExtractRequestEvent { player });
            true
        }
        ClientRequestV1::StartSearch {
            container_entity_id,
            ..
        } => {
            tracing::info!(
                "[bong][network] client_request start_search entity={player:?} container_bits={container_entity_id}"
            );
            let Some(tx) = combat.start_search_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped start_search because StartSearchRequest event resource is missing"
                );
                return true;
            };
            let Ok(container) = Entity::try_from_bits(*container_entity_id) else {
                tracing::warn!(
                    "[bong][network] dropped start_search: invalid container_entity_id bits={container_entity_id}"
                );
                return true;
            };
            tx.send(StartSearchRequestEvent { player, container });
            true
        }
        ClientRequestV1::CancelSearch { .. } => {
            tracing::info!("[bong][network] client_request cancel_search entity={player:?}");
            let Some(tx) = combat.cancel_search_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped cancel_search because CancelSearchRequest event resource is missing"
                );
                return true;
            };
            tx.send(CancelSearchRequestEvent { player });
            true
        }
        ClientRequestV1::SupplyCoffinOpen { entity_id, .. } => {
            tracing::info!(
                "[bong][network] client_request supply_coffin_open entity={player:?} target_id={entity_id}"
            );
            let Some(entity_manager) = combat.entity_manager.as_deref() else {
                tracing::warn!(
                    "[bong][network] dropped supply_coffin_open because EntityManager resource is missing"
                );
                return true;
            };
            let Some(target) = entity_manager.get_by_id(*entity_id) else {
                tracing::debug!(
                    "[bong][network] supply_coffin_open rejected: no entity for protocol id {entity_id}"
                );
                if let Ok((_username, mut client)) = clients.get_mut(player) {
                    client.send_chat_message("§c[物资棺] 目标不存在。");
                }
                return true;
            };
            if let Some(tx) = dispatch.supply_coffin_open_tx.as_deref_mut() {
                tx.send(crate::supply_coffin::interact::SupplyCoffinOpenRequest {
                    client: player,
                    target,
                });
            } else {
                tracing::warn!(
                    "[bong][network] dropped supply_coffin_open because SupplyCoffinOpenRequest event resource is missing"
                );
            }
            true
        }
        ClientRequestV1::WorkbenchOpen { entity_id, .. } => {
            tracing::info!(
                "[bong][network] client_request workbench_open entity={player:?} target_id={entity_id}"
            );
            let Some(entity_manager) = combat.entity_manager.as_deref() else {
                tracing::warn!(
                    "[bong][network] dropped workbench_open because EntityManager resource is missing"
                );
                return true;
            };
            let Some(workbench) = entity_manager.get_by_id(*entity_id) else {
                tracing::debug!(
                    "[bong][network] workbench_open rejected: no entity for protocol id {entity_id}"
                );
                if let Ok((_username, mut client)) = clients.get_mut(player) {
                    client.send_chat_message("§c[制作台] 目标不存在。");
                }
                return true;
            };
            if let Some(tx) = dispatch.workbench_open_tx.as_deref_mut() {
                tx.send(crate::craft::WorkbenchOpenRequest {
                    client: player,
                    workbench,
                });
            } else {
                tracing::warn!(
                    "[bong][network] dropped workbench_open because WorkbenchOpenRequest event resource is missing"
                );
            }
            true
        }
        ClientRequestV1::ScrollReadRequest { instance_id, .. } => {
            crate::network::scroll_open_emit::dispatch_scroll_read_open(
                player,
                *instance_id,
                inventories,
                &combat.item_registry,
                clients,
                &combat.positions,
                &combat.unique_ids,
                commands,
                vfx_events,
            );
            true
        }
        ClientRequestV1::ScrollReadClosed { .. } => {
            crate::network::scroll_open_emit::dispatch_scroll_read_close(
                player,
                &combat.scroll_reading_q,
                &combat.positions,
                &combat.unique_ids,
                commands,
                vfx_events,
            );
            true
        }
        _ => false,
    }
}
