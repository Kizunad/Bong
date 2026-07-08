//! plan-mineral-v1 §3 / §7 — mineral 相关 Bevy events。

use valence::message::SendMessage;
use valence::prelude::{bevy_ecs, BlockPos, Client, Entity, Event, EventReader, Query};

use super::types::MineralId;
use crate::world::dimension::DimensionKind;

pub const MSG_MINERAL_INVALID_FOR_FORGE: &str = "mineral.invalid_for_forge";
pub const MSG_MINERAL_INVALID_FOR_ALCHEMY: &str = "mineral.invalid_for_alchemy";
pub const MSG_MINERAL_UNKNOWN_ID: &str = "mineral.unknown_id";
pub const MSG_FORGE_TIER_MISMATCH: &str = "forge.tier_mismatch";
pub const MSG_MINERAL_PICKAXE_TIER_MISMATCH: &str = "mineral.pickaxe_tier_mismatch";
/// plan-forge-session-entry-wiring-v1 §4.1#1 — 起炉时未习得该图谱。
pub const MSG_FORGE_BLUEPRINT_NOT_LEARNED: &str = "forge.blueprint_not_learned";
/// plan-forge-session-entry-wiring-v1 §4.1#4 — 起炉时背包持有量核验未通过。
pub const MSG_FORGE_MATERIALS_INSUFFICIENT: &str = "forge.materials_insufficient";
/// plan-forge-session-entry-wiring-v1 修复轮 — 砧上已有进行中会话（对齐 alchemy is_busy）。
pub const MSG_FORGE_STATION_BUSY: &str = "forge.station_busy";

#[derive(Debug, Clone, PartialEq, Eq, Event)]
pub struct MineralFeedbackEvent {
    pub player: Entity,
    pub message_id: &'static str,
    pub text: String,
}

impl MineralFeedbackEvent {
    pub fn invalid_for_forge(player: Entity, vanilla_name: impl AsRef<str>) -> Self {
        Self {
            player,
            message_id: MSG_MINERAL_INVALID_FOR_FORGE,
            text: format!("此为凡俗{}，不可入炉", vanilla_name.as_ref()),
        }
    }

    pub fn invalid_for_alchemy(player: Entity, vanilla_name: impl AsRef<str>) -> Self {
        Self {
            player,
            message_id: MSG_MINERAL_INVALID_FOR_ALCHEMY,
            text: format!("此为凡俗{}，不可入药", vanilla_name.as_ref()),
        }
    }

    pub fn unknown_for_alchemy(player: Entity) -> Self {
        Self {
            player,
            message_id: MSG_MINERAL_UNKNOWN_ID,
            text: "此物未经矿录，无法入药".to_string(),
        }
    }

    pub fn unknown_for_forge(player: Entity) -> Self {
        Self {
            player,
            message_id: MSG_MINERAL_UNKNOWN_ID,
            text: "此物未经矿录，无法入炉".to_string(),
        }
    }

    pub fn forge_tier_mismatch(
        player: Entity,
        furnace_name: impl AsRef<str>,
        material_name: impl AsRef<str>,
        required_tier: u8,
    ) -> Self {
        Self {
            player,
            message_id: MSG_FORGE_TIER_MISMATCH,
            text: format!(
                "{}炼不动{}，需升炉品至 {}",
                furnace_name.as_ref(),
                material_name.as_ref(),
                required_tier
            ),
        }
    }

    /// plan-forge-session-entry-wiring-v1 §4.1#1 — 起炉时该图谱尚未习得。
    pub fn forge_blueprint_not_learned(player: Entity, blueprint_name: impl AsRef<str>) -> Self {
        Self {
            player,
            message_id: MSG_FORGE_BLUEPRINT_NOT_LEARNED,
            text: format!("尚未习得图谱《{}》，无法起炉", blueprint_name.as_ref()),
        }
    }

    /// plan-forge-session-entry-wiring-v1 §4.1#4 — 起炉时背包持有量不足以覆盖声明的投料。
    /// `deficits` = (display_name_zh, have, need) 列表，由 `consume_forge_materials_atomic`
    /// 的 `Err` 分支经 MineralRegistry 转译成中文名后传入（玩家可读，不漏内部 id）。
    pub fn forge_materials_insufficient(player: Entity, deficits: &[(String, u32, u32)]) -> Self {
        let detail = deficits
            .iter()
            .map(|(material, have, need)| format!("{material} {have}/{need}"))
            .collect::<Vec<_>>()
            .join("、");
        Self {
            player,
            message_id: MSG_FORGE_MATERIALS_INSUFFICIENT,
            text: format!("材料不足：{detail}，起炉失败"),
        }
    }

    /// plan-forge-session-entry-wiring-v1 修复轮 — 砧上已有进行中会话时拒绝再次起炉，
    /// 防止双扣料 + 孤儿会话（对齐 alchemy `is_busy()` 守卫）。
    pub fn forge_station_busy(player: Entity) -> Self {
        Self {
            player,
            message_id: MSG_FORGE_STATION_BUSY,
            text: "这座砧上已有进行中的锻造，须待其结束".to_string(),
        }
    }

    pub fn pickaxe_tier_mismatch(
        player: Entity,
        pickaxe_name: impl AsRef<str>,
        material_name: impl AsRef<str>,
        required_tier: u8,
    ) -> Self {
        Self {
            player,
            message_id: MSG_MINERAL_PICKAXE_TIER_MISMATCH,
            text: format!(
                "{}掘不动{}，需镐品至 {}",
                pickaxe_name.as_ref(),
                material_name.as_ref(),
                required_tier
            ),
        }
    }
}

pub fn emit_mineral_feedback_chat(
    mut events: EventReader<MineralFeedbackEvent>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.player) else {
            tracing::debug!(
                target: "bong::mineral",
                "mineral feedback {} dropped because player {:?} has no Client",
                event.message_id,
                event.player
            );
            continue;
        };
        client.send_chat_message(event.text.clone());
    }
}

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
            .add_event::<MineralDropEvent>()
            .add_event::<MineralFeedbackEvent>();
    }

    #[test]
    fn mineral_feedback_message_ids_are_pinned() {
        let player = Entity::from_raw(1);
        let forge = MineralFeedbackEvent::invalid_for_forge(player, "iron_ingot");
        assert_eq!(forge.message_id, MSG_MINERAL_INVALID_FOR_FORGE);
        assert_eq!(forge.text, "此为凡俗iron_ingot，不可入炉");

        let alchemy = MineralFeedbackEvent::invalid_for_alchemy(player, "iron_ingot");
        assert_eq!(alchemy.message_id, MSG_MINERAL_INVALID_FOR_ALCHEMY);
        assert_eq!(alchemy.text, "此为凡俗iron_ingot，不可入药");

        let unknown = MineralFeedbackEvent::unknown_for_alchemy(player);
        assert_eq!(unknown.message_id, MSG_MINERAL_UNKNOWN_ID);
        assert_eq!(unknown.text, "此物未经矿录，无法入药");

        let tier = MineralFeedbackEvent::forge_tier_mismatch(player, "凡铁炉", "灵铁", 2);
        assert_eq!(tier.message_id, MSG_FORGE_TIER_MISMATCH);
        assert_eq!(tier.text, "凡铁炉炼不动灵铁，需升炉品至 2");

        let pickaxe = MineralFeedbackEvent::pickaxe_tier_mismatch(player, "凡镐", "灵铁", 2);
        assert_eq!(pickaxe.message_id, MSG_MINERAL_PICKAXE_TIER_MISMATCH);
        assert_eq!(pickaxe.text, "凡镐掘不动灵铁，需镐品至 2");

        let not_learned =
            MineralFeedbackEvent::forge_blueprint_not_learned(player, "青锋剑（测试）");
        assert_eq!(not_learned.message_id, MSG_FORGE_BLUEPRINT_NOT_LEARNED);
        assert_eq!(not_learned.text, "尚未习得图谱《青锋剑（测试）》，无法起炉");

        // 契约：调用方（handle_start_forge_requests）经 MineralRegistry 转译后传入中文名，
        // 玩家回执不得漏内部 canonical id。
        let insufficient = MineralFeedbackEvent::forge_materials_insufficient(
            player,
            &[("凡铁".to_string(), 2, 4), ("杂钢".to_string(), 0, 1)],
        );
        assert_eq!(insufficient.message_id, MSG_FORGE_MATERIALS_INSUFFICIENT);
        assert_eq!(insufficient.text, "材料不足：凡铁 2/4、杂钢 0/1，起炉失败");
    }

    #[test]
    fn forge_materials_insufficient_empty_deficits_still_produces_message() {
        // 边界：deficits 空切片（理论上不该被调用，但函数须对此输入保持总-空防御）。
        let player = Entity::from_raw(1);
        let empty = MineralFeedbackEvent::forge_materials_insufficient(player, &[]);
        assert_eq!(empty.message_id, MSG_FORGE_MATERIALS_INSUFFICIENT);
        assert_eq!(empty.text, "材料不足：，起炉失败");
    }
}
