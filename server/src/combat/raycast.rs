use valence::prelude::DVec3;

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
}
