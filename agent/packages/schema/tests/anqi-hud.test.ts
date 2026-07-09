import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  AnqiHudV1,
  ServerDataAnqiHudV1,
  ServerDataType,
  ServerDataV1,
} from "../src/server-data.js";
import {
  assertGeneratedSchemasFresh,
  getGeneratedSchemaDrift,
  renderGeneratedSchemas,
  writeGeneratedSchemas,
} from "../src/generated-artifacts.js";
import {
  GENERATED_SCHEMA_FILES,
  SCHEMA_REGISTRY,
} from "../src/schema-registry.js";
import { validate } from "../src/validate.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");
const rustServerDataPath = join(
  __dirname,
  "..",
  "..",
  "..",
  "..",
  "server",
  "src",
  "schema",
  "server_data.rs",
);

const ANQI_HUD_FIELDS = [
  "kind",
  "echo_count",
  "aim_progress",
  "charge_progress",
  "abrasion_container",
  "abrasion_qi_payload",
  "tick",
] as const;

function loadSample(name: string): unknown {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf8"));
}

describe("anqi_hud ServerDataV1 contract", () => {
  const validSamples = [
    "server-data.anqi-hud.echo.sample.json",
    "server-data.anqi-hud.charge.sample.json",
    "server-data.anqi-hud.abrasion.sample.json",
  ];
  const invalidSamples = [
    "server-data.anqi-hud.invalid-missing-field.sample.json",
    "server-data.anqi-hud.invalid-extra-field.sample.json",
    "server-data.anqi-hud.invalid-numeric.sample.json",
    "server-data.anqi-hud.invalid-kind.sample.json",
  ];

  it.each(validSamples)("accepts positive sample %s", (sampleName) => {
    const sample = loadSample(sampleName);
    const payloadResult = validate(ServerDataAnqiHudV1, sample);
    const unionResult = validate(ServerDataV1, sample);

    expect(
      payloadResult.ok,
      `${sampleName} should satisfy ServerDataAnqiHudV1: ${payloadResult.errors.join("; ")}`,
    ).toBe(true);
    expect(
      unionResult.ok,
      `${sampleName} should satisfy ServerDataV1: ${unionResult.errors.join("; ")}`,
    ).toBe(true);
  });

  it.each(invalidSamples)("rejects negative sample %s", (sampleName) => {
    const sample = loadSample(sampleName);
    expect(validate(ServerDataAnqiHudV1, sample).ok).toBe(false);
    expect(validate(ServerDataV1, sample).ok).toBe(false);
  });

  it("pins reserved aim and production multishot kinds at numeric boundaries", () => {
    for (const [kind, echoCount, aimProgress] of [
      ["aim", 0, 1],
      ["multishot", 4294967295, 0],
    ] as const) {
      const result = validate(ServerDataV1, {
        v: 1,
        type: "anqi_hud",
        kind,
        echo_count: echoCount,
        aim_progress: aimProgress,
        charge_progress: 1,
        abrasion_container: "",
        abrasion_qi_payload: 0,
        tick: 0,
      });
      expect(
        result.ok,
        `${kind} boundary payload should be accepted: ${result.errors.join("; ")}`,
      ).toBe(true);
    }
  });

  it("registers anqi_hud in ServerDataType and rejects unknown type", () => {
    expect(validate(ServerDataType, "anqi_hud").ok).toBe(true);
    expect(validate(ServerDataType, "anqi_hud_v2").ok).toBe(false);
  });

  it("keeps Rust AnqiHudV1 and TypeBox field sets in exact parity", () => {
    const rustSource = readFileSync(rustServerDataPath, "utf8");
    const structMatch = rustSource.match(
      /pub struct AnqiHudV1\s*\{(?<body>[\s\S]*?)\n\}/,
    );
    expect(structMatch, "Rust AnqiHudV1 struct must remain discoverable").not.toBeNull();

    const rustFields = [
      ...(structMatch?.groups?.body ?? "").matchAll(/pub\s+([a-z_]+):/g),
    ].map((match) => match[1]);

    expect(rustFields).toEqual(ANQI_HUD_FIELDS);
    expect(Object.keys(AnqiHudV1.properties)).toEqual(ANQI_HUD_FIELDS);
    expect(Object.keys(ServerDataAnqiHudV1.properties)).toEqual([
      "v",
      "type",
      ...ANQI_HUD_FIELDS,
    ]);
  });

  it("pins the Rust/TS wrapper type and every required wire field", () => {
    const rustSource = readFileSync(rustServerDataPath, "utf8");
    expect(rustSource).toContain("AnqiHud(AnqiHudV1)");
    expect(rustSource).toContain('label, "anqi_hud"');

    const generatedWrapper = JSON.parse(
      renderGeneratedSchemas()["server-data-anqi-hud-v1.json"],
    ) as {
      properties: Record<string, unknown>;
      required: string[];
      additionalProperties: boolean;
    };
    expect(Object.keys(generatedWrapper.properties)).toEqual([
      "v",
      "type",
      ...ANQI_HUD_FIELDS,
    ]);
    expect(generatedWrapper.required).toEqual([
      "v",
      "type",
      ...ANQI_HUD_FIELDS,
    ]);
    expect(generatedWrapper.additionalProperties).toBe(false);
    expect(renderGeneratedSchemas()["server-data-v1.json"]).toContain(
      '"const": "anqi_hud"',
    );
  });

  it("registers standalone and wrapped generated artifacts", () => {
    expect(SCHEMA_REGISTRY.anqiHudV1).toBe(AnqiHudV1);
    expect(SCHEMA_REGISTRY.serverDataAnqiHudV1).toBe(ServerDataAnqiHudV1);
    expect(GENERATED_SCHEMA_FILES["anqi-hud-v1.json"]).toBe(AnqiHudV1);
    expect(GENERATED_SCHEMA_FILES["server-data-anqi-hud-v1.json"]).toBe(
      ServerDataAnqiHudV1,
    );
  });

  it("fails freshness when server-data-anqi-hud-v1.json is deleted", () => {
    const outputDir = mkdtempSync(join(tmpdir(), "bong-anqi-hud-schema-"));
    try {
      writeGeneratedSchemas(outputDir);
      rmSync(join(outputDir, "server-data-anqi-hud-v1.json"));

      expect(getGeneratedSchemaDrift(outputDir).missing).toContain(
        "server-data-anqi-hud-v1.json",
      );
      expect(() => assertGeneratedSchemasFresh(outputDir)).toThrowError(
        /server-data-anqi-hud-v1\.json/,
      );
    } finally {
      rmSync(outputDir, { recursive: true, force: true });
    }
  });
});
