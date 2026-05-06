//! plan-alchemy-recycle-v1 §3 — plot 级杂染的补灵触发与自然净化。

use super::plot::LingtianPlot;
use super::session::ReplenishSource;

use crate::alchemy::residue::contamination_triggers;

pub fn apply_dye_contamination_on_replenish(
    plot: &mut LingtianPlot,
    source: ReplenishSource,
    roll: f32,
) -> f32 {
    let ReplenishSource::PillResidue { residue_kind } = source else {
        return 0.0;
    };
    if !contamination_triggers(residue_kind, roll) {
        return 0.0;
    }
    plot.add_dye_contamination(residue_kind.spec().contamination_delta)
}

pub fn dye_contamination_decay_tick(plot: &mut LingtianPlot) {
    plot.decay_dye_contamination();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::residue::PillResidueKind;
    use valence::prelude::BlockPos;

    #[test]
    fn replenish_applies_residue_contamination_when_roll_hits() {
        let mut plot = LingtianPlot::new(BlockPos::new(0, 64, 0), None);
        let added = apply_dye_contamination_on_replenish(
            &mut plot,
            ReplenishSource::PillResidue {
                residue_kind: PillResidueKind::FailedPill,
            },
            0.29,
        );
        assert!((added - 0.1).abs() < 1e-6);
        assert!((plot.dye_contamination - 0.1).abs() < 1e-6);
    }

    #[test]
    fn replenish_ignores_clean_sources_and_missed_rolls() {
        let mut plot = LingtianPlot::new(BlockPos::new(0, 64, 0), None);
        assert_eq!(
            apply_dye_contamination_on_replenish(&mut plot, ReplenishSource::LingShui, 0.0),
            0.0
        );
        assert_eq!(
            apply_dye_contamination_on_replenish(
                &mut plot,
                ReplenishSource::PillResidue {
                    residue_kind: PillResidueKind::FailedPill,
                },
                0.30,
            ),
            0.0
        );
        assert_eq!(plot.dye_contamination, 0.0);
    }
}
