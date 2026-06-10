//! plan-shield-block-v1 P1 — 盾牌格挡持续状态与输入层。
//!
//! # 职责
//! - `ShieldBlock` ECS component：玩家当前正在持盾格挡的标记（独立于 Weapon 路径）。
//! - `raise_shield_handler` / `lower_shield_handler`：消费 server 端 RaiseShield/LowerShield
//!   事件（由 client_request_handler 投递），校验 off_hand 实装盾后操作 StatusEffects。
//! - `cleanup_shield_on_death`：死亡时强制清理防残留（接 DeathEvent）。
//! - `cleanup_shield_on_disconnect`：断线时强制清理（在 despawn_disconnected_clients 前运行）。
//!
//! # 接入注意
//! - StatusEffectKind::ShieldBlocking 的 `magnitude` 存储 block_ratio 占位（P2 写入真实值）。
//! - 动画触发（`bong:shield_raise`）通过 `vfx_animation_trigger::emit_shield_raise_for_entity`。
//! - P2 会在此模块追加 `shield_fov_check` 和 stamina drain 分支，不新增文件。

use valence::prelude::{
    bevy_ecs, Commands, Component, Entity, Event, EventReader, EventWriter, Position, Query, Res,
    UniqueId,
};

use crate::combat::components::{ActiveStatusEffect, StatusEffects};
use crate::combat::events::{DeathEvent, StatusEffectKind};
use crate::combat::status::{has_active_status, remove_status_effect, upsert_status_effect};
use crate::combat::CombatClock;
use crate::inventory::{PlayerInventory, EQUIP_SLOT_OFF_HAND};
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

/// plan-shield-block-v1 P1 — block_ratio 占位：P1 暂存 0.5（P2 从 ShieldSpec 读取真实值）。
const SHIELD_BLOCKING_MAGNITUDE_P1_PLACEHOLDER: f32 = 0.5;

/// 检查 off_hand 槽物品是否是已知盾牌模板。
/// 与 client InventoryEquipRules.SHIELD_TEMPLATE_IDS 保持同步。
pub fn is_shield_template_id(template_id: &str) -> bool {
    matches!(template_id, "wooden_shield" | "bone_shield")
}

/// 处理 RaiseShieldIntent：校验 off_hand 盾 → 插入 ShieldBlock component + ShieldBlocking status。
pub fn raise_shield_handler(
    mut intents: EventReader<RaiseShieldIntent>,
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut status_q: Query<(&mut StatusEffects, Option<&ShieldBlock>)>,
    inventory_q: Query<&PlayerInventory>,
    players_q: Query<(&Position, &UniqueId)>,
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

        // 2. 校验并操作 StatusEffects + component
        let Ok((mut status_effects, existing_shield_block)) = status_q.get_mut(entity) else {
            continue;
        };

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
                    magnitude: SHIELD_BLOCKING_MAGNITUDE_P1_PLACEHOLDER,
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
                magnitude: SHIELD_BLOCKING_MAGNITUDE_P1_PLACEHOLDER,
                remaining_ticks: SHIELD_BLOCKING_DURATION_TICKS,
                source_pill: None,
            },
        );
        // 插入 ShieldBlock component
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(ShieldBlock {
                template_id: template_id.clone(),
            });
        }

        tracing::debug!(
            "[bong][shield] RaiseShield entity={entity:?}: shield raised (template={template_id}) tick={}",
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

/// 处理 LowerShieldIntent：移除 ShieldBlocking 状态 + ShieldBlock component。
pub fn lower_shield_handler(
    mut intents: EventReader<LowerShieldIntent>,
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut status_q: Query<&mut StatusEffects>,
) {
    for intent in intents.read() {
        let entity = intent.player;
        if let Ok(mut status_effects) = status_q.get_mut(entity) {
            remove_status_effect(&mut status_effects, StatusEffectKind::ShieldBlocking);
        }
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<ShieldBlock>();
        }
        tracing::debug!(
            "[bong][shield] LowerShield entity={entity:?}: shield lowered tick={}",
            clock.tick
        );
    }
}

/// plan-shield-block-v1 P1 — 玩家死亡时强制清理 ShieldBlocking 状态，防残留。
pub fn cleanup_shield_on_death(
    mut death_events: EventReader<DeathEvent>,
    mut commands: Commands,
    mut status_q: Query<&mut StatusEffects>,
) {
    for ev in death_events.read() {
        let entity = ev.target;
        if let Ok(mut status_effects) = status_q.get_mut(entity) {
            if has_active_status(&status_effects, StatusEffectKind::ShieldBlocking) {
                remove_status_effect(&mut status_effects, StatusEffectKind::ShieldBlocking);
                tracing::debug!(
                    "[bong][shield] cleanup_on_death: removed ShieldBlocking for {entity:?}"
                );
            }
        }
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<ShieldBlock>();
        }
    }
}

/// plan-shield-block-v1 P1 — 断线时强制清理盾牌格挡状态。
/// 由 `despawn_disconnected_clients` 之前的 system 调用。
/// 使用 RemovedComponents<valence::prelude::Client> 探测断线实体。
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
    use crate::combat::events::StatusEffectKind;
    use crate::combat::status::has_active_status;
    use crate::inventory::{ItemInstance, PlayerInventory, EQUIP_SLOT_OFF_HAND};
    use valence::prelude::{App, Events, Update};

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
}
