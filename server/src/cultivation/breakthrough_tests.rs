#![allow(dead_code, unused_imports)]

use super::*;
use crate::cultivation::components::MeridianId;
use crate::cultivation::overload;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::{QiAccountId, QiTransferReason, WorldQiAccount};
use crate::schema::common::NarrationScope;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::karma::KarmaWeightStore;
use crate::world::zone::ZoneRegistry;
use valence::prelude::{App, Events, Update, Username};

struct FixedRoll(f64);
impl RollSource for FixedRoll {
    fn roll_unit(&mut self) -> f64 {
        self.0
    }
}

fn setup_for_induce() -> (Cultivation, MeridianSystem) {
    let mut c = Cultivation {
        qi_current: 100.0,
        qi_max: 100.0,
        composure: 1.0,
        realm: Realm::Awaken,
        ..Default::default()
    };
    c.realm = Realm::Awaken;
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 3);
    (c, m)
}

fn open_regular(meridians: &mut MeridianSystem, count: usize) {
    for id in MeridianId::REGULAR.iter().take(count) {
        meridians.get_mut(*id).opened = true;
    }
}

fn open_extraordinary(meridians: &mut MeridianSystem, count: usize) {
    for id in MeridianId::EXTRAORDINARY.iter().take(count) {
        meridians.get_mut(*id).opened = true;
    }
}

fn open_all_meridians(meridians: &mut MeridianSystem) {
    for id in MeridianId::REGULAR
        .iter()
        .chain(MeridianId::EXTRAORDINARY.iter())
    {
        meridians.get_mut(*id).opened = true;
    }
}

#[test]
fn breakthrough_actor_account_id_uses_stable_life_record_id() {
    let player_life = LifeRecord::new("player_a");
    let npc_life = LifeRecord::new("npc_a");

    assert_eq!(
        breakthrough_actor_account_id(Some(&player_life), false)
            .expect("player life record with character_id should produce an account id"),
        QiAccountId::player("player_a"),
        "player breakthrough ledger id must come from stable LifeRecord.character_id"
    );
    assert_eq!(
        breakthrough_actor_account_id(Some(&npc_life), true)
            .expect("npc life record with character_id should produce an account id"),
        QiAccountId::npc("npc_a"),
        "npc breakthrough ledger id must come from stable LifeRecord.character_id"
    );
}

#[test]
fn breakthrough_actor_account_id_rejects_missing_or_blank_id() {
    let blank_life = LifeRecord::new("   ");

    assert!(
        matches!(
            breakthrough_actor_account_id(None, true),
            Err(BreakthroughLedgerError::MissingStableActorId { is_npc: true })
        ),
        "npc breakthrough ledger id must reject missing LifeRecord instead of falling back to unstable Entity ids"
    );
    assert!(
        matches!(
            breakthrough_actor_account_id(Some(&blank_life), false),
            Err(BreakthroughLedgerError::MissingStableActorId { is_npc: false })
        ),
        "player breakthrough ledger id must reject blank LifeRecord.character_id"
    );
}

#[test]
fn credit_active_breakthrough_cost_handles_boundaries() {
    let mut ledger = WorldQiAccount::default();
    let from = QiAccountId::player("player_a");
    // plan-zone-qi-economy-v1 P0 §8.1 决议 #1：目标是独立待分配池，不是
    // zone:<name>（那个 key 会被 dormant regen 整体覆写，credit 进去等于蒸发）。
    let pending_pool = crate::qi_physics::pending_inflow_account();

    credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), 0.0)
        .expect("zero breakthrough cost should be a no-op");
    assert_eq!(
        ledger.balance(&pending_pool),
        0.0,
        "zero breakthrough cost should not create pending pool balance"
    );
    assert_eq!(
        ledger.balance(&QiAccountId::zone("spawn")),
        0.0,
        "zero breakthrough cost must never touch the zone:<name> ledger account"
    );
    assert!(
        ledger.transfers().is_empty(),
        "zero breakthrough cost should not append a transfer audit"
    );

    let err = credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), -1.0)
        .expect_err("negative breakthrough cost must be rejected");
    assert!(
        matches!(
            err,
            BreakthroughLedgerError::QiPhysics(QiPhysicsError::InvalidAmount {
                field: "transfer.amount",
                ..
            })
        ),
        "negative breakthrough cost should surface the QiPhysics invalid amount error; got {err:?}"
    );

    credit_active_breakthrough_cost(&mut ledger, "spawn", from.clone(), 8.0)
        .expect("positive breakthrough cost should credit the pending inflow pool");
    assert_eq!(
        ledger.balance(&pending_pool),
        8.0,
        "first positive breakthrough cost should create the pending pool account and \
         credit the spent qi"
    );
    assert_eq!(
        ledger.balance(&QiAccountId::zone("spawn")),
        0.0,
        "positive breakthrough cost must still never touch the zone:<name> ledger account \
         (dormant regen owns that key and overwrites it wholesale from zone.spirit_qi)"
    );
    let transfer = ledger
        .transfers()
        .last()
        .expect("positive breakthrough cost should append one transfer audit");
    assert_eq!(
        transfer.from, from,
        "breakthrough audit transfer must preserve the stable actor account as source"
    );
    assert_eq!(
        transfer.to, pending_pool,
        "breakthrough audit transfer must target the independent pending inflow pool"
    );
    assert_eq!(
        transfer.reason,
        QiTransferReason::Breakthrough,
        "breakthrough audit transfer must use the dedicated reason"
    );
    assert_eq!(
        transfer.amount, 8.0,
        "breakthrough audit transfer amount must equal the spent qi"
    );
}

#[test]
fn breakthrough_error_message_covers_all_error_variants() {
    let cases = [
        (
            BreakthroughError::AtMaxRealm,
            "突破未成：你已抵达当前最高境界。",
        ),
        (
            BreakthroughError::RequiresTribulation,
            "突破未成：通灵至化虚必须先走渡虚劫。",
        ),
        (
            BreakthroughError::NotEnoughMeridians { need: 16, have: 15 },
            "突破未成：需先打通 16 条经脉（当前 15）。",
        ),
        (
            BreakthroughError::NotEnoughRegularMeridians { need: 12, have: 8 },
            "突破未成：需先打通 12 条正经（当前 8）。",
        ),
        (
            BreakthroughError::NotEnoughExtraordinaryMeridians { need: 4, have: 3 },
            "突破未成：需先打通 4 条奇经（当前 3）。",
        ),
        (
            BreakthroughError::NotEnoughQi {
                need: 100.0,
                have: 42.5,
            },
            "突破未成：真元不足（需 100.0，当前 42.5）。",
        ),
        (
            BreakthroughError::ZoneTooWeak {
                need: 0.8,
                have: 0.4,
            },
            "突破未成：此地灵气不足（需 0.80，当前 0.40）。",
        ),
        (
            BreakthroughError::EnvInsufficient {
                need: 0.7,
                have: 0.3,
                in_spirit_eye: true,
            },
            "突破未成：灵眼扰动未稳（需 0.70，当前 0.30）。",
        ),
        (
            BreakthroughError::EnvInsufficient {
                need: 0.7,
                have: 0.3,
                in_spirit_eye: false,
            },
            "突破未成：固元须在灵气浓处或灵眼内（需 0.70，当前 0.30）。",
        ),
        (
            BreakthroughError::LedgerUnavailable,
            "突破未成：真元账本未就绪，仪式暂缓。",
        ),
        (
            BreakthroughError::RolledFailure { severity: 0.75 },
            "突破失败：气机反噬，伤势强度 0.75。",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(
            breakthrough_error_message(&error),
            expected,
            "expected stable breakthrough error text for {error:?}"
        );
    }
}

#[test]
fn breakthrough_system_pushes_system_warning_on_precondition_error() {
    let mut app = App::new();
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(PendingGameplayNarrations::default());
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_systems(Update, breakthrough_system);
    let player = app
        .world_mut()
        .spawn((
            Cultivation::default(),
            MeridianSystem::default(),
            LifeRecord::default(),
            Username("Azure".to_string()),
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();

    let narrations = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert_eq!(narrations.len(), 1);
    assert_eq!(narrations[0].scope, NarrationScope::Player);
    assert_eq!(narrations[0].target.as_deref(), Some("Azure"));
    assert_eq!(narrations[0].style, NarrationStyle::SystemWarning);
    assert!(
        narrations[0].text.contains("突破未成"),
        "expected system warning to include breakthrough failure reason, actual text={}",
        narrations[0].text
    );
}

/// plan-race-system-v1 P6b review major-5 收口：production 级集成测试——真实
/// `Entity` + `Cultivation.race` + `BodyPlanRegistry`/`RaceRegistry` 资源驱动
/// `breakthrough_system`（不是直接调用 `try_breakthrough_with_profile`，那只锁得住
/// "传对了 profile 就能工作"，锁不住 `breakthrough_system` 内部 `meridian_profile_for_target`
/// 接线是否真的把 `req.entity` 解析到了自己的 race 而不是 humanoid）。
///
/// 合成非人构型 `test_breakthrough_synthetic_race` 的 Induce 门槛只需 1 条 channel
/// （humanoid 需要 3 条），只开 1 条 channel：若系统内部悄悄用了 humanoid 曲线，
/// 会因 `NotEnoughMeridians{need:3, have:1}` 拒绝；若真按自身构型解析，应该成功。
#[test]
fn breakthrough_system_uses_target_entity_own_race_profile_not_humanoid() {
    use crate::body_plan::race_registry::RaceEntry;
    use crate::body_plan::types::{
        BodyPartDef, ChannelDef, HeightBand, HeightBandAssignment, HitGeometry, MeridianFamily,
        MeridianProfile, PartConsequence, RealmMeridianReq, StandingAabbSpec,
    };
    use crate::body_plan::{BodyPlanId, BodyPlanRegistry, RaceId, RaceRegistry, HUMAN_RACE_ID};

    fn synthetic_race_plan() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: BodyPlanId::new("test_breakthrough_synthetic_race_plan"),
            display_name: "测试突破合成构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "body".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 2.0,
                    height: 3.0,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "body".into(),
                    },
                }],
                lateral_threshold: 0.5,
            },
            equip_slots: vec![],
            meridian_profile: Some(MeridianProfile {
                channels: vec![
                    ChannelDef {
                        id: "chan_a".into(),
                        family: MeridianFamily::Regular,
                        body_part: None,
                        roles: vec![],
                    },
                    ChannelDef {
                        id: "chan_b".into(),
                        family: MeridianFamily::Regular,
                        body_part: None,
                        roles: vec![],
                    },
                    ChannelDef {
                        id: "chan_c".into(),
                        family: MeridianFamily::Regular,
                        body_part: None,
                        roles: vec![],
                    },
                ],
                topology_edges: vec![],
                // Induce（index 1）只需 1 条——humanoid 同一档需要 3 条
                // （`humanoid.json realm_requirements[1] = {total:3, regular_min:3}`）。
                realm_requirements: [
                    RealmMeridianReq {
                        total: 0,
                        regular_min: 0,
                        extraordinary_min: 0,
                    },
                    RealmMeridianReq {
                        total: 1,
                        regular_min: 1,
                        extraordinary_min: 0,
                    },
                    RealmMeridianReq {
                        total: 2,
                        regular_min: 2,
                        extraordinary_min: 0,
                    },
                    RealmMeridianReq {
                        total: 3,
                        regular_min: 3,
                        extraordinary_min: 0,
                    },
                    RealmMeridianReq {
                        total: 3,
                        regular_min: 3,
                        extraordinary_min: 0,
                    },
                    RealmMeridianReq {
                        total: 3,
                        regular_min: 3,
                        extraordinary_min: 0,
                    },
                ],
                dugu_injection: vec![],
            }),
            mutation_slot_mapping: Default::default(),
        }
    }

    fn human_placeholder_plan() -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: BodyPlanId::new("test_breakthrough_human_placeholder_plan"),
            display_name: "测试人族占位构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "body".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 2.0,
                    height: 3.0,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "body".into(),
                    },
                }],
                lateral_threshold: 0.5,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: Default::default(),
        }
    }

    let body_plans =
        BodyPlanRegistry::from_plans(vec![synthetic_race_plan(), human_placeholder_plan()])
            .expect("synthetic race + human placeholder plans must validate");
    let races = RaceRegistry::from_parts_for_test(
        vec![
            RaceEntry {
                id: RaceId::new(HUMAN_RACE_ID),
                display_name: "人族".to_string(),
                body_plan_id: BodyPlanId::new("test_breakthrough_human_placeholder_plan"),
                beast_kinds: vec![],
            },
            RaceEntry {
                id: RaceId::new("test_breakthrough_synthetic_race"),
                display_name: "测试突破合成种族".to_string(),
                body_plan_id: BodyPlanId::new("test_breakthrough_synthetic_race_plan"),
                beast_kinds: vec![],
            },
        ],
        vec![],
        &body_plans,
    )
    .expect("synthetic race registry fixture must validate");

    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.insert_resource(WorldQiAccount::default());
    app.insert_resource(body_plans);
    app.insert_resource(races);
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let mut meridians =
        MeridianSystem::for_profile(synthetic_race_plan().meridian_profile.as_ref().unwrap());
    // 只打通 1 条 channel——humanoid 曲线（need=3）会拒绝，合成构型自己的曲线
    // （need=1）应该放行。
    meridians.regular[0].opened = true;
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 100.0,
        qi_max: 100.0,
        composure: 1.0,
        race: RaceId::new("test_breakthrough_synthetic_race"),
        ..Default::default()
    };
    let entity = app
        .world_mut()
        .spawn((
            cultivation,
            meridians,
            LifeRecord::new("synthetic_race_char"),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity,
        material_bonus: 0.0,
    });
    app.update();

    // Roll-independent assertion: `try_breakthrough_with_profile` debits qi cost
    // "不论成败"(win or lose the roll) the moment the precondition check passes —
    // so a qi debit here is decisive proof that `have=1 >= need` was evaluated
    // against the *synthetic race's own* curve (need=1), not the humanoid curve
    // (need=3, which this 1-channel-opened entity would fail and leave qi
    // untouched). Asserting the exact realm outcome would flakily depend on the
    // system's internal fixed-seed roll instead of the profile wiring itself.
    let cultivation = app.world().get::<Cultivation>(entity).unwrap();
    assert_eq!(
        cultivation.qi_current, 92.0,
        "breakthrough_system must resolve the target entity's own race profile (need=1) \
         through meridian_profile_for_target and attempt the breakthrough (debiting the 8.0 \
         qi cost), not silently fall back to the humanoid curve (need=3) which would reject \
         this 1-channel-opened entity outright and leave qi_current untouched at 100.0 — \
         actual qi_current after the attempt: {}",
        cultivation.qi_current
    );
}

#[test]
fn breakthrough_rejects_when_zone_qi_too_weak() {
    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.0;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let (mut cultivation, meridians) = setup_for_induce();
    cultivation.pending_material_bonus = 0.12;
    let player = app
        .world_mut()
        .spawn((
            cultivation,
            meridians,
            LifeRecord::default(),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();

    let outcomes = app.world().resource::<Events<BreakthroughOutcome>>();
    let outcome = outcomes.iter_current_update_events().next().unwrap();
    assert!(matches!(
        outcome.result,
        Err(BreakthroughError::ZoneTooWeak { .. })
    ));
    let cultivation = app.world().get::<Cultivation>(player).unwrap();
    assert_eq!(cultivation.qi_current, 100.0);
    assert!((cultivation.pending_material_bonus - 0.12).abs() < 1e-9);
}

#[test]
fn breakthrough_system_without_ledger_does_not_consume_qi() {
    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let (cultivation, meridians) = setup_for_induce();
    let player = app
        .world_mut()
        .spawn((
            cultivation,
            meridians,
            LifeRecord::new("player_a"),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();

    let outcomes = app.world().resource::<Events<BreakthroughOutcome>>();
    let outcome = outcomes.iter_current_update_events().next().unwrap();
    assert!(matches!(
        outcome.result,
        Err(BreakthroughError::LedgerUnavailable)
    ));
    let cultivation = app.world().get::<Cultivation>(player).unwrap();
    assert_eq!(cultivation.realm, Realm::Awaken);
    assert_eq!(cultivation.qi_current, 100.0);
}

#[test]
fn breakthrough_success_credits_cost_to_pending_inflow_pool_not_zone_ledger() {
    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let (cultivation, meridians) = setup_for_induce();
    let player = app
        .world_mut()
        .spawn((
            cultivation,
            meridians,
            LifeRecord::new("player_a"),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();

    let cultivation = app.world().get::<Cultivation>(player).unwrap();
    assert_eq!(
        cultivation.realm,
        Realm::Induce,
        "successful breakthrough should advance the player realm"
    );
    assert_eq!(
        cultivation.qi_current, 92.0,
        "successful breakthrough should spend exactly 8 qi from the player"
    );
    let ledger = app.world().resource::<WorldQiAccount>();
    let pending_pool = crate::qi_physics::pending_inflow_account();
    assert_eq!(
        ledger.balance(&pending_pool),
        8.0,
        "successful breakthrough should credit the spent 8 qi to the independent pending \
         inflow pool"
    );
    assert_eq!(
        ledger.balance(&QiAccountId::zone("spawn")),
        0.0,
        "successful breakthrough must never touch the zone:<name> ledger account (that key \
         is owned/overwritten wholesale by dormant regen from zone.spirit_qi)"
    );
    let transfer = ledger
        .transfers()
        .last()
        .expect("breakthrough should leave a QiTransfer audit");
    assert_eq!(
        transfer.reason,
        QiTransferReason::Breakthrough,
        "successful breakthrough audit should use the dedicated reason"
    );
    assert_eq!(
        transfer.from,
        QiAccountId::player("player_a"),
        "successful breakthrough audit should use stable player id as source"
    );
    assert_eq!(
        transfer.to, pending_pool,
        "successful breakthrough audit should target the independent pending inflow pool"
    );
    assert_eq!(
        transfer.amount, 8.0,
        "successful breakthrough audit amount should match spent qi"
    );
}

#[test]
fn breakthrough_and_meridian_open_preserve_total_observed_qi_conservation() {
    // plan-zone-qi-economy-v1 P0 §10.3 — 开脉→突破全链路总量不变的端到端守恒对拍。
    // total_observed() = player_qi + zone_qi + container_qi + ledger_qi（含待分配池）。
    // 消耗 → 待分配池等额升，player_qi 等额降，total_observed() 必须严格不变
    // （无天道时代衰减，era_decay=0）。
    use crate::qi_physics::{assert_conservation, summarize_world_qi};

    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let (cultivation, meridians) = setup_for_induce();
    let player = app
        .world_mut()
        .spawn((
            cultivation,
            meridians,
            LifeRecord::new("player_a"),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    let before = summarize_world_qi(app.world_mut());

    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();

    let cultivation = app.world().get::<Cultivation>(player).unwrap();
    assert_eq!(
        cultivation.realm,
        Realm::Induce,
        "sanity: breakthrough must actually succeed for this conservation test to be \
         meaningful (spent qi must leave the player)"
    );

    let after = summarize_world_qi(app.world_mut());
    assert_conservation(&before, &after, 0.0).unwrap_or_else(|error| {
        panic!(
            "breakthrough must conserve total_observed qi (player_qi + zone_qi + \
             container_qi + ledger_qi) with zero era decay — got drift: {error} \
             (before={before:?}, after={after:?}); a mismatch here means spent qi is \
             vanishing (not reaching the pending inflow pool) or being double-counted"
        )
    });
    assert!(
        (before.total_observed() - after.total_observed()).abs() < 1e-9,
        "explicit total_observed equality check (belt-and-suspenders alongside \
         assert_conservation): before={}, after={}",
        before.total_observed(),
        after.total_observed()
    );
}

#[test]
fn breakthrough_failure_also_credits_cost_to_pending_inflow_pool_not_zone_ledger() {
    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let (mut cultivation, meridians) = setup_for_induce();
    cultivation.composure = 0.0;
    let player = app
        .world_mut()
        .spawn((
            cultivation,
            meridians,
            LifeRecord::new("player_a"),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();

    let cultivation = app.world().get::<Cultivation>(player).unwrap();
    assert_eq!(
        cultivation.realm,
        Realm::Awaken,
        "failed breakthrough should keep the player in the original realm"
    );
    assert_eq!(
        cultivation.qi_current, 92.0,
        "failed breakthrough should still spend exactly 8 qi"
    );
    let ledger = app.world().resource::<WorldQiAccount>();
    let pending_pool = crate::qi_physics::pending_inflow_account();
    assert_eq!(
        ledger.balance(&pending_pool),
        8.0,
        "failed breakthrough should still credit spent qi to the independent pending \
         inflow pool"
    );
    assert_eq!(
        ledger.balance(&QiAccountId::zone("spawn")),
        0.0,
        "failed breakthrough must never touch the zone:<name> ledger account either"
    );
    let transfer = ledger
        .transfers()
        .last()
        .expect("failed breakthrough should leave a QiTransfer audit");
    assert_eq!(
        transfer.reason,
        QiTransferReason::Breakthrough,
        "failed breakthrough audit should use the dedicated reason"
    );
    assert_eq!(
        transfer.to, pending_pool,
        "failed breakthrough audit should target the independent pending inflow pool"
    );
    assert_eq!(
        transfer.amount, 8.0,
        "failed breakthrough audit amount should match spent qi"
    );
}

#[test]
fn npc_breakthrough_emits_vfx() {
    let mut app = App::new();
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(ZoneRegistry::fallback());
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let (cultivation, meridians) = setup_for_induce();
    let npc = app
        .world_mut()
        .spawn((
            cultivation,
            meridians,
            LifeRecord::new("npc_42v0"),
            Position::new([8.0, 66.0, 8.0]),
            NpcMarker,
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity: npc,
        material_bonus: 0.0,
    });
    app.update();

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let ids = vfx_events
        .iter_current_update_events()
        .filter_map(|event| match &event.payload {
            VfxEventPayloadV1::SpawnParticle { event_id, .. } => Some(event_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(ids.contains(&"bong:breakthrough_pillar"));
}

#[test]
fn breakthrough_fail_emits_vfx() {
    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let (mut cultivation, meridians) = setup_for_induce();
    cultivation.composure = 0.0;
    let player = app
        .world_mut()
        .spawn((
            cultivation,
            meridians,
            LifeRecord::default(),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();

    let events = app.world().resource::<Events<VfxEventRequest>>();
    let emitted = events
        .iter_current_update_events()
        .find(|event| {
            matches!(
                &event.payload,
                VfxEventPayloadV1::SpawnParticle { event_id, .. }
                    if event_id == gameplay_vfx::BREAKTHROUGH_FAIL
            )
        })
        .expect("rolled breakthrough failure should emit breakthrough_fail vfx");
    match &emitted.payload {
        VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
            assert_eq!(event_id, gameplay_vfx::BREAKTHROUGH_FAIL);
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

/// 确定性 control（review finding major-1）——1/tick 拆批：两条请求分属两个 Update，
/// 消费**连续**的 roll 值（r1 失败 → r2 必胜），Solidify→Spirit 拆批双连发收敛。
/// 这是修复前的必死路径：per-Update 重建 XorshiftRoll 时，两条请求各自消费 r1=0.8597…，
/// 而 Solidify→Spirit 顶到全态夏季也只有 0.693 < r1，永远过不去。
#[test]
fn breakthrough_roll_state_advances_across_updates_for_split_pair() {
    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let mut meridians = MeridianSystem::default();
    open_all_meridians(&mut meridians);
    let player = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Solidify,
                qi_current: 500.0,
                qi_max: 500.0,
                composure: 1.0,
                ..Default::default()
            },
            meridians,
            LifeRecord::new("player_a"),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    // tick N：只有第一条请求。r1=0.8597 > 全态夏季成功率 0.693 → 失败，境界停在 Solidify。
    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();
    let cultivation = app.world().get::<Cultivation>(player).unwrap();
    assert_eq!(
        cultivation.realm,
        Realm::Solidify,
        "1/tick 拆批首条请求消费 r1（高值）应失败，境界仍为 Solidify"
    );

    // tick N+1：第二条请求。roll 状态跨 Update 持久，消费 r2=0.3943 ≤ 成功率 → 成功。
    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();
    let cultivation = app.world().get::<Cultivation>(player).unwrap();
    assert_eq!(
        cultivation.realm,
        Realm::Spirit,
        "方案要求 1/tick 拆批的第二条请求消费连续 r2 并成功进阶 Spirit；若 roll 仍每 Update 重建（= 修复前），第二条请求会再消费 r1 而失败，境界停 Solidify"
    );
}

/// 确定性 control（review finding major-1）——同 Update 双连发：两条请求在同一 tick
/// 消费 r1（败）、r2（胜），Solidify→Spirit 收敛。锁住"连续消费"的原有语义。
#[test]
fn breakthrough_roll_state_advances_within_same_update_for_paired_requests() {
    let mut app = App::new();
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
    app.insert_resource(CultivationClock { tick: 10 });
    app.insert_resource(zones);
    app.insert_resource(WorldQiAccount::default());
    app.add_event::<BreakthroughRequest>();
    app.add_event::<BreakthroughOutcome>();
    app.add_event::<CultivationDeathTrigger>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<SkillCapChanged>();
    app.add_event::<SkillXpGain>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_systems(Update, breakthrough_system);

    let mut meridians = MeridianSystem::default();
    open_all_meridians(&mut meridians);
    let player = app
        .world_mut()
        .spawn((
            Cultivation {
                realm: Realm::Solidify,
                qi_current: 500.0,
                qi_max: 500.0,
                composure: 1.0,
                ..Default::default()
            },
            meridians,
            LifeRecord::new("player_b"),
            Position::new([8.0, 66.0, 8.0]),
        ))
        .id();

    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.world_mut().send_event(BreakthroughRequest {
        entity: player,
        material_bonus: 0.0,
    });
    app.update();

    let cultivation = app.world().get::<Cultivation>(player).unwrap();
    assert_eq!(
        cultivation.realm,
        Realm::Spirit,
        "同 Update 双连发应连续消费 r1(败)/r2(胜) 并进阶 Spirit；若 roll 不随每笔请求推进，两条请求都消费 r1 而双败，境界停在 Solidify"
    );
}

#[test]
fn guyuan_requires_high_qi_or_spirit_eye() {
    let mut zones = ZoneRegistry::fallback();
    zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.6;
    let position = Position::new([8.0, 66.0, 8.0]);

    let err = breakthrough_environment_error(
        &position,
        DimensionKind::Overworld,
        Some(&zones),
        None,
        Realm::Condense,
    )
    .expect("low qi outside spirit eye should reject guyuan");

    assert_eq!(
        err,
        BreakthroughError::EnvInsufficient {
            need: MIN_ZONE_QI_TO_GUYUAN,
            have: 0.6,
            in_spirit_eye: false,
        }
    );
}

#[test]
fn spirit_eye_bonus_is_gated_to_guyuan_breakthrough() {
    assert_eq!(
        spirit_eye_env_bonus_for(Realm::Condense, Some(false)),
        SPIRIT_EYE_BREAKTHROUGH_SUCCESS_BONUS
    );
    assert_eq!(
        spirit_eye_env_bonus_for(Realm::Condense, Some(true)),
        BLOOD_VALLEY_BREAKTHROUGH_SUCCESS_BONUS
    );
    assert_eq!(spirit_eye_env_bonus_for(Realm::Solidify, Some(false)), 0.0);
    assert_eq!(spirit_eye_env_bonus_for(Realm::Induce, Some(false)), 0.0);
    assert_eq!(spirit_eye_env_bonus_for(Realm::Condense, None), 0.0);
}

fn setup_rapid_breakthrough_karma_app(now: u64) -> App {
    let mut app = App::new();
    app.insert_resource(CultivationClock { tick: now });
    app.insert_resource(KarmaWeightStore::default());
    app.insert_resource(ZoneRegistry::fallback());
    app.add_event::<BreakthroughOutcome>();
    app.add_systems(Update, rapid_breakthrough_karma_mark_system);
    app
}

fn breakthrough_success_outcome(entity: Entity) -> BreakthroughOutcome {
    BreakthroughOutcome {
        entity,
        from: Realm::Awaken,
        result: Ok(BreakthroughSuccess {
            to: Realm::Induce,
            success_rate: 1.0,
            used_qi: 0.0,
        }),
    }
}

// ───────────────────────────────────────────────────────────────────────
// qi_max_frozen cap: 突破失败不能永久废人
// ───────────────────────────────────────────────────────────────────────

/// 单次失败：qi_max_frozen 精确加上 severity * FREEZE_FACTOR，且不超过 qi_max * 0.5。
#[test]
fn single_breakthrough_failure_freezes_qi_within_cap() {
    let (mut c, mut m) = setup_for_induce();
    // 强制失败：roll > base_success_rate(Induce)=0.90
    let err = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0))
        .expect_err("roll=1.0 must fail the 0.90 Induce breakthrough");

    // severity = (1.0 - success_rate).clamp(0.1, 0.9)
    // success_rate = base × composure × integrity × completeness = 0.90 × 1.0 × 1.0 × 1.0 = 0.90
    // severity = 0.10, freeze_add = 0.10 * FREEZE_FACTOR = 0.5
    // cap = qi_max(100.0) * 0.5 = 50.0 → 0.5 < 50.0, no clamping
    let severity = match err {
        BreakthroughError::RolledFailure { severity } => severity,
        other => panic!("expected rolled failure, got {other:?}"),
    };
    let expected = severity * FREEZE_FACTOR;
    let frozen = c
        .qi_max_frozen
        .expect("qi_max_frozen should be Some after failure");
    assert!(
        (frozen - expected).abs() < 1e-9,
        "期望 qi_max_frozen = severity({severity}) × factor({FREEZE_FACTOR}) = {expected}，实际 = {frozen}"
    );
    let effective = c.qi_max - frozen;
    assert!(
        effective > 0.0,
        "期望有效 qi_max > 0 以防玩家废人，实际 effective_qi_max = {effective}"
    );
}

/// 验证 cap 边界：pre-existing frozen 接近 cap 时，再叠一次不会超过 cap。
#[test]
fn breakthrough_failure_does_not_exceed_cap_when_already_near_cap() {
    let qi_max = 100.0;
    // 预填到接近 cap（48/100），单次 severity=0.9 的新增冻结量 4.5 会跨过 50 的 cap。
    let mut c = Cultivation {
        realm: Realm::Awaken,
        qi_current: qi_max,
        qi_max,
        composure: 0.0, // severity=0.9 → freeze_add=0.9×FREEZE_FACTOR=4.5，足够跨过 cap
        qi_max_frozen: Some(48.0),
        ..Default::default()
    };
    let mut m = MeridianSystem::default();
    open_regular(&mut m, 3);

    let _ = try_breakthrough(&mut c, &mut m, 0.0, &mut FixedRoll(1.0));

    let frozen = c
        .qi_max_frozen
        .expect("qi_max_frozen should be Some after failure");
    let cap = qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO;

    assert!(
        (frozen - cap).abs() < 1e-9,
        "期望 pre-existing frozen + severity×factor 跨越后精确 clamp 到 cap={cap}，实际 = {frozen}"
    );
    let effective = c.qi_max - frozen;
    assert!(
        (effective - qi_max * (1.0 - BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO)).abs() < 1e-9,
        "期望 cap 后有效 qi_max 精确为 {cap}，实际 = {effective}"
    );
}

/// 相同 severity 经真实突破失败与 overload event-reader 写入时必须得到相同冻结量。
#[test]
fn breakthrough_and_overload_share_freeze_factor() {
    let (mut breakthrough_cultivation, mut breakthrough_meridians) = setup_for_induce();
    let breakthrough_error = try_breakthrough(
        &mut breakthrough_cultivation,
        &mut breakthrough_meridians,
        0.0,
        &mut FixedRoll(1.0),
    )
    .expect_err("roll=1.0 must exercise the production breakthrough failure path");
    let severity = match breakthrough_error {
        BreakthroughError::RolledFailure { severity } => severity,
        other => panic!("expected rolled failure, got {other:?}"),
    };

    let mut overload_app = App::new();
    overload_app.insert_resource(CultivationClock { tick: 7 });
    overload_app.add_event::<overload::MeridianOverloadEvent>();
    overload_app.add_systems(Update, overload::apply_meridian_overload_events);
    let overload_entity = overload_app
        .world_mut()
        .spawn((
            Cultivation {
                qi_max: breakthrough_cultivation.qi_max,
                ..Default::default()
            },
            MeridianSystem::default(),
        ))
        .id();
    overload_app
        .world_mut()
        .send_event(overload::MeridianOverloadEvent {
            entity: overload_entity,
            severity,
        });
    overload_app.update();

    let overload_frozen = overload_app
        .world()
        .get::<Cultivation>(overload_entity)
        .and_then(|cultivation| cultivation.qi_max_frozen)
        .expect("overload event-reader must write qi_max_frozen");
    let breakthrough_frozen = breakthrough_cultivation
        .qi_max_frozen
        .expect("breakthrough failure must write qi_max_frozen");
    assert!(
        (breakthrough_frozen - overload_frozen).abs() < 1e-9,
        "same severity={severity} must freeze equally across breakthrough and overload: breakthrough={breakthrough_frozen}, overload={overload_frozen}"
    );
    assert!(
        (breakthrough_frozen - severity * FREEZE_FACTOR).abs() < 1e-9,
        "both production paths must use canonical factor={FREEZE_FACTOR}; severity={severity}, actual={breakthrough_frozen}"
    );
}
