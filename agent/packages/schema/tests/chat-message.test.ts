import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { ChatSignal } from "../src/chat-message.js";
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
  it("accepts a non-negative integer observation timestamp", () => {
    const result = validate(ChatSignal, VALID_CHAT_SIGNAL);

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
