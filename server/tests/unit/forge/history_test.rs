use bong_server::forge::events::ForgeBucket;
use bong_server::forge::history::{ForgeAttempt, ForgeHistory};

#[test]
fn bucket_tag_mapping() {
    assert_eq!(ForgeAttempt::from_bucket(&ForgeBucket::Perfect), "perfect");
    assert_eq!(ForgeAttempt::from_bucket(&ForgeBucket::Waste), "waste");
}

#[test]
fn recent_tails_n_entries() {
    let mut h = ForgeHistory::new();
    for i in 0..5 {
        h.push(ForgeAttempt {
            tick: i,
            blueprint: "x".into(),
            bucket_tag: "good".into(),
            achieved_tier: 1,
            weapon_item: None,
            quality: 1.0,
            color: None,
            side_effects: vec![],
        });
    }
    assert_eq!(h.recent(3).len(), 3);
    assert_eq!(h.recent(3)[0].tick, 2);
}
