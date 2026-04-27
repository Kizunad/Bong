import { describe, expect, it, vi } from "vitest";
import type { WorldStateV1 } from "@bong/schema";
import {
  MAX_TOOL_CALL_ROUNDS,
  TOOL_LOOP_TRUNCATED_RESPONSE,
  createClient,
} from "../src/llm.js";
import { createToolContext, toolSchema } from "../src/tools/types.js";
import { queryPlayerSkillMilestonesTool } from "../src/tools/query-player-skill-milestones.js";
import { WorldModel } from "../src/world-model.js";
import { createTestWorldState } from "./support/fakes.js";

describe("tool-calling loop", () => {
  it("executes at most three read-only tool rounds and truncates the fourth", async () => {
    const state = createTestWorldState();
    const toolExecute = vi.fn(async ({ round }: { round: number }) => ({
      round,
      zone: state.zones[0]?.name ?? "unknown",
    }));
    const chatCompletionRequest = vi
      .fn()
      .mockImplementationOnce(async ({ model }: { model: string }) => ({
        content: "",
        requestId: "req_1",
        model,
        toolCalls: [
          { id: "call_1", name: "query-zone", arguments: JSON.stringify({ round: 1 }) },
        ],
      }))
      .mockImplementationOnce(async ({ model }: { model: string }) => ({
        content: "",
        requestId: "req_2",
        model,
        toolCalls: [
          { id: "call_2", name: "query-zone", arguments: JSON.stringify({ round: 2 }) },
        ],
      }))
      .mockImplementationOnce(async ({ model }: { model: string }) => ({
        content: "",
        requestId: "req_3",
        model,
        toolCalls: [
          { id: "call_3", name: "query-zone", arguments: JSON.stringify({ round: 3 }) },
        ],
      }))
      .mockImplementationOnce(async ({ model }: { model: string }) => ({
        content: "",
        requestId: "req_4",
        model,
        toolCalls: [
          { id: "call_4", name: "query-zone", arguments: JSON.stringify({ round: 4 }) },
        ],
      }));
    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      chatCompletionRequest,
    });

    const result = await client.chat(
      "mock-model",
      [{ role: "user", content: "loop until you are cut off" }],
      {
        tools: [
          {
            name: "query-zone",
            description: "Reads a zone snapshot",
            readonly: true,
            parameters: toolSchema.object({ round: toolSchema.number() }),
            result: toolSchema.object({ round: toolSchema.number(), zone: toolSchema.string() }),
            execute: toolExecute,
          },
        ],
        toolContext: createToolContext({
          latestState: state,
          worldModel: WorldModel.fromState(state),
        }),
      },
    );

    expect(toolExecute).toHaveBeenCalledTimes(MAX_TOOL_CALL_ROUNDS);
    expect(chatCompletionRequest).toHaveBeenCalledTimes(MAX_TOOL_CALL_ROUNDS + 1);
    expect(result.content).toBe(TOOL_LOOP_TRUNCATED_RESPONSE);
    expect(result.toolUsage).toMatchObject({
      rounds: MAX_TOOL_CALL_ROUNDS,
      executedCalls: MAX_TOOL_CALL_ROUNDS,
      truncated: true,
    });
  });

  it("deduplicates repeated tool name and args without re-executing", async () => {
    const state = createTestWorldState();
    const toolExecute = vi.fn(async ({ zone }: { zone: string }) => ({
      zone,
      active: state.zones.some((candidate) => candidate.name === zone),
    }));
    const chatCompletionRequest = vi
      .fn()
      .mockImplementationOnce(async ({ model }: { model: string }) => ({
        content: "",
        requestId: "req_dup_1",
        model,
        toolCalls: [
          { id: "call_1", name: "list-zone", arguments: JSON.stringify({ zone: "starter_zone" }) },
          { id: "call_2", name: "list-zone", arguments: JSON.stringify({ zone: "starter_zone" }) },
        ],
      }))
      .mockImplementationOnce(async ({ model, messages }: { model: string; messages: Array<{ role?: string; content?: string }> }) => {
        const toolMessages = messages.filter((message) => message.role === "tool");
        expect(toolMessages).toHaveLength(2);
        expect(JSON.parse(String(toolMessages[0]?.content))).toMatchObject({
          deduplicated: false,
          output: { zone: "starter_zone", active: true },
        });
        expect(JSON.parse(String(toolMessages[1]?.content))).toMatchObject({
          deduplicated: true,
          output: { zone: "starter_zone", active: true },
        });
        return {
          content: JSON.stringify({ commands: [], narrations: [], reasoning: "done" }),
          requestId: "req_dup_2",
          model,
        };
      });
    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      chatCompletionRequest,
    });

    const result = await client.chat(
      "mock-model",
      [{ role: "user", content: "dedupe repeated lookups" }],
      {
        tools: [
          {
            name: "list-zone",
            description: "Reads zone availability",
            readonly: true,
            parameters: toolSchema.object({ zone: toolSchema.string() }),
            result: toolSchema.object({ zone: toolSchema.string(), active: toolSchema.boolean() }),
            execute: toolExecute,
          },
        ],
        toolContext: createToolContext({
          latestState: state,
          worldModel: WorldModel.fromState(state),
        }),
      },
    );

    expect(toolExecute).toHaveBeenCalledTimes(1);
    expect(result.content).toBe(JSON.stringify({ commands: [], narrations: [], reasoning: "done" }));
    expect(result.toolUsage).toMatchObject({
      rounds: 1,
      totalCalls: 2,
      executedCalls: 1,
      deduplicatedCalls: 1,
      errorCount: 0,
      truncated: false,
    });
  });

  it("rejects malformed tool args via schema validation and feeds back a structured error", async () => {
    const state = createTestWorldState();
    const toolExecute = vi.fn(async ({ player }: { player: string }) => ({
      player,
      zone: state.players[0]?.zone ?? "unknown",
    }));
    const chatCompletionRequest = vi
      .fn()
      .mockImplementationOnce(async ({ model }: { model: string }) => ({
        content: "",
        requestId: "req_bad_1",
        model,
        toolCalls: [
          { id: "call_1", name: "query-player", arguments: JSON.stringify({ player: 123 }) },
        ],
      }))
      .mockImplementationOnce(async ({ model, messages }: { model: string; messages: Array<{ role?: string; content?: string }> }) => {
        const toolMessage = messages.find((message) => message.role === "tool");
        expect(JSON.parse(String(toolMessage?.content))).toMatchObject({
          status: "error",
          deduplicated: false,
          error: {
            code: "INVALID_TOOL_ARGS",
          },
        });
        return {
          content: JSON.stringify({ commands: [], narrations: [], reasoning: "handled invalid args" }),
          requestId: "req_bad_2",
          model,
        };
      });
    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      chatCompletionRequest,
    });

    const result = await client.chat(
      "mock-model",
      [{ role: "user", content: "ask with malformed args" }],
      {
        tools: [
          {
            name: "query-player",
            description: "Reads player info",
            readonly: true,
            parameters: toolSchema.object({ player: toolSchema.string() }),
            result: toolSchema.object({ player: toolSchema.string(), zone: toolSchema.string() }),
            execute: toolExecute,
          },
        ],
        toolContext: createToolContext({
          latestState: state,
          worldModel: WorldModel.fromState(state),
        }),
      },
    );

    expect(toolExecute).not.toHaveBeenCalled();
    expect(result.content).toBe(
      JSON.stringify({ commands: [], narrations: [], reasoning: "handled invalid args" }),
    );
    expect(result.toolUsage).toMatchObject({
      rounds: 1,
      totalCalls: 1,
      executedCalls: 0,
      errorCount: 1,
      truncated: false,
    });
  });

  it("executes query-player-skill-milestones and returns narration-rich milestone payloads", async () => {
    const state: WorldStateV1 = {
      ...createTestWorldState(),
      players: [
        {
          uuid: "offline:Veteran",
          name: "Veteran",
          realm: "Awaken",
          composite_power: 0.82,
          breakdown: {
            combat: 0.8,
            wealth: 0.3,
            social: 0.2,
            karma: -0.2,
            territory: 0.1,
          },
          trend: "rising",
          active_hours: 8,
          zone: "blood_valley",
          pos: [8, 66, 8],
          recent_kills: 4,
          recent_deaths: 1,
          life_record: {
            recent_biography_summary: "t82000:reach:Spirit",
            recent_skill_milestones_summary: "t83000:skill:alchemy:lv2",
            skill_milestones: [
              {
                skill: "alchemy" as const,
                new_lv: 2,
                achieved_at: 83000,
                narration: "炉火识性稍深，丹道已至Lv.2。",
                total_xp_at: 240,
              },
            ],
          },
        },
      ],
      zones: [
        {
          name: "blood_valley",
          spirit_qi: 0.5,
          danger_level: 1,
          active_events: [],
          player_count: 1,
        },
      ],
    };
    const chatCompletionRequest = vi
      .fn()
      .mockImplementationOnce(async ({ model }: { model: string }) => ({
        content: "",
        requestId: "req_skill_1",
        model,
        toolCalls: [
          {
            id: "call_1",
            name: "query-player-skill-milestones",
            arguments: JSON.stringify({ uuid: "offline:Veteran", limit: 1 }),
          },
        ],
      }))
      .mockImplementationOnce(async ({ model, messages }: { model: string; messages: Array<{ role?: string; content?: string }> }) => {
        const toolMessage = messages.find((message) => message.role === "tool");
        const payload = JSON.parse(String(toolMessage?.content));
        expect(payload).toMatchObject({
          status: "ok",
          output: {
            player: {
              uuid: "offline:Veteran",
              name: "Veteran",
            },
            milestones: [
              {
                skill: "alchemy",
                newLv: 2,
                narration: "炉火识性稍深，丹道已至Lv.2。",
                totalXpAt: 240,
              },
            ],
          },
        });
        return {
          content: JSON.stringify({ commands: [], narrations: [], reasoning: "used skill milestones" }),
          requestId: "req_skill_2",
          model,
        };
      });
    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      chatCompletionRequest,
    });

    const result = await client.chat(
      "mock-model",
      [{ role: "user", content: "inspect recent skill breakthroughs" }],
      {
        tools: [queryPlayerSkillMilestonesTool],
        toolContext: createToolContext({
          latestState: state,
          worldModel: WorldModel.fromState(state),
        }),
      },
    );

    expect(result.content).toBe(
      JSON.stringify({ commands: [], narrations: [], reasoning: "used skill milestones" }),
    );
    expect(result.toolUsage).toMatchObject({
      rounds: 1,
      totalCalls: 1,
      executedCalls: 1,
      errorCount: 0,
      truncated: false,
    });
  });
});
