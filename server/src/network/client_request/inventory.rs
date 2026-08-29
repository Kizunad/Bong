//! C2S inventory/container typed dispatch.
//!
//! This module only owns compile-time extraction and routing for the six
//! inventory/container requests. Existing inventory and session helpers remain
//! the single source of validation, logging, and mutation behavior.

use valence::prelude::{Client, Commands, Entity, Events, Query, Username};

use crate::inventory::PlayerInventory;
use crate::network::client_request_handler::{
    AlchemyRequestParams, ClientRequestDispatchParams, CombatRequestParams,
    DroppedLootRequestParams, SkillScrollRequestParams,
};
use crate::player::state::{PlayerState, PlayerStatePersistence};
use crate::schema::client_request::ClientRequestV1;
use crate::schema::inventory::InventoryLocationV1;
use crate::world::karma::KarmaWeightStore;

/// Inventory/container requests admitted by the top-level ingress gates.
#[derive(Debug, PartialEq)]
pub enum InventoryRequest {
    InventoryMoveIntent {
        instance_id: u64,
        from: InventoryLocationV1,
        to: InventoryLocationV1,
        rotated: bool,
    },
    InventoryDiscardItem {
        instance_id: u64,
        from: InventoryLocationV1,
    },
    PickupDroppedItem {
        instance_id: u64,
    },
    ContainerOpen {
        entity_id: i32,
    },
    ExternalContainerMove {
        session_id: u64,
        instance_id: u64,
        from: InventoryLocationV1,
        to: InventoryLocationV1,
    },
    ExternalContainerClose {
        session_id: u64,
    },
}

/// Extract exactly the six inventory/container variants; all other requests
/// are returned unchanged to the next typed route or the legacy match.
pub fn try_into_inventory_request(
    request: ClientRequestV1,
) -> Result<InventoryRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::InventoryMoveIntent {
            instance_id,
            from,
            to,
            rotated,
            ..
        } => Ok(InventoryRequest::InventoryMoveIntent {
            instance_id,
            from,
            to,
            rotated,
        }),
        ClientRequestV1::InventoryDiscardItem {
            instance_id, from, ..
        } => Ok(InventoryRequest::InventoryDiscardItem { instance_id, from }),
        ClientRequestV1::PickupDroppedItem { instance_id, .. } => {
            Ok(InventoryRequest::PickupDroppedItem { instance_id })
        }
        ClientRequestV1::ContainerOpen { entity_id, .. } => {
            Ok(InventoryRequest::ContainerOpen { entity_id })
        }
        ClientRequestV1::ExternalContainerMove {
            session_id,
            instance_id,
            from,
            to,
            ..
        } => Ok(InventoryRequest::ExternalContainerMove {
            session_id,
            instance_id,
            from,
            to,
        }),
        ClientRequestV1::ExternalContainerClose { session_id, .. } => {
            Ok(InventoryRequest::ExternalContainerClose { session_id })
        }
        request => Err(request),
    }
}

/// Dispatch one extracted inventory/container request through the existing
/// helpers. No business checks or mutation are duplicated here.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_inventory_request<
    'alchemy_w,
    'alchemy_s,
    'combat_w,
    'combat_s,
    'dispatch_w,
    'dropped_w,
    'dropped_s,
    'skill_w,
    'skill_s,
>(
    request: InventoryRequest,
    player: Entity,
    alchemy: &mut AlchemyRequestParams<'alchemy_w, 'alchemy_s>,
    combat: &mut CombatRequestParams<'combat_w, 'combat_s>,
    dispatch: &mut ClientRequestDispatchParams<'dispatch_w>,
    dropped_loot: &mut DroppedLootRequestParams<'dropped_w, 'dropped_s>,
    skill_scroll: &mut SkillScrollRequestParams<'skill_w, 'skill_s>,
    persistence: Option<&PlayerStatePersistence>,
    karma_weights: Option<&KarmaWeightStore>,
    durability_changed_tx: Option<&mut Events<crate::inventory::InventoryDurabilityChangedEvent>>,
    clients: &mut Query<(&Username, &mut Client)>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    commands: &mut Commands,
) {
    match request {
        InventoryRequest::InventoryMoveIntent {
            instance_id,
            from,
            to,
            rotated,
        } => crate::network::client_request_handler::handle_inventory_move(
            player,
            instance_id,
            from,
            to,
            rotated,
            &combat.item_registry,
            inventories,
            clients,
            player_states,
            &skill_scroll.cultivations,
            karma_weights,
            durability_changed_tx,
            &skill_scroll.positions,
            &skill_scroll.dimensions,
            alchemy.zones.as_deref_mut(),
            alchemy.attrition_qi_transfers.as_deref_mut(),
            alchemy.attrition_applied_events.as_deref_mut(),
            alchemy.tsy_lifecycle.as_deref(),
            &mut dropped_loot.registry,
            alchemy.vfx_events.as_deref_mut(),
            combat.body_plans.as_deref(),
            combat.race_registry.as_deref(),
            &skill_scroll.morph_states,
        ),
        InventoryRequest::InventoryDiscardItem { instance_id, from } => {
            crate::network::client_request_handler::handle_inventory_discard(
                player,
                instance_id,
                from,
                inventories,
                &mut dropped_loot.registry,
                &combat.item_registry,
                clients,
                player_states,
                &skill_scroll.cultivations,
                &dropped_loot.positions,
                &skill_scroll.dimensions,
            );
        }
        InventoryRequest::PickupDroppedItem { instance_id } => {
            crate::network::client_request_handler::handle_pickup_dropped_item(
                player,
                instance_id,
                inventories,
                &mut dropped_loot.registry,
                &combat.item_registry,
                clients,
                player_states,
                &skill_scroll.cultivations,
                &dropped_loot.positions,
                &skill_scroll.dimensions,
                alchemy.zones.as_deref_mut(),
                alchemy.attrition_qi_transfers.as_deref_mut(),
                alchemy.attrition_applied_events.as_deref_mut(),
                alchemy.tsy_lifecycle.as_deref(),
                persistence,
            );
        }
        InventoryRequest::ContainerOpen { entity_id } => {
            tracing::info!(
                "[bong][network] client_request container_open entity={player:?} target_id={entity_id}"
            );
            let Some(entity_manager) = combat.entity_manager.as_deref() else {
                tracing::warn!(
                    "[bong][network] dropped container_open because EntityManager resource is missing"
                );
                return;
            };
            let Some(target) = entity_manager.get_by_id(entity_id) else {
                tracing::debug!(
                    "[bong][network] container_open rejected: no entity for protocol id {entity_id}"
                );
                if let Ok((_username, mut client)) = clients.get_mut(player) {
                    use valence::message::SendMessage;
                    client.send_chat_message("§c[容器] 目标不存在。");
                }
                return;
            };
            if let Some(tx) = dispatch.container_open_tx.as_deref_mut() {
                tx.send(crate::world::container_open::ContainerOpenRequest {
                    client: player,
                    target,
                });
            } else {
                tracing::warn!(
                    "[bong][network] dropped container_open because ContainerOpenRequest event resource is missing"
                );
            }
        }
        InventoryRequest::ExternalContainerMove {
            session_id,
            instance_id,
            from,
            to,
        } => crate::network::client_request_handler::handle_external_container_move(
            player,
            session_id,
            instance_id,
            &from,
            &to,
            dispatch,
            combat,
            inventories,
            player_states,
            &skill_scroll.cultivations,
            clients,
            &skill_scroll.positions,
            &skill_scroll.dimensions,
            commands,
        ),
        InventoryRequest::ExternalContainerClose { session_id } => {
            crate::network::client_request_handler::handle_external_container_close(
                player,
                session_id,
                dispatch,
                combat,
                inventories,
                player_states,
                &skill_scroll.cultivations,
                clients,
                commands,
            );
        }
    }
}
