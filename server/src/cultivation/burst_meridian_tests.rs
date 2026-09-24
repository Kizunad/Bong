#![allow(dead_code, unused_imports)]

use super::*;
use valence::prelude::{App, DVec3, Events, Update};

fn spawn_caster(app: &mut App, realm: Realm, qi_current: f64, position: DVec3) -> Entity {
    let mut meridians = MeridianSystem::default();
    if let Some(registry) = app.world().get_resource::<TechniqueRegistry>() {
        for definition in registry.iter() {
            for required in &definition.required_meridians {
                let Some(id) = parse_meridian_id(&required.channel) else {
                    continue;
                };
                let meridian = meridians.get_mut(id);
                meridian.opened = true;
                meridian.integrity = 1.0;
                meridian.throughput_current = 1.0;
            }
        }
    }
    app.world_mut()
        .spawn((
            Cultivation {
                realm,
                qi_current,
                qi_max: 100.0,
                ..Default::default()
            },
            meridians,
            Position::new([position.x, position.y, position.z]),
            SkillBarBindings::default(),
            PracticeLog::default(),
        ))
        .id()
}

fn spawn_target(app: &mut App, position: DVec3) -> Entity {
    app.world_mut()
        .spawn(Position::new([position.x, position.y, position.z]))
        .id()
}

fn app() -> App {
    let mut app = App::new();
    app.insert_resource(TechniqueRegistry::load_for_tests());
    app.insert_resource(CombatClock { tick: 10 });
    app.add_event::<AttackIntent>();
    app.add_event::<BurstMeridianEvent>();
    app.add_event::<VfxEventRequest>();
    app
}

fn assert_no_mutation(app: &App, caster: Entity, qi: f64, integrity: f64) {
    assert_eq!(
        app.world().get::<Cultivation>(caster).unwrap().qi_current,
        qi
    );
    for id in RIGHT_ARM_MERIDIANS {
        assert_eq!(
            app.world()
                .get::<MeridianSystem>(caster)
                .unwrap()
                .get(id)
                .integrity,
            integrity
        );
    }
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app.world().resource::<Events<AttackIntent>>().is_empty());
    assert!(app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// 贴山靠 / 血崩步 / 逆脉护体 —— 实装 skeleton 招的饱和测试。
//   覆盖：① happy path（守恒 + 事件 + AV）② 拒绝分支（realm/qi/cooldown/meridian/
//   target/facing）③ 守恒（拒绝路径零突变、qi/integrity 精确数值）④ 状态转换
//   （Casting 插入、Position 位移、DamageReduction buff）。
// ─────────────────────────────────────────────────────────────────────────────

use valence::entity::Look;

/// 注册 3 招会用到的全部 event 资源（含 burst 共用的 ApplyStatusEffectIntent /
/// PlaySoundRecipeRequest）。
fn full_app() -> App {
    let mut app = app();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app
}

/// 带 Look 的 caster（血崩步突进需要朝向）。yaw=0 → facing = (0,0,1)。
fn spawn_caster_with_look(
    app: &mut App,
    realm: Realm,
    qi_current: f64,
    position: DVec3,
    yaw: f32,
) -> Entity {
    let caster = spawn_caster(app, realm, qi_current, position);
    app.world_mut()
        .entity_mut(caster)
        .insert(Look::new(yaw, 0.0));
    caster
}

fn meridian_integrity(app: &App, caster: Entity, id: MeridianId) -> f64 {
    app.world()
        .get::<MeridianSystem>(caster)
        .unwrap()
        .get(id)
        .integrity
}

fn qi(app: &App, caster: Entity) -> f64 {
    app.world().get::<Cultivation>(caster).unwrap().qi_current
}

fn sever_meridian(app: &mut App, caster: Entity, id: MeridianId) {
    app.world_mut()
        .entity_mut(caster)
        .insert(MeridianSeveredPermanent::default());
    let mut sev = app
        .world_mut()
        .get_mut::<MeridianSeveredPermanent>(caster)
        .unwrap();
    sev.insert(
        id,
        crate::cultivation::meridian::severed::SeveredSource::CombatWound,
        0,
    );
}

fn last_burst_skill(app: &App) -> &'static str {
    app.world()
        .resource::<Events<BurstMeridianEvent>>()
        .iter_current_update_events()
        .next()
        .expect("a BurstMeridianEvent should be emitted")
        .skill
}

fn emitted_audio_recipe(app: &App) -> Option<String> {
    app.world()
        .resource::<Events<PlaySoundRecipeRequest>>()
        .iter_current_update_events()
        .next()
        .map(|req| req.recipe_id.clone())
}

fn assert_single_meridian_untouched(app: &App, caster: Entity, id: MeridianId, qi_val: f64) {
    assert_eq!(qi(app, caster), qi_val, "qi must be untouched on rejection");
    assert_eq!(
        meridian_integrity(app, caster, id),
        1.0,
        "meridian must be untouched on rejection"
    );
    assert!(
        app.world().get::<Casting>(caster).is_none(),
        "no Casting must be inserted on rejection"
    );
    assert!(
        app.world()
            .resource::<Events<BurstMeridianEvent>>()
            .is_empty(),
        "no BurstMeridianEvent on rejection"
    );
}

// ─── 贴山靠（tie_shan_kao）─ Condense / Stomach≥0.5 / qi 35 / cd 70 ─────────────

// ─── 血崩步（xue_beng_bu）─ Condense / Gallbladder≥0.4 / qi 25 / dash 4 ─────────

// ─── 逆脉护体（ni_mai_hu_ti）─ Solidify / Pericardium≥0.55 / qi 45 / cd 120 ─────

// ─── plan-skill-anim-fidelity-v1 P3：专属动画 id pin（去复用 + 缺失补齐）────────

/// 收集本次 update 内发出的全部 PlayAnim anim_id。
fn emitted_play_anim_ids(app: &App) -> Vec<String> {
    app.world()
        .resource::<Events<VfxEventRequest>>()
        .iter_current_update_events()
        .filter_map(|request| match &request.payload {
            VfxEventPayloadV1::PlayAnim { anim_id, .. } => Some(anim_id.clone()),
            _ => None,
        })
        .collect()
}

// 事件路径 pin：三招 resolver 各自发出**专属** PlayAnim——tie_shan_kao 靠身
// 撞击 / xue_beng_bu 步法突进解除崩拳借用，ni_mai_hu_ti 从 `anim_id: None`
// 补齐护体结印动画；任何一招回退到 `bong:beng_quan`（旧借用）立即撞红。
// 崩拳本尊仍合法使用 `bong:beng_quan`，见 beng_quan 专属用例。

// 无 UniqueId（非玩家/无头）时 PlayAnim 分支静默跳过、粒子照发——
// 锁 emit_burst_av 的防御分支不因 P3 接线改变。
// ─── 跨招守恒 / 精度 / 边界 ───────────────────────────────────────────────────

// ─── 真元守恒：burst_meridian 招式扣 qi 后必须释放回区域 ────────────────────────
// 对齐 baomai_v3 / tuike_v2 的 emit_spent_qi_release 模式。
// resolve.rs 把 BurstMeridian 加入 source_uses_prepaid_qi 白名单，
// 意味着 combat resolver 不会补做任何区域释放——必须在 spend_qi 本地完成。
// ─────────────────────────────────────────────────────────────────────────────
use crate::qi_physics::{QiTransfer, QiTransferReason};
use crate::world::zone::ZoneRegistry;

/// 带 ZoneRegistry（fallback spawn zone，覆盖 y=70 层面）和 QiTransfer event 的测试 App。
/// caster 位置应使用 DVec3::new(0.0, 70.0, 0.0) 才落在 spawn zone AABB 内。
fn app_with_zone() -> App {
    let mut app = App::new();
    app.insert_resource(TechniqueRegistry::load_for_tests());
    app.insert_resource(CombatClock { tick: 10 });
    app.add_event::<AttackIntent>();
    app.add_event::<BurstMeridianEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_event::<QiTransfer>();
    app.insert_resource(ZoneRegistry::fallback());
    app
}

fn zone_spirit_qi(app: &App) -> f64 {
    app.world()
        .resource::<ZoneRegistry>()
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(0.0, 70.0, 0.0),
        )
        .expect("spawn zone must cover (0,70,0)")
        .spirit_qi
}

fn qi_transfer_count(app: &App) -> usize {
    app.world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .count()
}

// beng_quan 施法后区域 spirit_qi 必须升高（消耗的真元返还区域）。

// tie_shan_kao 施法后区域 spirit_qi 必须升高（守恒同 beng_quan）。

// xue_beng_bu 施法后区域 spirit_qi 必须升高。

// ni_mai_hu_ti 施法后区域 spirit_qi 必须升高。

// 拒绝路径（如真元不足）不触发任何区域释放——守恒不能凭空注入灵气。

// 区域已满（spirit_qi = 1.0）时，消耗的真元应路由至 overflow，zone 不超限。

// ─── plan-skill-anim-fidelity-v1 P5：粒子去复用回归锁 ─────────────────────────
//
// P3 解除了动画借用,粒子借用留到 P5：三招此前 100% 借崩拳
// `bong:burst_meridian_beng_quan`。与真脉相反,爆脉三招**共用**识别色 #C58B3F,
// 读招完全靠形态(GroundDecal 冲击环 / Ribbon 步法残影 / Sprite 体表逆流纹),
// 所以这里锁的是「id 全异 + 颜色全同」这一对方向相反的性质。

/// 收集本次 update 内发出的全部 SpawnParticle `(event_id, color)`。
fn emitted_particles(app: &App) -> Vec<(String, Option<String>)> {
    app.world()
        .resource::<Events<VfxEventRequest>>()
        .iter_current_update_events()
        .filter_map(|request| match &request.payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id, color, ..
            } => Some((event_id.clone(), color.clone())),
            _ => None,
        })
        .collect()
}

/// 逐招对拍共享接线表（`network::skill_vfx_wiring`,client 按同一份表注册）。
fn assert_emits_wired_particle(app: &App, skill_id: &str) {
    let wiring = crate::network::skill_vfx_wiring::wiring_for(skill_id)
        .unwrap_or_else(|| panic!("{skill_id} 未登记进 P5_SKILL_VFX_WIRING 接线表"));
    let particles = emitted_particles(app);
    assert_eq!(
        particles.len(),
        1,
        "{skill_id} 应恰好发 1 条 SpawnParticle,实际 {particles:?}"
    );
    assert_eq!(
        particles[0].0, wiring.event_id,
        "{skill_id} 发出的粒子 event_id 与接线表不符(client 按表注册,不符即 bridgeMiss 静默无特效)"
    );
    assert_eq!(
        particles[0].1.as_deref(),
        Some(wiring.color),
        "{skill_id} 的粒子颜色应为爆脉家族色 {}",
        wiring.color
    );
    assert_ne!(
        particles[0].0, wiring.legacy_event_id,
        "{skill_id} 回退到了 P5 之前借用的崩拳粒子 `{}`",
        wiring.legacy_event_id
    );
}

// ─── 逆脉护体「体表逆流纹」跟随施法者 ─────────────────────────────────────────
//
// 交付物是「体表」逆流纹：环必须贴着身体，而不是钉在施法瞬间的世界坐标上。
// 由于 `SpawnParticle` payload 无实体标识，跟随靠 server 在 buff 存续期内以施法者
// 当前 `Position` 周期重发短寿命环实现，下列用例逐个状态转换锁住这条链路。

/// `app()` 的初始 clock —— 下面所有相位算术都以它为 cast tick。
const CAST_TICK: u64 = 10;

/// 真跑 `ni_mai_hu_ti_aura_vfx_tick` 的取证 app。
fn aura_app() -> App {
    let mut app = full_app();
    app.add_systems(Update, ni_mai_hu_ti_aura_vfx_tick);
    app
}

fn clear_vfx_events(app: &mut App) {
    app.world_mut()
        .resource_mut::<Events<VfxEventRequest>>()
        .clear();
}

fn move_caster(app: &mut App, caster: Entity, position: DVec3) {
    app.world_mut()
        .entity_mut(caster)
        .insert(Position::new([position.x, position.y, position.z]));
}

/// 本帧发出的护体环 payload（已按 event_id 过滤）。
fn ni_mai_rings(app: &App) -> Vec<VfxEventPayloadV1> {
    app.world()
        .resource::<Events<VfxEventRequest>>()
        .iter_current_update_events()
        .filter(|request| {
            matches!(
                &request.payload,
                VfxEventPayloadV1::SpawnParticle { event_id, .. }
                    if event_id == NI_MAI_HU_TI_PARTICLE_ID
            )
        })
        .map(|request| request.payload.clone())
        .collect()
}

/// 把 clock 拨到 `tick` 跑一帧，返回本帧发出的护体环。
fn tick_aura(app: &mut App, tick: u64) -> Vec<VfxEventPayloadV1> {
    app.world_mut().resource_mut::<CombatClock>().tick = tick;
    app.update();
    ni_mai_rings(app)
}

fn ring_origin(ring: &VfxEventPayloadV1) -> [f64; 3] {
    match ring {
        VfxEventPayloadV1::SpawnParticle { origin, .. } => *origin,
        other => panic!("护体环应为 SpawnParticle，实际 {other:?}"),
    }
}

fn ring_duration(ring: &VfxEventPayloadV1) -> Option<u16> {
    match ring {
        VfxEventPayloadV1::SpawnParticle { duration_ticks, .. } => *duration_ticks,
        other => panic!("护体环应为 SpawnParticle，实际 {other:?}"),
    }
}

fn cast_ni_mai_hu_ti(app: &mut App, position: DVec3) -> Entity {
    let caster = spawn_caster(app, Realm::Solidify, 100.0, position);
    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
    assert!(
        matches!(result, CastResult::Started { .. }),
        "护体施放前置不满足，用例本身失效：{result:?}"
    );
    caster
}

// 施放即装上锚点，窗口 = cast tick + 60t（buff 常量），供重发系统判存续与到期。

// 首环不再用 60t 长寿命撑满窗口（那正是纹脱离身体的根因）。

// **核心回归锁**：窗口内施法者一路移动，每个重发环的圆心都必须落在其当时的位置。

// 重发相位固定，且「首环 + 各重发环」严丝合缝铺满 buff 窗口：不叠环、不留空窗。

// cast 同帧不得补环（`emit_burst_av` 已发首环），非相位 tick 同样静默。

// buff 到期 → 摘锚点 + 永久停发（否则纹会一直转下去）。

// 首环与重发环除 origin 外逐字段同形 —— 两条发射路径发散会让窗口中途的环突然换模样。

// 冷却后重新施放 → 窗口整体后移，锚点被覆盖而不是叠出第二套环。

#[test]
fn beng_quan_uses_overridden_runtime_metadata() {
    let mut app = app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        BENG_QUAN_SKILL_ID,
        |definition| {
            definition.qi_cost = 0.25;
            definition.range = 2.75;
            definition.cast_ticks = 13;
            definition.cooldown_ticks = 91;
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Induce, 80.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::LargeIntestine)
        .integrity = 0.7;
    let target = spawn_target(&mut app, DVec3::new(2.5, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 91,
            anim_duration_ticks: 13,
        }
    );
    assert!((qi(&app, caster) - 60.0).abs() < 1e-9);
    assert_eq!(
        app.world().get::<Casting>(caster).unwrap().duration_ticks,
        13
    );
    let attack = app
        .world()
        .resource::<Events<AttackIntent>>()
        .iter_current_update_events()
        .next()
        .expect("崩拳成功必须发 AttackIntent");
    assert_eq!(
        attack.reach, FIST_REACH,
        "metadata range override 只影响展示，resolver 必须保留旧版 2.6 有效距离与衰减基准"
    );
}

#[test]
fn beng_quan_uses_overridden_runtime_race_gate_without_mutation() {
    let mut app = app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        BENG_QUAN_SKILL_ID,
        |definition| {
            definition.required_race = crate::body_plan::RaceGateOwned::Species {
                species: vec![crate::body_plan::RaceId::new("whale")],
            };
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Induce, 80.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        CastResult::Rejected {
            reason: CastRejectReason::RaceMismatch,
        }
    );
    assert_no_mutation(&app, caster, 80.0, 1.0);
}

#[test]
fn beng_quan_rejects_when_all_meridians_miss_overridden_health_threshold() {
    let mut app = app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        BENG_QUAN_SKILL_ID,
        |definition| {
            for required in &mut definition.required_meridians {
                required.min_health = 0.8;
            }
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    for id in RIGHT_ARM_MERIDIANS {
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(id)
            .integrity = 0.7;
    }
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(
            MeridianId::LargeIntestine
        )))
    );
    assert_no_mutation(&app, caster, 100.0, 0.7);
}

#[test]
fn beng_quan_happy_path_mutates_atomically_and_emits_events() {
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(2.6, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 60,
            anim_duration_ticks: 8,
        }
    );
    assert_eq!(
        app.world().get::<Cultivation>(caster).unwrap().qi_current,
        60.0
    );
    for id in RIGHT_ARM_MERIDIANS {
        assert_eq!(
            app.world()
                .get::<MeridianSystem>(caster)
                .unwrap()
                .get(id)
                .integrity,
            0.7
        );
    }
    assert_eq!(
        app.world().get::<Casting>(caster).unwrap().duration_ticks,
        8
    );
    assert_eq!(
        app.world()
            .get::<PracticeLog>(caster)
            .unwrap()
            .weights
            .get(&ColorKind::Heavy)
            .copied(),
        Some(crate::cultivation::color::STYLE_PRACTICE_AMOUNT)
    );

    let attack_events = app.world().resource::<Events<AttackIntent>>();
    let attack = attack_events.iter_current_update_events().next().unwrap();
    assert_eq!(attack.target, Some(target));
    assert_eq!(attack.source, AttackSource::BurstMeridian);
    assert_eq!(
        attack.reach, FIST_REACH,
        "2.6 reach must retain base 2.0 and step bonus 0.6"
    );
    assert_eq!(attack.qi_invest, 60.0);
    assert_eq!(attack.wound_kind, WoundKind::Blunt);

    let burst_events = app.world().resource::<Events<BurstMeridianEvent>>();
    let burst = burst_events.iter_current_update_events().next().unwrap();
    assert_eq!(burst.skill, BENG_QUAN_EVENT_SKILL);
    assert_eq!(burst.target, Some(target));
    assert_eq!(burst.overload_ratio, 1.5);
    assert_eq!(burst.integrity_snapshot, 1.0);

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let vfx: Vec<_> = vfx_events.iter_current_update_events().collect();
    assert_eq!(vfx.len(), 1);
    match &vfx[0].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id, color, ..
        } => {
            assert_eq!(event_id, BENG_QUAN_PARTICLE_ID);
            assert_eq!(color.as_deref(), Some("#C58B3F"));
        }
        other => panic!("expected beng_quan particle, got {other:?}"),
    }
}

#[test]
fn tie_shan_kao_uses_overridden_runtime_metadata() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        TIE_SHAN_KAO_SKILL_ID,
        |definition| {
            definition.required_realm = "Awaken".to_string();
            definition.qi_cost = 7.5;
            definition.range = 4.0;
            definition.cast_ticks = 13;
            definition.cooldown_ticks = 91;
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Awaken, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(3.5, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 91,
            anim_duration_ticks: 13,
        }
    );
    assert!((qi(&app, caster) - 92.5).abs() < 1e-9);
    assert_eq!(
        app.world().get::<Casting>(caster).unwrap().duration_ticks,
        13
    );
    assert_eq!(
        app.world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .next()
            .unwrap()
            .reach
            .max,
        4.0
    );
}

#[test]
fn tie_shan_kao_uses_overridden_runtime_race_gate_without_mutation() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        TIE_SHAN_KAO_SKILL_ID,
        |definition| {
            definition.required_race = crate::body_plan::RaceGateOwned::Species {
                species: vec![crate::body_plan::RaceId::new("whale")],
            };
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        rejected(CastRejectReason::RaceMismatch),
        "M19: race gate must follow definition.required_race override"
    );
    assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Stomach),
        1.0,
        "Stomach must be untouched on rejection"
    );
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .is_empty());
}

#[test]
fn tie_shan_kao_tears_first_overridden_meridian_not_hard_coded_stomach() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        TIE_SHAN_KAO_SKILL_ID,
        |definition| {
            definition.required_meridians = vec![
                TechniqueRequiredMeridian {
                    channel: "Lung".to_string(),
                    min_health: 0.5,
                },
                TechniqueRequiredMeridian {
                    channel: "Stomach".to_string(),
                    min_health: 0.5,
                },
            ];
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 70,
            anim_duration_ticks: 10,
        }
    );
    assert_eq!(qi(&app, caster), 65.0, "100 - 35 flat qi_cost");
    assert!(
        (meridian_integrity(&app, caster, MeridianId::Lung) - 0.8).abs() < 1e-12,
        "M19: torn meridian must follow the FIRST registry required_meridian (Lung -> 0.8)"
    );
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Stomach),
        1.0,
        "Stomach must NOT be torn even though present in required_meridians"
    );
}

#[test]
fn tie_shan_kao_rejects_closed_overridden_meridian_without_mutation() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        TIE_SHAN_KAO_SKILL_ID,
        |definition| {
            definition.required_meridians = vec![TechniqueRequiredMeridian {
                channel: "Lung".to_string(),
                min_health: 0.5,
            }];
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::Lung)
        .opened = false;
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Lung))),
        "M19: meridian gate must report the registry-overridden meridian, not the legacy default"
    );
    assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Lung),
        1.0,
        "Lung must be untouched on rejection"
    );
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .is_empty());
}

#[test]
fn xue_beng_bu_uses_overridden_runtime_metadata() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        XUE_BENG_BU_SKILL_ID,
        |definition| {
            definition.required_realm = "Awaken".to_string();
            definition.qi_cost = 6.25;
            definition.range = 7.5;
            definition.cast_ticks = 11;
            definition.cooldown_ticks = 89;
            definition.stamina_cost = 3.0;
        },
    ));
    let caster = spawn_caster_with_look(&mut app, Realm::Awaken, 100.0, DVec3::ZERO, 0.0);
    app.world_mut().entity_mut(caster).insert(Stamina {
        current: 20.0,
        max: 20.0,
        recover_per_sec: 1.0,
        last_drain_tick: None,
        state: StaminaState::Idle,
    });

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 89,
            anim_duration_ticks: 11,
        }
    );
    assert!((qi(&app, caster) - 93.75).abs() < 1e-9);
    // M31：stamina_cost=3.0 已扣除（20 → 17），并打了 last_drain_tick。
    let stamina = app
        .world()
        .get::<crate::combat::components::Stamina>(caster)
        .unwrap();
    assert_eq!(stamina.current, 17.0, "M31: 必须扣 definition.stamina_cost");
    assert_eq!(stamina.last_drain_tick, Some(10), "M31: 必须打 drain tick");
    let position = app.world().get::<Position>(caster).unwrap().get();
    assert!((position.z - 7.5).abs() < 1e-9 && position.x.abs() < 1e-9);
}

#[test]
fn xue_beng_bu_uses_overridden_runtime_race_gate_without_mutation() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        XUE_BENG_BU_SKILL_ID,
        |definition| {
            definition.required_race = crate::body_plan::RaceGateOwned::Species {
                species: vec![crate::body_plan::RaceId::new("whale")],
            };
        },
    ));
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        rejected(CastRejectReason::RaceMismatch),
        "M19: race gate must follow definition.required_race override"
    );
    assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Gallbladder),
        1.0,
        "GallBladder must be untouched on rejection"
    );
    assert_eq!(
        app.world().get::<Position>(caster).unwrap().get(),
        DVec3::ZERO,
        "no dash on rejection"
    );
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .is_empty());
}

#[test]
fn xue_beng_bu_tears_first_overridden_meridian_not_hard_coded_gallbladder() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        XUE_BENG_BU_SKILL_ID,
        |definition| {
            definition.required_meridians = vec![
                TechniqueRequiredMeridian {
                    channel: "Lung".to_string(),
                    min_health: 0.4,
                },
                TechniqueRequiredMeridian {
                    channel: "GallBladder".to_string(),
                    min_health: 0.4,
                },
            ];
        },
    ));
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 50,
            anim_duration_ticks: 6,
        }
    );
    assert_eq!(qi(&app, caster), 75.0, "100 - 25 flat qi_cost");
    assert!(
        (meridian_integrity(&app, caster, MeridianId::Lung) - 0.75).abs() < 1e-12,
        "M19: torn meridian must follow the FIRST registry required_meridian (Lung -> 0.75)"
    );
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Gallbladder),
        1.0,
        "GallBladder must NOT be torn even though present in required_meridians"
    );
}

#[test]
fn xue_beng_bu_rejects_closed_overridden_meridian_without_mutation() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        XUE_BENG_BU_SKILL_ID,
        |definition| {
            definition.required_meridians = vec![TechniqueRequiredMeridian {
                channel: "Lung".to_string(),
                min_health: 0.4,
            }];
        },
    ));
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::Lung)
        .opened = false;

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Lung))),
        "M19: meridian gate must report the registry-overridden meridian, not the legacy default"
    );
    assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Lung),
        1.0,
        "Lung must be untouched on rejection"
    );
    assert_eq!(
        app.world().get::<Position>(caster).unwrap().get(),
        DVec3::ZERO,
        "no dash on rejection"
    );
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .is_empty());
}

#[test]
fn xue_beng_bu_exact_stamina_cost_enters_exhausted() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        XUE_BENG_BU_SKILL_ID,
        |definition| {
            definition.required_realm = "Awaken".to_string();
            definition.qi_cost = 6.25;
            definition.range = 7.5;
            definition.stamina_cost = 3.0;
        },
    ));
    let caster = spawn_caster_with_look(&mut app, Realm::Awaken, 100.0, DVec3::ZERO, 0.0);
    app.world_mut().entity_mut(caster).insert(Stamina {
        current: 3.0,
        max: 20.0,
        recover_per_sec: 1.0,
        last_drain_tick: None,
        state: StaminaState::Idle,
    });

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert!(matches!(result, CastResult::Started { .. }));
    let stamina = app.world().get::<Stamina>(caster).unwrap();
    assert_eq!(stamina.current, 0.0);
    assert_eq!(
        stamina.state,
        StaminaState::Exhausted,
        "爆脉施法扣空体力后必须进入 Exhausted，不能保留 Idle"
    );
    assert_eq!(stamina.last_drain_tick, Some(10));
}

#[test]
fn xue_beng_bu_rejects_when_stamina_insufficient_without_displacement() {
    // M31 负向：stamina_cost 高于当前体力 → 拒绝且零副作用（不位移/不扣 qi/不撕裂）。
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        XUE_BENG_BU_SKILL_ID,
        |definition| {
            definition.required_realm = "Awaken".to_string();
            definition.qi_cost = 6.25;
            definition.range = 7.5;
            definition.stamina_cost = 3.0;
        },
    ));
    let caster = spawn_caster_with_look(&mut app, Realm::Awaken, 100.0, DVec3::ZERO, 0.0);
    app.world_mut().entity_mut(caster).insert(Stamina {
        current: 2.0,
        max: 20.0,
        recover_per_sec: 1.0,
        last_drain_tick: None,
        state: StaminaState::Idle,
    });

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(result, rejected(CastRejectReason::InRecovery));
    assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
    let position = app.world().get::<Position>(caster).unwrap().get();
    assert_eq!(position, DVec3::ZERO, "no displacement on rejection");
    let stamina = app.world().get::<Stamina>(caster).unwrap();
    assert_eq!(
        stamina.current, 2.0,
        "stamina must be untouched on rejection"
    );
}

#[test]
fn xue_beng_bu_zero_stamina_cost_casts_even_when_exhausted() {
    // M33：valid zero-cost metadata（stamina_cost = 0.0）不能被体力 gate 当
    // insufficient 拒绝——Exhausted 状态下零成本招仍可施放。与 qi gate 的
    // 零成本语义（`cost <= EPSILON` 放行）对称。
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        XUE_BENG_BU_SKILL_ID,
        |definition| {
            definition.required_realm = "Awaken".to_string();
            definition.qi_cost = 6.25;
            definition.range = 7.5;
            definition.stamina_cost = 0.0;
        },
    ));
    let caster = spawn_caster_with_look(&mut app, Realm::Awaken, 100.0, DVec3::ZERO, 0.0);
    // Exhausted + 0 体力——若 stamina gate 错误拦零成本，这里会 InRecovery。
    app.world_mut().entity_mut(caster).insert(Stamina {
        current: 0.0,
        max: 20.0,
        recover_per_sec: 1.0,
        last_drain_tick: Some(5),
        state: StaminaState::Exhausted,
    });

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 50,
            anim_duration_ticks: 6,
        },
        "M33: zero-cost stamina must not block casting"
    );
    assert!((qi(&app, caster) - 93.75).abs() < 1e-9);
}

#[test]
fn ni_mai_hu_ti_uses_overridden_runtime_metadata() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        NI_MAI_HU_TI_SKILL_ID,
        |definition| {
            definition.required_realm = "Awaken".to_string();
            definition.qi_cost = 8.5;
            definition.cast_ticks = 15;
            definition.cooldown_ticks = 87;
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Awaken, 100.0, DVec3::ZERO);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 87,
            anim_duration_ticks: 15,
        }
    );
    assert!((qi(&app, caster) - 91.5).abs() < 1e-9);
    assert_eq!(
        app.world().get::<Casting>(caster).unwrap().duration_ticks,
        15
    );
}

#[test]
fn ni_mai_hu_ti_uses_overridden_runtime_race_gate_without_mutation() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        NI_MAI_HU_TI_SKILL_ID,
        |definition| {
            definition.required_race = crate::body_plan::RaceGateOwned::Species {
                species: vec![crate::body_plan::RaceId::new("whale")],
            };
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        rejected(CastRejectReason::RaceMismatch),
        "M19: race gate must follow definition.required_race override"
    );
    assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Pericardium),
        1.0,
        "Pericardium must be untouched on rejection"
    );
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .is_empty());
    assert!(app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .is_empty());
}

#[test]
fn ni_mai_hu_ti_tears_first_overridden_meridian_not_hard_coded_pericardium() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        NI_MAI_HU_TI_SKILL_ID,
        |definition| {
            definition.required_meridians = vec![
                TechniqueRequiredMeridian {
                    channel: "Lung".to_string(),
                    min_health: 0.55,
                },
                TechniqueRequiredMeridian {
                    channel: "Pericardium".to_string(),
                    min_health: 0.55,
                },
            ];
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 120,
            anim_duration_ticks: 12,
        }
    );
    assert_eq!(qi(&app, caster), 55.0, "100 - 45 flat qi_cost");
    assert!(
        (meridian_integrity(&app, caster, MeridianId::Lung) - 0.7).abs() < 1e-12,
        "M19: torn meridian must follow the FIRST registry required_meridian (Lung -> 0.7)"
    );
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Pericardium),
        1.0,
        "Pericardium must NOT be torn even though present in required_meridians"
    );
}

#[test]
fn ni_mai_hu_ti_rejects_closed_overridden_meridian_without_mutation() {
    let mut app = full_app();
    app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
        NI_MAI_HU_TI_SKILL_ID,
        |definition| {
            definition.required_meridians = vec![TechniqueRequiredMeridian {
                channel: "Lung".to_string(),
                min_health: 0.55,
            }];
        },
    ));
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::Lung)
        .opened = false;

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Lung))),
        "M19: meridian gate must report the registry-overridden meridian, not the legacy default"
    );
    assert_eq!(qi(&app, caster), 100.0, "qi must be untouched on rejection");
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::Lung),
        1.0,
        "Lung must be untouched on rejection"
    );
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .is_empty());
    assert!(app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .is_empty());
}

#[test]
fn p3_bespoke_anim_ids_emitted_and_beng_quan_borrow_removed() {
    let cases: [(&str, &str); 3] = [
        (TIE_SHAN_KAO_SKILL_ID, TIE_SHAN_KAO_ANIM_ID),
        (XUE_BENG_BU_SKILL_ID, XUE_BENG_BU_ANIM_ID),
        (NI_MAI_HU_TI_SKILL_ID, NI_MAI_HU_TI_ANIM_ID),
    ];
    // 常量层面先锁专属 + 互异（id 拼写回退在 resolver 跑起来之前就撞红）。
    assert_eq!(TIE_SHAN_KAO_ANIM_ID, "bong:tie_shan_kao");
    assert_eq!(XUE_BENG_BU_ANIM_ID, "bong:xue_beng_bu");
    assert_eq!(NI_MAI_HU_TI_ANIM_ID, "bong:ni_mai_hu_ti");
    for (skill_id, anim_id) in cases {
        assert_ne!(
            anim_id, BENG_QUAN_ANIM_ID,
            "去复用回归锁：{skill_id} 不得回退借崩拳动画 {BENG_QUAN_ANIM_ID}"
        );
    }

    // 事件路径：tie_shan_kao（Condense，带 UniqueId 才走 PlayAnim 分支）。
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    app.world_mut()
        .entity_mut(caster)
        .insert(valence::prelude::UniqueId::default());
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));
    assert!(matches!(result, CastResult::Started { .. }));
    let anims = emitted_play_anim_ids(&app);
    assert_eq!(
        anims,
        vec![TIE_SHAN_KAO_ANIM_ID.to_string()],
        "tie_shan_kao 应恰好发一条专属靠撞 PlayAnim，实际 {anims:?}"
    );

    // 事件路径：xue_beng_bu（需 Look 提供朝向）。
    let mut app = full_app();
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
    app.world_mut()
        .entity_mut(caster)
        .insert(valence::prelude::UniqueId::default());
    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));
    let anims = emitted_play_anim_ids(&app);
    assert_eq!(
        anims,
        vec![XUE_BENG_BU_ANIM_ID.to_string()],
        "xue_beng_bu 应恰好发一条专属步法 PlayAnim，实际 {anims:?}"
    );

    // 事件路径：ni_mai_hu_ti（缺失补齐——此前 anim_id: None 完全不发）。
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
    app.world_mut()
        .entity_mut(caster)
        .insert(valence::prelude::UniqueId::default());
    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));
    let anims = emitted_play_anim_ids(&app);
    assert_eq!(
        anims,
        vec![NI_MAI_HU_TI_ANIM_ID.to_string()],
        "ni_mai_hu_ti 应恰好发一条专属护体结印 PlayAnim（MISSING 缺口已补），实际 {anims:?}"
    );
}

#[test]
fn flat_qi_cost_reads_from_known_techniques_single_source() {
    // 严禁本文件硬编码重复 qi_cost：值必须来自 checked-in registry。
    let techniques = TechniqueRegistry::load_for_tests();
    assert_eq!(flat_qi_cost(&techniques, TIE_SHAN_KAO_SKILL_ID), Some(35.0));
    assert_eq!(flat_qi_cost(&techniques, XUE_BENG_BU_SKILL_ID), Some(25.0));
    assert_eq!(flat_qi_cost(&techniques, NI_MAI_HU_TI_SKILL_ID), Some(45.0));
    assert_eq!(flat_qi_cost(&techniques, "nonexistent.skill"), None);
}

#[test]
fn p5_bespoke_particle_ids_emitted_and_beng_quan_particle_borrow_removed() {
    // 常量层面先锁专属 + 互异。
    assert_eq!(TIE_SHAN_KAO_PARTICLE_ID, "bong:burst_meridian_tie_shan_kao");
    assert_eq!(XUE_BENG_BU_PARTICLE_ID, "bong:burst_meridian_xue_beng_bu");
    assert_eq!(NI_MAI_HU_TI_PARTICLE_ID, "bong:burst_meridian_ni_mai_hu_ti");
    for particle_id in [
        TIE_SHAN_KAO_PARTICLE_ID,
        XUE_BENG_BU_PARTICLE_ID,
        NI_MAI_HU_TI_PARTICLE_ID,
    ] {
        assert_ne!(
            particle_id, BENG_QUAN_PARTICLE_ID,
            "去复用回归锁：{particle_id} 不得回退借崩拳粒子 {BENG_QUAN_PARTICLE_ID}"
        );
    }

    // 事件路径：tie_shan_kao。
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));
    assert!(matches!(result, CastResult::Started { .. }));
    assert_emits_wired_particle(&app, TIE_SHAN_KAO_SKILL_ID);

    // 事件路径：xue_beng_bu(需 Look 提供朝向)。
    let mut app = full_app();
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));
    assert_emits_wired_particle(&app, XUE_BENG_BU_SKILL_ID);

    // 事件路径：ni_mai_hu_ti。
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));
    assert_emits_wired_particle(&app, NI_MAI_HU_TI_SKILL_ID);
}

#[test]
fn p5_burst_family_shares_one_color_but_never_one_id() {
    // 家族设计：共用识别色 + 形态分化。两个性质都必须成立——
    // 只共用色而 id 也相同 = 回到借用；只 id 不同而颜色发散 = 失去家族识别。
    let ids = [
        BENG_QUAN_PARTICLE_ID,
        TIE_SHAN_KAO_PARTICLE_ID,
        XUE_BENG_BU_PARTICLE_ID,
        NI_MAI_HU_TI_PARTICLE_ID,
    ];
    let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        ids.len(),
        "爆脉四招粒子 id 必须两两不同,实际 {unique:?}"
    );
    for id in ids {
        assert!(
            id.starts_with("bong:burst_meridian_"),
            "{id} 不在 bong:burst_meridian_ 家族前缀下,会掉出 Important 优先级档"
        );
    }
    assert_eq!(
        BURST_MERIDIAN_FAMILY_COLOR, "#C58B3F",
        "爆脉家族识别色被改动——plan §P5.1 ② 指定为 #C58B3F"
    );
}

#[test]
fn p5_ni_mai_hu_ti_aura_ring_follows_moving_caster() {
    let mut app = aura_app();
    let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
    clear_vfx_events(&mut app);

    // 相位落在 cast tick + 12 的整数倍上。
    let waypoints = [
        (CAST_TICK + 12, DVec3::new(3.0, 0.0, 0.0)),
        (CAST_TICK + 24, DVec3::new(3.0, 0.0, 5.0)),
        (CAST_TICK + 36, DVec3::new(-2.0, 1.0, 5.0)),
        (CAST_TICK + 48, DVec3::new(-2.0, 1.0, 9.0)),
    ];
    for (tick, position) in waypoints {
        move_caster(&mut app, caster, position);
        let rings = tick_aura(&mut app, tick);
        assert_eq!(
            rings.len(),
            1,
            "tick {tick} 应恰好重发 1 个护体环，实际 {rings:?}"
        );
        assert_eq!(
            ring_origin(&rings[0]),
            [
                position.x,
                position.y + BURST_AV_PARTICLE_Y_LIFT,
                position.z
            ],
            "护体环圆心必须锚在施法者 tick {tick} 的**当前**位置（plan §P5.1 ② 「体表」\
             逆流纹是字面交付物），而不是 cast 瞬间的 origin"
        );
        clear_vfx_events(&mut app);
    }
}
