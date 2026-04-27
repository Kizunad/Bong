//! plan-mineral-v1 §3 / §7 — mineral 相关 Bevy events。

use valence::prelude::{bevy_ecs, BlockPos, Entity, Event};

use super::types::MineralId;
use crate::world::dimension::DimensionKind;

/// 玩家神识感知触发（plan §3）— 修为 ≥ 凝脉 时右键矿块。
///
/// 由 client request 路径转发；listener 侧应反查 `MineralOreIndex` + `MineralRegistry`
/// 把 mineral_id 与剩余储量送回 client tooltip / chat。
#[derive(Debug, Clone, Copy, Event)]
pub struct MineralProbeIntent {
    pub player: Entity,
    pub dimension: DimensionKind,
    pub position: BlockPos,
}

/// 神识感知回执（plan §3）— 由 server 侧 resolver 产出，网络层后续可转 chat/HUD。
#[derive(Debug, Clone, Event)]
pub struct MineralProbeResponse {
    pub player: Entity,
    pub position: BlockPos,
    pub result: MineralProbeResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MineralProbeResult {
    Denied {
        reason: MineralProbeDenialReason,
    },
    Found {
        mineral_id: MineralId,
        remaining_units: u32,
        display_name_zh: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MineralProbeDenialReason {
    RealmTooLow,
    OutOfRange,
    NotMineralOre,
    StaleOreIndex,
    MineralNotRegistered,
}

/// 矿脉单方块耗尽 — `BlockBreakEvent` 处理后，若剩余储量降至 0 则触发。
///
/// persistence 侧（plan §M6）订阅此事件落盘 data/minerals/exhausted.json。
#[derive(Debug, Clone, Copy, Event)]
pub struct MineralExhaustedEvent {
    pub mineral_id: MineralId,
    pub position: BlockPos,
}

/// 极品矿脉触发劫气标记（plan §3 第 2 条 / worldview §七 天道劫气章）。
///
/// 品阶 ≥ 3 的 mineral 被挖出时按概率 5%-30% 推此事件给天道 agent。
/// 实际推送给 LLM 由 network bridge 系统消费。
#[derive(Debug, Clone, Copy, Event)]
pub struct KarmaFlagIntent {
    pub player: Entity,
    pub mineral_id: MineralId,
    pub position: BlockPos,
    /// 概率档位（0.05 - 0.30）— 由 listener 按 mineral.tier 计算。
    pub probability: f32,
}

/// mineral_id 的物品 drop 事件 — `BlockBreakEvent` 处理后由 server 主动 spawn。
///
/// inventory 侧订阅此事件，把 mineral_id NBT 写入新 InventoryItem。
/// 与 `inventory::DroppedItemEvent` 互补：那个是从 inventory 抛出，此事件是从 world 进入。
#[derive(Debug, Clone, Copy, Event)]
pub struct MineralDropEvent {
    pub player: Entity,
    pub mineral_id: MineralId,
    pub position: BlockPos,
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::App;

    #[test]
    fn events_register_into_app_without_panic() {
        // 单独跑确保 Event derive macro 正常 + register 路径 OK
        let mut app = App::new();
        app.add_event::<MineralProbeIntent>()
            .add_event::<MineralProbeResponse>()
            .add_event::<MineralExhaustedEvent>()
            .add_event::<KarmaFlagIntent>()
            .add_event::<MineralDropEvent>();
    }
}
