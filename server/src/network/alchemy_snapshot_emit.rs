//! 玩家加入时一次性推送 alchemy 6 个 mock 快照（plan-alchemy-v1 §4 vertical slice）。
//!
//! 当前为 **mock-only**：未挂 ECS 真实炉/会话状态，目的是验证双端通路（client 应能看到
//! server 推过来的 mock 数据替换掉本地 store 默认值）。
//!
//! 后续切片：
//!   * 改成响应 `AlchemyOpenFurnace` 请求（按需推）
//!   * 接 ECS — 从真实 `AlchemyFurnace` / `AlchemySession` Component 取数据
//!   * 配合 alchemy_emit_state 增量推送（只推变化字段）

use valence::prelude::{Added, Client, Entity, Query, Username, With};

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

pub fn emit_join_alchemy_snapshots(
    mut joined_clients: Query<JoinedClientQueryItem<'_>, (With<Client>, Added<PlayerInventory>)>,
) {
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
) {
    let data = match &furnace.session {
        Some(s) => AlchemySessionDataV1 {
            recipe_id: Some(s.recipe.clone()),
            active: !s.finished,
            elapsed_ticks: s.elapsed_ticks,
            target_ticks: 0,
            temp_current: s.temp_current,
            temp_target: 0.0,
            temp_band: 0.0,
            qi_injected: s.qi_injected,
            qi_target: 0.0,
            status_label: "ECS-driven".into(),
            stages: vec![],
            interventions_recent: s
                .interventions
                .iter()
                .rev()
                .take(3)
                .rev()
                .map(|i| format!("§7{i:?}"))
                .collect(),
        },
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
    };
    let payload = ServerDataV1::new(ServerDataPayloadV1::AlchemySession(Box::new(data)));
    send_payload(client, &payload, player_id);
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
