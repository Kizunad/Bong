/**
 * 死亡遗念 runtime — 事件驱动（plan-death-lifecycle-v1 §6 / §7）。
 *
 * 订阅 `bong:death_insight`，生成面向死者本人的 `NarrationV1`，
 * 再发布到 `bong:agent_narrate`。LLM 输出只采纳文本，scope/target 由本 runtime
 * 强制覆盖，避免模型误广播或误投递到其他玩家。
 */

import {
  CHANNELS,
  MAX_NARRATION_LENGTH,
  type DeathInsightRequestV1,
  type Narration,
  type NarrationStyle,
  validateDeathInsightRequestV1Contract,
  validateNarrationV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { AGENT_NARRATE, DEATH_INSIGHT } = CHANNELS;

const DEATH_INSIGHT_SYSTEM_PROMPT = `你是 Bong 的天道遗念记录者。根据 DeathInsightRequestV1 生成一条死亡瞬间遗念。只输出 JSON：{"text":"...","style":"perception"}。信息必须真实、冷漠、具体；不要编造请求中没有的坐标、敌人或掉落。若 known_spirit_eyes 非空，把已知灵眼坐标当作临死遗念中的情报残页，不要广播给其他玩家。醒灵/引气短句，凝脉/固元 1-2 句，通灵/化虚可含天道评语。劫数期必须写明此次运数概率；老死必须回顾 recent_biography。`;

export interface DeathInsightRuntimeLogger {
  info: (...args: unknown[]) => void;
  warn: (...args: unknown[]) => void;
  error?: (...args: unknown[]) => void;
}

export interface DeathInsightRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface DeathInsightRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: DeathInsightRuntimeClient;
  pub: DeathInsightRuntimeClient;
  logger?: DeathInsightRuntimeLogger;
  systemPrompt?: string;
}

export interface DeathInsightRuntimeStats {
  received: number;
  narrated: number;
  rejectedContract: number;
  rejectedOutput: number;
  llmFailures: number;
}

interface ParsedNarrationText {
  text: string;
  style?: NarrationStyle;
}

export class DeathInsightRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: DeathInsightRuntimeClient;
  private readonly pub: DeathInsightRuntimeClient;
  private readonly logger: DeathInsightRuntimeLogger;
  private readonly systemPrompt: string;
  private connected = false;

  readonly stats: DeathInsightRuntimeStats = {
    received: 0,
    narrated: 0,
    rejectedContract: 0,
    rejectedOutput: 0,
    llmFailures: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== DEATH_INSIGHT) return;
    void this.handleRequestPayload(message);
  };

  constructor(config: DeathInsightRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
    this.systemPrompt = config.systemPrompt ?? DEATH_INSIGHT_SYSTEM_PROMPT;
  }

  async connect(): Promise<void> {
    if (this.connected) return;
    await this.sub.subscribe(DEATH_INSIGHT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    this.logger.info(`[death-insight-runtime] subscribed to ${DEATH_INSIGHT}`);
  }

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  /** Testable seam: process one raw payload and publish resulting narration. */
  async handleRequestPayload(message: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[death-insight-runtime] non-JSON payload:", error);
      return;
    }

    const contract = validateDeathInsightRequestV1Contract(parsed);
    if (!contract.ok) {
      this.stats.rejectedContract += 1;
      this.logger.warn(
        "[death-insight-runtime] DeathInsightRequestV1 contract rejected:",
        contract.errors.join("; "),
      );
      return;
    }

    const request = parsed as DeathInsightRequestV1;
    this.stats.received += 1;

    const narration = await this.produceNarration(request);
    const validation = validateNarrationV1Contract({ v: 1, narrations: [narration] });
    if (!validation.ok) {
      this.stats.rejectedOutput += 1;
      this.logger.warn(
        "[death-insight-runtime] generated NarrationV1 contract rejected:",
        validation.errors.join("; "),
      );
      return;
    }

    try {
      const subscribers = await this.pub.publish(
        AGENT_NARRATE,
        JSON.stringify({ v: 1, narrations: [narration] }),
      );
      this.stats.narrated += 1;
      this.logger.info(
        `[death-insight-runtime] published death insight ${request.request_id} (${subscribers} subs)`,
      );
    } catch (error) {
      this.logger.warn("[death-insight-runtime] publish failed:", error);
    }
  }

  private async produceNarration(request: DeathInsightRequestV1): Promise<Narration> {
    let candidate: ParsedNarrationText | null = null;
    try {
      const result = await this.llm.chat(this.model, [
        { role: "system", content: this.systemPrompt },
        { role: "user", content: JSON.stringify(request) },
      ]);
      const normalized = normalizeLlmChatResult(result, this.model);
      candidate = parseAgentNarrationOutput(normalized.content, this.logger);
    } catch (error) {
      this.stats.llmFailures += 1;
      this.logger.warn("[death-insight-runtime] LLM error:", error);
    }

    if (!candidate) {
      this.stats.rejectedOutput += 1;
      candidate = {
        text: buildFallbackDeathInsightText(request),
        style: selectFallbackStyle(request),
      };
    }

    return {
      scope: "player",
      target: request.character_id,
      text: clampNarrationText(candidate.text),
      style: candidate.style ?? selectFallbackStyle(request),
      kind: "death_insight",
    };
  }
}

function parseAgentNarrationOutput(
  content: string,
  logger: DeathInsightRuntimeLogger,
): ParsedNarrationText | null {
  const trimmed = content.trim();
  if (trimmed.length === 0) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    logger.warn("[death-insight-runtime] LLM output is not valid JSON:", error);
    return null;
  }

  if (typeof parsed === "string") {
    return parsed.length > 0 ? { text: parsed } : null;
  }

  if (!isRecord(parsed)) {
    return null;
  }

  if (typeof parsed.text === "string" && parsed.text.trim().length > 0) {
    return {
      text: parsed.text,
      style: normalizeNarrationStyle(parsed.style),
    };
  }

  if (Array.isArray(parsed.narrations)) {
    for (const entry of parsed.narrations) {
      if (isRecord(entry) && typeof entry.text === "string" && entry.text.trim().length > 0) {
        return {
          text: entry.text,
          style: normalizeNarrationStyle(entry.style),
        };
      }
    }
  }

  return null;
}

function buildFallbackDeathInsightText(request: DeathInsightRequestV1): string {
  if (request.category === "natural") {
    return buildNaturalDeathInsight(request);
  }

  if (isTribulationInsight(request)) {
    return buildTribulationDeathInsight(request);
  }

  if (isLowRealm(request.realm)) {
    return clampNarrationText(`${formatCause(request.cause)}处，有冷光一闪。`, 20);
  }

  const location = formatLocation(request);
  const cause = formatCause(request.cause);
  const spiritEyeTrace = formatKnownSpiritEyeTrace(request);
  if (request.realm === "Spirit" || request.realm === "Void") {
    const bio = formatBiographyTail(request, 2);
    return clampNarrationText(
      `你死于${location}，死因：${cause}。${bio}${spiritEyeTrace}天道只记因果，不记哀荣。`,
    );
  }

  return clampNarrationText(`你死于${location}，死因：${cause}。${spiritEyeTrace}临灭前，周遭灵机已偏向${zoneKindLabel(request.zone_kind)}。`);
}

function buildNaturalDeathInsight(request: DeathInsightRequestV1): string {
  const realm = request.realm ?? request.player_realm ?? "未明境界";
  const remaining =
    typeof request.lifespan_remaining_years === "number"
      ? `余寿 ${request.lifespan_remaining_years.toFixed(1)} 年。`
      : "寿数已尽。";
  const bio = formatBiographyTail(request, 4);
  const spiritEyeTrace = formatKnownSpiritEyeTrace(request);
  return clampNarrationText(`寿火已尽，${realm}一生至此合卷。${remaining}${bio}${spiritEyeTrace}天道不添悲喜，只留此页。`);
}

function buildTribulationDeathInsight(request: DeathInsightRequestV1): string {
  const chance = formatChance(request.rebirth_chance);
  const spiritEyeTrace = formatKnownSpiritEyeTrace(request);
  const finalWords = isWillTerminate(request)
    ? "终焉之言：命薄至此，非天所夺，是因果自尽。"
    : "你以为这是天道的怜悯？下次只会更薄。";
  return clampNarrationText(
    `劫数临身，死因：${formatCause(request.cause)}。此次运数：${chance}。${spiritEyeTrace}${finalWords}`,
  );
}

function formatBiographyTail(request: DeathInsightRequestV1, maxItems: number): string {
  const entries = request.recent_biography
    .filter((entry) => entry.trim().length > 0)
    .slice(-maxItems);
  if (entries.length === 0) {
    return "生平卷空白。";
  }

  return `生平残页：${entries.join("；")}。`;
}

function formatLocation(request: DeathInsightRequestV1): string {
  if (request.position) {
    return `${zoneKindLabel(request.zone_kind)}(${formatCoord(request.position.x)}, ${formatCoord(request.position.y)}, ${formatCoord(request.position.z)})`;
  }
  return zoneKindLabel(request.zone_kind);
}

function formatCause(cause: string): string {
  return cause.replace(/^cultivation:/, "修炼-").replace(/^combat:/, "战斗-");
}

function formatChance(chance: number | undefined): string {
  if (typeof chance !== "number" || !Number.isFinite(chance)) {
    return "未知";
  }
  return `${Math.round(chance * 100)}%`;
}

function formatKnownSpiritEyeTrace(request: DeathInsightRequestV1): string {
  const eye = request.known_spirit_eyes?.[0];
  if (!eye) {
    return "";
  }
  const zone = eye.zone ? `${eye.zone}` : "未知地";
  return `遗念残页：灵眼 ${eye.eye_id} 在${zone}(${formatCoord(eye.pos.x)}, ${formatCoord(eye.pos.y)}, ${formatCoord(eye.pos.z)})。`;
}

function formatCoord(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function zoneKindLabel(kind: DeathInsightRequestV1["zone_kind"]): string {
  switch (kind) {
    case "death":
      return "死域";
    case "negative":
      return "负灵域";
    case "ordinary":
      return "凡域";
  }
}

function selectFallbackStyle(request: DeathInsightRequestV1): NarrationStyle {
  return isTribulationInsight(request) && isWillTerminate(request) ? "era_decree" : "perception";
}

function normalizeNarrationStyle(value: unknown): NarrationStyle | undefined {
  if (value === "perception" || value === "era_decree") {
    return value;
  }
  return undefined;
}

function clampNarrationText(text: string, maxLength = MAX_NARRATION_LENGTH): string {
  return [...text.trim()].slice(0, maxLength).join("");
}

function isLowRealm(realm: DeathInsightRequestV1["realm"]): boolean {
  return realm === "Awaken" || realm === "Induce" || realm === undefined;
}

function isTribulationInsight(request: DeathInsightRequestV1): boolean {
  return request.category === "tribulation" || request.death_count >= 4;
}

function isWillTerminate(request: DeathInsightRequestV1): boolean {
  return isRecord(request.context) && request.context.will_terminate === true;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
