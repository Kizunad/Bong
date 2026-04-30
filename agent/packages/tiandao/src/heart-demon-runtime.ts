import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  HEART_DEMON_CANONICAL_CHOICES,
  type HeartDemonOfferDraftV1,
  type HeartDemonPregenRequestV1,
  validateHeartDemonOfferDraftV1Contract,
  validateHeartDemonPregenRequestV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { HEART_DEMON_REQUEST, HEART_DEMON_OFFER } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));
const MILLIS_PER_TICK = 50;
const HEART_DEMON_TIMEOUT_TICKS = 30 * 20;

export interface HeartDemonRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
}

export interface HeartDemonRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface HeartDemonRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: HeartDemonRuntimeClient;
  pub: HeartDemonRuntimeClient;
  logger?: HeartDemonRuntimeLogger;
  now?: () => number;
  systemPrompt?: string;
}

export interface HeartDemonRuntimeStats {
  received: number;
  offered: number;
  rejectedContract: number;
  rejectedArbiter: number;
  llmFailures: number;
  fallbackUsed: number;
}

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "heart-demon.md"), "utf-8");
}

export function fallbackHeartDemonOffer(
  request: HeartDemonPregenRequestV1,
  now: () => number = () => Date.now(),
): HeartDemonOfferDraftV1 {
  return {
    offer_id: request.trigger_id,
    trigger_id: request.trigger_id,
    trigger_label: "心魔劫临身",
    realm_label: "渡虚劫 · 心魔",
    composure: clamp01(request.composure),
    quota_remaining: 1,
    quota_total: 1,
    expires_at_ms: now() + HEART_DEMON_TIMEOUT_TICKS * MILLIS_PER_TICK,
    choices: HEART_DEMON_CANONICAL_CHOICES.map((choice) => ({
      choice_id: choice.choice_id,
      category: choice.category,
      title: choice.title,
      effect_summary: choice.effect_summary,
      flavor: fallbackFlavor(choice.kind, request),
      style_hint: choice.style_hint,
    })),
  };
}

export function applyHeartDemonArbiter(
  request: HeartDemonPregenRequestV1,
  offer: HeartDemonOfferDraftV1,
  now: () => number = () => Date.now(),
): HeartDemonOfferDraftV1 {
  const byId = new Map(offer.choices.map((choice) => [choice.choice_id, choice]));
  const fallback = fallbackHeartDemonOffer(request, now);
  const choices = HEART_DEMON_CANONICAL_CHOICES.map((canonical, index) => {
    const proposed = byId.get(canonical.choice_id);
    const fallbackChoice = fallback.choices[index];
    return {
      choice_id: canonical.choice_id,
      category: canonical.category,
      title: cleanText(proposed?.title, fallbackChoice.title, 64),
      effect_summary: canonical.effect_summary,
      flavor: cleanText(proposed?.flavor, fallbackChoice.flavor, 500),
      style_hint: cleanText(proposed?.style_hint, canonical.style_hint, 64),
    };
  });

  return {
    offer_id: offer.offer_id || request.trigger_id,
    trigger_id: request.trigger_id,
    trigger_label: cleanText(offer.trigger_label, "心魔劫临身", 128),
    realm_label: cleanText(offer.realm_label, "渡虚劫 · 心魔", 128),
    composure: clamp01(offer.composure),
    quota_remaining: 1,
    quota_total: 1,
    expires_at_ms: offer.expires_at_ms > 0 ? offer.expires_at_ms : fallback.expires_at_ms,
    choices,
  };
}

function parseOfferDraft(
  content: string,
  request: HeartDemonPregenRequestV1,
  logger: HeartDemonRuntimeLogger,
): HeartDemonOfferDraftV1 | null {
  const trimmed = content.trim();
  if (trimmed.length === 0) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    logger.warn("[heart-demon-runtime] LLM output is not valid JSON:", error);
    return null;
  }

  const loose = parsed as Partial<HeartDemonOfferDraftV1>;
  const patched: HeartDemonOfferDraftV1 = {
    offer_id: loose.offer_id ?? request.trigger_id,
    trigger_id: loose.trigger_id ?? request.trigger_id,
    trigger_label: loose.trigger_label ?? "心魔劫临身",
    realm_label: loose.realm_label ?? "渡虚劫 · 心魔",
    composure: typeof loose.composure === "number" ? loose.composure : request.composure,
    quota_remaining: typeof loose.quota_remaining === "number" ? loose.quota_remaining : 1,
    quota_total: typeof loose.quota_total === "number" ? loose.quota_total : 1,
    expires_at_ms: typeof loose.expires_at_ms === "number" ? loose.expires_at_ms : 1,
    choices: Array.isArray(loose.choices) ? loose.choices : [],
  };
  const validation = validateHeartDemonOfferDraftV1Contract(patched);
  if (!validation.ok) {
    logger.warn(
      "[heart-demon-runtime] LLM output fails HeartDemonOfferDraftV1 contract:",
      validation.errors.join("; "),
    );
    return null;
  }

  return patched;
}

function fallbackFlavor(
  kind: (typeof HEART_DEMON_CANONICAL_CHOICES)[number]["kind"],
  request: HeartDemonPregenRequestV1,
): string {
  const tail = request.recent_biography.at(-1);
  const memory = tail ? `旧事 ${tail} 浮起，` : "旧事无名浮起，";
  switch (kind) {
    case "steadfast":
      return `${memory}${request.actor_name} 不逐影，不辩幻象，只把呼吸压回丹田。`;
    case "obsession":
      return `${memory}${request.actor_name} 提起一念作刀，刀锋却照见自己的影。`;
    case "no_solution":
      return `${memory}${request.actor_name} 看清此问无门，便不再替天道补题。`;
  }
  return `${memory}${request.actor_name} 默然看着心魔退回云下。`;
}

function cleanText(value: string | undefined, fallback: string, maxLength: number): string {
  const trimmed = value?.trim();
  if (!trimmed) return fallback;
  return trimmed.length > maxLength ? trimmed.slice(0, maxLength) : trimmed;
}

function clamp01(value: number): number {
  return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0.5;
}

export class HeartDemonRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: HeartDemonRuntimeClient;
  private readonly pub: HeartDemonRuntimeClient;
  private readonly logger: HeartDemonRuntimeLogger;
  private readonly now: () => number;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: HeartDemonRuntimeStats = {
    received: 0,
    offered: 0,
    rejectedContract: 0,
    rejectedArbiter: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== HEART_DEMON_REQUEST) return;
    void this.handleRequestPayload(message);
  };

  constructor(config: HeartDemonRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.now = config.now ?? (() => Date.now());
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(HEART_DEMON_REQUEST);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[heart-demon-runtime] subscribed to ${HEART_DEMON_REQUEST}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handleRequestPayload(message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[heart-demon-runtime] non-JSON payload:", error);
      return;
    }

    const contract = validateHeartDemonPregenRequestV1Contract(parsed);
    if (!contract.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[heart-demon-runtime] HeartDemonPregenRequestV1 contract rejected:",
        contract.errors.join("; "),
      );
      return;
    }

    const request = parsed as HeartDemonPregenRequestV1;
    this.stats.received += 1;

    const offer = await this.produceOffer(request);
    const payload = JSON.stringify(offer);
    try {
      const subscribers = await this.pub.publish(HEART_DEMON_OFFER, payload);
      this.stats.offered += 1;
      this.logger.info(
        `[heart-demon-runtime] published offer ${offer.offer_id} for ${request.trigger_id} (${offer.choices.length} choices, ${subscribers} subs)`,
      );
    } catch (error) {
      this.logger.warn("[heart-demon-runtime] publish failed:", error);
    }
  }

  private async produceOffer(request: HeartDemonPregenRequestV1): Promise<HeartDemonOfferDraftV1> {
    let offer: HeartDemonOfferDraftV1 | null = null;
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(request) },
      ]);
      const normalized = normalizeLlmChatResult(result, this.model);
      offer = parseOfferDraft(normalized.content, request, this.logger);
    } catch (error) {
      this.stats.llmFailures += 1;
      this.logger.warn("[heart-demon-runtime] LLM error:", error);
    }

    if (!offer) {
      this.stats.fallbackUsed += 1;
      return fallbackHeartDemonOffer(request, this.now);
    }

    const filtered = applyHeartDemonArbiter(request, offer, this.now);
    const validation = validateHeartDemonOfferDraftV1Contract(filtered);
    if (!validation.ok) {
      this.stats.rejectedArbiter += 1;
      this.stats.fallbackUsed += 1;
      return fallbackHeartDemonOffer(request, this.now);
    }
    return filtered;
  }
}
