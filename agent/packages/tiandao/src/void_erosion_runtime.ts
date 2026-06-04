import {
  CHANNELS,
  type Narration,
  type VoidErosionEventV1,
  validateVoidErosionEventV1Contract,
} from "@bong/schema";

import type { LlmClient } from "./llm.js";
import { normalizeLlmChatResult } from "./llm.js";

const { AGENT_NARRATE, VOID_EROSION_EVENT } = CHANNELS;

export interface VoidErosionRuntimeClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
}

export interface VoidErosionRuntimeConfig {
  llm: LlmClient;
  model: string;
  sub: VoidErosionRuntimeClient;
  pub: VoidErosionRuntimeClient;
  logger?: Pick<Console, "info" | "warn">;
}

// ── 阶段叙事文案（fallback，按 to_stage 选择） ────────────────────────────────

const STAGE_NARRATION: Record<string, string> = {
  low_pressure: "灵压开始偏移，虚蚀已入第一阶——你察觉到修炼场域有些格格不入。",
  void_shadow:  "虚蚀第二阶，你的身影在某些角度开始显得不太真实，旁人难以直视。",
  echo_body:    "虚蚀第三阶——涡流回响开始失控，你的影子在空间里多了一个。灵压渐移，你感受到身影正在薄化。",
  void_eroded:  "虚蚀第四阶，你已完全进入虚蚀态。天道几乎无法追踪你，但这意味着你也在失去「存在于此间」的锚点。",
};

/**
 * 根据虚蚀阶段推进事件生成叙事，供 fallback 或 LLM 失败时使用。
 */
export function renderVoidErosionNarration(event: VoidErosionEventV1): Narration {
  const stageText = STAGE_NARRATION[event.to_stage]
    ?? `虚蚀推进至 ${event.to_stage}，灵压场域正在重塑。`;
  return {
    scope: "player",
    // target 格式：entity_id 必须在 "|" 前缀之前，server normalize_player_target 取
    // target.split('|')[0] 与玩家 username/char_id 比对进行路由（参 agent_bridge.rs）。
    // 对齐已工作 runtime（meridian-severed-narration.ts、mutation-narration-runtime.ts）的格式。
    target: `${event.entity}|void_erosion:advance|from:${event.from_stage}|to:${event.to_stage}|tick:${event.server_tick}`,
    text: stageText,
    style: "narration",
  };
}

function parseNarration(content: string, fallback: Narration): Narration {
  try {
    const parsed = JSON.parse(content.trim()) as unknown;
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return fallback;
    const candidate = parsed as { text?: unknown; style?: unknown };
    if (typeof candidate.text !== "string" || typeof candidate.style !== "string") return fallback;
    return {
      scope: fallback.scope,
      target: fallback.target,
      text: candidate.text,
      style: candidate.style as Narration["style"],
    };
  } catch {
    return fallback;
  }
}

/**
 * 我流虚蚀阶段推进叙事 runtime。
 *
 * 订阅 `bong:void_erosion_event` → 按阶段生成 narration → publish `bong:agent_narrate`。
 */
export class VoidErosionNarrationRuntime {
  private readonly llm: LlmClient;
  private readonly model: string;
  private readonly sub: VoidErosionRuntimeClient;
  private readonly pub: VoidErosionRuntimeClient;
  private readonly logger: Pick<Console, "info" | "warn">;

  readonly stats = {
    received: 0,
    published: 0,
    rejectedContract: 0,
    fallbackUsed: 0,
    publishFailed: 0,
  };

  private readonly onMessage = (channel: string, message: string): void => {
    this.handlePayload(channel, message).catch((err: unknown) => {
      this.stats.publishFailed += 1;
      this.logger.warn("[void-erosion-runtime] unhandled error in handlePayload:", err);
    });
  };

  constructor(config: VoidErosionRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model;
    this.sub = config.sub;
    this.pub = config.pub;
    this.logger = config.logger ?? console;
  }

  async connect(): Promise<void> {
    await this.sub.subscribe(VOID_EROSION_EVENT);
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.logger.info("[void-erosion-runtime] subscribed to void_erosion_event");
  }

  async disconnect(): Promise<void> {
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
  }

  async handlePayload(channel: string, message: string): Promise<void> {
    if (channel !== VOID_EROSION_EVENT) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(message);
    } catch (error) {
      this.stats.rejectedContract += 1;
      this.logger.warn("[void-erosion-runtime] non-JSON payload:", error);
      return;
    }
    if (!validateVoidErosionEventV1Contract(parsed).ok) {
      this.stats.rejectedContract += 1;
      return;
    }
    const event = parsed as VoidErosionEventV1;
    this.stats.received += 1;

    const fallback = renderVoidErosionNarration(event);
    let narration = fallback;
    try {
      const result = await this.llm.chat(this.model, [
        {
          role: "system",
          content:
            "按末法残土叙事口吻，用一条 JSON {\"text\",\"style\"} 描述虚蚀修士灵压渐移的感受，不解释机制，style=\"narration\"。",
        },
        { role: "user", content: JSON.stringify(event) },
      ]);
      const candidate = parseNarration(
        normalizeLlmChatResult(result, this.model).content,
        fallback,
      );
      narration = candidate;
      if (narration.text === fallback.text) this.stats.fallbackUsed += 1;
    } catch {
      this.stats.fallbackUsed += 1;
    }

    try {
      await this.pub.publish(AGENT_NARRATE, JSON.stringify({ v: 1, narrations: [narration] }));
      this.stats.published += 1;
    } catch (err: unknown) {
      this.stats.publishFailed += 1;
      this.logger.warn("[void-erosion-runtime] publish failed:", err);
    }
  }
}
