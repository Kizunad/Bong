//! plan-lingtian-v1 §4 — 事件总线（client/agent → server，server → world）。
//!
//! P1 切片：开垦 / 翻新两类。生长 / 补灵 / 收获 / 偷菜 / 偷灵留待 P2+。

use valence::prelude::{bevy_ecs, BlockPos, Entity, Event};

use super::hoe::HoeKind;
use super::session::SessionMode;
use super::terrain::TerrainKind;

/// 玩家请求开垦某方块。
///
/// `terrain` 由调用方（valence block ↔ TerrainKind 适配层）填入；session 层
/// 拒绝不合规地形。本字段独立于 hoe / pos 的目的：把"读方块种类"职责留给
/// 上层适配，本模块单测无需 mock valence world。
#[derive(Debug, Clone, Event)]
pub struct StartTillRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub hoe: HoeKind,
    pub mode: SessionMode,
    pub terrain: TerrainKind,
}

/// 开垦完成（session.tick 完成后由 system 派发）。
#[derive(Debug, Clone, Event)]
pub struct TillCompleted {
    pub player: Entity,
    pub pos: BlockPos,
    pub hoe: HoeKind,
}

/// 玩家请求翻新某 plot。仅当 plot.is_barren() 才生效。
#[derive(Debug, Clone, Event)]
pub struct StartRenewRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub hoe: HoeKind,
}

#[derive(Debug, Clone, Event)]
pub struct RenewCompleted {
    pub player: Entity,
    pub pos: BlockPos,
    pub hoe: HoeKind,
}
