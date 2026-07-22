import { createRequire } from "node:module";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { CHANNELS, type ChatMessageV1, type WorldStateV1 } from "../../agent/packages/schema/dist/index.js";
import { TiandaoAgent } from "../../agent/packages/tiandao/src/agent.js";
import { CALAMITY_RECIPE } from "../../agent/packages/tiandao/src/context.js";
import type { LlmClient } from "../../agent/packages/tiandao/src/llm.js";
import { RedisIpc } from "../../agent/packages/tiandao/src/redis-ipc.js";
import {
  DEFAULT_MODEL,
  runRuntime,
  type RuntimeRedis,
} from "../../agent/packages/tiandao/src/runtime.js";

// scripts/ is outside the agent package graph; resolve ioredis from agent/node_modules.
const require = createRequire(import.meta.url);
const RedisModule = require(
  join(dirname(fileURLToPath(import.meta.url)), "../../agent/node_modules/ioredis"),
) as { default?: new (url: string, options?: Record<string, unknown>) => RedisClient } & (
  new (url: string, options?: Record<string, unknown>) => RedisClient
);
type RedisClient = {
  connect(): Promise<void>;
  rpush(key: string, value: string): Promise<number>;
  disconnect(): void;
};
const Redis = (RedisModule.default ?? RedisModule) as new (
  url: string,
  options?: Record<string, unknown>,
) => RedisClient;

function requireCondition(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function createSeedWorldState(tick: number, tsSeconds: number): WorldStateV1 {
  return {
    v: 1,
    ts: tsSeconds,
    tick,
    season_state: {
      season: "summer",
      tick_into_phase: tick,
      phase_total_ticks: 1_382_400,
      year_index: 0,
    },
    players: [
      {
        uuid: "offline:ChatWindowBot",
        name: "ChatWindowBot",
        realm: "Awaken",
        composite_power: 0.2,
        breakdown: {
          combat: 0.2,
          wealth: 0.2,
          social: 0.2,
          karma: 0,
          territory: 0.1,
        },
        trend: "stable",
        active_hours: 1,
        zone: "spawn",
        pos: [0, 64, 0],
        recent_kills: 0,
        recent_deaths: 0,
      },
    ],
    npcs: [],
    zones: [
      {
        name: "spawn",
        spirit_qi: 0.5,
        danger_level: 1,
        active_events: [],
        player_count: 1,
      },
    ],
    rat_density_heatmap: {
      zones: {},
    },
    recent_events: [],
  };
}

type CapturedPrompt = {
  role: string;
  model: string;
  messages: Array<{ role: string; content: string }>;
};

async function withIsolatedCwd<T>(run: () => Promise<T>): Promise<T> {
  const tempDir = await mkdtemp(join(tmpdir(), "chat-window-e2e-"));
  const previousCwd = process.cwd();
  try {
    process.chdir(tempDir);
    return await run();
  } finally {
    process.chdir(previousCwd);
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function drainMarkerMessage(redis: RedisIpc, marker: string): Promise<ChatMessageV1> {
  let sourceMessage: ChatMessageV1 | undefined;
  for (let attempt = 0; attempt < 30 && !sourceMessage; attempt += 1) {
    const drained = await redis.drainPlayerChat({ logger: console });
    sourceMessage = drained.find((message) => message.raw === marker);
    if (!sourceMessage) {
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  requireCondition(sourceMessage, `真实 bot chat 未抵达 bong:player_chat，marker=${marker}`);
  return sourceMessage;
}

async function rpushChatMessage(redisUrl: string, message: ChatMessageV1): Promise<void> {
  const client = new Redis(redisUrl, {
    maxRetriesPerRequest: 1,
    enableReadyCheck: false,
    lazyConnect: true,
  });
  try {
    await client.connect();
    await client.rpush(CHANNELS.PLAYER_CHAT, JSON.stringify(message));
  } finally {
    client.disconnect();
  }
}

function createPromptCapturingClient(captured: CapturedPrompt[]): LlmClient {
  return {
    async chat(model, messages) {
      const normalized = (messages ?? []).map((message) => ({
        role: String((message as { role?: unknown }).role ?? ""),
        content: String((message as { content?: unknown }).content ?? ""),
      }));
      const systemContent = normalized.find((message) => message.role === "system")?.content ?? "";
      const isAnnotate =
        systemContent.includes("聊天信号标注器") || systemContent.includes("严格输出 JSON 数组");

      captured.push({
        role: isAnnotate ? "annotate" : "agent",
        model,
        messages: normalized,
      });

      if (isAnnotate) {
        const userContent = normalized.find((message) => message.role === "user")?.content ?? "";
        // buildAnnotatePrompt serializes [{player,zone,raw}, ...]
        let player = "offline:ChatWindowBot";
        let zone = "spawn";
        let raw = "";
        try {
          const jsonMatch = userContent.match(/\[[\s\S]*\]/);
          if (jsonMatch) {
            const rows = JSON.parse(jsonMatch[0]) as Array<Record<string, unknown>>;
            const first = rows[0] ?? {};
            if (typeof first.player === "string") player = first.player;
            if (typeof first.zone === "string") zone = first.zone;
            if (typeof first.raw === "string") raw = first.raw;
          }
        } catch {
          // fall through with defaults; processChatBatch still injects ts from msg
        }
        return {
          content: JSON.stringify([
            {
              player,
              zone,
              raw,
              sentiment: 0,
              intent: "social",
              influence_weight: 0.5,
            },
          ]),
          durationMs: 0,
          requestId: "chat-window-e2e-annotate",
          model,
        };
      }

      return {
        content: "[]",
        durationMs: 0,
        requestId: "chat-window-e2e-agent",
        model,
      };
    },
  };
}

function createObservedRuntimeRedis(
  redisUrl: string,
  seedState: WorldStateV1,
): RuntimeRedis {
  // Real RedisIpc for PLAYER_CHAT drain; seed world state so runTick always fires.
  const ipc = new RedisIpc({ url: redisUrl });
  return {
    connect: () => ipc.connect(),
    getLatestState: () => seedState,
    drainPlayerChat: (options) => ipc.drainPlayerChat(options),
    publishCommands: async () => {},
    publishNarrations: async () => {},
    disconnect: () => ipc.disconnect(),
  };
}

async function runObservedRuntimeRound(args: {
  redisUrl: string;
  message: ChatMessageV1;
  nowMs: number;
  tick: number;
}): Promise<CapturedPrompt[]> {
  const { redisUrl, message, nowMs, tick } = args;
  const captured: CapturedPrompt[] = [];
  const nowSeconds = Math.floor(nowMs / 1000);
  const clockMs = nowMs;
  const seedState = createSeedWorldState(tick, nowSeconds);

  await rpushChatMessage(redisUrl, message);

  await withIsolatedCwd(async () => {
    await runRuntime(
      {
        mockMode: false,
        model: DEFAULT_MODEL,
        modelOverrides: {
          default: DEFAULT_MODEL,
          annotate: DEFAULT_MODEL,
          calamity: DEFAULT_MODEL,
          mutation: DEFAULT_MODEL,
          era: DEFAULT_MODEL,
        },
        redisUrl,
        baseUrl: "https://chat-window-e2e.local/v1",
        apiKey: "chat-window-e2e-key",
      },
      {
        agents: [
          new TiandaoAgent({
            name: "calamity",
            skillFile: "calamity.md",
            recipe: CALAMITY_RECIPE,
            intervalMs: 0,
            now: () => clockMs,
          }),
        ],
        createRedis: () => createObservedRuntimeRedis(redisUrl, seedState),
        createClient: () => createPromptCapturingClient(captured),
        sleep: async () => {},
        now: () => clockMs,
        maxLoopIterations: 1,
        logger: console,
      },
    );
  });

  return captured;
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
    const message = await drainMarkerMessage(redis, marker);
    const wallNowSeconds = Math.floor(Date.now() / 1_000);
    const ageSeconds = wallNowSeconds - message.ts;
    const forgedClientTimestampSeconds = Math.floor(forgedClientTimestampMillis / 1_000);

    requireCondition(
      Number.isInteger(message.ts) && ageSeconds >= 0 && ageSeconds <= 30,
      `ChatMessageV1.ts 必须是 server 当前观察秒；实际 ts=${message.ts}, now=${wallNowSeconds}, age=${ageSeconds}`,
    );
    requireCondition(
      message.ts !== forgedClientTimestampSeconds
        && forgedClientTimestampSeconds - message.ts >= 82_800,
      `server 必须忽略客户端未来时间；client_ms=${forgedClientTimestampMillis}, wire_ts=${message.ts}`,
    );

    const firstCaptured = await runObservedRuntimeRound({
      redisUrl,
      message,
      nowMs: message.ts * 1000,
      tick: 10_001,
    });
    const firstAgentUserPrompts = firstCaptured
      .filter((entry) => entry.role === "agent")
      .flatMap((entry) => entry.messages.filter((m) => m.role === "user").map((m) => m.content));
    requireCondition(
      firstAgentUserPrompts.length > 0,
      `第一轮必须捕获到真实 TiandaoAgent user prompt，captured=${JSON.stringify(firstCaptured.map((c) => c.role))}`,
    );
    requireCondition(
      firstAgentUserPrompts.some((prompt) => prompt.includes(marker)),
      `第一轮 agent user prompt 必须含 marker（真实 context→LLM 路径）；prompts=${JSON.stringify(firstAgentUserPrompts)}`,
    );
    requireCondition(
      firstAgentUserPrompts.some((prompt) => prompt.includes("近期民意")),
      `第一轮 agent user prompt 必须含近期民意块；prompts=${JSON.stringify(firstAgentUserPrompts)}`,
    );

    const secondCaptured = await runObservedRuntimeRound({
      redisUrl,
      message,
      nowMs: (message.ts + 301) * 1000,
      tick: 10_002,
    });
    const secondAgentUserPrompts = secondCaptured
      .filter((entry) => entry.role === "agent")
      .flatMap((entry) => entry.messages.filter((m) => m.role === "user").map((m) => m.content));
    requireCondition(
      secondAgentUserPrompts.length > 0,
      `第二轮必须捕获到真实 TiandaoAgent user prompt，captured=${JSON.stringify(secondCaptured.map((c) => c.role))}`,
    );
    requireCondition(
      secondAgentUserPrompts.every((prompt) => !prompt.includes(marker)),
      `第二轮 now=ts+301 后 agent user prompt 不得再含 marker；prompts=${JSON.stringify(secondAgentUserPrompts)}`,
    );

    console.log(
      `[chat-window-e2e] PASS marker=${marker} client_ms=${forgedClientTimestampMillis} wire_ts=${message.ts} now=${wallNowSeconds} age=${ageSeconds}`,
    );
  } finally {
    await redis.disconnect();
  }
}

main().catch((error: unknown) => {
  console.error("[chat-window-e2e] FAIL", error);
  process.exitCode = 1;
});
