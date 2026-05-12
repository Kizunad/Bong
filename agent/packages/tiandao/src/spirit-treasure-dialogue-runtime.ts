import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type SpiritTreasureDialogueToneV1,
  type SpiritTreasureDialogueRequestV1,
  type SpiritTreasureDialogueV1,
  validateSpiritTreasureDialogueRequestV1Contract,
  validateSpiritTreasureDialogueV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { SPIRIT_TREASURE_DIALOGUE_REQUEST, SPIRIT_TREASURE_DIALOGUE } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

const JIZHAOJING_TREASURE_ID = "spirit_treasure_jizhaojing";

export interface SpiritTreasureDialogueRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
}

export interface SpiritTreasureDialogueRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface SpiritTreasureDialogueRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: SpiritTreasureDialogueRuntimeClient;
  pub: SpiritTreasureDialogueRuntimeClient;
  logger?: SpiritTreasureDialogueRuntimeLogger;
  systemPrompt?: string;
}

export interface SpiritTreasureDialogueRuntimeStats {
  received: number;
  replied: number;
  rejectedContract: number;
  rejectedOutput: number;
  llmFailures: number;
  fallbackUsed: number;
}

interface ParsedDialogue {
  text: string;
  tone?: SpiritTreasureDialogueToneV1;
  affinity_delta?: number;
}

export class SpiritTreasureDialogueRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: SpiritTreasureDialogueRuntimeClient;
  private readonly pub: SpiritTreasureDialogueRuntimeClient;
  private readonly logger: SpiritTreasureDialogueRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: SpiritTreasureDialogueRuntimeStats = {
    received: 0,
    replied: 0,
    rejectedContract: 0,
    rejectedOutput: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== SPIRIT_TREASURE_DIALOGUE_REQUEST) return;
    void this.handleRequestPayload(message);
  };

  constructor(config: SpiritTreasureDialogueRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? readSkillPrompt("spirit-treasure-jizhaojing.md");
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(SPIRIT_TREASURE_DIALOGUE_REQUEST);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(
      `[spirit-treasure-runtime] subscribed to ${SPIRIT_TREASURE_DIALOGUE_REQUEST}`,
    );
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
      this.logger.warn("[spirit-treasure-runtime] non-JSON payload:", error);
      return;
    }

    const contract = validateSpiritTreasureDialogueRequestV1Contract(parsed);
    if (!contract.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[spirit-treasure-runtime] SpiritTreasureDialogueRequestV1 rejected:",
        contract.errors.join("; "),
      );
      return;
    }

    const request = parsed as SpiritTreasureDialogueRequestV1;
    this.stats.received += 1;

    const dialogue = await this.produceDialogue(request);
    const validation = validateSpiritTreasureDialogueV1Contract(dialogue);
    if (!validation.ok) {
      this.stats.rejectedOutput += 1;
      this.logger.warn(
        "[spirit-treasure-runtime] generated SpiritTreasureDialogueV1 rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    try {
      const subscribers = await this.pub.publish(
        SPIRIT_TREASURE_DIALOGUE,
        JSON.stringify(dialogue),
      );
      this.stats.replied += 1;
      this.logger.info(
        `[spirit-treasure-runtime] published dialogue ${request.request_id} (${subscribers} subs)`,
      );
    } catch (error) {
      this.logger.warn("[spirit-treasure-runtime] publish failed:", error);
    }
  }

  private async produceDialogue(
    request: SpiritTreasureDialogueRequestV1,
  ): Promise<SpiritTreasureDialogueV1> {
    let parsed: ParsedDialogue | null = null;
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(request) },
      ]);
      parsed = parseDialogueOutput(
        normalizeLlmChatResult(result, this.model).content,
        this.logger,
      );
    } catch (error) {
      this.stats.llmFailures += 1;
      this.logger.warn("[spirit-treasure-runtime] LLM error:", error);
    }

    if (!parsed) {
      this.stats.fallbackUsed += 1;
      parsed = fallbackDialogue(request);
    }

    return {
      v: 1,
      request_id: request.request_id,
      character_id: request.character_id,
      treasure_id: request.treasure_id,
      text: cleanText(parsed.text, fallbackDialogue(request).text, 180),
      tone: parsed.tone ?? fallbackTone(request),
      affinity_delta: clamp(parsed.affinity_delta ?? 0, -0.1, 0.1),
    };
  }
}

function readSkillPrompt(fileName: string): string {
  const candidates = [
    resolve(__dirname, "skills", fileName),
    resolve(__dirname, "../src/skills", fileName),
  ];
  let lastError: unknown;
  for (const path of candidates) {
    try {
      return readFileSync(path, "utf-8");
    } catch (error) {
      lastError = error;
    }
  }
  throw lastError;
}

function parseDialogueOutput(
  content: string,
  logger: SpiritTreasureDialogueRuntimeLogger,
): ParsedDialogue | null {
  const trimmed = content.trim();
  if (trimmed.length === 0) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    logger.warn("[spirit-treasure-runtime] LLM output is not valid JSON:", error);
    return null;
  }

  if (typeof parsed === "string") {
    return parsed.trim().length > 0 ? { text: parsed } : null;
  }
  if (!isRecord(parsed) || typeof parsed.text !== "string") {
    return null;
  }

  return {
    text: parsed.text,
    tone: normalizeTone(parsed.tone),
    affinity_delta: typeof parsed.affinity_delta === "number" ? parsed.affinity_delta : undefined,
  };
}

function fallbackDialogue(request: SpiritTreasureDialogueRequestV1): ParsedDialogue {
  const playerMessage = request.player_message?.trim();
  if (request.treasure_id !== JIZHAOJING_TREASURE_ID) {
    return {
      text: "器灵没有应声，只在法器深处留下一点冷光。",
      tone: "silent",
      affinity_delta: 0,
    };
  }
  if (!request.context.equipped) {
    return {
      text: "镜在囊中轻响：带我在身，再问。",
      tone: "warning",
      affinity_delta: -0.01,
    };
  }
  if (playerMessage && /哪|何|谁|什么|如何|可见|可知|？|\?/.test(playerMessage)) {
    return {
      text: "镜面微冷，只照出近处灵机正往低处流，别把它当成答案。",
      tone: "curious",
      affinity_delta: 0.01,
    };
  }
  return {
    text: "明虚在镜里低笑一声，没答，只把一线寒光落在你脚边。",
    tone: fallbackTone(request),
    affinity_delta: 0,
  };
}

function fallbackTone(
  request: SpiritTreasureDialogueRequestV1,
): SpiritTreasureDialogueToneV1 {
  if (request.context.affinity < 0.25) return "cold";
  if (!request.context.equipped) return "warning";
  return "curious";
}

function normalizeTone(value: unknown): SpiritTreasureDialogueToneV1 | undefined {
  switch (value) {
    case "cold":
    case "curious":
    case "warning":
    case "amused":
    case "silent":
      return value;
    default:
      return undefined;
  }
}

function cleanText(value: string, fallback: string, maxLength: number): string {
  const trimmed = value.trim();
  const text = trimmed.length > 0 ? trimmed : fallback;
  return Array.from(text).slice(0, maxLength).join("");
}

function clamp(value: number, min: number, max: number): number {
  return Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
