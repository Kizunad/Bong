import {
  INTENSITY_MAX,
  INTENSITY_MIN,
  MAX_COMMANDS_PER_TICK,
  NEWBIE_POWER_THRESHOLD,
  type Command,
  type Narration,
  type WorldStateV1,
} from "@bong/schema";
import type { AgentDecision } from "./parse.js";

const SOURCE_PRIORITY: Record<string, number> = {
  calamity: 0,
  mutation: 1,
  era: 2,
};

export const MAX_NARRATIONS_PER_TICK = 10;
export const NEWBIE_HIGH_INTENSITY_THRESHOLD = 0.5;
const SPIRIT_QI_REBALANCE_THRESHOLD = 0.01;
const FLOAT_EPSILON = 1e-9;

type PrivateCommand = Command & {
  _source?: unknown;
  source?: unknown;
};

interface IndexedCommand {
  index: number;
  command: PrivateCommand;
}

interface ZoneSpawnCandidate {
  index: number;
  command: PrivateCommand;
}

interface ZoneModifyAccumulator {
  index: number;
  target: string;
  spirit_qi_delta: number;
  hasSpiritQiDelta: boolean;
  danger_level_delta: number;
  hasDangerLevelDelta: boolean;
}

export interface MergedResult {
  commands: Command[];
  narrations: Narration[];
}

function getSourcePriority(command: PrivateCommand): number {
  const source = command._source;
  if (typeof source !== "string") return -1;
  return SOURCE_PRIORITY[source] ?? -1;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function getCommandParams(command: PrivateCommand): Record<string, unknown> {
  return command.params && typeof command.params === "object"
    ? (command.params as Record<string, unknown>)
    : {};
}

function getSpiritQiDelta(command: PrivateCommand): number {
  const params = getCommandParams(command);
  const delta = params.spirit_qi_delta;
  return isFiniteNumber(delta) ? delta : 0;
}

function getNewbieZones(state: WorldStateV1): Set<string> {
  const newbieZones = new Set<string>();
  for (const player of state.players) {
    if (player.composite_power < NEWBIE_POWER_THRESHOLD) {
      newbieZones.add(player.zone);
    }
  }
  return newbieZones;
}

function getPlayerPowerLookup(state: WorldStateV1): Map<string, number> {
  const lookup = new Map<string, number>();
  for (const player of state.players) {
    lookup.set(player.uuid, player.composite_power);
    lookup.set(player.name, player.composite_power);
  }
  return lookup;
}

function isHighIntensityNewbieSpawnBlocked(
  command: PrivateCommand,
  newbieZones: Set<string>,
  playerPowerLookup: Map<string, number>,
): boolean {
  if (command.type !== "spawn_event") return false;

  const params = getCommandParams(command);
  const intensity = params.intensity;
  if (!isFiniteNumber(intensity)) return true;

  if (intensity <= NEWBIE_HIGH_INTENSITY_THRESHOLD) return false;

  const targetPlayer = params.target_player;
  if (typeof targetPlayer === "string") {
    const targetPower = playerPowerLookup.get(targetPlayer);
    if (targetPower !== undefined && targetPower < NEWBIE_POWER_THRESHOLD) {
      return true;
    }
  }

  return newbieZones.has(command.target);
}

function isLegalZoneCommand(command: PrivateCommand, zones: Set<string>): boolean {
  if (command.type !== "spawn_event" && command.type !== "modify_zone") {
    return true;
  }
  return zones.has(command.target);
}

function isLegalSpawnCommand(command: PrivateCommand): boolean {
  if (command.type !== "spawn_event") return true;
  const params = getCommandParams(command);
  const intensity = params.intensity;
  if (!isFiniteNumber(intensity)) return false;
  return intensity >= INTENSITY_MIN && intensity <= INTENSITY_MAX;
}

function cloneAsPublicCommand(command: PrivateCommand): Command {
  const params = getCommandParams(command);
  const { _source: _ignoredPrivateSource, ...publicParams } = params;
  return {
    type: command.type,
    target: command.target,
    params: { ...publicParams },
  };
}

function toZoneModifyCommand(item: ZoneModifyAccumulator): PrivateCommand {
  const params: Record<string, unknown> = {};
  if (item.hasSpiritQiDelta) {
    params.spirit_qi_delta = item.spirit_qi_delta;
  }
  if (item.hasDangerLevelDelta) {
    params.danger_level_delta = item.danger_level_delta;
  }
  return {
    type: "modify_zone",
    target: item.target,
    params,
  };
}

function scaleSpiritQiTowardZero(indexedCommands: IndexedCommand[]): void {
  const modifyCommands = indexedCommands.filter((item) => item.command.type === "modify_zone");
  if (modifyCommands.length === 0) return;

  let net = 0;
  let positiveTotal = 0;
  let negativeMagnitude = 0;

  for (const item of modifyCommands) {
    const delta = getSpiritQiDelta(item.command);
    net += delta;
    if (delta > 0) positiveTotal += delta;
    if (delta < 0) negativeMagnitude += Math.abs(delta);
  }

  if (Math.abs(net) <= SPIRIT_QI_REBALANCE_THRESHOLD + FLOAT_EPSILON) return;

  if (net > 0 && positiveTotal > 0) {
    const targetPositive = Math.max(0, positiveTotal - net);
    const factor = targetPositive / positiveTotal;
    for (const item of modifyCommands) {
      const params = getCommandParams(item.command);
      const delta = params.spirit_qi_delta;
      if (isFiniteNumber(delta) && delta > 0) {
        params.spirit_qi_delta = delta * factor;
      }
    }
    return;
  }

  if (net < 0 && negativeMagnitude > 0) {
    const targetNegativeMagnitude = Math.max(0, negativeMagnitude - Math.abs(net));
    const factor = targetNegativeMagnitude / negativeMagnitude;
    for (const item of modifyCommands) {
      const params = getCommandParams(item.command);
      const delta = params.spirit_qi_delta;
      if (isFiniteNumber(delta) && delta < 0) {
        params.spirit_qi_delta = delta * factor;
      }
    }
  }
}

export class Arbiter {
  merge(decisions: AgentDecision[], state: WorldStateV1): MergedResult {
    const allNarrations = decisions.flatMap((decision) => decision.narrations);
    const narrations = allNarrations.slice(0, MAX_NARRATIONS_PER_TICK);

    const knownZones = new Set(state.zones.map((zone) => zone.name));
    const newbieZones = getNewbieZones(state);
    const playerPowerLookup = getPlayerPowerLookup(state);

    const legalCommands: IndexedCommand[] = [];
    let index = 0;

    for (const decision of decisions) {
      for (const command of decision.commands as PrivateCommand[]) {
        if (!isLegalZoneCommand(command, knownZones)) {
          index += 1;
          continue;
        }
        if (!isLegalSpawnCommand(command)) {
          index += 1;
          continue;
        }
        if (isHighIntensityNewbieSpawnBlocked(command, newbieZones, playerPowerLookup)) {
          index += 1;
          continue;
        }
        legalCommands.push({ index, command });
        index += 1;
      }
    }

    const spawnByZone = new Map<string, ZoneSpawnCandidate>();
    const modifyByZone = new Map<string, ZoneModifyAccumulator>();
    const passthrough: IndexedCommand[] = [];

    for (const item of legalCommands) {
      const { command } = item;

      if (command.type === "spawn_event") {
        const existing = spawnByZone.get(command.target);
        if (!existing) {
          spawnByZone.set(command.target, { ...item });
          continue;
        }

        const currentPriority = getSourcePriority(command);
        const existingPriority = getSourcePriority(existing.command);
        if (currentPriority > existingPriority) {
          spawnByZone.set(command.target, { ...item });
        }
        continue;
      }

      if (command.type === "modify_zone") {
        const params = getCommandParams(command);
        const acc =
          modifyByZone.get(command.target) ??
          {
            index: item.index,
            target: command.target,
            spirit_qi_delta: 0,
            hasSpiritQiDelta: false,
            danger_level_delta: 0,
            hasDangerLevelDelta: false,
          };

        const spiritDelta = params.spirit_qi_delta;
        if (isFiniteNumber(spiritDelta)) {
          acc.spirit_qi_delta += spiritDelta;
          acc.hasSpiritQiDelta = true;
        }

        const dangerDelta = params.danger_level_delta;
        if (isFiniteNumber(dangerDelta)) {
          acc.danger_level_delta += dangerDelta;
          acc.hasDangerLevelDelta = true;
        }

        modifyByZone.set(command.target, acc);
        continue;
      }

      passthrough.push(item);
    }

    const merged: IndexedCommand[] = [];
    for (const candidate of spawnByZone.values()) {
      merged.push(candidate);
    }
    for (const acc of modifyByZone.values()) {
      if (!acc.hasSpiritQiDelta && !acc.hasDangerLevelDelta) {
        continue;
      }
      merged.push({
        index: acc.index,
        command: toZoneModifyCommand(acc),
      });
    }
    for (const item of passthrough) {
      merged.push(item);
    }

    merged.sort((a, b) => a.index - b.index);
    scaleSpiritQiTowardZero(merged);

    const commands = merged
      .slice(0, MAX_COMMANDS_PER_TICK)
      .map((item) => cloneAsPublicCommand(item.command));

    return {
      commands,
      narrations,
    };
  }
}
