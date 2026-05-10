import {
  CHANNELS,
  type Narration,
  type ZhenfaV2EventV1,
  validateNarrationV1Contract,
  validateZhenfaV2EventV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, ZHENFA_V2_EVENT } = CHANNELS;

export interface ZhenfaV2NarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface ZhenfaV2NarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface ZhenfaV2NarrationRuntimeConfig {
  sub: ZhenfaV2NarrationRuntimeClient;
  pub: ZhenfaV2NarrationRuntimeClient;
  logger?: ZhenfaV2NarrationRuntimeLogger;
}

export interface ZhenfaV2NarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  ignored: number;
}

export function renderZhenfaV2Narration(event: ZhenfaV2EventV1): Narration | null {
  const actor = shortName(event.owner);
  const metadataTarget = `zhenfa:${event.event}|${event.kind}|id:${event.array_id}|tick:${event.tick}`;
  const zone = event.zone?.trim();
  let scope: Narration["scope"] = "broadcast";
  let target = metadataTarget;
  if (event.event !== "deceive_heaven_exposed" && zone) {
    scope = "zone";
    target = zone;
  }
  let text: string;

  switch (event.event) {
    case "deploy":
      text = deployText(event, actor);
      break;
    case "decay":
      text = `${actor} 留下的${arrayName(event.kind)}气机散了，阵眼只剩一圈冷灰。`;
      break;
    case "breakthrough":
      text = event.force_break
        ? `${actor} 的${arrayName(event.kind)}被人硬破，反冲先咬住破阵者。`
        : `${actor} 的${arrayName(event.kind)}被拆开，封存的真元漏回地脉。`;
      break;
    case "deceive_heaven_exposed":
      text = "欺天阵露了破绽。天道不懂羞辱，只把假账连本带息记回布阵者身上。";
      break;
    default:
      return null;
  }

  const narration: Narration = {
    scope,
    target,
    text,
    style: event.event === "deceive_heaven_exposed" ? "system_warning" : "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export class ZhenfaV2NarrationRuntime {
  private readonly sub: ZhenfaV2NarrationRuntimeClient;
  private readonly pub: ZhenfaV2NarrationRuntimeClient;
  private readonly logger: ZhenfaV2NarrationRuntimeLogger;
  private connected = false;

  readonly stats: ZhenfaV2NarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    ignored: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== ZHENFA_V2_EVENT) return;
    void this.handlePayload(message);
  };

  constructor(config: ZhenfaV2NarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(ZHENFA_V2_EVENT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[zhenfa-v2-runtime] subscribed to ${ZHENFA_V2_EVENT}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[zhenfa-v2-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateZhenfaV2EventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[zhenfa-v2-runtime] invalid event:", validation.errors.join("; "));
      return;
    }
    this.stats.received += 1;

    const narration = renderZhenfaV2Narration(parsed as ZhenfaV2EventV1);
    if (!narration) {
      this.stats.ignored += 1;
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[zhenfa-v2-runtime] publish failed:", error);
    }
  }
}

function deployText(event: ZhenfaV2EventV1, actor: string): string {
  switch (event.kind) {
    case "shrine_ward":
      return `${actor} 在灵龛外压下一圈阵纹，靠近者先撞见自己的力道。`;
    case "lingju":
      return `${actor} 把真元封进地脉，灵气暂时聚拢，天道也多看了一眼。`;
    case "deceive_heaven":
      return `${actor} 起欺天阵，账面上的劫气被悄悄改写；假的终究要被守恒追账。`;
    case "illusion":
      return `${actor} 以缜密色覆住阵眼，外人看见的只是一块寻常地面。`;
    case "trap":
      return `${actor} 埋下真元诡雷，地面安静得有些不对。`;
    case "ward":
      return `${actor} 立起警戒场，陌生气机一入边界便会回声。`;
    default:
      return `${actor} 摆下一座阵法，阵眼在土里慢慢发热。`;
  }
}

function arrayName(kind: string): string {
  switch (kind) {
    case "shrine_ward":
      return "护龛阵";
    case "lingju":
      return "聚灵阵";
    case "deceive_heaven":
      return "欺天阵";
    case "illusion":
      return "幻阵";
    case "ward":
      return "警戒场";
    default:
      return "真元诡雷";
  }
}

function shortName(id: string): string {
  const stripped = id.replace(/^offline:/u, "").replace(/^entity_bits:/u, "实体");
  return stripped.length > 0 ? stripped : "某修士";
}
