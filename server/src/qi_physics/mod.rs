//! 真元/灵气物理底盘。
//!
//! 本模块只提供 server 内部物理算子、账本与守恒断言；既有系统迁移由
//! plan-qi-physics-patch-v1 承接。服务器启动时的全服总量解析、优先级和
//! `WorldQiBudget` 注入也归这里定义；玩法模块只读取注入后的资源，不自行读取环境变量。

#![allow(unused_imports)]

use std::fmt;

use constants::DEFAULT_SPIRIT_QI_TOTAL;

/// `BONG_SPIRIT_QI_TOTAL` 是服务器启动时读取的全服真元预算环境变量。
pub const SPIRIT_QI_TOTAL_ENV: &str = "BONG_SPIRIT_QI_TOTAL";
/// `--spirit-qi-total` 是服务器启动时注入全服真元预算的命令行参数。
pub const SPIRIT_QI_TOTAL_ARG: &str = "--spirit-qi-total";

/// 记录全服真元预算最终来自哪里，供启动日志和诊断使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldQiTotalSource {
    /// 命令行显式提供了 `--spirit-qi-total`。
    CommandLine,
    /// 环境变量 `BONG_SPIRIT_QI_TOTAL` 提供了总量。
    Environment,
    /// 未提供覆盖值，采用 `DEFAULT_SPIRIT_QI_TOTAL`。
    Default,
}

impl fmt::Display for WorldQiTotalSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::CommandLine => "命令行",
            Self::Environment => "环境变量",
            Self::Default => "默认值",
        };
        formatter.write_str(label)
    }
}

/// 起服阶段解析后的全服真元预算。
///
/// 该值在构建 Bevy `App` 之前完成校验，保证 NaN、无穷、非正数和无法解析的输入
/// 不会静默回退成另一个预算。`WorldQiBudget` 只保存已验证的数值，业务系统通过
/// 资源读取本次运行的实际总量。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldQiTotalConfig {
    /// 已通过有限性和正数校验的全服真元总量。
    pub total: f64,
    /// 记录总量的生效来源，供启动日志和诊断使用。
    pub source: WorldQiTotalSource,
}

/// 全服真元预算启动配置的解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldQiTotalConfigError {
    /// 参数名后没有可解析的数值。
    MissingCommandLineValue,
    /// 同一进程参数列表重复指定总量。
    DuplicateCommandLineValue,
    /// 命令行或环境变量提供了非有限、非正或无法解析的值。
    InvalidValue {
        source: WorldQiTotalSource,
        raw: String,
    },
}

impl fmt::Display for WorldQiTotalConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommandLineValue => {
                formatter.write_str("--spirit-qi-total 需要一个数值参数")
            }
            Self::DuplicateCommandLineValue => {
                formatter.write_str("--spirit-qi-total 只能指定一次")
            }
            Self::InvalidValue { source, raw } => write!(
                formatter,
                "{source}提供的全服真元总量 {raw:?} 非法；必须是有限且大于 0 的数字"
            ),
        }
    }
}

impl std::error::Error for WorldQiTotalConfigError {}

impl WorldQiTotalConfig {
    /// 按命令行优先、环境变量其次、默认值最后的顺序解析预算。
    pub fn resolve(
        command_line_value: Option<&str>,
        environment_value: Option<&str>,
    ) -> Result<Self, WorldQiTotalConfigError> {
        if let Some(raw) = command_line_value {
            return Ok(Self {
                total: parse_positive_finite_total(raw, WorldQiTotalSource::CommandLine)?,
                source: WorldQiTotalSource::CommandLine,
            });
        }
        if let Some(raw) = environment_value {
            return Ok(Self {
                total: parse_positive_finite_total(raw, WorldQiTotalSource::Environment)?,
                source: WorldQiTotalSource::Environment,
            });
        }
        Ok(Self {
            total: DEFAULT_SPIRIT_QI_TOTAL,
            source: WorldQiTotalSource::Default,
        })
    }

    /// 从传给服务器的参数列表中提取 `--spirit-qi-total`，其余参数交给既有 CLI。
    pub fn from_args<I, S>(arguments: I) -> Result<Self, WorldQiTotalConfigError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut command_line_value = None;
        let mut iter = arguments.into_iter();
        while let Some(argument) = iter.next() {
            let argument = argument.as_ref();
            let value = if argument == SPIRIT_QI_TOTAL_ARG {
                let Some(value) = iter.next() else {
                    return Err(WorldQiTotalConfigError::MissingCommandLineValue);
                };
                let value = value.as_ref();
                if value.starts_with("--") {
                    return Err(WorldQiTotalConfigError::MissingCommandLineValue);
                }
                Some(value.to_owned())
            } else {
                argument
                    .strip_prefix("--spirit-qi-total=")
                    .map(str::to_owned)
            };

            if let Some(value) = value {
                if command_line_value.replace(value).is_some() {
                    return Err(WorldQiTotalConfigError::DuplicateCommandLineValue);
                }
            }
        }

        let environment_value = std::env::var(SPIRIT_QI_TOTAL_ENV).ok();
        Self::resolve(command_line_value.as_deref(), environment_value.as_deref())
    }
}

fn parse_positive_finite_total(
    raw: &str,
    source: WorldQiTotalSource,
) -> Result<f64, WorldQiTotalConfigError> {
    let value = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| WorldQiTotalConfigError::InvalidValue {
            source,
            raw: raw.to_owned(),
        })?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(WorldQiTotalConfigError::InvalidValue {
            source,
            raw: raw.to_owned(),
        })
    }
}

pub mod attrition;
pub mod channeling;
pub mod collision;
pub mod constants;
pub mod container;
pub mod cost;
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
    apply_attrition, apply_attrition_checked, apply_attrition_checked_with_ledger,
    dead_tsy_family_id, env_multiplier, is_attrition_exempt, release_attrition_to_zone,
    AttritionApplyOutcome, AttritionConfig, AttritionSkipReason,
};
pub use channeling::{qi_channeling, qi_channeling_transfer, ChannelDirection, ChannelingOutcome};
pub use collision::{
    flow_modifier, qi_collision, qi_negative_field_drain_ratio,
    qi_woliu_vortex_field_strength_for_realm, reverse_clamp, CollisionOutcome, QI_ZHENMAI_BETA,
};
pub use container::{abrasion_loss, AbrasionDirection, AbrasionOutcome, AnqiContainerKind};
pub use cost::proportional_qi_cost;
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
    contam_purge, mass_meridian_repair, meridian_repair, yidao_cast_ticks, ContamPurgeOutcome,
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
    transfer_ledger_qi_to_external, transfer_ledger_qi_to_zone, transfer_zone_qi_to_ledger,
    AttritionOpKind, QiAccountId, QiAccountKind, QiPhysicsIpcSnapshot, QiTransfer,
    QiTransferReason, WorldQiAccount, WorldQiBudget, WorldQiSnapshot,
    DYING_ELDER_DAN_EXCESS_ACCOUNT_ID, DYING_ELDER_RELEASE_OVERFLOW_ACCOUNT_ID,
    PENDING_INFLOW_ACCOUNT_ID, PERSISTENT_RUNTIME_QI_ACCOUNT_IDS, QI_FLOW_OVERFLOW_ACCOUNT_ID,
    QI_LEDGER_ACCOUNT_FIELD_PREFIX, RIFT_DRAIN_ACCOUNT_ID,
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

/// 判断有限非负真元值的正向扣减是否会让 IEEE-754 `f64` 产生可观察进展。
///
/// 该 helper 只负责数值可表示性，不替调用方判断 `amount` 是否不超过余额；调用方仍须
/// 先执行自己的业务边界校验。把判据放在 `qi_physics`，避免各 gameplay 路径各自拍 epsilon。
pub(crate) fn subtraction_makes_progress(before: f64, amount: f64) -> Result<bool, QiPhysicsError> {
    let before = finite_non_negative(before, "subtraction.before")?;
    let amount = finite_non_negative(amount, "subtraction.amount")?;
    let after = before - amount;
    if !after.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "subtraction.after",
            value: after,
        });
    }
    Ok(amount > 0.0 && after != before)
}

pub fn register(app: &mut App) {
    let config =
        WorldQiTotalConfig::resolve(None, std::env::var(SPIRIT_QI_TOTAL_ENV).ok().as_deref())
            .unwrap_or_else(|error| panic!("[bong][qi_physics] invalid spirit qi total: {error}"));
    register_with_total(app, config);
}

/// 将已在进程入口校验的总量注入 qi physics 资源，并记录最终来源。
pub fn register_with_total(app: &mut App, config: WorldQiTotalConfig) {
    tracing::info!(
        "[bong][qi_physics] world spirit qi total initialized: {} (source={})",
        config.total,
        config.source
    );
    app.insert_resource(WorldQiBudget::from_total(config.total))
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
            "budget must reset to the (now 2_000_000.0) default total on boot unless \
             BONG_SPIRIT_QI_TOTAL overrides it — no carry-over from a prior run"
        );
        assert_eq!(budget.era_decay_accum, 0.0);
    }

    #[test]
    fn world_qi_total_cli_has_priority_over_environment() {
        let config = WorldQiTotalConfig::resolve(Some("1234.5"), Some("6789.0"))
            .expect("valid command-line total should resolve");
        assert_eq!(config.total, 1234.5);
        assert_eq!(config.source, WorldQiTotalSource::CommandLine);
    }

    #[test]
    fn world_qi_total_environment_is_used_when_cli_is_absent() {
        let config = WorldQiTotalConfig::resolve(None, Some("6789.0"))
            .expect("valid environment total should resolve");
        assert_eq!(config.total, 6789.0);
        assert_eq!(config.source, WorldQiTotalSource::Environment);
    }

    #[test]
    fn world_qi_total_default_is_two_million() {
        let config = WorldQiTotalConfig::resolve(None, None).expect("default should resolve");
        assert_eq!(config.total, DEFAULT_SPIRIT_QI_TOTAL);
        assert_eq!(config.source, WorldQiTotalSource::Default);
    }

    #[test]
    fn world_qi_total_rejects_non_positive_non_finite_and_malformed_values() {
        for raw in ["0", "-1", "NaN", "inf", "-inf", "not-a-number"] {
            let error = WorldQiTotalConfig::resolve(Some(raw), None)
                .expect_err("invalid command-line total must fail closed");
            assert!(matches!(
                error,
                WorldQiTotalConfigError::InvalidValue {
                    source: WorldQiTotalSource::CommandLine,
                    ..
                }
            ));
        }
        let error = WorldQiTotalConfig::resolve(None, Some("not-a-number"))
            .expect_err("invalid environment total must fail closed");
        assert!(matches!(
            error,
            WorldQiTotalConfigError::InvalidValue {
                source: WorldQiTotalSource::Environment,
                ..
            }
        ));
    }

    #[test]
    fn world_qi_total_argument_parser_accepts_separate_and_equals_forms() {
        let separate = WorldQiTotalConfig::from_args(["--spirit-qi-total", "42.0"])
            .expect("separate argument form should resolve");
        assert_eq!(separate.total, 42.0);
        let equals = WorldQiTotalConfig::from_args(["--spirit-qi-total=43.0"])
            .expect("equals argument form should resolve");
        assert_eq!(equals.total, 43.0);
    }

    #[test]
    fn world_qi_total_argument_parser_leaves_unrelated_arguments_for_existing_cli() {
        let config = WorldQiTotalConfig::from_args([
            "--lifecycle-mode",
            "managed",
            "--spirit-qi-total",
            "44.0",
            "--startup-marker=ci",
        ])
        .expect("unrelated lifecycle arguments must not block startup configuration");
        assert_eq!(config.total, 44.0);
        assert_eq!(config.source, WorldQiTotalSource::CommandLine);
    }

    #[test]
    fn subtraction_progress_uses_f64_result_without_a_gameplay_epsilon() {
        assert!(!subtraction_makes_progress(1.0, 1e-17).unwrap());
        assert!(subtraction_makes_progress(1.0, f64::EPSILON).unwrap());
        assert!(!subtraction_makes_progress(1.0, 0.0).unwrap());
        assert!(matches!(
            subtraction_makes_progress(f64::NAN, 1.0),
            Err(QiPhysicsError::InvalidAmount {
                field: "subtraction.before",
                ..
            })
        ));
    }
}
