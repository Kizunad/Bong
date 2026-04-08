import {
  ChatMessageV1,
  ChatSignal,
  type ChatIntent,
  type ChatMessageV1 as ChatMessage,
  type ChatSignal as ChatSignalRecord,
  validate,
} from "@bong/schema";

export interface ChatProcessorLogger {
  warn(message: string, details?: Record<string, unknown>): void;
}

export interface ChatAnnotator {
  annotate(messages: ChatMessage[]): Promise<ChatSignalRecord[]>;
}

const DEFAULT_LOGGER: ChatProcessorLogger = {
  warn(message, details) {
    console.warn(message, details ?? {});
  },
};

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

interface HeuristicHint {
  sentiment: number;
  intent: ChatIntent;
  mentionsMechanic?: string;
  influenceWeight: number;
}

const COMPLAINT_HINTS = ["太", "太少", "不够", "垃圾", "坑", "难受", "崩", "卡", "痛苦"];
const HELP_HINTS = ["救", "帮", "怎么", "求", "不会", "求助", "求带", "请问"];
const PROVOKE_HINTS = ["来战", "敢不敢", "单挑", "废物", "怂", "打你", "挑衅"];
const BOAST_HINTS = ["我无敌", "我最强", "轻松", "拿下", "乱杀", "稳赢"];
const SOCIAL_HINTS = ["哈哈", "hi", "hello", "早", "晚安", "谢谢", "好耶"];

const MECHANIC_HINTS: Array<{ keyword: string; mechanic: string }> = [
  { keyword: "灵气", mechanic: "spirit_qi" },
  { keyword: "karma", mechanic: "karma" },
  { keyword: "功德", mechanic: "karma" },
  { keyword: "劫", mechanic: "event" },
  { keyword: "兽潮", mechanic: "event" },
  { keyword: "区域", mechanic: "zone" },
  { keyword: "zone", mechanic: "zone" },
  { keyword: "npc", mechanic: "npc" },
];

function inferHeuristic(raw: string): HeuristicHint {
  const lower = raw.toLowerCase();

  let intent: ChatIntent = "unknown";
  let sentiment = 0;
  let influenceWeight = 0.35;

  if (COMPLAINT_HINTS.some((token) => raw.includes(token) || lower.includes(token.toLowerCase()))) {
    intent = "complaint";
    sentiment = -0.65;
    influenceWeight = 0.85;
  } else if (HELP_HINTS.some((token) => raw.includes(token) || lower.includes(token.toLowerCase()))) {
    intent = "help";
    sentiment = -0.15;
    influenceWeight = 0.75;
  } else if (PROVOKE_HINTS.some((token) => raw.includes(token) || lower.includes(token.toLowerCase()))) {
    intent = "provoke";
    sentiment = -0.45;
    influenceWeight = 0.65;
  } else if (BOAST_HINTS.some((token) => raw.includes(token) || lower.includes(token.toLowerCase()))) {
    intent = "boast";
    sentiment = 0.45;
    influenceWeight = 0.55;
  } else if (SOCIAL_HINTS.some((token) => raw.includes(token) || lower.includes(token.toLowerCase()))) {
    intent = "social";
    sentiment = 0.2;
    influenceWeight = 0.25;
  }

  if (raw.includes("!")) {
    influenceWeight += 0.1;
  }
  if (raw.includes("？") || raw.includes("?")) {
    influenceWeight += 0.05;
  }

  const mechanic = MECHANIC_HINTS.find((entry) =>
    raw.includes(entry.keyword) || lower.includes(entry.keyword.toLowerCase()),
  )?.mechanic;

  return {
    sentiment: clamp(sentiment, -1, 1),
    intent,
    mentionsMechanic: mechanic,
    influenceWeight: clamp(influenceWeight, 0, 1),
  };
}

export class FakeHeuristicChatAnnotator implements ChatAnnotator {
  async annotate(messages: ChatMessage[]): Promise<ChatSignalRecord[]> {
    return messages.map((message) => {
      const hint = inferHeuristic(message.raw);
      return {
        player: message.player,
        raw: message.raw,
        sentiment: hint.sentiment,
        intent: hint.intent,
        mentions_mechanic: hint.mentionsMechanic,
        influence_weight: hint.influenceWeight,
      };
    });
  }
}

export interface ProcessedChatBatch {
  messages: ChatMessage[];
  signals: ChatSignalRecord[];
}

export class ChatProcessor {
  constructor(
    private readonly annotator: ChatAnnotator = new FakeHeuristicChatAnnotator(),
    private readonly logger: ChatProcessorLogger = DEFAULT_LOGGER,
  ) {}

  async processRawEntries(rawEntries: string[]): Promise<ProcessedChatBatch> {
    const messages = this.parseAndValidate(rawEntries);
    if (messages.length === 0) {
      return {
        messages: [],
        signals: [],
      };
    }

    const annotated = await this.annotator.annotate(messages);
    const signals = this.validateAnnotatedSignals(annotated);

    return {
      messages,
      signals,
    };
  }

  private parseAndValidate(rawEntries: string[]): ChatMessage[] {
    const validMessages: ChatMessage[] = [];

    for (let i = 0; i < rawEntries.length; i++) {
      const rawEntry = rawEntries[i];

      let parsed: unknown;
      try {
        parsed = JSON.parse(rawEntry);
      } catch (error) {
        this.logger.warn("[chat-processor] drop invalid chat json", {
          reason: "invalid_json",
          index: i,
          error: error instanceof Error ? error.message : String(error),
          raw: rawEntry,
        });
        continue;
      }

      const validation = validate(ChatMessageV1, parsed);
      if (!validation.ok) {
        this.logger.warn("[chat-processor] drop schema-invalid chat message", {
          reason: "schema_invalid",
          index: i,
          errors: validation.errors,
          raw: rawEntry,
        });
        continue;
      }

      validMessages.push(parsed as ChatMessage);
    }

    return validMessages;
  }

  private validateAnnotatedSignals(signals: ChatSignalRecord[]): ChatSignalRecord[] {
    const validSignals: ChatSignalRecord[] = [];

    for (let i = 0; i < signals.length; i++) {
      const signal = signals[i];
      const validation = validate(ChatSignal, signal);

      if (!validation.ok) {
        this.logger.warn("[chat-processor] drop schema-invalid chat signal", {
          reason: "signal_schema_invalid",
          index: i,
          errors: validation.errors,
          signal,
        });
        continue;
      }

      validSignals.push(signal);
    }

    return validSignals;
  }
}
