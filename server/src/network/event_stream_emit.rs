//! plan-HUD-v1 §2.3 / §11.4 server-side event_stream 推送。
//!
//! 把 server 已有的 `CombatEvent` / `DeathEvent` 翻译成 client `EventStreamPushV1`
//! 并 push 到相关 client（攻击方 + 受击方都收到，自打自除外）。
//!
//! 当前 v1 限制：仅战斗事件源；cultivation/world/system 路由后续接（每条只需要
//! 在对应 system 里 EventReader + 复用本文件的 push 函数）。

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Client, Entity, EventReader, Query, Username};

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
            EventPriorityV1::P0Critical,
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
        EventPriorityV1::P1Important,
        now_ms,
    );
}

fn push_to_client_priority(
    clients: &mut Query<(&Username, &mut Client)>,
    entity: Entity,
    source_tag: &str,
    text: &str,
    priority: EventPriorityV1,
    now_ms: u64,
) {
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return; // entity 不是 Client（NPC），跳过
    };

    let payload = ServerDataV1::new(ServerDataPayloadV1::EventStreamPush(EventStreamPushV1 {
        channel: EventChannelV1::Combat,
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
