import { describe, expect, it, vi } from "vitest";

import { TiandaoAgent, resolveAgentTools } from "../src/agent.js";
import { CALAMITY_RECIPE } from "../src/context.js";
import { createClient, createMockClient } from "../src/llm.js";
import { createMockWorldState } from "../src/mock-state.js";
import { toolSchema } from "../src/tools/types.js";
import { WorldModel } from "../src/world-model.js";

describe("TiandaoAgent fake clock seam", () => {
  it("restores default role tool wiring and keeps era tool-free", () => {
    expect(resolveAgentTools("calamity.md").map((tool) => tool.name)).toEqual([
      "query-player",
      "query-player-skill-milestones",
      "list-active-events",
      "query-rat-density",
    ]);
    expect(resolveAgentTools("mutation.md").map((tool) => tool.name)).toEqual([
      "query-zone-history",
    ]);
    expect(resolveAgentTools("era.md")).toEqual([]);
  });

  it("respects interval via injected now()", async () => {
    const times = [30_000, 35_000, 61_000];
    let index = 0;
    const now = () => {
      const value = times[index] ?? times[times.length - 1];
      index += 1;
      return value;
    };

    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 30_000,
      now,
    });

    const world = createMockWorldState();
    const llm = createMockClient();

    const first = await agent.tick(llm, "mock-model", world);
    const second = await agent.tick(llm, "mock-model", world);
    const third = await agent.tick(llm, "mock-model", world);

    expect(first).not.toBeNull();
    expect(second).toBeNull();
    expect(third).not.toBeNull();
  });

  it("passes readonly tools and frozen snapshot context into llm chat", async () => {
    const world = createMockWorldState();
    const worldModel = WorldModel.fromState(world);
    const toolExecute = vi.fn(async (_args: { player: string }, ctx: { latestState: { tick: number }; worldModel: { lastTick: number | null } }) => {
      expect(Object.isFrozen(ctx.latestState)).toBe(true);
      expect(Object.isFrozen(ctx.worldModel)).toBe(true);
      return {
        tick: ctx.latestState.tick,
        lastTick: ctx.worldModel.lastTick,
      };
    });
    const chatCompletionRequest = vi
      .fn()
      .mockImplementationOnce(async ({ tools }: { tools?: Array<{ function: { name: string } }> }) => {
        expect(tools?.map((tool) => tool.function.name)).toEqual(["query-player"]);
        return {
          content: "",
          requestId: "req_agent_1",
          model: "mock-model",
          toolCalls: [
            {
              id: "tool_call_1",
              name: "query-player",
              arguments: JSON.stringify({ player: "TestPlayer" }),
            },
          ],
        };
      })
      .mockImplementationOnce(async ({ messages }: { messages: Array<{ role?: string; content?: string }> }) => {
        const toolMessage = messages.find((message) => message.role === "tool");
        const payload = JSON.parse(String(toolMessage?.content));
        expect(payload.output).toEqual({ tick: world.tick, lastTick: world.tick });
        return {
          content: JSON.stringify({ commands: [], narrations: [], reasoning: "used tool" }),
          requestId: "req_agent_2",
          model: "mock-model",
        };
      });
    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      chatCompletionRequest,
    });
    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 30_000,
      now: () => 30_000,
      tools: [
        {
          name: "query-player",
          description: "Reads player snapshot",
          readonly: true,
          parameters: toolSchema.object({ player: toolSchema.string() }),
          result: toolSchema.object({
            tick: toolSchema.number(),
            lastTick: toolSchema.anyOf(toolSchema.number(), toolSchema.null()),
          }),
          execute: toolExecute,
        },
      ],
    });
    agent.setWorldModel(worldModel);

    const decision = await agent.tick(client, "mock-model", world);

    expect(decision).not.toBeNull();
    expect(decision?.reasoning).toBe("used tool");
    expect(toolExecute).toHaveBeenCalledTimes(1);
    expect(decision?.__agentTickMetadata?.toolUsage).toMatchObject({
      rounds: 1,
      executedCalls: 1,
      truncated: false,
    });
  });
});
