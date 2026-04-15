use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde_json::{Map, Value};
use std::fmt;
use std::time::Duration;

use crate::schema::agent_command::AgentCommandV1;
use crate::schema::channels::{
    CH_AGENT_COMMAND, CH_AGENT_NARRATE, CH_BREAKTHROUGH_EVENT, CH_COMBAT_REALTIME,
    CH_COMBAT_SUMMARY, CH_CULTIVATION_DEATH, CH_FORGE_EVENT, CH_INSIGHT_OFFER, CH_INSIGHT_REQUEST,
    CH_PLAYER_CHAT, CH_WORLD_STATE,
};
use crate::schema::chat_message::ChatMessageV1;
use crate::schema::combat_event::{CombatRealtimeEventV1, CombatSummaryV1};
use crate::schema::common::{MAX_COMMANDS_PER_TICK, MAX_NARRATION_LENGTH};
use crate::schema::cultivation::{
    BreakthroughEventV1, CultivationDeathV1, ForgeEventV1, InsightOfferV1, InsightRequestV1,
};
use crate::schema::narration::NarrationV1;
use crate::schema::world_state::WorldStateV1;

const BRIDGE_LOOP_INTERVAL: Duration = Duration::from_millis(25);
const REDIS_IO_TIMEOUT: Duration = Duration::from_millis(100);
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_millis(250);
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(5);
const OUTBOUND_DRAIN_BUDGET: usize = 16;
const CHAT_MESSAGE_MAX_LENGTH: usize = 256;

#[derive(Debug, Clone)]
pub enum RedisInbound {
    AgentCommand(AgentCommandV1),
    AgentNarration(NarrationV1),
    InsightOffer(InsightOfferV1),
}

#[derive(Debug, Clone)]
pub enum RedisOutbound {
    WorldState(WorldStateV1),
    #[allow(dead_code)]
    PlayerChat(ChatMessageV1),
    CombatRealtime(CombatRealtimeEventV1),
    CombatSummary(CombatSummaryV1),
    BreakthroughEvent(BreakthroughEventV1),
    ForgeEvent(ForgeEventV1),
    CultivationDeath(CultivationDeathV1),
    InsightRequest(InsightRequestV1),
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

#[derive(Debug, PartialEq, Eq)]
enum DrainOutcome {
    Healthy,
    Reconnect { reason: String },
    Stop,
}

#[derive(Debug, PartialEq, Eq)]
enum BridgeLoopControl {
    Reconnect { reason: String },
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubscriberTaskExit {
    StreamEnded,
    GameChannelClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReconnectSchedule {
    attempt: u32,
    delay: Duration,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ReconnectBackoff {
    attempts: u32,
}

impl ReconnectBackoff {
    fn next(&mut self) -> ReconnectSchedule {
        self.attempts = self.attempts.saturating_add(1);

        let shift = self.attempts.saturating_sub(1).min(16);
        let delay_ms = (RECONNECT_BACKOFF_INITIAL.as_millis() as u64)
            .saturating_mul(1u64 << shift)
            .min(RECONNECT_BACKOFF_MAX.as_millis() as u64);

        ReconnectSchedule {
            attempt: self.attempts,
            delay: Duration::from_millis(delay_ms),
        }
    }

    fn reset(&mut self) {
        self.attempts = 0;
    }
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
            let client = match redis::Client::open(url.as_str()) {
                Ok(client) => client,
                Err(error) => {
                    tracing::error!("[bong][redis] failed to open client: {error}");
                    return;
                }
            };
            let mut backoff = ReconnectBackoff::default();
            let mut pending_command = None;

            loop {
                match connect_bridge_session(&client, url.as_str(), &tx_to_game).await {
                    Ok((pub_conn, sub_task)) => {
                        backoff.reset();

                        match run_bridge_session(
                            &rx_from_game,
                            &mut pending_command,
                            pub_conn,
                            sub_task,
                        )
                        .await
                        {
                            BridgeLoopControl::Reconnect { reason } => {
                                sleep_before_reconnect(url.as_str(), &mut backoff, reason.as_str())
                                    .await;
                            }
                            BridgeLoopControl::Stop => break,
                        }
                    }
                    Err(error) => {
                        tracing::warn!("[bong][redis] {error}");
                        sleep_before_reconnect(url.as_str(), &mut backoff, error.as_str()).await;
                    }
                }
            }
        });
    });

    (handle, tx_outbound, rx_inbound)
}

async fn drain_outbound_messages(
    rx_from_game: &Receiver<RedisOutbound>,
    pub_conn: &mut redis::aio::MultiplexedConnection,
    pending_command: &mut Option<RedisIoCommand>,
) -> DrainOutcome {
    let mut drained = 0;

    if let Some(command) = pending_command.take() {
        if let Err(error) = execute_outbound_command(pub_conn, &command).await {
            *pending_command = Some(command);
            return DrainOutcome::Reconnect {
                reason: format!("outbound_retry_failed: {error}"),
            };
        }
    }

    while drained < OUTBOUND_DRAIN_BUDGET {
        match rx_from_game.try_recv() {
            Ok(message) => {
                drained += 1;

                match prepare_outbound_command(message) {
                    Ok(command) => {
                        if let Err(error) = execute_outbound_command(pub_conn, &command).await {
                            *pending_command = Some(command);
                            return DrainOutcome::Reconnect {
                                reason: format!("outbound_failed: {error}"),
                            };
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
                return DrainOutcome::Stop;
            }
        }
    }

    if drained == OUTBOUND_DRAIN_BUDGET {
        tracing::debug!(
            "[bong][redis] outbound drain hit budget {OUTBOUND_DRAIN_BUDGET}; remaining messages will flush next cycle"
        );
    }

    DrainOutcome::Healthy
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
        RedisOutbound::CombatRealtime(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize CombatRealtimeEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_COMBAT_REALTIME,
                payload,
            })
        }
        RedisOutbound::CombatSummary(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize CombatSummaryV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_COMBAT_SUMMARY,
                payload,
            })
        }
        RedisOutbound::BreakthroughEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize BreakthroughEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BREAKTHROUGH_EVENT,
                payload,
            })
        }
        RedisOutbound::ForgeEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ForgeEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FORGE_EVENT,
                payload,
            })
        }
        RedisOutbound::CultivationDeath(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize CultivationDeathV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_CULTIVATION_DEATH,
                payload,
            })
        }
        RedisOutbound::InsightRequest(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize InsightRequestV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_INSIGHT_REQUEST,
                payload,
            })
        }
    }
}

fn redact_redis_url_for_log(redis_url: &str) -> String {
    let Some(scheme_index) = redis_url.find("://") else {
        return "[redacted redis endpoint]".to_string();
    };

    let authority_and_path = &redis_url[(scheme_index + 3)..];
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let endpoint = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority)
        .trim();

    if endpoint.is_empty() {
        "[redacted redis endpoint]".to_string()
    } else {
        endpoint.to_string()
    }
}

async fn execute_outbound_command(
    pub_conn: &mut redis::aio::MultiplexedConnection,
    command: &RedisIoCommand,
) -> Result<(), String> {
    match command {
        RedisIoCommand::Publish { channel, payload } => {
            match tokio::time::timeout(
                REDIS_IO_TIMEOUT,
                redis::cmd("PUBLISH")
                    .arg(channel)
                    .arg(payload)
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
                    .arg(payload)
                    .query_async::<i64>(pub_conn),
            )
            .await
            {
                Ok(Ok(list_len)) => {
                    tracing::debug!(
                        "[bong][redis] pushed payload onto {key}; list length {list_len}"
                    );
                    Ok(())
                }
                Ok(Err(error)) => Err(format!("failed to RPUSH {key}: {error}")),
                Err(_) => Err(format!(
                    "timed out RPUSH {key} after {:?}",
                    REDIS_IO_TIMEOUT
                )),
            }
        }
    }
}

async fn connect_bridge_session(
    client: &redis::Client,
    redis_url: &str,
    tx_to_game: &Sender<RedisInbound>,
) -> Result<
    (
        redis::aio::MultiplexedConnection,
        tokio::task::JoinHandle<SubscriberTaskExit>,
    ),
    String,
> {
    tracing::info!(
        "[bong][redis] connecting to {}",
        redact_redis_url_for_log(redis_url)
    );

    let pub_conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|error| format!("failed to get pub connection: {error}"))?;

    let mut pubsub = client
        .get_async_pubsub()
        .await
        .map_err(|error| format!("failed to get pubsub connection: {error}"))?;

    subscribe_inbound_channels(&mut pubsub).await?;
    tracing::info!(
        "[bong][redis] subscribed to {CH_AGENT_COMMAND}, {CH_AGENT_NARRATE}, {CH_INSIGHT_OFFER}"
    );

    let tx_to_game_clone = tx_to_game.clone();
    let sub_task = tokio::spawn(async move { run_subscriber_task(pubsub, tx_to_game_clone).await });

    Ok((pub_conn, sub_task))
}

async fn subscribe_inbound_channels(pubsub: &mut redis::aio::PubSub) -> Result<(), String> {
    pubsub
        .subscribe(CH_AGENT_COMMAND)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_AGENT_COMMAND}: {error}"))?;

    pubsub
        .subscribe(CH_AGENT_NARRATE)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_AGENT_NARRATE}: {error}"))?;

    pubsub
        .subscribe(CH_INSIGHT_OFFER)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_INSIGHT_OFFER}: {error}"))?;

    Ok(())
}

async fn run_bridge_session(
    rx_from_game: &Receiver<RedisOutbound>,
    pending_command: &mut Option<RedisIoCommand>,
    mut pub_conn: redis::aio::MultiplexedConnection,
    sub_task: tokio::task::JoinHandle<SubscriberTaskExit>,
) -> BridgeLoopControl {
    let mut bridge_tick = tokio::time::interval(BRIDGE_LOOP_INTERVAL);
    let sub_task = sub_task;

    loop {
        bridge_tick.tick().await;

        match drain_outbound_messages(rx_from_game, &mut pub_conn, pending_command).await {
            DrainOutcome::Healthy => {}
            DrainOutcome::Reconnect { reason } => {
                abort_subscriber_task(sub_task).await;
                return BridgeLoopControl::Reconnect { reason };
            }
            DrainOutcome::Stop => {
                abort_subscriber_task(sub_task).await;
                return BridgeLoopControl::Stop;
            }
        }

        if sub_task.is_finished() {
            return handle_finished_subscriber_task(sub_task).await;
        }
    }
}

async fn abort_subscriber_task(sub_task: tokio::task::JoinHandle<SubscriberTaskExit>) {
    if !sub_task.is_finished() {
        sub_task.abort();
    }

    let _ = sub_task.await;
}

async fn handle_finished_subscriber_task(
    sub_task: tokio::task::JoinHandle<SubscriberTaskExit>,
) -> BridgeLoopControl {
    map_subscriber_join_result(sub_task.await)
}

fn map_subscriber_join_result(
    result: Result<SubscriberTaskExit, tokio::task::JoinError>,
) -> BridgeLoopControl {
    match result {
        Ok(SubscriberTaskExit::StreamEnded) => {
            tracing::warn!("[bong][redis] subscriber_ended reason=stream_ended");
            BridgeLoopControl::Reconnect {
                reason: "subscriber_ended:stream_ended".to_string(),
            }
        }
        Ok(SubscriberTaskExit::GameChannelClosed) => {
            tracing::warn!("[bong][redis] subscriber_ended reason=game_channel_closed");
            BridgeLoopControl::Reconnect {
                reason: "subscriber_ended:game_channel_closed".to_string(),
            }
        }
        Err(error) if error.is_cancelled() => BridgeLoopControl::Reconnect {
            reason: "subscriber_cancelled".to_string(),
        },
        Err(error) => {
            tracing::warn!("[bong][redis] subscriber_ended reason=join_error error={error}");
            BridgeLoopControl::Reconnect {
                reason: format!("subscriber_join_error: {error}"),
            }
        }
    }
}

async fn run_subscriber_task(
    mut pubsub: redis::aio::PubSub,
    tx_to_game: Sender<RedisInbound>,
) -> SubscriberTaskExit {
    use futures_util::StreamExt;

    let mut stream = pubsub.on_message();
    while let Some(message) = stream.next().await {
        let channel: String = match message.get_channel() {
            Ok(channel) => channel,
            Err(error) => {
                tracing::warn!("[bong][redis] failed to read inbound channel name: {error}");
                continue;
            }
        };

        let payload: String = match message.get_payload() {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!("[bong][redis] failed to read payload from {channel}: {error}");
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
                    RedisInbound::InsightOffer(offer) => tracing::info!(
                        "[bong][redis] received insight offer: trigger={} ({} choices)",
                        offer.trigger_id,
                        offer.choices.len()
                    ),
                }

                if tx_to_game.send(inbound).is_err() {
                    tracing::warn!(
                        "[bong][redis] inbound channel to game closed; stopping subscriber task"
                    );
                    return SubscriberTaskExit::GameChannelClosed;
                }
            }
            Ok(None) => {
                tracing::debug!("[bong][redis] ignoring message on unexpected channel {channel}");
            }
            Err(error) => tracing::warn!(
                "[bong][redis] dropped invalid inbound payload on {channel}: {error}"
            ),
        }
    }

    SubscriberTaskExit::StreamEnded
}

async fn sleep_before_reconnect(redis_url: &str, backoff: &mut ReconnectBackoff, reason: &str) {
    let schedule = backoff.next();
    tracing::info!(
        "[bong][redis] reconnect attempt={} endpoint={} reason={reason}",
        schedule.attempt,
        redact_redis_url_for_log(redis_url),
    );
    tracing::info!(
        "[bong][redis] backoff {:?} before reconnect attempt={}",
        schedule.delay,
        schedule.attempt,
    );
    tokio::time::sleep(schedule.delay).await;
}

fn parse_inbound_message(
    channel: &str,
    payload: &str,
) -> Result<Option<RedisInbound>, ValidationError> {
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
        CH_INSIGHT_OFFER => {
            let offer = serde_json::from_value::<InsightOfferV1>(value).map_err(|error| {
                ValidationError::new(format!("failed to deserialize InsightOfferV1: {error}"))
            })?;
            Ok(Some(RedisInbound::InsightOffer(offer)))
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
        return Err(ValidationError::new(format!(
            "{context}.params must be an object"
        )));
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
    validate_known_keys(
        object,
        &["scope", "target", "text", "style"],
        context.as_str(),
    )?;

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
    if !matches!(
        style,
        "system_warning" | "perception" | "narration" | "era_decree"
    ) {
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
    use crate::schema::combat_event::{
        CombatRealtimeEventV1, CombatRealtimeKindV1, CombatSummaryV1,
    };
    use tokio::task;

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
    fn publishes_cultivation_events_on_correct_channels() {
        let bt = prepare_outbound_command(RedisOutbound::BreakthroughEvent(BreakthroughEventV1 {
            kind: "Succeeded".into(),
            from_realm: "Awaken".into(),
            to_realm: Some("Induce".into()),
            success_rate: Some(0.9),
            severity: None,
        }))
        .expect("breakthrough payload should serialize");
        match bt {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_BREAKTHROUGH_EVENT);
                let v: Value = serde_json::from_str(&payload).unwrap();
                assert_eq!(v["kind"], "Succeeded");
                assert_eq!(v["to_realm"], "Induce");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let forge = prepare_outbound_command(RedisOutbound::ForgeEvent(ForgeEventV1 {
            meridian: "Lung".into(),
            axis: "Rate".into(),
            from_tier: 2,
            to_tier: 3,
            success: true,
        }))
        .expect("forge payload should serialize");
        match forge {
            RedisIoCommand::Publish { channel, .. } => assert_eq!(channel, CH_FORGE_EVENT),
            other => panic!("expected publish, got {other:?}"),
        }

        let death = prepare_outbound_command(RedisOutbound::CultivationDeath(CultivationDeathV1 {
            cause: "BreakthroughBackfire".into(),
            context: serde_json::json!({"from":"Spirit"}),
        }))
        .expect("death payload should serialize");
        match death {
            RedisIoCommand::Publish { channel, .. } => assert_eq!(channel, CH_CULTIVATION_DEATH),
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_combat_realtime_and_summary_on_correct_channels() {
        let realtime =
            prepare_outbound_command(RedisOutbound::CombatRealtime(CombatRealtimeEventV1 {
                v: 1,
                kind: CombatRealtimeKindV1::CombatEvent,
                tick: 44,
                target_id: "offline:Crimson".to_string(),
                attacker_id: Some("offline:Azure".to_string()),
                body_part: Some(crate::schema::combat_event::CombatBodyPartV1::Chest),
                wound_kind: Some(crate::schema::combat_event::CombatWoundKindV1::Blunt),
                damage: Some(20.0),
                contam_delta: None,
                description: Some(
                    "attack_intent offline:Azure -> offline:Crimson hit Chest with Blunt for 20.0 damage at 0.90 reach decay"
                        .to_string(),
                ),
                cause: None,
            }))
            .expect("combat realtime payload should serialize");
        match realtime {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_COMBAT_REALTIME);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["kind"], "combat_event");
                assert_eq!(v["tick"], 44);
                assert_eq!(v["target_id"], "offline:Crimson");
                assert_eq!(v["attacker_id"], "offline:Azure");
                assert_eq!(v["body_part"], "chest");
                assert_eq!(v["wound_kind"], "blunt");
                assert_eq!(v["damage"], 20.0);
                assert_eq!(
                    v["description"],
                    "attack_intent offline:Azure -> offline:Crimson hit Chest with Blunt for 20.0 damage at 0.90 reach decay"
                );
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let summary = prepare_outbound_command(RedisOutbound::CombatSummary(CombatSummaryV1 {
            v: 1,
            window_start_tick: 201,
            window_end_tick: 400,
            combat_event_count: 9,
            death_event_count: 2,
            damage_total: 88.0,
            contam_delta_total: 16.0,
        }))
        .expect("combat summary payload should serialize");
        match summary {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_COMBAT_SUMMARY);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["window_start_tick"], 201);
                assert_eq!(v["window_end_tick"], 400);
                assert_eq!(v["combat_event_count"], 9);
                assert_eq!(v["death_event_count"], 2);
                assert_eq!(v["damage_total"], 88.0);
                assert_eq!(v["contam_delta_total"], 16.0);
            }
            other => panic!("expected publish, got {other:?}"),
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

        let valid_agent_command =
            include_str!("../../../agent/packages/schema/samples/agent-command.sample.json");
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

    #[test]
    fn reconnect_backoff_grows_and_caps() {
        let mut backoff = ReconnectBackoff::default();

        let first = backoff.next();
        let second = backoff.next();
        let third = backoff.next();

        assert_eq!(first.attempt, 1);
        assert_eq!(first.delay, RECONNECT_BACKOFF_INITIAL);
        assert_eq!(second.attempt, 2);
        assert_eq!(second.delay, Duration::from_millis(500));
        assert_eq!(third.attempt, 3);
        assert_eq!(third.delay, Duration::from_secs(1));

        let mut capped = third;
        for _ in 0..8 {
            capped = backoff.next();
        }

        assert_eq!(capped.delay, RECONNECT_BACKOFF_MAX);

        backoff.reset();
        let reset = backoff.next();
        assert_eq!(reset.attempt, 1);
        assert_eq!(reset.delay, RECONNECT_BACKOFF_INITIAL);
    }

    #[test]
    fn redact_redis_url_for_log_strips_credentials_in_bridge_logs() {
        assert_eq!(
            redact_redis_url_for_log("redis://:password@cache.internal:6380/4"),
            "cache.internal:6380"
        );
        assert_eq!(
            redact_redis_url_for_log("rediss://user:password@[::1]:6390/0?tls=true"),
            "[::1]:6390"
        );
        assert_eq!(
            redact_redis_url_for_log("not-a-redis-url"),
            "[redacted redis endpoint]"
        );
    }

    #[tokio::test]
    async fn finished_subscriber_stream_triggers_reconnect() {
        let sub_task = task::spawn(async { SubscriberTaskExit::StreamEnded });

        assert_eq!(
            handle_finished_subscriber_task(sub_task).await,
            BridgeLoopControl::Reconnect {
                reason: "subscriber_ended:stream_ended".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn finished_subscriber_game_channel_close_triggers_reconnect() {
        let sub_task = task::spawn(async { SubscriberTaskExit::GameChannelClosed });

        assert_eq!(
            handle_finished_subscriber_task(sub_task).await,
            BridgeLoopControl::Reconnect {
                reason: "subscriber_ended:game_channel_closed".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn cancelled_subscriber_maps_to_reconnect() {
        let sub_task = task::spawn(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            SubscriberTaskExit::StreamEnded
        });
        sub_task.abort();

        assert_eq!(
            map_subscriber_join_result(sub_task.await),
            BridgeLoopControl::Reconnect {
                reason: "subscriber_cancelled".to_string(),
            }
        );
    }
}
