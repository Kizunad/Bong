//! plan-HUD-v1 §2.3 / §11.4 server-side event_stream 推送。
//!
//! 把 server 已有的 `CombatEvent` / `DeathEvent` 翻译成 client `EventStreamPushV1`
//! 并 push 到相关 client（攻击方 + 受击方都收到，自打自除外）。
//!
//! 当前 v1 限制：仅战斗事件源；cultivation/world/system 路由后续接（每条只需要
//! 在对应 system 里 EventReader + 复用本文件的 push 函数）。

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Client, Entity, EventReader, Query, Username};

use crate::botany::components::HarvestTerminalEvent;
use crate::combat::events::DefenseKind;
use crate::combat::events::{CombatEvent, DeathEvent};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{EventChannelV1, EventPriorityV1, EventStreamPushV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

pub fn emit_combat_events_to_event_stream(
    mut combat_reader: EventReader<CombatEvent>,
    mut death_reader: EventReader<DeathEvent>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    let now_ms = current_unix_millis();

    for ev in combat_reader.read() {
        let body = format!("{:?}", ev.body_part);
        let kind = format!("{:?}", ev.wound_kind);
        let source_tag = if ev.defense_kind == Some(DefenseKind::JieMai) {
            "zhenmai-parry".to_string()
        } else {
            format!("hit-{body}-{kind}")
        };

        // 攻击方视角
        let attacker_text = if ev.defense_kind == Some(DefenseKind::JieMai) {
            format!("截脉震爆抵消，仍命中 {body} -{:.0}", ev.damage)
        } else {
            format!("命中 {body} {kind} -{:.0}", ev.damage)
        };
        push_to_client(
            &mut clients,
            ev.attacker,
            &source_tag,
            &attacker_text,
            now_ms,
        );

        // 受击方视角（自打自不重复推）
        if ev.attacker != ev.target {
            let target_text = if ev.defense_kind == Some(DefenseKind::JieMai) {
                let effectiveness = ev.defense_effectiveness.unwrap_or(0.0);
                format!("截脉震爆 {:.0}%：僵直半息", effectiveness * 100.0)
            } else {
                format!("受 {body} {kind} 伤 -{:.0}", ev.damage)
            };
            push_to_client(&mut clients, ev.target, &source_tag, &target_text, now_ms);
        }
    }

    for ev in death_reader.read() {
        // 死亡视角
        let target_text = format!("你已倒下 ({})", ev.cause);
        push_to_client_priority(
            &mut clients,
            ev.target,
            "death",
            &target_text,
            EventChannelV1::Combat,
            EventPriorityV1::P0Critical,
            now_ms,
        );
    }
}

/// plan-botany-harvest-full-inventory-loss-v1 §8.1 决议 #3：满包掉地面时给玩家一条
/// 结构化 event_stream 提示（World channel，非战斗但复用同一既有推送管线）。
/// 只在 `overflow_to_ground && completed && !interrupted` 时推送——正常收获（没有满包）
/// 不刷屏，遵循 `feedback_hud_immersive_minimal`：事件流不为 happy path 常驻。
pub fn emit_botany_harvest_overflow_to_event_stream(
    mut terminal: EventReader<HarvestTerminalEvent>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    let now_ms = current_unix_millis();
    for ev in terminal.read() {
        if !ev.overflow_to_ground || !ev.completed || ev.interrupted {
            continue;
        }
        push_to_client_priority(
            &mut clients,
            ev.client_entity,
            "botany-overflow",
            &ev.detail,
            EventChannelV1::World,
            EventPriorityV1::P2Normal,
            now_ms,
        );
    }
}

fn push_to_client(
    clients: &mut Query<(&Username, &mut Client)>,
    entity: Entity,
    source_tag: &str,
    text: &str,
    now_ms: u64,
) {
    push_to_client_priority(
        clients,
        entity,
        source_tag,
        text,
        EventChannelV1::Combat,
        EventPriorityV1::P1Important,
        now_ms,
    );
}

pub(crate) fn push_to_client_priority(
    clients: &mut Query<(&Username, &mut Client)>,
    entity: Entity,
    source_tag: &str,
    text: &str,
    channel: EventChannelV1,
    priority: EventPriorityV1,
    now_ms: u64,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return; // entity 不是 Client（NPC），跳过
    };

    let payload = ServerDataV1::new(ServerDataPayloadV1::EventStreamPush(EventStreamPushV1 {
        channel,
        priority,
        source_tag: source_tag.to_string(),
        text: text.to_string(),
        color: 0, // 0 = client 用 channel default
        created_at_ms: now_ms,
    }));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    send_server_data_payload(&mut client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload to entity {entity:?} for `{}` (text=\"{text}\")",
        SERVER_DATA_CHANNEL,
        payload_type,
        username.0
    );
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::botany::components::BotanyHarvestMode;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client(app: &mut App, name: &str) -> (Entity, MockClientHelper) {
        let (bundle, helper) = create_mock_client(name);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_event_stream_pushes(helper: &mut MockClientHelper) -> Vec<EventStreamPushV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            if let ServerDataPayloadV1::EventStreamPush(data) = payload.payload {
                payloads.push(data);
            }
        }
        payloads
    }

    /// plan-botany-harvest-full-inventory-loss-v1 §P1 测试用 `HarvestTerminalEvent` 构造器。
    fn overflow_terminal_event(
        client_entity: Entity,
        overflow_to_ground: bool,
        completed: bool,
        interrupted: bool,
    ) -> HarvestTerminalEvent {
        HarvestTerminalEvent {
            client_entity,
            session_id: "offline:Azure".to_string(),
            target_id: "plant-1".to_string(),
            target_name: "ci_she_hao".to_string(),
            plant_kind: "ci_she_hao".to_string(),
            mode: BotanyHarvestMode::Manual,
            interrupted,
            completed,
            detail: "采得 1 株 · 背包已满，已放置于地面 · 灵气流出 0.002".to_string(),
            target_pos: Some([10.0, 64.0, 10.0]),
            spirit_quality: 0.9,
            duration_ticks: 40,
            gathering_quality: None,
            tool_used: None,
            overflow_to_ground,
        }
    }

    /// ① overflow_to_ground=true → client 收到 1 条 World/P2Normal 推送，文案含"背包已满"。
    #[test]
    fn overflow_terminal_event_pushes_world_channel_event_stream() {
        let mut app = App::new();
        app.add_event::<HarvestTerminalEvent>();
        app.add_systems(Update, emit_botany_harvest_overflow_to_event_stream);

        let (player, mut player_helper) = spawn_mock_client(&mut app, "Azure");
        app.world_mut()
            .send_event(overflow_terminal_event(player, true, true, false));

        app.update();
        flush_all_client_packets(&mut app);

        let pushes = collect_event_stream_pushes(&mut player_helper);
        assert_eq!(
            pushes.len(),
            1,
            "overflow completion should push exactly one event_stream entry"
        );
        let push = &pushes[0];
        assert_eq!(push.channel, EventChannelV1::World);
        assert_eq!(push.priority, EventPriorityV1::P2Normal);
        assert!(
            push.text.contains("背包已满"),
            "pushed text should mention 背包已满, got {:?}",
            push.text
        );
    }

    /// ② overflow_to_ground=false → 无推送（happy path 不刷屏）。
    #[test]
    fn non_overflow_terminal_event_does_not_push() {
        let mut app = App::new();
        app.add_event::<HarvestTerminalEvent>();
        app.add_systems(Update, emit_botany_harvest_overflow_to_event_stream);

        let (player, mut player_helper) = spawn_mock_client(&mut app, "Azure");
        app.world_mut()
            .send_event(overflow_terminal_event(player, false, true, false));

        app.update();
        flush_all_client_packets(&mut app);

        let pushes = collect_event_stream_pushes(&mut player_helper);
        assert_eq!(
            pushes.len(),
            0,
            "non-overflow harvest completion must not push a botany-overflow event"
        );
    }

    /// ③ interrupted=true 时即使误设 overflow_to_ground=true 也不推送（防御性用例）。
    #[test]
    fn interrupted_terminal_event_never_pushes_even_if_overflow_flag_mistakenly_set() {
        let mut app = App::new();
        app.add_event::<HarvestTerminalEvent>();
        app.add_systems(Update, emit_botany_harvest_overflow_to_event_stream);

        let (player, mut player_helper) = spawn_mock_client(&mut app, "Azure");
        // production 只在 completed 分支设置 overflow_to_ground，interrupted=true 恒为
        // false；这里故意误设 true，验证消费端也有防御性判定，不单靠生产端自律。
        app.world_mut()
            .send_event(overflow_terminal_event(player, true, false, true));

        app.update();
        flush_all_client_packets(&mut app);

        let pushes = collect_event_stream_pushes(&mut player_helper);
        assert_eq!(
            pushes.len(),
            0,
            "interrupted terminal events must never push a botany-overflow event, even with overflow_to_ground mistakenly true"
        );
    }
}
