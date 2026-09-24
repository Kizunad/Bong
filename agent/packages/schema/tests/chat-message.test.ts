import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { ChatMessageV1, ChatSignal } from "../src/chat-message.js";
import { validate } from "../src/validate.js";

const VALID_CHAT_SIGNAL = {
  ts: 1_711_111_111,
  player: "offline:Steve",
  raw: "灵气太少了",
  sentiment: -0.7,
  intent: "complaint",
  mentions_mechanic: "spirit_qi",
  influence_weight: 0.8,
};

describe("ChatSignal timestamp contract", () => {
  it.each([0, 1_711_111_111])("accepts non-negative integer observation timestamp %d", (ts) => {
    const result = validate(ChatSignal, { ...VALID_CHAT_SIGNAL, ts });

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  const { ts: _ts, ...missingTimestamp } = VALID_CHAT_SIGNAL;

  it.each([
    ["missing", missingTimestamp],
    ["negative", { ...VALID_CHAT_SIGNAL, ts: -1 }],
    ["fractional", { ...VALID_CHAT_SIGNAL, ts: 1.5 }],
    ["string", { ...VALID_CHAT_SIGNAL, ts: "1711111111" }],
  ])("rejects %s timestamps", (_caseName, candidate) => {
    const result = validate(ChatSignal, candidate);

    expect(result.ok, `${_caseName} ts must be rejected`).toBe(false);
  });

  it("rejects additional fields", () => {
    const result = validate(ChatSignal, { ...VALID_CHAT_SIGNAL, unexpected: true });

    expect(result.ok, "ChatSignal must stay closed to unknown fields").toBe(false);
  });

  it("pins ts as required in the committed generated schema", () => {
    const generated = JSON.parse(
      readFileSync(new URL("../generated/chat-signal.json", import.meta.url), "utf8"),
    ) as { required?: string[]; properties?: Record<string, unknown> };

    expect(generated.required).toContain("ts");
    expect(generated.properties?.ts).toMatchObject({ type: "integer", minimum: 0 });
  });
});

describe("ChatMessageV1 timestamp contract", () => {
  const validMessage = {
    v: 1,
    ts: 0,
    player: "offline:Steve",
    raw: "灵气太少了",
    zone: "spawn",
  };

  it("accepts the zero boundary used by an epoch-starting source", () => {
    const result = validate(ChatMessageV1, validMessage);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it.each([-1, 1.5, "1711111111"])("rejects invalid source timestamp %s", (ts) => {
    const result = validate(ChatMessageV1, { ...validMessage, ts });

    expect(result.ok, `source ts=${String(ts)} must be rejected`).toBe(false);
  });

  it("pins ts as a server-observed Unix second in the committed generated schema", () => {
    const generated = JSON.parse(
      readFileSync(new URL("../generated/chat-message-v1.json", import.meta.url), "utf8"),
    ) as { properties?: Record<string, unknown> };

    expect(generated.properties?.ts).toMatchObject({
      type: "integer",
      minimum: 0,
      description: "Server-observed Unix timestamp (seconds)",
    });
  });
});
