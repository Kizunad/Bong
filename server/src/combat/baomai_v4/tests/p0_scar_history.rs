//! P0 ScarHistory 测试。

use valence::prelude::{App, Events};

use crate::combat::baomai_v3::events::{BaomaiSkillId, OverloadMeridianRippleEvent};
use crate::combat::baomai_v3::state::MeridianRippleScar;
use crate::combat::baomai_v4::scar_history::ScarHistory;
use crate::combat::events::CombatEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::MeridianId;

fn app() -> App {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 100 });
    app.add_event::<OverloadMeridianRippleEvent>();
    app.add_event::<CombatEvent>();
    crate::combat::baomai_v4::register(&mut app);
    app
}

fn spawn_entity_with_scar(app: &mut App, accumulated: u32) -> valence::prelude::Entity {
    app.world_mut()
        .spawn((MeridianRippleScar {
            severity: 0.0,
            accumulated_overloads: accumulated,
            last_updated_tick: 0,
        },))
        .id()
}

fn spawn_entity_with_history(app: &mut App, total: u32) -> valence::prelude::Entity {
    app.world_mut()
        .spawn(ScarHistory {
            total_overloads: total,
            ..Default::default()
        })
        .id()
}

fn send_overload(
    app: &mut App,
    caster: valence::prelude::Entity,
    skill: BaomaiSkillId,
    meridians: Vec<MeridianId>,
) {
    app.world_mut()
        .resource_mut::<Events<OverloadMeridianRippleEvent>>()
        .send(OverloadMeridianRippleEvent {
            caster,
            tick: 100,
            skill,
            severity_delta: 0.05,
            total_severity: 0.05,
            meridian_ids: meridians,
        });
}

#[test]
fn scar_history_increments_on_overload() {
    let mut app = app();
    let entity = spawn_entity_with_history(&mut app, 0);

    send_overload(
        &mut app,
        entity,
        BaomaiSkillId::BengQuan,
        vec![MeridianId::Lung],
    );
    app.update();

    let history = app.world().get::<ScarHistory>(entity).unwrap();
    assert_eq!(
        history.total_overloads, 1,
        "After one overload event, total_overloads should be 1, got {}",
        history.total_overloads
    );
}

#[test]
fn scar_history_per_skill_bucket() {
    let mut app = app();
    let entity = spawn_entity_with_history(&mut app, 0);

    send_overload(
        &mut app,
        entity,
        BaomaiSkillId::BengQuan,
        vec![MeridianId::Lung],
    );
    send_overload(
        &mut app,
        entity,
        BaomaiSkillId::MountainShake,
        vec![MeridianId::Heart],
    );
    send_overload(
        &mut app,
        entity,
        BaomaiSkillId::BengQuan,
        vec![MeridianId::Lung],
    );
    app.update();

    let history = app.world().get::<ScarHistory>(entity).unwrap();
    assert_eq!(
        history.overloads_by_skill.get(&BaomaiSkillId::BengQuan),
        Some(&2),
        "BengQuan should have 2 overloads in its bucket"
    );
    assert_eq!(
        history
            .overloads_by_skill
            .get(&BaomaiSkillId::MountainShake),
        Some(&1),
        "MountainShake should have 1 overload in its bucket"
    );
    assert_eq!(
        history.total_overloads, 3,
        "Total overloads should be 3 after 3 events"
    );
}

#[test]
fn scar_history_per_meridian_bucket() {
    let mut app = app();
    let entity = spawn_entity_with_history(&mut app, 0);

    send_overload(
        &mut app,
        entity,
        BaomaiSkillId::BengQuan,
        vec![MeridianId::Lung, MeridianId::Heart],
    );
    send_overload(
        &mut app,
        entity,
        BaomaiSkillId::BengQuan,
        vec![MeridianId::Lung],
    );
    app.update();

    let history = app.world().get::<ScarHistory>(entity).unwrap();
    assert_eq!(
        history.overloads_by_meridian.get(&MeridianId::Lung),
        Some(&2),
        "Lung should have 2 overloads because it was affected in both events"
    );
    assert_eq!(
        history.overloads_by_meridian.get(&MeridianId::Heart),
        Some(&1),
        "Heart should have 1 overload because it was affected in only the first event"
    );
}

#[test]
fn scar_history_survives_death() {
    // ScarHistory is a plain Component — it survives as long as the entity exists.
    // Death/revival in Bong does NOT despawn the entity, so ScarHistory persists.
    let mut app = app();
    let entity = spawn_entity_with_history(&mut app, 42);

    // Simulate "death" by doing nothing to the entity — it stays.
    app.update();

    let history = app.world().get::<ScarHistory>(entity).unwrap();
    assert_eq!(
        history.total_overloads, 42,
        "ScarHistory should persist across updates (simulating death/revival), got {}",
        history.total_overloads
    );
}

#[test]
fn scar_history_migration_from_ripple_scar() {
    // Entity has MeridianRippleScar but no ScarHistory — first overload should
    // trigger migration via from_ripple_scar().
    let mut app = app();
    let entity = spawn_entity_with_scar(&mut app, 10);

    send_overload(
        &mut app,
        entity,
        BaomaiSkillId::BloodBurn,
        vec![MeridianId::Kidney],
    );
    app.update();

    let history = app.world().get::<ScarHistory>(entity).unwrap();
    assert_eq!(
        history.total_overloads, 11,
        "Migration from MeridianRippleScar(10) + 1 new overload should give 11, got {}",
        history.total_overloads
    );
    assert_eq!(
        history.overloads_by_skill.get(&BaomaiSkillId::BloodBurn),
        Some(&1),
        "The new overload should be recorded in the skill bucket"
    );
}
