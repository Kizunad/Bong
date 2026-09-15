#![allow(dead_code, unused_imports)]

use bong_server::combat::body_conditioning::{GuangboTicaoPracticeEvent, GUANGBO_TICAO_ID};
use bong_server::combat::components::QuickSlotBindings;
use bong_server::network::cast_emit::{
    has_direct_generic_completion_consumer, CAST_MOVEMENT_INTERRUPT_THRESHOLD_M,
};
use valence::prelude::DVec3;

#[test]
fn movement_interrupt_threshold_classifies_within_and_beyond() {
    use valence::prelude::DVec3;
    let start = DVec3::new(10.0, 64.0, 20.0);
    let still_ok = DVec3::new(10.2, 64.0, 20.0); // 0.2m → 不中断
    let too_far = DVec3::new(10.5, 64.0, 20.0); // 0.5m → 中断
    assert!(still_ok.distance(start) <= CAST_MOVEMENT_INTERRUPT_THRESHOLD_M);
    assert!(too_far.distance(start) > CAST_MOVEMENT_INTERRUPT_THRESHOLD_M);
}

#[test]
fn cooldown_set_get_round_trip() {
    let mut bindings = QuickSlotBindings::default();
    assert!(!bindings.is_on_cooldown(1, 100));
    bindings.set_cooldown(1, 130);
    assert!(bindings.is_on_cooldown(1, 100));
    assert!(bindings.is_on_cooldown(1, 129));
    assert!(!bindings.is_on_cooldown(1, 130));
    assert!(!bindings.is_on_cooldown(1, 131));
    // out-of-range slot is silently no-op
    assert!(!bindings.is_on_cooldown(99, 0));
    bindings.set_cooldown(99, 100);
    assert!(!bindings.is_on_cooldown(99, 50));
}

#[test]
fn direct_generic_completion_registry_matches_the_real_guangbo_consumer() {
    assert!(has_direct_generic_completion_consumer(GUANGBO_TICAO_ID));
    assert!(!has_direct_generic_completion_consumer("movement.dash"));
    assert!(!has_direct_generic_completion_consumer("shield_block"));
    assert!(!has_direct_generic_completion_consumer(
        "unknown.consumerless"
    ));
}
