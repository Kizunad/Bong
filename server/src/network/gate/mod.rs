//! Contract-first primitives for C2S request gates.
//!
//! This module is deliberately independent from wire decoding, ECS queries,
//! and gameplay mutation.  An adapter resolves the requester's and target's
//! authoritative facts into [`GateContext`]; this module only evaluates the
//! pure declaration against those facts.

use crate::world::dimension::DimensionKind;
use valence::prelude::DVec3;

/// Small, allocation-free position representation used by the pure rules.
pub type GatePosition = [f64; 3];

/// Stable identity supplied by an authenticated authority adapter.
pub type GateAuthority = String;

/// Convert common server position types into the pure gate representation.
pub trait IntoGatePosition {
    fn into_gate_position(self) -> GatePosition;
}

impl IntoGatePosition for GatePosition {
    fn into_gate_position(self) -> GatePosition {
        self
    }
}

impl IntoGatePosition for (f64, f64, f64) {
    fn into_gate_position(self) -> GatePosition {
        [self.0, self.1, self.2]
    }
}

impl IntoGatePosition for DVec3 {
    fn into_gate_position(self) -> GatePosition {
        [self.x, self.y, self.z]
    }
}

/// Geometry used by a distance profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Compare squared Euclidean distance, avoiding a square root.
    Euclidean3dSquared,
    /// Compare the largest absolute difference on any axis.
    Chebyshev3d,
}

/// Spatial policy attached to a [`GateSpec`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DistanceRule {
    /// This request has no spatial target.
    #[default]
    None,
    /// A metric-aware, inclusive radius.  The caller must provide both parts
    /// so a radius cannot silently change the shape of the interaction area.
    Profile {
        metric: DistanceMetric,
        max_blocks: f64,
    },
}

/// Frozen Workbench reach in blocks.
pub const WORKBENCH_MAX_BLOCKS: f64 = 3.0;
/// Frozen dropped-loot reach in blocks.
pub const DROPPED_LOOT_MAX_BLOCKS: f64 = 2.5;
/// Frozen supply-coffin opening reach in blocks.
pub const SUPPLY_COFFIN_OPEN_MAX_BLOCKS: f64 = 4.5;
/// Frozen external-session tolerance in blocks.
pub const EXTERNAL_SESSION_MAX_BLOCKS: f64 = 6.5;
/// Frozen nearby-interaction reach in blocks.
pub const NEARBY_INTERACT_MAX_BLOCKS: f64 = 6.0;

impl DistanceRule {
    /// Generic metric-aware profile constructor.
    pub const fn profile(metric: DistanceMetric, max_blocks: f64) -> Self {
        Self::Profile { metric, max_blocks }
    }

    /// Frozen Workbench profile: Chebyshev3d, 3.0 blocks.
    pub const fn workbench() -> Self {
        Self::Profile {
            metric: DistanceMetric::Chebyshev3d,
            max_blocks: WORKBENCH_MAX_BLOCKS,
        }
    }

    /// Frozen DroppedLoot profile: Euclidean3dSquared, 2.5 blocks.
    pub const fn dropped_loot() -> Self {
        Self::euclidean3d_squared(DROPPED_LOOT_MAX_BLOCKS)
    }

    /// Frozen SupplyCoffinOpen profile: Euclidean3dSquared, 4.5 blocks.
    pub const fn supply_coffin_open() -> Self {
        Self::euclidean3d_squared(SUPPLY_COFFIN_OPEN_MAX_BLOCKS)
    }

    /// Frozen ExternalSession profile: Euclidean3dSquared, 6.5 blocks.
    pub const fn external_session() -> Self {
        Self::euclidean3d_squared(EXTERNAL_SESSION_MAX_BLOCKS)
    }

    /// Frozen NearbyInteract profile: Euclidean3dSquared, 6.0 blocks.
    pub const fn nearby_interact() -> Self {
        Self::euclidean3d_squared(NEARBY_INTERACT_MAX_BLOCKS)
    }

    /// Named constant for callers that prefer a value over a constructor.
    pub const WORKBENCH: Self = Self::workbench();
    /// Named constant for callers that prefer a value over a constructor.
    pub const DROPPED_LOOT: Self = Self::dropped_loot();
    /// Named constant for callers that prefer a value over a constructor.
    pub const SUPPLY_COFFIN_OPEN: Self = Self::supply_coffin_open();
    /// Named constant for callers that prefer a value over a constructor.
    pub const EXTERNAL_SESSION: Self = Self::external_session();
    /// Named constant for callers that prefer a value over a constructor.
    pub const NEARBY_INTERACT: Self = Self::nearby_interact();

    /// Construct a squared-Euclidean profile.
    pub const fn euclidean3d_squared(max_blocks: f64) -> Self {
        Self::Profile {
            metric: DistanceMetric::Euclidean3dSquared,
            max_blocks,
        }
    }

    /// Construct a Chebyshev profile.
    pub const fn chebyshev3d(max_blocks: f64) -> Self {
        Self::Profile {
            metric: DistanceMetric::Chebyshev3d,
            max_blocks,
        }
    }

    /// Return the metric and inclusive radius, if this is a spatial profile.
    pub const fn profile_parts(self) -> Option<(DistanceMetric, f64)> {
        match self {
            Self::None => None,
            Self::Profile { metric, max_blocks } => Some((metric, max_blocks)),
        }
    }

    /// Check a resolved requester-target pair.
    ///
    /// Both the boundary and negative coordinates are handled naturally by
    /// the delta calculation.  Invalid radii and non-finite coordinates fail
    /// closed so NaN cannot turn into an accidental allow.
    pub fn allows<P, Q>(self, requester: P, target: Q) -> bool
    where
        P: IntoGatePosition,
        Q: IntoGatePosition,
    {
        let requester = requester.into_gate_position();
        let target = target.into_gate_position();

        match self {
            Self::None => true,
            Self::Profile { metric, max_blocks } => {
                if !max_blocks.is_finite() || max_blocks < 0.0 {
                    return false;
                }
                if requester
                    .iter()
                    .chain(target.iter())
                    .any(|coordinate| !coordinate.is_finite())
                {
                    return false;
                }

                let dx = requester[0] - target[0];
                let dy = requester[1] - target[1];
                let dz = requester[2] - target[2];

                match metric {
                    DistanceMetric::Euclidean3dSquared => {
                        let radius_squared = max_blocks * max_blocks;
                        let distance_squared = dx * dx + dy * dy + dz * dz;
                        distance_squared <= radius_squared
                    }
                    DistanceMetric::Chebyshev3d => {
                        let distance = dx.abs().max(dy.abs()).max(dz.abs());
                        distance <= max_blocks
                    }
                }
            }
        }
    }

    /// Check positions already resolved into a [`GateContext`].
    pub fn allows_context(self, context: &GateContext) -> bool {
        match self {
            Self::None => true,
            Self::Profile { .. } => match (context.position, context.target_position) {
                (Some(requester), Some(target)) => self.allows(requester, target),
                _ => false,
            },
        }
    }
}

/// How the target dimension relates to the requester's dimension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DimensionRule {
    /// Both dimensions must be present, but they may differ.
    Any,
    /// Requester and target must share a dimension.
    #[default]
    Same,
    /// Both requester and target must be in the Overworld.
    OverworldOnly,
    /// Both requester and target must be in the given dimension.
    Exact(DimensionKind),
}

impl DimensionRule {
    /// Evaluate two resolved dimensions.  Missing dimensions always reject.
    pub fn allows(self, requester: Option<DimensionKind>, target: Option<DimensionKind>) -> bool {
        let (Some(requester), Some(target)) = (requester, target) else {
            return false;
        };

        match self {
            Self::Any => true,
            Self::Same => requester == target,
            Self::OverworldOnly => {
                requester == DimensionKind::Overworld && target == DimensionKind::Overworld
            }
            Self::Exact(expected) => requester == expected && target == expected,
        }
    }

    /// Evaluate the dimensions stored in a context.
    pub fn allows_context(self, context: &GateContext) -> bool {
        self.allows(context.dimension, context.target_dimension)
    }
}

/// Authenticated ownership relationship required by a gate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnershipRule {
    /// No target-owner comparison is needed.
    #[default]
    None,
    /// The resolved target is public.
    Any,
    /// Target authority must equal the requester's authority.
    Requester,
    /// Durable owner authority must equal the requester's authority.
    Owner,
    /// Session participant authority must equal the requester's authority.
    Participant,
    /// Explicit spelling for an authenticated owner adapter.
    AuthenticatedOwner,
}

impl OwnershipRule {
    /// Evaluate ownership facts.  A requester authority is required even for
    /// public/no-owner rules; [`GateSpec::check`] reports that missing fact as
    /// [`GateDenialReason::MissingAuthorityContext`] before calling this.
    pub fn allows(self, context: &GateContext) -> bool {
        let Some(requester) = context.authority.as_deref() else {
            return false;
        };

        match self {
            Self::None | Self::Any => true,
            Self::Requester | Self::Owner | Self::Participant | Self::AuthenticatedOwner => {
                context.target_authority.as_deref() == Some(requester)
            }
        }
    }
}

/// Authoritative target resolution mode.  These variants describe how a
/// later adapter resolves a target; they do not store client-provided ids.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GateTarget {
    #[default]
    None,
    RequestBlockPosition,
    BlockPosition,
    ProtocolEntityId,
    EntityId,
    Uuid,
    InventoryInstance,
    SessionId,
    PlayerId,
    ZoneId,
}

/// Domain state prerequisites declared by a gate and evaluated by a later
/// domain adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateGateId {
    PlayerAlive,
    TargetExists,
    SessionOpen,
    SessionParticipant,
    OwnerAuthenticated,
    InventoryOpen,
    WorkbenchPresent,
    CraftSession,
    ForgeStepAdvance,
    ForgeTemperingHit,
    ExternalSession,
    Custom(&'static str),
}

/// Stable internal denial reasons.  These are not client-facing text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GateDenialReason {
    UnsupportedVersion,
    MissingAuthorityContext,
    TargetNotFound,
    NotVisible,
    WrongDimension,
    OutOfReach,
    NotOwner,
    InvalidState,
    Busy,
    Expired,
    Conflict,
    RateLimited,
}

impl GateDenialReason {
    /// Stable machine-readable spelling for logs and future reject adapters.
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::MissingAuthorityContext => "missing_authority_context",
            Self::TargetNotFound => "target_not_found",
            Self::NotVisible => "not_visible",
            Self::WrongDimension => "wrong_dimension",
            Self::OutOfReach => "out_of_reach",
            Self::NotOwner => "not_owner",
            Self::InvalidState => "invalid_state",
            Self::Busy => "busy",
            Self::Expired => "expired",
            Self::Conflict => "conflict",
            Self::RateLimited => "rate_limited",
        }
    }
}

impl std::fmt::Display for GateDenialReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Reason carried by an explicit `RequestGate::NoGate` declaration.
///
/// It is an alias of the stable denial vocabulary for this contract-first
/// slice: a no-gate declaration is never implicit allow, and the reason is
/// still machine-readable by the future middleware.
pub type NoGateReason = GateDenialReason;

/// Facts resolved by an authority adapter before a [`GateSpec`] is checked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GateContext {
    pub position: Option<GatePosition>,
    pub dimension: Option<DimensionKind>,
    pub authority: Option<GateAuthority>,
    pub target_position: Option<GatePosition>,
    pub target_dimension: Option<DimensionKind>,
    pub target_authority: Option<GateAuthority>,
}

impl GateContext {
    /// Create a context with requester facts and no resolved target.
    pub fn new(
        position: Option<GatePosition>,
        dimension: Option<DimensionKind>,
        authority: Option<GateAuthority>,
    ) -> Self {
        Self {
            position,
            dimension,
            authority,
            ..Self::default()
        }
    }

    /// Add resolved target facts.
    pub fn with_target(
        mut self,
        position: Option<GatePosition>,
        dimension: Option<DimensionKind>,
        authority: Option<GateAuthority>,
    ) -> Self {
        self.target_position = position;
        self.target_dimension = dimension;
        self.target_authority = authority;
        self
    }

    /// Set a requester position from a common server position type.
    pub fn with_requester_position<P>(mut self, position: P) -> Self
    where
        P: IntoGatePosition,
    {
        self.position = Some(position.into_gate_position());
        self
    }

    /// Set a target position from a common server position type.
    pub fn with_target_position<P>(mut self, position: P) -> Self
    where
        P: IntoGatePosition,
    {
        self.target_position = Some(position.into_gate_position());
        self
    }

    fn requester_complete(&self) -> bool {
        self.position.is_some() && self.dimension.is_some() && self.authority.is_some()
    }
}

/// Complete, immutable declaration consumed by a future request middleware.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateSpec {
    pub target: GateTarget,
    pub distance: DistanceRule,
    pub dimension: DimensionRule,
    pub ownership: OwnershipRule,
    pub state: &'static [StateGateId],
}

impl GateSpec {
    /// Check the pure gate facts in deterministic order.
    pub fn check(&self, context: &GateContext) -> Result<(), GateDenialReason> {
        if !context.requester_complete() {
            return Err(GateDenialReason::MissingAuthorityContext);
        }

        let has_target = !matches!(self.target, GateTarget::None);
        if has_target && context.target_dimension.is_none() {
            return Err(GateDenialReason::TargetNotFound);
        }
        if has_target
            && !matches!(self.distance, DistanceRule::None)
            && context.target_position.is_none()
        {
            return Err(GateDenialReason::TargetNotFound);
        }

        let dimension_allowed = if has_target {
            self.dimension.allows_context(context)
        } else {
            // A target-less request has no target dimension to compare.  The
            // requester's dimension is still required by requester_complete.
            match self.dimension {
                DimensionRule::Any | DimensionRule::Same => true,
                DimensionRule::OverworldOnly => context.dimension == Some(DimensionKind::Overworld),
                DimensionRule::Exact(expected) => context.dimension == Some(expected),
            }
        };
        if !dimension_allowed {
            return Err(GateDenialReason::WrongDimension);
        }

        if !self.distance.allows_context(context) {
            return Err(GateDenialReason::OutOfReach);
        }
        if !self.ownership.allows(context) {
            return Err(GateDenialReason::NotOwner);
        }

        // `state` is a declaration only.  Domain adapters evaluate these ids
        // immediately before dispatch and mutation in a later slice.
        Ok(())
    }

    pub fn allows(&self, context: &GateContext) -> bool {
        self.check(context).is_ok()
    }
}

/// Either a gate specification or an explicit fail-closed no-gate reason.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RequestGate {
    Spec(GateSpec),
    NoGate(NoGateReason),
}

impl RequestGate {
    pub fn check(&self, context: &GateContext) -> Result<(), GateDenialReason> {
        match self {
            Self::Spec(spec) => spec.check(context),
            Self::NoGate(reason) => Err(*reason),
        }
    }

    pub fn allows(&self, context: &GateContext) -> bool {
        self.check(context).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1.0e-9;

    fn profile_parts(rule: DistanceRule) -> (DistanceMetric, f64) {
        rule.profile_parts()
            .expect("frozen profile must carry metric and radius")
    }

    #[test]
    fn frozen_profiles_pin_metric_and_radius() {
        assert_eq!(
            profile_parts(DistanceRule::workbench()),
            (DistanceMetric::Chebyshev3d, WORKBENCH_MAX_BLOCKS)
        );
        assert_eq!(
            profile_parts(DistanceRule::dropped_loot()),
            (DistanceMetric::Euclidean3dSquared, DROPPED_LOOT_MAX_BLOCKS)
        );
        assert_eq!(
            profile_parts(DistanceRule::supply_coffin_open()),
            (
                DistanceMetric::Euclidean3dSquared,
                SUPPLY_COFFIN_OPEN_MAX_BLOCKS
            )
        );
        assert_eq!(
            profile_parts(DistanceRule::external_session()),
            (
                DistanceMetric::Euclidean3dSquared,
                EXTERNAL_SESSION_MAX_BLOCKS
            )
        );
        assert_eq!(
            profile_parts(DistanceRule::nearby_interact()),
            (
                DistanceMetric::Euclidean3dSquared,
                NEARBY_INTERACT_MAX_BLOCKS
            )
        );
        assert_eq!(DistanceRule::WORKBENCH, DistanceRule::workbench());
        assert_eq!(DistanceRule::DROPPED_LOOT, DistanceRule::dropped_loot());
    }

    #[test]
    fn euclidean_squared_is_inclusive_and_uses_all_three_axes() {
        let rule = DistanceRule::euclidean3d_squared(5.0);

        assert!(rule.allows([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]));
        assert!(rule.allows([0.0, 0.0, 0.0], [1.0, 2.0, 2.0]));
        assert!(!rule.allows([0.0, 0.0, 0.0], [3.0, 4.0 + EPSILON, 0.0]));
    }

    #[test]
    fn negative_coordinates_are_measured_by_delta_not_absolute_location() {
        let euclidean = DistanceRule::euclidean3d_squared(5.0);
        let chebyshev = DistanceRule::chebyshev3d(3.0);

        assert!(euclidean.allows([-10.0, -10.0, -10.0], [-7.0, -6.0, -10.0]));
        assert!(chebyshev.allows([-10.0, -10.0, -10.0], [-13.0, -7.0, -12.5]));
        assert!(!chebyshev.allows([-10.0, -10.0, -10.0], [-13.0 - EPSILON, -10.0, -10.0]));
    }

    #[test]
    fn metric_difference_is_pinned_at_diagonal_and_axis_cases() {
        let chebyshev = DistanceRule::workbench();
        let euclidean = DistanceRule::euclidean3d_squared(3.0);

        // Chebyshev accepts the cube corner; Euclidean does not.
        assert!(chebyshev.allows([0.0, 0.0, 0.0], [3.0, 3.0, 3.0]));
        assert!(!euclidean.allows([0.0, 0.0, 0.0], [3.0, 3.0, 3.0]));
        // Euclidean accepts a 3-4-0 point at radius 5; Chebyshev radius 3
        // rejects the same shape because one axis exceeds the cube.
        assert!(DistanceRule::euclidean3d_squared(5.0).allows([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]));
        assert!(!chebyshev.allows([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]));
    }

    #[test]
    fn frozen_profile_boundaries_are_inclusive() {
        assert!(DistanceRule::dropped_loot()
            .allows([0.0, 0.0, 0.0], [DROPPED_LOOT_MAX_BLOCKS, 0.0, 0.0]));
        assert!(DistanceRule::supply_coffin_open()
            .allows([0.0, 0.0, 0.0], [0.0, SUPPLY_COFFIN_OPEN_MAX_BLOCKS, 0.0]));
        assert!(DistanceRule::external_session()
            .allows([0.0, 0.0, 0.0], [0.0, 0.0, EXTERNAL_SESSION_MAX_BLOCKS]));
        assert!(DistanceRule::nearby_interact()
            .allows([0.0, 0.0, 0.0], [NEARBY_INTERACT_MAX_BLOCKS, 0.0, 0.0]));
    }

    #[test]
    fn none_distance_rule_does_not_require_or_inspect_positions() {
        let rule = DistanceRule::None;

        assert!(rule.allows([f64::NAN, f64::INFINITY, -1.0], [0.0, 0.0, 0.0]));
        assert!(rule.allows_context(&GateContext::default()));
        assert_eq!(rule.profile_parts(), None);
    }

    #[test]
    fn invalid_profiles_and_non_finite_coordinates_fail_closed() {
        let zero = DistanceRule::euclidean3d_squared(0.0);

        assert!(zero.allows([1.0, -2.0, 3.0], [1.0, -2.0, 3.0]));
        assert!(!DistanceRule::euclidean3d_squared(-1.0).allows([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]));
        assert!(
            !DistanceRule::euclidean3d_squared(f64::NAN).allows([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        );
        assert!(!DistanceRule::workbench().allows([f64::NAN, 0.0, 0.0], [0.0, 0.0, 0.0]));
        assert!(!DistanceRule::workbench().allows([0.0, 0.0, 0.0], [0.0, f64::INFINITY, 0.0]));
    }

    fn complete_context() -> GateContext {
        GateContext::new(
            Some([0.0, 0.0, 0.0]),
            Some(DimensionKind::Overworld),
            Some("player-1".to_owned()),
        )
        .with_target(
            Some([3.0, 3.0, 3.0]),
            Some(DimensionKind::Overworld),
            Some("player-1".to_owned()),
        )
    }

    #[test]
    fn gate_spec_checks_target_dimension_distance_and_ownership() {
        let spec = GateSpec {
            target: GateTarget::BlockPosition,
            distance: DistanceRule::workbench(),
            dimension: DimensionRule::Same,
            ownership: OwnershipRule::Requester,
            state: &[StateGateId::PlayerAlive],
        };
        let context = complete_context();

        assert!(spec.allows(&context));
        assert_eq!(
            spec.check(&GateContext {
                target_dimension: Some(DimensionKind::Tsy),
                ..context.clone()
            }),
            Err(GateDenialReason::WrongDimension)
        );
        assert_eq!(
            spec.check(&GateContext {
                target_position: Some([3.0 + EPSILON, 0.0, 0.0]),
                ..context.clone()
            }),
            Err(GateDenialReason::OutOfReach)
        );
        assert_eq!(
            spec.check(&GateContext {
                target_authority: Some("other-player".to_owned()),
                ..context
            }),
            Err(GateDenialReason::NotOwner)
        );
    }

    #[test]
    fn missing_requester_or_target_facts_fail_closed() {
        let spec = GateSpec {
            target: GateTarget::ProtocolEntityId,
            distance: DistanceRule::nearby_interact(),
            dimension: DimensionRule::Same,
            ownership: OwnershipRule::Any,
            state: &[],
        };
        let complete = complete_context();

        for missing in [
            GateContext {
                position: None,
                ..complete.clone()
            },
            GateContext {
                dimension: None,
                ..complete.clone()
            },
            GateContext {
                authority: None,
                ..complete.clone()
            },
        ] {
            assert_eq!(
                spec.check(&missing),
                Err(GateDenialReason::MissingAuthorityContext),
                "missing requester authority context must not pass a gate"
            );
        }

        assert_eq!(
            spec.check(&GateContext {
                target_position: None,
                ..complete.clone()
            }),
            Err(GateDenialReason::TargetNotFound)
        );
        assert_eq!(
            spec.check(&GateContext {
                target_dimension: None,
                ..complete
            }),
            Err(GateDenialReason::TargetNotFound)
        );
    }

    #[test]
    fn dimension_and_ownership_rules_fail_closed_on_missing_context() {
        let empty = GateContext::default();

        assert!(!DimensionRule::Any.allows_context(&empty));
        assert!(!OwnershipRule::Any.allows(&empty));
        assert!(!DistanceRule::workbench().allows_context(&empty));
        assert!(DistanceRule::None.allows_context(&empty));
    }

    #[test]
    fn targetless_spec_can_use_none_distance_without_target_facts() {
        let spec = GateSpec {
            target: GateTarget::None,
            distance: DistanceRule::None,
            dimension: DimensionRule::Same,
            ownership: OwnershipRule::None,
            state: &[],
        };
        let context = GateContext::new(
            Some([0.0, 0.0, 0.0]),
            Some(DimensionKind::Tsy),
            Some("player-1".to_owned()),
        );

        assert!(spec.allows(&context));
    }

    #[test]
    fn request_gate_no_gate_is_an_explicit_denial() {
        let gate = RequestGate::NoGate(GateDenialReason::InvalidState);

        assert_eq!(
            gate.check(&GateContext::default()),
            Err(GateDenialReason::InvalidState)
        );
        assert!(!gate.allows(&GateContext::default()));
        assert_eq!(GateDenialReason::OutOfReach.code(), "out_of_reach");
    }
}
