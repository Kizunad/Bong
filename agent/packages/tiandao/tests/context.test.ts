import { describe, expect, it } from "vitest";
import type { ChatSignal, PlayerProfile, WorldStateV1, ZoneSnapshot } from "@bong/schema";
import {
  assembleContext,
  CALAMITY_RECIPE,
  ERA_RECIPE,
  balanceBlock,
  createContextInput,
  chatSignalsBlock,
  keyPlayerBlock,
  perceptionEnvelopeBlock,
  peerDecisionsBlock,
  recentNarrationsBlock,
  worldSnapshotBlock,
  worldTrendBlock,
} from "../src/context.js";
import { WorldModel } from "../src/world-model.js";
import { createTestWorldState } from "./support/fakes.js";

interface PlayerOverrides extends Partial<Omit<PlayerProfile, "breakdown" | "pos" | "name">> {
  breakdown?: Partial<PlayerProfile["breakdown"]>;
  pos?: PlayerProfile["pos"];
}

function createPlayer(name: string, overrides: PlayerOverrides = {}): PlayerProfile {
  return {
    uuid: overrides.uuid ?? `offline:${name}`,
    name,
    realm: overrides.realm ?? "Awaken",
    composite_power: overrides.composite_power ?? 0.2,
    breakdown: {
      combat: 0.2,
      wealth: 0.2,
      social: 0.2,
      karma: 0,
      territory: 0.2,
      ...overrides.breakdown,
    },
    trend: overrides.trend ?? "stable",
    active_hours: overrides.active_hours ?? 1,
    zone: overrides.zone ?? "starter_zone",
    pos: overrides.pos ?? [0, 64, 0],
    recent_kills: overrides.recent_kills ?? 0,
    recent_deaths: overrides.recent_deaths ?? 0,
  };
}

function createZone(name: string, spiritQi: number, overrides: Partial<ZoneSnapshot> = {}): ZoneSnapshot {
  return {
    name,
    spirit_qi: spiritQi,
    danger_level: overrides.danger_level ?? 1,
    active_events: overrides.active_events ?? [],
    player_count: overrides.player_count ?? 0,
  };
}

function createState(args: {
  tick: number;
  players?: PlayerProfile[];
  zones?: ZoneSnapshot[];
}): WorldStateV1 {
  const players = args.players ?? [];
  return {
    v: 1,
    ts: 1_710_000_000 + args.tick,
    tick: args.tick,
    season_state: {
      season: "summer",
      tick_into_phase: args.tick,
      phase_total_ticks: 1_382_400,
      year_index: 0,
    },
    players,
    npcs: [],
    zones:
      args.zones ?? [createZone("starter_zone", 0.5, { player_count: players.length })],
    recent_events: [],
  };
}

function createSeededWorldModel(): { model: WorldModel; state: WorldStateV1 } {
  const model = new WorldModel();

  const basePlayers = [
    createPlayer("Steve", {
      composite_power: 0.98,
      zone: "blood_valley",
      recent_kills: 8,
      breakdown: { combat: 0.95, karma: -0.45 },
    }),
    createPlayer("Keeper", {
      composite_power: 0.15,
      zone: "green_cloud_peak",
      breakdown: { karma: 0.2, social: 0.5 },
    }),
    createPlayer("Wanderer", {
      composite_power: 0.05,
      zone: "newbie_valley",
      breakdown: { karma: 0 },
    }),
  ];

  const bloodValleyHistory = [0.62, 0.6, 0.58, 0.5, 0.48, 0.46];
  const greenCloudHistory = [0.82, 0.83, 0.84, 0.86, 0.88, 0.9];

  for (let i = 0; i < bloodValleyHistory.length - 1; i++) {
    model.updateState(
      createState({
        tick: i + 1,
        players: basePlayers,
        zones: [
          createZone("blood_valley", bloodValleyHistory[i] ?? 0.5, { player_count: 1 }),
          createZone("green_cloud_peak", greenCloudHistory[i] ?? 0.8, { player_count: 1 }),
          createZone("newbie_valley", 0.93, { player_count: 1 }),
        ],
      }),
    );
  }

    model.recordDecision("calamity", {
      commands: [
      {
        type: "spawn_event",
        target: "blood_valley",
        params: { event: "thunder_tribulation", intensity: 0.6 },
      },
      ],
      narrations: [
        {
          scope: "zone",
          target: "blood_valley",
          text: "血谷旧雷未散，草木尚伏。下一轮若仍有人争杀，云气必再低三寸。",
          style: "system_warning",
        },
      ],
      reasoning: "punish the strongest",
    });
  model.recordDecision("mutation", {
    commands: [
      { type: "modify_zone", target: "blood_valley", params: { spirit_qi_delta: -0.05 } },
      { type: "modify_zone", target: "green_cloud_peak", params: { spirit_qi_delta: 0.05 } },
    ],
    narrations: [],
    reasoning: "rebalance resources",
  });
  model.recordDecision("era", {
    commands: [],
    narrations: [],
    reasoning: "observe this round",
  });

  model.setCurrentEra({
    name: "末法纪",
    sinceTick: 4,
    globalEffect: "灵机渐枯，诸域修行更艰",
  });

  const latestState = createState({
    tick: 6,
    players: [
      ...basePlayers,
      createPlayer("FreshFace", {
        composite_power: 0.02,
        zone: "newbie_valley",
        breakdown: { karma: 0 },
      }),
    ],
    zones: [
      createZone("blood_valley", 0.46, { player_count: 1 }),
      createZone("green_cloud_peak", 0.9, { player_count: 1 }),
      createZone("newbie_valley", 0.95, { player_count: 2 }),
    ],
  });

  model.updateState(latestState);

  return { model, state: latestState };
}

describe("context with chat signals", () => {
  it("renders chat signals block with sentiment summary", () => {
    const input = createContextInput(createTestWorldState(), [
      {
        player: "offline:Steve",
        raw: "灵气太少了",
        sentiment: -0.7,
        intent: "complaint",
        influence_weight: 0.8,
      },
      {
        player: "offline:Alex",
        raw: "今天不错",
        sentiment: 0.3,
        intent: "social",
        influence_weight: 0.2,
      },
    ]);

    const text = chatSignalsBlock.render(input);
    expect(text).toContain("## 近期民意");
    expect(text).toContain("offline:Steve");
    expect(text).toContain("intent=complaint");
    expect(text).toContain("民意倾向:");
  });

  it("injects chat block into assembled recipe context", () => {
    const context = assembleContext(
      CALAMITY_RECIPE,
      createContextInput(createTestWorldState(), [
        {
          player: "offline:Steve",
          raw: "灵气太少了",
          sentiment: -0.7,
          intent: "complaint",
          influence_weight: 0.8,
        },
      ]),
    );

    expect(context).toContain("## 近期民意");
    expect(context).toContain("offline:Steve");
  });

  it("drops optional chat block when token budget is too small", () => {
    const tinyRecipe = {
      ...CALAMITY_RECIPE,
      maxTokenEstimate: 10,
    };

    const manySignals: ChatSignal[] = Array.from({ length: 10 }, (_, i) => ({
      player: `offline:p${i}`,
      raw: "灵气太少了灵气太少了灵气太少了",
      sentiment: -0.5,
      intent: "complaint",
      influence_weight: 0.5,
    }));

    const context = assembleContext(
      tinyRecipe,
      createContextInput(createTestWorldState(), manySignals),
    );

    expect(context).not.toContain("## 近期民意");
  });
});

describe("context with task-21 world model blocks", () => {
  it("renders peer decisions from previous round memory", () => {
    const { model, state } = createSeededWorldModel();

    const text = peerDecisionsBlock.render(
      createContextInput(state, [], 1_710_000_123, {
        agentName: "calamity",
        worldModel: model,
      }),
    );

    expect(text).toContain("## 其他天道意志");
    expect(text).toContain("变化 Agent (上一轮): blood_valley 灵气 -0.05");
    expect(text).toContain("green_cloud_peak 灵气 +0.05");
    expect(text).toContain("演绎时代 Agent (上一轮): 无行动");
    expect(text).not.toContain("灾劫 Agent");
  });

  it("renders recent narrations so prompts can avoid near-duplicates", () => {
    const { model, state } = createSeededWorldModel();

    const text = recentNarrationsBlock.render(
      createContextInput(state, [], 1_710_000_123, {
        agentName: "mutation",
        worldModel: model,
      }),
    );

    expect(text).toContain("## 近轮天道叙事");
    expect(text).toContain("灾劫 Agent: zone target=blood_valley system_warning");
    expect(text).toContain("必须换物象、换句式");
  });

  it("renders per-player perception envelope for viewpoint clipping", () => {
    const state = createState({
      tick: 10,
      players: [
        createPlayer("Novice", { realm: "Induce", pos: [1.2, 64, -4.8] }),
        createPlayer("VoidWalker", { realm: "Void", zone: "blood_valley", pos: [100, 70, 200] }),
      ],
    });

    const text = perceptionEnvelopeBlock.render(createContextInput(state));

    expect(text).toContain("## 玩家可感知边界");
    expect(text).toContain("叙事只能写玩家肉眼或神识可感之事");
    expect(text).toContain("offline:Novice: Induce @ starter_zone (1,64,-5)；神识 50m");
    expect(text).toContain("offline:VoidWalker: Void @ blood_valley (100,70,200)；神识三圈");
    expect(text).toContain("匿名：正文不要主动写玩家名");
  });

  it("renders world trend, balance, and key-player blocks from the shared world model", () => {
    const { model, state } = createSeededWorldModel();
    const input = createContextInput(state, [], 1_710_000_123, {
      agentName: "era",
      worldModel: model,
    });

    const trendText = worldTrendBlock.render(input);
    const balanceText = balanceBlock.render(input);
    const keyPlayerText = keyPlayerBlock.render(input);

    expect(trendText).toContain("## 当前时代");
    expect(trendText).toContain("末法纪（始于 tick 4）");
    expect(trendText).toContain("律令: 灵机渐枯，诸域修行更艰");
    expect(trendText).toContain("## 世界趋势 (最近 10 轮)");
    expect(trendText).toContain("blood_valley: 灵气 0.60 → 0.48 (↓下降中)");
    expect(trendText).toContain("green_cloud_peak: 灵气 0.83 → 0.88 (↑上升中)");

    expect(balanceText).toContain("## 天道平衡态");
    expect(balanceText).toContain("Gini 系数:");
    expect(balanceText).toContain("严重失衡");
    expect(balanceText).toContain("对 Steve 施压");

    expect(keyPlayerText).toContain("## 关键人物");
    expect(keyPlayerText).toContain("Steve: 综合最强(0.98)");
    expect(keyPlayerText).toContain("FreshFace: 新入世(0.02)");
  });

  it("injects task-21 blocks into the existing recipes", () => {
    const { model, state } = createSeededWorldModel();

    const calamityContext = assembleContext(
      CALAMITY_RECIPE,
      createContextInput(state, [], 1_710_000_123, {
        agentName: "calamity",
        worldModel: model,
      }),
    );
    const eraContext = assembleContext(
      ERA_RECIPE,
      createContextInput(state, [], 1_710_000_123, {
        agentName: "era",
        worldModel: model,
      }),
    );

    expect(calamityContext).toContain("## 关键人物");
    expect(calamityContext).toContain("## 玩家可感知边界");
    expect(calamityContext).toContain("## 天道平衡态");
    expect(calamityContext).toContain("## 近轮天道叙事");
    expect(calamityContext).toContain("## 其他天道意志");
    expect(eraContext).toContain("## 当前时代");
    expect(eraContext).toContain("## 玩家可感知边界");
    expect(eraContext).toContain("## 世界趋势 (最近 10 轮)");
    expect(eraContext).toContain("## 近轮天道叙事");
    expect(eraContext).toContain("## 其他天道意志");
  });

  it("preserves required blocks and trims optional task-21 blocks under token pressure", () => {
    const { model, state } = createSeededWorldModel();
    const noisySignals: ChatSignal[] = Array.from({ length: 10 }, (_, index) => ({
      player: `offline:p${index}`,
      raw: "灵气太少了灵气太少了灵气太少了灵气太少了",
      sentiment: -0.5,
      intent: "complaint",
      influence_weight: 0.5,
    }));

    const tinyRecipe = {
      agentName: "test",
      maxTokenEstimate: 80,
      blocks: [
        { ...keyPlayerBlock, priority: 0, required: true },
        { ...worldSnapshotBlock, priority: 1, required: true },
        { ...balanceBlock, priority: 2, required: false },
        { ...peerDecisionsBlock, priority: 3, required: false },
        { ...chatSignalsBlock, priority: 4, required: false },
      ],
    };

    const context = assembleContext(
      tinyRecipe,
      createContextInput(state, noisySignals, 1_710_000_123, {
        agentName: "calamity",
        worldModel: model,
      }),
    );

    expect(context).toContain("## 关键人物");
    expect(context).toContain("## 世界快照");
    expect(context).not.toContain("## 天道平衡态");
    expect(context).not.toContain("## 其他天道意志");
    expect(context).not.toContain("## 近期民意");
  });
});
