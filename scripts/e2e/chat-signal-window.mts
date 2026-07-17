import {
  buildChatSignalsBlock,
  isRecentSignal,
  mergeChatSignals,
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
  const forgedClientTimestampMillis = Number(
    process.env.CHAT_WINDOW_E2E_CLIENT_TIMESTAMP_MILLIS,
  );
  requireCondition(redisUrl, "REDIS_URL is required");
  requireCondition(marker, "CHAT_WINDOW_E2E_MARKER is required");
  requireCondition(
    Number.isSafeInteger(forgedClientTimestampMillis) && forgedClientTimestampMillis > 0,
    "CHAT_WINDOW_E2E_CLIENT_TIMESTAMP_MILLIS must be a positive safe integer",
  );

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
    const forgedClientTimestampSeconds = Math.floor(forgedClientTimestampMillis / 1_000);
    requireCondition(
      Number.isInteger(message.ts) && ageSeconds >= 0 && ageSeconds <= 30,
      `ChatMessageV1.ts 必须是 server 当前观察秒；实际 ts=${message.ts}, now=${nowSeconds}, age=${ageSeconds}`,
    );
    requireCondition(
      message.ts !== forgedClientTimestampSeconds
        && forgedClientTimestampSeconds - message.ts >= 82_800,
      `server 必须忽略客户端未来时间；client_ms=${forgedClientTimestampMillis}, wire_ts=${message.ts}`,
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

    const forgedFutureSignal = { ...signal, ts: forgedClientTimestampSeconds };
    requireCondition(
      !isRecentSignal(forgedFutureSignal, message.ts),
      "Tiandao 纵深边界必须拒绝未来 ChatSignal",
    );

    const firstRound = mergeChatSignals([], [forgedFutureSignal, signal], message.ts);
    requireCondition(
      firstRound.length === 1 && firstRound[0]?.ts === message.ts,
      `首轮 merge 必须只保留 server-observed signal，实际 ${JSON.stringify(firstRound)}`,
    );
    const boundaryRound = mergeChatSignals(firstRound, [], message.ts + 300);
    requireCondition(
      boundaryRound.length === 1,
      "五分钟窗口的 300 秒下界必须保留",
    );
    const expiredRound = mergeChatSignals(boundaryRound, [], message.ts + 301);
    requireCondition(
      expiredRound.length === 0,
      "五分钟窗口必须淘汰 301 秒前的聊天",
    );

    const block = buildChatSignalsBlock({ signals: firstRound, nowSeconds: message.ts });
    requireCondition(
      block.includes(marker),
      `当前真实 bot 聊天必须进入 Tiandao prompt block，实际 block=${block}`,
    );
    const expiredBlock = buildChatSignalsBlock({
      signals: boundaryRound,
      nowSeconds: message.ts + 301,
    });
    requireCondition(
      expiredBlock === "",
      `301 秒后的 prompt 必须清空真实 bot 聊天，实际 block=${expiredBlock}`,
    );

    console.log(
      `[chat-window-e2e] PASS marker=${marker} client_ms=${forgedClientTimestampMillis} wire_ts=${message.ts} now=${nowSeconds} age=${ageSeconds}`,
    );
  } finally {
    await redis.disconnect();
  }
}

main().catch((error: unknown) => {
  console.error("[chat-window-e2e] FAIL", error);
  process.exitCode = 1;
});
