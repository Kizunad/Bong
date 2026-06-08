import type { Command, Narration, PlayerProfile, WorldStateV1, ZoneSnapshot } from "@bong/schema";
import type { WorldModel } from "./world-model.js";

const TRIBULATION_EVENT = "thunder_tribulation";
const NEG_DOMAIN_QI_THRESHOLD = 0;
const SPIRIT_REALM = "Spirit";

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

function isSpiritRealm(player: PlayerProfile): boolean {
  return player.realm === SPIRIT_REALM;
}

function isNegativeDomain(zone: ZoneSnapshot): boolean {
  return zone.spirit_qi < NEG_DOMAIN_QI_THRESHOLD;
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
