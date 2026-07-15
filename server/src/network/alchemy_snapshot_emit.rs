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

use valence::prelude::{Added, Client, Entity, Query, Username, With};

use crate::alchemy::RecipeRegistry;
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
    let data = build_session_data(furnace, registry);
    let payload = ServerDataV1::new(ServerDataPayloadV1::AlchemySession(Box::new(data)));
    send_payload(client, &payload, player_id);
}

fn build_session_data(
    furnace: &crate::alchemy::AlchemyFurnace,
    registry: &RecipeRegistry,
) -> AlchemySessionDataV1 {
    match &furnace.session {
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
    use super::{alchemy_join_mocks_enabled_value, build_session_data};
    use crate::alchemy::recipe::{
        FireProfile, IngredientSpec, Outcomes, Recipe, RecipeStage, ToleranceSpec,
    };
    use crate::alchemy::{AlchemyFurnace, AlchemySession, Intervention, RecipeRegistry};
    use crate::schema::alchemy::AlchemyStageHintV1;

    const RECIPE_ID: &str = "hud_contract_recipe";

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
    fn active_session_snapshot_uses_recipe_targets_and_stage_state() {
        let data = build_session_data(&active_furnace(), &test_registry());

        assert!(data.active);
        assert_eq!(data.recipe_id.as_deref(), Some(RECIPE_ID));
        assert_eq!(data.elapsed_ticks, 44);
        assert_eq!(data.target_ticks, 180);
        assert_eq!(data.temp_current, 0.58);
        assert_eq!(data.temp_target, 0.62);
        assert_eq!(data.temp_band, 0.08);
        assert_eq!(data.qi_injected, 7.25);
        assert_eq!(data.qi_target, 12.5);
        assert_eq!(
            data.stages,
            vec![
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
            ]
        );
        assert_eq!(data.interventions_recent.len(), 1);
    }

    #[test]
    fn empty_furnace_snapshot_clears_active_hud() {
        let data = build_session_data(&AlchemyFurnace::default(), &test_registry());

        assert!(!data.active);
        assert_eq!(data.recipe_id, None);
        assert_eq!(data.status_label, "未起炉");
        assert!(data.stages.is_empty());
    }

    #[test]
    fn unknown_recipe_snapshot_fails_closed_instead_of_emitting_active_zero_targets() {
        let mut furnace = active_furnace();
        furnace.session.as_mut().unwrap().recipe = "missing_recipe".into();

        let data = build_session_data(&furnace, &RecipeRegistry::new());

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
    fn finished_known_session_is_inactive_but_retains_recipe_guidance() {
        let mut furnace = active_furnace();
        furnace.session.as_mut().unwrap().finished = true;

        let data = build_session_data(&furnace, &test_registry());

        assert!(!data.active);
        assert_eq!(data.status_label, "已结束");
        assert_eq!(data.target_ticks, 180);
        assert_eq!(data.temp_target, 0.62);
        assert_eq!(data.temp_band, 0.08);
        assert_eq!(data.qi_target, 12.5);
        assert_eq!(data.stages.len(), 2);
    }
}
