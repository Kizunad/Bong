//! Inventory capacity and durable spill contracts for RF-33 R10 P1 and RF-34 R10 P2a.
//!
//! The provider is deliberately independent from gameplay-specific inventory code. It gives
//! R3/R10 a single typed admission point for bounded dropped-loot storage; R10 P2a routes the
//! first production writer wave through that gate. [`SpillContext`] carries the facts a spill
//! needs so a caller cannot silently invent a position, dimension, source revision, or
//! transaction identity.

use super::{DroppedLootEntry, DroppedLootRegistry, InventoryRevision};
use crate::world::dimension::DimensionKind;

/// Global durable dropped-loot queue bound from the inventory core contract.
pub const MAX_DURABLE_DROPPED_LOOT_ENTRIES: usize = 4_096;
/// Per-player bound for owner-only discard entries.
pub const MAX_OWNER_ONLY_DISCARD_ENTRIES_PER_PLAYER: usize = 256;
/// Capacity reserved for system/production writers.
pub const SYSTEM_RESERVED_DURABLE_DROPPED_LOOT_ENTRIES: usize = 512;

/// A typed capacity failure.  The three values are intentionally part of the error so callers
/// can report the failed admission without inspecting provider internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapacityError {
    /// The requested entries do not fit in the bounded queue.
    LimitExceeded {
        current: usize,
        required: usize,
        limit: usize,
    },
    /// An owner-only discard would exceed the per-player quota.
    OwnerQuotaExceeded {
        current: usize,
        required: usize,
        limit: usize,
    },
    /// A player discard may not consume the system-reserved tail of the queue.
    SystemReserved {
        current: usize,
        required: usize,
        limit: usize,
    },
}

impl CapacityError {
    /// Return the admission numbers in the common `{ current, required, limit }` shape.
    pub fn numbers(&self) -> (usize, usize, usize) {
        match self {
            Self::LimitExceeded {
                current,
                required,
                limit,
            }
            | Self::OwnerQuotaExceeded {
                current,
                required,
                limit,
            }
            | Self::SystemReserved {
                current,
                required,
                limit,
            } => (*current, *required, *limit),
        }
    }
}

/// A capacity snapshot useful to tests and admission logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacitySnapshot {
    pub current: usize,
    pub reserved: usize,
    pub limit: usize,
}

/// A reservation that has not yet been committed to durable storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityReservation {
    amount: usize,
}

impl CapacityReservation {
    pub fn amount(self) -> usize {
        self.amount
    }
}

/// Shared bounded-admission contract for all future dropped-loot writers.
pub trait CapacityProvider {
    /// Reserve capacity before mutating an inventory/source.
    fn reserve(&mut self, required: usize) -> Result<CapacityReservation, CapacityError>;
    /// Commit a reservation after the durable spill write succeeds.
    fn commit(&mut self, reservation: CapacityReservation);
    /// Return a reservation when the durable write failed before commit.
    fn release(&mut self, reservation: CapacityReservation);
    /// Observe the current bounded state.
    fn snapshot(&self) -> CapacitySnapshot;
}

/// Small in-memory provider used by contract tests and as an adapter seam for R3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedCapacityProvider {
    snapshot: CapacitySnapshot,
}

impl BoundedCapacityProvider {
    pub fn new(current: usize, limit: usize) -> Result<Self, CapacityError> {
        if current > limit {
            return Err(CapacityError::LimitExceeded {
                current,
                required: 0,
                limit,
            });
        }
        Ok(Self {
            snapshot: CapacitySnapshot {
                current,
                reserved: 0,
                limit,
            },
        })
    }

    pub fn with_default_limit(current: usize) -> Result<Self, CapacityError> {
        Self::new(current, MAX_DURABLE_DROPPED_LOOT_ENTRIES)
    }
}

impl CapacityProvider for BoundedCapacityProvider {
    fn reserve(&mut self, required: usize) -> Result<CapacityReservation, CapacityError> {
        let occupied = self.snapshot.current.saturating_add(self.snapshot.reserved);
        if required > self.snapshot.limit.saturating_sub(occupied) {
            return Err(CapacityError::LimitExceeded {
                current: occupied,
                required,
                limit: self.snapshot.limit,
            });
        }
        self.snapshot.reserved = self.snapshot.reserved.saturating_add(required);
        Ok(CapacityReservation { amount: required })
    }

    fn commit(&mut self, reservation: CapacityReservation) {
        self.snapshot.reserved = self.snapshot.reserved.saturating_sub(reservation.amount);
        self.snapshot.current = self.snapshot.current.saturating_add(reservation.amount);
    }

    fn release(&mut self, reservation: CapacityReservation) {
        self.snapshot.reserved = self.snapshot.reserved.saturating_sub(reservation.amount);
    }

    fn snapshot(&self) -> CapacitySnapshot {
        self.snapshot
    }
}

/// Durable write seam used by [`SpillContext`].  Implementations must make the whole batch
/// recoverable and idempotent by `(transaction_id, dropped_id)`; a failure must leave the batch
/// unapplied so the caller can release its reservation without a one-sided source mutation.
pub trait DurableSpill {
    fn persist_batch(
        &mut self,
        transaction_id: &str,
        entries: &[DroppedLootEntry],
    ) -> Result<(), String>;
}

/// A no-op durable seam for pure transaction tests.  Production callers must provide the R3
/// outbox/database adapter instead of relying on this type.
#[derive(Debug, Default)]
pub struct NoopDurableSpill;

impl DurableSpill for NoopDurableSpill {
    fn persist_batch(
        &mut self,
        _transaction_id: &str,
        _entries: &[DroppedLootEntry],
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Facts required to spill an item without inventing an unowned handoff.
pub struct SpillContext<'a> {
    pub source_identity: String,
    pub source_revision: InventoryRevision,
    pub dimension: DimensionKind,
    pub world_pos: [f64; 3],
    pub registry: &'a mut DroppedLootRegistry,
    pub capacity: &'a mut dyn CapacityProvider,
    pub durable: &'a mut dyn DurableSpill,
    pub transaction_id: String,
}

impl<'a> SpillContext<'a> {
    pub fn validate(&self) -> Result<(), SpillContextError> {
        if self.source_identity.trim().is_empty() {
            return Err(SpillContextError::MissingSourceIdentity);
        }
        if self.transaction_id.trim().is_empty() {
            return Err(SpillContextError::MissingTransactionId);
        }
        if self.world_pos.iter().any(|value| !value.is_finite()) {
            return Err(SpillContextError::InvalidWorldPosition);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpillContextError {
    MissingSourceIdentity,
    MissingTransactionId,
    InvalidWorldPosition,
}

/// Check a bounded queue without allocating a provider.  This is the common contract used by
/// admission tests and by future provider implementations.
pub fn check_capacity(current: usize, required: usize, limit: usize) -> Result<(), CapacityError> {
    if required > limit.saturating_sub(current) {
        Err(CapacityError::LimitExceeded {
            current,
            required,
            limit,
        })
    } else {
        Ok(())
    }
}

/// Check the owner-only quota while preserving the system-reserved tail for production spills.
pub fn check_owner_only_discard_capacity(
    owner_current: usize,
    required: usize,
    global_current: usize,
    global_limit: usize,
) -> Result<(), CapacityError> {
    if required > MAX_OWNER_ONLY_DISCARD_ENTRIES_PER_PLAYER.saturating_sub(owner_current) {
        return Err(CapacityError::OwnerQuotaExceeded {
            current: owner_current,
            required,
            limit: MAX_OWNER_ONLY_DISCARD_ENTRIES_PER_PLAYER,
        });
    }
    let player_limit = global_limit.saturating_sub(SYSTEM_RESERVED_DURABLE_DROPPED_LOOT_ENTRIES);
    if required > player_limit.saturating_sub(global_current) {
        return Err(CapacityError::SystemReserved {
            current: global_current,
            required,
            limit: player_limit,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_provider_enforces_limit_minus_one_limit_and_plus_one() {
        let mut provider = BoundedCapacityProvider::new(2, 4).expect("valid initial state");
        let reservation = provider.reserve(1).expect("limit minus one fits");
        provider.commit(reservation);
        assert_eq!(provider.snapshot().current, 3);

        let reservation = provider.reserve(1).expect("exact limit fits");
        provider.commit(reservation);
        assert_eq!(provider.snapshot().current, 4);
        assert_eq!(
            provider.reserve(1),
            Err(CapacityError::LimitExceeded {
                current: 4,
                required: 1,
                limit: 4,
            })
        );
    }

    #[test]
    fn owner_quota_keeps_system_reserved_tail() {
        assert_eq!(
            check_owner_only_discard_capacity(
                MAX_OWNER_ONLY_DISCARD_ENTRIES_PER_PLAYER,
                1,
                0,
                MAX_DURABLE_DROPPED_LOOT_ENTRIES,
            ),
            Err(CapacityError::OwnerQuotaExceeded {
                current: MAX_OWNER_ONLY_DISCARD_ENTRIES_PER_PLAYER,
                required: 1,
                limit: MAX_OWNER_ONLY_DISCARD_ENTRIES_PER_PLAYER,
            })
        );
        assert_eq!(
            check_owner_only_discard_capacity(
                0,
                1,
                MAX_DURABLE_DROPPED_LOOT_ENTRIES - SYSTEM_RESERVED_DURABLE_DROPPED_LOOT_ENTRIES,
                MAX_DURABLE_DROPPED_LOOT_ENTRIES,
            ),
            Err(CapacityError::SystemReserved {
                current: MAX_DURABLE_DROPPED_LOOT_ENTRIES
                    - SYSTEM_RESERVED_DURABLE_DROPPED_LOOT_ENTRIES,
                required: 1,
                limit: MAX_DURABLE_DROPPED_LOOT_ENTRIES
                    - SYSTEM_RESERVED_DURABLE_DROPPED_LOOT_ENTRIES,
            })
        );
    }
}
