use bong_server::alchemy::{furnace_tier_from_item_id, AlchemyFurnace, AlchemySession};
use valence::prelude::BlockPos;

#[test]
fn can_run_requires_tier_and_integrity() {
    let f = AlchemyFurnace::new(2);
    assert!(f.can_run(1));
    assert!(f.can_run(2));
    assert!(!f.can_run(3));
    let mut broken = AlchemyFurnace::new(2);
    broken.integrity = 0.0;
    assert!(!broken.can_run(1));
}

#[test]
fn start_and_end_session() {
    let mut f = AlchemyFurnace::new(1);
    let session = AlchemySession::new("r".into(), "alice".into());
    f.start_session(session).unwrap();
    assert!(f.is_busy());
    let ended = f.end_session();
    assert!(ended.is_some());
    assert!(!f.is_busy());
}

#[test]
fn cannot_start_when_busy() {
    let mut f = AlchemyFurnace::new(1);
    f.start_session(AlchemySession::new("r".into(), "a".into()))
        .unwrap();
    let again = f.start_session(AlchemySession::new("r".into(), "a".into()));
    assert!(again.is_err());
}

#[test]
fn apply_explode_clamps_at_zero() {
    let mut f = AlchemyFurnace::new(1);
    assert!(!f.apply_explode(0.5));
    assert!(f.apply_explode(0.8)); // 毁
    assert_eq!(f.integrity, 0.0);
}

#[test]
fn placed_carries_pos_and_tier() {
    let pos = BlockPos {
        x: -12,
        y: 64,
        z: 38,
    };
    let f = AlchemyFurnace::placed(pos, 2);
    assert_eq!(f.tier, 2);
    assert_eq!(f.pos, Some((-12, 64, 38)));
    assert_eq!(f.block_pos(), Some(pos));
}

#[test]
fn new_has_no_pos() {
    let f = AlchemyFurnace::new(1);
    assert!(f.pos.is_none());
    assert!(f.block_pos().is_none());
}

#[test]
fn furnace_tier_from_item_id_covers_fantie() {
    assert_eq!(furnace_tier_from_item_id("furnace_fantie"), Some(1));
    assert_eq!(furnace_tier_from_item_id("furnace_lingtie"), Some(2));
    assert_eq!(furnace_tier_from_item_id("furnace_xitie"), Some(3));
    assert_eq!(furnace_tier_from_item_id("hoe_iron"), None);
    assert_eq!(furnace_tier_from_item_id(""), None);
}
