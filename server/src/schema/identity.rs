//! plan-identity-v1 P5 IPC schema (Rust serde 镜像)。
//!
//! 与 `agent/packages/schema/src/identity.ts` 双端对齐。Reviewer 改任一端时
//! 都要同步另一端 + samples + tests。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevealedTagKindV1 {
    DuguRevealed,
    AnqiMaster,
    ZhenfaMaster,
    BaomaiUser,
    TuikeUser,
    WoliuMaster,
    ZhenmaiUser,
    SwordMaster,
    ForgeMaster,
    AlchemyMaster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionTierV1 {
    High,
    Normal,
    Low,
    Wanted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WantedPlayerEventV1 {
    pub event: WantedPlayerEventTag,
    pub player_uuid: String,
    pub char_id: String,
    pub identity_display_name: String,
    pub identity_id: u32,
    pub reputation_score: i32,
    pub primary_tag: RevealedTagKindV1,
    pub tick: u64,
}

/// `WantedPlayerEventV1.event` 字段固定 string literal 的别名（`"wanted_player"`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WantedPlayerEventTag {
    WantedPlayer,
}

impl Default for WantedPlayerEventTag {
    fn default() -> Self {
        Self::WantedPlayer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IdentityPanelEntryV1 {
    pub identity_id: u32,
    pub display_name: String,
    pub reputation_score: i32,
    pub frozen: bool,
    pub revealed_tag_kinds: Vec<RevealedTagKindV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IdentityPanelStateV1 {
    pub active_identity_id: u32,
    pub last_switch_tick: u64,
    pub cooldown_remaining_ticks: u64,
    pub identities: Vec<IdentityPanelEntryV1>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wanted_player_event_serialize_matches_typescript_sample() {
        // 对拍 agent/packages/schema/samples/wanted-player-event.sample.json
        let event = WantedPlayerEventV1 {
            event: WantedPlayerEventTag::WantedPlayer,
            player_uuid: "11111111-1111-1111-1111-111111111111".to_string(),
            char_id: "offline:kiz".to_string(),
            identity_display_name: "毒蛊师小李".to_string(),
            identity_id: 0,
            reputation_score: -100,
            primary_tag: RevealedTagKindV1::DuguRevealed,
            tick: 24_000,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "wanted_player");
        assert_eq!(json["primary_tag"], "dugu_revealed");
        assert_eq!(json["reputation_score"], -100);
        assert_eq!(json["tick"], 24_000);

        let round_trip: WantedPlayerEventV1 = serde_json::from_value(json).unwrap();
        assert_eq!(round_trip, event);
    }

    #[test]
    fn revealed_tag_kind_v1_each_variant_round_trips() {
        for kind in [
            RevealedTagKindV1::DuguRevealed,
            RevealedTagKindV1::AnqiMaster,
            RevealedTagKindV1::ZhenfaMaster,
            RevealedTagKindV1::BaomaiUser,
            RevealedTagKindV1::TuikeUser,
            RevealedTagKindV1::WoliuMaster,
            RevealedTagKindV1::ZhenmaiUser,
            RevealedTagKindV1::SwordMaster,
            RevealedTagKindV1::ForgeMaster,
            RevealedTagKindV1::AlchemyMaster,
        ] {
            let s = serde_json::to_string(&kind).unwrap();
            let parsed: RevealedTagKindV1 = serde_json::from_str(&s).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn reaction_tier_v1_round_trip() {
        for tier in [
            ReactionTierV1::High,
            ReactionTierV1::Normal,
            ReactionTierV1::Low,
            ReactionTierV1::Wanted,
        ] {
            let s = serde_json::to_string(&tier).unwrap();
            let parsed: ReactionTierV1 = serde_json::from_str(&s).unwrap();
            assert_eq!(parsed, tier);
        }
    }

    #[test]
    fn identity_panel_state_round_trip() {
        let state = IdentityPanelStateV1 {
            active_identity_id: 1,
            last_switch_tick: 12_000,
            cooldown_remaining_ticks: 12_000,
            identities: vec![
                IdentityPanelEntryV1 {
                    identity_id: 0,
                    display_name: "kiz".to_string(),
                    reputation_score: -50,
                    frozen: true,
                    revealed_tag_kinds: vec![RevealedTagKindV1::DuguRevealed],
                },
                IdentityPanelEntryV1 {
                    identity_id: 1,
                    display_name: "alt".to_string(),
                    reputation_score: 0,
                    frozen: false,
                    revealed_tag_kinds: vec![],
                },
            ],
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: IdentityPanelStateV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, state);
    }

    #[test]
    fn wanted_player_event_rejects_extra_fields() {
        let json = r#"{
            "event": "wanted_player",
            "player_uuid": "u",
            "char_id": "c",
            "identity_display_name": "n",
            "identity_id": 0,
            "reputation_score": -100,
            "primary_tag": "dugu_revealed",
            "tick": 1,
            "extra_field": 42
        }"#;
        let result: Result<WantedPlayerEventV1, _> = serde_json::from_str(json);
        assert!(
            result.is_err(),
            "denying extra fields per TypeBox additionalProperties=false"
        );
    }
}
