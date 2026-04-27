import { describe, expect, it } from "vitest";

import {
  ExtractAbortedV1,
  ExtractCompletedV1,
  ExtractFailedV1,
  ExtractProgressV1,
  ExtractStartedV1,
  RiftPortalStateV1,
  TsyCollapseStartedIpcV1,
  validateExtractStartedV1Contract,
} from "../src/extract-v1.js";
import {
  ServerDataExtractAbortedV1,
  ServerDataExtractCompletedV1,
  ServerDataExtractFailedV1,
  ServerDataExtractProgressV1,
  ServerDataExtractStartedV1,
  ServerDataRiftPortalStateV1,
  ServerDataTsyCollapseStartedIpcV1,
  ServerDataV1,
} from "../src/server-data.js";
import { validate } from "../src/validate.js";

describe("plan-tsy-extract-v1 §4.1 extract-v1 schema", () => {
  it("accepts portal state payloads", () => {
    expect(validate(RiftPortalStateV1, {
      entity_id: 42,
      kind: "collapse_tear",
      family_id: "tsy_lingxu_01",
      world_pos: [1, 2, 3],
      current_extract_ticks: 60,
      activation_window_end: 1234,
    }).ok).toBe(true);
  });

  it("accepts started/progress/completed/aborted/failed payloads", () => {
    expect(validateExtractStartedV1Contract({
      player_id: "offline:Kiz",
      portal_entity_id: 42,
      portal_kind: "main_rift",
      required_ticks: 160,
      at_tick: 100,
    }).ok).toBe(true);
    expect(validate(ExtractProgressV1, {
      player_id: "offline:Kiz",
      portal_entity_id: 42,
      elapsed_ticks: 5,
      required_ticks: 160,
    }).ok).toBe(true);
    expect(validate(ExtractCompletedV1, {
      player_id: "offline:Kiz",
      portal_kind: "main_rift",
      family_id: "tsy_lingxu_01",
      exit_world_pos: [0, 65, 0],
      at_tick: 260,
    }).ok).toBe(true);
    expect(validate(ExtractAbortedV1, { player_id: "offline:Kiz", reason: "damaged" }).ok).toBe(true);
    expect(validate(ExtractFailedV1, { player_id: "offline:Kiz", reason: "spirit_qi_drained" }).ok).toBe(true);
  });

  it("rejects unknown abort reasons and portal kinds", () => {
    expect(validate(ExtractAbortedV1, { player_id: "offline:Kiz", reason: "hungry" }).ok).toBe(false);
    expect(validate(ExtractStartedV1, {
      player_id: "offline:Kiz",
      portal_entity_id: 42,
      portal_kind: "blue_door",
      required_ticks: 160,
      at_tick: 100,
    }).ok).toBe(false);
  });

  it("accepts collapse started IPC payload", () => {
    expect(validate(TsyCollapseStartedIpcV1, {
      family_id: "tsy_lingxu_01",
      at_tick: 100,
      remaining_ticks: 600,
      collapse_tear_entity_ids: [1, 2, 3],
    }).ok).toBe(true);
  });

  it("accepts extract payloads as server_data envelopes", () => {
    const payloads = [
      {
        schema: ServerDataRiftPortalStateV1,
        value: {
          v: 1,
          type: "rift_portal_state",
          entity_id: 42,
          kind: "main_rift",
          family_id: "tsy_lingxu_01",
          world_pos: [1, 2, 3],
          current_extract_ticks: 160,
        },
      },
      {
        schema: ServerDataExtractStartedV1,
        value: {
          v: 1,
          type: "extract_started",
          player_id: "offline:Kiz",
          portal_entity_id: 42,
          portal_kind: "main_rift",
          required_ticks: 160,
          at_tick: 100,
        },
      },
      {
        schema: ServerDataExtractProgressV1,
        value: {
          v: 1,
          type: "extract_progress",
          player_id: "offline:Kiz",
          portal_entity_id: 42,
          elapsed_ticks: 5,
          required_ticks: 160,
        },
      },
      {
        schema: ServerDataExtractCompletedV1,
        value: {
          v: 1,
          type: "extract_completed",
          player_id: "offline:Kiz",
          portal_kind: "main_rift",
          family_id: "tsy_lingxu_01",
          exit_world_pos: [0, 65, 0],
          at_tick: 260,
        },
      },
      {
        schema: ServerDataExtractAbortedV1,
        value: { v: 1, type: "extract_aborted", player_id: "offline:Kiz", reason: "portal_expired" },
      },
      {
        schema: ServerDataExtractFailedV1,
        value: { v: 1, type: "extract_failed", player_id: "offline:Kiz", reason: "spirit_qi_drained" },
      },
      {
        schema: ServerDataTsyCollapseStartedIpcV1,
        value: {
          v: 1,
          type: "tsy_collapse_started_ipc",
          family_id: "tsy_lingxu_01",
          at_tick: 100,
          remaining_ticks: 600,
          collapse_tear_entity_ids: [1, 2, 3],
        },
      },
    ];

    for (const { schema, value } of payloads) {
      expect(validate(schema, value).ok).toBe(true);
      expect(validate(ServerDataV1, value).ok).toBe(true);
    }
  });
});
