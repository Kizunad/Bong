import {
  existsSync,
  mkdirSync,
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
type ValidatorExemption = Readonly<{ reason: string; expiresOn: string }>;
const TODAY = "2026-07-13";
const exemption = (reason: string): ValidatorExemption => ({ reason, expiresOn: "2028-07-13" });
const RUNTIME_VALIDATOR_EXEMPTIONS: Readonly<Record<string, ValidatorExemption>> = {
  validateAgentCommandV1Contract: exemption("Tiandao produces commands for the server."),
  validateNarrationV1Contract: exemption("Tiandao produces narration for the server/client."),
  validateCarrierImpactEventV1Contract: exemption("Tiandao-local combat narration input, not a public server Redis wire."),
  validateProjectileDespawnedEventV1Contract: exemption("Tiandao-local combat narration input, not a public server Redis wire."),
  validateMultiShotEventV1Contract: exemption("Tiandao-local combat narration input, not a public server Redis wire."),
  validateQiInjectionEventV1Contract: exemption("Tiandao-local combat narration input, not a public server Redis wire."),
  validateEchoFractalEventV1Contract: exemption("Tiandao-local combat narration input, not a public server Redis wire."),
  validateCarrierAbrasionEventV1Contract: exemption("Tiandao-local combat narration input, not a public server Redis wire."),
  validateContainerSwapEventV1Contract: exemption("Tiandao-local combat narration input, not a public server Redis wire."),
  validateAntidoteResultEventV1Contract: exemption("Tiandao-local gameplay narration input, not a public server Redis wire."),
  validateFactionStateV1Contract: exemption("Tiandao internal state contract, not a public server Redis wire."),
  validateHalfStepRechallengeTriggerPayloadV1Contract: exemption("Tiandao internal trigger payload, not a public server Redis wire."),
  validateAgentUiResponsePayloadV1Contract: exemption("Tiandao UI response output, not a server Redis input."),
  validateVortexBackfireEventV1Contract: exemption("Tiandao-local narration input, not a public server Redis wire."),
  validateProjectileQiDrainedEventV1Contract: exemption("Tiandao-local narration input, not a public server Redis wire."),
  validateWoliuSkillCastV1Contract: exemption("Tiandao-local narration input, not a public server Redis wire."),
  validateWoliuBackfireV1Contract: exemption("Tiandao-local narration input, not a public server Redis wire."),
  validateTurbulenceFieldV1Contract: exemption("Tiandao-local narration input, not a public server Redis wire."),
};

function sourceFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return entry.name === "__tests__" ? [] : sourceFiles(path);
    if (!entry.isFile() || !/\.tsx?$/.test(entry.name) || /\.(?:test|spec)\.tsx?$/.test(entry.name)) return [];
    return [path];
  });
}

function discoverCalledSchemaValidators(directory: string): Set<string> {
  const found = new Set<string>();
  for (const path of sourceFiles(directory)) {
    const source = ts.createSourceFile(path, readFileSync(path, "utf8"), ts.ScriptTarget.Latest, true);
    const bindings = new Map<string, string>();
    const namespaces = new Set<string>();
    for (const statement of source.statements) {
      if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier) ||
          statement.moduleSpecifier.text !== "@bong/schema" || !statement.importClause?.namedBindings) continue;
      const named = statement.importClause.namedBindings;
      if (ts.isNamespaceImport(named)) namespaces.add(named.name.text);
      else for (const element of named.elements) {
        const imported = (element.propertyName ?? element.name).text;
        if (/^validate[A-Za-z0-9]+V1Contract$/.test(imported)) bindings.set(element.name.text, imported);
      }
    }
    const validatorFor = (expression: ts.Expression): string | undefined => {
      if (ts.isIdentifier(expression)) return bindings.get(expression.text);
      if (ts.isPropertyAccessExpression(expression) && ts.isIdentifier(expression.expression) &&
          namespaces.has(expression.expression.text) && /^validate[A-Za-z0-9]+V1Contract$/.test(expression.name.text))
        return expression.name.text;
      return undefined;
    };
    // One-hop aliases cover common const check = importedValidator patterns without data-flow inference.
    for (const statement of source.statements) if (ts.isVariableStatement(statement))
      for (const declaration of statement.declarationList.declarations) if (ts.isIdentifier(declaration.name) && declaration.initializer) {
        const validator = validatorFor(declaration.initializer);
        if (validator) bindings.set(declaration.name.text, validator);
      }
    const visit = (node: ts.Node): void => {
      if (ts.isIdentifier(node)) {
        const validator = bindings.get(node.text);
        const isDeclaration =
          ts.isImportSpecifier(node.parent) ||
          (ts.isVariableDeclaration(node.parent) && node.parent.name === node);
        if (validator && !isDeclaration) found.add(validator);
      } else if (ts.isPropertyAccessExpression(node)) {
        const validator = validatorFor(node);
        if (validator) found.add(validator);
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
  exports: Readonly<Record<string, unknown>>, exemptions: Readonly<Record<string, ValidatorExemption>>, today = TODAY): string[] {
  const errors: string[] = [];
  for (const validator of validators) {
    if (validator in exemptions) {
      const item = exemptions[validator];
      if (!item.reason.trim()) errors.push(`${validator}: exemption requires a reason`);
      if (!/^\d{4}-\d{2}-\d{2}$/.test(item.expiresOn) || Number.isNaN(Date.parse(`${item.expiresOn}T00:00:00Z`)))
        errors.push(`${validator}: exemption expiry must be an ISO date`);
      else if (item.expiresOn < today) errors.push(`${validator}: exemption expired on ${item.expiresOn}`);
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
    writeFileSync(join(directory, "runtime.ts"), 'import { validateFooV1Contract as original } from "@bong/schema"; const check = original; const config = { validate: original }; consume(original); check({}); void config;\n');
    writeFileSync(join(directory, "namespace.tsx"), 'import * as schema from "@bong/schema"; const config = { validate: schema.validateBazV1Contract }; void config;\n');
    for (const ignored of ["ignored.test.ts", "ignored.test.tsx", "ignored.spec.ts", "ignored.spec.tsx"]) writeFileSync(join(directory, ignored), 'import { validateBarV1Contract } from "@bong/schema"; validateBarV1Contract({});\n');
    const testsDir = join(directory, "__tests__"); mkdirSync(testsDir);
    writeFileSync(join(testsDir, "hidden.ts"), 'import { validateHiddenV1Contract } from "@bong/schema"; validateHiddenV1Contract({});\n');
    const validators = discoverCalledSchemaValidators(directory);
    expect([...validators].sort()).toEqual(["validateBazV1Contract", "validateFooV1Contract"]);
    const valid = { validateFooV1Contract: { reason: "not public wire", expiresOn: "2026-07-13" }, validateBazV1Contract: { reason: "not public wire", expiresOn: "2026-07-14" } };
    expect(runtimeErrors(validators, {}, {}, valid)).toEqual([]);
    expect(runtimeErrors(new Set(["validateFooV1Contract"]), {}, {}, { validateFooV1Contract: { reason: "", expiresOn: "2026-07-14" } })).toEqual(["validateFooV1Contract: exemption requires a reason"]);
    expect(runtimeErrors(new Set(["validateFooV1Contract"]), {}, {}, { validateFooV1Contract: { reason: "reason", expiresOn: "2026/07/14" } })).toEqual(["validateFooV1Contract: exemption expiry must be an ISO date"]);
    expect(runtimeErrors(new Set(["validateFooV1Contract"]), {}, {}, { validateFooV1Contract: { reason: "reason", expiresOn: "2026-07-12" } })).toEqual(["validateFooV1Contract: exemption expired on 2026-07-12"]);
    expect(runtimeErrors(new Set(), {}, {}, { validateFooV1Contract: { reason: "reason", expiresOn: "2026-07-14" } })).toEqual(["validateFooV1Contract: stale exemption"]);
  });

  it("verifies filename-to-schema identity for every generated file", () => {
    expect(identityErrors(GENERATED_SCHEMA_FILES, SCHEMA_REGISTRY)).toEqual([]);
    const left = { type: "string" }, right = { type: "number" };
    expect(identityErrors({ "left-v1.json": right, "right-v1.json": left }, { leftV1: left, rightV1: right })).toEqual([
      "left-v1.json: wrong schema identity (leftV1)", "right-v1.json: wrong schema identity (rightV1)",
    ]);
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
