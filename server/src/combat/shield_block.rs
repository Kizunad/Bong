//! plan-shield-block-v1 P1 — 盾牌格挡持续状态与输入层。
//!
//! # 职责
//! - `ShieldBlock` ECS component：玩家当前正在持盾格挡的标记（独立于 Weapon 路径）。
//! - `raise_shield_handler` / `lower_shield_handler`：消费 server 端 RaiseShield/LowerShield
//!   事件（由 client_request_handler 投递），校验 off_hand 实装盾后操作 StatusEffects。
//! - `cleanup_shield_on_death`：死亡时强制清理防残留（接 DeathEvent）。
//! - `cleanup_shield_on_disconnect`：断线时强制清理（`.before(despawn_disconnected_clients)` 约束保证顺序）。
//!
//! # 接入注意
//! - StatusEffectKind::ShieldBlocking 的 `magnitude` 存储真实 block_ratio（P2 从 ShieldSpec 读取）。
//! - 动画触发（`bong:shield_raise`）通过 `vfx_animation_trigger::emit_shield_raise_for_entity`。
//! - P2 追加 `shield_fov_check`（正面 FOV 判定）和 `stamina_drain_shield_blocking_tick`（体力 drain）。

use valence::prelude::{
    bevy_ecs, Commands, Component, Entity, Event, EventReader, EventWriter, Position, Query, Res,
    UniqueId,
};

use crate::combat::components::{ActiveStatusEffect, Stamina, StaminaState, StatusEffects};
use crate::combat::events::{ApplyStatusEffectIntent, DeathEvent, StatusEffectKind};
use crate::combat::status::{has_active_status, remove_status_effect, upsert_status_effect};
use crate::combat::CombatClock;
use crate::inventory::{ItemRegistry, PlayerInventory, EQUIP_SLOT_OFF_HAND};
use crate::network::vfx_event_emit::VfxEventRequest;

/// plan-shield-block-v1 P1 — 玩家正在举盾的持续标记 component。
/// 独立于 Weapon component——盾无 weapon_spec，不走 combat/weapon.rs 路径。
#[derive(Debug, Clone, Component)]
pub struct ShieldBlock {
    /// off_hand 槽盾牌的模板 id（快照），用于后续 P3 读取 ShieldSpec。
    /// P1 存储但未读，P3 的 shield_fov_check 会消费。
    #[allow(dead_code)]
    pub template_id: String,
}

/// P1 内部事件：client_request_handler → raise_shield_handler。
#[derive(Debug, Clone, Event)]
pub struct RaiseShieldIntent {
    pub player: Entity,
}

/// P1 内部事件：client_request_handler → lower_shield_handler。
#[derive(Debug, Clone, Event)]
pub struct LowerShieldIntent {
    pub player: Entity,
}

/// plan-shield-block-v1 P1 — 举盾 ShieldBlocking 状态的持续 duration。
/// 超大 duration 让 status_effect_tick 不会在持举期间超时移除。
pub const SHIELD_BLOCKING_DURATION_TICKS: u64 = u64::MAX / 2;

/// plan-shield-block-v1 P2 — 正面 FOV 阈值常数（±120° 弧度，cos(-120°/2) = -0.5）。
/// 与境界无关，凡人盾无修士 FOV 加成。
pub const SHIELD_FOV_DOT: f64 = -0.5;

/// plan-shield-block-v1 P2 — 体力归零强制放盾时施加的破势硬直 ticks（约 1s = 20 ticks）。
/// 语义复用 ParryRecovery 破势硬直，防止玩家立刻再次举盾。
pub const SHIELD_EXHAUSTED_PARRY_RECOVERY_TICKS: u64 = 20;

/// 检查 off_hand 槽物品是否是已知盾牌模板。
/// 与 client InventoryEquipRules.SHIELD_TEMPLATE_IDS 保持同步。
pub fn is_shield_template_id(template_id: &str) -> bool {
    matches!(template_id, "wooden_shield" | "bone_shield")
}

/// plan-shield-block-v1 P2 — 盾牌正面 FOV 判定。
/// 检查攻击者是否在防御者正面 ±120° 范围内。
/// - dot ≥ -0.5 表示命中（在正面弧度内），返回 true。
/// - dot < -0.5 表示背面，返回 false（盾无效）。
/// - 绝不调用 `jiemai_fov_check`（其阈值随境界变化，盾不应有境界加成）。
/// - 无 Look 时（NPC 等无头部组件情况）保守返回 true（正面方向不确定，视为可挡）。
pub fn shield_fov_check(
    attacker_pos: valence::prelude::DVec3,
    defender_pos: valence::prelude::DVec3,
    defender_look: Option<&valence::entity::Look>,
) -> bool {
    let Some(look) = defender_look else {
        return true;
    };
    let to_attacker = valence::prelude::DVec3::new(
        attacker_pos.x - defender_pos.x,
        0.0,
        attacker_pos.z - defender_pos.z,
    );
    let len_sq = to_attacker.length_squared();
    if len_sq <= f64::EPSILON {
        return true;
    }
    let yaw = f64::from(look.yaw).to_radians();
    let facing = valence::prelude::DVec3::new(-yaw.sin(), 0.0, yaw.cos());
    facing.dot(to_attacker / len_sq.sqrt()) >= SHIELD_FOV_DOT
}

/// 处理 RaiseShieldIntent：校验 off_hand 盾 → 插入 ShieldBlock component + ShieldBlocking status。
/// P2：magnitude 从 ShieldSpec.block_ratio 读取；同时将 StaminaState 切换到 ShieldBlocking。
#[allow(clippy::too_many_arguments)]
pub fn raise_shield_handler(
    mut intents: EventReader<RaiseShieldIntent>,
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut status_q: Query<(
        &mut StatusEffects,
        Option<&ShieldBlock>,
        Option<&mut Stamina>,
    )>,
    inventory_q: Query<&PlayerInventory>,
    players_q: Query<(&Position, &UniqueId)>,
    item_registry: Option<Res<ItemRegistry>>,
) {
    for intent in intents.read() {
        let entity = intent.player;

        // 1. 读取 off_hand 槽物品 template_id
        let template_id = match inventory_q.get(entity) {
            Ok(inv) => {
                match inv
                    .equipped
                    .get(EQUIP_SLOT_OFF_HAND)
                    .map(|item| item.template_id.as_str())
                    .filter(|id| is_shield_template_id(id))
                {
                    Some(id) => id.to_string(),
                    None => {
                        tracing::debug!(
                            "[bong][shield] RaiseShield entity={entity:?}: off_hand is not a shield, ignoring"
                        );
                        continue;
                    }
                }
            }
            Err(_) => {
                tracing::debug!(
                    "[bong][shield] RaiseShield entity={entity:?}: no PlayerInventory component, ignoring"
                );
                continue;
            }
        };

        // P2 — 从 ShieldSpec 读取 block_ratio；如无 ItemRegistry（单测环境）fallback 到 0.5。
        let block_ratio = item_registry
            .as_deref()
            .and_then(|reg| reg.get(&template_id))
            .and_then(|tpl| tpl.shield_spec.as_ref())
            .map(|spec| spec.block_ratio as f32)
            .unwrap_or(0.5);

        // 2. 校验并操作 StatusEffects + component
        let Ok((mut status_effects, existing_shield_block, stamina)) = status_q.get_mut(entity)
        else {
            continue;
        };

        // 拒绝：Exhausted 状态下不允许举盾（体力不足）
        if stamina
            .as_ref()
            .is_some_and(|s| s.state == StaminaState::Exhausted)
        {
            tracing::debug!(
                "[bong][shield] RaiseShield entity={entity:?}: stamina exhausted, ignoring"
            );
            continue;
        }

        // 幂等：已在举盾状态则刷新持续时间即可（不叠加）
        if existing_shield_block.is_some()
            || has_active_status(&status_effects, StatusEffectKind::ShieldBlocking)
        {
            tracing::debug!(
                "[bong][shield] RaiseShield entity={entity:?}: already blocking, refreshing"
            );
            upsert_status_effect(
                &mut status_effects,
                ActiveStatusEffect {
                    kind: StatusEffectKind::ShieldBlocking,
                    magnitude: block_ratio,
                    remaining_ticks: SHIELD_BLOCKING_DURATION_TICKS,
                    source_pill: None,
                },
            );
            continue;
        }

        // 新举盾：插入状态
        upsert_status_effect(
            &mut status_effects,
            ActiveStatusEffect {
                kind: StatusEffectKind::ShieldBlocking,
                magnitude: block_ratio,
                remaining_ticks: SHIELD_BLOCKING_DURATION_TICKS,
                source_pill: None,
            },
        );
        // P2 — 切换体力状态到 ShieldBlocking（触发 stamina_tick 的持续 drain）
        if let Some(mut s) = stamina {
            if !matches!(s.state, StaminaState::Exhausted) {
                s.state = StaminaState::ShieldBlocking;
            }
        }
        // 插入 ShieldBlock component
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(ShieldBlock {
                template_id: template_id.clone(),
            });
        }

        tracing::debug!(
            "[bong][shield] RaiseShield entity={entity:?}: shield raised (template={template_id}, block_ratio={block_ratio:.2}) tick={}",
            clock.tick
        );

        // 3. 触发 bong:shield_raise 动画
        crate::network::vfx_animation_trigger::emit_shield_raise_for_entity(
            entity,
            &players_q,
            &mut vfx_events,
        );
    }
}

/// 处理 LowerShieldIntent：移除 ShieldBlocking 状态 + ShieldBlock component + 停止举盾动画。
/// P2：同时将 StaminaState 从 ShieldBlocking 恢复到 Idle（或 Combat）。
pub fn lower_shield_handler(
    mut intents: EventReader<LowerShieldIntent>,
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut status_q: Query<(&mut StatusEffects, Option<&mut Stamina>)>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    players_q: Query<(&Position, &UniqueId)>,
) {
    for intent in intents.read() {
        let entity = intent.player;
        if let Ok((mut status_effects, stamina)) = status_q.get_mut(entity) {
            remove_status_effect(&mut status_effects, StatusEffectKind::ShieldBlocking);
            // P2 — 恢复体力状态（ShieldBlocking → Idle；若已 Exhausted 则保留）
            if let Some(mut s) = stamina {
                if s.state == StaminaState::ShieldBlocking {
                    s.state = StaminaState::Idle;
                }
            }
        }
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<ShieldBlock>();
        }
        // 停止 isLoop:true 的举盾循环动画，否则客户端手臂永远卡在举盾姿势
        crate::network::vfx_animation_trigger::emit_shield_stop_for_entity(
            entity,
            &players_q,
            &mut vfx_events,
        );
        tracing::debug!(
            "[bong][shield] LowerShield entity={entity:?}: shield lowered tick={}",
            clock.tick
        );
    }
}

/// plan-shield-block-v1 P1 — 玩家死亡时强制清理 ShieldBlocking 状态，防残留。
/// 仍需发 StopAnim：玩家死亡后仍保持连接（死亡画面），视觉需要复位；
/// 若 entity 无 Position/UniqueId 则 emit_shield_stop_for_entity 会静默 skip，不 panic。
///
/// # 判据说明
/// StopAnim 的 emit 判据使用 `ShieldBlock` component 是否在场，**而非**
/// `has_active_status(ShieldBlocking)`。原因：此系统注册在
/// `death_arbiter_tick` 之后运行，而 `death_arbiter_tick` 内的 `enter_near_death`
/// 会无条件 `status_effects.active.clear()`，先于本系统清掉 ShieldBlocking status。
/// 若依赖 `has_active_status` 判断，死亡时举盾的 StopAnim 永远不会发出（生产孤岛）。
/// `ShieldBlock` component 是盾牌模块专属的持续标记，`enter_near_death` 不会触碰它，
/// 因此它在本系统运行时仍准确反映"玩家死前是否在举盾"。
pub fn cleanup_shield_on_death(
    mut death_events: EventReader<DeathEvent>,
    mut commands: Commands,
    mut status_q: Query<(&mut StatusEffects, Option<&ShieldBlock>)>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    players_q: Query<(&Position, &UniqueId)>,
) {
    for ev in death_events.read() {
        let entity = ev.target;
        // 以 ShieldBlock component 是否在场作为「死前举盾」的可靠真相源（见上注释）。
        let was_blocking = if let Ok((_, shield_block)) = status_q.get(entity) {
            shield_block.is_some()
        } else {
            false
        };
        // Defensive：若 ShieldBlocking status 仍在（如非 NearDeath 路径）也一并清理。
        if let Ok((mut status_effects, _)) = status_q.get_mut(entity) {
            if has_active_status(&status_effects, StatusEffectKind::ShieldBlocking) {
                remove_status_effect(&mut status_effects, StatusEffectKind::ShieldBlocking);
            }
        }
        if was_blocking {
            // 死亡时复位循环举盾动画，玩家死后仍连接需视觉复位
            crate::network::vfx_animation_trigger::emit_shield_stop_for_entity(
                entity,
                &players_q,
                &mut vfx_events,
            );
            tracing::debug!(
                "[bong][shield] cleanup_on_death: removed ShieldBlocking for {entity:?}"
            );
        }
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<ShieldBlock>();
        }
    }
}

/// plan-shield-block-v1 P2 — 体力归零时强制放盾。
///
/// 检测所有同时拥有 `ShieldBlock` component 且 `Stamina.state == Exhausted` 的实体，
/// 执行强制放盾流程：
/// 1. 移除 `ShieldBlocking` status + `ShieldBlock` component（与 lower_shield_handler 语义等价）。
/// 2. 施加短暂 `ParryRecovery`（复用破势硬直，防立即再举盾）。
/// 3. 发送 S2C StopAnim（通知 client 复位举盾姿态）。
///
/// 此系统运行在 `stamina_tick` 之后（确保 state 已更新为 Exhausted），
/// 在 `raise_shield_handler` 之前（Exhausted 下 raise 直接被体力系统拒绝，实测冗余但保险）。
pub fn force_lower_shield_on_stamina_exhausted(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut shield_q: Query<
        (Entity, &Stamina, &mut StatusEffects),
        valence::prelude::With<ShieldBlock>,
    >,
    mut status_effect_intents: EventWriter<ApplyStatusEffectIntent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    players_q: Query<(&Position, &UniqueId)>,
) {
    for (entity, stamina, mut status_effects) in &mut shield_q {
        if stamina.state != StaminaState::Exhausted {
            continue;
        }
        // 1. 移除 ShieldBlocking status
        remove_status_effect(&mut status_effects, StatusEffectKind::ShieldBlocking);
        // 2. 移除 ShieldBlock component
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<ShieldBlock>();
        }
        // 3. 施加 ParryRecovery 破势硬直（防立即再举盾）
        status_effect_intents.send(ApplyStatusEffectIntent {
            target: entity,
            kind: StatusEffectKind::ParryRecovery,
            magnitude: 1.0,
            duration_ticks: SHIELD_EXHAUSTED_PARRY_RECOVERY_TICKS,
            issued_at_tick: clock.tick,
        });
        // 4. 发 S2C StopAnim（通知 client 收举盾姿态）
        crate::network::vfx_animation_trigger::emit_shield_stop_for_entity(
            entity,
            &players_q,
            &mut vfx_events,
        );
        tracing::debug!(
            "[bong][shield] force_lower_shield entity={entity:?}: stamina exhausted → shield lowered tick={}",
            clock.tick
        );
    }
}

/// plan-shield-block-v1 P1 — 断线时强制清理盾牌格挡状态。
/// 在 `despawn_disconnected_clients` 之前运行（见 combat/mod.rs 的 `.before()` 约束）。
/// 使用 RemovedComponents<valence::prelude::Client> 探测断线实体。
/// 注：断线时实体即将 despawn、渲染模型随之移除，无需单独发 StopAnim（模型消失动画也随之消失）。
pub fn cleanup_shield_on_disconnect(
    mut commands: Commands,
    mut disconnected_clients: valence::prelude::RemovedComponents<valence::prelude::Client>,
    mut status_q: Query<&mut StatusEffects>,
) {
    for entity in disconnected_clients.read() {
        if let Ok(mut status_effects) = status_q.get_mut(entity) {
            if has_active_status(&status_effects, StatusEffectKind::ShieldBlocking) {
                remove_status_effect(&mut status_effects, StatusEffectKind::ShieldBlocking);
                tracing::debug!(
                    "[bong][shield] cleanup_on_disconnect: removed ShieldBlocking for {entity:?}"
                );
            }
        }
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<ShieldBlock>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::StatusEffects;
    use crate::combat::components::{CombatState, Lifecycle, Stamina, Wounds};
    use crate::combat::events::DeathInsightRequested;
    use crate::combat::events::StatusEffectKind;
    use crate::combat::lifecycle::death_arbiter_tick;
    use crate::combat::status::has_active_status;
    use crate::cultivation::death_hooks::{CultivationDeathTrigger, PlayerTerminated};
    use crate::cultivation::life_record::LifeRecord;
    use crate::inventory::{ItemInstance, PlayerInventory, EQUIP_SLOT_OFF_HAND};
    use crate::network::vfx_event_emit::VfxEventRequest;
    use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
    use crate::schema::vfx_event::VfxEventPayloadV1;
    use uuid::Uuid;
    use valence::prelude::{App, DVec3, Events, IntoSystemConfigs, Update};

    fn make_app() -> App {
        let mut app = App::new();
        app.add_event::<RaiseShieldIntent>();
        app.add_event::<LowerShieldIntent>();
        app.add_event::<DeathEvent>();
        app.add_event::<VfxEventRequest>();
        app.insert_resource(crate::combat::CombatClock::default());
        app.add_systems(
            Update,
            (
                raise_shield_handler,
                lower_shield_handler,
                cleanup_shield_on_death,
            ),
        );
        app
    }

    fn make_item_instance(template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 3.0,
            rarity: crate::inventory::ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn make_inventory_with_off_hand(template_id: &str) -> PlayerInventory {
        let mut inv = PlayerInventory {
            revision: crate::inventory::InventoryRevision(0),
            containers: vec![],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        };
        inv.equipped.insert(
            EQUIP_SLOT_OFF_HAND.to_string(),
            make_item_instance(template_id),
        );
        inv
    }

    fn make_inventory_empty() -> PlayerInventory {
        PlayerInventory {
            revision: crate::inventory::InventoryRevision(0),
            containers: vec![],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    // ── schema pin ──────────────────────────────────────────────────────────
    #[test]
    fn shield_blocking_variant_is_distinct_from_sword_parrying() {
        assert_ne!(
            StatusEffectKind::ShieldBlocking,
            StatusEffectKind::SwordParrying,
            "ShieldBlocking must not reuse SwordParrying variant"
        );
    }

    #[test]
    fn is_shield_template_known_shields() {
        assert!(is_shield_template_id("wooden_shield"));
        assert!(is_shield_template_id("bone_shield"));
    }

    #[test]
    fn is_shield_template_rejects_non_shield() {
        assert!(!is_shield_template_id("iron_sword"));
        assert!(!is_shield_template_id(""));
        assert!(!is_shield_template_id("wooden_shield_extra"));
    }

    // ── 动画隔离：SHIELD_RAISE vs GUARD_RAISE ID 不共用 ──────────────────────
    #[test]
    fn shield_raise_anim_id_distinct_from_guard_raise() {
        assert_ne!(
            crate::network::vfx_animation_trigger::ANIM_SHIELD_RAISE,
            "bong:guard_raise",
            "ANIM_SHIELD_RAISE must not equal 'bong:guard_raise' — would break FullPowerCharge"
        );
        assert_eq!(
            crate::network::vfx_animation_trigger::ANIM_SHIELD_RAISE,
            "bong:shield_raise",
            "ANIM_SHIELD_RAISE must equal 'bong:shield_raise'"
        );
    }

    // ── 大 duration 语义 ──────────────────────────────────────────────────
    #[test]
    fn shield_blocking_duration_is_very_large() {
        // Use const assertion to avoid clippy::assertions_on_constants.
        const _: () = assert!(
            SHIELD_BLOCKING_DURATION_TICKS > 20 * 60 * 60,
            "SHIELD_BLOCKING_DURATION_TICKS must be large enough to not expire during normal gameplay (>72000 ticks)"
        );
        // Suppress "test never panics" — the const assertion above is the real check.
        let _ = SHIELD_BLOCKING_DURATION_TICKS;
    }

    // ── 状態転換: off_hand 無盾 → RaiseShield 被拒 ──────────────────────────
    #[test]
    fn raise_shield_rejected_when_no_shield_in_off_hand() {
        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((StatusEffects::default(), make_inventory_empty()))
            .id();
        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            !has_active_status(status, StatusEffectKind::ShieldBlocking),
            "ShieldBlocking must NOT be inserted when off_hand has no shield"
        );
        assert!(
            app.world().entity(entity).get::<ShieldBlock>().is_none(),
            "ShieldBlock component must NOT be inserted when off_hand has no shield"
        );
    }

    // ── 状態転換: 無盾 → Raise → ShieldBlocking 挿入 ────────────────────────
    #[test]
    fn raise_shield_inserts_status_when_shield_in_off_hand() {
        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            has_active_status(status, StatusEffectKind::ShieldBlocking),
            "ShieldBlocking must be inserted when wooden_shield is in off_hand"
        );
        assert!(
            app.world().entity(entity).get::<ShieldBlock>().is_some(),
            "ShieldBlock component must be inserted after RaiseShield"
        );
    }

    // ── 状態転換: bone_shield も受け入れる ──────────────────────────────────
    #[test]
    fn raise_shield_accepts_bone_shield() {
        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("bone_shield"),
            ))
            .id();
        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            has_active_status(status, StatusEffectKind::ShieldBlocking),
            "ShieldBlocking must be inserted when bone_shield is in off_hand"
        );
    }

    // ── ShieldBlocking → LowerShield → 移除 ────────────────────────────────
    #[test]
    fn lower_shield_removes_status_and_component() {
        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
            ))
            .id();

        // First raise
        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        // Then lower
        app.world_mut()
            .resource_mut::<Events<LowerShieldIntent>>()
            .send(LowerShieldIntent { player: entity });
        app.update();

        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            !has_active_status(status, StatusEffectKind::ShieldBlocking),
            "ShieldBlocking must be removed after LowerShield"
        );
        assert!(
            app.world().entity(entity).get::<ShieldBlock>().is_none(),
            "ShieldBlock component must be removed after LowerShield"
        );
    }

    // ── ShieldBlocking → 死亡 → 強制削除 ────────────────────────────────────
    #[test]
    fn cleanup_on_death_removes_shield_blocking() {
        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
            ))
            .id();

        // Raise
        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        // Send death event
        app.world_mut()
            .resource_mut::<Events<DeathEvent>>()
            .send(DeathEvent {
                target: entity,
                cause: "test_death".to_string(),
                attacker: None,
                attacker_player_id: None,
                at_tick: 0,
            });
        app.update();

        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            !has_active_status(status, StatusEffectKind::ShieldBlocking),
            "ShieldBlocking must be forcibly removed on death to prevent state residual"
        );
        assert!(
            app.world().entity(entity).get::<ShieldBlock>().is_none(),
            "ShieldBlock component must be removed on death"
        );
    }

    // ── 重複 Raise 幂等性 ─────────────────────────────────────────────────────
    #[test]
    fn raise_shield_is_idempotent() {
        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
            ))
            .id();

        // Raise twice
        {
            let mut events = app.world_mut().resource_mut::<Events<RaiseShieldIntent>>();
            events.send(RaiseShieldIntent { player: entity });
        }
        app.update();
        {
            let mut events = app.world_mut().resource_mut::<Events<RaiseShieldIntent>>();
            events.send(RaiseShieldIntent { player: entity });
        }
        app.update();

        // Should still have exactly one ShieldBlocking entry
        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        let shield_count = status
            .active
            .iter()
            .filter(|e| e.kind == StatusEffectKind::ShieldBlocking)
            .count();
        assert_eq!(
            shield_count, 1,
            "Repeated RaiseShield must not stack ShieldBlocking — expected exactly 1, got {shield_count}"
        );
    }

    // ── ShieldBlocking → 断线 → 強制削除 ──────────────────────────────────────
    // plan-shield-block-v1 P1 — 验证 cleanup_shield_on_disconnect 的逻辑等价语义：
    // 玩家断线后 ShieldBlocking 状态 + ShieldBlock component 被强制移除，防止残留。
    //
    // ⚠️ 局限性说明：
    // cleanup_shield_on_disconnect 使用 RemovedComponents<valence::prelude::Client> 探测断线，
    // 而 valence Client 不可在测试中构造（非 unit-constructible），故真实断线的运行时行为
    // **未被本测试锁定**。此处仅通过 lower_shield_handler 路径验证语义等价的状态移除。
    // 真断线运行时行为未被锁定（Client 不可单测构造），此处仅验语义等价的状态移除。
    // 以下断言：
    //   - 系统注册已在 combat/mod.rs:418 中声明（编译守护）
    //   - cleanup_shield_on_disconnect 函数签名接受 RemovedComponents<Client>（编译守护）
    //   - 清理后状态完全干净（语义等价断言）
    #[test]
    fn cleanup_on_disconnect_semantic_removes_shield_blocking() {
        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
            ))
            .id();

        // 先举盾 → 插入 ShieldBlocking
        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            has_active_status(status, StatusEffectKind::ShieldBlocking),
            "前提：ShieldBlocking 应在 RaiseShield 后插入"
        );

        // cleanup_shield_on_disconnect 与 lower_shield_handler 语义等价：
        // 同样调用 remove_status_effect + commands.remove::<ShieldBlock>()
        app.world_mut()
            .resource_mut::<Events<LowerShieldIntent>>()
            .send(LowerShieldIntent { player: entity });
        app.update();

        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            !has_active_status(status, StatusEffectKind::ShieldBlocking),
            "断线等价路径：ShieldBlocking 必须强制移除，防止断线后格挡状态残留"
        );
        assert!(
            app.world().entity(entity).get::<ShieldBlock>().is_none(),
            "断线等价路径：ShieldBlock component 必须在清理后移除"
        );
    }

    // ── cleanup_shield_on_disconnect 系统编译注册断言 ─────────────────────────
    // 断言 cleanup_shield_on_disconnect 函数可以被引用（编译守护：签名匹配 System trait）。
    // 真实 RemovedComponents<Client> 触发路径由 combat/mod.rs:418 系统注册保证。
    #[test]
    fn cleanup_on_disconnect_function_compiles_as_system() {
        // 通过将函数放入系统集来验证类型签名正确（编译守护）
        let mut app = App::new();
        app.add_event::<RaiseShieldIntent>();
        app.add_event::<LowerShieldIntent>();
        app.add_event::<DeathEvent>();
        app.add_event::<VfxEventRequest>();
        app.insert_resource(crate::combat::CombatClock::default());
        app.add_systems(Update, cleanup_shield_on_disconnect);
        // 只需 app 能构建不 panic，不需要 update（没有真实 Client component）
        // 这确保 RemovedComponents<valence::prelude::Client> 的系统签名编译正确
        let _ = app; // suppress unused warning
    }

    // ── 动画隔离：guard_raise.json 的 isLoop 仍为 false（防回归）─────────────
    // plan-shield-block-v1 P1 §10.1 — 断言 guard_raise.json 未被此 PR 改动，
    // isLoop 仍为 false（FullPowerCharge 消费方要求），防止 shield_raise 独立化被破坏。
    #[test]
    fn guard_raise_json_is_loop_false_untouched() {
        // 读取 guard_raise.json，断言 isLoop:false 未被修改
        let json_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../client/src/main/resources/assets/bong/player_animation/guard_raise.json"
        );
        let content = std::fs::read_to_string(json_path)
            .expect("guard_raise.json must exist — it is the FullPowerCharge animation asset");
        let value: serde_json::Value =
            serde_json::from_str(&content).expect("guard_raise.json must be valid JSON");
        let is_loop = value
            .get("emote")
            .and_then(|e| e.get("isLoop"))
            .and_then(|v| v.as_bool());
        assert_eq!(
            is_loop,
            Some(false),
            "guard_raise.json emote.isLoop must remain false (FullPowerCharge consumes it, \
             changing to true would break the 4-tick snap-up charge pose) — \
             plan-shield-block-v1 P1 uses a separate shield_raise.json with isLoop:true"
        );
        // 同时断言 endTick=4（FullPowerCharge 4-tick snap-up 语义）
        let end_tick = value
            .get("emote")
            .and_then(|e| e.get("endTick"))
            .and_then(|v| v.as_u64());
        assert_eq!(
            end_tick,
            Some(4),
            "guard_raise.json emote.endTick must remain 4 — FullPowerCharge 4-tick snap-up semantics"
        );
    }

    // ── shield_raise.json isLoop=true 正向断言 ────────────────────────────────
    #[test]
    fn shield_raise_json_is_loop_true() {
        let json_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../client/src/main/resources/assets/bong/player_animation/shield_raise.json"
        );
        let content = std::fs::read_to_string(json_path)
            .expect("shield_raise.json must exist — created by plan-shield-block-v1 P1");
        let value: serde_json::Value =
            serde_json::from_str(&content).expect("shield_raise.json must be valid JSON");
        let is_loop = value
            .get("emote")
            .and_then(|e| e.get("isLoop"))
            .and_then(|v| v.as_bool());
        assert_eq!(
            is_loop,
            Some(true),
            "shield_raise.json emote.isLoop must be true —持续举盾姿态需要循环播放"
        );
        // PROMISE 块检查
        assert!(
            content.contains("<PROMISE>"),
            "shield_raise.json must contain a <PROMISE> block as required by §10.1 3-round polish rule"
        );
    }

    // ── 辅助：带 Position + UniqueId 的完整实体（动画 emit 需要这两个 component）───
    fn spawn_entity_with_pos_uid(app: &mut App, name: &str) -> Entity {
        let uid = UniqueId(Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes()));
        app.world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
                Position::new(DVec3::new(0.0, 64.0, 0.0)),
                uid,
            ))
            .id()
    }

    fn drain_vfx(app: &mut App) -> Vec<VfxEventRequest> {
        app.world_mut()
            .resource_mut::<Events<VfxEventRequest>>()
            .drain()
            .collect()
    }

    fn find_play_anim<'a>(
        reqs: &'a [VfxEventRequest],
        anim_id: &str,
    ) -> Option<&'a VfxEventRequest> {
        reqs.iter().find(|r| {
            matches!(&r.payload, VfxEventPayloadV1::PlayAnim { anim_id: id, .. } if id == anim_id)
        })
    }

    fn find_stop_anim<'a>(
        reqs: &'a [VfxEventRequest],
        anim_id: &str,
    ) -> Option<&'a VfxEventRequest> {
        reqs.iter().find(|r| {
            matches!(&r.payload, VfxEventPayloadV1::StopAnim { anim_id: id, .. } if id == anim_id)
        })
    }

    // ── #2: raise_shield_handler 发 PlayAnim{bong:shield_raise} ────────────────
    // 验证完整实体（带 Position+UniqueId）触发 RaiseShield 后 VfxEventRequest 队列含
    // PlayAnim{anim_id=="bong:shield_raise"}。锁住动画 emit 分支——之前此分支零覆盖。
    #[test]
    fn raise_shield_emits_play_anim_when_entity_has_position_and_unique_id() {
        let mut app = make_app();
        let entity = spawn_entity_with_pos_uid(&mut app, "alice_raise");

        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        let emitted = drain_vfx(&mut app);
        let play = find_play_anim(
            &emitted,
            crate::network::vfx_animation_trigger::ANIM_SHIELD_RAISE,
        );
        assert!(
            play.is_some(),
            "raise_shield_handler must emit PlayAnim{{anim_id==\"bong:shield_raise\"}} \
             when entity has Position+UniqueId — emit branch was previously unreachable in tests; \
             emitted events: {emitted:?}"
        );
    }

    // ── #1: lower_shield_handler 发 StopAnim{bong:shield_raise} ───────────────
    // 验证 LowerShield 后 VfxEventRequest 队列含 StopAnim{anim_id=="bong:shield_raise"}，
    // 锁住「松开右键→停循环动画」闭环。
    #[test]
    fn lower_shield_emits_stop_anim_when_entity_has_position_and_unique_id() {
        let mut app = make_app();
        let entity = spawn_entity_with_pos_uid(&mut app, "alice_lower");

        // 先举盾（产生 PlayAnim），再排空，专注验证 LowerShield 的 StopAnim
        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();
        let _ = drain_vfx(&mut app); // discard raise events

        // 放盾
        app.world_mut()
            .resource_mut::<Events<LowerShieldIntent>>()
            .send(LowerShieldIntent { player: entity });
        app.update();

        let emitted = drain_vfx(&mut app);
        let stop = find_stop_anim(
            &emitted,
            crate::network::vfx_animation_trigger::ANIM_SHIELD_RAISE,
        );
        assert!(
            stop.is_some(),
            "lower_shield_handler must emit StopAnim{{anim_id==\"bong:shield_raise\"}} \
             after LowerShieldIntent — isLoop:true animation must be explicitly stopped; \
             emitted events: {emitted:?}"
        );
    }

    // ── #1: cleanup_shield_on_death 发 StopAnim{bong:shield_raise} ────────────
    // 死亡时举盾态的循环动画必须被停止（死亡后仍连接，视觉需复位）。
    #[test]
    fn cleanup_on_death_emits_stop_anim_when_blocking() {
        let mut app = make_app();
        let entity = spawn_entity_with_pos_uid(&mut app, "alice_death");

        // 举盾
        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();
        let _ = drain_vfx(&mut app); // discard raise events

        // 触发死亡
        app.world_mut()
            .resource_mut::<Events<DeathEvent>>()
            .send(DeathEvent {
                target: entity,
                cause: "test_death_anim".to_string(),
                attacker: None,
                attacker_player_id: None,
                at_tick: 0,
            });
        app.update();

        let emitted = drain_vfx(&mut app);
        let stop = find_stop_anim(
            &emitted,
            crate::network::vfx_animation_trigger::ANIM_SHIELD_RAISE,
        );
        assert!(
            stop.is_some(),
            "cleanup_shield_on_death must emit StopAnim{{anim_id==\"bong:shield_raise\"}} \
             when player dies while blocking — isLoop:true needs explicit stop even on death; \
             emitted events: {emitted:?}"
        );
    }

    // ── #1: cleanup_shield_on_death 无举盾时不发 StopAnim ────────────────────
    // 死亡时没有 ShieldBlocking 状态，不应 emit StopAnim（无举盾无需停动画）。
    #[test]
    fn cleanup_on_death_no_stop_anim_when_not_blocking() {
        let mut app = make_app();
        let entity = spawn_entity_with_pos_uid(&mut app, "alice_death_no_block");

        // 直接死亡，未举盾
        app.world_mut()
            .resource_mut::<Events<DeathEvent>>()
            .send(DeathEvent {
                target: entity,
                cause: "test_death_no_block".to_string(),
                attacker: None,
                attacker_player_id: None,
                at_tick: 0,
            });
        app.update();

        let emitted = drain_vfx(&mut app);
        let stop = find_stop_anim(
            &emitted,
            crate::network::vfx_animation_trigger::ANIM_SHIELD_RAISE,
        );
        assert!(
            stop.is_none(),
            "cleanup_shield_on_death must NOT emit StopAnim when player was not blocking; \
             emitted events: {emitted:?}"
        );
    }

    // ── e2e 生产序：death_arbiter_tick 先清 status → cleanup_shield_on_death 仍发 StopAnim ──
    //
    // 这是锁定「生产执行序」的真 e2e 测试。
    // 背景：mod.rs 注册 cleanup_shield_on_death .after(death_arbiter_tick)。
    // death_arbiter_tick 内的 enter_near_death 会无条件 status_effects.active.clear()，
    // 在 cleanup_shield_on_death 运行前已清空 ShieldBlocking status。
    // 若 cleanup_shield_on_death 以 has_active_status(ShieldBlocking) 判 emit，
    // 生产环境下 StopAnim 永远不发出（isLoop:true 的 bong:shield_raise 在死亡后永久卡住）。
    //
    // 此测试在同一 App 内同时注册两个系统（保持 after 顺序），用带完整组件的真实 DeathEvent
    // 触发完整死亡链路，断言「death_arbiter 先清 status 后 cleanup_shield 仍发 StopAnim」。
    // 破坏修法（改回 has_active_status 判断）必须让此测试撞红。
    #[test]
    fn e2e_cleanup_on_death_emits_stop_anim_after_death_arbiter_clears_status() {
        use std::time::{SystemTime, UNIX_EPOCH};

        // -- 构造 PersistenceSettings（death_arbiter_tick 需要此 Res）--
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bong-shield-death-e2e-{}-{unique_suffix}",
            std::process::id()
        ));
        let db_path = root.join("data").join("bong.db");
        std::fs::create_dir_all(db_path.parent().unwrap())
            .expect("temp dir creation should succeed");
        bootstrap_sqlite(&db_path, "shield-death-e2e").expect("sqlite bootstrap should succeed");
        let persistence =
            PersistenceSettings::with_paths(&db_path, root.join("deceased"), "shield-death-e2e");

        // -- App 构建：注册 death_arbiter_tick + cleanup_shield_on_death（保持 after 顺序）--
        let mut app = App::new();
        app.insert_resource(persistence);
        app.insert_resource(crate::combat::CombatClock { tick: 1 });
        // death_arbiter_tick 需要的事件
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        // shield handler 需要的事件
        app.add_event::<RaiseShieldIntent>();
        app.add_event::<LowerShieldIntent>();
        // 按 mod.rs 注册顺序：cleanup_shield_on_death after death_arbiter_tick
        app.add_systems(
            Update,
            (
                death_arbiter_tick,
                cleanup_shield_on_death.after(death_arbiter_tick),
            ),
        );

        // -- spawn 带完整组件的玩家实体（Lifecycle + StatusEffects 含 ShieldBlocking + ShieldBlock）--
        let uid = UniqueId(Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            b"shield_death_e2e_player",
        ));
        let entity = app
            .world_mut()
            .spawn((
                Wounds {
                    health_current: 0.0,
                    health_max: 30.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                LifeRecord::default(),
                Lifecycle {
                    fortune_remaining: 1,
                    ..Default::default()
                },
                // ShieldBlocking status — death_arbiter 会在 cleanup_shield_on_death 前清掉它
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::ShieldBlocking,
                        magnitude: 0.5,
                        remaining_ticks: SHIELD_BLOCKING_DURATION_TICKS,
                        source_pill: None,
                    }],
                },
                // ShieldBlock component — enter_near_death 不会碰它，是可靠真相源
                ShieldBlock {
                    template_id: "wooden_shield".to_string(),
                },
                Position::new(DVec3::new(8.0, 64.0, 8.0)),
                uid,
            ))
            .id();

        // -- 发送真实 DeathEvent，触发完整死亡链路 --
        app.world_mut()
            .resource_mut::<Events<DeathEvent>>()
            .send(DeathEvent {
                target: entity,
                cause: "e2e_test_death_while_blocking".to_string(),
                attacker: None,
                attacker_player_id: None,
                at_tick: 1,
            });

        // 排空 Raise 前的 VfxEvents（此处无，但保持干净）
        let _ = drain_vfx(&mut app);

        app.update();

        // -- 关键断言：death_arbiter 先清 ShieldBlocking status，但 StopAnim 仍必须发出 --
        let status_after = app.world().entity(entity).get::<StatusEffects>();
        if let Some(status) = status_after {
            assert!(
                !has_active_status(status, StatusEffectKind::ShieldBlocking),
                "death_arbiter_tick must have cleared ShieldBlocking status via enter_near_death.active.clear()"
            );
        }
        // ShieldBlock component 也必须被 cleanup_shield_on_death 移除
        assert!(
            app.world().entity(entity).get::<ShieldBlock>().is_none(),
            "ShieldBlock component must be removed by cleanup_shield_on_death after death"
        );
        // 核心断言：即使 death_arbiter 先清了 status，StopAnim 仍必须发出
        let emitted = drain_vfx(&mut app);
        let stop = find_stop_anim(
            &emitted,
            crate::network::vfx_animation_trigger::ANIM_SHIELD_RAISE,
        );
        assert!(
            stop.is_some(),
            "cleanup_shield_on_death MUST emit StopAnim{{anim_id==\"bong:shield_raise\"}} even after \
             death_arbiter_tick clears ShieldBlocking via enter_near_death.active.clear(). \
             If this fails, the emit gate has regressed to has_active_status() which is always false \
             at this point in the execution order — the isLoop:true shield_raise animation would be \
             permanently stuck on connected-but-dead players. \
             emitted events: {emitted:?}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    // ── 同帧 raise+lower 净结果放盾（顺序约束测试）────────────────────────────
    //
    // plan-shield-block-v1 P1 §must-fix: 同 tick RaiseShieldIntent + LowerShieldIntent
    // 并发送达时，lower_shield_handler 必须在 raise_shield_handler 之后执行（.after 约束）。
    // 净结果：玩家松键 → 放盾（无 ShieldBlocking status，无 ShieldBlock component）。
    //
    // 安全方向：松键即放盾，不留残留 ShieldBlocking（P2 减伤路径的 exploit 根源被消除）。
    //
    // 约束失效时的语义：若 lower 先 raise 后，最终 ShieldBlocking 被留存（残留 exploit）。
    // 本测试锁定「净放盾」语义，并间接锁定 .after 顺序约束。
    #[test]
    fn same_tick_raise_then_lower_net_result_is_shield_down() {
        // 独立 App，显式注册带 .after 约束的系统（镜像 combat/mod.rs 真实注册）
        let mut app = App::new();
        app.add_event::<RaiseShieldIntent>();
        app.add_event::<LowerShieldIntent>();
        app.add_event::<DeathEvent>();
        app.add_event::<VfxEventRequest>();
        app.insert_resource(crate::combat::CombatClock::default());
        // 关键：lower_shield_handler .after(raise_shield_handler)，镜像 mod.rs 约束
        app.add_systems(
            Update,
            (
                raise_shield_handler,
                lower_shield_handler.after(raise_shield_handler),
            ),
        );

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
            ))
            .id();

        // 同 tick 内同时投递 Raise + Lower（模拟同帧 dispatch）
        {
            let world = app.world_mut();
            world
                .resource_mut::<Events<RaiseShieldIntent>>()
                .send(RaiseShieldIntent { player: entity });
            world
                .resource_mut::<Events<LowerShieldIntent>>()
                .send(LowerShieldIntent { player: entity });
        }

        // 单次 update = 同 tick 执行顺序：raise_shield_handler 先，lower_shield_handler 后
        app.update();

        // 净结果断言：放盾（松键优先，安全方向）
        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            !has_active_status(status, StatusEffectKind::ShieldBlocking),
            "同 tick raise+lower 净结果必须是放盾（无 ShieldBlocking status）。\
             若 lower 先 raise 后执行，ShieldBlocking 会被残留——松键后免费减伤 exploit。\
             .after(raise_shield_handler) 约束保证 raise→lower 顺序，净结果应为放盾。\
             actual status.active: {:?}",
            status.active
        );
        assert!(
            app.world().entity(entity).get::<ShieldBlock>().is_none(),
            "同 tick raise+lower 净结果必须无 ShieldBlock component（松键应彻底清理）。\
             actual: component still present"
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // plan-shield-block-v1 P2 — FOV 判定 / StaminaState / force-lower 饱和化测试
    // ══════════════════════════════════════════════════════════════════════════

    // ── shield_fov_check happy path：正面（dot ≥ -0.5）──────────────────────
    #[test]
    fn shield_fov_check_front_face_returns_true() {
        use valence::entity::Look;
        // 防御者在 (0,0,0) 朝 +Z（yaw=0），攻击者在 (0,0,1)——正面
        let defender_pos = DVec3::ZERO;
        let attacker_pos = DVec3::new(0.0, 0.0, 2.0);
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        assert!(
            shield_fov_check(attacker_pos, defender_pos, Some(&look)),
            "攻击者在防御者正前方时 shield_fov_check 应返回 true（dot=1.0 ≥ SHIELD_FOV_DOT=-0.5）"
        );
    }

    // ── shield_fov_check 背面（dot < -0.5）──────────────────────────────────
    #[test]
    fn shield_fov_check_back_face_returns_false() {
        use valence::entity::Look;
        // 防御者在 (0,0,0) 朝 +Z（yaw=0），攻击者在 (0,0,-2)——背后 dot=-1.0
        let defender_pos = DVec3::ZERO;
        let attacker_pos = DVec3::new(0.0, 0.0, -2.0);
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        assert!(
            !shield_fov_check(attacker_pos, defender_pos, Some(&look)),
            "攻击者在防御者正后方时 shield_fov_check 应返回 false（dot=-1.0 < SHIELD_FOV_DOT=-0.5）；\
             背面命中不应被盾拦截"
        );
    }

    // ── shield_fov_check 边界 dot=-0.5 恰好通过 ─────────────────────────────
    #[test]
    fn shield_fov_check_boundary_dot_minus_half_passes() {
        use valence::entity::Look;
        // dot = cos(120°) = -0.5 的方向：攻击者在 120° 侧翼
        // 防御者朝 +Z（yaw=0），facing=(0,0,1)
        // 攻击者方向：(-sin(120°), 0, cos(120°)) = (-√3/2, 0, -0.5)，normalize 后 dot with (0,0,1) = -0.5
        let angle: f64 = 120.0_f64.to_radians();
        let attacker_dir = DVec3::new(-angle.sin(), 0.0, angle.cos());
        let attacker_pos = attacker_dir * 2.0; // 距离 2
        let defender_pos = DVec3::ZERO;
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        let result = shield_fov_check(attacker_pos, defender_pos, Some(&look));
        assert!(
            result,
            "dot=-0.5 恰好等于 SHIELD_FOV_DOT 阈值，应 >= 比较为 true（>=, not >）；\
             shield_fov_check 应返回 true，实际返回 false"
        );
    }

    // ── shield_fov_check 边界 dot 稍小于 -0.5 被拒 ──────────────────────────
    #[test]
    fn shield_fov_check_boundary_just_past_threshold_fails() {
        use valence::entity::Look;
        // 攻击者方向：120° + 小偏差 → dot 略小于 -0.5
        let angle: f64 = 121.0_f64.to_radians(); // 1° 超出
        let attacker_dir = DVec3::new(-angle.sin(), 0.0, angle.cos());
        let attacker_pos = attacker_dir * 2.0;
        let defender_pos = DVec3::ZERO;
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        assert!(
            !shield_fov_check(attacker_pos, defender_pos, Some(&look)),
            "dot < -0.5 时 shield_fov_check 应返回 false（超出 ±120° 格挡弧）"
        );
    }

    // ── shield_fov_check no-Look 保守返回 true ───────────────────────────────
    #[test]
    fn shield_fov_check_no_look_returns_true_conservatively() {
        // 无 Look component（如 NPC）时保守视为正面可挡
        let result = shield_fov_check(DVec3::new(1.0, 0.0, 0.0), DVec3::ZERO, None);
        assert!(
            result,
            "无 Look 组件时 shield_fov_check 应保守返回 true（正面方向不确定）"
        );
    }

    // ── shield_fov_check 零距离保守返回 true ────────────────────────────────
    #[test]
    fn shield_fov_check_zero_distance_returns_true_conservatively() {
        use valence::entity::Look;
        // 攻击者与防御者同位置（零向量无法 normalize）
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        let result = shield_fov_check(DVec3::ZERO, DVec3::ZERO, Some(&look));
        assert!(
            result,
            "攻击者与防御者同位置（零距离）时 shield_fov_check 应保守返回 true，不除以零"
        );
    }

    // ── StaminaState::ShieldBlocking 在 stamina_tick 中独立 drain ────────────
    // 通过直接调用 stamina_tick 系统验证 ShieldBlocking 状态的 drain 速率正确。
    #[test]
    fn stamina_tick_shield_blocking_drains_at_correct_rate() {
        use crate::combat::components::{Stamina, StaminaState};
        use crate::combat::lifecycle::{stamina_tick, SHIELD_DRAIN_PER_SEC};

        let mut app = App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 0 });
        app.add_systems(Update, stamina_tick);

        let entity = app
            .world_mut()
            .spawn(Stamina {
                current: 100.0,
                max: 100.0,
                recover_per_sec: 5.0,
                state: StaminaState::ShieldBlocking,
                last_drain_tick: None,
            })
            .id();

        app.update();

        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        // stamina_tick 每 STAMINA_TICK_INTERVAL_TICKS=4 ticks 更新一次（dt = 4/20 = 0.2s）
        // drain = SHIELD_DRAIN_PER_SEC * dt = 3.0 * 0.2 = 0.6
        use crate::combat::components::{STAMINA_TICK_INTERVAL_TICKS, TICKS_PER_SECOND};
        let dt = STAMINA_TICK_INTERVAL_TICKS as f32 / TICKS_PER_SECOND as f32;
        let expected_drain = SHIELD_DRAIN_PER_SEC * dt;
        let expected_current = 100.0 - expected_drain;
        assert!(
            (stamina.current - expected_current).abs() < 0.01,
            "ShieldBlocking stamina drain 应为 {expected_drain:.4}（drain_rate={SHIELD_DRAIN_PER_SEC} * dt={dt:.3}），\
             期望 current≈{expected_current:.4}，实际 {:.4}",
            stamina.current
        );
        assert_eq!(
            stamina.state,
            StaminaState::ShieldBlocking,
            "ShieldBlocking 状态在体力未耗尽时应保持不变"
        );
    }

    // ── 体力归零时 stamina_tick 将状态切换到 Exhausted ──────────────────────
    #[test]
    fn stamina_tick_shield_blocking_transitions_to_exhausted_when_depleted() {
        use crate::combat::components::{Stamina, StaminaState};
        use crate::combat::lifecycle::stamina_tick;

        let mut app = App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 0 });
        app.add_systems(Update, stamina_tick);

        // 极低体力确保一 tick 内必耗尽
        let entity = app
            .world_mut()
            .spawn(Stamina {
                current: 0.01,
                max: 100.0,
                recover_per_sec: 0.0,
                state: StaminaState::ShieldBlocking,
                last_drain_tick: None,
            })
            .id();

        app.update();

        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        assert_eq!(
            stamina.state,
            StaminaState::Exhausted,
            "ShieldBlocking 体力耗尽时 stamina_tick 应将状态切为 Exhausted，\
             actual: {:?}",
            stamina.state
        );
    }

    // ── 持续举盾可支撑时间与 max_stamina 成正比 ─────────────────────────────
    #[test]
    fn shield_blocking_hold_duration_proportional_to_max_stamina() {
        use crate::combat::components::TICKS_PER_SECOND;
        use crate::combat::lifecycle::SHIELD_DRAIN_PER_SEC;
        // hold_seconds = max_stamina / drain_per_sec（与 STAMINA_TICK_INTERVAL_TICKS 无关）
        let max_stamina = 60.0_f32;
        let hold_seconds = max_stamina / SHIELD_DRAIN_PER_SEC;
        let expected_ticks = (hold_seconds * TICKS_PER_SECOND as f32) as u64;
        // 60 / 3.0 = 20s = 400 game-ticks（@ 20 tps）
        assert_eq!(
            expected_ticks, 400,
            "max_stamina=60 / SHIELD_DRAIN_PER_SEC=3.0 应支撑 400 game-ticks（20s @ 20tps）；\
             SHIELD_DRAIN_PER_SEC 或 TICKS_PER_SECOND 改动会破坏此断言；\
             实际计算 {expected_ticks}"
        );
    }

    // ── raise_shield 拒绝 Exhausted 状态下举盾 ──────────────────────────────
    #[test]
    fn raise_shield_rejected_when_stamina_exhausted() {
        use crate::combat::components::{Stamina, StaminaState};

        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
                Stamina {
                    current: 0.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    state: StaminaState::Exhausted,
                    last_drain_tick: None,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        let status = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            !has_active_status(status, StatusEffectKind::ShieldBlocking),
            "Exhausted 状态下 raise_shield 应被拒绝，ShieldBlocking 不应被插入；\
             actual status.active: {:?}",
            status.active
        );
    }

    // ── raise_shield 切换 StaminaState 到 ShieldBlocking ────────────────────
    #[test]
    fn raise_shield_transitions_stamina_state_to_shield_blocking() {
        use crate::combat::components::{Stamina, StaminaState};

        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
                Stamina {
                    current: 100.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    state: StaminaState::Idle,
                    last_drain_tick: None,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<RaiseShieldIntent>>()
            .send(RaiseShieldIntent { player: entity });
        app.update();

        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        assert_eq!(
            stamina.state,
            StaminaState::ShieldBlocking,
            "举盾后 StaminaState 应切换到 ShieldBlocking（触发 drain），\
             actual: {:?}",
            stamina.state
        );
    }

    // ── lower_shield 恢复 StaminaState 到 Idle ──────────────────────────────
    #[test]
    fn lower_shield_restores_stamina_state_to_idle() {
        use crate::combat::components::{Stamina, StaminaState};

        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
                Stamina {
                    current: 100.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    state: StaminaState::ShieldBlocking,
                    last_drain_tick: None,
                },
            ))
            .id();

        // 直接 lower（不需要先 raise 在这里）
        app.world_mut()
            .resource_mut::<Events<LowerShieldIntent>>()
            .send(LowerShieldIntent { player: entity });
        app.update();

        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        assert_eq!(
            stamina.state,
            StaminaState::Idle,
            "放盾后 StaminaState 应从 ShieldBlocking 恢复到 Idle，\
             actual: {:?}",
            stamina.state
        );
    }

    // ── lower_shield 不强制覆盖 Exhausted 状态 ──────────────────────────────
    #[test]
    fn lower_shield_preserves_exhausted_state() {
        use crate::combat::components::{Stamina, StaminaState};

        let mut app = make_app();
        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                make_inventory_with_off_hand("wooden_shield"),
                Stamina {
                    current: 0.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    state: StaminaState::Exhausted,
                    last_drain_tick: None,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<Events<LowerShieldIntent>>()
            .send(LowerShieldIntent { player: entity });
        app.update();

        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        assert_eq!(
            stamina.state,
            StaminaState::Exhausted,
            "lower_shield 不应将 Exhausted 状态强制改为 Idle（体力仍为 0）；\
             actual: {:?}",
            stamina.state
        );
    }

    // ── force_lower 系统编译注册断言（签名守护）────────────────────────────
    // 验证 force_lower_shield_on_stamina_exhausted 函数可作为 Bevy 系统注册（类型守护）。
    #[test]
    fn force_lower_shield_on_stamina_exhausted_compiles_as_system() {
        use crate::combat::events::ApplyStatusEffectIntent;
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(crate::combat::CombatClock::default());
        app.add_systems(Update, force_lower_shield_on_stamina_exhausted);
        let _ = app; // 只需构建不 panic
    }

    // ── SHIELD_DRAIN_PER_SEC 常量值锁定 ─────────────────────────────────────
    #[test]
    fn shield_drain_per_sec_constant_is_3_0() {
        use crate::combat::lifecycle::SHIELD_DRAIN_PER_SEC;
        assert!(
            (SHIELD_DRAIN_PER_SEC - 3.0).abs() < f32::EPSILON,
            "SHIELD_DRAIN_PER_SEC 应为 3.0（plan-shield-block-v1 P2 spec），\
             实际值 {SHIELD_DRAIN_PER_SEC}（改动此值必须同步更新 plan spec）"
        );
    }

    // ── SHIELD_FOV_DOT 常量值锁定 ────────────────────────────────────────────
    #[test]
    fn shield_fov_dot_constant_is_minus_half() {
        assert!(
            (SHIELD_FOV_DOT - (-0.5)).abs() < f64::EPSILON,
            "SHIELD_FOV_DOT 应为 -0.5（对应 ±120° 格挡弧，凡人盾无境界加成），\
             实际值 {SHIELD_FOV_DOT}"
        );
    }

    // ── CombatDefenseKindV1::ShieldBlock serde roundtrip ────────────────────
    // 锁住 schema 序列化契约：ShieldBlock → "shield_block" (snake_case)。
    #[test]
    fn combat_defense_kind_shield_block_serde_roundtrip() {
        use crate::schema::combat_event::CombatDefenseKindV1;
        let variant = CombatDefenseKindV1::ShieldBlock;
        let json =
            serde_json::to_string(&variant).expect("CombatDefenseKindV1::ShieldBlock 应可序列化");
        assert_eq!(
            json, "\"shield_block\"",
            "CombatDefenseKindV1::ShieldBlock 应序列化为 \"shield_block\"（snake_case），\
             实际 {json}"
        );
        let back: CombatDefenseKindV1 =
            serde_json::from_str(&json).expect("\"shield_block\" 应可反序列化为 ShieldBlock");
        assert_eq!(
            back,
            CombatDefenseKindV1::ShieldBlock,
            "\"shield_block\" 反序列化应还原为 ShieldBlock 变体，实际 {back:?}"
        );
    }

    // ── map_defense_kind 全变体覆盖（编译 + 语义）──────────────────────────
    // 锁住 combat_bridge.rs map_defense_kind 映射的三个变体全部正确。
    #[test]
    fn map_defense_kind_all_variants_map_correctly() {
        use crate::combat::events::DefenseKind;
        use crate::network::combat_bridge::map_defense_kind_pub;
        use crate::schema::combat_event::CombatDefenseKindV1;

        assert_eq!(
            map_defense_kind_pub(DefenseKind::JieMai),
            CombatDefenseKindV1::JieMai,
            "JieMai 应映射到 CombatDefenseKindV1::JieMai"
        );
        assert_eq!(
            map_defense_kind_pub(DefenseKind::SwordParry),
            CombatDefenseKindV1::SwordParry,
            "SwordParry 应映射到 CombatDefenseKindV1::SwordParry"
        );
        assert_eq!(
            map_defense_kind_pub(DefenseKind::ShieldBlock),
            CombatDefenseKindV1::ShieldBlock,
            "ShieldBlock 应映射到 CombatDefenseKindV1::ShieldBlock"
        );
    }
}
