use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde_json::{Map, Value};
use std::fmt;
use std::time::Duration;

use crate::schema::agent_command::AgentCommandV1;
use crate::schema::channels::{CH_AGENT_COMMAND, CH_AGENT_NARRATE, CH_PLAYER_CHAT, CH_WORLD_STATE};
use crate::schema::chat_message::ChatMessageV1;
use crate::schema::common::{MAX_COMMANDS_PER_TICK, MAX_NARRATION_LENGTH};
use crate::schema::narration::NarrationV1;
use crate::schema::world_state::WorldStateV1;

const BRIDGE_LOOP_INTERVAL: Duration = Duration::from_millis(25);
const REDIS_IO_TIMEOUT: Duration = Duration::from_millis(100);
const OUTBOUND_DRAIN_BUDGET: usize = 16;
const CHAT_MESSAGE_MAX_LENGTH: usize = 256;

#[derive(Debug, Clone)]
pub enum RedisInbound {
    AgentCommand(AgentCommandV1),
    AgentNarration(NarrationV1),
}

#[derive(Debug, Clone)]
pub enum RedisOutbound {
    WorldState(WorldStateV1),
    #[allow(dead_code)]
    PlayerChat(ChatMessageV1),
}

#[derive(Debug, PartialEq)]
enum RedisIoCommand {
    Publish {
        channel: &'static str,
        payload: String,
    },
    ListPush {
        key: &'static str,
        payload: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidationError(String);

impl ValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn spawn_redis_bridge(
    redis_url: &str,
) -> (
    std::thread::JoinHandle<()>,
    Sender<RedisOutbound>,
    Receiver<RedisInbound>,
) {
    let (tx_to_game, rx_inbound) = crossbeam_channel::unbounded::<RedisInbound>();
    let (tx_outbound, rx_from_game) = crossbeam_channel::unbounded::<RedisOutbound>();

    let url = redis_url.to_string();

    let handle = std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(error) => {
                tracing::error!("[bong][redis] failed to create tokio runtime: {error}");
                return;
            }
        };

        rt.block_on(async move {
            tracing::info!("[bong][redis] connecting to {url}");

            let client = match redis::Client::open(url.as_str()) {
                Ok(client) => client,
                Err(error) => {
                    tracing::error!("[bong][redis] failed to open client: {error}");
                    return;
                }
            };

            let mut pub_conn = match client.get_multiplexed_async_connection().await {
                Ok(connection) => connection,
                Err(error) => {
                    tracing::error!("[bong][redis] failed to get pub connection: {error}");
                    return;
                }
            };

            let mut pubsub = match client.get_async_pubsub().await {
                Ok(pubsub) => pubsub,
                Err(error) => {
                    tracing::error!("[bong][redis] failed to get pubsub connection: {error}");
                    return;
                }
            };

            if let Err(error) = pubsub.subscribe(CH_AGENT_COMMAND).await {
                tracing::error!(
                    "[bong][redis] failed to subscribe to {CH_AGENT_COMMAND}: {error}"
                );
                return;
            }

            if let Err(error) = pubsub.subscribe(CH_AGENT_NARRATE).await {
                tracing::error!(
                    "[bong][redis] failed to subscribe to {CH_AGENT_NARRATE}: {error}"
                );
                return;
            }

            tracing::info!(
                "[bong][redis] subscribed to {CH_AGENT_COMMAND} and {CH_AGENT_NARRATE}"
            );

            let tx_to_game_clone = tx_to_game.clone();
            let sub_task = tokio::spawn(async move {
                use futures_util::StreamExt;

                let mut stream = pubsub.on_message();
                while let Some(message) = stream.next().await {
                    let channel: String = match message.get_channel() {
                        Ok(channel) => channel,
                        Err(error) => {
                            tracing::warn!(
                                "[bong][redis] failed to read inbound channel name: {error}"
                            );
                            continue;
                        }
                    };

                    let payload: String = match message.get_payload() {
                        Ok(payload) => payload,
                        Err(error) => {
                            tracing::warn!(
                                "[bong][redis] failed to read payload from {channel}: {error}"
                            );
                            continue;
                        }
                    };

                    match parse_inbound_message(channel.as_str(), payload.as_str()) {
                        Ok(Some(inbound)) => {
                            match &inbound {
                                RedisInbound::AgentCommand(command) => tracing::info!(
                                    "[bong][redis] received agent command: {} ({} cmds)",
                                    command.id,
                                    command.commands.len()
                                ),
                                RedisInbound::AgentNarration(narration) => tracing::info!(
                                    "[bong][redis] received narration ({} entries)",
                                    narration.narrations.len()
                                ),
                            }

                            if tx_to_game_clone.send(inbound).is_err() {
                                tracing::warn!(
                                    "[bong][redis] inbound channel to game closed; stopping subscriber task"
                                );
                                break;
                            }
                        }
                        Ok(None) => {
                            tracing::debug!(
                                "[bong][redis] ignoring message on unexpected channel {channel}"
                            );
                        }
                        Err(error) => tracing::warn!(
                            "[bong][redis] dropped invalid inbound payload on {channel}: {error}"
                        ),
                    }
                }
            });

            let mut bridge_tick = tokio::time::interval(BRIDGE_LOOP_INTERVAL);
            loop {
                bridge_tick.tick().await;

                if !drain_outbound_messages(&rx_from_game, &mut pub_conn).await {
                    break;
                }

                if sub_task.is_finished() {
                    tracing::warn!("[bong][redis] subscriber task ended, stopping bridge");
                    break;
                }
            }
        });
    });

    (handle, tx_outbound, rx_inbound)
}

async fn drain_outbound_messages(
    rx_from_game: &Receiver<RedisOutbound>,
    pub_conn: &mut redis::aio::MultiplexedConnection,
) -> bool {
    let mut drained = 0;

    while drained < OUTBOUND_DRAIN_BUDGET {
        match rx_from_game.try_recv() {
            Ok(message) => {
                drained += 1;

                match prepare_outbound_command(message) {
                    Ok(command) => {
                        if let Err(error) = execute_outbound_command(pub_conn, command).await {
                            tracing::warn!("[bong][redis] {error}");
                        }
                    }
                    Err(error) => {
                        tracing::warn!("[bong][redis] dropped invalid outbound payload: {error}");
                    }
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                tracing::warn!("[bong][redis] outbound channel from game closed; stopping bridge");
                return false;
            }
        }
    }

    if drained == OUTBOUND_DRAIN_BUDGET {
        tracing::debug!(
            "[bong][redis] outbound drain hit budget {OUTBOUND_DRAIN_BUDGET}; remaining messages will flush next cycle"
        );
    }

    true
}

fn prepare_outbound_command(message: RedisOutbound) -> Result<RedisIoCommand, ValidationError> {
    match message {
        RedisOutbound::WorldState(state) => {
            validate_world_state(&state)?;

            let payload = serde_json::to_string(&state).map_err(|error| {
                ValidationError::new(format!("failed to serialize WorldStateV1: {error}"))
            })?;

            Ok(RedisIoCommand::Publish {
                channel: CH_WORLD_STATE,
                payload,
            })
        }
        RedisOutbound::PlayerChat(chat) => {
            validate_chat_message(&chat)?;

            let payload = serde_json::to_string(&chat).map_err(|error| {
                ValidationError::new(format!("failed to serialize ChatMessageV1: {error}"))
            })?;

            Ok(RedisIoCommand::ListPush {
                key: CH_PLAYER_CHAT,
                payload,
            })
        }
    }
}

async fn execute_outbound_command(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    command: RedisIoCommand,
) -> Result<(), String> {
    match command {
        RedisIoCommand::Publish { channel, payload } => {
            match tokio::time::timeout(
                REDIS_IO_TIMEOUT,
                redis::cmd("PUBLISH")
                    .arg(channel)
                    .arg(&payload)
                    .query_async::<i64>(pub_conn),
            )
            .await
            {
                Ok(Ok(subscribers)) => {
                    tracing::debug!(
                        "[bong][redis] published {channel}; observed {subscribers} subscribers"
                    );
                    Ok(())
                }
                Ok(Err(error)) => Err(format!("failed to publish {channel}: {error}")),
                Err(_) => Err(format!(
                    "timed out publishing {channel} after {:?}",
                    REDIS_IO_TIMEOUT
                )),
            }
        }
        RedisIoCommand::ListPush { key, payload } => {
            match tokio::time::timeout(
                REDIS_IO_TIMEOUT,
                redis::cmd("RPUSH")
                    .arg(key)
                    .arg(&payload)
                    .query_async::<i64>(pub_conn),
            )
            .await
            {
                Ok(Ok(list_len)) => {
                    tracing::debug!("[bong][redis] pushed payload onto {key}; list length {list_len}");
                    Ok(())
                }
                Ok(Err(error)) => Err(format!("failed to RPUSH {key}: {error}")),
                Err(_) => Err(format!("timed out RPUSH {key} after {:?}", REDIS_IO_TIMEOUT)),
            }
        }
    }
}

fn parse_inbound_message(channel: &str, payload: &str) -> Result<Option<RedisInbound>, ValidationError> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| ValidationError::new(format!("invalid JSON payload: {error}")))?;

    match channel {
        CH_AGENT_COMMAND => {
            validate_agent_command_value(&value)?;
            let command = serde_json::from_value::<AgentCommandV1>(value).map_err(|error| {
                ValidationError::new(format!("failed to deserialize AgentCommandV1: {error}"))
            })?;
            Ok(Some(RedisInbound::AgentCommand(command)))
        }
        CH_AGENT_NARRATE => {
            validate_narration_value(&value)?;
            let narration = serde_json::from_value::<NarrationV1>(value).map_err(|error| {
                ValidationError::new(format!("failed to deserialize NarrationV1: {error}"))
            })?;
            Ok(Some(RedisInbound::AgentNarration(narration)))
        }
        _ => Ok(None),
    }
}

fn validate_world_state(state: &WorldStateV1) -> Result<(), ValidationError> {
    if state.v != 1 {
        return Err(ValidationError::new(format!(
            "WorldStateV1 must use version 1, got {}",
            state.v
        )));
    }

    Ok(())
}

fn validate_chat_message(chat: &ChatMessageV1) -> Result<(), ValidationError> {
    if chat.v != 1 {
        return Err(ValidationError::new(format!(
            "ChatMessageV1 must use version 1, got {}",
            chat.v
        )));
    }

    if chat.raw.chars().count() > CHAT_MESSAGE_MAX_LENGTH {
        return Err(ValidationError::new(format!(
            "ChatMessageV1.raw exceeds {CHAT_MESSAGE_MAX_LENGTH} characters"
        )));
    }

    Ok(())
}

fn validate_agent_command_value(value: &Value) -> Result<(), ValidationError> {
    let object = expect_object(value, "AgentCommandV1")?;
    validate_known_keys(object, &["v", "id", "source", "commands"], "AgentCommandV1")?;
    validate_schema_version(object, "AgentCommandV1")?;
    expect_string_field(object, "id", "AgentCommandV1")?;

    if let Some(source) = object.get("source") {
        let source = source.as_str().ok_or_else(|| {
            ValidationError::new("AgentCommandV1.source must be a string when present")
        })?;

        if !matches!(source, "calamity" | "mutation" | "era" | "arbiter") {
            return Err(ValidationError::new(format!(
                "AgentCommandV1.source has unsupported value `{source}`"
            )));
        }
    }

    let commands = expect_array_field(object, "commands", "AgentCommandV1")?;
    if commands.len() > MAX_COMMANDS_PER_TICK {
        return Err(ValidationError::new(format!(
            "AgentCommandV1.commands exceeds maxItems {MAX_COMMANDS_PER_TICK}"
        )));
    }

    for (index, command) in commands.iter().enumerate() {
        validate_command_value(command, index)?;
    }

    Ok(())
}

fn validate_command_value(value: &Value, index: usize) -> Result<(), ValidationError> {
    let context = format!("AgentCommandV1.commands[{index}]");
    let object = expect_object(value, context.as_str())?;
    validate_known_keys(object, &["type", "target", "params"], context.as_str())?;

    let command_type = expect_string_field(object, "type", context.as_str())?;
    if !matches!(command_type, "spawn_event" | "modify_zone" | "npc_behavior") {
        return Err(ValidationError::new(format!(
            "{context}.type has unsupported value `{command_type}`"
        )));
    }

    expect_string_field(object, "target", context.as_str())?;

    let params = expect_field(object, "params", context.as_str())?;
    if !params.is_object() {
        return Err(ValidationError::new(format!("{context}.params must be an object")));
    }

    Ok(())
}

fn validate_narration_value(value: &Value) -> Result<(), ValidationError> {
    let object = expect_object(value, "NarrationV1")?;
    validate_known_keys(object, &["v", "narrations"], "NarrationV1")?;
    validate_schema_version(object, "NarrationV1")?;

    let narrations = expect_array_field(object, "narrations", "NarrationV1")?;
    for (index, narration) in narrations.iter().enumerate() {
        validate_narration_entry(narration, index)?;
    }

    Ok(())
}

fn validate_narration_entry(value: &Value, index: usize) -> Result<(), ValidationError> {
    let context = format!("NarrationV1.narrations[{index}]");
    let object = expect_object(value, context.as_str())?;
    validate_known_keys(object, &["scope", "target", "text", "style"], context.as_str())?;

    let scope = expect_string_field(object, "scope", context.as_str())?;
    if !matches!(scope, "broadcast" | "zone" | "player") {
        return Err(ValidationError::new(format!(
            "{context}.scope has unsupported value `{scope}`"
        )));
    }

    let has_target = object.get("target").is_some();
    if let Some(target) = object.get("target") {
        if !target.is_string() {
            return Err(ValidationError::new(format!(
                "{context}.target must be a string when present"
            )));
        }
    }

    if scope != "broadcast" && !has_target {
        return Err(ValidationError::new(format!(
            "{context}.target is required when scope is `{scope}`"
        )));
    }

    let text = expect_string_field(object, "text", context.as_str())?;
    if text.chars().count() > MAX_NARRATION_LENGTH {
        return Err(ValidationError::new(format!(
            "{context}.text exceeds {MAX_NARRATION_LENGTH} characters"
        )));
    }

    let style = expect_string_field(object, "style", context.as_str())?;
    if !matches!(style, "system_warning" | "perception" | "narration" | "era_decree") {
        return Err(ValidationError::new(format!(
            "{context}.style has unsupported value `{style}`"
        )));
    }

    Ok(())
}

fn validate_schema_version(
    object: &Map<String, Value>,
    context: &str,
) -> Result<(), ValidationError> {
    let version = expect_field(object, "v", context)?
        .as_u64()
        .ok_or_else(|| ValidationError::new(format!("{context}.v must be an integer")))?;

    if version != 1 {
        return Err(ValidationError::new(format!(
            "{context}.v must be 1, got {version}"
        )));
    }

    Ok(())
}

fn validate_known_keys(
    object: &Map<String, Value>,
    allowed_keys: &[&str],
    context: &str,
) -> Result<(), ValidationError> {
    if let Some(unexpected) = object
        .keys()
        .find(|key| !allowed_keys.contains(&key.as_str()))
    {
        return Err(ValidationError::new(format!(
            "{context} contains unsupported field `{unexpected}`"
        )));
    }

    Ok(())
}

fn expect_object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a Map<String, Value>, ValidationError> {
    value
        .as_object()
        .ok_or_else(|| ValidationError::new(format!("{context} must be a JSON object")))
}

fn expect_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a Value, ValidationError> {
    object.get(field).ok_or_else(|| {
        ValidationError::new(format!("{context} is missing required field `{field}`"))
    })
}

fn expect_string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, ValidationError> {
    expect_field(object, field, context)?
        .as_str()
        .ok_or_else(|| ValidationError::new(format!("{context}.{field} must be a string")))
}

fn expect_array_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a Vec<Value>, ValidationError> {
    expect_field(object, field, context)?
        .as_array()
        .ok_or_else(|| ValidationError::new(format!("{context}.{field} must be an array")))
}

#[cfg(test)]
mod redis_bridge_tests {
    use super::*;

    fn sample_world_state() -> WorldStateV1 {
        serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/world-state.sample.json"
        ))
        .expect("world-state sample should deserialize")
    }

    fn sample_chat_message() -> ChatMessageV1 {
        serde_json::from_str(include_str!(
            "../../../agent/packages/schema/samples/chat-message.sample.json"
        ))
        .expect("chat-message sample should deserialize")
    }

    #[test]
    fn publishes_world_state() {
        let command = prepare_outbound_command(RedisOutbound::WorldState(sample_world_state()))
            .expect("world state should produce a publish command");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_WORLD_STATE);

                let payload: Value =
                    serde_json::from_str(&payload).expect("publish payload should be valid JSON");
                assert_eq!(payload["v"], 1);
                assert_eq!(payload["tick"], 84000);
            }
            other => panic!("expected PUBLISH command, got {other:?}"),
        }
    }

    #[test]
    fn pushes_chat_messages() {
        let command = prepare_outbound_command(RedisOutbound::PlayerChat(sample_chat_message()))
            .expect("chat payload should produce an RPUSH command");

        match command {
            RedisIoCommand::ListPush { key, payload } => {
                assert_eq!(key, CH_PLAYER_CHAT);

                let payload: Value =
                    serde_json::from_str(&payload).expect("chat payload should be valid JSON");
                assert_eq!(payload["v"], 1);
                assert_eq!(payload["player"], "offline:Steve");
            }
            other => panic!("expected RPUSH command, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_inbound_payloads() {
        let invalid_agent_command = r#"{
            "v": 1,
            "id": "cmd_bad",
            "commands": [],
            "unexpected": true
        }"#;
        let invalid_narration = format!(
            r#"{{
                "v": 1,
                "narrations": [{{
                    "scope": "broadcast",
                    "text": "{}",
                    "style": "narration"
                }}]
            }}"#,
            "x".repeat(MAX_NARRATION_LENGTH + 1)
        );

        assert!(parse_inbound_message(CH_AGENT_COMMAND, invalid_agent_command).is_err());
        assert!(parse_inbound_message(CH_AGENT_NARRATE, &invalid_narration).is_err());

        let valid_agent_command = include_str!(
            "../../../agent/packages/schema/samples/agent-command.sample.json"
        );
        assert!(matches!(
            parse_inbound_message(CH_AGENT_COMMAND, valid_agent_command)
                .expect("valid command payload should pass"),
            Some(RedisInbound::AgentCommand(_))
        ));

        let arbiter_agent_command = r#"{
            "v": 1,
            "id": "cmd_arbiter",
            "source": "arbiter",
            "commands": []
        }"#;
        assert!(matches!(
            parse_inbound_message(CH_AGENT_COMMAND, arbiter_agent_command)
                .expect("arbiter command payload should pass"),
            Some(RedisInbound::AgentCommand(_))
        ));
    }
}
