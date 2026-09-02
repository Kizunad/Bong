//! R6 P1 contract-first S2C `ServerDataV1` emission builder.
//!
//! This module deliberately has no production producer registration. It freezes the
//! recipient/scope and replay metadata contract so later migration slices can replace
//! individual emitters without inventing a second routing path.

use valence::prelude::{Client, Entity, Position, Query};

use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::ServerDataV1;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

/// Recipient scope for a single `ServerDataV1` emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitScope {
    /// Every entity currently carrying a `Client` component, across dimensions.
    Global,
    /// Only clients whose authoritative current dimension equals this dimension.
    Dimension(DimensionKind),
    /// Only clients whose dimension and canonical `ZoneRegistry` lookup both match.
    Zone {
        dimension: DimensionKind,
        zone: String,
    },
    /// Only the exact target entity; there is no broadcast fallback.
    Player(Entity),
}

/// Join/reconnect replay declaration attached to an emission.
///
/// The builder records this metadata only. It does not query business registries or
/// synthesize a snapshot for `JoinSnapshot`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayPolicy {
    None,
    JoinSnapshot(JoinSnapshotKey),
}

/// Opaque key owned by the join-snapshot producer registry.
///
/// Keeping the key opaque prevents this transport layer from becoming a second copy of
/// the business snapshot catalogue. The key is metadata only and is never sent on the
/// wire by this builder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JoinSnapshotKey(String);

impl JoinSnapshotKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for JoinSnapshotKey {
    fn from(key: String) -> Self {
        Self(key)
    }
}

impl From<&str> for JoinSnapshotKey {
    fn from(key: &str) -> Self {
        Self::new(key)
    }
}

impl AsRef<str> for JoinSnapshotKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// One immutable payload plus its recipient and replay declarations.
#[derive(Debug, Clone)]
pub struct ServerDataEmission {
    pub payload: ServerDataV1,
    pub scope: EmitScope,
    pub replay: ReplayPolicy,
}

impl ServerDataEmission {
    /// Construct an emission with no join replay registration.
    pub fn new(payload: ServerDataV1, scope: EmitScope) -> Self {
        Self {
            payload,
            scope,
            replay: ReplayPolicy::None,
        }
    }

    /// Attach a join replay key without changing the payload or routing scope.
    pub fn with_replay(mut self, key: impl Into<JoinSnapshotKey>) -> Self {
        self.replay = ReplayPolicy::JoinSnapshot(key.into());
        self
    }
}

/// Observable result of one builder invocation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EmitReport {
    /// Number of clients satisfying the declared scope.
    pub matched: usize,
    /// Number of custom payloads queued on matched clients.
    pub sent: usize,
    /// Number of emission-level serialization failures. Serialization happens once.
    pub serialization_failed: usize,
}

/// Serialize one `ServerDataV1` exactly once, route it by the declared scope, and queue
/// the immutable bytes on `bong:server_data` for every matching client.
///
/// Scope matching is fail-closed:
/// - `Global` needs no dimension or position metadata.
/// - `Dimension` requires `CurrentDimension`.
/// - `Zone` requires `CurrentDimension`, `Position`, and an authoritative registry, then
///   resolves the canonical zone through `ZoneRegistry::find_zone`.
/// - `Player` compares only the exact ECS entity and never falls back to broadcast.
///
/// `ReplayPolicy` is intentionally not inspected beyond being carried by `emission`; it
/// is registration metadata for a separate join-snapshot dispatcher.
pub fn emit_server_data(
    emission: &ServerDataEmission,
    recipients: &mut Query<(
        Entity,
        &mut Client,
        Option<&CurrentDimension>,
        Option<&Position>,
    )>,
    zones: Option<&ZoneRegistry>,
) -> EmitReport {
    let payload_type = payload_type_label(emission.payload.payload_type());
    let bytes = match serialize_server_data_payload(&emission.payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return EmitReport {
                serialization_failed: 1,
                ..EmitReport::default()
            };
        }
    };

    let mut report = EmitReport::default();
    for (entity, mut client, current_dimension, position) in recipients.iter_mut() {
        if !scope_matches(&emission.scope, entity, current_dimension, position, zones) {
            continue;
        }

        report.matched += 1;
        // `bytes` is immutable and is shared by every recipient of this emission.
        send_server_data_payload(&mut client, bytes.as_slice());
        report.sent += 1;
    }

    tracing::debug!(
        "[bong][network][emit] queued {} {} payload(s): matched={}, sent={}, serialization_failed={}",
        SERVER_DATA_CHANNEL,
        payload_type,
        report.matched,
        report.sent,
        report.serialization_failed,
    );
    report
}

fn scope_matches(
    scope: &EmitScope,
    entity: Entity,
    current_dimension: Option<&CurrentDimension>,
    position: Option<&Position>,
    zones: Option<&ZoneRegistry>,
) -> bool {
    match scope {
        EmitScope::Global => true,
        EmitScope::Dimension(expected) => {
            current_dimension.is_some_and(|current| current.0 == *expected)
        }
        EmitScope::Zone { dimension, zone } => {
            let (Some(current), Some(position), Some(registry)) =
                (current_dimension, position, zones)
            else {
                return false;
            };

            if current.0 != *dimension {
                return false;
            }

            registry
                .find_zone(*dimension, position.get())
                .is_some_and(|canonical_zone| canonical_zone.name == *zone)
        }
        EmitScope::Player(target) => entity == *target,
    }
}
