use std::collections::{BTreeMap, HashMap};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use super::agent_command::Command;
use super::narration::Narration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentWorldModelDecisionV1 {
    pub commands: Vec<Command>,
    pub narrations: Vec<Narration>,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentWorldModelSnapshotV1 {
    pub current_era: Option<CurrentEraV1>,
    #[serde(default)]
    pub zone_history: HashMap<String, Vec<ZoneHistoryEntryV1>>,
    #[serde(default)]
    pub last_decisions: BTreeMap<String, AgentWorldModelDecisionV1>,
    #[serde(default)]
    pub player_first_seen_tick: BTreeMap<String, i64>,
    // fix/world-model-schema-drift：neg_domain 三字段自诞生 commit 起就没有 Rust
    // 对应结构体，server 用 deny_unknown_fields 静默丢弃 agent 的全部发布。
    // #[serde(default)] 仅用于容忍旧 mirror/SQLite 数据缺字段的平滑升级，
    // 不是"两种大小写都收"的兼容层——wire 上仍只认 snake_case。
    #[serde(default)]
    pub neg_domain_pending_tribulations: BTreeMap<String, NegDomainPendingTribulationV1>,
    #[serde(default)]
    pub neg_domain_escape_telemetry: NegDomainEscapeTelemetryV1,
    #[serde(default)]
    pub neg_domain_escape_sessions: BTreeMap<String, NegDomainEscapeSessionV1>,
    pub last_tick: Option<i64>,
    pub last_state_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CurrentEraV1 {
    pub name: String,
    pub since_tick: i64,
    pub global_effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ZoneHistoryEntryV1 {
    pub name: String,
    pub spirit_qi: f64,
    pub danger_level: i64,
    pub active_events: Vec<String>,
    pub player_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NegDomainPendingTribulationV1 {
    pub player_uuid: String,
    pub player_name: String,
    pub zone: String,
    pub entered_at_tick: i64,
    pub last_suppressed_tick: i64,
    #[serde(deserialize_with = "deserialize_neg_domain_tribulation_reason")]
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NegDomainEscapeTelemetryV1 {
    pub escape_entry_count: i64,
    pub post_escape_realm_drop_count: i64,
    pub successful_tribulation_avoidance_count: i64,
    pub active_escape_session_count: i64,
    pub post_escape_realm_drop_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NegDomainEscapeSessionV1 {
    pub player_uuid: String,
    pub player_name: String,
    pub zone: String,
    pub entered_at_tick: i64,
    pub entry_realm_rank: f64,
}

fn deserialize_neg_domain_tribulation_reason<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    const EXPECTED: &str = "negative_domain_tribulation_exempt";
    let reason = String::deserialize(deserializer)?;
    if reason != EXPECTED {
        return Err(D::Error::custom(format!(
            "NegDomainPendingTribulationV1.reason must be `{EXPECTED}`, got `{reason}`"
        )));
    }
    Ok(reason)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentWorldModelEnvelopeV1 {
    #[serde(deserialize_with = "deserialize_v1_version")]
    pub v: u8,
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_source")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub snapshot: AgentWorldModelSnapshotV1,
}

fn deserialize_v1_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version == 1 {
        Ok(version)
    } else {
        Err(D::Error::custom(format!(
            "AgentWorldModelEnvelopeV1.v must be 1, got {version}"
        )))
    }
}

fn deserialize_source<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let source = Option::<String>::deserialize(deserializer)?;
    if let Some(source_value) = source.as_deref() {
        if !matches!(source_value, "arbiter" | "calamity" | "mutation" | "era") {
            return Err(D::Error::custom(format!(
                "AgentWorldModelEnvelopeV1.source has unsupported value `{source_value}`"
            )));
        }
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_agent_world_model_version() {
        let json = r#"{
            "v": 2,
            "id": "wm-1",
            "source": "arbiter",
            "snapshot": {
                "current_era": null,
                "zone_history": {},
                "last_decisions": {},
                "player_first_seen_tick": {},
                "last_tick": null,
                "last_state_ts": null
            }
        }"#;

        let error = serde_json::from_str::<AgentWorldModelEnvelopeV1>(json)
            .expect_err("unknown agent world model version should be rejected");

        assert!(
            error
                .to_string()
                .contains("AgentWorldModelEnvelopeV1.v must be 1"),
            "unexpected agent world model version error: {error}"
        );
    }

    #[test]
    fn rejects_unsupported_agent_world_model_source() {
        let json = r#"{
            "v": 1,
            "id": "wm-1",
            "source": "oracle",
            "snapshot": {
                "current_era": null,
                "zone_history": {},
                "last_decisions": {},
                "player_first_seen_tick": {},
                "last_tick": null,
                "last_state_ts": null
            }
        }"#;

        let error = serde_json::from_str::<AgentWorldModelEnvelopeV1>(json)
            .expect_err("unsupported agent world model source should be rejected");

        assert!(
            error
                .to_string()
                .contains("AgentWorldModelEnvelopeV1.source has unsupported value"),
            "unexpected agent world model source error: {error}"
        );
    }

    // fix/world-model-schema-drift：老快照缺失 neg_domain 三字段（迁移前的
    // mirror/SQLite 数据）必须仍能反序列化，#[serde(default)] 负责补零值。
    #[test]
    fn tolerates_snapshot_missing_neg_domain_fields_via_default() {
        let json = r#"{
            "v": 1,
            "id": "wm-legacy",
            "source": "arbiter",
            "snapshot": {
                "current_era": null,
                "zone_history": {},
                "last_decisions": {},
                "player_first_seen_tick": {},
                "last_tick": 100,
                "last_state_ts": 1700000000
            }
        }"#;

        let envelope: AgentWorldModelEnvelopeV1 = serde_json::from_str(json)
            .expect("legacy snapshot missing neg_domain fields should still parse");

        assert!(envelope.snapshot.neg_domain_pending_tribulations.is_empty());
        assert_eq!(
            envelope.snapshot.neg_domain_escape_telemetry,
            NegDomainEscapeTelemetryV1::default()
        );
        assert!(envelope.snapshot.neg_domain_escape_sessions.is_empty());
    }

    #[test]
    fn rejects_neg_domain_pending_tribulation_with_wrong_reason_literal() {
        let json = r#"{
            "v": 1,
            "id": "wm-bad-reason",
            "source": "arbiter",
            "snapshot": {
                "current_era": null,
                "zone_history": {},
                "last_decisions": {},
                "player_first_seen_tick": {},
                "neg_domain_pending_tribulations": {
                    "offline:Elder": {
                        "player_uuid": "offline:Elder",
                        "player_name": "Elder",
                        "zone": "rift_valley",
                        "entered_at_tick": 100,
                        "last_suppressed_tick": 150,
                        "reason": "made_up_reason"
                    }
                },
                "last_tick": 100,
                "last_state_ts": 1700000000
            }
        }"#;

        let error = serde_json::from_str::<AgentWorldModelEnvelopeV1>(json)
            .expect_err("wrong reason literal should be rejected");

        assert!(
            error
                .to_string()
                .contains("NegDomainPendingTribulationV1.reason must be"),
            "unexpected reason validation error: {error}"
        );
    }

    #[test]
    fn rejects_neg_domain_escape_telemetry_with_unknown_field() {
        let json = r#"{
            "v": 1,
            "id": "wm-bad-telemetry",
            "source": "arbiter",
            "snapshot": {
                "current_era": null,
                "zone_history": {},
                "last_decisions": {},
                "player_first_seen_tick": {},
                "neg_domain_escape_telemetry": {
                    "escape_entry_count": 1,
                    "post_escape_realm_drop_count": 0,
                    "successful_tribulation_avoidance_count": 0,
                    "active_escape_session_count": 0,
                    "post_escape_realm_drop_rate": 0.0,
                    "unexpected_field": true
                },
                "last_tick": 100,
                "last_state_ts": 1700000000
            }
        }"#;

        serde_json::from_str::<AgentWorldModelEnvelopeV1>(json)
            .expect_err("unknown field on neg_domain_escape_telemetry should be rejected");
    }

    #[test]
    fn rejects_neg_domain_pending_tribulation_with_unknown_field() {
        // 与 telemetry 同一契约的对等变体：pending 条目的 deny_unknown_fields 也要有
        // 专属错误分支用例，否则未来某端字段漂移不会在此撞红（CR #860）。
        let json = r#"{
            "v": 1,
            "id": "wm-bad-pending",
            "source": "arbiter",
            "snapshot": {
                "current_era": null,
                "zone_history": {},
                "last_decisions": {},
                "player_first_seen_tick": {},
                "neg_domain_pending_tribulations": {
                    "player-1": {
                        "player_uuid": "uuid-1",
                        "player_name": "Foo",
                        "zone": "blood_valley",
                        "entered_at_tick": 100,
                        "last_suppressed_tick": 200,
                        "reason": "negative_domain_tribulation_exempt",
                        "unexpected_field": true
                    }
                },
                "last_tick": 100,
                "last_state_ts": 1700000000
            }
        }"#;

        serde_json::from_str::<AgentWorldModelEnvelopeV1>(json).expect_err(
            "unknown field on neg_domain_pending_tribulations entry should be rejected",
        );
    }

    #[test]
    fn rejects_neg_domain_escape_session_with_unknown_field() {
        let json = r#"{
            "v": 1,
            "id": "wm-bad-session",
            "source": "arbiter",
            "snapshot": {
                "current_era": null,
                "zone_history": {},
                "last_decisions": {},
                "player_first_seen_tick": {},
                "neg_domain_escape_sessions": {
                    "player-1": {
                        "player_uuid": "uuid-1",
                        "player_name": "Foo",
                        "zone": "blood_valley",
                        "entered_at_tick": 100,
                        "entry_realm_rank": 2.0,
                        "unexpected_field": true
                    }
                },
                "last_tick": 100,
                "last_state_ts": 1700000000
            }
        }"#;

        serde_json::from_str::<AgentWorldModelEnvelopeV1>(json)
            .expect_err("unknown field on neg_domain_escape_sessions entry should be rejected");
    }

    // ── 共享 sample 双端对拍（agent 侧 TypeBox 也校验同一份文件）───────────────
    // wire 通道自诞生 commit 起因 camelCase/snake_case 不一致丢弃了 agent 的全部
    // 发布；这份 sample 锁死 snake_case 全字段形状，任何一端回退都会在这里撞红。

    #[test]
    fn deserializes_shared_agent_world_model_envelope_sample() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/agent-world-model-envelope.sample.json"
        );
        let envelope: AgentWorldModelEnvelopeV1 = serde_json::from_str(json)
            .expect("agent-world-model-envelope.sample.json should deserialize into AgentWorldModelEnvelopeV1");

        assert_eq!(envelope.v, 1);
        assert_eq!(envelope.source.as_deref(), Some("arbiter"));

        let current_era = envelope
            .snapshot
            .current_era
            .expect("sample should carry a non-null current_era");
        assert_eq!(current_era.name, "calamity");
        assert_eq!(current_era.since_tick, 80000);

        assert!(envelope.snapshot.zone_history.contains_key("blood_valley"));
        assert!(envelope.snapshot.last_decisions.contains_key("mutation"));
        assert_eq!(
            envelope
                .snapshot
                .player_first_seen_tick
                .get("offline:Elder"),
            Some(&82000)
        );

        let pending = envelope
            .snapshot
            .neg_domain_pending_tribulations
            .get("offline:Elder")
            .expect("sample should carry a pending tribulation for offline:Elder");
        assert_eq!(pending.zone, "rift_valley");
        assert_eq!(pending.reason, "negative_domain_tribulation_exempt");

        assert_eq!(
            envelope
                .snapshot
                .neg_domain_escape_telemetry
                .escape_entry_count,
            4
        );
        assert_eq!(
            envelope
                .snapshot
                .neg_domain_escape_telemetry
                .post_escape_realm_drop_rate,
            0.25
        );

        let session = envelope
            .snapshot
            .neg_domain_escape_sessions
            .get("offline:Elder")
            .expect("sample should carry an escape session for offline:Elder");
        assert_eq!(session.entry_realm_rank, 4.0);

        assert_eq!(envelope.snapshot.last_tick, Some(84000));
        assert_eq!(envelope.snapshot.last_state_ts, Some(1712345678));
    }

    #[test]
    fn rejects_shared_camel_case_regression_sample() {
        let json = include_str!(
            "../../../agent/packages/schema/samples/agent-world-model-envelope.invalid-camel-case.sample.json"
        );

        let error = serde_json::from_str::<AgentWorldModelEnvelopeV1>(json)
            .expect_err("camelCase world-model snapshot must be rejected by deny_unknown_fields");

        // 断言拒收发生在 snapshot 层：camelCase 键（currentEra 等）在 snake_case
        // schema 下既不是必填字段也不是已知字段，deny_unknown_fields 必须原地报错，
        // 而不是静默丢弃/兼容通过。
        assert!(
            error.to_string().contains("current_era")
                || error.to_string().contains("currentEra")
                || error.to_string().contains("missing field"),
            "unexpected camelCase regression error: {error}"
        );
    }
}
