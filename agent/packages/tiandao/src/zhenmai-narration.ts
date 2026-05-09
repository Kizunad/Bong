import {
  CHANNELS,
  type CombatRealtimeEventV1,
  type Narration,
  type ZhenmaiSkillEventV1,
  validateCombatRealtimeEventV1Contract,
  validateNarrationV1Contract,
  validateZhenmaiSkillEventV1Contract,
} from "@bong/schema";

const { COMBAT_REALTIME, ZHENMAI_SKILL_EVENT, AGENT_NARRATE } = CHANNELS;

export interface ZhenmaiNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface ZhenmaiNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface ZhenmaiNarrationRuntimeStats {
  received: number;
  ignored: number;
  published: number;
  rejectedContract: number;
}

export interface ZhenmaiNarrationRuntimeConfig {
  sub: ZhenmaiNarrationRuntimeClient;
  pub: ZhenmaiNarrationRuntimeClient;
  logger?: ZhenmaiNarrationRuntimeLogger;
}

export function renderZhenmaiNarration(event: CombatRealtimeEventV1): Narration | null {
  if (event.kind !== "combat_event" || event.defense_kind !== "jie_mai") return null;
  const effectiveness = clampNumber(event.defense_effectiveness, 0.3, 1.0);
  const target = `zhenmai:parry|target:${event.target_id}|tick:${event.tick}`;
  const actor = shortName(event.target_id);
  let text: string;
  if (effectiveness >= 0.7) {
    text = `${actor} 皮下真元一震，异音未及入脉便被截断，千钧之势只剩半息僵直。`;
  } else if (effectiveness > 0.3) {
    text = `${actor} 勉强引爆截脉，外劲被削去一截，身形却已露出破绽。`;
  } else {
    text = `${actor} 被逼到贴身处才震爆，经脉护住了些，反冲却全压回血肉里。`;
  }

  const narration: Narration = {
    scope: "player",
    target,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export function renderZhenmaiSkillNarration(event: ZhenmaiSkillEventV1): Narration | null {
  if (event.type !== "zhenmai_skill_event") return null;
  const actor = shortName(event.caster_id);
  const meridian = event.meridian_id ?? event.meridian_ids?.join("、") ?? "经脉";
  const target = `zhenmai:${event.skill_id}|caster:${event.caster_id}|tick:${event.tick}`;
  let text: string;
  switch (event.skill_id) {
    case "parry":
      text = `${actor} 把血肉绷成一面短盾，等接触那一瞬反震回去。`;
      break;
    case "neutralize":
      text = `${actor} 指尖点住 ${meridian}，异种真元被一寸寸磨散，自己也亏去一口清气。`;
      break;
    case "multipoint":
      text = `${actor} 周身数处皮下齐震，来劲被分到各处，血雾一闪即灭。`;
      break;
    case "harden_meridian":
      text = `${actor} 把 ${meridian} 暂时绷硬，真元在脉壁上低鸣。`;
      break;
    case "sever_chain":
      text = event.grants_amplification === false
        ? `${actor} 自断 ${meridian}，断处空响，却没有引来足够反震。`
        : `${actor} 按断 ${meridian}，断隙里生出一条反震路，六十息内只赌这一线。`;
      break;
    default:
      text = `${actor} 运起截脉法，血肉里响过一声短促回音。`;
      break;
  }

  const narration: Narration = {
    scope: "player",
    target,
    text,
    style: "narration",
  };
  const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  return validation.ok ? narration : null;
}

export class ZhenmaiNarrationRuntime {
  private readonly sub: ZhenmaiNarrationRuntimeClient;
  private readonly pub: ZhenmaiNarrationRuntimeClient;
  private readonly logger: ZhenmaiNarrationRuntimeLogger;
  private connected = false;

  readonly stats: ZhenmaiNarrationRuntimeStats = {
    received: 0,
    ignored: 0,
    published: 0,
    rejectedContract: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== COMBAT_REALTIME && channel !== ZHENMAI_SKILL_EVENT) return;
    void this.handlePayload(channel, message);
  };

  constructor(config: ZhenmaiNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(COMBAT_REALTIME);
    await this.sub.subscribe(ZHENMAI_SKILL_EVENT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[zhenmai-runtime] subscribed to ${COMBAT_REALTIME}, ${ZHENMAI_SKILL_EVENT}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(channelOrMessage: string, maybeMessage?: string): Promise<void> {
    const channel = maybeMessage === undefined ? COMBAT_REALTIME : channelOrMessage;
    const message = maybeMessage === undefined ? channelOrMessage : maybeMessage;
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[zhenmai-runtime] non-JSON payload:", error);
      return;
    }

    const narration = this.renderPayload(channel, parsed);
    if (narration === undefined) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[zhenmai-runtime] invalid payload on channel:", channel);
      return;
    }
    this.stats.received += 1;

    if (!narration) {
      this.stats.ignored += 1;
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[zhenmai-runtime] publish failed:", error);
    }
  }

  private renderPayload(channel: string, parsed: unknown): Narration | null | undefined {
    if (channel === COMBAT_REALTIME) {
      const validation = validateCombatRealtimeEventV1Contract(parsed);
      return validation.ok ? renderZhenmaiNarration(parsed as CombatRealtimeEventV1) : undefined;
    }
    if (channel === ZHENMAI_SKILL_EVENT) {
      const validation = validateZhenmaiSkillEventV1Contract(parsed);
      return validation.ok ? renderZhenmaiSkillNarration(parsed as ZhenmaiSkillEventV1) : undefined;
    }
    return undefined;
  }
}

function clampNumber(value: unknown, min: number, max: number): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(min, Math.min(max, value))
    : min;
}

function shortName(id: string): string {
  const stripped = id.replace(/^offline:/u, "");
  return stripped.length > 0 ? stripped : "某修士";
}
