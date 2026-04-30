import {
  INTENSITY_MAX,
  INTENSITY_MIN,
  MAX_COMMANDS_PER_TICK,
  NEWBIE_POWER_THRESHOLD,
} from "@bong/schema";
import type { Command, CommandType, Narration, WorldStateV1 } from "@bong/schema";
import type { AgentDecision } from "./parse.js";
import type { CurrentEra } from "./world-model.js";

const MAX_NARRATIONS_PER_TICK = 10;
const MAX_RECENT_COMBAT_EVENT_AGE_TICKS = 100;
const SPIRIT_QI_CONSERVATION_EPSILON = 0.01;
const GLOBAL_ERA_TARGETS = new Set(["all_zones", "all", "global", "全局"]);
const DUXU_EVENT_RE = /(?:du_?xu|xuhua|void|渡虚|化虚)/iu;

const SOURCE_PRIORITY: Record<string, number> = {
  calamity: 1,
  mutation: 2,
  era: 3,
};

export interface SourcedDecision {
  source: string;
  decision: AgentDecision;
}

export interface MergedResult {
  commands: Command[];
  narrations: Narration[];
  currentEra: CurrentEra | null;
}

interface TaggedCommand {
  source: string;
  command: Command;
  index: number;
  bypassSpiritQiConservation: boolean;
}

interface FlattenedCommands {
  commands: TaggedCommand[];
  currentEra: CurrentEra | null;
}

interface ZoneConflictBucket {
  zone: string;
  firstIndex: number;
  firstCommandType: "spawn_event" | "modify_zone";
  spawnEvents: TaggedCommand[];
  modifyZones: TaggedCommand[];
}

interface OrderedTaggedCommand {
  order: number;
  tagged: TaggedCommand;
}

export class Arbiter {
  constructor(private readonly state: WorldStateV1) {}

  merge(decisions: SourcedDecision[]): MergedResult {
    const flattened = this.flattenCommands(decisions);
    const narrations = decisions
      .flatMap(({ decision }) => decision.narrations)
      .map((narration) => this.applyNarrationScopeRules(narration))
      .filter((narration): narration is Narration => narration !== null)
      .slice(0, MAX_NARRATIONS_PER_TICK);

    const constrained = flattened.commands.filter((tagged) => this.passesHardConstraints(tagged));
    const conservedLocals = this.applySpiritQiConservation(
      this.resolveZoneConflicts(constrained.filter((tagged) => !tagged.bypassSpiritQiConservation)),
    );
    const resolved = this.resolveZoneConflicts([
      ...conservedLocals,
      ...constrained.filter((tagged) => tagged.bypassSpiritQiConservation),
    ]);

    let currentEra = flattened.currentEra;
    if (!currentEra) {
      currentEra = this.detectEraFromNarrations(decisions);
    }

    return {
      commands: resolved.map((tagged) => tagged.command).slice(0, MAX_COMMANDS_PER_TICK),
      narrations,
      currentEra,
    };
  }

  private flattenCommands(decisions: SourcedDecision[]): FlattenedCommands {
    const flattened: TaggedCommand[] = [];

    const pushCommand = (
      source: string,
      command: Command,
      options: { bypassSpiritQiConservation?: boolean } = {},
    ): void => {
      flattened.push({
        source,
        command: cloneCommand(command),
        index: flattened.length,
        bypassSpiritQiConservation: options.bypassSpiritQiConservation ?? false,
      });
    };

    let currentEra: CurrentEra | null = null;

    for (const { source, decision } of decisions) {
      for (const command of decision.commands) {
        const materialized = this.materializeEraCommand(source, command);
        if (materialized) {
          currentEra = materialized.currentEra;
          for (const eraCommand of materialized.commands) {
            pushCommand(source, eraCommand, { bypassSpiritQiConservation: true });
          }
          continue;
        }

        pushCommand(source, command);
      }
    }

    return {
      commands: flattened,
      currentEra,
    };
  }

  private passesHardConstraints(tagged: TaggedCommand): boolean {
    const { command } = tagged;
    if (targetsKnownZone(command) && !this.hasZone(command.target)) {
      return false;
    }

    if (command.type === "modify_zone") {
      return getNumericParam(command.params, "spirit_qi_delta") !== null;
    }

    if (command.type !== "spawn_event") {
      return true;
    }

    const hasIntensity = Object.hasOwn(command.params, "intensity");
    if (hasIntensity) {
      const intensity = getNumericParam(command.params, "intensity");
      if (intensity === null || intensity < INTENSITY_MIN || intensity > INTENSITY_MAX) {
        return false;
      }
    }

    const targetPlayer = getStringParam(command.params, "target_player");
    if (!targetPlayer) {
      return true;
    }

    const playerPower = this.getPlayerPower(targetPlayer);
    if (playerPower === null) {
      return true;
    }

    return playerPower >= NEWBIE_POWER_THRESHOLD;
  }

  private applyNarrationScopeRules(narration: Narration): Narration | null {
    if (this.isCombatTick() && narration.kind !== "death_insight" && narration.style !== "era_decree") {
      return null;
    }

    const sanitized = this.redactPlayerNames(narration);
    if (sanitized.scope === "broadcast") {
      if (!this.isBroadcastAllowed(sanitized)) {
        return this.narrowBroadcastNarration(sanitized);
      }
      return sanitized;
    }

    if (sanitized.scope === "zone" && sanitized.target && !this.hasZone(sanitized.target)) {
      return null;
    }

    if (sanitized.scope === "player" && sanitized.target && !this.hasPlayer(sanitized.target)) {
      return null;
    }

    return sanitized;
  }

  private isBroadcastAllowed(narration: Narration): boolean {
    if (narration.style === "era_decree") {
      return true;
    }

    if (narration.kind === "death_insight") {
      return true;
    }

    return this.hasDuxuSignal(narration);
  }

  private narrowBroadcastNarration(narration: Narration): Narration | null {
    const explicitZoneTarget = narration.target && this.hasZone(narration.target) ? narration.target : null;
    const activeZoneTarget = this.findActiveEventZone();
    const populatedZoneTarget = this.findMostPopulatedZone();
    const target = explicitZoneTarget ?? activeZoneTarget ?? populatedZoneTarget;

    if (!target) {
      return null;
    }

    return {
      ...narration,
      scope: "zone",
      target,
    };
  }

  private hasDuxuSignal(narration: Narration): boolean {
    if (DUXU_EVENT_RE.test(narration.text)) {
      return true;
    }

    return this.state.recent_events.some((event) => {
      const values = [event.type, event.target, event.zone, ...Object.values(event.details ?? {})];
      return values.some((value) => typeof value === "string" && DUXU_EVENT_RE.test(value));
    });
  }

  private redactPlayerNames(narration: Narration): Narration {
    let text = narration.text;
    for (const player of this.state.players) {
      if (player.name.trim().length === 0) {
        continue;
      }
      text = replaceAllLiteral(text, player.name, "某修士");
      text = replaceAllLiteral(text, player.uuid, "某修士");
    }

    if (text === narration.text) {
      return narration;
    }

    return {
      ...narration,
      text,
    };
  }

  private isCombatTick(): boolean {
    return this.state.recent_events.some((event) => {
      if (this.state.tick - event.tick > MAX_RECENT_COMBAT_EVENT_AGE_TICKS) {
        return false;
      }
      return event.type === "player_kill_npc" || event.type === "player_kill_player";
    });
  }

  private materializeEraCommand(
    source: string,
    command: Command,
  ): { commands: Command[]; currentEra: CurrentEra } | null {
    if (source.toLowerCase() !== "era") {
      return null;
    }

    if (command.type !== "modify_zone" || !isGlobalEraTarget(command.target)) {
      return null;
    }

    const spiritQiDelta = getNumericParam(command.params, "spirit_qi_delta");
    if (spiritQiDelta === null) {
      return null;
    }

    const dangerLevelDelta = getNumericParam(command.params, "danger_level_delta") ?? 0;
    const eraName =
      getStringParam(command.params, "era_name") ??
      getStringParam(command.params, "name") ??
      "未名时代";
    const effectDescription =
      getStringParam(command.params, "global_effect") ??
      describeEraEffect(spiritQiDelta, dangerLevelDelta);

    return {
      commands: this.state.zones.map((zone) => ({
        type: "modify_zone",
        target: zone.name,
        params: buildModifyZoneParams(spiritQiDelta, dangerLevelDelta),
      })),
      currentEra: {
        name: eraName,
        sinceTick: this.state.tick,
        globalEffect: effectDescription,
      },
    };
  }

  private detectEraFromNarrations(decisions: SourcedDecision[]): CurrentEra | null {
    for (const { source, decision } of decisions) {
      if (source.toLowerCase() !== "era") continue;
      for (const narration of decision.narrations) {
        if (narration.style !== "era_decree") continue;
        const eraName = extractEraName(narration.text);
        if (eraName) {
          return {
            name: eraName,
            sinceTick: this.state.tick,
            globalEffect: narration.text,
          };
        }
      }
    }
    return null;
  }

  private resolveZoneConflicts(commands: TaggedCommand[]): TaggedCommand[] {
    const zoneBuckets = new Map<string, ZoneConflictBucket>();
    const passthrough: OrderedTaggedCommand[] = [];

    for (const tagged of commands) {
      const { command } = tagged;
      if (!isZoneConflictCommand(command)) {
        passthrough.push({ order: tagged.index, tagged });
        continue;
      }

      const existing = zoneBuckets.get(command.target);
      if (!existing) {
        zoneBuckets.set(command.target, {
          zone: command.target,
          firstIndex: tagged.index,
          firstCommandType: command.type,
          spawnEvents: command.type === "spawn_event" ? [tagged] : [],
          modifyZones: command.type === "modify_zone" ? [tagged] : [],
        });
        continue;
      }

      if (command.type === "spawn_event") {
        existing.spawnEvents.push(tagged);
      } else {
        existing.modifyZones.push(tagged);
      }
    }

    const resolvedZoneCommands: OrderedTaggedCommand[] = [];
    for (const bucket of zoneBuckets.values()) {
      const mergedModify = this.mergeModifyZoneCommands(bucket);
      const selectedSpawn = this.selectHighestPrioritySpawn(bucket.spawnEvents);

      const orderedZoneCommands =
        bucket.firstCommandType === "spawn_event"
          ? [selectedSpawn, mergedModify]
          : [mergedModify, selectedSpawn];

      let orderOffset = 0;
      for (const command of orderedZoneCommands) {
        if (!command) {
          continue;
        }
        resolvedZoneCommands.push({
          order: bucket.firstIndex + orderOffset,
          tagged: command,
        });
        orderOffset += 0.001;
      }
    }

    return [...passthrough, ...resolvedZoneCommands]
      .sort((a, b) => a.order - b.order)
      .map((entry) => entry.tagged);
  }

  private mergeModifyZoneCommands(bucket: ZoneConflictBucket): TaggedCommand | null {
    if (bucket.modifyZones.length === 0) {
      return null;
    }

    let spiritQiDelta = 0;
    let dangerLevelDelta = 0;
    let hasDangerLevelDelta = false;

    for (const tagged of bucket.modifyZones) {
      const delta = getNumericParam(tagged.command.params, "spirit_qi_delta");
      if (delta !== null) {
        spiritQiDelta += delta;
      }

      const dangerDelta = getNumericParam(tagged.command.params, "danger_level_delta");
      if (dangerDelta !== null) {
        dangerLevelDelta += dangerDelta;
        hasDangerLevelDelta = true;
      }
    }

    const params: Record<string, unknown> = {
      spirit_qi_delta: spiritQiDelta,
    };

    if (hasDangerLevelDelta) {
      params["danger_level_delta"] = dangerLevelDelta;
    }

    const primary = bucket.modifyZones[0];
    return {
      source: primary.source,
      index: primary.index,
      bypassSpiritQiConservation: bucket.modifyZones.some(
        (tagged) => tagged.bypassSpiritQiConservation,
      ),
      command: {
        type: "modify_zone",
        target: bucket.zone,
        params,
      },
    };
  }

  private selectHighestPrioritySpawn(spawnEvents: TaggedCommand[]): TaggedCommand | null {
    if (spawnEvents.length === 0) {
      return null;
    }

    let selected = spawnEvents[0];
    for (let i = 1; i < spawnEvents.length; i++) {
      const current = spawnEvents[i];
      const currentPriority = sourcePriority(current.source);
      const selectedPriority = sourcePriority(selected.source);
      if (currentPriority > selectedPriority) {
        selected = current;
      }
    }

    return selected;
  }

  private applySpiritQiConservation(commands: TaggedCommand[]): TaggedCommand[] {
    const clones = commands.map((tagged) => ({
      ...tagged,
      command: cloneCommand(tagged.command),
    }));

    const deltas: number[] = [];
    const indexes: number[] = [];

    for (let i = 0; i < clones.length; i++) {
      const tagged = clones[i];
      const command = tagged.command;
      if (command.type !== "modify_zone" || tagged.bypassSpiritQiConservation) {
        continue;
      }

      const delta = getNumericParam(command.params, "spirit_qi_delta");
      if (delta === null) {
        continue;
      }

      indexes.push(i);
      deltas.push(delta);
    }

    const netDelta = deltas.reduce((acc, delta) => acc + delta, 0);
    if (Math.abs(netDelta) <= SPIRIT_QI_CONSERVATION_EPSILON) {
      return clones;
    }

    const positiveSum = deltas.filter((delta) => delta > 0).reduce((acc, delta) => acc + delta, 0);
    const negativeAbsSum = deltas
      .filter((delta) => delta < 0)
      .reduce((acc, delta) => acc + Math.abs(delta), 0);

    if (netDelta > 0) {
      if (positiveSum === 0) {
        return clones;
      }

      const positiveScale = negativeAbsSum === 0 ? 0 : negativeAbsSum / positiveSum;
      for (let i = 0; i < indexes.length; i++) {
        if (deltas[i] <= 0) {
          continue;
        }

        const commandIndex = indexes[i];
        const command = clones[commandIndex].command;
        const current = getNumericParam(command.params, "spirit_qi_delta");
        if (current === null) {
          continue;
        }

        command.params["spirit_qi_delta"] = current * positiveScale;
      }

      return clones;
    }

    if (negativeAbsSum === 0) {
      return clones;
    }

    const negativeScale = positiveSum === 0 ? 0 : positiveSum / negativeAbsSum;
    for (let i = 0; i < indexes.length; i++) {
      if (deltas[i] >= 0) {
        continue;
      }

      const commandIndex = indexes[i];
      const command = clones[commandIndex].command;
      const current = getNumericParam(command.params, "spirit_qi_delta");
      if (current === null) {
        continue;
      }

      command.params["spirit_qi_delta"] = current * negativeScale;
    }

    return clones;
  }

  private hasZone(zoneName: string): boolean {
    return this.state.zones.some((zone) => zone.name === zoneName);
  }

  private hasPlayer(playerId: string): boolean {
    return this.state.players.some((player) => player.uuid === playerId || player.name === playerId);
  }

  private findActiveEventZone(): string | null {
    return this.state.zones.find((zone) => zone.active_events.length > 0)?.name ?? null;
  }

  private findMostPopulatedZone(): string | null {
    const zones = [...this.state.zones].sort((left, right) => right.player_count - left.player_count);
    return zones[0]?.name ?? null;
  }

  private getPlayerPower(targetPlayer: string): number | null {
    const byUuid = this.state.players.find((player) => player.uuid === targetPlayer);
    if (byUuid) {
      return byUuid.composite_power;
    }

    const normalizedName = targetPlayer.startsWith("offline:")
      ? targetPlayer.slice("offline:".length)
      : targetPlayer;
    const byName = this.state.players.find((player) => player.name === normalizedName);
    if (byName) {
      return byName.composite_power;
    }

    return null;
  }
}

function targetsKnownZone(
  command: Command,
): command is Command & { type: Extract<CommandType, "spawn_event" | "spawn_npc" | "modify_zone"> } {
  return ["spawn_event", "spawn_npc", "modify_zone"].includes(command.type);
}

function isZoneConflictCommand(command: Command): command is Command & { type: "spawn_event" | "modify_zone" } {
  return command.type === "spawn_event" || command.type === "modify_zone";
}

function sourcePriority(source: string): number {
  return SOURCE_PRIORITY[source.toLowerCase()] ?? 0;
}

function getNumericParam(params: Record<string, unknown>, key: string): number | null {
  const value = params[key];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return null;
  }
  return value;
}

function getStringParam(params: Record<string, unknown>, key: string): string | null {
  const value = params[key];
  if (typeof value !== "string") {
    return null;
  }
  return value;
}

function isGlobalEraTarget(target: string): boolean {
  return GLOBAL_ERA_TARGETS.has(target.trim().toLowerCase());
}

function replaceAllLiteral(text: string, needle: string, replacement: string): string {
  return text.split(needle).join(replacement);
}

function buildModifyZoneParams(
  spiritQiDelta: number,
  dangerLevelDelta: number,
): Record<string, unknown> {
  const params: Record<string, unknown> = {
    spirit_qi_delta: spiritQiDelta,
  };

  if (dangerLevelDelta !== 0) {
    params["danger_level_delta"] = dangerLevelDelta;
  }

  return params;
}

function describeEraEffect(spiritQiDelta: number, dangerLevelDelta: number): string {
  const parts: string[] = [];

  if (spiritQiDelta !== 0) {
    parts.push(`诸域灵气 ${formatSigned(spiritQiDelta)}`);
  }

  if (dangerLevelDelta !== 0) {
    parts.push(`诸域危险 ${formatSigned(dangerLevelDelta)}`);
  }

  return parts.length > 0 ? parts.join("，") : "诸域法则微调";
}

function formatSigned(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}`;
}

function extractEraName(text: string): string | null {
  const match = text.match(/([\u4e00-\u9fff]{1,6}(?:纪|时代|劫|世))/);
  return match?.[1] ?? null;
}

function cloneCommand(command: Command): Command {
  return {
    type: command.type,
    target: command.target,
    params: { ...command.params },
  };
}
