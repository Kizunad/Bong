use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExposureKindV1 {
    Chat,
    Trade,
    Divine,
    Death,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKindV1 {
    Master,
    Disciple,
    Companion,
    Pact,
    Feud,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RenownTagV1 {
    pub tag: String,
    pub weight: f64,
    pub last_seen_tick: u64,
    #[serde(default)]
    pub permanent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RelationshipSnapshotV1 {
    pub kind: RelationshipKindV1,
    pub peer: String,
    pub since_tick: u64,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RenownSnapshotV1 {
    pub fame: i32,
    pub notoriety: i32,
    #[serde(default)]
    pub top_tags: Vec<RenownTagV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FactionMembershipSnapshotV1 {
    pub faction: String,
    pub rank: u8,
    pub loyalty: i32,
    pub betrayal_count: u8,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub invite_block_until_tick: Option<u64>,
    #[serde(default)]
    pub permanently_refused: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlayerSocialSnapshotV1 {
    #[serde(default)]
    pub renown: RenownSnapshotV1,
    #[serde(default)]
    pub relationships: Vec<RelationshipSnapshotV1>,
    #[serde(default)]
    pub exposed_to_count: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub faction_membership: Option<FactionMembershipSnapshotV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocialRemoteIdentityV1 {
    pub player_uuid: String,
    pub anonymous: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub realm_band: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub breath_hint: Option<String>,
    #[serde(default)]
    pub renown_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocialAnonymityPayloadV1 {
    pub viewer: String,
    #[serde(default)]
    pub remotes: Vec<SocialRemoteIdentityV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocialExposureEventV1 {
    pub v: u8,
    pub actor: String,
    pub kind: ExposureKindV1,
    pub witnesses: Vec<String>,
    pub tick: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocialPactEventV1 {
    pub v: u8,
    pub left: String,
    pub right: String,
    pub terms: String,
    pub tick: u64,
    pub broken: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocialFeudEventV1 {
    pub v: u8,
    pub left: String,
    pub right: String,
    pub tick: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub place: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SocialRenownDeltaV1 {
    pub v: u8,
    pub char_id: String,
    pub fame_delta: i32,
    pub notoriety_delta: i32,
    #[serde(default)]
    pub tags_added: Vec<RenownTagV1>,
    pub tick: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SparringInvitePayloadV1 {
    pub invite_id: String,
    pub initiator: String,
    pub target: String,
    pub realm_band: String,
    pub breath_hint: String,
    pub terms: String,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TradeItemSummaryV1 {
    pub instance_id: u64,
    pub item_id: String,
    pub display_name: String,
    pub stack_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TradeOfferPayloadV1 {
    pub offer_id: String,
    pub initiator: String,
    pub target: String,
    pub offered_item: TradeItemSummaryV1,
    #[serde(default)]
    pub requested_items: Vec<TradeItemSummaryV1>,
    pub expires_at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn social_exposure_serializes_as_snake_case_contract() {
        let payload = SocialExposureEventV1 {
            v: 1,
            actor: "char:alice".to_string(),
            kind: ExposureKindV1::Chat,
            witnesses: vec!["char:bob".to_string()],
            tick: 42,
            zone: Some("spawn".to_string()),
        };

        let value = serde_json::to_value(payload).expect("social exposure should serialize");
        assert_eq!(value["kind"], "chat");
        assert_eq!(value["witnesses"][0], "char:bob");
    }

    #[test]
    fn social_snapshot_defaults_to_public_empty_shape() {
        let snapshot = PlayerSocialSnapshotV1::default();
        assert_eq!(snapshot.renown.fame, 0);
        assert_eq!(snapshot.exposed_to_count, 0);
        assert!(snapshot.relationships.is_empty());
        assert!(snapshot.faction_membership.is_none());
    }
}
