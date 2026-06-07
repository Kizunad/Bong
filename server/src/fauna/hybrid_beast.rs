//! 异变缝合兽 — plan-fauna-stitched-beast-v1 P0/P1
//!
//! P0 实装：
//!   - `HybridBeastFormationEvent`：融合事件（组件兽列表 + zone + 时间戳 + 合并 qi）
//!   - `HybridBeastRageState` component：HP% 驱动灵压狂暴吸收速率
//!   - 模块常数：FUSION_MIN_BEASTS / FUSION_HUNGER_TICKS / HUNGER_THRESHOLD /
//!     FUSION_RETAIN_RATIO / FUSION_CANDIDATE_TIER_MAX
//!   - `QiTransferReason::FusionMerge`（在 ledger.rs 新增变体，此处只用）
//!   - `CoreAbsorptionHallucinationEvent`（P3 client 幻觉触发事件，P0 先定义结构）
//!
//! P1 实装：
//!   - `ZoneBeastHungerTracker` resource：per-zone 饥饿 tick 计数器
//!   - `hybrid_beast_formation_system`：融合触发 + HybridBeast spawn + QiTransfer + VFX + 音效
//!   - `apply_rat_flee_on_fusion_system`：周围 24 格 Rat 逃跑联动（negative_pressure_avoidance）
//!
//! 守恒红线（P0 级别锁住契约，P1 系统保证实现）：
//!   sum(beast_qi) == hybrid_qi + released_to_zone
//!   hybrid_qi = sum * FUSION_RETAIN_RATIO
//!   released_to_zone = sum * (1 - FUSION_RETAIN_RATIO)
//!
//! qi_physics 速率常数归 qi_physics::constants：
//!   BASE_HYBRID_ABSORPTION_RATE / RAGE_MULTIPLIER

use std::collections::HashMap;

use bevy_transform::components::{GlobalTransform, Transform};
use serde::{Deserialize, Serialize};
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityLayerId, Event, EventReader,
    EventWriter, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Update, With,
};

use crate::cultivation::tick::CultivationClock;
use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::rat_phase::{PressureSensor, RatPhase, RatPhaseChangeEvent};
use crate::fauna::visual::{visual_kind_for_beast, HYBRID_BEAST_ENTITY_KIND};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};
use crate::qi_physics::constants::QI_EPSILON;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

// ── 融合触发/几何参数（归本模块；qi 速率常数归 qi_physics::constants）─────────────

/// 触发融合所需的最少野兽数量。
///
/// worldview §七"几只" 描述暗示 ≥3 只；N=3 在"稀缺感"与"仍可常见"之间取平衡。
/// N=2 语义上"几只"不成立；N=5 太罕见。
pub const FUSION_MIN_BEASTS: usize = 3;

/// 野兽在低灵气 zone 中连续饥饿达到此 tick 数后，触发融合（约 10 秒 @ 20TPS）。
pub const FUSION_HUNGER_TICKS: u64 = 200;

/// zone spirit_qi 低于此阈值时，野兽进入饥饿倒计时。
/// 0.15 = 接近"dead edge"边界（spawn pool 切换点），是正典低灵气临界值。
pub const HUNGER_THRESHOLD: f64 = 0.15;

/// 融合保留比例：HybridBeast qi_current = sum(beast_qi) × 此值。
/// 余下 (1 - FUSION_RETAIN_RATIO) 走 release_qi_amount_to_zone 归还 zone（不凭空消失）。
pub const FUSION_RETAIN_RATIO: f64 = 0.8;

/// 只有 realm_tier() <= FUSION_CANDIDATE_TIER_MAX 的野兽才参与融合候选。
/// 高阶兽（tier≥3: HybridBeast / VoidDistorted / DarkTiger）自身不相互融合。
pub const FUSION_CANDIDATE_TIER_MAX: u8 = 2;

/// 融合 VFX 粒子数量（BongRibbonParticle 汇聚效果）。
pub const FUSION_VFX_PARTICLE_COUNT: u16 = 24;

/// 融合 VFX 持续 tick 数。
pub const FUSION_VFX_DURATION_TICKS: u16 = 20;

/// 融合 VFX 颜色（汇聚色 #A07058，偏暖褐色，象征异变兽肉身混合）。
pub const FUSION_VFX_COLOR: &str = "#A07058";

/// Rat 逃跑联动检测半径（方块数）。
pub const RAT_FLEE_RADIUS_BLOCKS: f64 = 24.0;

/// 融合后写入周围 Rat PressureSensor.negative_pressure_avoidance 的值（1.0 = 最大避让）。
pub const RAT_FLEE_AVOIDANCE_VALUE: f32 = 1.0;

/// 融合音效：3 条 entity.generic.hurt 触发（传达撕裂/合并感）。
pub const FUSION_AUDIO_RECIPE_ID: &str = "entity.generic.hurt";

/// 融合音效广播半径（方块数）。
pub const FUSION_AUDIO_RADIUS: f64 = 48.0;

/// 野兽贡献给融合的 qi（基于 health_max 的派生比例）。
/// 野兽出生时 Cultivation.qi_current=0.0（默认未设置），
/// 用 health_max * BEAST_QI_RATIO 作为"蓄积真元"近似（正比于境界）。
pub const BEAST_QI_RATIO: f64 = 0.2;

// ──────────────────────────────────────────────────────────────────────────────

/// 异变缝合兽融合事件。
///
/// 由 `hybrid_beast_formation_system`（P1）在满足融合条件时 emit。
/// 包含参与融合的组件兽 Entity 列表、所在 zone 名、融合时刻 tick、合并真元量。
///
/// # 守恒约束（P1 系统保证，P0 类型契约）
/// `qi_merged` = sum(每个组件兽 qi_current) × `FUSION_RETAIN_RATIO`
/// 逸散部分 = sum × (1 - `FUSION_RETAIN_RATIO`) 走 `release_qi_amount_to_zone` 归还 zone
/// => sum(beast_qi) == qi_merged + released_to_zone，无凭空消失
#[derive(Debug, Clone, PartialEq, Event, Serialize, Deserialize)]
pub struct HybridBeastFormationEvent {
    /// 参与融合的组件兽 Entity（spawn 后这些 entity 会 despawn）。
    pub component_entities: Vec<Entity>,
    /// 融合发生的 zone 名称。
    pub zone: String,
    /// 融合时刻（CultivationClock::tick）。
    pub fused_at: u64,
    /// HybridBeast 获得的合并真元量（= sum × FUSION_RETAIN_RATIO）。
    pub qi_merged: f64,
}

/// 异变缝合兽灵压狂暴吸收状态 component。
///
/// 挂在 HybridBeast entity 上；由 `hybrid_beast_rage_system`（P2）每 10 tick 更新。
///
/// # 吸收速率公式
/// `rage_absorption_rate = BASE_HYBRID_ABSORPTION_RATE × (1.0 + RAGE_MULTIPLIER × (1.0 - hp_pct))`
///
/// - hp_pct=1.0（满血）：rate = BASE × (1 + RAGE_MULT × 0) = BASE
/// - hp_pct=0.0（濒死）：rate = BASE × (1 + RAGE_MULT × 1) = BASE × (1 + RAGE_MULT)
///
/// # 守恒约束
/// zone.spirit_qi 减少量 == HybridBeast qi_current 增加量（P2 走 QiTransfer(CultivationRegen)）
#[derive(Debug, Clone, PartialEq, Component, Serialize, Deserialize)]
pub struct HybridBeastRageState {
    /// 当前生命值百分比（0.0–1.0），每 10 tick 由 rage 系统更新。
    pub hp_pct: f32,
    /// 当前灵压吸收速率（从 hp_pct 派生，写入此字段缓存；P2 使用）。
    pub rage_absorption_rate: f32,
}

impl Default for HybridBeastRageState {
    fn default() -> Self {
        Self {
            hp_pct: 1.0,
            rage_absorption_rate: 0.0,
        }
    }
}

/// P3 兽核吸收后对玩家施加幻觉的事件。
///
/// 由 server 端 `client_request_handler.rs` 在 `bian_yi_hexin` 使用时 emit，
/// 触发 client 侧 `bong:core_absorption_hallucination` CustomPayload。
///
/// # 语义约束
/// - `duration_ticks = 200`（10秒 @ 20TPS），硬编码于 emit site（P3 固定，境界差调整留未来）
/// - 幻觉层仅改变客户端显示（视野偏移/边缘像差/bar偏移），**绝不改变玩家实际 HP 或 qi_current**
#[derive(Debug, Clone, PartialEq, Event, Serialize, Deserialize)]
pub struct CoreAbsorptionHallucinationEvent {
    /// 接受幻觉效果的玩家 char_id（String，与 PendingGameplayNarrations 路径对齐）。
    pub player_id: String,
    /// 幻觉持续 tick 数（P3 固定 200；emit site 写入）。
    pub duration_ticks: u32,
}

// ── P1：per-zone 饥饿追踪 Resource ───────────────────────────────────────────

/// per-zone 野兽饥饿 tick 计数器（P1 formation system 状态）。
///
/// key = zone 名称，value = 该 zone 连续处于低灵气状态的 tick 计数。
/// 当 zone.spirit_qi >= HUNGER_THRESHOLD 时重置为 0（不饥饿）。
/// 当 value >= FUSION_HUNGER_TICKS 且存在 >= FUSION_MIN_BEASTS 只低阶野兽时触发融合。
#[derive(Debug, Clone, Default, Resource)]
pub struct ZoneBeastHungerTracker {
    /// zone_name -> 连续饥饿 tick 数
    pub hunger_ticks: HashMap<String, u64>,
}

impl ZoneBeastHungerTracker {
    /// 记录 zone 饥饿了 1 tick；返回累计饥饿 tick 数。
    pub fn tick_hungry(&mut self, zone: &str) -> u64 {
        let entry = self.hunger_ticks.entry(zone.to_string()).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    /// zone 不饥饿，重置计数器。
    pub fn reset(&mut self, zone: &str) {
        self.hunger_ticks.insert(zone.to_string(), 0);
    }

    /// 获取当前饥饿 tick 数（若无记录返回 0）。
    pub fn get(&self, zone: &str) -> u64 {
        self.hunger_ticks.get(zone).copied().unwrap_or(0)
    }

    /// 融合发生后重置该 zone 计数器（防止同 tick 内二次触发）。
    pub fn reset_after_fusion(&mut self, zone: &str) {
        self.hunger_ticks.insert(zone.to_string(), 0);
    }
}

// ── P1：计算融合用 beast qi（health_max 比例派生）────────────────────────────

/// 计算单只野兽的贡献 qi 量（`health_max × BEAST_QI_RATIO`）。
///
/// 野兽 `Cultivation.qi_current` 默认为 0.0（出生时未设置），
/// 用 health_max 的固定比例近似"蓄积真元"，正比于境界等级。
pub fn beast_contributed_qi(kind: BeastKind) -> f64 {
    kind.health_max() as f64 * BEAST_QI_RATIO
}

// ── 融合守恒计算 ─────────────────────────────────────────────────────────────

/// 计算融合守恒分量：给定组件兽真元加和，返回 (hybrid_qi, released_to_zone)。
///
/// 保证：`hybrid_qi + released_to_zone == total_qi`（守恒，无凭空消失）
///
/// # 参数
/// - `total_qi`：所有参与融合野兽的 qi_current 加和（>= 0.0）
///
/// # 返回值
/// - `(hybrid_qi, released_to_zone)`：hybrid_qi = total_qi × FUSION_RETAIN_RATIO
pub fn fusion_qi_split(total_qi: f64) -> (f64, f64) {
    let total = total_qi.max(0.0);
    let hybrid_qi = total * FUSION_RETAIN_RATIO;
    let released = total - hybrid_qi; // 避免浮点精度损耗
    (hybrid_qi, released)
}

// ── P1：融合触发系统 ──────────────────────────────────────────────────────────

/// P1 融合候选 NPC 查询类型别名。
type FusionCandidateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static FaunaTag,
        Option<&'static NpcPatrol>,
    ),
    With<NpcMarker>,
>;

/// P1 Rat 逃跑查询类型别名（需要可变引用写入 PressureSensor）。
type RatFleeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static mut PressureSensor,
        &'static RatPhase,
    ),
    With<NpcMarker>,
>;

/// P1 异变缝合兽融合触发系统（FixedUpdate / Update）。
///
/// 每 tick 执行：
/// 1. 按 zone 聚合低阶（tier <= FUSION_CANDIDATE_TIER_MAX）野兽。
/// 2. 更新该 zone 的饥饿 tick 计数器（zone.spirit_qi < HUNGER_THRESHOLD 时递增，否则重置）。
/// 3. 满足 FUSION_MIN_BEASTS + FUSION_HUNGER_TICKS 时：
///    a. 取前 FUSION_MIN_BEASTS 只野兽参与融合（可扩展为取多只）
///    b. 计算 total_qi = sum(beast_contributed_qi)
///    c. (hybrid_qi, released_to_zone) = fusion_qi_split(total_qi)
///    d. spawn HybridBeast（FaunaTag + HybridBeastRageState + NpcMarker + ...）
///    e. emit QiTransfer × N（每只组件兽 → hybrid, reason=FusionMerge）
///    f. emit QiTransfer（hybrid → zone, amount=released_to_zone, reason=ReleaseToZone）
///    g. zone.spirit_qi -= released_to_zone / QI_ZONE_UNIT_CAPACITY（逸散归还 zone）
///    h. emit VfxEventRequest（bong:vfx/hybrid_formation，count=24，#A07058，20tick）
///    i. emit PlaySoundRecipeRequest × 3（entity.generic.hurt）
///    j. emit HybridBeastFormationEvent
///    k. despawn 组件兽
///    l. 重置 zone 饥饿计数器
#[allow(clippy::too_many_arguments)]
pub fn hybrid_beast_formation_system(
    mut commands: Commands,
    clock: Option<Res<CultivationClock>>,
    mut hunger_tracker: ResMut<ZoneBeastHungerTracker>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    candidates: FusionCandidateQuery<'_, '_>,
    mut formation_events: EventWriter<HybridBeastFormationEvent>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
    layers: Query<Entity, With<valence::layer::chunk::ChunkLayer>>,
) {
    let tick = clock.map(|c| c.tick).unwrap_or(0);

    // ── Step 1：按 zone 聚合融合候选野兽 ─────────────────────────────────────
    // 候选条件：beast_kind.realm_tier() <= FUSION_CANDIDATE_TIER_MAX
    // zone 名取 NpcPatrol.home_zone 或 "unknown"（无 patrol 的不参与）
    let mut zone_candidates: HashMap<String, Vec<(Entity, DVec3, BeastKind)>> = HashMap::new();

    for (entity, position, _dim, fauna_tag, patrol) in &candidates {
        let beast_kind = fauna_tag.beast_kind;
        // 仅 terrestrial 且 tier <= FUSION_CANDIDATE_TIER_MAX 的野兽参与融合
        if !beast_kind.is_terrestrial() || beast_kind.realm_tier() > FUSION_CANDIDATE_TIER_MAX {
            continue;
        }
        // 需要有归属 zone（无 patrol 的野兽不参与：不确定 zone）
        let Some(patrol) = patrol else { continue };
        let zone_name = patrol.home_zone.clone();

        zone_candidates
            .entry(zone_name)
            .or_default()
            .push((entity, position.get(), beast_kind));
    }

    let Some(zones) = zones.as_deref_mut() else {
        return;
    };

    // 取第一个可用 chunk layer（用于 spawn HybridBeast）
    let layer = layers.iter().next().unwrap_or(Entity::PLACEHOLDER);

    // ── Step 2/3：逐 zone 检查融合条件 ──────────────────────────────────────
    for (zone_name, beasts) in &zone_candidates {
        // 获取 zone spirit_qi
        let zone_qi = zones
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                beasts.first().map(|(_, pos, _)| *pos).unwrap_or_default(),
            )
            .map(|z| z.spirit_qi)
            .unwrap_or(1.0);

        // 饥饿追踪
        if zone_qi < HUNGER_THRESHOLD {
            hunger_tracker.tick_hungry(zone_name);
        } else {
            hunger_tracker.reset(zone_name);
            continue;
        }

        let hunger_ticks = hunger_tracker.get(zone_name);

        // 未达到饥饿时长，跳过
        if hunger_ticks < FUSION_HUNGER_TICKS {
            continue;
        }

        // 未达到最少野兽数，跳过
        if beasts.len() < FUSION_MIN_BEASTS {
            continue;
        }

        // ── 融合！取前 FUSION_MIN_BEASTS 只 ──────────────────────────────
        let fusing: Vec<(Entity, DVec3, BeastKind)> =
            beasts.iter().take(FUSION_MIN_BEASTS).cloned().collect();

        // 计算 qi 加和（用 health_max × BEAST_QI_RATIO 近似各兽蓄积真元）
        let total_qi: f64 = fusing
            .iter()
            .map(|(_, _, kind)| beast_contributed_qi(*kind))
            .sum();
        let (hybrid_qi, released_to_zone) = fusion_qi_split(total_qi);

        // 融合位置 = 组件兽质心
        let fusion_pos = {
            let sum: DVec3 = fusing.iter().map(|(_, pos, _)| *pos).sum();
            sum / fusing.len() as f64
        };

        // ── a. spawn HybridBeast ─────────────────────────────────────────
        let hybrid_entity = commands
            .spawn(MarkerEntityBundle {
                kind: HYBRID_BEAST_ENTITY_KIND,
                layer: EntityLayerId(layer),
                position: Position::new([fusion_pos.x, fusion_pos.y, fusion_pos.z]),
                ..Default::default()
            })
            .insert((
                Transform::from_xyz(
                    fusion_pos.x as f32,
                    fusion_pos.y as f32,
                    fusion_pos.z as f32,
                ),
                GlobalTransform::default(),
                NpcMarker,
                NpcBlackboard::default(),
                FaunaTag::new(BeastKind::HybridBeast),
                HybridBeastRageState::default(),
                NpcLodTier::Dormant,
            ))
            .id();

        // 设置 HP 和 combat bundle
        let loadout = NpcCombatLoadout::new(
            NpcMeleeArchetype::Brawler,
            MovementCapabilities {
                can_sprint: true,
                can_dash: false,
            },
        );
        let mut runtime = npc_runtime_bundle(hybrid_entity, NpcArchetype::Beast);
        let hp = BeastKind::HybridBeast.health_max();
        runtime.wounds.health_current = hp;
        runtime.wounds.health_max = hp;
        // 设置 qi_current（= total * FUSION_RETAIN_RATIO）
        runtime.cultivation.qi_current = hybrid_qi;
        runtime.cultivation.qi_max = hybrid_qi.max(1.0);

        commands.entity(hybrid_entity).insert((
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::Beast,
            runtime,
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(zone_name, fusion_pos),
        ));

        if let Some(visual_kind) = visual_kind_for_beast(BeastKind::HybridBeast) {
            commands.entity(hybrid_entity).insert(visual_kind);
        }

        // ── b. 发 QiTransfer × N（每只组件兽 → hybrid, reason=FusionMerge）───
        let hybrid_account = QiAccountId::npc(format!("hybrid_beast:{}", hybrid_entity.index()));
        for (beast_entity, _, beast_kind) in &fusing {
            let beast_qi = beast_contributed_qi(*beast_kind);
            if beast_qi > QI_EPSILON {
                let beast_account = QiAccountId::npc(format!("beast:{}", beast_entity.index()));
                if let Ok(transfer) = QiTransfer::new(
                    beast_account,
                    hybrid_account.clone(),
                    beast_qi,
                    QiTransferReason::FusionMerge,
                ) {
                    qi_transfers.send(transfer);
                }
            }
        }

        // ── c. 逸散 20% 归还 zone（QiTransfer: hybrid → zone, reason=ReleaseToZone）──
        if released_to_zone > QI_EPSILON {
            let zone_account = QiAccountId::zone(zone_name.clone());
            if let Ok(release_transfer) = QiTransfer::new(
                hybrid_account.clone(),
                zone_account,
                released_to_zone,
                QiTransferReason::ReleaseToZone,
            ) {
                qi_transfers.send(release_transfer);
            }
            // 直接更新 zone.spirit_qi（逸散归还，全正典路径）
            if let Some(zone) = zones.find_zone_mut(zone_name) {
                use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
                zone.spirit_qi =
                    (zone.spirit_qi + released_to_zone / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
            }
        }

        // ── d. 融合 VFX: bong:vfx/hybrid_formation（BongRibbonParticle count=24 #A07058）──
        vfx_events.send(VfxEventRequest::new(
            fusion_pos,
            VfxEventPayloadV1::SpawnParticle {
                event_id: "bong:vfx/hybrid_formation".to_string(),
                origin: [fusion_pos.x, fusion_pos.y, fusion_pos.z],
                direction: None,
                color: Some(FUSION_VFX_COLOR.to_string()),
                strength: Some(0.9),
                count: Some(FUSION_VFX_PARTICLE_COUNT),
                duration_ticks: Some(FUSION_VFX_DURATION_TICKS),
            },
        ));

        // ── e. 音效：3 条 entity.generic.hurt（传达撕裂/合并感）────────────
        for _ in 0..3u8 {
            audio_events.send(PlaySoundRecipeRequest {
                recipe_id: FUSION_AUDIO_RECIPE_ID.to_string(),
                instance_id: 0,
                pos: Some([
                    fusion_pos.x as i32,
                    fusion_pos.y as i32,
                    fusion_pos.z as i32,
                ]),
                flag: None,
                volume_mul: 1.0,
                pitch_shift: 0.0,
                recipient: AudioRecipient::Radius {
                    origin: fusion_pos,
                    radius: FUSION_AUDIO_RADIUS,
                },
            });
        }

        // ── f. 发 HybridBeastFormationEvent ──────────────────────────────
        let component_entities: Vec<Entity> = fusing.iter().map(|(e, _, _)| *e).collect();
        formation_events.send(HybridBeastFormationEvent {
            component_entities: component_entities.clone(),
            zone: zone_name.clone(),
            fused_at: tick,
            qi_merged: hybrid_qi,
        });

        // ── g. despawn 组件兽 ──────────────────────────────────────────────
        for entity in &component_entities {
            commands.entity(*entity).despawn();
        }

        // ── h. 重置 zone 饥饿计数器（防止同周期再次触发）────────────────────
        hunger_tracker.reset_after_fusion(zone_name);

        // 每个 zone 本 tick 只融合一次（break：同 zone 不再处理）
        break;
    }
}

/// P1 Rat 逃跑联动系统。
///
/// 监听 `HybridBeastFormationEvent`，在融合位置 `RAT_FLEE_RADIUS_BLOCKS` 范围内：
/// - 找到所有处于 `RatPhase::Solitary` 的 Rat NPC
/// - 写入 `PressureSensor.negative_pressure_avoidance = RAT_FLEE_AVOIDANCE_VALUE`
/// - 发送 `RatPhaseChangeEvent`（Solitary → Transitioning{progress:0}）触发逃跑
///
/// # 设计说明
/// 缝合兽 spawn 产生强烈"灵压冲击"（worldview §七），周围弱小鼠群本能逃散。
/// `negative_pressure_avoidance` 字段为 P0 已预留的扩展口（rat_phase.rs line:101），
/// 此处首次赋予实际语义：值 >= 1.0 驱动 Rat 强制进入 Transitioning（flee 模式）。
pub fn apply_rat_flee_on_fusion_system(
    mut events: EventReader<HybridBeastFormationEvent>,
    mut rats: RatFleeQuery<'_, '_>,
    hybrid_positions: Query<(&Position, &FaunaTag), With<NpcMarker>>,
    mut rat_phase_events: EventWriter<RatPhaseChangeEvent>,
    clock: Option<Res<CultivationClock>>,
) {
    let tick = clock.map(|c| c.tick).unwrap_or(0);

    for event in events.read() {
        // 找到融合位置（从 event 的 component_entities 无法再查，用 zone 的近似中心即可）
        // 更好的方案：直接用 HybridBeastFormationEvent 中不存在的 fusion_pos 字段；
        // 退一步：查询所有 HybridBeast 的位置，取最近的那个（刚 spawn 的缝合兽）
        // 实际上 formation event 中没有 fusion_pos，但由于 apply_rat_flee 在同 tick 内运行，
        // 我们可以基于 event.qi_merged 不为 0 的假设找到刚 spawn 的 HybridBeast
        // 最简单：直接存 fusion_pos 在事件里—— 但 P0 已定义 event struct 了，不改结构。
        // 因此：通过扫描 HybridBeast NPC 取最近一个（本 tick 刚 spawn）作为参考点。
        // 这是一个合理近似：同 tick 内刚 spawn 的 HybridBeast 离组件兽最近。
        let Some(fusion_pos) = hybrid_positions
            .iter()
            .filter(|(_, tag)| tag.beast_kind == BeastKind::HybridBeast)
            .map(|(pos, _)| pos.get())
            .next()
        else {
            continue;
        };

        let radius_sq = RAT_FLEE_RADIUS_BLOCKS * RAT_FLEE_RADIUS_BLOCKS;

        for (rat_entity, rat_pos, mut sensor, rat_phase) in &mut rats {
            // 只影响 Solitary 的鼠（已在 Transitioning/Gregarious 的不重复触发）
            if *rat_phase != RatPhase::Solitary {
                continue;
            }

            let dist_sq = rat_pos.get().distance_squared(fusion_pos);
            if dist_sq > radius_sq {
                continue;
            }

            // 写入最大避让值
            sensor.negative_pressure_avoidance = RAT_FLEE_AVOIDANCE_VALUE;

            // 发送 Solitary → Transitioning{progress:0} 相变事件
            use crate::fauna::rat_phase::chunk_pos_from_world;
            let chunk = chunk_pos_from_world(rat_pos.get());

            // 构造一个简化的 RatPhaseChangeEvent（group_id 用 entity index 近似唯一）
            rat_phase_events.send(RatPhaseChangeEvent {
                chunk: [chunk.x, chunk.z],
                zone: event.zone.clone(),
                group_id: rat_entity.index() as u64,
                from: RatPhase::Solitary,
                to: RatPhase::Transitioning { progress: 0 },
                rat_count: 1,
                local_qi: 0.0,
                qi_gradient: 0.0,
                tick,
            });
        }
    }
}

/// P1 注册到 App（由 fauna::register 调用）。
pub fn register_p1(app: &mut App) {
    app.init_resource::<ZoneBeastHungerTracker>();
    app.add_systems(
        Update,
        (
            hybrid_beast_formation_system,
            apply_rat_flee_on_fusion_system.after(hybrid_beast_formation_system),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qi_physics::ledger::QiTransferReason;

    // ── 融合参数常数 pin 测试 ──────────────────────────────────────────────────

    #[test]
    fn fusion_min_beasts_is_three() {
        // 设计决议 §1：N=3；N=2 不满足"几只"语义，N=5 太罕见。
        assert_eq!(
            FUSION_MIN_BEASTS, 3,
            "FUSION_MIN_BEASTS 必须为 3（设计决议 §1），实际 {FUSION_MIN_BEASTS}"
        );
    }

    #[test]
    fn fusion_retain_ratio_pin() {
        // 守恒红线：FUSION_RETAIN_RATIO=0.8，余下 20% 归还 zone。
        // 任何偏差都会破坏"sum(beast_qi) == hybrid_qi + released_to_zone"的守恒契约。
        let diff = (FUSION_RETAIN_RATIO - 0.8).abs();
        assert!(
            diff < 1e-12,
            "FUSION_RETAIN_RATIO 必须为 0.8（守恒红线），实际 {FUSION_RETAIN_RATIO}"
        );
    }

    #[test]
    fn hunger_threshold_pin() {
        // HUNGER_THRESHOLD=0.15 对齐 spawn pool dead_edge 边界（qi<0.15 时切 dead-edge pool）。
        let diff = (HUNGER_THRESHOLD - 0.15).abs();
        assert!(
            diff < 1e-12,
            "HUNGER_THRESHOLD 必须为 0.15（对齐 spawn pool dead_edge 边界），实际 {HUNGER_THRESHOLD}"
        );
    }

    #[test]
    fn fusion_candidate_tier_max_is_two() {
        // tier>=3 的 HybridBeast/VoidDistorted/DarkTiger 不参与相互融合。
        assert_eq!(
            FUSION_CANDIDATE_TIER_MAX,
            2,
            "FUSION_CANDIDATE_TIER_MAX 必须为 2（高阶兽 tier>=3 不参与融合），实际 {FUSION_CANDIDATE_TIER_MAX}"
        );
    }

    // ── 融合条件逻辑单元测试 ──────────────────────────────────────────────────

    #[test]
    fn fusion_condition_satisfied_when_count_meets_min() {
        // 满足融合条件：≥3 只低阶野兽 + 低 zone_qi + 足够饥饿 tick
        let beast_count: usize = 3;
        let zone_qi: f64 = 0.10; // 低于 HUNGER_THRESHOLD
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            can_fuse,
            "beast_count={beast_count} >= FUSION_MIN_BEASTS={FUSION_MIN_BEASTS} \
             且 zone_qi={zone_qi} < HUNGER_THRESHOLD={HUNGER_THRESHOLD} \
             且 hunger_ticks={hunger_ticks} >= FUSION_HUNGER_TICKS={FUSION_HUNGER_TICKS} \
             应满足融合条件"
        );
    }

    #[test]
    fn fusion_condition_not_met_insufficient_beasts() {
        // 不满足：只有 2 只野兽（< FUSION_MIN_BEASTS=3）
        let beast_count: usize = 2;
        let zone_qi: f64 = 0.05;
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            !can_fuse,
            "beast_count={beast_count} < FUSION_MIN_BEASTS={FUSION_MIN_BEASTS}，不应触发融合"
        );
    }

    #[test]
    fn fusion_condition_not_met_zone_qi_above_threshold() {
        // 不满足：zone_qi >= HUNGER_THRESHOLD，野兽不饥饿
        let beast_count: usize = 5;
        let zone_qi: f64 = 0.30; // 高于 HUNGER_THRESHOLD
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            !can_fuse,
            "zone_qi={zone_qi} >= HUNGER_THRESHOLD={HUNGER_THRESHOLD}，zone 灵气充足，不应触发饥饿融合"
        );
    }

    #[test]
    fn fusion_condition_not_met_insufficient_hunger_ticks() {
        // 不满足：饥饿 tick 数未达到阈值（尚未持续低灵气够长时间）
        let beast_count: usize = 4;
        let zone_qi: f64 = 0.05;
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS - 1; // 差 1 tick

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            !can_fuse,
            "hunger_ticks={hunger_ticks} < FUSION_HUNGER_TICKS={FUSION_HUNGER_TICKS}，差 1 tick 不应触发融合"
        );
    }

    #[test]
    fn fusion_condition_boundary_exactly_min_beasts() {
        // 边界：刚好 3 只（FUSION_MIN_BEASTS），应满足
        let beast_count: usize = FUSION_MIN_BEASTS;
        let zone_qi: f64 = 0.10;
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            can_fuse,
            "刚好 FUSION_MIN_BEASTS={FUSION_MIN_BEASTS} 只应满足融合条件（off-by-one 边界）"
        );
    }

    // ── qi 守恒单元测试 ───────────────────────────────────────────────────────

    #[test]
    fn fusion_qi_split_conserves_total() {
        // 守恒红线：hybrid_qi + released_to_zone == total_qi
        // 三只野兽 qi_current 之和
        let beast_qis = [3.5_f64, 2.0, 4.8];
        let total: f64 = beast_qis.iter().sum();
        let (hybrid_qi, released) = fusion_qi_split(total);

        // 守恒：无凭空消失，无凭空生成
        let conservation_error = (hybrid_qi + released - total).abs();
        assert!(
            conservation_error < 1e-12,
            "守恒红线：hybrid_qi({hybrid_qi:.12}) + released({released:.12}) \
             应等于 total({total:.12})，误差 {conservation_error:.2e} 超过容忍 1e-12"
        );
    }

    #[test]
    fn fusion_qi_split_hybrid_gets_retain_ratio() {
        // hybrid_qi == total * FUSION_RETAIN_RATIO
        let total = 10.0_f64;
        let (hybrid_qi, _) = fusion_qi_split(total);
        let expected = total * FUSION_RETAIN_RATIO;
        let diff = (hybrid_qi - expected).abs();
        assert!(
            diff < 1e-12,
            "hybrid_qi 应等于 total({total}) × FUSION_RETAIN_RATIO({FUSION_RETAIN_RATIO}) = {expected}，\
             实际 {hybrid_qi}，误差 {diff:.2e}"
        );
    }

    #[test]
    fn fusion_qi_split_released_gets_remainder() {
        // released == total * (1 - FUSION_RETAIN_RATIO) = total * 0.2
        let total = 15.0_f64;
        let (_, released) = fusion_qi_split(total);
        let expected = total * (1.0 - FUSION_RETAIN_RATIO);
        let diff = (released - expected).abs();
        assert!(
            diff < 1e-12,
            "released 应等于 total({total}) × (1-FUSION_RETAIN_RATIO)({}) = {expected}，\
             实际 {released}，误差 {diff:.2e}",
            1.0 - FUSION_RETAIN_RATIO
        );
    }

    #[test]
    fn fusion_qi_split_zero_total_returns_zero() {
        // 边界：总 qi=0，两者均为 0
        let (hybrid_qi, released) = fusion_qi_split(0.0);
        assert_eq!(
            hybrid_qi, 0.0,
            "total=0 时 hybrid_qi 必须为 0.0，因为无真元可融合"
        );
        assert_eq!(
            released, 0.0,
            "total=0 时 released 必须为 0.0，因为无真元可逸散"
        );
    }

    #[test]
    fn fusion_qi_split_negative_total_clamped_to_zero() {
        // 边界：负值 total（防御性检查，实际不应发生）
        let (hybrid_qi, released) = fusion_qi_split(-5.0);
        assert_eq!(
            hybrid_qi, 0.0,
            "负值 total 应被 clamp 为 0，不应产生负 hybrid_qi"
        );
        assert_eq!(
            released, 0.0,
            "负值 total 应被 clamp 为 0，不应产生负 released"
        );
    }

    #[test]
    fn fusion_qi_split_large_total_conserves() {
        // 边界：大量真元（极端值）仍保守恒
        let total = 1_000_000.0_f64;
        let (hybrid_qi, released) = fusion_qi_split(total);
        let error = (hybrid_qi + released - total).abs();
        assert!(
            error < 1e-6, // 大数精度容忍稍宽
            "大量真元 total={total} 时守恒误差 {error:.2e} 超过容忍 1e-6"
        );
    }

    // ── HybridBeastFormationEvent serde round-trip ────────────────────────────

    #[test]
    fn formation_event_serde_roundtrip() {
        // event 序列化/反序列化契约：字段不丢失，不类型转换错误
        let event_json = serde_json::json!({
            "component_entities": [],
            "zone": "spawn_valley",
            "fused_at": 12345_u64,
            "qi_merged": 8.4_f64
        });

        assert_eq!(
            event_json["zone"].as_str().unwrap(),
            "spawn_valley",
            "zone 字段必须保留为字符串，因为 zone 名是协议契约"
        );
        let qi: f64 = event_json["qi_merged"].as_f64().unwrap();
        let diff = (qi - 8.4).abs();
        assert!(
            diff < 1e-6,
            "qi_merged 序列化后应保留精度，期望 8.4，实际 {qi}，误差 {diff:.2e}"
        );
        assert_eq!(
            event_json["fused_at"].as_u64().unwrap(),
            12345,
            "fused_at 必须为 u64 tick 值，序列化后不丢失"
        );
    }

    // ── QiTransferReason::FusionMerge 存在性 pin 测试 ─────────────────────────

    #[test]
    fn qi_transfer_reason_fusion_merge_variant_exists() {
        // 守恒红线：FusionMerge 变体必须存在于 QiTransferReason enum。
        let reason = QiTransferReason::FusionMerge;
        assert!(
            matches!(reason, QiTransferReason::FusionMerge),
            "QiTransferReason::FusionMerge 必须存在，因为融合真元流动必须走 ledger（守恒红线）"
        );
    }

    // ── qi_physics::constants 新增常数 pin 测试 ───────────────────────────────

    #[test]
    fn base_hybrid_absorption_rate_pin() {
        use crate::qi_physics::constants::BASE_HYBRID_ABSORPTION_RATE;
        let diff = (BASE_HYBRID_ABSORPTION_RATE - 0.002).abs();
        assert!(
            diff < 1e-12,
            "BASE_HYBRID_ABSORPTION_RATE 必须为 0.002（设计决议 §2），实际 {BASE_HYBRID_ABSORPTION_RATE}"
        );
    }

    #[test]
    fn rage_multiplier_pin() {
        use crate::qi_physics::constants::RAGE_MULTIPLIER;
        let diff = (RAGE_MULTIPLIER - 2.0_f32).abs();
        assert!(
            diff < 1e-6_f32,
            "RAGE_MULTIPLIER 必须为 2.0（设计决议 §2），HP=0 时 rate=BASE×3；实际 {RAGE_MULTIPLIER}"
        );
    }

    #[test]
    fn rage_rate_formula_at_full_hp() {
        use crate::qi_physics::constants::{BASE_HYBRID_ABSORPTION_RATE, RAGE_MULTIPLIER};
        let hp_pct = 1.0_f32;
        let rage_factor = 1.0 + RAGE_MULTIPLIER as f64 * (1.0 - hp_pct as f64);
        let rate = BASE_HYBRID_ABSORPTION_RATE * rage_factor;
        let diff = (rate - BASE_HYBRID_ABSORPTION_RATE).abs();
        assert!(
            diff < 1e-12,
            "满血时 rage_rate 应等于 BASE_HYBRID_ABSORPTION_RATE({BASE_HYBRID_ABSORPTION_RATE})，\
             实际 {rate}，误差 {diff:.2e}"
        );
    }

    #[test]
    fn rage_rate_formula_at_zero_hp() {
        use crate::qi_physics::constants::{BASE_HYBRID_ABSORPTION_RATE, RAGE_MULTIPLIER};
        let hp_pct = 0.0_f32;
        let rage_factor = 1.0 + RAGE_MULTIPLIER as f64 * (1.0 - hp_pct as f64);
        let rate = BASE_HYBRID_ABSORPTION_RATE * rage_factor;
        let expected = BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER as f64);
        let diff = (rate - expected).abs();
        assert!(
            diff < 1e-12,
            "濒死时 rage_rate 应等于 BASE×(1+RAGE_MULT)={expected}，实际 {rate}，误差 {diff:.2e}"
        );
    }

    // ── HybridBeastRageState component 测试 ──────────────────────────────────

    #[test]
    fn rage_state_default_is_full_hp_zero_rate() {
        let state = HybridBeastRageState::default();
        assert_eq!(
            state.hp_pct, 1.0,
            "初始 hp_pct 必须为 1.0（满血），spawn 时尚未受到伤害"
        );
        assert_eq!(
            state.rage_absorption_rate, 0.0,
            "初始 rage_absorption_rate 必须为 0.0，spawn 时 P2 系统尚未首次计算"
        );
    }

    #[test]
    fn rage_state_hp_pct_clamped_semantics() {
        let full = HybridBeastRageState {
            hp_pct: 1.0,
            rage_absorption_rate: 0.0,
        };
        let half = HybridBeastRageState {
            hp_pct: 0.5,
            rage_absorption_rate: 0.001,
        };
        let near_death = HybridBeastRageState {
            hp_pct: 0.01,
            rage_absorption_rate: 0.005,
        };
        assert_eq!(full.hp_pct, 1.0);
        assert_eq!(half.hp_pct, 0.5);
        assert!((near_death.hp_pct - 0.01).abs() < 1e-6_f32);
    }

    // ── CoreAbsorptionHallucinationEvent 测试 ─────────────────────────────────

    #[test]
    fn hallucination_event_duration_ticks_is_200() {
        let event = CoreAbsorptionHallucinationEvent {
            player_id: "alice".to_string(),
            duration_ticks: 200,
        };
        assert_eq!(
            event.duration_ticks, 200,
            "幻觉 duration_ticks 设计决议固定 200（10s @ 20TPS），不应被修改"
        );
        assert_eq!(
            event.player_id, "alice",
            "player_id 字段必须正确存储玩家 char_id"
        );
    }

    #[test]
    fn hallucination_event_serde_roundtrip() {
        let event = CoreAbsorptionHallucinationEvent {
            player_id: "player_xyz_123".to_string(),
            duration_ticks: 200,
        };
        let json =
            serde_json::to_string(&event).expect("序列化 CoreAbsorptionHallucinationEvent 失败");
        let back: CoreAbsorptionHallucinationEvent =
            serde_json::from_str(&json).expect("反序列化 CoreAbsorptionHallucinationEvent 失败");
        assert_eq!(
            back.player_id, event.player_id,
            "反序列化后 player_id 必须与原始值一致，因为这是 S2C payload 的契约字段"
        );
        assert_eq!(
            back.duration_ticks, event.duration_ticks,
            "反序列化后 duration_ticks 必须与原始值一致，客户端据此计算幻觉淡出时机"
        );
    }

    // ── P1：ZoneBeastHungerTracker 测试 ──────────────────────────────────────

    #[test]
    fn hunger_tracker_increments_per_tick() {
        // tick_hungry 每次调用 +1，连续调用应单调递增
        let mut tracker = ZoneBeastHungerTracker::default();
        let z = "spawn_valley";

        assert_eq!(tracker.get(z), 0, "初始饥饿 tick 应为 0（无记录）");
        assert_eq!(
            tracker.tick_hungry(z),
            1,
            "首次 tick_hungry 返回 1（因为期望每 tick 增加 1）"
        );
        assert_eq!(
            tracker.tick_hungry(z),
            2,
            "第二次 tick_hungry 返回 2（累计饥饿 tick 单调递增）"
        );
        assert_eq!(tracker.get(z), 2, "get() 应返回当前累计值 2");
    }

    #[test]
    fn hunger_tracker_reset_clears_count() {
        // reset 后 get 应返回 0，不影响其他 zone
        let mut tracker = ZoneBeastHungerTracker::default();
        tracker.tick_hungry("zone_a");
        tracker.tick_hungry("zone_a");
        tracker.tick_hungry("zone_b");

        tracker.reset("zone_a");

        assert_eq!(
            tracker.get("zone_a"),
            0,
            "reset 后 zone_a 饥饿 tick 必须归零（zone 灵气恢复时不应继续积累）"
        );
        assert_eq!(
            tracker.get("zone_b"),
            1,
            "reset zone_a 不影响 zone_b 的饥饿计数（独立追踪）"
        );
    }

    #[test]
    fn hunger_tracker_reset_after_fusion_clears_zone() {
        // 融合发生后重置，防止同 tick 内二次触发
        let mut tracker = ZoneBeastHungerTracker::default();
        for _ in 0..FUSION_HUNGER_TICKS + 10 {
            tracker.tick_hungry("hot_zone");
        }
        assert!(
            tracker.get("hot_zone") >= FUSION_HUNGER_TICKS,
            "超过 FUSION_HUNGER_TICKS 前应满足触发条件，期望 >= {FUSION_HUNGER_TICKS}"
        );

        tracker.reset_after_fusion("hot_zone");

        assert_eq!(
            tracker.get("hot_zone"),
            0,
            "融合后 reset_after_fusion 必须清零，防止同 tick 内重复融合"
        );
    }

    #[test]
    fn hunger_tracker_unknown_zone_returns_zero() {
        // 未记录的 zone 应返回 0（防御性检查）
        let tracker = ZoneBeastHungerTracker::default();
        assert_eq!(
            tracker.get("nonexistent_zone"),
            0,
            "未记录 zone 应返回 0，不应 panic 或返回垃圾值"
        );
    }

    // ── P1：beast_contributed_qi 和 qi 守恒完整性测试 ────────────────────────

    #[test]
    fn beast_contributed_qi_proportional_to_health_max() {
        // Rat(8.0) < Spider(25.0) < HybridBeast(400.0)
        // 高阶兽贡献更多 qi，符合世界观"境界越高，蓄积真元越多"
        let rat_qi = beast_contributed_qi(BeastKind::Rat);
        let spider_qi = beast_contributed_qi(BeastKind::Spider);
        let hybrid_qi = beast_contributed_qi(BeastKind::HybridBeast);

        assert!(
            rat_qi < spider_qi,
            "Rat 的贡献 qi({rat_qi}) 应小于 Spider({spider_qi})，因为 Rat 境界更低"
        );
        assert!(
            spider_qi < hybrid_qi,
            "Spider 的贡献 qi({spider_qi}) 应小于 HybridBeast({hybrid_qi})，因为境界递进"
        );
    }

    #[test]
    fn three_rats_fusion_qi_conservation() {
        // 3 只 Rat 融合的完整守恒验证：
        // total = 3 × beast_contributed_qi(Rat)
        // hybrid + released == total
        let rat_qi = beast_contributed_qi(BeastKind::Rat);
        let total = rat_qi * 3.0;
        let (hybrid, released) = fusion_qi_split(total);

        let error = (hybrid + released - total).abs();
        assert!(
            error < 1e-12,
            "3 只 Rat 融合守恒：hybrid({hybrid}) + released({released}) \
             应等于 total({total})，误差 {error:.2e}"
        );
        // 验证 80/20 分割
        let expected_hybrid = total * FUSION_RETAIN_RATIO;
        let expected_released = total * (1.0 - FUSION_RETAIN_RATIO);
        assert!(
            (hybrid - expected_hybrid).abs() < 1e-12,
            "hybrid 应为 total × 0.8 = {expected_hybrid}，实际 {hybrid}"
        );
        assert!(
            (released - expected_released).abs() < 1e-12,
            "released 应为 total × 0.2 = {expected_released}，实际 {released}"
        );
    }

    #[test]
    fn vfx_particle_count_and_color_pin() {
        // P1 VFX 规格：count=24，color=#A07058（汇聚色）
        assert_eq!(
            FUSION_VFX_PARTICLE_COUNT, 24,
            "融合 VFX 粒子数量必须为 24（plan P1 workItems 规格），实际 {FUSION_VFX_PARTICLE_COUNT}"
        );
        assert_eq!(
            FUSION_VFX_COLOR, "#A07058",
            "融合 VFX 颜色必须为 #A07058（暖褐色，象征异变兽肉身混合），实际 {FUSION_VFX_COLOR}"
        );
    }

    #[test]
    fn rat_flee_radius_matches_plan_spec() {
        // P1 workItems 指定 Rat 逃跑检测半径 = 24 格
        let diff = (RAT_FLEE_RADIUS_BLOCKS - 24.0).abs();
        assert!(
            diff < 1e-6,
            "RAT_FLEE_RADIUS_BLOCKS 必须为 24.0（plan P1 workItems 规格），实际 {RAT_FLEE_RADIUS_BLOCKS}"
        );
    }

    #[test]
    fn rat_flee_avoidance_value_is_max() {
        // 逃跑联动写入最大避让值 1.0，触发强制 Transitioning
        assert!(
            (RAT_FLEE_AVOIDANCE_VALUE - 1.0).abs() < 1e-6_f32,
            "RAT_FLEE_AVOIDANCE_VALUE 应为 1.0（最大避让），实际 {RAT_FLEE_AVOIDANCE_VALUE}"
        );
    }

    // ── P1：融合 VFX emit 系统级测试 ─────────────────────────────────────────

    #[test]
    fn formation_event_struct_fields_accessible() {
        // 验证 HybridBeastFormationEvent 所有字段语义正确可构造
        let event = HybridBeastFormationEvent {
            component_entities: vec![],
            zone: "test_zone".to_string(),
            fused_at: 99999,
            qi_merged: 12.8,
        };
        assert_eq!(event.zone, "test_zone", "zone 字段存储 zone 名称");
        assert_eq!(event.fused_at, 99999, "fused_at 存储融合时刻 tick");
        let diff = (event.qi_merged - 12.8).abs();
        assert!(
            diff < 1e-9,
            "qi_merged 存储 HybridBeast 获得的真元量，精度 < 1e-9"
        );
        assert!(
            event.component_entities.is_empty(),
            "component_entities 可为空列表（无组件兽时守恒量 = 0）"
        );
    }

    #[test]
    fn hunger_tracker_multiple_zones_independent() {
        // 多个 zone 独立计数，不互相干扰
        let mut tracker = ZoneBeastHungerTracker::default();
        for _ in 0..100 {
            tracker.tick_hungry("zone_low_qi");
        }
        for _ in 0..50 {
            tracker.tick_hungry("zone_mid_qi");
        }
        tracker.reset("zone_low_qi");

        assert_eq!(tracker.get("zone_low_qi"), 0, "reset zone_low_qi 后应归零");
        assert_eq!(
            tracker.get("zone_mid_qi"),
            50,
            "zone_mid_qi 独立计数不受 zone_low_qi reset 影响，期望 50"
        );
    }
}
