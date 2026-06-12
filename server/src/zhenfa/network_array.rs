use valence::prelude::Entity;

pub const NETWORK_ARRAY_MIN_FLAGS: usize = 3;
pub const NETWORK_ARRAY_MAX_FLAGS: usize = 4;
pub const NETWORK_ARRAY_MAX_AREA: f64 = 256.0;
pub const NETWORK_ARRAY_EYE_FLAG_MAX_DISTANCE: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkFlag {
    pub instance_id: u64,
    pub owner: Entity,
    pub pos: [i32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkArrayGeometry {
    pub flag_instance_ids: Vec<u64>,
    pub flag_positions: Vec<[i32; 3]>,
    pub hull: Vec<[i32; 3]>,
    pub area: f64,
}

pub fn try_form_network(
    eye_pos: [i32; 3],
    owner: Entity,
    flags: &[NetworkFlag],
    max_area: f64,
    eye_flag_max_dist: f64,
) -> Option<NetworkArrayGeometry> {
    let mut candidates = flags
        .iter()
        .copied()
        .filter(|flag| flag.owner == owner)
        .filter(|flag| horizontal_distance(eye_pos, flag.pos) <= eye_flag_max_dist)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        horizontal_distance_sq(eye_pos, left.pos)
            .cmp(&horizontal_distance_sq(eye_pos, right.pos))
            .then(left.instance_id.cmp(&right.instance_id))
    });

    if candidates.len() < NETWORK_ARRAY_MIN_FLAGS {
        return None;
    }

    if candidates.len() > NETWORK_ARRAY_MAX_FLAGS {
        candidates.truncate(NETWORK_ARRAY_MAX_FLAGS);
    }

    geometry_for_choice(eye_pos, &candidates, max_area)
}

pub fn point_inside_hull_xz(pos: [i32; 3], hull: &[[i32; 3]]) -> bool {
    point_inside_hull_xz_f64(f64::from(pos[0]) + 0.5, f64::from(pos[2]) + 0.5, hull)
}

pub fn point_inside_hull_xz_f64(x: f64, z: f64, hull: &[[i32; 3]]) -> bool {
    if hull.len() < NETWORK_ARRAY_MIN_FLAGS {
        return false;
    }
    let mut inside = false;
    let mut previous = hull.len() - 1;
    for current in 0..hull.len() {
        let xi = f64::from(hull[current][0]);
        let zi = f64::from(hull[current][2]);
        let xj = f64::from(hull[previous][0]);
        let zj = f64::from(hull[previous][2]);
        if point_on_segment(x, z, xi, zi, xj, zj) {
            return true;
        }
        let crosses = (zi > z) != (zj > z);
        if crosses {
            let x_intersect = (xj - xi) * (z - zi) / (zj - zi) + xi;
            if x <= x_intersect {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

fn geometry_for_choice(
    eye_pos: [i32; 3],
    choice: &[NetworkFlag],
    max_area: f64,
) -> Option<NetworkArrayGeometry> {
    let flag_positions = choice.iter().map(|flag| flag.pos).collect::<Vec<_>>();
    let hull = convex_hull_xz(&flag_positions);
    if hull.len() < NETWORK_ARRAY_MIN_FLAGS {
        return None;
    }
    let area = polygon_area_xz(&hull);
    if area <= 0.0 || area > max_area + f64::EPSILON {
        return None;
    }
    if !point_inside_hull_xz(eye_pos, &hull) {
        return None;
    }
    Some(NetworkArrayGeometry {
        flag_instance_ids: choice.iter().map(|flag| flag.instance_id).collect(),
        flag_positions,
        hull,
        area,
    })
}

fn convex_hull_xz(points: &[[i32; 3]]) -> Vec<[i32; 3]> {
    let mut unique = points.to_vec();
    unique.sort_by_key(|pos| (pos[0], pos[2], pos[1]));
    unique.dedup_by_key(|pos| (pos[0], pos[2]));
    if unique.len() <= 1 {
        return unique;
    }

    let mut lower = Vec::new();
    for point in &unique {
        while lower.len() >= 2
            && cross_xz(lower[lower.len() - 2], lower[lower.len() - 1], *point) <= 0
        {
            lower.pop();
        }
        lower.push(*point);
    }

    let mut upper = Vec::new();
    for point in unique.iter().rev() {
        while upper.len() >= 2
            && cross_xz(upper[upper.len() - 2], upper[upper.len() - 1], *point) <= 0
        {
            upper.pop();
        }
        upper.push(*point);
    }

    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn polygon_area_xz(points: &[[i32; 3]]) -> f64 {
    if points.len() < NETWORK_ARRAY_MIN_FLAGS {
        return 0.0;
    }
    let mut area = 0.0;
    for (current, next) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        area += f64::from(current[0] * next[2] - next[0] * current[2]);
    }
    area.abs() * 0.5
}

fn cross_xz(origin: [i32; 3], left: [i32; 3], right: [i32; 3]) -> i64 {
    let ax = i64::from(left[0] - origin[0]);
    let az = i64::from(left[2] - origin[2]);
    let bx = i64::from(right[0] - origin[0]);
    let bz = i64::from(right[2] - origin[2]);
    ax * bz - az * bx
}

fn point_on_segment(x: f64, z: f64, ax: f64, az: f64, bx: f64, bz: f64) -> bool {
    let cross = (x - ax) * (bz - az) - (z - az) * (bx - ax);
    if cross.abs() > 1e-9 {
        return false;
    }
    x >= ax.min(bx) - 1e-9
        && x <= ax.max(bx) + 1e-9
        && z >= az.min(bz) - 1e-9
        && z <= az.max(bz) + 1e-9
}

fn horizontal_distance(left: [i32; 3], right: [i32; 3]) -> f64 {
    (horizontal_distance_sq(left, right) as f64).sqrt()
}

fn horizontal_distance_sq(left: [i32; 3], right: [i32; 3]) -> i32 {
    let dx = left[0] - right[0];
    let dz = left[2] - right[2];
    dx * dx + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(instance_id: u64, owner: Entity, x: i32, z: i32) -> NetworkFlag {
        NetworkFlag {
            instance_id,
            owner,
            pos: [x, 64, z],
        }
    }

    #[test]
    fn three_flags_and_eye_inside_form_network() {
        let owner = Entity::from_raw(1);
        let flags = vec![
            flag(1, owner, 0, 0),
            flag(2, owner, 6, 0),
            flag(3, owner, 0, 6),
        ];

        let network = try_form_network([1, 64, 1], owner, &flags, 256.0, 12.0)
            .expect("三旗 + 圈内阵眼应成阵");

        assert_eq!(network.flag_instance_ids, vec![1, 2, 3]);
        assert!((network.area - 18.0).abs() < 1e-9);
    }

    #[test]
    fn two_flags_do_not_form_network() {
        let owner = Entity::from_raw(1);
        let flags = vec![flag(1, owner, 0, 0), flag(2, owner, 6, 0)];

        assert!(
            try_form_network([1, 64, 1], owner, &flags, 256.0, 12.0).is_none(),
            "两旗低于凸多边形下限，必须拒绝"
        );
    }

    #[test]
    fn area_boundary_accepts_16_by_16_and_rejects_overflow() {
        let owner = Entity::from_raw(1);
        let accepted = vec![
            flag(1, owner, 0, 0),
            flag(2, owner, 16, 0),
            flag(3, owner, 16, 16),
            flag(4, owner, 0, 16),
        ];
        assert!(
            try_form_network([8, 64, 8], owner, &accepted, 256.0, 12.0).is_some(),
            "恰好 16×16=256 格² 应接受"
        );

        let rejected = vec![
            flag(1, owner, 0, 0),
            flag(2, owner, 17, 0),
            flag(3, owner, 17, 16),
            flag(4, owner, 0, 16),
        ];
        assert!(
            try_form_network([8, 64, 8], owner, &rejected, 256.0, 12.1).is_none(),
            "超过最大面积 1 格宽必须拒绝"
        );
    }

    #[test]
    fn eye_flag_max_distance_filters_flags() {
        let owner = Entity::from_raw(1);
        let flags = vec![
            flag(1, owner, 0, 0),
            flag(2, owner, 6, 0),
            flag(3, owner, 0, 6),
        ];

        assert!(
            try_form_network([1, 64, 1], owner, &flags, 256.0, 4.0).is_none(),
            "超过 eye_flag_max_dist 的旗不计入，剩余不足三旗时不成阵"
        );
    }

    #[test]
    fn point_inside_and_outside_hull_are_distinguished() {
        let hull = vec![[0, 64, 0], [6, 64, 0], [6, 64, 6], [0, 64, 6]];

        assert!(point_inside_hull_xz([3, 64, 3], &hull));
        assert!(!point_inside_hull_xz([7, 64, 3], &hull));
    }

    #[test]
    fn concave_layout_uses_convex_hull() {
        let owner = Entity::from_raw(1);
        let flags = vec![
            flag(1, owner, 0, 0),
            flag(2, owner, 8, 0),
            flag(3, owner, 4, 2),
            flag(4, owner, 0, 8),
        ];

        let network =
            try_form_network([4, 64, 3], owner, &flags, 256.0, 12.0).expect("凹布局应取凸包后判定");

        assert_eq!(network.hull.len(), 3, "内凹点应被凸包吞掉");
        assert!(point_inside_hull_xz([4, 64, 3], &network.hull));
    }
}
