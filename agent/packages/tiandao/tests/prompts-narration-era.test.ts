import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it, vi } from "vitest";

import { MAX_NARRATION_LENGTH } from "@bong/schema";

import { createMockClient } from "../src/llm.js";
import { runMockTickForTest } from "../src/main.js";
import { parseDecision } from "../src/parse.js";
import { WorldModel } from "../src/world-model.js";

const SKILLS_DIR = resolve(import.meta.dirname, "../src/skills");

function readSkill(name: string): string {
  return readFileSync(resolve(SKILLS_DIR, name), "utf-8");
}

describe("Tiandao prompts narration era coverage", () => {
  it("writes narration constraints explicitly into calamity and mutation prompts", () => {
    const calamityPrompt = readSkill("calamity.md");
    const mutationPrompt = readSkill("mutation.md");

    expect(calamityPrompt).toContain("半文言半白话");
    expect(calamityPrompt).toContain("约 100-200 个中文字符");
    expect(calamityPrompt).toContain("当前因果/触发缘由");
    expect(calamityPrompt).toContain("对下一轮或下一步的暗示");
    expect(calamityPrompt).toContain("只输出**单个合法 JSON 对象**");

    expect(mutationPrompt).toContain("半文言半白话");
    expect(mutationPrompt).toContain("约 100-200 个中文字符");
    expect(mutationPrompt).toContain("本轮变化的当前成因");
    expect(mutationPrompt).toContain("对下一轮或后续演化的预兆");
    expect(mutationPrompt).toContain("只输出**单个合法 JSON 对象**");
  });

  it("writes era decree narration constraints explicitly into era prompt", () => {
    const eraPrompt = readSkill("era.md");

    expect(eraPrompt).toContain("`era_decree`");
    expect(eraPrompt).toContain("半文言半白话");
    expect(eraPrompt).toContain("约 100-200 个中文字符");
    expect(eraPrompt).toContain("当下时代转折的成因/大势");
    expect(eraPrompt).toContain("对下一轮或后势的预兆");
    expect(eraPrompt).toContain("只输出**单个合法 JSON 对象**");
  });

  it("still parses valid strict JSON decisions after prompt changes", () => {
    const longNarration = "灵潮翻涌".repeat(140);
    const raw = JSON.stringify({
      commands: [],
      narrations: [
        {
          scope: "broadcast",
          text: longNarration,
          style: "era_decree",
        },
      ],
      reasoning: "deterministic valid json",
    });

    const decision = parseDecision(raw);

    expect(decision.commands).toEqual([]);
    expect(decision.reasoning).toBe("deterministic valid json");
    expect(decision.narrations).toHaveLength(1);
    expect(decision.narrations[0]).toEqual({
      scope: "broadcast",
      text: longNarration.slice(0, MAX_NARRATION_LENGTH),
      style: "era_decree",
    });
    expect(decision.narrations[0]?.text).toHaveLength(MAX_NARRATION_LENGTH);
  });

  it("degrades non-json free-form outputs to empty decision", () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    try {
      const decision = parseDecision(
        "天道曰：今夜劫云将起，诸修各自珍重。此乃自由散文，并非 JSON。",
      );

      expect(decision).toEqual({
        commands: [],
        narrations: [],
        reasoning: "no action",
      });
      expect(warnSpy).toHaveBeenCalledOnce();
    } finally {
      warnSpy.mockRestore();
    }
  });

  it("allows era_decree narration to survive parse and reach downstream world model assertions", async () => {
    const worldModel = new WorldModel();
    const llm = createMockClient(
      JSON.stringify({
        commands: [],
        narrations: [
          {
            scope: "broadcast",
            text: "天道昭告：赤霄纪已至，皆因诸域强弱久失其衡，今朝赤云压野、灵脉微灼；若众修仍竞逐杀伐，下一轮诸方火性异变将更炽，宜早作收敛。",
            style: "era_decree",
          },
        ],
        reasoning: "deterministic era decree",
      }),
    );

    const summary = await runMockTickForTest({
      llmClient: llm,
      worldModel,
      now: () => 1_000_000,
      model: "mock-model",
      sink: null,
    });

    const eraDecision = worldModel.lastDecisions.get("era");

    expect(summary.skipped).toBe(false);
    expect(eraDecision).toBeDefined();
    expect(eraDecision?.narrations).toHaveLength(1);
    expect(eraDecision?.narrations[0]).toEqual({
      scope: "broadcast",
      text: "天道昭告：赤霄纪已至，皆因诸域强弱久失其衡，今朝赤云压野、灵脉微灼；若众修仍竞逐杀伐，下一轮诸方火性异变将更炽，宜早作收敛。",
      style: "era_decree",
    });
    expect(worldModel.currentEra).toEqual(
      expect.objectContaining({
        name: "赤霄纪",
        sinceTick: 84_000,
      }),
    );
    expect(worldModel.currentEra?.globalEffect).toContain("赤霄纪已至");
  });
});
