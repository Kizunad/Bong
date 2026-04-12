use crate::npc::spawn::NpcMarker;
use bevy_transform::components::Transform;
use valence::prelude::{App, Position, PostUpdate, Query, With};

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering sync system");
    app.add_systems(PostUpdate, sync_position_to_transform);
}

/// One-way sync: Position → Transform.
///
/// Facing (Look / HeadYaw) is now managed by the Navigator system,
/// which sets them directly when advancing along a path.
fn sync_position_to_transform(mut npc_query: Query<(&Position, &mut Transform), With<NpcMarker>>) {
    for (position, mut transform) in &mut npc_query {
        let pos = position.get();
        transform.translation.x = pos.x as f32;
        transform.translation.y = pos.y as f32;
        transform.translation.z = pos.z as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::spawn::NpcMarker;
    use bevy_transform::components::Transform;
    use valence::prelude::{App, DVec3, Position};

    #[test]
    fn position_to_transform_sync_is_one_way() {
        let mut app = App::new();
        app.add_systems(PostUpdate, sync_position_to_transform);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([14.0, 66.0, 14.0]),
                Transform::from_xyz(99.0, 1.0, 99.0),
            ))
            .id();

        app.update();

        {
            let transform = app
                .world_mut()
                .get_mut::<Transform>(npc)
                .expect("Transform should exist");
            assert_eq!(transform.translation.x, 14.0);
            assert_eq!(transform.translation.y, 66.0);
            assert_eq!(transform.translation.z, 14.0);
        }

        {
            let mut transform = app
                .world_mut()
                .get_mut::<Transform>(npc)
                .expect("Transform should exist");
            transform.translation.x = 200.0;
            transform.translation.y = 1.0;
            transform.translation.z = 210.0;
        }

        let position_after_transform_change = app
            .world()
            .get::<Position>(npc)
            .expect("Position should exist")
            .get();
        assert_eq!(
            position_after_transform_change,
            DVec3::new(14.0, 66.0, 14.0)
        );

        app.update();

        let position_after_second_tick = app
            .world()
            .get::<Position>(npc)
            .expect("Position should still exist")
            .get();
        let transform_after_second_tick = app
            .world()
            .get::<Transform>(npc)
            .expect("Transform should still exist");

        assert_eq!(position_after_second_tick, DVec3::new(14.0, 66.0, 14.0));
        assert_eq!(transform_after_second_tick.translation.x, 14.0);
        assert_eq!(transform_after_second_tick.translation.y, 66.0);
        assert_eq!(transform_after_second_tick.translation.z, 14.0);
    }
}
