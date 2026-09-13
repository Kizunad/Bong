#![allow(dead_code, unused_imports)]

use bong_server::combat::baomai_v3::state::BloodBurnActive;
use bong_server::combat::components::*;
use bong_server::combat::events::StatusEffectKind;
use bong_server::combat::woliu_v2::state::TurbulenceExposure;
use bong_server::combat::CombatClock;
use bong_server::cultivation::color::*;
use bong_server::cultivation::components::*;
use bong_server::cultivation::life_record::LifeRecord;
use bong_server::cultivation::lifespan::*;
use bong_server::cultivation::tick::*;
use bong_server::cultivation::tribulation::JueBiAftershockDebuff;
use bong_server::fauna::mundane::{MundaneFaunaKind, MundaneFaunaSpecies};
use bong_server::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use bong_server::npc::scenario::ScenarioNpc;
use bong_server::npc::spawn::NpcMarker;
use bong_server::player::state::canonical_player_id;
use bong_server::qi_physics::*;
use bong_server::schema::vfx_event::VfxEventPayloadV1;
use bong_server::world::dimension::*;
use bong_server::world::events::EVENT_REALM_COLLAPSE;
use bong_server::world::territory_perks::*;
use bong_server::world::zone::*;
use valence::prelude::*;

fn add_qi_regen_system(app: &mut App) {
    app.insert_resource(WorldQiAccount::default());
    app.add_systems(Update, qi_regen_and_zone_drain_tick);
}

fn blood_burn_until(active_until_tick: u64) -> BloodBurnActive {
    BloodBurnActive {
        started_at_tick: 0,
        active_until_tick,
        hp_burned: 0.0,
        qi_multiplier: 1.0,
        cooldown_until_tick: 0,
    }
}

fn run_qi_regen_once_with_scar_modifier(
    attrs: Option<DerivedAttrs>,
    blood_burn: Option<BloodBurnActive>,
    despawned: bool,
) -> (f64, f64, f64, usize) {
    run_qi_regen_once_with_scar_modifier_at_combat_tick(attrs, blood_burn, despawned, None)
}

fn run_qi_regen_once_with_scar_modifier_at_combat_tick(
    attrs: Option<DerivedAttrs>,
    blood_burn: Option<BloodBurnActive>,
    despawned: bool,
    combat_tick: Option<u64>,
) -> (f64, f64, f64, usize) {
    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    app.insert_resource(zone_with_spirit_qi(0.8));
    if let Some(tick) = combat_tick {
        app.insert_resource(CombatClock { tick });
    }
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let mut entity = app.world_mut().spawn((
        Position::new([8.0, 66.0, 8.0]),
        meridians,
        Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Cultivation::default()
        },
        LifeRecord::new("scar_qi_regen_test".to_string()),
    ));
    if let Some(attrs) = attrs {
        entity.insert(attrs);
    }
    if let Some(blood_burn) = blood_burn {
        entity.insert(blood_burn);
    }
    if despawned {
        entity.insert(Despawned);
    }
    let entity = entity.id();

    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    app.update();

    let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    let ledger = app.world().resource::<WorldQiAccount>();
    let transfer_amount = ledger.transfers().last().map(|t| t.amount).unwrap_or(0.0);
    (
        cultivation.qi_current,
        zone_before - zone_after,
        transfer_amount,
        ledger.transfers().len(),
    )
}

fn zone_with_spirit_qi(spirit_qi: f64) -> ZoneRegistry {
    let mut registry = ZoneRegistry::fallback();
    registry.zones[0].spirit_qi = spirit_qi;
    registry
}

fn zone_unit_qi() -> f64 {
    bong_server::qi_physics::constants::QI_ZONE_UNIT_CAPACITY
}

fn make_status_effects(effects: Vec<ActiveStatusEffect>) -> StatusEffects {
    StatusEffects { active: effects }
}

fn accel_effect(magnitude: f32, remaining_ticks: u64) -> ActiveStatusEffect {
    ActiveStatusEffect {
        kind: StatusEffectKind::CultivationAcceleration,
        magnitude,
        remaining_ticks,
        source_pill: None,
    }
}

fn slowed_effect(magnitude: f32, remaining_ticks: u64) -> ActiveStatusEffect {
    ActiveStatusEffect {
        kind: StatusEffectKind::QiRegenSlowed,
        magnitude,
        remaining_ticks,
        source_pill: None,
    }
}

fn boost_effect(magnitude: f32, remaining_ticks: u64) -> ActiveStatusEffect {
    ActiveStatusEffect {
        kind: StatusEffectKind::QiRegenBoost,
        magnitude,
        remaining_ticks,
        source_pill: None,
    }
}

#[test]
fn no_gain_in_dead_zone() {
    let (g, d) = compute_regen(0.0, 1.0, 1.0, 100.0);
    assert_eq!(g, 0.0);
    assert_eq!(d, 0.0);
}

#[test]
fn gain_drains_zone_by_ratio() {
    let (g, d) = compute_regen(0.5, 1.0, 1.0, 100.0);
    assert!(g > 0.0);
    assert!((g - d * zone_unit_qi()).abs() < 1e-9);
}

#[test]
fn qi_room_caps_gain() {
    let (g, d) = compute_regen(1.0, 100.0, 1.0, 0.5);
    assert!(g <= 0.5);
    // 即使被 qi_room 截断，drain 依然按 gain 换算
    assert!((g - d * zone_unit_qi()).abs() < 1e-9);
}

#[test]
fn drain_clamped_to_zone_available() {
    // rate 巨大会把 drain 推到超过 zone_qi
    let zone_qi = 0.001;
    let (g, d) = compute_regen(zone_qi, 1e6, 1.0, 1e9);
    assert!(d <= zone_qi + 1e-12);
    assert!((g - d * zone_unit_qi()).abs() < 1e-6);
}

#[test]
fn zero_sum_property() {
    // 多次 tick 后累积玩家 gain == 累积 zone drain × 底盘换算系数
    let mut zone_qi = 0.5;
    let mut player_qi = 0.0;
    for _ in 0..50 {
        let room = 1e9_f64;
        let (g, d) = compute_regen(zone_qi, 1.0, 1.0, room);
        player_qi += g;
        zone_qi -= d;
    }
    let leaked = player_qi - (0.5 - zone_qi) * zone_unit_qi();
    assert!(leaked.abs() < 1e-6);
}

#[test]
fn regen_migration_preserves_spirit_qi_total_budget() {
    let zone_before = 0.5;
    let player_before = 10.0;
    let reserve_qi = bong_server::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL
        - player_before
        - zone_before * zone_unit_qi();
    assert!(reserve_qi > 0.0);

    let (gain, drain) = compute_regen(zone_before, 1.0, 1.0, 100.0);
    let before_total = player_before + zone_before * zone_unit_qi() + reserve_qi;
    let after_total = (player_before + gain) + (zone_before - drain) * zone_unit_qi() + reserve_qi;

    assert!(
        (before_total - bong_server::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL).abs() < 1e-9
    );
    assert!((before_total - after_total).abs() < 1e-6);
}

#[test]
fn qi_regen_records_transfer_audit_without_mirroring_ledger_balance() {
    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    app.insert_resource(ZoneRegistry::fallback());
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let character_id = canonical_player_id("Audit");
    let entity = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new(character_id.clone()),
        ))
        .id();

    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    app.update();

    let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
    assert!(cultivation.qi_current > 0.0);
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    let ledger = app.world().resource::<WorldQiAccount>();
    let transfer = ledger
        .transfers()
        .last()
        .expect("qi regen must leave a QiTransfer audit trail");

    assert_eq!(transfer.from, QiAccountId::zone("spawn"));
    assert_eq!(transfer.to, QiAccountId::player(character_id));
    assert_eq!(transfer.reason, QiTransferReason::CultivationRegen);
    assert!((transfer.amount - cultivation.qi_current).abs() < 1e-9);
    assert!((zone_before - zone_after - transfer.amount / zone_unit_qi()).abs() < 1e-9);
    assert_eq!(
        ledger.total(),
        0.0,
        "玩家与 zone 活余额已经在 ECS/ZoneRegistry 中，ledger 只能 audit-only，不能镜像双计"
    );
}

#[test]
fn scar_qi_regen_multiplier_increases_gain_and_preserves_ledger_conservation() {
    let (base_gain, base_drain, base_transfer, _) =
        run_qi_regen_once_with_scar_modifier(None, None, false);
    let (boost_gain, boost_drain, boost_transfer, _) = run_qi_regen_once_with_scar_modifier(
        Some(DerivedAttrs {
            qi_regen_multiplier: 2.0,
            ..DerivedAttrs::default()
        }),
        Some(blood_burn_until(10)),
        false,
    );

    assert!(base_gain > 0.0, "baseline regen should produce positive qi");
    assert!(
        (boost_gain - base_gain * 2.0).abs() < 1e-9,
        "心肺短路 + 焚血应只把 rate 放大 2 倍；base={base_gain}, boosted={boost_gain}"
    );
    assert!((boost_transfer - boost_gain).abs() < 1e-9);
    assert!((base_transfer - base_gain).abs() < 1e-9);
    assert!(
        (boost_gain - boost_drain * zone_unit_qi()).abs() < 1e-9,
        "boosted gain 必须等于 zone drain × QI_ZONE_UNIT_CAPACITY"
    );
    assert!(
        (base_gain - base_drain * zone_unit_qi()).abs() < 1e-9,
        "baseline gain 必须等于 zone drain × QI_ZONE_UNIT_CAPACITY"
    );
}

#[test]
fn scar_qi_regen_uses_combat_clock_for_blood_burn_expiry() {
    let (base_gain, _, _, _) = run_qi_regen_once_with_scar_modifier(None, None, false);
    let (boost_gain, _, _, _) = run_qi_regen_once_with_scar_modifier_at_combat_tick(
        Some(DerivedAttrs {
            qi_regen_multiplier: 2.0,
            ..DerivedAttrs::default()
        }),
        Some(blood_burn_until(10)),
        false,
        Some(5),
    );
    let (expired_gain, _, _, _) = run_qi_regen_once_with_scar_modifier_at_combat_tick(
        Some(DerivedAttrs {
            qi_regen_multiplier: 2.0,
            ..DerivedAttrs::default()
        }),
        Some(blood_burn_until(10)),
        false,
        Some(10),
    );

    assert!(
        (boost_gain - base_gain * 2.0).abs() < 1e-9,
        "CombatClock.tick=5 < active_until_tick=10 时心肺短路倍率应生效"
    );
    assert!(
        (expired_gain - base_gain).abs() < 1e-9,
        "CombatClock.tick=10 到期时，即使 CultivationClock 刚推进到 1，也不能继续回气加成"
    );
}

#[test]
fn scar_qi_regen_multiplier_is_neutral_after_blood_burn_expiry_or_relogin_default_attrs() {
    let (base_gain, _, _, _) = run_qi_regen_once_with_scar_modifier(None, None, false);
    let (expired_gain, _, _, _) = run_qi_regen_once_with_scar_modifier(
        Some(DerivedAttrs {
            qi_regen_multiplier: 2.0,
            ..DerivedAttrs::default()
        }),
        Some(blood_burn_until(1)),
        false,
    );
    let (relogin_default_gain, _, _, _) = run_qi_regen_once_with_scar_modifier(
        Some(DerivedAttrs::default()),
        Some(blood_burn_until(10)),
        false,
    );

    assert!(
        (expired_gain - base_gain).abs() < 1e-9,
        "焚血到期后旧 DerivedAttrs.qi_regen_multiplier 不能继续影响回气"
    );
    assert!(
        (relogin_default_gain - base_gain).abs() < 1e-9,
        "重登/新挂载的默认 DerivedAttrs 必须保持中性回气"
    );
}

#[test]
fn qi_regen_skips_despawned_offline_cultivators() {
    let (gain, drain, transfer, transfer_count) = run_qi_regen_once_with_scar_modifier(
        Some(DerivedAttrs {
            qi_regen_multiplier: 2.0,
            ..DerivedAttrs::default()
        }),
        Some(blood_burn_until(10)),
        true,
    );

    assert_eq!(gain, 0.0, "离线 Despawned 修士不应继续吸收 zone 灵气");
    assert_eq!(drain, 0.0, "离线 Despawned 修士不应扣减 zone.spirit_qi");
    assert_eq!(transfer, 0.0, "离线 Despawned 修士不应留下回气 transfer");
    assert_eq!(transfer_count, 0, "离线 Despawned 修士不应产生回气 audit");
}

#[test]
fn qi_regen_skips_state_mutation_when_world_qi_account_missing() {
    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    app.insert_resource(ZoneRegistry::fallback());
    app.add_systems(Update, qi_regen_and_zone_drain_tick);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let entity = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
        ))
        .id();

    let zone_before = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;

    app.update();

    let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert_eq!(cultivation.qi_current, 0.0);
    assert_eq!(zone_after, zone_before);
}

#[test]
fn qi_regen_audits_npc_cultivators_as_npc_accounts() {
    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    app.insert_resource(ZoneRegistry::fallback());
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let npc_id = "npc_7v0".to_string();
    app.world_mut().spawn((
        Position::new([8.0, 66.0, 8.0]),
        meridians,
        Cultivation::default(),
        LifeRecord::new(npc_id.clone()),
        NpcMarker,
    ));

    app.update();

    let ledger = app.world().resource::<WorldQiAccount>();
    let transfer = ledger
        .transfers()
        .last()
        .expect("npc qi regen must leave a QiTransfer audit trail");
    assert_eq!(transfer.to, QiAccountId::npc(npc_id));
}

/// plan-zone-qi-economy-v1 P2：NPC 地板红线——zone_qi 在 (0, QI_NPC_ABSORB_FLOOR] 时
/// NPC（NpcMarker）必须完全放弃吸取，zone 与 NPC qi_current 都不应变化。
#[test]
fn qi_regen_npc_stops_at_or_below_absorb_floor() {
    let floor = bong_server::qi_physics::constants::QI_NPC_ABSORB_FLOOR;
    for zone_qi in [floor, 0.2, 0.05, 0.0] {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(zone_with_spirit_qi(zone_qi));
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let entity = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
                LifeRecord::new("npc_floor_test".to_string()),
                NpcMarker,
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert_eq!(
            cultivation.qi_current, 0.0,
            "zone_qi={zone_qi} 时 NPC 不应吸到任何 qi（地板 {floor}）"
        );
        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;
        assert_eq!(
            zone_after, zone_qi,
            "zone_qi={zone_qi} 时 zone.spirit_qi 不应被 NPC regen 触碰"
        );
    }
}

/// plan-mundane-fauna-v1 守恒红线：凡兽（`MundaneFaunaSpecies`）无灵，即便携带 live
/// `Cultivation`（qi_max>0 满足 hunt 猎物契约）+ 已开经脉，在**富灵区**（远高于 NPC 地板，
/// 普通 NPC 必吸）也**绝不**吸 zone 灵气：`qi_regen_and_zone_drain_tick` 的
/// `Without<MundaneFaunaSpecies>` 把凡兽整体挡在真元吐纳外。否则凡兽只进不出地抽 zone、
/// 死亡/回收时 qi_current 蒸发破守恒（plan §59 + mundane.rs 模块 doc 承诺）。
#[test]
fn qi_regen_excludes_mundane_fauna_even_in_rich_zone() {
    use bong_server::fauna::mundane::{MundaneFaunaKind, MundaneFaunaSpecies};

    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    // 0.8 富灵区，远高于 QI_NPC_ABSORB_FLOOR——普通 NPC 在此必吸。
    app.insert_resource(zone_with_spirit_qi(0.8));
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    // 凡兽携带非零 qi_max（复刻 npc_runtime_bundle_with_age(Awaken) 产出）+ NpcMarker +
    // MundaneFaunaSpecies，唯一区别于普通 NPC 的就是这枚物种标记。
    let fauna = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation {
                qi_max: 10.0,
                ..Cultivation::default()
            },
            LifeRecord::new("mundane_rabbit_test".to_string()),
            NpcMarker,
            MundaneFaunaSpecies(MundaneFaunaKind::Rabbit),
        ))
        .id();

    app.update();

    let cultivation = app.world().entity(fauna).get::<Cultivation>().unwrap();
    assert_eq!(
        cultivation.qi_current, 0.0,
        "凡兽无灵：富灵区 qi_regen 后 qi_current 仍应为 0（Without<MundaneFaunaSpecies> 排除）"
    );
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert_eq!(
        zone_after, 0.8,
        "凡兽不吸灵气：zone.spirit_qi 必须原样保持 0.8，一分都不被凡兽抽走（守恒）"
    );
}

#[test]
fn qi_regen_excludes_transient_scenario_npcs_even_in_rich_zone() {
    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    app.insert_resource(zone_with_spirit_qi(0.8));
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let npc = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation {
                qi_max: 10.0,
                ..Cultivation::default()
            },
            LifeRecord::new("scenario_regen_test"),
            NpcMarker,
            ScenarioNpc,
        ))
        .id();

    app.update();

    assert_eq!(
        app.world().get::<Cultivation>(npc).unwrap().qi_current,
        0.0,
        "transient scenario NPC must not acquire qi that its clear path then has to settle"
    );
    assert_eq!(
        app.world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi,
        0.8,
        "scenario NPC regen exclusion must leave zone qi unchanged"
    );
    assert!(
        app.world()
            .resource::<WorldQiAccount>()
            .transfers()
            .is_empty(),
        "excluded scenario NPC must not leave a false regen audit"
    );
}

/// 玩家（无 NpcMarker）不受 NPC 地板约束——zone_qi 低于地板时玩家仍可继续吸取
/// 直到 zone_qi 真正见底（0.0），验证"玩家吸取不受地板"红线。
#[test]
fn qi_regen_player_not_limited_by_npc_absorb_floor() {
    let floor = bong_server::qi_physics::constants::QI_NPC_ABSORB_FLOOR;
    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    // zone_qi 故意设在地板以下（NPC 会被完全挡住的区间）。
    app.insert_resource(zone_with_spirit_qi(floor - 0.1));
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let entity = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new(canonical_player_id("FloorImmunePlayer")),
        ))
        .id();

    app.update();

    let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
    assert!(
        cultivation.qi_current > 0.0,
        "玩家吸取不受 NPC 地板约束，zone_qi={} (< floor {floor}) 时仍应吸到正 qi，实际 {}",
        floor - 0.1,
        cultivation.qi_current
    );
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert!(
        zone_after < floor - 0.1,
        "玩家吸取必须真实扣减 zone_qi，即便已低于 NPC 地板；zone_after={zone_after}"
    );
}

/// 边界回归：zone_qi 略高于地板时 NPC 仍可吸取，但写回后必须 >= 地板——单 tick 的
/// 微量 drain 远小于 0.31-0.3 的余量，不会把 zone 拉穿地板。
#[test]
fn qi_regen_npc_never_dips_zone_below_absorb_floor() {
    let floor = bong_server::qi_physics::constants::QI_NPC_ABSORB_FLOOR;
    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    app.insert_resource(zone_with_spirit_qi(floor + 0.01));
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    app.world_mut().spawn((
        Position::new([8.0, 66.0, 8.0]),
        meridians,
        Cultivation::default(),
        LifeRecord::new("npc_boundary_test".to_string()),
        NpcMarker,
    ));

    app.update();

    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name("spawn")
        .unwrap()
        .spirit_qi;
    assert!(
        zone_after >= floor,
        "zone_qi 写回后 {zone_after} 不应低于地板 {floor}"
    );
}

#[test]
fn integrity_scales_gain() {
    let (g_full, _) = compute_regen(0.5, 1.0, 1.0, 1e9);
    let (g_half, _) = compute_regen(0.5, 1.0, 0.5, 1e9);
    assert!((g_half - g_full * 0.5).abs() < 1e-9);
}

#[test]
fn wind_candle_applies_realm_specific_qi_regen_penalty() {
    fn run_once(lifespan: LifespanComponent) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let entity = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
                LifeRecord::new("wind-candle-test".to_string()),
                lifespan,
            ))
            .id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let normal_qi = run_once(LifespanComponent::new(LifespanCapTable::MORTAL));
    let mut wind_candle_lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
    wind_candle_lifespan.years_lived = 73.0;
    let wind_candle_qi = run_once(wind_candle_lifespan);

    assert!(normal_qi > 0.0);
    assert!((wind_candle_qi - normal_qi * 0.7).abs() < 1e-6);
}

/// 虚脱 debuff（StatusEffectKind::Exhausted, magnitude=0.5）使 qi 回复减半。
/// 锁住「转 debuff 后 qi 回复守恒」：与旧游离 Exhausted 组件 ×0.5 结果一致。
#[test]
fn exhausted_qi_recovery_is_halved() {
    fn run_once(exhausted: Option<StatusEffects>) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let mut entity = app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new("exhausted-qi-test".to_string()),
        ));
        if let Some(state) = exhausted {
            entity.insert(state);
        }
        let entity = entity.id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let normal_qi = run_once(None);
    let exhausted_qi = run_once(Some(StatusEffects {
        active: vec![ActiveStatusEffect {
            kind: StatusEffectKind::Exhausted,
            magnitude: 0.5,
            remaining_ticks: 200,
            source_pill: None,
        }],
    }));

    assert!(normal_qi > 0.0);
    assert!(
        (exhausted_qi - normal_qi * 0.5).abs() < 1e-6,
        "虚脱 debuff(mag=0.5) 应使 qi 回复减半；normal={normal_qi}, exhausted={exhausted_qi}"
    );
}

#[test]
fn juebi_aftershock_debuff_halves_qi_recovery_until_expired() {
    fn run_once(aftershock: Option<JueBiAftershockDebuff>) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let mut entity = app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new("juebi-aftershock-test".to_string()),
        ));
        if let Some(debuff) = aftershock {
            entity.insert(debuff);
        }
        let entity = entity.id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let normal_qi = run_once(None);
    let debuffed_qi = run_once(Some(JueBiAftershockDebuff {
        until_tick: 100,
        rhythm_multiplier: 0.5,
    }));
    let expired_qi = run_once(Some(JueBiAftershockDebuff {
        until_tick: 0,
        rhythm_multiplier: 0.5,
    }));

    assert!(normal_qi > 0.0);
    assert!((debuffed_qi - normal_qi * 0.5).abs() < 1e-6);
    assert!((expired_qi - normal_qi).abs() < 1e-6);
}

#[test]
fn turbulence_exposure_blocks_qi_regen() {
    fn run_once(
        turbulence: Option<bong_server::combat::woliu_v2::state::TurbulenceExposure>,
    ) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let mut entity = app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new("turbulence-exposure-test".to_string()),
        ));
        if let Some(exposure) = turbulence {
            entity.insert(exposure);
        }
        let entity = entity.id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let normal_qi = run_once(None);
    let turbulent_qi = run_once(Some(
        bong_server::combat::woliu_v2::state::TurbulenceExposure::new(Entity::from_raw(99), 1.0, 1),
    ));

    assert!(normal_qi > 0.0);
    assert_eq!(turbulent_qi, 0.0);
}

#[test]
fn qi_regen_paused_status_blocks_qi_recovery_until_expired() {
    fn run_once(remaining_ticks: Option<u64>) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let mut entity = app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new("qi-regen-paused-test".to_string()),
        ));
        if let Some(remaining_ticks) = remaining_ticks {
            entity.insert(StatusEffects {
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::QiRegenPaused,
                    magnitude: 1.0,
                    remaining_ticks,
                    source_pill: None,
                }],
            });
        }
        let entity = entity.id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let normal_qi = run_once(None);
    let paused_qi = run_once(Some(100));
    let expired_qi = run_once(Some(0));

    assert!(
        normal_qi > 0.0,
        "expected normal qi regen to be positive; actual={normal_qi}"
    );
    assert_eq!(
        paused_qi, 0.0,
        "expected QiRegenPaused with remaining ticks to block regen; actual={paused_qi}"
    );
    assert!(
            (expired_qi - normal_qi).abs() < 1e-6,
            "expected expired QiRegenPaused to restore normal regen; expected={normal_qi}, actual={expired_qi}"
        );
}

#[test]
fn meditate_emits_absorb_vfx() {
    let mut app = App::new();
    app.insert_resource(CultivationClock { tick: 39 });
    app.insert_resource(ZoneRegistry::fallback());
    app.add_event::<VfxEventRequest>();
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    app.world_mut().spawn((
        Position::new([8.0, 66.0, 8.0]),
        meridians,
        Cultivation::default(),
        LifeRecord::new("meditate-vfx-test".to_string()),
    ));

    app.update();

    let events = app.world().resource::<Events<VfxEventRequest>>();
    let emitted = events
        .iter_current_update_events()
        .next()
        .expect("qi regen tick should emit cultivation_absorb vfx");
    match &emitted.payload {
        bong_server::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
            assert_eq!(event_id, gameplay_vfx::CULTIVATION_ABSORB);
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn qi_regen_emits_session_practice_after_actual_regen_minute() {
    let mut app = App::new();
    app.insert_resource(CultivationClock {
        tick: CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE - 1,
    });
    app.insert_resource(CultivationSessionPracticeAccumulator::default());
    app.insert_resource(ZoneRegistry::fallback());
    app.add_event::<CultivationSessionPracticeEvent>();
    add_qi_regen_system(&mut app);
    app.add_systems(
        Update,
        record_cultivation_session_practice_events.after(qi_regen_and_zone_drain_tick),
    );

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let entity = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new("session-practice-test".to_string()),
            QiColor {
                main: ColorKind::Heavy,
                ..Default::default()
            },
            PracticeLog::default(),
        ))
        .id();

    app.update();
    let log = app.world().entity(entity).get::<PracticeLog>().unwrap();
    assert_eq!(log.weights.get(&ColorKind::Heavy).copied(), None);

    for _ in 0..CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE - 1 {
        app.update();
    }

    let log = app.world().entity(entity).get::<PracticeLog>().unwrap();
    // P0 起 record_cultivation_session_practice 应用 color_style_bonus：
    // active_color=Heavy，QiColor.main=Heavy → 主色匹配 → bonus=1.1（事半功倍，worldview §六.1）
    let expected = STYLE_PRACTICE_AMOUNT * 1.1;
    let actual = log.weights.get(&ColorKind::Heavy).copied().unwrap_or(0.0);
    assert!(
        (actual - expected).abs() < 1e-9,
        "期望 Heavy 主色匹配时 session practice amount={expected}（1.1x 事半功倍），实际={actual}"
    );
}

#[test]
fn collapsed_zone_blocks_qi_regen_even_with_stale_qi() {
    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    let mut zones = ZoneRegistry::fallback();
    let zone = zones.find_zone_mut("spawn").unwrap();
    zone.spirit_qi = 0.9;
    zone.active_events.push(EVENT_REALM_COLLAPSE.to_string());
    app.insert_resource(zones);
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let player = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
        ))
        .id();

    app.update();

    assert_eq!(
        app.world()
            .entity(player)
            .get::<Cultivation>()
            .unwrap()
            .qi_current,
        0.0
    );
    assert_eq!(
        app.world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi,
        0.0
    );
}

#[test]
fn qi_regen_paused_short_circuits_slowed() {
    fn run_once(effects: Vec<ActiveStatusEffect>) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let entity = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
                StatusEffects { active: effects },
            ))
            .id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let paused_only = run_once(vec![ActiveStatusEffect {
        kind: StatusEffectKind::QiRegenPaused,
        magnitude: 1.0,
        remaining_ticks: 100,
        source_pill: None,
    }]);
    let paused_plus_slowed = run_once(vec![
        ActiveStatusEffect {
            kind: StatusEffectKind::QiRegenPaused,
            magnitude: 1.0,
            remaining_ticks: 100,
            source_pill: None,
        },
        slowed_effect(0.5, 100),
    ]);

    assert_eq!(paused_only, 0.0, "QiRegenPaused 独立时 qi 应为 0");
    assert_eq!(
        paused_plus_slowed, 0.0,
        "QiRegenPaused=0 短路 QiRegenSlowed(0.5)，qi 应仍为 0；实际 {paused_plus_slowed}"
    );
}

#[test]
fn qi_regen_doubled_under_cultivation_acceleration_mag_one() {
    fn run_once(effects: Option<StatusEffects>) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let mut entity = app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new("cultivation-acceleration-test".to_string()),
        ));
        if let Some(se) = effects {
            entity.insert(se);
        }
        let entity = entity.id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let normal_qi = run_once(None);
    let accel_qi = run_once(Some(make_status_effects(vec![accel_effect(1.0, 100)])));

    assert!(normal_qi > 0.0, "基线 qi 回复应为正；实际 {normal_qi}");
    assert!(
        (accel_qi - normal_qi * 2.0).abs() < 1e-6,
        "CultivationAcceleration(mag=1.0) 应使 qi 回复 2×；\
             期望 {:.6}，实际 {:.6}",
        normal_qi * 2.0,
        accel_qi
    );
}

#[test]
fn dominance_regen_store_multiplier_applied_in_qi_tick() {
    use bong_server::cultivation::components::MeridianId;
    use bong_server::world::territory_perks::{
        ZoneDominanceRegenStore, DOMINANCE_REGEN_MULTIPLIER,
    };
    use bong_server::world::zone::ZoneRegistry;

    fn run_once(inject_dominance_store: bool) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());

        if inject_dominance_store {
            let mut store = ZoneDominanceRegenStore::default();
            // fallback zone name は "spawn"
            store
                .multipliers
                .insert("spawn".to_string(), DOMINANCE_REGEN_MULTIPLIER);
            app.insert_resource(store);
        }

        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let entity = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                meridians,
                Cultivation::default(),
                LifeRecord::new("dominance-regen-test".to_string()),
            ))
            .id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let baseline_qi = run_once(false);
    let dominant_qi = run_once(true);

    assert!(baseline_qi > 0.0, "基线 qi 回复应为正；实际 {baseline_qi}");
    assert!(
        dominant_qi > baseline_qi,
        "ZoneDominanceRegenStore(×{DOMINANCE_REGEN_MULTIPLIER}) 应使 qi 回复 > 基线；\
             baseline={baseline_qi:.9}, dominant={dominant_qi:.9}"
    );
    // 允许 1e-6 误差（qi_room 截断可能使实际倍率略小于 1.20）
    assert!(
        (dominant_qi - baseline_qi * DOMINANCE_REGEN_MULTIPLIER).abs() < 1e-6,
        "dominant qi 应 ≈ baseline × DOMINANCE_REGEN_MULTIPLIER({DOMINANCE_REGEN_MULTIPLIER})；\
             期望 {:.9}，实际 {dominant_qi:.9}",
        baseline_qi * DOMINANCE_REGEN_MULTIPLIER
    );
}

#[test]
fn dominance_regen_store_absent_does_not_affect_qi_tick() {
    use bong_server::cultivation::components::MeridianId;
    use bong_server::world::zone::ZoneRegistry;

    // 不注入 ZoneDominanceRegenStore → multiplier 应回落 1.0，qi 与基线一致
    let mut app_no_store = App::new();
    app_no_store.insert_resource(CultivationClock::default());
    app_no_store.insert_resource(ZoneRegistry::fallback());
    add_qi_regen_system(&mut app_no_store);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;
    let entity_no_store = app_no_store
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians.clone(),
            Cultivation::default(),
        ))
        .id();

    let mut app_empty_store = App::new();
    app_empty_store.insert_resource(CultivationClock::default());
    app_empty_store.insert_resource(ZoneRegistry::fallback());
    use bong_server::world::territory_perks::ZoneDominanceRegenStore;
    app_empty_store.insert_resource(ZoneDominanceRegenStore::default()); // 空 store，无 dominant
    add_qi_regen_system(&mut app_empty_store);

    let entity_empty_store = app_empty_store
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
        ))
        .id();

    app_no_store.update();
    app_empty_store.update();

    let qi_no_store = app_no_store
        .world()
        .entity(entity_no_store)
        .get::<Cultivation>()
        .unwrap()
        .qi_current;
    let qi_empty_store = app_empty_store
        .world()
        .entity(entity_empty_store)
        .get::<Cultivation>()
        .unwrap()
        .qi_current;

    assert!(
        (qi_no_store - qi_empty_store).abs() < 1e-9,
        "无 dominant zone 时 qi 回复应与无 store 时一致；\
             no_store={qi_no_store:.9}, empty_store={qi_empty_store:.9}"
    );
}

/// QiRegenBoost(mag=0.25) 应使真实 qi 回复速率 ≈ 1.25×。
#[test]
fn qi_regen_boosted_by_25_percent_under_qi_regen_boost_mag_quarter() {
    fn run_once(effects: Option<StatusEffects>) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let mut entity = app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
            LifeRecord::new("qi-regen-boost-test".to_string()),
        ));
        if let Some(se) = effects {
            entity.insert(se);
        }
        let entity = entity.id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let normal_qi = run_once(None);
    let boosted_qi = run_once(Some(make_status_effects(vec![boost_effect(0.25, 100)])));

    assert!(normal_qi > 0.0, "基线 qi 回复应为正；实际 {normal_qi}");
    assert!(
        (boosted_qi - normal_qi * 1.25).abs() < 1e-6,
        "QiRegenBoost(mag=0.25) 应使 qi 回复 1.25×；\
             期望 {:.6}，实际 {:.6}",
        normal_qi * 1.25,
        boosted_qi
    );
}

/// qi_max_multiplier < 1.0 时，qi_room 被正确截断，qi_current 不超过折损后上限。
#[test]
fn qi_cap_perm_minus_reduces_effective_max_via_derived_attrs() {
    use bong_server::combat::components::DerivedAttrs;

    let mut app = App::new();
    app.insert_resource(CultivationClock::default());
    app.insert_resource(ZoneRegistry::fallback());
    add_qi_regen_system(&mut app);

    let mut meridians = MeridianSystem::default();
    meridians.get_mut(MeridianId::Lung).opened = true;

    // qi_max=10.0（默认），qi_max_multiplier=0.5 → effective_max=5.0。
    // 设 qi_current=4.9（逼近折损后上限），验证下一 tick qi_current ≤ 5.0。
    let cultivation = Cultivation {
        qi_current: 4.9,
        qi_max: 10.0,
        ..Default::default()
    };
    let derived = DerivedAttrs {
        qi_max_multiplier: 0.5,
        ..Default::default()
    };

    let entity = app
        .world_mut()
        .spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            cultivation,
            derived,
        ))
        .id();

    app.update();

    let cult = app.world().entity(entity).get::<Cultivation>().unwrap();
    assert!(
        cult.qi_current <= 5.0 + 1e-9,
        "qi_max_multiplier=0.5 时 qi_current 不应超过 5.0（折损后上限）；\
             期望 ≤ 5.0，实际 {:.6}",
        cult.qi_current
    );
    assert!(
        cult.qi_current >= 4.9 - 1e-9,
        "qi_current 应从 4.9 增加或保持（zone 有灵气）；实际 {:.6}",
        cult.qi_current
    );
}

/// qi_max_multiplier=1.0（默认中性）时行为与无 DerivedAttrs 一致。
#[test]
fn qi_cap_perm_minus_multiplier_one_is_neutral() {
    use bong_server::combat::components::DerivedAttrs;

    fn run_once(with_derived: bool) -> f64 {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        add_qi_regen_system(&mut app);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let mut entity = app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            meridians,
            Cultivation::default(),
        ));
        if with_derived {
            // 中性 multiplier=1.0
            entity.insert(DerivedAttrs::default());
        }
        let entity = entity.id();

        app.update();

        app.world()
            .entity(entity)
            .get::<Cultivation>()
            .unwrap()
            .qi_current
    }

    let without_derived = run_once(false);
    let with_neutral_derived = run_once(true);

    assert!(
        (without_derived - with_neutral_derived).abs() < 1e-9,
        "qi_max_multiplier=1.0 应与无 DerivedAttrs 时 qi 回复一致；\
             without={without_derived:.9}, with_neutral={with_neutral_derived:.9}"
    );
}
