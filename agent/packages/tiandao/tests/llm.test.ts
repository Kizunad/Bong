import { describe, expect, it, vi } from "vitest";

import {
  LlmBackoffError,
  LlmTimeoutError,
  MAX_TOOL_CALL_ROUNDS,
  MOCK_LLM_RESPONSE,
  TOOL_LOOP_TRUNCATED_RESPONSE,
  createClient,
  createMockClient,
} from "../src/llm.js";
import { toolSchema } from "../src/tools/types.js";

describe("createMockClient", () => {
  it("returns deterministic default response with stable metadata", async () => {
    const client = createMockClient();
    const result = await client.chat("mock-model", [
      { role: "system", content: "system" },
      { role: "user", content: "user" },
    ]);

    expect(result).toEqual({
      content: MOCK_LLM_RESPONSE,
      durationMs: 0,
      requestId: null,
      model: "mock-model",
    });
    expect(JSON.parse(result.content)).toEqual({
      commands: [],
      narrations: [],
      reasoning: "mock deterministic noop",
    });
  });

  it("supports custom deterministic response", async () => {
    const response = JSON.stringify({
      commands: [
        {
          type: "spawn_event",
          target: "blood_valley",
          params: { event: "beast_tide" },
        },
      ],
      narrations: [{ scope: "broadcast", text: "劫云翻涌", style: "narration" }],
      reasoning: "test",
    });
    const client = createMockClient(response);

    await expect(client.chat("any", [])).resolves.toEqual({
      content: response,
      durationMs: 0,
      requestId: null,
      model: "any",
    });
  });
});

describe("createClient runtime guard seam", () => {
  it("wraps successful chat completions with structured metadata", async () => {
    vi.useFakeTimers();

    try {
      const chatCompletionRequest = vi.fn(
        ({ model }: { model: string }) =>
          new Promise<{
            content: string;
            requestId: string | null;
            model: string;
          }>((resolve) => {
            setTimeout(() => {
              resolve({
                content: "structured-ok",
                requestId: "req_123",
                model,
              });
            }, 25);
          }),
      );

      const client = createClient({
        baseURL: "http://unit-test.local",
        apiKey: "test-key",
        model: "mock-model",
        timeoutMs: 100,
        chatCompletionRequest,
      });

      const pending = client.chat("mock-model", [{ role: "user", content: "hello" }]);
      await vi.advanceTimersByTimeAsync(25);

      await expect(pending).resolves.toEqual({
        content: "structured-ok",
        durationMs: 25,
        requestId: "req_123",
        model: "mock-model",
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps non-tool path unchanged when chat options are omitted or empty", async () => {
    const chatCompletionRequest = vi.fn().mockResolvedValue({
      content: "plain-response",
      requestId: "req_plain",
      model: "mock-model",
    });

    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      chatCompletionRequest,
    });

    const withoutTools = await client.chat("mock-model", []);
    expect(withoutTools).toMatchObject({
      content: "plain-response",
      requestId: "req_plain",
      model: "mock-model",
    });
    expect(withoutTools.durationMs).toBeGreaterThanOrEqual(0);

    const withEmptyTools = await client.chat("mock-model", [], { tools: [] });
    expect(withEmptyTools).toMatchObject({
      content: "plain-response",
      requestId: "req_plain",
      model: "mock-model",
    });
    expect(withEmptyTools.durationMs).toBeGreaterThanOrEqual(0);
    expect(chatCompletionRequest).toHaveBeenCalledTimes(2);
  });

  it("times out a single chat call with bounded timeout", async () => {
    vi.useFakeTimers();

    try {
      const chatCompletionRequest = vi.fn(
        ({ signal }: { signal: AbortSignal }) =>
          new Promise<{
            content: string;
            requestId: string | null;
            model: string;
          }>((_resolve, reject) => {
            signal.addEventListener(
              "abort",
              () => {
                reject(new Error("aborted"));
              },
              { once: true },
            );
          }),
      );

      const client = createClient({
        baseURL: "http://unit-test.local",
        apiKey: "test-key",
        model: "mock-model",
        timeoutMs: 50,
        chatCompletionRequest,
      });

      const pending = client.chat("mock-model", [{ role: "user", content: "timeout please" }]);
      const rejection = expect(pending).rejects.toBeInstanceOf(LlmTimeoutError);
      await vi.advanceTimersByTimeAsync(50);

      await rejection;
      expect(chatCompletionRequest).toHaveBeenCalledOnce();
    } finally {
      vi.useRealTimers();
    }
  });

  it("enters backoff after reaching consecutive failure threshold", async () => {
    const nowTs = 1_000;
    const chatCompletionRequest = vi.fn().mockRejectedValue(new Error("llm failure"));

    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      maxConsecutiveFailures: 2,
      backoffMs: 60_000,
      now: () => nowTs,
      chatCompletionRequest,
    });

    await expect(client.chat("mock-model", [])).rejects.toThrow("llm failure");
    await expect(client.chat("mock-model", [])).rejects.toThrow("llm failure");

    const thirdError = await client.chat("mock-model", []).catch((error) => error as Error);
    expect(thirdError).toBeInstanceOf(LlmBackoffError);
    expect((thirdError as LlmBackoffError).retryAt).toBe(nowTs + 60_000);
    expect(chatCompletionRequest).toHaveBeenCalledTimes(2);
  });

  it("skips request during cooldown and retries after backoff window", async () => {
    let nowTs = 5_000;
    const chatCompletionRequest = vi
      .fn()
      .mockRejectedValueOnce(new Error("f1"))
      .mockRejectedValueOnce(new Error("f2"))
      .mockResolvedValueOnce({
        content: "recovered",
        requestId: "req_recovered",
        model: "mock-model",
      });

    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      maxConsecutiveFailures: 2,
      backoffMs: 2_000,
      now: () => nowTs,
      chatCompletionRequest,
    });

    await expect(client.chat("mock-model", [])).rejects.toThrow("f1");
    await expect(client.chat("mock-model", [])).rejects.toThrow("f2");

    const cooldownError = await client.chat("mock-model", []).catch((error) => error as Error);
    expect(cooldownError).toBeInstanceOf(LlmBackoffError);
    expect(chatCompletionRequest).toHaveBeenCalledTimes(2);

    nowTs = 7_001;
    await expect(client.chat("mock-model", [])).resolves.toMatchObject({
      content: "recovered",
      requestId: "req_recovered",
      model: "mock-model",
    });
    expect(chatCompletionRequest).toHaveBeenCalledTimes(3);
  });

  it("resets failure streak after a successful call", async () => {
    const chatCompletionRequest = vi
      .fn()
      .mockRejectedValueOnce(new Error("first-failure"))
      .mockResolvedValueOnce({
        content: "ok",
        requestId: "req_ok",
        model: "mock-model",
      })
      .mockRejectedValueOnce(new Error("second-failure"))
      .mockRejectedValueOnce(new Error("third-failure"));

    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      maxConsecutiveFailures: 2,
      backoffMs: 10_000,
      now: () => 10_000,
      chatCompletionRequest,
    });

    await expect(client.chat("mock-model", [])).rejects.toThrow("first-failure");
    const recovered = await client.chat("mock-model", []);
    expect(recovered).toMatchObject({
      content: "ok",
      requestId: "req_ok",
      model: "mock-model",
    });
    expect(recovered.durationMs).toBeGreaterThanOrEqual(0);
    await expect(client.chat("mock-model", [])).rejects.toThrow("second-failure");
    await expect(client.chat("mock-model", [])).rejects.toThrow("third-failure");

    const cooldownError = await client.chat("mock-model", []).catch((error) => error as Error);
    expect(cooldownError).toBeInstanceOf(LlmBackoffError);
    expect(chatCompletionRequest).toHaveBeenCalledTimes(4);
  });

  it("returns a deterministic fallback response when the tool loop exceeds the round budget", async () => {
    const chatCompletionRequest = vi
      .fn()
      .mockResolvedValueOnce({
        content: "",
        requestId: "req_1",
        model: "mock-model",
        toolCalls: [
          {
            id: "call_1",
            name: "lookup-status",
            arguments: JSON.stringify({ zone: "starter_zone", round: 1 }),
          },
        ],
      })
      .mockResolvedValueOnce({
        content: "",
        requestId: "req_2",
        model: "mock-model",
        toolCalls: [
          {
            id: "call_2",
            name: "lookup-status",
            arguments: JSON.stringify({ zone: "starter_zone", round: 2 }),
          },
        ],
      })
      .mockResolvedValueOnce({
        content: "",
        requestId: "req_3",
        model: "mock-model",
        toolCalls: [
          {
            id: "call_3",
            name: "lookup-status",
            arguments: JSON.stringify({ zone: "starter_zone", round: 3 }),
          },
        ],
      })
      .mockResolvedValueOnce({
        content: "",
        requestId: "req_4",
        model: "mock-model",
        toolCalls: [
          {
            id: "call_4",
            name: "lookup-status",
            arguments: JSON.stringify({ zone: "starter_zone", round: 4 }),
          },
        ],
      });

    const client = createClient({
      baseURL: "http://unit-test.local",
      apiKey: "test-key",
      model: "mock-model",
      chatCompletionRequest,
    });

    const result = await client.chat(
      "mock-model",
      [{ role: "user", content: "keep calling tools" }],
      {
        tools: [
          {
            name: "lookup-status",
            description: "Reads zone status",
            readonly: true,
            parameters: toolSchema.object({ zone: toolSchema.string(), round: toolSchema.number() }),
            result: toolSchema.object({ ok: toolSchema.boolean() }),
            execute: async () => ({ ok: true }),
          },
        ],
        toolContext: {
          latestState: {} as never,
          worldModel: {} as never,
        },
      },
    );

    expect(result.content).toBe(TOOL_LOOP_TRUNCATED_RESPONSE);
    expect(result.toolUsage).toMatchObject({
      rounds: MAX_TOOL_CALL_ROUNDS,
      truncated: true,
    });
    expect(chatCompletionRequest).toHaveBeenCalledTimes(MAX_TOOL_CALL_ROUNDS + 1);
  });
});
