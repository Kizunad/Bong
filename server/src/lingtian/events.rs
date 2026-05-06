//! plan-lingtian-v1 §4 — 事件总线（client/agent → server，server → world）。
//!
//! P1 切片：开垦 / 翻新两类。生长 / 补灵 / 收获 / 偷菜 / 偷灵留待 P2+。

use valence::prelude::{bevy_ecs, BlockPos, Entity, Event};

use crate::botany::PlantId;

use super::environment::PlotEnvironment;
use super::hoe::HoeKind;
use super::pressure::PressureLevel;
use super::session::{ReplenishSource, SessionMode};
use super::terrain::TerrainKind;

/// 玩家请求开垦某方块。
///
/// `terrain` 由调用方（valence block ↔ TerrainKind 适配层）填入；session 层
/// 拒绝不合规地形。本字段独立于 hoe / pos 的目的：把"读方块种类"职责留给
/// 上层适配，本模块单测无需 mock valence world。
///
/// `hoe_instance_id` 指明用哪把具体锄头（玩家可能背两把同档不同耐久）；
/// server 验"主手 ItemInstance.instance_id == hoe_instance_id"，否则拒。
/// HoeKind 由该 instance 的 template_id 反查得出（无需 client 重复传）。
#[derive(Debug, Clone, Event)]
pub struct StartTillRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub hoe_instance_id: u64,
    pub mode: SessionMode,
    pub terrain: TerrainKind,
    /// plot 环境修饰（plan §1.1：水源 / 湿地 / 聚灵阵）。Default 为 base
    /// → cap 1.0。由 valence world ↔ env 适配层填，session 单测可省。
    pub environment: PlotEnvironment,
}

/// 开垦完成（session.tick 完成后由 system 派发）。
#[derive(Debug, Clone, Event)]
pub struct TillCompleted {
    /// 历史字段名；完成事件可由玩家或散修 NPC actor 触发。
    pub player: Entity,
    pub pos: BlockPos,
    pub hoe: HoeKind,
    pub hoe_instance_id: u64,
}

/// 玩家请求翻新某 plot。仅当 plot.is_barren() 才生效。
/// `hoe_instance_id` 同 [`StartTillRequest`]。
#[derive(Debug, Clone, Event)]
pub struct StartRenewRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub hoe_instance_id: u64,
}

#[derive(Debug, Clone, Event)]
pub struct RenewCompleted {
    /// 历史字段名；完成事件可由玩家或散修 NPC actor 触发。
    pub player: Entity,
    pub pos: BlockPos,
    pub hoe: HoeKind,
    pub hoe_instance_id: u64,
}

/// 玩家请求在某 plot 种下指定 plant（plan §1.2.3）。
///
/// 调用方（client UI）应已通过 SeedRegistry 选定 plant 并验证背包有种子；
/// server 侧在 `handle_start_planting` 复验所有前置（plot 空且未贫瘠 / 玩家
/// 背包有种子 / SeedRegistry 已知 plant_id）。
#[derive(Debug, Clone, Event)]
pub struct StartPlantingRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub plant_id: PlantId,
}

#[derive(Debug, Clone, Event)]
pub struct PlantingCompleted {
    /// 历史字段名；完成事件可由玩家或散修 NPC actor 触发。
    pub player: Entity,
    pub pos: BlockPos,
    pub plant_id: PlantId,
}

/// 玩家请求收获某熟 plot（plan §1.5）。`mode` 控制 manual 2.5s / auto 7s。
/// auto 模式在 client 层应已校验 herbalism Lv.3+；server 信任请求。
#[derive(Debug, Clone, Event)]
pub struct StartHarvestRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub mode: SessionMode,
}

#[derive(Debug, Clone, Event)]
pub struct HarvestCompleted {
    /// 历史字段名；完成事件可由玩家或散修 NPC actor 触发。
    pub player: Entity,
    pub pos: BlockPos,
    pub plant_id: PlantId,
    /// 是否同时掉落 1 颗种子（按 PlantRarity::seed_drop_rate）。
    pub seed_dropped: bool,
}

/// 玩家请求补灵某 plot（plan §1.4）。
#[derive(Debug, Clone, Event)]
pub struct StartReplenishRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub source: ReplenishSource,
}

#[derive(Debug, Clone, Event)]
pub struct ReplenishCompleted {
    /// 历史字段名；完成事件可由玩家或散修 NPC actor 触发。
    pub player: Entity,
    pub pos: BlockPos,
    pub source: ReplenishSource,
    /// 实际灌入 plot_qi 的量（cap 之内的部分）。
    pub plot_qi_added: f32,
    /// 溢出回馈到 zone qi 的量（plan §1.4：来源材料不退）。
    pub overflow_to_zone: f32,
}

/// plan-alchemy-recycle-v1 §5 P4 — plot 首次跨过杂染警戒线时写入 world_state
/// recent_events，供天道叙事上下文消费。
#[derive(Debug, Clone, Event)]
pub struct DyeContaminationWarning {
    pub player: Entity,
    pub pos: BlockPos,
    pub source: ReplenishSource,
    pub dye_contamination: f32,
    pub added: f32,
}

/// plan §5.1 — zone_pressure 跨入更高档时由 lingtian 发出（仅"上升"边沿，
/// 不发"回落"事件，避免噪音）。下游（npc/agent）按 level 接：
///   * Low  → 天道 narration
///   * Mid  → 异变兽刷新率 +30%
///   * High → 该 zone plot_qi 已被本系统清零；下游可加 spawn 道伥
#[derive(Debug, Clone, Event)]
pub struct ZonePressureCrossed {
    pub zone: String,
    pub level: PressureLevel,
    pub raw_pressure: f32,
}

/// plan §1.7 — 偷灵：把目标 plot 的 plot_qi 全部抽走，80% 注入操作者，
/// 20% 散逸到 zone（保持灵气零和）。本切片不限制 owner 自吸（无意义但允许）；
/// 偷灵双方 LifeRecord 仅在 owner != player 时记。
#[derive(Debug, Clone, Event)]
pub struct StartDrainQiRequest {
    pub player: Entity,
    pub pos: BlockPos,
}

#[derive(Debug, Clone, Event)]
pub struct DrainQiCompleted {
    pub player: Entity,
    pub pos: BlockPos,
    pub plot_qi_drained: f32,
    pub qi_to_player: f32,
    pub qi_to_zone: f32,
}
