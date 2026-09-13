//! plan-shield-block-v1 P1 — 盾牌格挡持续状态与输入层；P4 — 熟练度缩放 + 经脉 + narration。
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
    ResMut, UniqueId,
};

use crate::combat::components::{
    ActiveStatusEffect, ShieldDrainOverride, Stamina, StaminaState, StatusEffects,
};
use crate::combat::events::{ApplyStatusEffectIntent, DeathEvent, StatusEffectKind};
use crate::combat::status::{has_active_status, remove_status_effect, upsert_status_effect};
use crate::combat::CombatClock;
use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
use crate::cultivation::meridian::severed::SkillMeridianDependencies;
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

// ── plan-shield-block-v1 P4 ── 熟练度 / 经脉 / narration ──────────────────────

/// plan-shield-block-v1 P4 — 盾牌格挡技能 technique_id。
/// 前缀 `shield_block`，避开 `woliu.vortex_shield` / `woliu.*` 命名空间（§8.1#7）。
pub const SHIELD_BLOCK_TECHNIQUE_ID: &str = "shield_block";

/// plan-shield-block-v1 P4 — 格挡成功时熟练度增益（对称 sword_proficiency_gain 递减曲线）。
/// 每次成功格挡 +gain；熟练度越高增益越小（激励早期进阶）。
pub fn shield_block_proficiency_gain(current: f32) -> f32 {
    let current = current.clamp(0.0, 1.0);
    if current < 0.40 {
        0.012
    } else if current < 0.70 {
        0.006
    } else if current < 0.90 {
        0.003
    } else {
        0.001
    }
}

/// plan-shield-block-v1 P4 — 盾牌格挡收益 profile（仿 sword_profile 线性插值）。
///
/// - `block_ratio`：按 proficiency 线性上浮（木盾 0.5→0.6，骨盾 0.65→0.72）。
///   硬上限 < 0.95（§8.1#6，凡人盾不压过修士防御），各盾各自上限。
/// - `drain_per_s`：3.0→2.0 下限，随 proficiency 线性下降（不归零，防满熟练无限举盾）。
///
/// `template_id` 须是 `"wooden_shield"` 或 `"bone_shield"`，其余 fallback 木盾参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShieldBlockProfile {
    /// 正面命中伤害削减比例（clamp 0..0.95）。
    pub block_ratio: f32,
    /// 持续举盾每秒体力 drain（>= 2.0 下限）。
    pub drain_per_s: f32,
}

pub fn shield_block_profile(template_id: &str, proficiency: f32) -> ShieldBlockProfile {
    let prof = proficiency.clamp(0.0, 1.0);
    // block_ratio 线性插值：wooden 0.5→0.6，bone 0.65→0.72
    let (ratio_base, ratio_cap) = if template_id == "bone_shield" {
        (0.65_f32, 0.72_f32)
    } else {
        // wooden_shield + 任意未知盾 fallback
        (0.50_f32, 0.60_f32)
    };
    let block_ratio = (ratio_base + (ratio_cap - ratio_base) * prof).clamp(0.0, 0.95);
    // drain_per_s 线性插值：3.0→2.0（熟练度越高耗体力越少）
    let drain_per_s = (3.0_f32 - 1.0 * prof).max(2.0);
    ShieldBlockProfile {
        block_ratio,
        drain_per_s,
    }
}

/// plan-shield-block-v1 P4 — 确保 KnownTechniques 中有 `shield_block` 条目（auto-insert）。
/// 首次举盾时补入盾挡功法。
pub fn ensure_shield_block_entry(known: &mut KnownTechniques) -> &mut KnownTechnique {
    if let Some(index) = known
        .entries
        .iter()
        .position(|entry| entry.id == SHIELD_BLOCK_TECHNIQUE_ID)
    {
        return &mut known.entries[index];
    }
    known.entries.push(KnownTechnique {
        id: SHIELD_BLOCK_TECHNIQUE_ID.to_string(),
        proficiency: 0.0,
        active: true,
    });
    known
        .entries
        .last_mut()
        .expect("shield_block entry was just inserted")
}

/// plan-shield-block-v1 P4 — 格挡成功时写入熟练度增益。
///
/// 签名镜像 `sword_basics::record_sword_parry_success(world, defender)`。
/// 由 `resolve.rs` 的 `shield_block_success` 分支通过 `commands.add` 延迟调用。
pub fn record_shield_block_success(world: &mut bevy_ecs::world::World, defender: Entity) {
    let Some(mut known) = world.get_mut::<KnownTechniques>(defender) else {
        return;
    };
    let entry = ensure_shield_block_entry(&mut known);
    let gain = shield_block_proficiency_gain(entry.proficiency);
    entry.proficiency = (entry.proficiency + gain).clamp(0.0, 1.0);
}

/// plan-shield-block-v1 P4 — 声明 shield_block 的经脉依赖（空 vec = 不依赖任何经脉）。
/// 凡人盾是纯物理防御，无真元经脉要求（§8.1 #4）。
/// 注册点：`cultivation/mod.rs` 的 `SkillMeridianDependencies` 组装处。
pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    dependencies.declare(SHIELD_BLOCK_TECHNIQUE_ID, Vec::new());
}

// ── narration 阈值常数 ────────────────────────────────────────────────────────

/// plan-shield-block-v1 P4 — 接近破盾时耐久阈值（ratio 低于此值发出骨盾裂纹 narration）。
/// 设为 0.25：剩余 25% 耐久时给玩家预警（约剩 1/4 格挡次数）。
pub const SHIELD_NEAR_BREAK_DURABILITY_THRESHOLD: f64 = 0.25;

/// plan-shield-block-v1 P4 — 体力低预警阈值（ratio 低于此值发出臂膀酸痛 narration）。
/// 设为 0.25：剩余 25% 体力时预警（与 SHIELD_NEAR_BREAK_DURABILITY_THRESHOLD 对称）。
pub const SHIELD_LOW_STAMINA_NARRATION_THRESHOLD: f32 = 0.25;

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
/// P4：插入 ShieldDrainOverride（经 shield_block_profile 按熟练度缩放的 drain_per_s）。
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
    known_q: Query<Option<&KnownTechniques>>,
) {
    for intent in intents.read() {
        let entity = intent.player;

        // 1. 读取 off_hand 槽物品 template_id
        let template_id = match inventory_q.get(entity) {
            Ok(inv) => {
                match inv
                    .equipped
                    .get(EQUIP_SLOT_OFF_HAND)
                    .and_then(|s| s.held.as_ref())
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

        // P4 — 读取当前 shield_block proficiency，计算 drain_per_s 覆写值。
        let shield_proficiency = known_q
            .get(entity)
            .ok()
            .flatten()
            .and_then(|known| {
                known
                    .entries
                    .iter()
                    .find(|e| e.id == SHIELD_BLOCK_TECHNIQUE_ID)
                    .map(|e| e.proficiency)
            })
            .unwrap_or(0.0);
        let profile = shield_block_profile(&template_id, shield_proficiency);

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
        // 插入 ShieldBlock component + ShieldDrainOverride（P4 熟练度缩放 drain）
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert((
                ShieldBlock {
                    template_id: template_id.clone(),
                },
                ShieldDrainOverride {
                    drain_per_s: profile.drain_per_s,
                },
            ));
        }

        tracing::debug!(
            "[bong][shield] RaiseShield entity={entity:?}: shield raised (template={template_id}, block_ratio={block_ratio:.2}, drain_per_s={:.2}) tick={}",
            profile.drain_per_s,
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
            // P4: 同时移除 ShieldDrainOverride（防止 stamina_tick 用旧 drain 值继续消耗）
            entity_commands.remove::<(ShieldBlock, ShieldDrainOverride)>();
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
/// `death_arbiter_tick` 之后运行，而 `death_arbiter_tick` 内的 `clear_death_combat_state`
/// 会无条件 `status_effects.active.clear()`，先于本系统清掉 ShieldBlocking status。
/// 若依赖 `has_active_status` 判断，死亡时举盾的 StopAnim 永远不会发出（生产孤岛）。
/// `ShieldBlock` component 是盾牌模块专属的持续标记，`clear_death_combat_state` 不会触碰它，
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
        // 盾挡状态可能仍然残留，在死亡时一并清理。
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
            // P4: 同时移除 ShieldDrainOverride
            entity_commands.remove::<(ShieldBlock, ShieldDrainOverride)>();
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
/// 此系统注册在 Physics set 内 `stamina_tick` 之后（确保 state 已更新为 Exhausted）。
/// set 链为 Intent→Physics（`.chain()`），故本系统实际运行在 Intent set 的
/// `raise_shield_handler` *之后*；但顺序无关安全——Exhausted 下 raise 本就被体力系统拒绝。
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
        // 2. 移除 ShieldBlock component + ShieldDrainOverride（P4）
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<(ShieldBlock, ShieldDrainOverride)>();
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

/// plan-shield-block-v1 P4 — 体力低预警 narration 系统。
///
/// 每 tick 扫描所有正在举盾（`ShieldBlock` component + `StaminaState::ShieldBlocking`）的实体，
/// 若体力低于 `SHIELD_LOW_STAMINA_NARRATION_THRESHOLD`（25%），向该玩家发送一次感知叙事。
///
/// 为避免每 tick 重复发，使用 `last_stamina_warn_tick` 标记（记录在 `ShieldBlock` component 上）。
/// 但由于 `ShieldBlock` 是不可变 snapshot，改用外部节流：检查当前 tick 是否满足 4s 间隔
/// （80 ticks @ 20 tps），防止刷屏。
///
/// `PendingGameplayNarrations` 需以 `Option<ResMut>` 访问（与 relic_hydrate 惯例一致）。
pub fn shield_low_stamina_narration_tick(
    clock: Res<CombatClock>,
    shield_q: Query<(Entity, &Stamina), valence::prelude::With<ShieldBlock>>,
    clients: valence::prelude::Query<
        (Entity, &valence::prelude::Username),
        valence::prelude::With<valence::prelude::Client>,
    >,
    mut narrations: Option<ResMut<crate::player::gameplay::PendingGameplayNarrations>>,
) {
    // 节流：每 80 ticks（4s @ 20tps）发一次，防刷屏。
    const WARN_INTERVAL_TICKS: u64 = 80;
    if !clock.tick.is_multiple_of(WARN_INTERVAL_TICKS) {
        return;
    }
    let Some(narrations) = narrations.as_deref_mut() else {
        return;
    };
    for (entity, stamina) in &shield_q {
        if stamina.state != crate::combat::components::StaminaState::ShieldBlocking {
            continue;
        }
        let ratio = if stamina.max > 0.0 {
            stamina.current / stamina.max
        } else {
            0.0
        };
        if ratio >= SHIELD_LOW_STAMINA_NARRATION_THRESHOLD {
            continue;
        }
        // 仅对有 Username 的实体（即真实玩家客户端）发 narration。
        if let Some((_, username)) = clients.iter().find(|(e, _)| *e == entity) {
            let player_id = crate::player::state::canonical_player_id(username.0.as_str());
            narrations.push_player(
                &player_id,
                "臂膀发酸，撑不了几下了。",
                crate::schema::common::NarrationStyle::Perception,
            );
        }
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
            entity_commands.remove::<(ShieldBlock, ShieldDrainOverride)>();
        }
    }
}

#[cfg(test)]
#[path = "shield_block_tests.rs"]
mod tests;
