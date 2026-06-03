/**
 * plan-combat-skill-feedback-bridges-v1 P0 — 经脉永久 SEVERED 叙事 runtime。
 *
 * 订阅 `bong:meridian_severed`（`CHANNELS.MERIDIAN_SEVERED`），
 * 验证 `MeridianSeveredEventV1` schema，
 * 调用 `renderMeridianSeveredNarration` → publish 到 `AGENT_NARRATE`。
 *
 * 结构仿 `baomai-v3-runtime.ts`（class + connect/disconnect + stats）。
 */

import {
  CHANNELS,
  type MeridianSeveredEventV1,
  validateMeridianSeveredEventV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

import { renderMeridianSeveredNarration } from "./meridian-severed-narration.js";

const { AGENT_NARRATE, MERIDIAN_SEVERED } = CHANNELS;

export interface MeridianSeveredRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface MeridianSeveredRuntimeStats {
  received: number;
  ignored: number;
  published: number;
  rejectedContract: number;
}

export interface MeridianSeveredRuntimeConfig {
  sub: MeridianSeveredRuntimeClient;
  pub: MeridianSeveredRuntimeClient;
  logger?: Pick<typeof console, "info" | "warn">;
}

export class MeridianSeveredNarrationRuntime {
  private readonly sub: MeridianSeveredRuntimeClient;
  private readonly pub: MeridianSeveredRuntimeClient;
  private readonly logger: Pick<typeof console, "info" | "warn">;
  private connected = false;

  readonly stats: MeridianSeveredRuntimeStats = {
    received: 0,
    ignored: 0,
    published: 0,
    rejectedContract: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== MERIDIAN_SEVERED) return;
    void this.handlePayload(channel, message);
  };

  constructor(config: MeridianSeveredRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(MERIDIAN_SEVERED);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(
      `[meridian-severed-runtime] subscribed to ${MERIDIAN_SEVERED}`,
    );
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(
    channelOrMessage: string,
    maybeMessage?: string,
  ): Promise<void> {
    const message =
      maybeMessage === undefined ? channelOrMessage : maybeMessage;
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[meridian-severed-runtime] non-JSON payload:",
        error,
      );
      return;
    }

    const validation = validateMeridianSeveredEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[meridian-severed-runtime] invalid payload:",
        validation.errors,
      );
      return;
    }
    this.stats.received += 1;

    const narration = renderMeridianSeveredNarration(
      parsed as MeridianSeveredEventV1,
    );
    if (!narration) {
      this.stats.ignored += 1;
      return;
    }

    const narrationEnvelope = { v: 1, narrations: [narration] };
    const contractCheck = validateNarrationV1Contract(narrationEnvelope);
    if (!contractCheck.ok) {
      this.stats.ignored += 1;
      this.logger.warn(
        "[meridian-severed-runtime] narration contract rejected:",
        contractCheck.errors,
      );
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify(narrationEnvelope));
      this.stats.published += 1;
    } catch (error) {
      this.logger.warn("[meridian-severed-runtime] publish failed:", error);
    }
  }
}

/**
 * 构造 MeridianSeveredNarrationRuntime 并 connect，返回 cleanup fn。
 * 供 main.ts `startAuxiliaryRuntimes` 调用。
 */
export async function startMeridianSeveredNarrationRuntime(opts: {
  sub: MeridianSeveredRuntimeClient;
  pub: MeridianSeveredRuntimeClient;
}): Promise<() => Promise<void>> {
  const runtime = new MeridianSeveredNarrationRuntime({
    sub: opts.sub,
    pub: opts.pub,
  });
  runtime
    .connect()
    .then(() =>
      console.log("[tiandao] meridian-severed narration runtime online"),
    )
    .catch((error) =>
      console.warn(
        "[tiandao] meridian-severed narration runtime failed to start:",
        error,
      ),
    );
  return async () => {
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 500));
    try {
      await Promise.race([runtime.disconnect(), timeout]);
    } catch (error) {
      console.warn(
        "[tiandao] meridian-severed narration runtime disconnect error:",
        error,
      );
    }
  };
}
