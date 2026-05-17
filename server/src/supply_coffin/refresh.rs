//! plan-supply-coffin-v1 P2.3/P2.4 — 物资棺刷新 tick + 涌现视听。
//!
//! - 每 Update tick 检查各档 active count + cooldown 是否到期；满足条件就在
//!   zone AABB 内随机选点 spawn 一具新棺，并 emit `supply_coffin_emerge`
//!   音效 + `bong:supply_coffin_emerge` 粒子事件
//! - 初始刷新：active < max_active 且无任何冷却记录 → 视为"启动初始化"，
//!   不需等冷却直接 spawn。所以服务器重启后会先填满 max_active 个棺。
//! - 选点失败超过 20 次：把首个匹配 grade 的冷却推迟 60s，下 tick 再试。

use bevy_transform::components::{GlobalTransform, Transform};
use valence::entity::entity::NoGravity;
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, EntityLayerId, EventWriter, Look, Position, Res, ResMut,
};

use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_AREA_RADIUS};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::dimension::DimensionLayers;
use crate::world::entity_model::{BongVisualEntity, BongVisualState};

use super::{current_wall_clock_secs, SupplyCoffinGrade, SupplyCoffinRegistry};

/// 选点最大重试次数（plan §P2.3）。
const MAX_SPAWN_RETRY: usize = 20;
/// 同棺最小间距（plan §P2.3：避免堆叠在同一格附近）。
const MIN_SPACING_BLOCKS: f64 = 10.0;
/// 选点全失败时延后下次重试的秒数（plan §P2.3）。
const RETRY_DELAY_SECS: u64 = 60;

/// 标记某个 ECS Entity 是物资棺 marker。挂在 spawn 出的棺 entity 上，配合
/// `SupplyCoffinRegistry.active` 双向验证（registry 是 source-of-truth）。
///
/// `grade` 字段供 dev 命令 / debug 列举使用；当前 interact / refresh 路径都从
/// registry 取 grade，不读 marker——`#[allow(dead_code)]` 直到 dev cmd 实装。
#[derive(Debug, Clone, Copy, Component)]
pub struct SupplyCoffinMarker {
    #[allow(dead_code)]
    pub grade: SupplyCoffinGrade,
}

/// Update 系统：尝试为各档刷新一具新棺。
pub fn supply_coffin_refresh_tick(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    mut registry: ResMut<SupplyCoffinRegistry>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
    mut vfx: EventWriter<VfxEventRequest>,
) {
    let Some(layers) = layers else {
        return;
    };
    let now = current_wall_clock_secs();

    for grade in SupplyCoffinGrade::ALL {
        if registry.active_count(grade) >= grade.max_active() {
            continue;
        }

        let any_cooldown = registry.cooldowns.iter().any(|c| c.grade == grade);
        let ready_cooldown = registry
            .cooldowns
            .iter()
            .any(|c| c.grade == grade && c.is_ready(now));

        // 只在两种情况下 spawn：
        //   ① 无任何冷却记录（initial fill 或 reset 后）
        //   ② 存在冷却且其中一条已到期
        // 否则 spawn 必须等冷却到期。
        if any_cooldown && !ready_cooldown {
            continue;
        }

        let Some(pos) = pick_valid_pos(&mut registry) else {
            if ready_cooldown {
                registry.delay_oldest_cooldown(grade, RETRY_DELAY_SECS);
            }
            continue;
        };

        let visual_kind = grade.visual_kind();
        let entity = commands
            .spawn((
                MarkerEntityBundle {
                    kind: visual_kind.entity_kind(),
                    layer: EntityLayerId(layers.overworld),
                    position: Position::new([pos.x, pos.y, pos.z]),
                    entity_no_gravity: NoGravity(true),
                    look: Look::new(0.0, 0.0),
                    ..Default::default()
                },
                Transform::from_xyz(pos.x as f32, pos.y as f32, pos.z as f32),
                GlobalTransform::default(),
                BongVisualEntity {
                    kind: visual_kind,
                    source: None,
                },
                BongVisualState(0),
                SupplyCoffinMarker { grade },
            ))
            .id();

        registry.insert_active(entity, grade, pos, now);
        if ready_cooldown {
            registry.pop_ready_cooldown(grade, now);
        }

        // 涌现视听
        audio.send(PlaySoundRecipeRequest {
            recipe_id: "supply_coffin_emerge".to_string(),
            instance_id: 0,
            pos: Some([pos.x as i32, pos.y as i32, pos.z as i32]),
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin: pos,
                radius: AUDIO_AREA_RADIUS,
            },
        });
        vfx.send(VfxEventRequest::new(
            pos,
            VfxEventPayloadV1::SpawnParticle {
                event_id: "bong:supply_coffin_emerge".to_string(),
                origin: [pos.x, pos.y, pos.z],
                direction: None,
                color: Some("#A08050".to_string()),
                strength: None,
                count: Some(6),
                duration_ticks: Some(25),
            },
        ));

        tracing::debug!(
            "[bong][supply_coffin] spawned {:?} at ({:.1},{:.1},{:.1}) active={} cooldown_q={}",
            grade,
            pos.x,
            pos.y,
            pos.z,
            registry.active_count(grade),
            registry.cooldowns.len()
        );
    }
}

/// 在 zone AABB 的 xz 区间 + 固定 `spawn_y` 随机选点，最多重试 `MAX_SPAWN_RETRY`
/// 次，距已有棺位 < `MIN_SPACING_BLOCKS` 时拒绝。
///
/// y 当前用 `registry.spawn_y` 常数（plan ChunkLayer ground-height 查询 deferred；
/// 见 `## Finish Evidence` 遗留项）。
fn pick_valid_pos(registry: &mut SupplyCoffinRegistry) -> Option<DVec3> {
    let (min, max) = registry.zone_aabb;
    let y = registry.spawn_y;
    let span_x = (max.x - min.x).max(1.0);
    let span_z = (max.z - min.z).max(1.0);

    for _ in 0..MAX_SPAWN_RETRY {
        let r_x = registry.next_rand_u64();
        let r_z = registry.next_rand_u64();
        // u64::MAX → 1.0；这里允许 x/z 上下游 inclusive。
        let nx = (r_x as f64) / (u64::MAX as f64);
        let nz = (r_z as f64) / (u64::MAX as f64);
        let candidate = DVec3::new(min.x + nx * span_x, y, min.z + nz * span_z);

        if registry.min_distance_to_active(candidate) >= MIN_SPACING_BLOCKS {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Entity;

    #[test]
    fn pick_valid_pos_returns_some_for_empty_registry() {
        let mut r = SupplyCoffinRegistry::new(
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 0.0, 100.0)),
            65.0,
            42,
        );
        let pos = pick_valid_pos(&mut r);
        assert!(pos.is_some(), "空 registry 任何 xz 位置都合法");
        let pos = pos.unwrap();
        assert!(
            (0.0..=100.0).contains(&pos.x) && pos.y == 65.0 && (0.0..=100.0).contains(&pos.z),
            "pos 必须落在 AABB 内：{:?}",
            pos
        );
    }

    #[test]
    fn pick_valid_pos_returns_none_when_zone_saturated() {
        // AABB = 5x5；如果整个区域用 0.5 格密度填满 active，无论怎么 roll 都
        // 不可能找到 ≥10 块远的位置 → 返回 None
        let mut r = SupplyCoffinRegistry::new(
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 5.0)),
            65.0,
            42,
        );
        // 在 AABB 中心放一个 active，整个 AABB 都在 5*1.41 ≈ 7 < 10 范围内
        r.insert_active(
            Entity::from_raw(1),
            SupplyCoffinGrade::Common,
            DVec3::new(2.5, 65.0, 2.5),
            0,
        );
        let pos = pick_valid_pos(&mut r);
        assert!(
            pos.is_none(),
            "5x5 AABB 中心已占据时不可能找到 >=10 块远的点；实际 {:?}",
            pos
        );
    }

    #[test]
    fn pick_valid_pos_respects_min_spacing_when_solvable() {
        // AABB 100x100，center 已占；新点必须距 center >= 10
        let mut r = SupplyCoffinRegistry::new(
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 0.0, 100.0)),
            65.0,
            42,
        );
        r.insert_active(
            Entity::from_raw(1),
            SupplyCoffinGrade::Common,
            DVec3::new(50.0, 65.0, 50.0),
            0,
        );
        let pos = pick_valid_pos(&mut r).expect("100x100 AABB 应能找到 >=10 远的点");
        let d = pos.distance(DVec3::new(50.0, 65.0, 50.0));
        assert!(
            d >= MIN_SPACING_BLOCKS,
            "pick_valid_pos 必须遵守最小间距 {}：实际距离 {}",
            MIN_SPACING_BLOCKS,
            d
        );
    }
}
