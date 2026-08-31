//! Contract-first primitives for C2S request gates.
//!
//! This module deliberately has no ECS queries, wire decoding, or mutation
//! hooks.  A gate specification describes what a later adapter must prove;
//! this module only evaluates the proofs that are already present in a
//! [`GateContext`].  In particular, an absent requester component is never
//! interpreted as a safe default.

pub mod budget;

pub use crate::reach::{
    DistanceMetric, DistanceRule, GatePosition, IntoGatePosition, DROPPED_LOOT_MAX_BLOCKS,
    EXTERNAL_SESSION_MAX_BLOCKS, NEARBY_INTERACT_MAX_BLOCKS, SUPPLY_COFFIN_OPEN_MAX_BLOCKS,
    WORKBENCH_MAX_BLOCKS,
};
use crate::world::dimension::DimensionKind;

/// A stable identity supplied by an authenticated authority adapter.
pub type GateAuthority = String;

impl DistanceRule {
    /// Check the position fields from a gate context, failing closed when a
    /// spatial profile has not been resolved yet.
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

/// How the target dimension relates to the requester dimension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DimensionRule {
    /// The target may be in any dimension, but both dimensions still need to
    /// be present so a missing component cannot bypass a gate.
    Any,
    /// Requester and target must be in the same logical dimension.
    #[default]
    Same,
    /// The requester and target must both be in the Overworld.
    OverworldOnly,
    /// Both sides must be in the specified dimension.
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

    pub fn allows_context(self, context: &GateContext) -> bool {
        self.allows(context.dimension, context.target_dimension)
    }
}

/// Which authenticated identity relationship a request must prove.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnershipRule {
    /// No target-owner comparison is required.  The requester authority is
    /// still required by [`GateSpec::check`].
    #[default]
    None,
    /// The target is public once it has been resolved.
    Any,
    /// Target authority must match the requester authority.
    Requester,
    /// Alias used by durable owner adapters.
    Owner,
    /// A session participant/owner adapter has supplied the same identity on
    /// both sides.
    Participant,
    /// Explicit authenticated-owner spelling for contract readers.
    AuthenticatedOwner,
}

impl OwnershipRule {
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

/// Authoritative target resolution mode.  These variants do not store
/// client-provided ids or entities; adapters resolve them before evaluation.
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

impl GateTarget {
    fn needs_target_position(self) -> bool {
        !matches!(self, Self::None)
    }

    fn needs_target_dimension(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// State adapters use this id list to express preconditions without putting
/// domain-specific state machines into the gate primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateGateId {
    PlayerAlive,
    TargetExists,
    SessionOpen,
    SessionParticipant,
    SessionOwner,
    OwnerAuthenticated,
    InventoryOpen,
    WorkbenchPresent,
    CraftSession,
    ForgeStepAdvance,
    ForgeTemperingHit,
    ExternalSession,
    Custom(&'static str),
}

/// Stable internal denial reasons.  The list is intentionally independent of
/// client-facing text; callers can safely fold target-resolution reasons.
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

/// Compatibility spelling used by the frozen `RequestGate::NoGate` contract.
pub type NoGateReason = GateDenialReason;

impl GateDenialReason {
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

/// Resolved requester/target facts supplied by an authority adapter.
///
/// All fields are optional because ECS queries and session lookups can fail.
/// `GateSpec::check` treats missing requester position, dimension, or
/// authority as [`GateDenialReason::MissingAuthorityContext`].
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

    /// Builder for callers that already have a concrete position type such as
    /// Valence's `DVec3`.
    pub fn with_target_position<P>(mut self, position: P) -> Self
    where
        P: IntoGatePosition,
    {
        self.target_position = Some(position.into_gate_position());
        self
    }

    pub fn with_requester_position<P>(mut self, position: P) -> Self
    where
        P: IntoGatePosition,
    {
        self.position = Some(position.into_gate_position());
        self
    }

    pub fn requester_complete(&self) -> bool {
        self.position.is_some() && self.dimension.is_some() && self.authority.is_some()
    }
}

/// A complete declaration consumed by the future request middleware.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateSpec {
    pub target: GateTarget,
    pub distance: DistanceRule,
    pub dimension: DimensionRule,
    pub ownership: OwnershipRule,
    pub state: &'static [StateGateId],
}

impl GateSpec {
    /// Evaluate the pure portion of a gate in deterministic order.
    pub fn check(&self, context: &GateContext) -> Result<(), GateDenialReason> {
        if !context.requester_complete() {
            return Err(GateDenialReason::MissingAuthorityContext);
        }

        if self.target.needs_target_dimension() && context.target_dimension.is_none() {
            return Err(GateDenialReason::TargetNotFound);
        }

        if !matches!(self.distance, DistanceRule::None)
            && (self.target.needs_target_position() && context.target_position.is_none())
        {
            return Err(GateDenialReason::TargetNotFound);
        }

        let dimension_allowed = if self.target.needs_target_dimension() {
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

        // State ids are declarations only.  Their domain adapters are applied
        // after this primitive and before the eventual mutation.
        Ok(())
    }

    pub fn allows(&self, context: &GateContext) -> bool {
        self.check(context).is_ok()
    }
}

/// A request either declares a gate or explicitly records why no gate can be
/// used.  `NoGate` never means "implicitly allow".
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

    #[test]
    fn euclidean_profile_accepts_inside_exact_boundary_and_rejects_outside() {
        let rule = DistanceRule::euclidean3d_squared(5.0);

        assert!(rule.allows([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]));
        assert!(rule.allows([0.0, 0.0, 0.0], [1.0, 2.0, 2.0]));
        assert!(!rule.allows([0.0, 0.0, 0.0], [3.0, 4.0 + EPSILON, 0.0]));
    }

    #[test]
    fn frozen_profiles_pin_metric_radius_and_named_constants() {
        assert_eq!(
            DistanceRule::workbench().profile_parts(),
            Some((DistanceMetric::Chebyshev3d, WORKBENCH_MAX_BLOCKS))
        );
        assert_eq!(
            DistanceRule::dropped_loot().profile_parts(),
            Some((DistanceMetric::Euclidean3dSquared, DROPPED_LOOT_MAX_BLOCKS))
        );
        assert_eq!(
            DistanceRule::supply_coffin_open().profile_parts(),
            Some((
                DistanceMetric::Euclidean3dSquared,
                SUPPLY_COFFIN_OPEN_MAX_BLOCKS
            ))
        );
        assert_eq!(
            DistanceRule::external_session().profile_parts(),
            Some((
                DistanceMetric::Euclidean3dSquared,
                EXTERNAL_SESSION_MAX_BLOCKS
            ))
        );
        assert_eq!(
            DistanceRule::nearby_interact().profile_parts(),
            Some((
                DistanceMetric::Euclidean3dSquared,
                NEARBY_INTERACT_MAX_BLOCKS
            ))
        );
        assert_eq!(DistanceRule::WORKBENCH, DistanceRule::workbench());
        assert_eq!(DistanceRule::DROPPED_LOOT, DistanceRule::dropped_loot());
        assert_eq!(
            DistanceRule::SUPPLY_COFFIN_OPEN,
            DistanceRule::supply_coffin_open()
        );
        assert_eq!(
            DistanceRule::EXTERNAL_SESSION,
            DistanceRule::external_session()
        );
        assert_eq!(
            DistanceRule::NEARBY_INTERACT,
            DistanceRule::nearby_interact()
        );
    }

    #[test]
    fn euclidean_squared_rejects_overflowing_out_of_range_distance() {
        let max = f64::MAX;
        let rule = DistanceRule::euclidean3d_squared(max);

        assert!(
            rule.allows([0.0, 0.0, 0.0], [max, 0.0, 0.0]),
            "a finite distance exactly at the largest finite radius must remain allowed"
        );
        assert!(
            !rule.allows([max, 0.0, 0.0], [-max, 0.0, 0.0]),
            "an overflowing distance beyond the finite radius must fail closed"
        );
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

        assert!(chebyshev.allows([0.0, 0.0, 0.0], [3.0, 3.0, 3.0]));
        assert!(!euclidean.allows([0.0, 0.0, 0.0], [3.0, 3.0, 3.0]));
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
    fn none_distance_rule_ignores_positions_and_invalid_profiles_fail_closed() {
        assert!(DistanceRule::None.allows([f64::NAN, f64::INFINITY, -1.0], [0.0, 0.0, 0.0]));
        assert!(DistanceRule::None.allows_context(&GateContext::default()));
        assert_eq!(DistanceRule::None.profile_parts(), None);

        assert!(DistanceRule::euclidean3d_squared(0.0).allows([1.0, -2.0, 3.0], [1.0, -2.0, 3.0]));
        assert!(!DistanceRule::euclidean3d_squared(-1.0).allows([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]));
        assert!(
            !DistanceRule::euclidean3d_squared(f64::NAN).allows([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        );
    }

    #[test]
    fn chebyshev_workbench_accepts_three_axis_boundary() {
        let rule = DistanceRule::workbench();

        assert!(rule.allows([0.0, 0.0, 0.0], [3.0, 3.0, 3.0]));
        assert!(rule.allows([0.0, 0.0, 0.0], [-3.0, 0.0, 2.5]));
        assert!(!rule.allows([0.0, 0.0, 0.0], [3.0 + EPSILON, 0.0, 0.0]));
    }

    #[test]
    fn non_finite_coordinates_are_rejected() {
        let chebyshev = DistanceRule::workbench();
        let euclidean = DistanceRule::euclidean3d_squared(3.0);

        assert!(!chebyshev.allows([f64::NAN, 0.0, 0.0], [0.0, 0.0, 0.0]));
        assert!(!chebyshev.allows([0.0, 0.0, 0.0], [0.0, f64::NAN, 0.0]));
        assert!(!euclidean.allows([f64::INFINITY, 0.0, 0.0], [0.0, 0.0, 0.0]));
    }

    #[test]
    fn missing_position_dimension_or_authority_rejects_by_default() {
        let spec = GateSpec {
            target: GateTarget::BlockPosition,
            distance: DistanceRule::workbench(),
            dimension: DimensionRule::Same,
            ownership: OwnershipRule::Requester,
            state: &[],
        };

        let complete = GateContext::new(
            Some([0.0, 0.0, 0.0]),
            Some(DimensionKind::Overworld),
            Some("player-1".to_owned()),
        )
        .with_target(
            Some([3.0, 3.0, 3.0]),
            Some(DimensionKind::Overworld),
            Some("player-1".to_owned()),
        );
        assert!(spec.allows(&complete));

        let missing_position = GateContext {
            position: None,
            ..complete.clone()
        };
        assert_eq!(
            spec.check(&missing_position),
            Err(GateDenialReason::MissingAuthorityContext)
        );

        let missing_dimension = GateContext {
            dimension: None,
            ..complete.clone()
        };
        assert_eq!(
            spec.check(&missing_dimension),
            Err(GateDenialReason::MissingAuthorityContext)
        );

        let missing_authority = GateContext {
            authority: None,
            ..complete
        };
        assert_eq!(
            spec.check(&missing_authority),
            Err(GateDenialReason::MissingAuthorityContext)
        );
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
    fn missing_target_facts_fail_closed_for_targeted_specs() {
        let spec = GateSpec {
            target: GateTarget::ProtocolEntityId,
            distance: DistanceRule::nearby_interact(),
            dimension: DimensionRule::Same,
            ownership: OwnershipRule::Any,
            state: &[],
        };
        let complete = complete_context();

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
    fn dimension_and_ownership_rules_fail_closed_on_missing_context() {
        let context = GateContext::default();
        assert!(!DimensionRule::Any.allows_context(&context));
        assert!(!OwnershipRule::Any.allows(&context));
        assert!(!DistanceRule::workbench().allows_context(&context));
    }

    #[test]
    fn request_gate_no_gate_is_an_explicit_denial() {
        let gate = RequestGate::NoGate(GateDenialReason::InvalidState);
        assert_eq!(
            gate.check(&GateContext::default()),
            Err(GateDenialReason::InvalidState)
        );
        assert!(!gate.allows(&GateContext::default()));
    }
}
