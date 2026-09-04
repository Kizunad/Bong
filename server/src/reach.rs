//! Shared spatial reach policies.
//!
//! This module is independent from request ingress and domain adapters so
//! gameplay domains can consume the same authoritative distance profiles
//! without depending on `network`.

use valence::prelude::DVec3;

/// A position accepted by the pure distance predicates.
pub type GatePosition = [f64; 3];

/// Convert server position types into the small representation used here.
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

/// The way a distance profile measures a requester-target pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Compare the squared Euclidean distance, avoiding a square root.
    Euclidean3dSquared,
    /// Compare the largest absolute component difference.
    Chebyshev3d,
}

/// Distance policy with a metric and inclusive radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DistanceRule {
    /// No spatial target is required by this gate.
    #[default]
    None,
    /// A named policy's metric and radius.
    Profile {
        metric: DistanceMetric,
        max_blocks: f64,
    },
}

impl DistanceRule {
    /// Construct a profile without allowing a caller to forget its metric.
    pub const fn profile(metric: DistanceMetric, max_blocks: f64) -> Self {
        Self::Profile { metric, max_blocks }
    }

    /// The server-authoritative Workbench reach profile.
    pub const fn workbench() -> Self {
        Self::Profile {
            metric: DistanceMetric::Chebyshev3d,
            max_blocks: WORKBENCH_MAX_BLOCKS,
        }
    }

    /// The existing dropped-loot pickup reach profile.
    pub const fn dropped_loot() -> Self {
        Self::euclidean3d_squared(DROPPED_LOOT_MAX_BLOCKS)
    }

    /// The existing supply-coffin opening reach profile.
    pub const fn supply_coffin_open() -> Self {
        Self::euclidean3d_squared(SUPPLY_COFFIN_OPEN_MAX_BLOCKS)
    }

    /// The existing external-session tolerance profile.
    pub const fn external_session() -> Self {
        Self::euclidean3d_squared(EXTERNAL_SESSION_MAX_BLOCKS)
    }

    /// The common nearby-interaction reach profile.
    pub const fn nearby_interact() -> Self {
        Self::euclidean3d_squared(NEARBY_INTERACT_MAX_BLOCKS)
    }

    /// The lingtian interaction reach profile.
    pub const fn lingtian_interact() -> Self {
        Self::euclidean3d_squared(LINGTIAN_INTERACT_MAX_BLOCKS)
    }

    /// Named constants for callers that prefer frozen values over constructors.
    pub const WORKBENCH: Self = Self::workbench();
    pub const DROPPED_LOOT: Self = Self::dropped_loot();
    pub const SUPPLY_COFFIN_OPEN: Self = Self::supply_coffin_open();
    pub const EXTERNAL_SESSION: Self = Self::external_session();
    pub const NEARBY_INTERACT: Self = Self::nearby_interact();
    pub const LINGTIAN_INTERACT: Self = Self::lingtian_interact();

    /// Euclidean profile constructor. The comparison remains squared.
    pub const fn euclidean3d_squared(max_blocks: f64) -> Self {
        Self::Profile {
            metric: DistanceMetric::Euclidean3dSquared,
            max_blocks,
        }
    }

    /// Chebyshev profile constructor.
    pub const fn chebyshev3d(max_blocks: f64) -> Self {
        Self::Profile {
            metric: DistanceMetric::Chebyshev3d,
            max_blocks,
        }
    }

    /// Return the metric and radius for a profile.
    pub const fn profile_parts(self) -> Option<(DistanceMetric, f64)> {
        match self {
            Self::None => None,
            Self::Profile { metric, max_blocks } => Some((metric, max_blocks)),
        }
    }

    /// Check an already-resolved requester and target position.
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
                        // Normalize by the largest delta before squaring so
                        // overflowing squared distances still fail closed.
                        let scale = dx.abs().max(dy.abs()).max(dz.abs());
                        if !scale.is_finite() {
                            return false;
                        }
                        if scale == 0.0 {
                            return true;
                        }

                        let normalized_dx = dx / scale;
                        let normalized_dy = dy / scale;
                        let normalized_dz = dz / scale;
                        let normalized_distance_squared = normalized_dx * normalized_dx
                            + normalized_dy * normalized_dy
                            + normalized_dz * normalized_dz;
                        let normalized_radius = max_blocks / scale;

                        normalized_radius >= 2.0
                            || normalized_distance_squared <= normalized_radius * normalized_radius
                    }
                    DistanceMetric::Chebyshev3d => {
                        let chebyshev = dx.abs().max(dy.abs()).max(dz.abs());
                        chebyshev <= max_blocks
                    }
                }
            }
        }
    }
}

pub const WORKBENCH_MAX_BLOCKS: f64 = 3.0;
pub const DROPPED_LOOT_MAX_BLOCKS: f64 = 2.5;
pub const SUPPLY_COFFIN_OPEN_MAX_BLOCKS: f64 = 4.5;
pub const EXTERNAL_SESSION_MAX_BLOCKS: f64 = 6.5;
pub const NEARBY_INTERACT_MAX_BLOCKS: f64 = 6.0;
pub const LINGTIAN_INTERACT_MAX_BLOCKS: f64 = 4.5;
