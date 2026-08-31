use valence::prelude::{BlockPos, DVec3, Entity, Position, Query};

use crate::world::dimension::{CurrentDimension, DimensionKind};

#[cfg(test)]
use std::sync::Mutex;

/// Canonical maximum distance for player-to-lingtian interactions, before the
/// block-center tolerance used by the interaction contract.
pub const LINGTIAN_INTERACT_MAX_DISTANCE: f64 = 4.0;
const LINGTIAN_INTERACT_TOLERANCE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LingtianInteractionDenial {
    MissingPosition,
    MissingDimension,
    WrongDimension,
    OutOfRange,
}

#[cfg(test)]
static DENIAL_LOGS: Mutex<Vec<(Entity, BlockPos, LingtianInteractionDenial)>> =
    Mutex::new(Vec::new());

pub fn is_lingtian_position_in_scope(
    actor_position: DVec3,
    actor_dimension: DimensionKind,
    target: BlockPos,
) -> bool {
    if actor_dimension != DimensionKind::Overworld {
        return false;
    }

    let target_center = DVec3::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y) + 0.5,
        f64::from(target.z) + 0.5,
    );
    actor_position.distance(target_center)
        <= LINGTIAN_INTERACT_MAX_DISTANCE + LINGTIAN_INTERACT_TOLERANCE
}

pub fn validate_lingtian_interaction(
    actor: Entity,
    target: BlockPos,
    positions: &Query<&Position>,
    dimensions: &Query<&CurrentDimension>,
) -> Result<(), LingtianInteractionDenial> {
    let actor_position = positions
        .get(actor)
        .map(|position| position.0)
        .map_err(|_| LingtianInteractionDenial::MissingPosition)?;
    let actor_dimension = dimensions
        .get(actor)
        .map(|dimension| dimension.0)
        .map_err(|_| LingtianInteractionDenial::MissingDimension)?;

    if actor_dimension != DimensionKind::Overworld {
        return Err(LingtianInteractionDenial::WrongDimension);
    }
    if !is_lingtian_position_in_scope(actor_position, actor_dimension, target) {
        return Err(LingtianInteractionDenial::OutOfRange);
    }
    Ok(())
}

pub fn log_lingtian_interaction_denial(
    context: &'static str,
    actor: Entity,
    target: BlockPos,
    reason: LingtianInteractionDenial,
) {
    #[cfg(test)]
    DENIAL_LOGS.lock().unwrap().push((actor, target, reason));

    match reason {
        LingtianInteractionDenial::MissingDimension => tracing::warn!(
            target: "bong::lingtian",
            context,
            actor = ?actor,
            target = ?target,
            reason = ?reason,
            "interaction request rejected"
        ),
        _ => tracing::debug!(
            target: "bong::lingtian",
            context,
            actor = ?actor,
            target = ?target,
            reason = ?reason,
            "interaction request rejected"
        ),
    }
}

#[cfg(test)]
pub fn denial_was_logged(
    actor: Entity,
    target: BlockPos,
    reason: LingtianInteractionDenial,
) -> bool {
    DENIAL_LOGS
        .lock()
        .unwrap()
        .contains(&(actor, target, reason))
}
