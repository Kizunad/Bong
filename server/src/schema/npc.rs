use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NpcSpawnedV1 {
    pub v: u8,
    pub kind: String,
    pub npc_id: String,
    pub archetype: String,
    pub source: String,
    pub zone: String,
    pub pos: [f64; 3],
    pub initial_age_ticks: f64,
    pub at_tick: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NpcDeathV1 {
    pub v: u8,
    pub kind: String,
    pub npc_id: String,
    pub archetype: String,
    pub cause: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub life_record_snapshot: Option<String>,
    pub age_ticks: f64,
    pub max_age_ticks: f64,
    pub at_tick: u64,
    /// plan-offscreen-war-v1 P0：是否离屏 dormant 派系互殴所致。
    /// serde default=false 让旧 payload（无此字段）仍可反序列化，向后兼容。
    #[serde(default)]
    pub from_dormant_combat: bool,
    /// plan-offscreen-war-v1 P0：死亡坐标 [x,y,z]，无则 None 不上线。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pos: Option<[f64; 3]>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FactionEventV1 {
    pub v: u8,
    pub kind: String,
    pub faction_id: String,
    pub event_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_id: Option<String>,
    pub loyalty_bias: f64,
    pub mission_queue_size: u32,
    pub at_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_death_omits_absent_optional_fields() {
        let payload = NpcDeathV1 {
            v: 1,
            kind: "npc_death".to_string(),
            npc_id: "npc_1v1".to_string(),
            archetype: "commoner".to_string(),
            cause: "natural_aging".to_string(),
            faction_id: None,
            life_record_snapshot: None,
            age_ticks: 10.0,
            max_age_ticks: 20.0,
            at_tick: 3,
            from_dormant_combat: false,
            pos: None,
        };

        let value = serde_json::to_value(payload).expect("serialize");
        assert!(value.get("faction_id").is_none());
        assert!(value.get("life_record_snapshot").is_none());
        // pos 为 None 时不上线（skip_serializing_if）。
        assert!(
            value.get("pos").is_none(),
            "pos=None 应被 skip，不出现在 wire JSON"
        );
        // from_dormant_combat 是 bool（非 Option），始终上线，默认场景为 false。
        assert_eq!(
            value.get("from_dormant_combat"),
            Some(&serde_json::json!(false)),
            "from_dormant_combat 始终序列化，默认 false"
        );
    }

    #[test]
    fn npc_death_v1_roundtrip_with_from_dormant_combat() {
        // plan-offscreen-war-v1 P0：离屏 dormant 互殴死亡 wire 必须完整 roundtrip
        // 新字段（from_dormant_combat=true + pos），让 agent 能区分战死 vs 老死。
        let payload = NpcDeathV1 {
            v: 1,
            kind: "npc_death".to_string(),
            npc_id: "dormant:rogue:7".to_string(),
            archetype: "rogue".to_string(),
            cause: "combat".to_string(),
            faction_id: Some("attack".to_string()),
            life_record_snapshot: Some("残灰谷争脉".to_string()),
            age_ticks: 50.0,
            max_age_ticks: 200.0,
            at_tick: 1234,
            from_dormant_combat: true,
            pos: Some([12.5, 64.0, -8.0]),
        };

        let json = serde_json::to_string(&payload).expect("serialize");
        let parsed: NpcDeathV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, payload, "新字段必须无损 roundtrip");

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["from_dormant_combat"], serde_json::json!(true));
        assert_eq!(value["pos"], serde_json::json!([12.5, 64.0, -8.0]));
        assert_eq!(value["cause"], serde_json::json!("combat"));
    }

    #[test]
    fn npc_death_v1_deserializes_legacy_payload_without_new_fields() {
        // 向后兼容：旧 server 发的、不带 from_dormant_combat/pos 的 payload 仍能解析。
        // serde default 让 from_dormant_combat 回退 false、pos 回退 None。
        let legacy = serde_json::json!({
            "v": 1,
            "kind": "npc_death",
            "npc_id": "npc_old",
            "archetype": "commoner",
            "cause": "natural_aging",
            "age_ticks": 10.0,
            "max_age_ticks": 20.0,
            "at_tick": 3
        });
        let parsed: NpcDeathV1 =
            serde_json::from_value(legacy).expect("legacy payload without new fields must parse");
        assert!(
            !parsed.from_dormant_combat,
            "缺字段时 from_dormant_combat 必须 default 为 false（向后兼容）"
        );
        assert_eq!(parsed.pos, None, "缺字段时 pos 必须 default 为 None");
    }
}
