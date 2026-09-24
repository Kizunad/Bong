import { describe, expect, it, vi } from "vitest";
import type { ChatSignal } from "@bong/schema";
import {
  buildChatSignalsBlock,
  isRecentSignal,
  mergeChatSignals,
  parseChatMessages,
  parseChatSignalBatch,
  processChatBatch,
} from "../src/chat-processor.js";
import type { LlmClient } from "../src/llm.js";

function createStructuredChatResult(content: string, model: string) {
  return {
    content,
    durationMs: 0,
    requestId: null,
    model,
  };
}

function createTimedSignal(ts: number, overrides: Partial<ChatSignal> = {}): ChatSignal {
  return {
    ts,
    player: "offline:Steve",
    raw: "灵气太少了",
    sentiment: -0.7,
    intent: "complaint",
    influence_weight: 0.8,
    ...overrides,
  };
}

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
        JSON.stringify({
          v: 1,
          ts: -1,
          player: "offline:InvalidClock",
          raw: "hello",
          zone: "spawn",
        }),
      ],
      { warn },
    );

    expect(rows).toHaveLength(1);
    expect(rows[0]?.player).toBe("offline:Steve");
    expect(warn).toHaveBeenCalledTimes(3);
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
      chat: vi.fn(async (model: string) =>
        createStructuredChatResult(
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
          model,
        ),
      ),
    };

    const warn = vi.fn();
    const signals = await processChatBatch({
      annotateClient: llmClient,
      annotateModel: "gpt-5.4-mini",
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
    expect(signals[0]).toMatchObject({ intent: "unknown", ts: 1711111111 });
    expect(signals[1]).toMatchObject({ intent: "unknown", ts: 1711111112 });
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it("maps valid annotation results in original message order", async () => {
    const llmClient: LlmClient = {
      chat: vi.fn(async (model: string) =>
        createStructuredChatResult(
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
              ts: 1,
            },
          ]),
          model,
        ),
      ),
    };

    const signals = await processChatBatch({
      annotateClient: llmClient,
      annotateModel: "gpt-5.4-mini",
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
      ts: 1711111111,
    });
    expect(signals[1]).toMatchObject({
      player: "offline:Alex",
      intent: "social",
      ts: 1711111112,
    });
  });

  it("keeps the exact 300-second boundary and drops 301-second-old signals", () => {
    const nowSeconds = 10_000;
    const boundary = createTimedSignal(nowSeconds - 300, { raw: "边界内" });
    const expired = createTimedSignal(nowSeconds - 301, { raw: "已经过期" });

    expect(isRecentSignal(boundary, nowSeconds)).toBe(true);
    expect(isRecentSignal(expired, nowSeconds)).toBe(false);
    expect(mergeChatSignals([expired, boundary], [], nowSeconds)).toEqual([boundary]);
  });

  it("rejects future signals and expires a server-observed signal across later merge rounds", () => {
    const observedAt = 10_000;
    const observed = createTimedSignal(observedAt, { raw: "可信观察时间" });
    const oneSecondFuture = createTimedSignal(observedAt + 1, { raw: "未来一秒" });
    const farFuture = createTimedSignal(observedAt + 86_400, { raw: "未来一天" });

    expect(isRecentSignal(oneSecondFuture, observedAt)).toBe(false);
    expect(isRecentSignal(farFuture, observedAt)).toBe(false);

    const firstRound = mergeChatSignals([], [oneSecondFuture, farFuture, observed], observedAt);
    expect(firstRound).toEqual([observed]);

    const boundaryRound = mergeChatSignals(firstRound, [], observedAt + 300);
    expect(boundaryRound).toEqual([observed]);
    expect(
      buildChatSignalsBlock({ signals: boundaryRound, nowSeconds: observedAt + 300 }),
    ).toContain(observed.raw);

    const expiredRound = mergeChatSignals(boundaryRound, [], observedAt + 301);
    expect(expiredRound).toEqual([]);
    expect(
      buildChatSignalsBlock({ signals: boundaryRound, nowSeconds: observedAt + 301 }),
    ).toBe("");
  });

  it("uses explicit ts instead of timestamp-like mechanic annotations", () => {
    const nowSeconds = 10_000;
    const recent = createTimedSignal(nowSeconds, { mentions_mechanic: "spirit_qi;ts:1" });
    const expired = createTimedSignal(nowSeconds - 301, { mentions_mechanic: "ts:9999999999" });

    expect(isRecentSignal(recent, nowSeconds)).toBe(true);
    expect(isRecentSignal(expired, nowSeconds)).toBe(false);
  });

  it("filters expired signals before enforcing the 20-signal cap", () => {
    const nowSeconds = 10_000;
    const recent = Array.from({ length: 21 }, (_, index) =>
      createTimedSignal(nowSeconds, { player: `offline:p${index}`, raw: `recent-${index}` }),
    );
    const expired = createTimedSignal(nowSeconds - 301, { raw: "expired-tail" });

    const merged = mergeChatSignals(recent, [expired], nowSeconds);

    expect(merged).toHaveLength(20);
    expect(merged.map((signal) => signal.raw)).toEqual(recent.slice(1).map((signal) => signal.raw));
    expect(merged).not.toContainEqual(expired);
  });

  it("does not render a low-volume expired chat signal into the prompt block", () => {
    const nowSeconds = 10_000;
    const expired = createTimedSignal(nowSeconds - 301, { raw: "旧抱怨不应进入天道上下文" });

    const block = buildChatSignalsBlock({ signals: [expired], nowSeconds });

    expect(block).toBe("");
    expect(block).not.toContain(expired.raw);
  });

  it("uses the explicit annotate model and client route", async () => {
    const annotateClient: LlmClient = {
      chat: vi.fn(async (model: string) =>
        createStructuredChatResult(
          JSON.stringify([
            {
              player: "offline:Steve",
              zone: "spawn",
              raw: "灵气太少了",
              sentiment: -0.5,
              intent: "complaint",
              influence_weight: 0.6,
            },
          ]),
          model,
        ),
      ),
    };

    await processChatBatch({
      annotateClient,
      annotateModel: "gpt-5.4-mini",
      logger: { warn: vi.fn() },
      messages: [
        {
          v: 1,
          ts: 1711111111,
          player: "offline:Steve",
          raw: "灵气太少了",
          zone: "spawn",
        },
      ],
    });

    expect(annotateClient.chat).toHaveBeenCalledWith("gpt-5.4-mini", expect.any(Array));
  });
});
