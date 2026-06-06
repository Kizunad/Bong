//! 负灵域诡影（plan-neg-domain-fauna-v1 P0）。
//!
//! 诡影是负灵域专属的 server-only 漂移实体（**不发 SpawnEntity MC packet**，
//! 无 HP/Damageable，不可攻击，仅对玩家生效）。
//!
//! 三个系统：
//! - [`ghost_spawn_system`]：spirit_qi < 0 时按密度公式 spawn，per-zone ≤ 10 上限。
//! - [`ghost_drift_system`]：每 tick 更新位置，每 60 tick 重采样方向；AABB clamp 防跨 zone。
//! - [`ghost_contact_system`]：距离 < `GHOST_SIPHON_RADIUS` 时触发接触，
//!   扣玩家真元守恒流入 zone，多诡影共享 1s cooldown 防叠爆。
//!
//! 真元守恒红线：所有扣量通过 `QiTransfer{reason: GhostContact}` 流向 zone 账户，
//! **严禁** `cultivation.qi_current -= X` 无对应 zone 增。

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, Events, Position, Query, Res, ResMut, Resource,
    Without,
};

use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::tick::CultivationClock;
use crate::npc::spawn::common::NpcMarker;
use crate::qi_physics::constants::{GHOST_CONTACT_FACTOR, QI_EPSILON};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

// ── 模块常量（生成/几何参数，非真元物理率；留本模块不归 qi_physics） ──

/// 诡影每格漂移速度（每 tick 移动格数）。
pub const GHOST_DRIFT_SPEED: f64 = 0.05;
/// 每 60 tick 重采样漂移方向一次。
pub const GHOST_DRIFT_RESAMPLE_TICKS: u64 = 60;
/// 诡影 siphon 触发半径（格）。
pub const GHOST_SIPHON_RADIUS: f64 = 2.0;
/// 每 zone 诡影上限。
pub const GHOST_MAX_PER_ZONE: usize = 10;
/// 密度公式：target_count = floor(|spirit_qi| × GHOST_DENSITY_FACTOR).min(GHOST_MAX_PER_ZONE)。
pub const GHOST_DENSITY_FACTOR: f64 = 5.0;
/// 诡影刷出最小 spirit_qi 压力（负值）。仅影响 spawn 密度公式，负灵域定义为 spirit_qi < 0。
pub const GHOST_SPAWN_MIN_PRESSURE: f64 = -0.2;
/// 多诡影共享 cooldown 时长（ticks，1 秒 = 20 tick）。
pub const GHOST_COOLDOWN_TICKS: u64 = 20;

// ── 数据类型 ──

/// 诡影实体数据。挂在 ECS Entity 上，但 **不** 对应 MC 可见实体。
#[derive(Debug, Clone, Component)]
pub struct GhostEntity {
    /// 诡影当前位置（服务端坐标）。
    pub position: DVec3,
    /// 当前漂移速度方向（已归一化或零向量）。
    pub drift_velocity: DVec3,
    /// 所属 zone 名称（用于 AABB clamp + 守恒 zone 账户 ID）。
    pub zone_name: String,
    /// 内部 tick 计数，用于定时重采样方向。
    pub tick_counter: u64,
}

/// 玩家级诡影接触 cooldown。共享 cooldown：任一诡影触发后 1s 内其他诡影不重复触发。
#[derive(Debug, Clone, Copy, Component)]
pub struct GhostContactCooldown {
    /// 最近一次接触发生的 tick（服务端 tick_count）。
    pub last_contact_tick: u64,
}

/// 全服诡影注册表。zone_name → 该 zone 的活跃诡影 Entity 列表。
#[derive(Debug, Default, Resource)]
pub struct GhostZoneRegistry {
    pub zones: HashMap<String, Vec<Entity>>,
}

impl GhostZoneRegistry {
    /// 在指定 zone 注册一个新诡影 entity。
    pub fn register(&mut self, zone_name: &str, entity: Entity) {
        self.zones
            .entry(zone_name.to_string())
            .or_default()
            .push(entity);
    }

    /// 返回指定 zone 的当前诡影数量。
    pub fn count(&self, zone_name: &str) -> usize {
        self.zones.get(zone_name).map(|v| v.len()).unwrap_or(0)
    }

    /// 清理已被 despawn 的 entity（entity 不在 world 中则移除）。
    /// 调用方需确保传入的活跃 entity 集合；此处对比移除失效条目。
    pub fn remove_entity(&mut self, zone_name: &str, entity: Entity) {
        if let Some(list) = self.zones.get_mut(zone_name) {
            list.retain(|&e| e != entity);
        }
    }
}

// ── 系统 ──

/// ghost_spawn_system：在 spirit_qi < 0 的 zone 中按密度公式 spawn 诡影。
///
/// target_count = floor(|spirit_qi| × GHOST_DENSITY_FACTOR).min(GHOST_MAX_PER_ZONE)
/// 若 spirit_qi >= GHOST_SPAWN_MIN_PRESSURE，不生成（压力不足）。
/// **不发 SpawnEntity MC packet。**
pub fn ghost_spawn_system(
    zones: Option<Res<ZoneRegistry>>,
    mut registry: ResMut<GhostZoneRegistry>,
    mut commands: Commands,
    tick: Option<Res<CultivationClock>>,
) {
    let Some(zones) = zones else {
        return;
    };
    let tick_seed = tick.as_deref().map(|t| t.tick).unwrap_or(0);

    for zone in &zones.zones {
        if zone.spirit_qi >= 0.0 {
            // 正灵域：无诡影
            continue;
        }
        let pressure = -zone.spirit_qi;
        if pressure < GHOST_SPAWN_MIN_PRESSURE.abs() {
            // 压力不足 0.2，不触发 spawn
            continue;
        }
        let target_count = (pressure * GHOST_DENSITY_FACTOR)
            .floor()
            .min(GHOST_MAX_PER_ZONE as f64) as usize;
        let current_count = registry.count(&zone.name);
        if current_count >= target_count {
            continue;
        }
        let to_spawn = target_count.saturating_sub(current_count);
        let center = zone.center();
        let (min, max) = zone.bounds;

        for i in 0..to_spawn {
            // 简单确定性散布：用 tick + zone_name_hash + i 作为伪随机种子
            let seed = tick_seed
                .wrapping_add(i as u64)
                .wrapping_mul(0x9E3779B97F4A7C15)
                .wrapping_add(
                    zone.name
                        .bytes()
                        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64)),
                );
            let offset_x = ((seed & 0xFF) as f64 / 255.0 - 0.5) * (max.x - min.x).abs();
            let offset_z = (((seed >> 8) & 0xFF) as f64 / 255.0 - 0.5) * (max.z - min.z).abs();
            let spawn_pos = DVec3::new(
                (center.x + offset_x).clamp(min.x, max.x),
                center.y,
                (center.z + offset_z).clamp(min.z, max.z),
            );

            // 初始漂移方向：随机单位向量（xz 平面）
            let angle = (seed as f64 / u64::MAX as f64) * std::f64::consts::TAU;
            let drift = DVec3::new(
                angle.cos() * GHOST_DRIFT_SPEED,
                0.0,
                angle.sin() * GHOST_DRIFT_SPEED,
            );

            let ghost = GhostEntity {
                position: spawn_pos,
                drift_velocity: drift,
                zone_name: zone.name.clone(),
                tick_counter: 0,
            };

            let entity = commands.spawn(ghost).id();
            registry.register(&zone.name, entity);
        }
    }
}

/// ghost_drift_system：每 tick 更新诡影位置，每 60 tick 重采样方向，AABB clamp 防跨 zone。
pub fn ghost_drift_system(zones: Option<Res<ZoneRegistry>>, mut ghosts: Query<&mut GhostEntity>) {
    let Some(zones) = zones else {
        return;
    };

    for mut ghost in ghosts.iter_mut() {
        // 更新 tick 计数
        ghost.tick_counter = ghost.tick_counter.wrapping_add(1);

        // 每 GHOST_DRIFT_RESAMPLE_TICKS 重采样漂移方向
        if ghost.tick_counter % GHOST_DRIFT_RESAMPLE_TICKS == 0 {
            let seed = ghost.tick_counter.wrapping_mul(0x6C62272E07BB0142);
            let angle = (seed as f64 / u64::MAX as f64) * std::f64::consts::TAU;
            ghost.drift_velocity = DVec3::new(
                angle.cos() * GHOST_DRIFT_SPEED,
                0.0,
                angle.sin() * GHOST_DRIFT_SPEED,
            );
        }

        // 更新位置
        let new_pos = ghost.position + ghost.drift_velocity;

        // AABB clamp：不跨 zone 边界
        if let Some(zone) = zones.find_zone_by_name(&ghost.zone_name) {
            ghost.position = zone.clamp_position(new_pos);
        } else {
            // zone 已消失；保持原位不移动
        }
    }
}

/// ghost_contact_system：检测诡影与玩家距离，触发真元抽取。
///
/// - 仅对玩家生效（`Without<NpcMarker>` 过滤掉 NPC）
/// - 多诡影共享 cooldown（per-player 1s 内只触发一次）
/// - pulse_amount = |zone_qi| × qi_max × GHOST_CONTACT_FACTOR
/// - 通过 `QiTransfer{reason: GhostContact}` 守恒流向 zone 账户
#[allow(clippy::type_complexity)]
pub fn ghost_contact_system(
    zones: Option<Res<ZoneRegistry>>,
    mut qi_transfer_events: Option<ResMut<Events<QiTransfer>>>,
    ghosts: Query<&GhostEntity>,
    mut players: Query<
        (
            Entity,
            &Position,
            Option<&CurrentDimension>,
            Option<&LifeRecord>,
            &mut Cultivation,
            Option<&mut GhostContactCooldown>,
        ),
        Without<NpcMarker>,
    >,
    tick: Option<Res<CultivationClock>>,
    mut commands: Commands,
) {
    let Some(zones) = zones else {
        return;
    };
    let current_tick = tick.as_deref().map(|t| t.tick).unwrap_or(0);

    for (entity, pos, current_dim, life_record, mut cultivation, cooldown) in players.iter_mut() {
        // 检查 cooldown（共享：任一诡影触发后 1s 内不重复）
        if let Some(cd) = cooldown.as_ref() {
            if current_tick.saturating_sub(cd.last_contact_tick) < GHOST_COOLDOWN_TICKS {
                continue;
            }
        }

        // 确定玩家所在 zone 的 spirit_qi
        let player_zone = current_dim
            .and_then(|dim| zones.find_zone(dim.0, pos.0))
            .filter(|z| z.spirit_qi < 0.0);
        let Some(zone) = player_zone else {
            continue; // 不在负灵域，跳过
        };
        let zone_qi = zone.spirit_qi;
        let zone_name = zone.name.clone();

        // 检查是否有诡影在 siphon 半径内
        let mut triggered = false;
        for ghost in ghosts.iter() {
            if ghost.zone_name != zone_name {
                continue;
            }
            let dist = (ghost.position - pos.0).length();
            if dist < GHOST_SIPHON_RADIUS {
                triggered = true;
                break;
            }
        }

        if !triggered {
            continue;
        }

        // 计算 pulse_amount = |zone_qi| × qi_max × GHOST_CONTACT_FACTOR
        let pulse_amount = (-zone_qi) * cultivation.qi_max * GHOST_CONTACT_FACTOR;
        if pulse_amount <= QI_EPSILON {
            continue;
        }

        // 扣真元（优先从 qi_current 扣；不足则扣到 0）
        let actual_pulse = pulse_amount.min(cultivation.qi_current.max(0.0));
        cultivation.qi_current = (cultivation.qi_current - actual_pulse).max(0.0);

        // 守恒流向 zone 账户：emit QiTransfer 事件
        if actual_pulse > QI_EPSILON {
            let player_account_id = life_record
                .map(|lr| QiAccountId::player(&lr.character_id))
                .unwrap_or_else(|| QiAccountId::player(format!("{entity:?}")));
            let zone_account_id = QiAccountId::zone(&zone_name);

            if let Ok(transfer) = QiTransfer::new(
                player_account_id,
                zone_account_id,
                actual_pulse,
                QiTransferReason::GhostContact,
            ) {
                if let Some(events) = qi_transfer_events.as_deref_mut() {
                    events.send(transfer);
                }
            }
        }

        // 更新 cooldown（insert 或 update）
        commands.entity(entity).insert(GhostContactCooldown {
            last_contact_tick: current_tick,
        });
    }
}

// ── 单测 ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Cultivation;
    use crate::player::state::canonical_player_id;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use valence::prelude::{App, Events, Update};

    // ── helper ──

    fn make_app_with_neg_zone(spirit_qi: f64) -> App {
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = spirit_qi;
        app.insert_resource(zones);
        app.init_resource::<GhostZoneRegistry>();
        app
    }

    // ── P0 单测：QiTransfer 守恒 / pulse 公式 ──

    #[test]
    fn ghost_contact_factor_used_in_pulse_formula() {
        // pulse = |zone_qi| × qi_max × GHOST_CONTACT_FACTOR
        // zone_qi=-0.5, qi_max=100 → pulse=0.5×100×0.05=2.5
        let zone_qi = -0.5_f64;
        let qi_max = 100.0_f64;
        let expected = (-zone_qi) * qi_max * GHOST_CONTACT_FACTOR;
        assert!(
            (expected - 2.5).abs() < QI_EPSILON,
            "pulse 公式期望 2.5，实际 {expected}"
        );
    }

    #[test]
    fn ghost_zone_registry_count_tracks_spawned_entities() {
        // GhostZoneRegistry::count 必须准确反映注册数量
        let mut registry = GhostZoneRegistry::default();
        let e1 = Entity::from_raw(1);
        let e2 = Entity::from_raw(2);

        assert_eq!(registry.count("neg_zone"), 0, "空注册表 count 应为 0");
        registry.register("neg_zone", e1);
        assert_eq!(registry.count("neg_zone"), 1, "注册 1 个后 count 应为 1");
        registry.register("neg_zone", e2);
        assert_eq!(registry.count("neg_zone"), 2, "注册 2 个后 count 应为 2");
    }

    #[test]
    fn ghost_zone_registry_remove_entity_reduces_count() {
        // remove_entity 后 count 减少，被移除的 entity 不再出现在 zone 列表
        let mut registry = GhostZoneRegistry::default();
        let e1 = Entity::from_raw(1);
        let e2 = Entity::from_raw(2);
        registry.register("neg_zone", e1);
        registry.register("neg_zone", e2);
        registry.remove_entity("neg_zone", e1);
        assert_eq!(registry.count("neg_zone"), 1, "移除 1 个后 count 应为 1");
        // e2 仍在
        assert!(
            registry.zones["neg_zone"].contains(&e2),
            "e2 应仍在注册表中"
        );
    }

    #[test]
    fn ghost_zone_registry_max_per_zone_enforced_logically() {
        // GhostZoneRegistry 本身不限制 count，限制在 spawn_system；
        // pin 测试：GHOST_MAX_PER_ZONE 设计决议为 10（per-zone 上限 ≤ 10）。
        assert_eq!(
            GHOST_MAX_PER_ZONE, 10,
            "GHOST_MAX_PER_ZONE 应为 10（plan-neg-domain-fauna-v1 §6 设计决议），实际 {GHOST_MAX_PER_ZONE}"
        );
        // GHOST_DENSITY_FACTOR=5.0，spirit_qi=-1.0 → target=5 ≤ 10（不超过上限）
        let target_at_full_pressure = (1.0_f64 * GHOST_DENSITY_FACTOR)
            .floor()
            .min(GHOST_MAX_PER_ZONE as f64) as usize;
        assert_eq!(
            target_at_full_pressure, 5,
            "spirit_qi=-1.0 时 target_count 应为 5（floor(1.0×5.0)=5），实际 {target_at_full_pressure}"
        );
    }

    #[test]
    fn ghost_spawn_no_spawn_in_positive_zone() {
        // 正灵域（spirit_qi >= 0）不 spawn 诡影
        let mut app = make_app_with_neg_zone(0.5);
        app.add_systems(Update, ghost_spawn_system);
        app.update();

        let registry = app.world().resource::<GhostZoneRegistry>();
        let total: usize = registry.zones.values().map(|v| v.len()).sum();
        assert_eq!(
            total, 0,
            "正灵域（spirit_qi=0.5）不应有诡影，实际 {total} 个"
        );
    }

    #[test]
    fn ghost_spawn_creates_ghosts_in_negative_zone() {
        // spirit_qi=-0.5 → target = floor(0.5 × 5.0) = 2（密度公式，压力 0.5 ≥ 0.2）
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, ghost_spawn_system);
        app.update();

        let registry = app.world().resource::<GhostZoneRegistry>();
        let total: usize = registry.zones.values().map(|v| v.len()).sum();
        assert!(
            total >= 1,
            "spirit_qi=-0.5 时应 spawn ≥ 1 个诡影，实际 {total} 个"
        );
        assert!(
            total <= GHOST_MAX_PER_ZONE,
            "诡影数不应超过上限 {GHOST_MAX_PER_ZONE}，实际 {total} 个"
        );
    }

    #[test]
    fn ghost_spawn_respects_max_per_zone() {
        // spirit_qi=-1.0 → target = floor(1.0 × 5.0) = 5，min(5, 10) = 5
        // 再次 update 不应超过上限
        let mut app = make_app_with_neg_zone(-1.0);
        app.add_systems(Update, ghost_spawn_system);
        // 运行多次
        for _ in 0..20 {
            app.update();
        }
        let registry = app.world().resource::<GhostZoneRegistry>();
        let total: usize = registry.zones.values().map(|v| v.len()).sum();
        assert!(
            total <= GHOST_MAX_PER_ZONE,
            "多次 spawn 后诡影数不应超过 {GHOST_MAX_PER_ZONE}，实际 {total} 个"
        );
    }

    #[test]
    fn ghost_spawn_no_spawn_below_min_pressure() {
        // spirit_qi=-0.1 → pressure=0.1 < 0.2 → 不生成
        let mut app = make_app_with_neg_zone(-0.1);
        app.add_systems(Update, ghost_spawn_system);
        app.update();

        let registry = app.world().resource::<GhostZoneRegistry>();
        let total: usize = registry.zones.values().map(|v| v.len()).sum();
        assert_eq!(
            total, 0,
            "压力 0.1 < GHOST_SPAWN_MIN_PRESSURE 0.2 时不应 spawn，实际 {total} 个"
        );
    }

    #[test]
    fn ghost_drift_system_updates_position_within_zone_bounds() {
        // ghost_drift_system 更新位置后，新位置必须在 zone AABB 内（AABB clamp）
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, (ghost_spawn_system, ghost_drift_system));
        app.update(); // spawn
        app.update(); // drift

        let zones = app.world().resource::<ZoneRegistry>();
        let zone = zones.find_zone_by_name("spawn").expect("spawn zone exists");
        let (min, max) = zone.bounds;

        let mut ghost_query = app.world_mut().query::<&GhostEntity>();
        for ghost in ghost_query.iter(app.world()) {
            assert!(
                ghost.position.x >= min.x && ghost.position.x <= max.x,
                "诡影 x={} 超出 zone bounds [{}, {}]",
                ghost.position.x,
                min.x,
                max.x
            );
            assert!(
                ghost.position.z >= min.z && ghost.position.z <= max.z,
                "诡影 z={} 超出 zone bounds [{}, {}]",
                ghost.position.z,
                min.z,
                max.z
            );
        }
    }

    #[test]
    fn ghost_contact_system_emits_qi_transfer_on_contact() {
        // 玩家在负灵域 + 诡影在接触半径内 → emit QiTransfer{reason: GhostContact}
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, ghost_contact_system);

        // 手动 spawn 一个诡影在玩家附近
        app.world_mut().spawn(GhostEntity {
            position: DVec3::new(8.0, 66.0, 8.1), // 距玩家 0.1 格，< SIPHON_RADIUS
            drift_velocity: DVec3::ZERO,
            zone_name: "spawn".to_string(),
            tick_counter: 0,
        });

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 50.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();

        app.update();

        // 必须 emit 至少 1 条 QiTransfer
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert!(
            !transfers.is_empty(),
            "诡影接触应 emit QiTransfer，实际未 emit"
        );
        assert!(
            transfers
                .iter()
                .any(|t| matches!(t.reason, QiTransferReason::GhostContact)),
            "emit 的 QiTransfer reason 应为 GhostContact，实际 {:?}",
            transfers.iter().map(|t| t.reason).collect::<Vec<_>>()
        );

        // 玩家 qi_current 应减少
        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!(
            cultivation.qi_current < 50.0,
            "诡影接触后玩家 qi_current 应减少，实际 {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn ghost_contact_system_respects_shared_cooldown() {
        // 同一 tick 内，cooldown 已设置时第二次接触不应再 emit
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, ghost_contact_system);

        // 两个诡影都在接触半径内
        for i in 0..2u32 {
            app.world_mut().spawn(GhostEntity {
                position: DVec3::new(8.0, 66.0, 8.0 + 0.1 * i as f64),
                drift_velocity: DVec3::ZERO,
                zone_name: "spawn".to_string(),
                tick_counter: 0,
            });
        }

        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            },
            LifeRecord::new(canonical_player_id("Azure")),
            // 预设 cooldown 为 tick 0（当前也是 0），cooldown 未过期
            GhostContactCooldown {
                last_contact_tick: 0,
            },
        ));

        app.update();

        // cooldown 生效时，不应 emit 任何 transfer
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert!(
            transfers.is_empty(),
            "cooldown 未过期时不应 emit QiTransfer，实际 {} 条",
            transfers.len()
        );
    }

    #[test]
    fn ghost_contact_system_no_effect_in_positive_zone() {
        // 正灵域（spirit_qi=0.5）不触发诡影接触效果
        let mut app = make_app_with_neg_zone(0.5);
        app.add_systems(Update, ghost_contact_system);

        app.world_mut().spawn(GhostEntity {
            position: DVec3::new(8.0, 66.0, 8.0),
            drift_velocity: DVec3::ZERO,
            zone_name: "spawn".to_string(),
            tick_counter: 0,
        });

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 50.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();

        app.update();

        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert!(
            transfers.is_empty(),
            "正灵域中不应 emit QiTransfer，实际 {} 条",
            transfers.len()
        );

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!(
            (cultivation.qi_current - 50.0).abs() < QI_EPSILON,
            "正灵域中玩家 qi_current 不应改变，实际 {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn ghost_contact_pulse_amount_scales_with_qi_max() {
        // qi_max=200 的玩家比 qi_max=100 的玩家被抽更多——高手在负灵域更危险
        // 验证：对同一 zone_qi，qi_max 翻倍 → pulse 翻倍
        let zone_qi = -0.5_f64;
        let pulse_low = (-zone_qi) * 100.0 * GHOST_CONTACT_FACTOR;
        let pulse_high = (-zone_qi) * 200.0 * GHOST_CONTACT_FACTOR;
        assert!(
            (pulse_high - 2.0 * pulse_low).abs() < QI_EPSILON,
            "qi_max 翻倍，pulse 应翻倍（高手更危险）；期望 {}, 实际 {pulse_high}",
            2.0 * pulse_low
        );
    }

    #[test]
    fn ghost_cooldown_constant_is_one_second() {
        // GHOST_COOLDOWN_TICKS 必须等于 20（1 秒 = 20 tick，MC 20TPS）
        assert_eq!(
            GHOST_COOLDOWN_TICKS, 20,
            "GHOST_COOLDOWN_TICKS 应为 20（1 秒），实际 {GHOST_COOLDOWN_TICKS}"
        );
    }

    #[test]
    fn ghost_contact_system_skips_entity_without_current_dimension() {
        // 无 CurrentDimension 的玩家不应触发接触（无法确认所在 zone）
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, ghost_contact_system);

        app.world_mut().spawn(GhostEntity {
            position: DVec3::new(8.0, 66.0, 8.0),
            drift_velocity: DVec3::ZERO,
            zone_name: "spawn".to_string(),
            tick_counter: 0,
        });

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                // 无 CurrentDimension
                Cultivation {
                    qi_current: 50.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!(
            (cultivation.qi_current - 50.0).abs() < QI_EPSILON,
            "无 CurrentDimension 时 qi_current 不应改变，实际 {}",
            cultivation.qi_current
        );
    }

    // ── P2 生态联动测试 ──

    /// P2-T1: spirit_qi=-0.2 vs spirit_qi=-1.0 诡影数量对比
    /// 更负的 spirit_qi 产生更多诡影（密度公式正比于 |spirit_qi|）。
    #[test]
    fn p2_ghost_count_scales_with_spirit_qi_magnitude() {
        // spirit_qi=-0.2 → target = floor(0.2 × 5.0) = 1
        // spirit_qi=-1.0 → target = floor(1.0 × 5.0) = 5
        // 两者 target 不同，验证生态联动：越负越多诡影
        let target_low_pressure = (0.2_f64 * GHOST_DENSITY_FACTOR)
            .floor()
            .min(GHOST_MAX_PER_ZONE as f64) as usize;
        let target_high_pressure = (1.0_f64 * GHOST_DENSITY_FACTOR)
            .floor()
            .min(GHOST_MAX_PER_ZONE as f64) as usize;

        assert_eq!(
            target_low_pressure, 1,
            "spirit_qi=-0.2 时 target_count 应为 1（floor(0.2×5.0)=1），实际 {target_low_pressure}（P2 生态联动密度公式）"
        );
        assert_eq!(
            target_high_pressure, 5,
            "spirit_qi=-1.0 时 target_count 应为 5（floor(1.0×5.0)=5），实际 {target_high_pressure}（P2 生态联动密度公式）"
        );
        assert!(
            target_high_pressure > target_low_pressure,
            "spirit_qi=-1.0 时诡影数（{target_high_pressure}）应多于 spirit_qi=-0.2 时（{target_low_pressure}）（越负越危险）"
        );
    }

    /// P2-T2: spirit_qi=-0.2（刚好满足 MIN_PRESSURE）时 spawn 1 个诡影
    /// 边界：压力恰好等于阈值时，density formula 产生 target=1，必须 spawn。
    #[test]
    fn p2_ghost_spawn_at_exact_min_pressure_boundary() {
        // GHOST_SPAWN_MIN_PRESSURE = -0.2，pressure=0.2 → 恰好满足条件
        // target = floor(0.2 × 5.0) = floor(1.0) = 1 → spawn 1 个
        let pressure = GHOST_SPAWN_MIN_PRESSURE.abs(); // 0.2
        let target = (pressure * GHOST_DENSITY_FACTOR)
            .floor()
            .min(GHOST_MAX_PER_ZONE as f64) as usize;
        assert_eq!(
            target, 1,
            "pressure=0.2（刚好满足 MIN_PRESSURE）时 target_count 应为 1（floor(0.2×5)=1），实际 {target}（P2 边界检查）"
        );

        let mut app = make_app_with_neg_zone(GHOST_SPAWN_MIN_PRESSURE);
        app.add_systems(Update, ghost_spawn_system);
        app.update();

        let registry = app.world().resource::<GhostZoneRegistry>();
        let total: usize = registry.zones.values().map(|v| v.len()).sum();
        assert_eq!(
            total, 1,
            "spirit_qi=-0.2（MIN_PRESSURE 边界）时应 spawn 1 个诡影（期望 1，实际 {total}）（P2 边界检查）"
        );
    }

    /// P2-T3: 上限 10 的强制检查——无论 spirit_qi 多负都不超过 GHOST_MAX_PER_ZONE
    /// 即使 spirit_qi 极端负（-100），density formula 通过 .min(10) 保证上限。
    #[test]
    fn p2_ghost_max_per_zone_hard_cap_even_at_extreme_pressure() {
        // 极端 spirit_qi=-100 → pressure=100 → target=min(500, 10)=10
        let extreme_pressure = 100.0_f64;
        let target = (extreme_pressure * GHOST_DENSITY_FACTOR)
            .floor()
            .min(GHOST_MAX_PER_ZONE as f64) as usize;
        assert_eq!(
            target, GHOST_MAX_PER_ZONE,
            "极端压力下 target_count 应被 .min({GHOST_MAX_PER_ZONE}) 限制（期望 {GHOST_MAX_PER_ZONE}，实际 {target}）（P2 上限强制）"
        );
    }

    /// P2-T4: density formula 在 spirit_qi=-0.4 时产生 target=2（中间值）
    /// 验证 GHOST_DENSITY_FACTOR=5.0 配置下的典型中间值。
    #[test]
    fn p2_ghost_density_formula_mid_pressure_value() {
        // spirit_qi=-0.4 → pressure=0.4 → target=floor(0.4×5.0)=floor(2.0)=2
        let pressure = 0.4_f64;
        let target = (pressure * GHOST_DENSITY_FACTOR)
            .floor()
            .min(GHOST_MAX_PER_ZONE as f64) as usize;
        assert_eq!(
            target, 2,
            "spirit_qi=-0.4 时 target_count 应为 2（floor(0.4×5.0)=2），实际 {target}（P2 中间值密度公式）"
        );
    }
}
