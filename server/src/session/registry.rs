//! Session registry and centralized busy-claim index for the R1 contract.
//!
//! The registry owns only in-memory identity and claim indexes. It deliberately does not
//! install a Bevy resource, persist a session, or invoke a domain producer; those owners will
//! consume this contract in later refactor phases. Registering a session validates both indexes
//! before mutating either one, while dispatch applies the reducer's typed claim effect after the
//! session transition has been accepted.

use std::collections::HashMap;
use std::fmt;

use super::lifecycle::{
    admit_session, AdmissionOutcome, AdmissionRequest, BusyClaim, ClaimEffect, InteractionSession,
    SessionDecision, SessionEvent, SessionKey, SessionLifecycleCtx,
};

/// A busy claim already held by another session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusyClaimConflict {
    pub claim: BusyClaim,
    pub existing_session: SessionKey,
}

impl fmt::Display for BusyClaimConflict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "busy claim {} is held by session {}",
            claim_label(&self.claim),
            self.existing_session
        )
    }
}

/// Errors returned by the session and busy-claim registries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRegistryError {
    InvalidIdentity(SessionKey),
    DuplicateSession(SessionKey),
    BusyClaimConflict(BusyClaimConflict),
    UnknownSession(SessionKey),
    ClaimOwnershipMismatch {
        claim: BusyClaim,
        expected_session: SessionKey,
    },
}

impl fmt::Display for SessionRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentity(key) => {
                write!(formatter, "session {key} has an invalid identity")
            }
            Self::DuplicateSession(key) => write!(formatter, "session {key} is already registered"),
            Self::BusyClaimConflict(conflict) => conflict.fmt(formatter),
            Self::UnknownSession(key) => write!(formatter, "session {key} is not registered"),
            Self::ClaimOwnershipMismatch {
                claim,
                expected_session,
            } => write!(
                formatter,
                "busy claim {} is not held by session {expected_session}",
                claim_label(claim)
            ),
        }
    }
}

impl std::error::Error for SessionRegistryError {}

/// The authoritative in-memory index for mutually exclusive player, target, and facility work.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BusyClaimRegistry {
    claims: HashMap<BusyClaim, SessionKey>,
}

impl BusyClaimRegistry {
    /// Creates an empty claim index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserves a claim for a session, rejecting a same-namespace conflict atomically.
    pub fn try_acquire(
        &mut self,
        session_key: &SessionKey,
        claim: BusyClaim,
    ) -> Result<(), BusyClaimConflict> {
        if let Some(existing_session) = self.claims.get(&claim) {
            if existing_session != session_key {
                return Err(BusyClaimConflict {
                    claim,
                    existing_session: existing_session.clone(),
                });
            }
            return Ok(());
        }
        self.claims.insert(claim, session_key.clone());
        Ok(())
    }

    /// Releases a claim only when the named session is still its owner.
    pub fn release(&mut self, session_key: &SessionKey, claim: &BusyClaim) -> bool {
        if self.claims.get(claim) != Some(session_key) {
            return false;
        }
        self.claims.remove(claim).is_some()
    }

    /// Returns the session currently holding a claim, if any.
    pub fn owner(&self, claim: &BusyClaim) -> Option<&SessionKey> {
        self.claims.get(claim)
    }

    /// Reports whether a claim is currently held.
    pub fn contains(&self, claim: &BusyClaim) -> bool {
        self.claims.contains_key(claim)
    }

    /// Returns the number of active claims.
    pub fn len(&self) -> usize {
        self.claims.len()
    }

    /// Reports whether no claims are held.
    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }
}

/// In-memory session store used by the future domain adapters.
///
/// The store keeps the session map and busy-claim map together so registration and terminal
/// release cannot accidentally update only one side of the ownership index.
pub struct SessionRegistry {
    sessions: HashMap<SessionKey, Box<dyn InteractionSession>>,
    busy_claims: BusyClaimRegistry,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionRegistry {
    /// Creates an empty session registry.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            busy_claims: BusyClaimRegistry::new(),
        }
    }

    /// Registers one session after validating both the session key and its busy claim.
    pub fn register<S>(&mut self, session: S) -> Result<(), SessionRegistryError>
    where
        S: InteractionSession + 'static,
    {
        self.register_boxed(Box::new(session))
    }

    /// Registers an already boxed adapter without exposing production wiring.
    pub fn register_boxed(
        &mut self,
        session: Box<dyn InteractionSession>,
    ) -> Result<(), SessionRegistryError> {
        let session_key = session.session_key();
        let claim = session.busy_claim();
        if session_key.as_str().trim().is_empty()
            || session.owner_key().as_str().trim().is_empty()
            || !claim.is_valid()
        {
            return Err(SessionRegistryError::InvalidIdentity(session_key));
        }
        if self.sessions.contains_key(&session_key) {
            return Err(SessionRegistryError::DuplicateSession(session_key));
        }
        if let Err(conflict) = self.busy_claims.try_acquire(&session_key, claim) {
            return Err(SessionRegistryError::BusyClaimConflict(conflict));
        }
        self.sessions.insert(session_key, session);
        Ok(())
    }

    /// Applies an admission decision and registers the new record only after the claim wins.
    pub fn admit(
        &mut self,
        request: AdmissionRequest,
        outcome: AdmissionOutcome,
    ) -> Result<SessionDecision, SessionRegistryError> {
        let decision = admit_session(request, outcome);
        if let Some(session) = decision.admitted_session.clone() {
            self.register(session)?;
        }
        Ok(decision)
    }

    /// Alias used by owners that treat the registry as a map-like store.
    pub fn insert<S>(&mut self, session: S) -> Result<(), SessionRegistryError>
    where
        S: InteractionSession + 'static,
    {
        self.register(session)
    }

    /// Removes a session and releases its claim when the registry still owns that claim.
    pub fn remove(&mut self, session_key: &SessionKey) -> Option<Box<dyn InteractionSession>> {
        let session = self.sessions.remove(session_key)?;
        let claim = session.busy_claim();
        self.busy_claims.release(session_key, &claim);
        Some(session)
    }

    /// Returns an immutable adapter view.
    pub fn get(&self, session_key: &SessionKey) -> Option<&dyn InteractionSession> {
        self.sessions.get(session_key).map(Box::as_ref)
    }

    /// Returns a mutable adapter view for owner-side maintenance.
    pub fn get_mut(
        &mut self,
        session_key: &SessionKey,
    ) -> Option<&mut (dyn InteractionSession + '_)> {
        match self.sessions.get_mut(session_key) {
            Some(session) => Some(session.as_mut()),
            None => None,
        }
    }

    /// Applies one lifecycle event and keeps the claim index aligned with its typed effect.
    pub fn dispatch(
        &mut self,
        session_key: &SessionKey,
        event: SessionEvent,
        context: &mut SessionLifecycleCtx,
    ) -> Result<SessionDecision, SessionRegistryError> {
        let session = self
            .sessions
            .get_mut(session_key)
            .ok_or_else(|| SessionRegistryError::UnknownSession(session_key.clone()))?;
        let decision = session.reduce(event, context);
        if decision.accepted {
            self.apply_claim_effect(session_key, &decision.claim_effect)?;
        }
        Ok(decision)
    }

    /// Reports whether a session key is registered.
    pub fn contains(&self, session_key: &SessionKey) -> bool {
        self.sessions.contains_key(session_key)
    }

    /// Returns the number of registered sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Reports whether no sessions are registered.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Exposes the claim index for contract checks and future owner reconciliation.
    pub fn busy_claims(&self) -> &BusyClaimRegistry {
        &self.busy_claims
    }

    fn apply_claim_effect(
        &mut self,
        session_key: &SessionKey,
        effect: &ClaimEffect,
    ) -> Result<(), SessionRegistryError> {
        match effect {
            ClaimEffect::None | ClaimEffect::Retain => Ok(()),
            ClaimEffect::Release(claim) => {
                if self.busy_claims.release(session_key, claim) {
                    Ok(())
                } else {
                    Err(SessionRegistryError::ClaimOwnershipMismatch {
                        claim: claim.clone(),
                        expected_session: session_key.clone(),
                    })
                }
            }
            ClaimEffect::Acquire(claim) => self
                .busy_claims
                .try_acquire(session_key, claim.clone())
                .map_err(SessionRegistryError::BusyClaimConflict),
        }
    }
}

fn claim_label(claim: &BusyClaim) -> String {
    match claim {
        BusyClaim::PlayerExclusive(player) => format!("player:{player}"),
        BusyClaim::TargetExclusive(target) => format!("target:{target}"),
        BusyClaim::FacilityExclusive(facility) => format!("facility:{facility}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{
        AdmissionOutcome, AdmissionRequest, SessionDurability, SessionEvent, SessionRecord,
        SessionState,
    };

    fn record(session_key: &str, player: &str, claim: BusyClaim) -> SessionRecord {
        SessionRecord::new(session_key, player, SessionDurability::Checkpointed, claim)
    }

    #[test]
    fn claim_namespaces_allow_independent_player_target_and_facility_work() {
        let mut registry = BusyClaimRegistry::new();
        let first = SessionKey::new("session-1");
        let second = SessionKey::new("session-2");
        registry
            .try_acquire(&first, BusyClaim::player("alice"))
            .expect("first player claim should win");
        registry
            .try_acquire(&second, BusyClaim::target("target-1"))
            .expect("target claim should not conflict with player claim");
        registry
            .try_acquire(&second, BusyClaim::facility("facility-1"))
            .expect("facility claim should not conflict with other namespaces");
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn same_claim_race_has_one_winner_and_does_not_leave_loser_claim() {
        let mut registry = SessionRegistry::new();
        registry
            .register(record("session-1", "alice", BusyClaim::facility("bench-1")))
            .expect("first registration should win");
        let second = registry.register(record("session-2", "bob", BusyClaim::facility("bench-1")));
        assert!(matches!(
            second,
            Err(SessionRegistryError::BusyClaimConflict(_))
        ));
        assert!(!registry.contains(&SessionKey::new("session-2")));
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.busy_claims().len(), 1);
    }

    #[test]
    fn duplicate_session_key_is_rejected_before_claim_index_changes() {
        let mut registry = SessionRegistry::new();
        registry
            .register(record("session-1", "alice", BusyClaim::player("alice")))
            .expect("first registration should win");
        let duplicate = registry.register(record(
            "session-1",
            "alice",
            BusyClaim::facility("other-bench"),
        ));
        assert_eq!(
            duplicate,
            Err(SessionRegistryError::DuplicateSession(SessionKey::new(
                "session-1"
            )))
        );
        assert!(!registry
            .busy_claims()
            .contains(&BusyClaim::facility("other-bench")));
    }

    #[test]
    fn invalid_identity_is_rejected_before_a_claim_is_reserved() {
        let mut registry = SessionRegistry::new();
        let result = registry.register(record(" ", "alice", BusyClaim::facility("bench-1")));
        assert!(matches!(
            result,
            Err(SessionRegistryError::InvalidIdentity(key)) if key.as_str() == " "
        ));
        assert!(registry.is_empty());
        assert!(registry.busy_claims().is_empty());
    }

    #[test]
    fn removing_session_releases_claim_for_next_registration() {
        let mut registry = SessionRegistry::new();
        let first_key = SessionKey::new("session-1");
        registry
            .register(record(
                first_key.as_str(),
                "alice",
                BusyClaim::target("target-1"),
            ))
            .expect("first registration should win");
        registry.remove(&first_key).expect("session should exist");
        registry
            .register(record("session-2", "bob", BusyClaim::target("target-1")))
            .expect("released claim should be reusable");
        assert_eq!(
            registry.busy_claims().owner(&BusyClaim::target("target-1")),
            Some(&SessionKey::new("session-2"))
        );
    }

    #[test]
    fn admission_registers_only_the_claim_winner() {
        let mut registry = SessionRegistry::new();
        let request = AdmissionRequest {
            request_id: Some("open-1".to_string()),
            session_key: SessionKey::new("session-1"),
            owner_key: "alice".into(),
            durability: SessionDurability::Checkpointed,
            busy_claim: BusyClaim::player("alice"),
        };
        let decision = registry
            .admit(request, AdmissionOutcome::ReservedClaimWon)
            .expect("claim winner should register");
        assert!(decision.accepted);
        assert!(registry.contains(&SessionKey::new("session-1")));
        assert_eq!(registry.busy_claims().len(), 1);
    }

    #[test]
    fn terminal_dispatch_releases_claim_without_removing_the_audit_record() {
        let mut registry = SessionRegistry::new();
        let session_key = SessionKey::new("session-1");
        registry
            .register(record(
                session_key.as_str(),
                "alice",
                BusyClaim::player("alice"),
            ))
            .expect("session should register");
        let identity = registry
            .get(&session_key)
            .expect("session should be present")
            .session_key();
        let owner = registry
            .get(&session_key)
            .expect("session should be present")
            .owner_key()
            .clone();
        let event_identity = crate::session::SessionIdentity {
            session_key: identity,
            owner_key: owner,
            generation: 0,
        };
        let mut context = Default::default();
        registry
            .dispatch(
                &session_key,
                SessionEvent::DimensionChange {
                    identity: event_identity.clone(),
                },
                &mut context,
            )
            .expect("dimension handoff should reduce");
        let decision = registry
            .dispatch(
                &session_key,
                SessionEvent::HandoffCommitted {
                    identity: event_identity,
                },
                &mut context,
            )
            .expect("handoff commit should reduce");
        assert!(decision.accepted);
        assert_eq!(decision.next_state, SessionState::Ended);
        assert!(!registry.busy_claims().contains(&BusyClaim::player("alice")));
        assert!(registry.contains(&session_key));
    }
}
