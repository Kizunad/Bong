import { describe, expect, it } from "vitest";
import { ClientRequestV1 } from "../src/client-request.js";
import { validate } from "../src/validate.js";

describe("technique binding contract", () => {
  it("carries the confirmed old binding for both destinations and rejects unavailable slots", () => {
    const request = { v: 1, type: "technique_bind", skill_id: "movement.dash", expected_binding: "skill:movement.dash" };
    for (const target of [{ kind: "combat", slot: 0 }, { kind: "dash" }]) {
      expect(validate(ClientRequestV1, { ...request, target }).ok).toBe(true);
    }
    for (const target of [{ kind: "combat", slot: 2 }, { kind: "dash", slot: 0 }, { kind: "unknown" }]) {
      expect(validate(ClientRequestV1, { ...request, target }).ok).toBe(false);
    }
    expect(validate(ClientRequestV1, { v: 1, type: "technique_bind", skill_id: "movement.dash", target: { kind: "dash" } }).ok).toBe(false);
  });
});
