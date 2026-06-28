//! plan-supply-coffin-loot-ui P2 — ExternalContainer 生命周期 tick。
//!
//! 每 tick 检查所有活跃 ExternalContainer：
//! - **超时**：`now >= timeout_wall_secs` → close + despawn + 入冷却
//! - **距离**：玩家离开 > 6 格 → close（棺不碎，等超时后自然碎裂）
//! - **掉线**：`opened_by` 指向的 entity 不存在 → 释放占用锁

use valence::prelude::{
    bevy_ecs, Client, Commands, Despawned, Entity, Position, Query, ResMut, Username, With,
};

use crate::cultivation::components::Cultivation;
use crate::inventory::external_container::{
    ExternalContainer, ExternalContainerKind, ExternalContainerRegistry,
};
use crate::inventory::PlayerInventory;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_AREA_RADIUS};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::PlayerState;
use crate::schema::server_data::{
    LootContainerCloseReasonV1, LootContainerCloseV1, ServerDataPayloadV1, ServerDataV1,
};
use crate::schema::vfx_event::VfxEventPayloadV1;

use super::{current_wall_clock_secs, SupplyCoffinGrade, SupplyCoffinRegistry};

const CLOSE_DISTANCE_BLOCKS: f64 = 6.0;
const CLOSE_DISTANCE_TOLERANCE: f64 = 0.5;

type PlayerQueryItem<'a> = (
    Entity,
    &'a Position,
    &'a mut Client,
    &'a Username,
    &'a PlayerInventory,
    &'a PlayerState,
    &'a Cultivation,
);

#[allow(clippy::too_many_arguments)]
pub fn external_container_lifecycle_tick(
    mut commands: Commands,
    mut ext_registry: ResMut<ExternalContainerRegistry>,
    mut coffin_registry: ResMut<SupplyCoffinRegistry>,
    ext_containers: Query<(Entity, &ExternalContainer)>,
    mut players: Query<PlayerQueryItem<'_>, With<Client>>,
    mut audio: bevy_ecs::event::EventWriter<PlaySoundRecipeRequest>,
    mut vfx: bevy_ecs::event::EventWriter<VfxEventRequest>,
) {
    let now = current_wall_clock_secs();

    for (coffin_entity, ext) in ext_containers.iter() {
        if !is_supply_coffin_lifecycle_managed(ext) {
            continue;
        }
        let session_id = ext.session_id;

        // 超时检查
        if now >= ext.timeout_wall_secs {
            close_session(
                &mut commands,
                &mut ext_registry,
                &mut coffin_registry,
                coffin_entity,
                ext,
                LootContainerCloseReasonV1::Timeout,
                &mut players,
                true,
                &mut audio,
                &mut vfx,
            );
            tracing::info!(
                "[bong][supply_coffin] session {session_id} timed out, coffin despawned"
            );
            continue;
        }

        // 玩家存在性 + 距离检查
        let Some(player_entity) = ext.opened_by else {
            continue;
        };

        let Ok((_, player_pos, _, _, _, _, _)) = players.get(player_entity) else {
            // 玩家掉线——释放锁，不关闭 UI（玩家已不在）
            tracing::info!(
                "[bong][supply_coffin] session {session_id} player disconnected, releasing lock"
            );
            release_lock(&mut commands, coffin_entity, ext);
            continue;
        };

        let Some(coffin_pos) = coffin_registry.active.get(&coffin_entity).map(|a| a.pos) else {
            tracing::warn!(
                "[bong][supply_coffin] session {session_id} coffin entity {coffin_entity:?} not in active registry, skipping distance check"
            );
            continue;
        };

        let dist = coffin_pos.distance(player_pos.get());
        if dist > CLOSE_DISTANCE_BLOCKS + CLOSE_DISTANCE_TOLERANCE {
            close_session(
                &mut commands,
                &mut ext_registry,
                &mut coffin_registry,
                coffin_entity,
                ext,
                LootContainerCloseReasonV1::Distance,
                &mut players,
                false,
                &mut audio,
                &mut vfx,
            );
            tracing::info!(
                "[bong][supply_coffin] session {session_id} closed (player too far: {dist:.1})"
            );
        }
    }
}

pub fn is_supply_coffin_lifecycle_managed(ext: &ExternalContainer) -> bool {
    matches!(ext.source_kind, ExternalContainerKind::SupplyCoffin { .. })
}

#[allow(clippy::too_many_arguments)]
fn close_session(
    commands: &mut Commands,
    ext_registry: &mut ExternalContainerRegistry,
    coffin_registry: &mut SupplyCoffinRegistry,
    coffin_entity: Entity,
    ext: &ExternalContainer,
    reason: LootContainerCloseReasonV1,
    players: &mut Query<PlayerQueryItem<'_>, With<Client>>,
    despawn: bool,
    audio: &mut bevy_ecs::event::EventWriter<PlaySoundRecipeRequest>,
    vfx: &mut bevy_ecs::event::EventWriter<VfxEventRequest>,
) {
    let session_id = ext.session_id;

    if let Some(player_entity) = ext.opened_by {
        send_close_payload(session_id, &reason, players, player_entity);

        if let Ok((_, _, mut client, username, inventory, player_state, cultivation)) =
            players.get_mut(player_entity)
        {
            send_inventory_snapshot_to_client(
                player_entity,
                &mut client,
                username.as_str(),
                inventory,
                player_state,
                cultivation,
                "supply_coffin_close",
            );
        }
    }

    ext_registry.remove_session(session_id);

    if despawn {
        if let Some(active) = coffin_registry.remove_active(coffin_entity) {
            coffin_registry.enqueue_cooldown(active.grade, current_wall_clock_secs());

            let break_recipe = match active.grade {
                SupplyCoffinGrade::Common => "supply_coffin_break_common",
                SupplyCoffinGrade::Rare => "supply_coffin_break_rare",
                SupplyCoffinGrade::Precious => "supply_coffin_break_precious",
            };
            audio.send(PlaySoundRecipeRequest {
                recipe_id: break_recipe.to_string(),
                instance_id: 0,
                pos: Some([
                    active.pos.x as i32,
                    active.pos.y as i32,
                    active.pos.z as i32,
                ]),
                flag: None,
                volume_mul: 1.0,
                pitch_shift: 0.0,
                recipient: AudioRecipient::Radius {
                    origin: active.pos,
                    radius: AUDIO_AREA_RADIUS,
                },
            });

            let color = match active.grade {
                SupplyCoffinGrade::Common => "#8B6914",
                SupplyCoffinGrade::Rare => "#2A1506",
                SupplyCoffinGrade::Precious => "#C4A35A",
            };
            vfx.send(VfxEventRequest::new(
                active.pos,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: "bong:supply_coffin_break".to_string(),
                    origin: [active.pos.x, active.pos.y, active.pos.z],
                    direction: None,
                    color: Some(color.to_string()),
                    strength: None,
                    count: Some(12),
                    duration_ticks: Some(15),
                },
            ));
        }
        commands.entity(coffin_entity).insert(Despawned);
    } else {
        // 距离/主动关闭——棺不碎，只释放锁，等超时碎裂
        release_lock(commands, coffin_entity, ext);
    }
}

fn release_lock(commands: &mut Commands, coffin_entity: Entity, ext: &ExternalContainer) {
    let mut updated = ext.clone();
    updated.opened_by = None;
    commands.entity(coffin_entity).insert(updated);
}

fn send_close_payload(
    session_id: u64,
    reason: &LootContainerCloseReasonV1,
    players: &mut Query<PlayerQueryItem<'_>, With<Client>>,
    player_entity: Entity,
) {
    let close_payload = ServerDataV1::new(ServerDataPayloadV1::LootContainerClose(
        LootContainerCloseV1 {
            session_id,
            reason: reason.clone(),
        },
    ));

    let payload_type = payload_type_label(close_payload.payload_type());
    let bytes = match serialize_server_data_payload(&close_payload) {
        Ok(b) => b,
        Err(e) => {
            log_payload_build_error(payload_type, &e);
            return;
        }
    };

    if let Ok((_, _, mut client, _, _, _, _)) = players.get_mut(player_entity) {
        send_server_data_payload(&mut client, bytes.as_slice());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ContainerState;

    #[test]
    fn lifecycle_manages_only_supply_coffins() {
        let supply = ext_with_kind(ExternalContainerKind::SupplyCoffin {
            grade: SupplyCoffinGrade::Common,
        });
        let trade_crate = ext_with_kind(ExternalContainerKind::StorageCrate { is_herb: false });
        let herb_crate = ext_with_kind(ExternalContainerKind::StorageCrate { is_herb: true });
        let dead_drop = ext_with_kind(ExternalContainerKind::DeadDrop);

        assert!(
            is_supply_coffin_lifecycle_managed(&supply),
            "supply coffin sessions remain owned by supply_coffin lifecycle"
        );
        assert!(
            !is_supply_coffin_lifecycle_managed(&trade_crate),
            "trade crate must not be timed out by supply_coffin lifecycle"
        );
        assert!(
            !is_supply_coffin_lifecycle_managed(&herb_crate),
            "herb crate must not be timed out by supply_coffin lifecycle"
        );
        assert!(
            !is_supply_coffin_lifecycle_managed(&dead_drop),
            "dead drop must not be timed out by supply_coffin lifecycle"
        );
    }

    fn ext_with_kind(source_kind: ExternalContainerKind) -> ExternalContainer {
        ExternalContainer {
            session_id: 1,
            container: ContainerState {
                quick_access: false,
                id: ExternalContainer::container_id(1),
                name: "container".to_string(),
                rows: 1,
                cols: 1,
                items: vec![],
                owner_instance_id: None,
            },
            opened_by: None,
            timeout_wall_secs: 0,
            source_kind,
        }
    }
}
