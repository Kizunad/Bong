use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, Event, EventReader, EventWriter, Position, Query,
    Res, ResMut,
};

use crate::combat::arm_wound;
use crate::combat::components::{Lifecycle, LifecycleState, Stamina, WoundKind, Wounds};
use crate::combat::events::{AttackIntent, AttackReach, AttackSource};
use crate::combat::projectile::residual_qi_after_miss;
use crate::combat::raycast;
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, QiColor, Realm};
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::qi_physics::release::qi_release_to_zone;
use crate::world::dimension::DimensionKind;
use crate::world::zone::ZoneRegistry;

type NeedleActorQueryItem<'a> = (
    &'a mut Cultivation,
    &'a mut Stamina,
    Option<&'a Lifecycle>,
    Option<&'a Position>,
    Option<&'a QiColor>,
    // plan-combat-hit-location-v1 P1（决议 §8.1 #2）— 发射者主手臂伤势缩放气针散布角。
    Option<&'a Wounds>,
);

/// plan-combat-hit-location-v1 P1（决议 §8.1 #2）— 气针基线散布角标准差（度）。
/// 气针发射方向此前无任何随机抖动（`dir_unit` 直传即用，零误差），臂伤惩罚"散布角+50%"
/// 若无基线可乘则永远是 0×1.5=0——引入一个保守的小基线代表持针手的自然轻微颤抖，
/// 健康臂时命中精度肉眼几乎无感知差异，臂伤（Fracture/Severed）时按
/// `arm_wound::spread_multiplier` 放大该标准差。
pub const QI_NEEDLE_BASE_SPREAD_SIGMA_DEG: f64 = 3.0;

pub const QI_NEEDLE_SKILL_ID: &str = "dugu.shoot_needle";
pub const QI_NEEDLE_QI_COST: f64 = 1.0;
pub const QI_NEEDLE_STAMINA_COST: f32 = 2.0;
pub const QI_NEEDLE_SPEED_BLOCKS_PER_SEC: f32 = 90.0;
pub const QI_NEEDLE_MAX_DISTANCE_BLOCKS: f32 = 50.0;
pub const QI_NEEDLE_HITBOX_INFLATION: f32 = 0.6;
pub const QI_NEEDLE_COOLDOWN_TICKS: u64 = 12;

pub const QI_NEEDLE_REACH: AttackReach = AttackReach {
    base: QI_NEEDLE_MAX_DISTANCE_BLOCKS,
    step_bonus: 0.0,
    max: QI_NEEDLE_MAX_DISTANCE_BLOCKS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentSource {
    Client,
    SkillBar,
    Test,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct ShootNeedleIntent {
    pub shooter: Entity,
    pub target: Option<Entity>,
    pub dir_unit: [f64; 3],
    pub source: IntentSource,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct QiNeedle {
    pub shooter: Entity,
    pub qi_payload: f64,
    pub qi_color: String,
    pub infused_dugu: bool,
    pub spawn_pos: [f64; 3],
    pub velocity: [f64; 3],
    pub max_distance: f32,
    pub hitbox_inflation: f32,
    pub spawned_at_tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct QiNeedleChargedEvent {
    pub shooter: Entity,
    pub target: Option<Entity>,
    pub tick: u64,
}

pub fn resolve_shoot_needle_intents(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut intents: EventReader<ShootNeedleIntent>,
    mut actors: Query<NeedleActorQueryItem<'_>>,
    mut attacks: EventWriter<AttackIntent>,
    mut charged_events: EventWriter<QiNeedleChargedEvent>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    for intent in intents.read() {
        let Ok((mut cultivation, mut stamina, lifecycle, position, qi_color, wounds)) =
            actors.get_mut(intent.shooter)
        else {
            continue;
        };
        if !can_shoot_needle(&cultivation, &stamina, lifecycle) {
            continue;
        }

        cultivation.qi_current =
            (cultivation.qi_current - QI_NEEDLE_QI_COST).clamp(0.0, cultivation.qi_max);
        stamina.current = (stamina.current - QI_NEEDLE_STAMINA_COST).clamp(0.0, stamina.max);

        // plan-combat-hit-location-v1 P1（决议 §8.1 #2）— 主手臂伤势放大发射方向散布抖动。
        let spread_multiplier = arm_wound::combined_factor_from_optional(wounds).spread_multiplier;
        let jitter_seed = intent.shooter.to_bits() ^ clock.tick;
        let dir = jitter_direction(
            normalized_dir(intent.dir_unit),
            jitter_seed,
            QI_NEEDLE_BASE_SPREAD_SIGMA_DEG * f64::from(spread_multiplier),
        );
        let spawn_pos = position
            .map(|position| position.get() + dir * 0.3)
            .unwrap_or(DVec3::ZERO);
        let needle_entity = commands
            .spawn(QiNeedle {
                shooter: intent.shooter,
                qi_payload: QI_NEEDLE_QI_COST,
                qi_color: qi_color_label(qi_color),
                infused_dugu: false,
                spawn_pos: [spawn_pos.x, spawn_pos.y, spawn_pos.z],
                velocity: [
                    dir.x * f64::from(QI_NEEDLE_SPEED_BLOCKS_PER_SEC),
                    dir.y * f64::from(QI_NEEDLE_SPEED_BLOCKS_PER_SEC),
                    dir.z * f64::from(QI_NEEDLE_SPEED_BLOCKS_PER_SEC),
                ],
                max_distance: QI_NEEDLE_MAX_DISTANCE_BLOCKS,
                hitbox_inflation: QI_NEEDLE_HITBOX_INFLATION,
                spawned_at_tick: clock.tick,
            })
            .id();
        emit_needle_channeling_transfer(&mut qi_transfers, intent.shooter, needle_entity);
        if let Some(target) = intent.target {
            attacks.send(AttackIntent {
                attacker: intent.shooter,
                target: Some(target),
                issued_at_tick: clock.tick,
                reach: QI_NEEDLE_REACH,
                qi_invest: QI_NEEDLE_QI_COST as f32,
                wound_kind: WoundKind::Pierce,
                source: AttackSource::QiNeedle,
                debug_command: None,
            });
        }
        charged_events.send(QiNeedleChargedEvent {
            shooter: intent.shooter,
            target: intent.target,
            tick: clock.tick,
        });
    }
}

/// qc-P0：过期针归还真元路径。维度固定为 Overworld（针不跨维度飞行）。
pub fn despawn_expired_qi_needles(
    mut commands: Commands,
    clock: Res<CombatClock>,
    needles: Query<(Entity, &QiNeedle, Option<&Position>)>,
    mut zones: ResMut<ZoneRegistry>,
    mut qi_transfers: EventWriter<QiTransfer>,
) {
    let max_age_ticks =
        ((QI_NEEDLE_MAX_DISTANCE_BLOCKS / QI_NEEDLE_SPEED_BLOCKS_PER_SEC) * 20.0).ceil() as u64 + 1;

    for (entity, needle, position) in &needles {
        if clock.tick.saturating_sub(needle.spawned_at_tick) > max_age_ticks {
            // qc-P0：针过期时把针容器里的完整 qi_payload 归还落点 zone（守恒）。
            // residual_qi_after_miss 只描述投射物威力衰减；账本上发射阶段已经把
            // 完整 payload 转入针容器，despawn 前必须全量转出。
            let qi_payload = needle.qi_payload as f32;
            let (evaporated, residual) = residual_qi_after_miss(qi_payload);
            let release_f64 = f64::from(evaporated + residual);
            if release_f64 > f64::EPSILON {
                // 落点：优先用 Position 组件（若有），否则按 velocity 外推真实消亡点。
                // 针无 Position 组件时 spawn_pos 仅出发点；针以 velocity 飞行、最远
                // max_distance，跨 zone 飞行时残真元须归还实际落点 zone（而非出发 zone）。
                let pos = position.map(|p| p.get()).unwrap_or_else(|| {
                    let elapsed_secs =
                        clock.tick.saturating_sub(needle.spawned_at_tick) as f64 / 20.0;
                    let traveled = (DVec3::from(needle.velocity) * elapsed_secs)
                        .clamp_length_max(f64::from(needle.max_distance));
                    DVec3::from(needle.spawn_pos) + traveled
                });
                release_needle_qi_to_zone(&mut zones, &mut qi_transfers, entity, pos, release_f64);
            }
            commands.entity(entity).despawn();
        }
    }
}

pub fn can_shoot_needle(
    cultivation: &Cultivation,
    stamina: &Stamina,
    lifecycle: Option<&Lifecycle>,
) -> bool {
    realm_rank(cultivation.realm) >= realm_rank(Realm::Induce)
        && cultivation.qi_current + f64::EPSILON >= QI_NEEDLE_QI_COST
        && stamina.current + f32::EPSILON >= QI_NEEDLE_STAMINA_COST
        && lifecycle.is_none_or(|lifecycle| lifecycle.state == LifecycleState::Alive)
}

pub fn normalized_dir(dir: [f64; 3]) -> DVec3 {
    let raw = DVec3::new(dir[0], dir[1], dir[2]);
    if raw.length_squared() <= f64::EPSILON {
        DVec3::new(0.0, 0.0, 1.0)
    } else {
        raw.normalize()
    }
}

/// plan-combat-hit-location-v1 P1（决议 §8.1 #2）— 对已归一化方向 `dir` 施加确定性
/// 二维高斯角度抖动（复用 `combat::raycast::gaussian_pair` 同一套 splitmix64 PRNG）。
/// `sigma_deg <= 0.0` 时直接返回原方向（不做任何计算，健康臂在 spread_multiplier=1.0
/// 且 base=3.0 时仍会有基线抖动——只有显式传 0 才完全跳过，供测试锁定"零抖动"分支）。
///
/// 实现：在垂直于 `dir` 的平面内取两条正交基向量，用小角度正切位移代替真正的球面旋转
/// （sigma 通常个位数度，`tan(θ)≈θ` 近似误差可忽略），再重新归一化。
pub fn jitter_direction(dir: DVec3, seed: u64, sigma_deg: f64) -> DVec3 {
    if sigma_deg <= 0.0 {
        return dir;
    }
    let up_hint = if dir.y.abs() < 0.99 {
        DVec3::new(0.0, 1.0, 0.0)
    } else {
        DVec3::new(1.0, 0.0, 0.0)
    };
    let right = dir.cross(up_hint).normalize();
    let up = right.cross(dir).normalize();

    let (g1, g2) = raycast::gaussian_pair(seed);
    let angle1 = (g1 * sigma_deg).to_radians();
    let angle2 = (g2 * sigma_deg).to_radians();
    let jittered = dir + right * angle1.tan() + up * angle2.tan();

    if jittered.length_squared() <= f64::EPSILON {
        dir
    } else {
        jittered.normalize()
    }
}

fn qi_color_label(qi_color: Option<&QiColor>) -> String {
    qi_color
        .map(|qi_color| format!("{:?}", qi_color.main))
        .unwrap_or_else(|| "Mellow".to_string())
}

fn player_qi_account(player: Entity) -> QiAccountId {
    QiAccountId::player(format!("entity:{}", player.to_bits()))
}

fn needle_qi_account(needle: Entity) -> QiAccountId {
    QiAccountId::container(format!("qi_needle:entity:{}", needle.to_bits()))
}

fn emit_needle_channeling_transfer(
    qi_transfers: &mut EventWriter<QiTransfer>,
    shooter: Entity,
    needle: Entity,
) {
    if let Ok(transfer) = QiTransfer::new(
        player_qi_account(shooter),
        needle_qi_account(needle),
        QI_NEEDLE_QI_COST,
        QiTransferReason::Channeling,
    ) {
        qi_transfers.send(transfer);
    }
}

fn release_needle_qi_to_zone(
    zones: &mut ZoneRegistry,
    qi_transfers: &mut EventWriter<QiTransfer>,
    needle: Entity,
    pos: DVec3,
    residual: f64,
) {
    if residual <= f64::EPSILON {
        return;
    }

    let from = needle_qi_account(needle);
    let entity_bits = needle.to_bits();
    let Some(zone_name) = zones
        .find_zone(DimensionKind::Overworld, pos)
        .map(|zone| zone.name.clone())
    else {
        emit_needle_overflow_transfer(
            qi_transfers,
            from,
            format!("qi_needle_expire_no_zone:entity:{entity_bits}"),
            residual,
        );
        return;
    };

    let Some(zone) = zones.find_zone_mut(&zone_name) else {
        emit_needle_overflow_transfer(
            qi_transfers,
            from,
            format!("qi_needle_expire_no_mut_zone:entity:{entity_bits}"),
            residual,
        );
        return;
    };

    let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    match qi_release_to_zone(
        residual,
        from.clone(),
        QiAccountId::zone(zone_name),
        zone_current,
        QI_ZONE_UNIT_CAPACITY,
    ) {
        Ok(outcome) => {
            zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
            if let Some(transfer) = outcome.transfer {
                qi_transfers.send(transfer);
            }
            if outcome.overflow > f64::EPSILON {
                emit_needle_overflow_transfer(
                    qi_transfers,
                    from,
                    format!("qi_needle_expire_overflow:entity:{entity_bits}"),
                    outcome.overflow,
                );
            }
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                entity_bits,
                residual,
                "[bong][qi_needle] qi release error; routing to overflow"
            );
            emit_needle_overflow_transfer(
                qi_transfers,
                from,
                format!("qi_needle_expire_err_overflow:entity:{entity_bits}"),
                residual,
            );
        }
    }
}

fn emit_needle_overflow_transfer(
    qi_transfers: &mut EventWriter<QiTransfer>,
    from: QiAccountId,
    overflow_id: String,
    amount: f64,
) {
    if let Ok(transfer) = QiTransfer::new(
        from,
        QiAccountId::overflow(overflow_id),
        amount,
        QiTransferReason::ReleaseToZone,
    ) {
        qi_transfers.send(transfer);
    }
}

pub fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Events, Update};

    fn actor(realm: Realm, qi_current: f64, stamina_current: f32) -> (Cultivation, Stamina) {
        (
            Cultivation {
                realm,
                qi_current,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            Stamina {
                current: stamina_current,
                ..Stamina::default()
            },
        )
    }

    #[test]
    fn shoot_needle_rejects_awaken_or_empty_pools() {
        let (cultivation, stamina) = actor(Realm::Awaken, 10.0, 10.0);
        assert!(!can_shoot_needle(&cultivation, &stamina, None));

        let (cultivation, stamina) = actor(Realm::Induce, 0.5, 10.0);
        assert!(!can_shoot_needle(&cultivation, &stamina, None));

        let (cultivation, stamina) = actor(Realm::Induce, 10.0, 1.0);
        assert!(!can_shoot_needle(&cultivation, &stamina, None));
    }

    #[test]
    fn shoot_needle_consumes_qi_and_emits_attack_for_target() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 9 });
        app.add_event::<ShootNeedleIntent>();
        app.add_event::<AttackIntent>();
        app.add_event::<QiNeedleChargedEvent>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, resolve_shoot_needle_intents);

        let (cultivation, stamina) = actor(Realm::Induce, 10.0, 10.0);
        let shooter = app
            .world_mut()
            .spawn((cultivation, stamina, Lifecycle::default()))
            .id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ShootNeedleIntent {
            shooter,
            target: Some(target),
            dir_unit: [0.0, 0.0, 2.0],
            source: IntentSource::Test,
        });

        app.update();

        let cultivation = app.world().get::<Cultivation>(shooter).unwrap();
        let stamina = app.world().get::<Stamina>(shooter).unwrap();
        assert_eq!(cultivation.qi_current, 9.0);
        assert_eq!(stamina.current, 8.0);
        let attack_events = app.world().resource::<Events<AttackIntent>>();
        assert!(attack_events
            .get_reader()
            .read(attack_events)
            .any(|event| event.source == AttackSource::QiNeedle && event.target == Some(target)));
        let needle_entities = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &QiNeedle)>();
            query
                .iter(world)
                .map(|(entity, _needle)| entity)
                .collect::<Vec<_>>()
        };
        assert_eq!(needle_entities.len(), 1);

        let qi_transfers = app.world().resource::<Events<QiTransfer>>();
        let transfer_events = qi_transfers
            .get_reader()
            .read(qi_transfers)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            transfer_events.len(),
            1,
            "气针发射扣 qi_current 时必须 emit 1 条 QiTransfer，实际 {} 条",
            transfer_events.len()
        );
        assert_eq!(transfer_events[0].from, player_qi_account(shooter));
        assert_eq!(transfer_events[0].to, needle_qi_account(needle_entities[0]));
        assert_eq!(transfer_events[0].amount, QI_NEEDLE_QI_COST);
        assert_eq!(transfer_events[0].reason, QiTransferReason::Channeling);
    }

    // ── qc-P0 守恒测试：despawn_expired_qi_needles ───────────────────────────────

    /// 辅助：构建带 ZoneRegistry + QiTransfer 事件的 App 并注册 expire-despawn 系统。
    fn expire_app() -> App {
        use crate::qi_physics::ledger::QiTransfer;
        use crate::world::zone::ZoneRegistry;

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 0 });
        app.add_event::<QiTransfer>();
        app.insert_resource(ZoneRegistry::default()); // spawn zone 在 [-128..128]
        app.add_systems(Update, despawn_expired_qi_needles);
        app
    }

    fn spawn_needle(app: &mut App, spawned_at_tick: u64, qi_payload: f64) -> Entity {
        // spawn_pos [0, 66, 0] 在 default spawn zone 内。
        // 先 spawn shooter，再 spawn needle，避免 borrow 冲突。
        let shooter = app.world_mut().spawn_empty().id();
        app.world_mut()
            .spawn(QiNeedle {
                shooter,
                qi_payload,
                qi_color: "Mellow".to_string(),
                infused_dugu: false,
                spawn_pos: [0.0, 66.0, 0.0],
                velocity: [0.0, 0.0, f64::from(QI_NEEDLE_SPEED_BLOCKS_PER_SEC)],
                max_distance: QI_NEEDLE_MAX_DISTANCE_BLOCKS,
                hitbox_inflation: QI_NEEDLE_HITBOX_INFLATION,
                spawned_at_tick,
            })
            .id()
    }

    #[test]
    fn expired_needle_releases_residual_to_zone() {
        // 期望：过期针被 despawn，残真元归还 spawn zone（spirit_qi 上升）。
        use crate::world::zone::ZoneRegistry;

        let mut app = expire_app();

        let max_age_ticks =
            ((QI_NEEDLE_MAX_DISTANCE_BLOCKS / QI_NEEDLE_SPEED_BLOCKS_PER_SEC) * 20.0).ceil() as u64
                + 1;
        // 子弹在 tick=0 产生，advance clock 超过 max_age_ticks
        let _ = spawn_needle(&mut app, 0, QI_NEEDLE_QI_COST);

        // 把 clock 推到过期
        app.world_mut().resource_mut::<CombatClock>().tick = max_age_ticks + 2;

        let zone_before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        app.update();

        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        assert!(
            zone_after > zone_before,
            "期望针过期后 spawn zone.spirit_qi 上升（残真元归还），\
             实际 before={zone_before:.6} after={zone_after:.6}"
        );

        // 针实体应已被 despawn
        let needle_count = {
            let world = app.world_mut();
            let mut q = world.query::<&QiNeedle>();
            q.iter(world).count()
        };
        assert_eq!(
            needle_count, 0,
            "期望过期针被 despawn，实际还剩 {needle_count} 个"
        );
    }

    #[test]
    fn expired_needle_releases_to_despawn_point_not_spawn_zone() {
        // 期望：针从 spawn zone 边缘(z=120)朝外飞，过期消亡点 z≈170 在 spawn zone
        // [-128..128] 之外 → 残真元**不**归 spawn zone（证明用 velocity 外推的消亡点
        // 定 zone，而非出发点 spawn_pos）。锁死 must_fix：跨 zone 飞行归还正确 zone。
        use crate::world::zone::ZoneRegistry;

        let mut app = expire_app();
        let max_age_ticks =
            ((QI_NEEDLE_MAX_DISTANCE_BLOCKS / QI_NEEDLE_SPEED_BLOCKS_PER_SEC) * 20.0).ceil() as u64
                + 1;
        let shooter = app.world_mut().spawn_empty().id();
        app.world_mut().spawn(QiNeedle {
            shooter,
            qi_payload: QI_NEEDLE_QI_COST,
            qi_color: "Mellow".to_string(),
            infused_dugu: false,
            // spawn_pos z=120 在 spawn zone 内；velocity +z 90 blocks/s 飞满 50 格
            // (max_distance) 后消亡点 z=170，已出 spawn zone。
            spawn_pos: [0.0, 66.0, 120.0],
            velocity: [0.0, 0.0, f64::from(QI_NEEDLE_SPEED_BLOCKS_PER_SEC)],
            max_distance: QI_NEEDLE_MAX_DISTANCE_BLOCKS,
            hitbox_inflation: QI_NEEDLE_HITBOX_INFLATION,
            spawned_at_tick: 0,
        });
        app.world_mut().resource_mut::<CombatClock>().tick = max_age_ticks + 2;

        let spawn_before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;
        app.update();
        let spawn_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;

        assert!(
            (spawn_after - spawn_before).abs() < 1e-9,
            "期望残真元归还消亡点(z≈170，spawn zone 外)而非出发 zone，\
             故 spawn zone.spirit_qi 应不变；实际 before={spawn_before:.6} after={spawn_after:.6}\
             （若变化说明仍用 spawn_pos 定 zone，must_fix 未修）"
        );
    }

    #[test]
    fn young_needle_is_not_despawned() {
        // 期望：未过期的针不被 despawn，也不 emit QiTransfer。
        use crate::qi_physics::ledger::QiTransfer;

        let mut app = expire_app();
        // 针在 tick=0 产生，clock 停在 tick=5（远小于 max_age_ticks）
        let _ = spawn_needle(&mut app, 0, QI_NEEDLE_QI_COST);
        app.world_mut().resource_mut::<CombatClock>().tick = 5;
        app.update();

        let needle_count = {
            let world = app.world_mut();
            let mut q = world.query::<&QiNeedle>();
            q.iter(world).count()
        };
        assert_eq!(needle_count, 1, "未过期的针不应被 despawn");

        let transfers = app.world().resource::<Events<QiTransfer>>();
        assert!(
            transfers.is_empty(),
            "未过期针不应 emit QiTransfer，实际 emit 了 {} 条",
            transfers.len()
        );
    }

    #[test]
    fn needle_expire_conservation_invariant() {
        // 期望：过期针 Σ QiTransfer.amount == qi_payload（容器全量释放）。
        use crate::qi_physics::ledger::QiTransfer;

        let mut app = expire_app();

        let max_age_ticks =
            ((QI_NEEDLE_MAX_DISTANCE_BLOCKS / QI_NEEDLE_SPEED_BLOCKS_PER_SEC) * 20.0).ceil() as u64
                + 1;
        let needle = spawn_needle(&mut app, 0, QI_NEEDLE_QI_COST);
        app.world_mut().resource_mut::<CombatClock>().tick = max_age_ticks + 2;
        app.update();

        let events = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers = reader.read(events).cloned().collect::<Vec<_>>();
        let total: f64 = transfers.iter().map(|transfer| transfer.amount).sum();

        let expected_release = QI_NEEDLE_QI_COST;
        assert_eq!(
            transfers.len(),
            1,
            "气针过期 payload 应 emit 1 条 ReleaseToZone，实际 {} 条",
            transfers.len()
        );
        assert_eq!(transfers[0].from, needle_qi_account(needle));
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
        assert!(
            (total - expected_release).abs() < 1e-5,
            "守恒不变式：QiTransfer 总量应等于 payload={}，实际 {total}",
            QI_NEEDLE_QI_COST
        );
    }

    #[test]
    fn needle_expire_zero_qi_payload_is_noop() {
        // 期望：qi_payload=0 的针过期时不 emit QiTransfer（residual=0 → noop）。
        use crate::qi_physics::ledger::QiTransfer;

        let mut app = expire_app();

        let max_age_ticks =
            ((QI_NEEDLE_MAX_DISTANCE_BLOCKS / QI_NEEDLE_SPEED_BLOCKS_PER_SEC) * 20.0).ceil() as u64
                + 1;
        let _ = spawn_needle(&mut app, 0, 0.0); // qi_payload = 0
        app.world_mut().resource_mut::<CombatClock>().tick = max_age_ticks + 2;
        app.update();

        let transfers = app.world().resource::<Events<QiTransfer>>();
        assert!(
            transfers.is_empty(),
            "qi_payload=0 针过期不应 emit QiTransfer（期望 noop），实际 emit {}",
            transfers.len()
        );
    }

    // ── plan-combat-hit-location-v1 P1（决议 §8.1 #2）— jitter_direction 散布抖动 ──

    #[test]
    fn jitter_direction_zero_sigma_returns_original_direction_unchanged() {
        let dir = DVec3::new(0.0, 0.0, 1.0);
        assert_eq!(jitter_direction(dir, 12345, 0.0), dir);
    }

    #[test]
    fn jitter_direction_is_deterministic_for_same_seed() {
        let dir = DVec3::new(0.0, 0.0, 1.0);
        let a = jitter_direction(dir, 777, 5.0);
        let b = jitter_direction(dir, 777, 5.0);
        assert_eq!(a, b, "同一 seed 必须复现完全相同的抖动方向（确定性要求）");
    }

    #[test]
    fn jitter_direction_varies_across_distinct_seeds() {
        let dir = DVec3::new(0.0, 0.0, 1.0);
        let a = jitter_direction(dir, 1, 5.0);
        let b = jitter_direction(dir, 2, 5.0);
        assert_ne!(a, b, "不同 seed 应产生不同抖动方向，否则抖动形同虚设");
    }

    #[test]
    fn jitter_direction_output_stays_normalized() {
        let dir = DVec3::new(0.3, 0.1, 0.95).normalize();
        let jittered = jitter_direction(dir, 999, 8.0);
        assert!(
            (jittered.length() - 1.0).abs() < 1e-9,
            "抖动后方向必须仍是单位向量，实际长度 {}",
            jittered.length()
        );
    }

    #[test]
    fn jitter_direction_larger_sigma_widens_average_deviation() {
        // 散布直方图 pin：更大 sigma 应产生更大的平均偏移角（非单次抽样，用批量均值降噪）。
        let dir = DVec3::new(0.0, 0.0, 1.0);
        let sample_deviation = |sigma: f64| -> f64 {
            let mut total = 0.0;
            for tick in 0..500u64 {
                let jittered = jitter_direction(dir, tick, sigma);
                total += dir.dot(jittered).clamp(-1.0, 1.0).acos();
            }
            total / 500.0
        };
        let small = sample_deviation(3.0);
        let large = sample_deviation(4.5); // 3.0 * spread_multiplier(Fracture)=1.5
        assert!(
            large > small,
            "sigma=4.5(臂伤后)的平均偏移角({large:.5}rad) 应大于 sigma=3.0(健康臂)的平均偏移角({small:.5}rad)"
        );
    }

    // ── resolve_shoot_needle_intents 消费臂伤散布惩罚（端到端）────────────────────

    fn spawn_shooter_with_wounds(app: &mut App, realm: Realm, wounds: Option<Wounds>) -> Entity {
        let (cultivation, stamina) = actor(realm, 10.0, 10.0);
        let mut entity = app
            .world_mut()
            .spawn((cultivation, stamina, Lifecycle::default()));
        if let Some(wounds) = wounds {
            entity.insert(wounds);
        }
        entity.id()
    }

    fn severed_main_arm_wounds() -> Wounds {
        use crate::combat::arm_wound::MAIN_ARM;
        use crate::combat::components::{Wound, WoundKind};
        Wounds {
            entries: vec![Wound {
                location: MAIN_ARM,
                kind: WoundKind::Cut,
                severity: 80.0, // Severed grade → spread_multiplier 1.5
                bleeding_per_sec: 0.0,
                created_at_tick: 0,
                inflicted_by: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn resolve_shoot_needle_intents_widens_spread_when_main_arm_severed() {
        // 端到端：主手臂断裂（Severed）的射手，气针发射方向的抖动幅度应系统性大于
        // 健康臂射手——用同一批固定 tick 序列对比两者的平均偏移角。
        let dir_unit = [0.0, 0.0, 1.0];

        let healthy_deviation = {
            let mut app = App::new();
            app.insert_resource(CombatClock { tick: 0 });
            app.add_event::<ShootNeedleIntent>();
            app.add_event::<AttackIntent>();
            app.add_event::<QiNeedleChargedEvent>();
            app.add_event::<QiTransfer>();
            app.add_systems(Update, resolve_shoot_needle_intents);
            let shooter = spawn_shooter_with_wounds(&mut app, Realm::Induce, None);
            let mut total = 0.0;
            for tick in 0..200u64 {
                app.world_mut().resource_mut::<CombatClock>().tick = tick;
                app.world_mut()
                    .get_mut::<Cultivation>(shooter)
                    .unwrap()
                    .qi_current = 10.0;
                app.world_mut().get_mut::<Stamina>(shooter).unwrap().current = 10.0;
                app.world_mut().send_event(ShootNeedleIntent {
                    shooter,
                    target: None,
                    dir_unit,
                    source: IntentSource::Test,
                });
                app.update();
                let needle = {
                    let world = app.world_mut();
                    let mut query = world.query::<&QiNeedle>();
                    query.iter(world).last().cloned()
                };
                if let Some(needle) = needle {
                    let v = DVec3::from(needle.velocity).normalize();
                    total += DVec3::from(dir_unit)
                        .normalize()
                        .dot(v)
                        .clamp(-1.0, 1.0)
                        .acos();
                }
            }
            total / 200.0
        };

        let severed_deviation = {
            let mut app = App::new();
            app.insert_resource(CombatClock { tick: 0 });
            app.add_event::<ShootNeedleIntent>();
            app.add_event::<AttackIntent>();
            app.add_event::<QiNeedleChargedEvent>();
            app.add_event::<QiTransfer>();
            app.add_systems(Update, resolve_shoot_needle_intents);
            let shooter =
                spawn_shooter_with_wounds(&mut app, Realm::Induce, Some(severed_main_arm_wounds()));
            let mut total = 0.0;
            for tick in 0..200u64 {
                app.world_mut().resource_mut::<CombatClock>().tick = tick;
                app.world_mut()
                    .get_mut::<Cultivation>(shooter)
                    .unwrap()
                    .qi_current = 10.0;
                app.world_mut().get_mut::<Stamina>(shooter).unwrap().current = 10.0;
                app.world_mut().send_event(ShootNeedleIntent {
                    shooter,
                    target: None,
                    dir_unit,
                    source: IntentSource::Test,
                });
                app.update();
                let needle = {
                    let world = app.world_mut();
                    let mut query = world.query::<&QiNeedle>();
                    query.iter(world).last().cloned()
                };
                if let Some(needle) = needle {
                    let v = DVec3::from(needle.velocity).normalize();
                    total += DVec3::from(dir_unit)
                        .normalize()
                        .dot(v)
                        .clamp(-1.0, 1.0)
                        .acos();
                }
            }
            total / 200.0
        };

        assert!(
            severed_deviation > healthy_deviation,
            "主手臂断裂射手的平均发射偏移角({severed_deviation:.5}rad) 应大于健康臂射手\
             ({healthy_deviation:.5}rad)——散布惩罚未生效"
        );
    }
}
