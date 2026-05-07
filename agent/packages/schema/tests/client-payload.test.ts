import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  ClientNarrationPayloadV1,
  ClientPayloadV1,
  EventAlertPayloadV1,
  HeartbeatPayloadV1,
  LocustSwarmWarningPayloadV1,
  PlayerStatePayloadV1,
  WelcomePayloadV1,
  ZoneInfoPayloadV1,
  getClientPayloadByteLength,
  validateClientPayloadV1,
} from "../src/client-payload.js";
import { MAX_PAYLOAD_BYTES } from "../src/common.js";
import {
  ClientNarrationPayloadV1 as RootClientNarrationPayloadV1,
  ClientPayloadV1 as RootClientPayloadV1,
  EventAlertPayloadV1 as RootEventAlertPayloadV1,
  HeartbeatPayloadV1 as RootHeartbeatPayloadV1,
  LocustSwarmWarningPayloadV1 as RootLocustSwarmWarningPayloadV1,
  PlayerStatePayloadV1 as RootPlayerStatePayloadV1,
  WelcomePayloadV1 as RootWelcomePayloadV1,
  ZoneInfoPayloadV1 as RootZoneInfoPayloadV1,
  getClientPayloadByteLength as rootGetClientPayloadByteLength,
  validateClientPayloadV1 as rootValidateClientPayloadV1,
} from "../src/index.js";
import { validate } from "../src/validate.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

type JsonObject = Record<string, unknown>;

function loadSample<T = unknown>(name: string): T {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf-8")) as T;
}

function loadSampleObject(name: string): JsonObject {
  return loadSample<JsonObject>(name);
}

function expectObject(value: unknown): JsonObject {
  expect(typeof value).toBe("object");
  expect(value).not.toBeNull();
  expect(Array.isArray(value)).toBe(false);

  return value as JsonObject;
}

function expectFirstObject(value: unknown): JsonObject {
  expect(Array.isArray(value)).toBe(true);

  const [firstItem] = value as unknown[];

  expect(firstItem).toBeDefined();

  return expectObject(firstItem);
}

function expectInvalidClientPayload(data: unknown): string[] {
  const result = rootValidateClientPayloadV1(data);

  expect(result.ok, result.errors.join("; ")).toBe(false);
  expect(result.errors.length).toBeGreaterThan(0);

  return result.errors;
}

describe("client payload samples pass schema validation", () => {
  it("client-payload-welcome.sample.json", () => {
    const data = loadSample("client-payload-welcome.sample.json");
    const result = rootValidateClientPayloadV1(data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-payload-heartbeat.sample.json", () => {
    const data = loadSample("client-payload-heartbeat.sample.json");
    const result = rootValidateClientPayloadV1(data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-payload-narration.sample.json", () => {
    const data = loadSample("client-payload-narration.sample.json");
    const result = rootValidateClientPayloadV1(data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-payload-zone-info.sample.json", () => {
    const data = loadSample("client-payload-zone-info.sample.json");
    const result = rootValidateClientPayloadV1(data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-payload-event-alert.sample.json", () => {
    const data = loadSample("client-payload-event-alert.sample.json");
    const result = rootValidateClientPayloadV1(data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-payload-locust-swarm-warning.sample.json", () => {
    const data = loadSample("client-payload-locust-swarm-warning.sample.json");
    const result = rootValidateClientPayloadV1(data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });

  it("client-payload-player-state.sample.json", () => {
    const data = loadSample("client-payload-player-state.sample.json");
    const result = rootValidateClientPayloadV1(data);

    expect(result.ok, result.errors.join("; ")).toBe(true);
  });
});

describe("client payload root exports stay aligned", () => {
  it("re-exports client payload schemas and helpers from package root", () => {
    const welcome = loadSample("client-payload-welcome.sample.json");
    const rawValidation = validate(ClientPayloadV1, welcome);

    expect(RootClientPayloadV1).toBe(ClientPayloadV1);
    expect(RootWelcomePayloadV1).toBe(WelcomePayloadV1);
    expect(RootHeartbeatPayloadV1).toBe(HeartbeatPayloadV1);
    expect(RootClientNarrationPayloadV1).toBe(ClientNarrationPayloadV1);
    expect(RootZoneInfoPayloadV1).toBe(ZoneInfoPayloadV1);
    expect(RootEventAlertPayloadV1).toBe(EventAlertPayloadV1);
    expect(RootLocustSwarmWarningPayloadV1).toBe(LocustSwarmWarningPayloadV1);
    expect(RootPlayerStatePayloadV1).toBe(PlayerStatePayloadV1);
    expect(rootValidateClientPayloadV1).toBe(validateClientPayloadV1);
    expect(rootGetClientPayloadByteLength).toBe(getClientPayloadByteLength);
    expect(rawValidation.ok, rawValidation.errors.join("; ")).toBe(true);
  });
});

describe("client payload schema rejects invalid data", () => {
  it("rejects unknown type", () => {
    const data = loadSampleObject("client-payload-welcome.sample.json");

    data.type = "unknown";

    const errors = expectInvalidClientPayload(data);

    expect(errors.join(" ")).toContain("Expected union value");
  });

  it("rejects wrong version", () => {
    const data = loadSampleObject("client-payload-heartbeat.sample.json");

    data.v = 2;

    expectInvalidClientPayload(data);
  });

  it("rejects client narration payload with more than one narration", () => {
    const data = loadSampleObject("client-payload-narration.sample.json");

    expect(Array.isArray(data.narrations)).toBe(true);

    (data.narrations as unknown[]).push({
      scope: "player",
      target: "offline:Steve",
      text: "第二条叙事不应出现在同一个 payload 中。",
      style: "perception",
    });

    expectInvalidClientPayload(data);
  });

  it("rejects zone_info payload missing nested object", () => {
    const data = loadSampleObject("client-payload-zone-info.sample.json");

    delete data.zone_info;

    expectInvalidClientPayload(data);
  });

  it("rejects event_alert payload missing nested object", () => {
    const data = loadSampleObject("client-payload-event-alert.sample.json");

    delete data.event_alert;

    expectInvalidClientPayload(data);
  });

  it("rejects player_state payload missing nested object", () => {
    const data = loadSampleObject("client-payload-player-state.sample.json");

    delete data.player_state;

    expectInvalidClientPayload(data);
  });

  it("rejects locust_swarm_warning payload without zone", () => {
    const data = loadSampleObject("client-payload-locust-swarm-warning.sample.json");

    delete data.zone;

    expectInvalidClientPayload(data);
  });

  it("rejects payloads whose serialized form exceeds the shared byte budget", () => {
    const data = loadSampleObject("client-payload-event-alert.sample.json");
    const eventAlert = expectObject(data.event_alert);
    const oversizedDetail = "x".repeat(MAX_PAYLOAD_BYTES + 1);
    const oversizedPayload = Object.assign(
      Object.create({
        toJSON() {
          return {
            ...data,
            event_alert: {
              ...eventAlert,
              detail: oversizedDetail,
            },
          };
        },
      }),
      data,
    );

    const rawValidation = validate(ClientPayloadV1, oversizedPayload);

    expect(rawValidation.ok, rawValidation.errors.join("; ")).toBe(true);

    const byteLength = rootGetClientPayloadByteLength(oversizedPayload);

    expect(byteLength).toBeGreaterThan(MAX_PAYLOAD_BYTES);

    const errors = expectInvalidClientPayload(oversizedPayload);

    expect(errors[0]).toContain(`serialized payload exceeds ${MAX_PAYLOAD_BYTES} bytes`);
  });

  it("keeps valid samples under the byte budget", () => {
    const samples = [
      "client-payload-welcome.sample.json",
      "client-payload-heartbeat.sample.json",
      "client-payload-narration.sample.json",
      "client-payload-zone-info.sample.json",
      "client-payload-event-alert.sample.json",
      "client-payload-locust-swarm-warning.sample.json",
      "client-payload-player-state.sample.json",
    ];

    for (const sampleName of samples) {
      const payload = loadSample(sampleName);

      expect(rootGetClientPayloadByteLength(payload)).toBeLessThanOrEqual(MAX_PAYLOAD_BYTES);
    }
  });

  it("keeps client narration payload pinned to one narration item", () => {
    const data = loadSampleObject("client-payload-narration.sample.json");
    const firstNarration = expectFirstObject(data.narrations);

    expect(firstNarration.style).toBe("system_warning");
    expect((data.narrations as unknown[]).length).toBe(1);
  });
});
