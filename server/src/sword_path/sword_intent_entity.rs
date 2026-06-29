//! 剑意化形追踪实体（plan-sword-path-complete §C）。
//!
//! 纯逻辑实体（无 Layer / MarkerEntityBundle），可裸 despawn。
//! 视觉靠 `bong:sword_intent_hit` 粒子在命中时触发。

use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, EventWriter, Position, Query, Res, Without, World,
};

use crate::combat::components::WoundKind;
use crate::combat::events::{AttackIntent, AttackReach, AttackSource};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::movement::GameTick;
use crate::schema::vfx_event::VfxEventPayloadV1;

// ── 常数 ─────────────────────────────────────────────────────────────────────

pub const SWORD_INTENT_LIFETIME_TICKS: u32 = 100; // 5s @20tps
pub const SWORD_INTENT_HIT_COUNT: u8 = 5;
pub const SWORD_INTENT_SPEED_PER_TICK: f64 = 0.5; // 10 格/s
pub const SWORD_INTENT_HIT_RADIUS: f64 = 3.0;
pub const SWORD_INTENT_HIT_COOLDOWN: u8 = 6;

/// 剑意命中粒子 event_id（client SwordPathVfxPlayer 必须注册此 id）。
const VFX_SWORD_INTENT_HIT: &str = "bong:sword_intent_hit";

// ── 组件 ─────────────────────────────────────────────────────────────────────

/// 剑意化形追踪实体组件。
///
/// 实体生命周期：spawn → 追踪目标 → 命中 5 次或 lifetime 耗尽 → despawn。
/// 纯逻辑（无 Layer）→ 可裸 `.despawn()`。
#[derive(Debug, Clone, Component)]
pub struct SwordIntentEntity {
    /// 施法者（玩家或 boss）。
    pub owner: Entity,
    /// 追踪目标（None 时只倒计时不命中）。
    pub target: Option<Entity>,
    /// 剩余存活 tick（初始 100）。
    pub lifetime_ticks: u32,
    /// 剩余命中次数（初始 5）。
    pub hits_remaining: u8,
    /// 每次命中伤害（qi_invest 语义）。
    pub damage_per_hit: f64,
    /// 攻击来源（玩家化形 / boss 复用 SwordPathManifest）。
    pub source: AttackSource,
    /// 命中后短冷却（初始 0，命中后置 SWORD_INTENT_HIT_COOLDOWN，防同 tick 多命中）。
    pub hit_cooldown: u8,
}

// ── Spawn 函数 ────────────────────────────────────────────────────────────────

/// 生成剑意化形追踪实体，返回新实体 id。
///
/// 纯逻辑实体（无 Layer），调用方无需提供 layer entity。
/// 走 `Commands` 路径（System 参数友好）。
pub fn spawn_sword_intent(
    commands: &mut Commands,
    owner: Entity,
    origin: DVec3,
    target: Option<Entity>,
    damage_per_hit: f64,
    source: AttackSource,
) -> Entity {
    commands
        .spawn((
            SwordIntentEntity {
                owner,
                target,
                lifetime_ticks: SWORD_INTENT_LIFETIME_TICKS,
                hits_remaining: SWORD_INTENT_HIT_COUNT,
                damage_per_hit,
                source,
                hit_cooldown: 0,
            },
            Position::new([origin.x, origin.y, origin.z]),
        ))
        .id()
}

/// `World` 路径版本（供 skill_register.rs 的 `cast_manifest` 调用，
/// 因为 SkillFn 签名是 `fn(&mut World, ...) -> CastResult`，无 Commands 可用）。
pub fn spawn_sword_intent_in_world(
    world: &mut World,
    owner: Entity,
    origin: DVec3,
    target: Option<Entity>,
    damage_per_hit: f64,
    source: AttackSource,
) -> Entity {
    world
        .spawn((
            SwordIntentEntity {
                owner,
                target,
                lifetime_ticks: SWORD_INTENT_LIFETIME_TICKS,
                hits_remaining: SWORD_INTENT_HIT_COUNT,
                damage_per_hit,
                source,
                hit_cooldown: 0,
            },
            Position::new([origin.x, origin.y, origin.z]),
        ))
        .id()
}

// ── Tracking System ───────────────────────────────────────────────────────────

/// 每 tick 推进剑意实体：移动 → 命中判定 → 销毁。
pub fn sword_intent_tracking_system(
    tick: Option<Res<GameTick>>,
    mut intents: Query<(Entity, &mut SwordIntentEntity, &mut Position)>,
    target_positions: Query<&Position, Without<SwordIntentEntity>>,
    mut attack_events: EventWriter<AttackIntent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut commands: Commands,
) {
    let now = tick.map(|t| u64::from(t.0)).unwrap_or(0);

    for (entity, mut intent, mut pos) in &mut intents {
        // 1. 倒计时
        intent.lifetime_ticks = intent.lifetime_ticks.saturating_sub(1);
        intent.hit_cooldown = intent.hit_cooldown.saturating_sub(1);

        // 2. 朝目标移动
        if let Some(target_entity) = intent.target {
            if let Ok(target_pos) = target_positions.get(target_entity) {
                let current = pos.get();
                let target = target_pos.get();
                let delta = target - current;
                let dist = delta.length();
                if dist > f64::EPSILON {
                    let move_dist = dist.min(SWORD_INTENT_SPEED_PER_TICK);
                    let new_pos = current + delta.normalize() * move_dist;
                    pos.set([new_pos.x, new_pos.y, new_pos.z]);
                }

                // 3. 命中判定
                let current_after_move = pos.get();
                let dist_to_target = current_after_move.distance(target);
                if dist_to_target <= SWORD_INTENT_HIT_RADIUS && intent.hit_cooldown == 0 {
                    // emit AttackIntent
                    attack_events.send(AttackIntent {
                        attacker: intent.owner,
                        target: Some(target_entity),
                        issued_at_tick: now,
                        reach: AttackReach::new(SWORD_INTENT_HIT_RADIUS as f32, 0.0),
                        qi_invest: intent.damage_per_hit as f32,
                        wound_kind: WoundKind::Cut,
                        source: intent.source,
                        debug_command: None,
                    });
                    intent.hits_remaining = intent.hits_remaining.saturating_sub(1);
                    intent.hit_cooldown = SWORD_INTENT_HIT_COOLDOWN;

                    // 命中粒子
                    let hit_origin = current_after_move;
                    vfx_events.send(VfxEventRequest::new(
                        hit_origin,
                        VfxEventPayloadV1::SpawnParticle {
                            event_id: VFX_SWORD_INTENT_HIT.to_string(),
                            origin: [hit_origin.x, hit_origin.y, hit_origin.z],
                            direction: None,
                            color: Some("#E0E8D0".to_string()),
                            strength: Some(0.8),
                            count: Some(8),
                            duration_ticks: Some(16),
                        },
                    ));
                }
            }
        }

        // 4. 销毁条件
        if intent.hits_remaining == 0 || intent.lifetime_ticks == 0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};

    fn make_app() -> App {
        let mut app = App::new();
        app.add_event::<AttackIntent>()
            .add_event::<VfxEventRequest>()
            .add_systems(Update, sword_intent_tracking_system);
        app
    }

    fn spawn_intent(app: &mut App, target: Option<Entity>, origin: DVec3) -> Entity {
        let world = app.world_mut();
        world
            .spawn((
                SwordIntentEntity {
                    owner: Entity::PLACEHOLDER,
                    target,
                    lifetime_ticks: SWORD_INTENT_LIFETIME_TICKS,
                    hits_remaining: SWORD_INTENT_HIT_COUNT,
                    damage_per_hit: 10.0,
                    source: AttackSource::SwordPathManifest,
                    hit_cooldown: 0,
                },
                Position::new([origin.x, origin.y, origin.z]),
            ))
            .id()
    }

    #[test]
    fn tracking_moves_toward_target() {
        let mut app = make_app();
        // 实体在 (0,64,0)，目标在 (0,64,10)
        let target = app.world_mut().spawn(Position::new([0.0, 64.0, 10.0])).id();
        let intent_entity = spawn_intent(&mut app, Some(target), DVec3::new(0.0, 64.0, 0.0));

        app.update(); // 移动 0.5 格

        let new_pos = app
            .world()
            .get::<Position>(intent_entity)
            .map(|p| p.get())
            .expect("实体应存在");
        assert!(
            new_pos.z > 0.0 && new_pos.z <= SWORD_INTENT_SPEED_PER_TICK,
            "每 tick 应朝目标移动 SPEED_PER_TICK，实际 z={:.3}",
            new_pos.z
        );
    }

    #[test]
    fn destroyed_after_five_hits() {
        let mut app = make_app();
        app.insert_resource(GameTick(1));
        // 目标与实体重合（0 距离，立即命中），但 hit_cooldown 是 6，所以需要足够 tick
        let target = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let intent_entity = spawn_intent(&mut app, Some(target), DVec3::new(0.0, 64.0, 0.0));

        // 手动把实体放到目标同位置，使每次 hit_cooldown=0 时必然命中
        // 运行 5*(1+CD=6) = 35 tick，5 次命中后 hits_remaining=0 → despawn
        for i in 0..40u32 {
            app.world_mut().resource_mut::<GameTick>().0 = i + 1;
            app.update();
        }

        // 实体应已 despawn
        assert!(
            app.world()
                .get::<SwordIntentEntity>(intent_entity)
                .is_none(),
            "5 次命中后实体应 despawn"
        );
    }

    #[test]
    fn destroyed_on_lifetime_expiry() {
        let mut app = make_app();
        // 无目标，只倒计时
        let intent_entity = spawn_intent(&mut app, None, DVec3::new(0.0, 64.0, 0.0));

        // 运行 SWORD_INTENT_LIFETIME_TICKS+1 次
        for _ in 0..(SWORD_INTENT_LIFETIME_TICKS + 1) {
            app.update();
        }

        assert!(
            app.world()
                .get::<SwordIntentEntity>(intent_entity)
                .is_none(),
            "lifetime 耗尽后实体应 despawn"
        );
    }

    #[test]
    fn no_hit_without_target() {
        let mut app = make_app();
        spawn_intent(&mut app, None, DVec3::new(0.0, 64.0, 0.0));
        app.update();

        let attacks = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .count();
        assert_eq!(attacks, 0, "无目标时不应发出 AttackIntent");
    }

    #[test]
    fn hit_cooldown_prevents_multi_hit_same_tick() {
        let mut app = make_app();
        app.insert_resource(GameTick(1));
        // 目标在 0 格（重合），实体 hit_cooldown=0 → 第一 tick 命中，之后 cooldown=6
        let target = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        spawn_intent(&mut app, Some(target), DVec3::new(0.0, 64.0, 0.0));

        // 只运行 1 tick
        app.update();

        let attacks = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .count();
        assert_eq!(attacks, 1, "第一 tick 应命中恰好 1 次");
    }

    #[test]
    fn spawn_sword_intent_in_world_creates_correct_component() {
        let mut app = make_app();
        let owner = Entity::PLACEHOLDER;
        let origin = DVec3::new(5.0, 64.0, 5.0);
        let entity = spawn_sword_intent_in_world(
            app.world_mut(),
            owner,
            origin,
            None,
            15.0,
            AttackSource::SwordPathManifest,
        );

        let intent = app
            .world()
            .get::<SwordIntentEntity>(entity)
            .expect("生成后应有 SwordIntentEntity 组件");
        assert_eq!(intent.lifetime_ticks, SWORD_INTENT_LIFETIME_TICKS);
        assert_eq!(intent.hits_remaining, SWORD_INTENT_HIT_COUNT);
        assert_eq!(intent.damage_per_hit, 15.0);
        assert_eq!(intent.source, AttackSource::SwordPathManifest);
        assert_eq!(intent.hit_cooldown, 0);

        let pos = app
            .world()
            .get::<Position>(entity)
            .expect("应有 Position 组件");
        assert!(
            (pos.get().x - 5.0).abs() < 1e-6,
            "Position.x 应为 5.0，实际 {}",
            pos.get().x
        );
    }
}
