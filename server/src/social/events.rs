use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use crate::schema::social::{ExposureKindV1, RelationshipKindV1, RenownTagV1};

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct PlayerChatCollected {
    pub entity: Entity,
    pub username: String,
    pub char_id: String,
    pub zone: String,
    pub raw: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SocialExposureEvent {
    pub actor: String,
    pub kind: ExposureKindV1,
    pub witnesses: Vec<String>,
    pub tick: u64,
    pub zone: Option<String>,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SocialRenownDeltaEvent {
    pub char_id: String,
    pub fame_delta: i32,
    pub notoriety_delta: i32,
    pub tags_added: Vec<RenownTagV1>,
    pub tick: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SocialRelationshipEvent {
    pub left: String,
    pub right: String,
    pub left_kind: RelationshipKindV1,
    pub right_kind: RelationshipKindV1,
    pub tick: u64,
    pub metadata: serde_json::Value,
}
