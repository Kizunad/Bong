import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type Narration,
  type NarrationStyle,
  type TiandaoHuntNarrationRequestV1,
  validateNarrationV1Contract,
  validateTiandaoHuntNarrationRequestV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { AGENT_NARRATE, TIANDAO_HUNT_NARRATION_REQUEST } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

const ALLOWED_STYLES = new Set<NarrationStyle>([
  "system_warning",
  "perception",
  "narration",
  "era_decree",
  "political_jianghu",
]);

export interface TiandaoHuntNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
}

export interface TiandaoHuntNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface TiandaoHuntNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: TiandaoHuntNarrationRuntimeClient;
  pub: TiandaoHuntNarrationRuntimeClient;
  logger?: TiandaoHuntNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface TiandaoHuntNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  rejectedOutput: number;
  llmFailures: number;
  fallbackUsed: number;
}

interface ParsedLlmNarration {
  text: string;
  style: NarrationStyle;
}

export class TiandaoHuntNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: TiandaoHuntNarrationRuntimeClient;
  private readonly pub: TiandaoHuntNarrationRuntimeClient;
  private readonly logger: TiandaoHuntNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: TiandaoHuntNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    rejectedOutput: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== TIANDAO_HUNT_NARRATION_REQUEST) return;
    void this.handleRequestPayload(message);
  };

  constructor(config: TiandaoHuntNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? readSkillPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(TIANDAO_HUNT_NARRATION_REQUEST);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(
      `[tiandao-hunt-narration-runtime] subscribed to ${TIANDAO_HUNT_NARRATION_REQUEST}`,
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
      this.logger.warn("[tiandao-hunt-narration-runtime] non-JSON payload:", error);
      return;
    }

    const contract = validateTiandaoHuntNarrationRequestV1Contract(parsed);
    if (!contract.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[tiandao-hunt-narration-runtime] TiandaoHuntNarrationRequestV1 rejected:",
        contract.errors.join("; "),
      );
      return;
    }

    const request = parsed as TiandaoHuntNarrationRequestV1;
    this.stats.received += 1;

    const narration = await this.produceNarration(request);
    const envelope = { v: 1 as const, narrations: [narration] };
    const outputContract = validateNarrationV1Contract(envelope);
    if (!outputContract.ok) {
      this.stats.rejectedOutput += 1;
      this.logger.warn(
        "[tiandao-hunt-narration-runtime] generated NarrationV1 rejected:",
        outputContract.errors.join("; "),
      );
      return;
    }

    try {
      const subscribers = await this.pub.publish(AGENT_NARRATE, JSON.stringify(envelope));
      this.stats.published += 1;
      this.logger.info(
        `[tiandao-hunt-narration-runtime] published ${request.response_level} narration for ${request.character_id} (${subscribers} subs)`,
      );
    } catch (error) {
      this.logger.warn("[tiandao-hunt-narration-runtime] publish failed:", error);
    }
  }

  private async produceNarration(
    request: TiandaoHuntNarrationRequestV1,
  ): Promise<Narration> {
    let parsed: ParsedLlmNarration | null = null;
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(request) },
      ]);
      parsed = parseLlmNarration(
        normalizeLlmChatResult(result, this.model).content,
        this.logger,
      );
    } catch (error) {
      this.stats.llmFailures += 1;
      this.logger.warn("[tiandao-hunt-narration-runtime] LLM error:", error);
    }

    if (!parsed) {
      this.stats.fallbackUsed += 1;
      parsed = fallbackParsedNarration(request);
    }

    return buildNarration(request, parsed);
  }
}

function readSkillPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "tiandao-hunt-narration.md"), "utf-8");
}

function parseLlmNarration(
  content: string,
  logger: TiandaoHuntNarrationRuntimeLogger,
): ParsedLlmNarration | null {
  const trimmed = content.trim();
  if (trimmed.length === 0) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    logger.warn("[tiandao-hunt-narration-runtime] LLM output is not valid JSON:", error);
    return null;
  }

  if (!isRecord(parsed) || typeof parsed.text !== "string") return null;
  const text = cleanText(parsed.text);
  if (!text) return null;

  const style =
    typeof parsed.style === "string" && ALLOWED_STYLES.has(parsed.style as NarrationStyle)
      ? (parsed.style as NarrationStyle)
      : null;
  if (!style) return null;

  return { text, style };
}

function buildNarration(
  request: TiandaoHuntNarrationRequestV1,
  parsed: ParsedLlmNarration,
): Narration {
  if (request.response_level === "tribulation" || request.response_level === "annihilate") {
    return {
      scope: "broadcast",
      text: cleanText(parsed.text),
      style: parsed.style,
    };
  }

  return {
    scope: "player",
    target: request.character_id,
    text: cleanText(parsed.text),
    style: parsed.style,
  };
}

function fallbackParsedNarration(
  request: TiandaoHuntNarrationRequestV1,
): ParsedLlmNarration {
  switch (request.response_level) {
    case "watch":
      return {
        text: `你行至${request.zone}，忽觉天色像停了一息。不是雷，也不是风，只是有东西在远处记下了你的名字。`,
        style: "perception",
      };
    case "pressure":
      return {
        text: `${request.zone}的灵机忽然压低，骨缝里泛起冷意。天道未落罚，只先把路收窄了。`,
        style: "system_warning",
      };
    case "tribulation":
      return {
        text: `${request.zone}上空一声闷响，尘光倒卷。有人被天道盯紧，灾劫已经起势。`,
        style: "system_warning",
      };
    case "annihilate":
      return {
        text: `${request.zone}的光色沉入灰白，像有一只无形的手抹去生机。天道已不再只是警告。`,
        style: "system_warning",
      };
  }

  return {
    text: `${request.zone}的气息忽然收紧，天道狩猎已有回应。`,
    style: "system_warning",
  };
}

function cleanText(text: string): string {
  return text.trim().replace(/\s+/g, " ").slice(0, 500);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
