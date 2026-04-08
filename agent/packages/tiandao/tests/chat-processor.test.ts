import { describe, expect, it, vi } from "vitest";

import {
  ChatProcessor,
  FakeHeuristicChatAnnotator,
  type ChatAnnotator,
  type ChatProcessorLogger,
} from "../src/chat-processor.js";

describe("ChatProcessor", () => {
  it("drops invalid json and schema-invalid chat messages with structured logs", async () => {
    const warn = vi.fn<(message: string, details?: Record<string, unknown>) => void>();
    const logger: ChatProcessorLogger = {
      warn,
    };

    const processor = new ChatProcessor(new FakeHeuristicChatAnnotator(), logger);

    const batch = await processor.processRawEntries([
      "{not-valid-json}",
      JSON.stringify({
        v: 2,
        ts: 1_700_000_000,
        player: "Steve",
        raw: "灵气太少了",
        zone: "spawn",
      }),
      JSON.stringify({
        v: 1,
        ts: 1_700_000_001,
        player: "Alex",
        raw: "灵气太少了",
        zone: "spawn",
      }),
    ]);

    expect(batch.messages).toHaveLength(1);
    expect(batch.messages[0]?.player).toBe("Alex");
    expect(batch.signals).toHaveLength(1);

    expect(warn).toHaveBeenCalledTimes(2);
    expect(warn).toHaveBeenNthCalledWith(
      1,
      "[chat-processor] drop invalid chat json",
      expect.objectContaining({
        reason: "invalid_json",
        index: 0,
      }),
    );
    expect(warn).toHaveBeenNthCalledWith(
      2,
      "[chat-processor] drop schema-invalid chat message",
      expect.objectContaining({
        reason: "schema_invalid",
        index: 1,
        errors: expect.arrayContaining([expect.stringContaining("/v")]),
      }),
    );
  });

  it("produces deterministic heuristic chat signals for offline tests", async () => {
    const processor = new ChatProcessor(new FakeHeuristicChatAnnotator());

    const batch = await processor.processRawEntries([
      JSON.stringify({
        v: 1,
        ts: 1_700_000_002,
        player: "Steve",
        raw: "灵气太少了!",
        zone: "spawn",
      }),
      JSON.stringify({
        v: 1,
        ts: 1_700_000_003,
        player: "Alex",
        raw: "请问怎么提升功德？",
        zone: "spawn",
      }),
      JSON.stringify({
        v: 1,
        ts: 1_700_000_004,
        player: "Eve",
        raw: "哈哈大家早",
        zone: "spawn",
      }),
    ]);

    expect(batch.signals).toEqual([
      {
        player: "Steve",
        raw: "灵气太少了!",
        sentiment: -0.65,
        intent: "complaint",
        mentions_mechanic: "spirit_qi",
        influence_weight: 0.95,
      },
      {
        player: "Alex",
        raw: "请问怎么提升功德？",
        sentiment: -0.15,
        intent: "help",
        mentions_mechanic: "karma",
        influence_weight: 0.8,
      },
      {
        player: "Eve",
        raw: "哈哈大家早",
        sentiment: 0.2,
        intent: "social",
        influence_weight: 0.25,
      },
    ]);
  });

  it("drops schema-invalid chat signals from annotator without crashing batch", async () => {
    const warn = vi.fn<(message: string, details?: Record<string, unknown>) => void>();
    const logger: ChatProcessorLogger = { warn };

    const annotator: ChatAnnotator = {
      async annotate(messages) {
        return messages.map((message, index) =>
          index === 0
            ? {
                player: message.player,
                raw: message.raw,
                sentiment: 2,
                intent: "unknown",
                influence_weight: 0.5,
              }
            : {
                player: message.player,
                raw: message.raw,
                sentiment: 0,
                intent: "unknown",
                influence_weight: 0.5,
              },
        );
      },
    };

    const processor = new ChatProcessor(annotator, logger);
    const batch = await processor.processRawEntries([
      JSON.stringify({
        v: 1,
        ts: 1_700_000_005,
        player: "Steve",
        raw: "普通聊天",
        zone: "spawn",
      }),
      JSON.stringify({
        v: 1,
        ts: 1_700_000_006,
        player: "Alex",
        raw: "普通聊天",
        zone: "spawn",
      }),
    ]);

    expect(batch.messages).toHaveLength(2);
    expect(batch.signals).toEqual([
      {
        player: "Alex",
        raw: "普通聊天",
        sentiment: 0,
        intent: "unknown",
        influence_weight: 0.5,
      },
    ]);

    expect(warn).toHaveBeenCalledWith(
      "[chat-processor] drop schema-invalid chat signal",
      expect.objectContaining({
        reason: "signal_schema_invalid",
        index: 0,
      }),
    );
  });
});
