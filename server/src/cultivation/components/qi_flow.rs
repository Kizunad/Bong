//! R5 真元账本强制化：活体、区域与稳定账本池之间的类型化事务入口。
//!
//! `Cultivation.qi_current` 与 `Zone.spirit_qi` 仍分别由 ECS / `ZoneRegistry` 承载，
//! 不在 `WorldQiAccount` 中长期镜像，避免 `summarize_world_qi` 双计。这里把物理字段提交、
//! 真实 overflow 入账与 `QiTransfer` 审计收进同一次调用；调用方不再自行拼接“先改字段、
//! 后补 audit”的半事务。

use std::fmt;

use super::Cultivation;
use crate::cultivation::breakthrough::BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO;
use crate::cultivation::life_record::LifeRecord;
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::ledger::{
    qi_flow_overflow_account, reject_audit_only_qi_reason, transfer_external_qi_to_ledger,
    QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::qi_physics::release::qi_release_to_zone;
use crate::qi_physics::{finite_non_negative, QiPhysicsError};
use crate::world::zone::Zone;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CultivationQiInit {
    pub current: f64,
    pub max: f64,
    pub frozen: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CultivationQiSnapshot {
    pub current: f64,
    pub max: f64,
    pub frozen: Option<f64>,
    pub effective_max: f64,
    pub room: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QiFlowOutcome {
    pub requested: f64,
    pub source_debited: f64,
    pub target_credited: f64,
    pub zone_accepted: f64,
    pub overflow_credited: f64,
    pub untransferred: f64,
    pub transfers: Vec<QiTransfer>,
}

impl QiFlowOutcome {
    fn noop(requested: f64) -> Self {
        Self {
            requested,
            source_debited: 0.0,
            target_credited: 0.0,
            zone_accepted: 0.0,
            overflow_credited: 0.0,
            untransferred: requested,
            transfers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorQiKind {
    Player,
    Npc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorQiIdentity {
    account: QiAccountId,
}

impl ActorQiIdentity {
    pub fn from_life_record(
        life_record: &LifeRecord,
        kind: ActorQiKind,
    ) -> Result<Self, QiFlowError> {
        let character_id = life_record.character_id.as_str();
        let canonical_id = character_id.trim();
        if canonical_id.is_empty()
            || canonical_id == "unassigned:life_record"
            || canonical_id != character_id
        {
            return Err(QiFlowError::InvalidActorIdentity);
        }
        let account = match kind {
            ActorQiKind::Player => QiAccountId::player(character_id),
            ActorQiKind::Npc => QiAccountId::npc(character_id),
        };
        Ok(Self { account })
    }

    pub(crate) fn account(&self) -> QiAccountId {
        self.account.clone()
    }

    pub(crate) fn transfer_from_external(
        &self,
        source: QiAccountId,
        cultivation: &mut Cultivation,
        ledger: &mut WorldQiAccount,
        requested: f64,
        reason: QiTransferReason,
    ) -> Result<QiFlowOutcome, QiFlowError> {
        cultivation.validate_qi_state()?;
        let requested = finite_non_negative(requested, "transfer_from_external.requested")?;
        reject_audit_only_qi_reason(reason)?;
        if requested == 0.0 {
            return Ok(QiFlowOutcome::noop(0.0));
        }
        let target = self.account();
        if source == target {
            return Err(QiFlowError::SameAccount {
                account: source.to_string(),
            });
        }
        let room = cultivation.qi_room();
        if requested > room {
            return Err(QiFlowError::InsufficientCapacity { room, requested });
        }
        let target_after = checked_add_progress(
            cultivation.qi_current,
            requested,
            "cultivation.qi_current.external_target",
        )?;
        let transfer = QiTransfer::new(source, target, requested, reason)?;

        cultivation.qi_current = target_after;
        ledger.push_transfer_audit(transfer.clone());

        Ok(QiFlowOutcome {
            requested,
            source_debited: requested,
            target_credited: requested,
            zone_accepted: 0.0,
            overflow_credited: 0.0,
            untransferred: 0.0,
            transfers: vec![transfer],
        })
    }

    pub(crate) fn matches_life_record(&self, life_record: &LifeRecord, kind: ActorQiKind) -> bool {
        Self::from_life_record(life_record, kind)
            .is_ok_and(|identity| identity.account == self.account)
    }

    #[cfg(test)]
    fn for_test(account: QiAccountId) -> Self {
        Self { account }
    }
}

#[derive(Debug)]
pub struct ActorQiTarget<'a> {
    cultivation: &'a mut Cultivation,
    identity: ActorQiIdentity,
}

impl<'a> ActorQiTarget<'a> {
    pub fn new(cultivation: &'a mut Cultivation, identity: ActorQiIdentity) -> Self {
        Self {
            cultivation,
            identity,
        }
    }

    fn into_parts(self) -> (&'a mut Cultivation, QiAccountId) {
        (self.cultivation, self.identity.account)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistentQiSink {
    PendingInflow,
    QiFlowOverflow,
    DyingElderDanExcess,
    DyingElderReleaseOverflow,
    RiftDrain,
}

impl PersistentQiSink {
    pub fn account(self) -> QiAccountId {
        match self {
            Self::PendingInflow => crate::qi_physics::ledger::pending_inflow_account(),
            Self::QiFlowOverflow => qi_flow_overflow_account(),
            Self::DyingElderDanExcess => {
                crate::qi_physics::ledger::dying_elder_dan_excess_account()
            }
            Self::DyingElderReleaseOverflow => {
                crate::qi_physics::ledger::dying_elder_release_overflow_account()
            }
            Self::RiftDrain => crate::qi_physics::ledger::rift_drain_account(),
        }
    }
}

#[derive(Debug)]
pub enum QiFlowTarget<'a> {
    Actor(ActorQiTarget<'a>),
    Persistent(PersistentQiSink),
}

#[derive(Debug, Clone, PartialEq)]
pub struct QiResizeOutcome {
    pub old_max: f64,
    pub new_max: f64,
    pub excess: f64,
    pub release: Option<QiFlowOutcome>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QiFlowError {
    Physics(QiPhysicsError),
    InvalidActorIdentity,
    InvalidCultivationState {
        current: f64,
        max: f64,
        frozen: Option<f64>,
    },
    InvalidInitSnapshot {
        current: f64,
        max: f64,
        frozen: Option<f64>,
    },
    UnrepresentableFlow {
        field: &'static str,
        before: f64,
        amount: f64,
    },
    InsufficientCurrent {
        available: f64,
        requested: f64,
    },
    InsufficientCapacity {
        room: f64,
        requested: f64,
    },
    SameAccount {
        account: String,
    },
}

impl fmt::Display for QiFlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Physics(error) => error.fmt(f),
            Self::InvalidActorIdentity => write!(f, "actor qi endpoint requires a non-empty character id"),
            Self::InvalidCultivationState {
                current,
                max,
                frozen,
            } => write!(
                f,
                "invalid cultivation qi state: current={current}, max={max}, frozen={frozen:?}"
            ),
            Self::InvalidInitSnapshot {
                current,
                max,
                frozen,
            } => write!(
                f,
                "invalid cultivation qi init snapshot: current={current}, max={max}, frozen={frozen:?}"
            ),
            Self::UnrepresentableFlow {
                field,
                before,
                amount,
            } => write!(
                f,
                "qi flow cannot make representable progress for {field}: before={before}, amount={amount}"
            ),
            Self::InsufficientCurrent {
                available,
                requested,
            } => write!(
                f,
                "insufficient cultivation qi: available {available}, requested {requested}"
            ),
            Self::InsufficientCapacity { room, requested } => write!(
                f,
                "insufficient cultivation qi capacity: room {room}, requested {requested}"
            ),
            Self::SameAccount { account } => {
                write!(f, "qi transfer source and target are identical: {account}")
            }
        }
    }
}

impl std::error::Error for QiFlowError {}

impl From<QiPhysicsError> for QiFlowError {
    fn from(value: QiPhysicsError) -> Self {
        Self::Physics(value)
    }
}

impl Cultivation {
    pub fn qi_current(&self) -> f64 {
        self.qi_current
    }

    pub fn qi_max(&self) -> f64 {
        self.qi_max
    }

    pub fn qi_max_frozen(&self) -> Option<f64> {
        self.qi_max_frozen
    }

    pub fn effective_qi_max(&self) -> f64 {
        (self.qi_max - self.qi_max_frozen.unwrap_or(0.0)).max(0.0)
    }

    pub fn qi_room(&self) -> f64 {
        (self.effective_qi_max() - self.qi_current).max(0.0)
    }

    pub fn qi_snapshot(&self) -> CultivationQiSnapshot {
        CultivationQiSnapshot {
            current: self.qi_current,
            max: self.qi_max,
            frozen: self.qi_max_frozen,
            effective_max: self.effective_qi_max(),
            room: self.qi_room(),
        }
    }

    /// 仅供构造、验证后的持久化恢复、一次性迁移与测试 fixture 使用。
    /// gameplay 与 dev command 不得借此绕过账本事务。
    pub(crate) fn set_for_init(&mut self, init: CultivationQiInit) -> Result<(), QiFlowError> {
        if !valid_snapshot(init.current, init.max, init.frozen) {
            return Err(QiFlowError::InvalidInitSnapshot {
                current: init.current,
                max: init.max,
                frozen: init.frozen,
            });
        }

        self.qi_current = init.current;
        self.qi_max = init.max;
        self.qi_max_frozen = init.frozen;
        Ok(())
    }

    /// 仅供 `/qi` 等 dev-only admin override 使用。它显式不产生 `QiTransfer`，
    /// 但仍以同一 snapshot 校验拒绝 NaN、负值、越界 current 和非法 frozen。
    pub(crate) fn set_for_dev_only(&mut self, init: CultivationQiInit) -> Result<(), QiFlowError> {
        self.set_for_init(init)
    }

    /// 从 signed zone 吸收 raw qi。负灵域和死域不产出；目标装不下的部分留在 zone。
    pub(crate) fn gain_from_zone(
        &mut self,
        zone: &mut Zone,
        ledger: &mut WorldQiAccount,
        actor: &ActorQiIdentity,
        requested: f64,
        reason: QiTransferReason,
    ) -> Result<QiFlowOutcome, QiFlowError> {
        self.validate_qi_state()?;
        let requested = finite_non_negative(requested, "gain_from_zone.requested")?;
        reject_audit_only_qi_reason(reason)?;
        if !zone.spirit_qi.is_finite() {
            return Err(QiPhysicsError::InvalidAmount {
                field: "zone.spirit_qi",
                value: zone.spirit_qi,
            }
            .into());
        }

        let available = finite_non_negative(
            zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY,
            "gain_from_zone.available",
        )?;
        let credited = requested.min(self.qi_room()).min(available);
        if credited == 0.0 {
            return Ok(QiFlowOutcome::noop(requested));
        }
        let actor_after = checked_add_to_cap(
            self.qi_current,
            credited,
            self.effective_qi_max(),
            "cultivation.qi_current",
        )?;
        let zone_after = if credited == available {
            0.0
        } else {
            checked_sub_progress(
                zone.spirit_qi,
                credited / QI_ZONE_UNIT_CAPACITY,
                "zone.spirit_qi",
            )?
        };

        let actor_account = actor.account();
        let transfer = QiTransfer::new(
            QiAccountId::zone(zone.name.clone()),
            actor_account,
            credited,
            reason,
        )?;
        reject_same_account(&transfer)?;

        self.qi_current = actor_after;
        zone.spirit_qi = zone_after;
        ledger.push_transfer_audit(transfer.clone());

        Ok(QiFlowOutcome {
            requested,
            source_debited: credited,
            target_credited: credited,
            zone_accepted: 0.0,
            overflow_credited: 0.0,
            untransferred: requested - credited,
            transfers: vec![transfer],
        })
    }

    /// 从活体释放 raw qi。zone 可缺失；zone 装不下或无法定位时，余量进入持久化
    /// `qi_flow_overflow` 账户，绝不以 emit-only event 冒充真实落账。
    pub(crate) fn release_to_zone(
        &mut self,
        zone: Option<&mut Zone>,
        ledger: &mut WorldQiAccount,
        actor: &ActorQiIdentity,
        requested: f64,
        reason: QiTransferReason,
    ) -> Result<QiFlowOutcome, QiFlowError> {
        self.validate_qi_state()?;
        release_external_qi_to_zone(&mut self.qi_current, actor, zone, ledger, requested, reason)
    }

    /// 从活体转入另一活体或稳定 ledger owner。活体目标容量不足时余量留在 source；
    /// 稳定账户接收全部请求量。外部 source 扣减只在目标真实入账 / audit 可确定成功后提交。
    pub(crate) fn transfer_to(
        &mut self,
        target: QiFlowTarget<'_>,
        ledger: &mut WorldQiAccount,
        source: &ActorQiIdentity,
        requested: f64,
        reason: QiTransferReason,
    ) -> Result<QiFlowOutcome, QiFlowError> {
        self.validate_qi_state()?;
        let requested = finite_non_negative(requested, "transfer_to.requested")?;
        reject_audit_only_qi_reason(reason)?;
        if requested > self.qi_current {
            return Err(QiFlowError::InsufficientCurrent {
                available: self.qi_current,
                requested,
            });
        }
        if requested == 0.0 {
            return Ok(QiFlowOutcome::noop(0.0));
        }

        let source_account = source.account();
        match target {
            QiFlowTarget::Actor(target) => {
                let (cultivation, target_account) = target.into_parts();
                cultivation.validate_qi_state()?;
                if source_account == target_account {
                    return Err(QiFlowError::SameAccount {
                        account: source_account.to_string(),
                    });
                }

                let credited = requested.min(cultivation.qi_room());
                if credited == 0.0 {
                    return Ok(QiFlowOutcome::noop(requested));
                }
                let source_after = if credited == self.qi_current {
                    0.0
                } else {
                    checked_sub_progress(
                        self.qi_current,
                        credited,
                        "cultivation.qi_current.source",
                    )?
                };
                let target_before = cultivation.qi_current;
                let target_after = checked_add_to_cap(
                    target_before,
                    credited,
                    cultivation.effective_qi_max(),
                    "cultivation.qi_current.target",
                )?;
                let transfer = QiTransfer::new(source_account, target_account, credited, reason)?;

                self.qi_current = source_after;
                cultivation.qi_current = target_after;
                ledger.push_transfer_audit(transfer.clone());

                Ok(QiFlowOutcome {
                    requested,
                    source_debited: credited,
                    target_credited: credited,
                    zone_accepted: 0.0,
                    overflow_credited: 0.0,
                    untransferred: requested - credited,
                    transfers: vec![transfer],
                })
            }
            QiFlowTarget::Persistent(sink) => {
                let target_account = sink.account();
                if source_account == target_account {
                    return Err(QiFlowError::SameAccount {
                        account: source_account.to_string(),
                    });
                }
                let transfer = QiTransfer::new(source_account, target_account, requested, reason)?;

                let source_after = if requested == self.qi_current {
                    0.0
                } else {
                    checked_sub_progress(self.qi_current, requested, "cultivation.qi_current")?
                };
                transfer_external_qi_to_ledger(
                    ledger,
                    transfer.from.clone(),
                    transfer.to.clone(),
                    transfer.amount,
                    transfer.reason,
                )?;
                self.qi_current = source_after;

                Ok(QiFlowOutcome {
                    requested,
                    source_debited: requested,
                    target_credited: requested,
                    zone_accepted: 0.0,
                    overflow_credited: 0.0,
                    untransferred: 0.0,
                    transfers: vec![transfer],
                })
            }
        }
    }

    /// Transfer qi from this live actor into a canonical actor whose physical reserve is held in
    /// the ledger rather than another `Cultivation` component (for example a devour rat's swallowed
    /// reserve). Both endpoints remain typed actor capabilities; callers cannot supply an arbitrary
    /// ledger account. The source field is debited only after the ledger owner was credited.
    pub(crate) fn transfer_to_external_actor(
        &mut self,
        target: &ActorQiIdentity,
        ledger: &mut WorldQiAccount,
        source: &ActorQiIdentity,
        requested: f64,
        reason: QiTransferReason,
    ) -> Result<QiFlowOutcome, QiFlowError> {
        self.validate_qi_state()?;
        let requested = finite_non_negative(requested, "transfer_to_external_actor.requested")?;
        reject_audit_only_qi_reason(reason)?;
        if requested > self.qi_current {
            return Err(QiFlowError::InsufficientCurrent {
                available: self.qi_current,
                requested,
            });
        }
        if requested == 0.0 {
            return Ok(QiFlowOutcome::noop(0.0));
        }

        let source_account = source.account();
        let target_account = target.account();
        if source_account == target_account {
            return Err(QiFlowError::SameAccount {
                account: source_account.to_string(),
            });
        }
        let source_after = if requested == self.qi_current {
            0.0
        } else {
            checked_sub_progress(
                self.qi_current,
                requested,
                "cultivation.qi_current.external_actor_source",
            )?
        };
        let transfer = transfer_external_qi_to_ledger(
            ledger,
            source_account,
            target_account,
            requested,
            reason,
        )?
        .expect("positive validated actor transfer must produce a transfer");
        self.qi_current = source_after;

        Ok(QiFlowOutcome {
            requested,
            source_debited: requested,
            target_credited: requested,
            zone_accepted: 0.0,
            overflow_credited: 0.0,
            untransferred: 0.0,
            transfers: vec![transfer],
        })
    }

    /// 普通 gameplay 修改 raw `qi_max` 的唯一入口。缩容前先把 excess 守恒释放；
    /// release 失败时 max/current/zone/ledger 全部保持原样。
    pub(crate) fn resize_qi_max_and_release_excess(
        &mut self,
        zone: Option<&mut Zone>,
        ledger: &mut WorldQiAccount,
        actor: &ActorQiIdentity,
        new_max: f64,
        reason: QiTransferReason,
    ) -> Result<QiResizeOutcome, QiFlowError> {
        self.validate_qi_state()?;
        let new_max = finite_non_negative(new_max, "resize_qi_max.new_max")?;
        reject_audit_only_qi_reason(reason)?;
        let old_max = self.qi_max;
        let excess = (self.qi_current - new_max).max(0.0);

        let release = if excess > 0.0 {
            Some(self.release_to_zone(zone, ledger, actor, excess, reason)?)
        } else {
            None
        };

        self.qi_max = new_max;
        let frozen_cap = new_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO;
        self.qi_max_frozen = self.qi_max_frozen.map(|frozen| frozen.min(frozen_cap));

        Ok(QiResizeOutcome {
            old_max,
            new_max,
            excess,
            release,
        })
    }

    pub(crate) fn validate_qi_state(&self) -> Result<(), QiFlowError> {
        if valid_snapshot(self.qi_current, self.qi_max, self.qi_max_frozen) {
            Ok(())
        } else {
            Err(QiFlowError::InvalidCultivationState {
                current: self.qi_current,
                max: self.qi_max,
                frozen: self.qi_max_frozen,
            })
        }
    }
}

/// 从非 [`Cultivation`] 的外部 owner 释放 raw qi（例如道伥 blackboard）。
///
/// source 字段、signed zone、稳定 overflow 与审计共用活体事务的同一失败原子性边界；
/// source 必须是从 canonical [`LifeRecord`] 构造的 actor capability，禁止调用者传入任意
/// `QiAccountId` 或从 `Entity` debug 文本兜底。
pub(crate) fn release_external_qi_to_zone(
    source_current: &mut f64,
    source: &ActorQiIdentity,
    zone: Option<&mut Zone>,
    ledger: &mut WorldQiAccount,
    requested: f64,
    reason: QiTransferReason,
) -> Result<QiFlowOutcome, QiFlowError> {
    let source_account = source.account();
    let available = finite_non_negative(*source_current, "external_qi_source.current")?;
    let requested = finite_non_negative(requested, "release_to_zone.requested")?;
    reject_audit_only_qi_reason(reason)?;
    if requested > available {
        return Err(QiFlowError::InsufficientCurrent {
            available,
            requested,
        });
    }
    if requested == 0.0 {
        return Ok(QiFlowOutcome::noop(0.0));
    }

    let source_after = if requested == available {
        0.0
    } else {
        checked_sub_progress(available, requested, "external_qi_source.current")?
    };
    let (zone_after, zone_accepted, overflow, zone_transfer) = if let Some(zone) = zone.as_deref() {
        if !zone.spirit_qi.is_finite() {
            return Err(QiPhysicsError::InvalidAmount {
                field: "zone.spirit_qi",
                value: zone.spirit_qi,
            }
            .into());
        }
        let zone_current = finite_signed_scaled(
            zone.spirit_qi,
            QI_ZONE_UNIT_CAPACITY,
            "release_to_zone.zone_current",
        )?;
        let outcome = qi_release_to_zone(
            requested,
            source_account.clone(),
            QiAccountId::zone(zone.name.clone()),
            zone_current,
            QI_ZONE_UNIT_CAPACITY,
        )?;
        let normalized_zone_after = outcome.zone_after / QI_ZONE_UNIT_CAPACITY;
        if outcome.accepted > 0.0 && normalized_zone_after == zone.spirit_qi {
            return Err(QiFlowError::UnrepresentableFlow {
                field: "zone.spirit_qi",
                before: zone.spirit_qi,
                amount: outcome.accepted / QI_ZONE_UNIT_CAPACITY,
            });
        }
        let transfer = if outcome.accepted > 0.0 {
            let transfer = QiTransfer::new(
                source_account.clone(),
                QiAccountId::zone(zone.name.clone()),
                outcome.accepted,
                reason,
            )?;
            reject_same_account(&transfer)?;
            Some(transfer)
        } else {
            None
        };
        (
            Some(normalized_zone_after),
            outcome.accepted,
            outcome.overflow,
            transfer,
        )
    } else {
        (None, 0.0, requested, None)
    };

    let overflow_transfer = if overflow > 0.0 {
        let to = qi_flow_overflow_account();
        if source_account == to {
            return Err(QiFlowError::SameAccount {
                account: source_account.to_string(),
            });
        }
        Some(QiTransfer::new(
            source_account.clone(),
            to,
            overflow,
            reason,
        )?)
    } else {
        None
    };

    // 唯一可能失败的真实账本写入先提交；它失败时 external source/zone/audit 均未改变。
    if let Some(transfer) = overflow_transfer.as_ref() {
        transfer_external_qi_to_ledger(
            ledger,
            transfer.from.clone(),
            transfer.to.clone(),
            transfer.amount,
            transfer.reason,
        )?;
    }

    *source_current = source_after;
    if let (Some(zone), Some(zone_after)) = (zone, zone_after) {
        zone.spirit_qi = zone_after;
    }
    if let Some(transfer) = zone_transfer.as_ref() {
        ledger.push_transfer_audit(transfer.clone());
    }

    // split release 的 outcome 顺序以真实 ledger commit 顺序为准：overflow 先落稳定池，
    // zone audit 后追加；调用方不得假设 zone 总在第一项。
    let mut transfers = Vec::with_capacity(2);
    if let Some(transfer) = overflow_transfer {
        transfers.push(transfer);
    }
    if let Some(transfer) = zone_transfer {
        transfers.push(transfer);
    }

    Ok(QiFlowOutcome {
        requested,
        source_debited: requested,
        target_credited: requested,
        zone_accepted,
        overflow_credited: overflow,
        untransferred: 0.0,
        transfers,
    })
}

fn finite_signed_scaled(value: f64, factor: f64, field: &'static str) -> Result<f64, QiFlowError> {
    let scaled = value * factor;
    if scaled.is_finite() {
        Ok(scaled)
    } else {
        Err(QiPhysicsError::InvalidAmount {
            field,
            value: scaled,
        }
        .into())
    }
}

fn checked_add_to_cap(
    before: f64,
    amount: f64,
    cap: f64,
    field: &'static str,
) -> Result<f64, QiFlowError> {
    let after = checked_add_progress(before, amount, field)?;
    if !cap.is_finite() || after > cap {
        return Err(QiFlowError::UnrepresentableFlow {
            field,
            before,
            amount,
        });
    }
    Ok(after)
}

fn checked_add_progress(before: f64, amount: f64, field: &'static str) -> Result<f64, QiFlowError> {
    let after = before + amount;
    if !after.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field,
            value: after,
        }
        .into());
    }
    if amount > 0.0 && after == before {
        return Err(QiFlowError::UnrepresentableFlow {
            field,
            before,
            amount,
        });
    }
    Ok(after)
}

fn checked_sub_progress(before: f64, amount: f64, field: &'static str) -> Result<f64, QiFlowError> {
    let after = before - amount;
    if !after.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field,
            value: after,
        }
        .into());
    }
    if amount > 0.0 && after == before {
        return Err(QiFlowError::UnrepresentableFlow {
            field,
            before,
            amount,
        });
    }
    Ok(after)
}

fn valid_snapshot(current: f64, max: f64, frozen: Option<f64>) -> bool {
    current.is_finite()
        && current >= 0.0
        && max.is_finite()
        && max >= 0.0
        && current <= max
        && frozen.is_none_or(|value| {
            value.is_finite() && value >= 0.0 && value <= max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO
        })
}

fn reject_same_account(transfer: &QiTransfer) -> Result<(), QiFlowError> {
    if transfer.from == transfer.to {
        Err(QiFlowError::SameAccount {
            account: transfer.from.to_string(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::DVec3;

    fn cultivation(current: f64, max: f64) -> Cultivation {
        let mut cultivation = Cultivation::default();
        cultivation
            .set_for_init(CultivationQiInit {
                current,
                max,
                frozen: None,
            })
            .expect("test cultivation snapshot must be valid");
        cultivation
    }

    fn zone(name: &str, spirit_qi: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::ZERO, DVec3::splat(16.0)),
            spirit_qi,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    #[test]
    fn dev_only_setter_is_atomic_and_does_not_touch_ledger() {
        let mut actor = cultivation(4.0, 10.0);
        let mut ledger = WorldQiAccount::default();

        actor
            .set_for_dev_only(CultivationQiInit {
                current: 7.0,
                max: 12.0,
                frozen: Some(2.0),
            })
            .expect("valid dev override should commit the complete snapshot");
        assert_eq!(
            actor.qi_snapshot(),
            CultivationQiSnapshot {
                current: 7.0,
                max: 12.0,
                frozen: Some(2.0),
                effective_max: 10.0,
                room: 3.0,
            }
        );
        let before = actor.qi_snapshot();
        assert!(matches!(
            actor.set_for_dev_only(CultivationQiInit {
                current: f64::NAN,
                max: 12.0,
                frozen: None,
            }),
            Err(QiFlowError::InvalidInitSnapshot { .. })
        ));
        assert_eq!(actor.qi_snapshot(), before);
        assert_eq!(ledger.total(), 0.0);
        assert!(ledger.transfers().is_empty());
        assert!(ledger.remove_balance(&qi_flow_overflow_account()).is_none());
    }

    #[test]
    fn sub_ulp_flows_fail_before_owner_or_audit_mutation() {
        let tiny = f64::MIN_POSITIVE;
        let actor_id = ActorQiIdentity::for_test(QiAccountId::player("p1"));

        let mut gain_actor = cultivation(1.0, 2.0);
        let mut gain_zone = zone("gain", 0.5);
        let mut gain_ledger = WorldQiAccount::default();
        let gain_before = gain_actor.qi_snapshot();
        assert!(matches!(
            gain_actor.gain_from_zone(
                &mut gain_zone,
                &mut gain_ledger,
                &actor_id,
                tiny,
                QiTransferReason::CultivationRegen,
            ),
            Err(QiFlowError::UnrepresentableFlow { .. })
        ));
        assert_eq!(gain_actor.qi_snapshot(), gain_before);
        assert_eq!(gain_zone.spirit_qi, 0.5);
        assert!(gain_ledger.transfers().is_empty());

        let mut release_actor = cultivation(1.0, 2.0);
        let mut release_zone = zone("release", 0.5);
        let mut release_ledger = WorldQiAccount::default();
        let release_before = release_actor.qi_snapshot();
        assert!(matches!(
            release_actor.release_to_zone(
                Some(&mut release_zone),
                &mut release_ledger,
                &actor_id,
                tiny,
                QiTransferReason::ReleaseToZone,
            ),
            Err(QiFlowError::UnrepresentableFlow { .. })
        ));
        assert_eq!(release_actor.qi_snapshot(), release_before);
        assert_eq!(release_zone.spirit_qi, 0.5);
        assert_eq!(release_ledger.total(), 0.0);
        assert!(release_ledger.transfers().is_empty());

        let mut exact_release_actor = cultivation(tiny, tiny);
        let mut exact_release_zone = zone("exact-release", 0.5);
        let mut exact_release_ledger = WorldQiAccount::default();
        let exact_release_before = exact_release_actor.qi_snapshot();
        assert!(matches!(
            exact_release_actor.release_to_zone(
                Some(&mut exact_release_zone),
                &mut exact_release_ledger,
                &actor_id,
                tiny,
                QiTransferReason::ReleaseToZone,
            ),
            Err(QiFlowError::UnrepresentableFlow {
                field: "zone.spirit_qi",
                ..
            })
        ));
        assert_eq!(exact_release_actor.qi_snapshot(), exact_release_before);
        assert_eq!(exact_release_zone.spirit_qi, 0.5);
        assert_eq!(exact_release_ledger.total(), 0.0);
        assert!(exact_release_ledger.transfers().is_empty());

        let mut external_target = cultivation(1.0, 2.0);
        let mut external_ledger = WorldQiAccount::default();
        let external_before = external_target.qi_snapshot();
        assert!(matches!(
            actor_id.transfer_from_external(
                QiAccountId::container("tiny"),
                &mut external_target,
                &mut external_ledger,
                tiny,
                QiTransferReason::TradeDan,
            ),
            Err(QiFlowError::UnrepresentableFlow { .. })
        ));
        assert_eq!(external_target.qi_snapshot(), external_before);
        assert!(external_ledger.transfers().is_empty());

        let mut source = cultivation(1.0, 2.0);
        let mut target = cultivation(1.0, 2.0);
        let mut transfer_ledger = WorldQiAccount::default();
        assert!(matches!(
            source.transfer_to(
                QiFlowTarget::Actor(ActorQiTarget::new(
                    &mut target,
                    ActorQiIdentity::for_test(QiAccountId::player("target")),
                )),
                &mut transfer_ledger,
                &actor_id,
                tiny,
                QiTransferReason::Healing,
            ),
            Err(QiFlowError::UnrepresentableFlow { .. })
        ));
        assert_eq!(source.qi_current(), 1.0);
        assert_eq!(target.qi_current(), 1.0);
        assert!(transfer_ledger.transfers().is_empty());
    }

    #[test]
    fn extreme_finite_zone_conversion_fails_closed() {
        let mut actor = cultivation(1.0, 2.0);
        let mut extreme_zone = zone("extreme", f64::MAX);
        let mut ledger = WorldQiAccount::default();
        let before = actor.qi_snapshot();

        assert!(matches!(
            actor.release_to_zone(
                Some(&mut extreme_zone),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                1.0,
                QiTransferReason::ReleaseToZone,
            ),
            Err(QiFlowError::Physics(QiPhysicsError::InvalidAmount { .. }))
        ));
        assert_eq!(actor.qi_snapshot(), before);
        assert_eq!(extreme_zone.spirit_qi, f64::MAX);
        assert_eq!(ledger.total(), 0.0);
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn snapshot_uses_frozen_adjusted_effective_cap() {
        let mut cultivation = cultivation(4.0, 10.0);
        cultivation
            .set_for_init(CultivationQiInit {
                current: 4.0,
                max: 10.0,
                frozen: Some(2.5),
            })
            .unwrap();

        assert_eq!(
            cultivation.qi_snapshot(),
            CultivationQiSnapshot {
                current: 4.0,
                max: 10.0,
                frozen: Some(2.5),
                effective_max: 7.5,
                room: 3.5,
            }
        );
    }

    #[test]
    fn init_rejects_every_invalid_boundary_without_partial_write() {
        let invalid = [
            CultivationQiInit {
                current: -1.0,
                max: 10.0,
                frozen: None,
            },
            CultivationQiInit {
                current: 11.0,
                max: 10.0,
                frozen: None,
            },
            CultivationQiInit {
                current: f64::NAN,
                max: 10.0,
                frozen: None,
            },
            CultivationQiInit {
                current: 1.0,
                max: -1.0,
                frozen: None,
            },
            CultivationQiInit {
                current: 1.0,
                max: 10.0,
                frozen: Some(5.000_000_1),
            },
        ];

        for init in invalid {
            let mut cultivation = cultivation(2.0, 8.0);
            let before = cultivation.qi_snapshot();
            assert!(matches!(
                cultivation.set_for_init(init),
                Err(QiFlowError::InvalidInitSnapshot { .. })
            ));
            assert_eq!(cultivation.qi_snapshot(), before);
        }
    }

    #[test]
    fn external_source_credit_commits_actor_and_audit_together() {
        let actor = ActorQiIdentity::for_test(QiAccountId::npc("elder-1"));
        let source = QiAccountId::container("pill-1");
        let mut cultivation = cultivation(2.0, 10.0);
        let mut ledger = WorldQiAccount::default();

        let outcome = actor
            .transfer_from_external(
                source.clone(),
                &mut cultivation,
                &mut ledger,
                3.0,
                QiTransferReason::TradeDan,
            )
            .expect("valid external credit should commit");

        assert_eq!(cultivation.qi_current(), 5.0);
        assert_eq!(outcome.source_debited, 3.0);
        assert_eq!(outcome.target_credited, 3.0);
        assert_eq!(ledger.total(), 0.0, "external owners must not be mirrored");
        assert_eq!(ledger.transfers(), outcome.transfers);
        assert_eq!(outcome.transfers[0].from, source);
        assert_eq!(outcome.transfers[0].to, actor.account());
    }

    #[test]
    fn external_source_credit_zero_is_true_noop() {
        let actor = ActorQiIdentity::for_test(QiAccountId::npc("elder-1"));
        let mut cultivation = cultivation(2.0, 10.0);
        let before = cultivation.qi_snapshot();
        let mut ledger = WorldQiAccount::default();

        let outcome = actor
            .transfer_from_external(
                QiAccountId::container("pill-1"),
                &mut cultivation,
                &mut ledger,
                0.0,
                QiTransferReason::TradeDan,
            )
            .expect("zero credit should be a valid no-op");

        assert_eq!(cultivation.qi_snapshot(), before);
        assert_eq!(outcome, QiFlowOutcome::noop(0.0));
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn external_source_credit_rejects_every_invalid_boundary_atomically() {
        let actor = ActorQiIdentity::for_test(QiAccountId::npc("elder-1"));
        let cases = [
            (
                QiAccountId::container("pill-over-capacity"),
                3.0,
                QiTransferReason::TradeDan,
            ),
            (actor.account(), 1.0, QiTransferReason::TradeDan),
            (
                QiAccountId::container("pill-negative"),
                -1.0,
                QiTransferReason::TradeDan,
            ),
            (
                QiAccountId::container("pill-nan"),
                f64::NAN,
                QiTransferReason::TradeDan,
            ),
            (
                QiAccountId::container("pill-audit-only"),
                0.0,
                QiTransferReason::HalfStepBuff,
            ),
        ];

        for (source, requested, reason) in cases {
            let mut cultivation = cultivation(8.0, 10.0);
            let before = cultivation.qi_snapshot();
            let mut ledger = WorldQiAccount::default();
            assert!(actor
                .transfer_from_external(source, &mut cultivation, &mut ledger, requested, reason,)
                .is_err());
            assert_eq!(cultivation.qi_snapshot(), before);
            assert_eq!(ledger.total(), 0.0);
            assert!(ledger.transfers().is_empty());
        }
    }

    #[test]
    fn external_source_credit_rejects_invalid_actor_snapshot_atomically() {
        let actor = ActorQiIdentity::for_test(QiAccountId::npc("elder-1"));
        let mut cultivation = Cultivation {
            qi_current: f64::NAN,
            qi_max: 10.0,
            ..Cultivation::default()
        };
        let mut ledger = WorldQiAccount::default();

        assert!(matches!(
            actor.transfer_from_external(
                QiAccountId::container("pill-1"),
                &mut cultivation,
                &mut ledger,
                1.0,
                QiTransferReason::TradeDan,
            ),
            Err(QiFlowError::InvalidCultivationState { .. })
        ));
        assert!(cultivation.qi_current.is_nan());
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn gain_from_zone_commits_fields_and_ledger_together() {
        let mut cultivation = cultivation(1.0, 10.0);
        let mut zone = zone("spawn", 0.5);
        let mut ledger = WorldQiAccount::default();

        let outcome = cultivation
            .gain_from_zone(
                &mut zone,
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                3.0,
                QiTransferReason::CultivationRegen,
            )
            .unwrap();

        assert_eq!(cultivation.qi_current(), 4.0);
        assert!((zone.spirit_qi - 0.44).abs() < 1e-12);
        assert_eq!(outcome.source_debited, 3.0);
        assert_eq!(outcome.target_credited, 3.0);
        assert_eq!(ledger.transfers(), outcome.transfers);
        assert_eq!(
            ledger.total(),
            0.0,
            "external endpoints must not be mirrored"
        );
    }

    #[test]
    fn gain_respects_actor_room_and_leaves_untransferred_qi_in_zone() {
        let mut cultivation = cultivation(9.0, 10.0);
        let mut zone = zone("spawn", 0.5);
        let mut ledger = WorldQiAccount::default();

        let outcome = cultivation
            .gain_from_zone(
                &mut zone,
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                4.0,
                QiTransferReason::CultivationRegen,
            )
            .unwrap();

        assert_eq!(cultivation.qi_current(), 10.0);
        assert!((zone.spirit_qi - 0.48).abs() < 1e-12);
        assert_eq!(outcome.untransferred, 3.0);
    }

    #[test]
    fn gain_from_dead_or_negative_zone_is_true_noop() {
        for pressure in [0.0, -0.6] {
            let mut cultivation = cultivation(1.0, 10.0);
            let mut zone = zone("dead_edge", pressure);
            let mut ledger = WorldQiAccount::default();

            let outcome = cultivation
                .gain_from_zone(
                    &mut zone,
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                    3.0,
                    QiTransferReason::CultivationRegen,
                )
                .unwrap();

            assert_eq!(cultivation.qi_current(), 1.0);
            assert_eq!(zone.spirit_qi, pressure);
            assert_eq!(outcome.untransferred, 3.0);
            assert!(ledger.transfers().is_empty());
        }
    }

    #[test]
    fn typed_balance_transactions_reject_audit_only_reasons_atomically() {
        let audit_only_reasons = [
            QiTransferReason::HalfStepBuff,
            QiTransferReason::DuguReturnToZone,
            QiTransferReason::DuguReverseVictimQi,
        ];

        for reason in audit_only_reasons {
            let mut actor = cultivation(5.0, 10.0);
            let mut source_zone = zone("source", 0.5);
            let mut ledger = WorldQiAccount::default();
            assert!(matches!(
                actor.gain_from_zone(
                    &mut source_zone,
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                    1.0,
                    reason,
                ),
                Err(QiFlowError::Physics(QiPhysicsError::AuditOnlyReason { .. }))
            ));
            assert_eq!(actor.qi_current(), 5.0);
            assert_eq!(source_zone.spirit_qi, 0.5);
            assert!(ledger.transfers().is_empty());

            let mut release_zone = zone("release", 0.5);
            assert!(matches!(
                actor.release_to_zone(
                    Some(&mut release_zone),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                    1.0,
                    reason,
                ),
                Err(QiFlowError::Physics(QiPhysicsError::AuditOnlyReason { .. }))
            ));
            assert_eq!(actor.qi_current(), 5.0);
            assert_eq!(release_zone.spirit_qi, 0.5);
            assert_eq!(ledger.total(), 0.0);
            assert!(ledger.transfers().is_empty());
            let mut transfer_source = cultivation(5.0, 10.0);
            let mut transfer_target = cultivation(1.0, 10.0);
            assert!(matches!(
                transfer_source.transfer_to(
                    QiFlowTarget::Actor(ActorQiTarget::new(
                        &mut transfer_target,
                        ActorQiIdentity::for_test(QiAccountId::player("target")),
                    )),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("source")),
                    1.0,
                    reason,
                ),
                Err(QiFlowError::Physics(QiPhysicsError::AuditOnlyReason { .. }))
            ));
            assert_eq!(transfer_source.qi_current(), 5.0);
            assert_eq!(transfer_target.qi_current(), 1.0);
            assert_eq!(ledger.total(), 0.0);
            assert!(ledger.transfers().is_empty());

            let mut resize_actor = cultivation(5.0, 10.0);
            let mut resize_zone = zone("resize", 0.5);
            assert!(matches!(
                resize_actor.resize_qi_max_and_release_excess(
                    Some(&mut resize_zone),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("resize-source")),
                    4.0,
                    reason,
                ),
                Err(QiFlowError::Physics(QiPhysicsError::AuditOnlyReason { .. }))
            ));
            assert_eq!(
                resize_actor.qi_snapshot(),
                cultivation(5.0, 10.0).qi_snapshot()
            );
            assert_eq!(resize_zone.spirit_qi, 0.5);
            assert_eq!(ledger.total(), 0.0);
            assert!(ledger.transfers().is_empty());
        }
    }

    #[test]
    fn zero_balance_transactions_still_reject_audit_only_reasons() {
        let audit_only_reasons = [
            QiTransferReason::HalfStepBuff,
            QiTransferReason::DuguReturnToZone,
            QiTransferReason::DuguReverseVictimQi,
        ];
        for reason in audit_only_reasons {
            let mut actor = cultivation(5.0, 10.0);
            let mut zone = zone("zero", 0.5);
            let mut ledger = WorldQiAccount::default();

            assert!(matches!(
                actor.gain_from_zone(
                    &mut zone,
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("source")),
                    0.0,
                    reason,
                ),
                Err(QiFlowError::Physics(QiPhysicsError::AuditOnlyReason { .. }))
            ));
            assert!(matches!(
                actor.release_to_zone(
                    Some(&mut zone),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("source")),
                    0.0,
                    reason,
                ),
                Err(QiFlowError::Physics(QiPhysicsError::AuditOnlyReason { .. }))
            ));

            let mut target = cultivation(1.0, 10.0);
            assert!(matches!(
                actor.transfer_to(
                    QiFlowTarget::Actor(ActorQiTarget::new(
                        &mut target,
                        ActorQiIdentity::for_test(QiAccountId::player("target")),
                    )),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("source")),
                    0.0,
                    reason,
                ),
                Err(QiFlowError::Physics(QiPhysicsError::AuditOnlyReason { .. }))
            ));
            let unchanged_max = actor.qi_max();
            assert!(matches!(
                actor.resize_qi_max_and_release_excess(
                    Some(&mut zone),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("source")),
                    unchanged_max,
                    reason,
                ),
                Err(QiFlowError::Physics(QiPhysicsError::AuditOnlyReason { .. }))
            ));

            assert_eq!(actor.qi_current(), 5.0);
            assert_eq!(actor.qi_max(), 10.0);
            assert_eq!(target.qi_current(), 1.0);
            assert_eq!(zone.spirit_qi, 0.5);
            assert_eq!(ledger.total(), 0.0);
            assert!(ledger.transfers().is_empty());
        }
    }

    #[test]
    fn release_to_negative_zone_repays_signed_debt() {
        let mut cultivation = cultivation(5.0, 10.0);
        let mut zone = zone("dead_edge", -0.6);
        let mut ledger = WorldQiAccount::default();

        let outcome = cultivation
            .release_to_zone(
                Some(&mut zone),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                4.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap();

        assert_eq!(cultivation.qi_current(), 1.0);
        assert!((zone.spirit_qi - -0.52).abs() < 1e-12);
        assert_eq!(outcome.zone_accepted, 4.0);
        assert_eq!(outcome.overflow_credited, 0.0);
        assert_eq!(ledger.total(), 0.0);
    }

    #[test]
    fn full_or_missing_zone_credits_real_persistent_overflow() {
        for with_zone in [true, false] {
            let mut cultivation = cultivation(5.0, 10.0);
            let mut full_zone = zone("spawn", 1.0);
            let mut ledger = WorldQiAccount::default();

            let outcome = cultivation
                .release_to_zone(
                    with_zone.then_some(&mut full_zone),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                    4.0,
                    QiTransferReason::ReleaseToZone,
                )
                .unwrap();

            assert_eq!(cultivation.qi_current(), 1.0);
            assert_eq!(ledger.balance(&qi_flow_overflow_account()), 4.0);
            assert_eq!(outcome.overflow_credited, 4.0);
            assert_eq!(outcome.source_debited, outcome.target_credited);
        }
    }

    #[test]
    fn release_failure_keeps_actor_zone_and_ledger_unchanged() {
        let mut cultivation = cultivation(3.0, 10.0);
        let mut zone = zone("spawn", 0.5);
        let mut ledger = WorldQiAccount::default();
        let before = cultivation.qi_snapshot();

        let error = cultivation
            .release_to_zone(
                Some(&mut zone),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                4.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap_err();

        assert!(matches!(error, QiFlowError::InsufficientCurrent { .. }));
        assert_eq!(cultivation.qi_snapshot(), before);
        assert_eq!(zone.spirit_qi, 0.5);
        assert_eq!(ledger.total(), 0.0);
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn overflow_credit_failure_is_atomic_for_external_state() {
        let mut cultivation = cultivation(3.0, 10.0);
        let mut zone = zone("spawn", 1.0);
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(qi_flow_overflow_account(), f64::MAX)
            .unwrap();
        let before = cultivation.qi_snapshot();

        assert!(cultivation
            .release_to_zone(
                Some(&mut zone),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                1.0,
                QiTransferReason::ReleaseToZone,
            )
            .is_err());
        assert_eq!(cultivation.qi_snapshot(), before);
        assert_eq!(zone.spirit_qi, 1.0);
        assert_eq!(ledger.balance(&qi_flow_overflow_account()), f64::MAX);
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn actor_target_binds_audit_identity_to_life_record() {
        let player_record = LifeRecord::new("offline:target:incarnation-2");
        let player =
            ActorQiIdentity::from_life_record(&player_record, ActorQiKind::Player).unwrap();
        assert_eq!(
            player.account,
            QiAccountId::player(&player_record.character_id)
        );

        let npc_record = LifeRecord::new("npc:rogue:7");
        let npc = ActorQiIdentity::from_life_record(&npc_record, ActorQiKind::Npc).unwrap();
        assert_eq!(npc.account, QiAccountId::npc(&npc_record.character_id));
    }

    #[test]
    fn actor_target_rejects_blank_placeholder_or_noncanonical_life_record_identity() {
        for invalid_id in [
            "   ",
            "unassigned:life_record",
            " npc:rogue:7",
            "npc:rogue:7 ",
        ] {
            let invalid_record = LifeRecord::new(invalid_id);

            assert!(matches!(
                ActorQiIdentity::from_life_record(&invalid_record, ActorQiKind::Player),
                Err(QiFlowError::InvalidActorIdentity)
            ));
        }
    }

    #[test]
    fn split_release_keeps_ledger_and_outcome_audit_order_identical() {
        let mut cultivation = cultivation(8.0, 10.0);
        let mut zone = zone("spawn", 0.98);
        let mut ledger = WorldQiAccount::default();

        let outcome = cultivation
            .release_to_zone(
                Some(&mut zone),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                3.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap();

        assert!((outcome.zone_accepted - 1.0).abs() < 1e-12);
        assert!((outcome.overflow_credited - 2.0).abs() < 1e-12);
        assert_eq!(ledger.transfers(), outcome.transfers);
        assert_eq!(outcome.transfers[0].to, qi_flow_overflow_account());
        assert_eq!(outcome.transfers[1].to, QiAccountId::zone("spawn"));
        assert_eq!(
            outcome.source_debited,
            outcome.zone_accepted + outcome.overflow_credited
        );
    }

    #[test]
    fn transfer_to_near_cap_target_leaves_remainder_in_source() {
        let mut source = cultivation(8.0, 10.0);
        let mut target = cultivation(9.0, 10.0);
        let mut ledger = WorldQiAccount::default();

        let outcome = source
            .transfer_to(
                QiFlowTarget::Actor(ActorQiTarget::new(
                    &mut target,
                    ActorQiIdentity::for_test(QiAccountId::player("target")),
                )),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("source")),
                4.0,
                QiTransferReason::Healing,
            )
            .unwrap();

        assert_eq!(source.qi_current(), 7.0);
        assert_eq!(target.qi_current(), 10.0);
        assert_eq!(outcome.source_debited, 1.0);
        assert_eq!(outcome.untransferred, 3.0);
        assert_eq!(ledger.transfers().len(), 1);
    }

    #[test]
    fn zone_gain_rounding_past_effective_cap_fails_atomically() {
        const TARGET_CURRENT: f64 = 6.761_984_549_338_739e132;
        const EFFECTIVE_CAP: f64 = 1.942_838_657_673_836e133;
        const REQUESTED_ROOM: f64 = 1.266_640_202_739_962_2e133;

        for frozen in [None, Some(EFFECTIVE_CAP)] {
            let target_max = if frozen.is_some() {
                EFFECTIVE_CAP * 2.0
            } else {
                EFFECTIVE_CAP
            };
            let mut actor = cultivation(TARGET_CURRENT, target_max);
            actor
                .set_for_init(CultivationQiInit {
                    current: TARGET_CURRENT,
                    max: target_max,
                    frozen,
                })
                .unwrap();
            let mut zone = zone("extreme", REQUESTED_ROOM / QI_ZONE_UNIT_CAPACITY);
            let zone_before = zone.spirit_qi;
            let actor_before = actor.qi_snapshot();
            let mut ledger = WorldQiAccount::default();

            let error = actor
                .gain_from_zone(
                    &mut zone,
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("target")),
                    REQUESTED_ROOM,
                    QiTransferReason::CultivationRegen,
                )
                .unwrap_err();

            assert!(matches!(
                error,
                QiFlowError::UnrepresentableFlow {
                    field: "cultivation.qi_current",
                    ..
                }
            ));
            assert_eq!(actor.qi_snapshot(), actor_before);
            assert_eq!(zone.spirit_qi, zone_before);
            assert!(ledger.transfers().is_empty());
            assert_eq!(ledger.total(), 0.0);
        }
    }

    #[test]
    fn actor_target_rounding_past_effective_cap_fails_atomically() {
        const TARGET_CURRENT: f64 = 6.761_984_549_338_739e132;
        const EFFECTIVE_CAP: f64 = 1.942_838_657_673_836e133;
        const REQUESTED_ROOM: f64 = 1.266_640_202_739_962_2e133;

        for frozen in [None, Some(EFFECTIVE_CAP)] {
            let target_max = if frozen.is_some() {
                EFFECTIVE_CAP * 2.0
            } else {
                EFFECTIVE_CAP
            };
            let mut source = cultivation(REQUESTED_ROOM, REQUESTED_ROOM);
            let mut target = cultivation(TARGET_CURRENT, target_max);
            target
                .set_for_init(CultivationQiInit {
                    current: TARGET_CURRENT,
                    max: target_max,
                    frozen,
                })
                .unwrap();
            assert_eq!(target.effective_qi_max(), EFFECTIVE_CAP);
            assert_eq!(target.qi_room(), REQUESTED_ROOM);
            assert!(target.qi_current() + target.qi_room() > target.effective_qi_max());

            let source_before = source.qi_snapshot();
            let target_before = target.qi_snapshot();
            let mut ledger = WorldQiAccount::default();
            let error = source
                .transfer_to(
                    QiFlowTarget::Actor(ActorQiTarget::new(
                        &mut target,
                        ActorQiIdentity::for_test(QiAccountId::player("target")),
                    )),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("source")),
                    REQUESTED_ROOM,
                    QiTransferReason::Healing,
                )
                .unwrap_err();

            assert!(matches!(
                error,
                QiFlowError::UnrepresentableFlow {
                    field: "cultivation.qi_current.target",
                    ..
                }
            ));
            assert_eq!(source.qi_snapshot(), source_before);
            assert_eq!(target.qi_snapshot(), target_before);
            assert!(ledger.transfers().is_empty());
            assert_eq!(ledger.total(), 0.0);
        }
    }

    #[test]
    fn transfer_to_stable_account_commits_real_balance_before_source_debit() {
        let mut source = cultivation(8.0, 10.0);
        let mut ledger = WorldQiAccount::default();
        let target = qi_flow_overflow_account();

        let outcome = source
            .transfer_to(
                QiFlowTarget::Persistent(PersistentQiSink::QiFlowOverflow),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("source")),
                3.0,
                QiTransferReason::VoidAction,
            )
            .unwrap();

        assert_eq!(source.qi_current(), 5.0);
        assert_eq!(ledger.balance(&target), 3.0);
        assert_eq!(outcome.source_debited, 3.0);
        assert_eq!(outcome.target_credited, 3.0);
        assert_eq!(ledger.transfers(), outcome.transfers);
    }

    #[test]
    fn stable_account_credit_failure_keeps_source_and_ledger_unchanged() {
        let mut source = cultivation(8.0, 10.0);
        let mut ledger = WorldQiAccount::default();
        let target = qi_flow_overflow_account();
        ledger.set_balance(target.clone(), f64::MAX).unwrap();

        assert!(source
            .transfer_to(
                QiFlowTarget::Persistent(PersistentQiSink::QiFlowOverflow),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("source")),
                1.0,
                QiTransferReason::VoidAction,
            )
            .is_err());
        assert_eq!(source.qi_current(), 8.0);
        assert_eq!(ledger.balance(&target), f64::MAX);
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn transfer_rejects_overdraft_and_same_account_without_mutation() {
        let cases = [(9.0, "target"), (1.0, "source")];
        for (requested, target_id) in cases {
            let mut source = cultivation(8.0, 10.0);
            let mut target = cultivation(1.0, 10.0);
            let mut ledger = WorldQiAccount::default();

            assert!(source
                .transfer_to(
                    QiFlowTarget::Actor(ActorQiTarget::new(
                        &mut target,
                        ActorQiIdentity::for_test(QiAccountId::player(target_id)),
                    )),
                    &mut ledger,
                    &ActorQiIdentity::for_test(QiAccountId::player("source")),
                    requested,
                    QiTransferReason::Healing,
                )
                .is_err());
            assert_eq!(source.qi_current(), 8.0);
            assert_eq!(target.qi_current(), 1.0);
            assert!(ledger.transfers().is_empty());
        }
    }

    #[test]
    fn resize_without_excess_changes_capacity_only() {
        let mut cultivation = cultivation(4.0, 10.0);
        let mut ledger = WorldQiAccount::default();

        let outcome = cultivation
            .resize_qi_max_and_release_excess(
                None,
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                6.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap();

        assert_eq!(outcome.excess, 0.0);
        assert!(outcome.release.is_none());
        assert_eq!(cultivation.qi_current(), 4.0);
        assert_eq!(cultivation.qi_max(), 6.0);
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn resize_releases_excess_and_clamps_frozen_metadata() {
        let mut cultivation = cultivation(9.0, 10.0);
        cultivation
            .set_for_init(CultivationQiInit {
                current: 9.0,
                max: 10.0,
                frozen: Some(5.0),
            })
            .unwrap();
        let mut zone = zone("dead_edge", -0.2);
        let mut ledger = WorldQiAccount::default();

        let outcome = cultivation
            .resize_qi_max_and_release_excess(
                Some(&mut zone),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                5.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap();

        assert_eq!(outcome.excess, 4.0);
        assert_eq!(cultivation.qi_current(), 5.0);
        assert_eq!(cultivation.qi_max(), 5.0);
        assert_eq!(cultivation.qi_max_frozen(), Some(2.5));
        assert!((zone.spirit_qi - -0.12).abs() < 1e-12);
    }

    #[test]
    fn resize_release_failure_leaves_max_current_zone_and_ledger_unchanged() {
        let mut cultivation = cultivation(9.0, 10.0);
        let mut zone = zone("spawn", 1.0);
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(qi_flow_overflow_account(), f64::MAX)
            .unwrap();
        let before = cultivation.qi_snapshot();

        assert!(cultivation
            .resize_qi_max_and_release_excess(
                Some(&mut zone),
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                5.0,
                QiTransferReason::ReleaseToZone,
            )
            .is_err());
        assert_eq!(cultivation.qi_snapshot(), before);
        assert_eq!(zone.spirit_qi, 1.0);
        assert_eq!(ledger.balance(&qi_flow_overflow_account()), f64::MAX);
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn invalid_live_state_is_rejected_before_any_flow() {
        let mut cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 11.0,
            qi_max: 10.0,
            ..Cultivation::default()
        };
        let mut zone = zone("spawn", 0.5);
        let mut ledger = WorldQiAccount::default();

        assert!(matches!(
            cultivation.gain_from_zone(
                &mut zone,
                &mut ledger,
                &ActorQiIdentity::for_test(QiAccountId::player("p1")),
                1.0,
                QiTransferReason::CultivationRegen,
            ),
            Err(QiFlowError::InvalidCultivationState { .. })
        ));
        assert_eq!(zone.spirit_qi, 0.5);
        assert!(ledger.transfers().is_empty());
    }
}
