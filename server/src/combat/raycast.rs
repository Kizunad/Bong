#![allow(dead_code)]

use valence::prelude::DVec3;

use super::components::BodyPart;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaycastHit {
    pub distance: f64,
    pub point: DVec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityHitProbe {
    pub distance: f64,
    pub point: DVec3,
    pub body_part: BodyPart,
}

const STANDING_HALF_WIDTH: f64 = 0.3;
const STANDING_HEIGHT: f64 = 1.8;
const CHEST_AIM_HEIGHT: f64 = 1.2;

pub fn standing_humanoid_aabb(feet_position: DVec3) -> Aabb {
    Aabb {
        min: DVec3::new(
            feet_position.x - STANDING_HALF_WIDTH,
            feet_position.y,
            feet_position.z - STANDING_HALF_WIDTH,
        ),
        max: DVec3::new(
            feet_position.x + STANDING_HALF_WIDTH,
            feet_position.y + STANDING_HEIGHT,
            feet_position.z + STANDING_HALF_WIDTH,
        ),
    }
}

pub fn classify_body_part(
    hit_point: DVec3,
    target_feet_position: DVec3,
    attack_origin: DVec3,
) -> BodyPart {
    let rel_y = ((hit_point.y - target_feet_position.y) / STANDING_HEIGHT).clamp(0.0, 1.0);
    let attack_dir = DVec3::new(
        hit_point.x - attack_origin.x,
        0.0,
        hit_point.z - attack_origin.z,
    );
    let lateral = if attack_dir.length_squared() <= f64::EPSILON {
        hit_point.z - target_feet_position.z
    } else {
        let dir = attack_dir.normalize();
        let perpendicular = DVec3::new(-dir.z, 0.0, dir.x);
        let relative = DVec3::new(
            hit_point.x - target_feet_position.x,
            0.0,
            hit_point.z - target_feet_position.z,
        );
        relative.dot(perpendicular)
    };

    if rel_y > 0.88 {
        BodyPart::Head
    } else if rel_y > 0.55 {
        if lateral.abs() > 0.18 {
            if lateral > 0.0 {
                BodyPart::ArmR
            } else {
                BodyPart::ArmL
            }
        } else {
            BodyPart::Chest
        }
    } else if rel_y > 0.35 {
        BodyPart::Abdomen
    } else if lateral > 0.0 {
        BodyPart::LegR
    } else {
        BodyPart::LegL
    }
}

pub fn raycast_humanoid(
    origin: DVec3,
    target_feet_position: DVec3,
    max_distance: f64,
) -> Option<EntityHitProbe> {
    let aabb = standing_humanoid_aabb(target_feet_position);
    let fallback_aim = DVec3::new(
        (aabb.min.x + aabb.max.x) * 0.5,
        target_feet_position.y + CHEST_AIM_HEIGHT,
        (aabb.min.z + aabb.max.z) * 0.5,
    ) - origin;
    let hit = raycast_aabb(origin, fallback_aim, max_distance, aabb)?;

    Some(EntityHitProbe {
        distance: hit.distance,
        point: hit.point,
        body_part: classify_body_part(hit.point, target_feet_position, origin),
    })
}
pub fn raycast_aabb(
    origin: DVec3,
    direction: DVec3,
    max_distance: f64,
    aabb: Aabb,
) -> Option<RaycastHit> {
    if max_distance <= 0.0 {
        return None;
    }

    let direction_len = direction.length();
    if direction_len <= f64::EPSILON {
        return None;
    }

    let dir = direction / direction_len;
    let mut t_min = 0.0_f64;
    let mut t_max = max_distance;

    if !slab_intersection(
        origin.x, dir.x, aabb.min.x, aabb.max.x, &mut t_min, &mut t_max,
    ) {
        return None;
    }
    if !slab_intersection(
        origin.y, dir.y, aabb.min.y, aabb.max.y, &mut t_min, &mut t_max,
    ) {
        return None;
    }
    if !slab_intersection(
        origin.z, dir.z, aabb.min.z, aabb.max.z, &mut t_min, &mut t_max,
    ) {
        return None;
    }

    if t_min > max_distance {
        return None;
    }

    let distance = t_min.max(0.0);
    if distance > max_distance {
        return None;
    }

    Some(RaycastHit {
        distance,
        point: origin + dir * distance,
    })
}

fn slab_intersection(
    origin_axis: f64,
    direction_axis: f64,
    slab_min: f64,
    slab_max: f64,
    t_min: &mut f64,
    t_max: &mut f64,
) -> bool {
    if direction_axis.abs() <= f64::EPSILON {
        return origin_axis >= slab_min && origin_axis <= slab_max;
    }

    let inv = 1.0 / direction_axis;
    let mut t1 = (slab_min - origin_axis) * inv;
    let mut t2 = (slab_max - origin_axis) * inv;

    if t1 > t2 {
        std::mem::swap(&mut t1, &mut t2);
    }

    *t_min = (*t_min).max(t1);
    *t_max = (*t_max).min(t2);

    *t_min <= *t_max
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_box() -> Aabb {
        Aabb {
            min: DVec3::new(2.0, -1.0, -1.0),
            max: DVec3::new(4.0, 1.0, 1.0),
        }
    }

    #[test]
    fn raycast_hits_when_box_is_in_front() {
        let hit = raycast_aabb(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            10.0,
            test_box(),
        )
        .expect("ray should hit front-facing box");

        assert!((hit.distance - 2.0).abs() < 1e-9);
        assert!((hit.point.x - 2.0).abs() < 1e-9);
    }

    #[test]
    fn raycast_returns_none_for_no_intersection() {
        let hit = raycast_aabb(
            DVec3::new(0.0, 5.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            10.0,
            test_box(),
        );

        assert!(hit.is_none());
    }

    #[test]
    fn raycast_returns_none_when_hit_is_out_of_range() {
        let hit = raycast_aabb(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            1.5,
            test_box(),
        );

        assert!(hit.is_none());
    }

    #[test]
    fn raycast_humanoid_hits_chest_from_front() {
        let probe = raycast_humanoid(DVec3::new(0.0, 0.9, 0.0), DVec3::new(2.0, 0.0, 0.0), 3.0)
            .expect("front ray should hit humanoid");

        assert_eq!(probe.body_part, BodyPart::Chest);
        assert!(probe.distance <= 3.0);
    }

    #[test]
    fn classify_body_part_maps_y_and_lateral_ranges() {
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(-2.0, 1.0, 0.0);

        assert_eq!(
            classify_body_part(DVec3::new(0.0, 1.7, 0.0), feet, origin),
            BodyPart::Head
        );
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 1.2, 0.0), feet, origin),
            BodyPart::Chest
        );
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 1.2, 0.25), feet, origin),
            BodyPart::ArmR
        );
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 1.2, -0.25), feet, origin),
            BodyPart::ArmL
        );
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 0.7, 0.0), feet, origin),
            BodyPart::Abdomen
        );
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 0.2, 0.2), feet, origin),
            BodyPart::LegR
        );
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 0.2, -0.2), feet, origin),
            BodyPart::LegL
        );
    }
}
