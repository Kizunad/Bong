//! 真元/灵气物理底盘。
//!
//! 本模块只提供 server 内部物理算子、账本与守恒断言；既有系统迁移由
//! plan-qi-physics-patch-v1 承接。

#![allow(unused_imports)]

pub mod channeling;
pub mod collision;
pub mod constants;
pub mod container;
pub mod distance;
pub mod env;
pub mod excretion;
pub mod field;
pub mod healing;
pub mod ledger;
pub mod projectile;
pub mod release;
pub mod tiandao;
pub mod traits;
pub mod wear;

use valence::prelude::App;

pub use channeling::{qi_channeling, qi_channeling_transfer, ChannelDirection, ChannelingOutcome};
pub use collision::{
    flow_modifier, qi_collision, qi_negative_field_drain_ratio,
    qi_woliu_vortex_field_strength_for_realm, reverse_clamp, CollisionOutcome, QI_ZHENMAI_BETA,
};
pub use container::{abrasion_loss, AbrasionDirection, AbrasionOutcome, AnqiContainerKind};
pub use distance::qi_distance_atten;
pub use env::{CarrierGrade, ContainerKind, EnvField, MediumKind};
pub use excretion::{qi_excretion, qi_excretion_loss, regen_from_zone};
pub use field::{
    aoe_ground_wave, blood_burn_conversion, body_transcendence, density_echo,
    multi_point_dispersion, reverse_burst_all_marks, sever_meridian, AoeGroundWaveOutcome,
    BloodBurnConversionOutcome, BodyTranscendenceOutcome, DuguReverseBurstOutcome,
    EchoFractalOutcome,
};
pub use healing::{
    contam_purge, emergency_stabilize, life_extend, mass_meridian_repair, meridian_repair,
    yidao_cast_ticks, ContamPurgeOutcome, EmergencyStabilizeOutcome, LifeExtendOutcome,
    MassMeridianRepairOutcome, MeridianRepairOutcome,
};
pub use ledger::{
    assert_conservation, snapshot_for_ipc, summarize_world_qi, QiAccountId, QiAccountKind,
    QiPhysicsIpcSnapshot, QiTransfer, QiTransferReason, WorldQiAccount, WorldQiBudget,
    WorldQiSnapshot,
};
pub use projectile::{
    armor_penetrate, cone_dispersion, high_density_inject, ArmorPenetrationOutcome,
    ConeDispersionShot, HighDensityInjectionOutcome,
};
pub use release::{accumulate_zone_release, qi_release_to_zone, ZoneReleaseOutcome};
pub use tiandao::{
    collapse_redistribute_qi, era_decay_step, era_decay_tick, tribulation_trigger, EraDecayClock,
    TribulationCause,
};
pub use traits::{Container, SimpleStyleAttack, SimpleStyleDefense, StyleAttack, StyleDefense};
pub use wear::qi_targeted_item_wear_fraction;

#[derive(Debug, Clone, PartialEq)]
pub enum QiPhysicsError {
    InvalidAmount {
        field: &'static str,
        value: f64,
    },
    InsufficientQi {
        account: String,
        available: f64,
        requested: f64,
    },
    ConservationDrift {
        expected: f64,
        actual: f64,
        tolerance: f64,
    },
}

impl std::fmt::Display for QiPhysicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAmount { field, value } => {
                write!(f, "invalid qi amount `{field}`: {value}")
            }
            Self::InsufficientQi {
                account,
                available,
                requested,
            } => write!(
                f,
                "insufficient qi in {account}: available {available}, requested {requested}"
            ),
            Self::ConservationDrift {
                expected,
                actual,
                tolerance,
            } => write!(
                f,
                "qi conservation drift: expected {expected}, actual {actual}, tolerance {tolerance}"
            ),
        }
    }
}

impl std::error::Error for QiPhysicsError {}

pub(crate) fn finite_non_negative(value: f64, field: &'static str) -> Result<f64, QiPhysicsError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(QiPhysicsError::InvalidAmount { field, value })
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][qi_physics] registering qi physics resources");
    app.insert_resource(WorldQiBudget::from_env())
        .init_resource::<EraDecayClock>()
        .init_resource::<WorldQiAccount>()
        .add_event::<QiTransfer>()
        .add_systems(valence::prelude::Update, era_decay_tick);
}
