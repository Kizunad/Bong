//! plan-supply-coffin-v1 P2.1/P2.2 — 开棺交互 + 碎裂视听。
//!
//! 玩家右键物资棺 → 距离校验（≤ 4 格）→ 按 grade roll loot →
//! `add_item_to_player_inventory` → 发 `SupplyCoffinOpened` 事件 → emit 碎裂
//! 音效 + VFX → despawn 棺 entity → 入冷却队列。
//!
//! 开棺即碎，没有搜刮进度条（plan §0 设计轴心 1：这不是 TSY 高风险搜刮）。

use bevy_ecs::event::EventReader;
use valence::prelude::{
    bevy_ecs, Commands, Despawned, Entity, EntityInteraction, Event, EventWriter, Hand,
    InteractEntityEvent, Position, Query, Res, ResMut,
};

use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest, AUDIO_AREA_RADIUS};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;

use super::{current_wall_clock_secs, loot::roll_loot, SupplyCoffinGrade, SupplyCoffinRegistry};

/// 开棺距离上限（plan §P2.1：≤ 4 格）+ Valence 抖动容差。
const OPEN_RANGE_BLOCKS: f64 = 4.0;
const OPEN_RANGE_TOLERANCE: f64 = 0.5;

/// 玩家成功开启一具物资棺时发的事件。
///
/// 下游 narration / agent / stats / achievements 可消费此事件——本 plan 范围只
/// 定义类型与 emit 点，不实装订阅者（与 plan-tsy-loot-v1 的 `RelicExtracted`
/// 同模式）。字段 `#[allow(dead_code)]` 直到订阅者接入。
#[derive(Debug, Clone, Event)]
pub struct SupplyCoffinOpened {
    /// 开棺玩家 Entity。
    #[allow(dead_code)]
    pub player: Entity,
    /// 物资棺档次。
    #[allow(dead_code)]
    pub grade: SupplyCoffinGrade,
    /// 棺的世界坐标。
    #[allow(dead_code)]
    pub pos: valence::prelude::DVec3,
    /// 实际授予的物品（template_id + 数量）。如果背包装不下，可能少于 rolled。
    #[allow(dead_code)]
    pub granted: Vec<(String, u8)>,
}

/// Update 系统：处理 InteractEntityEvent，识别物资棺 entity 并触发开棺流程。
#[allow(clippy::too_many_arguments)]
pub fn handle_supply_coffin_interact(
    mut commands: Commands,
    mut interactions: EventReader<InteractEntityEvent>,
    mut registry: ResMut<SupplyCoffinRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
    mut vfx: EventWriter<VfxEventRequest>,
    mut opened: EventWriter<SupplyCoffinOpened>,
    mut players: Query<(&mut PlayerInventory, &Position)>,
) {
    for ev in interactions.read() {
        // 只接受右手 Interact / InteractAt（与 inventory remains 拾取一致）。
        match ev.interact {
            EntityInteraction::Interact(Hand::Main)
            | EntityInteraction::InteractAt {
                hand: Hand::Main, ..
            } => {}
            _ => continue,
        }

        // 这个 entity 是物资棺吗？
        let Some(active) = registry.active.get(&ev.entity).cloned() else {
            continue;
        };

        // 玩家背包 + 位置。
        let Ok((mut inventory, player_pos)) = players.get_mut(ev.client) else {
            continue;
        };

        // 距离校验。
        let dist = active.pos.distance(player_pos.get());
        if dist > OPEN_RANGE_BLOCKS + OPEN_RANGE_TOLERANCE {
            tracing::debug!(
                "[bong][supply_coffin] interact rejected (out of range): grade={:?} dist={:.2}",
                active.grade,
                dist
            );
            continue;
        }

        // Roll loot —— seed 从 registry 的 splitmix64 advance 取，保证 deterministic
        // 且玩家间互不影响。
        let seed = registry.next_rand_u64();
        let rolled = roll_loot(active.grade, seed);

        // 实际授予：背包装不下的物品 drop（warning 日志），不阻断流程。
        let mut granted: Vec<(String, u8)> = Vec::with_capacity(rolled.len());
        for (template_id, count) in rolled {
            match add_item_to_player_inventory(
                &mut inventory,
                &item_registry,
                &mut allocator,
                template_id.as_str(),
                u32::from(count),
            ) {
                Ok(_) => granted.push((template_id, count)),
                Err(reason) => {
                    tracing::warn!(
                        "[bong][supply_coffin] grant `{} x{}` rejected for player {:?}: {reason}",
                        template_id,
                        count,
                        ev.client
                    );
                }
            }
        }

        // 视听 —— 碎裂粒子 + 按 grade 区分的破棺音效。
        let break_recipe = match active.grade {
            SupplyCoffinGrade::Common => "supply_coffin_break_common",
            SupplyCoffinGrade::Rare => "supply_coffin_break_rare",
            SupplyCoffinGrade::Precious => "supply_coffin_break_precious",
        };
        audio.send(PlaySoundRecipeRequest {
            recipe_id: break_recipe.to_string(),
            instance_id: 0,
            pos: Some([
                active.pos.x as i32,
                active.pos.y as i32,
                active.pos.z as i32,
            ]),
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin: active.pos,
                radius: AUDIO_AREA_RADIUS,
            },
        });

        let color = match active.grade {
            SupplyCoffinGrade::Common => "#8B6914",
            SupplyCoffinGrade::Rare => "#2A1506",
            SupplyCoffinGrade::Precious => "#C4A35A",
        };
        vfx.send(VfxEventRequest::new(
            active.pos,
            VfxEventPayloadV1::SpawnParticle {
                event_id: "bong:supply_coffin_break".to_string(),
                origin: [active.pos.x, active.pos.y, active.pos.z],
                direction: None,
                color: Some(color.to_string()),
                strength: None,
                count: Some(12),
                duration_ticks: Some(15),
            },
        ));

        // emit 上层事件（narration / stats hook）。
        opened.send(SupplyCoffinOpened {
            player: ev.client,
            grade: active.grade,
            pos: active.pos,
            granted,
        });

        // 棺碎：despawn entity + 入冷却队列。
        commands.entity(ev.entity).insert(Despawned);
        registry.remove_active(ev.entity);
        registry.enqueue_cooldown(active.grade, current_wall_clock_secs());

        tracing::debug!(
            "[bong][supply_coffin] opened {:?} at ({:.1},{:.1},{:.1}) by player {:?}",
            active.grade,
            active.pos.x,
            active.pos.y,
            active.pos.z,
            ev.client
        );
    }
}
