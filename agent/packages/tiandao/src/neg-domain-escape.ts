import type { Command, Narration, PlayerProfile, WorldStateV1, ZoneSnapshot } from "@bong/schema";
import type { WorldModel } from "./world-model.js";

const TRIBULATION_EVENT = "thunder_tribulation";
const NEG_DOMAIN_QI_THRESHOLD = 0;
const SPIRIT_REALM = "Spirit";
const NEG_DOMAIN_DROWNING_WINDOW_TICKS = 5;
const NEG_DOMAIN_DROWNING_DROP_RATIO = 0.25;
const NEG_DOMAIN_DROWNING_REALM_GAP = 2;
const REALM_RANK: Record<string, number> = {
  Awaken: 0,
  Induce: 1,
  Condense: 2,
  Solidify: 3,
  Spirit: 4,
  Void: 5,
};

export interface NegDomainEscapeGateResult {
  commands: Command[];
  narrations: Narration[];
}

export function applyNegDomainTribulationGate(args: {
  commands: Command[];
  state: WorldStateV1;
  worldModel?: WorldModel;
}): NegDomainEscapeGateResult {
  const commands: Command[] = [];
  const narrations: Narration[] = [];

  for (const command of args.commands) {
    const targetPlayerId = getTargetedTribulationPlayer(command);
    if (targetPlayerId === null) {
      commands.push(command);
      continue;
    }

    const player = findPlayer(args.state, targetPlayerId);
    if (!player) {
      commands.push(command);
      continue;
    }

    const zone = args.state.zones.find((candidate) => candidate.name === player.zone);
    if (!isSpiritRealm(player)) {
      commands.push(command);
      clearPendingIfSafe(args.worldModel, player.uuid);
      continue;
    }

    if (!zone || !isNegativeDomain(zone)) {
      commands.push(command);
      if (args.worldModel?.consumeNegDomainPendingTribulation(player.uuid)) {
        narrations.push(buildRelockedNarration(player));
      }
      continue;
    }

    args.worldModel?.recordNegDomainPendingTribulation({
      playerUuid: player.uuid,
      playerName: player.name,
      zone: zone.name,
      tick: args.state.tick,
      reason: "negative_domain_tribulation_exempt",
    });
    narrations.push(buildLostLockNarration(player));
  }

  return { commands, narrations };
}

export function renderNegDomainNarrations(args: {
  previousState: WorldStateV1 | null;
  state: WorldStateV1;
}): Narration[] {
  const previousState = args.previousState;
  if (previousState === null) {
    return [];
  }

  const narrations: Narration[] = [];
  const previousPlayers = new Map(previousState.players.map((player) => [player.uuid, player]));

  for (const player of args.state.players) {
    const previousPlayer = previousPlayers.get(player.uuid);
    if (!previousPlayer) {
      continue;
    }

    const currentZone = findZone(args.state, player.zone);
    const previousZone = findZone(previousState, previousPlayer.zone);
    const wasInNegativeDomain = previousZone ? isNegativeDomain(previousZone) : false;
    const isInNegativeDomain = currentZone ? isNegativeDomain(currentZone) : false;

    if (isSpiritRealm(player) && !wasInNegativeDomain && isInNegativeDomain) {
      narrations.push(buildLostLockNarration(player));
    }

    if (isSpiritRealm(player) && wasInNegativeDomain && !isInNegativeDomain) {
      narrations.push(buildRelockedNarration(player));
    }

    const tacticalNarrations = renderDrowningTacticNarrations({
      previousState,
      previousPlayer,
      state: args.state,
      player,
      isInNegativeDomain,
    });
    narrations.push(...tacticalNarrations);
  }

  return narrations;
}

function getTargetedTribulationPlayer(command: Command): string | null {
  if (command.type !== "spawn_event") {
    return null;
  }

  if (command.params.event !== TRIBULATION_EVENT) {
    return null;
  }

  const targetPlayer = command.params.target_player;
  return typeof targetPlayer === "string" && targetPlayer.trim().length > 0 ? targetPlayer : null;
}

function findPlayer(state: WorldStateV1, targetPlayerId: string): PlayerProfile | null {
  const normalized = targetPlayerId.startsWith("offline:")
    ? targetPlayerId.slice("offline:".length)
    : targetPlayerId;
  return (
    state.players.find(
      (player) =>
        player.uuid === targetPlayerId ||
        player.uuid === normalized ||
        player.name === targetPlayerId ||
        player.name === normalized,
    ) ?? null
  );
}

function findZone(state: WorldStateV1, zoneName: string): ZoneSnapshot | null {
  return state.zones.find((zone) => zone.name === zoneName) ?? null;
}

function isSpiritRealm(player: PlayerProfile): boolean {
  return player.realm === SPIRIT_REALM;
}

function isNegativeDomain(zone: ZoneSnapshot): boolean {
  return zone.spirit_qi < NEG_DOMAIN_QI_THRESHOLD;
}

function renderDrowningTacticNarrations(args: {
  previousState: WorldStateV1;
  previousPlayer: PlayerProfile;
  state: WorldStateV1;
  player: PlayerProfile;
  isInNegativeDomain: boolean;
}): Narration[] {
  if (!args.isInNegativeDomain) {
    return [];
  }

  const previousCultivation = args.previousPlayer.cultivation;
  const currentCultivation = args.player.cultivation;
  if (!previousCultivation || !currentCultivation || currentCultivation.qi_max <= 0) {
    return [];
  }

  const tickDelta = args.state.tick - args.previousState.tick;
  if (tickDelta < 0 || tickDelta > NEG_DOMAIN_DROWNING_WINDOW_TICKS) {
    return [];
  }

  const qiDrop = previousCultivation.qi_current - currentCultivation.qi_current;
  if (qiDrop < currentCultivation.qi_max * NEG_DOMAIN_DROWNING_DROP_RATIO) {
    return [];
  }

  const baiter = findBaiter(args.state, args.player);
  if (!baiter) {
    return [];
  }

  return [buildDrowningBaiterHint(baiter), buildDrowningPublicNarration(args.player, baiter)];
}

function findBaiter(state: WorldStateV1, target: PlayerProfile): PlayerProfile | null {
  const targetRank = realmRank(target.realm);
  if (targetRank === null) {
    return null;
  }

  return (
    state.players.find((player) => {
      if (player.uuid === target.uuid || player.zone !== target.zone) {
        return false;
      }

      const playerRank = realmRank(player.realm);
      return playerRank !== null && targetRank - playerRank >= NEG_DOMAIN_DROWNING_REALM_GAP;
    }) ?? null
  );
}

function realmRank(realm: string): number | null {
  return REALM_RANK[realm] ?? null;
}

function clearPendingIfSafe(worldModel: WorldModel | undefined, playerUuid: string): void {
  if (worldModel?.hasNegDomainPendingTribulation(playerUuid)) {
    worldModel.clearNegDomainPendingTribulation(playerUuid);
  }
}

function buildLostLockNarration(player: PlayerProfile): Narration {
  return {
    scope: "broadcast",
    style: "system_warning",
    text: `某通灵修士遁入负灵域，灵压倒悬，天道视线在其身侧断了一线。劫云不落，只待其离开此地。`,
    target: player.uuid,
  };
}

function buildRelockedNarration(player: PlayerProfile): Narration {
  return {
    scope: "broadcast",
    style: "system_warning",
    text: `某通灵修士离开负灵域庇护，断开的天道视线重新合拢。旧劫未消，只是换了落点与时辰。`,
    target: player.uuid,
  };
}

function buildDrowningBaiterHint(baiter: PlayerProfile): Narration {
  return {
    scope: "player",
    target: baiter.uuid,
    style: "perception",
    text: "你感知到对方的气息在急剧溃散——这是你的机会。",
  };
}

function buildDrowningPublicNarration(target: PlayerProfile, baiter: PlayerProfile): Narration {
  return {
    scope: "zone",
    target: target.zone,
    style: "narration",
    text: `${target.name}的真元如倒悬之泉，在负灵压下骤然溃散。${baiter.name}选择了溺水之地——此地天地之法，不偏向强者。`,
  };
}
