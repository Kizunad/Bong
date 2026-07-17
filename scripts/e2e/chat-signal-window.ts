import {
  buildChatSignalsBlock,
  isRecentSignal,
  processChatBatch,
} from "../../agent/packages/tiandao/src/chat-processor.js";
import { RedisIpc } from "../../agent/packages/tiandao/src/redis-ipc.js";
import type { LlmClient } from "../../agent/packages/tiandao/src/llm.js";

function requireCondition(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

async function main(): Promise<void> {
  const redisUrl = process.env.REDIS_URL;
  const marker = process.env.CHAT_WINDOW_E2E_MARKER;
  requireCondition(redisUrl, "REDIS_URL is required");
  requireCondition(marker, "CHAT_WINDOW_E2E_MARKER is required");

  const redis = new RedisIpc({ url: redisUrl });
  await redis.connect();

  try {
    let sourceMessage: Awaited<ReturnType<RedisIpc["drainPlayerChat"]>>[number] | undefined;
    for (let attempt = 0; attempt < 30 && !sourceMessage; attempt += 1) {
      const drained = await redis.drainPlayerChat({ logger: console });
      sourceMessage = drained.find((message) => message.raw === marker);
      if (!sourceMessage) {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    }

    requireCondition(
      sourceMessage,
      `真实 bot chat 未抵达 bong:player_chat，marker=${marker}`,
    );
    const message = sourceMessage;

    const nowSeconds = Math.floor(Date.now() / 1_000);
    const ageSeconds = nowSeconds - message.ts;
    requireCondition(
      Number.isInteger(message.ts) && ageSeconds >= -1 && ageSeconds <= 10,
      `ChatMessageV1.ts 必须是当前 Unix 秒；实际 ts=${message.ts}, now=${nowSeconds}, age=${ageSeconds}`,
    );

    const annotateClient: LlmClient = {
      async chat(model) {
        return {
          content: JSON.stringify([
            {
              player: message.player,
              zone: message.zone,
              raw: message.raw,
              sentiment: 0,
              intent: "social",
              influence_weight: 0.5,
            },
          ]),
          durationMs: 0,
          requestId: "chat-window-e2e",
          model,
        };
      },
    };

    const signals = await processChatBatch({
      messages: [message],
      annotateClient,
      annotateModel: "chat-window-e2e",
      logger: console,
    });
    requireCondition(
      signals.length === 1,
      `Tiandao 应生成 1 条 ChatSignal，实际 ${signals.length}`,
    );

    const signal = signals[0];
    requireCondition(
      signal.ts === message.ts,
      `ChatSignal.ts 必须保留 server 权威 Unix 秒，message=${message.ts}, signal=${signal.ts}`,
    );
    requireCondition(
      isRecentSignal(signal, nowSeconds),
      `真实 bot 当前聊天必须落入 Tiandao 五分钟窗口，ts=${signal.ts}, now=${nowSeconds}`,
    );
    requireCondition(
      isRecentSignal({ ...signal, ts: nowSeconds - 300 }, nowSeconds),
      "五分钟窗口的 300 秒下界必须保留",
    );
    requireCondition(
      !isRecentSignal({ ...signal, ts: nowSeconds - 301 }, nowSeconds),
      "五分钟窗口必须淘汰 301 秒前的聊天",
    );

    const block = buildChatSignalsBlock({ signals, nowSeconds });
    requireCondition(
      block.includes(marker),
      `当前真实 bot 聊天必须进入 Tiandao prompt block，实际 block=${block}`,
    );

    console.log(
      `[chat-window-e2e] PASS marker=${marker} wire_ts=${message.ts} now=${nowSeconds} age=${ageSeconds}`,
    );
  } finally {
    await redis.disconnect();
  }
}

main().catch((error: unknown) => {
  console.error("[chat-window-e2e] FAIL", error);
  process.exitCode = 1;
});
