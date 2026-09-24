use std::collections::HashMap;

use valence::prelude::App;

use crate::cultivation::components::Cultivation;
use crate::inventory::{
    ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState, PlayerInventory,
    SlotContents, EQUIP_SLOT_CHEST, EQUIP_SLOT_MAIN_HAND,
};
use crate::world::zone::ZoneRegistry;

use super::*;

#[test]
fn ledger_to_zone_credits_external_owner_without_zone_shadow() {
    let mut ledger = WorldQiAccount::default();
    let source = pending_inflow_account();
    let zone_account = QiAccountId::zone("spawn");
    ledger.set_balance(source.clone(), 12.0).unwrap();
    let mut zone_spirit_qi = 0.25;

    let transfer = transfer_ledger_qi_to_zone(
        &mut ledger,
        source.clone(),
        "spawn",
        &mut zone_spirit_qi,
        5.0,
        1.0,
        QiTransferReason::ZoneInflow,
    )
    .expect("stable-pool to external-zone transfer should settle")
    .expect("positive transfer must produce audit");

    assert_eq!(ledger.balance(&source), 7.0);
    assert_eq!(zone_spirit_qi, 0.25 + 5.0 / QI_ZONE_UNIT_CAPACITY);
    assert!(
        !ledger.has_account(&zone_account),
        "external Zone.spirit_qi is the physical owner; no zone:* ledger mirror may remain"
    );
    assert_eq!(transfer.from, source);
    assert_eq!(transfer.to, zone_account);
    assert_eq!(ledger.transfers(), &[transfer]);
}

#[test]
fn ledger_to_zone_repays_signed_zone_debt() {
    let mut ledger = WorldQiAccount::default();
    let source = pending_inflow_account();
    ledger.set_balance(source.clone(), 30.0).unwrap();
    let mut zone_spirit_qi = -1.2;

    transfer_ledger_qi_to_zone(
        &mut ledger,
        source,
        "negative_domain",
        &mut zone_spirit_qi,
        20.0,
        1.0,
        QiTransferReason::ZoneInflow,
    )
    .expect("signed negative zone must accept stable qi")
    .expect("positive transfer must produce audit");

    assert_eq!(zone_spirit_qi, -1.2 + 20.0 / QI_ZONE_UNIT_CAPACITY);
    assert!(
        zone_spirit_qi < 0.0,
        "inflow must repay negative pressure instead of clamping the zone to zero"
    );
}

#[test]
fn ledger_to_zone_zero_amount_is_noop() {
    let mut ledger = WorldQiAccount::default();
    let source = pending_inflow_account();
    ledger.set_balance(source.clone(), 3.0).unwrap();
    let mut zone_spirit_qi = -0.4;

    let transfer = transfer_ledger_qi_to_zone(
        &mut ledger,
        source.clone(),
        "spawn",
        &mut zone_spirit_qi,
        0.0,
        1.0,
        QiTransferReason::ZoneInflow,
    )
    .expect("zero transfer is an explicit no-op");

    assert!(transfer.is_none());
    assert_eq!(ledger.balance(&source), 3.0);
    assert_eq!(zone_spirit_qi, -0.4);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn ledger_to_zone_ceiling_preflight_debits_only_the_accepted_amount() {
    let mut ledger = WorldQiAccount::default();
    let source = pending_inflow_account();
    ledger.set_balance(source.clone(), 12.0).unwrap();
    let mut zone_spirit_qi = 0.99;

    let transfer = transfer_ledger_qi_to_zone(
        &mut ledger,
        source.clone(),
        "spawn",
        &mut zone_spirit_qi,
        5.0,
        1.0,
        QiTransferReason::ZoneInflow,
    )
    .expect("ceiling-clamped transfer should settle")
    .expect("positive room must produce an audit");

    assert_eq!(zone_spirit_qi, 1.0);
    assert!((transfer.amount - 0.5).abs() < 1e-12);
    assert!((ledger.balance(&source) - 11.5).abs() < 1e-12);
    assert_eq!(ledger.transfers(), &[transfer]);
}

#[test]
fn ledger_to_zone_failures_leave_both_owners_and_audit_untouched() {
    let cases = [
        ("insufficient", 2.0, 3.0, 0.2, 1.0),
        ("invalid-zone", 4.0, 1.0, f64::NAN, 1.0),
        ("invalid-ceiling", 4.0, 1.0, 0.2, f64::NAN),
    ];
    for (label, source_balance, amount, initial_zone, ceiling) in cases {
        let mut ledger = WorldQiAccount::default();
        let source = pending_inflow_account();
        ledger.set_balance(source.clone(), source_balance).unwrap();
        let mut zone_spirit_qi = initial_zone;

        let error = transfer_ledger_qi_to_zone(
            &mut ledger,
            source.clone(),
            "spawn",
            &mut zone_spirit_qi,
            amount,
            ceiling,
            QiTransferReason::ZoneInflow,
        )
        .expect_err("invalid transaction must fail closed");

        match label {
            "insufficient" => assert!(matches!(error, QiPhysicsError::InsufficientQi { .. })),
            "invalid-zone" => assert!(matches!(
                error,
                QiPhysicsError::InvalidAmount {
                    field: "zone.spirit_qi",
                    ..
                }
            )),
            "invalid-ceiling" => assert!(matches!(
                error,
                QiPhysicsError::InvalidAmount {
                    field: "ledger_to_zone.zone_ceiling",
                    ..
                }
            )),
            _ => unreachable!(),
        }
        assert_eq!(ledger.balance(&source), source_balance, "{label}");
        if initial_zone.is_nan() {
            assert!(zone_spirit_qi.is_nan(), "{label}");
        } else {
            assert_eq!(zone_spirit_qi, initial_zone, "{label}");
        }
        assert!(ledger.transfers().is_empty(), "{label}");
        assert!(!ledger.has_account(&QiAccountId::zone("spawn")), "{label}");
    }
}

#[test]
fn probe_transaction_discards_overlay_balances_audits_and_external_zone_state() {
    let source = QiAccountId::player("probe-source");
    let target = QiAccountId::zone("spawn");
    let mut ledger = WorldQiAccount::default();
    ledger.set_balance(source.clone(), 12.0).unwrap();
    ledger.push_transfer_audit(
        QiTransfer::new(
            QiAccountId::tiandao(),
            source.clone(),
            1.0,
            QiTransferReason::ZoneInflow,
        )
        .unwrap(),
    );
    let zones = ZoneRegistry::fallback();
    let zone_qi_before = zones.zones[0].spirit_qi;
    let balances_before: Vec<_> = ledger
        .iter_balances()
        .map(|(account, balance)| (account.clone(), balance))
        .collect();
    let transfers_before = ledger.transfers().len();

    let staged_transfer = ledger.probe_transaction(|transaction| {
        let transfer = transaction
            .transfer_external_qi_to_ledger(
                source.clone(),
                target.clone(),
                3.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap()
            .expect("positive probe transfer should produce an audit record");
        transaction.push_transfer_audit(transfer.clone());
        transfer
    });

    assert_eq!(staged_transfer.from, source);
    assert_eq!(staged_transfer.to, target);
    assert_eq!(staged_transfer.amount, 3.0);
    assert_eq!(
        ledger
            .iter_balances()
            .map(|(account, balance)| (account.clone(), balance))
            .collect::<Vec<_>>(),
        balances_before,
        "只读账本探测不得提交任何余额 overlay"
    );
    assert_eq!(
        ledger.transfers().len(),
        transfers_before,
        "只读账本探测不得追加 transfer 审计"
    );
    assert_eq!(
        zones.zones[0].spirit_qi, zone_qi_before,
        "只读账本探测不得改变外部 zone 灵气"
    );
}

#[test]
fn ledger_to_zone_rejects_audit_only_reasons_without_mutation() {
    let reasons = [
        QiTransferReason::HalfStepBuff,
        QiTransferReason::DuguReturnToZone,
        QiTransferReason::DuguReverseVictimQi,
        QiTransferReason::NegPressureDrain,
    ];
    for reason in reasons {
        let mut ledger = WorldQiAccount::default();
        let source = pending_inflow_account();
        ledger.set_balance(source.clone(), 4.0).unwrap();
        let mut zone_spirit_qi = 0.3;

        assert!(matches!(
            transfer_ledger_qi_to_zone(
                &mut ledger,
                source.clone(),
                "spawn",
                &mut zone_spirit_qi,
                1.0,
                1.0,
                reason,
            ),
            Err(QiPhysicsError::AuditOnlyReason { .. })
        ));
        assert_eq!(ledger.balance(&source), 4.0);
        assert_eq!(zone_spirit_qi, 0.3);
        assert!(ledger.transfers().is_empty());
        assert!(!ledger.has_account(&QiAccountId::zone("spawn")));
    }
}

#[test]
fn ledger_to_zone_rejects_same_account_without_mutation() {
    let mut ledger = WorldQiAccount::default();
    let source = QiAccountId::zone("spawn");
    ledger.set_balance(source.clone(), 4.0).unwrap();
    let mut zone_spirit_qi = 0.3;

    let error = transfer_ledger_qi_to_zone(
        &mut ledger,
        source.clone(),
        "spawn",
        &mut zone_spirit_qi,
        1.0,
        1.0,
        QiTransferReason::ZoneInflow,
    )
    .expect_err("source and audit destination must differ");

    assert!(matches!(error, QiPhysicsError::SameAccountTransfer { .. }));
    assert_eq!(ledger.balance(&source), 4.0);
    assert_eq!(zone_spirit_qi, 0.3);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn external_zone_sub_ulp_transfers_fail_before_owner_or_audit_mutation() {
    let tiny = f64::MIN_POSITIVE;

    let mut inflow_ledger = WorldQiAccount::default();
    let source = pending_inflow_account();
    inflow_ledger.set_balance(source.clone(), tiny).unwrap();
    let mut inflow_zone = 1.0;
    assert!(matches!(
        transfer_ledger_qi_to_zone(
            &mut inflow_ledger,
            source.clone(),
            "spawn",
            &mut inflow_zone,
            tiny,
            2.0,
            QiTransferReason::ZoneInflow,
        ),
        Err(QiPhysicsError::UnrepresentableChange {
            field: "zone.spirit_qi",
            ..
        })
    ));
    assert_eq!(inflow_ledger.balance(&source), tiny);
    assert_eq!(inflow_zone, 1.0);
    assert!(inflow_ledger.transfers().is_empty());

    let mut settle_ledger = WorldQiAccount::default();
    let destination = pending_inflow_account();
    let mut settle_zone = 1.0;
    assert!(matches!(
        transfer_zone_qi_to_ledger(
            &mut settle_ledger,
            "spawn",
            &mut settle_zone,
            destination.clone(),
            tiny,
            QiTransferReason::PseudoVeinSettle,
        ),
        Err(QiPhysicsError::UnrepresentableChange {
            field: "zone.spirit_qi",
            ..
        })
    ));
    assert_eq!(settle_zone, 1.0);
    assert!(!settle_ledger.has_account(&destination));
    assert!(settle_ledger.transfers().is_empty());
}

#[test]
fn zone_to_ledger_debits_external_owner_without_zone_shadow() {
    let mut ledger = WorldQiAccount::default();
    let destination = pending_inflow_account();
    let zone_account = QiAccountId::zone("spawn");
    ledger.set_balance(destination.clone(), 3.0).unwrap();
    let mut zone_spirit_qi = 0.4;

    let transfer = transfer_zone_qi_to_ledger(
        &mut ledger,
        "spawn",
        &mut zone_spirit_qi,
        destination.clone(),
        7.0,
        QiTransferReason::PseudoVeinSettle,
    )
    .expect("external-zone to stable-pool transfer should settle")
    .expect("positive transfer must produce an audit");

    assert_eq!(zone_spirit_qi, 0.4 - 7.0 / QI_ZONE_UNIT_CAPACITY);
    assert_eq!(ledger.balance(&destination), 10.0);
    assert!(
        !ledger.has_account(&zone_account),
        "external Zone.spirit_qi is the physical owner; no zone:* ledger mirror may remain"
    );
    assert_eq!(transfer.from, zone_account);
    assert_eq!(transfer.to, destination);
    assert_eq!(transfer.amount, 7.0);
    assert_eq!(ledger.transfers(), &[transfer]);
}

#[test]
fn zone_to_ledger_exact_drain_commits_zero_without_rounding_residue() {
    let mut ledger = WorldQiAccount::default();
    let destination = pending_inflow_account();
    let mut zone_spirit_qi = 0.2;

    let transfer = transfer_zone_qi_to_ledger(
        &mut ledger,
        "spawn",
        &mut zone_spirit_qi,
        destination.clone(),
        10.0,
        QiTransferReason::PseudoVeinSettle,
    )
    .expect("exact drain should settle")
    .expect("positive transfer must produce an audit");

    assert_eq!(zone_spirit_qi, 0.0);
    assert_eq!(ledger.balance(&destination), 10.0);
    assert_eq!(transfer.amount, 10.0);
    assert!(!ledger.has_account(&QiAccountId::zone("spawn")));
}

#[test]
fn zone_to_ledger_zero_is_a_valid_noop_without_creating_accounts() {
    let mut ledger = WorldQiAccount::default();
    let destination = pending_inflow_account();
    let mut zone_spirit_qi = -1.2;

    let transfer = transfer_zone_qi_to_ledger(
        &mut ledger,
        "negative_domain",
        &mut zone_spirit_qi,
        destination.clone(),
        0.0,
        QiTransferReason::PseudoVeinSettle,
    )
    .expect("valid zero transfer should be an explicit no-op");

    assert!(transfer.is_none());
    assert_eq!(zone_spirit_qi, -1.2);
    assert!(!ledger.has_account(&destination));
    assert!(!ledger.has_account(&QiAccountId::zone("negative_domain")));
    assert!(ledger.transfers().is_empty());
}

#[test]
fn zone_to_ledger_failures_leave_both_owners_and_audit_untouched() {
    struct Case {
        label: &'static str,
        initial_zone: f64,
        requested: f64,
        initial_destination: f64,
        expected_field: Option<&'static str>,
        insufficient: bool,
    }

    let cases = [
        Case {
            label: "insufficient-positive-zone",
            initial_zone: 0.1,
            requested: 6.0,
            initial_destination: 2.0,
            expected_field: None,
            insufficient: true,
        },
        Case {
            label: "negative-zone-is-not-a-source",
            initial_zone: -1.2,
            requested: 1.0,
            initial_destination: 2.0,
            expected_field: None,
            insufficient: true,
        },
        Case {
            label: "invalid-zone",
            initial_zone: f64::NAN,
            requested: 1.0,
            initial_destination: 2.0,
            expected_field: Some("zone.spirit_qi"),
            insufficient: false,
        },
        Case {
            label: "negative-request",
            initial_zone: 0.2,
            requested: -1.0,
            initial_destination: 2.0,
            expected_field: Some("zone_to_ledger.requested"),
            insufficient: false,
        },
        Case {
            label: "nan-request",
            initial_zone: 0.2,
            requested: f64::NAN,
            initial_destination: 2.0,
            expected_field: Some("zone_to_ledger.requested"),
            insufficient: false,
        },
        Case {
            label: "infinite-request",
            initial_zone: 0.2,
            requested: f64::INFINITY,
            initial_destination: 2.0,
            expected_field: Some("zone_to_ledger.requested"),
            insufficient: false,
        },
        Case {
            label: "destination-overflow",
            initial_zone: (f64::MAX * 0.5) / QI_ZONE_UNIT_CAPACITY,
            requested: f64::MAX * 0.5,
            initial_destination: f64::MAX * 0.75,
            expected_field: Some("destination_balance"),
            insufficient: false,
        },
    ];

    for case in cases {
        let mut ledger = WorldQiAccount::default();
        let destination = pending_inflow_account();
        ledger
            .set_balance(destination.clone(), case.initial_destination)
            .unwrap();
        let mut zone_spirit_qi = case.initial_zone;

        let error = transfer_zone_qi_to_ledger(
            &mut ledger,
            "spawn",
            &mut zone_spirit_qi,
            destination.clone(),
            case.requested,
            QiTransferReason::PseudoVeinSettle,
        )
        .expect_err("invalid transaction must fail closed");

        if case.insufficient {
            assert!(
                matches!(error, QiPhysicsError::InsufficientQi { .. }),
                "{}: {error:?}",
                case.label
            );
        } else {
            assert!(
                matches!(
                    error,
                    QiPhysicsError::InvalidAmount { field, .. }
                        if Some(field) == case.expected_field
                ),
                "{}: {error:?}",
                case.label
            );
        }
        if case.initial_zone.is_nan() {
            assert!(zone_spirit_qi.is_nan(), "{}", case.label);
        } else {
            assert_eq!(zone_spirit_qi, case.initial_zone, "{}", case.label);
        }
        assert_eq!(
            ledger.balance(&destination),
            case.initial_destination,
            "{}",
            case.label
        );
        assert!(ledger.transfers().is_empty(), "{}", case.label);
        assert!(
            !ledger.has_account(&QiAccountId::zone("spawn")),
            "{}",
            case.label
        );
    }
}

#[test]
fn zone_to_ledger_rejects_audit_only_reasons_even_for_zero() {
    let reasons = [
        QiTransferReason::HalfStepBuff,
        QiTransferReason::DuguReturnToZone,
        QiTransferReason::DuguReverseVictimQi,
        QiTransferReason::NegPressureDrain,
    ];
    for reason in reasons {
        for requested in [0.0, 1.0] {
            let mut ledger = WorldQiAccount::default();
            let destination = pending_inflow_account();
            ledger.set_balance(destination.clone(), 2.0).unwrap();
            let mut zone_spirit_qi = 0.3;

            assert!(matches!(
                transfer_zone_qi_to_ledger(
                    &mut ledger,
                    "spawn",
                    &mut zone_spirit_qi,
                    destination.clone(),
                    requested,
                    reason,
                ),
                Err(QiPhysicsError::AuditOnlyReason { .. })
            ));
            assert_eq!(zone_spirit_qi, 0.3, "reason={reason:?} amount={requested}");
            assert_eq!(
                ledger.balance(&destination),
                2.0,
                "reason={reason:?} amount={requested}"
            );
            assert!(ledger.transfers().is_empty());
            assert!(!ledger.has_account(&QiAccountId::zone("spawn")));
        }
    }
}

#[test]
fn zone_to_ledger_rejects_same_account_without_mutation() {
    let mut ledger = WorldQiAccount::default();
    let zone_account = QiAccountId::zone("spawn");
    ledger.set_balance(zone_account.clone(), 4.0).unwrap();
    let mut zone_spirit_qi = 0.3;

    let error = transfer_zone_qi_to_ledger(
        &mut ledger,
        "spawn",
        &mut zone_spirit_qi,
        zone_account.clone(),
        1.0,
        QiTransferReason::PseudoVeinSettle,
    )
    .expect_err("external source and ledger destination must differ");

    assert!(matches!(error, QiPhysicsError::SameAccountTransfer { .. }));
    assert_eq!(zone_spirit_qi, 0.3);
    assert_eq!(ledger.balance(&zone_account), 4.0);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn budget_defaults_to_config_default() {
    let budget = WorldQiBudget::default();
    assert_eq!(budget.initial_total, DEFAULT_SPIRIT_QI_TOTAL);
    assert_eq!(budget.current_total, DEFAULT_SPIRIT_QI_TOTAL);
}

#[test]
fn budget_rejects_invalid_total_to_default() {
    let budget = WorldQiBudget::from_total(-1.0);
    assert_eq!(budget.current_total, DEFAULT_SPIRIT_QI_TOTAL);
}

#[test]
fn budget_era_decay_updates_current_total() {
    let mut budget = WorldQiBudget::from_total(100.0);
    let decay = budget.apply_era_decay(0.02).expect("valid decay");
    assert_eq!(decay, 2.0);
    assert_eq!(budget.current_total, 98.0);
    assert_eq!(budget.era_decay_accum, 2.0);
}

#[test]
fn pending_inflow_account_uses_overflow_kind_and_stable_id() {
    let account = pending_inflow_account();
    assert_eq!(account.kind, QiAccountKind::Overflow);
    assert_eq!(account.id, PENDING_INFLOW_ACCOUNT_ID);
}

#[test]
fn fixed_runtime_accounts_preserve_physical_kinds_and_stable_ids() {
    let accounts = persistent_runtime_qi_accounts();
    assert_eq!(accounts.len(), PERSISTENT_RUNTIME_QI_ACCOUNT_IDS.len());
    assert_eq!(rift_drain_account().kind, QiAccountKind::Rift);
    assert_eq!(rift_drain_account().id, RIFT_DRAIN_ACCOUNT_ID);
    assert!(accounts.contains(&rift_drain_account()));
}

#[test]
fn transfer_rejects_sub_ulp_changes_without_partial_mutation() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("source");
    let to = QiAccountId::overflow("destination");
    ledger
        .set_balance(from.clone(), 1.0)
        .expect("fixture source balance");
    ledger
        .set_balance(to.clone(), 1.0)
        .expect("fixture destination balance");

    let error = ledger
        .transfer(
            QiTransfer::new(
                from.clone(),
                to.clone(),
                f64::MIN_POSITIVE,
                QiTransferReason::ReleaseToZone,
            )
            .expect("tiny positive transfer should pass amount validation"),
        )
        .expect_err("sub-ULP destination credit must fail before any balance changes");

    assert!(matches!(
        error,
        QiPhysicsError::InvalidAmount {
            field: "destination_balance",
            ..
        }
    ));
    assert_eq!(ledger.balance(&from), 1.0);
    assert_eq!(ledger.balance(&to), 1.0);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn transfer_rejects_destination_overflow_without_partial_mutation() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("source");
    let to = QiAccountId::overflow("full");
    ledger
        .set_balance(from.clone(), 2.0)
        .expect("fixture source balance");
    ledger
        .set_balance(to.clone(), f64::MAX)
        .expect("fixture destination balance");

    let error = ledger
        .transfer(
            QiTransfer::new(
                from.clone(),
                to.clone(),
                1.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap(),
        )
        .expect_err("finite destination plus amount must not become infinity");

    assert!(matches!(
        error,
        QiPhysicsError::InvalidAmount {
            field: "destination_balance",
            ..
        }
    ));
    assert_eq!(ledger.balance(&from), 2.0);
    assert_eq!(ledger.balance(&to), f64::MAX);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn transfer_rejects_same_account_without_mutation() {
    let mut ledger = WorldQiAccount::default();
    let account = QiAccountId::overflow("same");
    ledger
        .set_balance(account.clone(), 4.0)
        .expect("fixture balance");

    let error = ledger
        .transfer(
            QiTransfer::new(
                account.clone(),
                account.clone(),
                1.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap(),
        )
        .expect_err("same-account transfer cannot conserve by debit then overwrite credit");

    assert!(matches!(error, QiPhysicsError::SameAccountTransfer { .. }));
    assert_eq!(ledger.balance(&account), 4.0);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn external_qi_transfer_credits_sink_and_restores_existing_source_exactly() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::npc("external_elder");
    let to = QiAccountId::overflow("elder_overflow");
    ledger
        .set_balance(from.clone(), 7.0)
        .expect("fixture source balance");

    let transfer = transfer_external_qi_to_ledger(
        &mut ledger,
        from.clone(),
        to.clone(),
        12.5,
        QiTransferReason::ReleaseToZone,
    )
    .expect("external transfer should succeed")
    .expect("positive amount should produce transfer");

    assert_eq!(ledger.balance(&from), 7.0, "source shadow must be restored");
    assert_eq!(ledger.balance(&to), 12.5, "sink must receive real balance");
    assert_eq!(ledger.transfers(), &[transfer]);
}

#[test]
fn external_qi_transfer_removes_temporary_source_account_after_success() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::container("consumed_pill:1");
    let to = QiAccountId::overflow("pill_excess");

    transfer_external_qi_to_ledger(
        &mut ledger,
        from.clone(),
        to.clone(),
        3.0,
        QiTransferReason::TradeDan,
    )
    .expect("external transfer should succeed");

    assert!(
        !ledger.has_account(&from),
        "temporary source account must not persist after success"
    );
    assert_eq!(ledger.balance(&to), 3.0);
}

#[test]
fn external_qi_transfer_rejects_sub_ulp_source_shadow_without_mutation() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("external_player");
    let to = QiAccountId::overflow("must_stay_empty");
    ledger.set_balance(from.clone(), 1.0).unwrap();

    assert!(matches!(
        transfer_external_qi_to_ledger(
            &mut ledger,
            from.clone(),
            to.clone(),
            f64::MIN_POSITIVE,
            QiTransferReason::ReleaseToZone,
        ),
        Err(QiPhysicsError::UnrepresentableChange {
            field: "source_shadow_balance",
            ..
        })
    ));
    assert_eq!(ledger.balance(&from), 1.0);
    assert!(!ledger.has_account(&to));
    assert!(ledger.transfers().is_empty());
}

#[test]
fn external_qi_transfer_failure_rolls_source_back_without_credit_or_audit() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("external_player");
    let to = QiAccountId::overflow("must_stay_empty");
    ledger
        .set_balance(from.clone(), 4.0)
        .expect("fixture source balance");

    let error = transfer_external_qi_to_ledger(
        &mut ledger,
        from.clone(),
        to.clone(),
        2.0,
        QiTransferReason::NegPressureDrain,
    )
    .expect_err("audit-only reason must reject real balance mutation");

    assert!(matches!(error, QiPhysicsError::AuditOnlyReason { .. }));
    assert_eq!(ledger.balance(&from), 4.0, "failure must restore source");
    assert_eq!(ledger.balance(&to), 0.0, "failure must not credit sink");
    assert!(
        ledger.transfers().is_empty(),
        "failure must not append audit"
    );
}

#[test]
fn external_qi_transfer_zero_is_noop() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::npc("external_elder");
    let to = QiAccountId::overflow("elder_overflow");

    let result = transfer_external_qi_to_ledger(
        &mut ledger,
        from.clone(),
        to.clone(),
        0.0,
        QiTransferReason::ReleaseToZone,
    )
    .expect("zero must be accepted as no-op");

    assert!(result.is_none());
    assert!(!ledger.has_account(&from));
    assert!(!ledger.has_account(&to));
    assert!(ledger.transfers().is_empty());
}

#[test]
fn external_qi_transfer_zero_validates_amount_and_reason_before_noop() {
    let audit_only_reasons = [
        QiTransferReason::HalfStepBuff,
        QiTransferReason::DuguReturnToZone,
        QiTransferReason::DuguReverseVictimQi,
        QiTransferReason::NegPressureDrain,
    ];
    for reason in audit_only_reasons {
        let mut ledger = WorldQiAccount::default();
        let from = QiAccountId::npc("external_elder");
        let to = QiAccountId::overflow("elder_overflow");

        assert!(matches!(
            transfer_external_qi_to_ledger(&mut ledger, from, to, 0.0, reason),
            Err(QiPhysicsError::AuditOnlyReason { .. })
        ));
        assert!(ledger.iter_balances().next().is_none());
        assert!(ledger.transfers().is_empty());
    }

    for amount in [-1.0, f64::NAN, f64::INFINITY] {
        let mut ledger = WorldQiAccount::default();
        assert!(matches!(
            transfer_external_qi_to_ledger(
                &mut ledger,
                QiAccountId::npc("external_elder"),
                QiAccountId::overflow("elder_overflow"),
                amount,
                QiTransferReason::ReleaseToZone,
            ),
            Err(QiPhysicsError::InvalidAmount {
                field: "transfer.amount",
                ..
            })
        ));
        assert!(ledger.iter_balances().next().is_none());
        assert!(ledger.transfers().is_empty());
    }
}

#[test]
fn external_qi_transfer_same_account_zero_preserves_noop_contract() {
    let mut ledger = WorldQiAccount::default();
    let account = QiAccountId::overflow("same-account-zero");
    ledger
        .set_balance(account.clone(), 9.0)
        .expect("fixture balance should be valid");

    let result = transfer_external_qi_to_ledger(
        &mut ledger,
        account.clone(),
        account.clone(),
        0.0,
        QiTransferReason::ReleaseToZone,
    )
    .expect("zero amount remains a no-op even when source equals destination");

    assert!(result.is_none());
    assert_eq!(ledger.balance(&account), 9.0);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn external_qi_transfer_same_account_preserves_invalid_amount_errors() {
    for amount in [-1.0, f64::NAN] {
        let mut ledger = WorldQiAccount::default();
        let account = QiAccountId::overflow("same-account-invalid");
        let error = transfer_external_qi_to_ledger(
            &mut ledger,
            account.clone(),
            account,
            amount,
            QiTransferReason::ReleaseToZone,
        )
        .expect_err("invalid amount validation must precede positive same-account rejection");
        assert!(matches!(error, QiPhysicsError::InvalidAmount { .. }));
        assert!(ledger.iter_balances().next().is_none());
        assert!(ledger.transfers().is_empty());
    }
}

#[test]
fn external_qi_transfer_rejects_same_account_before_shadow_mutation() {
    let mut ledger = WorldQiAccount::default();
    let account = QiAccountId::overflow("same-account");
    ledger
        .set_balance(account.clone(), 9.0)
        .expect("fixture balance should be valid");

    let error = transfer_external_qi_to_ledger(
        &mut ledger,
        account.clone(),
        account.clone(),
        3.0,
        QiTransferReason::ReleaseToZone,
    )
    .expect_err("same-account external transfer must fail closed");

    assert!(matches!(
        error,
        QiPhysicsError::SameAccountTransfer { account: ref id }
            if id == "overflow:same-account"
    ));
    assert_eq!(ledger.balance(&account), 9.0);
    assert!(ledger.transfers().is_empty());
}

#[test]
fn credit_pending_inflow_zero_amount_is_noop() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("player_a");

    credit_pending_inflow(
        &mut ledger,
        "spawn",
        from,
        0.0,
        QiTransferReason::MeridianOpen,
    )
    .expect("zero amount must be a no-op, not an error");

    assert!(
        !ledger.has_account(&pending_inflow_account()),
        "zero-amount credit must not create the pending inflow account"
    );
    assert!(
        ledger.transfers().is_empty(),
        "zero-amount credit must not append a transfer audit"
    );
}

#[test]
fn credit_pending_inflow_rejects_negative_amount() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("player_a");

    let err = credit_pending_inflow(
        &mut ledger,
        "spawn",
        from,
        -1.0,
        QiTransferReason::MeridianOpen,
    )
    .expect_err("negative amount must be rejected");

    assert!(
        matches!(
            err,
            QiPhysicsError::InvalidAmount {
                field: "transfer.amount",
                value: -1.0,
            }
        ),
        "negative credit must surface transfer.amount InvalidAmount, matching the \
             pre-existing credit_active_breakthrough_cost boundary contract; got {err:?}"
    );
    assert!(
        !ledger.has_account(&pending_inflow_account()),
        "rejected negative credit must not create the pending inflow account"
    );
    assert!(ledger.transfers().is_empty());
}

#[test]
fn credit_pending_inflow_rejects_non_finite_amount() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("player_a");

    let err = credit_pending_inflow(
        &mut ledger,
        "spawn",
        from,
        f64::NAN,
        QiTransferReason::MeridianOpen,
    )
    .expect_err("NaN amount must be rejected");

    assert!(matches!(
        err,
        QiPhysicsError::InvalidAmount {
            field: "transfer.amount",
            ..
        }
    ));
}

#[test]
fn credit_pending_inflow_credits_pool_and_leaves_audit_trail() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("player_a");

    credit_pending_inflow(
        &mut ledger,
        "spawn",
        from.clone(),
        8.0,
        QiTransferReason::Breakthrough,
    )
    .expect("positive credit should succeed");

    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        8.0,
        "pending inflow pool must rise by the credited amount"
    );
    assert_eq!(
        ledger.balance(&from),
        0.0,
        "the audit-only actor account must not retain a residual ledger shadow balance \
             after the transfer (real qi lives in ECS Cultivation.qi_current, not here)"
    );
    let transfer = ledger
        .transfers()
        .last()
        .expect("credit should append exactly one transfer audit");
    assert_eq!(transfer.from, from);
    assert_eq!(transfer.to, pending_inflow_account());
    assert_eq!(transfer.amount, 8.0);
    assert_eq!(transfer.reason, QiTransferReason::Breakthrough);
}

#[test]
fn credit_pending_inflow_accumulates_across_repeated_calls_without_leakage() {
    let mut ledger = WorldQiAccount::default();

    credit_pending_inflow(
        &mut ledger,
        "spawn",
        QiAccountId::player("player_a"),
        5.0,
        QiTransferReason::MeridianOpen,
    )
    .expect("first credit should succeed");
    credit_pending_inflow(
        &mut ledger,
        "spawn",
        QiAccountId::npc("dormant:rogue:1"),
        3.0,
        QiTransferReason::Breakthrough,
    )
    .expect("second credit from a different actor should succeed");
    credit_pending_inflow(
        &mut ledger,
        "spawn",
        QiAccountId::player("player_a"),
        2.0,
        QiTransferReason::MeridianOpen,
    )
    .expect("repeated credit from the same actor must not leak or double-count");

    assert_eq!(
        ledger.balance(&pending_inflow_account()),
        10.0,
        "pool balance must equal the exact sum of all credited amounts (5+3+2), \
             proving repeated same-actor credits neither leak into nor double-count \
             against the shared pending pool"
    );
    assert_eq!(ledger.transfers().len(), 3);
}

#[test]
fn credit_pending_inflow_never_clobbers_a_same_named_zone_ledger_account() {
    // 回归锁：旧 bug 是 credit_meridian_open_cost 把消耗写进 `zone:<name>` 账户，
    // 而该 key 会被 apply_dormant_regen_with_multiplier 按
    // `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY` 整体覆写，credit 的钱被静默清零。
    // 待分配池必须是独立 key，不与任何 zone 账户碰撞。
    let mut ledger = WorldQiAccount::default();
    let zone_account = QiAccountId::zone("spawn");
    ledger
        .set_balance(zone_account.clone(), 999.0)
        .expect("seed a real zone ledger balance to prove no collision");

    credit_pending_inflow(
        &mut ledger,
        "spawn",
        QiAccountId::player("player_a"),
        8.0,
        QiTransferReason::MeridianOpen,
    )
    .expect("credit should succeed");

    assert_eq!(
        ledger.balance(&zone_account),
        999.0,
        "crediting the pending pool must not touch the same-named zone:<name> ledger \
             account that other systems (dormant regen) overwrite wholesale"
    );
    assert_eq!(ledger.balance(&pending_inflow_account()), 8.0);
}

#[test]
fn transfer_moves_qi_between_accounts() {
    let from = QiAccountId::player("a");
    let to = QiAccountId::zone("spawn");
    let mut account = WorldQiAccount::default();
    account.set_balance(from.clone(), 10.0).unwrap();
    account.set_balance(to.clone(), 1.0).unwrap();
    account
        .transfer(
            QiTransfer::new(
                from.clone(),
                to.clone(),
                3.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(account.balance(&from), 7.0);
    assert_eq!(account.balance(&to), 4.0);
}

#[test]
fn transfer_rejects_same_account_before_any_mutation() {
    let account_id = QiAccountId::overflow("same-account");
    let mut account = WorldQiAccount::default();
    account
        .set_balance(account_id.clone(), 9.0)
        .expect("fixture balance should be valid");
    let before_accounts = account
        .iter_balances()
        .map(|(id, balance)| (id.clone(), balance))
        .collect::<Vec<_>>();

    let error = account
        .transfer(
            QiTransfer::new(
                account_id.clone(),
                account_id.clone(),
                3.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap(),
        )
        .expect_err("same-account direct transfer must fail closed");

    assert!(matches!(
        error,
        QiPhysicsError::SameAccountTransfer { account: ref id }
            if id == "overflow:same-account"
    ));
    assert_eq!(
        account
            .iter_balances()
            .map(|(id, balance)| (id.clone(), balance))
            .collect::<Vec<_>>(),
        before_accounts,
        "same-account rejection must preserve every balance"
    );
    assert_eq!(account.total(), 9.0);
    assert!(account.transfers().is_empty());
}

#[test]
fn transfer_rejects_same_account_zero_after_amount_validation() {
    let account_id = QiAccountId::overflow("same-account-zero");
    let mut account = WorldQiAccount::default();
    account
        .set_balance(account_id.clone(), 9.0)
        .expect("fixture balance should be valid");

    let before_accounts = account
        .iter_balances()
        .map(|(id, balance)| (id.clone(), balance))
        .collect::<Vec<_>>();
    let error = account
        .transfer(
            QiTransfer::new(
                account_id.clone(),
                account_id,
                0.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap(),
        )
        .expect_err("direct zero same-account transfer must not silently succeed");

    assert!(matches!(error, QiPhysicsError::SameAccountTransfer { .. }));
    assert_eq!(
        account
            .iter_balances()
            .map(|(id, balance)| (id.clone(), balance))
            .collect::<Vec<_>>(),
        before_accounts,
        "zero same-account rejection must preserve every balance"
    );
    assert_eq!(account.total(), 9.0);
    assert!(account.transfers().is_empty());
}

#[test]
fn transfer_same_account_invalid_amount_still_reports_invalid_amount() {
    let account_id = QiAccountId::overflow("same-account-invalid");
    let mut account = WorldQiAccount::default();
    let error = account
        .transfer(QiTransfer {
            from: account_id.clone(),
            to: account_id,
            amount: f64::NAN,
            reason: QiTransferReason::ReleaseToZone,
        })
        .expect_err("invalid amount validation must precede same-account rejection");

    assert!(matches!(error, QiPhysicsError::InvalidAmount { .. }));
    assert!(account.iter_balances().next().is_none());
    assert!(account.transfers().is_empty());
}

#[test]
fn transfer_rejects_destination_credit_that_cannot_advance() {
    let from = QiAccountId::player("a");
    let to = QiAccountId::overflow("saturated");
    let mut account = WorldQiAccount::default();
    account.set_balance(from.clone(), 1.0).unwrap();
    account.set_balance(to.clone(), f64::MAX).unwrap();

    let error = account
        .transfer(
            QiTransfer::new(
                from.clone(),
                to.clone(),
                1.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap(),
        )
        .expect_err("a positive credit that leaves the destination unchanged must fail closed");

    assert!(matches!(
        error,
        QiPhysicsError::InvalidAmount {
            field: "destination_balance",
            ..
        }
    ));
    assert_eq!(account.balance(&from), 1.0);
    assert_eq!(account.balance(&to), f64::MAX);
    assert!(account.transfers().is_empty());
}

#[test]
fn transfer_rejects_source_debit_that_cannot_advance() {
    let from = QiAccountId::player("saturated");
    let to = QiAccountId::zone("spawn");
    let mut account = WorldQiAccount::default();
    account.set_balance(from.clone(), f64::MAX).unwrap();

    let error = account
        .transfer(
            QiTransfer::new(
                from.clone(),
                to.clone(),
                1.0,
                QiTransferReason::ReleaseToZone,
            )
            .unwrap(),
        )
        .expect_err("a positive debit that leaves the source unchanged must fail closed");

    assert!(matches!(
        error,
        QiPhysicsError::InvalidAmount {
            field: "source_balance",
            ..
        }
    ));
    assert_eq!(account.balance(&from), f64::MAX);
    assert_eq!(account.balance(&to), 0.0);
    assert!(account.transfers().is_empty());
}

#[test]
fn transfer_rejects_overdraft() {
    let from = QiAccountId::player("a");
    let to = QiAccountId::zone("spawn");
    let mut account = WorldQiAccount::default();
    account.set_balance(from.clone(), 1.0).unwrap();
    let err = account
        .transfer(QiTransfer::new(from, to, 3.0, QiTransferReason::ReleaseToZone).unwrap())
        .expect_err("overdraft should fail");
    assert!(matches!(err, QiPhysicsError::InsufficientQi { .. }));
}

#[test]
fn transfer_rejects_halfstep_buff_audit_only_reason() {
    // plan-halfstep-buff-v1 P1：HalfStepBuff 是 audit-only 标记，绝不可走变动 balance 的 transfer 路径。
    // 调用方应直接 emit `EventWriter<QiTransfer>` event，不走 WorldQiAccount::transfer。
    let from = QiAccountId::tiandao();
    let to = QiAccountId::player("alice");
    let mut account = WorldQiAccount::default();
    account.set_balance(from.clone(), 100.0).unwrap();
    let initial_from_balance = account.balance(&from);
    let initial_to_balance = account.balance(&to);
    let transfer = QiTransfer::new(
        from.clone(),
        to.clone(),
        10.0,
        QiTransferReason::HalfStepBuff,
    )
    .expect("QiTransfer::new must accept HalfStepBuff reason (event-level allowed)");
    let err = account.transfer(transfer).expect_err(
        "WorldQiAccount::transfer 必须拒绝 HalfStepBuff reason；audit-only 误用会破守恒律",
    );
    assert!(
        matches!(err, QiPhysicsError::AuditOnlyReason { reason } if reason == "HalfStepBuff"),
        "expected AuditOnlyReason::HalfStepBuff, got {err:?}"
    );
    // balance 必须未变动
    assert_eq!(
        account.balance(&from),
        initial_from_balance,
        "拒绝后 from balance 必须保持不变"
    );
    assert_eq!(
        account.balance(&to),
        initial_to_balance,
        "拒绝后 to balance 必须保持不变"
    );
    // transfers 记录也不应增加
    assert!(
        account.transfers().is_empty(),
        "拒绝的 transfer 不应留下 audit trail；防止统计被误污染"
    );
}

/// plan-qi-conservation-leaks-v1 P4 / bughunt r8 / bughunt QS-01 — F24 加固：
/// DuguReturnToZone / DuguReverseVictimQi / NegPressureDrain 三个 audit-only reason
/// 此前只在 doc-comment 里约定"禁止调 transfer"，没有编译期/运行期防护。
/// 每个变体各一条 case，断言 transfer() 拒绝 + balance 完全不变 + 不留 audit trail。
#[test]
fn transfer_rejects_all_audit_only_reasons_with_balance_untouched() {
    let cases = [
        (QiTransferReason::DuguReturnToZone, "DuguReturnToZone"),
        (QiTransferReason::DuguReverseVictimQi, "DuguReverseVictimQi"),
        (QiTransferReason::NegPressureDrain, "NegPressureDrain"),
    ];
    for (reason, label) in cases {
        let from = QiAccountId::player("caster");
        let to = QiAccountId::zone("spawn");
        let mut account = WorldQiAccount::default();
        account.set_balance(from.clone(), 50.0).unwrap();
        account.set_balance(to.clone(), 5.0).unwrap();

        let transfer =
            QiTransfer::new(from.clone(), to.clone(), 10.0, reason).unwrap_or_else(|e| {
                panic!("QiTransfer::new must accept {label} at event level: {e:?}")
            });
        let err = account.transfer(transfer).expect_err(&format!(
            "expected {label} to be rejected by WorldQiAccount::transfer, but it succeeded"
        ));
        assert!(
            matches!(err, QiPhysicsError::AuditOnlyReason { reason } if reason == label),
            "expected AuditOnlyReason::{label}, got {err:?}"
        );
        assert_eq!(
            account.balance(&from),
            50.0,
            "{label}: rejected transfer must leave `from` balance untouched (still 50.0), got {}",
            account.balance(&from)
        );
        assert_eq!(
            account.balance(&to),
            5.0,
            "{label}: rejected transfer must leave `to` balance untouched (still 5.0), got {}",
            account.balance(&to)
        );
        assert!(
            account.transfers().is_empty(),
            "{label}: rejected transfer must not append to the audit trail"
        );
    }
}

/// F24 加固不能误伤现有调用方：它们都走 `push_transfer_audit`（audit-only 记录，
/// 不touch balance），必须继续对这三个 reason 正常工作——否则真实调用方（
/// tsy_drain.rs / dugu_v2/tick.rs / cultivation/neg_pressure.rs 等）会被这次加固破坏。
#[test]
fn push_transfer_audit_still_works_for_hardened_reasons() {
    let cases = [
        QiTransferReason::DuguReturnToZone,
        QiTransferReason::DuguReverseVictimQi,
        QiTransferReason::NegPressureDrain,
        QiTransferReason::HalfStepBuff,
    ];
    let mut account = WorldQiAccount::default();
    // 故意不给 from 设置 balance —— push_transfer_audit 不检查余额，
    // 这本身就是与 transfer() 的关键行为差异（audit-only 不受限于 InsufficientQi）。
    let from = QiAccountId::player("caster");
    let to = QiAccountId::zone("spawn");
    for reason in cases {
        let transfer = QiTransfer::new(from.clone(), to.clone(), 10.0, reason)
            .expect("QiTransfer::new must accept audit-only reasons");
        account.push_transfer_audit(transfer.clone());
        assert_eq!(
                account.transfers().last(),
                Some(&transfer),
                "push_transfer_audit must append the transfer verbatim to the audit trail for {reason:?}"
            );
    }
    assert_eq!(
        account.transfers().len(),
        cases.len(),
        "expected exactly {} audit entries (one per hardened reason), no rejections/drops",
        cases.len()
    );
    assert_eq!(
        account.total(),
        0.0,
        "push_transfer_audit must never mutate any account balance"
    );
}

#[test]
fn transfer_rejects_epsilon_sized_overdraft() {
    let from = QiAccountId::player("a");
    let to = QiAccountId::zone("spawn");
    let mut account = WorldQiAccount::default();
    account.set_balance(from.clone(), 0.0).unwrap();
    account.set_balance(to.clone(), 0.0).unwrap();

    let err = account
        .transfer(
            QiTransfer::new(from, to, QI_EPSILON * 0.5, QiTransferReason::ReleaseToZone).unwrap(),
        )
        .expect_err("tiny positive overdraft should fail");
    assert!(matches!(err, QiPhysicsError::InsufficientQi { .. }));
}

#[test]
fn repeated_transfers_preserve_total() {
    let from = QiAccountId::player("a");
    let to = QiAccountId::zone("spawn");
    let mut account = WorldQiAccount::default();
    account.set_balance(from.clone(), 100.0).unwrap();
    account.set_balance(to.clone(), 0.0).unwrap();
    for _ in 0..100 {
        account
            .transfer(
                QiTransfer::new(from.clone(), to.clone(), 0.5, QiTransferReason::Channeling)
                    .unwrap(),
            )
            .unwrap();
    }
    assert!((account.total() - 100.0).abs() < QI_EPSILON);
}

#[test]
fn conservation_accepts_era_decay() {
    let before = snapshot(100.0);
    let after = snapshot(97.0);
    assert!(assert_conservation(&before, &after, 3.0).is_ok());
}

#[test]
fn conservation_rejects_drift() {
    let before = snapshot(100.0);
    let after = snapshot(90.0);
    let err = assert_conservation(&before, &after, 3.0).expect_err("drift should fail");
    assert!(matches!(err, QiPhysicsError::ConservationDrift { .. }));
}

#[test]
fn conservation_accepts_preserved_negative_observed_total() {
    let before = snapshot(-0.6);
    let after = snapshot(-0.6);
    assert!(assert_conservation(&before, &after, 0.0).is_ok());
}

#[test]
fn snapshot_for_ipc_keeps_budget_and_observed_total() {
    let snap = WorldQiSnapshot {
        player_qi: 1.0,
        zone_qi: 2.0,
        container_qi: 3.0,
        ledger_qi: 4.0,
        era_decay_accum: 5.0,
        budget_initial_total: 100.0,
        budget_current_total: 95.0,
    };
    let ipc = snapshot_for_ipc(&snap);
    assert_eq!(ipc.observed_total, 10.0);
    assert_eq!(ipc.budget_current_total, 95.0);
    assert_eq!(ipc.era_decay_accum, 5.0);
}

#[test]
fn summarize_world_qi_reads_budget_zones_players_and_inventory() {
    let mut app = App::new();
    app.insert_resource(WorldQiBudget::from_total(50.0));
    let mut zones = ZoneRegistry::fallback();
    zones.zones[0].spirit_qi = 0.5;
    app.insert_resource(zones);
    app.world_mut().spawn(Cultivation {
        qi_current: 7.0,
        ..Default::default()
    });
    app.world_mut().spawn(inventory_with_item(0.8, 2));

    let snap = summarize_world_qi(app.world_mut());
    assert_eq!(snap.budget_current_total, 50.0);
    assert_eq!(snap.zone_qi, 0.5 * QI_ZONE_UNIT_CAPACITY);
    assert_eq!(snap.player_qi, 7.0);
    assert_eq!(snap.container_qi, 1.6);
}

#[test]
fn summarize_world_qi_preserves_negative_zone_qi() {
    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.zones[0].spirit_qi = -0.6;
    app.insert_resource(zones);

    let snap = summarize_world_qi(app.world_mut());
    assert_eq!(snap.zone_qi, -0.6 * QI_ZONE_UNIT_CAPACITY);
    assert_eq!(snap.total_observed(), -0.6 * QI_ZONE_UNIT_CAPACITY);
}

#[test]
fn world_qi_snapshot_asserts_conservation_across_ledger_transfer() {
    let mut app = App::new();
    app.insert_resource(WorldQiBudget::from_total(100.0));
    let mut account = WorldQiAccount::default();
    let from = QiAccountId::player("p1");
    let to = QiAccountId::zone("spawn");
    account.set_balance(from.clone(), 10.0).unwrap();
    account.set_balance(to.clone(), 5.0).unwrap();
    app.insert_resource(account);
    let before = summarize_world_qi(app.world_mut());

    app.world_mut()
        .resource_mut::<WorldQiAccount>()
        .transfer(QiTransfer::new(from, to, 3.0, QiTransferReason::ReleaseToZone).unwrap())
        .unwrap();
    let after = summarize_world_qi(app.world_mut());

    assert_eq!(before.budget_current_total, 100.0);
    assert_eq!(after.budget_current_total, 100.0);
    assert!(assert_conservation(&before, &after, 0.0).is_ok());
}

// ── plan-offscreen-war-v1 P0：bong:qi/ledger telemetry builder ────────

fn field<'a>(fields: &'a [(String, String)], name: &str) -> &'a str {
    fields
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
        .unwrap_or_else(|| panic!("expected ledger HASH to carry field {name:?}, got {fields:?}"))
}

#[test]
fn qi_ledger_hash_fields_carry_conservation_aggregates() {
    // 顶层守恒分量必须可被外部脚本精确读出：total_observed == player+zone+container+ledger。
    let snap = WorldQiSnapshot {
        player_qi: 40.0,
        zone_qi: 35.0,
        container_qi: 15.0,
        ledger_qi: 10.0,
        era_decay_accum: 0.0,
        budget_initial_total: DEFAULT_SPIRIT_QI_TOTAL,
        budget_current_total: DEFAULT_SPIRIT_QI_TOTAL,
    };
    let accounts = WorldQiAccount::default();
    let fields = build_qi_ledger_hash_fields(&snap, &accounts);

    assert_eq!(
        field(&fields, "total_observed"),
        "100",
        "total_observed 必须等于四个分量之和（40+35+15+10），外部 e2e 据此做精确守恒断言"
    );
    assert_eq!(field(&fields, "player_qi"), "40");
    assert_eq!(field(&fields, "zone_qi"), "35");
    assert_eq!(field(&fields, "container_qi"), "15");
    assert_eq!(field(&fields, "ledger_qi"), "10");
    assert_eq!(
        field(&fields, "budget_initial_total"),
        DEFAULT_SPIRIT_QI_TOTAL.to_string(),
        "预算字段取 const，不写字面 100"
    );
    assert_eq!(
        field(&fields, "budget_current_total"),
        DEFAULT_SPIRIT_QI_TOTAL.to_string()
    );
    assert_eq!(field(&fields, "era_decay_accum"), "0");
}

#[test]
fn qi_ledger_hash_fields_emit_one_row_per_account() {
    // per-zone / per-npc 账户余额各占一行 account:<id>，外部脚本可逐账户对账。
    let mut accounts = WorldQiAccount::default();
    accounts
        .set_balance(QiAccountId::zone("spawn"), 25.0)
        .unwrap();
    accounts
        .set_balance(QiAccountId::npc("dormant:rogue:7"), 3.5)
        .unwrap();
    let fields = build_qi_ledger_hash_fields(&snapshot(0.0), &accounts);

    assert_eq!(
        field(&fields, "account:zone:spawn"),
        "25",
        "zone 账户余额必须以 account:zone:spawn 行暴露（kind 为稳定 lowercase wire 串）"
    );
    assert_eq!(
        field(&fields, "account:npc:dormant:rogue:7"),
        "3.5",
        "npc 账户余额必须以 account:npc:<char_id> 行暴露，供 P2 战死还灵气对账"
    );

    let account_rows = fields
        .iter()
        .filter(|(key, _)| key.starts_with(QI_LEDGER_ACCOUNT_FIELD_PREFIX))
        .count();
    assert_eq!(
        account_rows, 2,
        "两个账户应产出两行 account: 字段，不多不少"
    );
}

#[test]
fn qi_account_id_display_uses_stable_lowercase_wire_strings() {
    // wire 契约 pin：`bong:qi/ledger` 的 `account:<kind>:<id>` key 形如 `zone:spawn`。
    // 任一变体的 Display 串改动都会撞红——防止 Debug rename 静默漂移外部 schema。
    let cases: [(QiAccountId, &str); 7] = [
        (QiAccountId::player("alice"), "player:alice"),
        (QiAccountId::npc("dormant:rogue:7"), "npc:dormant:rogue:7"),
        (QiAccountId::zone("spawn"), "zone:spawn"),
        (QiAccountId::container("chest:3"), "container:chest:3"),
        (QiAccountId::rift("rift:north"), "rift:rift:north"),
        (QiAccountId::tiandao(), "tiandao:tiandao"),
        (QiAccountId::overflow("sink"), "overflow:sink"),
    ];
    for (account, expected) in cases {
        assert_eq!(
            account.to_string(),
            expected,
            "QiAccountId Display 必须输出稳定 lowercase wire 串 `{expected}`，\
                 因为外部 Redis schema 据此 diff；若此处红了说明改动了 wire 契约（须有意为之）"
        );
    }
}

#[test]
fn qi_ledger_hash_fields_empty_ledger_has_only_aggregates() {
    // 起服后 ledger 尚未记账：无 account: 行，但 total_observed 已可读（来自 zone/player 快照）。
    let fields = build_qi_ledger_hash_fields(&snapshot(100.0), &WorldQiAccount::default());
    assert!(
        fields
            .iter()
            .all(|(key, _)| !key.starts_with(QI_LEDGER_ACCOUNT_FIELD_PREFIX)),
        "空账本不应有 account: 行"
    );
    assert_eq!(
        field(&fields, "total_observed"),
        "100",
        "空账本下 total_observed 仍来自 player/zone 快照，起服后应 ≈ DEFAULT_SPIRIT_QI_TOTAL"
    );
}

fn snapshot(total: f64) -> WorldQiSnapshot {
    WorldQiSnapshot {
        player_qi: total,
        zone_qi: 0.0,
        container_qi: 0.0,
        ledger_qi: 0.0,
        era_decay_accum: 0.0,
        budget_initial_total: total,
        budget_current_total: total,
    }
}

fn inventory_with_item(spirit_quality: f64, stack_count: u32) -> PlayerInventory {
    PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: "main_pack".to_string(),
            name: "main".to_string(),
            rows: 1,
            cols: 1,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: ItemInstance {
                    instance_id: 1,
                    template_id: "bone_coin".to_string(),
                    display_name: "bone coin".to_string(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 1.0,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count,
                    spirit_quality,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            }],

            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

// ── plan-qi-handling-attrition-v1 P0 — AttritionTax 变体 pin 测试 ───────

#[test]
fn attrition_tax_variant_neq_half_step_buff() {
    // AttritionTax 与 HalfStepBuff 是独立变体，语义不同（税 vs 容量扩张）
    let attrition = QiTransferReason::AttritionTax {
        op_kind: AttritionOpKind::Pickup,
    };
    let halfstep = QiTransferReason::HalfStepBuff;
    assert_ne!(
        attrition, halfstep,
        "AttritionTax 与 HalfStepBuff 应为不同变体"
    );
}

#[test]
fn attrition_tax_variant_neq_release_to_zone() {
    // AttritionTax 与 ReleaseToZone 是独立变体，审计轨迹不应混淆
    let attrition = QiTransferReason::AttritionTax {
        op_kind: AttritionOpKind::Pickup,
    };
    let release = QiTransferReason::ReleaseToZone;
    assert_ne!(
        attrition, release,
        "AttritionTax 与 ReleaseToZone 应为不同变体，否则审计轨迹混淆"
    );
}

#[test]
fn attrition_tax_op_kind_all_five_distinct() {
    // 5 个 AttritionOpKind 变体应各自独立，pin 其相等性
    use AttritionOpKind::*;
    let variants = [Pickup, SlotMove, ContainerSearch, ForgeLoad, AlchemyLoad];
    for i in 0..variants.len() {
        for j in 0..variants.len() {
            if i == j {
                assert_eq!(variants[i], variants[j], "同一变体应相等");
            } else {
                assert_ne!(
                    variants[i], variants[j],
                    "{:?} 与 {:?} 应为不同变体",
                    variants[i], variants[j]
                );
            }
        }
    }
}

#[test]
fn attrition_tax_is_not_audit_only_in_transfer() {
    // AttritionTax 不是 audit-only，应可以走 emit QiTransfer::new（不被拒绝）
    let from = QiAccountId::container("item:123");
    let to = QiAccountId::zone("spawn");
    let result = QiTransfer::new(
        from,
        to,
        3.0,
        QiTransferReason::AttritionTax {
            op_kind: AttritionOpKind::Pickup,
        },
    );
    assert!(
        result.is_ok(),
        "AttritionTax QiTransfer::new 不应失败，实际 {:?}",
        result.err()
    );
}

// ── plan-qi-conservation-leaks-v1 P2 — ArtifactMaintenance/ArtifactEvolution pin 测试 ──

#[test]
fn artifact_maintenance_reason_is_distinct_from_other_variants() {
    // pin 测试：ArtifactMaintenance 与其他变体互不相等，审计轨迹不混淆。
    let m = QiTransferReason::ArtifactMaintenance;
    let e = QiTransferReason::ArtifactEvolution;
    let c = QiTransferReason::Crafting;
    let b = QiTransferReason::BossDrain;
    let r = QiTransferReason::ReleaseToZone;

    assert_ne!(
        m, e,
        "ArtifactMaintenance 与 ArtifactEvolution 应为不同变体"
    );
    assert_ne!(m, c, "ArtifactMaintenance 与 Crafting 应为不同变体");
    assert_ne!(m, b, "ArtifactMaintenance 与 BossDrain 应为不同变体");
    assert_ne!(m, r, "ArtifactMaintenance 与 ReleaseToZone 应为不同变体");
    assert_eq!(m, m, "ArtifactMaintenance 与自身应相等");
}

#[test]
fn artifact_evolution_reason_is_distinct_from_other_variants() {
    // pin 测试：ArtifactEvolution 与其他变体互不相等，审计轨迹不混淆。
    let e = QiTransferReason::ArtifactEvolution;
    let m = QiTransferReason::ArtifactMaintenance;
    let c = QiTransferReason::Crafting;
    let b = QiTransferReason::BossDrain;
    let r = QiTransferReason::ReleaseToZone;

    assert_ne!(
        e, m,
        "ArtifactEvolution 与 ArtifactMaintenance 应为不同变体"
    );
    assert_ne!(e, c, "ArtifactEvolution 与 Crafting 应为不同变体");
    assert_ne!(e, b, "ArtifactEvolution 与 BossDrain 应为不同变体");
    assert_ne!(e, r, "ArtifactEvolution 与 ReleaseToZone 应为不同变体");
    assert_eq!(e, e, "ArtifactEvolution 与自身应相等");
}

#[test]
fn artifact_maintenance_and_evolution_not_audit_only() {
    // 两个新变体均可通过 QiTransfer::new 构建（非 audit-only）。
    let from = QiAccountId::player("entity:1");
    let to = QiAccountId::zone("spawn");

    let maint = QiTransfer::new(
        from.clone(),
        to.clone(),
        2.0,
        QiTransferReason::ArtifactMaintenance,
    );
    assert!(
        maint.is_ok(),
        "ArtifactMaintenance QiTransfer::new 不应失败，实际 {:?}",
        maint.err()
    );

    let evo = QiTransfer::new(from, to, 30.0, QiTransferReason::ArtifactEvolution);
    assert!(
        evo.is_ok(),
        "ArtifactEvolution QiTransfer::new 不应失败，实际 {:?}",
        evo.err()
    );
}

// ── plan-zone-qi-economy-v1 P1 §8.1 决议 #5 — ZoneInflow 变体 pin 测试 ──

#[test]
fn zone_inflow_reason_is_distinct_from_other_variants() {
    let inflow = QiTransferReason::ZoneInflow;
    assert_ne!(inflow, QiTransferReason::MeridianOpen);
    assert_ne!(inflow, QiTransferReason::Breakthrough);
    assert_ne!(inflow, QiTransferReason::ReleaseToZone);
    assert_ne!(inflow, QiTransferReason::CultivationRegen);
    assert_eq!(
        inflow,
        QiTransferReason::ZoneInflow,
        "ZoneInflow 与自身应相等"
    );
}

#[test]
fn zone_inflow_is_not_audit_only_and_actually_moves_balance() {
    // ZoneInflow 是真实 WorldQiAccount::transfer（非 audit-only）：待分配池 -> zone
    // 镜像账户之间必须发生真实的余额搬运，而不是像 HalfStepBuff/NegPressureDrain 那样
    // 被 transfer() 拒绝、只能走 push_transfer_audit。
    let pool = pending_inflow_account();
    let zone_account = QiAccountId::zone("spawn");

    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pool.clone(), 10.0)
        .expect("seeding pool balance must succeed");
    ledger
        .set_balance(zone_account.clone(), 5.0)
        .expect("seeding zone mirror balance must succeed");

    let transfer = QiTransfer::new(
        pool.clone(),
        zone_account.clone(),
        3.0,
        QiTransferReason::ZoneInflow,
    )
    .expect("ZoneInflow QiTransfer::new must succeed (not audit-only)");
    ledger.transfer(transfer).expect(
        "ZoneInflow must be accepted by WorldQiAccount::transfer, not rejected as audit-only",
    );

    assert_eq!(
        ledger.balance(&pool),
        7.0,
        "pending pool balance must actually decrease by the transferred amount"
    );
    assert_eq!(
        ledger.balance(&zone_account),
        8.0,
        "zone mirror balance must actually increase by the transferred amount"
    );
}

#[test]
fn zone_inflow_transfer_rejects_insufficient_pool_balance() {
    // 待分配池余额不足时 transfer() 必须拒绝（InsufficientQi），而不是让池子变负数
    // ——这是"绝不透支"红线在 ledger 层的直接体现；heartbeat system 自己会先用
    // `ledger.balance(&pool)` 缩量，但 ledger 本身的兜底检查也必须存在。
    let pool = pending_inflow_account();
    let zone_account = QiAccountId::zone("spawn");

    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pool.clone(), 1.0)
        .expect("seeding pool balance must succeed");
    ledger
        .set_balance(zone_account.clone(), 0.0)
        .expect("seeding zone mirror balance must succeed");

    let transfer = QiTransfer::new(
        pool.clone(),
        zone_account.clone(),
        5.0,
        QiTransferReason::ZoneInflow,
    )
    .expect("QiTransfer::new only validates amount shape, not balance sufficiency");
    let err = ledger
        .transfer(transfer)
        .expect_err("transferring more than the pool holds must be rejected, never overdraw");
    assert!(
        matches!(err, QiPhysicsError::InsufficientQi { .. }),
        "expected InsufficientQi, got {err:?}"
    );
    assert_eq!(
        ledger.balance(&pool),
        1.0,
        "a rejected transfer must leave the pool balance completely untouched"
    );
}

// ── plan-zone-qi-economy-v1 P3 §8.1 决议 #3 — PseudoVeinSettle 变体 pin 测试 ──

#[test]
fn pseudo_vein_settle_reason_is_distinct_from_other_variants() {
    let settle = QiTransferReason::PseudoVeinSettle;
    assert_ne!(settle, QiTransferReason::ZoneInflow);
    assert_ne!(settle, QiTransferReason::ReleaseToZone);
    assert_ne!(settle, QiTransferReason::EraDecay);
    assert_eq!(
        settle,
        QiTransferReason::PseudoVeinSettle,
        "PseudoVeinSettle 与自身应相等"
    );
}

#[test]
fn pseudo_vein_settle_is_not_audit_only_and_actually_moves_balance() {
    // PseudoVeinSettle 是真实 WorldQiAccount::transfer（非 audit-only）：zone 镜像账户 ->
    // 待分配池之间必须发生真实的余额搬运（借款归还），而不是像 HalfStepBuff 那样被拒。
    let pool = pending_inflow_account();
    let zone_account = QiAccountId::zone("lingquan_marsh");

    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pool.clone(), 10.0)
        .expect("seeding pool balance must succeed");
    ledger
        .set_balance(zone_account.clone(), 8.0)
        .expect("seeding zone mirror balance must succeed");

    let transfer = QiTransfer::new(
        zone_account.clone(),
        pool.clone(),
        8.0,
        QiTransferReason::PseudoVeinSettle,
    )
    .expect("PseudoVeinSettle QiTransfer::new must succeed (not audit-only)");
    ledger.transfer(transfer).expect(
        "PseudoVeinSettle must be accepted by WorldQiAccount::transfer, not rejected as \
             audit-only",
    );

    assert_eq!(
        ledger.balance(&zone_account),
        0.0,
        "zone mirror balance must actually decrease by the repaid amount (full repayment, \
             not the old 30% partial collection)"
    );
    assert_eq!(
        ledger.balance(&pool),
        18.0,
        "pending pool balance must actually increase by the repaid amount"
    );
}

#[test]
fn pseudo_vein_settle_transfer_rejects_repaying_more_than_zone_holds() {
    // zone 镜像余额不足以覆盖借款额时（例如借款期间被玩家/NPC 正常吸收殆尽）transfer()
    // 必须拒绝，而不是让 zone 变负——调用方（apply_pseudo_vein_settlement）必须先用
    // `min(injected_qi, zone 当前绝对余额)` 缩量，但 ledger 本身的兜底检查也必须存在。
    let pool = pending_inflow_account();
    let zone_account = QiAccountId::zone("lingquan_marsh");

    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pool.clone(), 0.0)
        .expect("seeding pool balance must succeed");
    ledger
        .set_balance(zone_account.clone(), 2.0)
        .expect("seeding zone mirror balance must succeed");

    let transfer = QiTransfer::new(
        zone_account.clone(),
        pool.clone(),
        8.0,
        QiTransferReason::PseudoVeinSettle,
    )
    .expect("QiTransfer::new only validates amount shape, not balance sufficiency");
    let err = ledger
        .transfer(transfer)
        .expect_err("repaying more than the zone holds must be rejected, never go negative");
    assert!(
        matches!(err, QiPhysicsError::InsufficientQi { .. }),
        "expected InsufficientQi, got {err:?}"
    );
    assert_eq!(
        ledger.balance(&zone_account),
        2.0,
        "a rejected transfer must leave the zone mirror balance completely untouched"
    );
}

// ── plan-layered-equip-v1 P3 — equipped 真元 carrier 守恒求和 pin ──

fn qi_item(instance_id: u64, spirit_quality: f64, stack_count: u32) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: format!("carrier_{instance_id}"),
        display_name: "carrier".to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 1.0,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count,
        spirit_quality,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

fn bare_inventory() -> PlayerInventory {
    PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: Vec::new(),
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

#[test]
fn inventory_qi_sums_two_carriers_in_same_worn_stack() {
    // P3 守恒 pin：同槽 worn 两件 carrier（各带真元）→ inventory_qi = 两件 item_qi 之和。
    // 分层前 .values().map(item_qi) 只会取 SlotContents（编译红或丢件）；
    // 分层后 flat_map(worn.chain(held)) 保证同槽多件不漏。
    let a = qi_item(1, 0.6, 1); // item_qi = 0.6 × 1 = 0.6
    let b = qi_item(2, 0.3, 2); // item_qi = 0.3 × 2 = 0.6
    let mut inv = bare_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![a.clone(), b.clone()],
            held: None,
        },
    );

    let expected = item_qi(&a) + item_qi(&b);
    assert_eq!(
        inventory_qi(&inv),
        expected,
        "同槽 worn 两件 carrier 真元应为两件 item_qi 之和 ({} + {} = {expected})，分层求和不得漏件",
        item_qi(&a),
        item_qi(&b)
    );
}

#[test]
fn inventory_qi_includes_held_carrier_after_imprint() {
    // P3 carrier 守恒 pin（blocker）：手持 carrier 充能后 inventory_qi = held carrier item_qi。
    // carrier imprint 写在手持件的 spirit_quality 上（carrier.rs 走 ledger 守恒）；
    // inventory_qi 的 held 求和（chain(s.held.iter())）必须纳入该件，否则账实不符。
    let charged = qi_item(7, 0.9, 1); // 充能后 spirit_quality=0.9 → item_qi=0.9
    let mut inv = bare_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(charged.clone()),
    );

    assert_eq!(
        inventory_qi(&inv),
        item_qi(&charged),
        "手持 carrier 充能后 inventory_qi 必须等于该 held 件 item_qi（{}），held 求和不得漏",
        item_qi(&charged)
    );
}

#[test]
fn inventory_qi_counts_both_worn_and_held_in_one_slot() {
    // worn + held 共存槽：两侧都纳入求和（worn.chain(held)）。
    // 注：身体槽 held 恒空、手槽 worn 恒空是上层校验约束；此处只验 inventory_qi 求和不漏任一侧。
    let worn_a = qi_item(1, 0.5, 1); // 0.5
    let held_b = qi_item(2, 0.4, 1); // 0.4
    let mut inv = bare_inventory();
    inv.equipped.insert(
        EQUIP_SLOT_CHEST.to_string(),
        SlotContents {
            worn: vec![worn_a.clone()],
            held: Some(held_b.clone()),
        },
    );

    let expected = item_qi(&worn_a) + item_qi(&held_b);
    assert_eq!(
        inventory_qi(&inv),
        expected,
        "槽内 worn + held 两件都应计入 inventory_qi ({expected})"
    );
}

// ───────── §P0 验收抓手 #4 —— rat_bite_and_death_cycle_preserves_world_qi_total ─────────

/// plan-ambient-ratbite-ledger-leak-v1 §P0 验收抓手 #4 / §8.1 决议 #3 —— 咬 3 次
/// （qi_steal=2，累计 drained=6）+ 鼠死亡完整链路，头尾守恒必须严格不变，且必须能撞穿
/// "写回 zone.spirit_qi 字段"这一步被漏掉时、下一次 `zone_qi_inflow_tick` 覆盖式
/// `set_balance` 二次抹掉刚转入账户余额的回归。
///
/// `summarize_world_qi` now reports Zone.spirit_qi in absolute qi units and stable-ledger →
/// Zone settlement no longer leaves a `zone:*` balance shadow. Therefore the complete owner sum
/// is player + signed zone + durable ownerless pools; including the old mirror would double count.
#[test]
fn rat_bite_and_death_cycle_preserves_world_qi_total() {
    use crate::combat::events::DeathEvent;
    use crate::combat::rat_bite::{apply_rat_bite_qi_drain, RatBiteEvent};
    use crate::cultivation::death_hooks::CultivationDeathTrigger;
    use crate::cultivation::life_record::LifeRecord;
    use crate::cultivation::tick::CultivationClock;
    use crate::fauna::rat_phase::release_drained_qi_on_death_system;
    use crate::network::audio_event_emit::PlaySoundRecipeRequest;
    use crate::network::vfx_event_emit::VfxEventRequest;
    use crate::npc::spawn::NpcMarker;
    use crate::npc::spawn_rat::RatBlackboard;
    use crate::world::events::ActiveEventsResource;
    use crate::world::heartbeat::{zone_qi_inflow_tick, ZoneQiInflowClock};
    use valence::prelude::{ChunkPos, Position, Update};

    let mut app = App::new();
    app.add_event::<RatBiteEvent>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<QiTransfer>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<DeathEvent>();
    app.add_systems(
        Update,
        (apply_rat_bite_qi_drain, release_drained_qi_on_death_system),
    );

    // 门①：`CultivationClock` 从非零 tick 起步（对齐 `heartbeat::tests::inflow_test_app`
    // 的 `start_tick` 参数）——`zone_qi_inflow_tick` 要到"额外推进"那一步才第一次被
    // 注册进 schedule，此刻先把时钟准备好。
    app.insert_resource(CultivationClock { tick: 100 });

    // 门②③：spawn zone 配非零 qi_equilibrium/qi_inflow_per_min，且 spirit_qi 严格低于
    // equilibrium（照抄 zones.json spawn 区真实配置 0.35/0.4）。
    let initial_spirit_qi = 0.1_f64;
    let mut zones = ZoneRegistry::fallback();
    {
        let zone = zones
            .find_zone_mut("spawn")
            .expect("fallback ZoneRegistry must have spawn zone");
        zone.spirit_qi = initial_spirit_qi;
        zone.qi_equilibrium = 0.35;
        zone.qi_inflow_per_min = 0.4;
    }
    app.insert_resource(zones);
    app.insert_resource(ActiveEventsResource::default());

    // 门④：待分配池注资，供 `zone_qi_inflow_tick` 的额外推进步骤真实借出。
    let mut ledger = WorldQiAccount::default();
    ledger
        .set_balance(pending_inflow_account(), 1000.0)
        .expect("seeding the pending pool balance must succeed");
    app.insert_resource(ledger);

    let target = app
        .world_mut()
        .spawn((
            Cultivation {
                qi_current: 20.0,
                qi_max: 20.0,
                ..Default::default()
            },
            LifeRecord::new(crate::player::state::canonical_player_id("RatBiteTarget")),
        ))
        .id();
    let rat = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([0.0, 64.0, 0.0]),
            RatBlackboard::new("spawn", ChunkPos::new(0, 0)),
        ))
        .id();
    app.world_mut()
        .entity_mut(rat)
        .insert(LifeRecord::new(crate::npc::brain::canonical_npc_id(rat)));

    let before = summarize_world_qi(app.world_mut());

    // 咬 3 次，qi_steal=2，累计 drained=6.0。
    for _ in 0..3 {
        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
    }
    app.update();

    let cultivation_after_bites = app.world().get::<Cultivation>(target).unwrap();
    assert!(
        (cultivation_after_bites.qi_current - 14.0).abs() < 1e-9,
        "sanity: 3 次 qi_steal=2 应共扣 6.0，实际剩余 {}",
        cultivation_after_bites.qi_current
    );

    // 鼠死亡结算——必须与"额外推进"分开一次 `app.update()`，确保 `zone_qi_inflow_tick`
    // （下面才注册）不会与死亡结算的这一拍混在一起触发。
    app.world_mut().send_event(DeathEvent {
        target: rat,
        cause: "test".to_string(),
        attacker: None,
        attacker_player_id: None,
        at_tick: 100,
    });
    app.update();

    // 现在才把 `zone_qi_inflow_tick` 接入 schedule，并推进 `CultivationClock`——
    // 门①要求 `elapsed_ticks > 0`；这一步专门用于撞穿"没写回字段就被下一次
    // inflow tick 覆盖式清零"的二次蒸发回归。
    app.insert_resource(ZoneQiInflowClock::default());
    app.add_systems(Update, zone_qi_inflow_tick);
    {
        let mut clock = app.world_mut().resource_mut::<CultivationClock>();
        clock.tick = clock.tick.saturating_add(1);
    }
    app.update();

    let after = summarize_world_qi(app.world_mut());

    let total_before = before.total_observed();
    let total_after = after.total_observed();
    assert!(
        (total_before - total_after).abs() < 1e-9,
        "worldview §二守恒律：鼠咬 + 死亡结算 + 额外一次 zone_qi_inflow_tick 全链路，\
             player_qi + absolute zone_qi + stable ledger_qi 头尾必须严格相等，实际 \
             before={total_before} after={total_after}（before={before:?}, after={after:?}）——\
             不等说明某个物理 owner 被重复镜像或在写回时丢失"
    );
}
