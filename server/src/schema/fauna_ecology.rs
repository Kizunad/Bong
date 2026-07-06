//! plan-mundane-fauna-v1 P3 生态可视化：定期聚合 zone × 物种为凡兽快照。
//!
//! 与 `schema::botany::BotanyEcologySnapshotV1` 同构（无 variant 维度）；发到 channel
//! `bong:fauna/ecology`，天道 agent `EcologyAnalyzer.ingestFaunaEcology` 消费做 narration
//! 信号（凡兽绝迹 / 兽群迁徙 = 灵气恶化的可感征兆），非天道决策输入。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FaunaSpeciesCountEntryV1 {
    pub kind: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FaunaZoneEcologyV1 {
    pub zone: String,
    pub spirit_qi: f64,
    pub species_counts: Vec<FaunaSpeciesCountEntryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FaunaEcologySnapshotV1 {
    pub v: u8,
    pub tick: u64,
    pub zones: Vec<FaunaZoneEcologyV1>,
}

impl FaunaEcologySnapshotV1 {
    pub fn new(tick: u64, zones: Vec<FaunaZoneEcologyV1>) -> Self {
        Self { v: 1, tick, zones }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fauna_ecology_snapshot_roundtrip() {
        let payload = FaunaEcologySnapshotV1::new(
            1200,
            vec![
                FaunaZoneEcologyV1 {
                    zone: "spawn".to_string(),
                    spirit_qi: 0.45,
                    species_counts: vec![
                        FaunaSpeciesCountEntryV1 {
                            kind: "chicken".to_string(),
                            count: 4,
                        },
                        FaunaSpeciesCountEntryV1 {
                            kind: "rabbit".to_string(),
                            count: 3,
                        },
                    ],
                },
                FaunaZoneEcologyV1 {
                    zone: "blood_valley".to_string(),
                    spirit_qi: -0.2,
                    species_counts: vec![],
                },
            ],
        );

        let json = serde_json::to_string(&payload).unwrap();
        let back: FaunaEcologySnapshotV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
        assert_eq!(back.v, 1);
    }

    #[test]
    fn fauna_ecology_snapshot_rejects_unknown_fields() {
        // deny_unknown_fields 守住 wire 契约：多余字段必须解析失败，防 agent 端偷偷加字段漂移。
        let json = r#"{"v":1,"tick":10,"zones":[],"extra":true}"#;
        let parsed: Result<FaunaEcologySnapshotV1, _> = serde_json::from_str(json);
        assert!(
            parsed.is_err(),
            "unknown field `extra` must be rejected by deny_unknown_fields"
        );
    }

    #[test]
    fn fauna_ecology_empty_zones_roundtrip() {
        let payload = FaunaEcologySnapshotV1::new(0, vec![]);
        let json = serde_json::to_string(&payload).unwrap();
        let back: FaunaEcologySnapshotV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
        assert!(back.zones.is_empty());
    }
}
