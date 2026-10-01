import { describe, expect, test } from "vitest";
import { Value } from "@sinclair/typebox/value";

import {
  CastIdentityV1,
  CastOutcomeV1,
  CastSessionBeginV1,
  CastSyncV1,
  isCanonicalUint64String,
  isCastSessionBeginV1,
  isCastSyncV1,
} from "../src/cast.js";

const UUID = "00000000-0000-4000-8000-000000000001";

const identity = {
  session_id: UUID,
  session_generation: "1",
  cast_instance_id: "7",
};

function castSync(overrides: Record<string, unknown> = {}) {
  return {
    v: 1,
    identity,
    source: "SKILL_BAR",
    npc: false,
    skill_id: "contract.test",
    av_binding_key: "contract/test",
    target: { entity_uuid: "00000000-0000-4000-8000-000000000002" },
    phase: "CASTING",
    slot: { kind: "skill_bar" },
    duration_ms: 10,
    started_at_ms: 20,
    outcome: "NONE",
    ...overrides,
  };
}

describe("R9 P1 cast canonical contract", () => {
  test("canonical uint64 rejects zero, leading zero, number, and overflow", () => {
    expect(isCanonicalUint64String("1")).toBe(true);
    expect(isCanonicalUint64String("18446744073709551615")).toBe(true);
    expect(isCanonicalUint64String("0")).toBe(false);
    expect(isCanonicalUint64String("01")).toBe(false);
    expect(isCanonicalUint64String(1)).toBe(false);
    expect(isCanonicalUint64String("18446744073709551616")).toBe(false);
  });

  test("identity requires complete three-tuple and rejects unknown fields", () => {
    expect(Value.Check(CastIdentityV1, identity)).toBe(true);
    expect(Value.Check(CastIdentityV1, { ...identity, cast_instance_id: "0" })).toBe(false);
    expect(Value.Check(CastIdentityV1, { ...identity, session_generation: "01" })).toBe(false);
    expect(Value.Check(CastIdentityV1, { ...identity, extra: true })).toBe(false);
    expect(Value.Check(CastIdentityV1, { ...identity, cast_instance_id: 7 })).toBe(false);
    expect(isCanonicalUint64String({ toString: () => "1" })).toBe(false);
  });

  test("accepted source arms pin source, npc, skill, slot, phase, and outcome together", () => {
    expect(Value.Check(CastSyncV1, castSync())).toBe(true);
    expect(
      Value.Check(
        CastSyncV1,
        castSync({ source: "QUICK_SLOT", npc: false, skill_id: null, slot: { kind: "quick_slot", index: 0 } }),
      ),
    ).toBe(true);
    expect(
      Value.Check(
        CastSyncV1,
        castSync({ source: "QUICK_SLOT", npc: false, skill_id: "guessed", slot: { kind: "quick_slot", index: 0 } }),
      ),
    ).toBe(false);
    expect(
      Value.Check(
        CastSyncV1,
        castSync({ source: "DEDICATED", npc: true, slot: { kind: "dedicated" } }),
      ),
    ).toBe(true);
    expect(
      Value.Check(
        CastSyncV1,
        castSync({ phase: "COMPLETE", outcome: "NONE" }),
      ),
    ).toBe(false);
    expect(
      Value.Check(
        CastSyncV1,
        castSync({ phase: "IDLE", outcome: "REJECT_ON_COOLDOWN", source: "DEDICATED", npc: false, skill_id: null, slot: { kind: "dedicated" }, av_binding_key: "cast.reject/reject_on_cooldown" }),
      ),
    ).toBe(true);
  });

  test("every outcome enum variant is represented exactly once", () => {
    const outcomes = [
      "NONE",
      "COMPLETED",
      "INTERRUPT_MOVEMENT",
      "INTERRUPT_CONTAM",
      "INTERRUPT_CONTROL",
      "USER_CANCEL",
      "DEATH",
      "MERIDIAN_GATED",
      "REJECT_QI_INSUFFICIENT",
      "REJECT_ON_COOLDOWN",
      "REJECT_INVALID_TARGET",
      "REJECT_IN_RECOVERY",
      "REJECT_REALM_TOO_LOW",
      "REJECT_NO_WEAPON",
      "REJECT_TECHNIQUE_INACTIVE",
      "REJECT_RACE_MISMATCH",
      "REJECT_SKILL_CONFIG_INVALID",
    ];
    for (const outcome of outcomes) {
      expect(Value.Check(CastOutcomeV1, outcome), outcome).toBe(true);
    }
    expect(Value.Check(CastOutcomeV1, "UNKNOWN")).toBe(false);
  });

  test("BEGIN only accepts the four active/floor and exhaustion combinations", () => {
    const common = {
      v: 1,
      caster: { player_uuid: UUID },
      target_entity_id: 42,
      session_id: UUID,
      session_generation: "1",
    };
    expect(Value.Check(CastSessionBeginV1, { ...common, allocator_exhausted: false })).toBe(true);
    expect(
      Value.Check(CastSessionBeginV1, {
        ...common,
        allocator_exhausted: false,
        active_cast_instance_id: "7",
        minimum_cast_instance_id: "7",
      }),
    ).toBe(true);
    expect(
      Value.Check(CastSessionBeginV1, {
        ...common,
        allocator_exhausted: false,
        active_cast_instance_id: "7",
      }),
    ).toBe(false);
    expect(Value.Check(CastSessionBeginV1, { ...common, allocator_exhausted: true })).toBe(true);
    expect(
      isCastSessionBeginV1({
        ...common,
        allocator_exhausted: false,
        active_cast_instance_id: "7",
        minimum_cast_instance_id: "8",
      }),
    ).toBe(false);
    expect(isCastSessionBeginV1({ ...common, allocator_exhausted: false })).toBe(true);
  });

  test("CastSync contract helper rejects canonical integer overflow and malformed UUID", () => {
    expect(isCastSyncV1(castSync())).toBe(true);
    expect(
      isCastSyncV1(
        castSync({
          identity: { ...identity, cast_instance_id: "18446744073709551616" },
        }),
      ),
    ).toBe(false);
    expect(
      isCastSyncV1(
        castSync({
          identity: { ...identity, session_id: "not-a-uuid" },
        }),
      ),
    ).toBe(false);
  });
});
