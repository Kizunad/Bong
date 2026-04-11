import { describe, expect, it, vi } from "vitest";
import {
  MAX_TOOL_CALL_ROUNDS,
  TOOL_LOOP_TRUNCATED_RESPONSE,
  createClient,
} from "../src/llm.js";
import { createToolContext, toolSchema } from "../src/tools/types.js";
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
});
