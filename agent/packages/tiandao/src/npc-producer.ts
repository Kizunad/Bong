import { MAX_COMMANDS_PER_TICK, validateAgentCommandV1Contract } from "@bong/schema";
import type { Command, FactionIdV1, WorldStateV1, ZoneSnapshot } from "@bong/schema";
import type { SourcedDecision } from "./arbiter.js";
import type { AgentDecision } from "./parse.js";
import type { TickPublishMetadata } from "./runtime.js";
import type { WorldModel } from "./world-model.js";

const NPC_PRODUCER_SOURCE = "npc_producer";
const ROGUE_SPAWN_MIN_SPIRIT_QI = 0.72;
const ROGUE_SPAWN_MIN_TREND_DELTA = 0.05;
const ROGUE_SPAWN_MIN_TICK_GAP = 600;
const ROGUE_SPAWN_MAX_PER_ZONE = 12;
const TRIBULATION_INTERCEPT_MIN_KARMA = 0.25;
const TRIBULATION_INTERCEPT_MIN_DISCIPLE_LOYALTY = 0.55;
const FACTION_EVENT_MIN_TICK_GAP = 1_200;

export interface DeterministicNpcProducerInput {
  state: WorldStateV1;
  worldModel?: WorldModel;
  sourcedDecisions: readonly SourcedDecision[];
  metadata: TickPublishMetadata;
}

export type DeterministicNpcProducer = (
  input: DeterministicNpcProducerInput,
) => SourcedDecision[];

export function produceDeterministicNpcDecisions(
  input: DeterministicNpcProducerInput,
): SourcedDecision[] {
  const decisions = [
    produceSpiritQiRogueSpawns(input),
    produceFactionEraEvents(input),
    produceTribulationInterceptionIntents(input),
  ].filter((decision): decision is AgentDecision => decision !== null);

  return decisions.map((decision) => ({
    source: NPC_PRODUCER_SOURCE,
    decision: validateProducedDecision(decision),
  }));
}

function produceSpiritQiRogueSpawns(
  input: DeterministicNpcProducerInput,
): AgentDecision | null {
  const { state, worldModel } = input;
  const zone = selectRogueSpawnZone(state, worldModel);
  if (!zone) {
    return null;
  }

  if (state.tick % ROGUE_SPAWN_MIN_TICK_GAP !== 0) {
    return null;
  }

  const rogueCount = state.npcs.filter(
    (npc) => npc.digest?.archetype === "rogue" && npc.zone === zone.name,
  ).length;
  if (rogueCount >= ROGUE_SPAWN_MAX_PER_ZONE) {
    return null;
  }

  const availableSlots = ROGUE_SPAWN_MAX_PER_ZONE - rogueCount;
  const count = Math.min(
    MAX_COMMANDS_PER_TICK,
    availableSlots,
    Math.max(1, Math.ceil((zone.spirit_qi - 0.68) * 10)),
  );
  return decision([
    {
      type: "spawn_npc",
      target: zone.name,
      params: {
        archetype: "rogue",
        count,
        reason: "spirit_qi_fluctuation",
      },
    },
  ], `灵脉波动在 ${zone.name} 聚集，投放 ${count} 名散修`);
}

function produceFactionEraEvents(
  input: DeterministicNpcProducerInput,
): AgentDecision | null {
  const { state, sourcedDecisions } = input;
  if (!state.factions || state.factions.length === 0) {
    return null;
  }
  if (state.tick % FACTION_EVENT_MIN_TICK_GAP !== 0) {
    return null;
  }

  const eraActed = sourcedDecisions.some(
    ({ source, decision }) =>
      source.toLowerCase() === "era" &&
      (decision.narrations.some((narration) => narration.style === "era_decree") ||
        decision.commands.some((command) => command.type === "modify_zone" && isGlobalTarget(command.target))),
  );
  if (!eraActed) {
    return null;
  }

  const weakest = [...state.factions].sort(
    (a, b) =>
      (a.mission_queue?.pending_count ?? 0) - (b.mission_queue?.pending_count ?? 0) ||
      a.loyalty_bias - b.loyalty_bias,
  )[0];
  if (!weakest) {
    return null;
  }

  return decision([
    {
      type: "faction_event",
      target: weakest.id,
      params: {
        kind: "adjust_loyalty_bias",
        faction_id: weakest.id,
        loyalty_delta: 0.05,
      },
    },
  ], `演绎时代推动 ${weakest.id} 派系微调忠诚偏置`);
}

function produceTribulationInterceptionIntents(
  input: DeterministicNpcProducerInput,
): AgentDecision | null {
  const { state } = input;
  const target = state.players.find((player) => {
    if (player.breakdown.karma < TRIBULATION_INTERCEPT_MIN_KARMA) {
      return false;
    }
    return hasTribulationSignal(state, player.uuid, player.name, player.zone);
  });
  if (!target) {
    return null;
  }

  const factionId = selectInterceptionFaction(state);
  if (!factionId) {
    return null;
  }

  const missionId = `mission:intercept_duxu:${state.tick}:${safeMissionToken(target.uuid)}`;
  return decision([
    {
      type: "faction_event",
      target: factionId,
      params: {
        kind: "enqueue_mission",
        faction_id: factionId,
        mission_id: missionId,
        subject_id: target.uuid,
      },
    },
  ], `玩家 ${target.name} 渡虚劫触发敌对弟子截胡任务`);
}

function selectRogueSpawnZone(state: WorldStateV1, worldModel?: WorldModel): ZoneSnapshot | null {
  const candidates = state.zones
    .map((zone) => ({
      zone,
      trend: worldModel?.getZoneTrendSummary(zone.name) ?? null,
    }))
    .filter(({ zone, trend }) => {
      if (zone.spirit_qi < ROGUE_SPAWN_MIN_SPIRIT_QI) {
        return false;
      }
      if (!trend) {
        return true;
      }
      return trend.delta >= ROGUE_SPAWN_MIN_TREND_DELTA;
    })
    .sort((a, b) => {
      const delta = (b.trend?.delta ?? 0) - (a.trend?.delta ?? 0);
      if (Math.abs(delta) > 1e-9) {
        return delta;
      }
      return b.zone.spirit_qi - a.zone.spirit_qi;
    });

  return candidates[0]?.zone ?? null;
}

function hasTribulationSignal(
  state: WorldStateV1,
  playerUuid: string,
  playerName: string,
  zoneName: string,
): boolean {
  const zone = state.zones.find((candidate) => candidate.name === zoneName);
  if (zone?.active_events.some(isTribulationEventName)) {
    return true;
  }

  return state.recent_events.some((event) => {
    if (event.zone && event.zone !== zoneName) {
      return false;
    }
    if (event.player && event.player !== playerUuid && event.player !== playerName) {
      return false;
    }
    const detailsKind = typeof event.details?.kind === "string" ? event.details.kind : "";
    const detailsEvent = typeof event.details?.event === "string" ? event.details.event : "";
    return (
      isTribulationEventName(event.type) ||
      isTribulationEventName(detailsKind) ||
      isTribulationEventName(detailsEvent)
    );
  });
}

function selectInterceptionFaction(state: WorldStateV1): FactionIdV1 | null {
  const eligible = state.npcs
    .map((npc) => npc.digest?.disciple)
    .filter((disciple): disciple is NonNullable<typeof disciple> => Boolean(disciple))
    .filter((disciple) =>
      (disciple.faction_id === "attack" || disciple.faction_id === "defend") &&
      disciple.loyalty >= TRIBULATION_INTERCEPT_MIN_DISCIPLE_LOYALTY,
    )
    .sort((a, b) => b.loyalty - a.loyalty);

  return eligible[0]?.faction_id ?? null;
}

function validateProducedDecision(decision: AgentDecision): AgentDecision {
  for (const command of decision.commands) {
    const result = validateAgentCommandV1Contract({
      v: 1,
      id: "cmd_npc_producer_candidate",
      source: "arbiter",
      commands: [command],
    });
    if (!result.ok) {
      throw new Error(`[tiandao][npc-producer] invalid produced command: ${result.errors.join("; ")}`);
    }
  }
  return decision;
}

function decision(commands: Command[], reasoning: string): AgentDecision {
  return {
    commands,
    narrations: [],
    reasoning,
  };
}

function isTribulationEventName(value: string): boolean {
  return value.includes("tribulation") || value.includes("duxu") || value.includes("du_xu");
}

function isGlobalTarget(target: string): boolean {
  return ["all_zones", "all", "global", "全局"].includes(target.trim().toLowerCase());
}

function safeMissionToken(value: string): string {
  return value.replace(/[^a-zA-Z0-9:_-]/g, "_");
}
