use serde::{Deserialize, Serialize};

/// 战斗命中部位 id（plan-race-system-v1 P1c——闭合 8 段人形 enum 开放化为 string，
/// 与 `body_plan::BodyPartId` 同型）。humanoid 沿用现有 8 个 snake_case wire 字符串
/// （见下方关联常量），非 humanoid 构型（P5 飞鲸等）的部位不在这 8 段之列，物理上
/// 装不下闭合 enum——本类型不做兼容层，直接开放为任意 string id。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct CombatBodyPartV1(pub String);

impl CombatBodyPartV1 {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    // humanoid 8 段 wire 字符串常量（`combat::components::BodyPart` 现有 snake_case
    // 惯例，`network::cast_emit::parse_wound_heal_body_part` 同源）——保留作为向后
    // 兼容的构造捷径，不代表本类型仍是闭合枚举。
    pub const HEAD: &'static str = "head";
    pub const CHEST: &'static str = "chest";
    pub const BACK: &'static str = "back";
    pub const ABDOMEN: &'static str = "abdomen";
    pub const ARM_L: &'static str = "arm_l";
    pub const ARM_R: &'static str = "arm_r";
    pub const LEG_L: &'static str = "leg_l";
    pub const LEG_R: &'static str = "leg_r";

    pub fn head() -> Self {
        Self::new(Self::HEAD)
    }
    pub fn chest() -> Self {
        Self::new(Self::CHEST)
    }
    pub fn back() -> Self {
        Self::new(Self::BACK)
    }
    pub fn abdomen() -> Self {
        Self::new(Self::ABDOMEN)
    }
    pub fn arm_l() -> Self {
        Self::new(Self::ARM_L)
    }
    pub fn arm_r() -> Self {
        Self::new(Self::ARM_R)
    }
    pub fn leg_l() -> Self {
        Self::new(Self::LEG_L)
    }
    pub fn leg_r() -> Self {
        Self::new(Self::LEG_R)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatWoundKindV1 {
    Cut,
    Blunt,
    Pierce,
    Burn,
    Concussion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatAttackSourceV1 {
    Melee,
    BurstMeridian,
    QiNeedle,
    FullPower,
    SwordCleave,
    SwordThrust,
    // plan-sword-path-complete §E — 剑道五招专属变体（追加，旧 6 个顺序不变 → 向后兼容）
    SwordPathCondenseEdge,
    SwordPathQiSlash,
    SwordPathResonance,
    SwordPathManifest,
    SwordPathHeavenGate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatRealtimeKindV1 {
    CombatEvent,
    DeathEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombatDefenseKindV1 {
    JieMai,
    SwordParry,
    /// plan-shield-block-v1 P2 — 盾牌正面格挡成功。
    ShieldBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatRealtimeEventV1 {
    pub v: u8,
    pub kind: CombatRealtimeKindV1,
    pub tick: u64,
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacker_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_part: Option<CombatBodyPartV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wound_kind: Option<CombatWoundKindV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<CombatAttackSourceV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_damage: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contam_delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_kind: Option<CombatDefenseKindV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_effectiveness: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_contam_reduced: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defense_wound_severity: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatSummaryV1 {
    pub v: u8,
    pub window_start_tick: u64,
    pub window_end_tick: u64,
    pub combat_event_count: u64,
    pub death_event_count: u64,
    pub damage_total: f32,
    pub contam_delta_total: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realtime_and_summary_roundtrip() {
        let realtime = CombatRealtimeEventV1 {
            v: 1,
            kind: CombatRealtimeKindV1::CombatEvent,
            tick: 42,
            target_id: "offline:Crimson".to_string(),
            attacker_id: Some("offline:Azure".to_string()),
            body_part: Some(CombatBodyPartV1::chest()),
            wound_kind: Some(CombatWoundKindV1::Blunt),
            source: Some(CombatAttackSourceV1::Melee),
            damage: Some(20.0),
            physical_damage: Some(0.0),
            contam_delta: Some(5.0),
            description: Some("debug_attack_intent offline:Azure -> offline:Crimson".to_string()),
            cause: None,
            defense_kind: Some(CombatDefenseKindV1::JieMai),
            defense_effectiveness: Some(0.7),
            defense_contam_reduced: Some(4.0),
            defense_wound_severity: Some(0.4),
        };
        let realtime_json = serde_json::to_string(&realtime).expect("serialize realtime");
        let realtime_back: CombatRealtimeEventV1 =
            serde_json::from_str(realtime_json.as_str()).expect("deserialize realtime");
        assert_eq!(realtime_back.v, 1);
        assert_eq!(realtime_back.tick, 42);
        assert_eq!(realtime_back.body_part, Some(CombatBodyPartV1::chest()));
        assert_eq!(realtime_back.wound_kind, Some(CombatWoundKindV1::Blunt));
        assert_eq!(realtime_back.source, Some(CombatAttackSourceV1::Melee));
        assert_eq!(realtime_back.damage, Some(20.0));
        assert_eq!(
            realtime_back.defense_kind,
            Some(CombatDefenseKindV1::JieMai)
        );
        assert_eq!(realtime_back.defense_effectiveness, Some(0.7));

        let summary = CombatSummaryV1 {
            v: 1,
            window_start_tick: 201,
            window_end_tick: 400,
            combat_event_count: 12,
            death_event_count: 3,
            damage_total: 64.0,
            contam_delta_total: 12.5,
        };
        let summary_json = serde_json::to_string(&summary).expect("serialize summary");
        let summary_back: CombatSummaryV1 =
            serde_json::from_str(summary_json.as_str()).expect("deserialize summary");
        assert_eq!(summary_back.window_start_tick, 201);
        assert_eq!(summary_back.window_end_tick, 400);
        assert_eq!(summary_back.damage_total, 64.0);
        assert_eq!(summary_back.contam_delta_total, 12.5);
    }

    // plan-sword-path-complete §E — 剑道五招 serde 变体逐一正反对拍。
    // 正：每个变体序列化为锁定 wire 串，再反序列化回枚举值。
    // 反：非法串应导致 serde_json::from_str 返回 Err。
    #[test]
    fn sword_path_condense_edge_serde() {
        let v = CombatAttackSourceV1::SwordPathCondenseEdge;
        let json = serde_json::to_string(&v).expect("serialize SwordPathCondenseEdge");
        assert_eq!(
            json, "\"sword_path_condense_edge\"",
            "SwordPathCondenseEdge 应序列化为 \"sword_path_condense_edge\""
        );
        let back: CombatAttackSourceV1 =
            serde_json::from_str(&json).expect("deserialize SwordPathCondenseEdge");
        assert_eq!(
            back,
            CombatAttackSourceV1::SwordPathCondenseEdge,
            "SwordPathCondenseEdge 反序列化应还原"
        );
    }

    #[test]
    fn sword_path_qi_slash_serde() {
        let v = CombatAttackSourceV1::SwordPathQiSlash;
        let json = serde_json::to_string(&v).expect("serialize SwordPathQiSlash");
        assert_eq!(
            json, "\"sword_path_qi_slash\"",
            "SwordPathQiSlash 应序列化为 \"sword_path_qi_slash\""
        );
        let back: CombatAttackSourceV1 =
            serde_json::from_str(&json).expect("deserialize SwordPathQiSlash");
        assert_eq!(
            back,
            CombatAttackSourceV1::SwordPathQiSlash,
            "SwordPathQiSlash 反序列化应还原"
        );
    }

    #[test]
    fn sword_path_resonance_serde() {
        let v = CombatAttackSourceV1::SwordPathResonance;
        let json = serde_json::to_string(&v).expect("serialize SwordPathResonance");
        assert_eq!(
            json, "\"sword_path_resonance\"",
            "SwordPathResonance 应序列化为 \"sword_path_resonance\""
        );
        let back: CombatAttackSourceV1 =
            serde_json::from_str(&json).expect("deserialize SwordPathResonance");
        assert_eq!(
            back,
            CombatAttackSourceV1::SwordPathResonance,
            "SwordPathResonance 反序列化应还原"
        );
    }

    #[test]
    fn sword_path_manifest_serde() {
        let v = CombatAttackSourceV1::SwordPathManifest;
        let json = serde_json::to_string(&v).expect("serialize SwordPathManifest");
        assert_eq!(
            json, "\"sword_path_manifest\"",
            "SwordPathManifest 应序列化为 \"sword_path_manifest\""
        );
        let back: CombatAttackSourceV1 =
            serde_json::from_str(&json).expect("deserialize SwordPathManifest");
        assert_eq!(
            back,
            CombatAttackSourceV1::SwordPathManifest,
            "SwordPathManifest 反序列化应还原"
        );
    }

    #[test]
    fn sword_path_heaven_gate_serde() {
        let v = CombatAttackSourceV1::SwordPathHeavenGate;
        let json = serde_json::to_string(&v).expect("serialize SwordPathHeavenGate");
        assert_eq!(
            json, "\"sword_path_heaven_gate\"",
            "SwordPathHeavenGate 应序列化为 \"sword_path_heaven_gate\""
        );
        let back: CombatAttackSourceV1 =
            serde_json::from_str(&json).expect("deserialize SwordPathHeavenGate");
        assert_eq!(
            back,
            CombatAttackSourceV1::SwordPathHeavenGate,
            "SwordPathHeavenGate 反序列化应还原"
        );
    }

    #[test]
    fn sword_path_heaven_gate_in_event_roundtrip() {
        // 端到端：把 SwordPathHeavenGate 嵌入 CombatRealtimeEventV1 序列化再还原。
        let event = CombatRealtimeEventV1 {
            v: 1,
            kind: CombatRealtimeKindV1::CombatEvent,
            tick: 84011,
            target_id: "offline:Crimson".to_string(),
            attacker_id: Some("offline:Azure".to_string()),
            body_part: Some(CombatBodyPartV1::chest()),
            wound_kind: Some(CombatWoundKindV1::Cut),
            source: Some(CombatAttackSourceV1::SwordPathHeavenGate),
            damage: Some(480.0),
            physical_damage: None,
            contam_delta: Some(12.0),
            description: Some(
                "heaven_gate AoE offline:Azure -> offline:Crimson 100格内".to_string(),
            ),
            cause: None,
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        };
        let json = serde_json::to_string(&event).expect("serialize heaven_gate event");
        assert!(
            json.contains("\"sword_path_heaven_gate\""),
            "序列化结果应含 sword_path_heaven_gate wire 串，实际: {json}"
        );
        let back: CombatRealtimeEventV1 =
            serde_json::from_str(&json).expect("deserialize heaven_gate event");
        assert_eq!(
            back.source,
            Some(CombatAttackSourceV1::SwordPathHeavenGate),
            "事件还原后 source 应为 SwordPathHeavenGate"
        );
        assert_eq!(back.tick, 84011);
    }

    #[test]
    fn sword_path_source_reject_unknown_string() {
        // 反：非法源名应导致反序列化失败（`#[serde(rename_all="snake_case")]` + 无 `other` 变体）。
        let bad = "\"sword_path_void_walk\"";
        let result: Result<CombatAttackSourceV1, _> = serde_json::from_str(bad);
        assert!(
            result.is_err(),
            "非法串 sword_path_void_walk 应被 serde 拒绝，但返回了 Ok"
        );
    }

    #[test]
    fn all_six_legacy_sources_still_roundtrip() {
        // 回归：新增 5 变体不能破坏旧 6 个的 wire 值。
        let cases = [
            (CombatAttackSourceV1::Melee, "\"melee\""),
            (CombatAttackSourceV1::BurstMeridian, "\"burst_meridian\""),
            (CombatAttackSourceV1::QiNeedle, "\"qi_needle\""),
            (CombatAttackSourceV1::FullPower, "\"full_power\""),
            (CombatAttackSourceV1::SwordCleave, "\"sword_cleave\""),
            (CombatAttackSourceV1::SwordThrust, "\"sword_thrust\""),
        ];
        for (variant, expected_wire) in cases {
            let json = serde_json::to_string(&variant)
                .unwrap_or_else(|e| panic!("serialize {expected_wire}: {e}"));
            assert_eq!(
                json, expected_wire,
                "旧变体 wire 值不得改变：期望 {expected_wire}，实际 {json}"
            );
            let back: CombatAttackSourceV1 = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("deserialize {expected_wire}: {e}"));
            assert_eq!(
                serde_json::to_string(&back).unwrap(),
                expected_wire,
                "旧变体 {expected_wire} 反序列化后再序列化应一致"
            );
        }
    }

    // plan-race-system-v1 P1c — `CombatBodyPartV1` 闭合 8 段人形 enum 开放化为
    // string。以下三条锁住：① 8 段 humanoid 常量 wire 串不变（回归）② 非 humanoid
    // 任意 part id（如 P5 飞鲸尾鳍）可合法构造/往返 ③ 旧闭合枚举无法表达的值不再是
    // "拒绝"而是"开放接受"——这正是本轮 wire 开放化要达成的效果。
    #[test]
    fn all_eight_humanoid_body_parts_still_roundtrip() {
        let cases = [
            (CombatBodyPartV1::head(), "\"head\""),
            (CombatBodyPartV1::chest(), "\"chest\""),
            (CombatBodyPartV1::back(), "\"back\""),
            (CombatBodyPartV1::abdomen(), "\"abdomen\""),
            (CombatBodyPartV1::arm_l(), "\"arm_l\""),
            (CombatBodyPartV1::arm_r(), "\"arm_r\""),
            (CombatBodyPartV1::leg_l(), "\"leg_l\""),
            (CombatBodyPartV1::leg_r(), "\"leg_r\""),
        ];
        for (variant, expected_wire) in cases {
            let json = serde_json::to_string(&variant)
                .unwrap_or_else(|e| panic!("serialize {expected_wire}: {e}"));
            assert_eq!(
                json, expected_wire,
                "humanoid part id wire 值不得改变：期望 {expected_wire}，实际 {json}"
            );
            let back: CombatBodyPartV1 = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("deserialize {expected_wire}: {e}"));
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn non_humanoid_body_part_id_roundtrips() {
        // P5 飞鲸草案部位——不在 8 段 humanoid 之列，wire 开放化前无法表达。
        let v = CombatBodyPartV1::new("tail_fin");
        let json = serde_json::to_string(&v).expect("serialize tail_fin");
        assert_eq!(json, "\"tail_fin\"");
        let back: CombatBodyPartV1 = serde_json::from_str(&json).expect("deserialize tail_fin");
        assert_eq!(back, v);
    }

    #[test]
    fn body_part_embedded_in_combat_event_roundtrips_for_non_humanoid_id() {
        let event = CombatRealtimeEventV1 {
            v: 1,
            kind: CombatRealtimeKindV1::CombatEvent,
            tick: 100,
            target_id: "npc:whale_1".to_string(),
            attacker_id: None,
            body_part: Some(CombatBodyPartV1::new("dorsal_fin")),
            wound_kind: Some(CombatWoundKindV1::Blunt),
            source: Some(CombatAttackSourceV1::Melee),
            damage: Some(5.0),
            physical_damage: None,
            contam_delta: None,
            description: None,
            cause: None,
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        };
        let json = serde_json::to_string(&event).expect("serialize dorsal_fin event");
        assert!(json.contains("\"dorsal_fin\""));
        let back: CombatRealtimeEventV1 =
            serde_json::from_str(&json).expect("deserialize dorsal_fin event");
        assert_eq!(back.body_part, Some(CombatBodyPartV1::new("dorsal_fin")));
    }
}
