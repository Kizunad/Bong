//! plan-mineral-v1 §2.2 — `BlockBreakEvent`（valence `DiggingEvent`）监听器。
//!
//! 流程（plan §2.2）：
//!  1. 读取 valence `DiggingEvent` 的"挖掘完成"语义（Stop = Survival 完成）。
//!  2. 反查 `MineralOreIndex` 看 BlockPos 是否对应一个 `MineralOreNode`。
//!  3. 是 → 不让 vanilla loot table 走默认掉落，主动发 `MineralDropEvent`，
//!     让 inventory listener 按 mineral_id 写入物品 NBT。
//!  4. `remaining_units` 减一；归零则发 `MineralExhaustedEvent`、移除 entity 与 index。
//!  5. 品阶 ≥ 3 按概率 5% / 15% / 30% 发 `KarmaFlagIntent` 给天道 agent。
//!
//! 与 `inventory::DroppedItemEvent` 解耦：本系统只发 mineral_id 语义的 drop 事件，
//! 由 inventory 侧的 listener 把 mineral_id 序列化到新建 InventoryItem 的 NBT。

use valence::prelude::{
    Commands, DiggingEvent, DiggingState, EventReader, EventWriter, Query, ResMut,
};

use super::components::{MineralOreIndex, MineralOreNode};
use super::events::{KarmaFlagIntent, MineralDropEvent, MineralExhaustedEvent};
use super::types::MineralRarity;

/// plan-mineral-v1 §3 — 极品矿脉劫气概率（worldview §七）。
///
/// tier 1/2 = 0%（不推 KarmaFlag），tier 3 = 15%，tier 4 = 30%。
/// 概率值由 listener 直接写入 `KarmaFlagIntent.probability`，下游 agent 决定是否触发。
const KARMA_PROBABILITY_FAN: f32 = 0.0;
const KARMA_PROBABILITY_LING: f32 = 0.0;
const KARMA_PROBABILITY_XI: f32 = 0.15;
const KARMA_PROBABILITY_YI: f32 = 0.30;

pub fn karma_probability(rarity: MineralRarity) -> f32 {
    match rarity {
        MineralRarity::Fan => KARMA_PROBABILITY_FAN,
        MineralRarity::Ling => KARMA_PROBABILITY_LING,
        MineralRarity::Xi => KARMA_PROBABILITY_XI,
        MineralRarity::Yi => KARMA_PROBABILITY_YI,
    }
}

pub fn handle_block_break_for_mineral(
    mut commands: Commands,
    mut digs: EventReader<DiggingEvent>,
    mut nodes: Query<&mut MineralOreNode>,
    mut index: ResMut<MineralOreIndex>,
    mut drop_events: EventWriter<MineralDropEvent>,
    mut exhausted_events: EventWriter<MineralExhaustedEvent>,
    mut karma_events: EventWriter<KarmaFlagIntent>,
) {
    for event in digs.read() {
        // Survival 模式 Stop = 挖掘动画走完；plan §2.2 重写 drop 必须等 Stop 才触发，
        // 避免 Start/Abort 误判。Creative 模式（Start）的特例由 worldview 暂不支持。
        if event.state != DiggingState::Stop {
            continue;
        }

        let Some(entity) = index.lookup(event.position) else {
            // 该方块不是矿脉 — 走 vanilla loot table（其他模块或默认行为决定）
            continue;
        };

        let Ok(mut node) = nodes.get_mut(entity) else {
            // index 与 entity 失同步 — 清掉 stale 项以自愈
            tracing::warn!(
                target: "bong::mineral",
                "MineralOreIndex stale entry at {:?} — removing",
                event.position
            );
            index.remove(event.position);
            continue;
        };

        let mineral_id = node.mineral_id;

        drop_events.send(MineralDropEvent {
            player: event.client,
            mineral_id,
            position: event.position,
        });

        let probability = karma_probability(mineral_id.rarity());
        if probability > 0.0 {
            karma_events.send(KarmaFlagIntent {
                player: event.client,
                mineral_id,
                position: event.position,
                probability,
            });
        }

        node.remaining_units = node.remaining_units.saturating_sub(1);
        if node.remaining_units == 0 {
            exhausted_events.send(MineralExhaustedEvent {
                mineral_id,
                position: event.position,
            });
            index.remove(event.position);
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::MineralId;
    use super::*;

    #[test]
    fn karma_probability_zero_for_low_tier() {
        assert_eq!(karma_probability(MineralRarity::Fan), 0.0);
        assert_eq!(karma_probability(MineralRarity::Ling), 0.0);
    }

    #[test]
    fn karma_probability_nonzero_for_tier_3_and_4() {
        assert!(karma_probability(MineralRarity::Xi) > 0.0);
        assert!(karma_probability(MineralRarity::Yi) > karma_probability(MineralRarity::Xi));
    }

    #[test]
    fn karma_probability_per_tier_aligns_with_plan() {
        // plan §3 第 2 条：5% → 30% — 实装锚点：tier 3=15%, tier 4=30%
        assert_eq!(karma_probability(MineralId::SuiTie.rarity()), 0.15);
        assert_eq!(karma_probability(MineralId::KuJin.rarity()), 0.30);
        assert_eq!(karma_probability(MineralId::LingShiYi.rarity()), 0.30);
    }
}
