import {
  CHANNELS,
  type BaomaiSkillEventV1,
  type Narration,
  validateBaomaiSkillEventV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

const { AGENT_NARRATE, BAOMAI_V3_SKILL_EVENT } = CHANNELS;

export interface BaomaiV3RuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface BaomaiV3RuntimeStats {
  received: number;
  ignored: number;
  published: number;
  rejectedContract: number;
}

export interface BaomaiV3RuntimeConfig {
  sub: BaomaiV3RuntimeClient;
  pub: BaomaiV3RuntimeClient;
  logger?: Pick<typeof console, "info" | "warn">;
}

export function renderBaomaiV3Narration(event: BaomaiSkillEventV1): Narration | null {
  if (event.type !== "baomai_skill_event") return null;
  const actor = shortName(event.caster_id);
  const meridians = event.meridian_ids.length > 0 ? event.meridian_ids.join("、") : "经脉";
  let text: string;
  switch (event.skill_id) {
    case "beng_quan":
      text = `${actor} 贴身把真元压进拳骨，${meridians} 上裂纹一闪，劲力沉到对手胸前。`;
      break;
    case "full_power_charge":
      text = `${actor} 收肩沉息，整池真元向拳锋聚拢，手三阳先响了一声闷雷。`;
      break;
    case "full_power_release":
      text = `${actor} 把蓄满的一拳递出去，真元没有绕路，整条脉线只剩一记直撞。`;
      break;
    case "mountain_shake":
      text = `${actor} 一拳砸进地面，震波沿土石滚开，近处脚步全被抬乱。`;
      break;
    case "blood_burn":
      text = `${actor} 割开血线换一口猛劲，血雾贴着皮肤烧成沉红。`;
      break;
    case "disperse":
      text = event.flow_rate_multiplier >= 10
        ? `${actor} 烧去半池真元重铸凡躯，五息之内脉流暴涨十倍，却没有一分免伤余地。`
        : `${actor} 强行散功，凡躯没有应声，只白白折去一截真元池。`;
      break;
    default:
      text = `${actor} 运起爆脉法，肉身里传出一声低闷脉响。`;
      break;
  }

  const narration: Narration = {
    scope: "player",
    target: event.caster_id,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export class BaomaiV3NarrationRuntime {
  private readonly sub: BaomaiV3RuntimeClient;
  private readonly pub: BaomaiV3RuntimeClient;
  private readonly logger: Pick<typeof console, "info" | "warn">;
  private connected = false;

  readonly stats: BaomaiV3RuntimeStats = {
    received: 0,
    ignored: 0,
    published: 0,
    rejectedContract: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== BAOMAI_V3_SKILL_EVENT) return;
    void this.handlePayload(channel, message);
  };

  constructor(config: BaomaiV3RuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(BAOMAI_V3_SKILL_EVENT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[baomai-v3-runtime] subscribed to ${BAOMAI_V3_SKILL_EVENT}`);
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
      this.logger.warn("[baomai-v3-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateBaomaiSkillEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[baomai-v3-runtime] invalid payload:", validation.errors);
      return;
    }
    this.stats.received += 1;

    const narration = renderBaomaiV3Narration(parsed as BaomaiSkillEventV1);
    if (!narration) {
      this.stats.ignored += 1;
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[baomai-v3-runtime] publish failed:", error);
    }
  }
}

function shortName(id: string): string {
  const stripped = id.replace(/^offline:/u, "");
  return stripped.length > 0 ? stripped : "某体修";
}
