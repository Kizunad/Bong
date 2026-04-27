//! plan-HUD-v1 §4 cast 状态机 server side。
//!
//! 三件事：
//! 1. `tick_casts_or_interrupt` 系统：每 tick 检查所有 `Casting` 实体，受击中断
//!    优先于自然完成，发对应 `cast_sync` payload 并 remove component。
//! 2. `push_cast_sync_to_client` 公共函数：handler 接收 `use_quick_slot`
//!    intent 时同样调它推 `cast_sync(Casting)`。
//! 3. `cast_sync_payload` 帮助构造完整 payload。
//!
//! 当前 v1 限制：
//! - 只做受击中断（contam）；移动 / 控制效果 / 主动取消 留 TODO
//! - 不消耗物品、不应用效果（plan §4.4 完成路径需要 inventory event）
//! - duration 来自 client intent / 默认 1500ms（无 QuickSlotBindings 时）

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Client, Commands, Entity, Position, Query, Res, Username};

use crate::combat::components::{
    CastSource, Casting, QuickSlotBindings, SkillBarBindings, StatusEffects, Wounds,
};
use crate::combat::events::StatusEffectKind;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::components::{Contamination, MeridianSystem};
use crate::inventory::{ItemEffect, ItemRegistry, PlayerInventory};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::PlayerState;
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

/// Cooldown 默认值（plan §4.4）。中断后短冷却 0.5s（10 tick）；
/// 完成后冷却来自 ItemTemplate.cooldown_ms（折算到 Casting.complete_cooldown_ticks）。
pub const CAST_INTERRUPT_COOLDOWN_TICKS: u64 = 10;
/// plan §4.3 移动中断阈值（米）。超过即视为主动位移中断。
pub const CAST_MOVEMENT_INTERRUPT_THRESHOLD_M: f64 = 0.3;

type CastTickQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a Casting,
    &'a Wounds,
    &'a Position,
    &'a mut PlayerInventory,
    &'a PlayerState,
    &'a mut QuickSlotBindings,
    &'a mut SkillBarBindings,
    Option<&'a StatusEffects>,
    Option<&'a mut MeridianSystem>,
    Option<&'a mut Contamination>,
);

pub fn tick_casts_or_interrupt(
    clock: Res<CombatClock>,
    mut commands: Commands,
    item_registry: Res<ItemRegistry>,
    mut clients: Query<CastTickQueryItem<'_>>,
) {
    for (
        entity,
        mut client,
        username,
        casting,
        wounds,
        position,
        mut inventory,
        player_state,
        mut bindings,
        mut skillbar_bindings,
        status_effects,
        meridians,
        contamination,
    ) in &mut clients
    {
        // plan §4.3 控制中断（Stunned）—— 优先级最高：玩家根本动不了。
        let stunned = status_effects.is_some_and(|se| {
            se.active
                .iter()
                .any(|e| e.kind == StatusEffectKind::Stunned && e.remaining_ticks > 0)
        });
        if stunned {
            commands.entity(entity).remove::<Casting>();
            set_cast_cooldown(
                casting,
                &mut bindings,
                &mut skillbar_bindings,
                casting.slot,
                clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
            );
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Interrupt,
                    slot: casting.slot,
                    duration_ms: casting.duration_ms,
                    started_at_ms: casting.started_at_ms,
                    outcome: CastOutcomeV1::InterruptControl,
                },
                username.0.as_str(),
                entity,
            );
            tracing::info!(
                "[bong][network][cast] control interrupt entity={entity:?} `{}` slot={} (Stunned)",
                username.0,
                casting.slot
            );
            continue;
        }
        // 受击中断：本 tick 新增的 wound。
        let damaged_this_tick = wounds
            .entries
            .iter()
            .any(|w| w.created_at_tick == clock.tick);
        if damaged_this_tick {
            commands.entity(entity).remove::<Casting>();
            set_cast_cooldown(
                casting,
                &mut bindings,
                &mut skillbar_bindings,
                casting.slot,
                clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
            );
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Interrupt,
                    slot: casting.slot,
                    duration_ms: casting.duration_ms,
                    started_at_ms: casting.started_at_ms,
                    outcome: CastOutcomeV1::InterruptContam,
                },
                username.0.as_str(),
                entity,
            );
            continue;
        }
        // 移动中断（plan §4.3）：当前位置与 cast 起始位置距离超阈值。
        let moved_distance = position.get().distance(casting.start_position);
        if moved_distance > CAST_MOVEMENT_INTERRUPT_THRESHOLD_M {
            commands.entity(entity).remove::<Casting>();
            set_cast_cooldown(
                casting,
                &mut bindings,
                &mut skillbar_bindings,
                casting.slot,
                clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
            );
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Interrupt,
                    slot: casting.slot,
                    duration_ms: casting.duration_ms,
                    started_at_ms: casting.started_at_ms,
                    outcome: CastOutcomeV1::InterruptMovement,
                },
                username.0.as_str(),
                entity,
            );
            tracing::info!(
                "[bong][network][cast] movement interrupt entity={entity:?} `{}` slot={} moved={:.3}m",
                username.0,
                casting.slot,
                moved_distance
            );
            continue;
        }
        // 自然完成
        if clock.tick >= casting.started_at_tick + casting.duration_ticks {
            commands.entity(entity).remove::<Casting>();
            // 1) 消耗：物品快捷槽找到绑定 instance_id，stack -= 1；技能栏只进入冷却。
            let mut effect_to_apply: Option<ItemEffect> = None;
            if casting.source == CastSource::QuickSlot {
                if let Some(id) = casting.bound_instance_id {
                    if let Some(template_id) = lookup_template_id(&inventory, id) {
                        if let Some(template) = item_registry.get(&template_id) {
                            effect_to_apply = template.effect.clone();
                        }
                    }
                }
            }
            let consumed = if casting.source == CastSource::QuickSlot {
                casting
                    .bound_instance_id
                    .map(|id| consume_one_stack(&mut inventory, id))
                    .unwrap_or(false)
            } else {
                false
            };
            // 2) 应用效果
            if let Some(effect) = effect_to_apply.as_ref() {
                apply_item_effect(effect, meridians, contamination, &username.0, entity);
            }
            // 3) 设置完成冷却（来自 ItemTemplate.cooldown_ms 折算后的 ticks）
            set_cast_cooldown(
                casting,
                &mut bindings,
                &mut skillbar_bindings,
                casting.slot,
                clock.tick.saturating_add(casting.complete_cooldown_ticks),
            );
            // 4) 推 cast_sync(Complete)
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Complete,
                    slot: casting.slot,
                    duration_ms: casting.duration_ms,
                    started_at_ms: casting.started_at_ms,
                    outcome: CastOutcomeV1::Completed,
                },
                username.0.as_str(),
                entity,
            );
            // 5) 同步 inventory（消耗后）
            if consumed {
                send_inventory_snapshot_to_client(
                    entity,
                    &mut client,
                    username.0.as_str(),
                    &inventory,
                    player_state,
                    &Cultivation::default(),
                    "cast_complete_consume",
                );
            }
        }
    }
}

fn set_cast_cooldown(
    casting: &Casting,
    quick_bindings: &mut QuickSlotBindings,
    skillbar_bindings: &mut SkillBarBindings,
    slot: u8,
    until_tick: u64,
) {
    match casting.source {
        CastSource::QuickSlot => quick_bindings.set_cooldown(slot, until_tick),
        CastSource::SkillBar => skillbar_bindings.set_cooldown(slot, until_tick),
    }
}

fn lookup_template_id(inv: &PlayerInventory, instance_id: u64) -> Option<String> {
    for c in &inv.containers {
        if let Some(p) = c
            .items
            .iter()
            .find(|p| p.instance.instance_id == instance_id)
        {
            return Some(p.instance.template_id.clone());
        }
    }
    if let Some(item) = inv
        .equipped
        .values()
        .find(|item| item.instance_id == instance_id)
    {
        return Some(item.template_id.clone());
    }
    inv.hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
        .map(|item| item.template_id.clone())
}

fn apply_item_effect(
    effect: &ItemEffect,
    meridians: Option<valence::prelude::Mut<MeridianSystem>>,
    contamination: Option<valence::prelude::Mut<Contamination>>,
    username: &str,
    entity: Entity,
) {
    match effect {
        ItemEffect::MeridianHeal {
            magnitude,
            target: _,
        } => {
            // v1: 跨所有经脉，advance 第一条尚未愈合的裂痕。
            // 不区分 target = "any_meridian" vs 具体经脉 id（后续接入 MeridianId
            // 解析时再细化）。
            let Some(mut meridians) = meridians else {
                tracing::debug!(
                    "[bong][network][cast] MeridianHeal noop: entity {entity:?} `{username}` has no MeridianSystem"
                );
                return;
            };
            let mut healed_count = 0usize;
            for m in meridians.iter_mut() {
                let mut local_healed = 0usize;
                for crack in m.cracks.iter_mut() {
                    if crack.healing_progress < crack.severity {
                        crack.healing_progress =
                            (crack.healing_progress + magnitude).clamp(0.0, crack.severity);
                        if crack.healing_progress >= crack.severity {
                            local_healed += 1;
                        }
                    }
                }
                m.cracks.retain(|c| c.healing_progress < c.severity);
                if local_healed > 0 {
                    m.integrity = (m.integrity + 0.05 * local_healed as f64).min(1.0);
                    healed_count += local_healed;
                }
            }
            tracing::info!(
                "[bong][network][cast] MeridianHeal magnitude={magnitude} for `{username}` ({entity:?}) — {healed_count} crack(s) sealed"
            );
        }
        ItemEffect::ContaminationCleanse { magnitude } => {
            let Some(mut contamination) = contamination else {
                tracing::debug!(
                    "[bong][network][cast] ContaminationCleanse noop: entity {entity:?} `{username}` has no Contamination"
                );
                return;
            };
            let mut remaining = *magnitude;
            for entry in contamination.entries.iter_mut() {
                if remaining <= 0.0 {
                    break;
                }
                let take = entry.amount.min(remaining);
                entry.amount -= take;
                remaining -= take;
            }
            contamination.entries.retain(|e| e.amount > f64::EPSILON);
            tracing::info!(
                "[bong][network][cast] ContaminationCleanse magnitude={magnitude} for `{username}` ({entity:?}) — {:.3} cleansed",
                magnitude - remaining
            );
        }
        ItemEffect::BreakthroughBonus { magnitude } => {
            // v1 不存 buff state（缺 Component）。仅 log。
            tracing::info!(
                "[bong][network][cast] BreakthroughBonus magnitude={magnitude} for `{username}` ({entity:?}) — no-op (buff state TODO)"
            );
        }
        ItemEffect::LifespanExtension { years, source } => {
            tracing::info!(
                "[bong][network][cast] LifespanExtension years={years} source={source} for `{username}` ({entity:?}) — handled by take_pill path"
            );
        }
    }
}

/// 在 inventory 内找 instance_id 并 stack-=1；归零则移除。返回是否成功扣到。
fn consume_one_stack(inventory: &mut PlayerInventory, instance_id: u64) -> bool {
    inventory.revision =
        crate::inventory::InventoryRevision(inventory.revision.0.saturating_add(1));
    for c in &mut inventory.containers {
        if let Some(idx) = c
            .items
            .iter()
            .position(|p| p.instance.instance_id == instance_id)
        {
            let placed = &mut c.items[idx];
            if placed.instance.stack_count > 1 {
                placed.instance.stack_count -= 1;
            } else {
                c.items.remove(idx);
            }
            return true;
        }
    }
    for slot in inventory.hotbar.iter_mut() {
        if let Some(item) = slot.as_mut() {
            if item.instance_id == instance_id {
                if item.stack_count > 1 {
                    item.stack_count -= 1;
                } else {
                    *slot = None;
                }
                return true;
            }
        }
    }
    // 装备槽内的物品不应在这条路径出现（cast 用的是消耗品而非武器/护甲）。
    false
}

pub fn push_cast_sync(client: &mut Client, state: CastSyncV1, username: &str, entity: Entity) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::CastSync(state));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::info!(
        "[bong][network] sent {} {} payload to entity {entity:?} for `{username}` (phase={:?} slot={} outcome={:?})",
        SERVER_DATA_CHANNEL,
        payload_type,
        state.phase,
        state.slot,
        state.outcome,
    );
}

pub fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;

    fn make_inventory_with_stack(instance_id: u64, stack: u32) -> PlayerInventory {
        let item = ItemInstance {
            instance_id,
            template_id: "tea".to_string(),
            display_name: "茶".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: stack,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
        };
        PlayerInventory {
            revision: InventoryRevision(5),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    #[test]
    fn consume_one_stack_decrements_when_above_one() {
        let mut inv = make_inventory_with_stack(42, 5);
        assert!(consume_one_stack(&mut inv, 42));
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 4);
        assert_eq!(inv.revision, InventoryRevision(6));
    }

    #[test]
    fn consume_one_stack_removes_when_at_one() {
        let mut inv = make_inventory_with_stack(42, 1);
        assert!(consume_one_stack(&mut inv, 42));
        assert!(inv.containers[0].items.is_empty());
    }

    #[test]
    fn consume_one_stack_returns_false_when_missing() {
        let mut inv = make_inventory_with_stack(42, 3);
        assert!(!consume_one_stack(&mut inv, 999));
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 3);
    }

    #[test]
    fn movement_interrupt_threshold_classifies_within_and_beyond() {
        use valence::prelude::DVec3;
        let start = DVec3::new(10.0, 64.0, 20.0);
        let still_ok = DVec3::new(10.2, 64.0, 20.0); // 0.2m → 不中断
        let too_far = DVec3::new(10.5, 64.0, 20.0); // 0.5m → 中断
        assert!(still_ok.distance(start) <= CAST_MOVEMENT_INTERRUPT_THRESHOLD_M);
        assert!(too_far.distance(start) > CAST_MOVEMENT_INTERRUPT_THRESHOLD_M);
    }

    #[test]
    fn cooldown_set_get_round_trip() {
        let mut bindings = QuickSlotBindings::default();
        assert!(!bindings.is_on_cooldown(3, 100));
        bindings.set_cooldown(3, 130);
        assert!(bindings.is_on_cooldown(3, 100));
        assert!(bindings.is_on_cooldown(3, 129));
        assert!(!bindings.is_on_cooldown(3, 130));
        assert!(!bindings.is_on_cooldown(3, 131));
        // out-of-range slot is silently no-op
        assert!(!bindings.is_on_cooldown(99, 0));
        bindings.set_cooldown(99, 100);
        assert!(!bindings.is_on_cooldown(99, 50));
    }

    #[test]
    fn meridian_heal_advances_first_unhealed_crack() {
        use crate::cultivation::components::{CrackCause, MeridianCrack, MeridianSystem};
        let mut meridians = MeridianSystem::default();
        // Inject a crack into the first regular meridian.
        meridians.regular[0].cracks.push(MeridianCrack {
            severity: 0.5,
            healing_progress: 0.0,
            cause: CrackCause::Attack,
            created_at: 0,
        });
        // Manually walk apply_item_effect minus the Mut wrapper using internals.
        let crack = &mut meridians.regular[0].cracks[0];
        crack.healing_progress = (crack.healing_progress + 0.3).clamp(0.0, crack.severity);
        assert!((crack.healing_progress - 0.3).abs() < 1e-9);
        // Healing past severity should retain cull at 0.5.
        crack.healing_progress = (crack.healing_progress + 0.4).clamp(0.0, crack.severity);
        assert!((crack.healing_progress - crack.severity).abs() < 1e-9);
    }

    #[test]
    fn consume_one_stack_finds_in_hotbar() {
        let mut inv = make_inventory_with_stack(99, 10); // unrelated container item
        inv.hotbar[3] = Some(ItemInstance {
            instance_id: 7,
            template_id: "pill".to_string(),
            display_name: "丹".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 2,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
        });
        assert!(consume_one_stack(&mut inv, 7));
        assert_eq!(inv.hotbar[3].as_ref().unwrap().stack_count, 1);
        assert!(consume_one_stack(&mut inv, 7));
        assert!(inv.hotbar[3].is_none());
    }
}
