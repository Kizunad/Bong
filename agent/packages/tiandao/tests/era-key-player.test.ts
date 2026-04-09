import { describe, expect, it } from "vitest";

import { CALAMITY_RECIPE, ERA_RECIPE, assembleContext } from "../src/context.js";
import { createMockClient } from "../src/llm.js";
import { runMockTickForTest } from "../src/main.js";
import { createMockWorldState } from "../src/mock-state.js";
import { WorldModel } from "../src/world-model.js";

const ERA_DECLARATION_RESPONSE = JSON.stringify({
  commands: [],
  narrations: [
    {
      scope: "broadcast",
      text: "天道昭告：灵潮纪已至，诸域灵机渐盛。",
      style: "era_decree",
    },
  ],
  reasoning: "Era declaration for deterministic test",
});

function getSectionLines(context: string, title: string): string[] {
  const sections = context.split("\n\n---\n\n");
  const section = sections.find((block) => block.startsWith(title));
  if (!section) return [];
  return section.split("\n").slice(1).filter((line) => line.startsWith("- "));
}

describe("era + key-player focused coverage", () => {
  it("keeps key-player selection inside Task 17 semantics and max 3", () => {
    const state = createMockWorldState();
    const context = assembleContext(CALAMITY_RECIPE, state, {
      worldModel: new WorldModel(),
    });

    const keyPlayerLines = getSectionLines(context, "## 关键人物");
    expect(keyPlayerLines.length).toBeGreaterThanOrEqual(1);
    expect(keyPlayerLines.length).toBeLessThanOrEqual(3);
    expect(context).toContain("综合最强(");
    expect(context).not.toContain("综合最弱(");
    expect(context).not.toContain("连杀");
  });

  it("prioritizes current era block over peer decisions in era recipe", () => {
    const state = createMockWorldState();
    const model = new WorldModel();
    model.updateState(state);
    model.rememberCurrentEra({
      name: "灵潮纪",
      sinceTick: 84_000,
      globalEffect: "灵气缓升",
    });

    const context = assembleContext(ERA_RECIPE, state, { worldModel: model });
    expect(context.indexOf("## 当前时代")).toBeGreaterThanOrEqual(0);
    expect(context.indexOf("## 世界趋势")).toBeGreaterThan(context.indexOf("## 当前时代"));
    expect(context.indexOf("## 天道平衡态")).toBeGreaterThan(context.indexOf("## 世界趋势"));
  });

  it("updates retained currentEra from era agent output deterministically", async () => {
    const worldModel = new WorldModel();
    const llm = createMockClient(ERA_DECLARATION_RESPONSE);

    await runMockTickForTest({
      llmClient: llm,
      worldModel,
      now: () => 1_000_000,
      model: "mock-model",
      sink: null,
    });

    expect(worldModel.currentEra).toEqual(
      expect.objectContaining({
        name: "灵潮纪",
        sinceTick: 84_000,
      }),
    );
    expect(worldModel.currentEra?.globalEffect).toContain("灵潮纪已至");
  });
});
