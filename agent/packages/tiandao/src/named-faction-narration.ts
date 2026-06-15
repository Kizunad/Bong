import {
  CHANNELS,
  type NamedFactionStateV1,
  type Narration,
  validateNamedFactionStateV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, NAMED_FACTION_STATE } = CHANNELS;

type NamedFactionStatus = NamedFactionStateV1["named_factions"][number]["status"];
type NamedFactionEntry = NamedFactionStateV1["named_factions"][number];

export interface NamedFactionNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface NamedFactionNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface NamedFactionNarrationRuntimeConfig {
  sub: NamedFactionNarrationRuntimeClient;
  pub: NamedFactionNarrationRuntimeClient;
  logger?: NamedFactionNarrationRuntimeLogger;
}

export interface NamedFactionNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  ignored: number;
}

export class NamedFactionNarrationRuntime {
  private readonly sub: NamedFactionNarrationRuntimeClient;
  private readonly pub: NamedFactionNarrationRuntimeClient;
  private readonly logger: NamedFactionNarrationRuntimeLogger;
  private readonly lastStatusByFaction = new Map<string, NamedFactionStatus>();
  private connected = false;

  readonly stats: NamedFactionNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    ignored: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== NAMED_FACTION_STATE) return;
    if (!this.connected) return;
    void this.handlePayload(message).catch((error: unknown) => {
      this.logger.warn("[named-faction-runtime] failed to handle payload:", error);
    });
  };

  constructor(config: NamedFactionNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(NAMED_FACTION_STATE);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[named-faction-runtime] subscribed to ${NAMED_FACTION_STATE}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(channelOrMessage: string, maybeMessage?: string): Promise<void> {
    const message = maybeMessage === undefined ? channelOrMessage : maybeMessage;
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[named-faction-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateNamedFactionStateV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[named-faction-runtime] schema validation failed:",
        validation.errors.join("; "),
      );
      return;
    }

    const payload = parsed as NamedFactionStateV1;
    this.stats.received += 1;
    const narrations = this.collectTransitionNarrations(payload);
    if (narrations.length === 0) {
      this.stats.ignored += 1;
      return;
    }

    const envelope = { v: 1 as const, narrations };
    const output = validateNarrationV1Contract(envelope);
    if (!output.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[named-faction-runtime] generated narration rejected:",
        output.errors.join("; "),
      );
      return;
    }

    await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
    this.stats.published += narrations.length;
  }

  private collectTransitionNarrations(payload: NamedFactionStateV1): Narration[] {
    const narrations: Narration[] = [];
    for (const faction of payload.named_factions) {
      const previous = this.lastStatusByFaction.get(faction.id);
      this.lastStatusByFaction.set(faction.id, faction.status);
      if (previous === undefined) continue;
      if (previous !== "decayed" && faction.status === "decayed") {
        narrations.push(renderFactionDecayNarration(faction));
        continue;
      }
      if (previous === "active" && faction.status === "headless") {
        narrations.push(renderLeaderDownNarration(faction));
      }
    }
    return narrations;
  }
}

export function renderLeaderDownNarration(faction: NamedFactionEntry): Narration {
  return {
    scope: "broadcast",
    text:
      faction.id === "qingyun_hunters"
        ? "青云猎盟的盟主死在血谷口——残峰上群龙无首，过路费再没人收了。"
        : `${faction.display_name}的头领倒在${zoneLabel(faction.zone_anchor)}——旧旗还在，发号施令的人没了。`,
    style: "political_jianghu",
  };
}

export function renderFactionDecayNarration(faction: NamedFactionEntry): Narration {
  return {
    scope: "broadcast",
    text:
      faction.id === "qingyun_hunters"
        ? "青云猎盟的最后一支队伍在血谷覆灭——残峰上再无那面破旗帜飘动。"
        : `${faction.display_name}的最后一支队伍在${zoneLabel(faction.zone_anchor)}散尽——这片地界再无人认那面旧旗。`,
    style: "political_jianghu",
  };
}

function zoneLabel(zone: string): string {
  switch (zone) {
    case "qingyun_peaks":
      return "青云残峰";
    case "blood_valley":
      return "血谷";
    case "north_wastes":
      return "北荒";
    default:
      return zone;
  }
}
