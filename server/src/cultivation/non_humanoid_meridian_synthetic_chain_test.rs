//! plan-race-system-v1 P1 对抗审查 M2 —— 非人 plan 合成样本（6 脉构型）全链测试。
//!
//! §P1 交付物原文明写「非人 plan 合成样本（6 脉构型）走通开脉 → 突破配额 → severed
//! 全链」；对抗审查发现该测试此前缺失（P1a/b 只对拍了 humanoid 20 经）。本文件用
//! 一个**合成**（非 humanoid、非生产数据）6-channel `MeridianProfile`（4 正经 + 2
//! 奇经，草案数值对齐 §8.1 #8 whale 曲线形态：醒灵1/引气2/凝脉3/固元6/通灵6/化虚6）
//! 跑通：
//!   ① `MeridianSystem::for_profile` 建骨架（对该 profile，非 humanoid 骨架）
//!   ② 逐脉打开（按 `MeridianTopology::from_edges` 拓扑邻接校验）
//!   ③ `breakthrough_precondition_error_for_profile` 按该 profile 判定突破配额
//!      （不是 humanoid 曲线）
//!   ④ severed 登记 / 门控不 panic（`MeridianSeveredPermanent` + severed detection
//!      system 对非 humanoid channel 安全跳过而非 panic）
//!   ⑤ `npc_meridian_system_for_realm` 用该 profile 生成 NPC MeridianSystem 不 panic
//!      且数值来自该 profile 自身（不是 humanoid 曲线）

use valence::prelude::{App, Entity, Events, Update};

use crate::body_plan::types::{
    BodyPartDef, BodyPlan, BodyPlanId, ChannelDef, HeightBand, HeightBandAssignment, HitGeometry,
    MeridianFamily, MeridianProfile, PartConsequence, RealmMeridianReq, StandingAabbSpec,
    TopologyEdge,
};
use crate::cultivation::breakthrough::{
    breakthrough_precondition_error_for_profile, try_breakthrough_with_profile, BreakthroughError,
    RollSource,
};
use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
use crate::cultivation::meridian::severed::{
    meridian_severed_detection_tick, MeridianSeveredEvent, MeridianSeveredPermanent,
};
use crate::cultivation::tick::CultivationClock;
use crate::cultivation::topology::MeridianTopology;
use crate::npc::technique::npc_meridian_system_for_realm;

/// 6-channel 合成构型：`fin_1..fin_4`（Regular）+ `blowhole`/`tail_core`
/// （Extraordinary）。数值与体表映射均为测试合成数据，不代表任何生产种族。
fn synthetic_whale_profile() -> MeridianProfile {
    let channels = vec![
        ChannelDef {
            id: "fin_1".into(),
            family: MeridianFamily::Regular,
            body_part: None,
            roles: vec![],
        },
        ChannelDef {
            id: "fin_2".into(),
            family: MeridianFamily::Regular,
            body_part: None,
            roles: vec![],
        },
        ChannelDef {
            id: "fin_3".into(),
            family: MeridianFamily::Regular,
            body_part: None,
            roles: vec![],
        },
        ChannelDef {
            id: "fin_4".into(),
            family: MeridianFamily::Regular,
            body_part: None,
            roles: vec![],
        },
        ChannelDef {
            id: "blowhole".into(),
            family: MeridianFamily::Extraordinary,
            body_part: None,
            roles: vec![],
        },
        ChannelDef {
            id: "tail_core".into(),
            family: MeridianFamily::Extraordinary,
            body_part: None,
            roles: vec![],
        },
    ];
    let topology_edges = vec![
        TopologyEdge {
            from: "fin_1".into(),
            to: "fin_2".into(),
        },
        TopologyEdge {
            from: "fin_2".into(),
            to: "fin_3".into(),
        },
        TopologyEdge {
            from: "fin_3".into(),
            to: "fin_4".into(),
        },
        TopologyEdge {
            from: "fin_4".into(),
            to: "blowhole".into(),
        },
        TopologyEdge {
            from: "blowhole".into(),
            to: "tail_core".into(),
        },
    ];
    // 曲线形态参考 §8.1 #8 whale 草案（醒灵1 / 引气2 / 凝脉3 / … / 化虚全通），但
    // 固元/通灵档刻意设 total=5（< 全通 6）且带 3+2 子配额——若 total 与全通相等，
    // 任何子配额缺口都会先撞 NotEnoughMeridians，"total 已达标但奇经子配额不足"的
    // 判定分支永远测不到（本文件是合成测试构型，不是生产 whale 数据）。
    let realm_requirements = [
        RealmMeridianReq {
            total: 1,
            regular_min: 0,
            extraordinary_min: 0,
        },
        RealmMeridianReq {
            total: 2,
            regular_min: 0,
            extraordinary_min: 0,
        },
        RealmMeridianReq {
            total: 3,
            regular_min: 0,
            extraordinary_min: 0,
        },
        RealmMeridianReq {
            total: 5,
            regular_min: 3,
            extraordinary_min: 2,
        },
        RealmMeridianReq {
            total: 5,
            regular_min: 3,
            extraordinary_min: 2,
        },
        RealmMeridianReq {
            total: 6,
            regular_min: 4,
            extraordinary_min: 2,
        },
    ];
    MeridianProfile {
        channels,
        topology_edges,
        realm_requirements,
        dugu_injection: vec![],
    }
}

fn synthetic_whale_body_plan() -> BodyPlan {
    BodyPlan {
        id: BodyPlanId::new("synthetic_test_whale"),
        display_name: "合成测试鲸".to_string(),
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
        meridian_profile: Some(synthetic_whale_profile()),
        mutation_slot_mapping: Default::default(),
    }
}

// ─── ① for_profile 建骨架 ─────────────────────────────────────────────────────

#[test]
fn for_profile_builds_non_humanoid_skeleton_with_correct_channel_split() {
    let profile = synthetic_whale_profile();
    let sys = MeridianSystem::for_profile(&profile);
    assert_eq!(
        sys.regular.len(),
        4,
        "4 条合成 fin channel 应落入 regular 桶"
    );
    assert_eq!(
        sys.extraordinary.len(),
        2,
        "blowhole/tail_core 应落入 extraordinary 桶"
    );
    assert_eq!(sys.opened_count(), 0, "初建骨架全部未开");
    // 未知（humanoid）channel 不应出现在这个非人骨架里。
    assert!(!sys.contains(crate::cultivation::components::MeridianId::Lung.channel_id()));
}

// ─── ② 逐脉打开 + 拓扑邻接校验 ────────────────────────────────────────────────

#[test]
fn opening_channels_in_topology_order_succeeds_without_panicking() {
    let profile = synthetic_whale_profile();
    let mut sys = MeridianSystem::for_profile(&profile);
    let topo = MeridianTopology::from_edges(&profile.topology_edges);

    // fin_1 是根，无邻接前置——直接打通。
    assert!(topo.contains("fin_1"));
    sys.get_mut("fin_1").opened = true;

    // fin_2 邻接 fin_1（已开）——按拓扑合法打通。
    assert!(topo
        .neighbors("fin_1")
        .iter()
        .any(|c| c.as_str() == "fin_2"));
    sys.get_mut("fin_2").opened = true;
    sys.get_mut("fin_3").opened = true;
    sys.get_mut("fin_4").opened = true;
    sys.get_mut("blowhole").opened = true;
    sys.get_mut("tail_core").opened = true;

    assert_eq!(sys.opened_count(), 6);
    assert_eq!(sys.regular_opened_count(), 4);
    assert_eq!(sys.extraordinary_opened_count(), 2);
}

// ─── ③ breakthrough 配额按该 profile 判定（不是 humanoid 曲线） ───────────────

#[test]
fn breakthrough_precondition_uses_synthetic_profile_quota_not_humanoid_curve() {
    let profile = synthetic_whale_profile();
    let mut sys = MeridianSystem::for_profile(&profile);

    // Awaken -> Induce 需要 total=2（本 profile 的 realm_requirements[1]），humanoid
    // 曲线该档是 3——如果这里误读 humanoid 曲线，下面断言会用错误的 need 值撞红。
    let cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 1_000.0,
        qi_max: 1_000.0,
        ..Default::default()
    };

    // 只开 1 条：未达 need=2，应被拒。
    sys.get_mut("fin_1").opened = true;
    let err = breakthrough_precondition_error_for_profile(&cultivation, &sys, &profile);
    assert_eq!(
        err,
        Some(BreakthroughError::NotEnoughMeridians { need: 2, have: 1 }),
        "本 profile Induce 档 total=2（非 humanoid 曲线的 3），需要按 profile 自身判定"
    );

    // 开满 2 条：应放行（qi 充足）。
    sys.get_mut("fin_2").opened = true;
    let err = breakthrough_precondition_error_for_profile(&cultivation, &sys, &profile);
    assert_eq!(err, None, "开满该 profile 的 Induce 配额后应放行");
}

#[test]
fn breakthrough_precondition_rejects_missing_extraordinary_sub_quota() {
    let profile = synthetic_whale_profile();
    let mut sys = MeridianSystem::for_profile(&profile);
    // Solidify -> Spirit 档（realm_requirements[4]）要求 total=5 / regular_min=3 /
    // extraordinary_min=2。开 4 正经 + 1 奇经 = total 5 已达标、regular 4≥3 已达标，
    // 唯独 extraordinary 1<2——精确命中子配额判定分支（total 分支已被满足，不会先撞）。
    for id in ["fin_1", "fin_2", "fin_3", "fin_4"] {
        sys.get_mut(id).opened = true;
    }
    sys.get_mut("blowhole").opened = true; // 只开 1 条奇经，未达 extraordinary_min=2

    let cultivation = Cultivation {
        realm: Realm::Solidify,
        qi_current: 1_000.0,
        qi_max: 1_000.0,
        ..Default::default()
    };
    let err = breakthrough_precondition_error_for_profile(&cultivation, &sys, &profile);
    assert_eq!(
        err,
        Some(BreakthroughError::NotEnoughExtraordinaryMeridians { need: 2, have: 1 }),
        "期望 NotEnoughExtraordinaryMeridians{{need:2,have:1}}：total(5/5) 与 regular(4/3) \
         都已达标，只有奇经子配额缺口——若返回其他错误说明子配额判定没按本 profile 走"
    );
}

// ─── ④ severed 登记 / 门控不 panic ────────────────────────────────────────────

#[test]
fn severed_detection_skips_non_humanoid_channel_safely_without_panicking() {
    let profile = synthetic_whale_profile();
    let mut sys = MeridianSystem::for_profile(&profile);
    // "tail_core" integrity 归零 + 一条 crack 历史 —— 触发 detection system 的 SEVERED
    // 判定分支；tail_core 不在 humanoid 20 条经脉之列，`to_meridian_id()` 必为 None。
    {
        let m = sys.get_mut("tail_core");
        m.opened = true;
        m.integrity = 0.0;
        m.cracks
            .push(crate::cultivation::components::MeridianCrack {
                severity: 1.0,
                healing_progress: 0.0,
                cause: crate::cultivation::components::CrackCause::Overload,
                created_at: 3,
            });
    }

    let mut app = App::new();
    app.insert_resource(CultivationClock { tick: 3 });
    app.add_event::<MeridianSeveredEvent>();
    app.add_systems(Update, meridian_severed_detection_tick);
    let entity: Entity = app
        .world_mut()
        .spawn((sys, MeridianSeveredPermanent::default()))
        .id();

    // 必须不 panic —— 这是本用例的核心断言（B1/M3/M4 关注点）。
    app.update();

    let events = app.world().resource::<Events<MeridianSeveredEvent>>();
    let mut reader = events.get_reader();
    let collected: Vec<_> = reader.read(events).collect();
    assert!(
        collected.is_empty(),
        "非 humanoid channel 没有 legacy MeridianId 映射，detection system 必须安全跳过\
         （不广播一个假造的 event），实际收到 {} 条",
        collected.len()
    );
    // entity 变量仅用于让 spawn 结果保持可读；系统按 Query 迭代，不需要显式引用。
    let _ = entity;
}

// ─── ⑤ npc_meridian_system_for_realm 用非人 profile 生成不 panic ─────────────

#[test]
fn npc_meridian_system_for_realm_uses_synthetic_profile_not_humanoid_default() {
    let plan = synthetic_whale_body_plan();

    // Awaken 档：本 profile total=1（humanoid 曲线是 1，容易掩盖 bug）——用 Condense
    // 档（本 profile total=3，humanoid 曲线是 6）更能暴露"误用 humanoid 骨架/曲线"。
    let sys = npc_meridian_system_for_realm(Realm::Condense, &plan);
    assert_eq!(
        sys.regular.len(),
        4,
        "骨架必须来自本 profile（4 正经），不是 humanoid 12 正经"
    );
    assert_eq!(
        sys.extraordinary.len(),
        2,
        "骨架必须来自本 profile（2 奇经），不是 humanoid 8 奇经"
    );
    assert_eq!(
        sys.opened_count(),
        3,
        "Condense 档 need=3 来自本 profile 的 realm_requirements（humanoid 曲线是 6，\
         若这里等于 6 说明仍在误用 humanoid 曲线）"
    );

    // Void 档：本 profile total=6（全通）。
    let sys_void = npc_meridian_system_for_realm(Realm::Void, &plan);
    assert_eq!(sys_void.opened_count(), 6);
}

// ─── ⑥ try_breakthrough_with_profile 全链（非 breakthrough_precondition_error_for_profile
//      单独判定，跑通实际扣费/开境/失败裂痕的完整突破流程） ─────────────────────

struct FixedRoll(f64);
impl RollSource for FixedRoll {
    fn roll_unit(&mut self) -> f64 {
        self.0
    }
}

/// plan-race-system-v1 P5 —— whale 等非人构型走通突破全链：本 profile Awaken→Induce
/// 档 need=2（非 humanoid 曲线的 3），只开 2 条即应成功突破，且真元/境界按该 profile
/// 曲线变化（不是 humanoid 曲线）。
#[test]
fn try_breakthrough_with_profile_succeeds_for_non_humanoid_target_at_its_own_quota() {
    let profile = synthetic_whale_profile();
    let mut sys = MeridianSystem::for_profile(&profile);
    sys.get_mut("fin_1").opened = true;
    sys.get_mut("fin_2").opened = true; // 本 profile need=2，humanoid 曲线是 3——若误
                                        // 用 humanoid 曲线本测试会在这里就地撞
                                        // NotEnoughMeridians 而非跑到成功分支。

    let mut cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 1_000.0,
        qi_max: 1_000.0,
        composure: 1.0,
        ..Default::default()
    };

    let result = try_breakthrough_with_profile(
        &mut cultivation,
        &mut sys,
        0.0,
        0.0,
        None,
        &profile,
        &mut FixedRoll(0.0), // roll=0.0 <= 任意 success_rate，必然成功
    );

    let success = result.unwrap_or_else(|error| {
        panic!("非人构型（本 profile need=2）满足自身配额后应突破成功，实际 Err({error:?})")
    });
    assert_eq!(
        success.to,
        Realm::Induce,
        "突破成功后应到 Induce（本 profile Awaken→Induce 档），实际 {:?}",
        success.to
    );
    assert_eq!(
        success.used_qi, 8.0,
        "Induce 档 breakthrough_qi_cost=8.0（与境界曲线无关的独立常量表），实际 {}",
        success.used_qi
    );
    assert_eq!(
        cultivation.realm,
        Realm::Induce,
        "突破成功后 cultivation.realm 必须切到 Induce"
    );
}

/// 未达本 profile 自身配额（只开 1 条，need=2）时必须拒绝——即便 humanoid 曲线在
/// 同一档只需要更少经脉，也不能让非人构型"借用" humanoid 更宽松的配额。
#[test]
fn try_breakthrough_with_profile_rejects_non_humanoid_target_below_its_own_quota() {
    let profile = synthetic_whale_profile();
    let mut sys = MeridianSystem::for_profile(&profile);
    sys.get_mut("fin_1").opened = true; // 只开 1 条，本 profile need=2

    let mut cultivation = Cultivation {
        realm: Realm::Awaken,
        qi_current: 1_000.0,
        qi_max: 1_000.0,
        composure: 1.0,
        ..Default::default()
    };

    let err = try_breakthrough_with_profile(
        &mut cultivation,
        &mut sys,
        0.0,
        0.0,
        None,
        &profile,
        &mut FixedRoll(0.0),
    )
    .unwrap_err();

    assert_eq!(
        err,
        BreakthroughError::NotEnoughMeridians { need: 2, have: 1 },
        "非人构型未达自身 profile 配额（need=2）必须被拒绝，实际 {err:?}"
    );
    assert_eq!(cultivation.realm, Realm::Awaken, "配额门未过时境界不应改变");
}

/// plan-race-system-v1 P5 换轨 —— human 行为不变回归锁：`try_breakthrough_with_profile`
/// 显式传入 humanoid profile 时，与换轨前 `try_breakthrough`（走零参
/// `Realm::required_meridians()` 硬编码 humanoid 曲线）的 need 值逐档 bit-for-bit
/// 相等——这是本轮换轨"不改人族数值"的核心断言：任何把 humanoid 曲线改样的回归都会
/// 让下面某一档撞红。
#[test]
fn try_breakthrough_with_profile_humanoid_need_matches_realm_required_meridians_every_rank() {
    let humanoid_profile = crate::body_plan::humanoid_plan_static()
        .meridian_profile
        .as_ref()
        .expect("humanoid plan must declare meridian_profile");
    let ranked = [
        (Realm::Awaken, Realm::Induce),
        (Realm::Induce, Realm::Condense),
        (Realm::Condense, Realm::Solidify),
        (Realm::Solidify, Realm::Spirit),
        (Realm::Spirit, Realm::Void),
    ];
    for (_from, next) in ranked {
        let profile_need =
            humanoid_profile.realm_requirements[next.rank() as usize - 1].total as usize;
        assert_eq!(
            profile_need,
            next.required_meridians(),
            "换轨后 humanoid profile 派生的 need（{next:?} 档）必须与\
             Realm::required_meridians() 零参硬编码路径 bit-for-bit 相等，\
             实际 profile={profile_need} vs required_meridians={}",
            next.required_meridians()
        );
    }
}
