//! 物资棺服务端权威授权。
//!
//! 裸 XYZ 只在同一逻辑位面内有意义。open、move 与 lifecycle 必须共享同一份
//! source/dimension/distance 判定，避免任一入口把客户端可见层或已有 session 当作授权。

use valence::prelude::DVec3;

use crate::reach::{DistanceRule, EXTERNAL_SESSION_MAX_BLOCKS, SUPPLY_COFFIN_OPEN_MAX_BLOCKS};
use crate::world::dimension::DimensionKind;

use super::ActiveSupplyCoffin;

#[allow(dead_code)]
pub(crate) const SUPPLY_COFFIN_OPEN_MAX_DISTANCE: f64 = SUPPLY_COFFIN_OPEN_MAX_BLOCKS;
#[allow(dead_code)]
pub(crate) const SUPPLY_COFFIN_SESSION_MAX_DISTANCE: f64 = EXTERNAL_SESSION_MAX_BLOCKS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupplyCoffinAuthorityFailure {
    MissingSource,
    MissingPlayerDimension,
    DimensionMismatch,
    OutOfRange,
}

pub(crate) fn authorize_supply_coffin_open(
    active: Option<&ActiveSupplyCoffin>,
    player_pos: DVec3,
    player_dimension: Option<DimensionKind>,
) -> Result<f64, SupplyCoffinAuthorityFailure> {
    authorize_supply_coffin(
        active,
        player_pos,
        player_dimension,
        DistanceRule::SUPPLY_COFFIN_OPEN,
    )
}

pub(crate) fn authorize_supply_coffin_session(
    active: Option<&ActiveSupplyCoffin>,
    player_pos: DVec3,
    player_dimension: Option<DimensionKind>,
) -> Result<f64, SupplyCoffinAuthorityFailure> {
    authorize_supply_coffin(
        active,
        player_pos,
        player_dimension,
        DistanceRule::EXTERNAL_SESSION,
    )
}

fn authorize_supply_coffin(
    active: Option<&ActiveSupplyCoffin>,
    player_pos: DVec3,
    player_dimension: Option<DimensionKind>,
    distance_rule: DistanceRule,
) -> Result<f64, SupplyCoffinAuthorityFailure> {
    let active = active.ok_or(SupplyCoffinAuthorityFailure::MissingSource)?;
    let player_dimension =
        player_dimension.ok_or(SupplyCoffinAuthorityFailure::MissingPlayerDimension)?;
    if player_dimension != active.dimension {
        return Err(SupplyCoffinAuthorityFailure::DimensionMismatch);
    }

    let distance = active.pos.distance(player_pos);
    if !distance_rule.allows(active.pos, player_pos) {
        return Err(SupplyCoffinAuthorityFailure::OutOfRange);
    }
    Ok(distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supply_coffin::SupplyCoffinGrade;

    fn active(dimension: DimensionKind, pos: DVec3) -> ActiveSupplyCoffin {
        ActiveSupplyCoffin {
            grade: SupplyCoffinGrade::Common,
            pos,
            dimension,
            spawned_at_wall_secs: 1,
        }
    }

    #[test]
    fn open_authority_accepts_same_dimension_at_exact_boundary() {
        let source = active(DimensionKind::Overworld, DVec3::ZERO);
        assert_eq!(
            authorize_supply_coffin_open(
                Some(&source),
                DVec3::new(SUPPLY_COFFIN_OPEN_MAX_DISTANCE, 0.0, 0.0),
                Some(DimensionKind::Overworld),
            ),
            Ok(SUPPLY_COFFIN_OPEN_MAX_DISTANCE)
        );
    }

    #[test]
    fn session_authority_accepts_same_dimension_at_exact_boundary() {
        let source = active(DimensionKind::Overworld, DVec3::ZERO);
        assert_eq!(
            authorize_supply_coffin_session(
                Some(&source),
                DVec3::new(SUPPLY_COFFIN_SESSION_MAX_DISTANCE, 0.0, 0.0),
                Some(DimensionKind::Overworld),
            ),
            Ok(SUPPLY_COFFIN_SESSION_MAX_DISTANCE)
        );
    }

    #[test]
    fn authority_rejects_missing_source() {
        assert_eq!(
            authorize_supply_coffin_session(None, DVec3::ZERO, Some(DimensionKind::Overworld)),
            Err(SupplyCoffinAuthorityFailure::MissingSource)
        );
    }

    #[test]
    fn authority_rejects_missing_player_dimension() {
        let source = active(DimensionKind::Overworld, DVec3::ZERO);
        assert_eq!(
            authorize_supply_coffin_session(Some(&source), DVec3::ZERO, None),
            Err(SupplyCoffinAuthorityFailure::MissingPlayerDimension)
        );
    }

    #[test]
    fn authority_rejects_dimension_mismatch_at_same_xyz() {
        let source = active(DimensionKind::Overworld, DVec3::ZERO);
        assert_eq!(
            authorize_supply_coffin_session(Some(&source), DVec3::ZERO, Some(DimensionKind::Tsy)),
            Err(SupplyCoffinAuthorityFailure::DimensionMismatch)
        );
    }

    #[test]
    fn authority_rejects_just_outside_each_boundary() {
        let source = active(DimensionKind::Overworld, DVec3::ZERO);
        for (actual, result) in [
            (
                SUPPLY_COFFIN_OPEN_MAX_DISTANCE + 0.001,
                authorize_supply_coffin_open(
                    Some(&source),
                    DVec3::new(SUPPLY_COFFIN_OPEN_MAX_DISTANCE + 0.001, 0.0, 0.0),
                    Some(DimensionKind::Overworld),
                ),
            ),
            (
                SUPPLY_COFFIN_SESSION_MAX_DISTANCE + 0.001,
                authorize_supply_coffin_session(
                    Some(&source),
                    DVec3::new(SUPPLY_COFFIN_SESSION_MAX_DISTANCE + 0.001, 0.0, 0.0),
                    Some(DimensionKind::Overworld),
                ),
            ),
        ] {
            assert_eq!(
                result,
                Err(SupplyCoffinAuthorityFailure::OutOfRange),
                "distance {actual} must be outside its authorization boundary"
            );
        }
    }

    #[test]
    fn authority_rejects_non_finite_positions() {
        let finite = active(DimensionKind::Overworld, DVec3::ZERO);
        let non_finite_source = active(
            DimensionKind::Overworld,
            DVec3::new(f64::INFINITY, 0.0, 0.0),
        );
        for result in [
            authorize_supply_coffin_session(
                Some(&finite),
                DVec3::new(f64::NAN, 0.0, 0.0),
                Some(DimensionKind::Overworld),
            ),
            authorize_supply_coffin_session(
                Some(&non_finite_source),
                DVec3::ZERO,
                Some(DimensionKind::Overworld),
            ),
        ] {
            assert_eq!(
                result,
                Err(SupplyCoffinAuthorityFailure::OutOfRange),
                "non-finite coordinates must fail closed"
            );
        }
    }
}
