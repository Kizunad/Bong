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
//! 拿自己当基准"这类自证陷阱。**`PartBoxes`（非人形局部盒求交，`raycast_part_boxes`）
//! 现已接入两个生产消费点**：近战统一入口 `combat::raycast::raycast_humanoid` 与
//! 投射物 `combat::carrier::projectile_tick_system`（plan-race-system-v1 P0 review r3
//! 收口——carrier 原先经 `classify_part_boxes_point` 的"已知命中点 + 就近回退"反推部位，
//! 就近回退会把盒间空隙伪造成有效命中，现已改为对弹道线段直接调用
//! `raycast_part_boxes` 真实射线-盒求交）。两个消费点都直接调用 `raycast_part_boxes`
//! 本身，不经过 `classify_part_boxes_point`（后者现已降级为无生产调用者的测试/工具向
//! 几何原语，见该函数文档）。
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
///
/// plan-race-system-v1 bughunt major-1 修复：此前实现的符号与本文件顶部文档注释
/// 自称的"绕 Y 轴旋转 `-yaw`"相反——实际算出的是绕 `+yaw` 旋转，与 valence
/// `Look::to_vec()`（`valence_entity::Look`，pitch=0 时 `forward = (-sin(yaw), 0,
/// cos(yaw))`）的朝向约定对不上。旧实现下 yaw 的 sin 分量符号取反，使得非人形
/// `PartBoxes` 命中在实体转向后算出的局部系与"实体实际面朝方向"相反——一个偏
/// 移到实体右侧的部位盒，会被误判成"在左侧"。此前的 yaw 不变性测试之所以没测出
/// 来：`raycast_part_boxes_yaw_and_translation_invariance` 用同一个错误符号的
/// `world_forward` 公式反推期望值（旋转与逆旋转用同一套错误约定自证），
/// `combat::resolve` 里的 alien_carrier 系列测试则因攻方射线恰好沿目标局部系对称
/// 轴线（世界 x 轴，z 分量为 0）入射，旋转 90°/180° 后 sin 项刚好不影响结果，属于
/// "巧合过关"而非真验证。修复后符号与 valence 约定对齐，现有 yaw 象限/非对称盒
/// pin 测试见下方 `raycast_part_boxes_yaw_quadrants_match_valence_look_convention`
/// 与 `raycast_part_boxes_yaw_quadrants_reject_mirrored_convention`。
fn rotate_world_to_local(v: DVec3, yaw_radians: f64) -> DVec3 {
    let (sin_y, cos_y) = yaw_radians.sin_cos();
    DVec3::new(v.x * cos_y + v.z * sin_y, v.y, -v.x * sin_y + v.z * cos_y)
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
/// 用别的手段算出命中点、只需要"这个点归哪个部位管"的场景）。
///
/// plan-race-system-v1 P0 review r3（blocker 收口）—— **containment-only，无就近回退**：
/// 世界点变换到实体局部系后，只有落在某个盒的闭区间 `[min,max]`（逐轴同时满足）才判定
/// 命中该盒；若没有任何盒包含该点（命中点落在两盒之间的空隙——`PartBoxes` 不要求像
/// `HeightBands` 那样全高度覆盖，盒间空隙是合法状态），**返回 `None`，不再**退化为"点到
/// 盒表面最近距离"选最近的盒。旧版就近回退是一处语义缺陷：真实弹道根本没有打中任何
/// 部位，却被强行分类成离得最近的那个，把空隙伪造成了有效命中（plan-race-system-v1 P0
/// review r3 blocker/major）。命中多个重叠盒时优先级裁决与 [`raycast_part_boxes`] 一致
/// （`priority` 越大越优先，再等则声明顺序越靠前越优先，见 [`pick_better_containment`]）。
///
/// **调用者身份（review r3 后）**：本函数唯一的非测试调用点是
/// `combat::raycast::classify_body_part` 的 `PartBoxes` 分支——但该分支自身已无任何已知
/// 生产调用者：`combat::carrier`（原先唯一的生产消费点，`projectile_tick_system`）已改为
/// 对弹道线段直接调用 [`raycast_part_boxes`]（真实射线-盒求交，权威决定"是否命中"与
/// "命中哪个部位"），`combat::raycast::raycast_humanoid` 的 `PartBoxes` 分支同样直接调用
/// [`raycast_part_boxes`]、从未经过 `classify_body_part`/本函数。本函数因此实质上只剩
/// 测试/工具用途——保留是因为"给定已知命中点、不给方向、只要按包含关系归类"仍是一个
/// 独立自洽的几何原语（未来若出现这样的消费场景可以直接复用），但**当前没有任何生产
/// 路径依赖它对空隙的处理方式**，本文件下方 `classify_part_boxes_point_*` 测试组是
/// 它行为的唯一权威 pin。空集合同样返回 `None`（防御性——不 panic）。
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

    let mut best: Option<(i32, usize)> = None;
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
        let contains_point = local_point.x >= min.x
            && local_point.x <= max.x
            && local_point.y >= min.y
            && local_point.y <= max.y
            && local_point.z >= min.z
            && local_point.z <= max.z;
        if !contains_point {
            continue;
        }

        let candidate = (part_box.priority, index);
        best = Some(match best {
            None => candidate,
            Some(current) => pick_better_containment(current, candidate),
        });
    }

    best.map(|(_priority, index)| &boxes[index].part_id)
}

/// plan-race-system-v1 P5/PR-6c —— 给定目标 [`super::types::HitGeometry`]，算出一个
/// 保守的标量"粗筛半径"（供 `combat::carrier` 投射物命中的广义相位距离检测替换写死的
/// `ANQI_HITBOX_INFLATION` 常量——该常量原假设全体目标都是 humanoid 直立 1.8m/0.3
/// half_width 的比例，对 whale 这类横长非人构型严重失配）。
///
/// - `HeightBands`：直接取 `aabb.half_width`——与 `combat::carrier` 换轨前的写死值
///   `0.3` bit-for-bit 相同（humanoid 目标半径不回归）。
/// - `PartBoxes`：`max(|offset[axis]| + half_extents[axis])`，逐盒逐轴取最大——用单轴
///   上离局部原点最远的盒边界作为保守半径（未做真正的向量长度/球外接半径计算，因为
///   `carrier` 消费点本身就是用一个标量阈值和"点到线段距离"比较,标量粗筛只需要"不会
///   漏判命中"这个保守性质，不需要精确外接球）。空盒集合（理论上 `validate_body_plan`
///   已禁止 `PartBoxes` 空 boxes，但函数本身仍防御性处理）返回 `0.0`。
pub fn bounding_radius(hit_geometry: &super::types::HitGeometry) -> f64 {
    match hit_geometry {
        super::types::HitGeometry::HeightBands { aabb, .. } => aabb.half_width,
        super::types::HitGeometry::PartBoxes { boxes } => boxes
            .iter()
            .flat_map(|part_box| {
                (0..3).map(|axis| part_box.offset[axis].abs() + part_box.half_extents[axis])
            })
            .fold(0.0_f64, f64::max),
    }
}

/// containment-only 场景下的裁决：所有候选距离恒为 0（都真正包含该点），只需比较
/// `priority`（越大越优先），再等则声明顺序越靠前越优先——与 [`pick_better`] 的第二/
/// 第三裁决层级完全一致，抽成独立的二元组版本，避免为了复用三元组裁决器而给不存在的
/// 距离维度硬凑一个恒定 `0.0`。
fn pick_better_containment(a: (i32, usize), b: (i32, usize)) -> (i32, usize) {
    if a.0 != b.0 {
        if a.0 > b.0 {
            a
        } else {
            b
        }
    } else if a.1 <= b.1 {
        a
    } else {
        b
    }
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

    // plan-race-system-v1 P0 review r3（blocker 收口，负向测试三类之一：射线穿过空隙
    // 未命中）—— `raycast_part_boxes` 本身从未有过就近回退，这条测试专门锁死"多盒
    // 构型下，射线笔直穿过两盒之间的空隙"这个场景恒定返回 `None`，作为
    // `combat::carrier` 新接线（弹道线段直接调用本函数）的权威依据：carrier 目标若是
    // 这样的 PartBoxes 构型，空隙弹道必须被判定为未命中任何具体部位，而不是像旧版
    // `classify_part_boxes_point` 那样伪造出"最近"的部位。
    #[test]
    fn raycast_part_boxes_ray_through_gap_between_boxes_misses() {
        let boxes = vec![
            PartBox {
                part_id: "left".into(),
                offset: [-1.0, 0.0, 2.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 0,
            },
            PartBox {
                part_id: "right".into(),
                offset: [1.0, 0.0, 2.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 0,
            },
        ];
        // 射线沿局部 +Z 笔直穿过 x=0（两盒分别在 x=-1±0.3 / x=1±0.3，中间 x∈(-0.7,0.7)
        // 是空隙），必须不命中任何一个盒。
        let hit = raycast_part_boxes(
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 1.0),
            10.0,
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        );
        assert!(
            hit.is_none(),
            "射线沿 x=0 穿过 left/right 两盒之间的空隙必须未命中，实测 {hit:?}"
        );
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
            // 世界系下"实体局部 +Z 方向"随 yaw 旋转：local (0,0,1) -> world。
            // plan-race-system-v1 bughunt major-1：与 valence `Look::to_vec()`
            // （pitch=0）一致的公式是 forward = (-sin(yaw), 0, cos(yaw))——此前这里
            // 写成 `(sin(yaw), 0, cos(yaw))`，与当时 `rotate_world_to_local` 的错误
            // 符号自洽（旋转/逆旋转用同一套错误约定），测不出符号反转。
            let world_forward = DVec3::new(-yaw.sin(), 0.0, yaw.cos());
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

    // plan-race-system-v1 bughunt major-1：非对称盒 + 独立于 `rotate_world_to_local`
    // 内部实现的物理语义 pin。攻击射线沿世界 -Y 方向垂直下砸，完全不经过任何 yaw
    // 旋转，因此不会重蹈"用同一套（可能错误的）旋转公式反推期望值，旋转与逆旋转
    // 自证"的覆辙——期望的盒子世界坐标只由 valence `Look::to_vec()`（pitch=0）
    // 的独立公式 `right = (cos(yaw), 0, sin(yaw))` 算出。

    /// 独立于生产代码的 ground-truth：valence `Look::to_vec()`（`valence_entity::Look`，
    /// pitch=0）化简后 forward = (-sin(yaw), 0, cos(yaw))，right（forward 顺时针转
    /// 90°）= (cos(yaw), 0, sin(yaw))。
    fn valence_right_world(yaw_radians: f64) -> DVec3 {
        DVec3::new(yaw_radians.cos(), 0.0, yaw_radians.sin())
    }

    #[test]
    fn raycast_part_boxes_yaw_quadrants_match_valence_look_convention() {
        let entity_position = DVec3::new(5.0, 64.0, -3.0);
        let boxes = vec![PartBox {
            part_id: "right_fin".into(),
            offset: [1.0, 0.0, 0.0],
            half_extents: [0.2, 0.5, 0.2],
            priority: 0,
        }];

        for yaw_degrees in [0.0_f64, 90.0, 180.0, 270.0] {
            let yaw = yaw_degrees.to_radians();
            let expected_box_world = entity_position + valence_right_world(yaw);

            // 从正上方垂直下砸：射线本身不带任何 yaw 旋转假设，只依赖
            // `raycast_part_boxes` 内部把 (entity_position, yaw) 变换到局部系的
            // 正确性。
            let ray_origin = DVec3::new(
                expected_box_world.x,
                entity_position.y + 10.0,
                expected_box_world.z,
            );
            let ray_direction = DVec3::new(0.0, -1.0, 0.0);

            let hit = raycast_part_boxes(
                ray_origin,
                ray_direction,
                20.0,
                entity_position,
                yaw,
                &boxes,
            )
            .unwrap_or_else(|| {
                panic!(
                    "yaw={yaw_degrees}: 按 valence Look 右手系推算出的盒世界坐标 \
                         {expected_box_world:?} 应命中 right_fin，实测未命中"
                )
            });
            assert_eq!(
                hit.part_id,
                BodyPartId::new("right_fin"),
                "yaw={yaw_degrees}: 应命中 right_fin"
            );
        }
    }

    #[test]
    fn raycast_part_boxes_yaw_quadrants_reject_mirrored_convention() {
        // 反向 pin：若符号约定被镜像（right = (cos(yaw), 0, -sin(yaw))，即本次修复
        // 前的错误约定），除了 sin(yaw)=0 的 yaw=0°/180°（两种约定重合，非判别性
        // case）之外，用镜像坐标去打必须全部落空——证明修复后的实现不是"凑巧两种
        // 约定都能过"。
        let entity_position = DVec3::new(5.0, 64.0, -3.0);
        let boxes = vec![PartBox {
            part_id: "right_fin".into(),
            offset: [1.0, 0.0, 0.0],
            half_extents: [0.2, 0.5, 0.2],
            priority: 0,
        }];

        for yaw_degrees in [90.0_f64, 270.0] {
            let yaw = yaw_degrees.to_radians();
            let mirrored_right_world = DVec3::new(yaw.cos(), 0.0, -yaw.sin());
            let mirrored_box_world = entity_position + mirrored_right_world;

            let ray_origin = DVec3::new(
                mirrored_box_world.x,
                entity_position.y + 10.0,
                mirrored_box_world.z,
            );
            let ray_direction = DVec3::new(0.0, -1.0, 0.0);

            let hit = raycast_part_boxes(
                ray_origin,
                ray_direction,
                20.0,
                entity_position,
                yaw,
                &boxes,
            );
            assert!(
                hit.is_none(),
                "yaw={yaw_degrees}: 镜像（错误）符号约定推出的坐标 {mirrored_box_world:?} \
                 不应命中任何部位，实测 {hit:?}"
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

    // plan-race-system-v1 P0 review r3（blocker 收口）—— 就近回退已删除，以下三个
    // 测试锁死 containment-only 语义下的负向行为：盒间空隙 / 完全在所有盒之外，两者
    // 都必须返回 `None`，不得伪造出一个"最近"的命中部位。

    #[test]
    fn classify_part_boxes_point_gap_between_boxes_returns_none() {
        // 点落在两盒之间的空隙（PartBoxes 不要求全覆盖，空隙是合法状态）——曾经的
        // "就近回退"会把这里错分类成 left（距离 1.5 < right 距离 7.5），现在必须显式
        // 返回 None：没有任何盒真正包含这个点，就不该产出命中部位。
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
        let part = classify_part_boxes_point(
            DVec3::new(0.0, 0.0, 2.0),
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        );
        assert!(
            part.is_none(),
            "点落在两盒之间的空隙必须返回 None（不再退化为就近回退），实测 {part:?}"
        );
    }

    #[test]
    fn classify_part_boxes_point_all_boxes_missed_returns_none() {
        // 点整体上并不在任何盒附近的"夹缝"里，而是径直落在三个盒的公共包围范围之外
        // （侧向偏移量远超过任何单个盒的 half_extents）——与"盒间空隙"场景（点仍大致
        // 位于盒群跨度内）区分开的独立负向 case：即使拉远到明显在外面，也绝不允许
        // 兜底选中"最近"的那个盒。
        let boxes = vec![
            PartBox {
                part_id: "left".into(),
                offset: [-1.0, 0.0, 0.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 0,
            },
            PartBox {
                part_id: "center".into(),
                offset: [0.0, 0.0, 0.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 0,
            },
            PartBox {
                part_id: "right".into(),
                offset: [1.0, 0.0, 0.0],
                half_extents: [0.3, 0.3, 0.3],
                priority: 0,
            },
        ];
        let part = classify_part_boxes_point(
            DVec3::new(0.0, 0.0, 50.0),
            DVec3::new(0.0, 0.0, 0.0),
            0.0,
            &boxes,
        );
        assert!(
            part.is_none(),
            "点远离全部三个盒（z=50 对比盒群 z≈0）必须返回 None，实测 {part:?}"
        );
    }

    #[test]
    fn classify_part_boxes_point_multiple_containing_boxes_prefer_higher_priority() {
        // 两个盒同一偏移/同一尺寸完全重叠——点被两者同时真正包含（containment，非距离
        // 相等的回退场景），裁决必须落到 priority 更高的盒。
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
    fn classify_part_boxes_point_multiple_containing_boxes_equal_priority_prefers_earlier_declaration(
    ) {
        // 两个完全重叠、优先级也相同的盒都真正包含该点——稳定裁决必须选声明顺序更靠前的。
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

    // ───────────────────────── bounding_radius ─────────────────────────

    #[test]
    fn bounding_radius_height_bands_returns_half_width_unchanged() {
        // 换轨前 `combat::carrier::ANQI_HITBOX_INFLATION` 搭配的隐式 humanoid 半径
        // 就是 `STANDING_HALF_WIDTH=0.3`——bounding_radius 对 HeightBands 必须原样
        // 吐出 aabb.half_width，不做任何额外膨胀，humanoid 目标粗筛半径 bit-for-bit
        // 不回归。
        let geometry = super::super::types::HitGeometry::HeightBands {
            aabb: super::super::types::StandingAabbSpec {
                half_width: 0.3,
                height: 1.8,
            },
            bands: vec![HeightBand {
                min_rel_y: -1.0,
                assignment: HeightBandAssignment::Single {
                    part: "core".into(),
                },
            }],
            lateral_threshold: 0.19,
        };
        assert_eq!(bounding_radius(&geometry), 0.3);
    }

    #[test]
    fn bounding_radius_real_humanoid_plan_matches_legacy_hardcoded_half_width() {
        // 真实 humanoid.json 对拍：与 carrier 换轨前写死的 0.3 一致。
        let plan = crate::body_plan::registry::humanoid_plan_static();
        assert_eq!(bounding_radius(&plan.hit_geometry), 0.3);
    }

    #[test]
    fn bounding_radius_part_boxes_takes_max_of_offset_plus_half_extent_across_axes_and_boxes() {
        let boxes = vec![
            PartBox {
                part_id: "small".into(),
                offset: [0.0, 0.0, 1.0],
                half_extents: [0.2, 0.2, 0.2],
                priority: 0,
            },
            PartBox {
                part_id: "large".into(),
                offset: [3.0, 0.0, 0.0],
                half_extents: [1.0, 0.5, 0.5],
                priority: 0,
            },
        ];
        let geometry = super::super::types::HitGeometry::PartBoxes { boxes };
        // 最大候选 = |3.0| + 1.0 = 4.0（"large" 盒 x 轴），必须压过其余更小候选。
        assert_eq!(bounding_radius(&geometry), 4.0);
    }

    #[test]
    fn bounding_radius_part_boxes_negative_offset_uses_absolute_value() {
        let boxes = vec![PartBox {
            part_id: "left".into(),
            offset: [-2.5, 0.0, 0.0],
            half_extents: [0.3, 0.3, 0.3],
            priority: 0,
        }];
        let geometry = super::super::types::HitGeometry::PartBoxes { boxes };
        assert_eq!(
            bounding_radius(&geometry),
            2.8,
            "负 offset 必须取绝对值参与比较（|-2.5|+0.3=2.8）"
        );
    }

    #[test]
    fn bounding_radius_part_boxes_empty_defensively_returns_zero() {
        // validate_body_plan 已禁止空 boxes 落盘,但函数本身不假设调用方已校验。
        let geometry = super::super::types::HitGeometry::PartBoxes { boxes: vec![] };
        assert_eq!(bounding_radius(&geometry), 0.0);
    }

    #[test]
    fn bounding_radius_real_whale_plan_exceeds_real_humanoid_plan() {
        // plan-race-system-v1 P5/PR-6c 核心回归目标：whale（横长非人构型）的粗筛半径
        // 必须显著大于 humanoid,证明 carrier 不再对巨型构型使用与人形相同的固定半径。
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let whale_json =
            std::fs::read_to_string(manifest_dir.join("assets/body_plans/plans/whale.json"))
                .expect("real whale.json should exist");
        let whale_plan: super::super::types::BodyPlan =
            serde_json::from_str(&whale_json).expect("real whale.json should parse");
        crate::body_plan::validate::validate_body_plan(&whale_plan)
            .expect("real whale.json must pass validate_body_plan");

        let humanoid_plan = crate::body_plan::registry::humanoid_plan_static();
        let whale_radius = bounding_radius(&whale_plan.hit_geometry);
        let humanoid_radius = bounding_radius(&humanoid_plan.hit_geometry);
        assert!(
            whale_radius > humanoid_radius,
            "whale 粗筛半径 {whale_radius} 必须大于 humanoid {humanoid_radius}"
        );
    }

    // ───────────────────── whale.json PartBoxes 锚测试（4 类） ─────────────────────
    //
    // plan-race-system-v1 P5/PR-6c —— 直接从磁盘加载真实 `plans/whale.json`,对
    // `raycast_part_boxes` 做 4 类几何锚点验证（不复用抽象 fixture,锁的是本 PR 真实
    // 落盘的部位几何数据本身）：
    // ① 左右胸鳍同高度能区分 ② 头尾纵向命中 ③ 边界擦触 ④ 重叠区取最近交点/priority。

    fn load_real_whale_boxes() -> Vec<PartBox> {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let whale_json =
            std::fs::read_to_string(manifest_dir.join("assets/body_plans/plans/whale.json"))
                .expect("real whale.json should exist");
        let whale_plan: super::super::types::BodyPlan =
            serde_json::from_str(&whale_json).expect("real whale.json should parse");
        match whale_plan.hit_geometry {
            super::super::types::HitGeometry::PartBoxes { boxes } => boxes,
            other => panic!("whale.json must use PartBoxes hit_geometry, got {other:?}"),
        }
    }

    #[test]
    fn whale_anchor_left_and_right_pectoral_fin_distinguished_at_same_height() {
        let boxes = load_real_whale_boxes();
        let entity_position = DVec3::new(10.0, 70.0, -20.0);
        let yaw = 0.0;

        // 射线从上方垂直下砸，落在 left_pectoral_fin 局部偏移 x=-1.42 附近（负 x）。
        let left_ray_origin = entity_position + DVec3::new(-1.42, 20.0, -0.31);
        let left_hit = raycast_part_boxes(
            left_ray_origin,
            DVec3::new(0.0, -1.0, 0.0),
            30.0,
            entity_position,
            yaw,
            &boxes,
        )
        .expect("must hit left_pectoral_fin from directly above");
        assert_eq!(left_hit.part_id, BodyPartId::new("left_pectoral_fin"));

        // 同一高度,镜像到正 x（right_pectoral_fin）。
        let right_ray_origin = entity_position + DVec3::new(1.42, 20.0, -0.31);
        let right_hit = raycast_part_boxes(
            right_ray_origin,
            DVec3::new(0.0, -1.0, 0.0),
            30.0,
            entity_position,
            yaw,
            &boxes,
        )
        .expect("must hit right_pectoral_fin from directly above");
        assert_eq!(right_hit.part_id, BodyPartId::new("right_pectoral_fin"));
        assert_ne!(
            left_hit.part_id, right_hit.part_id,
            "左右胸鳍必须在同一高度被区分为不同部位"
        );
    }

    #[test]
    fn whale_anchor_head_and_tail_longitudinal_hit() {
        let boxes = load_real_whale_boxes();
        let entity_position = DVec3::new(0.0, 80.0, 0.0);
        let yaw = 0.0;

        // 头部（skull，局部 z=+1.67）。
        let skull_origin = entity_position + DVec3::new(0.0, 20.0, 1.67);
        let skull_hit = raycast_part_boxes(
            skull_origin,
            DVec3::new(0.0, -1.0, 0.0),
            30.0,
            entity_position,
            yaw,
            &boxes,
        )
        .expect("must hit skull at the front");
        assert_eq!(skull_hit.part_id, BodyPartId::new("skull"));

        // 尾部（tail_fin，局部 z=-3.74）。
        let tail_origin = entity_position + DVec3::new(0.0, 20.0, -3.74);
        let tail_hit = raycast_part_boxes(
            tail_origin,
            DVec3::new(0.0, -1.0, 0.0),
            30.0,
            entity_position,
            yaw,
            &boxes,
        )
        .expect("must hit tail_fin at the back");
        assert_eq!(tail_hit.part_id, BodyPartId::new("tail_fin"));
    }

    #[test]
    fn whale_anchor_grazing_boundary_hit_registers() {
        let boxes = load_real_whale_boxes();
        let entity_position = DVec3::new(5.0, 64.0, -3.0);
        let yaw = 0.0;

        // skull 盒：offset=[0,0.72,1.67] half_extents=[0.5,0.53,1.13]。贴着 y 上边界
        // （0.72+0.53=1.25）水平掠过局部 z=1.67（skull 中心线）。
        let ray_origin = entity_position + DVec3::new(-5.0, 1.25, 1.67);
        let hit = raycast_part_boxes(
            ray_origin,
            DVec3::new(1.0, 0.0, 0.0),
            20.0,
            entity_position,
            yaw,
            &boxes,
        );
        assert!(
            hit.is_some(),
            "贴着 skull 盒上边界的射线应仍判定命中（闭区间）,实测 {hit:?}"
        );
        assert_eq!(hit.unwrap().part_id, BodyPartId::new("skull"));
    }

    #[test]
    fn whale_anchor_overlap_zone_prefers_nearest_intersection() {
        let boxes = load_real_whale_boxes();
        let entity_position = DVec3::new(0.0, 90.0, 0.0);
        let yaw = 0.0;

        // torso（offset=[0,0.63,-0.66] half=[0.56,0.69,1.22]，局部 y∈[-0.06,1.32]）与
        // left_pectoral_fin（offset=[-1.42,0.14,-0.31] half=[1.26,0.67,0.74]，局部
        // y∈[-0.53,0.81]）在 (x=-0.4, z=-0.4) 这一列 x/z 都真实重叠（两盒的 x/z 范围
        // 在该点均覆盖，见 whale.json 设计注记），唯独 y 范围不同——left_pectoral_fin
        // 的下边界（-0.53）比 torso 的下边界（-0.06）更低。从两盒下方垂直上射，必须
        // 先命中 left_pectoral_fin（真实最近交点，非 priority 凑效——距离不等时
        // priority 从不参与裁决，只有等距才轮到它，见 `pick_better`）。
        let ray_origin = entity_position + DVec3::new(-0.4, -20.0, -0.4);
        let hit = raycast_part_boxes(
            ray_origin,
            DVec3::new(0.0, 1.0, 0.0),
            30.0,
            entity_position,
            yaw,
            &boxes,
        )
        .expect("overlap column ray must hit something");
        assert_eq!(
            hit.part_id,
            BodyPartId::new("left_pectoral_fin"),
            "torso/left_pectoral_fin 重叠列必须按真实最近交点命中 left_pectoral_fin（其下边界 \
             -0.53 比 torso 的 -0.06 更早被自下而上的射线触及）"
        );
    }
}
