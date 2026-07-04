#![allow(dead_code)]

use valence::prelude::DVec3;

use super::components::BodyPart;
use super::events::{AttackReach, DAGGER_REACH, FIST_REACH, SPEAR_REACH, STAFF_REACH, SWORD_REACH};

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

/// P1 校准值（决议 §8.1 #1，plan-combat-hit-location-v1 P1）：替换 P0 起始值 9.0/7.0。
///
/// P0 的几何估算把"够头需抬 pitch ~11°"当成与"够腿需压 pitch ~16°"同量级的对称偏移，
/// 但 `ATTACKER_EYE_HEIGHT=1.62` 已经贴近头部阈值（`rel_y>0.88` 对应 y>1.584），命中头部
/// 几乎不需要额外上仰；命中腿部（`rel_y<=leg 阈值`，y 更接近脚底）则需要大得多的下压角。
/// 固定 seed 直方图实测（3000 次 NPC 攻击，origin=(0,1.62,-2) target=(0,0,0)，fist 缩放）显示：
/// 纯调 σ 无法同时满足"头 8-12% 且腿 15-20%"——σ 越大头部命中率越先突破上限。
/// 因此 P1 校准同时下调 yaw_sigma（收窄横向散布，抑制臂命中率飙升）、
/// 小幅上调 pitch_sigma，并配合下方 `classify_body_part` 阈值调整（见该函数注释）
/// 把原本被 Abdomen 吞掉的中低段命中重新分给 Leg，最终校准分布（seed
/// `npc_aim_seed("npc:combat_dist_p1", 0..3000)`）：胸 48.9% / 腹 2.6%（窄带，见下方阈值调整
/// 说明）/ 头 11.6% / 臂 19.3%(左右各半) / 腿 17.5%(左右各半)，见
/// `npc_aim_direction_batch_distribution_hits_target_ranges_p1` pin 测试。
pub const AIM_JITTER_PITCH_SIGMA_DEG: f64 = 10.0;
pub const AIM_JITTER_YAW_SIGMA_DEG: f64 = 6.0;

/// 武器 reach → σ 缩放系数（决议 §8.1 #1）：reach 越长散布越松，近身武器更精准。
/// 武器只分拳/近战刃/长杆三档（`events.rs:26-32`），无独立"枪"类；spear 与 staff 同归长杆档。
/// 未匹配到已知档位的 reach（技能/暗器等特殊攻击途径）保守取中性 1.0，不额外收紧或放松散布。
pub fn weapon_aim_jitter_scale(reach: AttackReach) -> f64 {
    if reach == DAGGER_REACH {
        0.85
    } else if reach == SWORD_REACH {
        0.9
    } else if reach == FIST_REACH {
        1.0
    } else if reach == SPEAR_REACH || reach == STAFF_REACH {
        1.1
    } else {
        1.0
    }
}

/// NPC 瞄准 jitter 的确定性种子：`hash(attacker_id) ^ combat_tick`（决议 §8.1 #1/#3）。
/// 同一攻方 + 同一 tick 恒定复现同一散布结果（确定性要求），不同攻方/不同 tick 天然错开。
pub fn npc_aim_seed(attacker_id: &str, combat_tick: u64) -> u64 {
    fnv1a_hash(attacker_id.as_bytes()) ^ combat_tick
}

fn fnv1a_hash(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// splitmix64：从可变状态确定性推进出下一个 64-bit 值（纯函数式 PRNG 步进，无外部依赖）。
fn splitmix64_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 63-bit 精度均匀分布 [0, 1)。
fn uniform_01(bits: u64) -> f64 {
    ((bits >> 11) as f64) * (1.0 / (1u64 << 53) as f64)
}

/// Box-Muller 变换：从种子确定性生成一对独立标准正态样本（均值 0、标准差 1）。
///
/// `pub(crate)`：plan-combat-hit-location-v1 P1 起 `combat::needle` 复用同一套确定性
/// PRNG 给气针发射方向加散布抖动（决议 §8.1 #2），避免重复实现 Box-Muller/splitmix64。
pub(crate) fn gaussian_pair(seed: u64) -> (f64, f64) {
    let mut state = seed;
    // u1 钳位避开 ln(0)（seed=0 时 splitmix64 首个输出理论上可能极端接近 0）。
    let u1 = uniform_01(splitmix64_next(&mut state)).max(1e-12);
    let u2 = uniform_01(splitmix64_next(&mut state));
    let r = (-2.0 * u1.ln()).sqrt();
    let theta = 2.0 * std::f64::consts::PI * u2;
    (r * theta.cos(), r * theta.sin())
}

/// 世界方向向量 → (yaw, pitch) 弧度对（内部自洽的球面参数化，仅用于 jitter 旋转合成，
/// 与 valence `Look` 的 yaw/pitch 约定无需一致——只在本函数与其逆函数之间自洽即可）。
fn direction_to_yaw_pitch(dir: DVec3) -> (f64, f64) {
    let len = dir.length();
    if len <= f64::EPSILON {
        return (0.0, 0.0);
    }
    let normalized = dir / len;
    let yaw = normalized.z.atan2(normalized.x);
    let pitch = normalized.y.clamp(-1.0, 1.0).asin();
    (yaw, pitch)
}

fn yaw_pitch_to_direction(yaw: f64, pitch: f64) -> DVec3 {
    let (pitch_sin, pitch_cos) = pitch.sin_cos();
    let (yaw_sin, yaw_cos) = yaw.sin_cos();
    DVec3::new(yaw_cos * pitch_cos, pitch_sin, yaw_sin * pitch_cos)
}

/// 目标胸高瞄准点（原 `fallback_aim` 的几何中心，仅取决于目标脚下坐标，
/// 站立包围盒左右对称使得 X/Z 中心恒等于脚下坐标本身）。
fn chest_aim_point(target_feet_position: DVec3) -> DVec3 {
    DVec3::new(
        target_feet_position.x,
        target_feet_position.y + CHEST_AIM_HEIGHT,
        target_feet_position.z,
    )
}

/// 胸高瞄准方向（未叠加 jitter）：玩家 Look 缺失时的防御性兜底、NPC jitter 的基准方向。
pub fn chest_aim_direction(origin: DVec3, target_feet_position: DVec3) -> DVec3 {
    chest_aim_point(target_feet_position) - origin
}

/// NPC 瞄准方向：指向目标几何中心 + 确定性二维高斯 jitter（决议 §8.1 #1/#3）。
/// `weapon_sigma_scale` 由 [`weapon_aim_jitter_scale`] 按攻方武器 reach 档位给出。
pub fn npc_aim_direction(
    origin: DVec3,
    target_feet_position: DVec3,
    seed: u64,
    weapon_sigma_scale: f64,
) -> DVec3 {
    let base_dir = chest_aim_direction(origin, target_feet_position);
    let (pitch_jitter_std, yaw_jitter_std) = gaussian_pair(seed);
    let pitch_delta_deg = pitch_jitter_std * AIM_JITTER_PITCH_SIGMA_DEG * weapon_sigma_scale;
    let yaw_delta_deg = yaw_jitter_std * AIM_JITTER_YAW_SIGMA_DEG * weapon_sigma_scale;

    let (yaw, pitch) = direction_to_yaw_pitch(base_dir);
    let jittered_yaw = yaw + yaw_delta_deg.to_radians();
    let jittered_pitch = (pitch + pitch_delta_deg.to_radians())
        .clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
    yaw_pitch_to_direction(jittered_yaw, jittered_pitch)
}

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

/// 臂命中横向阈值（决议 §8.1 #4，plan-combat-hit-location-v1 P1 校准）。
/// P0 起始值 0.18 → P1 实测微调至 0.19（略收窄 Chest 命中带，把边缘让给 Arm，
/// 配合下方 `LEG_ABDOMEN_BOUNDARY` 一起把分布推向目标区间）。
const ARM_LATERAL_THRESHOLD: f64 = 0.19;

/// 躯干/腿部高度分界（决议 §8.1 #4，plan-combat-hit-location-v1 P1 校准）。
/// P0 起始值 0.35 → P1 实测**大幅上调**至 0.53（决议文本原文猜测方向是"如需可略降"，
/// 但直方图数据显示的是相反方向：`ATTACKER_EYE_HEIGHT=1.62` 已经贴近 Head 阈值，
/// 命中 Leg 需要的下压角远大于命中 Head 需要的上仰角，纯调 σ 会让 Head 提前撞
/// 8-12% 上限而 Leg 仍不到 15% 下限。上调本阈值把原本被 Abdomen 吞掉的中低段命中
/// 重新划给 Leg，使 Leg 落入 15-20% 目标区间，Abdomen 相应收窄为一个窄带（~3%，
/// 无强制区间，plan 只给了单值"15%"作参考，本次以数据为准接受偏差，见
/// `AIM_JITTER_PITCH_SIGMA_DEG` 注释的完整校准说明）。
const LEG_ABDOMEN_BOUNDARY: f64 = 0.53;

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
        if lateral.abs() > ARM_LATERAL_THRESHOLD {
            if lateral > 0.0 {
                BodyPart::ArmR
            } else {
                BodyPart::ArmL
            }
        } else {
            BodyPart::Chest
        }
    } else if rel_y > LEG_ABDOMEN_BOUNDARY {
        BodyPart::Abdomen
    } else if lateral > 0.0 {
        BodyPart::LegR
    } else {
        BodyPart::LegL
    }
}

/// 命中部位由 `aim_direction` 决定——调用方负责算出真实瞄准方向（决议 §8.1 #1）：
/// 玩家传真实 `Look` 转向向量（缺失时可退化为 [`chest_aim_direction`]）；
/// NPC 传 [`npc_aim_direction`] 算出的"目标几何中心 + 确定性 jitter"方向。
/// `raycast_humanoid` 本身不再内置恒定胸心 fallback，只做几何求交 + 部位分类。
pub fn raycast_humanoid(
    origin: DVec3,
    target_feet_position: DVec3,
    max_distance: f64,
    aim_direction: DVec3,
) -> Option<EntityHitProbe> {
    let aabb = standing_humanoid_aabb(target_feet_position);
    let hit = raycast_aabb(origin, aim_direction, max_distance, aabb)?;

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
        let origin = DVec3::new(0.0, 0.9, 0.0);
        let target = DVec3::new(2.0, 0.0, 0.0);
        let aim_direction = chest_aim_direction(origin, target);
        let probe = raycast_humanoid(origin, target, 3.0, aim_direction)
            .expect("front ray should hit humanoid");

        assert_eq!(probe.body_part, BodyPart::Chest);
        assert!(probe.distance <= 3.0);
    }

    #[test]
    fn raycast_humanoid_takes_caller_supplied_aim_no_internal_fallback() {
        // §P0 决议核心断言：raycast_humanoid 不再内置恒定胸心 fallback——
        // 玩家眼高(1.62)贴近目标头部阈值(rel_y>0.88≈1.584)，水平方向传入即可命中 Head，
        // 而不是不管方向如何都回落 Chest（旧实现内置恒定 CHEST_AIM_HEIGHT fallback 时的行为）。
        let origin = DVec3::new(0.0, 1.62, -2.0);
        let target = DVec3::new(0.0, 0.0, 0.0);
        let level_aim_direction = DVec3::new(0.0, 0.0, 1.0);
        let probe = raycast_humanoid(origin, target, 5.0, level_aim_direction)
            .expect("level ray at eye height should hit humanoid AABB");

        assert_eq!(
            probe.body_part,
            BodyPart::Head,
            "raycast_humanoid 必须尊重调用方传入的瞄准方向而非恒定命中 Chest"
        );
    }

    #[test]
    fn weapon_aim_jitter_scale_matches_decision_table_8_1() {
        // §8.1 #1 决议表：dagger 0.85 / sword 0.9 / fist 1.0 / spear·staff 1.1。
        assert_eq!(weapon_aim_jitter_scale(DAGGER_REACH), 0.85);
        assert_eq!(weapon_aim_jitter_scale(SWORD_REACH), 0.9);
        assert_eq!(weapon_aim_jitter_scale(FIST_REACH), 1.0);
        assert_eq!(weapon_aim_jitter_scale(SPEAR_REACH), 1.1);
        assert_eq!(weapon_aim_jitter_scale(STAFF_REACH), 1.1);
        // 未知/特殊技能 reach（非五档之一）保守取中性 1.0，不擅自收紧或放松。
        assert_eq!(
            weapon_aim_jitter_scale(AttackReach::new(3.7, 0.2)),
            1.0,
            "未匹配已知武器档位的 reach 应保守取中性缩放 1.0"
        );
    }

    #[test]
    fn npc_aim_direction_is_deterministic_for_same_seed() {
        // §8.1 #1/#3 决议核心：同一 attacker_id + 同一 combat_tick ⇒ 同一散布方向（可复现）。
        let origin = DVec3::new(0.0, 1.62, -2.0);
        let target = DVec3::new(0.0, 0.0, 0.0);
        let seed = npc_aim_seed("npc:zombie:1", 42);

        let first = npc_aim_direction(origin, target, seed, 1.0);
        let second = npc_aim_direction(origin, target, seed, 1.0);

        assert_eq!(
            first, second,
            "相同种子必须复现完全相同的瞄准方向（确定性 jitter 要求）"
        );
    }

    #[test]
    fn npc_aim_direction_varies_across_distinct_seeds() {
        // 不同 attacker_id / 不同 tick 应产生不同散布方向——否则 jitter 形同虚设。
        let origin = DVec3::new(0.0, 1.62, -2.0);
        let target = DVec3::new(0.0, 0.0, 0.0);

        let seed_a = npc_aim_seed("npc:zombie:1", 42);
        let seed_b = npc_aim_seed("npc:zombie:2", 42);
        let seed_c = npc_aim_seed("npc:zombie:1", 43);

        let dir_a = npc_aim_direction(origin, target, seed_a, 1.0);
        let dir_b = npc_aim_direction(origin, target, seed_b, 1.0);
        let dir_c = npc_aim_direction(origin, target, seed_c, 1.0);

        assert_ne!(
            dir_a, dir_b,
            "不同 attacker_id 在同一 tick 下应产生不同 jitter 方向"
        );
        assert_ne!(
            dir_a, dir_c,
            "同一 attacker_id 在不同 tick 下应产生不同 jitter 方向"
        );
    }

    #[test]
    fn npc_aim_jitter_scale_widens_spread_for_longer_reach_weapons() {
        // reach 越长散布越松：同一种子下 spear(1.1x) 的角度偏移幅度应大于 dagger(0.85x)。
        let origin = DVec3::new(0.0, 1.62, -2.0);
        let target = DVec3::new(0.0, 0.0, 0.0);
        let seed = npc_aim_seed("npc:zombie:widen", 7);
        let base_dir = chest_aim_direction(origin, target);

        let dagger_dir =
            npc_aim_direction(origin, target, seed, weapon_aim_jitter_scale(DAGGER_REACH));
        let spear_dir =
            npc_aim_direction(origin, target, seed, weapon_aim_jitter_scale(SPEAR_REACH));

        let angle_between = |a: DVec3, b: DVec3| -> f64 {
            (a.normalize().dot(b.normalize())).clamp(-1.0, 1.0).acos()
        };

        let dagger_deviation = angle_between(base_dir, dagger_dir);
        let spear_deviation = angle_between(base_dir, spear_dir);

        assert!(
            spear_deviation > dagger_deviation,
            "spear(σ×1.1) 相对基准方向的偏移角({spear_deviation:.4}rad) 应大于 dagger(σ×0.85) 的偏移角({dagger_deviation:.4}rad)"
        );
    }

    #[test]
    fn npc_aim_direction_batch_distribution_hits_all_seven_body_parts() {
        // 部位分布 pin：固定 seed 批量攻击，7 部位命中率各 > 0（决议 §8.1，废除恒瞄胸口）。
        let origin = DVec3::new(0.0, 1.62, -2.0);
        let target = DVec3::new(0.0, 0.0, 0.0);
        let mut counts: std::collections::HashMap<BodyPart, u32> = std::collections::HashMap::new();

        for tick in 0..3000u64 {
            let seed = npc_aim_seed("npc:zombie:distribution", tick);
            let aim_direction = npc_aim_direction(origin, target, seed, 1.0);
            if let Some(probe) = raycast_humanoid(origin, target, 5.0, aim_direction) {
                *counts.entry(probe.body_part).or_insert(0) += 1;
            }
        }

        for part in [
            BodyPart::Head,
            BodyPart::Chest,
            BodyPart::Abdomen,
            BodyPart::ArmL,
            BodyPart::ArmR,
            BodyPart::LegL,
            BodyPart::LegR,
        ] {
            assert!(
                counts.get(&part).copied().unwrap_or(0) > 0,
                "3000 次批量 NPC 攻击中部位 {part:?} 命中率为 0——散布 jitter 未能覆盖该部位（回归到恒瞄胸口）"
            );
        }
    }

    #[test]
    fn classify_body_part_from_elevated_pitch_hits_head() {
        // 俯视/仰视专项：攻方仰视（射线在目标处的高度接近头顶阈值）应命中 Head。
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(0.0, -3.0, -2.0);
        let hit_point = DVec3::new(0.0, 1.62, 0.0);

        assert_eq!(classify_body_part(hit_point, feet, origin), BodyPart::Head);
    }

    #[test]
    fn classify_body_part_from_depressed_pitch_hits_legs() {
        // 俯视专项：攻方居高临下压 pitch 瞄准目标腿部高度应命中 LegR 而非恒 Chest。
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(0.0, 3.0, -2.0);
        let hit_point = DVec3::new(-0.15, 0.15, 0.1);

        assert_eq!(classify_body_part(hit_point, feet, origin), BodyPart::LegR);
    }

    #[test]
    fn classify_body_part_from_lateral_yaw_hits_arm() {
        // 侧向命中专项：攻方沿侧向偏移瞄准应命中 ArmR 而非恒 Chest。
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(-2.0, 1.2, 0.0);
        let hit_point = DVec3::new(0.0, 1.2, 0.3);

        assert_eq!(classify_body_part(hit_point, feet, origin), BodyPart::ArmR);
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
        // P1 校准（决议 §8.1 #4）把 LEG_ABDOMEN_BOUNDARY 从 0.35 上调到 0.53——
        // rel_y=0.7/1.8≈0.389 曾经落在旧 Abdomen 区间 (0.35,0.55]，但在新阈值下
        // 已经跌破 0.53，改判 Leg。这里改用 rel_y=0.97/1.8≈0.539，落在新的
        // 窄 Abdomen 区间 (0.53,0.55] 内，继续钉住 Abdomen 分支存活。
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 0.97, 0.0), feet, origin),
            BodyPart::Abdomen,
            "rel_y≈0.539 应落在 P1 校准后的窄 Abdomen 区间 (0.53,0.55] 内"
        );
        // 旧阈值下会判 Abdomen 的 rel_y=0.389 现在必须落入 Leg（P1 校准的核心目的：
        // 把原本被 Abdomen 吞掉的中低段命中让给 Leg，冲高 Leg 命中率到目标区间）。
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 0.7, 0.0), feet, origin),
            BodyPart::LegL,
            "rel_y≈0.389 应在 P1 校准后的 LEG_ABDOMEN_BOUNDARY=0.53 之下改判 Leg（旧阈值 0.35 下曾是 Abdomen）"
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

    #[test]
    fn classify_body_part_arm_lateral_threshold_boundary_p1() {
        // P1 校准（决议 §8.1 #4）：ARM_LATERAL_THRESHOLD 从 0.18 微调到 0.19。
        // 专属边界 case：lateral 恰好落在新旧阈值之间（0.185）必须判 Chest（未超过新阈值），
        // 而 lateral=0.20（超过新阈值）必须判 ArmR——钉住新阈值本身生效，不是残留旧值。
        let feet = DVec3::new(0.0, 0.0, 0.0);
        let origin = DVec3::new(-2.0, 1.2, 0.0);
        // rel_y = 1.2/1.8 = 0.667，落在 Chest/Arm 判定带内（不受 leg 边界影响）。
        // 攻击方向沿 +x（origin z=0 == feet z=0）时 lateral 恰等于 hit.z。
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 1.2, 0.185), feet, origin),
            BodyPart::Chest,
            "lateral=0.185 未超过 P1 阈值 0.19，应仍判 Chest"
        );
        assert_eq!(
            classify_body_part(DVec3::new(0.0, 1.2, 0.20), feet, origin),
            BodyPart::ArmR,
            "lateral=0.20 超过 P1 阈值 0.19，应判 ArmR"
        );
    }

    #[test]
    fn npc_aim_direction_batch_distribution_hits_target_ranges_p1() {
        // P1 直方图校准 pin（决议 §8.1 #1/#4，plan-combat-hit-location-v1 P1）：
        // 固定 seed 批量 NPC 攻击（3000 次，fist 缩放，正面平视基线）落入目标分布区间——
        // 胸 40-50% / 头 8-12% / 臂(合计) 15-20% / 腿(合计) 15-20%。
        // Abdomen 无强制区间（plan 只给单值"15%"参考，AIM_JITTER_PITCH_SIGMA_DEG 注释
        // 解释了为何数据驱动校准接受 Abdomen 显著低于该参考值）——仅断言其 >0（分支存活）。
        let origin = DVec3::new(0.0, 1.62, -2.0);
        let target = DVec3::new(0.0, 0.0, 0.0);
        let mut counts: std::collections::HashMap<BodyPart, u32> = std::collections::HashMap::new();

        for tick in 0..3000u64 {
            let seed = npc_aim_seed("npc:combat_dist_p1", tick);
            let aim_direction = npc_aim_direction(origin, target, seed, 1.0);
            if let Some(probe) = raycast_humanoid(origin, target, 5.0, aim_direction) {
                *counts.entry(probe.body_part).or_insert(0) += 1;
            }
        }

        let total: u32 = counts.values().sum();
        assert!(total > 0, "批量攻击必须产生命中样本才能校验分布");
        let pct = |part: BodyPart| -> f64 {
            f64::from(counts.get(&part).copied().unwrap_or(0)) / f64::from(total) * 100.0
        };
        let chest_pct = pct(BodyPart::Chest);
        let head_pct = pct(BodyPart::Head);
        let arms_pct = pct(BodyPart::ArmL) + pct(BodyPart::ArmR);
        let legs_pct = pct(BodyPart::LegL) + pct(BodyPart::LegR);
        let abdomen_pct = pct(BodyPart::Abdomen);

        assert!(
            (40.0..=50.0).contains(&chest_pct),
            "胸部命中率应落在目标区间 40-50%，实际 {chest_pct:.1}%（校准依据见 AIM_JITTER_PITCH_SIGMA_DEG 注释）"
        );
        assert!(
            (8.0..=12.0).contains(&head_pct),
            "头部命中率应落在目标区间 8-12%，实际 {head_pct:.1}%"
        );
        assert!(
            (15.0..=20.0).contains(&arms_pct),
            "双臂合计命中率应落在目标区间 15-20%，实际 {arms_pct:.1}%"
        );
        assert!(
            (15.0..=20.0).contains(&legs_pct),
            "双腿合计命中率应落在目标区间 15-20%，实际 {legs_pct:.1}%"
        );
        assert!(
            abdomen_pct > 0.0,
            "腹部命中分支必须存活（>0），实际 {abdomen_pct:.1}%（无强制区间，见校准说明）"
        );
    }
}
