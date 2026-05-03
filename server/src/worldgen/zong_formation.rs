//! plan-terrain-jiuzong-ruin-v1 §7 P2 — 九宗故地阵核激活模型。

use crate::schema::pseudo_vein::PseudoVeinSeasonV1;
use crate::schema::zong_formation::ZongCoreActivationV1;

pub const ZONG_FORMATION_ACTIVE_QI: f64 = 0.60;
pub const ZONG_FORMATION_DEFAULT_BASE_QI: f64 = 0.40;
pub const ZONG_FORMATION_RADIUS_BLOCKS: f64 = 60.0;
pub const ZONG_FORMATION_DURATION_TICKS: u64 = 30 * 60 * 20;
pub const ZONG_FORMATION_NARRATION_RADIUS_BLOCKS: u32 = 1000;
pub const ZONG_FORMATION_WILD_FORMATION_KIND: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZongmenOrigin {
    Bloodstream = 1,
    Beiling = 2,
    Nanyuan = 3,
    Chixia = 4,
    Xuanshui = 5,
    Taichu = 6,
    Youan = 7,
}

impl ZongmenOrigin {
    pub const ALL: [Self; 7] = [
        Self::Bloodstream,
        Self::Beiling,
        Self::Nanyuan,
        Self::Chixia,
        Self::Xuanshui,
        Self::Taichu,
        Self::Youan,
    ];

    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            1 => Some(Self::Bloodstream),
            2 => Some(Self::Beiling),
            3 => Some(Self::Nanyuan),
            4 => Some(Self::Chixia),
            5 => Some(Self::Xuanshui),
            6 => Some(Self::Taichu),
            7 => Some(Self::Youan),
            _ => None,
        }
    }

    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn zone_id(self) -> &'static str {
        match self {
            Self::Bloodstream => "jiuzong_bloodstream_ruin",
            Self::Beiling => "jiuzong_beiling_ruin",
            Self::Nanyuan => "jiuzong_nanyuan_ruin",
            Self::Chixia => "jiuzong_chixia_ruin",
            Self::Xuanshui => "jiuzong_xuanshui_ruin",
            Self::Taichu => "jiuzong_taichu_ruin",
            Self::Youan => "jiuzong_youan_ruin",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Bloodstream => "血溪",
            Self::Beiling => "北陵",
            Self::Nanyuan => "南渊",
            Self::Chixia => "赤霞",
            Self::Xuanshui => "玄水",
            Self::Taichu => "太初",
            Self::Youan => "幽暗",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZongFormationCharge {
    SpiritHerb,
    BoneCoin,
    TrueQi,
}

impl ZongFormationCharge {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::SpiritHerb => "spirit_herb",
            Self::BoneCoin => "bone_coin",
            Self::TrueQi => "true_qi",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZongFormationCore {
    pub core_id: String,
    pub zone_id: String,
    pub origin: ZongmenOrigin,
    pub center_xz: [f64; 2],
    pub activated_until: u64,
    pub base_qi: f64,
    pub charge_required: Vec<ZongFormationCharge>,
}

impl ZongFormationCore {
    pub fn new(origin: ZongmenOrigin, center_xz: [f64; 2]) -> Self {
        let zone_id = origin.zone_id().to_string();
        Self {
            core_id: format!("{zone_id}:core:0"),
            zone_id,
            origin,
            center_xz,
            activated_until: 0,
            base_qi: ZONG_FORMATION_DEFAULT_BASE_QI,
            charge_required: vec![
                ZongFormationCharge::SpiritHerb,
                ZongFormationCharge::BoneCoin,
                ZongFormationCharge::TrueQi,
            ],
        }
    }

    pub fn is_active(&self, current_tick: u64) -> bool {
        current_tick < self.activated_until
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZongFormationSpawnPressure {
    pub mutant_beast_min: u8,
    pub mutant_beast_max: u8,
    pub daoxiang_probability: f64,
}

pub trait SeasonTideMultiplier {
    fn tide_multiplier(self) -> f64;
}

impl SeasonTideMultiplier for PseudoVeinSeasonV1 {
    fn tide_multiplier(self) -> f64 {
        match self {
            PseudoVeinSeasonV1::SummerToWinter | PseudoVeinSeasonV1::WinterToSummer => 2.0,
            PseudoVeinSeasonV1::Summer | PseudoVeinSeasonV1::Winter => 1.0,
        }
    }
}

pub fn activate_zong_formation_core(
    core: &mut ZongFormationCore,
    current_tick: u64,
    paid: &[ZongFormationCharge],
) -> Result<ZongCoreActivationV1, String> {
    for required in &core.charge_required {
        if !paid.contains(required) {
            return Err(format!("missing required charge `{}`", required.as_wire()));
        }
    }

    core.activated_until = current_tick + ZONG_FORMATION_DURATION_TICKS;
    Ok(ZongCoreActivationV1 {
        v: 1,
        zone_id: core.zone_id.clone(),
        core_id: core.core_id.clone(),
        origin_id: core.origin.id(),
        center_xz: core.center_xz,
        activated_until_tick: core.activated_until,
        base_qi: core.base_qi,
        active_qi: ZONG_FORMATION_ACTIVE_QI,
        charge_required: core
            .charge_required
            .iter()
            .map(|charge| charge.as_wire().to_string())
            .collect(),
        narration_radius_blocks: ZONG_FORMATION_NARRATION_RADIUS_BLOCKS,
        anomaly_kind: ZONG_FORMATION_WILD_FORMATION_KIND,
    })
}

pub fn zong_formation_qi_at_distance(
    core: &ZongFormationCore,
    current_tick: u64,
    distance_blocks: f64,
) -> f64 {
    if !core.is_active(current_tick) || distance_blocks > ZONG_FORMATION_RADIUS_BLOCKS {
        return core.base_qi;
    }
    ZONG_FORMATION_ACTIVE_QI
}

pub fn zong_core_self_activation_rate(base_probability: f64, season: PseudoVeinSeasonV1) -> f64 {
    (base_probability * season.tide_multiplier()).clamp(0.0, 1.0)
}

pub fn zong_formation_spawn_pressure(active: bool) -> ZongFormationSpawnPressure {
    if active {
        ZongFormationSpawnPressure {
            mutant_beast_min: 1,
            mutant_beast_max: 2,
            daoxiang_probability: 0.30,
        }
    } else {
        ZongFormationSpawnPressure {
            mutant_beast_min: 0,
            mutant_beast_max: 0,
            daoxiang_probability: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_requires_all_plan_charges_and_pins_thirty_minutes() {
        let mut core = ZongFormationCore::new(ZongmenOrigin::Bloodstream, [5500.0, -1000.0]);

        assert!(
            activate_zong_formation_core(&mut core, 0, &[ZongFormationCharge::SpiritHerb]).is_err()
        );

        let event = activate_zong_formation_core(
            &mut core,
            0,
            &[
                ZongFormationCharge::SpiritHerb,
                ZongFormationCharge::BoneCoin,
                ZongFormationCharge::TrueQi,
            ],
        )
        .expect("complete charge set should activate core");

        assert_eq!(core.activated_until, ZONG_FORMATION_DURATION_TICKS);
        assert_eq!(event.zone_id, "jiuzong_bloodstream_ruin");
        assert_eq!(event.origin_id, 1);
        assert_eq!(event.active_qi, 0.60);
        assert_eq!(event.anomaly_kind, 5);
    }

    #[test]
    fn active_core_stabilizes_qi_only_inside_sixty_blocks() {
        let mut core = ZongFormationCore::new(ZongmenOrigin::Beiling, [-1000.0, -8500.0]);
        activate_zong_formation_core(
            &mut core,
            10,
            &[
                ZongFormationCharge::SpiritHerb,
                ZongFormationCharge::BoneCoin,
                ZongFormationCharge::TrueQi,
            ],
        )
        .expect("activation should succeed");

        assert_eq!(zong_formation_qi_at_distance(&core, 11, 59.0), 0.60);
        assert_eq!(zong_formation_qi_at_distance(&core, 11, 61.0), 0.40);
        assert_eq!(
            zong_formation_qi_at_distance(&core, core.activated_until, 10.0),
            0.40
        );
    }

    #[test]
    fn tide_turn_season_doubles_self_activation_rate() {
        assert_eq!(
            zong_core_self_activation_rate(0.02, PseudoVeinSeasonV1::Summer),
            0.02
        );
        assert_eq!(
            zong_core_self_activation_rate(0.02, PseudoVeinSeasonV1::WinterToSummer),
            0.04
        );
    }

    #[test]
    fn active_core_spawn_pressure_matches_plan() {
        let pressure = zong_formation_spawn_pressure(true);

        assert_eq!(pressure.mutant_beast_min, 1);
        assert_eq!(pressure.mutant_beast_max, 2);
        assert_eq!(pressure.daoxiang_probability, 0.30);
        assert_eq!(
            zong_formation_spawn_pressure(false).daoxiang_probability,
            0.0
        );
    }
}
