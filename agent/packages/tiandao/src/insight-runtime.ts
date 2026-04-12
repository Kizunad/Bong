/**
 * 顿悟 Agent runtime — 事件驱动（plan-cultivation-v1 §5 / §P4）。
 *
 * 订阅 `bong:insight_request`，调 LLM（或 fallback）生成
 * `InsightOfferV1` 并发布到 `bong:insight_offer`。
 *
 * 与三层 tick-based agent 并列但独立：
 *   - 不受 Arbiter 的 era/zone conflict 逻辑影响
 *   - Arbiter 校验放在本 runtime 内部（白名单 + magnitude caps）
 *   - LLM 失败 / Arbiter 拒绝 / 空 choices → 交由 server 端 fallback 池兜底
 *     （本 runtime 选择发布空 offer，server `fallback_for()` 已在 insight_fallback.rs 就位）
 */

import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  InsightCategory,
  InsightOfferV1,
  InsightRequestV1,
  validateInsightOfferV1Contract,
  validateInsightRequestV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { INSIGHT_REQUEST, INSIGHT_OFFER } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

export interface InsightRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
}

export interface InsightRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface InsightRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: InsightRuntimeClient;
  pub: InsightRuntimeClient;
  logger?: InsightRuntimeLogger;
  now?: () => number;
  systemPrompt?: string;
  /** LLM 超时后是否发布空 offer（让 server fallback）。默认 true。 */
  publishEmptyOnFailure?: boolean;
}

export interface InsightRuntimeStats {
  received: number;
  offered: number;
  rejectedContract: number;
  rejectedArbiter: number;
  llmFailures: number;
}

/**
 * 校验 agent 产出的 choices：每条 category ∈ available_categories 且
 * magnitude ≤ global_caps[category]。违规条目被丢弃；返回过滤后数组。
 */
export function applyInsightArbiter(
  request: InsightRequestV1,
  offer: InsightOfferV1,
): InsightOfferV1 {
  const allowed = new Set<InsightCategory>(request.available_categories);
  const caps = request.global_caps;

  const filtered = offer.choices.filter((choice) => {
    if (!allowed.has(choice.category)) return false;
    const cap = caps[choice.category];
    if (typeof cap === "number" && choice.magnitude > cap) return false;
    return true;
  });

  return { ...offer, choices: filtered.slice(0, 4) };
}

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "insight.md"), "utf-8");
}

function parseAgentOutput(
  content: string,
  request: InsightRequestV1,
  logger: InsightRuntimeLogger,
): InsightOfferV1 | null {
  const trimmed = content.trim();
  if (trimmed.length === 0) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    logger.warn("[insight-runtime] LLM output is not valid JSON:", error);
    return null;
  }

  const validation = validateInsightOfferV1Contract(parsed);
  if (!validation.ok) {
    // 允许宽松一点：若缺 offer_id / trigger_id 我们补上再校验一次
    const loose = parsed as Partial<InsightOfferV1>;
    const patched: InsightOfferV1 = {
      offer_id: loose.offer_id ?? generateOfferId(request),
      trigger_id: loose.trigger_id ?? request.trigger_id,
      choices: Array.isArray(loose.choices) ? loose.choices : [],
    };
    const revalidated = validateInsightOfferV1Contract(patched);
    if (!revalidated.ok) {
      logger.warn(
        "[insight-runtime] LLM output fails InsightOfferV1 contract:",
        revalidated.errors.join("; "),
      );
      return null;
    }
    return patched;
  }

  return parsed as InsightOfferV1;
}

function generateOfferId(request: InsightRequestV1): string {
  const rand = Math.random().toString(36).slice(2, 8);
  return `ofr_${request.trigger_id}_${Date.now()}_${rand}`;
}

/** 空 offer 让 server 端 fallback_for() 接管（至少 3 条静态选项）。 */
function emptyOffer(request: InsightRequestV1): InsightOfferV1 {
  return {
    offer_id: generateOfferId(request),
    trigger_id: request.trigger_id,
    // minItems:1，所以放一条无效占位的话 server 端 arbiter 仍会拒收；
    // 直接构造带 1 条保底"拒绝"风味的选项，让用户体感一致。
    choices: [
      {
        category: request.available_categories[0] ?? "Composure",
        effect_kind: "NoOp",
        magnitude: 0,
        flavor_text: "心未契机，此番无所得。",
      },
    ],
  };
}

export class InsightRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: InsightRuntimeClient;
  private readonly pub: InsightRuntimeClient;
  private readonly logger: InsightRuntimeLogger;
  private readonly now: () => number;
  private readonly systemPrompt: string;
  private readonly publishEmptyOnFailure: boolean;
  private connected = false;
  readonly stats: InsightRuntimeStats = {
    received: 0,
    offered: 0,
    rejectedContract: 0,
    rejectedArbiter: 0,
    llmFailures: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== INSIGHT_REQUEST) return;
    void this.handleRequestPayload(message);
  };

  constructor(config: InsightRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.now = config.now ?? (() => Date.now());
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
    this.publishEmptyOnFailure = config.publishEmptyOnFailure ?? true;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(INSIGHT_REQUEST);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[insight-runtime] subscribed to ${INSIGHT_REQUEST}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  /** Testable seam: process one raw payload and publish resulting offer. */
  async handleRequestPayload(message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[insight-runtime] non-JSON payload:", error);
      return;
    }

    const contract = validateInsightRequestV1Contract(parsed);
    if (!contract.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[insight-runtime] InsightRequestV1 contract rejected:",
        contract.errors.join("; "),
      );
      return;
    }

    const request = parsed as InsightRequestV1;
    this.stats.received += 1;

    const offer = await this.produceOffer(request);
    if (!offer) {
      return;
    }

    const payload = JSON.stringify(offer);
    try {
      const subscribers = await this.pub.publish(INSIGHT_OFFER, payload);
      this.stats.offered += 1;
      this.logger.info(
        `[insight-runtime] published offer ${offer.offer_id} for ${request.trigger_id} (${offer.choices.length} choices, ${subscribers} subs)`,
      );
    } catch (error) {
      this.logger.warn("[insight-runtime] publish failed:", error);
    }
  }

  private async produceOffer(request: InsightRequestV1): Promise<InsightOfferV1 | null> {
    let offer: InsightOfferV1 | null = null;
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(request) },
      ]);
      const normalized = normalizeLlmChatResult(result, this.model);
      offer = parseAgentOutput(normalized.content, request, this.logger);
    } catch (error) {
      this.stats.llmFailures += 1;
      this.logger.warn("[insight-runtime] LLM error:", error);
    }

    if (!offer) {
      this.stats.rejectedContract += 1;
      if (!this.publishEmptyOnFailure) return null;
      return emptyOffer(request);
    }

    const filtered = applyInsightArbiter(request, offer);
    if (filtered.choices.length === 0) {
      this.stats.rejectedArbiter += 1;
      if (!this.publishEmptyOnFailure) return null;
      return emptyOffer(request);
    }

    // Ensure stable offer_id / trigger_id
    return {
      ...filtered,
      offer_id: filtered.offer_id || generateOfferId(request),
      trigger_id: request.trigger_id,
    };
  }
}
