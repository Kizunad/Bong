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
use crate::world::dimension::CurrentDimension;

use super::authority::authorize_supply_coffin_session;
use super::{current_wall_clock_secs, SupplyCoffinGrade, SupplyCoffinRegistry};

type PlayerQueryItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a CurrentDimension>,
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

        let Ok((_, player_pos, player_dimension, _, _, _, _, _)) = players.get(player_entity)
        else {
            // 玩家掉线 / entity 已消失——清 session 并释放锁，不发 UI（玩家已不在）。
            tracing::info!(
                "[bong][supply_coffin] session {session_id} player disappeared, closing session"
            );
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
            continue;
        };

        let authorization = authorize_supply_coffin_session(
            coffin_registry.active.get(&coffin_entity),
            player_pos.get(),
            player_dimension.map(|dimension| dimension.0),
        );
        if let Err(reason) = authorization {
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
                "[bong][supply_coffin] session {session_id} closed (authority invalid: {reason:?})"
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

        if let Ok((_, _, _, mut client, username, inventory, player_state, cultivation)) =
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

    if ext_registry.sessions.get(&session_id).copied() == Some(coffin_entity) {
        ext_registry.remove_session(session_id);
    } else if ext_registry.sessions.contains_key(&session_id) {
        tracing::warn!(
            "[bong][supply_coffin] close preserved conflicting session {session_id} mapping while closing {coffin_entity:?}"
        );
    }

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

    if let Ok((_, _, _, mut client, _, _, _, _)) = players.get_mut(player_entity) {
        send_server_data_payload(&mut client, bytes.as_slice());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ContainerState, InventoryRevision};
    use crate::supply_coffin::authority::SUPPLY_COFFIN_SESSION_MAX_DISTANCE;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use valence::prelude::{App, DVec3, Update};
    use valence::testing::{create_mock_client, MockClientHelper};

    const COFFIN_POS: DVec3 = DVec3::new(0.0, 64.0, 0.0);

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "main_pack".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
                owner_instance_id: None,
                quick_access: false,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
            triggered_treasures: Vec::new(),
        }
    }

    fn setup_lifecycle_app(
        player_dimension: Option<DimensionKind>,
        player_pos: DVec3,
        source_active: bool,
    ) -> (App, Entity, Entity, MockClientHelper) {
        let mut app = App::new();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, external_container_lifecycle_tick);

        let (client_bundle, helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                empty_inventory(),
                PlayerState::default(),
                Cultivation::default(),
            ))
            .id();
        app.world_mut()
            .entity_mut(player)
            .insert(Position::new(player_pos));
        if let Some(dimension) = player_dimension {
            app.world_mut()
                .entity_mut(player)
                .insert(CurrentDimension(dimension));
        }

        let session_id = 91;
        let coffin = app
            .world_mut()
            .spawn(ExternalContainer {
                session_id,
                container: ContainerState {
                    id: ExternalContainer::container_id(session_id),
                    name: "supply_coffin_common".to_string(),
                    rows: 3,
                    cols: 4,
                    items: Vec::new(),
                    owner_instance_id: None,
                    quick_access: false,
                },
                opened_by: Some(player),
                timeout_wall_secs: u64::MAX,
                source_kind: ExternalContainerKind::SupplyCoffin {
                    grade: SupplyCoffinGrade::Common,
                },
            })
            .id();

        let mut ext_registry = ExternalContainerRegistry::default();
        ext_registry.sessions.insert(session_id, coffin);
        ext_registry.next_session_id = session_id + 1;
        app.insert_resource(ext_registry);

        let mut coffin_registry =
            SupplyCoffinRegistry::new((DVec3::ZERO, DVec3::new(100.0, 100.0, 100.0)), 65.0, 0x4321);
        if source_active {
            coffin_registry.insert_active(
                coffin,
                SupplyCoffinGrade::Common,
                COFFIN_POS,
                current_wall_clock_secs(),
            );
        }
        app.insert_resource(coffin_registry);

        (app, player, coffin, helper)
    }

    fn assert_session_closed_without_despawn(app: &App, coffin: Entity, session_id: u64) {
        let ext = app
            .world()
            .get::<ExternalContainer>(coffin)
            .expect("distance/authority invalidation must keep the coffin container alive");
        assert_eq!(
            ext.opened_by, None,
            "invalid authority must release the supply-coffin occupancy lock"
        );
        assert!(
            !app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .contains_key(&session_id),
            "invalid authority must remove the stale session mapping"
        );
        assert!(
            app.world().get::<Despawned>(coffin).is_none(),
            "distance/dimension/source invalidation must not break or despawn the coffin"
        );
    }

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

    #[test]
    fn lifecycle_closes_cross_dimension_same_xyz_and_is_idempotent() {
        let (mut app, _player, coffin, _helper) =
            setup_lifecycle_app(Some(DimensionKind::Tsy), COFFIN_POS, true);

        app.update();
        assert_session_closed_without_despawn(&app, coffin, 91);
        assert!(
            app.world()
                .resource::<SupplyCoffinRegistry>()
                .active
                .contains_key(&coffin),
            "dimension invalidation closes only the session; the active coffin remains"
        );

        app.update();
        assert_session_closed_without_despawn(&app, coffin, 91);
    }

    #[test]
    fn lifecycle_closes_when_player_dimension_is_missing() {
        let (mut app, _player, coffin, _helper) = setup_lifecycle_app(None, COFFIN_POS, true);

        app.update();

        assert_session_closed_without_despawn(&app, coffin, 91);
    }

    #[test]
    fn lifecycle_closes_when_active_source_disappears() {
        let (mut app, _player, coffin, _helper) =
            setup_lifecycle_app(Some(DimensionKind::Overworld), COFFIN_POS, false);

        app.update();

        assert_session_closed_without_despawn(&app, coffin, 91);
    }

    #[test]
    fn lifecycle_close_does_not_remove_another_entitys_conflicting_session_mapping() {
        let (mut app, _player, coffin, _helper) =
            setup_lifecycle_app(Some(DimensionKind::Tsy), COFFIN_POS, true);
        let conflicting_target = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<ExternalContainerRegistry>()
            .sessions
            .insert(91, conflicting_target);

        app.update();

        assert_eq!(
            app.world()
                .get::<ExternalContainer>(coffin)
                .expect("coffin remains alive")
                .opened_by,
            None,
            "authority invalidation still releases this coffin's lock"
        );
        assert_eq!(
            app.world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .get(&91),
            Some(&conflicting_target),
            "closing one stale coffin must not delete another entity's session mapping"
        );
    }

    #[test]
    fn lifecycle_closes_and_clears_session_when_owner_entity_disappears() {
        let (mut app, player, coffin, _helper) =
            setup_lifecycle_app(Some(DimensionKind::Overworld), COFFIN_POS, true);
        assert!(app.world_mut().despawn(player), "test owner should despawn");

        app.update();

        assert_session_closed_without_despawn(&app, coffin, 91);
    }

    #[test]
    fn lifecycle_distance_boundary_is_inclusive_and_just_outside_closes() {
        let boundary = SUPPLY_COFFIN_SESSION_MAX_DISTANCE;
        let (mut at_boundary, _player, coffin, _helper) = setup_lifecycle_app(
            Some(DimensionKind::Overworld),
            COFFIN_POS + DVec3::new(boundary, 0.0, 0.0),
            true,
        );
        at_boundary.update();
        let ext = at_boundary
            .world()
            .get::<ExternalContainer>(coffin)
            .expect("coffin remains alive at boundary");
        assert!(
            ext.opened_by.is_some(),
            "distance exactly {boundary} must remain authorized"
        );
        assert_eq!(
            at_boundary
                .world()
                .resource::<ExternalContainerRegistry>()
                .sessions
                .get(&91),
            Some(&coffin),
            "boundary session must stay registered"
        );

        let (mut outside, _player, coffin, _helper) = setup_lifecycle_app(
            Some(DimensionKind::Overworld),
            COFFIN_POS + DVec3::new(boundary + 0.001, 0.0, 0.0),
            true,
        );
        outside.update();
        assert_session_closed_without_despawn(&outside, coffin, 91);
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
