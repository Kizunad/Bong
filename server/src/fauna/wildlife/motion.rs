//! 短位移逐段扫掠；未知区块按阻挡处理。击退拥有更高的位置写入优先级。

use bevy_transform::components::Transform;
use valence::prelude::*;

use super::brain::WildlifeBrain;
use super::skills::{self, WildlifeCast, WildlifeSkill};
use crate::combat::components::DerivedAttrs;
use crate::npc::movement::{sweep_grounded_motion, MovementController, MovementMode};

fn clear_body(layer: &ChunkLayer, position: DVec3) -> bool {
    for x in [-0.55, 0.55] {
        for z in [-0.55, 0.55] {
            for y in [0.1, 0.9, 1.5] {
                let point = position + DVec3::new(x, y, z);
                let block = BlockPos::new(
                    point.x.floor() as i32,
                    point.y.floor() as i32,
                    point.z.floor() as i32,
                );
                if !layer.block(block).is_some_and(|b| b.state.is_air()) {
                    return false;
                }
            }
        }
    }
    true
}

pub(crate) fn sweep_flight(layer: &ChunkLayer, start: DVec3, destination: DVec3) -> DVec3 {
    let delta = destination - start;
    let steps = (delta.length() / 0.25).ceil().max(1.0) as usize;
    let mut result = start;
    for step in 1..=steps {
        let point = start + delta * (step as f64 / steps as f64);
        if !clear_body(layer, point) {
            break;
        }
        result = point;
    }
    result
}

pub fn move_wildlife(world: &mut bevy_ecs::world::World) {
    let tick = skills::now(world);
    let actors: Vec<_> = world
        .query::<(
            Entity,
            &WildlifeBrain,
            &Position,
            &EntityLayerId,
            &MovementController,
        )>()
        .iter(world)
        .map(|(id, brain, p, layer, movement)| {
            (id, brain.clone(), p.get(), layer.0, movement.clone())
        })
        .collect();
    for (entity, brain, start, layer_id, controller) in actors {
        if matches!(controller.mode, MovementMode::Override(_)) {
            continue;
        }
        let cast = world.get::<WildlifeCast>(entity).cloned();
        let controlled = brain.airborne || cast.is_some() || skills::interrupted(world, entity);
        world
            .get_mut::<MovementController>(entity)
            .expect("移动底盘")
            .mode = if controlled {
            MovementMode::Wildlife
        } else {
            MovementMode::GroundNav
        };
        if skills::interrupted(world, entity) {
            continue;
        }
        let scale = world
            .get::<DerivedAttrs>(entity)
            .map(|attrs| f64::from(attrs.move_speed_multiplier))
            .unwrap_or(1.0)
            .clamp(0.0, 2.0);
        let Some(layer) = world.get::<ChunkLayer>(layer_id) else {
            continue;
        };
        let next = if let Some(cast) = cast.as_ref() {
            if tick < cast.hit_at
                || tick > cast.active_until
                || !skills::channels_ready(world, entity, cast.skill.id())
            {
                continue;
            }
            let speed = match cast.skill {
                WildlifeSkill::Pounce if cast.submitted_at.is_some() => 0.0,
                WildlifeSkill::Pounce => 0.65,
                WildlifeSkill::Trample => 0.60,
                WildlifeSkill::Dive => 0.70,
                WildlifeSkill::Rend | WildlifeSkill::Kick => 0.0,
            } * scale;
            if cast.skill == WildlifeSkill::Dive {
                sweep_flight(layer, start, start + cast.direction * speed)
            } else {
                sweep_grounded_motion(start, cast.direction, speed, start.y, Some(layer)).position
            }
        } else if brain.airborne && tick >= brain.flight_ready_at {
            let delta = brain.flight_goal - start;
            sweep_flight(
                layer,
                start,
                start + delta.normalize_or_zero() * delta.length().min(0.28 * scale),
            )
        } else {
            continue;
        };
        world
            .get_mut::<Position>(entity)
            .expect("生物位置")
            .set(next);
        if let Some(mut transform) = world.get_mut::<Transform>(entity) {
            transform.translation.x = next.x as f32;
            transform.translation.y = next.y as f32;
            transform.translation.z = next.z as f32;
        }
        let delta = next - start;
        if delta.length_squared() > 0.0001 {
            let yaw = (-delta.x).atan2(delta.z).to_degrees() as f32;
            if let Some(mut look) = world.get_mut::<Look>(entity) {
                look.yaw = yaw;
            }
            if let Some(mut head) = world.get_mut::<HeadYaw>(entity) {
                head.0 = yaw;
            }
        }
    }
}
