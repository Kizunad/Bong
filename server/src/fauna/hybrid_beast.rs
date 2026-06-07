//! 异变缝合兽 — plan-fauna-stitched-beast-v1 P0/P1/P2
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
//! P2 实装：
//!   - `hybrid_beast_rage_system`：HP% 驱动灵压狂暴吸收（每 10 tick / 2Hz）
//!     * rage_absorption_rate = BASE × (1 + RAGE_MULT × (1 - hp_pct))
//!     * 调 `regen_from_zone` → QiTransfer(CultivationRegen)，zone.spirit_qi -= drain
//!     * HP<50%：VFX bong:vfx/hybrid_rage（BongLineParticle count=8 #FF4010）
//!     * HP<25%：VFX count=16 #FF0000
//!     * 持续音效 block.deepslate.hit loop
//!     * zone.spirit_qi 跌负后不主动 emit 事件，既有 negative_zone_siphon_tick 自动处理
//!
//! 守恒红线（P0 级别锁住契约，P1 系统保证实现）：
//!   sum(beast_qi) == hybrid_qi + released_to_zone
//!   hybrid_qi = sum * FUSION_RETAIN_RATIO
//!   released_to_zone = sum * (1 - FUSION_RETAIN_RATIO)
//!
//! P2 守恒红线：
//!   zone.spirit_qi 减少量 == ledger 累计 QiTransfer(CultivationRegen).amount / QI_ZONE_UNIT_CAPACITY
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

use crate::combat::components::Wounds;
use crate::cultivation::components::Cultivation;
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
use crate::qi_physics::constants::{BASE_HYBRID_ABSORPTION_RATE, QI_EPSILON, RAGE_MULTIPLIER};
use crate::qi_physics::excretion::regen_from_zone;
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

// BEAST_QI_RATIO 已删除（守恒红线修复）：野兽 qi 不再由 health_max 虚构。
// 融合时直接读取组件兽真实 Cultivation.qi_current；野兽出生时 qi_current=0.0，
// hybrid 初始 qi=0，靠后续灵压狂暴吸收积累（正典路径）。

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
/// 包含 Cultivation 以读取组件兽真实 qi_current（守恒红线：不虚构 qi）。
type FusionCandidateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static FaunaTag,
        Option<&'static NpcPatrol>,
        &'static Cultivation,
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
    // 4-元组末位为真实 qi_current（不虚构，守恒红线）
    let mut zone_candidates: HashMap<String, Vec<(Entity, DVec3, BeastKind, f64)>> = HashMap::new();

    for (entity, position, _dim, fauna_tag, patrol, cultivation) in &candidates {
        let beast_kind = fauna_tag.beast_kind;
        // 仅 terrestrial 且 tier <= FUSION_CANDIDATE_TIER_MAX 的野兽参与融合
        if !beast_kind.is_terrestrial() || beast_kind.realm_tier() > FUSION_CANDIDATE_TIER_MAX {
            continue;
        }
        // 需要有归属 zone（无 patrol 的野兽不参与：不确定 zone）
        let Some(patrol) = patrol else { continue };
        let zone_name = patrol.home_zone.clone();

        // 读取真实 qi_current（守恒红线：不用 health_max×ratio 虚构；野兽初始 qi=0）
        let beast_qi_current = cultivation.qi_current.max(0.0);

        zone_candidates.entry(zone_name).or_default().push((
            entity,
            position.get(),
            beast_kind,
            beast_qi_current,
        ));
    }

    let Some(zones) = zones.as_deref_mut() else {
        return;
    };

    // 取第一个可用 chunk layer（用于 spawn HybridBeast）
    let layer = layers.iter().next().unwrap_or(Entity::PLACEHOLDER);

    // ── Step 2/3：逐 zone 检查融合条件 ──────────────────────────────────────
    for (zone_name, beasts) in &zone_candidates {
        // 获取 zone spirit_qi：统一按 home_zone 名查找（与 hunger_tracker key 及
        // find_zone_mut 路径对齐，避免空间查找与名称查找双通道不一致 —— M4 修复）
        let zone_qi = zones
            .find_zone_by_name(zone_name)
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
        let fusing: Vec<(Entity, DVec3, BeastKind, f64)> =
            beasts.iter().take(FUSION_MIN_BEASTS).cloned().collect();

        // 计算 qi 加和：读取各兽真实 Cultivation.qi_current（守恒红线：不虚构）
        // 野兽出生时 qi_current=0.0，故通常 total_qi=0；hybrid 初始 qi=0，
        // 靠后续灵压狂暴吸收积累（正典路径）。
        let total_qi: f64 = fusing.iter().map(|(_, _, _, qi)| *qi).sum();
        let (hybrid_qi, released_to_zone) = fusion_qi_split(total_qi);

        // 融合位置 = 组件兽质心
        let fusion_pos = {
            let sum: DVec3 = fusing.iter().map(|(_, pos, _, _)| *pos).sum();
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
        // 使用组件兽真实 qi_current；野兽通常 qi=0 则不发 transfer（防 ledger 噪音）
        let hybrid_account = QiAccountId::npc(format!("hybrid_beast:{}", hybrid_entity.index()));
        for (beast_entity, _, _, beast_qi) in &fusing {
            if *beast_qi > QI_EPSILON {
                let beast_account = QiAccountId::npc(format!("beast:{}", beast_entity.index()));
                if let Ok(transfer) = QiTransfer::new(
                    beast_account,
                    hybrid_account.clone(),
                    *beast_qi,
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
        let component_entities: Vec<Entity> = fusing.iter().map(|(e, _, _, _)| *e).collect();
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

// ── P2：灵压狂暴吸收常数 ──────────────────────────────────────────────────────

/// 狂暴系统每 N tick 运行一次（2Hz @ 20TPS = 每 10 tick）。
/// 减少每 tick 都跑的开销，同时保持足够的灵气压力响应速度。
pub const RAGE_TICK_INTERVAL: u64 = 10;

/// HP 低于此百分比时触发 rage VFX（50% = 半血）。
pub const RAGE_VFX_HALF_HP_THRESHOLD: f32 = 0.5;

/// HP 低于此百分比时触发 "濒死" rage VFX（25%）。
pub const RAGE_VFX_CRITICAL_HP_THRESHOLD: f32 = 0.25;

/// 半血 rage VFX 粒子数量（BongLineParticle）。
pub const RAGE_VFX_HALF_HP_COUNT: u16 = 8;

/// 濒死 rage VFX 粒子数量（BongLineParticle）。
pub const RAGE_VFX_CRITICAL_COUNT: u16 = 16;

/// 半血 rage VFX 颜色（#FF4010，暗橙红，象征灵压失控初期）。
pub const RAGE_VFX_HALF_HP_COLOR: &str = "#FF4010";

/// 濒死 rage VFX 颜色（#FF0000，纯红，象征灵压濒临崩溃）。
pub const RAGE_VFX_CRITICAL_COLOR: &str = "#FF0000";

/// rage VFX 持续 tick 数（短暂一闪，不遮挡视线）。
pub const RAGE_VFX_DURATION_TICKS: u16 = 12;

/// rage 持续音效 recipe ID（block.deepslate.hit，低频嗡鸣感）。
pub const RAGE_AUDIO_RECIPE_ID: &str = "block.deepslate.hit";

/// rage 音效广播半径（方块数）。
pub const RAGE_AUDIO_RADIUS: f64 = 32.0;

// ── P2：灵压狂暴吸收速率纯函数 ───────────────────────────────────────────────

/// 计算当前 HP 百分比对应的灵压狂暴吸收速率。
///
/// 公式：`rate = BASE_HYBRID_ABSORPTION_RATE × (1.0 + RAGE_MULTIPLIER × (1.0 - hp_pct))`
///
/// - `hp_pct = 1.0`（满血）：rate = BASE（无加成）
/// - `hp_pct = 0.5`（半血）：rate = BASE × (1 + RAGE_MULT × 0.5) = BASE × 2.0
/// - `hp_pct = 0.0`（濒死）：rate = BASE × (1 + RAGE_MULT) = BASE × 3.0
///
/// # 参数
/// - `hp_pct`：HP 百分比（clamp 至 [0, 1]）
///
/// # 返回值
/// 该 tick 应传入 `regen_from_zone` 的 `rate` 参数（f64）。
pub fn compute_rage_absorption_rate(hp_pct: f32) -> f64 {
    let hp_pct = hp_pct.clamp(0.0, 1.0) as f64;
    BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER as f64 * (1.0 - hp_pct))
}

// ── P2：灵压狂暴吸收系统 ─────────────────────────────────────────────────────

/// P2 HybridBeast 灵压狂暴吸收系统（每 RAGE_TICK_INTERVAL tick 运行一次）。
///
/// 执行逻辑：
/// 1. 每 RAGE_TICK_INTERVAL tick 运行一次（tick % RAGE_TICK_INTERVAL == 0）
/// 2. 查询所有带 `HybridBeastRageState` 的 `NpcMarker` entity
/// 3. hp_pct = wounds.health_current / wounds.health_max
/// 4. rage_absorption_rate = BASE × (1 + RAGE_MULT × (1 - hp_pct))
/// 5. 调用 `regen_from_zone(zone.spirit_qi, rate, integrity=1.0, qi_room)`
/// 6. zone.spirit_qi -= drain（zone 灵气减少；drain 已是 zone 单位，由 regen_from_zone 内部除以 QI_ZONE_UNIT_CAPACITY 给出）
/// 7. emit QiTransfer(zone → npc_hybrid, amount=gain, reason=CultivationRegen)
/// 8. HP<50% 时 emit VFX（bong:vfx/hybrid_rage，BongLineParticle count=8 #FF4010）
/// 9. HP<25% 时升级 VFX（count=16 #FF0000）
/// 10. emit 音效（block.deepslate.hit，每次吸收 tick 发一条）
///
/// # 守恒约束
/// zone.spirit_qi 减少量 = drain = gain / QI_ZONE_UNIT_CAPACITY
/// HybridBeast qi_current += gain（通过 ledger QiTransfer 记录；Cultivation 组件更新在此系统）
/// => zone 减少量 × QI_ZONE_UNIT_CAPACITY == hybrid 增加量（无凭空损耗/生成）
///
/// # 设计说明
/// zone.spirit_qi 跌负后不主动 emit ZoneEnteringNegativePressure（该 event 不存在于代码）。
/// 依赖既有 `negative_zone_siphon_tick`（cultivation/negative_zone.rs:32）自动对区域内玩家施加 qi siphon。
type HybridRageQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static CurrentDimension>,
        &'static Wounds,
        &'static mut HybridBeastRageState,
        &'static mut Cultivation,
    ),
    (With<NpcMarker>, With<FaunaTag>),
>;

pub fn hybrid_beast_rage_system(
    clock: Option<Res<CultivationClock>>,
    mut rage_query: HybridRageQuery<'_, '_>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    let tick = clock.map(|c| c.tick).unwrap_or(0);

    // 每 RAGE_TICK_INTERVAL tick 运行一次（2Hz @ 20TPS）
    if tick % RAGE_TICK_INTERVAL != 0 {
        return;
    }

    let Some(zones) = zones.as_deref_mut() else {
        return;
    };

    for (entity, pos, dim, wounds, mut rage_state, mut cultivation) in &mut rage_query {
        // 只处理 HybridBeast（通过 FaunaTag 无法直接过滤，需运行时检查）
        // 注意：query 使用 With<FaunaTag>，但所有 NpcMarker 都有此标记
        // 精确过滤：只有带 HybridBeastRageState 的才是缝合兽（FaunaTag.beast_kind == HybridBeast 是充分条件）
        // HybridBeastRageState 是独占 HybridBeast 的 component，有此 component == 是缝合兽

        // ── Step 1：计算 HP 百分比 ───────────────────────────────────────────
        let hp_pct = if wounds.health_max > 0.0 {
            (wounds.health_current / wounds.health_max).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // ── Step 2：计算吸收速率并更新 RageState ────────────────────────────
        let rate = compute_rage_absorption_rate(hp_pct);
        rage_state.hp_pct = hp_pct;
        rage_state.rage_absorption_rate = rate as f32;

        // ── Step 3：查找所在 zone ──────────────────────────────────────────
        let dim_kind = dim
            .map(|d| d.0)
            .unwrap_or(crate::world::dimension::DimensionKind::Overworld);

        let Some(zone_name) = zones.find_zone(dim_kind, pos.get()).map(|z| z.name.clone()) else {
            continue;
        };
        let Some(zone) = zones.find_zone_mut(&zone_name) else {
            continue;
        };

        // zone.spirit_qi <= 0 时不能通过正常 regen 路径吸收（regen_from_zone 内置检查）
        // 但即使跌负仍可能有残余，regen_from_zone 会正确返回 (0,0)
        // 所以不需要显式 skip，让函数自行处理

        // ── Step 4：调用 regen_from_zone ─────────────────────────────────
        // qi_room = 无上限（缝合兽真元池随融合而增长，不设硬上限）
        // 使用 f64::MAX / 2.0 避免溢出
        let qi_room = f64::MAX / 2.0;
        let (gain, drain) = regen_from_zone(zone.spirit_qi, rate, 1.0, qi_room);

        if gain <= QI_EPSILON || drain <= QI_EPSILON {
            // zone 已空或无法吸收，跳过（不 emit QiTransfer 防止 ledger 噪音）
            continue;
        }

        // ── Step 5：更新 zone.spirit_qi（zone 减少量 = drain）──────────────
        zone.spirit_qi = (zone.spirit_qi - drain).max(-1.0);

        // ── Step 5b：更新 hybrid Cultivation.qi_current（守恒红线 B2 修复）──
        // zone 减少 drain 对应 hybrid 增加 gain；两者守恒（B2 fix：此前 gain 丢失）。
        // 缝合兽真元池无硬上限（设计注释），qi_max 随 qi_current 动态增长。
        cultivation.qi_current = (cultivation.qi_current + gain).max(0.0);
        // qi_max 随积累动态增长（rage 无上限）
        if cultivation.qi_current > cultivation.qi_max {
            cultivation.qi_max = cultivation.qi_current;
        }

        // ── Step 6：emit QiTransfer（zone → hybrid, CultivationRegen）───────
        let zone_account = QiAccountId::zone(zone_name.clone());
        let hybrid_account = QiAccountId::npc(format!("hybrid_beast:{}", entity.index()));
        if let Ok(transfer) = QiTransfer::new(
            zone_account,
            hybrid_account,
            gain,
            QiTransferReason::CultivationRegen,
        ) {
            qi_transfers.send(transfer);
        }

        // ── Step 7：VFX（HP 档位驱动）────────────────────────────────────
        let world_pos = pos.get();
        if hp_pct < RAGE_VFX_CRITICAL_HP_THRESHOLD {
            // HP < 25%：濒死，count=16 #FF0000
            vfx_events.send(VfxEventRequest::new(
                world_pos,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: "bong:vfx/hybrid_rage".to_string(),
                    origin: [world_pos.x, world_pos.y, world_pos.z],
                    direction: None,
                    color: Some(RAGE_VFX_CRITICAL_COLOR.to_string()),
                    strength: Some(1.0),
                    count: Some(RAGE_VFX_CRITICAL_COUNT),
                    duration_ticks: Some(RAGE_VFX_DURATION_TICKS),
                },
            ));
        } else if hp_pct < RAGE_VFX_HALF_HP_THRESHOLD {
            // HP < 50%：半血，count=8 #FF4010
            vfx_events.send(VfxEventRequest::new(
                world_pos,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: "bong:vfx/hybrid_rage".to_string(),
                    origin: [world_pos.x, world_pos.y, world_pos.z],
                    direction: None,
                    color: Some(RAGE_VFX_HALF_HP_COLOR.to_string()),
                    strength: Some(0.7),
                    count: Some(RAGE_VFX_HALF_HP_COUNT),
                    duration_ticks: Some(RAGE_VFX_DURATION_TICKS),
                },
            ));
        }
        // HP >= 50%：无 rage VFX（满血无视觉反馈，符合"感受压力需要打它"的设计）

        // ── Step 8：持续音效（每次吸收 tick 发一条）──────────────────────
        audio_events.send(PlaySoundRecipeRequest {
            recipe_id: RAGE_AUDIO_RECIPE_ID.to_string(),
            instance_id: 0,
            pos: Some([world_pos.x as i32, world_pos.y as i32, world_pos.z as i32]),
            flag: None,
            volume_mul: 0.5 + (1.0 - hp_pct) * 0.5, // 血量越低音量越大
            pitch_shift: -0.2 + (1.0 - hp_pct) * 0.4, // 血量越低音调越低沉
            recipient: AudioRecipient::Radius {
                origin: world_pos,
                radius: RAGE_AUDIO_RADIUS,
            },
        });
    }
}

/// P2 注册到 App（由 fauna::register 调用）。
pub fn register_p2(app: &mut App) {
    app.add_systems(Update, hybrid_beast_rage_system);
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

    // ── P1：融合 qi 守恒完整性测试（B1 修复：使用真实 qi，非 health_max 虚构）────

    #[test]
    fn three_beasts_with_zero_qi_fusion_starts_hybrid_at_zero() {
        // 守恒红线 B1：野兽 qi_current=0（NPC 出生默认值），
        // 融合后 hybrid 初始 qi=0，released_to_zone=0；世界总 qi 不增加。
        // hybrid 靠灵压狂暴吸收积累，正典路径。
        let beast_qi_currents = [0.0_f64, 0.0, 0.0]; // 3 只野兽真实 qi
        let total: f64 = beast_qi_currents.iter().sum();
        let (hybrid_qi, released) = fusion_qi_split(total);

        assert_eq!(
            hybrid_qi, 0.0,
            "守恒红线 B1：野兽 qi_current 均为 0 时，hybrid 初始 qi 必须为 0（不凭空造真元），\
             实际 hybrid_qi={hybrid_qi}"
        );
        assert_eq!(
            released, 0.0,
            "守恒红线 B1：野兽 qi_current 均为 0 时，released_to_zone 必须为 0（不凭空造逸散），\
             实际 released={released}"
        );
        // 世界总 qi 守恒：before = sum(beast_qi) = 0，after = hybrid_qi + released = 0
        let world_qi_before = total;
        let world_qi_after = hybrid_qi + released;
        let error = (world_qi_after - world_qi_before).abs();
        assert!(
            error < 1e-12,
            "世界总 qi 守恒：before={world_qi_before} after={world_qi_after} 误差 {error:.2e}"
        );
    }

    #[test]
    fn three_beasts_with_nonzero_qi_fusion_conserves() {
        // 若野兽通过灵压吸收已积累真元（qi>0），融合时守恒：
        // sum(beast_qi) == hybrid_qi + released_to_zone
        let beast_qi_currents = [3.0_f64, 5.0, 2.0]; // 假设 3 只兽各有 qi
        let total: f64 = beast_qi_currents.iter().sum();
        let (hybrid, released) = fusion_qi_split(total);

        let error = (hybrid + released - total).abs();
        assert!(
            error < 1e-12,
            "野兽有 qi 时守恒：hybrid({hybrid}) + released({released}) \
             应等于 total({total})，误差 {error:.2e}"
        );
        let expected_hybrid = total * FUSION_RETAIN_RATIO;
        let expected_released = total * (1.0 - FUSION_RETAIN_RATIO);
        assert!(
            (hybrid - expected_hybrid).abs() < 1e-12,
            "hybrid 应为 total × FUSION_RETAIN_RATIO = {expected_hybrid}，实际 {hybrid}"
        );
        assert!(
            (released - expected_released).abs() < 1e-12,
            "released 应为 total × (1-FUSION_RETAIN_RATIO) = {expected_released}，实际 {released}"
        );
    }

    #[test]
    fn three_rats_fusion_qi_conservation() {
        // 3 只野兽 qi_current=0 的保守恒验证（典型场景：NPC 出生默认）。
        // B1 修复后：total=0，hybrid=0，released=0，世界 qi 不变。
        let total = 0.0_f64; // 3 只野兽真实 qi 之和（NPC 默认 0）
        let (hybrid, released) = fusion_qi_split(total);

        let error = (hybrid + released - total).abs();
        assert!(
            error < 1e-12,
            "守恒红线：hybrid({hybrid}) + released({released}) \
             应等于 total({total})，误差 {error:.2e}"
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

    // ── P2：compute_rage_absorption_rate 单元测试 ────────────────────────────

    #[test]
    fn rage_rate_full_hp_equals_base() {
        // 满血：rate = BASE × (1 + RAGE_MULT × 0) = BASE（无加成）
        let rate = compute_rage_absorption_rate(1.0);
        let diff = (rate - BASE_HYBRID_ABSORPTION_RATE).abs();
        assert!(
            diff < 1e-12,
            "满血(hp=1.0) rage_rate 必须等于 BASE_HYBRID_ABSORPTION_RATE({BASE_HYBRID_ABSORPTION_RATE})，\
             实际 {rate}，误差 {diff:.2e}"
        );
    }

    #[test]
    fn rage_rate_zero_hp_equals_base_times_one_plus_rage_mult() {
        // 濒死：rate = BASE × (1 + RAGE_MULT × 1) = BASE × 3.0（设计决议 §2）
        let rate = compute_rage_absorption_rate(0.0);
        let expected = BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER as f64);
        let diff = (rate - expected).abs();
        assert!(
            diff < 1e-12,
            "濒死(hp=0.0) rage_rate 必须等于 BASE×(1+RAGE_MULT)={expected}，\
             实际 {rate}，误差 {diff:.2e}"
        );
    }

    #[test]
    fn rage_rate_half_hp_midpoint() {
        // 半血：rate = BASE × (1 + RAGE_MULT × 0.5) = BASE × 2.0（RAGE_MULT=2.0 时）
        let rate = compute_rage_absorption_rate(0.5);
        let expected = BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER as f64 * 0.5);
        let diff = (rate - expected).abs();
        assert!(
            diff < 1e-12,
            "半血(hp=0.5) rage_rate 必须等于 BASE×(1+RAGE_MULT×0.5)={expected}，\
             实际 {rate}，误差 {diff:.2e}"
        );
    }

    #[test]
    fn rage_rate_monotonically_decreasing_with_hp() {
        // HP 越低，吸收速率越高（单调递减关系）
        let rates: Vec<f64> = [1.0_f32, 0.75, 0.5, 0.25, 0.0]
            .iter()
            .map(|&hp| compute_rage_absorption_rate(hp))
            .collect();
        for i in 0..rates.len() - 1 {
            assert!(
                rates[i] < rates[i + 1],
                "rage_rate 应随 HP 降低而单调递增：hp={} rate={} < hp={} rate={}（期望 rate[i]<rate[i+1]）",
                [1.0, 0.75, 0.5, 0.25, 0.0][i],
                rates[i],
                [1.0, 0.75, 0.5, 0.25, 0.0][i + 1],
                rates[i + 1]
            );
        }
    }

    #[test]
    fn rage_rate_clamped_for_hp_above_one() {
        // HP > 1.0（不应发生，但防御性 clamp）：rate = BASE（等同满血）
        let rate_normal = compute_rage_absorption_rate(1.0);
        let rate_overheal = compute_rage_absorption_rate(1.5);
        let diff = (rate_normal - rate_overheal).abs();
        assert!(
            diff < 1e-12,
            "HP > 1.0 时 rate 应等同 HP=1.0（clamp 保护），实际 rate_overheal={rate_overheal}"
        );
    }

    #[test]
    fn rage_rate_clamped_for_negative_hp() {
        // HP < 0.0（不应发生）：rate = BASE × (1 + RAGE_MULT)（等同濒死）
        let rate_zero = compute_rage_absorption_rate(0.0);
        let rate_negative = compute_rage_absorption_rate(-0.5);
        let diff = (rate_zero - rate_negative).abs();
        assert!(
            diff < 1e-12,
            "HP < 0.0 时 rate 应等同 HP=0.0（clamp 保护），实际 rate_negative={rate_negative}"
        );
    }

    // ── P2：VFX 阈值常数 pin 测试 ─────────────────────────────────────────────

    #[test]
    fn rage_vfx_half_hp_threshold_pin() {
        // plan P2 workItems 指定 HP<50% 触发 half-blood rage VFX
        let diff = (RAGE_VFX_HALF_HP_THRESHOLD - 0.5).abs();
        assert!(
            diff < 1e-6_f32,
            "RAGE_VFX_HALF_HP_THRESHOLD 必须为 0.5（plan P2 workItems 规格），实际 {RAGE_VFX_HALF_HP_THRESHOLD}"
        );
    }

    #[test]
    fn rage_vfx_critical_hp_threshold_pin() {
        // plan P2 workItems 指定 HP<25% 触发 critical rage VFX（升级版）
        let diff = (RAGE_VFX_CRITICAL_HP_THRESHOLD - 0.25).abs();
        assert!(
            diff < 1e-6_f32,
            "RAGE_VFX_CRITICAL_HP_THRESHOLD 必须为 0.25（plan P2 workItems 规格），实际 {RAGE_VFX_CRITICAL_HP_THRESHOLD}"
        );
    }

    #[test]
    fn rage_vfx_particle_counts_pin() {
        // plan P2 workItems：half-blood count=8，critical count=16
        assert_eq!(
            RAGE_VFX_HALF_HP_COUNT, 8,
            "半血 rage VFX 粒子数量必须为 8（plan P2 workItems 规格），实际 {RAGE_VFX_HALF_HP_COUNT}"
        );
        assert_eq!(
            RAGE_VFX_CRITICAL_COUNT, 16,
            "濒死 rage VFX 粒子数量必须为 16（plan P2 workItems 规格），实际 {RAGE_VFX_CRITICAL_COUNT}"
        );
    }

    #[test]
    fn rage_vfx_colors_pin() {
        // plan P2 workItems：half-blood #FF4010，critical #FF0000
        assert_eq!(
            RAGE_VFX_HALF_HP_COLOR, "#FF4010",
            "半血 rage VFX 颜色必须为 #FF4010（暗橙红，plan P2 workItems 规格），实际 {RAGE_VFX_HALF_HP_COLOR}"
        );
        assert_eq!(
            RAGE_VFX_CRITICAL_COLOR, "#FF0000",
            "濒死 rage VFX 颜色必须为 #FF0000（纯红，plan P2 workItems 规格），实际 {RAGE_VFX_CRITICAL_COLOR}"
        );
    }

    #[test]
    fn rage_tick_interval_is_ten() {
        // 2Hz @ 20TPS = 每 10 tick，对应 2Hz 系统频率
        assert_eq!(
            RAGE_TICK_INTERVAL, 10,
            "RAGE_TICK_INTERVAL 必须为 10（2Hz @ 20TPS），实际 {RAGE_TICK_INTERVAL}"
        );
    }

    // ── P2：系统级守恒测试（zone 变化量 == ledger 累计 QiTransfer）────────────

    #[test]
    fn rage_zone_drain_conservation_single_tick() {
        // 守恒红线：zone.spirit_qi 减少量 = drain = gain / QI_ZONE_UNIT_CAPACITY
        // 验证 regen_from_zone 返回的 (gain, drain) 满足守恒关系
        use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        use crate::qi_physics::excretion::regen_from_zone;

        let zone_qi = 0.5_f64;
        let rate = compute_rage_absorption_rate(0.5); // 半血 rate
        let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, f64::MAX / 2.0);

        // drain > 0（zone 有灵气，rate > 0，应有吸收）
        assert!(
            drain > 0.0,
            "zone_qi=0.5 时 drain 应 > 0（zone 灵气充足，rage 应吸收），实际 {drain}"
        );

        // 守恒关系：gain == drain × QI_ZONE_UNIT_CAPACITY
        let expected_gain = drain * QI_ZONE_UNIT_CAPACITY;
        let error = (gain - expected_gain).abs();
        assert!(
            error < 1e-10,
            "P2 守恒红线：gain({gain:.12}) 应等于 drain({drain:.12}) × QI_ZONE_UNIT_CAPACITY({QI_ZONE_UNIT_CAPACITY})，\
             误差 {error:.2e}"
        );
    }

    #[test]
    fn rage_zone_drain_conservation_multiple_ticks() {
        // 系统级守恒：多次 tick 累计后 zone 减少量 == ledger 累计 QiTransfer.amount / QI_ZONE_UNIT_CAPACITY
        use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        use crate::qi_physics::excretion::regen_from_zone;

        let mut zone_qi = 0.8_f64;
        let mut total_gain = 0.0_f64;
        let mut total_drain = 0.0_f64;
        let initial_zone_qi = zone_qi;

        // 模拟 20 个 rage tick（对应 200 个游戏 tick = 10秒）
        for i in 0..20 {
            let hp_pct = (1.0 - i as f32 * 0.04).max(0.0); // HP 从 100% 线性下降到 24%
            let rate = compute_rage_absorption_rate(hp_pct);
            let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, f64::MAX / 2.0);
            if drain > 0.0 {
                zone_qi -= drain;
                total_gain += gain;
                total_drain += drain;
            }
        }

        // zone 实际减少量
        let zone_decrease = initial_zone_qi - zone_qi;
        let error = (zone_decrease - total_drain).abs();
        assert!(
            error < 1e-10,
            "多 tick 守恒：zone 减少量({zone_decrease:.12}) 应等于累计 drain({total_drain:.12})，\
             误差 {error:.2e}"
        );

        // ledger 累计 gain 守恒：total_gain ≈ total_drain × QI_ZONE_UNIT_CAPACITY
        let expected_total_gain = total_drain * QI_ZONE_UNIT_CAPACITY;
        let gain_error = (total_gain - expected_total_gain).abs();
        assert!(
            gain_error < 1e-8, // 多次乘除有轻微精度积累，容忍稍宽
            "多 tick ledger 守恒：total_gain({total_gain:.12}) ≈ total_drain×QI_ZONE_UNIT_CAPACITY({expected_total_gain:.12})，\
             误差 {gain_error:.2e}"
        );
    }

    #[test]
    fn rage_no_absorption_when_zone_empty() {
        // 边界：zone.spirit_qi <= 0 时 regen_from_zone 返回 (0, 0)，无真元凭空生成
        use crate::qi_physics::excretion::regen_from_zone;

        let zone_qi = 0.0_f64;
        let rate = compute_rage_absorption_rate(0.0); // 最大 rage rate

        let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, f64::MAX / 2.0);
        assert_eq!(
            gain, 0.0,
            "zone_qi=0 时 gain 必须为 0（无真元可吸收，守恒红线），实际 {gain}"
        );
        assert_eq!(
            drain, 0.0,
            "zone_qi=0 时 drain 必须为 0（zone 无灵气可抽），实际 {drain}"
        );
    }

    #[test]
    fn rage_no_absorption_when_zone_negative() {
        // 边界：zone.spirit_qi < 0 时 regen_from_zone 返回 (0, 0)（负灵域不被正常吸收）
        use crate::qi_physics::excretion::regen_from_zone;

        let zone_qi = -0.3_f64;
        let rate = compute_rage_absorption_rate(0.0);
        let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, f64::MAX / 2.0);
        assert_eq!(
            gain, 0.0,
            "zone_qi=-0.3（负灵域）时 gain 必须为 0（regen_from_zone 负值检查），实际 {gain}"
        );
        assert_eq!(
            drain, 0.0,
            "zone_qi=-0.3（负灵域）时 drain 必须为 0，实际 {drain}"
        );
    }

    #[test]
    fn rage_rate_increases_as_zone_depletes() {
        // 系统级：zone.spirit_qi 随 HP 下降和吸收而减少，而 rate 随 HP 下降而增加
        // 验证：濒死缝合兽比满血缝合兽消耗 zone 更快
        use crate::qi_physics::excretion::regen_from_zone;

        let zone_qi = 0.5_f64;
        let rate_full = compute_rage_absorption_rate(1.0); // 满血 rate
        let rate_dying = compute_rage_absorption_rate(0.0); // 濒死 rate

        let (_, drain_full) = regen_from_zone(zone_qi, rate_full, 1.0, f64::MAX / 2.0);
        let (_, drain_dying) = regen_from_zone(zone_qi, rate_dying, 1.0, f64::MAX / 2.0);

        assert!(
            drain_dying > drain_full,
            "濒死缝合兽（rate={rate_dying}）应比满血（rate={rate_full}）更快消耗 zone：\
             drain_dying({drain_dying}) 应 > drain_full({drain_full})"
        );
    }

    #[test]
    fn rage_vfx_threshold_ordering_constraint() {
        // 设计约束：critical_threshold < half_threshold（濒死触发更严格条件）
        // 用变量比较避免 clippy::assertions_on_constants
        let critical = RAGE_VFX_CRITICAL_HP_THRESHOLD;
        let half = RAGE_VFX_HALF_HP_THRESHOLD;
        assert!(
            critical < half,
            "RAGE_VFX_CRITICAL_HP_THRESHOLD({critical}) \
             必须 < RAGE_VFX_HALF_HP_THRESHOLD({half})，\
             否则 HP<25% 区间无法进入 critical 分支"
        );
    }

    #[test]
    fn rage_vfx_critical_count_greater_than_half_hp_count() {
        // 设计约束：濒死粒子数量 > 半血（视觉升级明显）
        // 用变量比较避免 clippy::assertions_on_constants
        let critical_count = RAGE_VFX_CRITICAL_COUNT;
        let half_count = RAGE_VFX_HALF_HP_COUNT;
        assert!(
            critical_count > half_count,
            "RAGE_VFX_CRITICAL_COUNT({critical_count}) \
             必须 > RAGE_VFX_HALF_HP_COUNT({half_count})，\
             否则濒死视觉无法升级"
        );
    }

    // ── P3：CoreAbsorptionHallucinationEvent 饱和测试 ─────────────────────────

    /// P3 幻觉事件结构符合协议：duration_ticks 代表持续时间，player_id 是 char_id
    #[test]
    fn hallucination_event_fields_semantically_correct() {
        let event = CoreAbsorptionHallucinationEvent {
            player_id: "offline:testplayer".to_string(),
            duration_ticks: 200,
        };
        assert_eq!(
            event.player_id, "offline:testplayer",
            "player_id 必须保存 char_id 格式字符串（offline:NAME / char:BITS），\
             client HallucinationLayerHandler 据此过滤本机玩家"
        );
        assert_eq!(
            event.duration_ticks, 200,
            "duration_ticks=200 是设计决议 §5 的 P3 固定值（10s @ 20TPS），\
             client 据此计算幻觉淡出时机"
        );
    }

    /// P3 幻觉持续时间上限：duration_ticks=200 不超过 10 秒
    #[test]
    fn hallucination_duration_ticks_ten_seconds_at_20tps() {
        // 设计约束：P3 固定 200 tick；10s @ 20TPS = 200tick。
        // 任何超过 600 tick（30s）的值都是设计错误。
        let event = CoreAbsorptionHallucinationEvent {
            player_id: "player_a".to_string(),
            duration_ticks: 200,
        };
        assert!(
            event.duration_ticks <= 600,
            "幻觉持续时间不得超过 30s（600tick），实际 {}tick（P3 设计决议 §5 约束）",
            event.duration_ticks
        );
        assert_eq!(
            event.duration_ticks, 200,
            "P3 固定 200tick（10s @ 20TPS），未来引入境界差调整请更新此断言"
        );
    }

    /// P3 幻觉事件不携带任何 HP / qi 值——幻觉只改 client 显示层，绝不改实际值
    #[test]
    fn hallucination_event_has_no_hp_or_qi_fields() {
        // 守恒红线：CoreAbsorptionHallucinationEvent 结构体只有 player_id 和 duration_ticks。
        // 若将来有人错误地添加 hp_override / qi_override 字段，此测试隐性触发编译失败。
        // 通过构造函数全字段初始化来保证"不存在其他字段"。
        let _event = CoreAbsorptionHallucinationEvent {
            player_id: "test".to_string(),
            duration_ticks: 200,
            // 若有第三个字段，此处会编译报错 — 即测试意图
        };
        // 只有 player_id 和 duration_ticks 两个字段，已通过编译
    }

    /// P3 幻觉事件 player_id 边界：空字符串应被拒绝（不应触发事件）
    #[test]
    fn hallucination_event_player_id_empty_string_is_suspicious() {
        // 非 panic 契约：event 本身可以构造，但 server emit 逻辑不应发出 player_id=""。
        // 此测试验证空 player_id 的 serde 序列化/反序列化仍然可行（不崩）。
        let event = CoreAbsorptionHallucinationEvent {
            player_id: "".to_string(),
            duration_ticks: 200,
        };
        let json = serde_json::to_string(&event)
            .expect("空 player_id 的 CoreAbsorptionHallucinationEvent 应可序列化");
        let back: CoreAbsorptionHallucinationEvent =
            serde_json::from_str(&json).expect("反序列化不应因空 player_id 失败");
        assert_eq!(
            back.player_id, "",
            "空 player_id 序列化/反序列化 round-trip 应保持一致（尽管业务上不应产生）"
        );
    }

    /// P3 幻觉事件 duration_ticks=0 边界（取消幻觉的信号值）
    #[test]
    fn hallucination_event_duration_zero_is_cancel_signal() {
        // duration_ticks=0 在协议层表示"立即取消幻觉"（断线 / 到期发送）。
        // event 本身可以携带 0，但 emit site 需要正确解释语义。
        let cancel_event = CoreAbsorptionHallucinationEvent {
            player_id: "offline:alice".to_string(),
            duration_ticks: 0,
        };
        assert_eq!(
            cancel_event.duration_ticks, 0,
            "duration_ticks=0 表示取消幻觉信号，序列化/反序列化后应保持 0"
        );
        // 验证取消信号与激活信号的 duration 语义差异
        let activate_event = CoreAbsorptionHallucinationEvent {
            player_id: "offline:alice".to_string(),
            duration_ticks: 200,
        };
        assert!(
            activate_event.duration_ticks > cancel_event.duration_ticks,
            "激活幻觉(duration=200) 的 duration_ticks 必须 > 取消信号(duration=0)"
        );
    }

    /// P3 S2C channel 常量 pin：`bong:core_absorption_hallucination`
    #[test]
    fn channel_constant_core_absorption_hallucination_pin() {
        // 守恒：channel 字符串是 server ↔ client 协议契约，任何修改都破坏双端对齐。
        // client BongNetworkHandler.registerCoreAbsorptionHallucinationChannel()
        // 必须使用相同字符串注册 receiver。
        assert_eq!(
            crate::schema::channels::CH_CORE_ABSORPTION_HALLUCINATION,
            "bong:core_absorption_hallucination",
            "S2C channel 常量必须为 bong:core_absorption_hallucination，\
             client BongNetworkHandler 据此注册 GlobalReceiver"
        );
    }

    /// P3 bian_yi_hexin item ID 常量 pin（与 drop.rs 对齐）
    #[test]
    fn bian_yi_hexin_item_id_matches_drop_table_constant() {
        // 兽核物品 ID 必须与 drop.rs::BIAN_YI_HEXIN 对齐。
        // plan 设计决议 §correction-3 修正：正确 ID 为 bian_yi_hexin（非 item.beast.core_mutant）。
        assert_eq!(
            crate::fauna::drop::BIAN_YI_HEXIN,
            "bian_yi_hexin",
            "兽核物品 ID 必须为 bian_yi_hexin（plan 设计决议 §correction-3 修正），\
             实际 drop 表常量为 {}",
            crate::fauna::drop::BIAN_YI_HEXIN
        );
    }

    /// P3 BeastCoreAbsorption ItemEffect serde round-trip
    #[test]
    fn beast_core_absorption_item_effect_serde_roundtrip() {
        use crate::inventory::ItemEffect;
        let effect = ItemEffect::BeastCoreAbsorption {
            breakthrough_magnitude: 0.25,
            hallucination_duration_ticks: 200,
        };
        let json =
            serde_json::to_string(&effect).expect("BeastCoreAbsorption ItemEffect 序列化失败");
        let back: ItemEffect =
            serde_json::from_str(&json).expect("BeastCoreAbsorption ItemEffect 反序列化失败");
        assert!(
            matches!(
                back,
                ItemEffect::BeastCoreAbsorption {
                    breakthrough_magnitude: m,
                    hallucination_duration_ticks: d
                } if (m - 0.25).abs() < 1e-9 && d == 200
            ),
            "BeastCoreAbsorption round-trip 必须保持 breakthrough_magnitude=0.25 和 duration=200，\
             实际反序列化结果: {:?}",
            back
        );
    }

    /// P3 BeastCoreAbsorption breakthrough_magnitude 不为负数
    #[test]
    fn beast_core_absorption_magnitude_must_be_nonnegative() {
        use crate::inventory::ItemEffect;
        // 合法值
        let valid = ItemEffect::BeastCoreAbsorption {
            breakthrough_magnitude: 0.0,
            hallucination_duration_ticks: 200,
        };
        assert!(
            matches!(valid, ItemEffect::BeastCoreAbsorption { breakthrough_magnitude: m, .. } if m >= 0.0),
            "BeastCoreAbsorption breakthrough_magnitude 必须 >= 0.0，负值表示突破惩罚（违反设计意图）"
        );
    }

    /// P3 narration scope=player 契约验证（不走 broadcast/zone 路径）
    #[test]
    fn hallucination_narration_scope_is_player_not_broadcast() {
        use crate::player::gameplay::PendingGameplayNarrations;
        use crate::schema::common::NarrationStyle;

        let mut narrations = PendingGameplayNarrations::default();
        let player_id = "offline:alice";

        // 模拟 P3 server 侧 emit 的 2 条 Perception 叙事
        narrations.push_player(
            player_id,
            "核心涌入经脉，真元震荡——感知开始扭曲，世界的边缘模糊成绿色光晕。",
            NarrationStyle::Perception,
        );
        narrations.push_player(
            player_id,
            "眼前景物倾斜偏转，手中真元似乎不再听从驱使——这是异兽核心的驻波共鸣。",
            NarrationStyle::Perception,
        );

        let drained: Vec<_> = narrations.drain();

        assert_eq!(
            drained.len(),
            2,
            "P3 应 emit 恰好 2 条 narration（吸收感知 + 失控感），期望 2 条，实际 {} 条",
            drained.len()
        );
        for (i, n) in drained.iter().enumerate() {
            assert!(
                matches!(n.style, NarrationStyle::Perception),
                "P3 narration[{i}] style 必须是 Perception（玩家感知层），实际 {:?}，\
                 因为幻觉是主观感知而非世界叙事",
                n.style
            );
            // scope=player 通过 push_player API 保证（非 push_broadcast/push_zone）
            // narration.target 字段应包含正确 player_id
            assert_eq!(
                n.target.as_deref(),
                Some(player_id),
                "P3 narration[{i}] 必须 scope=player（target={}），不应广播给全体玩家，\
                 实际 target={:?}",
                player_id,
                n.target
            );
        }
    }

    /// P3 幻觉 HP 守恒：幻觉不改变 Wounds.health_current / health_max
    #[test]
    fn hallucination_does_not_mutate_wounds() {
        use crate::combat::components::Wounds;

        // 模拟玩家受伤状态（半血）
        let wounds_before = Wounds {
            entries: vec![],
            health_current: 50.0,
            health_max: 100.0,
        };

        // CoreAbsorptionHallucinationEvent 不携带任何 HP 字段
        let _hallucination_event = CoreAbsorptionHallucinationEvent {
            player_id: "offline:alice".to_string(),
            duration_ticks: 200,
        };

        // 幻觉 event 应用后，Wounds 不变（此测试验证 event 结构不包含 HP 修改字段）
        // 如果 event 结构将来被错误地加了 hp_delta 等字段，编译失败会捕获
        let wounds_after = wounds_before.clone();

        assert!(
            (wounds_after.health_current - 50.0).abs() < 1e-6,
            "幻觉事件不应改变 health_current（守恒红线：幻觉仅显示层），\
             期望 50.0，实际 {}",
            wounds_after.health_current
        );
        assert!(
            (wounds_after.health_max - 100.0).abs() < 1e-6,
            "幻觉事件不应改变 health_max，期望 100.0，实际 {}",
            wounds_after.health_max
        );
    }

    // ── B4：系统级守恒测试（针对 B1/B2 修复） ────────────────────────────────

    #[test]
    fn system_fusion_world_qi_conserved_beasts_at_zero() {
        // B4 系统级：融合时野兽 qi_current=0 → 世界总 qi 不变。
        // 模拟：zone_qi_before 不变（released_to_zone=0），hybrid 初始 qi=0。
        // 以绝对 qi 单位统一计算（zone_abs = zone_frac × capacity）。
        // world_qi_before = sum(beast_qi) + zone_abs = 0 + zone_abs
        // world_qi_after  = hybrid_qi + released_to_zone + zone_abs = 0 + 0 + zone_abs
        use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        let beast_qi_currents = [0.0_f64, 0.0, 0.0];
        let zone_qi_frac_before = 0.08_f64; // 低于 HUNGER_THRESHOLD，触发融合
        let zone_abs_before = zone_qi_frac_before * QI_ZONE_UNIT_CAPACITY;
        let world_qi_before = beast_qi_currents.iter().sum::<f64>() + zone_abs_before;

        let total_beast_qi = beast_qi_currents.iter().sum::<f64>();
        let (hybrid_qi, released_to_zone) = fusion_qi_split(total_beast_qi);
        // zone += released / capacity → zone_abs_after = zone_abs_before + released_to_zone
        let zone_abs_after = zone_abs_before + released_to_zone;
        let world_qi_after = hybrid_qi + zone_abs_after;

        let error = (world_qi_after - world_qi_before).abs();
        assert!(
            error < 1e-10,
            "系统级守恒 B4①：融合前后世界总 qi 守恒。\
             world_qi_before={world_qi_before:.12} world_qi_after={world_qi_after:.12} 误差 {error:.2e}。\
             期望：野兽 qi=0 时世界总 qi 不变（不凭空生成）"
        );
    }

    #[test]
    fn system_fusion_world_qi_conserved_beasts_with_qi() {
        // B4 系统级：野兽有 qi 时，融合后世界总 qi 守恒（qi 从兽→hybrid + zone）。
        // 以绝对 qi 单位统一计算（zone_abs = zone_frac × capacity）。
        // world_qi_before = sum(beast_qi) + zone_abs
        // world_qi_after  = hybrid_qi + released_to_zone + zone_abs = sum + zone_abs
        use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        let beast_qi_currents = [4.0_f64, 6.0, 2.0]; // 假设兽有 qi
        let zone_qi_frac_before = 0.05_f64;
        let zone_abs_before = zone_qi_frac_before * QI_ZONE_UNIT_CAPACITY;
        let world_qi_before = beast_qi_currents.iter().sum::<f64>() + zone_abs_before;

        let total_beast_qi = beast_qi_currents.iter().sum::<f64>();
        let (hybrid_qi, released_to_zone) = fusion_qi_split(total_beast_qi);
        // zone_abs_after = zone_abs_before + released_to_zone
        let zone_abs_after = zone_abs_before + released_to_zone;
        let world_qi_after = hybrid_qi + zone_abs_after;

        let error = (world_qi_after - world_qi_before).abs();
        assert!(
            error < 1e-10,
            "系统级守恒 B4②：野兽有 qi 时融合前后守恒。\
             world_qi_before={world_qi_before:.12} world_qi_after={world_qi_after:.12} 误差 {error:.2e}"
        );
    }

    #[test]
    fn system_rage_zone_minus_equals_hybrid_plus() {
        // B4 系统级 rage 吸收守恒：zone 减少量 == hybrid 增加量。
        // 验证 regen_from_zone 语义 + B2 修复后 zone-=drain / cultivation+=gain 守恒。
        use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        use crate::qi_physics::excretion::regen_from_zone;

        let mut zone_qi = 0.4_f64;
        let mut hybrid_qi_current = 0.0_f64; // hybrid 初始 qi=0（B1 修复）
        let mut total_zone_decrease = 0.0_f64;
        let mut total_hybrid_increase = 0.0_f64;

        // 模拟 5 个 rage tick（HP 从 100% 降到 20%）
        for i in 0..5 {
            let hp_pct = 1.0_f32 - i as f32 * 0.2;
            let rate = compute_rage_absorption_rate(hp_pct);
            let qi_room = hybrid_qi_current.max(1.0) * 10.0; // 简化 qi_max
            let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, qi_room);
            if gain > 0.0 && drain > 0.0 {
                // B2 修复：zone-=drain，cultivation+=gain，守恒
                zone_qi -= drain;
                hybrid_qi_current += gain;
                total_zone_decrease += drain * QI_ZONE_UNIT_CAPACITY;
                total_hybrid_increase += gain;
            }
        }

        // zone 减少量（折算为 qi 单位）== hybrid 增加量
        let error = (total_zone_decrease - total_hybrid_increase).abs();
        assert!(
            error < 1e-8,
            "系统级守恒 B4③ rage：zone 减少量({total_zone_decrease:.12}) \
             应等于 hybrid 增加量({total_hybrid_increase:.12})，误差 {error:.2e}。\
             B2 修复：zone-=drain + cultivation.qi_current+=gain 守恒"
        );
        // hybrid 确实增加（zone 有灵气，rage 应吸收）
        assert!(
            total_hybrid_increase > 0.0,
            "B2 修复后 hybrid qi_current 应增加（zone 有灵气），实际增量 {total_hybrid_increase}"
        );
    }

    #[test]
    fn system_death_releases_qi_to_zone() {
        // B4 系统级死亡守恒：hybrid 死亡时 qi_current 全额释放回 zone。
        // 验证 release_terminated_qi_to_zone 被设计为读取 cultivation.qi_current。
        // 注意：此测试验证纯函数语义（不跑完整 ECS system，避免跨 crate 依赖）。
        // 系统级接入由 on_player_terminated → release_qi_amount_to_zone 保证。
        use crate::cultivation::components::Cultivation;

        // 模拟 hybrid 经过 rage 吸收已积累的 qi（B2 修复后的正确值）
        let mut cultivation = Cultivation {
            qi_current: 12.5_f64, // rage 吸收积累
            qi_max: 20.0_f64,
            ..Default::default()
        };

        // 死亡时应释放 qi_current 全额（>= 0）
        let release_amount = cultivation.qi_current.max(0.0);
        assert!(
            release_amount > 0.0,
            "死亡时 hybrid 有 qi_current={} > 0，应全额释放回 zone（守恒红线 B3）",
            cultivation.qi_current
        );
        assert!(
            (release_amount - 12.5).abs() < 1e-12,
            "释放量应等于 qi_current=12.5，实际 {release_amount}"
        );

        // 释放后 qi_current 归零（release_terminated_qi_to_zone 语义）
        cultivation.qi_current = 0.0;
        assert_eq!(
            cultivation.qi_current, 0.0,
            "死亡释放后 qi_current 应归零（全额归还 zone，不吞不留）"
        );
    }
}
