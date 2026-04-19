//! plan-lingtian-v1 §4 — 事件总线（client/agent → server，server → world）。
//!
//! P1 切片：开垦 / 翻新两类。生长 / 补灵 / 收获 / 偷菜 / 偷灵留待 P2+。

use valence::prelude::{bevy_ecs, BlockPos, Entity, Event};

use crate::botany::PlantId;

use super::hoe::HoeKind;
use super::session::SessionMode;
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
}

/// 开垦完成（session.tick 完成后由 system 派发）。
#[derive(Debug, Clone, Event)]
pub struct TillCompleted {
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
    pub player: Entity,
    pub pos: BlockPos,
    pub plant_id: PlantId,
}
