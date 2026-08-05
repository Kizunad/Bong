//! 真元/灵气物理底盘。
//!
//! 本模块只提供 server 内部物理算子、账本与守恒断言；既有系统迁移由
//! plan-qi-physics-patch-v1 承接。

#![allow(unused_imports)]

pub mod attrition;
pub mod channeling;
pub mod collision;
pub mod constants;
pub mod container;
pub mod distance;
pub mod env;
pub mod excretion;
pub mod field;
pub mod healing;
pub mod knockback;
pub mod ledger;
pub mod prepare;
pub mod projectile;
pub mod release;
pub mod tiandao;
pub mod traits;
pub mod wear;
pub mod zone_inflow;

use valence::prelude::App;

pub use attrition::{
    apply_attrition, apply_attrition_checked, dead_tsy_family_id, env_multiplier,
    is_attrition_exempt, release_attrition_to_zone, AttritionApplyOutcome, AttritionConfig,
    AttritionSkipReason,
};
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
    aoe_ground_wave, blood_burn_conversion, body_transcendence, density_amplifier, density_echo,
    inverse_diffusion, multi_point_dispersion, reverse_burst_all_marks, sever_meridian,
    shed_to_carrier, tiandao_signal_distort, AoeGroundWaveOutcome, BloodBurnConversionOutcome,
    BodyTranscendenceOutcome, DensityAmplifierOutcome, DuguReverseBurstOutcome, EchoFractalOutcome,
    InverseDiffusionOutcome, ShedToCarrierOutcome, TiandaoSignalDistortionOutcome,
};
pub use healing::{
    contam_purge, emergency_stabilize, life_extend, mass_meridian_repair, meridian_repair,
    yidao_cast_ticks, ContamPurgeOutcome, EmergencyStabilizeOutcome, LifeExtendOutcome,
    MassMeridianRepairOutcome, MeridianRepairOutcome,
};
pub use knockback::{
    compute_knockback, entity_collision, wall_collision, EntityCollisionInput,
    EntityCollisionResult, KnockbackInput, KnockbackResult, WallCollisionInput,
    WallCollisionResult, MAX_BLOCK_PENETRATION, MAX_KNOCKBACK_DISTANCE,
};
pub use ledger::{
    assert_conservation, build_qi_ledger_hash_fields, credit_pending_inflow,
    dying_elder_dan_excess_account, dying_elder_release_overflow_account, pending_inflow_account,
    persistent_runtime_qi_accounts, qi_flow_overflow_account, reject_audit_only_qi_reason,
    rift_drain_account, snapshot_for_ipc, summarize_world_qi, transfer_external_qi_to_ledger,
    transfer_ledger_qi_to_zone, transfer_zone_qi_to_ledger, AttritionOpKind, QiAccountId,
    QiAccountKind, QiPhysicsIpcSnapshot, QiTransfer, QiTransferReason, WorldQiAccount,
    WorldQiBudget, WorldQiSnapshot, DYING_ELDER_DAN_EXCESS_ACCOUNT_ID,
    DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID, PENDING_INFLOW_ACCOUNT_ID,
    PERSISTENT_RUNTIME_QI_ACCOUNT_IDS, QI_FLOW_OVERFLOW_ACCOUNT_ID, QI_LEDGER_ACCOUNT_FIELD_PREFIX,
    RIFT_DRAIN_ACCOUNT_ID,
};
pub use prepare::{prepare_transfer, TransferPlan};
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
pub use zone_inflow::zone_equilibrium_inflow;

#[derive(Debug, Clone, PartialEq)]
pub enum QiPhysicsError {
    InvalidAmount {
        field: &'static str,
        value: f64,
    },
    UnrepresentableChange {
        field: &'static str,
        before: f64,
        amount: f64,
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
    /// plan-halfstep-buff-v1 P1：标记为 audit-only 的 QiTransferReason 被误传给会变动余额的
    /// `WorldQiAccount::transfer`，应改为单纯 emit `QiTransfer` 事件而不调 transfer 方法。
    AuditOnlyReason {
        reason: &'static str,
    },
    /// 外部物理权威转入 ledger 时 source 与 sink 相同；这会让临时 source 恢复步骤
    /// 抹掉看似成功的 sink credit，因此必须在任何余额变更前拒绝。
    SameAccountTransfer {
        account: String,
    },
}

impl std::fmt::Display for QiPhysicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAmount { field, value } => {
                write!(f, "invalid qi amount `{field}`: {value}")
            }
            Self::UnrepresentableChange {
                field,
                before,
                amount,
            } => write!(
                f,
                "qi change cannot make representable progress for {field}: before={before}, amount={amount}"
            ),
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
            Self::AuditOnlyReason { reason } => write!(
                f,
                "QiTransferReason::{reason} is audit-only and must not mutate physical qi owners"
            ),
            Self::SameAccountTransfer { account } => {
                write!(
                    f,
                    "qi transfer source and destination are identical: {account}"
                )
            }
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

#[cfg(test)]
mod tests {
    use valence::prelude::App;

    use super::*;
    use crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL;

    /// `qi_physics::register` 本身只创建空运行期账本，不携带上一进程的审计轨迹或镜像。
    /// 生产 Startup 随后由 persistence 从各自物理权威恢复：zone 账户来自 zones_runtime；
    /// 没有 ECS/zone 字段承载的三项稳定 runtime 池由 qi_runtime_accounts 白名单恢复。
    /// 本测试刻意只调用 register，锁住“资源初始化不暗中注水”的边界。
    #[test]
    fn register_starts_empty_before_persistence_hydration() {
        let mut app = App::new();
        register(&mut app);

        let account = app.world().resource::<WorldQiAccount>();
        assert_eq!(
            account.total(),
            0.0,
            "a freshly-registered WorldQiAccount must start with zero balance across all \
             accounts (no historical inflated `zone:<name>` residue survives a restart)"
        );
        assert!(
            !account.has_account(&pending_inflow_account()),
            "the pending inflow pool must not pre-exist on boot — it is created lazily on \
             first credit"
        );
        assert!(
            account.transfers().is_empty(),
            "a freshly-registered ledger must carry no audit history from a previous boot"
        );

        let budget = app.world().resource::<WorldQiBudget>();
        assert_eq!(
            budget.current_total, DEFAULT_SPIRIT_QI_TOTAL,
            "budget must reset to the (now 20000.0) default total on boot unless \
             BONG_SPIRIT_QI_TOTAL overrides it — no carry-over from a prior run"
        );
        assert_eq!(budget.era_decay_accum, 0.0);
    }
}
