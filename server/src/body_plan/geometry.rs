//! plan-race-system-v1 P0 — 命中几何的纯几何函数：`HeightBands`（人形高度带分类）与
//! `PartBoxes`（非人形局部盒求交）两套模式各自的求值逻辑。
//!
//! 本文件**不**改动 `combat::raycast` 任何行为——`classify_height_bands` 是
//! `combat::raycast::classify_body_part` 的数据驱动参数化重实现，两者在
//! `registry.rs`/独立测试中做 bit-for-bit 批量对拍（见本文件测试
//! `classify_height_bands_matches_legacy_classify_body_part_for_humanoid_bands`），
//! 但生产 `combat::raycast` 消费点的改造（真正调用本模块）留给后续阶段。
//!
//! 坐标系约定（`PartBoxes` 专用，P0 只支持 yaw，不支持 pitch/roll）：
//! - 局部系原点 = 实体位置（`entity_position_world`）
//! - 局部 +Z = 实体 yaw 正前方；`entity_yaw_radians = 0` 时局部系与世界系重合
//! - 世界→局部：先平移（减去实体位置，仅对"点"生效，方向向量跳过平移）再绕 Y 轴
//!   旋转 `-yaw`

use valence::prelude::DVec3;

use super::types::{HeightBandAssignment, PartBox};

#[derive(Debug, Clone, Copy, PartialEq)]
struct LocalAabb {
    min: DVec3,
    max: DVec3,
}

/// `HeightBands` 模式分类：与 `combat::raycast::classify_body_part` 同构的算法，
/// 参数从硬编码常量搬进 `bands`/`lateral_threshold` 数据。`bands` 必须已通过
/// `validate_body_plan`（严格降序 + 最低带 `min_rel_y < 0.0` 全覆盖）——满足该前提时
/// 本函数永不返回 `None`；未满足时返回 `None`（防御性,不 panic）。
pub fn classify_height_bands(
    hit_point: DVec3,
    target_feet_position: DVec3,
    attack_origin: DVec3,
    height: f64,
    bands: &[super::types::HeightBand],
    lateral_threshold: f64,
) -> Option<&super::types::BodyPartId> {
    let rel_y = ((hit_point.y - target_feet_position.y) / height).clamp(0.0, 1.0);
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

    for band in bands {
        if rel_y > band.min_rel_y {
            return Some(resolve_band_assignment(
                &band.assignment,
                lateral,
                lateral_threshold,
            ));
        }
    }
    None
}

fn resolve_band_assignment(
    assignment: &HeightBandAssignment,
    lateral: f64,
    lateral_threshold: f64,
) -> &super::types::BodyPartId {
    match assignment {
        HeightBandAssignment::Single { part } => part,
        HeightBandAssignment::LateralSplitWithCenter {
            left,
            right,
            center,
        } => {
            if lateral.abs() > lateral_threshold {
                if lateral > 0.0 {
                    right
                } else {
                    left
                }
            } else {
                center
            }
        }
        HeightBandAssignment::LateralSplit { left, right } => {
            if lateral > 0.0 {
                right
            } else {
                left
            }
        }
    }
}

/// `PartBoxes` 模式的命中结果——只暴露对外契约（部位 id + 距离），不绑定内部遍历顺序。
#[derive(Debug, Clone, PartialEq)]
pub struct PartBoxHit {
    pub part_id: super::types::BodyPartId,
    pub distance: f64,
}

/// 世界坐标点/方向 → 实体局部系（仅 yaw 旋转）。
fn rotate_world_to_local(v: DVec3, yaw_radians: f64) -> DVec3 {
    let (sin_y, cos_y) = yaw_radians.sin_cos();
    DVec3::new(v.x * cos_y - v.z * sin_y, v.y, v.x * sin_y + v.z * cos_y)
}

fn world_point_to_local(world_point: DVec3, entity_position: DVec3, yaw_radians: f64) -> DVec3 {
    rotate_world_to_local(world_point - entity_position, yaw_radians)
}

fn world_direction_to_local(world_direction: DVec3, yaw_radians: f64) -> DVec3 {
    rotate_world_to_local(world_direction, yaw_radians)
}

/// 局部系下的 slab ray-AABB 求交（复刻 `combat::raycast::raycast_aabb` 的算法，独立
/// 实现以保持 `body_plan` 对 `combat` 零依赖——P0b 起 `combat::raycast` 会反过来调用
/// 本模块，届时可评估是否收敛成单一实现）。
fn intersect_ray_aabb(
    origin: DVec3,
    direction: DVec3,
    max_distance: f64,
    aabb: LocalAabb,
) -> Option<(f64, DVec3)> {
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

    if !slab(
        origin.x, dir.x, aabb.min.x, aabb.max.x, &mut t_min, &mut t_max,
    ) {
        return None;
    }
    if !slab(
        origin.y, dir.y, aabb.min.y, aabb.max.y, &mut t_min, &mut t_max,
    ) {
        return None;
    }
    if !slab(
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
    Some((distance, origin + dir * distance))
}

fn slab(
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

/// `PartBoxes` 模式求交：世界系射线 → 局部系（仅 yaw）→ 逐盒求交 → 取最近命中；
/// 等距按 `priority`（越大越优先）裁决，再等则按声明顺序（数组下标）稳定裁决。
/// 空集合 / 零长方向 / `max_distance <= 0` 均返回 `None`。
pub fn raycast_part_boxes(
    ray_origin_world: DVec3,
    ray_direction_world: DVec3,
    max_distance: f64,
    entity_position_world: DVec3,
    entity_yaw_radians: f64,
    boxes: &[PartBox],
) -> Option<PartBoxHit> {
    if max_distance <= 0.0 || boxes.is_empty() {
        return None;
    }
    if ray_direction_world.length() <= f64::EPSILON {
        return None;
    }

    let local_origin =
        world_point_to_local(ray_origin_world, entity_position_world, entity_yaw_radians);
    let local_direction = world_direction_to_local(ray_direction_world, entity_yaw_radians);

    let mut best: Option<(f64, i32, usize)> = None;
    for (index, part_box) in boxes.iter().enumerate() {
        let aabb = LocalAabb {
            min: DVec3::new(
                part_box.offset[0] - part_box.half_extents[0],
                part_box.offset[1] - part_box.half_extents[1],
                part_box.offset[2] - part_box.half_extents[2],
            ),
            max: DVec3::new(
                part_box.offset[0] + part_box.half_extents[0],
                part_box.offset[1] + part_box.half_extents[1],
                part_box.offset[2] + part_box.half_extents[2],
            ),
        };
        let Some((distance, _point)) =
            intersect_ray_aabb(local_origin, local_direction, max_distance, aabb)
        else {
            continue;
        };
        let candidate = (distance, part_box.priority, index);
        best = Some(match best {
            None => candidate,
            Some(current) => pick_better(current, candidate),
        });
    }

    best.map(|(distance, _priority, index)| PartBoxHit {
        part_id: boxes[index].part_id.clone(),
        distance,
    })
}

/// 距离越小越好；等距时 priority 越大越好；再等则原始下标越小越好（稳定序，先声明者赢）。
fn pick_better(a: (f64, i32, usize), b: (f64, i32, usize)) -> (f64, i32, usize) {
    use std::cmp::Ordering;
    match a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal) {
        Ordering::Less => a,
        Ordering::Greater => b,
        Ordering::Equal => {
            if a.1 != b.1 {
                if a.1 > b.1 {
                    a
                } else {
                    b
                }
            } else if a.2 <= b.2 {
                a
            } else {
                b
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::types::{BodyPartId, HeightBand};

    // ───────────────────────── classify_height_bands ─────────────────────────

    /// humanoid.json 校准值的独立复刻（不 import 资产文件，避免测试对文件系统布局
    /// 产生隐式依赖；数值来源见 `combat/raycast.rs` `ARM_LATERAL_THRESHOLD` /
    /// `LEG_ABDOMEN_BOUNDARY` / 0.88 头部阈值 / 0.55 胸臂分界）。
    fn humanoid_bands() -> Vec<HeightBand> {
        use crate::body_plan::types::HeightBandAssignment as A;
        vec![
            HeightBand {
                min_rel_y: 0.88,
                assignment: A::Single {
                    part: "head".into(),
                },
            },
            HeightBand {
                min_rel_y: 0.55,
                assignment: A::LateralSplitWithCenter {
                    left: "arm_l".into(),
                    right: "arm_r".into(),
                    center: "chest".into(),
                },
            },
            HeightBand {
                min_rel_y: 0.53,
                assignment: A::Single {
                    part: "abdomen".into(),
                },
            },
            HeightBand {
                min_rel_y: -1.0,
                assignment: A::LateralSplit {
                    left: "leg_l".into(),
                    right: "leg_r".into(),
                },
            },
        ]
    }

    const HUMANOID_HEIGHT: f64 = 1.8;
    const HUMANOID_LATERAL_THRESHOLD: f64 = 0.19;

    #[test]
    fn classify_matches_legacy_head_from_elevated_pitch() {
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(0.0, -3.0, -2.0);
        let hit_point = DVec3::new(0.0, 1.62, 0.0);
        let bands = humanoid_bands();
        let part = classify_height_bands(
            hit_point,
            feet,
            origin,
            HUMANOID_HEIGHT,
            &bands,
            HUMANOID_LATERAL_THRESHOLD,
        )
        .expect("must classify");
        assert_eq!(part, &BodyPartId::new("head"));
    }

    #[test]
    fn classify_matches_legacy_leg_from_depressed_pitch() {
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(0.0, 3.0, -2.0);
        let hit_point = DVec3::new(-0.15, 0.15, 0.1);
        let bands = humanoid_bands();
        let part = classify_height_bands(
            hit_point,
            feet,
            origin,
            HUMANOID_HEIGHT,
            &bands,
            HUMANOID_LATERAL_THRESHOLD,
        )
        .expect("must classify");
        assert_eq!(part, &BodyPartId::new("leg_r"));
    }

    #[test]
    fn classify_matches_legacy_arm_from_lateral_yaw() {
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(-2.0, 1.2, 0.0);
        let hit_point = DVec3::new(0.0, 1.2, 0.3);
        let bands = humanoid_bands();
        let part = classify_height_bands(
            hit_point,
            feet,
            origin,
            HUMANOID_HEIGHT,
            &bands,
            HUMANOID_LATERAL_THRESHOLD,
        )
        .expect("must classify");
        assert_eq!(part, &BodyPartId::new("arm_r"));
    }

    #[test]
    fn classify_maps_y_and_lateral_ranges_matches_legacy_table() {
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(-2.0, 1.0, 0.0);
        let bands = humanoid_bands();
        let classify = |hit: DVec3| {
            classify_height_bands(
                hit,
                feet,
                origin,
                HUMANOID_HEIGHT,
                &bands,
                HUMANOID_LATERAL_THRESHOLD,
            )
            .expect("must classify")
            .clone()
        };

        assert_eq!(classify(DVec3::new(0.0, 1.7, 0.0)), BodyPartId::new("head"));
        assert_eq!(
            classify(DVec3::new(0.0, 1.2, 0.0)),
            BodyPartId::new("chest")
        );
        assert_eq!(
            classify(DVec3::new(0.0, 1.2, 0.25)),
            BodyPartId::new("arm_r")
        );
        assert_eq!(
            classify(DVec3::new(0.0, 1.2, -0.25)),
            BodyPartId::new("arm_l")
        );
        assert_eq!(
            classify(DVec3::new(0.0, 0.97, 0.0)),
            BodyPartId::new("abdomen"),
            "rel_y≈0.539 应落在窄 Abdomen 区间 (0.53,0.55]"
        );
        assert_eq!(
            classify(DVec3::new(0.0, 0.7, 0.0)),
            BodyPartId::new("leg_l"),
            "rel_y≈0.389 应改判 Leg（P1 校准把 Abdomen 收窄到 0.53 以上）"
        );
        assert_eq!(
            classify(DVec3::new(0.0, 0.2, 0.2)),
            BodyPartId::new("leg_r")
        );
        assert_eq!(
            classify(DVec3::new(0.0, 0.2, -0.2)),
            BodyPartId::new("leg_l")
        );
    }

    #[test]
    fn classify_arm_lateral_threshold_boundary() {
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(-2.0, 1.2, 0.0);
        let bands = humanoid_bands();
        let classify = |hit: DVec3| {
            classify_height_bands(
                hit,
                feet,
                origin,
                HUMANOID_HEIGHT,
                &bands,
                HUMANOID_LATERAL_THRESHOLD,
            )
            .expect("must classify")
            .clone()
        };
        assert_eq!(
            classify(DVec3::new(0.0, 1.2, 0.185)),
            BodyPartId::new("chest"),
            "lateral=0.185 未超过阈值 0.19，应判 chest"
        );
        assert_eq!(
            classify(DVec3::new(0.0, 1.2, 0.20)),
            BodyPartId::new("arm_r"),
            "lateral=0.20 超过阈值 0.19，应判 arm_r"
        );
    }

    #[test]
    fn classify_matches_legacy_across_batch_of_npc_aim_samples() {
        // bit-for-bit 行为对拍：复用 combat::raycast 的确定性 NPC 瞄准分布（同一批
        // hit_point/feet/origin 三元组）喂给数据驱动的 classify_height_bands，逐样本
        // 断言与硬编码 classify_body_part 完全一致——这就是 P0 交付物要求的
        // 「raycast 分类回归（P1 直方图样本重跑）」。
        use crate::combat::components::BodyPart;
        use crate::combat::raycast::{
            classify_body_part, npc_aim_direction, npc_aim_seed, raycast_humanoid,
        };

        fn legacy_part_to_id(part: BodyPart) -> BodyPartId {
            BodyPartId::new(match part {
                BodyPart::Head => "head",
                BodyPart::Chest => "chest",
                BodyPart::Back => "back",
                BodyPart::Abdomen => "abdomen",
                BodyPart::ArmL => "arm_l",
                BodyPart::ArmR => "arm_r",
                BodyPart::LegL => "leg_l",
                BodyPart::LegR => "leg_r",
            })
        }

        let origin = DVec3::new(0.0, 1.62, -2.0);
        let target = DVec3::new(0.0, 0.0, 0.0);
        let bands = humanoid_bands();
        let mut compared = 0u32;

        for tick in 0..3000u64 {
            let seed = npc_aim_seed("npc:body_plan_geometry_parity", tick);
            let aim_direction = npc_aim_direction(origin, target, seed, 1.0);
            let Some(probe) = raycast_humanoid(origin, target, 5.0, aim_direction) else {
                continue;
            };
            // classify_body_part 内部对同一 hit point 独立重算，双方喂同一批
            // (hit_point, feet, origin) 三元组以保证严格 apples-to-apples。
            let legacy = classify_body_part(probe.point, target, origin);
            let data_driven = classify_height_bands(
                probe.point,
                target,
                origin,
                HUMANOID_HEIGHT,
                &bands,
                HUMANOID_LATERAL_THRESHOLD,
            )
            .unwrap_or_else(|| {
                panic!("tick {tick}: validated humanoid bands must cover full [0,1] rel_y range")
            });
            assert_eq!(
                data_driven,
                &legacy_part_to_id(legacy),
                "tick {tick}: classify_height_bands({data_driven:?}) 与 legacy classify_body_part \
                 ({legacy:?}) 不一致 — bit-for-bit 行为对拍失败"
            );
            compared += 1;
        }

        assert!(
            compared > 2500,
            "批量对拍样本数过少（{compared}），jitter 分布异常导致大量射线脱靶"
        );
    }

    // ───────────────────────── raycast_part_boxes ─────────────────────────

    fn two_boxes() -> Vec<PartBox> {
        vec![
            PartBox {
                part_id: "near".into(),
                offset: [0.0, 0.0, 2.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
            PartBox {
                part_id: "far".into(),
                offset: [0.0, 0.0, 5.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
        ]
    }

    #[test]
    fn raycast_part_boxes_picks_nearest_hit() {
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &two_boxes(),
        )
        .expect("should hit the near box first");
        assert_eq!(hit.part_id, BodyPartId::new("near"));
        assert!((hit.distance - 1.5).abs() < 1e-9);
    }

    #[test]
    fn raycast_part_boxes_equal_distance_prefers_higher_priority() {
        // 两个盒沿 z 完全重叠（仅优先级不同），制造真正的等距命中场景。
        let overlapping = vec![
            PartBox {
                part_id: "low_priority".into(),
                offset: [0.0, 0.0, 2.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
            PartBox {
                part_id: "high_priority".into(),
                offset: [0.0, 0.0, 2.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 5,
            },
        ];
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &overlapping,
        )
        .expect("should hit");
        assert_eq!(hit.part_id, BodyPartId::new("high_priority"));
    }

    #[test]
    fn raycast_part_boxes_equal_distance_equal_priority_prefers_earlier_declaration() {
        let boxes = vec![
            PartBox {
                part_id: "first".into(),
                offset: [0.0, 0.0, 2.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
            PartBox {
                part_id: "second".into(),
                offset: [0.0, 0.0, 2.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
        ];
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        )
        .expect("should hit");
        assert_eq!(
            hit.part_id,
            BodyPartId::new("first"),
            "stable tie-break must prefer the earlier-declared box"
        );
    }

    #[test]
    fn raycast_part_boxes_origin_starts_inside_box() {
        let boxes = vec![PartBox {
            part_id: "enclosing".into(),
            offset: [0.0, 0.0, 0.0],
            half_extents: [1.0, 1.0, 1.0],
            priority: 0,
        }];
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        )
        .expect("origin inside box should still register a hit at distance 0");
        assert_eq!(hit.distance, 0.0);
    }

    #[test]
    fn raycast_part_boxes_grazing_boundary_hit_registers() {
        let boxes = vec![PartBox {
            part_id: "edge".into(),
            offset: [0.0, 0.0, 2.0],
            half_extents: [0.5, 0.5, 0.5],
            priority: 0,
        }];
        // 射线贴着盒子顶面（y = 0.5 边界）水平掠过。
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.5, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        );
        assert!(hit.is_some(), "边界擦触应仍判定命中（闭区间）");
    }

    #[test]
    fn raycast_part_boxes_parallel_ray_within_slab_still_hits() {
        let boxes = vec![PartBox {
            part_id: "wall".into(),
            offset: [0.0, 0.0, 2.0],
            half_extents: [0.5, 0.5, 0.5],
            priority: 0,
        }];
        // 方向没有 x 分量（与 x 轴平行的 slab 无关），origin.x=0 落在 [-0.5,0.5] 内。
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        );
        assert!(hit.is_some());
    }

    #[test]
    fn raycast_part_boxes_parallel_ray_outside_slab_misses() {
        let boxes = vec![PartBox {
            part_id: "wall".into(),
            offset: [0.0, 0.0, 2.0],
            half_extents: [0.5, 0.5, 0.5],
            priority: 0,
        }];
        // origin.x=2.0 在盒子的 x slab [-0.5,0.5] 之外，方向没有 x 分量无法进入 slab。
        let hit = raycast_part_boxes(
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn raycast_part_boxes_empty_slice_never_hits() {
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &[],
        );
        assert!(hit.is_none());
    }

    #[test]
    fn raycast_part_boxes_zero_length_direction_never_hits() {
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 0.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &two_boxes(),
        );
        assert!(hit.is_none());
    }

    #[test]
    fn raycast_part_boxes_non_positive_max_distance_never_hits() {
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            0.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &two_boxes(),
        );
        assert!(hit.is_none());
    }

    #[test]
    fn raycast_part_boxes_beyond_max_distance_misses() {
        let boxes = vec![PartBox {
            part_id: "far".into(),
            offset: [0.0, 0.0, 5.0],
            half_extents: [0.5, 0.5, 0.5],
            priority: 0,
        }];
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            2.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        );
        assert!(hit.is_none());
    }

    /// yaw 旋转 + 平移不变性：把「实体在原点不转向」的场景整体平移 + 绕 Y 轴旋转
    /// yaw 后，只要射线同步做相同的刚体变换，命中的 part_id + 局部系距离必须不变。
    #[test]
    fn raycast_part_boxes_yaw_and_translation_invariance() {
        let boxes = vec![
            PartBox {
                part_id: "front".into(),
                offset: [0.0, 0.0, 2.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
            PartBox {
                part_id: "right".into(),
                offset: [2.0, 0.0, 0.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
        ];

        // 基准：实体在原点，yaw=0，射线沿局部 +Z 打向 "front"。
        let baseline = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        )
        .expect("baseline should hit front");
        assert_eq!(baseline.part_id, BodyPartId::new("front"));

        for (yaw_degrees, entity_position) in [
            (0.0_f64, DVec3::new(10.0, 0.0, -5.0)),
            (90.0_f64, DVec3::new(-3.0, 0.0, 7.0)),
            (180.0_f64, DVec3::new(1.0, 0.0, 1.0)),
        ] {
            let yaw = yaw_degrees.to_radians();
            // 世界系下"实体局部 +Z 方向"随 yaw 旋转：local (0,0,1) -> world.
            let world_forward = DVec3::new(yaw.sin(), 0.0, yaw.cos());
            let ray_origin_world = entity_position; // 从实体位置本身发出，等价于局部原点。
            let ray_direction_world = world_forward;

            let hit = raycast_part_boxes(
                ray_origin_world,
                ray_direction_world,
                10.0,
                entity_position,
                yaw,
                &boxes,
            )
            .unwrap_or_else(|| panic!("yaw={yaw_degrees} translated case should still hit front"));

            assert_eq!(
                hit.part_id,
                BodyPartId::new("front"),
                "yaw={yaw_degrees} 平移+旋转后应仍命中 front（局部系不变性）"
            );
            assert!(
                (hit.distance - baseline.distance).abs() < 1e-9,
                "yaw={yaw_degrees} 距离应与基准一致：baseline={} got={}",
                baseline.distance,
                hit.distance
            );
        }
    }
}
