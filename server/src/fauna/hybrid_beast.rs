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
    bevy_ecs, App, Commands, Component, DVec3, Despawned, Entity, EntityLayerId, Event,
    EventReader, EventWriter, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Update,
    With, Without,
};

use crate::combat::components::Wounds;
use crate::cultivation::components::{Cultivation, Realm};
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
    // Without<Despawned>：已标记待 despawn 的组件兽（融合后 insert(Despawned) 仍存活一帧）
    // 不应再被选为融合候选，否则会对悬空实体重复发 HybridBeastFormationEvent。
    (With<NpcMarker>, Without<Despawned>),
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
        let mut runtime = npc_runtime_bundle(hybrid_entity, NpcArchetype::Beast, Realm::Awaken);
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
            // NpcArchetype 由 `runtime`（npc_runtime_bundle）提供，此处不可再显式加，
            // 否则同一 insert bundle 含重复组件 → Bevy 运行时 panic（缝合兽融合即崩服）。
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
        // 组件兽是 spawn_beast_npc_at 的 MarkerEntityBundle 层实体，须经 Despawned 标记移除
        // （valence 先发客户端移除包），裸 .despawn() 会让 entity layer 索引悬空触发 panic。
        for entity in &component_entities {
            commands.entity(*entity).insert(Despawned);
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
    if !tick.is_multiple_of(RAGE_TICK_INTERVAL) {
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
