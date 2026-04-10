import { ChatMessageV1, ChatSignal, validate, type ChatIntent } from "@bong/schema";
import type { LlmClient } from "./llm.js";

const CHAT_CONTEXT_WINDOW_SECONDS = 5 * 60;
const CHAT_CONTEXT_MAX_SIGNALS = 20;

export const DEFAULT_CHAT_SIGNAL: Pick<ChatSignal, "sentiment" | "intent" | "influence_weight"> = {
  sentiment: 0,
  intent: "unknown",
  influence_weight: 0,
};

export interface ChatSignalInput {
  player: string;
  zone: string;
  raw: string;
  sentiment: number;
  intent: ChatIntent;
  influence_weight: number;
  mentions_mechanic?: string;
}

export interface ProcessChatBatchOptions {
  messages: ChatMessageV1[];
  llmClient: LlmClient;
  model: string;
  logger: Pick<typeof console, "warn">;
}

export interface BuildChatSignalsBlockArgs {
  signals: ChatSignal[];
  nowSeconds: number;
}

export function parseChatMessages(rawMessages: string[], logger: Pick<typeof console, "warn">): ChatMessageV1[] {
  const messages: ChatMessageV1[] = [];

  for (const payload of rawMessages) {
    try {
      const parsed = JSON.parse(payload) as unknown;
      const validation = validate(ChatMessageV1, parsed);
      if (!validation.ok) {
        logger.warn(`[chat-processor] dropped invalid chat payload: ${validation.errors.join("; ")}`);
        continue;
      }
      messages.push(parsed as ChatMessageV1);
    } catch {
      logger.warn("[chat-processor] dropped malformed chat payload");
    }
  }

  return messages;
}

export function parseChatSignalBatch(raw: string, logger: Pick<typeof console, "warn">): ChatSignalInput[] {
  const jsonMatch = raw.match(/```(?:json)?\s*\n?([\s\S]*?)\n?```/);
  const jsonStr = jsonMatch ? jsonMatch[1] : raw;

  let parsed: unknown;
  try {
    parsed = JSON.parse(jsonStr.trim());
  } catch {
    logger.warn("[chat-processor] failed to parse chat signal batch JSON");
    throw new Error("failed to parse chat signal batch JSON");
  }

  if (!Array.isArray(parsed)) {
    logger.warn("[chat-processor] chat signal batch is not an array");
    throw new Error("chat signal batch is not an array");
  }

  const rows: ChatSignalInput[] = [];
  for (const item of parsed) {
    if (!isChatSignalInput(item)) {
      continue;
    }

    rows.push({
      player: item.player,
      zone: item.zone,
      raw: item.raw,
      sentiment: item.sentiment,
      intent: item.intent,
      influence_weight: item.influence_weight,
      mentions_mechanic: item.mentions_mechanic,
    });
  }

  return rows;
}

export async function processChatBatch(options: ProcessChatBatchOptions): Promise<ChatSignal[]> {
  const { messages, llmClient, model, logger } = options;
  if (messages.length === 0) {
    return [];
  }

  const prompt = buildAnnotatePrompt(messages);
  const raw = await llmClient.chat(
    model,
    [
      {
        role: "system",
        content:
          "你是聊天信号标注器。严格输出 JSON 数组，每项字段: player, zone, raw, sentiment(-1~1), intent(complaint|boast|social|help|provoke|unknown), influence_weight(0~1)。不得输出解释。",
      },
      {
        role: "user",
        content: prompt,
      },
    ],
  );

  const batch = parseChatSignalBatch(raw, logger);
  const byKey = new Map(batch.map((entry) => [chatKey(entry.player, entry.zone, entry.raw), entry]));

  const signals: ChatSignal[] = [];
  for (const msg of messages) {
    const fallback: ChatSignal = {
      player: msg.player,
      raw: msg.raw,
      sentiment: DEFAULT_CHAT_SIGNAL.sentiment,
      intent: DEFAULT_CHAT_SIGNAL.intent,
      influence_weight: DEFAULT_CHAT_SIGNAL.influence_weight,
    };

    const picked = byKey.get(chatKey(msg.player, msg.zone, msg.raw));
    if (!picked) {
      signals.push(fallback);
      continue;
    }

    const candidate: ChatSignal = {
      player: picked.player,
      raw: picked.raw,
      sentiment: picked.sentiment,
      intent: picked.intent,
      influence_weight: picked.influence_weight,
      mentions_mechanic: picked.mentions_mechanic,
    };

    const validation = validate(ChatSignal, candidate);
    if (!validation.ok) {
      logger.warn(
        `[chat-processor] invalid chat signal, fallback to unknown: ${validation.errors.join("; ")}`,
      );
      signals.push(fallback);
      continue;
    }

    signals.push(candidate);
  }

  return signals;
}

export function mergeChatSignals(
  existingSignals: ChatSignal[],
  incomingSignals: ChatSignal[],
  nowSeconds: number,
): ChatSignal[] {
  const recentExisting = selectRecentSignals(existingSignals, nowSeconds);
  const recentIncoming = selectRecentSignals(incomingSignals, nowSeconds);
  const merged = [...recentExisting, ...recentIncoming];
  if (merged.length <= CHAT_CONTEXT_MAX_SIGNALS) {
    return merged;
  }
  return merged.slice(-CHAT_CONTEXT_MAX_SIGNALS);
}

export function selectRecentSignals(signals: ChatSignal[], nowSeconds: number): ChatSignal[] {
  return signals.filter((signal) => isRecentSignal(signal, nowSeconds));
}

export function isRecentSignal(signal: ChatSignal, nowSeconds: number): boolean {
  const observedTs = extractSignalTimestamp(signal);
  if (observedTs === null) {
    return true;
  }
  return observedTs >= nowSeconds - CHAT_CONTEXT_WINDOW_SECONDS;
}

export function buildChatSignalsBlock(args: BuildChatSignalsBlockArgs): string {
  const recentSignals = selectRecentSignals(args.signals, args.nowSeconds).slice(-5);
  if (recentSignals.length === 0) {
    return "";
  }

  const totalSentiment = recentSignals.reduce((sum, signal) => sum + signal.sentiment, 0);
  const avgSentiment = totalSentiment / recentSignals.length;
  const trendLabel = avgSentiment > 0.2 ? "偏正面" : avgSentiment < -0.2 ? "偏负面" : "中性";

  const lines = recentSignals.map((signal) => {
    return `- ${signal.player}: ${signal.raw} (intent=${signal.intent}, sentiment=${signal.sentiment.toFixed(2)}, weight=${signal.influence_weight.toFixed(2)})`;
  });

  return `## 近期民意 (最近 5 分钟)\n${lines.join("\n")}\n民意倾向: ${trendLabel} (${avgSentiment.toFixed(2)})`;
}

function buildAnnotatePrompt(messages: ChatMessageV1[]): string {
  const serialized = messages.map((message) => ({
    player: message.player,
    zone: message.zone,
    raw: message.raw,
  }));

  return ["请批量标注以下玩家聊天。", "只输出 JSON 数组，不要 markdown，不要解释。", JSON.stringify(serialized)].join(
    "\n",
  );
}

function isChatSignalInput(input: unknown): input is ChatSignalInput {
  if (!input || typeof input !== "object") {
    return false;
  }

  const row = input as Record<string, unknown>;
  if (typeof row.player !== "string") {
    return false;
  }
  if (typeof row.zone !== "string") {
    return false;
  }
  if (typeof row.raw !== "string") {
    return false;
  }
  if (typeof row.sentiment !== "number" || !Number.isFinite(row.sentiment)) {
    return false;
  }
  if (typeof row.intent !== "string") {
    return false;
  }
  if (typeof row.influence_weight !== "number" || !Number.isFinite(row.influence_weight)) {
    return false;
  }
  if (typeof row.mentions_mechanic !== "undefined" && typeof row.mentions_mechanic !== "string") {
    return false;
  }

  return true;
}

function chatKey(player: string, zone: string, raw: string): string {
  return `${player}|${zone}|${raw}`;
}

function extractSignalTimestamp(signal: ChatSignal): number | null {
  if (typeof signal.mentions_mechanic !== "string") {
    return null;
  }

  const matched = signal.mentions_mechanic.match(/(?:^|;)ts:(\d+)(?:;|$)/);
  if (!matched) {
    return null;
  }

  const parsed = Number.parseInt(matched[1], 10);
  if (!Number.isFinite(parsed)) {
    return null;
  }

  return parsed;
}
