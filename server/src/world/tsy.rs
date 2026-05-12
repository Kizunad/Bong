//! plan-tsy-zone-v1 §1.3 / §3.2 — 活坍缩渊（TSY）的玩家 presence + 裂缝 POI 数据结构。
//!
//! 仅类型/component 定义；drain tick / portal system / entry filter 在各自模块中实现。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, DVec3};

use super::dimension::DimensionKind;

pub use super::rift_portal::{PortalDirection, RiftKind, RiftPortal, TickWindow};

/// 跨位面锚点（dim + 位面内部坐标）。
///
/// `plan-tsy-zone-v1 §1.3` / `plan-tsy-dimension-v1 §3` — 出关 / 入场跨位面传送
/// 的目标全部以本类型表示，避免 (DimensionKind, DVec3) 二元组到处分裂。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimensionAnchor {
    pub dimension: DimensionKind,
    /// 序列化为 `[f64; 3]`，与 schema/src/tsy.ts 的 wire 形态对齐。
    #[serde(with = "dvec3_array_serde")]
    pub pos: DVec3,
}

mod dvec3_array_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use valence::prelude::DVec3;

    pub fn serialize<S: Serializer>(value: &DVec3, ser: S) -> Result<S::Ok, S::Error> {
        [value.x, value.y, value.z].serialize(ser)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<DVec3, D::Error> {
        let arr: [f64; 3] = Deserialize::deserialize(de)?;
        Ok(DVec3::new(arr[0], arr[1], arr[2]))
    }
}

/// 玩家在 TSY 内的 ECS 状态。
///
/// 生命周期：
/// - 玩家踏进 Entry 裂缝 → `tsy_entry_portal_system` `commands.insert` 本组件
/// - 玩家踏进 Exit 裂缝 / 在 TSY 内死亡 → `commands.remove` 本组件
/// - 死亡结算（`plan-tsy-loot-v1` P1）会读 `entry_inventory_snapshot` 区分
///   "秘境所得" vs "原带物"
#[allow(dead_code)] // 字段全部由 portal / loot / lifecycle plan 消费；P0 仅落定义。
#[derive(Component, Debug, Clone)]
pub struct TsyPresence {
    /// 玩家所在的 TSY 系列 id（如 `"tsy_lingxu_01"`）—— 由 `Zone::tsy_family_id` 派生。
    pub family_id: String,
    /// 进入 tick（用于计算 duration、drain total 等）。
    pub entered_at_tick: u64,
    /// 入场时 inventory 内所有 instance_id；`plan-tsy-loot-v1` 死亡结算用此区分秘境所得。
    pub entry_inventory_snapshot: Vec<u64>,
    /// 出关锚点：传回到哪个位面 + 哪个坐标。
    /// 通常 = `(Overworld, 触发 Entry 裂缝的主世界坐标 + (0,1,0))`。
    /// 塌缩时若主世界锚点失效 → P2 lifecycle 决定 fallback（出生点 / 灵龛）。
    pub return_to: DimensionAnchor,
}

/// 容器 POI marker（plan-tsy-worldgen-v1 §1.1，本 plan 落最简 schema）。
///
/// archetype/lock 字符串约定（在 plan §1.1 表）：
/// - archetype: `dry_corpse` / `skeleton` / `storage_pouch` / `stone_casket` / `relic_core`
/// - lock: `stone_key` / `jade_seal` / `array_sigil` / None=unlocked
///
/// **Schema 故意保留 String** —— 严格 enum 化属 `plan-tsy-container-v1` (P3) 责任，
/// P3 active 阶段会扩字段（loot_pool 命中表、open animation、locked drop chain 等）；
/// 本 plan 仅承担"POI → marker entity"通道，让 P3 可以直接 query 已 spawn 的 marker。
#[allow(dead_code)] // 字段由未来的 plan-tsy-container-v1 系统消费。
#[derive(Component, Debug, Clone)]
pub struct LootContainer {
    /// 形态档次：见 archetype 字符串约定。Worldgen consumer 解析 POI tag
    /// `archetype:X`；未知值 → log warn + skip spawn（plan §1.4）。
    pub archetype: String,
    /// 钥匙约束。POI tag `locked:X`；缺省 = unlocked。
    pub lock: Option<String>,
    /// 战利品池覆写。POI tag `loot_pool:X`；缺省 = 用 archetype 默认池（P3 决定）。
    pub loot_pool: Option<String>,
}

/// NPC anchor POI marker（plan-tsy-worldgen-v1 §1.1）。
///
/// archetype/trigger 字符串约定：
/// - archetype: `daoxiang` / `zhinian` / `sentinel` / `fuya` / `ancient_sentinel`
///   (P4 命名 TBD 用 ancient_sentinel 占位；未知值 consumer 走 warn+skip)
/// - trigger: `on_enter` / `on_relic_touched` / `always`
///
/// 行为系统（aggro 半径、AI tree、leash 物理）属 `plan-tsy-hostile-v1` (P4)。
#[allow(dead_code)]
#[derive(Component, Debug, Clone)]
pub struct NpcAnchor {
    pub archetype: String,
    pub trigger: String,
    pub leash_radius: f64,
}

/// Relic-core 槽位 POI marker（plan-tsy-worldgen-v1 §1.1）。
///
/// 槽位数 1..=8（worldgen consumer clamp）。`plan-tsy-container-v1` (P3) 决定
/// "remove relic" 流程（取走核心 → 触发塌缩事件 → P2 lifecycle 接手）。
#[allow(dead_code)]
#[derive(Component, Debug, Clone)]
pub struct RelicCoreSlot {
    pub slot_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // P4 records extract-point PvP math before full live encounter telemetry exists.
pub struct TsyExtractPvpProfile {
    pub waiting_players: usize,
    pub extract_ticks: u64,
    pub pvp_window_ticks: u64,
    pub race_out: bool,
}

impl TsyExtractPvpProfile {
    #[allow(dead_code)] // See TsyExtractPvpProfile.
    pub fn prisoner_dilemma_active(&self) -> bool {
        self.waiting_players >= 2 && self.pvp_window_ticks > 0
    }
}

#[allow(dead_code)] // See TsyExtractPvpProfile.
pub fn pvp_extract_point_profile(
    waiting_players: usize,
    extract_ticks: u64,
    race_out: bool,
) -> TsyExtractPvpProfile {
    let pvp_window_ticks = if waiting_players < 2 {
        0
    } else if race_out {
        extract_ticks.min(3 * 20)
    } else {
        extract_ticks.min(15 * 20)
    };
    TsyExtractPvpProfile {
        waiting_players,
        extract_ticks,
        pvp_window_ticks,
        race_out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_anchor_round_trips_through_serde_json() {
        let anchor = DimensionAnchor {
            dimension: DimensionKind::Tsy,
            pos: DVec3::new(1.5, 80.0, -3.25),
        };
        let json = serde_json::to_string(&anchor).expect("serialize");
        // pos 必须是 [x, y, z] 数组形态以匹配 wire schema。
        assert!(json.contains("[1.5,80.0,-3.25]"), "got: {json}");
        let parsed: DimensionAnchor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, anchor);
    }

    #[test]
    fn dimension_anchor_default_dimension_overworld_round_trip() {
        let anchor = DimensionAnchor {
            dimension: DimensionKind::Overworld,
            pos: DVec3::new(0.0, 64.0, 0.0),
        };
        let json = serde_json::to_string(&anchor).expect("serialize");
        let parsed: DimensionAnchor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.dimension, DimensionKind::Overworld);
        assert_eq!(parsed.pos, DVec3::new(0.0, 64.0, 0.0));
    }

    #[test]
    fn tsy_presence_default_field_shape() {
        let presence = TsyPresence {
            family_id: "tsy_lingxu_01".to_string(),
            entered_at_tick: 100,
            entry_inventory_snapshot: vec![1, 2, 3],
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 65.0, 0.0),
            },
        };
        assert_eq!(presence.family_id, "tsy_lingxu_01");
        assert_eq!(presence.entered_at_tick, 100);
        assert_eq!(presence.entry_inventory_snapshot.len(), 3);
        assert_eq!(presence.return_to.dimension, DimensionKind::Overworld);
    }

    #[test]
    fn rift_portal_can_be_constructed_for_entry_and_exit() {
        let entry = RiftPortal::entry(
            "tsy_lingxu_01".to_string(),
            DimensionAnchor {
                dimension: DimensionKind::Tsy,
                pos: DVec3::new(50.0, 80.0, 50.0),
            },
            1.5,
        );
        assert_eq!(entry.direction, PortalDirection::Entry);
        assert_eq!(entry.target.dimension, DimensionKind::Tsy);

        let exit = RiftPortal::exit(
            "tsy_lingxu_01".to_string(),
            DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 66.0, 0.0),
            },
            1.5,
            RiftKind::MainRift,
        );
        assert_eq!(exit.direction, PortalDirection::Exit);
        assert_eq!(exit.target.dimension, DimensionKind::Overworld);
    }

    #[test]
    fn pvp_at_extract_point() {
        let normal = pvp_extract_point_profile(2, 12 * 20, false);
        assert!(normal.prisoner_dilemma_active());
        assert_eq!(normal.pvp_window_ticks, 12 * 20);

        let race_out = pvp_extract_point_profile(3, 12 * 20, true);
        assert!(race_out.prisoner_dilemma_active());
        assert_eq!(race_out.pvp_window_ticks, 3 * 20);

        let alone = pvp_extract_point_profile(1, 12 * 20, false);
        assert!(!alone.prisoner_dilemma_active());
        assert_eq!(alone.pvp_window_ticks, 0);
    }

    #[test]
    fn pvp_extract_point_boundaries() {
        let zero_ticks = pvp_extract_point_profile(2, 0, false);
        assert!(!zero_ticks.prisoner_dilemma_active());
        assert_eq!(zero_ticks.pvp_window_ticks, 0);

        for (extract_ticks, expected_window) in [(59, 59), (60, 60), (61, 60)] {
            let profile = pvp_extract_point_profile(2, extract_ticks, true);
            assert!(
                profile.prisoner_dilemma_active(),
                "race-out should stay active at threshold input {extract_ticks}"
            );
            assert_eq!(
                profile.pvp_window_ticks, expected_window,
                "race-out pvp window should clamp at 60 ticks"
            );
        }

        for (extract_ticks, expected_window) in [(299, 299), (300, 300), (301, 300)] {
            let profile = pvp_extract_point_profile(2, extract_ticks, false);
            assert!(
                profile.prisoner_dilemma_active(),
                "normal extract should stay active at threshold input {extract_ticks}"
            );
            assert_eq!(
                profile.pvp_window_ticks, expected_window,
                "normal extract pvp window should clamp at 300 ticks"
            );
        }
    }
}
