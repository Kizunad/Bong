use bong_server::cultivation::components::{Cultivation, Realm};
use bong_server::cultivation::life_record::LifeRecord;
use bong_server::skill::components::{SkillEntry, SkillId, SkillSet};
use bong_server::skill::events::{SkillLvUp, SkillXpGain, XpGainSource};
use bong_server::skill::{consume_skill_xp_gain, record_skill_lv_up, register};
use valence::prelude::{App, Events, Update};

#[test]
fn register_adds_all_four_events() {
    let mut app = App::new();
    register(&mut app);
    assert!(app.world().contains_resource::<Events<SkillXpGain>>());
    assert!(app.world().contains_resource::<Events<SkillLvUp>>());
    assert!(app
        .world()
        .contains_resource::<Events<bong_server::skill::events::SkillCapChanged>>());
    assert!(app
        .world()
        .contains_resource::<Events<bong_server::skill::events::SkillScrollUsed>>());
}

#[test]
fn xp_above_cap_is_scaled_down_to_thirty_percent() {
    let mut app = App::new();
    app.add_event::<SkillXpGain>();
    app.add_event::<SkillLvUp>();
    app.add_systems(Update, consume_skill_xp_gain);

    let mut skill_set = SkillSet::default();
    skill_set.skills.insert(
        SkillId::Herbalism,
        SkillEntry {
            lv: 5,
            xp: 0,
            total_xp: 0,
            last_action_at: 0,
            recent_repeat_count: 0,
        },
    );
    let entity = app
        .world_mut()
        .spawn((
            skill_set,
            Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().send_event(SkillXpGain {
        char_entity: entity,
        skill: SkillId::Herbalism,
        amount: 100,
        source: XpGainSource::Action {
            plan_id: "botany",
            action: "harvest_manual",
        },
    });
    app.update();

    let set = app.world().get::<SkillSet>(entity).unwrap();
    let entry = set.skills.get(&SkillId::Herbalism).unwrap();
    assert_eq!(entry.lv, 5);
    assert_eq!(entry.xp, 30);
}

#[test]
fn xp_below_cap_is_not_scaled() {
    let mut app = App::new();
    app.add_event::<SkillXpGain>();
    app.add_event::<SkillLvUp>();
    app.add_systems(Update, consume_skill_xp_gain);
    let entity = app
        .world_mut()
        .spawn((
            SkillSet::default(),
            Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            },
        ))
        .id();

    app.world_mut().send_event(SkillXpGain {
        char_entity: entity,
        skill: SkillId::Herbalism,
        amount: 100,
        source: XpGainSource::Action {
            plan_id: "botany",
            action: "harvest_manual",
        },
    });
    app.update();

    let set = app.world().get::<SkillSet>(entity).unwrap();
    let entry = set.skills.get(&SkillId::Herbalism).unwrap();
    assert_eq!(entry.lv, 1);
    assert_eq!(entry.xp, 0);
}

#[test]
fn record_skill_lv_up_appends_milestone() {
    let mut app = App::new();
    app.add_event::<SkillLvUp>();
    app.add_systems(Update, record_skill_lv_up);
    let mut skill_set = SkillSet::default();
    skill_set.skills.insert(
        SkillId::Forging,
        SkillEntry {
            lv: 3,
            xp: 0,
            total_xp: 700,
            last_action_at: 0,
            recent_repeat_count: 0,
        },
    );
    let entity = app
        .world_mut()
        .spawn((skill_set, LifeRecord::default()))
        .id();
    app.world_mut().send_event(SkillLvUp {
        char_entity: entity,
        skill: SkillId::Forging,
        new_lv: 3,
    });

    app.update();

    let life = app.world().get::<LifeRecord>(entity).unwrap();
    assert_eq!(life.skill_milestones.len(), 1);
    assert_eq!(life.skill_milestones[0].total_xp_at, 700);
    assert_eq!(life.skill_milestones[0].new_lv, 3);
}

#[test]
fn consume_skill_xp_gain_applies_over_cap_penalty_in_system() {
    let mut app = App::new();
    app.add_event::<SkillXpGain>();
    app.add_event::<SkillLvUp>();
    app.add_systems(Update, consume_skill_xp_gain);

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Induce,
                ..Default::default()
            },
            SkillSet {
                skills: std::collections::HashMap::from([(
                    SkillId::Herbalism,
                    SkillEntry {
                        lv: 6,
                        xp: 10,
                        total_xp: 100,
                        last_action_at: 0,
                        recent_repeat_count: 0,
                    },
                )]),
                consumed_scrolls: Default::default(),
            },
        ))
        .id();

    app.world_mut().send_event(SkillXpGain {
        char_entity: entity,
        skill: SkillId::Herbalism,
        amount: 10,
        source: XpGainSource::Action {
            plan_id: "lingtian",
            action: "harvest_auto",
        },
    });

    app.update();

    let set = app
        .world()
        .get::<SkillSet>(entity)
        .expect("skill set should remain attached");
    let entry = set
        .skills
        .get(&SkillId::Herbalism)
        .expect("entry should exist");
    assert_eq!(
        entry.xp, 13,
        "10 xp over cap should be reduced to 3 before adding"
    );
    assert_eq!(
        entry.total_xp, 103,
        "total_xp should track effective awarded xp"
    );
    assert_eq!(
        entry.last_action_at, 0,
        "missing GameplayTick resource should fall back to tick 0"
    );
}

#[test]
fn consume_skill_xp_gain_does_not_level_when_penalty_drops_below_threshold() {
    let mut app = App::new();
    app.add_event::<SkillXpGain>();
    app.add_event::<SkillLvUp>();
    app.add_systems(Update, consume_skill_xp_gain);

    let entity = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Induce,
                ..Default::default()
            },
            SkillSet {
                skills: std::collections::HashMap::from([(
                    SkillId::Herbalism,
                    SkillEntry {
                        lv: 6,
                        xp: 4_891,
                        total_xp: 9_991,
                        last_action_at: 0,
                        recent_repeat_count: 0,
                    },
                )]),
                consumed_scrolls: Default::default(),
            },
        ))
        .id();

    app.world_mut().send_event(SkillXpGain {
        char_entity: entity,
        skill: SkillId::Herbalism,
        amount: 10,
        source: XpGainSource::Action {
            plan_id: "lingtian",
            action: "harvest_auto",
        },
    });

    app.update();

    let set = app
        .world()
        .get::<SkillSet>(entity)
        .expect("skill set should remain attached");
    let entry = set
        .skills
        .get(&SkillId::Herbalism)
        .expect("entry should exist");
    assert_eq!(
        entry.lv, 6,
        "penalized xp should no longer be enough to level up"
    );
    assert_eq!(
        entry.xp, 4_894,
        "only 3 effective xp should be added over cap"
    );
    assert_eq!(entry.total_xp, 9_994);

    let lv_events = app.world().resource::<Events<SkillLvUp>>();
    assert_eq!(
        lv_events.len(),
        0,
        "no SkillLvUp event should be emitted when the penalty prevents leveling"
    );
}

#[test]
fn xp_gain_full_coverage_accumulates_for_every_skill_id() {
    let mut app = App::new();
    app.add_event::<SkillXpGain>();
    app.add_event::<SkillLvUp>();
    app.add_systems(Update, consume_skill_xp_gain);

    let entity = app
        .world_mut()
        .spawn((
            SkillSet::default(),
            Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            },
        ))
        .id();

    for skill in SkillId::ALL {
        app.world_mut().send_event(SkillXpGain {
            char_entity: entity,
            skill,
            amount: 1,
            source: XpGainSource::Action {
                plan_id: "coverage",
                action: skill.as_str(),
            },
        });
    }

    app.update();

    let set = app.world().get::<SkillSet>(entity).unwrap();
    for skill in SkillId::ALL {
        let entry = set.skills.get(&skill).expect("entry should be created");
        assert_eq!(entry.xp, 1, "{} xp should increment", skill.as_str());
        assert_eq!(
            entry.total_xp,
            1,
            "{} total_xp should increment",
            skill.as_str()
        );
    }
}
