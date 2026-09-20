#![allow(dead_code, unused_imports)]

use bong_server::body_plan::*;
use bong_server::combat::components::*;
use bong_server::combat::events::*;
use bong_server::combat::CombatClock;
use bong_server::cultivation::burst_meridian::*;
use bong_server::cultivation::color::*;
use bong_server::cultivation::components::*;
use bong_server::cultivation::known_techniques::*;
use bong_server::cultivation::meridian::severed::*;
use bong_server::cultivation::skill_registry::*;
use bong_server::cultivation::technique_scroll::*;
use bong_server::network::audio_event_emit::*;
use bong_server::network::cast_emit::*;
use bong_server::network::skill_vfx_wiring;
use bong_server::network::vfx_event_emit::*;
use bong_server::qi_physics::*;
use bong_server::schema::server_data::*;
use bong_server::schema::vfx_event::*;
use bong_server::world::dimension::*;
use bong_server::world::zone::*;
use std::path::Path;
use std::sync::OnceLock;
use valence::prelude::*;

fn checked_in_technique_registry() -> &'static TechniqueRegistry {
    static REGISTRY: OnceLock<TechniqueRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_TECHNIQUES_PATH);
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let plans = bong_server::body_plan::BodyPlanRegistry::load_dir(
            root.join("assets/body_plans/plans"),
        )
        .unwrap();
        let races =
            RaceRegistry::load_file(root.join("assets/body_plans/races.json"), &plans).unwrap();
        TechniqueRegistry::load_from_path(path, &races)
            .expect("checked-in technique catalog must load")
    })
}

fn ni_mai_hu_ti_particle_id() -> &'static str {
    skill_vfx_wiring::wiring_for(NI_MAI_HU_TI_SKILL_ID)
        .expect("ni_mai_hu_ti particle must be in the public VFX wiring table")
        .event_id
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

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
    app.insert_resource(checked_in_technique_registry().clone());
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
        bong_server::cultivation::meridian::severed::SeveredSource::CombatWound,
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
use bong_server::qi_physics::{QiTransfer, QiTransferReason};
use bong_server::world::zone::ZoneRegistry;

/// 带 ZoneRegistry（fallback spawn zone，覆盖 y=70 层面）和 QiTransfer event 的测试 App。
/// caster 位置应使用 DVec3::new(0.0, 70.0, 0.0) 才落在 spawn zone AABB 内。
fn app_with_zone() -> App {
    let mut app = App::new();
    app.insert_resource(checked_in_technique_registry().clone());
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
            bong_server::world::dimension::DimensionKind::Overworld,
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
    let wiring = bong_server::network::skill_vfx_wiring::wiring_for(skill_id)
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
                    if event_id == ni_mai_hu_ti_particle_id()
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
fn beng_quan_checked_in_metadata_keeps_legacy_display_range() {
    let app = app();
    let definition = app
        .world()
        .resource::<TechniqueRegistry>()
        .get(BENG_QUAN_SKILL_ID)
        .expect("checked-in beng_quan metadata must exist");
    assert_eq!(
        definition.range, 1.3,
        "registry datafication must preserve the legacy 1.3 display metadata"
    );
    assert_eq!(
        FIST_REACH,
        AttackReach::new(2.0, 0.6),
        "combat resolution keeps base reach and step bonus separate from display metadata"
    );
}

#[test]
fn burst_event_payload_uses_stable_entity_wire_ids() {
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));
    let event = BurstMeridianEvent {
        skill: BENG_QUAN_EVENT_SKILL,
        caster,
        target: Some(target),
        tick: 10,
        overload_ratio: 1.5,
        integrity_snapshot: 1.0,
    };

    let ServerDataPayloadV1::BurstMeridianEvent(payload) = event.to_payload(app.world()) else {
        panic!("expected burst meridian payload");
    };

    assert_eq!(payload.skill, BENG_QUAN_EVENT_SKILL);
    assert_eq!(payload.caster, format!("entity:{}", caster.to_bits()));
    assert_eq!(payload.target, Some(format!("entity:{}", target.to_bits())));
}

#[test]
fn beng_quan_rejects_low_realm_without_mutation() {
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Awaken, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
    assert_no_mutation(&app, caster, 100.0, 1.0);
}

#[test]
fn beng_quan_rejects_all_right_arm_meridians_severed() {
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    for id in RIGHT_ARM_MERIDIANS {
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(id)
            .integrity = 0.0;
    }
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    // 全断时退化为 "声明顺序首条" —— RIGHT_ARM_MERIDIANS[0] = LargeIntestine
    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(
            MeridianId::LargeIntestine
        )))
    );
    assert_no_mutation(&app, caster, 100.0, 0.0);
}

#[test]
fn beng_quan_rejects_when_severed_component_marks_first_dep() {
    // SEVERED component 标 LargeIntestine → 即使 Meridian.integrity 仍 1.0 也拒绝
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    app.world_mut()
        .entity_mut(caster)
        .insert(MeridianSeveredPermanent::default());
    {
        let mut sev = app
            .world_mut()
            .get_mut::<MeridianSeveredPermanent>(caster)
            .unwrap();
        sev.insert(
            MeridianId::LargeIntestine,
            bong_server::cultivation::meridian::severed::SeveredSource::CombatWound,
            0,
        );
    }
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    // check_meridian_runtime_integrity 先查 SEVERED component → 返回首个 SEVERED
    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(
            MeridianId::LargeIntestine
        )))
    );
    assert_no_mutation(&app, caster, 100.0, 1.0);
}

#[test]
fn beng_quan_rejects_when_any_right_arm_meridian_is_unavailable() {
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::SmallIntestine)
        .integrity = 0.0;
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::TripleEnergizer)
        .integrity = 0.0;
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(
            MeridianId::SmallIntestine
        )))
    );
    assert_eq!(qi(&app, caster), 100.0, "拒绝路径不得扣真元");
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::LargeIntestine),
        1.0
    );
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::SmallIntestine),
        0.0
    );
    assert_eq!(
        meridian_integrity(&app, caster, MeridianId::TripleEnergizer),
        0.0
    );
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app.world().resource::<Events<AttackIntent>>().is_empty());
    assert!(app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .is_empty());
}

#[test]
fn beng_quan_rejects_out_of_range_target_without_mutation() {
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(2.600_001, 0.0, 0.0));

    // 锁定超距目标硬轰仍是"目标无效"的正确语义（有目标时距离门保留）。
    assert_eq!(
        resolve_beng_quan(app.world_mut(), caster, 0, Some(target)),
        rejected(CastRejectReason::InvalidTarget)
    );
    assert_no_mutation(&app, caster, 100.0, 1.0);
}

#[test]
fn beng_quan_whiffs_without_target_spending_cost() {
    // Option B 去目标门禁（对齐 sword_basics 劈/刺）：无目标 = 空挥，
    // Started + 照常扣真元撕脉 + AttackIntent/BurstMeridianEvent target=None。
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);

    let result = resolve_beng_quan(app.world_mut(), caster, 0, None);
    assert!(
        matches!(result, CastResult::Started { .. }),
        "无目标崩拳应空挥 Started（不再报 InvalidTarget），实际 {result:?}"
    );
    let qi = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    assert!(
        qi < 100.0,
        "空挥必须照常扣真元（防无目标白嫖挥拳），实际 qi={qi}"
    );
    let attack_events = app.world().resource::<Events<AttackIntent>>();
    let attack = attack_events
        .iter_current_update_events()
        .next()
        .expect("空挥仍应发 AttackIntent（target=None 由 combat resolver 跳过命中）");
    assert_eq!(attack.target, None);
    assert_eq!(attack.source, AttackSource::BurstMeridian);
    let burst_events = app.world().resource::<Events<BurstMeridianEvent>>();
    assert_eq!(
        burst_events
            .iter_current_update_events()
            .next()
            .expect("空挥仍应发 BurstMeridianEvent（AV/proto 桥照走）")
            .target,
        None
    );
    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    assert!(
        vfx_events.iter_current_update_events().count() > 0,
        "空挥应照常出动画/粒子（whiff_focus_point 沿朝向兜底）"
    );
}

#[test]
fn beng_quan_whiff_particle_direction_follows_caster_look() {
    // CR #835：覆盖 whiff_focus_point 的 Look 分支——空挥粒子方向必须沿
    // 施法者视线（yaw=-90 => 朝东 +X），而非无 Look 时的 +Z 兜底。
    let mut app = app();
    let caster = spawn_caster_with_look(&mut app, Realm::Induce, 100.0, DVec3::ZERO, -90.0);

    let result = resolve_beng_quan(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let direction = vfx_events
        .iter_current_update_events()
        .find_map(|event| match &event.payload {
            VfxEventPayloadV1::SpawnParticle { direction, .. } => *direction,
            _ => None,
        })
        .expect("空挥应发带方向的 SpawnParticle");
    assert!(
        direction[0] > 0.0 && direction[1].abs() < 1e-3 && direction[2].abs() < 1e-3,
        "yaw=-90 的空挥粒子方向应沿视线朝 +X（whiff_focus_point Look 分支），\
         实际 direction={direction:?}"
    );
    let direction_len = (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2)).sqrt();
    assert!(
        (direction_len - f64::from(FIST_REACH.max)).abs() < 1e-6,
        "空挥焦点必须沿视线前推旧版有效射程 2.6，而不是 metadata 1.3；实际 {direction_len}"
    );
}

#[test]
fn beng_quan_rejects_cooldown_before_mutation() {
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<SkillBarBindings>(caster)
        .unwrap()
        .set_cooldown(BENG_QUAN_SKILL_ID, 11);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert_eq!(result, rejected(CastRejectReason::OnCooldown));
    assert_no_mutation(&app, caster, 100.0, 1.0);
}

#[test]
fn beng_quan_preserves_float_precision_and_pre_mutation_snapshot() {
    let mut app = app();
    let caster = spawn_caster(&mut app, Realm::Induce, 99.9, DVec3::ZERO);
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::LargeIntestine)
        .integrity = 0.1;
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert!(matches!(result, CastResult::Started { .. }));
    let qi = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    assert!((qi - 59.94).abs() < 1e-9);
    let li = app
        .world()
        .get::<MeridianSystem>(caster)
        .unwrap()
        .get(MeridianId::LargeIntestine)
        .integrity;
    assert!((li - 0.07).abs() < 1e-12);
    let burst_events = app.world().resource::<Events<BurstMeridianEvent>>();
    let burst = burst_events.iter_current_update_events().next().unwrap();
    assert!((burst.integrity_snapshot - 0.7).abs() < 1e-12);
}

#[test]
fn beng_quan_style_attack_uses_heavy_color_and_integrity_purity() {
    let attack = BengQuanStyleAttack {
        qi_invest: 12.0,
        integrity_snapshot: 0.7,
    };

    assert_eq!(attack.style_color(), ColorKind::Heavy);
    assert_eq!(attack.injected_qi(), 12.0);
    assert_eq!(attack.purity(), 0.7);
    assert_eq!(attack.rejection_rate(), 0.65);
}

#[test]
fn tie_shan_kao_happy_path_spends_qi_tears_stomach_and_strikes() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 70,
            anim_duration_ticks: 10,
        },
        "cd/cast must come from known_techniques.tie_shan_kao"
    );
    // 守恒：扣 flat 35（非 ratio），Stomach ×0.8。
    assert_eq!(qi(&app, caster), 65.0, "100 - 35 flat qi_cost");
    assert!(
        (meridian_integrity(&app, caster, MeridianId::Stomach) - 0.8).abs() < 1e-12,
        "Stomach integrity torn 1.0 -> 0.8"
    );
    assert_eq!(
        app.world().get::<Casting>(caster).unwrap().duration_ticks,
        10
    );

    // 攻击：BurstMeridian 来源（自带 ×2.5 击退），qi_invest = cost * overload。
    let attack = app
        .world()
        .resource::<Events<AttackIntent>>()
        .iter_current_update_events()
        .next()
        .expect("AttackIntent emitted");
    assert_eq!(attack.target, Some(target));
    assert_eq!(attack.source, AttackSource::BurstMeridian);
    assert_eq!(attack.wound_kind, WoundKind::Blunt);
    assert!(
        (attack.qi_invest - (35.0 * TIE_SHAN_KAO_OVERLOAD_RATIO) as f32).abs() < 1e-3,
        "qi_invest = cost * overload_ratio"
    );

    assert_eq!(last_burst_skill(&app), TIE_SHAN_KAO_EVENT_SKILL);
    assert_eq!(emitted_audio_recipe(&app).as_deref(), Some("hit_heavy"));
}

#[test]
fn tie_shan_kao_rejects_low_realm_without_mutation() {
    let mut app = full_app();
    // Induce < Condense → 拒绝。
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
    assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 100.0);
}

#[test]
fn tie_shan_kao_rejects_insufficient_qi_without_mutation() {
    let mut app = full_app();
    // 30 < 35 flat cost → QiInsufficient。
    let caster = spawn_caster(&mut app, Realm::Condense, 30.0, DVec3::ZERO);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(result, rejected(CastRejectReason::QiInsufficient));
    assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 30.0);
}

#[test]
fn tie_shan_kao_rejects_out_of_range_and_whiffs_without_target() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    // 超 reach.max (1.5) → InvalidTarget（有目标时距离门保留）。
    let range = f64::from(
        app.world()
            .resource::<TechniqueRegistry>()
            .get(TIE_SHAN_KAO_SKILL_ID)
            .expect("tie_shan_kao metadata must exist")
            .range,
    );
    let far = spawn_target(&mut app, DVec3::new(range + 0.5, 0.0, 0.0));
    assert_eq!(
        resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(far)),
        rejected(CastRejectReason::InvalidTarget)
    );
    assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 100.0);

    // Option B：无目标 = 空撞，Started + 扣费撕脉 + 事件 target=None。
    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, None);
    assert!(
        matches!(result, CastResult::Started { .. }),
        "无目标贴山靠应空撞 Started（不再报 InvalidTarget），实际 {result:?}"
    );
    let qi = app.world().get::<Cultivation>(caster).unwrap().qi_current;
    assert!(qi < 100.0, "空撞必须照常扣真元，实际 qi={qi}");
    let attack_events = app.world().resource::<Events<AttackIntent>>();
    assert_eq!(
        attack_events
            .iter_current_update_events()
            .next()
            .expect("空撞仍应发 AttackIntent（target=None）")
            .target,
        None
    );
}

#[test]
fn tie_shan_kao_rejects_when_stomach_severed() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    sever_meridian(&mut app, caster, MeridianId::Stomach);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(MeridianId::Stomach)))
    );
    // SEVERED component 把 Stomach 钳到 0；只断言 qi 未动 + 无副作用。
    assert_eq!(qi(&app, caster), 100.0);
    assert!(app.world().get::<Casting>(caster).is_none());
    assert!(app.world().resource::<Events<AttackIntent>>().is_empty());
}

#[test]
fn tie_shan_kao_rejects_on_cooldown_before_mutation() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<SkillBarBindings>(caster)
        .unwrap()
        .set_cooldown(TIE_SHAN_KAO_SKILL_ID, 11);
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert_eq!(result, rejected(CastRejectReason::OnCooldown));
    assert_single_meridian_untouched(&app, caster, MeridianId::Stomach, 100.0);
}

#[test]
fn xue_beng_bu_happy_path_dashes_forward_spends_qi_tears_leg() {
    let mut app = full_app();
    // yaw=0 → facing (0,0,1)；起点原点。
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 50,
            anim_duration_ticks: 6,
        }
    );
    // 守恒：扣 flat 25，Gallbladder ×0.75。
    assert_eq!(qi(&app, caster), 75.0, "100 - 25 flat qi_cost");
    assert!(
        (meridian_integrity(&app, caster, MeridianId::Gallbladder) - 0.75).abs() < 1e-12,
        "Gallbladder torn 1.0 -> 0.75"
    );
    // 位移：沿 +Z 推 4.0 格（服务器权威 Position.set）。
    let pos = app.world().get::<Position>(caster).unwrap().get();
    let expected_range = f64::from(
        app.world()
            .resource::<TechniqueRegistry>()
            .get(XUE_BENG_BU_SKILL_ID)
            .expect("xue_beng_bu metadata must exist")
            .range,
    );
    assert!(
        (pos.z - expected_range).abs() < 1e-9 && pos.x.abs() < 1e-9,
        "caster should dash +Z by 4.0 blocks, got {pos:?}"
    );
    // 无攻击事件（纯位移招）。
    assert!(
        app.world().resource::<Events<AttackIntent>>().is_empty(),
        "xue_beng_bu must not emit AttackIntent"
    );
    assert_eq!(last_burst_skill(&app), XUE_BENG_BU_EVENT_SKILL);
    assert_eq!(emitted_audio_recipe(&app).as_deref(), Some("movement_dash"));
}

#[test]
fn xue_beng_bu_rejects_when_no_look_no_displacement() {
    let mut app = full_app();
    // 无 Look component → 无朝向 → 拒绝，绝不凭空位移。
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(result, rejected(CastRejectReason::InvalidTarget));
    let pos = app.world().get::<Position>(caster).unwrap().get();
    assert_eq!(pos, DVec3::ZERO, "no displacement on rejection");
    assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 100.0);
}

#[test]
fn xue_beng_bu_rejects_low_realm_without_mutation() {
    let mut app = full_app();
    let caster = spawn_caster_with_look(&mut app, Realm::Induce, 100.0, DVec3::ZERO, 0.0);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
    let pos = app.world().get::<Position>(caster).unwrap().get();
    assert_eq!(pos, DVec3::ZERO);
    assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 100.0);
}

#[test]
fn xue_beng_bu_rejects_insufficient_qi_without_mutation() {
    let mut app = full_app();
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 20.0, DVec3::ZERO, 0.0);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(result, rejected(CastRejectReason::QiInsufficient));
    let pos = app.world().get::<Position>(caster).unwrap().get();
    assert_eq!(pos, DVec3::ZERO);
    assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 20.0);
}

#[test]
fn xue_beng_bu_rejects_when_gallbladder_severed() {
    let mut app = full_app();
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
    sever_meridian(&mut app, caster, MeridianId::Gallbladder);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(
            MeridianId::Gallbladder
        )))
    );
    assert_eq!(qi(&app, caster), 100.0);
    let pos = app.world().get::<Position>(caster).unwrap().get();
    assert_eq!(pos, DVec3::ZERO);
}

#[test]
fn xue_beng_bu_rejects_on_cooldown() {
    let mut app = full_app();
    let caster = spawn_caster_with_look(&mut app, Realm::Condense, 100.0, DVec3::ZERO, 0.0);
    app.world_mut()
        .get_mut::<SkillBarBindings>(caster)
        .unwrap()
        .set_cooldown(XUE_BENG_BU_SKILL_ID, 11);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert_eq!(result, rejected(CastRejectReason::OnCooldown));
    let pos = app.world().get::<Position>(caster).unwrap().get();
    assert_eq!(pos, DVec3::ZERO);
    assert_single_meridian_untouched(&app, caster, MeridianId::Gallbladder, 100.0);
}

#[test]
fn ni_mai_hu_ti_happy_path_spends_qi_tears_pericardium_applies_buff() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        CastResult::Started {
            cooldown_ticks: 120,
            anim_duration_ticks: 12,
        }
    );
    // 守恒：扣 flat 45，Pericardium ×0.7。
    assert_eq!(qi(&app, caster), 55.0, "100 - 45 flat qi_cost");
    assert!(
        (meridian_integrity(&app, caster, MeridianId::Pericardium) - 0.7).abs() < 1e-12,
        "Pericardium torn 1.0 -> 0.7"
    );
    // buff：DamageReduction(0.35, 60t) 给自身。
    let buff = app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .iter_current_update_events()
        .next()
        .expect("ApplyStatusEffectIntent emitted");
    assert_eq!(buff.target, caster);
    assert_eq!(buff.kind, StatusEffectKind::DamageReduction);
    assert!((buff.magnitude - NI_MAI_HU_TI_DAMAGE_REDUCTION).abs() < 1e-6);
    assert_eq!(buff.duration_ticks, NI_MAI_HU_TI_BUFF_DURATION_TICKS);
    // 自身招：无攻击。
    assert!(app.world().resource::<Events<AttackIntent>>().is_empty());
    assert_eq!(last_burst_skill(&app), NI_MAI_HU_TI_EVENT_SKILL);
    assert_eq!(
        emitted_audio_recipe(&app).as_deref(),
        Some("zhenmai_shield_hum")
    );
}

#[test]
fn ni_mai_hu_ti_rejects_low_realm_without_mutation() {
    let mut app = full_app();
    // Condense < Solidify → 拒绝。
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::ZERO);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(result, rejected(CastRejectReason::RealmTooLow));
    assert_single_meridian_untouched(&app, caster, MeridianId::Pericardium, 100.0);
    assert!(app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .is_empty());
}

#[test]
fn ni_mai_hu_ti_rejects_insufficient_qi_without_mutation() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Solidify, 40.0, DVec3::ZERO);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(result, rejected(CastRejectReason::QiInsufficient));
    assert_single_meridian_untouched(&app, caster, MeridianId::Pericardium, 40.0);
    assert!(app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .is_empty());
}

#[test]
fn ni_mai_hu_ti_rejects_when_pericardium_severed() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
    sever_meridian(&mut app, caster, MeridianId::Pericardium);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(
        result,
        rejected(CastRejectReason::MeridianSevered(Some(
            MeridianId::Pericardium
        )))
    );
    assert_eq!(qi(&app, caster), 100.0);
    assert!(app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .is_empty());
}

#[test]
fn ni_mai_hu_ti_rejects_on_cooldown() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
    app.world_mut()
        .get_mut::<SkillBarBindings>(caster)
        .unwrap()
        .set_cooldown(NI_MAI_HU_TI_SKILL_ID, 11);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert_eq!(result, rejected(CastRejectReason::OnCooldown));
    assert_single_meridian_untouched(&app, caster, MeridianId::Pericardium, 100.0);
    assert!(app
        .world()
        .resource::<Events<ApplyStatusEffectIntent>>()
        .is_empty());
}

#[test]
fn p3_ni_mai_hu_ti_without_unique_id_skips_anim_keeps_particle() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::ZERO);
    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Started { .. }));
    assert!(
        emitted_play_anim_ids(&app).is_empty(),
        "无 UniqueId 不应发 PlayAnim（防御分支）"
    );
    let particles = app
        .world()
        .resource::<Events<VfxEventRequest>>()
        .iter_current_update_events()
        .filter(|request| matches!(&request.payload, VfxEventPayloadV1::SpawnParticle { .. }))
        .count();
    assert_eq!(particles, 1, "护体粒子环应照发");
}

#[test]
fn tie_shan_kao_float_precision_preserved() {
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Condense, 99.9, DVec3::ZERO);
    // 把 Stomach 设到刚好满足 min_health=0.5 边界附近（runtime gate 只看 >ε）。
    app.world_mut()
        .get_mut::<MeridianSystem>(caster)
        .unwrap()
        .get_mut(MeridianId::Stomach)
        .integrity = 0.6;
    let target = spawn_target(&mut app, DVec3::new(1.0, 0.0, 0.0));

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert!(matches!(result, CastResult::Started { .. }));
    assert!((qi(&app, caster) - 64.9).abs() < 1e-9, "99.9 - 35 = 64.9");
    assert!(
        (meridian_integrity(&app, caster, MeridianId::Stomach) - 0.48).abs() < 1e-12,
        "0.6 * 0.8 = 0.48"
    );
    // integrity_snapshot 记的是损前快照 0.6。
    let snap = app
        .world()
        .resource::<Events<BurstMeridianEvent>>()
        .iter_current_update_events()
        .next()
        .unwrap()
        .integrity_snapshot;
    assert!((snap - 0.6).abs() < 1e-12);
}

#[test]
fn all_three_skills_register_and_declare_so_audit_invariant_holds() {
    // 注册 + declare 配对：缺一即 skill_registry 审计不变量赤。
    let mut registry = SkillRegistry::default();
    register_skills(&mut registry);
    assert!(registry.lookup(TIE_SHAN_KAO_SKILL_ID).is_some());
    assert!(registry.lookup(XUE_BENG_BU_SKILL_ID).is_some());
    assert!(registry.lookup(NI_MAI_HU_TI_SKILL_ID).is_some());

    let mut deps =
        bong_server::cultivation::meridian::severed::SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut deps);
    assert_eq!(
        deps.lookup(BENG_QUAN_SKILL_ID),
        RIGHT_ARM_MERIDIANS.as_slice()
    );
    assert_eq!(
        deps.lookup(TIE_SHAN_KAO_SKILL_ID),
        TIE_SHAN_KAO_MERIDIANS.as_slice()
    );
    assert_eq!(
        deps.lookup(XUE_BENG_BU_SKILL_ID),
        XUE_BENG_BU_MERIDIANS.as_slice()
    );
    assert_eq!(
        deps.lookup(NI_MAI_HU_TI_SKILL_ID),
        NI_MAI_HU_TI_MERIDIANS.as_slice()
    );
}

#[test]
fn beng_quan_spend_qi_releases_to_zone() {
    let mut app = app_with_zone();
    // 在 spawn zone AABB 内（y=70）生成施法者。
    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::new(0.0, 70.0, 0.0));
    let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, f64::from(FIST_REACH.max)));

    let initial_spirit_qi = zone_spirit_qi(&app);

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert!(
        matches!(result, CastResult::Started { .. }),
        "beng_quan must succeed; got {result:?}"
    );
    assert!(
        zone_spirit_qi(&app) > initial_spirit_qi,
        "zone spirit_qi must rise after beng_quan spend_qi; \
         before={initial_spirit_qi}, after={}",
        zone_spirit_qi(&app)
    );
    assert!(
        qi_transfer_count(&app) > 0,
        "a QiTransfer must be emitted for the zone release"
    );
}

#[test]
fn tie_shan_kao_spend_qi_releases_to_zone() {
    let mut app = app_with_zone();
    let caster = spawn_caster(&mut app, Realm::Condense, 100.0, DVec3::new(0.0, 70.0, 0.0));
    let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, 1.0));

    let initial_spirit_qi = zone_spirit_qi(&app);

    let result = resolve_tie_shan_kao(app.world_mut(), caster, 0, Some(target));

    assert!(
        matches!(result, CastResult::Started { .. }),
        "tie_shan_kao must succeed; got {result:?}"
    );
    assert!(
        zone_spirit_qi(&app) > initial_spirit_qi,
        "zone spirit_qi must rise after tie_shan_kao spend_qi; \
         before={initial_spirit_qi}, after={}",
        zone_spirit_qi(&app)
    );
}

#[test]
fn xue_beng_bu_spend_qi_releases_to_zone() {
    let mut app = app_with_zone();
    // yaw=0 → facing = -z 方向（sin(0)=0, cos(0)=1），用 Look::new(0.0, 0.0)。
    let caster = spawn_caster_with_look(
        &mut app,
        Realm::Solidify,
        100.0,
        DVec3::new(0.0, 70.0, 0.0),
        0.0,
    );

    let initial_spirit_qi = zone_spirit_qi(&app);

    let result = resolve_xue_beng_bu(app.world_mut(), caster, 0, None);

    assert!(
        matches!(result, CastResult::Started { .. }),
        "xue_beng_bu must succeed; got {result:?}"
    );
    assert!(
        zone_spirit_qi(&app) > initial_spirit_qi,
        "zone spirit_qi must rise after xue_beng_bu spend_qi; \
         before={initial_spirit_qi}, after={}",
        zone_spirit_qi(&app)
    );
}

#[test]
fn ni_mai_hu_ti_spend_qi_releases_to_zone() {
    let mut app = app_with_zone();
    let caster = spawn_caster(&mut app, Realm::Solidify, 100.0, DVec3::new(0.0, 70.0, 0.0));

    let initial_spirit_qi = zone_spirit_qi(&app);

    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);

    assert!(
        matches!(result, CastResult::Started { .. }),
        "ni_mai_hu_ti must succeed; got {result:?}"
    );
    assert!(
        zone_spirit_qi(&app) > initial_spirit_qi,
        "zone spirit_qi must rise after ni_mai_hu_ti spend_qi; \
         before={initial_spirit_qi}, after={}",
        zone_spirit_qi(&app)
    );
}

#[test]
fn rejected_cast_does_not_release_qi_to_zone() {
    let mut app = app_with_zone();
    let caster = spawn_caster(&mut app, Realm::Awaken, 0.0, DVec3::new(0.0, 70.0, 0.0));
    let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, 1.0));

    let initial_spirit_qi = zone_spirit_qi(&app);

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert!(
        matches!(result, CastResult::Rejected { .. }),
        "must be rejected; got {result:?}"
    );
    assert_eq!(
        zone_spirit_qi(&app),
        initial_spirit_qi,
        "zone spirit_qi must be unchanged when cast is rejected — \
         no qi was consumed so none can be released"
    );
    assert_eq!(
        qi_transfer_count(&app),
        0,
        "no QiTransfer must be emitted on rejected cast"
    );
}

#[test]
fn beng_quan_routes_overflow_when_zone_is_full() {
    let mut app = app_with_zone();
    // 把 spawn zone 灵气填满。
    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .find_zone_mut("spawn")
        .expect("spawn zone must exist")
        .spirit_qi = 1.0;

    let caster = spawn_caster(&mut app, Realm::Induce, 100.0, DVec3::new(0.0, 70.0, 0.0));
    let target = spawn_target(&mut app, DVec3::new(0.0, 70.0, f64::from(FIST_REACH.max)));

    let result = resolve_beng_quan(app.world_mut(), caster, 0, Some(target));

    assert!(
        matches!(result, CastResult::Started { .. }),
        "beng_quan must succeed even when zone is full; got {result:?}"
    );
    assert_eq!(
        zone_spirit_qi(&app),
        1.0,
        "zone spirit_qi must not exceed 1.0 when zone is full"
    );
    // overflow 路径：仍发 QiTransfer（overflow 账户）。
    assert!(
        qi_transfer_count(&app) > 0,
        "a QiTransfer (overflow) must be emitted even when zone is full"
    );
    // 拿到 overflow transfer，验证其目的账户种类。
    let transfer = app
        .world()
        .resource::<Events<QiTransfer>>()
        .iter_current_update_events()
        .next()
        .expect("at least one QiTransfer must be emitted");
    assert_eq!(
        transfer.reason,
        QiTransferReason::ReleaseToZone,
        "overflow transfer reason must be ReleaseToZone"
    );
}

#[test]
fn p5_rejected_burst_cast_emits_no_particle() {
    // 拒绝路径(真元不足)不得发粒子。
    let mut app = full_app();
    let caster = spawn_caster(&mut app, Realm::Condense, 0.0, DVec3::ZERO);
    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
    assert!(matches!(result, CastResult::Rejected { .. }));
    assert!(emitted_particles(&app).is_empty(), "被拒绝的施放不得发粒子");
}

#[test]
fn p5_ni_mai_hu_ti_cast_installs_aura_anchor_matching_buff_window() {
    let mut app = aura_app();
    let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);

    let aura = app
        .world()
        .get::<NiMaiHuTiAura>(caster)
        .copied()
        .expect("施放后必须挂上 NiMaiHuTiAura 锚点，否则重发系统永远找不到施法者");
    assert_eq!(aura.started_at_tick, CAST_TICK, "重发相位基准应为施法 tick");
    assert_eq!(
        aura.expires_at_tick,
        CAST_TICK + NI_MAI_HU_TI_BUFF_DURATION_TICKS,
        "锚点窗口必须与减伤 buff 时长严格同步——视觉窗口比 buff 长=护体已过纹还在转，\
         短=窗口后段裸奔无反馈"
    );
}

#[test]
fn p5_ni_mai_hu_ti_cast_ring_lives_one_reemit_interval_not_whole_buff() {
    let mut app = aura_app();
    cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);

    let rings = ni_mai_rings(&app);
    assert_eq!(rings.len(), 1, "施放应恰好发 1 个首环，实际 {rings:?}");
    assert_eq!(
        ring_duration(&rings[0]),
        Some(NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS),
        "首环寿命应为一个重发间隔（{NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS}t）；\
         若回到整个 buff 窗口 {NI_MAI_HU_TI_BUFF_DURATION_TICKS}t，环就会钉在施法瞬间的\
         坐标上，玩家一移动纹即脱离体表"
    );
    assert!(
        u64::from(NI_MAI_HU_TI_AURA_PARTICLE_LIFETIME_TICKS) < NI_MAI_HU_TI_BUFF_DURATION_TICKS,
        "单环寿命必须短于 buff 窗口，否则无从接力跟随"
    );
}

#[test]
fn p5_ni_mai_hu_ti_aura_cadence_tiles_buff_window_exactly() {
    let mut app = aura_app();
    cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
    clear_vfx_events(&mut app);

    let mut emitted_at = Vec::new();
    for tick in CAST_TICK..=CAST_TICK + NI_MAI_HU_TI_BUFF_DURATION_TICKS + 20 {
        if !tick_aura(&mut app, tick).is_empty() {
            emitted_at.push(tick);
        }
        clear_vfx_events(&mut app);
    }

    assert_eq!(
        emitted_at,
        vec![
            CAST_TICK + 12,
            CAST_TICK + 24,
            CAST_TICK + 36,
            CAST_TICK + 48
        ],
        "重发相位应严格是 cast tick 之后每 {NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS}t 一次\
         且不越过 buff 到期，实际 {emitted_at:?}"
    );
    assert_eq!(
        (emitted_at.len() as u64 + 1) * NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS,
        NI_MAI_HU_TI_BUFF_DURATION_TICKS,
        "首环 + {} 次重发，每环活 {}t，应严格铺满 {}t buff 窗口：\
         不足则窗口中途断纹，超出则护体已过而纹还在转",
        emitted_at.len(),
        NI_MAI_HU_TI_AURA_REEMIT_INTERVAL_TICKS,
        NI_MAI_HU_TI_BUFF_DURATION_TICKS
    );
}

#[test]
fn p5_ni_mai_hu_ti_aura_stays_silent_on_cast_tick_and_off_phase_ticks() {
    let mut app = aura_app();
    cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
    clear_vfx_events(&mut app);

    assert!(
        tick_aura(&mut app, CAST_TICK).is_empty(),
        "cast 同帧系统不得再补一环——emit_burst_av 已发首环，重复即叠成双圈"
    );
    clear_vfx_events(&mut app);
    for offset in [1_u64, 5, 11, 13, 23, 47, 59] {
        let tick = CAST_TICK + offset;
        assert!(
            tick_aura(&mut app, tick).is_empty(),
            "非重发相位 tick {tick}（cast 后第 {offset} tick）不得发环"
        );
        clear_vfx_events(&mut app);
    }
}

#[test]
fn p5_ni_mai_hu_ti_aura_anchor_removed_and_silent_after_buff_expiry() {
    let mut app = aura_app();
    let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
    clear_vfx_events(&mut app);

    let expiry = CAST_TICK + NI_MAI_HU_TI_BUFF_DURATION_TICKS;
    for tick in CAST_TICK..=expiry {
        tick_aura(&mut app, tick);
        clear_vfx_events(&mut app);
    }

    assert!(
        app.world().get::<NiMaiHuTiAura>(caster).is_none(),
        "buff 到期（tick {expiry}）后必须摘除锚点，否则重发会永久持续"
    );
    for tick in expiry + 1..=expiry + 60 {
        assert!(
            tick_aura(&mut app, tick).is_empty(),
            "buff 到期后 tick {tick} 仍在发护体环"
        );
        clear_vfx_events(&mut app);
    }
}

#[test]
fn p5_ni_mai_hu_ti_cast_ring_and_reemit_ring_share_one_form_spec() {
    let mut app = aura_app();
    let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
    let cast_ring = ni_mai_rings(&app).into_iter().next().expect("施放应发首环");
    clear_vfx_events(&mut app);

    let moved = DVec3::new(7.0, 2.0, -4.0);
    move_caster(&mut app, caster, moved);
    let reemit_ring = tick_aura(&mut app, CAST_TICK + 12)
        .into_iter()
        .next()
        .expect("存续期应重发护体环");

    assert_ne!(
        ring_origin(&cast_ring),
        ring_origin(&reemit_ring),
        "施法者已移动，重发环必须换到新位置"
    );

    // 把首环的 origin 换成重发环的，其余字段应完全相等。
    let VfxEventPayloadV1::SpawnParticle {
        event_id,
        direction,
        color,
        strength,
        count,
        duration_ticks,
        ..
    } = cast_ring.clone()
    else {
        panic!("首环应为 SpawnParticle");
    };
    let normalized = VfxEventPayloadV1::SpawnParticle {
        event_id,
        origin: ring_origin(&reemit_ring),
        direction,
        color,
        strength,
        count,
        duration_ticks,
    };
    assert_eq!(
        normalized, reemit_ring,
        "首环与重发环只允许 origin 不同；其余字段（颜色/强度/颗数/寿命）发散会让\
         护体窗口中途的环突然换个模样"
    );
}

#[test]
fn p5_ni_mai_hu_ti_recast_resets_aura_window_without_stacking() {
    let mut app = aura_app();
    let caster = cast_ni_mai_hu_ti(&mut app, DVec3::ZERO);
    clear_vfx_events(&mut app);

    // 冷却 120t 之后再来一发。
    let recast_tick = CAST_TICK + 200;
    app.world_mut().resource_mut::<CombatClock>().tick = recast_tick;
    let result = resolve_ni_mai_hu_ti(app.world_mut(), caster, 0, None);
    assert!(
        matches!(result, CastResult::Started { .. }),
        "冷却已过应可重放：{result:?}"
    );
    clear_vfx_events(&mut app);

    let aura = app
        .world()
        .get::<NiMaiHuTiAura>(caster)
        .copied()
        .expect("重放后锚点仍在");
    assert_eq!(
        (aura.started_at_tick, aura.expires_at_tick),
        (recast_tick, recast_tick + NI_MAI_HU_TI_BUFF_DURATION_TICKS),
        "重放应把窗口整体后移（insert 覆盖语义），而不是保留旧窗口"
    );

    let rings = tick_aura(&mut app, recast_tick + 12);
    assert_eq!(
        rings.len(),
        1,
        "重放后每个相位 tick 仍只发 1 个环，不得叠出两套，实际 {rings:?}"
    );
}
