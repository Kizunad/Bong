//! plan-race-system-v1 P0 — 命中几何的纯几何函数：`HeightBands`（人形高度带分类）与
//! `PartBoxes`（非人形局部盒求交）两套模式各自的求值逻辑。
//!
//! **现状（P0c 起已接入生产）**：`classify_height_bands` / `resolve_band_assignment`
//! 已是 `combat::raycast::classify_body_part` 的真实实现——后者按目标实体解析出
//! `&BodyPlan`（经 `resolve_body_plan_for_target`）后直接委托给本模块，不再有独立的
//! 硬编码 if/else 阶梯；`standing_humanoid_aabb` 同理委托本模块背后的 `HeightBands.aabb`
//! 数据。humanoid 路径 bit-for-bit 不变由本文件的批量对拍测试
//! `classify_matches_independent_legacy_oracle_across_batch_of_npc_aim_samples` 锁死
//! ——该测试用 PR 前旧算法字面量重新实现的独立 oracle（不读 `humanoid_plan_static`/
//! `humanoid.json`，不调用生产 `classify_body_part`）逐样本对拍，防止"改造后的代码
//! 拿自己当基准"这类自证陷阱。`PartBoxes`（非人形局部盒求交，`raycast_part_boxes`）
//! 尚未接入任何生产消费点，留给 P5 whale 等非人形构型。
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

/// `PartBoxes` 模式的**点**分类：给定一个已知世界坐标命中点（不是射线——用于调用方已经
/// 用别的手段算出命中点、只需要"这个点归哪个部位管"的场景，如投射物弹道终点分类，见
/// `combat::raycast::classify_body_part` 的 `PartBoxes` 分支）。
///
/// 语义：世界点变换到实体局部系后，优先选**包含该点**的盒（局部系下逐轴都落在
/// `[min,max]` 闭区间内，等价于到盒的欧氏距离为 0）；若没有任何盒包含该点（弹道终点落在
/// 两盒之间的空隙——`PartBoxes` 不要求像 `HeightBands` 那样全高度覆盖，盒间空隙是合法
/// 状态），退化为按"点到盒表面最近距离"挑最近的盒（标准 AABB 点距离：逐轴 clamp 到
/// `[min,max]` 内取差值再取模长）。距离相同时优先级裁决与 [`raycast_part_boxes`] 完全一致
/// （`priority` 越大越优先，再等则声明顺序越靠前越优先）——两个函数共享同一个
/// [`pick_better`] 裁决器，保证"射线求交"与"点分类"两条路径的稳定序语义永不分叉。
/// 空集合返回 `None`（防御性——不 panic；调用方对已校验的非空 `PartBoxes` plan 可视为
/// 必然 `Some`）。
pub fn classify_part_boxes_point(
    world_point: DVec3,
    entity_position_world: DVec3,
    entity_yaw_radians: f64,
    boxes: &[PartBox],
) -> Option<&super::types::BodyPartId> {
    if boxes.is_empty() {
        return None;
    }

    let local_point = world_point_to_local(world_point, entity_position_world, entity_yaw_radians);

    let mut best: Option<(f64, i32, usize)> = None;
    for (index, part_box) in boxes.iter().enumerate() {
        let min = DVec3::new(
            part_box.offset[0] - part_box.half_extents[0],
            part_box.offset[1] - part_box.half_extents[1],
            part_box.offset[2] - part_box.half_extents[2],
        );
        let max = DVec3::new(
            part_box.offset[0] + part_box.half_extents[0],
            part_box.offset[1] + part_box.half_extents[1],
            part_box.offset[2] + part_box.half_extents[2],
        );
        let clamp_axis = |value: f64, lo: f64, hi: f64| -> f64 {
            let delta_below = (lo - value).max(0.0);
            let delta_above = (value - hi).max(0.0);
            delta_below + delta_above
        };
        let dx = clamp_axis(local_point.x, min.x, max.x);
        let dy = clamp_axis(local_point.y, min.y, max.y);
        let dz = clamp_axis(local_point.z, min.z, max.z);
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        let candidate = (distance, part_box.priority, index);
        best = Some(match best {
            None => candidate,
            Some(current) => pick_better(current, candidate),
        });
    }

    best.map(|(_distance, _priority, index)| &boxes[index].part_id)
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

    // ── plan-race-system-v1 P0 review 修复（major）：独立 legacy oracle ──────────
    //
    // 此前的批量对拍测试拿 `combat::raycast::classify_body_part` 当"legacy"参照组，
    // 但该函数经 P0c 重构后自身已经是 `classify_height_bands` 的薄包装（内部直接调用
    // 同一份数据驱动实现）——用改造后的生产路径给改造后的生产路径自证，测不出任何
    // 回归。以下 oracle 是 PR 前旧算法（git 历史 commit `80b328f8~1` 的
    // `combat/raycast.rs::classify_body_part`/`standing_humanoid_aabb`）字面量的独立
    // 重新实现：不 import/调用 `combat::raycast` 的 `classify_body_part`，不读取
    // `humanoid_plan_static()`/`humanoid.json`，只用下面这组硬编码常量——预期值完全
    // 由本模块自身产出。

    /// 旧 `combat/raycast.rs::STANDING_HALF_WIDTH`（PR 前字面量）。
    const LEGACY_ORACLE_STANDING_HALF_WIDTH: f64 = 0.3;
    /// 旧 `combat/raycast.rs::STANDING_HEIGHT`（PR 前字面量）。
    const LEGACY_ORACLE_STANDING_HEIGHT: f64 = 1.8;
    /// 旧 `combat/raycast.rs::ARM_LATERAL_THRESHOLD`（PR 前字面量）。
    const LEGACY_ORACLE_ARM_LATERAL_THRESHOLD: f64 = 0.19;
    /// 旧 `combat/raycast.rs::LEG_ABDOMEN_BOUNDARY`（PR 前字面量）。
    const LEGACY_ORACLE_LEG_ABDOMEN_BOUNDARY: f64 = 0.53;
    /// 旧 `classify_body_part` 头部高度阈值（PR 前字面量，`rel_y > 0.88`）。
    const LEGACY_ORACLE_HEAD_BOUNDARY: f64 = 0.88;
    /// 旧 `classify_body_part` 胸/臂高度阈值（PR 前字面量，`rel_y > 0.55`）。
    const LEGACY_ORACLE_CHEST_ARM_BOUNDARY: f64 = 0.55;

    /// 旧 `standing_humanoid_aabb(feet_position)`（PR 前签名，无 `&BodyPlan` 参数）的
    /// 独立重实现——只用上面的字面量常量，不读 `BodyPlan`。
    fn legacy_oracle_standing_humanoid_aabb(feet_position: DVec3) -> crate::combat::raycast::Aabb {
        crate::combat::raycast::Aabb {
            min: DVec3::new(
                feet_position.x - LEGACY_ORACLE_STANDING_HALF_WIDTH,
                feet_position.y,
                feet_position.z - LEGACY_ORACLE_STANDING_HALF_WIDTH,
            ),
            max: DVec3::new(
                feet_position.x + LEGACY_ORACLE_STANDING_HALF_WIDTH,
                feet_position.y + LEGACY_ORACLE_STANDING_HEIGHT,
                feet_position.z + LEGACY_ORACLE_STANDING_HALF_WIDTH,
            ),
        }
    }

    /// 旧 `classify_body_part(hit_point, target_feet_position, attack_origin)`（PR 前
    /// 签名，无 `&BodyPlan` 参数）的独立重实现——字面量 if/else-if 阶梯，与
    /// `classify_height_bands`/`resolve_band_assignment` 完全不共享代码路径。
    fn legacy_oracle_classify_body_part(
        hit_point: DVec3,
        target_feet_position: DVec3,
        attack_origin: DVec3,
    ) -> BodyPartId {
        let rel_y = ((hit_point.y - target_feet_position.y) / LEGACY_ORACLE_STANDING_HEIGHT)
            .clamp(0.0, 1.0);
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

        BodyPartId::new(if rel_y > LEGACY_ORACLE_HEAD_BOUNDARY {
            "head"
        } else if rel_y > LEGACY_ORACLE_CHEST_ARM_BOUNDARY {
            if lateral.abs() > LEGACY_ORACLE_ARM_LATERAL_THRESHOLD {
                if lateral > 0.0 {
                    "arm_r"
                } else {
                    "arm_l"
                }
            } else {
                "chest"
            }
        } else if rel_y > LEGACY_ORACLE_LEG_ABDOMEN_BOUNDARY {
            "abdomen"
        } else if lateral > 0.0 {
            "leg_r"
        } else {
            "leg_l"
        })
    }

    #[test]
    fn legacy_oracle_standing_humanoid_aabb_matches_hardcoded_pr_values() {
        // 自检：oracle 自身的 AABB 常量与 pin 文档引用的旧数值一致（0.3/1.8），
        // 防止未来有人手滑改动 oracle 常量却没人发现。
        let aabb = legacy_oracle_standing_humanoid_aabb(DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(aabb.min, DVec3::new(0.7, 2.0, 2.7));
        assert_eq!(aabb.max, DVec3::new(1.3, 3.8, 3.3));
    }

    #[test]
    fn classify_matches_independent_legacy_oracle_across_batch_of_npc_aim_samples() {
        // bit-for-bit 行为对拍：确定性 NPC 瞄准分布产出的 3000 个 (hit_point, feet,
        // origin) 三元组，逐样本喂给数据驱动的 `classify_height_bands`（新路径）与
        // 上面独立重实现的 `legacy_oracle_classify_body_part`（旧路径，零生产依赖）
        // ——这就是 P0 交付物要求的「raycast 分类回归（P1 直方图样本重跑）」，且不再
        // 让新路径给自己当 oracle。
        use crate::combat::raycast::{npc_aim_direction, npc_aim_seed, raycast_aabb};

        let origin = DVec3::new(0.0, 1.62, -2.0);
        let target = DVec3::new(0.0, 0.0, 0.0);
        let bands = humanoid_bands();
        let mut compared = 0u32;

        // 采样几何（决定哪些 tick 会产生命中）同样只用 oracle 自身的字面量 AABB +
        // 与本次重构无关、从未改动过的通用射线-AABB 求交原语 `raycast_aabb`——不经
        // `combat::raycast::raycast_humanoid`/`standing_humanoid_aabb(plan, ..)`，
        // 采样阶段也不依赖 `humanoid_plan_static()`。
        let oracle_aabb = legacy_oracle_standing_humanoid_aabb(target);

        for tick in 0..3000u64 {
            let seed = npc_aim_seed("npc:body_plan_geometry_parity_v2", tick);
            let aim_direction = npc_aim_direction(origin, target, seed, 1.0);
            let Some(hit) = raycast_aabb(origin, aim_direction, 5.0, oracle_aabb) else {
                continue;
            };

            let expected = legacy_oracle_classify_body_part(hit.point, target, origin);
            let actual = classify_height_bands(
                hit.point,
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
                actual, &expected,
                "tick {tick}: classify_height_bands({actual:?}) 与独立 legacy oracle（PR 前\
                 字面量重实现，非生产路径自证）({expected:?}) 不一致 — bit-for-bit 行为对拍失败"
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

    // ───────────────────────── classify_part_boxes_point ─────────────────────────

    #[test]
    fn classify_part_boxes_point_containment_picks_enclosing_box() {
        let boxes = vec![
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
        ];
        let part = classify_part_boxes_point(
            DVec3::new(0.0, 0.0, 2.1),
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        )
        .expect("point inside 'near' box must classify");
        assert_eq!(part, &BodyPartId::new("near"));
    }

    #[test]
    fn classify_part_boxes_point_gap_between_boxes_falls_back_to_nearest() {
        // 点落在两盒之间的空隙（PartBoxes 不要求全覆盖）——必须退化为"点到盒表面最近"。
        let boxes = vec![
            PartBox {
                part_id: "left".into(),
                offset: [0.0, 0.0, 0.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
            PartBox {
                part_id: "right".into(),
                offset: [0.0, 0.0, 10.0],
                half_extents: [0.5, 0.5, 0.5],
                priority: 0,
            },
        ];
        // z=2.0: 到 left 盒表面 (z=0.5) 距离 1.5；到 right 盒表面 (z=9.5) 距离 7.5——left 更近。
        let part = classify_part_boxes_point(
            DVec3::new(0.0, 0.0, 2.0),
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        )
        .expect("gap point must still classify via nearest-box fallback");
        assert_eq!(part, &BodyPartId::new("left"));
    }

    #[test]
    fn classify_part_boxes_point_equal_distance_prefers_higher_priority() {
        let boxes = vec![
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
        let part = classify_part_boxes_point(
            DVec3::new(0.0, 0.0, 2.0),
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        )
        .expect("should classify");
        assert_eq!(part, &BodyPartId::new("high_priority"));
    }

    #[test]
    fn classify_part_boxes_point_equal_distance_equal_priority_prefers_earlier_declaration() {
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
        let part = classify_part_boxes_point(
            DVec3::new(0.0, 0.0, 2.0),
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        )
        .expect("should classify");
        assert_eq!(
            part,
            &BodyPartId::new("first"),
            "stable tie-break must prefer the earlier-declared box"
        );
    }

    #[test]
    fn classify_part_boxes_point_empty_slice_never_classifies() {
        assert!(classify_part_boxes_point(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &[],
        )
        .is_none());
    }

    #[test]
    fn classify_part_boxes_point_yaw_and_translation_invariance() {
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

        for (yaw_degrees, entity_position) in [
            (0.0_f64, DVec3::new(0.0, 0.0, 0.0)),
            (90.0_f64, DVec3::new(-3.0, 0.0, 7.0)),
            (180.0_f64, DVec3::new(1.0, 0.0, 1.0)),
        ] {
            let yaw = yaw_degrees.to_radians();
            // 世界系下"局部 (0,0,2.1)"（front 盒内部一点）随 yaw 旋转 + 实体位置平移。
            let local_offset = DVec3::new(0.0, 0.0, 2.1);
            let world_point = entity_position + rotate_world_to_local(local_offset, -yaw);

            let part = classify_part_boxes_point(world_point, entity_position, yaw, &boxes)
                .unwrap_or_else(|| panic!("yaw={yaw_degrees} translated point must classify"));
            assert_eq!(
                part,
                &BodyPartId::new("front"),
                "yaw={yaw_degrees} 平移+旋转后应仍命中 front（局部系不变性）"
            );
        }
    }
}
