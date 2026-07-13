import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { validateAgentCommandV1Contract } from "../src/agent-command.js";
import { ElderEncounterEventV1 } from "../src/elder-encounter.js";
import { MeridianSeveredEventV1 } from "../src/meridian-severed.js";
import { validateNarrationV1Contract } from "../src/narration.js";
import { FactionWarEventV1, NamedFactionStateV1 } from "../src/npc.js";
import { TuikeAshDecayV1 } from "../src/tuike-v2.js";
import { validateWorldStateV1Contract } from "../src/world-state.js";
import {
  assertGeneratedSchemasFresh,
  GENERATED_DIR,
  getGeneratedSchemaDrift,
  renderGeneratedSchemas,
  writeGeneratedSchemas,
} from "../src/generated-artifacts.js";
import { GENERATED_SCHEMA_FILES } from "../src/schema-registry.js";

const tempDirs: string[] = [];

function createTempDir(): string {
  const directory = mkdtempSync(join(tmpdir(), "bong-schema-"));
  tempDirs.push(directory);
  return directory;
}

afterEach(() => {
  for (const directory of tempDirs.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

describe("generated schema freshness gate", () => {
  it("keeps committed generated artifacts fresh", () => {
    expect(() => assertGeneratedSchemasFresh(GENERATED_DIR)).not.toThrow();
  });

  it("fails on drift and passes again after regeneration", () => {
    const outputDir = createTempDir();
    writeGeneratedSchemas(outputDir);

    const changedFile = join(outputDir, "chat-message-v1.json");
    const staleContent = readFileSync(changedFile, "utf8").replace(/\n$/, "\n\n");
    writeFileSync(changedFile, staleContent);

    const missingFile = join(outputDir, "narration-v1.json");
    rmSync(missingFile);

    const unexpectedFile = join(outputDir, "unexpected.json");
    writeFileSync(unexpectedFile, "{}\n");

    expect(() => assertGeneratedSchemasFresh(outputDir)).toThrowError(
      /Generated schema artifacts are out of date/,
    );
    expect(getGeneratedSchemaDrift(outputDir)).toEqual({
      missing: ["narration-v1.json"],
      changed: ["chat-message-v1.json"],
      unexpected: ["unexpected.json"],
    });

    writeGeneratedSchemas(outputDir);

    expect(existsSync(unexpectedFile)).toBe(false);
    expect(() => assertGeneratedSchemasFresh(outputDir)).not.toThrow();
  });

  it("uses a stable generated snapshot even if runtime schema objects are mutated", () => {
    const expectedChatSchema = renderGeneratedSchemas()["chat-message-v1.json"];
    const schema = GENERATED_SCHEMA_FILES["chat-message-v1.json"] as Record<string, unknown>;
    const originalType = schema.type;

    schema.type = "mutated-at-runtime";

    try {
      expect(renderGeneratedSchemas()["chat-message-v1.json"]).toBe(expectedChatSchema);
      expect(getGeneratedSchemaDrift(GENERATED_DIR)).toEqual({
        missing: [],
        changed: [],
        unexpected: [],
      });
      expect(() => assertGeneratedSchemasFresh(GENERATED_DIR)).not.toThrow();
    } finally {
      schema.type = originalType;
    }
  });

  it("registers every runtime-consumed server-to-agent Redis V1 contract", () => {
    const runtimeContracts = {
      "elder-encounter-event-v1.json": ElderEncounterEventV1,
      "faction-war-event-v1.json": FactionWarEventV1,
      "meridian-severed-event-v1.json": MeridianSeveredEventV1,
      "named-faction-state-v1.json": NamedFactionStateV1,
      "tuike-ash-decay-v1.json": TuikeAshDecayV1,
    } as const;

    for (const [fileName, contract] of Object.entries(runtimeContracts)) {
      expect(
        GENERATED_SCHEMA_FILES[fileName as keyof typeof GENERATED_SCHEMA_FILES],
        `${fileName} is consumed by Tiandao runtime and must remain freshness-gated`,
      ).toBe(contract);
      expect(renderGeneratedSchemas()[fileName], `${fileName} must be rendered`).toBe(
        `${JSON.stringify(contract, null, 2)}\n`,
      );
    }
  });

  it("fails freshness when a runtime-consumed contract artifact is missing", () => {
    const outputDir = createTempDir();
    writeGeneratedSchemas(outputDir);
    rmSync(join(outputDir, "meridian-severed-event-v1.json"));

    expect(getGeneratedSchemaDrift(outputDir).missing).toContain(
      "meridian-severed-event-v1.json",
    );
    expect(() => assertGeneratedSchemasFresh(outputDir)).toThrowError(
      /meridian-severed-event-v1\.json/,
    );
  });

  it("runtime Redis V1 parity helpers do not introduce generated schema drift", () => {
    expect(
      validateWorldStateV1Contract({
        v: 2,
        ts: 1712345678,
        tick: 84000,
        players: [],
        npcs: [],
        zones: [],
        recent_events: [],
      }).ok,
    ).toBe(false);
    expect(
      validateAgentCommandV1Contract({
        v: 1,
        id: "cmd_bad",
        commands: [{ type: "spawn_event", target: "blood_valley", params: [] }],
      }).ok,
    ).toBe(false);
    expect(
      validateNarrationV1Contract({
        v: 1,
        narrations: [{ scope: "player", text: "天雷将至。", style: "system_warning" }],
      }).ok,
    ).toBe(false);

    expect(getGeneratedSchemaDrift(GENERATED_DIR)).toEqual({
      missing: [],
      changed: [],
      unexpected: [],
    });
    expect(() => assertGeneratedSchemasFresh(GENERATED_DIR)).not.toThrow();
  });
});
