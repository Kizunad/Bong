import { describe, expect, it, vi } from "vitest";
import { parseChatMessages, parseChatSignalBatch, processChatBatch } from "../src/chat-processor.js";
import type { LlmClient } from "../src/llm.js";

describe("chat-processor", () => {
  it("parses valid chat messages and drops invalid payloads", () => {
    const warn = vi.fn();
    const rows = parseChatMessages(
      [
        JSON.stringify({
          v: 1,
          ts: 1711111111,
          player: "offline:Steve",
          raw: "灵气太少了",
          zone: "spawn",
        }),
        "{bad-json}",
        JSON.stringify({
          v: 2,
          ts: 1711111112,
          player: "offline:Alex",
          raw: "hello",
          zone: "spawn",
        }),
      ],
      { warn },
    );

    expect(rows).toHaveLength(1);
    expect(rows[0]?.player).toBe("offline:Steve");
    expect(warn).toHaveBeenCalledTimes(2);
  });

  it("extracts chat signal rows from markdown code block", () => {
    const warn = vi.fn();
    const rows = parseChatSignalBatch(
      [
        "```json",
        JSON.stringify([
          {
            player: "offline:Steve",
            zone: "spawn",
            raw: "灵气太少了",
            sentiment: -0.6,
            intent: "complaint",
            influence_weight: 0.7,
          },
        ]),
        "```",
      ].join("\n"),
      { warn },
    );

    expect(rows).toHaveLength(1);
    expect(rows[0]?.intent).toBe("complaint");
    expect(warn).not.toHaveBeenCalled();
  });

  it("falls back to unknown for missing rows or invalid fields", async () => {
    const llmClient: LlmClient = {
      chat: vi.fn(async () =>
        JSON.stringify([
          {
            player: "offline:Steve",
            zone: "spawn",
            raw: "灵气太少了",
            sentiment: 1.5,
            intent: "complaint",
            influence_weight: 0.7,
          },
        ]),
      ),
    };

    const warn = vi.fn();
    const signals = await processChatBatch({
      llmClient,
      model: "mock",
      logger: { warn },
      messages: [
        {
          v: 1,
          ts: 1711111111,
          player: "offline:Steve",
          raw: "灵气太少了",
          zone: "spawn",
        },
        {
          v: 1,
          ts: 1711111112,
          player: "offline:Alex",
          raw: "路过看看",
          zone: "spawn",
        },
      ],
    });

    expect(signals).toHaveLength(2);
    expect(signals[0]?.intent).toBe("unknown");
    expect(signals[1]?.intent).toBe("unknown");
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it("maps valid annotation results in original message order", async () => {
    const llmClient: LlmClient = {
      chat: vi.fn(async () =>
        JSON.stringify([
          {
            player: "offline:Alex",
            zone: "spawn",
            raw: "路过看看",
            sentiment: 0.2,
            intent: "social",
            influence_weight: 0.1,
          },
          {
            player: "offline:Steve",
            zone: "spawn",
            raw: "灵气太少了",
            sentiment: -0.7,
            intent: "complaint",
            influence_weight: 0.8,
            mentions_mechanic: "spirit_qi",
          },
        ]),
      ),
    };

    const signals = await processChatBatch({
      llmClient,
      model: "mock",
      logger: { warn: vi.fn() },
      messages: [
        {
          v: 1,
          ts: 1711111111,
          player: "offline:Steve",
          raw: "灵气太少了",
          zone: "spawn",
        },
        {
          v: 1,
          ts: 1711111112,
          player: "offline:Alex",
          raw: "路过看看",
          zone: "spawn",
        },
      ],
    });

    expect(signals).toHaveLength(2);
    expect(signals[0]).toMatchObject({
      player: "offline:Steve",
      intent: "complaint",
      mentions_mechanic: "spirit_qi",
    });
    expect(signals[1]).toMatchObject({
      player: "offline:Alex",
      intent: "social",
    });
  });
});
