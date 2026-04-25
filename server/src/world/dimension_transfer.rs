//! Cross-dimension transfer for clients (and future NPCs).
//!
//! Plan-tsy-dimension-v1 §3: callers send a [`DimensionTransferRequest`] event
//! and [`apply_dimension_transfers`] mutates the entity's layer pointers +
//! position. Valence's `respawn` system reacts to `Changed<VisibleChunkLayer>`
//! by sending the `PlayerRespawnS2c` packet automatically, so clients see the
//! dimension change without us writing the packet ourselves.

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, App, DVec3, Entity, EntityLayerId, Event, EventReader, IntoSystemConfigs, Position,
    Query, Res, Update, VisibleChunkLayer, VisibleEntityLayers,
};

use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};

/// Request to transfer `entity` to `target` dimension at `target_pos`.
///
/// Multiple requests for the same entity in the same tick: only the last one
/// applied wins (see §3.2).
#[derive(Event, Debug, Clone, Copy)]
pub struct DimensionTransferRequest {
    pub entity: Entity,
    pub target: DimensionKind,
    pub target_pos: DVec3,
}

/// Bevy system applying queued [`DimensionTransferRequest`] events.
///
/// Expects the entity to already have `CurrentDimension`, `EntityLayerId`,
/// `VisibleChunkLayer`, `VisibleEntityLayers` and `Position` components — the
/// production wiring (`player::init_clients`) ensures this for clients.
pub fn apply_dimension_transfers(
    layers: Option<Res<DimensionLayers>>,
    mut requests: EventReader<DimensionTransferRequest>,
    mut clients: Query<(
        &mut EntityLayerId,
        &mut VisibleChunkLayer,
        &mut VisibleEntityLayers,
        &mut Position,
        &mut CurrentDimension,
    )>,
) {
    let Some(layers) = layers else {
        // No dimension layers resource → silently drain requests so events do not pile up.
        for _ in requests.read() {}
        return;
    };

    // Same-tick dedup: keep only the last request per entity.
    let latest: HashMap<Entity, DimensionTransferRequest> =
        requests.read().map(|req| (req.entity, *req)).collect();

    for (entity, req) in latest {
        let target_layer = layers.entity_for(req.target);
        let Ok((mut layer_id, mut visible_chunk, mut visible_entities, mut position, mut current)) =
            clients.get_mut(entity)
        else {
            tracing::warn!(
                "[bong][world] dimension transfer ignored — entity {entity:?} missing required components"
            );
            continue;
        };

        // Drop the previous layer's entity-replication subscription before adding the new one.
        let previous_layer = layer_id.0;
        if previous_layer != target_layer {
            visible_entities.0.remove(&previous_layer);
        }

        layer_id.0 = target_layer;
        visible_chunk.0 = target_layer;
        visible_entities.0.insert(target_layer);
        position.set(req.target_pos);
        current.0 = req.target;

        tracing::info!(
            "[bong][world] dimension transfer entity={entity:?} -> {:?} @ ({:.1},{:.1},{:.1})",
            req.target,
            req.target_pos.x,
            req.target_pos.y,
            req.target_pos.z
        );
    }
}

pub fn register(app: &mut App) {
    app.add_event::<DimensionTransferRequest>().add_systems(
        Update,
        apply_dimension_transfers.in_set(DimensionTransferSet),
    );
}

/// SystemSet for the dimension transfer step. Other systems can `.before(DimensionTransferSet)` /
/// `.after(DimensionTransferSet)` to order against it.
#[derive(bevy_ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DimensionTransferSet;

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events};

    fn run_apply(layers: DimensionLayers, events: &[DimensionTransferRequest]) -> App {
        let mut app = App::new();
        app.insert_resource(layers);
        app.add_event::<DimensionTransferRequest>();
        // Add a single update of `apply_dimension_transfers`. We seed events on the
        // `Events<DimensionTransferRequest>` resource directly to avoid set-up
        // ordering issues with a Schedule.
        app.add_systems(Update, apply_dimension_transfers);

        {
            let mut tx = app
                .world_mut()
                .resource_mut::<Events<DimensionTransferRequest>>();
            for ev in events {
                tx.send(*ev);
            }
        }
        app.update();
        app
    }

    #[test]
    fn missing_components_are_logged_and_skipped() {
        // Build an entity that has CurrentDimension but no Position etc.
        let mut app = App::new();
        let layers = DimensionLayers {
            overworld: app
                .world_mut()
                .spawn(crate::world::dimension::OverworldLayer)
                .id(),
            tsy: app
                .world_mut()
                .spawn(crate::world::dimension::TsyLayer)
                .id(),
        };
        app.insert_resource(layers);
        app.add_event::<DimensionTransferRequest>();
        app.add_systems(Update, apply_dimension_transfers);

        let dummy = app
            .world_mut()
            .spawn(CurrentDimension(DimensionKind::Overworld))
            .id();
        {
            let mut tx = app
                .world_mut()
                .resource_mut::<Events<DimensionTransferRequest>>();
            tx.send(DimensionTransferRequest {
                entity: dummy,
                target: DimensionKind::Tsy,
                target_pos: DVec3::new(0.0, 80.0, 0.0),
            });
        }
        app.update();

        // Entity should still report Overworld since the system bailed out (no Position).
        let cd = app
            .world()
            .entity(dummy)
            .get::<CurrentDimension>()
            .copied()
            .unwrap();
        assert_eq!(cd, CurrentDimension(DimensionKind::Overworld));
    }

    #[test]
    fn same_tick_double_request_is_deduplicated_to_last() {
        let mut app = App::new();
        // Use placeholder layer entities so DimensionLayers resolves.
        let overworld = app
            .world_mut()
            .spawn(crate::world::dimension::OverworldLayer)
            .id();
        let tsy = app
            .world_mut()
            .spawn(crate::world::dimension::TsyLayer)
            .id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.add_event::<DimensionTransferRequest>();
        app.add_systems(Update, apply_dimension_transfers);

        // Spawn an entity with all required components, sitting on overworld.
        let entity = app
            .world_mut()
            .spawn((
                EntityLayerId(overworld),
                VisibleChunkLayer(overworld),
                VisibleEntityLayers::default(),
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        {
            let mut tx = app
                .world_mut()
                .resource_mut::<Events<DimensionTransferRequest>>();
            tx.send(DimensionTransferRequest {
                entity,
                target: DimensionKind::Tsy,
                target_pos: DVec3::new(1.0, 80.0, 1.0),
            });
            // Last-write-wins: this one should be the only effect.
            tx.send(DimensionTransferRequest {
                entity,
                target: DimensionKind::Overworld,
                target_pos: DVec3::new(2.0, 64.0, 2.0),
            });
        }
        app.update();

        let world = app.world();
        let er = world.entity(entity);
        let cd = er.get::<CurrentDimension>().copied().unwrap();
        let layer_id = er.get::<EntityLayerId>().unwrap().0;
        let visible_chunk = er.get::<VisibleChunkLayer>().unwrap().0;
        let pos = er.get::<Position>().unwrap().get();
        assert_eq!(cd, CurrentDimension(DimensionKind::Overworld));
        assert_eq!(layer_id, overworld);
        assert_eq!(visible_chunk, overworld);
        assert_eq!(pos, DVec3::new(2.0, 64.0, 2.0));
    }

    #[test]
    fn transfer_overworld_to_tsy_updates_layer_and_position() {
        let mut app = App::new();
        let overworld = app
            .world_mut()
            .spawn(crate::world::dimension::OverworldLayer)
            .id();
        let tsy = app
            .world_mut()
            .spawn(crate::world::dimension::TsyLayer)
            .id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.add_event::<DimensionTransferRequest>();
        app.add_systems(Update, apply_dimension_transfers);

        let mut visible = VisibleEntityLayers::default();
        visible.0.insert(overworld);
        let entity = app
            .world_mut()
            .spawn((
                EntityLayerId(overworld),
                VisibleChunkLayer(overworld),
                visible,
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        {
            let mut tx = app
                .world_mut()
                .resource_mut::<Events<DimensionTransferRequest>>();
            tx.send(DimensionTransferRequest {
                entity,
                target: DimensionKind::Tsy,
                target_pos: DVec3::new(0.0, 80.0, 0.0),
            });
        }
        app.update();

        let world = app.world();
        let er = world.entity(entity);
        assert_eq!(
            er.get::<CurrentDimension>().copied().unwrap(),
            CurrentDimension(DimensionKind::Tsy)
        );
        assert_eq!(er.get::<EntityLayerId>().unwrap().0, tsy);
        assert_eq!(er.get::<VisibleChunkLayer>().unwrap().0, tsy);
        let visible_set = &er.get::<VisibleEntityLayers>().unwrap().0;
        assert!(visible_set.contains(&tsy));
        assert!(!visible_set.contains(&overworld));
        assert_eq!(
            er.get::<Position>().unwrap().get(),
            DVec3::new(0.0, 80.0, 0.0)
        );
    }

    #[test]
    fn round_trip_overworld_tsy_overworld_keeps_layer_set_clean() {
        let mut app = App::new();
        let overworld = app
            .world_mut()
            .spawn(crate::world::dimension::OverworldLayer)
            .id();
        let tsy = app
            .world_mut()
            .spawn(crate::world::dimension::TsyLayer)
            .id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.add_event::<DimensionTransferRequest>();
        app.add_systems(Update, apply_dimension_transfers);

        let mut visible = VisibleEntityLayers::default();
        visible.0.insert(overworld);
        let entity = app
            .world_mut()
            .spawn((
                EntityLayerId(overworld),
                VisibleChunkLayer(overworld),
                visible,
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        // Tick 1: → TSY
        {
            let mut tx = app
                .world_mut()
                .resource_mut::<Events<DimensionTransferRequest>>();
            tx.send(DimensionTransferRequest {
                entity,
                target: DimensionKind::Tsy,
                target_pos: DVec3::new(0.0, 80.0, 0.0),
            });
        }
        app.update();

        // Tick 2: → Overworld
        {
            let mut tx = app
                .world_mut()
                .resource_mut::<Events<DimensionTransferRequest>>();
            tx.send(DimensionTransferRequest {
                entity,
                target: DimensionKind::Overworld,
                target_pos: DVec3::new(8.0, 66.0, 8.0),
            });
        }
        app.update();

        let world = app.world();
        let er = world.entity(entity);
        let visible_set = &er.get::<VisibleEntityLayers>().unwrap().0;
        // After round trip the entity should be subscribed only to overworld layer.
        assert_eq!(er.get::<EntityLayerId>().unwrap().0, overworld);
        assert!(visible_set.contains(&overworld));
        assert!(!visible_set.contains(&tsy));
    }

    #[test]
    fn empty_request_stream_is_no_op() {
        let layers = DimensionLayers {
            overworld: Entity::from_raw(1),
            tsy: Entity::from_raw(2),
        };
        let _app = run_apply(layers, &[]);
        // Reaching here without panic is the success signal.
    }
}
