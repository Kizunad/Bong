use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::world::dimension::DimensionKind;

use super::events::{BackfireCauseV2, BackfireLevel, WoliuSkillId};

pub fn backfire_level_for_overflow(overflow_qi: f64, qi_max: f64) -> Option<BackfireLevel> {
    if !overflow_qi.is_finite() || overflow_qi <= f64::EPSILON {
        return None;
    }
    let max = if qi_max.is_finite() && qi_max > 0.0 {
        qi_max
    } else {
        1.0
    };
    let ratio = overflow_qi / max;
    Some(if ratio < 0.10 {
        BackfireLevel::Sensation
    } else if ratio < 0.30 {
        BackfireLevel::MicroTear
    } else if ratio < 0.60 {
        BackfireLevel::Torn
    } else {
        BackfireLevel::Severed
    })
}

pub fn forced_backfire(
    skill: WoliuSkillId,
    dimension: DimensionKind,
    active_seconds: f64,
) -> Option<(BackfireLevel, BackfireCauseV2)> {
    if skill == WoliuSkillId::Heart && dimension == DimensionKind::Tsy {
        return Some((BackfireLevel::Severed, BackfireCauseV2::TsyNegativeField));
    }
    if skill == WoliuSkillId::Heart && active_seconds >= 30.0 {
        return Some((
            BackfireLevel::Severed,
            BackfireCauseV2::VoidHeartTribulation,
        ));
    }
    None
}

pub fn apply_backfire_to_hand_meridians(
    meridians: &mut MeridianSystem,
    level: BackfireLevel,
) -> Option<MeridianId> {
    match level {
        BackfireLevel::Sensation => None,
        BackfireLevel::MicroTear => {
            let lung = meridians.get_mut(MeridianId::Lung);
            lung.integrity = (lung.integrity * 0.85).clamp(0.0, 1.0);
            lung.flow_capacity *= 0.85;
            Some(MeridianId::Lung)
        }
        BackfireLevel::Torn => {
            for id in [MeridianId::Lung, MeridianId::LargeIntestine] {
                let meridian = meridians.get_mut(id);
                meridian.integrity = (meridian.integrity * 0.5).clamp(0.0, 1.0);
                meridian.flow_capacity *= 0.5;
            }
            Some(MeridianId::Lung)
        }
        BackfireLevel::Severed => {
            let lung = meridians.get_mut(MeridianId::Lung);
            lung.integrity = 0.0;
            lung.flow_capacity = 0.0;
            lung.opened = false;
            Some(MeridianId::Lung)
        }
    }
}
