import { describe, expect, it, vi } from "vitest";

import {
  LlmBackoffError,
  LlmTimeoutError,
  MOCK_LLM_RESPONSE,
  createClient,
  createMockClient,
} from "../src/llm.js";

describe("createMockClient", () => {
  it("returns deterministic default response", async () => {
    const client = createMockClient();
    const raw = await client.chat("mock-model", [
      { role: "system", content: "system" },
      { role: "user", content: "user" },
    ]);

    expect(raw).toBe(MOCK_LLM_RESPONSE);
    expect(JSON.parse(raw)).toEqual({
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

    await expect(client.chat("any", [])).resolves.toBe(response);
  });
});

describe("createClient runtime guard seam", () => {
  it("times out a single chat call with bounded timeout", async () => {
    vi.useFakeTimers();

    try {
      const chatCompletionRequest = vi.fn(
        ({ signal }: { signal: AbortSignal }) =>
          new Promise<string>((_resolve, reject) => {
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
      .mockResolvedValueOnce("recovered");

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
    await expect(client.chat("mock-model", [])).resolves.toBe("recovered");
    expect(chatCompletionRequest).toHaveBeenCalledTimes(3);
  });

  it("resets failure streak after a successful call", async () => {
    const chatCompletionRequest = vi
      .fn()
      .mockRejectedValueOnce(new Error("first-failure"))
      .mockResolvedValueOnce("ok")
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
    await expect(client.chat("mock-model", [])).resolves.toBe("ok");
    await expect(client.chat("mock-model", [])).rejects.toThrow("second-failure");
    await expect(client.chat("mock-model", [])).rejects.toThrow("third-failure");

    const cooldownError = await client.chat("mock-model", []).catch((error) => error as Error);
    expect(cooldownError).toBeInstanceOf(LlmBackoffError);
    expect(chatCompletionRequest).toHaveBeenCalledTimes(4);
  });
});
