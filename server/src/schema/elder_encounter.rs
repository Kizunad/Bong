//! plan-dying-elder-v1 P3 — ElderEncounterEventV1 IPC schema.
//!
//! server → agent 叙事频道 `bong:elder_encounter`（`CH_ELDER_ENCOUNTER`）的 payload。
//!
//! agent 订阅后按 `event_kind` 生成两类 narration：
//! - `appeared` / `dan_received`：zone perception（感知类叙事，scope="zone"）
//! - `betrayal` / `dead_natural` / `dead_player_kill`：death broadcast（死亡广播，scope="broadcast"）

use serde::{Deserialize, Serialize};

/// 垂死大能遭遇事件种类（wire format, snake_case）。
///
/// agent 按此字段路由 narration 模板（感知类 vs. 死亡广播类）：
/// - 感知类（zone scope）：`appeared`, `dan_received`
/// - 死亡广播类（broadcast scope）：`betrayal`, `dead_natural`, `dead_player_kill`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElderEncounterEventKindV1 {
    /// 大能首次出现在坍缩渊，向玩家发出乞求（Plea 态入场）。
    Appeared,
    /// 玩家向大能交付了一颗回元丹（Recovering 态计数更新）。
    DanReceived,
    /// 大能翻脸夺舍（Betrayal 态，SoulSeize 已发生）。
    Betrayal,
    /// 大能自然力竭死亡（真元耗尽，守信自裁 = dead_by_betrayal=false）。
    DeadNatural,
    /// 大能被玩家击杀（非守信路线，外部击杀）。
    DeadPlayerKill,
}

/// 垂死大能遭遇事件 payload（server → agent + client，`bong:elder_encounter`）。
///
/// ## 字段说明
/// - `zone_name`：大能所在 TSY zone 名称（agent 用于 zone-scoped narration）
/// - `elder_entity_idx`：大能 ECS entity index（纯 u32，agent 用于区分不同大能）
/// - `event_kind`：事件种类（感知 vs. 死亡广播）
/// - `betray_probability`：翻脸概率（0.0-1.0；`appeared` 时填实际值，其他事件填 0.0）
///   - SpiritEye 激活时 client HUD 显示具体 %；未激活时显示「气息有异」
///   - agent narration 中 `appeared` 事件使用此值描述危险程度
/// - `dan_count`：累计收到丹数量（`dan_received` 事件时递增；其他事件填 0）
/// - `offered_skill_id`：承诺传授的功法 ID（地阶，四门之一；`appeared` 时填，其他可填空）
/// - `qi_fraction`：大能当前真元比例（qi_current / qi_max_cache，0.0-1.0）；
///   client HUD 真元进度条从此字段渲染；死亡事件填 0.0
/// - `server_tick`：server 游戏 tick（用于 agent 时序审计）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElderEncounterEventV1 {
    /// 大能所在 TSY zone 名称（对应 ZoneRegistry zone.name）。
    pub zone_name: String,
    /// 大能 ECS entity index（u32，用于区分多次遭遇）。
    pub elder_entity_idx: u32,
    /// 事件种类（感知 vs. 死亡广播）。
    pub event_kind: ElderEncounterEventKindV1,
    /// 翻脸概率（0.0-1.0；`appeared` 时填实际值，其他事件填 0.0）。
    ///
    /// - 玩家 SpiritEye 激活时 client HUD 显示具体 %
    /// - 未激活时 client HUD 显示「气息有异」
    pub betray_probability: f64,
    /// 累计收到丹数量（`dan_received` 事件时递增；其他事件填 0）。
    pub dan_count: u32,
    /// 承诺传授的功法 ID（地阶；`appeared` 时填实际值，其他可填空字符串）。
    pub offered_skill_id: String,
    /// 大能当前真元比例（qi_current / qi_max_cache，[0.0, 1.0]）。
    ///
    /// - `appeared` / `dan_received` 时填实际值（大能真元恢复进度）
    /// - 死亡类事件（betrayal / dead_natural / dead_player_kill）填 0.0
    /// - client HUD 进度条从此字段渲染（B1 修复：实际真元而非硬编码 0.8）
    pub qi_fraction: f32,
    /// Server 游戏 tick（时序审计用）。
    pub server_tick: u64,
}

#[cfg(test)]
mod elder_encounter_schema_tests {
    use super::*;

    // ── ElderEncounterEventKindV1 serde ──────────────────────────────────────

    #[test]
    fn event_kind_all_variants_serde_roundtrip() {
        // 期望：所有 5 个 variant 均可 JSON 往返（wire contract pin）
        let variants = [
            ElderEncounterEventKindV1::Appeared,
            ElderEncounterEventKindV1::DanReceived,
            ElderEncounterEventKindV1::Betrayal,
            ElderEncounterEventKindV1::DeadNatural,
            ElderEncounterEventKindV1::DeadPlayerKill,
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap_or_else(|e| {
                panic!("ElderEncounterEventKindV1::{v:?} serialize failed: {e}")
            });
            let back: ElderEncounterEventKindV1 = serde_json::from_str(&json).unwrap_or_else(|e| {
                panic!("ElderEncounterEventKindV1 deserialize from {json} failed: {e}")
            });
            assert_eq!(
                *v, back,
                "ElderEncounterEventKindV1::{v:?} serde round-trip must preserve variant"
            );
        }
    }

    #[test]
    fn event_kind_snake_case_pin() {
        // 期望：各 variant 序列化为指定 snake_case 字符串（agent 消费侧依赖此）
        let cases = [
            (ElderEncounterEventKindV1::Appeared, "\"appeared\""),
            (ElderEncounterEventKindV1::DanReceived, "\"dan_received\""),
            (ElderEncounterEventKindV1::Betrayal, "\"betrayal\""),
            (ElderEncounterEventKindV1::DeadNatural, "\"dead_natural\""),
            (
                ElderEncounterEventKindV1::DeadPlayerKill,
                "\"dead_player_kill\"",
            ),
        ];
        for (v, expected) in cases {
            let json = serde_json::to_string(&v).expect("serialize");
            assert_eq!(
                json, expected,
                "ElderEncounterEventKindV1::{v:?} should serialize as {expected}, got {json}"
            );
        }
    }

    #[test]
    fn event_kind_rejects_invalid_string() {
        // 期望：非法字符串反序列化失败（不静默接受无效 kind）
        let result = serde_json::from_str::<ElderEncounterEventKindV1>(r#""unknown_event""#);
        assert!(
            result.is_err(),
            "invalid event kind string should fail deserialization, not silently accept"
        );
    }

    // ── ElderEncounterEventV1 serde ──────────────────────────────────────────

    #[test]
    fn event_v1_appeared_serde_roundtrip() {
        // 期望：appeared 事件（含 betray_probability + qi_fraction）完整往返
        let ev = ElderEncounterEventV1 {
            zone_name: "tsy_deep".to_string(),
            elder_entity_idx: 42,
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: 0.65,
            dan_count: 0,
            offered_skill_id: "woliu.heart".to_string(),
            qi_fraction: 0.9,
            server_tick: 720_000,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            ev, back,
            "ElderEncounterEventV1 appeared serde round-trip must preserve all fields"
        );
    }

    #[test]
    fn event_v1_dan_received_serde_roundtrip() {
        // 期望：dan_received 事件（dan_count=3）往返（qi_fraction 反映恢复进度）
        let ev = ElderEncounterEventV1 {
            zone_name: "tsy_shallow".to_string(),
            elder_entity_idx: 99,
            event_kind: ElderEncounterEventKindV1::DanReceived,
            betray_probability: 0.0,
            dan_count: 3,
            offered_skill_id: "anqi.echo_fractal".to_string(),
            qi_fraction: 0.6,
            server_tick: 1000,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev, back, "dan_received event serde round-trip failed");
    }

    #[test]
    fn event_v1_betrayal_serde_roundtrip() {
        // 期望：betrayal 事件往返（betray_probability=0.0，offered_skill_id 可为空，qi_fraction=0.0）
        let ev = ElderEncounterEventV1 {
            zone_name: "tsy_deep".to_string(),
            elder_entity_idx: 7,
            event_kind: ElderEncounterEventKindV1::Betrayal,
            betray_probability: 0.0,
            dan_count: 5,
            offered_skill_id: String::new(),
            qi_fraction: 0.0,
            server_tick: 999_999,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev, back, "betrayal event serde round-trip failed");
    }

    #[test]
    fn event_v1_dead_natural_serde_roundtrip() {
        // 期望：dead_natural 事件往返（qi_fraction=0.0 死亡后真元耗尽）
        let ev = ElderEncounterEventV1 {
            zone_name: "tsy_abyss".to_string(),
            elder_entity_idx: 11,
            event_kind: ElderEncounterEventKindV1::DeadNatural,
            betray_probability: 0.0,
            dan_count: 0,
            offered_skill_id: String::new(),
            qi_fraction: 0.0,
            server_tick: 500,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev, back, "dead_natural event serde round-trip failed");
    }

    #[test]
    fn event_v1_dead_player_kill_serde_roundtrip() {
        // 期望：dead_player_kill 事件往返（qi_fraction=0.0 被击杀后无真元）
        let ev = ElderEncounterEventV1 {
            zone_name: "tsy_deep".to_string(),
            elder_entity_idx: 3,
            event_kind: ElderEncounterEventKindV1::DeadPlayerKill,
            betray_probability: 0.0,
            dan_count: 2,
            offered_skill_id: String::new(),
            qi_fraction: 0.0,
            server_tick: 1234,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev, back, "dead_player_kill event serde round-trip failed");
    }

    #[test]
    fn event_v1_rejects_unknown_fields() {
        // 期望：额外字段导致反序列化失败（deny_unknown_fields）
        let json = r#"{
            "zone_name":"tsy_deep",
            "elder_entity_idx":1,
            "event_kind":"appeared",
            "betray_probability":0.5,
            "dan_count":0,
            "offered_skill_id":"woliu.heart",
            "server_tick":100,
            "surprise_field":42
        }"#;
        let result = serde_json::from_str::<ElderEncounterEventV1>(json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject payload with unknown field 'surprise_field'"
        );
    }

    #[test]
    fn event_v1_betray_probability_bounds() {
        // 期望：betray_probability 合法范围 [0.0, 1.0]，序列化不截断
        for prob in [0.0_f64, 0.3, 0.65, 0.95, 1.0] {
            let ev = ElderEncounterEventV1 {
                zone_name: "tsy".to_string(),
                elder_entity_idx: 1,
                event_kind: ElderEncounterEventKindV1::Appeared,
                betray_probability: prob,
                dan_count: 0,
                offered_skill_id: "woliu.heart".to_string(),
                qi_fraction: 0.8,
                server_tick: 0,
            };
            let json = serde_json::to_string(&ev).expect("serialize");
            let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
            assert!(
                (back.betray_probability - prob).abs() < 1e-9,
                "betray_probability={prob} should survive serde unchanged, got {}",
                back.betray_probability
            );
        }
    }

    #[test]
    fn event_v1_entity_idx_zero_is_valid() {
        // 期望：elder_entity_idx=0 是合法值（ECS entity index 从 0 开始）
        let ev = ElderEncounterEventV1 {
            zone_name: "tsy_deep".to_string(),
            elder_entity_idx: 0,
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: 0.5,
            dan_count: 0,
            offered_skill_id: "woliu.turbulence_burst".to_string(),
            qi_fraction: 1.0,
            server_tick: 0,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.elder_entity_idx, 0,
            "elder_entity_idx=0 must survive serde (valid ECS entity)"
        );
    }
}
