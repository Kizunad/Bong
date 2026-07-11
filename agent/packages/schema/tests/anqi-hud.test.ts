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

interface WireCorpusCase {
  name: string;
  accepted: boolean;
  set?: Record<string, unknown>;
  remove?: string;
  repeat?: {
    field: string;
    value: string;
    count: number;
  };
}

interface WireCorpus {
  base: Record<string, unknown>;
  cases: WireCorpusCase[];
}

function loadWireCorpus(): WireCorpus {
  return loadSample("server-data.anqi-hud.wire-corpus.json") as WireCorpus;
}

function materializeCorpusCase(
  base: Record<string, unknown>,
  testCase: WireCorpusCase,
): Record<string, unknown> {
  const payload = { ...base, ...testCase.set };
  if (testCase.remove !== undefined) {
    delete payload[testCase.remove];
  }
  if (testCase.repeat !== undefined) {
    payload[testCase.repeat.field] = testCase.repeat.value.repeat(
      testCase.repeat.count,
    );
  }
  return payload;
}

describe("anqi_hud ServerDataV1 contract", () => {
  const validSamples = [
    "server-data.anqi-hud.echo.sample.json",
    "server-data.anqi-hud.aim.sample.json",
    "server-data.anqi-hud.charge.sample.json",
    "server-data.anqi-hud.abrasion.sample.json",
    "server-data.anqi-hud.multishot.sample.json",
  ];
  const invalidSamples = [
    "server-data.anqi-hud.invalid-missing-field.sample.json",
    "server-data.anqi-hud.invalid-extra-field.sample.json",
    "server-data.anqi-hud.invalid-kind.sample.json",
    "server-data.anqi-hud.invalid-tick-overflow.sample.json",
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

  it("matches every isolated shared wire corpus case", () => {
    const corpus = loadWireCorpus();
    const names = new Set<string>();

    for (const testCase of corpus.cases) {
      expect(names.has(testCase.name), `duplicate corpus case ${testCase.name}`).toBe(
        false,
      );
      names.add(testCase.name);

      const mutationCount =
        (testCase.set === undefined ? 0 : Object.keys(testCase.set).length) +
        (testCase.remove === undefined ? 0 : 1) +
        (testCase.repeat === undefined ? 0 : 1);
      expect(
        mutationCount,
        `${testCase.name} must isolate at most one field constraint`,
      ).toBeLessThanOrEqual(1);

      const payload = materializeCorpusCase(corpus.base, testCase);
      const wrapperResult = validate(ServerDataAnqiHudV1, payload);
      const unionResult = validate(ServerDataV1, payload);
      expect(
        wrapperResult.ok,
        `${testCase.name} wrapper result: ${wrapperResult.errors.join("; ")}`,
      ).toBe(testCase.accepted);
      expect(
        unionResult.ok,
        `${testCase.name} union result: ${unionResult.errors.join("; ")}`,
      ).toBe(testCase.accepted);
    }
  });

  it.each([
    ["aim_progress", Number.NaN],
    ["aim_progress", Number.POSITIVE_INFINITY],
    ["charge_progress", Number.NEGATIVE_INFINITY],
    ["abrasion_qi_payload", Number.NaN],
    ["abrasion_qi_payload", Number.POSITIVE_INFINITY],
  ] as const)("rejects non-finite %s=%s", (field, value) => {
    const corpus = loadWireCorpus();
    const payload = { ...corpus.base, [field]: value };
    expect(validate(ServerDataAnqiHudV1, payload).ok).toBe(false);
    expect(validate(ServerDataV1, payload).ok).toBe(false);
  });

  it("registers anqi_hud in ServerDataType and rejects unknown type", () => {
    expect(validate(ServerDataType, "anqi_hud").ok).toBe(true);
    expect(validate(ServerDataType, "anqi_hud_v2").ok).toBe(false);
  });

  it("pins every TypeBox field and generated JSON constraint", () => {
    expect(Object.keys(AnqiHudV1.properties)).toEqual(ANQI_HUD_FIELDS);
    expect(Object.keys(ServerDataAnqiHudV1.properties)).toEqual([
      "v",
      "type",
      ...ANQI_HUD_FIELDS,
    ]);

    const generatedWrapper = JSON.parse(
      renderGeneratedSchemas()["server-data-anqi-hud-v1.json"],
    ) as {
      properties: Record<string, Record<string, unknown>>;
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
    expect(generatedWrapper.properties.v).toEqual({ const: 1, type: "number" });
    expect(generatedWrapper.properties.type).toEqual({
      const: "anqi_hud",
      type: "string",
    });
    expect(generatedWrapper.properties.kind).toEqual({
      anyOf: ["echo", "aim", "charge", "abrasion", "multishot"].map(
        (kind) => ({ const: kind, type: "string" }),
      ),
    });
    expect(generatedWrapper.properties.echo_count).toEqual({
      minimum: 0,
      maximum: 4_294_967_295,
      type: "integer",
    });
    expect(generatedWrapper.properties.aim_progress).toEqual({
      minimum: 0,
      maximum: 1,
      type: "number",
    });
    expect(generatedWrapper.properties.charge_progress).toEqual({
      minimum: 0,
      maximum: 1,
      type: "number",
    });
    expect(generatedWrapper.properties.abrasion_container).toEqual({
      maxLength: 32_768,
      type: "string",
    });
    expect(generatedWrapper.properties.abrasion_qi_payload).toEqual({
      minimum: 0,
      type: "number",
    });
    expect(generatedWrapper.properties.tick).toEqual({
      minimum: 0,
      maximum: Number.MAX_SAFE_INTEGER,
      type: "integer",
    });
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
