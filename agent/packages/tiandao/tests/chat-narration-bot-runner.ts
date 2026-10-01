/**
 * Bot e2e 的 Redis→Tiandao→narration 确定性适配器。
 *
 * 它使用生产 RedisIpc 的列表 drain 和 narration publish，并使用生产
 * processChatBatch；唯一替换的是 LLM 标注器，令 CI 不依赖外部模型服务。
 */

import { validateNarrationV1Contract, type ChatMessageV1 } from "@bong/schema";
import { processChatBatch } from "../src/chat-processor.js";
import type { LlmClient } from "../src/llm.js";
import { RedisIpc } from "../src/redis-ipc.js";

const redisUrl = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const targetName = process.env.TARGET_NAME;
const chatToken = process.env.CHAT_TOKEN;
const POLL_TIMEOUT_MS = 20_000;
const POLL_INTERVAL_MS = 100;
const PUBLISH_SETTLE_DELAY_MS = 250;

if (!targetName || !chatToken) {
  throw new Error("TARGET_NAME and CHAT_TOKEN are required");
}

const delay = (milliseconds: number) =>
  new Promise<void>((resolve) => setTimeout(resolve, milliseconds));

const deterministicAnnotator: LlmClient = {
  async chat(model, messages) {
    const prompt = messages.find((message) => message.role === "user")?.content;
    if (typeof prompt !== "string") {
      throw new Error("chat annotation prompt must contain a string user message");
    }
    const rows = JSON.parse(prompt.split("\n").at(-1) ?? "[]") as Array<{
      player: string;
      zone: string;
      raw: string;
    }>;
    return {
      content: JSON.stringify(
        rows.map((row) => ({
          ...row,
          sentiment: 0.4,
          intent: "social",
          influence_weight: 0.6,
        })),
      ),
      durationMs: 0,
      requestId: "bot-e2e-deterministic-annotator",
      model,
    };
  },
};

function findMatchingMessage(messages: ChatMessageV1[]): ChatMessageV1 | undefined {
  return messages.find(
    (message) =>
      message.raw.includes(chatToken!) && message.player === `offline:${targetName}`,
  );
}

const ipc = new RedisIpc({ url: redisUrl });
let matchedMessage: ChatMessageV1 | undefined;

try {
  await ipc.connect();
  const deadline = Date.now() + POLL_TIMEOUT_MS;
  while (!matchedMessage && Date.now() < deadline) {
    matchedMessage = findMatchingMessage(
      await ipc.drainPlayerChat({ maxItems: 128, logger: console }),
    );
    if (!matchedMessage) {
      await delay(POLL_INTERVAL_MS);
    }
  }

  if (!matchedMessage) {
    throw new Error(`timed out waiting for bong:player_chat token=${chatToken}`);
  }

  const signals = await processChatBatch({
    messages: [matchedMessage],
    annotateClient: deterministicAnnotator,
    annotateModel: "gpt-5.4-mini",
    logger: console,
  });
  const signal = signals[0];
  if (!signal || signal.raw !== matchedMessage.raw || signal.ts !== matchedMessage.ts) {
    throw new Error(
      `production processChatBatch did not preserve chat identity: ${JSON.stringify({
        matchedMessage,
        signal,
      })}`,
    );
  }

  const narration = {
    scope: "player" as const,
    target: `offline:${targetName}`,
    style: "narration" as const,
    text: `天道回流：${signal.raw}（server_ts=${signal.ts}）`,
  };
  const contract = validateNarrationV1Contract({ v: 1, narrations: [narration] });
  if (!contract.ok) {
    throw new Error(`constructed narration violates schema: ${contract.errors.join("; ")}`);
  }

  await ipc.publishNarrations({
    narrations: [narration],
    metadata: {
      sourceTick: signal.ts,
      correlationId: `bot-e2e-chat-${chatToken}`,
    },
  });
  await delay(PUBLISH_SETTLE_DELAY_MS);

  process.stdout.write(
    `${JSON.stringify({
      chat_channel: "bong:player_chat",
      narration_channel: "bong:agent_narrate",
      player: matchedMessage.player,
      zone: matchedMessage.zone,
      raw: matchedMessage.raw,
      observed_ts: matchedMessage.ts,
      signal_ts: signal.ts,
      narration,
    })}\n`,
  );
} finally {
  await ipc.disconnect().catch(() => undefined);
}
