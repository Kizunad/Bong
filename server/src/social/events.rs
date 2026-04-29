use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use crate::npc::faction::FactionId;
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

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SocialPactEvent {
    pub left: String,
    pub right: String,
    pub terms: String,
    pub tick: u64,
    pub broken: bool,
    pub breaker: Option<String>,
    #[serde(default)]
    pub witnesses: Vec<String>,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SocialMentorshipEvent {
    pub master: String,
    pub disciple: String,
    pub tick: u64,
    pub technique_hint: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SparringInviteRequest {
    pub initiator: Entity,
    pub target: Entity,
    pub terms: String,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SparringInviteResponseKind {
    Accept,
    Decline,
    Timeout,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SparringInviteResponseEvent {
    pub player: Entity,
    pub invite_id: String,
    pub kind: SparringInviteResponseKind,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionMembershipDecisionKind {
    AcceptInvite,
    Resign,
    Expel,
    Betray,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct FactionMembershipDecisionEvent {
    pub player: Entity,
    pub faction: FactionId,
    pub kind: FactionMembershipDecisionKind,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SpiritNichePlaceRequest {
    pub player: Entity,
    pub pos: [i32; 3],
    pub item_instance_id: Option<u64>,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpiritNicheRevealSource {
    Gaze,
    BreakAttempt,
    MarkCoordinate,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SpiritNicheRevealRequest {
    pub observer: Option<Entity>,
    pub owner: String,
    pub source: SpiritNicheRevealSource,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct SpiritNicheCoordinateRevealRequest {
    pub observer: Entity,
    pub pos: [i32; 3],
    pub source: SpiritNicheRevealSource,
    pub tick: u64,
}
