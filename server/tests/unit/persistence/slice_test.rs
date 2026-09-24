#![allow(dead_code, unused_imports)]

use bong_server::persistence::slice::{
    rebase_remaining_deadline, OfflineTimePolicy, RemainingDeadline, SliceLoad, SliceLoadStatus,
    TickRebaseError,
};

#[test]
fn slice_load_status_predicates_cover_every_state() {
    let cases = [
        (
            SliceLoad::<u32, &str>::missing(),
            SliceLoadStatus::Missing,
            (true, false, false),
        ),
        (
            SliceLoad::<u32, &str>::loaded(17),
            SliceLoadStatus::Loaded,
            (false, true, false),
        ),
        (
            SliceLoad::<u32, &str>::failed("secret failure"),
            SliceLoadStatus::Failed,
            (false, false, true),
        ),
    ];

    for (load, status, predicates) in cases {
        assert_eq!(load.status(), status);
        assert_eq!(
            (load.is_missing(), load.is_loaded(), load.is_failed()),
            predicates,
            "each SliceLoad state must report one and only one matching predicate"
        );
    }
}

#[test]
fn slice_load_debug_reports_status_without_payloads() {
    let missing = format!("{:?}", SliceLoad::<String, String>::missing());
    let loaded = format!(
        "{:?}",
        SliceLoad::<String, String>::loaded("private player payload".to_string())
    );
    let failed = format!(
        "{:?}",
        SliceLoad::<String, String>::failed("database credential leaked".to_string())
    );

    assert!(missing.contains("status: Missing"));
    assert!(loaded.contains("status: Loaded"));
    assert!(failed.contains("status: Failed"));
    assert!(!loaded.contains("private player payload"));
    assert!(!failed.contains("database credential leaked"));
}

#[test]
fn deadline_rebase_pins_pause_continue_and_boundaries() {
    let paused = RemainingDeadline {
        remaining_ticks: 200,
        saved_at_wall_millis: 1_000_000,
        offline_policy: OfflineTimePolicy::Pause,
    };
    let advancing = RemainingDeadline {
        offline_policy: OfflineTimePolicy::Continue,
        ..paused
    };

    assert_eq!(
        rebase_remaining_deadline(paused, 10, 1_005_000),
        Ok(210),
        "online-only deadlines preserve all remaining ticks"
    );
    assert_eq!(
        rebase_remaining_deadline(advancing, 10, 1_000_049),
        Ok(210),
        "49ms is below one logical tick and must not reduce the deadline"
    );
    assert_eq!(
        rebase_remaining_deadline(advancing, 10, 1_000_050),
        Ok(209),
        "50ms is exactly one logical tick"
    );
    let one_tick = RemainingDeadline {
        remaining_ticks: 1,
        offline_policy: OfflineTimePolicy::Continue,
        ..paused
    };
    assert_eq!(
        rebase_remaining_deadline(one_tick, 10, 1_000_049),
        Ok(11),
        "one remaining tick survives until the 50ms boundary"
    );
    assert_eq!(
        rebase_remaining_deadline(one_tick, 10, 1_000_050),
        Ok(10),
        "one remaining tick expires exactly at the 50ms boundary"
    );
    assert_eq!(
        rebase_remaining_deadline(advancing, 10, 1_005_000),
        Ok(110),
        "five offline seconds consume exactly 100 ticks at 50ms/tick"
    );
    assert_eq!(
        rebase_remaining_deadline(advancing, 10, 999_000),
        Ok(210),
        "a wall clock moving backwards must not create negative elapsed time"
    );
    assert_eq!(
        rebase_remaining_deadline(advancing, 10, 2_000_000),
        Ok(10),
        "offline elapsed beyond the remaining duration clamps the deadline to now"
    );
    assert_eq!(
        rebase_remaining_deadline(
            RemainingDeadline {
                remaining_ticks: 1,
                ..paused
            },
            u64::MAX,
            1_000_000,
        ),
        Err(TickRebaseError::DeadlineOverflow)
    );
}
