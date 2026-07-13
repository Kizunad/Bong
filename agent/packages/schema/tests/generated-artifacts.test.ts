import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import ts from "typescript";
import { afterEach, describe, expect, it } from "vitest";

import { validateAgentCommandV1Contract } from "../src/agent-command.js";
import { validateNarrationV1Contract } from "../src/narration.js";
import { validateWorldStateV1Contract } from "../src/world-state.js";
import * as schemaExports from "../src/index.js";
import {
  assertGeneratedSchemasFresh,
  GENERATED_DIR,
  getGeneratedSchemaDrift,
  renderGeneratedSchemas,
  writeGeneratedSchemas,
} from "../src/generated-artifacts.js";
import { GENERATED_SCHEMA_FILES, SCHEMA_REGISTRY } from "../src/schema-registry.js";

const tempDirs: string[] = [];
const TIANDAO_SOURCE_DIR = join(import.meta.dirname, "../../tiandao/src");

// These runtime validators are not server -> Tiandao public Redis consumers.
const RUNTIME_VALIDATOR_EXEMPTIONS: Readonly<Record<string, string>> = {
  validateAgentCommandV1Contract: "Tiandao produces commands for the server.",
  validateNarrationV1Contract: "Tiandao produces narration for the server/client.",
  validateCarrierImpactEventV1Contract: "Tiandao-local combat narration input, not a public server Redis wire.",
  validateProjectileDespawnedEventV1Contract: "Tiandao-local combat narration input, not a public server Redis wire.",
  validateMultiShotEventV1Contract: "Tiandao-local combat narration input, not a public server Redis wire.",
  validateQiInjectionEventV1Contract: "Tiandao-local combat narration input, not a public server Redis wire.",
  validateEchoFractalEventV1Contract: "Tiandao-local combat narration input, not a public server Redis wire.",
  validateCarrierAbrasionEventV1Contract: "Tiandao-local combat narration input, not a public server Redis wire.",
  validateContainerSwapEventV1Contract: "Tiandao-local combat narration input, not a public server Redis wire.",
  validateAntidoteResultEventV1Contract: "Tiandao-local gameplay narration input, not a public server Redis wire.",
  validateFactionStateV1Contract: "Tiandao internal state contract, not a public server Redis wire.",
  validateHalfStepRechallengeTriggerPayloadV1Contract: "Tiandao internal trigger payload, not a public server Redis wire.",
  validateAgentUiResponsePayloadV1Contract: "Tiandao UI response output, not a server Redis input.",
  validateVortexBackfireEventV1Contract: "Tiandao-local narration input, not a public server Redis wire.",
  validateProjectileQiDrainedEventV1Contract: "Tiandao-local narration input, not a public server Redis wire.",
  validateWoliuSkillCastV1Contract: "Tiandao-local narration input, not a public server Redis wire.",
  validateWoliuBackfireV1Contract: "Tiandao-local narration input, not a public server Redis wire.",
  validateTurbulenceFieldV1Contract: "Tiandao-local narration input, not a public server Redis wire.",
};

function sourceFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return entry.isFile() && entry.name.endsWith(".ts") && !entry.name.endsWith(".test.ts") ? [path] : [];
  });
}

function discoverCalledSchemaValidators(directory: string): Set<string> {
  const found = new Set<string>();
  for (const path of sourceFiles(directory)) {
    const source = ts.createSourceFile(path, readFileSync(path, "utf8"), ts.ScriptTarget.Latest, true);
    const imports = new Map<string, string>();
    for (const statement of source.statements) {
      if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier) ||
          statement.moduleSpecifier.text !== "@bong/schema" || !statement.importClause?.namedBindings ||
          !ts.isNamedImports(statement.importClause.namedBindings)) continue;
      for (const element of statement.importClause.namedBindings.elements) {
        const imported = (element.propertyName ?? element.name).text;
        if (/^validate[A-Za-z0-9]+V1Contract$/.test(imported)) imports.set(element.name.text, imported);
      }
    }
    const visit = (node: ts.Node): void => {
      if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
        const imported = imports.get(node.expression.text);
        if (imported) found.add(imported);
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  return found;
}

function exportName(validator: string): string {
  return validator.replace(/^validate/, "").replace(/Contract$/, "");
}
function fileName(name: string): string {
  return `${name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").replace(/([A-Z]+)([A-Z][a-z])/g, "$1-$2").toLowerCase()}.json`;
}
function runtimeErrors(validators: ReadonlySet<string>, registry: Readonly<Record<string, unknown>>,
  exports: Readonly<Record<string, unknown>>, exemptions: Readonly<Record<string, string>>): string[] {
  const errors: string[] = [];
  for (const validator of validators) {
    if (validator in exemptions) {
      if (!exemptions[validator]?.trim()) errors.push(`${validator}: exemption requires a reason`);
      continue;
    }
    const name = exportName(validator), file = fileName(name), contract = exports[name];
    if (contract === undefined) errors.push(`${validator}: missing schema export ${name}`);
    else if (!(file in registry)) errors.push(`${validator}: missing ${file}`);
    else if (registry[file] !== contract) errors.push(`${validator}: ${file} maps to wrong schema`);
  }
  for (const exemption of Object.keys(exemptions)) if (!validators.has(exemption)) errors.push(`${exemption}: stale exemption`);
  return errors;
}
const GENERATED_IDENTITY_EXCEPTIONS: Readonly<Record<string, string>> = {
  "anticheat-report-v1.json": "antiCheatReportV1",
  "halfstep-rechallenge-trigger-payload-v1.json": "halfStepRechallengeTriggerPayloadV1",
  "dugu-antidote-result-v1.json": "antidoteResultV1",
  "dugu-antidote-result-event-v1.json": "antidoteResultEventV1",
};
function registryName(file: string): string {
  const stem = file.replace(/\.json$/, "");
  return stem.replace(/-([a-z0-9])/g, (_, letter: string) => letter.toUpperCase());
}
function identityErrors(registry: Readonly<Record<string, unknown>>, canonical: Readonly<Record<string, unknown>>): string[] {
  return Object.entries(registry).flatMap(([file, contract]) => {
    const name = GENERATED_IDENTITY_EXCEPTIONS[file] ?? registryName(file);
    return canonical[name] === contract ? [] : [`${file}: wrong schema identity (${name})`];
  });
}

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

  it("discovers called Tiandao runtime V1 validators and checks registry identity", () => {
    const validators = discoverCalledSchemaValidators(TIANDAO_SOURCE_DIR);
    expect(validators.size).toBeGreaterThan(0);
    expect(runtimeErrors(validators, GENERATED_SCHEMA_FILES, schemaExports, RUNTIME_VALIDATOR_EXEMPTIONS)).toEqual([]);
  });

  it("reports missing and wrong runtime mappings", () => {
    const validators = new Set(["validateCraftOutcomeV1Contract"]), contract = {};
    expect(runtimeErrors(validators, {}, { CraftOutcomeV1: contract }, {})).toEqual([
      "validateCraftOutcomeV1Contract: missing craft-outcome-v1.json",
    ]);
    expect(runtimeErrors(validators, { "craft-outcome-v1.json": {} }, { CraftOutcomeV1: contract }, {})).toEqual([
      "validateCraftOutcomeV1Contract: craft-outcome-v1.json maps to wrong schema",
    ]);
  });

  it("requires justified live exemptions and excludes test-only calls", () => {
    const directory = createTempDir();
    writeFileSync(join(directory, "runtime.ts"), 'import { validateFooV1Contract as check } from "@bong/schema"; check({});\n');
    writeFileSync(join(directory, "ignored.test.ts"), 'import { validateBarV1Contract } from "@bong/schema"; validateBarV1Contract({});\n');
    const validators = discoverCalledSchemaValidators(directory);
    expect([...validators]).toEqual(["validateFooV1Contract"]);
    expect(runtimeErrors(validators, {}, {}, { validateFooV1Contract: "not public wire" })).toEqual([]);
    expect(runtimeErrors(validators, {}, {}, { validateFooV1Contract: "" })).toEqual(["validateFooV1Contract: exemption requires a reason"]);
    expect(runtimeErrors(new Set(), {}, {}, { validateFooV1Contract: "reason" })).toEqual(["validateFooV1Contract: stale exemption"]);
  });

  it("verifies filename-to-schema identity for every generated file", () => {
    expect(identityErrors(GENERATED_SCHEMA_FILES, SCHEMA_REGISTRY)).toEqual([]);
    const contract = {};
    expect(identityErrors({ "wrong-v1.json": contract }, { rightV1: {} })).toEqual(["wrong-v1.json: wrong schema identity (wrongV1)"]);
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
