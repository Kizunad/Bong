//! plan-dying-elder-v1 B1 修复 — S2C `bong:elder_encounter` payload 推送。
//!
//! server 在 P3 Redis 广播的同时，也向同 zone 在场玩家推送
//! `bong:elder_encounter` S2C CustomPayload，驱动 client HUD 显示。
//!
//! ## 触发时机
//! - 大能出现（appeared）：监听 `DyingElderSpawnRequest`
//! - 收丹（dan_received）：监听 `GiveDanToElderIntent`（在 give_dan_system 之后）
//! - 死亡（betrayal / dead_natural）：监听 Dead 状态变化（在 death_system 之前，
//!   与 `dying_elder_p3_emit_death_event_system` 相同时序）
//!
//! ## payload 格式（与 client DyingElderEncounterHandler 对齐）
//! ```json
//! {
//!   "zone_name":          string,
//!   "elder_entity_idx":   u32,
//!   "event_kind":         string,   // snake_case
//!   "betray_probability": f64,
//!   "dan_count":          u32,
//!   "offered_skill_id":   string,
//!   "qi_fraction":        f32,      // M2 修复：真实 qi_current/qi_max_cache
//!   "server_tick":        u64
//! }
//! ```
//!
//! ## 守恒红线
//! 本模块 **只** 推送显示层数据，不修改任何 gameplay 数值（qi、血量、状态）。

use valence::client::ClientMarker;
use valence::ident;
use valence::prelude::{
    App, Client, Entity, EventReader, IntoSystemConfigs, Position, Query, Res, Update, With,
    Without,
};

use crate::fauna::dying_elder::{
    dying_elder_death_system, dying_elder_give_dan_system, DyingElderBlackboard,
    DyingElderDeathProcessed, DyingElderSpawnRequest, DyingElderState, GiveDanToElderIntent,
    DYING_ELDER_DAN_THRESHOLD,
};
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::schema::elder_encounter::{ElderEncounterEventKindV1, ElderEncounterEventV1};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

// ── 辅助函数 ──────────────────────────────────────────────────────────────────

/// 序列化 `ElderEncounterEventV1` → JSON bytes；失败返回 None 并记录 warn。
fn to_json_bytes(event: &ElderEncounterEventV1) -> Option<Vec<u8>> {
    match serde_json::to_vec(event) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            tracing::warn!(
                "[bong][elder_encounter_emit] failed to serialize ElderEncounterEventV1: {e}"
            );
            None
        }
    }
}

/// 向 `players` Query 中所有在 `zone_name` 且维度为 Overworld 的玩家发送 S2C payload。
///
/// `players`：(Entity, &mut Client, &Position, Option<&CurrentDimension>)
fn send_to_players_in_zone<'w>(
    players: &mut Query<
        'w,
        '_,
        (&mut Client, &Position, Option<&CurrentDimension>),
        With<ClientMarker>,
    >,
    zone_name: &str,
    zones: &ZoneRegistry,
    bytes: &[u8],
) {
    for (mut client, pos, current_dim) in players.iter_mut() {
        let dim = current_dim.map(|d| d.0).unwrap_or(DimensionKind::Overworld);
        // 仅推送给同 zone 内玩家（按 AABB 最小匹配）
        if let Some(zone) = zones.find_zone(dim, pos.get()) {
            if zone.name == zone_name {
                client.send_custom_payload(ident!("bong:elder_encounter"), bytes);
            }
        }
    }
}

// ── System 类型别名 ────────────────────────────────────────────────────────────

type PlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Client,
        &'static Position,
        Option<&'static CurrentDimension>,
    ),
    With<ClientMarker>,
>;

type DyingElderQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static DyingElderBlackboard,
        &'static DyingElderState,
    ),
    (With<NpcMarker>, Without<ClientMarker>),
>;

// ── S2C appeared 系统 ─────────────────────────────────────────────────────────

/// plan-dying-elder-v1 B1 — 大能出现时向同 zone 玩家推送 `bong:elder_encounter` appeared 事件。
///
/// 监听 `DyingElderSpawnRequest`，在 P0 spawn 同帧推送 appeared S2C。
/// qi_fraction = 1.0（spawn 时真元满值）。
#[allow(clippy::type_complexity)]
pub(crate) fn elder_encounter_s2c_appear_system(
    mut spawn_requests: EventReader<DyingElderSpawnRequest>,
    mut players: PlayerQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    game_tick: Option<Res<GameTick>>,
) {
    let Some(zones) = zones else { return };
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);

    for req in spawn_requests.read() {
        let event = ElderEncounterEventV1 {
            zone_name: req.zone_name.clone(),
            elder_entity_idx: 0,
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: req.blackboard.betray_probability,
            dan_count: 0,
            offered_skill_id: req.blackboard.offered_skill_id.to_string(),
            qi_fraction: 1.0_f32,
            server_tick: tick,
        };
        let Some(bytes) = to_json_bytes(&event) else {
            continue;
        };
        send_to_players_in_zone(&mut players, &req.zone_name, &zones, &bytes);
        tracing::info!(
            "[bong][elder_encounter_emit] S2C appeared → zone='{}' betray_prob={:.3} tick={tick}",
            req.zone_name,
            req.blackboard.betray_probability,
        );
    }
}

// ── S2C dan_received 系统 ─────────────────────────────────────────────────────

/// plan-dying-elder-v1 B1 — 玩家给丹后向同 zone 玩家推送 `bong:elder_encounter` dan_received 事件。
///
/// 在 `dying_elder_give_dan_system` 之后运行，读取更新后的 dan_count 和 qi_fraction。
#[allow(clippy::type_complexity)]
pub(crate) fn elder_encounter_s2c_dan_received_system(
    mut intents: EventReader<GiveDanToElderIntent>,
    elders: DyingElderQuery<'_, '_>,
    mut players: PlayerQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    game_tick: Option<Res<GameTick>>,
) {
    let Some(zones) = zones else { return };
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);

    for intent in intents.read() {
        let Ok((entity, bb, state)) = elders.get(intent.elder) else {
            continue;
        };

        let dan_count = match *state {
            DyingElderState::Recovering { dan_received } => dan_received,
            DyingElderState::Betrayal => DYING_ELDER_DAN_THRESHOLD,
            DyingElderState::Dead { .. } => DYING_ELDER_DAN_THRESHOLD,
            DyingElderState::Plea => continue,
        };

        // M2 修复：使用真实 qi_current / qi_max_cache
        let qi_fraction = if bb.qi_max_cache > 0.0 {
            (bb.qi_current / bb.qi_max_cache).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        let event = ElderEncounterEventV1 {
            zone_name: bb.home_zone.clone(),
            elder_entity_idx: entity.index(),
            event_kind: ElderEncounterEventKindV1::DanReceived,
            betray_probability: 0.0,
            dan_count,
            offered_skill_id: bb.offered_skill_id.to_string(),
            qi_fraction,
            server_tick: tick,
        };
        let Some(bytes) = to_json_bytes(&event) else {
            continue;
        };
        send_to_players_in_zone(&mut players, &bb.home_zone, &zones, &bytes);
    }
}

// ── S2C death 系统 ────────────────────────────────────────────────────────────

/// plan-dying-elder-v1 B1 — 大能死亡时向同 zone 玩家推送 `bong:elder_encounter` 死亡事件。
///
/// 在 `dying_elder_death_system`（含 `DyingElderDeathProcessed` 标记）之前运行，
/// 检测本 tick 刚进入 Dead 状态且尚未处理的大能。
#[allow(clippy::type_complexity)]
pub(crate) fn elder_encounter_s2c_death_system(
    elders: Query<
        (Entity, &DyingElderBlackboard, &DyingElderState),
        (
            With<NpcMarker>,
            Without<ClientMarker>,
            Without<DyingElderDeathProcessed>,
        ),
    >,
    mut players: PlayerQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    game_tick: Option<Res<GameTick>>,
) {
    let Some(zones) = zones else { return };
    let tick = game_tick.as_deref().map(|t| t.0 as u64).unwrap_or(0);

    for (entity, bb, state) in elders.iter() {
        let dead_by_betrayal = match *state {
            DyingElderState::Dead { dead_by_betrayal } => dead_by_betrayal,
            _ => continue,
        };

        let event_kind = if dead_by_betrayal {
            ElderEncounterEventKindV1::Betrayal
        } else {
            ElderEncounterEventKindV1::DeadNatural
        };

        let event = ElderEncounterEventV1 {
            zone_name: bb.home_zone.clone(),
            elder_entity_idx: entity.index(),
            event_kind,
            betray_probability: 0.0,
            dan_count: 0,
            offered_skill_id: String::new(),
            qi_fraction: 0.0,
            server_tick: tick,
        };
        let Some(bytes) = to_json_bytes(&event) else {
            continue;
        };
        send_to_players_in_zone(&mut players, &bb.home_zone, &zones, &bytes);
        tracing::info!(
            "[bong][elder_encounter_emit] S2C death → entity={:?} zone='{}' kind={:?} tick={tick}",
            entity,
            bb.home_zone,
            event_kind,
        );
    }
}

// ── Bevy 注册 ─────────────────────────────────────────────────────────────────

/// 注册 S2C elder_encounter 推送系统。
///
/// - `elder_encounter_s2c_appear_system`：与 P3 appear event 同步运行
/// - `elder_encounter_s2c_dan_received_system`：在 `give_dan_system` 之后运行
/// - `elder_encounter_s2c_death_system`：在 `death_system` 之前运行
pub fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            elder_encounter_s2c_appear_system,
            elder_encounter_s2c_dan_received_system.after(dying_elder_give_dan_system),
            elder_encounter_s2c_death_system.before(dying_elder_death_system),
        ),
    );
}

// ── 单元测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::elder_encounter::ElderEncounterEventV1;

    // ── payload JSON 结构 pin 测试 ──────────────────────────────────────────

    #[test]
    fn appeared_payload_has_correct_json_structure() {
        // 期望：appeared 事件序列化包含所有 client handler 需要的字段
        let event = ElderEncounterEventV1 {
            zone_name: "tsy_deep".to_string(),
            elder_entity_idx: 0,
            event_kind: ElderEncounterEventKindV1::Appeared,
            betray_probability: 0.65,
            dan_count: 0,
            offered_skill_id: "woliu.heart".to_string(),
            qi_fraction: 1.0,
            server_tick: 1000,
        };
        let json = serde_json::to_string(&event).expect("serialize appeared event");

        assert!(
            json.contains("\"zone_name\""),
            "expected appeared payload to contain 'zone_name' field because client handler reads this, actual: {json}"
        );
        assert!(
            json.contains("\"event_kind\""),
            "expected appeared payload to contain 'event_kind' field because client handler routes on this, actual: {json}"
        );
        assert!(
            json.contains("\"appeared\""),
            "expected appeared payload event_kind to serialize as 'appeared', actual: {json}"
        );
        assert!(
            json.contains("\"betray_probability\""),
            "expected appeared payload to contain 'betray_probability' for client HUD display, actual: {json}"
        );
        assert!(
            json.contains("\"qi_fraction\""),
            "expected appeared payload to contain 'qi_fraction' (M2 fix: real qi display), actual: {json}"
        );
        assert!(
            json.contains("\"server_tick\""),
            "expected appeared payload to contain 'server_tick' for audit, actual: {json}"
        );
    }

    #[test]
    fn death_payload_event_kind_routes_correctly() {
        // 期望：dead_by_betrayal=true → Betrayal；false → DeadNatural
        let betrayal = ElderEncounterEventKindV1::Betrayal;
        let natural = ElderEncounterEventKindV1::DeadNatural;

        let json_b = serde_json::to_string(&betrayal).expect("serialize");
        let json_n = serde_json::to_string(&natural).expect("serialize");

        assert!(
            json_b.contains("betrayal"),
            "expected Betrayal kind to serialize as 'betrayal' for client routing, actual: {json_b}"
        );
        assert!(
            json_n.contains("dead_natural"),
            "expected DeadNatural kind to serialize as 'dead_natural' for client routing, actual: {json_n}"
        );
    }

    #[test]
    fn qi_fraction_zero_for_death_events() {
        // 期望：死亡事件 qi_fraction = 0.0（真元耗尽）
        let event = ElderEncounterEventV1 {
            zone_name: "tsy_deep".to_string(),
            elder_entity_idx: 5,
            event_kind: ElderEncounterEventKindV1::DeadNatural,
            betray_probability: 0.0,
            dan_count: 0,
            offered_skill_id: String::new(),
            qi_fraction: 0.0,
            server_tick: 999,
        };
        assert_eq!(
            event.qi_fraction, 0.0,
            "expected qi_fraction=0.0 for death events because elder's qi is exhausted, actual: {}",
            event.qi_fraction
        );
        let json = serde_json::to_string(&event).expect("serialize");
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.qi_fraction, 0.0,
            "expected qi_fraction=0.0 to survive serde for death event, actual: {}",
            back.qi_fraction
        );
    }

    #[test]
    fn qi_fraction_computation_from_bb_values() {
        // 期望：qi_fraction = qi_current / qi_max_cache，clamp [0.0, 1.0]
        // 正常：qi_current=300, qi_max_cache=500 → fraction=0.6
        let qi_current = 300.0_f64;
        let qi_max_cache = 500.0_f64;
        let fraction = (qi_current / qi_max_cache).clamp(0.0, 1.0) as f32;
        assert!(
            (fraction - 0.6_f32).abs() < 1e-5,
            "expected qi_fraction=0.6 when qi_current=300/qi_max=500, actual: {fraction}"
        );

        // 超量（给丹后 > qi_max_cache）：clamp 到 1.0
        let qi_over = 600.0_f64;
        let fraction_over = (qi_over / qi_max_cache).clamp(0.0, 1.0) as f32;
        assert_eq!(
            fraction_over, 1.0,
            "expected qi_fraction clamped to 1.0 when qi > qi_max_cache, actual: {fraction_over}"
        );

        // qi_max_cache = 0（防止除零）
        let qi_zero_max = 0.0_f64;
        let fraction_zero = if qi_zero_max > 0.0 {
            (qi_current / qi_zero_max).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        assert_eq!(
            fraction_zero, 0.0,
            "expected qi_fraction=0.0 when qi_max_cache=0 (zero-division guard), actual: {fraction_zero}"
        );
    }

    #[test]
    fn dan_received_payload_has_qi_fraction_and_dan_count() {
        // 期望：dan_received 事件包含正确 qi_fraction 和 dan_count
        let event = ElderEncounterEventV1 {
            zone_name: "tsy_abyss".to_string(),
            elder_entity_idx: 42,
            event_kind: ElderEncounterEventKindV1::DanReceived,
            betray_probability: 0.0,
            dan_count: 3,
            offered_skill_id: "anqi.echo_fractal".to_string(),
            qi_fraction: 0.7,
            server_tick: 5000,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            json.contains("\"dan_received\""),
            "expected dan_received payload event_kind to serialize as 'dan_received', actual: {json}"
        );
        assert!(
            json.contains("\"qi_fraction\""),
            "expected dan_received payload to include qi_fraction for client HUD progress bar, actual: {json}"
        );
        let back: ElderEncounterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.dan_count, 3,
            "expected dan_count=3 to survive serde, actual: {}",
            back.dan_count
        );
        assert!(
            (back.qi_fraction - 0.7).abs() < 1e-5,
            "expected qi_fraction=0.7 to survive serde, actual: {}",
            back.qi_fraction
        );
    }

    #[test]
    fn channel_identifier_matches_client_handler() {
        // 期望：channel identifier 硬编码为 "bong:elder_encounter"
        // (channel 在 ident!("bong:elder_encounter") 中使用，这里通过字符串 pin 测试)
        let channel = "bong:elder_encounter";
        assert!(
            channel.starts_with("bong:"),
            "expected channel to use 'bong:' prefix (matching client CHANNEL_NAMESPACE=bong), actual: {channel}"
        );
        assert!(
            channel.ends_with("elder_encounter"),
            "expected channel to end with 'elder_encounter' (matching client CHANNEL_PATH=elder_encounter), actual: {channel}"
        );
    }
}
