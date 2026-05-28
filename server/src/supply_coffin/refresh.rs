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
use crate::world::terrain::{SurfaceProvider, TerrainProviders};

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
    providers: Option<Res<TerrainProviders>>,
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

        let terrain = providers.as_ref().map(|p| &p.overworld);
        let Some(pos) = pick_valid_pos(&mut registry, terrain) else {
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

/// 地表上方放置偏移（+1 = 地面正上方一格）。
const SURFACE_Y_OFFSET: f64 = 1.0;
/// 棺上方必须至少有这么多格空气（避免卡进地形/树冠）。
const MIN_CLEARANCE_BLOCKS: i32 = 5;

/// 在 zone AABB 的 xz 区间随机选点，查询 `TerrainProvider` 获取真实地表 y 坐标，
/// 最多重试 `MAX_SPAWN_RETRY` 次。
///
/// 拒绝条件（任一命中即跳过该候选）：
/// - terrain 未加载（raster 缺失时完全不 spawn，避免刷到地下）
/// - `!passable`（水面 / 岩浆上方）
/// - surface y ≤ 该列的 water_y（低于本地水面，不用全局常量）
/// - 上方 `MIN_CLEARANCE_BLOCKS` 格内被相邻列地表遮挡
/// - 距已有棺位 < `MIN_SPACING_BLOCKS`
fn pick_valid_pos(
    registry: &mut SupplyCoffinRegistry,
    terrain: Option<&impl SurfaceProvider>,
) -> Option<DVec3> {
    let terrain = terrain?;

    let (min, max) = registry.zone_aabb;
    let span_x = (max.x - min.x).max(1.0);
    let span_z = (max.z - min.z).max(1.0);

    for _ in 0..MAX_SPAWN_RETRY {
        let r_x = registry.next_rand_u64();
        let r_z = registry.next_rand_u64();
        let nx = (r_x as f64) / (u64::MAX as f64);
        let nz = (r_z as f64) / (u64::MAX as f64);
        let cx = min.x + nx * span_x;
        let cz = min.z + nz * span_z;

        let info = terrain.query_surface(cx.floor() as i32, cz.floor() as i32);

        if !info.passable {
            continue;
        }
        if info.y <= info.water_y {
            continue;
        }

        // 头顶空间检查：采样 surface 上方若干格的相邻列，如果 surface provider
        // 报告邻居列的 surface 高度高于候选 y + clearance，说明头顶被遮挡。
        let spawn_y = f64::from(info.y) + SURFACE_Y_OFFSET;
        let ceiling_y = info.y + MIN_CLEARANCE_BLOCKS;
        let blocked =
            adjacent_columns_block(terrain, cx.floor() as i32, cz.floor() as i32, ceiling_y);
        if blocked {
            continue;
        }

        let candidate = DVec3::new(cx, spawn_y, cz);

        if registry.min_distance_to_active(candidate) >= MIN_SPACING_BLOCKS {
            return Some(candidate);
        }
    }
    None
}

/// 检查相邻 3x3 列的 surface 是否高于 `ceiling_y`。如果有，说明候选点
/// 被地形 / 巨树 / 洞穴顶部遮挡。
fn adjacent_columns_block(
    terrain: &impl SurfaceProvider,
    cx: i32,
    cz: i32,
    ceiling_y: i32,
) -> bool {
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            let neighbor = terrain.query_surface(cx + dx, cz + dz);
            if neighbor.y >= ceiling_y {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::SurfaceInfo;
    use valence::prelude::Entity;

    struct FlatSurface {
        y: i32,
        passable: bool,
        water_y: i32,
    }

    impl FlatSurface {
        fn new(y: i32) -> Self {
            Self {
                y,
                passable: true,
                water_y: i32::MIN,
            }
        }
        fn impassable(y: i32) -> Self {
            Self {
                y,
                passable: false,
                water_y: i32::MIN,
            }
        }
        fn with_water(y: i32, water_y: i32) -> Self {
            Self {
                y,
                passable: y > water_y,
                water_y,
            }
        }
    }

    impl SurfaceProvider for FlatSurface {
        fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
            SurfaceInfo {
                y: self.y,
                passable: self.passable,
                water_y: self.water_y,
            }
        }
    }

    struct SlopeSurface;

    impl SurfaceProvider for SlopeSurface {
        fn query_surface(&self, x: i32, z: i32) -> SurfaceInfo {
            SurfaceInfo {
                y: 75 + (x.abs() + z.abs()) % 50,
                passable: true,
                water_y: i32::MIN,
            }
        }
    }

    /// 模拟四面环壁的小山谷：仅 (0..2, 0..2) 是低地（y=80），外围全是
    /// y=200 峭壁。任何落在谷底的候选点其邻居至少有一列 >= ceiling。
    struct BoxedValley;

    impl SurfaceProvider for BoxedValley {
        fn query_surface(&self, x: i32, z: i32) -> SurfaceInfo {
            let in_valley = (0..=1).contains(&x) && (0..=1).contains(&z);
            SurfaceInfo {
                y: if in_valley { 80 } else { 200 },
                passable: true,
                water_y: i32::MIN,
            }
        }
    }

    fn make_registry() -> SupplyCoffinRegistry {
        SupplyCoffinRegistry::new(
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 0.0, 100.0)),
            65.0,
            42,
        )
    }

    #[test]
    fn pick_valid_pos_uses_terrain_surface_height() {
        let surface_y = 120;
        let terrain = FlatSurface::new(surface_y);
        let mut r = make_registry();
        let pos = pick_valid_pos(&mut r, Some(&terrain));
        assert!(pos.is_some(), "空 registry + 高地表应能选到点");
        let pos = pos.unwrap();
        let expected_y = f64::from(surface_y) + SURFACE_Y_OFFSET;
        assert!(
            (pos.y - expected_y).abs() < 1e-6,
            "y 应为地表高度({surface_y}) + 偏移({SURFACE_Y_OFFSET}) = {expected_y}，实际 {}",
            pos.y
        );
        assert!(
            (0.0..=100.0).contains(&pos.x) && (0.0..=100.0).contains(&pos.z),
            "pos xz 必须落在 AABB 内：{:?}",
            pos
        );
    }

    #[test]
    fn pick_valid_pos_returns_none_without_terrain() {
        let mut r = make_registry();
        let pos = pick_valid_pos(&mut r, None::<&FlatSurface>);
        assert!(
            pos.is_none(),
            "terrain 不可用时必须返回 None（不允许 fallback 到盲猜 y）"
        );
    }

    #[test]
    fn pick_valid_pos_rejects_at_or_below_local_water_level() {
        // 模拟不同区域有不同水位
        for (surface_y, water_y) in [(60, 64), (70, 70), (50, 80)] {
            let terrain = FlatSurface::with_water(surface_y, water_y);
            let mut r = make_registry();
            let pos = pick_valid_pos(&mut r, Some(&terrain));
            assert!(
                pos.is_none(),
                "surface y={surface_y} ≤ water_y={water_y} 应被拒绝；实际 {:?}",
                pos
            );
        }
    }

    #[test]
    fn pick_valid_pos_accepts_above_local_water_level() {
        let terrain = FlatSurface::with_water(80, 64);
        let mut r = make_registry();
        let pos = pick_valid_pos(&mut r, Some(&terrain));
        assert!(pos.is_some(), "surface y=80 高于 water_y=64 应被接受");
    }

    #[test]
    fn pick_valid_pos_rejects_impassable_terrain() {
        let terrain = FlatSurface::impassable(80);
        let mut r = make_registry();
        let pos = pick_valid_pos(&mut r, Some(&terrain));
        assert!(
            pos.is_none(),
            "impassable 地形（水下/岩浆）应被拒绝；实际 {:?}",
            pos
        );
    }

    #[test]
    fn pick_valid_pos_rejects_obstructed_clearance() {
        let terrain = BoxedValley;
        let mut r = SupplyCoffinRegistry::new(
            // AABB 限制在谷底范围内，所有候选点 floor 后都落在 (0..2, 0..2)
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 2.0)),
            65.0,
            42,
        );
        let pos = pick_valid_pos(&mut r, Some(&terrain));
        assert!(
            pos.is_none(),
            "四面环壁山谷中，谷底候选点邻居至少有一列 y=200 >= ceiling=85；实际 {:?}",
            pos
        );
    }

    #[test]
    fn pick_valid_pos_returns_some_for_empty_registry_above_sea_level() {
        let terrain = FlatSurface::new(80);
        let mut r = make_registry();
        let pos = pick_valid_pos(&mut r, Some(&terrain));
        assert!(pos.is_some(), "空 registry + 高地表应能选到点");
        let pos = pos.unwrap();
        let expected_y = 80.0 + SURFACE_Y_OFFSET;
        assert!(
            (0.0..=100.0).contains(&pos.x)
                && (pos.y - expected_y).abs() < 1e-6
                && (0.0..=100.0).contains(&pos.z),
            "pos 必须落在 AABB 内且 y 匹配地表：{:?}",
            pos
        );
    }

    #[test]
    fn pick_valid_pos_returns_none_when_zone_saturated() {
        let terrain = FlatSurface::new(80);
        let mut r = SupplyCoffinRegistry::new(
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 5.0)),
            65.0,
            42,
        );
        let y = 80.0 + SURFACE_Y_OFFSET;
        r.insert_active(
            Entity::from_raw(1),
            SupplyCoffinGrade::Common,
            DVec3::new(2.5, y, 2.5),
            0,
        );
        let pos = pick_valid_pos(&mut r, Some(&terrain));
        assert!(
            pos.is_none(),
            "5x5 AABB 中心已占据时不可能找到 >=10 块远的点；实际 {:?}",
            pos
        );
    }

    #[test]
    fn pick_valid_pos_respects_min_spacing_when_solvable() {
        let terrain = FlatSurface::new(80);
        let mut r = make_registry();
        let y = 80.0 + SURFACE_Y_OFFSET;
        r.insert_active(
            Entity::from_raw(1),
            SupplyCoffinGrade::Common,
            DVec3::new(50.0, y, 50.0),
            0,
        );
        let pos =
            pick_valid_pos(&mut r, Some(&terrain)).expect("100x100 AABB 应能找到 >=10 远的点");
        let d = pos.distance(DVec3::new(50.0, y, 50.0));
        assert!(
            d >= MIN_SPACING_BLOCKS,
            "pick_valid_pos 必须遵守最小间距 {}：实际距离 {}",
            MIN_SPACING_BLOCKS,
            d
        );
    }

    #[test]
    fn pick_valid_pos_uses_per_column_height_on_varied_terrain() {
        let terrain = SlopeSurface;
        let mut r = SupplyCoffinRegistry::new(
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(200.0, 0.0, 200.0)),
            65.0,
            42,
        );
        for _ in 0..5 {
            let pos =
                pick_valid_pos(&mut r, Some(&terrain)).expect("200x200 空 registry 必能选到点");
            let expected_info = terrain.query_surface(pos.x.floor() as i32, pos.z.floor() as i32);
            let expected_y = f64::from(expected_info.y) + SURFACE_Y_OFFSET;
            assert!(
                (pos.y - expected_y).abs() < 1e-6,
                "y 应匹配 terrain.query_surface 返回值 + 偏移：期望 {expected_y}，实际 {}，\
                 at xz=({:.0}, {:.0})",
                pos.y,
                pos.x,
                pos.z
            );
            r.insert_active(
                Entity::from_raw(r.active.len() as u32 + 1),
                SupplyCoffinGrade::Common,
                pos,
                0,
            );
        }
    }

    #[test]
    fn pick_valid_pos_all_accepted_above_water_level() {
        let water_y = 72;
        let terrain = FlatSurface::with_water(80, water_y);
        let mut r = SupplyCoffinRegistry::new(
            (DVec3::new(0.0, 0.0, 0.0), DVec3::new(500.0, 0.0, 500.0)),
            65.0,
            42,
        );
        for i in 0..20 {
            let pos = pick_valid_pos(&mut r, Some(&terrain))
                .unwrap_or_else(|| panic!("第 {} 次选点不应失败（500x500 AABB 足够大）", i));
            assert!(
                pos.y > f64::from(water_y),
                "所有被接受的候选点 y={} 必须高于本地水面 {}",
                pos.y,
                water_y
            );
            r.insert_active(
                Entity::from_raw(i as u32 + 1),
                SupplyCoffinGrade::Common,
                pos,
                0,
            );
        }
    }

    #[test]
    fn adjacent_columns_block_detects_tall_neighbor() {
        let terrain = BoxedValley;
        // (1, 1) 谷底 y=80；邻居 (2, 1) 谷外 y=200
        assert!(
            adjacent_columns_block(&terrain, 1, 1, 85),
            "邻居 (2,1) surface=200 >= ceiling=85 应判定遮挡"
        );
    }

    #[test]
    fn adjacent_columns_block_passes_when_all_neighbors_low() {
        let terrain = FlatSurface::new(70);
        assert!(
            !adjacent_columns_block(&terrain, 50, 50, 75),
            "所有邻居 surface=70 < ceiling=75 不应判定遮挡"
        );
    }

    #[test]
    fn adjacent_columns_block_boundary_at_ceiling_y() {
        let terrain = FlatSurface::new(85);
        // neighbor.y == ceiling_y → blocked (>= comparison)
        assert!(
            adjacent_columns_block(&terrain, 50, 50, 85),
            "邻居 surface=85 == ceiling=85 应判定遮挡（>= 语义）"
        );
    }

    #[test]
    fn adjacent_columns_block_boundary_just_below_ceiling() {
        let terrain = FlatSurface::new(84);
        // neighbor.y == ceiling_y - 1 → not blocked
        assert!(
            !adjacent_columns_block(&terrain, 50, 50, 85),
            "邻居 surface=84 < ceiling=85 不应判定遮挡"
        );
    }
}
