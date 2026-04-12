import { DEFAULT_MODEL, runRuntime } from "./runtime.js";

const deterministicReasoning = {
  calamity: "task-13 deterministic calamity",
  mutation: "task-13 deterministic mutation",
  era: "task-13 deterministic era",
};

const deterministicChat = async (_model: string, messages: unknown[]) => {
  const firstMessage = messages[0] as { content?: unknown } | undefined;
  const systemPrompt = String(firstMessage?.content ?? "");

  if (systemPrompt.includes("# 灾劫 Agent")) {
    return {
      content: JSON.stringify({
        commands: [
          {
            type: "spawn_event",
            target: "spawn",
            params: {
              event: "thunder_tribulation",
              intensity: 0.7,
              duration_ticks: 120,
            },
          },
        ],
        narrations: [
          {
            scope: "broadcast",
            text: "天穹微黯，雷意先行。此刻杀伐之气渐盛，劫云已在高处缓缓凝聚；若仍执意争胜，下一轮雷火将循因果而落，先至者未必能安然退去。",
            style: "system_warning",
          },
        ],
        reasoning: deterministicReasoning.calamity,
      }),
      durationMs: 0,
      requestId: "task-13-calamity",
      model: DEFAULT_MODEL,
    };
  }

  if (systemPrompt.includes("# 变化 Agent")) {
    return {
      content: JSON.stringify({
        commands: [
          {
            type: "modify_zone",
            target: "spawn",
            params: {
              spirit_qi_delta: -0.05,
              danger_level_delta: 1,
            },
          },
        ],
        narrations: [
          {
            scope: "broadcast",
            text: "灵机忽有逆转，地脉微颤。看似平静的气流已生偏移，若众修仍在此间逗留，下一轮山川气数多半将再起险变，宜早做取舍。",
            style: "zone_change",
          },
        ],
        reasoning: deterministicReasoning.mutation,
      }),
      durationMs: 0,
      requestId: "task-13-mutation",
      model: DEFAULT_MODEL,
    };
  }

  if (systemPrompt.includes("# 纪元 Agent")) {
    return {
      content: JSON.stringify({
        commands: [],
        narrations: [
          {
            scope: "broadcast",
            text: "天道昭示：此番雷势并非孤起，而是诸域杀机互引所成。今朝风色尚可辨，若众生仍轻忽先兆，下一轮诸方劫象将更早显形，不待人备。",
            style: "era_decree",
          },
        ],
        reasoning: deterministicReasoning.era,
      }),
      durationMs: 0,
      requestId: "task-13-era",
      model: "gpt-5.4",
    };
  }

  return {
    content: JSON.stringify([]),
    durationMs: 0,
    requestId: "task-13-annotate",
    model: DEFAULT_MODEL,
  };
};

await runRuntime(
  {
    mockMode: false,
    model: DEFAULT_MODEL,
    modelOverrides: {
      default: DEFAULT_MODEL,
      annotate: DEFAULT_MODEL,
      calamity: DEFAULT_MODEL,
      mutation: DEFAULT_MODEL,
      era: "gpt-5.4",
    },
    redisUrl: process.env.REDIS_URL ?? "redis://127.0.0.1:6379",
    baseUrl: "https://deterministic.local/v1",
    apiKey: "task-13-local-key",
  },
  {
    createClient: () => ({ chat: deterministicChat }),
    sleep: async (ms) => {
      await new Promise((resolve) => setTimeout(resolve, Math.min(ms, 500)));
    },
    logger: console,
    maxLoopIterations: 40,
  },
);
