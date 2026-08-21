//! 玩家加入时可选推送 alchemy 6 个 mock 快照（plan-alchemy-v1 §4 vertical slice）。
//!
//! 默认不推 mock：这类快照会进入全局 HUD，若在正常本地联调里自动下发，会让玩家
//! 一进服就看到假的炼丹进度和结算 toast。需要验证双端通路时显式设置
//! `BONG_ALCHEMY_JOIN_MOCKS=1`。
//!
//! 后续切片：
//!   * 改成响应 `AlchemyOpenFurnace` 请求（按需推）
//!   * 接 ECS — 从真实 `AlchemyFurnace` / `AlchemySession` Component 取数据
//!   * 配合 alchemy_emit_state 增量推送（只推变化字段）

use valence::prelude::{Added, Client, Entity, EventReader, Query, Username, With};

use crate::alchemy::{AlchemyOutcomeEvent, AlchemySession, RecipeRegistry, ResolvedOutcome};
use crate::cultivation::components::ColorKind;
use crate::inventory::PlayerInventory;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::canonical_player_id;
use crate::schema::alchemy::{
    AlchemyContaminationDataV1, AlchemyContaminationLevelV1, AlchemyFurnaceDataV1,
    AlchemyOutcomeBucketV1, AlchemyOutcomeForecastDataV1, AlchemyOutcomeResolvedDataV1,
    AlchemyRecipeBookDataV1, AlchemyRecipeEntryV1, AlchemySessionDataV1, AlchemyStageHintV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type JoinedClientQueryItem<'a> = (Entity, &'a mut Client, &'a Username);
const ALCHEMY_JOIN_MOCKS_ENV_VAR: &str = "BONG_ALCHEMY_JOIN_MOCKS";

pub fn emit_join_alchemy_snapshots(
    mut joined_clients: Query<JoinedClientQueryItem<'_>, (With<Client>, Added<PlayerInventory>)>,
) {
    if !alchemy_join_mocks_enabled() {
        return;
    }
    for (entity, mut client, username) in &mut joined_clients {
        let player_id = canonical_player_id(username.0.as_str());
        let payloads = [
            ServerDataV1::new(ServerDataPayloadV1::AlchemyFurnace(Box::new(mock_furnace(
                &player_id,
            )))),
            ServerDataV1::new(ServerDataPayloadV1::AlchemySession(
                Box::new(mock_session()),
            )),
            ServerDataV1::new(ServerDataPayloadV1::AlchemyOutcomeForecast(Box::new(
                mock_forecast(),
            ))),
            ServerDataV1::new(ServerDataPayloadV1::AlchemyRecipeBook(Box::new(
                mock_recipe_book(),
            ))),
            ServerDataV1::new(ServerDataPayloadV1::AlchemyContamination(Box::new(
                mock_contamination(),
            ))),
            ServerDataV1::new(ServerDataPayloadV1::AlchemyOutcomeResolved(Box::new(
                mock_outcome_resolved(),
            ))),
        ];
        for payload in &payloads {
            let label = payload_type_label(payload.payload_type());
            let bytes = match serialize_server_data_payload(payload) {
                Ok(b) => b,
                Err(err) => {
                    log_payload_build_error(label, &err);
                    continue;
                }
            };
            send_server_data_payload(&mut client, bytes.as_slice());
            tracing::info!(
                "[bong][network] sent {} {} payload to client entity {entity:?} for `{}`",
                SERVER_DATA_CHANNEL,
                label,
                player_id
            );
        }
    }
}

fn alchemy_join_mocks_enabled() -> bool {
    std::env::var(ALCHEMY_JOIN_MOCKS_ENV_VAR)
        .ok()
        .is_some_and(|raw| alchemy_join_mocks_enabled_value(raw.as_str()))
}

fn alchemy_join_mocks_enabled_value(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn mock_furnace(owner: &str) -> AlchemyFurnaceDataV1 {
    AlchemyFurnaceDataV1 {
        pos: Some((-12, 64, 38)),
        tier: 1,
        integrity: 88.0,
        integrity_max: 100.0,
        owner_name: owner.to_string(),
        has_session: true,
    }
}

pub fn send_furnace_from_furnace(
    client: &mut Client,
    player_id: &str,
    furnace: &crate::alchemy::AlchemyFurnace,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::AlchemyFurnace(Box::new(
        AlchemyFurnaceDataV1 {
            pos: furnace.pos,
            tier: furnace.tier,
            integrity: furnace.integrity * 100.0,
            integrity_max: 100.0,
            owner_name: furnace.owner.clone().unwrap_or_default(),
            has_session: furnace.session.is_some(),
        },
    )));
    send_payload(client, &payload, player_id);
}

fn mock_session() -> AlchemySessionDataV1 {
    AlchemySessionDataV1 {
        recipe_id: Some("kai_mai_pill_v0".into()),
        active: true,
        elapsed_ticks: 64,
        target_ticks: 200,
        temp_current: 0.55,
        temp_target: 0.60,
        temp_band: 0.10,
        qi_injected: 4.5,
        qi_target: 15.0,
        status_label: "server-driven".into(),
        stages: vec![AlchemyStageHintV1 {
            at_tick: 0,
            window: 0,
            summary: "ci_she_hao×3 + ling_shui×1".into(),
            completed: true,
            missed: false,
        }],
        interventions_recent: vec![
            "§7[t+12] AdjustTemp(0.55) ← server".into(),
            "§7[t+45] InjectQi(4.5) ← server".into(),
        ],
    }
}

fn mock_forecast() -> AlchemyOutcomeForecastDataV1 {
    AlchemyOutcomeForecastDataV1 {
        perfect_pct: 22.0,
        good_pct: 51.0,
        flawed_pct: 20.0,
        waste_pct: 6.0,
        explode_pct: 1.0,
        perfect_note: "q1.0 · Mellow 0.30 (server)".into(),
        good_note: "q0.7 · Mellow 0.50 (server)".into(),
        flawed_note: "q0.4 · Turbid 0.80 (server)".into(),
    }
}

fn mock_recipe_book() -> AlchemyRecipeBookDataV1 {
    mock_recipe_book_at(0)
}

fn mock_recipe_book_at(idx: i32) -> AlchemyRecipeBookDataV1 {
    let learned = vec![
        AlchemyRecipeEntryV1 {
            id: "kai_mai_pill_v0".into(),
            display_name: "开脉丹方 (server)".into(),
            body_text: "§e服务器推送的开脉丹方文案".into(),
            author: "散修 刘三".into(),
            era: "末法 十二年".into(),
            max_known: 8,
        },
        AlchemyRecipeEntryV1 {
            id: "hui_yuan_pill_v0".into(),
            display_name: "回元丹方 (server)".into(),
            body_text: "§e服务器推送的回元丹方文案 — 翻页可见".into(),
            author: "散修 刘三".into(),
            era: "末法 十二年".into(),
            max_known: 8,
        },
    ];
    let n = learned.len() as i32;
    let normalized = idx.rem_euclid(n.max(1));
    AlchemyRecipeBookDataV1 {
        learned,
        current_index: normalized as u32,
    }
}

/// 真 ECS 路径:从 `LearnedRecipes` Component 构造并推送 recipe_book。
pub fn send_recipe_book_from_learned(
    client: &mut Client,
    player_id: &str,
    learned: &crate::alchemy::LearnedRecipes,
) {
    let entries: Vec<AlchemyRecipeEntryV1> = learned
        .ids
        .iter()
        .map(|id| AlchemyRecipeEntryV1 {
            id: id.clone(),
            display_name: id.clone(),
            body_text: format!("§7{id} (server-driven)"),
            author: "本人".into(),
            era: "末法".into(),
            max_known: 8,
        })
        .collect();
    let payload = ServerDataV1::new(ServerDataPayloadV1::AlchemyRecipeBook(Box::new(
        AlchemyRecipeBookDataV1 {
            learned: entries,
            current_index: learned.current_index as u32,
        },
    )));
    send_payload(client, &payload, player_id);
}

/// 真 ECS 路径:从 `AlchemyFurnace` Component (含 session) 构造并推送 session payload。
pub fn send_session_from_furnace(
    client: &mut Client,
    player_id: &str,
    furnace: &crate::alchemy::AlchemyFurnace,
    registry: &RecipeRegistry,
) {
    send_session(client, player_id, furnace.session.as_ref(), registry);
}

/// 收炉后的 session 已从炉组件移除；仍向客户端推送一次完整的 inactive 结束快照。
pub fn send_session_from_completed_session(
    client: &mut Client,
    player_id: &str,
    session: &AlchemySession,
    registry: &RecipeRegistry,
) {
    send_session(client, player_id, Some(session), registry);
}

fn send_session(
    client: &mut Client,
    player_id: &str,
    session: Option<&AlchemySession>,
    registry: &RecipeRegistry,
) {
    let data = build_session_data(session, registry);
    let payload = ServerDataV1::new(ServerDataPayloadV1::AlchemySession(Box::new(data)));
    send_payload(client, &payload, player_id);
}

fn build_session_data(
    session: Option<&AlchemySession>,
    registry: &RecipeRegistry,
) -> AlchemySessionDataV1 {
    match session {
        Some(session) => {
            let interventions_recent = session
                .interventions
                .iter()
                .rev()
                .take(3)
                .rev()
                .map(|intervention| format!("§7{intervention:?}"))
                .collect();
            let Some(recipe) = registry.get(&session.recipe) else {
                tracing::warn!(
                    "[bong][network][alchemy] session references unknown recipe `{}`; sending inactive snapshot",
                    session.recipe
                );
                return AlchemySessionDataV1 {
                    recipe_id: Some(session.recipe.clone()),
                    active: false,
                    elapsed_ticks: session.elapsed_ticks,
                    target_ticks: 0,
                    temp_current: session.temp_current,
                    temp_target: 0.0,
                    temp_band: 0.0,
                    qi_injected: session.qi_injected,
                    qi_target: 0.0,
                    status_label: "丹方数据缺失".into(),
                    stages: vec![],
                    interventions_recent,
                };
            };

            AlchemySessionDataV1 {
                recipe_id: Some(session.recipe.clone()),
                active: !session.finished,
                elapsed_ticks: session.elapsed_ticks,
                target_ticks: recipe.fire_profile.target_duration_ticks,
                temp_current: session.temp_current,
                temp_target: recipe.fire_profile.target_temp,
                temp_band: recipe.fire_profile.tolerance.temp_band,
                qi_injected: session.qi_injected,
                qi_target: recipe.fire_profile.qi_cost,
                status_label: if session.finished {
                    "已结束".into()
                } else {
                    "炼制中".into()
                },
                stages: recipe
                    .stages
                    .iter()
                    .enumerate()
                    .map(|(stage_index, stage)| AlchemyStageHintV1 {
                        at_tick: stage.at_tick,
                        window: stage.window,
                        summary: stage
                            .required
                            .iter()
                            .map(|ingredient| {
                                format!("{}×{}", ingredient.material, ingredient.count)
                            })
                            .collect::<Vec<_>>()
                            .join(" + "),
                        completed: session.staged.completed_stages.contains(&stage_index),
                        missed: session.staged.missed_stages.contains(&stage_index),
                    })
                    .collect(),
                interventions_recent,
            }
        }
        None => AlchemySessionDataV1 {
            recipe_id: None,
            active: false,
            elapsed_ticks: 0,
            target_ticks: 0,
            temp_current: 0.0,
            temp_target: 0.0,
            temp_band: 0.0,
            qi_injected: 0.0,
            qi_target: 0.0,
            status_label: "未起炉".into(),
            stages: vec![],
            interventions_recent: vec![],
        },
    }
}

fn send_payload(client: &mut Client, payload: &ServerDataV1, player_id: &str) {
    let label = payload_type_label(payload.payload_type());
    let bytes = match serialize_server_data_payload(payload) {
        Ok(b) => b,
        Err(err) => {
            log_payload_build_error(label, &err);
            return;
        }
    };
    send_server_data_payload(client, bytes.as_slice());
    tracing::info!("[bong][network] re-sent {label} payload for `{player_id}`");
}

/// 公共入口：处理 `AlchemyTurnPage` 后用此函数重推 recipe_book 给 client。
pub fn send_recipe_book(client: &mut Client, player_id: &str, idx: i32) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::AlchemyRecipeBook(Box::new(
        mock_recipe_book_at(idx),
    )));
    let label = payload_type_label(payload.payload_type());
    let bytes = match serialize_server_data_payload(&payload) {
        Ok(b) => b,
        Err(err) => {
            log_payload_build_error(label, &err);
            return;
        }
    };
    send_server_data_payload(client, bytes.as_slice());
    tracing::info!(
        "[bong][network] re-sent {} payload (idx={idx}) for `{player_id}`",
        label
    );
}

fn mock_contamination() -> AlchemyContaminationDataV1 {
    AlchemyContaminationDataV1 {
        levels: vec![
            AlchemyContaminationLevelV1 {
                color: ColorKind::Mellow,
                current: 0.22,
                max: 0.6,
                ok: true,
            },
            AlchemyContaminationLevelV1 {
                color: ColorKind::Violent,
                current: 0.10,
                max: 1.0,
                ok: true,
            },
        ],
        metabolism_note: "服务器推送 · 代谢同 cultivation contamination_tick".into(),
    }
}

pub fn emit_alchemy_outcome_resolved(
    mut events: EventReader<AlchemyOutcomeEvent>,
    mut clients: Query<(&Username, &mut Client), With<Client>>,
) {
    for event in events.read() {
        let Some((_username, mut client)) = clients
            .iter_mut()
            .find(|(username, _)| username_matches_caster(username.0.as_str(), &event.caster_id))
        else {
            tracing::warn!(
                "[bong][network][alchemy] resolved outcome for unknown caster `{}`",
                event.caster_id
            );
            continue;
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::AlchemyOutcomeResolved(Box::new(
            build_outcome_resolved_data(event),
        )));
        send_payload(&mut client, &payload, event.caster_id.as_str());
    }
}

fn username_matches_caster(username: &str, caster_id: &str) -> bool {
    caster_id == username || canonical_player_id(username) == caster_id
}

fn build_outcome_resolved_data(event: &AlchemyOutcomeEvent) -> AlchemyOutcomeResolvedDataV1 {
    let mut data = AlchemyOutcomeResolvedDataV1 {
        bucket: event.bucket.into(),
        recipe_id: event.recipe_id.clone(),
        pill: None,
        quality: None,
        toxin_amount: None,
        toxin_color: None,
        qi_gain: None,
        side_effect_tag: None,
        flawed_path: false,
        damage: None,
        meridian_crack: None,
    };

    match &event.outcome {
        ResolvedOutcome::Pill {
            recipe_id,
            pill,
            quality,
            toxin_amount,
            toxin_color,
            qi_gain,
            side_effect,
            flawed_path,
            ..
        } => {
            data.recipe_id = Some(recipe_id.clone());
            data.pill = Some(pill.clone());
            data.quality = Some(*quality);
            data.toxin_amount = Some(*toxin_amount);
            data.toxin_color = Some(*toxin_color);
            data.qi_gain = *qi_gain;
            data.side_effect_tag = side_effect.as_ref().map(|effect| effect.tag.clone());
            data.flawed_path = *flawed_path;
        }
        ResolvedOutcome::Waste { recipe_id } => {
            data.recipe_id = recipe_id.clone().or_else(|| event.recipe_id.clone());
        }
        ResolvedOutcome::Explode {
            damage,
            meridian_crack,
        } => {
            data.damage = Some(*damage);
            data.meridian_crack = Some(*meridian_crack);
        }
        ResolvedOutcome::Mismatch => {}
    }
    data
}

fn mock_outcome_resolved() -> AlchemyOutcomeResolvedDataV1 {
    AlchemyOutcomeResolvedDataV1 {
        bucket: AlchemyOutcomeBucketV1::Good,
        recipe_id: Some("hui_yuan_pill_v0".into()),
        pill: Some("hui_yuan_pill".into()),
        quality: Some(0.7),
        toxin_amount: Some(0.3),
        toxin_color: Some(ColorKind::Mellow),
        qi_gain: Some(18.0),
        side_effect_tag: None,
        flawed_path: false,
        damage: None,
        meridian_crack: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        alchemy_join_mocks_enabled_value, build_outcome_resolved_data, build_session_data,
        username_matches_caster,
    };
    use crate::alchemy::outcome::OutcomeBucket;
    use crate::alchemy::recipe::{
        FireProfile, IngredientSpec, Outcomes, Recipe, RecipeStage, SideEffect, ToleranceSpec,
    };
    use crate::alchemy::{
        AlchemyFurnace, AlchemyOutcomeEvent, AlchemySession, Intervention, RecipeRegistry,
        ResolvedOutcome,
    };
    use crate::cultivation::components::ColorKind;
    use crate::network::agent_bridge::serialize_server_data_payload_proto;
    use crate::schema::alchemy::{AlchemySessionDataV1, AlchemyStageHintV1};
    use crate::schema::proto_gen::bong::{self, server_data_envelope};
    use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
    use prost::Message;
    use std::fs;
    use std::path::PathBuf;

    const RECIPE_ID: &str = "hud_contract_recipe";
    const ACTIVE_PROTO_FIXTURE: &str = "alchemy_session_active_v1.pb";
    const FINISHED_PROTO_FIXTURE: &str = "alchemy_session_finished_v1.pb";
    const REGENERATE_FIXTURES_COMMAND: &str = "scripts/build-token.sh cargo test network::alchemy_snapshot_emit::tests::regenerate_alchemy_session_production_proto_fixtures -- --ignored --exact --nocapture";

    fn test_registry() -> RecipeRegistry {
        let mut registry = RecipeRegistry::new();
        registry
            .insert(Recipe {
                id: RECIPE_ID.into(),
                name: "HUD 契约丹方".into(),
                furnace_tier_min: 1,
                stages: vec![
                    RecipeStage {
                        at_tick: 0,
                        required: vec![
                            IngredientSpec {
                                material: "ci_she_hao".into(),
                                count: 2,
                                mineral_id: None,
                            },
                            IngredientSpec {
                                material: "ling_shui".into(),
                                count: 1,
                                mineral_id: None,
                            },
                        ],
                        window: 0,
                    },
                    RecipeStage {
                        at_tick: 40,
                        required: vec![IngredientSpec {
                            material: "dan_sha".into(),
                            count: 3,
                            mineral_id: None,
                        }],
                        window: 6,
                    },
                    RecipeStage {
                        at_tick: 120,
                        required: vec![],
                        window: 4,
                    },
                ],
                fire_profile: FireProfile {
                    target_temp: 0.62,
                    target_duration_ticks: 180,
                    qi_cost: 12.5,
                    tolerance: ToleranceSpec {
                        temp_band: 0.08,
                        duration_band: 20,
                    },
                },
                outcomes: Outcomes {
                    perfect: None,
                    good: None,
                    flawed: None,
                    waste: None,
                    explode: None,
                },
                flawed_fallback: None,
            })
            .unwrap();
        registry
    }

    fn active_furnace() -> AlchemyFurnace {
        let mut session = AlchemySession::new(RECIPE_ID.into(), "alice".into());
        session.elapsed_ticks = 44;
        session.temp_current = 0.58;
        session.qi_injected = 7.25;
        session.staged.completed_stages = vec![0];
        session.staged.missed_stages = vec![1];
        session.interventions.push(Intervention::AdjustTemp(0.58));
        AlchemyFurnace {
            session: Some(session),
            ..AlchemyFurnace::default()
        }
    }

    fn expected_active_data(active: bool, status_label: &str) -> AlchemySessionDataV1 {
        AlchemySessionDataV1 {
            recipe_id: Some(RECIPE_ID.into()),
            active,
            elapsed_ticks: 44,
            target_ticks: 180,
            temp_current: 0.58,
            temp_target: 0.62,
            temp_band: 0.08,
            qi_injected: 7.25,
            qi_target: 12.5,
            status_label: status_label.into(),
            stages: vec![
                AlchemyStageHintV1 {
                    at_tick: 0,
                    window: 0,
                    summary: "ci_she_hao×2 + ling_shui×1".into(),
                    completed: true,
                    missed: false,
                },
                AlchemyStageHintV1 {
                    at_tick: 40,
                    window: 6,
                    summary: "dan_sha×3".into(),
                    completed: false,
                    missed: true,
                },
                AlchemyStageHintV1 {
                    at_tick: 120,
                    window: 4,
                    summary: String::new(),
                    completed: false,
                    missed: false,
                },
            ],
            interventions_recent: vec!["§7AdjustTemp(0.58)".into()],
        }
    }

    fn production_fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../proto/fixtures")
    }

    fn production_fixture_bytes(finished: bool) -> Vec<u8> {
        let mut furnace = active_furnace();
        furnace
            .session
            .as_mut()
            .expect("fixture furnace must contain an alchemy session")
            .finished = finished;
        let data = build_session_data(furnace.session.as_ref(), &test_registry());
        let payload = ServerDataV1::new(ServerDataPayloadV1::AlchemySession(Box::new(data)));

        serialize_server_data_payload_proto(&payload)
            .expect("alchemy fixture must serialize through the production envelope path")
    }

    fn assert_shared_fixture_is_current(file_name: &str, finished: bool) -> Vec<u8> {
        let generated = production_fixture_bytes(finished);
        let fixture_path = production_fixture_dir().join(file_name);
        let checked_in = fs::read(&fixture_path).unwrap_or_else(|err| {
            panic!(
                "shared Rust production fixture `{}` is missing: {err}; regenerate explicitly with `{REGENERATE_FIXTURES_COMMAND}`",
                fixture_path.display()
            )
        });

        assert_eq!(
            checked_in.as_slice(),
            generated.as_slice(),
            "shared Rust production fixture `{}` is stale; regenerate explicitly with `{REGENERATE_FIXTURES_COMMAND}`",
            fixture_path.display()
        );
        generated
    }

    fn assert_production_fixture_semantics(bytes: &[u8], active: bool, status_label: &str) {
        let decoded = bong::ServerDataEnvelope::decode(bytes)
            .expect("Rust production alchemy fixture must decode as ServerDataEnvelope");
        let session = match decoded.payload {
            Some(server_data_envelope::Payload::AlchemySession(session)) => session,
            other => panic!("expected alchemy_session proto payload, got {other:?}"),
        };

        assert_eq!(session.recipe_id.as_deref(), Some(RECIPE_ID));
        assert_eq!(session.active, active);
        assert_eq!(session.elapsed_ticks, 44);
        assert_eq!(session.target_ticks, 180);
        assert_eq!(session.temp_current, 0.58);
        assert_eq!(session.temp_target, 0.62);
        assert_eq!(session.temp_band, 0.08);
        assert_eq!(session.qi_injected, 7.25);
        assert_eq!(session.qi_target, 12.5);
        assert_eq!(session.status_label, status_label);
        assert_eq!(session.stages.len(), 3);
        assert_eq!(session.stages[0].at_tick, 0);
        assert_eq!(session.stages[0].window, 0);
        assert_eq!(session.stages[0].summary, "ci_she_hao×2 + ling_shui×1");
        assert!(session.stages[0].completed);
        assert!(!session.stages[0].missed);
        assert_eq!(session.stages[1].at_tick, 40);
        assert_eq!(session.stages[1].window, 6);
        assert_eq!(session.stages[1].summary, "dan_sha×3");
        assert!(!session.stages[1].completed);
        assert!(session.stages[1].missed);
        assert_eq!(session.stages[2].at_tick, 120);
        assert_eq!(session.stages[2].window, 4);
        assert_eq!(session.stages[2].summary, "");
        assert!(!session.stages[2].completed);
        assert!(!session.stages[2].missed);
        assert_eq!(session.interventions_recent, vec!["§7AdjustTemp(0.58)"]);
    }

    fn outcome_event(bucket: OutcomeBucket, outcome: ResolvedOutcome) -> AlchemyOutcomeEvent {
        AlchemyOutcomeEvent {
            furnace: valence::prelude::Entity::from_raw(7),
            caster_id: "offline:alice".into(),
            recipe_id: Some("event_recipe".into()),
            bucket,
            outcome,
            elapsed_ticks: 80,
        }
    }

    #[test]
    fn outcome_recipient_matching_accepts_only_exact_username_or_canonical_player_id() {
        assert!(username_matches_caster("alice", "alice"));
        assert!(username_matches_caster("alice", "offline:alice"));
        assert!(!username_matches_caster("bystander", "offline:alice"));
        assert!(!username_matches_caster("alice_2", "alice"));
    }

    #[test]
    fn pill_outcome_preserves_every_optional_field_and_resolved_recipe_identity() {
        let event = outcome_event(
            OutcomeBucket::Flawed,
            ResolvedOutcome::Pill {
                recipe_id: "resolved_recipe".into(),
                pill: "flawed_pill".into(),
                quality: 0.4,
                toxin_amount: 0.8,
                toxin_color: ColorKind::Turbid,
                qi_gain: Some(18.0),
                quality_tier: 1,
                effect_multiplier: 0.5,
                consecrated: false,
                side_effect: Some(SideEffect {
                    tag: "qi_cap_perm_minus_1".into(),
                    duration_s: 0,
                    weight: 1,
                    perm: true,
                    color: None,
                    amount: None,
                }),
                flawed_path: true,
            },
        );

        let data = build_outcome_resolved_data(&event);
        assert_eq!(
            data.bucket,
            crate::schema::alchemy::AlchemyOutcomeBucketV1::Flawed
        );
        assert_eq!(data.recipe_id.as_deref(), Some("resolved_recipe"));
        assert_eq!(data.pill.as_deref(), Some("flawed_pill"));
        assert_eq!(data.quality, Some(0.4));
        assert_eq!(data.toxin_amount, Some(0.8));
        assert_eq!(data.toxin_color, Some(ColorKind::Turbid));
        assert_eq!(data.qi_gain, Some(18.0));
        assert_eq!(data.side_effect_tag.as_deref(), Some("qi_cap_perm_minus_1"));
        assert!(data.flawed_path);
        assert_eq!(data.damage, None);
        assert_eq!(data.meridian_crack, None);
    }

    #[test]
    fn waste_outcome_keeps_recipe_and_leaves_non_applicable_optionals_absent() {
        let data = build_outcome_resolved_data(&outcome_event(
            OutcomeBucket::Waste,
            ResolvedOutcome::Waste { recipe_id: None },
        ));

        assert_eq!(
            data.bucket,
            crate::schema::alchemy::AlchemyOutcomeBucketV1::Waste
        );
        assert_eq!(data.recipe_id.as_deref(), Some("event_recipe"));
        assert_eq!(data.pill, None);
        assert_eq!(data.quality, None);
        assert_eq!(data.toxin_amount, None);
        assert_eq!(data.toxin_color, None);
        assert_eq!(data.qi_gain, None);
        assert_eq!(data.side_effect_tag, None);
        assert!(!data.flawed_path);
        assert_eq!(data.damage, None);
        assert_eq!(data.meridian_crack, None);
    }

    #[test]
    fn explode_outcome_sets_only_damage_and_meridian_crack() {
        let data = build_outcome_resolved_data(&outcome_event(
            OutcomeBucket::Explode,
            ResolvedOutcome::Explode {
                damage: 12.0,
                meridian_crack: 0.2,
            },
        ));

        assert_eq!(
            data.bucket,
            crate::schema::alchemy::AlchemyOutcomeBucketV1::Explode
        );
        assert_eq!(data.recipe_id.as_deref(), Some("event_recipe"));
        assert_eq!(data.pill, None);
        assert_eq!(data.damage, Some(12.0));
        assert_eq!(data.meridian_crack, Some(0.2));
    }

    #[test]
    fn mismatch_outcome_preserves_event_bucket_without_counterfeit_optional_values() {
        let data = build_outcome_resolved_data(&outcome_event(
            OutcomeBucket::Waste,
            ResolvedOutcome::Mismatch,
        ));

        assert_eq!(
            data.bucket,
            crate::schema::alchemy::AlchemyOutcomeBucketV1::Waste
        );
        assert_eq!(data.recipe_id.as_deref(), Some("event_recipe"));
        assert_eq!(data.pill, None);
        assert_eq!(data.quality, None);
        assert_eq!(data.toxin_amount, None);
        assert_eq!(data.toxin_color, None);
        assert_eq!(data.qi_gain, None);
        assert_eq!(data.side_effect_tag, None);
        assert!(!data.flawed_path);
        assert_eq!(data.damage, None);
        assert_eq!(data.meridian_crack, None);
    }

    #[test]
    fn join_alchemy_mocks_require_explicit_truthy_env() {
        assert!(alchemy_join_mocks_enabled_value("1"));
        assert!(alchemy_join_mocks_enabled_value("true"));
        assert!(alchemy_join_mocks_enabled_value("YES"));
        assert!(!alchemy_join_mocks_enabled_value(""));
        assert!(!alchemy_join_mocks_enabled_value("0"));
        assert!(!alchemy_join_mocks_enabled_value("false"));
    }

    #[test]
    fn active_session_snapshot_uses_complete_recipe_contract_in_declared_stage_order() {
        let furnace = active_furnace();
        let data = build_session_data(furnace.session.as_ref(), &test_registry());

        assert_eq!(
            data,
            expected_active_data(true, "炼制中"),
            "active snapshot must preserve every runtime field, authoritative recipe target, ordered stage hint, empty-required summary, and intervention"
        );
    }

    #[test]
    fn active_session_production_proto_fixture_matches_real_builder_and_encoding() {
        let bytes = assert_shared_fixture_is_current(ACTIVE_PROTO_FIXTURE, false);

        assert_production_fixture_semantics(bytes.as_slice(), true, "炼制中");
    }

    #[test]
    fn finished_session_production_proto_fixture_matches_real_builder_and_encoding() {
        let bytes = assert_shared_fixture_is_current(FINISHED_PROTO_FIXTURE, true);

        assert_production_fixture_semantics(bytes.as_slice(), false, "已结束");
    }

    #[test]
    #[ignore = "maintenance-only fixture writer; ordinary tests must never modify checked-in bytes"]
    fn regenerate_alchemy_session_production_proto_fixtures() {
        let fixture_dir = production_fixture_dir();
        fs::create_dir_all(&fixture_dir).unwrap_or_else(|err| {
            panic!(
                "failed to create shared proto fixture directory `{}`: {err}",
                fixture_dir.display()
            )
        });

        for (file_name, finished) in [
            (ACTIVE_PROTO_FIXTURE, false),
            (FINISHED_PROTO_FIXTURE, true),
        ] {
            let fixture_path = fixture_dir.join(file_name);
            fs::write(&fixture_path, production_fixture_bytes(finished)).unwrap_or_else(|err| {
                panic!(
                    "failed to write shared Rust production fixture `{}`: {err}",
                    fixture_path.display()
                )
            });
            println!("wrote {}", fixture_path.display());
        }
    }

    #[test]
    fn empty_furnace_snapshot_clears_every_active_hud_field() {
        let data = build_session_data(None, &test_registry());

        assert_eq!(
            data,
            AlchemySessionDataV1 {
                recipe_id: None,
                active: false,
                elapsed_ticks: 0,
                target_ticks: 0,
                temp_current: 0.0,
                temp_target: 0.0,
                temp_band: 0.0,
                qi_injected: 0.0,
                qi_target: 0.0,
                status_label: "未起炉".into(),
                stages: vec![],
                interventions_recent: vec![],
            },
            "empty furnace must clear the complete alchemy HUD contract rather than leave stale guidance"
        );
    }

    #[test]
    fn unknown_recipe_snapshot_fails_closed_instead_of_emitting_active_zero_targets() {
        let mut furnace = active_furnace();
        furnace.session.as_mut().unwrap().recipe = "missing_recipe".into();

        let data = build_session_data(furnace.session.as_ref(), &RecipeRegistry::new());

        assert!(!data.active);
        assert_eq!(data.recipe_id.as_deref(), Some("missing_recipe"));
        assert_eq!(data.status_label, "丹方数据缺失");
        assert_eq!(data.target_ticks, 0);
        assert_eq!(data.temp_target, 0.0);
        assert_eq!(data.temp_band, 0.0);
        assert_eq!(data.qi_target, 0.0);
        assert!(data.stages.is_empty());
    }

    #[test]
    fn finished_known_session_retains_complete_ordered_recipe_guidance() {
        let mut furnace = active_furnace();
        furnace.session.as_mut().unwrap().finished = true;

        let data = build_session_data(furnace.session.as_ref(), &test_registry());

        assert_eq!(
            data,
            expected_active_data(false, "已结束"),
            "finished snapshot must become inactive without losing any authoritative target, ordered stage state, empty-required summary, runtime field, or intervention"
        );
    }
}
