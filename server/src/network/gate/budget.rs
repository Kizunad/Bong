//! Pure C2S ingress, rejection-feedback, and log-budget state machines.
//!
//! This module intentionally has no knowledge of wire decoding, ECS entities,
//! handlers, or alert transport.  Callers must run [`BudgetStore::admit`] as
//! the first operation for a payload, before decoding or resolving a target.
//! The store is bounded by construction: new client state is refused once the
//! configured client limit is reached, and each client's aggregation maps have
//! an independent finite key limit.

use std::collections::{hash_map::Entry, HashMap};
use std::hash::Hash;

use super::GateDenialReason;

/// Maximum number of payload tokens held by one client.
pub const INGRESS_BUCKET_CAPACITY: u32 = 32;
/// Number of payload tokens restored for each elapsed tick.
pub const INGRESS_REFILL_PER_TICK: u32 = 8;
/// Cost of one payload admission attempt, including a rejected attempt after
/// the bucket has been drained (the latter simply has no token to consume).
pub const INGRESS_PAYLOAD_COST: u32 = 1;
/// Minimum distance between two feedback emissions for one client.
pub const FEEDBACK_WINDOW_TICKS: u64 = 20;
/// Minimum distance between two log summaries for one client and denial key.
pub const LOG_WINDOW_TICKS: u64 = 100;
/// Default maximum number of client entries retained by a store.
pub const DEFAULT_MAX_CLIENTS: usize = 1_024;
/// Default maximum number of request-kind/reason aggregation keys per client.
pub const DEFAULT_MAX_AGGREGATION_KEYS: usize = 128;
/// A request kind is an internal label, never a raw payload.  Keeping this
/// limit prevents a caller from using unbounded labels as an allocation sink.
pub const MAX_REQUEST_KIND_BYTES: usize = 128;

/// Compatibility aliases that make the frozen budget values easy to discover.
pub const BUCKET_CAPACITY: u32 = INGRESS_BUCKET_CAPACITY;
pub const REFILL_PER_TICK: u32 = INGRESS_REFILL_PER_TICK;
pub const PAYLOAD_COST: u32 = INGRESS_PAYLOAD_COST;
pub const REJECTION_FEEDBACK_WINDOW_TICKS: u64 = FEEDBACK_WINDOW_TICKS;
pub const LOG_AGGREGATION_WINDOW_TICKS: u64 = LOG_WINDOW_TICKS;

/// Result of the pre-decode ingress admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IngressAdmission {
    /// Whether the payload may proceed to JSON decode and gate evaluation.
    pub admitted: bool,
    /// Tokens available after this decision.  A rejected request never makes
    /// this value negative or consumes a token.
    pub remaining_tokens: u32,
}

impl IngressAdmission {
    pub const fn admitted(remaining_tokens: u32) -> Self {
        Self {
            admitted: true,
            remaining_tokens,
        }
    }

    pub const fn rejected(remaining_tokens: u32) -> Self {
        Self {
            admitted: false,
            remaining_tokens,
        }
    }

    pub const fn is_admitted(self) -> bool {
        self.admitted
    }
}

/// Result shared by rejection feedback and log-summary admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoalescedAdmission {
    /// Whether this call is allowed to emit one item.
    pub emit: bool,
    /// Number of same-key calls coalesced since the previous emission.  For a
    /// suppressed call this includes the current call; for an emitted call it
    /// is the count attached to that emission.
    pub suppressed_count: u32,
}

impl CoalescedAdmission {
    pub const fn emit(suppressed_count: u32) -> Self {
        Self {
            emit: true,
            suppressed_count,
        }
    }

    pub const fn suppress(suppressed_count: u32) -> Self {
        Self {
            emit: false,
            suppressed_count,
        }
    }

    pub const fn is_emitted(self) -> bool {
        self.emit
    }
}

/// Semantic aliases for callers that want the two output types named by the
/// corresponding budget.
pub type FeedbackAdmission = CoalescedAdmission;
pub type FeedbackDecision = CoalescedAdmission;
pub type LogAdmission = CoalescedAdmission;
pub type LogDecision = CoalescedAdmission;
pub type IngressDecision = IngressAdmission;

/// The bounded aggregation identity for feedback and logs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DenialKey {
    pub request_kind: String,
    pub reason: GateDenialReason,
}

impl DenialKey {
    fn new(request_kind: String, reason: GateDenialReason) -> Option<Self> {
        if request_kind.is_empty() || request_kind.len() > MAX_REQUEST_KIND_BYTES {
            return None;
        }

        Some(Self {
            request_kind,
            reason,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct FeedbackAggregate {
    /// Tick at which this window's first suppression occurred.
    window_start: u64,
    suppressed_count: u32,
}

#[derive(Clone, Copy, Debug)]
struct LogAggregate {
    last_emit_tick: u64,
    suppressed_count: u32,
}

#[derive(Debug)]
struct ClientBudgetState {
    tokens: u32,
    last_tick: u64,
    feedback_last_emit: Option<u64>,
    feedback: HashMap<DenialKey, FeedbackAggregate>,
    logs: HashMap<DenialKey, LogAggregate>,
}

impl ClientBudgetState {
    fn new(tick: u64) -> Self {
        Self {
            tokens: INGRESS_BUCKET_CAPACITY,
            last_tick: tick,
            feedback_last_emit: None,
            feedback: HashMap::new(),
            logs: HashMap::new(),
        }
    }

    /// Advance a state monotonically.  A tick rollback is rejected without
    /// modifying anything.  `u128` makes the refill calculation safe even
    /// when an adversarial tick jumps from zero to `u64::MAX`.
    fn advance(&mut self, tick: u64) -> bool {
        if tick < self.last_tick {
            return false;
        }

        let elapsed = tick - self.last_tick;
        if elapsed != 0 {
            let refill = (u128::from(elapsed) * u128::from(INGRESS_REFILL_PER_TICK))
                .min(u128::from(INGRESS_BUCKET_CAPACITY)) as u32;
            self.tokens = self
                .tokens
                .saturating_add(refill)
                .min(INGRESS_BUCKET_CAPACITY);
            self.last_tick = tick;
        }

        true
    }
}

/// Pure, bounded state for one server ingress boundary.
///
/// `K` is deliberately generic so a caller can use an authenticated player
/// id, a connection id, or another stable client key without coupling this
/// module to the server's ECS types.  The key must be cloned only when a new
/// entry is created by the store.
#[derive(Debug)]
pub struct BudgetStore<K> {
    clients: HashMap<K, ClientBudgetState>,
    max_clients: usize,
    max_aggregation_keys: usize,
}

impl<K> Default for BudgetStore<K> {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_CLIENTS)
    }
}

impl<K> BudgetStore<K> {
    /// Create a store with a finite client bound.
    pub fn new(max_clients: usize) -> Self {
        Self::with_limits(max_clients, DEFAULT_MAX_AGGREGATION_KEYS)
    }

    /// Create a store with explicit client and per-client aggregation bounds.
    pub fn with_limits(max_clients: usize, max_aggregation_keys: usize) -> Self {
        Self {
            clients: HashMap::new(),
            max_clients,
            max_aggregation_keys,
        }
    }

    pub fn len(&self) -> usize {
        self.clients.len()
    }

    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }

    pub const fn max_clients(&self) -> usize {
        self.max_clients
    }

    pub const fn max_aggregation_keys(&self) -> usize {
        self.max_aggregation_keys
    }
}

impl<K> BudgetStore<K>
where
    K: Eq + Hash,
{
    fn state_for(&mut self, client: K, tick: u64) -> Option<&mut ClientBudgetState> {
        let already_present = self.clients.contains_key(&client);
        if !already_present && self.clients.len() >= self.max_clients {
            return None;
        }

        match self.clients.entry(client) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
            Entry::Vacant(entry) => Some(entry.insert(ClientBudgetState::new(tick))),
        }
    }

    /// Admit a payload before JSON decode.  A request rejected for lack of a
    /// token does not perform any downstream work and does not consume a
    /// negative token.  Existing state is also fail-closed on tick rollback.
    pub fn admit(&mut self, client: K, tick: u64) -> IngressAdmission {
        let Some(state) = self.state_for(client, tick) else {
            return IngressAdmission::rejected(0);
        };

        if !state.advance(tick) {
            return IngressAdmission::rejected(state.tokens);
        }

        if state.tokens < INGRESS_PAYLOAD_COST {
            return IngressAdmission::rejected(state.tokens);
        }

        state.tokens -= INGRESS_PAYLOAD_COST;
        IngressAdmission::admitted(state.tokens)
    }

    /// Explicitly named spelling for ingress call sites.
    pub fn admit_ingress(&mut self, client: K, tick: u64) -> IngressAdmission {
        self.admit(client, tick)
    }

    /// Return the current token balance without advancing the clock.
    pub fn tokens_for(&self, client: &K) -> Option<u32> {
        self.clients.get(client).map(|state| state.tokens)
    }

    pub fn contains_client(&self, client: &K) -> bool {
        self.clients.contains_key(client)
    }

    /// Remove one client's bucket and all associated aggregation state.
    pub fn cleanup(&mut self, client: &K) -> bool {
        self.clients.remove(client).is_some()
    }

    /// Retain only the supplied active client keys.  This is the lifecycle
    /// hook for disconnect and role-switch cleanup; it never grows the store.
    pub fn retain_active<I>(&mut self, active_client_keys: I)
    where
        I: IntoIterator<Item = K>,
    {
        let active: std::collections::HashSet<K> = active_client_keys.into_iter().collect();
        self.clients.retain(|client, _| active.contains(client));
    }

    /// Predicate form for callers that already own a key set or ECS snapshot.
    pub fn retain_active_by<F>(&mut self, mut is_active: F)
    where
        F: FnMut(&K) -> bool,
    {
        self.clients.retain(|client, _| is_active(client));
    }

    fn normalize_key<R>(request_kind: R, reason: GateDenialReason) -> Option<DenialKey>
    where
        R: Into<String>,
    {
        DenialKey::new(request_kind.into(), reason)
    }

    /// Admit a client-facing rejection feedback item.  There is at most one
    /// emission per client in each 20-tick interval.  Suppressed counts are
    /// keyed by request kind and internal reason, so unrelated denials never
    /// share a counter or leak their details to one another.
    pub fn admit_feedback<R>(
        &mut self,
        client: K,
        tick: u64,
        request_kind: R,
        reason: GateDenialReason,
    ) -> FeedbackAdmission
    where
        R: Into<String>,
    {
        let Some(key) = Self::normalize_key(request_kind, reason) else {
            return CoalescedAdmission::suppress(0);
        };
        let max_keys = self.max_aggregation_keys;
        let Some(state) = self.state_for(client, tick) else {
            return CoalescedAdmission::suppress(0);
        };
        if !state.advance(tick) {
            return CoalescedAdmission::suppress(0);
        }

        let can_store_key = state.feedback.contains_key(&key) || state.feedback.len() < max_keys;
        if !can_store_key {
            return CoalescedAdmission::suppress(0);
        }

        let eligible = state
            .feedback_last_emit
            .is_none_or(|last| tick.saturating_sub(last) >= FEEDBACK_WINDOW_TICKS);

        if eligible {
            let baseline = state.feedback_last_emit;
            let suppressed_count = state
                .feedback
                .remove(&key)
                .filter(|aggregate| baseline.is_none_or(|last| aggregate.window_start >= last))
                .map_or(0, |aggregate| aggregate.suppressed_count);
            state.feedback_last_emit = Some(tick);
            return CoalescedAdmission::emit(suppressed_count);
        }

        let baseline = state.feedback_last_emit;
        let aggregate = state.feedback.entry(key).or_insert(FeedbackAggregate {
            window_start: tick,
            suppressed_count: 0,
        });
        if baseline.is_some_and(|last| aggregate.window_start < last) {
            aggregate.window_start = tick;
            aggregate.suppressed_count = 0;
        }
        aggregate.suppressed_count = aggregate.suppressed_count.saturating_add(1);
        CoalescedAdmission::suppress(aggregate.suppressed_count)
    }

    /// Short alias for feedback-oriented call sites.
    pub fn feedback<R>(
        &mut self,
        client: K,
        tick: u64,
        request_kind: R,
        reason: GateDenialReason,
    ) -> FeedbackAdmission
    where
        R: Into<String>,
    {
        self.admit_feedback(client, tick, request_kind, reason)
    }

    /// Admit a log/metric summary.  Unlike feedback, the 100-tick budget is
    /// independent for every request-kind/reason key.
    pub fn admit_log<R>(
        &mut self,
        client: K,
        tick: u64,
        request_kind: R,
        reason: GateDenialReason,
    ) -> LogAdmission
    where
        R: Into<String>,
    {
        let Some(key) = Self::normalize_key(request_kind, reason) else {
            return CoalescedAdmission::suppress(0);
        };
        let max_keys = self.max_aggregation_keys;
        let Some(state) = self.state_for(client, tick) else {
            return CoalescedAdmission::suppress(0);
        };
        if !state.advance(tick) {
            return CoalescedAdmission::suppress(0);
        }

        let Some(aggregate) = state.logs.get_mut(&key) else {
            if state.logs.len() >= max_keys {
                return CoalescedAdmission::suppress(0);
            }
            state.logs.insert(
                key,
                LogAggregate {
                    last_emit_tick: tick,
                    suppressed_count: 0,
                },
            );
            return CoalescedAdmission::emit(0);
        };

        if tick.saturating_sub(aggregate.last_emit_tick) >= LOG_WINDOW_TICKS {
            let suppressed_count = aggregate.suppressed_count;
            aggregate.last_emit_tick = tick;
            aggregate.suppressed_count = 0;
            CoalescedAdmission::emit(suppressed_count)
        } else {
            aggregate.suppressed_count = aggregate.suppressed_count.saturating_add(1);
            CoalescedAdmission::suppress(aggregate.suppressed_count)
        }
    }

    /// Explicit spelling for metric/log call sites.
    pub fn admit_log_summary<R>(
        &mut self,
        client: K,
        tick: u64,
        request_kind: R,
        reason: GateDenialReason,
    ) -> LogAdmission
    where
        R: Into<String>,
    {
        self.admit_log(client, tick, request_kind, reason)
    }
}

/// A commonly useful concrete spelling for numeric connection ids.
pub type ClientBudgetStore = BudgetStore<u64>;
/// Alias matching the terminology used by the ingress design.
pub type IngressBudgetStore<K> = BudgetStore<K>;

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT: &str = "client-a";
    const OTHER: &str = "client-b";
    const KIND: &str = "workbench_open";
    const OTHER_KIND: &str = "craft_start";

    fn feedback(store: &mut BudgetStore<&'static str>, tick: u64) -> FeedbackAdmission {
        store.admit_feedback(CLIENT, tick, KIND, GateDenialReason::OutOfReach)
    }

    #[test]
    fn initial_capacity_allows_32_and_rejects_33rd() {
        let mut store = BudgetStore::new(2);
        for expected in (0..INGRESS_BUCKET_CAPACITY).rev() {
            let result = store.admit(CLIENT, 0);
            assert_eq!(result, IngressAdmission::admitted(expected));
        }
        assert_eq!(store.admit(CLIENT, 0), IngressAdmission::rejected(0));
    }

    #[test]
    fn exact_exhaustion_is_fail_closed_without_negative_tokens() {
        let mut store = BudgetStore::new(1);
        for _ in 0..INGRESS_BUCKET_CAPACITY {
            assert!(store.admit(CLIENT, 10).admitted);
        }
        let denied = store.admit(CLIENT, 10);
        assert!(!denied.admitted);
        assert_eq!(denied.remaining_tokens, 0);
        assert_eq!(store.tokens_for(&CLIENT), Some(0));
    }

    #[test]
    fn refill_is_eight_per_tick_and_clamped_to_32() {
        let mut store = BudgetStore::new(1);
        for _ in 0..10 {
            assert!(store.admit(CLIENT, 0).admitted);
        }
        assert_eq!(store.admit(CLIENT, 1).remaining_tokens, 29);
        for _ in 0..29 {
            assert!(store.admit(CLIENT, 1).admitted);
        }
        assert!(!store.admit(CLIENT, 1).admitted);
        assert_eq!(store.admit(CLIENT, 2).remaining_tokens, 7);

        let mut full = BudgetStore::new(1);
        assert_eq!(full.admit(CLIENT, 0), IngressAdmission::admitted(31));
        assert_eq!(full.admit(CLIENT, u64::MAX), IngressAdmission::admitted(31));
        assert_eq!(full.admit(CLIENT, u64::MAX), IngressAdmission::admitted(30));
    }

    #[test]
    fn clients_have_isolated_buckets() {
        let mut store = BudgetStore::new(2);
        for _ in 0..INGRESS_BUCKET_CAPACITY {
            assert!(store.admit(CLIENT, 0).admitted);
        }
        assert!(!store.admit(CLIENT, 0).admitted);
        assert_eq!(store.admit(OTHER, 0), IngressAdmission::admitted(31));
    }

    #[test]
    fn feedback_emits_at_zero_then_once_after_20_ticks() {
        let mut store = BudgetStore::new(1);
        assert_eq!(feedback(&mut store, 0), CoalescedAdmission::emit(0));
        assert!(!feedback(&mut store, 1).emit);
        assert!(!feedback(&mut store, 19).emit);
        assert!(feedback(&mut store, 20).emit);
        assert!(!feedback(&mut store, 20).emit);
        assert!(feedback(&mut store, 40).emit);
    }

    #[test]
    fn feedback_coalesces_same_key_and_reports_suppressed_count() {
        let mut store = BudgetStore::new(1);
        assert_eq!(feedback(&mut store, 0), CoalescedAdmission::emit(0));
        assert_eq!(feedback(&mut store, 1), CoalescedAdmission::suppress(1));
        assert_eq!(feedback(&mut store, 2), CoalescedAdmission::suppress(2));
        assert_eq!(feedback(&mut store, 20), CoalescedAdmission::emit(2));
        assert_eq!(feedback(&mut store, 20), CoalescedAdmission::suppress(1));
    }

    #[test]
    fn feedback_keys_are_isolated_by_kind_and_reason() {
        let mut store = BudgetStore::new(1);
        assert_eq!(feedback(&mut store, 0), CoalescedAdmission::emit(0));
        assert_eq!(
            store.admit_feedback(CLIENT, 1, OTHER_KIND, GateDenialReason::OutOfReach),
            CoalescedAdmission::suppress(1)
        );
        assert_eq!(
            store.admit_feedback(CLIENT, 2, KIND, GateDenialReason::Busy),
            CoalescedAdmission::suppress(1)
        );
        assert_eq!(
            store.admit_feedback(CLIENT, 20, OTHER_KIND, GateDenialReason::OutOfReach),
            CoalescedAdmission::emit(1)
        );
        assert_eq!(
            store.admit_feedback(CLIENT, 20, KIND, GateDenialReason::Busy),
            CoalescedAdmission::suppress(1)
        );
    }

    #[test]
    fn logs_are_independent_and_have_100_tick_boundary() {
        let mut store = BudgetStore::new(1);
        assert_eq!(
            store.admit_log(CLIENT, 0, KIND, GateDenialReason::Busy),
            CoalescedAdmission::emit(0)
        );
        assert_eq!(
            store.admit_log(CLIENT, 99, KIND, GateDenialReason::Busy),
            CoalescedAdmission::suppress(1)
        );
        assert_eq!(
            store.admit_log(CLIENT, 100, KIND, GateDenialReason::Busy),
            CoalescedAdmission::emit(1)
        );
        assert_eq!(
            store.admit_log(CLIENT, 100, OTHER_KIND, GateDenialReason::Busy),
            CoalescedAdmission::emit(0)
        );
        assert_eq!(
            store.admit_log(CLIENT, 101, KIND, GateDenialReason::Busy),
            CoalescedAdmission::suppress(1)
        );
    }

    #[test]
    fn cleanup_restarts_with_a_clean_budget() {
        let mut store = BudgetStore::new(1);
        for _ in 0..INGRESS_BUCKET_CAPACITY {
            assert!(store.admit(CLIENT, 0).admitted);
        }
        assert!(!store.admit(CLIENT, 0).admitted);
        assert!(store.cleanup(&CLIENT));
        assert!(store.is_empty());
        assert_eq!(store.admit(CLIENT, 0), IngressAdmission::admitted(31));
    }

    #[test]
    fn retain_active_removes_disconnected_clients() {
        let mut store = BudgetStore::new(3);
        assert!(store.admit(CLIENT, 0).admitted);
        assert!(store.admit(OTHER, 0).admitted);
        store.retain_active([CLIENT]);
        assert!(store.contains_client(&CLIENT));
        assert!(!store.contains_client(&OTHER));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn client_limit_keeps_historical_state_bounded() {
        let mut store = BudgetStore::<String>::new(2);
        assert!(store.admit("first".to_owned(), 0).admitted);
        assert!(store.admit("second".to_owned(), 0).admitted);
        for index in 0..100 {
            assert!(!store.admit(format!("old-{index}"), 0).admitted);
        }
        assert_eq!(store.len(), 2);
        store.retain_active(std::iter::empty::<String>());
        assert!(store.is_empty());
    }

    #[test]
    fn tick_rollback_is_rejected_without_refill_or_emission() {
        let mut store = BudgetStore::new(1);
        assert_eq!(store.admit(CLIENT, 10), IngressAdmission::admitted(31));
        assert_eq!(store.admit(CLIENT, 9), IngressAdmission::rejected(31));
        assert_eq!(
            store.admit_feedback(CLIENT, 10, KIND, GateDenialReason::Busy),
            CoalescedAdmission::emit(0)
        );
        assert_eq!(
            store.admit_feedback(CLIENT, 9, KIND, GateDenialReason::Busy),
            CoalescedAdmission::suppress(0)
        );
        assert_eq!(
            store.admit_log(CLIENT, 9, KIND, GateDenialReason::Busy),
            CoalescedAdmission::suppress(0)
        );
    }

    #[test]
    fn invalid_kind_and_zero_aggregation_capacity_fail_closed() {
        let mut invalid = BudgetStore::new(1);
        assert_eq!(
            invalid.admit_feedback(CLIENT, 0, "", GateDenialReason::Busy),
            CoalescedAdmission::suppress(0)
        );
        assert!(invalid.is_empty());

        let mut no_keys = BudgetStore::with_limits(1, 0);
        assert_eq!(
            no_keys.admit_feedback(CLIENT, 0, KIND, GateDenialReason::Busy),
            CoalescedAdmission::suppress(0)
        );
        assert_eq!(
            no_keys.admit_log(CLIENT, 0, KIND, GateDenialReason::Busy),
            CoalescedAdmission::suppress(0)
        );
    }

    #[test]
    fn huge_tick_jumps_are_deterministic_and_clamped() {
        let mut store = BudgetStore::new(1);
        assert_eq!(store.admit(CLIENT, 0), IngressAdmission::admitted(31));
        assert_eq!(
            store.admit(CLIENT, u64::MAX),
            IngressAdmission::admitted(31)
        );
        assert_eq!(
            store.admit(CLIENT, u64::MAX - 1),
            IngressAdmission::rejected(31)
        );
    }
}
