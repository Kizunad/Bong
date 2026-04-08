import { describe, expect, it } from "vitest";

import type { ChatSignal, WorldStateV1 } from "@bong/schema";

import {
  CALAMITY_RECIPE,
  ERA_RECIPE,
  MUTATION_RECIPE,
  assembleContext,
  balanceBlock,
  chatSignalsBlock,
  peerDecisionsBlock,
  worldTrendBlock,
  worldSnapshotBlock,
  type ContextRecipe,
} from "../src/context.js";
import { createMockWorldState } from "../src/mock-state.js";
import type { AgentDecision } from "../src/parse.js";
import { WorldModel } from "../src/world-model.js";

function withZoneSpiritQi(
  baseState: WorldStateV1,
  tick: number,
  updates: Record<string, number>,
): WorldStateV1 {
  return {
    ...baseState,
    tick,
    zones: baseState.zones.map((zone) =>
      updates[zone.name] === undefined
        ? zone
        : {
            ...zone,
            spirit_qi: updates[zone.name] as number,
          },
    ),
  };
}

function buildSignal(player: string, raw: string, sentiment: number, intent: ChatSignal["intent"]): ChatSignal {
  return {
    player,
    raw,
    sentiment,
    intent,
    influence_weight: 0.7,
  };
}

function buildDecision(command: AgentDecision["commands"][number]): AgentDecision {
  return {
    commands: [command],
    narrations: [],
    reasoning: "deterministic test decision",
  };
}

function getSectionLines(context: string, title: string): string[] {
  const sections = context.split("\n\n---\n\n");
  const section = sections.find((block) => block.startsWith(title));
  if (!section) return [];
  return section.split("\n").slice(1).filter((line) => line.startsWith("- "));
}

function createWorldModelWithHistory(state: WorldStateV1): WorldModel {
  const model = new WorldModel();

  [
    { blood_valley: 0.2, green_cloud_peak: 0.84, newbie_valley: 0.98 },
    { blood_valley: 0.24, green_cloud_peak: 0.85, newbie_valley: 0.97 },
    { blood_valley: 0.28, green_cloud_peak: 0.84, newbie_valley: 0.96 },
    { blood_valley: 0.42, green_cloud_peak: 0.84, newbie_valley: 0.92 },
    { blood_valley: 0.46, green_cloud_peak: 0.85, newbie_valley: 0.88 },
    { blood_valley: 0.5, green_cloud_peak: 0.84, newbie_valley: 0.84 },
  ].forEach((updates, index) => {
    model.updateState(withZoneSpiritQi(state, 84_000 + index, updates));
  });

  model.rememberChatSignals([
    buildSignal("Steve", "灵气太少了，血谷越来越挤", -0.7, "complaint"),
    buildSignal("Alex", "大家早，今天气象不错", 0.3, "social"),
  ]);
  model.rememberDecision(
    "calamity",
    buildDecision({
      type: "spawn_event",
      target: "blood_valley",
      params: {
        event: "thunder_tribulation",
        intensity: 0.6,
      },
    }),
  );
  model.rememberDecision(
    "mutation",
    buildDecision({
      type: "modify_zone",
      target: "green_cloud_peak",
      params: {
        spirit_qi_delta: 0.05,
      },
    }),
  );
  model.rememberCurrentEra({
    name: "灵潮纪",
    sinceTick: 83_900,
    globalEffect: "灵气缓升，异象渐密",
  });

  return model;
}

describe("assembleContext era + key-player semantics", () => {
  it("renders key-player block using only strongest + karma extreme + newcomer signals (max 3)", () => {
    const state = createMockWorldState();
    const worldModel = createWorldModelWithHistory(state);

    const context = assembleContext(CALAMITY_RECIPE, state, { worldModel });
    const keyPlayerLines = getSectionLines(context, "## 关键人物");

    expect(context.indexOf("## 关键人物")).toBeGreaterThanOrEqual(0);
    expect(context.indexOf("## 玩家画像")).toBeGreaterThan(context.indexOf("## 关键人物"));
    expect(context).toContain("Steve: 综合最强(0.85)");
    expect(context).not.toContain("综合最弱(");
    expect(context).not.toContain("连杀");
    expect(keyPlayerLines.length).toBeGreaterThanOrEqual(1);
    expect(keyPlayerLines.length).toBeLessThanOrEqual(3);
    expect(context).toContain("## 近期民意");
    expect(context).toContain("民意倾向: 偏负面");
  });

  it("prioritizes current era + world trend + balance above peer decisions for era recipe", () => {
    const state = createMockWorldState();
    const worldModel = createWorldModelWithHistory(state);

    const mutationContext = assembleContext(MUTATION_RECIPE, state, { worldModel });
    const eraContext = assembleContext(ERA_RECIPE, state, { worldModel });

    expect(mutationContext.indexOf("## 世界趋势")).toBeGreaterThanOrEqual(0);
    expect(mutationContext.indexOf("## 世界快照")).toBeGreaterThan(mutationContext.indexOf("## 世界趋势"));
    expect(mutationContext).toContain("blood_valley: 灵气 0.24 → 0.46 (↑上升中)");
    expect(mutationContext).toContain("newbie_valley: 灵气 0.97 → 0.88 (↓下降中)");

    expect(eraContext.indexOf("## 当前时代")).toBeGreaterThanOrEqual(0);
    expect(eraContext).toContain("名称: 灵潮纪");
    expect(eraContext.indexOf("## 世界趋势")).toBeGreaterThan(eraContext.indexOf("## 当前时代"));
    expect(eraContext.indexOf("## 天道平衡态")).toBeGreaterThan(eraContext.indexOf("## 世界趋势"));
    expect(eraContext.indexOf("## 其他天道意志")).toBeGreaterThan(eraContext.indexOf("## 天道平衡态"));
    expect(eraContext).toContain("灾劫 Agent (上一轮): 在 blood_valley 触发 thunder_tribulation (intensity 0.60)");
    expect(eraContext).toContain("变化 Agent (上一轮): green_cloud_peak 灵气 +0.05");
  });

  it("can render balance block from retained world-model snapshot without changing deterministic output", () => {
    const state = createMockWorldState();
    const worldModel = new WorldModel();

    worldModel.updateState(state);
    const balanceText = balanceBlock.render({
      agentName: "era",
      state,
      worldModel,
    });

    expect(balanceText).toContain("## 天道平衡态");
    expect(balanceText).toContain("Gini 系数:");
    expect(balanceText).toContain("Steve(0.85) @ blood_valley");
    expect(balanceText).toContain("[pressure_strongest] 对 Steve 施压");
  });

  it("crops lower-priority optional blocks when token budget is tiny", () => {
    const state = createMockWorldState();
    const worldModel = createWorldModelWithHistory(state);

    const tinyRecipe: ContextRecipe = {
      agentName: "era",
      maxTokenEstimate: 1,
      blocks: [
        { ...worldTrendBlock, priority: 0, required: true },
        { ...balanceBlock, priority: 1, required: false },
        { ...peerDecisionsBlock, priority: 2, required: false },
        { ...chatSignalsBlock, priority: 3, required: false },
        { ...worldSnapshotBlock, priority: 4, required: false },
      ],
    };

    const context = assembleContext(tinyRecipe, state, { worldModel });

    expect(context).toContain("## 世界趋势");
    expect(context).not.toContain("## 天道平衡态");
    expect(context).not.toContain("## 其他天道意志");
    expect(context).not.toContain("## 近期民意");
    expect(context).not.toContain("## 世界快照");
  });

  it("gracefully degrades with sparse history, empty players, and no chat signals", () => {
    const sparseState: WorldStateV1 = {
      ...createMockWorldState(),
      players: [],
      npcs: [],
      zones: [],
      recent_events: [],
    };

    const mutationContext = assembleContext(MUTATION_RECIPE, sparseState, {
      worldModel: new WorldModel(),
    });
    const eraContext = assembleContext(ERA_RECIPE, sparseState, {
      worldModel: new WorldModel(),
    });

    expect(mutationContext).toContain("## 世界趋势");
    expect(mutationContext).toContain("- 暂无 zone 数据");
    expect(mutationContext).toContain("## 世界快照");
    expect(mutationContext).not.toContain("## 玩家画像");
    expect(mutationContext).not.toContain("## 近期民意");

    expect(eraContext).toContain("## 当前时代");
    expect(eraContext).toContain("尚无时代宣告");
    expect(eraContext).toContain("## 天道平衡态");
    expect(eraContext).toContain("Gini 系数: 0.00 (平衡)");
    expect(eraContext).not.toContain("## 关键人物");
    expect(eraContext).not.toContain("## 其他天道意志");
  });
});
