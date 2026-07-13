//! 定期向 Fabric 客户端推送 `cultivation_detail` CustomPayload（`bong:server_data`）。
//!
//! payload 使用 SoA(parallel arrays) 布局，顺序为 `MeridianSystem::regular` 紧接
//! `MeridianSystem::extraordinary`（plan-race-system-v1 P1c——不再假设恰好 20 条
//! TCM 经脉；数组长度随实体 `MeridianProfile` 变化）。`channel_ids[i]` 携带第 i 条
//! 经脉的 snake_case channel id，客户端按 channel id 而非固定索引还原。
//!
//! 节流：每 20 tick 最多发一次（~1s @ 20TPS）。

use valence::prelude::{
    bevy_ecs, Client, Commands, Component, Entity, Position, Query, Res, ResMut, Resource, With,
};

use crate::body_plan::layout::BodyPlanLayoutRegistry;
use crate::body_plan::{
    resolve_body_plan_for_target, BodyPlanPurpose, BodyPlanRegistry, BodyPlanResolveInputs,
    RaceRegistry,
};
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{
    ColorKind, Contamination, Cultivation, MeridianSystem, QiColor,
};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{
    lifespan_tick_rate_multiplier, LifespanCapTable, LifespanComponent,
};
use crate::cultivation::meridian_open::MeridianTarget;
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::cultivation::SkillMilestoneSnapshotV1;
use crate::schema::server_data::{
    LifespanPreviewV1, PracticeWeightV1, ServerDataPayloadV1, ServerDataV1,
};
use crate::world::zone::ZoneRegistry;

const EMIT_INTERVAL_TICKS: u64 = 20;

#[derive(Default, Resource)]
pub struct CultivationDetailEmitState {
    last_emit_tick: u64,
}

type CultivationDetailEmitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a MeridianSystem,
    &'a Cultivation,
    Option<&'a Contamination>,
    Option<&'a LifespanComponent>,
    Option<&'a Position>,
    Option<&'a LifeRecord>,
    Option<&'a QiColor>,
    Option<&'a PracticeLog>,
    Option<&'a MeridianTarget>,
);

pub fn emit_cultivation_detail_payloads(
    clock: Res<CultivationClock>,
    zones: Option<Res<ZoneRegistry>>,
    body_plans: Option<Res<BodyPlanRegistry>>,
    races: Option<Res<RaceRegistry>>,
    mut state: ResMut<CultivationDetailEmitState>,
    mut clients: Query<CultivationDetailEmitQueryItem<'_>, With<Client>>,
) {
    if clock.tick.saturating_sub(state.last_emit_tick) < EMIT_INTERVAL_TICKS {
        return;
    }
    state.last_emit_tick = clock.tick;

    let zones = zones.as_deref();

    for (
        entity,
        mut client,
        meridians,
        cultivation,
        contamination,
        lifespan,
        position,
        life_record,
        qi_color,
        practice_log,
        meridian_target,
    ) in &mut clients
    {
        let channel_count = meridians.regular.len() + meridians.extraordinary.len();
        let mut channel_ids = Vec::with_capacity(channel_count);
        let mut opened = Vec::with_capacity(channel_count);
        let mut flow_rate = Vec::with_capacity(channel_count);
        let mut flow_capacity = Vec::with_capacity(channel_count);
        let mut integrity = Vec::with_capacity(channel_count);
        let mut open_progress = Vec::with_capacity(channel_count);
        let mut cracks_count = Vec::with_capacity(channel_count);
        for m in meridians
            .regular
            .iter()
            .chain(meridians.extraordinary.iter())
        {
            channel_ids.push(m.id.as_str().to_string());
            opened.push(m.opened);
            flow_rate.push(m.flow_rate);
            flow_capacity.push(m.flow_capacity);
            integrity.push(m.integrity);
            open_progress.push(if m.opened { 1.0 } else { m.open_progress });
            cracks_count.push(u8::try_from(m.cracks.len()).unwrap_or(u8::MAX));
        }

        // plan-race-system-v1 P2a — 实体本体（`BodyPlanPurpose::Intrinsic`）的
        // body_plan_id 随 cultivation_detail 一起下发，供 client 寻址 `BodyPlanLayout`
        // 缓存；玩家客户端从不携带 `BeastKind`，`beast_kind: None` 恒落 resolve 的
        // Tier2（`Cultivation.race`）分支（未知 race 时 `resolve_body_plan_for_target`
        // 优雅退化到 humanoid，见其文档）。
        let resolve_inputs = BodyPlanResolveInputs {
            cultivation: Some(cultivation),
            beast_kind: None,
        };
        let intrinsic_plan = resolve_body_plan_for_target(
            entity,
            BodyPlanPurpose::Intrinsic,
            resolve_inputs,
            body_plans.as_deref(),
            races.as_deref(),
        );
        let body_plan_id = intrinsic_plan.id.as_str().to_string();
        let intrinsic_is_humanoid = intrinsic_plan.is_humanoid;

        // plan-race-system-v1 P3b（决议 §8.1 身份快照 bullet）—— 身份快照五字段：
        // client gate 判定（装备置灰等）的权威真源。`Form` purpose 在 P4 `MorphState`
        // 落地前与 `Intrinsic` 恒等（见 `body_plan::resolve` 模块文档），故当前
        // form_* 三字段必然 = 对应本体字段；P4 落地 MorphState 后本处无需改动，
        // resolve 入口会自动切换到真实易形态数据。
        let form_plan = resolve_body_plan_for_target(
            entity,
            BodyPlanPurpose::Form,
            resolve_inputs,
            body_plans.as_deref(),
            races.as_deref(),
        );
        let form_body_plan_id = form_plan.id.as_str().to_string();
        let form_is_humanoid = form_plan.is_humanoid;
        let race_id = cultivation.race.as_str().to_string();
        // 未易形时 form_race_id = race_id；`Cultivation.race` 是本体权威真源，
        // Form purpose 尚未接入独立 race 追踪（P4 `MorphState` 落地时同步补齐）。
        let form_race_id = race_id.clone();

        let contamination_total = contamination
            .map(|c| c.entries.iter().map(|e| e.amount).sum::<f64>())
            .unwrap_or(0.0);
        let lifespan = lifespan.map(|lifespan| LifespanPreviewV1 {
            years_lived: lifespan.years_lived,
            cap_by_realm: lifespan.cap_by_realm,
            remaining_years: lifespan.remaining_years(),
            death_penalty_years: LifespanCapTable::death_penalty_years_for_cap(
                lifespan.cap_by_realm,
            ),
            tick_rate_multiplier: lifespan_tick_rate_multiplier(position, zones),
            is_wind_candle: lifespan.is_wind_candle(),
        });
        let recent_skill_milestones_summary = life_record
            .map(|life| life.recent_skill_milestones_summary_text(3))
            .unwrap_or_default();
        let skill_milestones = life_record
            .map(|life| {
                let len = life.skill_milestones.len();
                let start = len.saturating_sub(6);
                life.skill_milestones[start..]
                    .iter()
                    .map(SkillMilestoneSnapshotV1::from_runtime)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
            realm: format!("{:?}", cultivation.realm),
            channel_ids,
            opened,
            flow_rate,
            flow_capacity,
            integrity,
            open_progress,
            cracks_count,
            contamination_total,
            lifespan,
            recent_skill_milestones_summary,
            skill_milestones,
            qi_color_main: qi_color
                .map(|color| color.main)
                .unwrap_or(ColorKind::Mellow),
            qi_color_secondary: qi_color.and_then(|color| color.secondary),
            qi_color_chaotic: qi_color.is_some_and(|color| color.is_chaotic),
            qi_color_hunyuan: qi_color.is_some_and(|color| color.is_hunyuan),
            practice_weights: practice_weights_payload(practice_log),
            // plan-race-system-v1 P1c：wire 开放化后 `target_meridian` 直接下发
            // channel id 字符串（与 `channel_ids` 同形态），非 humanoid channel 也能
            // 合法表达，不再依赖 `MeridianId::ALL` 数组下标。
            target_meridian: meridian_target.map(|t| t.0.as_str().to_string()),
            body_plan_id,
            race_id,
            form_race_id,
            form_body_plan_id,
            intrinsic_is_humanoid,
            form_is_humanoid,
        });
        let label = payload_type_label(payload.payload_type());
        let bytes = match serialize_server_data_payload(&payload) {
            Ok(b) => b,
            Err(err) => {
                tracing::warn!(
                    "[bong][network] failed to serialize {label} for {entity:?}: {err:?}"
                );
                continue;
            }
        };

        use valence::ident;
        let _ = SERVER_DATA_CHANNEL; // channel constant, matches ident! literal below
        client.send_custom_payload(ident!("bong:server_data"), &bytes);
    }
}

/// plan-race-system-v1 P2a — 记录上一次给该客户端下发过的 `BodyPlanLayout.body_plan_id`
/// ，用于判定「join 首帧」（组件缺失）与「实体 body_plan 变化时重发」（值不同）两种
/// 触发条件；易形（P4）不改本体 plan，因此不会触发重发（本组件只跟踪
/// `BodyPlanPurpose::Intrinsic` 解析结果，不是形态层）。
#[derive(Debug, Clone, PartialEq, Eq, Component)]
pub struct LastSentBodyPlanLayout(pub String);

/// plan-race-system-v1 P2a — join 首帧随 `cultivation_detail` 附带 `body_plan_id`
/// 供 client 寻址；本系统独立下发对应的 `BodyPlanLayout` payload 本体（剪影 / 经脉图 /
/// server→展示段映射），首帧必发一次，此后仅在实体本体 body_plan 变化（真实换 race）
/// 时重发。缺失 `BodyPlanLayoutRegistry` 资源（大量既有单测未插入）或目标 body_plan_id
/// 没有对应布局数据时优雅跳过（记录警告，不 panic、不阻塞其余 payload 下发）。
type BodyPlanLayoutEmitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    Option<&'a Cultivation>,
    Option<&'a LastSentBodyPlanLayout>,
);

pub fn emit_body_plan_layout_payloads(
    mut commands: Commands,
    body_plans: Option<Res<BodyPlanRegistry>>,
    races: Option<Res<RaceRegistry>>,
    layouts: Option<Res<BodyPlanLayoutRegistry>>,
    mut clients: Query<BodyPlanLayoutEmitQueryItem<'_>, With<Client>>,
) {
    let Some(layouts) = layouts.as_deref() else {
        return;
    };

    for (entity, mut client, cultivation, last_sent) in &mut clients {
        let plan = resolve_body_plan_for_target(
            entity,
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation,
                beast_kind: None,
            },
            body_plans.as_deref(),
            races.as_deref(),
        );
        let body_plan_id = plan.id.as_str().to_string();
        if last_sent.is_some_and(|last| last.0 == body_plan_id) {
            continue;
        }
        let Some(layout) = layouts.get(&plan.id) else {
            tracing::warn!(
                "[bong][network] no BodyPlanLayout registered for body_plan_id={body_plan_id} \
                 — skipping BodyPlanLayout emit for {entity:?}"
            );
            continue;
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::BodyPlanLayout(layout.clone()));
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => {
                send_server_data_payload(&mut client, bytes.as_slice());
                commands
                    .entity(entity)
                    .insert(LastSentBodyPlanLayout(body_plan_id));
            }
            Err(error) => log_payload_build_error(payload_type, &error),
        }
    }
}

fn practice_weights_payload(log: Option<&PracticeLog>) -> Vec<PracticeWeightV1> {
    let Some(log) = log else {
        return Vec::new();
    };
    let total = log.total();
    if total <= f64::EPSILON {
        return Vec::new();
    }
    [
        ColorKind::Sharp,
        ColorKind::Heavy,
        ColorKind::Mellow,
        ColorKind::Solid,
        ColorKind::Light,
        ColorKind::Intricate,
        ColorKind::Gentle,
        ColorKind::Insidious,
        ColorKind::Violent,
        ColorKind::Turbid,
    ]
    .into_iter()
    .filter_map(|color| {
        let weight = log.weights.get(&color).copied().unwrap_or(0.0);
        (weight > 0.0).then_some(PracticeWeightV1 {
            color,
            weight,
            ratio: (weight / total).clamp(0.0, 1.0),
        })
    })
    .collect()
}

#[cfg(test)]
mod body_plan_layout_emit_tests {
    use super::*;
    use crate::body_plan::layout::BodyPlanLayoutRegistry;
    use crate::schema::server_data::{
        BodyPlanPartAnchorV1, BodyPlanPartDisplayMappingV1, BodyPlanPoint2V1,
        BodyPlanSilhouettePartV1,
    };
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn fixture_layouts() -> BodyPlanLayoutRegistry {
        let body_plans =
            BodyPlanRegistry::from_plans(vec![crate::body_plan::humanoid_plan_static().clone()])
                .expect("humanoid plan should validate");
        let layout = crate::schema::server_data::BodyPlanLayoutV1 {
            body_plan_id: "humanoid".to_string(),
            silhouette: vec![BodyPlanSilhouettePartV1 {
                part_id: "chest".to_string(),
                polygon: vec![
                    BodyPlanPoint2V1 { x: 0.3, y: 0.1 },
                    BodyPlanPoint2V1 { x: 0.7, y: 0.1 },
                    BodyPlanPoint2V1 { x: 0.7, y: 0.3 },
                    BodyPlanPoint2V1 { x: 0.3, y: 0.3 },
                ],
            }],
            anchors: vec![BodyPlanPartAnchorV1 {
                part_id: "chest".to_string(),
                point: BodyPlanPoint2V1 { x: 0.5, y: 0.2 },
            }],
            meridian_paths: vec![],
            part_display_map: vec![BodyPlanPartDisplayMappingV1 {
                server_part_id: "chest".to_string(),
                display_segment_id: "chest".to_string(),
            }],
            hud_anchors: vec![],
        };
        BodyPlanLayoutRegistry::from_layouts(vec![layout], &body_plans)
            .expect("fixture layout should validate against humanoid plan")
    }

    /// plan-race-system-v1 P2 minor① 修复 —— 给定 `body_plan_id`/plan 组装一份最小
    /// 合法 layout（供 changed_body_plan_id_resends 测试需要「两个不同 body_plan_id
    /// 各自的 layout」）。
    fn layout_for(body_plan_id: &str) -> crate::schema::server_data::BodyPlanLayoutV1 {
        crate::schema::server_data::BodyPlanLayoutV1 {
            body_plan_id: body_plan_id.to_string(),
            silhouette: vec![BodyPlanSilhouettePartV1 {
                part_id: "chest".to_string(),
                polygon: vec![
                    BodyPlanPoint2V1 { x: 0.3, y: 0.1 },
                    BodyPlanPoint2V1 { x: 0.7, y: 0.1 },
                    BodyPlanPoint2V1 { x: 0.7, y: 0.3 },
                    BodyPlanPoint2V1 { x: 0.3, y: 0.3 },
                ],
            }],
            anchors: vec![BodyPlanPartAnchorV1 {
                part_id: "chest".to_string(),
                point: BodyPlanPoint2V1 { x: 0.5, y: 0.2 },
            }],
            meridian_paths: vec![],
            part_display_map: vec![BodyPlanPartDisplayMappingV1 {
                server_part_id: "chest".to_string(),
                display_segment_id: "chest".to_string(),
            }],
            hud_anchors: vec![],
        }
    }

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_body_plan_layout_payloads(
        helper: &mut MockClientHelper,
    ) -> Vec<crate::schema::server_data::BodyPlanLayoutV1> {
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::BodyPlanLayout(layout) => Some(layout),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn missing_registry_resource_emits_nothing_and_does_not_panic() {
        let mut app = App::new();
        app.add_systems(Update, emit_body_plan_layout_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut()
            .spawn((client_bundle, Cultivation::default()));

        app.update();
        flush_client_packets(&mut app);

        assert!(collect_body_plan_layout_payloads(&mut helper).is_empty());
    }

    #[test]
    fn first_frame_join_emits_layout_once_and_no_registries_falls_back_to_humanoid() {
        let mut app = App::new();
        app.insert_resource(fixture_layouts());
        app.add_systems(Update, emit_body_plan_layout_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        // 不插入 BodyPlanRegistry/RaceRegistry —— `resolve_body_plan_for_target` 优雅
        // 退化到 humanoid，仍应命中 fixture_layouts() 里为 "humanoid" 注册的布局。
        app.world_mut()
            .spawn((client_bundle, Cultivation::default()));

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_body_plan_layout_payloads(&mut helper);
        assert_eq!(payloads.len(), 1, "join 首帧必须恰好发一次 BodyPlanLayout");
        assert_eq!(payloads[0].body_plan_id, "humanoid");
    }

    #[test]
    fn unchanged_body_plan_id_does_not_resend_on_subsequent_ticks() {
        let mut app = App::new();
        app.insert_resource(fixture_layouts());
        app.add_systems(Update, emit_body_plan_layout_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut()
            .spawn((client_bundle, Cultivation::default()));

        app.update();
        flush_client_packets(&mut app);
        assert_eq!(collect_body_plan_layout_payloads(&mut helper).len(), 1);

        app.update();
        flush_client_packets(&mut app);
        assert!(
            collect_body_plan_layout_payloads(&mut helper).is_empty(),
            "第二个 tick body_plan_id 未变化不应重发"
        );
    }

    /// plan-race-system-v1 P2 minor① 修复 —— `LastSentBodyPlanLayout` 重发正向分支
    /// （实体本体 `body_plan_id` 真的变化——真实换 race——时必须重发一次新 layout）
    /// 此前完全没有测试覆盖，只测了"缺失资源"/"不变不重发"/"未知 id 跳过"三条分支。
    /// 这里让 `Cultivation.race` 在两个 tick 之间从 human 变到 otherrace，驱动
    /// `resolve_body_plan_for_target` 解析出不同的 body_plan_id。
    #[test]
    fn changed_body_plan_id_resends() {
        use crate::body_plan::race_registry::RaceEntry;
        use crate::body_plan::{BodyPlanId, RaceId};

        let other_plan = {
            let mut plan = crate::body_plan::humanoid_plan_static().clone();
            plan.id = BodyPlanId::new("other");
            plan
        };
        let body_plans = BodyPlanRegistry::from_plans(vec![
            crate::body_plan::humanoid_plan_static().clone(),
            other_plan,
        ])
        .expect("humanoid + other plan should both validate");

        let races = RaceRegistry::from_parts_for_test(
            vec![
                RaceEntry {
                    id: RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                    display_name: "人族".to_string(),
                    body_plan_id: BodyPlanId::new("humanoid"),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new("otherrace"),
                    display_name: "异构测试种族".to_string(),
                    body_plan_id: BodyPlanId::new("other"),
                    beast_kinds: vec![],
                },
            ],
            vec![],
            &body_plans,
        )
        .expect("race registry with two races should validate");

        let layouts = BodyPlanLayoutRegistry::from_layouts(
            vec![layout_for("humanoid"), layout_for("other")],
            &body_plans,
        )
        .expect("both humanoid and other layouts should validate");

        let mut app = App::new();
        app.insert_resource(body_plans);
        app.insert_resource(races);
        app.insert_resource(layouts);
        app.add_systems(Update, emit_body_plan_layout_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let entity = app
            .world_mut()
            .spawn((client_bundle, Cultivation::default()))
            .id();

        app.update();
        flush_client_packets(&mut app);
        let first = collect_body_plan_layout_payloads(&mut helper);
        assert_eq!(first.len(), 1, "join 首帧必须恰好发一次 BodyPlanLayout");
        assert_eq!(
            first[0].body_plan_id, "humanoid",
            "Cultivation::default().race 是 human，必须解析到 humanoid body_plan_id"
        );

        // 真实换 race：本体 body_plan 从 humanoid 变为 other。
        app.world_mut()
            .get_mut::<Cultivation>(entity)
            .expect("entity must still have Cultivation")
            .race = RaceId::new("otherrace");

        app.update();
        flush_client_packets(&mut app);
        let second = collect_body_plan_layout_payloads(&mut helper);
        assert_eq!(
            second.len(),
            1,
            "body_plan_id 真实变化（换 race）必须重发恰好一次新 layout，不是 0 次（吞更新）也不是 >1 次"
        );
        assert_eq!(
            second[0].body_plan_id, "other",
            "重发的 layout 必须对应变化后的新 body_plan_id，不是旧的 humanoid"
        );
    }

    #[test]
    fn unknown_body_plan_id_in_layout_registry_skips_without_panicking() {
        // registry 里只有一个和真实 humanoid 不匹配的合成 plan/layout（"other"），
        // 目标解析出的 "humanoid" 找不到对应 layout —— 必须优雅跳过，不 panic。
        let other_plan = {
            let mut plan = crate::body_plan::humanoid_plan_static().clone();
            plan.id = "other".into();
            plan
        };
        let body_plans =
            BodyPlanRegistry::from_plans(vec![other_plan]).expect("plan should validate");
        let other_layout = crate::schema::server_data::BodyPlanLayoutV1 {
            body_plan_id: "other".to_string(),
            silhouette: vec![BodyPlanSilhouettePartV1 {
                part_id: "chest".to_string(),
                polygon: vec![
                    BodyPlanPoint2V1 { x: 0.3, y: 0.1 },
                    BodyPlanPoint2V1 { x: 0.7, y: 0.1 },
                    BodyPlanPoint2V1 { x: 0.7, y: 0.3 },
                ],
            }],
            anchors: vec![],
            meridian_paths: vec![],
            part_display_map: vec![],
            hud_anchors: vec![],
        };
        let layouts = BodyPlanLayoutRegistry::from_layouts(vec![other_layout], &body_plans)
            .expect("other layout should validate against other plan");

        let mut app = App::new();
        app.insert_resource(layouts);
        app.add_systems(Update, emit_body_plan_layout_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        // 无 BodyPlanRegistry/RaceRegistry 资源 —— resolve 退化到真实 humanoid，但
        // layout registry 里只登记了 "other"，二者不匹配。
        app.world_mut()
            .spawn((client_bundle, Cultivation::default()));

        app.update();
        flush_client_packets(&mut app);

        assert!(
            collect_body_plan_layout_payloads(&mut helper).is_empty(),
            "未登记对应 body_plan_id 的布局必须跳过而不是 panic"
        );
    }
}

/// plan-race-system-v1 P3b（决议 §8.1 身份快照 bullet）—— `cultivation_detail` 携带的
/// 身份快照五字段（`race_id`/`form_race_id`/`form_body_plan_id`/`intrinsic_is_humanoid`/
/// `form_is_humanoid`）饱和测试。核心不变量：P4 `MorphState` 落地前恒无易形，
/// `resolve_body_plan_for_target(Form)` 与 `Intrinsic` 恒等，故 `form_*` 三字段必须
/// 逐字段等于对应本体字段——任何回归（比如误接了独立的 form 追踪却忘记同步）都会
/// 立刻撞红。
#[cfg(test)]
mod cultivation_detail_identity_snapshot_tests {
    use super::*;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    /// 驱动一次 `emit_cultivation_detail_payloads` 并解出唯一一条 CultivationDetail
    /// payload（测试断言前提：每次调用恰好产出 0 或 1 条，多于 1 条即测试自身写错）。
    fn run_once_and_collect_cultivation_detail(
        app: &mut App,
        helper: &mut MockClientHelper,
    ) -> Vec<(String, String, String, bool, bool)> {
        app.update();
        flush_client_packets(app);
        helper
            .collect_received()
            .0
            .into_iter()
            .filter_map(|frame| {
                let packet = frame.decode::<CustomPayloadS2c>().ok()?;
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    return None;
                }
                let payload = serde_json::from_slice::<ServerDataV1>(packet.data.0 .0).ok()?;
                match payload.payload {
                    ServerDataPayloadV1::CultivationDetail {
                        race_id,
                        form_race_id,
                        form_body_plan_id,
                        intrinsic_is_humanoid,
                        form_is_humanoid,
                        body_plan_id,
                        ..
                    } => Some((
                        race_id,
                        form_race_id,
                        form_body_plan_id,
                        intrinsic_is_humanoid,
                        form_is_humanoid,
                    ))
                    .inspect(|tuple| {
                        // body_plan_id 未在元组里返回，但一并断言其与 form_body_plan_id
                        // 恒等（本体/Form 未易形时 body_plan_id 也必须一致）。
                        assert_eq!(
                            tuple.2, body_plan_id,
                            "form_body_plan_id 必须等于 body_plan_id（未易形态）"
                        );
                    }),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn no_registries_defaults_to_humanoid_human_identity_with_form_equal_intrinsic() {
        // 既有大量单测未插入 BodyPlanRegistry/RaceRegistry 资源——退化路径必须仍
        // 产出自洽的五字段快照（不 panic、不缺字段），且 form_* 恒等 intrinsic_*。
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: EMIT_INTERVAL_TICKS,
        });
        app.init_resource::<CultivationDetailEmitState>();
        app.add_systems(Update, emit_cultivation_detail_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        app.world_mut().spawn((
            client_bundle,
            MeridianSystem::default(),
            Cultivation::default(),
        ));

        let payloads = run_once_and_collect_cultivation_detail(&mut app, &mut helper);
        assert_eq!(payloads.len(), 1, "首次 tick 必须发一条 cultivation_detail");
        let (race_id, form_race_id, form_body_plan_id, intrinsic_is_humanoid, form_is_humanoid) =
            &payloads[0];

        assert_eq!(
            race_id, "human",
            "Cultivation::default().race 是 human，必须原样下发"
        );
        assert_eq!(
            form_race_id, race_id,
            "P4 MorphState 落地前未易形，form_race_id 必须等于 race_id"
        );
        assert_eq!(
            form_body_plan_id, "humanoid",
            "退化路径必须解析到 humanoid body_plan_id"
        );
        assert!(intrinsic_is_humanoid, "human race 的本体必须是人形");
        assert_eq!(
            *form_is_humanoid, *intrinsic_is_humanoid,
            "P4 MorphState 落地前未易形，form_is_humanoid 必须等于 intrinsic_is_humanoid"
        );
    }

    #[test]
    fn custom_race_id_propagates_to_both_race_and_form_race_fields() {
        // 换一个非默认 race_id（不依赖真实 RaceRegistry 里是否登记 humanoid 之外的
        // 构型——本测试只锁"race_id 原样透传 + form_race_id 跟随"这条契约，不测
        // Registry 解析本身，那部分由 body_plan::resolve 自己的单测覆盖）。
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: EMIT_INTERVAL_TICKS,
        });
        app.init_resource::<CultivationDetailEmitState>();
        app.add_systems(Update, emit_cultivation_detail_payloads);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let cultivation = Cultivation {
            race: crate::body_plan::types::RaceId::new("whale"),
            ..Cultivation::default()
        };
        app.world_mut()
            .spawn((client_bundle, MeridianSystem::default(), cultivation));

        let payloads = run_once_and_collect_cultivation_detail(&mut app, &mut helper);
        assert_eq!(payloads.len(), 1);
        let (race_id, form_race_id, _, _, _) = &payloads[0];
        assert_eq!(race_id, "whale");
        assert_eq!(
            form_race_id, "whale",
            "form_race_id 必须跟随本体 race_id（未易形态）"
        );
    }
}
