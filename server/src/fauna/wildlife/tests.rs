//! 保护真实出生配置、施法事务和战术回执；不以私有状态数量作为契约。

use valence::prelude::bevy_ecs::{system::RunSystemOnce, world::CommandQueue};
use valence::prelude::*;
use valence::testing::ScenarioSingleClient;

use crate::body_plan::{BodyPlanRegistry, RaceRegistry};
use crate::combat::components::{BodyPart, DerivedAttrs, Stamina, WoundKind, Wounds};
use crate::combat::events::{AttackIntent, AttackSource, CombatEvent};
use crate::cultivation::components::{Cultivation, MeridianSystem};
use crate::cultivation::known_techniques::TechniqueRegistry;
use crate::cultivation::meridian::severed::{MeridianSeveredPermanent, SeveredSource};
use crate::cultivation::skill_registry::{init_registry, CastResult};
use crate::fauna::components::BeastKind;
use crate::npc::movement::GameTick;
use crate::npc::technique::NpcCooldownMap;
use crate::qi_physics::WorldQiAccount;
use crate::world::zone::{Zone, ZoneRegistry};

use super::{brain, config, motion, skills, spawn};

fn fixture() -> (App, Entity, Zone) {
    let scenario = ScenarioSingleClient::new();
    let layer = scenario.layer;
    let mut app = scenario.app;
    let plans = BodyPlanRegistry::load_dir(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/body_plans/plans"),
    )
    .unwrap();
    app.insert_resource(plans)
        .insert_resource(RaceRegistry::load_for_tests())
        .insert_resource(TechniqueRegistry::load_for_tests())
        .insert_resource(config::WildlifeCatalog::load())
        .insert_resource(init_registry())
        .init_resource::<NpcCooldownMap>()
        .init_resource::<GameTick>()
        .init_resource::<WorldQiAccount>()
        .add_event::<AttackIntent>()
        .add_event::<CombatEvent>();
    let mut chunk_layer = app.world_mut().get_mut::<ChunkLayer>(layer).unwrap();
    let mut chunk = UnloadedChunk::with_height(chunk_layer.height());
    for x in 0..16 {
        for z in 0..16 {
            chunk.set_block_state(
                x,
                (64 - chunk_layer.min_y()) as u32,
                z,
                BlockState::GRASS_BLOCK,
            );
        }
    }
    chunk_layer.insert_chunk([0, 0], chunk);
    let mut zone = ZoneRegistry::default().zones[0].clone();
    zone.name = "wildlife_test".to_string();
    zone.danger_level = 4;
    zone.spirit_qi = 0.8;
    (app, layer, zone)
}

fn spawn_one(app: &mut App, layer: Entity, zone: &Zone, kind: BeastKind, x: f64) -> Entity {
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world());
    let entity = spawn::spawn_at(
        &mut commands,
        layer,
        zone,
        DVec3::new(x, 65.0, 4.0),
        kind,
        DVec3::new(4.0, 65.0, 4.0),
    );
    queue.apply(app.world_mut());
    entity
}

fn begin(app: &mut App, actor: Entity, target: Entity, id: &str) -> CastResult {
    let callback = app
        .world()
        .resource::<crate::cultivation::skill_registry::SkillRegistry>()
        .lookup(id)
        .unwrap();
    callback(app.world_mut(), actor, 0, Some(target))
}

#[test]
fn natural_selection_reaches_all_species_and_respects_habitat_gates() {
    use crate::world::dimension::DimensionKind;

    let mut zone = ZoneRegistry::default().zones[0].clone();
    zone.spirit_qi = 0.8;
    zone.danger_level = 4;
    for name in ["wilderness", "grassland"] {
        zone.name = name.to_string();
        let mut kinds = std::collections::HashSet::new();
        for x in -8..8 {
            for z in -8..8 {
                let position = DVec3::new(f64::from(x * 48), 65.0, f64::from(z * 48));
                kinds.extend(spawn::choose_kind(&zone, position));
            }
        }
        for expected in [
            BeastKind::DainuLion,
            BeastKind::FuyuVulture,
            BeastKind::Horse,
        ] {
            assert!(
                kinds.contains(&expected),
                "区域 {name} 的合法物种 {expected:?} 不能被种子低位永久排除"
            );
        }
    }
    let position = DVec3::new(10.0, 65.0, 10.0);
    zone.name = "spawn".to_string();
    assert_eq!(spawn::choose_kind(&zone, position), None);
    zone.name = "wilderness".to_string();
    zone.spirit_qi = 0.0;
    assert_eq!(spawn::choose_kind(&zone, position), None);
    zone.spirit_qi = 0.8;
    zone.dimension = DimensionKind::Tsy;
    assert_eq!(spawn::choose_kind(&zone, position), None);
}

#[test]
fn invalid_species_skill_configuration_is_rejected_before_spawning() {
    let (app, _, _) = fixture();
    let techniques = app.world().resource::<TechniqueRegistry>();
    let skills = app
        .world()
        .resource::<crate::cultivation::skill_registry::SkillRegistry>();
    let races = app.world().resource::<RaceRegistry>();
    for id in ["lion.missing", "horse.kick"] {
        let mut catalog = config::WildlifeCatalog::load();
        let lion = catalog
            .creatures
            .iter_mut()
            .find(|creature| creature.kind == BeastKind::DainuLion)
            .unwrap();
        lion.engage_skill = id.to_string();
        let error = catalog
            .validate_skills(techniques, skills, races)
            .expect_err("拼错技能或装入异种技能应在启动时拒绝");
        assert!(error.contains("dainu_lion.engage_skill") && error.contains(id));
    }
}

#[test]
fn production_spawns_use_species_realms_channels_and_persistent_base_attributes() {
    let (mut app, layer, zone) = fixture();
    for kind in [
        BeastKind::DainuLion,
        BeastKind::FuyuVulture,
        BeastKind::Horse,
    ] {
        let entity = spawn_one(&mut app, layer, &zone, kind, 4.0);
        let cultivation = app.world().get::<Cultivation>(entity).unwrap();
        assert!((kind.realm_tier() + 1..=kind.realm_tier() + 2).contains(&cultivation.realm.rank()));
        assert_eq!(cultivation.race.as_str(), kind.as_str());
        assert_eq!(cultivation.qi_current, 0.0, "出生不能凭空生成真元");
        let definition = app.world().resource::<config::WildlifeCatalog>().get(kind);
        assert!(
            skills::channels_ready(app.world(), entity, &definition.engage_skill),
            "出生开的真实经脉应支持本物种的起手技能"
        );
        let attack = app
            .world()
            .get::<config::WildlifeAttributes>(entity)
            .unwrap()
            .attack;
        app.world_mut()
            .run_system_once(crate::combat::status::attribute_aggregate_tick);
        app.world_mut()
            .run_system_once(crate::combat::status::attribute_aggregate_tick);
        assert_eq!(
            app.world()
                .get::<DerivedAttrs>(entity)
                .unwrap()
                .attack_power,
            attack,
            "每帧聚合保留基础攻击，且不可重复叠乘"
        );
    }
}

#[test]
fn stamina_status_uses_species_base_and_restores_it_after_effects_expire() {
    use crate::combat::components::{ActiveStatusEffect, StatusEffects};
    use crate::combat::events::StatusEffectKind;

    let (mut app, layer, zone) = fixture();
    app.init_resource::<crate::combat::CombatClock>();
    let horse = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 4.0);
    let base = app.world().get::<Stamina>(horse).unwrap().max;
    for (effect, multiplier) in [
        (None, 1.0),
        (Some((StatusEffectKind::StaminaRecovBoost, 0.25)), 1.25),
        (Some((StatusEffectKind::StaminaCrash, 0.5)), 0.5),
        (None, 1.0),
    ] {
        app.world_mut().entity_mut(horse).insert(StatusEffects {
            active: effect
                .into_iter()
                .map(|(kind, magnitude)| ActiveStatusEffect {
                    kind,
                    magnitude,
                    remaining_ticks: 20,
                    source_pill: None,
                })
                .collect(),
        });
        app.world_mut()
            .run_system_once(crate::combat::status::combat_pill_stamina_status_tick);
        assert_eq!(
            app.world().get::<Stamina>(horse).unwrap().max,
            base * multiplier,
            "体力增减益及恢复必须以物种上限为基准，不能退回人形默认值"
        );
    }
}

#[test]
fn closed_damaged_or_severed_species_channel_rejects_without_spending() {
    for condition in ["closed", "damaged", "severed"] {
        let (mut app, layer, zone) = fixture();
        let lion = spawn_one(&mut app, layer, &zone, BeastKind::DainuLion, 4.0);
        let target = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 6.0);
        match condition {
            "closed" => {
                app.world_mut()
                    .get_mut::<MeridianSystem>(lion)
                    .unwrap()
                    .get_mut("lion_stride")
                    .opened = false
            }
            "damaged" => {
                app.world_mut()
                    .get_mut::<MeridianSystem>(lion)
                    .unwrap()
                    .get_mut("lion_stride")
                    .integrity = 0.1
            }
            _ => {
                app.world_mut()
                    .get_mut::<MeridianSeveredPermanent>(lion)
                    .unwrap()
                    .insert("lion_stride", SeveredSource::CombatWound, 0);
            }
        }
        let stamina = app.world().get::<Stamina>(lion).unwrap().current;
        assert!(
            matches!(
                begin(&mut app, lion, target, "lion.pounce"),
                CastResult::Rejected { .. }
            ),
            "{condition} 兽脉必须拒绝施法"
        );
        assert_eq!(app.world().get::<Stamina>(lion).unwrap().current, stamina);
        assert!(app.world().get::<skills::WildlifeCast>(lion).is_none());
        assert!(!app
            .world()
            .resource::<NpcCooldownMap>()
            .is_on_cooldown(lion, "lion.pounce", 0));
    }
}

#[test]
fn one_cast_submits_once_and_only_confirmed_damage_allows_lion_followup() {
    for confirmed in [false, true] {
        let (mut app, layer, zone) = fixture();
        let lion = spawn_one(&mut app, layer, &zone, BeastKind::DainuLion, 4.0);
        let target = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 6.0);
        assert!(matches!(
            begin(&mut app, lion, target, "lion.pounce"),
            CastResult::Started { .. }
        ));
        let cast = app
            .world()
            .get::<skills::WildlifeCast>(lion)
            .unwrap()
            .clone();
        app.world_mut().resource_mut::<GameTick>().0 = cast.hit_at as u32;
        skills::tick_casts(app.world_mut());
        skills::tick_casts(app.world_mut());
        app.world_mut().resource_mut::<GameTick>().0 += 1;
        skills::tick_casts(app.world_mut());
        assert_eq!(
            app.world().resource::<Events<AttackIntent>>().len(),
            1,
            "冲锋窗口只能提交一次，重复 tick 也不能重复扣血"
        );
        if confirmed {
            app.world_mut().send_event(hit_event(lion, target));
            app.world_mut().run_system_once(brain::combat_feedback);
        }
        app.world_mut().resource_mut::<GameTick>().0 = cast.finish_at as u32;
        skills::tick_casts(app.world_mut());
        assert_eq!(
            app.world()
                .get::<brain::WildlifeBrain>(lion)
                .unwrap()
                .last_attack_hit,
            confirmed,
            "提交意图不能当作命中回执"
        );
        assert_eq!(
            app.world().get::<Wounds>(target).unwrap().health_current,
            app.world().get::<Wounds>(target).unwrap().health_max,
            "技能不得绕过 resolver 扣血"
        );
    }
}

fn hit_event(attacker: Entity, target: Entity) -> CombatEvent {
    CombatEvent {
        attacker,
        target,
        resolved_at_tick: 999,
        body_part: BodyPart::Chest,
        wound_kind: WoundKind::Blunt,
        source: AttackSource::NpcMelee,
        debug_command: false,
        physical_damage: 5.0,
        damage: 0.0,
        contam_delta: 0.0,
        description: String::new(),
        defense_kind: None,
        defense_effectiveness: None,
        defense_contam_reduced: None,
        defense_wound_severity: None,
    }
}

#[test]
fn herd_damage_recruits_nearby_members_but_not_other_layers() {
    let (mut app, layer, zone) = fixture();
    let horse = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 4.0);
    let ally = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 7.0);
    let elsewhere = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 9.0);
    app.world_mut()
        .entity_mut(elsewhere)
        .insert(EntityLayerId(Entity::PLACEHOLDER));
    let attacker = spawn_one(&mut app, layer, &zone, BeastKind::DainuLion, 6.0);
    assert!(
        app.world()
            .get::<brain::WildlifeBrain>(horse)
            .unwrap()
            .target
            .is_none(),
        "马应保持中立"
    );
    app.world_mut().send_event(hit_event(attacker, horse));
    app.world_mut().run_system_once(brain::combat_feedback);
    for entity in [horse, ally] {
        let brain = app.world().get::<brain::WildlifeBrain>(entity).unwrap();
        assert_eq!(brain.target, Some(attacker));
        assert!(brain.rally_until > 0, "马群需要集结前摇");
    }
    assert!(app
        .world()
        .get::<brain::WildlifeBrain>(elsewhere)
        .unwrap()
        .target
        .is_none());
}

#[test]
fn configured_qi_cost_is_conserved_and_failed_ledger_payment_is_atomic() {
    for funded in [false, true] {
        let (mut app, layer, zone) = fixture();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            "lion.pounce",
            |def| def.qi_cost = 3.0,
        ));
        let lion = spawn_one(&mut app, layer, &zone, BeastKind::DainuLion, 4.0);
        let target = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 6.0);
        app.world_mut()
            .get_mut::<Cultivation>(lion)
            .unwrap()
            .qi_current = 8.0;
        // 活体真元由 Cultivation 持有，ledger 只记录稳定账户和转账审计。
        if !funded {
            app.world_mut().remove_resource::<WorldQiAccount>();
        }
        let stamina = app.world().get::<Stamina>(lion).unwrap().current;
        let result = begin(&mut app, lion, target, "lion.pounce");
        if funded {
            assert!(matches!(result, CastResult::Started { .. }));
            assert_eq!(
                app.world().get::<Cultivation>(lion).unwrap().qi_current,
                5.0
            );
            assert_eq!(
                app.world()
                    .resource::<WorldQiAccount>()
                    .iter_balances()
                    .map(|(_, balance)| balance)
                    .sum::<f64>()
                    + app.world().get::<Cultivation>(lion).unwrap().qi_current,
                8.0
            );
        } else {
            assert!(matches!(result, CastResult::Rejected { .. }));
            assert_eq!(
                app.world().get::<Cultivation>(lion).unwrap().qi_current,
                8.0
            );
            assert_eq!(app.world().get::<Stamina>(lion).unwrap().current, stamina);
        }
    }
}

#[test]
fn flight_and_attack_do_not_pass_through_solid_walls_or_unknown_chunks() {
    let (mut app, layer, zone) = fixture();
    let vulture = spawn_one(&mut app, layer, &zone, BeastKind::FuyuVulture, 4.0);
    let target = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 8.0);
    let mut chunks = app.world_mut().get_mut::<ChunkLayer>(layer).unwrap();
    for y in 65..69 {
        for z in 2..7 {
            chunks.set_block([6, y, z], BlockState::STONE);
        }
    }
    let start = DVec3::new(4.0, 65.0, 4.0);
    let blocked = motion::sweep_flight(&chunks, start, DVec3::new(9.0, 65.0, 4.0));
    assert!(blocked.x < 6.0);
    let edge = motion::sweep_flight(
        &chunks,
        DVec3::new(14.0, 65.0, 10.0),
        DVec3::new(18.0, 65.0, 10.0),
    );
    assert!(edge.x < 16.0, "未加载区块必须停止飞行");
    assert!(matches!(
        begin(&mut app, vulture, target, "vulture.dive"),
        CastResult::Rejected { .. }
    ));
}

#[test]
fn species_attack_reaches_real_resolver_and_emits_damage_feedback() {
    let (mut app, layer, zone) = fixture();
    app.insert_resource(crate::combat::CombatClock { tick: 20 })
        .add_event::<crate::combat::events::ApplyStatusEffectIntent>()
        .add_event::<crate::combat::events::DeathEvent>()
        .add_event::<crate::combat::weapon::WeaponBroken>()
        .add_event::<crate::combat::weapon::ShieldBroken>()
        .add_event::<crate::combat::weapon::ShieldBlockHit>()
        .add_event::<crate::inventory::InventoryDurabilityChangedEvent>();
    let lion = spawn_one(&mut app, layer, &zone, BeastKind::DainuLion, 4.0);
    let target = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 6.0);
    assert!(matches!(
        begin(&mut app, lion, target, "lion.pounce"),
        CastResult::Started { .. }
    ));
    app.world_mut().resource_mut::<GameTick>().0 = 13;
    skills::tick_casts(app.world_mut());
    app.world_mut()
        .run_system_once(crate::combat::status::attribute_aggregate_tick);
    app.world_mut()
        .run_system_once(crate::combat::resolve::resolve_attack_intents);
    let health = app.world().get::<Wounds>(target).unwrap();
    assert!(
        health.health_current < health.health_max,
        "真实非人形命中盒必须能被技能击中并由 resolver 扣血"
    );
    assert_eq!(app.world().resource::<Events<CombatEvent>>().len(), 1);
    app.world_mut().run_system_once(brain::combat_feedback);
    assert!(app.world().get::<skills::WildlifeCast>(lion).unwrap().hit);
}

#[test]
fn species_channels_support_overload_reading_and_realm_regression() {
    use crate::cultivation::components::{CrackCause, Realm};
    use crate::cultivation::qi_zero_decay::{
        qi_zero_decay_tick, RealmRegressed, DECAY_TRIGGER_TICKS,
    };
    let (mut app, layer, zone) = fixture();
    let lion = spawn_one(&mut app, layer, &zone, BeastKind::DainuLion, 4.0);
    let mut meridians = app.world().get::<MeridianSystem>(lion).unwrap().clone();
    let cracked = crate::cultivation::overload::apply_meridian_crack_to_system(
        &mut meridians,
        0.5,
        CrackCause::Overload,
        12,
    )
    .unwrap();
    assert!(cracked.as_str().starts_with("lion_"));
    let reading = crate::combat::baomai_v4::crack_reading::build_reading_result(
        lion, &meridians, None, None, true, false, 20,
    );
    let payload = crate::combat::baomai_v4::crack_reading::to_client_payload(&reading, 20);
    assert!(
        payload
            .entries
            .iter()
            .any(|entry| entry.meridian == cracked.as_str()),
        "裂读必须保留兽脉名称，不能伪造成人形肺经"
    );
    app.world_mut().entity_mut(lion).insert(meridians);
    {
        let mut cultivation = app.world_mut().get_mut::<Cultivation>(lion).unwrap();
        cultivation.realm = Realm::Condense;
        cultivation.last_qi_zero_at = Some(0);
    }
    app.insert_resource(crate::cultivation::tick::CultivationClock {
        tick: DECAY_TRIGGER_TICKS + 1,
    })
    .add_event::<RealmRegressed>();
    app.world_mut().run_system_once(qi_zero_decay_tick);
    assert_eq!(
        app.world().get::<Cultivation>(lion).unwrap().realm,
        Realm::Induce
    );
    assert_eq!(
        app.world()
            .get::<MeridianSystem>(lion)
            .unwrap()
            .iter()
            .filter(|m| m.opened)
            .count(),
        2,
        "降境按狮子的构型配额闭脉"
    );
}

#[test]
fn autonomous_tactics_drive_species_skills_and_vulture_climb() {
    for (kind, skill) in [
        (BeastKind::DainuLion, "lion.pounce"),
        (BeastKind::FuyuVulture, "vulture.dive"),
        (BeastKind::Horse, "horse.trample"),
    ] {
        let (mut app, layer, zone) = fixture();
        app.add_event::<crate::combat::knockback::KnockbackEvent>();
        app.add_plugins(big_brain::prelude::BigBrainPlugin::new(PreUpdate));
        crate::npc::movement::register(&mut app);
        crate::npc::navigator::register(&mut app);
        super::register(&mut app);
        let actor = spawn_one(&mut app, layer, &zone, kind, 4.0);
        let target = spawn_one(&mut app, layer, &zone, BeastKind::Horse, 8.0);
        // 固定伤者的位置，观察捕猎方完整的感知、决策和施法过程。
        app.world_mut()
            .entity_mut(target)
            .remove::<brain::WildlifeBrain>();
        app.world_mut()
            .get_mut::<Wounds>(target)
            .unwrap()
            .health_current = 5.0;
        let ally = (kind == BeastKind::Horse).then(|| spawn_one(&mut app, layer, &zone, kind, 5.0));
        if let Some(ally) = ally {
            for _ in 0..10 {
                app.update();
            }
            for horse in [actor, ally] {
                assert!(
                    app.world()
                        .get::<brain::WildlifeBrain>(horse)
                        .unwrap()
                        .target
                        .is_none(),
                    "马群不能主动攻击附近的伤者"
                );
            }
            app.world_mut()
                .resource_mut::<Events<CombatEvent>>()
                .send(hit_event(target, actor));
        }

        let mut cast_started = false;
        for _ in 0..180 {
            app.update();
            let now = skills::now(app.world());
            let cooldowns = app.world().resource::<NpcCooldownMap>();
            if cooldowns.is_on_cooldown(actor, skill, now)
                && ally.is_none_or(|ally| cooldowns.is_on_cooldown(ally, skill, now))
            {
                cast_started = true;
                if kind == BeastKind::FuyuVulture {
                    assert!(
                        app.world().get::<Position>(actor).unwrap().get().y > 68.0,
                        "鹫应先升空再俯冲，不能直接在地面释放俯冲"
                    );
                }
                if kind == BeastKind::Horse {
                    assert!(now >= 40, "马群受击后应先集结再冲锋");
                }
                break;
            }
        }
        assert!(cast_started, "{kind:?} 的 BigBrain 战术应实际调用 {skill}");
    }
}
