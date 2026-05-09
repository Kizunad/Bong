import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type HighRenownMilestoneEventV1,
  type Narration,
  type NicheIntrusionEventV1,
  type SocialFeudEventV1,
  type SocialPactEventV1,
  type WantedPlayerEventV1,
  validateHighRenownMilestoneEventV1Contract,
  validateNarrationV1Contract,
  validateNicheIntrusionEventV1Contract,
  validateSocialFeudEventV1Contract,
  validateSocialPactEventV1Contract,
  validateWantedPlayerEventV1,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";
import {
  scorePoliticalNarration,
} from "./narration-eval.js";

const {
  AGENT_NARRATE,
  SOCIAL_FEUD,
  SOCIAL_PACT,
  SOCIAL_NICHE_INTRUSION,
  WANTED_PLAYER,
  HIGH_RENOWN_MILESTONE,
} = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

export const POLITICAL_THROTTLE_MS = 5 * 60 * 1000;
const DEFAULT_ROUTEABLE_ZONE = "spawn";
export const POLITICAL_EVENT_CHANNELS = [
  SOCIAL_FEUD,
  SOCIAL_PACT,
  SOCIAL_NICHE_INTRUSION,
  WANTED_PLAYER,
  HIGH_RENOWN_MILESTONE,
] as const;

type PoliticalEventType = "feud" | "pact" | "niche_intrusion" | "wanted_player" | "high_renown_milestone";

export interface PoliticalNarrationContext {
  eventType: PoliticalEventType;
  scope: Narration["scope"];
  target?: string;
  zone: string;
  severity: number;
  bypassThrottle: boolean;
  identityExposed: boolean;
  exposedIdentities: string[];
  unexposedIdentities: string[];
  payload: unknown;
}

export class PoliticalNarrationThrottleStore {
  private readonly lastNarrationByZone = new Map<string, PoliticalNarrationThrottleEntry>();

  canEmit(zone: string, currentMs: number, bypass: boolean, severity = 0): boolean {
    if (bypass) return true;
    const last = this.lastNarrationByZone.get(zone);
    return (
      last === undefined ||
      currentMs - last.currentMs >= POLITICAL_THROTTLE_MS ||
      severity > last.severity
    );
  }

  record(zone: string, currentMs: number, severity = 0): void {
    this.lastNarrationByZone.set(zone, { currentMs, severity });
  }

  reserve(
    zone: string,
    currentMs: number,
    bypass: boolean,
    severity = 0,
  ): PoliticalNarrationThrottleReservation | null {
    if (bypass) {
      return {
        bypass: true,
        currentMs,
        severity,
        zone,
      };
    }
    const previous = this.lastNarrationByZone.get(zone);
    if (!this.canEmit(zone, currentMs, false, severity)) {
      return null;
    }
    this.record(zone, currentMs, severity);
    const reservation: PoliticalNarrationThrottleReservation = {
      bypass: false,
      currentMs,
      severity,
      zone,
    };
    if (previous !== undefined) {
      reservation.previous = previous;
    }
    return reservation;
  }

  commit(reservation: PoliticalNarrationThrottleReservation): void {
    if (reservation.bypass) {
      this.record(reservation.zone, reservation.currentMs, reservation.severity);
    }
  }

  rollback(reservation: PoliticalNarrationThrottleReservation): void {
    if (reservation.bypass) return;
    if (!this.isCurrent(reservation)) return;
    if (reservation.previous === undefined) {
      this.lastNarrationByZone.delete(reservation.zone);
      return;
    }
    this.lastNarrationByZone.set(reservation.zone, reservation.previous);
  }

  isCurrent(reservation: PoliticalNarrationThrottleReservation): boolean {
    if (reservation.bypass) return true;
    const current = this.lastNarrationByZone.get(reservation.zone);
    return current?.currentMs === reservation.currentMs && current.severity === reservation.severity;
  }
}

interface PoliticalNarrationThrottleEntry {
  currentMs: number;
  severity: number;
}

interface PoliticalNarrationThrottleReservation {
  zone: string;
  currentMs: number;
  severity: number;
  bypass: boolean;
  previous?: PoliticalNarrationThrottleEntry;
}

export interface PoliticalNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface PoliticalNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
}

export interface PoliticalNarrationRuntimeConfig {
  sub: PoliticalNarrationRuntimeClient;
  pub: PoliticalNarrationRuntimeClient;
  llm?: LlmClient;
  model?: string;
  logger?: PoliticalNarrationRuntimeLogger;
  now?: () => number;
  systemPrompt?: string;
  throttleStore?: PoliticalNarrationThrottleStore;
}

export interface PoliticalNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  ignored: number;
  throttled: number;
  llmFailures: number;
  fallbackUsed: number;
}

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "political.md"), "utf-8");
}

export class PoliticalNarrationRuntime {
  private readonly sub: PoliticalNarrationRuntimeClient;
  private readonly pub: PoliticalNarrationRuntimeClient;
  private readonly llm?: LlmClient;
  private readonly model: string;
  private readonly logger: PoliticalNarrationRuntimeLogger;
  private readonly now: () => number;
  private readonly systemPrompt: string;
  private readonly throttleStore: PoliticalNarrationThrottleStore;
  private connected = false;

  readonly stats: PoliticalNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    ignored: 0,
    throttled: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (!POLITICAL_EVENT_CHANNELS.includes(channel as (typeof POLITICAL_EVENT_CHANNELS)[number])) {
      return;
    }
    void this.handlePayload(channel, message);
  };

  constructor(config: PoliticalNarrationRuntimeConfig) {
    this.sub = config.sub;
    this.pub = config.pub;
    this.llm = config.llm;
    this.model = config.model ?? "mock";
    this.logger = config.logger ?? console;
    this.now = config.now ?? (() => Date.now());
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
    this.throttleStore = config.throttleStore ?? new PoliticalNarrationThrottleStore();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    for (const channel of POLITICAL_EVENT_CHANNELS) {
      await this.sub.subscribe(channel);
    }
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[political-runtime] subscribed to ${POLITICAL_EVENT_CHANNELS.join(", ")}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(channel: string, message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[political-runtime] non-JSON payload:", error);
      return;
    }

    const context = this.parseContext(channel, parsed);
    if (context === null) return;
    this.stats.received += 1;

    const currentMs = this.now();
    const reservation = this.throttleStore.reserve(
      context.zone,
      currentMs,
      context.bypassThrottle,
      context.severity,
    );
    if (reservation === null) {
      this.stats.throttled += 1;
      return;
    }

    const narration = await this.renderNarration(context);
    if (!this.throttleStore.isCurrent(reservation)) {
      this.stats.throttled += 1;
      return;
    }
    const envelope = { v: 1, narrations: [narration] };
    const validation = validateNarrationV1Contract(envelope);
    if (!validation.ok) {
      this.throttleStore.rollback(reservation);
      this.stats.rejectedContract += 1;
      this.logger.warn("[political-runtime] NarrationV1 contract rejected:", validation.errors.join("; "));
      return;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
      this.throttleStore.commit(reservation);
      this.stats.published += 1;
    } catch (error) {
      this.throttleStore.rollback(reservation);
      this.logger.warn("[political-runtime] publish failed:", error);
    }
  }

  private parseContext(channel: string, parsed: unknown): PoliticalNarrationContext | null {
    if (channel === SOCIAL_FEUD) return this.parseFeud(parsed);
    if (channel === SOCIAL_PACT) return this.parsePact(parsed);
    if (channel === SOCIAL_NICHE_INTRUSION) return this.parseNicheIntrusion(parsed);
    if (channel === WANTED_PLAYER) return this.parseWantedPlayer(parsed);
    if (channel === HIGH_RENOWN_MILESTONE) return this.parseHighRenownMilestone(parsed);
    this.stats.ignored += 1;
    return null;
  }

  private parseFeud(parsed: unknown): PoliticalNarrationContext | null {
    const validation = validateSocialFeudEventV1Contract(parsed);
    if (!validation.ok) return this.reject("SocialFeudEventV1", validation.errors);
    const payload = parsed as SocialFeudEventV1;
    const zone = payload.place?.trim() || "unknown_zone";
    return {
      eventType: "feud",
      scope: "zone",
      target: zone,
      zone,
      severity: 1,
      bypassThrottle: false,
      identityExposed: false,
      exposedIdentities: [],
      unexposedIdentities: [payload.left, payload.right],
      payload,
    };
  }

  private parsePact(parsed: unknown): PoliticalNarrationContext | null {
    const validation = validateSocialPactEventV1Contract(parsed);
    if (!validation.ok) return this.reject("SocialPactEventV1", validation.errors);
    const payload = parsed as SocialPactEventV1;
    if (payload.broken) {
      this.stats.ignored += 1;
      return null;
    }
    return {
      eventType: "pact",
      scope: "zone",
      target: DEFAULT_ROUTEABLE_ZONE,
      zone: DEFAULT_ROUTEABLE_ZONE,
      severity: 1,
      bypassThrottle: false,
      identityExposed: false,
      exposedIdentities: [],
      unexposedIdentities: [payload.left, payload.right],
      payload,
    };
  }

  private parseNicheIntrusion(parsed: unknown): PoliticalNarrationContext | null {
    const validation = validateNicheIntrusionEventV1Contract(parsed);
    if (!validation.ok) return this.reject("NicheIntrusionEventV1", validation.errors);
    const payload = parsed as NicheIntrusionEventV1;
    return {
      eventType: "niche_intrusion",
      scope: "zone",
      target: DEFAULT_ROUTEABLE_ZONE,
      zone: DEFAULT_ROUTEABLE_ZONE,
      severity: 3,
      bypassThrottle: true,
      identityExposed: false,
      exposedIdentities: [],
      unexposedIdentities: [payload.intruder_id],
      payload,
    };
  }

  private parseWantedPlayer(parsed: unknown): PoliticalNarrationContext | null {
    const validation = validateWantedPlayerEventV1(parsed);
    if (!validation.ok) return this.reject("WantedPlayerEventV1", validation.errors);
    const payload = parsed as WantedPlayerEventV1;
    return {
      eventType: "wanted_player",
      scope: "broadcast",
      zone: "broadcast",
      severity: 4,
      bypassThrottle: true,
      identityExposed: true,
      exposedIdentities: [payload.identity_display_name],
      unexposedIdentities: [],
      payload,
    };
  }

  private parseHighRenownMilestone(parsed: unknown): PoliticalNarrationContext | null {
    const validation = validateHighRenownMilestoneEventV1Contract(parsed);
    if (!validation.ok) return this.reject("HighRenownMilestoneEventV1", validation.errors);
    const payload = parsed as HighRenownMilestoneEventV1;
    const broadcast = payload.milestone >= 1000;
    const zone = broadcast ? "broadcast" : (payload.zone?.trim() || DEFAULT_ROUTEABLE_ZONE);
    return {
      eventType: "high_renown_milestone",
      scope: broadcast ? "broadcast" : "zone",
      target: broadcast ? undefined : zone,
      zone,
      severity: broadcast ? 4 : 2,
      bypassThrottle: broadcast,
      identityExposed: payload.identity_exposed,
      exposedIdentities: payload.identity_exposed ? [payload.identity_display_name] : [],
      unexposedIdentities: payload.identity_exposed ? [] : [payload.identity_display_name],
      payload,
    };
  }

  private reject(contractName: string, errors: string[]): null {
    this.stats.rejectedContract += 1;
    this.logger.warn(`[political-runtime] ${contractName} contract rejected:`, errors.join("; "));
    return null;
  }

  private async renderNarration(context: PoliticalNarrationContext): Promise<Narration> {
    const fallback = renderPoliticalNarration(context);
    if (!this.llm) {
      this.stats.fallbackUsed += 1;
      return fallback;
    }

    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(context) },
      ]);
      const candidate = parsePoliticalNarrationContent(
        normalizeLlmChatResult(result, this.model).content,
        context,
      );
      if (candidate.text === fallback.text) this.stats.fallbackUsed += 1;
      return candidate;
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[political-runtime] LLM error:", error);
      return fallback;
    }
  }
}

export function parsePoliticalNarrationContent(content: string, context: PoliticalNarrationContext): Narration {
  const fallback = renderPoliticalNarration(context);
  try {
    const parsed = JSON.parse(content.trim()) as unknown;
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed) ||
      typeof (parsed as { text?: unknown }).text !== "string"
    ) {
      return fallback;
    }
    const narration: Narration = {
      scope: context.scope,
      target: context.target,
      text: (parsed as { text: string }).text,
      style: "political_jianghu",
      kind: "political_jianghu",
    };
    const score = scorePoliticalNarration(narration.text, context);
    if (!score.hasJianghuVoice || !score.noModernPoliticalTerms || !score.anonymityOk) {
      return fallback;
    }
    const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
    return validation.ok ? narration : fallback;
  } catch {
    return fallback;
  }
}

export function renderPoliticalNarration(context: PoliticalNarrationContext): Narration {
  return {
    scope: context.scope,
    target: context.target,
    text: fallbackText(context),
    style: "political_jianghu",
    kind: "political_jianghu",
  };
}

function fallbackText(context: PoliticalNarrationContext): string {
  switch (context.eventType) {
    case "feud":
      return `江湖有传，${context.zone} 两名修士旧怨添血，誓言已不肯留明日；山风只递半句，谁先回头，便是谁先露怯。`;
    case "pact":
      return "市井相传，有二修士结契同行，以血口作证；契上字未必能久，旁人只见火光一瞬，后事仍藏在袖底。";
    case "niche_intrusion":
      return `山中有人道，${displayLocation(context)} 一处灵龛遭破，主人名姓仍被尘土遮着；取物者脚印未干，传闻已先行过岭。`;
    case "wanted_player": {
      const payload = context.payload as WantedPlayerEventV1;
      return `闻者道，${payload.identity_display_name} 的画影已过诸市，旧账与恶名一并钉在纸上；见者是避是杀，各凭命薄。`;
    }
    case "high_renown_milestone": {
      const payload = context.payload as HighRenownMilestoneEventV1;
      const name = payload.identity_exposed ? payload.identity_display_name : "某修士";
      return `江湖有传，${name} 之名已越 ${payload.milestone} 声名阈，酒肆里有人添灯，有人掩门；名声既起，后路便不再由己。`;
    }
  }
}

function displayLocation(context: PoliticalNarrationContext): string {
  if (context.eventType === "niche_intrusion" || context.zone.startsWith("niche:")) {
    return "林中";
  }
  return context.zone;
}
