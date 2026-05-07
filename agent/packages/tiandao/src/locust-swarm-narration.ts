import {
  validateRatPhaseChangeEventV1Contract,
  type Command,
  type Narration,
  type RatPhaseChangeEventV1,
  type RatPhaseV1,
  type WorldStateV1,
  type ZoneSnapshot,
} from "@bong/schema";

export const LOCUST_SWARM_COOLDOWN_TICKS = 24 * 3_600 * 20;
const HIGH_QI_THRESHOLD = 0.6;
const RAT_DENSITY_THRESHOLD = 8;
const ACTIVE_CALAMITIES = new Set([
  "thunder_tribulation",
  "realm_collapse",
  "beast_tide",
  "locust_swarm",
]);

export interface LocustSwarmDecision {
  commands: Command[];
  narrations: Narration[];
  reasoning: string;
}

export function parseRatPhaseEventFromRedis(message: string): RatPhaseChangeEventV1 | null {
  try {
    const data = JSON.parse(message) as unknown;
    const result = validateRatPhaseChangeEventV1Contract(data);
    return result.ok ? (data as RatPhaseChangeEventV1) : null;
  } catch {
    return null;
  }
}

export class LocustSwarmNarrationTracker {
  private readonly lastSwarmByTargetZone = new Map<string, number>();

  ingest(event: RatPhaseChangeEventV1, state: WorldStateV1): LocustSwarmDecision {
    if (!isTransitioning(event.to)) {
      return emptyDecision("rat phase event is not a transition trigger");
    }

    if (hasActiveCalamity(state)) {
      return emptyDecision("another calamity is already active");
    }

    const targetZone = selectTargetZone(state, event.zone);
    if (!targetZone) {
      return emptyDecision("no zone exceeds locust swarm qi threshold");
    }

    if (!hasActivePlayersNearPhaseZone(state, event.zone, targetZone.name)) {
      return emptyDecision("no active player pressure near phase or target zone");
    }

    if (!isRatDensityHigh(event, state)) {
      return emptyDecision("rat density is below locust swarm threshold");
    }

    const tick = Math.max(event.tick, state.tick);
    const lastTick = this.lastSwarmByTargetZone.get(targetZone.name);
    if (lastTick !== undefined && tick - lastTick < LOCUST_SWARM_COOLDOWN_TICKS) {
      return emptyDecision("locust swarm cooldown is still active for target zone");
    }

    this.lastSwarmByTargetZone.set(targetZone.name, tick);
    const intensity = clamp(
      0.45 + targetZone.spirit_qi * 0.35 + Math.min(event.rat_count, 24) / 120,
      0.1,
      1,
    );

    return {
      commands: [
        {
          type: "spawn_event",
          target: event.zone,
          params: {
            event: "beast_tide",
            tide_kind: "locust_swarm",
            origin_zone: event.zone,
            target_zone: targetZone.name,
            intensity,
            duration_ticks: 24_000,
          },
        },
      ],
      narrations: [
        {
          scope: "zone",
          target: targetZone.name,
          style: "system_warning",
          text: `鼠群在${event.zone}聚相，灵压已向${targetZone.name}回卷。地底沙声渐密，灵蝗潮将循高气处逼近，近处修士须立刻退离灵源。`,
        },
      ],
      reasoning: `rat transition in ${event.zone}, target=${targetZone.name}, qi=${targetZone.spirit_qi}, rats=${event.rat_count}`,
    };
  }
}

function emptyDecision(reasoning: string): LocustSwarmDecision {
  return {
    commands: [],
    narrations: [],
    reasoning,
  };
}

function isTransitioning(phase: RatPhaseV1): boolean {
  return typeof phase === "object" && phase !== null && "transitioning" in phase;
}

function selectTargetZone(state: WorldStateV1, phaseZone: string): ZoneSnapshot | null {
  return state.zones
    .filter((zone) => zone.name !== phaseZone && zone.spirit_qi > HIGH_QI_THRESHOLD)
    .sort((left, right) => right.spirit_qi - left.spirit_qi)[0] ?? null;
}

function hasActivePlayersNearPhaseZone(
  state: WorldStateV1,
  phaseZone: string,
  targetZone: string,
): boolean {
  const zones = new Set([phaseZone, targetZone]);
  if (state.zones.some((zone) => zones.has(zone.name) && zone.player_count > 0)) {
    return true;
  }

  return state.players.some((player) => zones.has(player.zone) && player.active_hours > 0);
}

function isRatDensityHigh(event: RatPhaseChangeEventV1, state: WorldStateV1): boolean {
  const zoneDensity = state.rat_density_heatmap.zones[event.zone];
  const stateRatTotal = zoneDensity?.total ?? 0;
  return Math.max(event.rat_count, stateRatTotal) >= RAT_DENSITY_THRESHOLD;
}

function hasActiveCalamity(state: WorldStateV1): boolean {
  return state.zones.some((zone) =>
    zone.active_events.some((eventName) => ACTIVE_CALAMITIES.has(eventName)),
  );
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
