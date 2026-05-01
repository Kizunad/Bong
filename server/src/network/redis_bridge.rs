use crossbeam_channel::{Receiver, Sender, TryRecvError};
use serde_json::{Map, Value};
use std::fmt;
use std::time::Duration;

use crate::schema::agent_command::AgentCommandV1;
use crate::schema::agent_world_model::AgentWorldModelEnvelopeV1;
use crate::schema::alchemy::{
    AlchemyInterventionResultV1, AlchemySessionEndV1, AlchemySessionStartV1,
};
use crate::schema::armor_event::ArmorDurabilityChangedV1;
use crate::schema::botany::BotanyEcologySnapshotV1;
use crate::schema::channels::{
    CH_AGENT_COMMAND, CH_AGENT_NARRATE, CH_AGENT_WORLD_MODEL, CH_AGING,
    CH_ALCHEMY_INTERVENTION_RESULT, CH_ALCHEMY_SESSION_END, CH_ALCHEMY_SESSION_START,
    CH_ARMOR_DURABILITY_CHANGED, CH_BOTANY_ECOLOGY, CH_BREAKTHROUGH_EVENT, CH_COMBAT_REALTIME,
    CH_COMBAT_SUMMARY, CH_CULTIVATION_DEATH, CH_DEATH_INSIGHT, CH_DUO_SHE_EVENT, CH_FACTION_EVENT,
    CH_FORGE_EVENT, CH_FORGE_OUTCOME, CH_FORGE_START, CH_INSIGHT_OFFER, CH_INSIGHT_REQUEST,
    CH_LIFESPAN_EVENT, CH_NPC_DEATH, CH_NPC_SPAWN, CH_PLAYER_CHAT, CH_REBIRTH,
    CH_SKILL_CAP_CHANGED, CH_SKILL_LV_UP, CH_SKILL_SCROLL_USED, CH_SKILL_XP_GAIN,
    CH_SOCIAL_EXPOSURE, CH_SOCIAL_FEUD, CH_SOCIAL_PACT, CH_SOCIAL_RENOWN_DELTA, CH_TSY_EVENT,
    CH_WORLD_STATE,
};
use crate::schema::chat_message::ChatMessageV1;
use crate::schema::combat_event::{CombatRealtimeEventV1, CombatSummaryV1};
use crate::schema::common::{MAX_COMMANDS_PER_TICK, MAX_NARRATION_LENGTH};
use crate::schema::cultivation::{
    BreakthroughEventV1, CultivationDeathV1, ForgeEventV1, InsightOfferV1, InsightRequestV1,
};
use crate::schema::death_insight::DeathInsightRequestV1;
use crate::schema::death_lifecycle::{
    AgingEventV1, DuoSheEventV1, LifespanEventV1, RebirthEventV1,
};
use crate::schema::forge_bridge::{ForgeOutcomePayloadV1, ForgeStartPayloadV1};
use crate::schema::narration::NarrationV1;
use crate::schema::npc::{FactionEventV1, NpcDeathV1, NpcSpawnedV1};
use crate::schema::skill::{
    SkillCapChangedPayloadV1, SkillLvUpPayloadV1, SkillScrollUsedPayloadV1, SkillXpGainPayloadV1,
};
use crate::schema::social::{
    SocialExposureEventV1, SocialFeudEventV1, SocialPactEventV1, SocialRenownDeltaV1,
};
use crate::schema::tsy::{TsyEnterEventV1, TsyExitEventV1};
use crate::schema::tsy_hostile::{TsyNpcSpawnedV1, TsySentinelPhaseChangedV1};
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
    AgentWorldModel(AgentWorldModelEnvelopeV1),
    InsightOffer(InsightOfferV1),
}

#[derive(Debug, Clone)]
pub enum RedisOutbound {
    WorldState(WorldStateV1),
    #[allow(dead_code)]
    PlayerChat(ChatMessageV1),
    CombatRealtime(CombatRealtimeEventV1),
    CombatSummary(CombatSummaryV1),
    ArmorDurabilityChanged(ArmorDurabilityChangedV1),
    BreakthroughEvent(BreakthroughEventV1),
    ForgeEvent(ForgeEventV1),
    ForgeStart(ForgeStartPayloadV1),
    ForgeOutcome(ForgeOutcomePayloadV1),
    AlchemySessionStart(AlchemySessionStartV1),
    AlchemySessionEnd(AlchemySessionEndV1),
    AlchemyInterventionResult(AlchemyInterventionResultV1),
    CultivationDeath(CultivationDeathV1),
    InsightRequest(InsightRequestV1),
    DeathInsight(DeathInsightRequestV1),
    Aging(AgingEventV1),
    LifespanEvent(LifespanEventV1),
    DuoSheEvent(DuoSheEventV1),
    Rebirth(RebirthEventV1),
    SkillXpGain(SkillXpGainPayloadV1),
    SkillLvUp(SkillLvUpPayloadV1),
    SkillCapChanged(SkillCapChangedPayloadV1),
    SkillScrollUsed(SkillScrollUsedPayloadV1),
    NpcSpawned(NpcSpawnedV1),
    NpcDeath(NpcDeathV1),
    FactionEvent(FactionEventV1),
    BotanyEcology(BotanyEcologySnapshotV1),
    TsyEnter(TsyEnterEventV1),
    TsyExit(TsyExitEventV1),
    TsyNpcSpawned(TsyNpcSpawnedV1),
    TsySentinelPhaseChanged(TsySentinelPhaseChangedV1),
    SocialExposure(SocialExposureEventV1),
    SocialPact(SocialPactEventV1),
    SocialFeud(SocialFeudEventV1),
    SocialRenownDelta(SocialRenownDeltaV1),
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
        RedisOutbound::ArmorDurabilityChanged(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ArmorDurabilityChangedV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ARMOR_DURABILITY_CHANGED,
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
        RedisOutbound::ForgeStart(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize ForgeStartPayloadV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FORGE_START,
                payload,
            })
        }
        RedisOutbound::ForgeOutcome(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize ForgeOutcomePayloadV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FORGE_OUTCOME,
                payload,
            })
        }
        RedisOutbound::AlchemySessionStart(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize AlchemySessionStartV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ALCHEMY_SESSION_START,
                payload,
            })
        }
        RedisOutbound::AlchemySessionEnd(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize AlchemySessionEndV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ALCHEMY_SESSION_END,
                payload,
            })
        }
        RedisOutbound::AlchemyInterventionResult(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize AlchemyInterventionResultV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_ALCHEMY_INTERVENTION_RESULT,
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
        RedisOutbound::DeathInsight(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize DeathInsightRequestV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DEATH_INSIGHT,
                payload,
            })
        }
        RedisOutbound::Aging(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize AgingEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_AGING,
                payload,
            })
        }
        RedisOutbound::LifespanEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize LifespanEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_LIFESPAN_EVENT,
                payload,
            })
        }
        RedisOutbound::DuoSheEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize DuoSheEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_DUO_SHE_EVENT,
                payload,
            })
        }
        RedisOutbound::Rebirth(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize RebirthEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_REBIRTH,
                payload,
            })
        }
        RedisOutbound::SkillXpGain(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SkillXpGainPayloadV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SKILL_XP_GAIN,
                payload,
            })
        }
        RedisOutbound::SkillLvUp(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SkillLvUpPayloadV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SKILL_LV_UP,
                payload,
            })
        }
        RedisOutbound::SkillCapChanged(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SkillCapChangedPayloadV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SKILL_CAP_CHANGED,
                payload,
            })
        }
        RedisOutbound::SkillScrollUsed(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SkillScrollUsedPayloadV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SKILL_SCROLL_USED,
                payload,
            })
        }
        RedisOutbound::NpcSpawned(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize NpcSpawnedV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_NPC_SPAWN,
                payload,
            })
        }
        RedisOutbound::NpcDeath(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize NpcDeathV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_NPC_DEATH,
                payload,
            })
        }
        RedisOutbound::FactionEvent(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize FactionEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_FACTION_EVENT,
                payload,
            })
        }
        RedisOutbound::BotanyEcology(snapshot) => {
            let payload = serde_json::to_string(&snapshot).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize BotanyEcologySnapshotV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_BOTANY_ECOLOGY,
                payload,
            })
        }
        RedisOutbound::TsyEnter(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TsyEnterEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::TsyExit(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TsyExitEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::TsyNpcSpawned(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize TsyNpcSpawnedV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::TsySentinelPhaseChanged(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize TsySentinelPhaseChangedV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_TSY_EVENT,
                payload,
            })
        }
        RedisOutbound::SocialExposure(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!(
                    "failed to serialize SocialExposureEventV1: {error}"
                ))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_EXPOSURE,
                payload,
            })
        }
        RedisOutbound::SocialPact(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SocialPactEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_PACT,
                payload,
            })
        }
        RedisOutbound::SocialFeud(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SocialFeudEventV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_FEUD,
                payload,
            })
        }
        RedisOutbound::SocialRenownDelta(evt) => {
            let payload = serde_json::to_string(&evt).map_err(|error| {
                ValidationError::new(format!("failed to serialize SocialRenownDeltaV1: {error}"))
            })?;
            Ok(RedisIoCommand::Publish {
                channel: CH_SOCIAL_RENOWN_DELTA,
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
        "[bong][redis] subscribed to {CH_AGENT_COMMAND}, {CH_AGENT_NARRATE}, {CH_AGENT_WORLD_MODEL}, {CH_INSIGHT_OFFER}"
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
        .subscribe(CH_AGENT_WORLD_MODEL)
        .await
        .map_err(|error| format!("failed to subscribe to {CH_AGENT_WORLD_MODEL}: {error}"))?;

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
                    RedisInbound::AgentWorldModel(envelope) => tracing::info!(
                        "[bong][redis] received world model envelope: {} (last_tick={:?})",
                        envelope.id,
                        envelope.snapshot.last_tick
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
        CH_AGENT_WORLD_MODEL => {
            validate_agent_world_model_value(&value)?;
            let envelope =
                serde_json::from_value::<AgentWorldModelEnvelopeV1>(value).map_err(|error| {
                    ValidationError::new(format!(
                        "failed to deserialize AgentWorldModelEnvelopeV1: {error}"
                    ))
                })?;
            Ok(Some(RedisInbound::AgentWorldModel(envelope)))
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

fn validate_agent_world_model_value(value: &Value) -> Result<(), ValidationError> {
    let object = expect_object(value, "AgentWorldModelEnvelopeV1")?;
    validate_known_keys(
        object,
        &["v", "id", "source", "snapshot"],
        "AgentWorldModelEnvelopeV1",
    )?;
    validate_schema_version(object, "AgentWorldModelEnvelopeV1")?;
    expect_string_field(object, "id", "AgentWorldModelEnvelopeV1")?;

    if let Some(source) = object.get("source") {
        let source = source.as_str().ok_or_else(|| {
            ValidationError::new("AgentWorldModelEnvelopeV1.source must be a string when present")
        })?;

        if !matches!(source, "calamity" | "mutation" | "era" | "arbiter") {
            return Err(ValidationError::new(format!(
                "AgentWorldModelEnvelopeV1.source has unsupported value `{source}`"
            )));
        }
    }

    let snapshot = expect_field(object, "snapshot", "AgentWorldModelEnvelopeV1")?;
    if !snapshot.is_object() {
        return Err(ValidationError::new(
            "AgentWorldModelEnvelopeV1.snapshot must be an object",
        ));
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
    if !matches!(
        command_type,
        "spawn_event"
            | "spawn_npc"
            | "despawn_npc"
            | "faction_event"
            | "modify_zone"
            | "npc_behavior"
    ) {
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

    if command_type == "spawn_npc" {
        let params = params
            .as_object()
            .ok_or_else(|| ValidationError::new(format!("{context}.params must be an object")))?;
        let archetype = params.get("archetype").ok_or_else(|| {
            ValidationError::new(format!(
                "{context}.params is missing required field `archetype`"
            ))
        })?;
        if !archetype.is_string() {
            return Err(ValidationError::new(format!(
                "{context}.params.archetype must be a string"
            )));
        }
    }

    if command_type == "faction_event" {
        let params = params
            .as_object()
            .ok_or_else(|| ValidationError::new(format!("{context}.params must be an object")))?;
        let kind = params.get("kind").ok_or_else(|| {
            ValidationError::new(format!("{context}.params is missing required field `kind`"))
        })?;
        if !kind.is_string() {
            return Err(ValidationError::new(format!(
                "{context}.params.kind must be a string"
            )));
        }

        let faction_id = params.get("faction_id").ok_or_else(|| {
            ValidationError::new(format!(
                "{context}.params is missing required field `faction_id`"
            ))
        })?;
        if !faction_id.is_string() {
            return Err(ValidationError::new(format!(
                "{context}.params.faction_id must be a string"
            )));
        }
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
        &["scope", "target", "text", "style", "kind"],
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

    if let Some(kind) = object.get("kind") {
        let Some(kind) = kind.as_str() else {
            return Err(ValidationError::new(format!(
                "{context}.kind must be a string when present"
            )));
        };
        if kind != "death_insight" {
            return Err(ValidationError::new(format!(
                "{context}.kind has unsupported value `{kind}`"
            )));
        }
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
    use crate::schema::death_insight::{
        DeathInsightCategoryV1, DeathInsightRequestV1, DeathInsightZoneKindV1,
    };
    use crate::schema::death_lifecycle::{AgingEventKindV1, LifespanEventKindV1};
    use crate::schema::forge::ForgeOutcomeBucketV1;
    use crate::schema::social::{ExposureKindV1, RenownTagV1};
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
    fn publishes_npc_and_faction_events_on_dedicated_channels() {
        let spawned = prepare_outbound_command(RedisOutbound::NpcSpawned(NpcSpawnedV1 {
            v: 1,
            kind: "npc_spawned".to_string(),
            npc_id: "npc_1v1".to_string(),
            archetype: "rogue".to_string(),
            source: "agent_command".to_string(),
            zone: "spawn".to_string(),
            pos: [1.0, 66.0, 2.0],
            initial_age_ticks: 0.0,
            at_tick: 0,
        }))
        .expect("NPC spawn payload should serialize");
        match spawned {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_NPC_SPAWN);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "npc_spawned");
                assert_eq!(v["archetype"], "rogue");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let death = prepare_outbound_command(RedisOutbound::NpcDeath(NpcDeathV1 {
            v: 1,
            kind: "npc_death".to_string(),
            npc_id: "npc_1v1".to_string(),
            archetype: "commoner".to_string(),
            cause: "natural_aging".to_string(),
            faction_id: None,
            life_record_snapshot: Some("生平摘要".to_string()),
            age_ticks: 10.0,
            max_age_ticks: 10.0,
            at_tick: 0,
        }))
        .expect("NPC death payload should serialize");
        match death {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_NPC_DEATH);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "npc_death");
                assert_eq!(v["cause"], "natural_aging");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let faction = prepare_outbound_command(RedisOutbound::FactionEvent(FactionEventV1 {
            v: 1,
            kind: "faction_event".to_string(),
            faction_id: "attack".to_string(),
            event_kind: "adjust_loyalty_bias".to_string(),
            leader_id: None,
            loyalty_bias: 0.6,
            mission_queue_size: 1,
            at_tick: 0,
        }))
        .expect("faction payload should serialize");
        match faction {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_FACTION_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "faction_event");
                assert_eq!(v["event_kind"], "adjust_loyalty_bias");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_forge_start_on_correct_channel() {
        let payload = ForgeStartPayloadV1 {
            v: 1,
            session_id: 7,
            blueprint_id: "qing_feng_v0".to_string(),
            station_id: "forge_station_42".to_string(),
            caster_id: "offline:Azure".to_string(),
            materials: vec![crate::schema::forge_bridge::ForgeMaterialStackV1 {
                material: "fan_tie".to_string(),
                count: 3,
            }],
            ts: 84_000,
        };

        let command = prepare_outbound_command(RedisOutbound::ForgeStart(payload))
            .expect("forge start payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_FORGE_START);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["session_id"], 7);
                assert_eq!(v["blueprint_id"], "qing_feng_v0");
                assert_eq!(v["materials"][0]["material"], "fan_tie");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_forge_outcome_on_correct_channel() {
        let cases = [
            ForgeOutcomePayloadV1 {
                v: 1,
                session_id: 7,
                blueprint_id: "qing_feng_v0".to_string(),
                bucket: ForgeOutcomeBucketV1::Perfect,
                weapon_item: Some("qing_feng_sword".to_string()),
                quality: 0.98,
                color: Some(crate::cultivation::components::ColorKind::Sharp),
                side_effects: vec![],
                achieved_tier: 2,
                caster_id: "offline:Azure".to_string(),
                ts: 84_020,
            },
            ForgeOutcomePayloadV1 {
                v: 1,
                session_id: 8,
                blueprint_id: "qing_feng_v0".to_string(),
                bucket: ForgeOutcomeBucketV1::Flawed,
                weapon_item: Some("iron_sword".to_string()),
                quality: 0.42,
                color: None,
                side_effects: vec!["brittle_edge".to_string()],
                achieved_tier: 1,
                caster_id: "offline:Azure".to_string(),
                ts: 84_040,
            },
        ];

        for case in cases {
            let command = prepare_outbound_command(RedisOutbound::ForgeOutcome(case))
                .expect("forge outcome payload should serialize");
            match command {
                RedisIoCommand::Publish { channel, payload } => {
                    assert_eq!(channel, CH_FORGE_OUTCOME);
                    let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                    assert_eq!(v["v"], 1);
                    assert!(matches!(
                        v["bucket"].as_str(),
                        Some("perfect") | Some("flawed")
                    ));
                }
                other => panic!("expected publish, got {other:?}"),
            }
        }
    }

    #[test]
    fn publishes_alchemy_bridge_payloads_on_correct_channels() {
        let start =
            prepare_outbound_command(RedisOutbound::AlchemySessionStart(AlchemySessionStartV1 {
                v: 1,
                session_id: "alchemy:-12:64:38:kai_mai_pill_v0".to_string(),
                recipe_id: "kai_mai_pill_v0".to_string(),
                furnace_pos: (-12, 64, 38),
                furnace_tier: 1,
                caster_id: "offline:Azure".to_string(),
                ts: 84_000,
            }))
            .expect("alchemy session start payload should serialize");
        match start {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ALCHEMY_SESSION_START);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["recipe_id"], "kai_mai_pill_v0");
                assert_eq!(v["furnace_pos"], serde_json::json!([-12, 64, 38]));
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let end = prepare_outbound_command(RedisOutbound::AlchemySessionEnd(AlchemySessionEndV1 {
            v: 1,
            session_id: "alchemy:-12:64:38:kai_mai_pill_v0".to_string(),
            recipe_id: Some("kai_mai_pill_v0".to_string()),
            furnace_pos: (-12, 64, 38),
            furnace_tier: 1,
            caster_id: "offline:Azure".to_string(),
            bucket: crate::schema::alchemy::AlchemyOutcomeBucketV1::Explode,
            pill: None,
            quality: None,
            damage: Some(12.0),
            meridian_crack: Some(0.2),
            elapsed_ticks: 120,
            ts: 84_120,
        }))
        .expect("alchemy session end payload should serialize");
        match end {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ALCHEMY_SESSION_END);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["bucket"], "explode");
                assert_eq!(v["damage"], 12.0);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let intervention = prepare_outbound_command(RedisOutbound::AlchemyInterventionResult(
            AlchemyInterventionResultV1 {
                v: 1,
                session_id: "alchemy:-12:64:38:kai_mai_pill_v0".to_string(),
                recipe_id: "kai_mai_pill_v0".to_string(),
                furnace_pos: (-12, 64, 38),
                caster_id: "offline:Azure".to_string(),
                intervention: crate::schema::alchemy::AlchemyInterventionV1::InjectQi { qi: 3.0 },
                temp_current: 0.6,
                qi_injected: 3.0,
                accepted: true,
                message: None,
                ts: 84_020,
            },
        ))
        .expect("alchemy intervention payload should serialize");
        match intervention {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ALCHEMY_INTERVENTION_RESULT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["intervention"]["kind"], "inject_qi");
                assert_eq!(v["accepted"], true);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_death_insight_on_correct_channel() {
        let command =
            prepare_outbound_command(RedisOutbound::DeathInsight(DeathInsightRequestV1 {
                v: 1,
                request_id: "death_insight:offline:Azure:84000:3".to_string(),
                character_id: "offline:Azure".to_string(),
                at_tick: 84_000,
                cause: "cultivation:NaturalAging".to_string(),
                category: DeathInsightCategoryV1::Natural,
                realm: Some("Condense".to_string()),
                player_realm: Some("qi_refining_6".to_string()),
                zone_kind: DeathInsightZoneKindV1::Ordinary,
                death_count: 3,
                rebirth_chance: None,
                lifespan_remaining_years: Some(0.0),
                recent_biography: vec!["t83980:near_death:cultivation:NaturalAging".to_string()],
                position: None,
                context: serde_json::json!({"will_terminate": true}),
            }))
            .expect("death insight payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_DEATH_INSIGHT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["character_id"], "offline:Azure");
                assert_eq!(v["category"], "natural");
                assert_eq!(v["zone_kind"], "ordinary");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_lifespan_and_aging_events_on_correct_channels() {
        let lifespan = prepare_outbound_command(RedisOutbound::LifespanEvent(LifespanEventV1 {
            v: 1,
            character_id: "offline:Azure".to_string(),
            at_tick: 84_000,
            kind: LifespanEventKindV1::DeathPenalty,
            delta_years: -4,
            source: "bleed_out".to_string(),
        }))
        .expect("lifespan payload should serialize");

        match lifespan {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_LIFESPAN_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["character_id"], "offline:Azure");
                assert_eq!(v["kind"], "death_penalty");
                assert_eq!(v["delta_years"], -4);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let aging = prepare_outbound_command(RedisOutbound::Aging(AgingEventV1 {
            v: 1,
            character_id: "offline:Azure".to_string(),
            at_tick: 84_000,
            kind: AgingEventKindV1::NaturalDeath,
            years_lived: 80.0,
            cap_by_realm: 80,
            remaining_years: 0.0,
            tick_rate_multiplier: 1.0,
            source: "online".to_string(),
        }))
        .expect("aging payload should serialize");

        match aging {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_AGING);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["character_id"], "offline:Azure");
                assert_eq!(v["kind"], "natural_death");
                assert_eq!(v["remaining_years"], 0.0);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let rebirth = prepare_outbound_command(RedisOutbound::Rebirth(RebirthEventV1 {
            v: 1,
            character_id: "offline:Azure".to_string(),
            at_tick: 84_100,
            prior_realm: "Induce".to_string(),
            new_realm: "Awaken".to_string(),
        }))
        .expect("rebirth payload should serialize");

        match rebirth {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_REBIRTH);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["character_id"], "offline:Azure");
                assert_eq!(v["prior_realm"], "Induce");
                assert_eq!(v["new_realm"], "Awaken");
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_skill_events_on_skill_channels() {
        let xp = prepare_outbound_command(RedisOutbound::SkillXpGain(SkillXpGainPayloadV1::new(
            1001,
            crate::schema::skill::SkillIdV1::Combat,
            4,
            crate::schema::skill::XpGainSourceV1::Action {
                plan_id: "combat".to_string(),
                action: "kill_npc".to_string(),
            },
        )))
        .expect("skill xp payload should serialize");
        match xp {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SKILL_XP_GAIN);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["skill"], "combat");
                assert_eq!(v["amount"], 4);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let lv = prepare_outbound_command(RedisOutbound::SkillLvUp(SkillLvUpPayloadV1::new(
            1001,
            crate::schema::skill::SkillIdV1::Mineral,
            2,
        )))
        .expect("skill level payload should serialize");
        match lv {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SKILL_LV_UP);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["skill"], "mineral");
                assert_eq!(v["new_lv"], 2);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let cap = prepare_outbound_command(RedisOutbound::SkillCapChanged(
            SkillCapChangedPayloadV1::new(1001, crate::schema::skill::SkillIdV1::Cultivation, 7),
        ))
        .expect("skill cap payload should serialize");
        match cap {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SKILL_CAP_CHANGED);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["skill"], "cultivation");
                assert_eq!(v["new_cap"], 7);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let scroll = prepare_outbound_command(RedisOutbound::SkillScrollUsed(
            SkillScrollUsedPayloadV1::new(
                1001,
                "scroll:mine_cave_scrap",
                crate::schema::skill::SkillIdV1::Mineral,
                100,
                false,
            ),
        ))
        .expect("skill scroll payload should serialize");
        match scroll {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SKILL_SCROLL_USED);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["skill"], "mineral");
                assert_eq!(v["scroll_id"], "scroll:mine_cave_scrap");
            }
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
    fn publishes_armor_durability_changed_on_correct_channel() {
        let command = prepare_outbound_command(RedisOutbound::ArmorDurabilityChanged(
            ArmorDurabilityChangedV1 {
                v: 1,
                entity_id: "offline:Crimson".to_string(),
                slot: crate::schema::inventory::EquipSlotV1::Chest,
                instance_id: 88,
                template_id: "fake_spirit_hide".to_string(),
                cur: 0.0,
                max: 100.0,
                durability_ratio: 0.0,
                broken: true,
            },
        ))
        .expect("armor durability payload should serialize");

        match command {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_ARMOR_DURABILITY_CHANGED);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["v"], 1);
                assert_eq!(v["entity_id"], "offline:Crimson");
                assert_eq!(v["slot"], "chest");
                assert_eq!(v["instance_id"], 88);
                assert_eq!(v["template_id"], "fake_spirit_hide");
                assert_eq!(v["broken"], true);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_tsy_hostile_events_on_tsy_channel() {
        let spawned = prepare_outbound_command(RedisOutbound::TsyNpcSpawned(TsyNpcSpawnedV1 {
            v: 1,
            kind: "tsy_npc_spawned".to_string(),
            family_id: "tsy_zongmen_yiji_01".to_string(),
            archetype: crate::schema::tsy_hostile::TsyHostileArchetypeV1::GuardianRelicSentinel,
            count: 3,
            at_tick: 12000,
        }))
        .expect("TSY NPC spawned payload should serialize");
        match spawned {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_TSY_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "tsy_npc_spawned");
                assert_eq!(v["archetype"], "guardian_relic_sentinel");
                assert_eq!(v["count"], 3);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let phase = prepare_outbound_command(RedisOutbound::TsySentinelPhaseChanged(
            TsySentinelPhaseChangedV1 {
                v: 1,
                kind: "tsy_sentinel_phase_changed".to_string(),
                family_id: "tsy_zongmen_yiji_01".to_string(),
                container_entity_id: 42,
                phase: 1,
                max_phase: 3,
                at_tick: 12345,
            },
        ))
        .expect("TSY sentinel phase payload should serialize");
        match phase {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_TSY_EVENT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "tsy_sentinel_phase_changed");
                assert_eq!(v["container_entity_id"], 42);
                assert_eq!(v["phase"], 1);
            }
            other => panic!("expected publish, got {other:?}"),
        }
    }

    #[test]
    fn publishes_social_events_on_social_channels() {
        let exposure =
            prepare_outbound_command(RedisOutbound::SocialExposure(SocialExposureEventV1 {
                v: 1,
                actor: "char:alice".to_string(),
                kind: ExposureKindV1::Chat,
                witnesses: vec!["char:bob".to_string()],
                tick: 120,
                zone: Some("spawn".to_string()),
            }))
            .expect("social exposure should serialize");
        match exposure {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SOCIAL_EXPOSURE);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["kind"], "chat");
                assert_eq!(v["witnesses"][0], "char:bob");
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let pact = prepare_outbound_command(RedisOutbound::SocialPact(SocialPactEventV1 {
            v: 1,
            left: "char:alice".to_string(),
            right: "char:bob".to_string(),
            terms: "shared shelter".to_string(),
            tick: 121,
            broken: false,
        }))
        .expect("social pact should serialize");
        match pact {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SOCIAL_PACT);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["left"], "char:alice");
                assert_eq!(v["broken"], false);
            }
            other => panic!("expected publish, got {other:?}"),
        }

        let renown =
            prepare_outbound_command(RedisOutbound::SocialRenownDelta(SocialRenownDeltaV1 {
                v: 1,
                char_id: "char:alice".to_string(),
                fame_delta: 0,
                notoriety_delta: 10,
                tags_added: vec![RenownTagV1 {
                    tag: "戮道者".to_string(),
                    weight: 10.0,
                    last_seen_tick: 120,
                    permanent: false,
                }],
                tick: 120,
                reason: "pk".to_string(),
            }))
            .expect("social renown should serialize");
        match renown {
            RedisIoCommand::Publish { channel, payload } => {
                assert_eq!(channel, CH_SOCIAL_RENOWN_DELTA);
                let v: Value = serde_json::from_str(payload.as_str()).unwrap();
                assert_eq!(v["notoriety_delta"], 10);
                assert_eq!(v["tags_added"][0]["tag"], "戮道者");
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

        let invalid_spawn_npc = r#"{
            "v": 1,
            "id": "cmd_spawn_bad",
            "source": "arbiter",
            "commands": [{
                "type": "spawn_npc",
                "target": "spawn",
                "params": {}
            }]
        }"#;
        assert!(parse_inbound_message(CH_AGENT_COMMAND, invalid_spawn_npc).is_err());
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
