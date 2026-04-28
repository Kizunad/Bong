import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CHANNELS,
  type Narration,
  type TribulationEventV1,
  validateNarrationV1Contract,
  validateTribulationEventV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { TRIBULATION, AGENT_NARRATE } = CHANNELS;
const __dirname = dirname(fileURLToPath(import.meta.url));

export interface TribulationNarrationRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
}

export interface TribulationNarrationRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface TribulationNarrationRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: TribulationNarrationRuntimeClient;
  pub: TribulationNarrationRuntimeClient;
  logger?: TribulationNarrationRuntimeLogger;
  systemPrompt?: string;
}

export interface TribulationNarrationRuntimeStats {
  received: number;
  published: number;
  rejectedContract: number;
  llmFailures: number;
  fallbackUsed: number;
}

function defaultSystemPrompt(): string {
  return readFileSync(resolve(__dirname, "skills", "tribulation.md"), "utf-8");
}

function narrationTarget(payload: TribulationEventV1): string {
  const charId = payload.result?.char_id ?? payload.char_id ?? "unknown";
  const phase = payload.phase.kind === "wave" ? `wave:${payload.phase.wave}` : payload.phase.kind;
  if (payload.kind === "zone_collapse") return `tribulation:zone_collapse|zone:${payload.zone ?? "unknown"}|${phase}`;
  if (payload.kind === "targeted") return `tribulation:targeted|zone:${payload.zone ?? "unknown"}|${phase}`;
  return `tribulation:${payload.kind}|char:${charId}|${phase}`;
}

function actorLabel(payload: TribulationEventV1): string {
  return payload.actor_name ?? payload.char_id ?? payload.result?.char_id ?? "某人";
}

function fallbackNarration(payload: TribulationEventV1): Narration {
  if (payload.kind === "zone_collapse") {
    const zone = payload.zone ?? "无名之地";
    const text = payload.phase.kind === "settle"
      ? `${zone} 灵机断绝，域崩已成，未退者皆归死寂。`
      : `${zone} 灵气低伏，灰风先起，此地将崩，尚有片刻可退。`;
    return {
      scope: "broadcast",
      target: narrationTarget(payload),
      text,
      style: "narration",
    };
  }
  if (payload.kind === "targeted") {
    const zone = payload.zone ?? "附近";
    return {
      scope: "zone",
      target: zone,
      text: `${zone} 近日运道不佳，灵机一动便多一分折耗。`,
      style: "narration",
    };
  }
  const actor = actorLabel(payload);
  let text = `${actor} 的渡虚劫有异动，雷声压低，旁人只宜退远。`;
  switch (payload.phase.kind) {
    case "omen":
      text = `${actor} 欲渡虚劫，天色先暗，雷意未落，四野已知不可近。`;
      break;
    case "lock":
      text = `${actor} 已入劫锁，退路断尽，旁观者但见风雷收窄。`;
      break;
    case "wave":
      text = `${actor} 扛过第 ${payload.phase.wave} 道劫雷，气海震荡，命数仍未定。`;
      break;
    case "heart_demon":
      text = `${actor} 心魔映照，旧债无声浮起，劫云反倒更静。`;
      break;
    case "settle":
      text = settlementText(payload);
      break;
  }
  return {
    scope: "broadcast",
    target: narrationTarget(payload),
    text,
    style: "narration",
  };
}

function settlementText(payload: TribulationEventV1): string {
  const result = payload.result;
  const actor = actorLabel(payload);
  if (!result) {
    return `${actor} 的渡虚劫散去，余雷在地上停了片刻。`;
  }
  switch (result.outcome) {
    case "ascended":
      return `${actor} 历尽 ${result.waves_survived} 道劫雷，终入化虚，天地并不称贺。`;
    case "half_step":
      return `${actor} 破劫未得天位，只成半步化虚，余生尚要向名额索债。`;
    case "failed":
      return `${actor} 渡虚劫败，雷息回落，经脉闭锁，境界退回通灵初期。`;
    case "killed":
      return `${actor} 死于劫中截胡，杀者 ${result.killer ?? "不明"} 得其遗物；天雷不辨勇怯，只记损益。`;
    case "fled":
      return `${actor} 逃离劫场，劫云记下此名，风声比雷声更冷。`;
  }
  return `${actor} 的渡虚劫散去，余雷在地上停了片刻。`;
}

function parseNarrationContent(content: string, payload: TribulationEventV1): Narration {
  const trimmed = content.trim();
  if (!trimmed) {
    return fallbackNarration(payload);
  }

  try {
    const parsed = JSON.parse(trimmed) as unknown;
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed) ||
      typeof (parsed as { text?: unknown }).text !== "string" ||
      typeof (parsed as { style?: unknown }).style !== "string"
    ) {
      return fallbackNarration(payload);
    }

    const first = parsed as { text: string; style: Narration["style"] };
    const narration: Narration = {
      scope: "broadcast",
      target: narrationTarget(payload),
      text: first.text,
      style: first.style,
    };
    const validation = validateNarrationV1Contract({
      v: 1,
      narrations: [narration],
    });
    return validation.ok ? narration : fallbackNarration(payload);
  } catch {
    return fallbackNarration(payload);
  }
}

export class TribulationNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: TribulationNarrationRuntimeClient;
  private readonly pub: TribulationNarrationRuntimeClient;
  private readonly logger: TribulationNarrationRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: TribulationNarrationRuntimeStats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    llmFailures: 0,
    fallbackUsed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== TRIBULATION) return;
    void this.handlePayload(message);
  };

  constructor(config: TribulationNarrationRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? defaultSystemPrompt();
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(TRIBULATION);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[tribulation-runtime] subscribed to ${TRIBULATION}`);
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
      this.logger.warn("[tribulation-runtime] non-JSON payload:", error);
      return;
    }

    const validation = validateTribulationEventV1Contract(parsed);
    if (!validation.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[tribulation-runtime] TribulationEventV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    const payload = parsed as TribulationEventV1;
    this.stats.received += 1;

    let narration = fallbackNarration(payload);
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(payload) },
      ]);
      narration = parseNarrationContent(normalizeLlmChatResult(result, this.model).content, payload);
      if (narration.text === fallbackNarration(payload).text) {
        this.stats.fallbackUsed += 1;
      }
    } catch (error) {
      this.stats.llmFailures += 1;
      this.stats.fallbackUsed += 1;
      this.logger.warn("[tribulation-runtime] LLM error:", error);
    }

    try {
      const subscribers = await this.pub.publish(
        AGENT_NARRATE,
        JSON.stringify({ v: 1, narrations: [narration] }),
      );
      this.stats.published += 1;
      this.logger.info(
        `[tribulation-runtime] published narration for ${narration.target} (${subscribers} subs)`,
      );
    } catch (error) {
      this.logger.warn("[tribulation-runtime] publish failed:", error);
    }
  }
}
