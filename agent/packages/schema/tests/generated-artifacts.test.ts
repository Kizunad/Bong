import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import Ajv from "ajv";
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

// Workspace-declared Ajv (devDependency of @bong/schema). Prefer package metadata over host paths.
const workspaceRequire = createRequire(import.meta.url);
const AJV_PACKAGE_JSON_PATH = workspaceRequire.resolve("ajv/package.json");
const AJV_PACKAGE_VERSION = (workspaceRequire(AJV_PACKAGE_JSON_PATH) as { version?: string })
  .version;
const AJV_RESOLVED_PATH = workspaceRequire.resolve("ajv");

const tempDirs: string[] = [];
const TIANDAO_SOURCE_DIR = join(import.meta.dirname, "../../tiandao/src");
const AGENT_UI_GENERATED_FILES = [
  "agent-ui-response-payload-v1.json",
  "client-request-v1.json",
  "server-data-v1.json",
] as const;
type AgentUiGeneratedFile = (typeof AGENT_UI_GENERATED_FILES)[number];
type AgentUiVariantType = "agent_ui_response" | "agent_ui_request" | "agent_ui_close";
type AgentUiIdFieldName = "request_id" | "target_player";
type AgentUiIdSchemaTarget = Readonly<{
  field: AgentUiIdFieldName;
  variantType?: AgentUiVariantType;
}>;

const AGENT_UI_ID_SCHEMA_TARGETS: Readonly<
  Record<AgentUiGeneratedFile, readonly AgentUiIdSchemaTarget[]>
> = {
  "agent-ui-response-payload-v1.json": [
    { field: "request_id" },
    { field: "target_player" },
  ],
  "client-request-v1.json": [
    { variantType: "agent_ui_response", field: "request_id" },
  ],
  "server-data-v1.json": [
    { variantType: "agent_ui_request", field: "request_id" },
    { variantType: "agent_ui_request", field: "target_player" },
    { variantType: "agent_ui_close", field: "request_id" },
  ],
};

/** 六个 Agent UI ID 字段在 workspace Ajv 下共享的 code-point 边界矩阵。 */
const AGENT_UI_ID_BOUNDARY_CASES = [
  { name: "empty", value: "", valid: false },
  { name: "64 emoji", value: "😀".repeat(64), valid: true },
  { name: "65 emoji", value: "😀".repeat(65), valid: true },
  { name: "128 emoji", value: "😀".repeat(128), valid: true },
  { name: "129 emoji", value: "😀".repeat(129), valid: false },
  { name: "127 BMP + 1 astral", value: `${"a".repeat(127)}😀`, valid: true },
  { name: "128 BMP + 1 astral", value: `${"a".repeat(128)}😀`, valid: false },
  { name: "128 BMP", value: "界".repeat(128), valid: true },
  { name: "129 BMP", value: "界".repeat(129), valid: false },
  { name: "127 BMP + LF", value: `${"a".repeat(127)}\n`, valid: true },
  { name: "128 BMP + LF", value: `${"a".repeat(128)}\n`, valid: false },
  { name: "127 BMP + CR", value: `${"a".repeat(127)}\r`, valid: true },
  { name: "128 BMP + CR", value: `${"a".repeat(128)}\r`, valid: false },
  { name: "126 BMP + CRLF", value: `${"a".repeat(126)}\r\n`, valid: true },
  { name: "127 BMP + CRLF", value: `${"a".repeat(127)}\r\n`, valid: false },
  { name: "127 BMP + U+2028", value: `${"a".repeat(127)} `, valid: true },
  { name: "128 BMP + U+2028", value: `${"a".repeat(128)} `, valid: false },
  { name: "127 BMP + U+2029", value: `${"a".repeat(127)} `, valid: true },
  { name: "128 BMP + U+2029", value: `${"a".repeat(128)} `, valid: false },
  { name: "lone high surrogate", value: "\ud800", valid: false },
  { name: "lone low surrogate", value: "\udc00", valid: false },
  { name: "embedded lone surrogate", value: "a\ud800b", valid: false },
] as const;

type LocatedStringSchema = Readonly<{
  path: string;
  schema: Record<string, unknown>;
}>;

type LocatedAgentUiIdSchema = LocatedStringSchema &
  Readonly<{ target: AgentUiIdSchemaTarget }>;

type AjvValidate = ((data: unknown) => boolean) & {
  errors: ReadonlyArray<unknown> | null | undefined;
};

type WorkspaceAjv = Readonly<{
  version: string;
  resolvedPath: string;
  packageJsonPath: string;
  compile: (schema: object) => AjvValidate;
}>;

/**
 * 使用 @bong/schema 工作区声明的可安装 Ajv（devDependency），不依赖机器宿主
 * `/usr/share/nodejs/ajv`。clean CI / GitHub runner 通过 npm workspace install 解析。
 * 禁止用纯 RegExp 冒充 JSON Schema 编译语义。
 */
function loadWorkspaceAjv(): WorkspaceAjv {
  if (typeof AJV_PACKAGE_VERSION !== "string" || !AJV_PACKAGE_VERSION) {
    throw new Error(`Workspace Ajv package.json at ${AJV_PACKAGE_JSON_PATH} is missing version`);
  }
  if (AJV_PACKAGE_VERSION !== "8.12.0") {
    throw new Error(
      `Workspace Ajv must be exactly 8.12.0 for reproducible schema behavior; got ${AJV_PACKAGE_VERSION} at ${AJV_PACKAGE_JSON_PATH}`,
    );
  }
  // 拒绝 package metadata 或 runtime 任一入口误解析到 Debian host 路径。
  const hostNodePath = "/usr/share/nodejs/";
  const resolvedPaths = [AJV_PACKAGE_JSON_PATH, AJV_RESOLVED_PATH];
  if (resolvedPaths.some((path) => path.includes(hostNodePath))) {
    throw new Error(
      [
        "Ajv resolved to host Node path, not the workspace package:",
        ...resolvedPaths,
        "Install @bong/schema devDependency ajv@8.12.0 and re-run from the agent workspace.",
      ].join(" "),
    );
  }

  // strict:false 兼容 TypeBox 导出的附加元数据；allErrors 便于失败断言带完整路径。
  const ajv = new Ajv({ allErrors: true, strict: false });
  return {
    version: AJV_PACKAGE_VERSION,
    resolvedPath: AJV_RESOLVED_PATH,
    packageJsonPath: AJV_PACKAGE_JSON_PATH,
    compile: (schema) => ajv.compile(schema) as AjvValidate,
  };
}

function readGeneratedArtifact(file: (typeof AGENT_UI_GENERATED_FILES)[number]): unknown {
  return JSON.parse(readFileSync(join(GENERATED_DIR, file), "utf8"));
}

function requireSchemaObject(value: unknown, context: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${context} must be a schema object; got ${JSON.stringify(value)}`);
  }
  return value as Record<string, unknown>;
}

function variantTypeConst(variant: unknown): unknown {
  if (typeof variant !== "object" || variant === null || Array.isArray(variant)) {
    return undefined;
  }
  const properties = (variant as Record<string, unknown>).properties;
  if (typeof properties !== "object" || properties === null || Array.isArray(properties)) {
    return undefined;
  }
  const typeSchema = (properties as Record<string, unknown>).type;
  if (typeof typeSchema !== "object" || typeSchema === null || Array.isArray(typeSchema)) {
    return undefined;
  }
  return (typeSchema as Record<string, unknown>).const;
}

/** 按 `properties.type.const` 唯一定位 anyOf variant，彻底解除生成顺序绑定。 */
function findUniqueVariantByType(
  anyOf: readonly unknown[],
  expectedType: string,
): Record<string, unknown> {
  const candidates = anyOf.map((variant, index) => ({
    index,
    variant,
    discriminant: variantTypeConst(variant),
  }));
  const matches = candidates.filter(({ discriminant }) => discriminant === expectedType);
  const available = candidates
    .filter(({ discriminant }) => discriminant !== undefined)
    .map(({ index, discriminant }) => `[${index}]=${JSON.stringify(discriminant)}`)
    .join(", ");
  const diagnostics =
    `anyOf length=${anyOf.length}; available properties.type.const discriminants: ` +
    (available || "<none>");

  if (matches.length === 0) {
    throw new Error(
      `Missing schema variant for type=${JSON.stringify(expectedType)}; ${diagnostics}`,
    );
  }
  if (matches.length > 1) {
    throw new Error(
      `Duplicate schema variants for type=${JSON.stringify(expectedType)}; ` +
        `matched indexes=[${matches.map(({ index }) => index).join(", ")}]; ${diagnostics}`,
    );
  }

  return requireSchemaObject(
    matches[0].variant,
    `Schema variant type=${JSON.stringify(expectedType)} at anyOf[${matches[0].index}]`,
  );
}

function locateAgentUiIdSchemas(
  file: AgentUiGeneratedFile,
  artifact: unknown,
): LocatedAgentUiIdSchema[] {
  const root = requireSchemaObject(artifact, `${file} root`);
  return AGENT_UI_ID_SCHEMA_TARGETS[file].map((target) => {
    const owner = (() => {
      if (target.variantType === undefined) return root;
      if (!Array.isArray(root.anyOf)) {
        throw new Error(
          `${file} must expose anyOf to locate type=${JSON.stringify(target.variantType)}; ` +
            `root keys=${Object.keys(root).sort().join(", ") || "<none>"}`,
        );
      }
      return findUniqueVariantByType(root.anyOf, target.variantType);
    })();
    const path =
      target.variantType === undefined
        ? `$.properties.${target.field}`
        : `$.anyOf{properties.type.const=${JSON.stringify(target.variantType)}}.properties.${target.field}`;
    const properties = requireSchemaObject(owner.properties, `${file}:${path} owner.properties`);
    if (!(target.field in properties)) {
      throw new Error(
        `Missing schema field ${file}:${path}; available properties: ` +
          (Object.keys(properties).sort().join(", ") || "<none>"),
      );
    }
    return {
      path,
      schema: requireSchemaObject(properties[target.field], `${file}:${path}`),
      target,
    };
  });
}

/**
 * 为每个 pinned Agent UI ID path 构造最小合法 payload，并把目标字段设为 value。
 * 校验走根 schema compile，而不是单字段 RegExp，覆盖 anyOf/required/additionalProperties。
 */
function buildAgentUiIdPayload(
  file: AgentUiGeneratedFile,
  target: AgentUiIdSchemaTarget,
  value: string,
): Record<string, unknown> {
  const semanticKey = `${file}:${target.variantType ?? "<root>"}:${target.field}`;
  switch (semanticKey) {
    case "agent-ui-response-payload-v1.json:<root>:request_id":
      return { request_id: value, action: "dismissed", params: {} };
    case "agent-ui-response-payload-v1.json:<root>:target_player":
      return {
        request_id: "response-request-id",
        action: "error",
        target_player: value,
        params: { reason: "realm_gate_rejected" },
      };
    case "client-request-v1.json:agent_ui_response:request_id":
      return {
        v: 1,
        type: "agent_ui_response",
        request_id: value,
        action: "dismissed",
        params: {},
      };
    case "server-data-v1.json:agent_ui_request:request_id":
      return {
        v: 1,
        type: "agent_ui_request",
        request_id: value,
        target_player: "offline:Target",
        xml: "<flow-layout />",
        timeout_ticks: 600,
      };
    case "server-data-v1.json:agent_ui_request:target_player":
      return {
        v: 1,
        type: "agent_ui_request",
        request_id: "server-data-request",
        target_player: value,
        xml: "<flow-layout />",
        timeout_ticks: 600,
      };
    case "server-data-v1.json:agent_ui_close:request_id":
      return {
        v: 1,
        type: "agent_ui_close",
        request_id: value,
        reason: "invalid_button_id",
      };
    default:
      throw new Error(`No payload builder for semantic Agent UI field ${semanticKey}`);
  }
}

function findAgentUiIdStringSchemas(value: unknown, path = "$"): LocatedStringSchema[] {
  if (Array.isArray(value)) {
    return value.flatMap((entry, index) =>
      findAgentUiIdStringSchemas(entry, `${path}[${index}]`),
    );
  }
  if (typeof value !== "object" || value === null) return [];

  const schema = value as Record<string, unknown>;
  const here =
    schema.type === "string" &&
    typeof schema.pattern === "string" &&
    schema.pattern.includes("\\uD800-\\uDFFF")
      ? [{ path, schema }]
      : [];

  return [
    ...here,
    ...Object.entries(schema).flatMap(([key, child]) =>
      findAgentUiIdStringSchemas(child, `${path}.${key}`),
    ),
  ];
}

function checkUnicodeAwareEcma262StringPattern(
  schema: Record<string, unknown>,
  value: string,
): boolean {
  if (schema.type !== "string") return false;
  return (
    typeof schema.pattern === "string" && new RegExp(schema.pattern, "u").test(value)
  );
}

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

  it(
    "discovers called Tiandao runtime V1 validators and checks registry identity",
    () => {
      const validators = discoverCalledSchemaValidators(TIANDAO_SOURCE_DIR);
      expect(validators.size).toBeGreaterThan(0);
      expect(runtimeErrors(validators, GENERATED_SCHEMA_FILES, schemaExports, RUNTIME_VALIDATOR_EXEMPTIONS)).toEqual([]);
    },
    60_000,
  );

  it("exports the server-to-Tiandao Agent UI response as its own generated artifact", () => {
    const file = "agent-ui-response-payload-v1.json";
    const schema = GENERATED_SCHEMA_FILES[file] as {
      required?: string[];
      properties?: Record<string, unknown>;
    };

    expect(schema).toBe(schemaExports.AgentUiResponsePayloadV1);
    expect(schema.required).toEqual(["request_id", "action", "params"]);
    expect(schema.required).not.toContain("target_player");
    expect(schema.properties).toHaveProperty("target_player");
  });

  it("finds a semantic Agent UI variant regardless of anyOf ordering", () => {
    const expected = {
      properties: { type: { const: "agent_ui_request" }, request_id: { type: "string" } },
    };
    const unrelated = { properties: { type: { const: "agent_ui_close" } } };

    expect(findUniqueVariantByType([unrelated, expected], "agent_ui_request")).toBe(expected);
    expect(findUniqueVariantByType([expected, unrelated], "agent_ui_request")).toBe(expected);
  });

  it("diagnoses a missing semantic Agent UI variant with available discriminants", () => {
    expect(() =>
      findUniqueVariantByType(
        [{ properties: { type: { const: "agent_ui_close" } } }],
        "agent_ui_request",
      ),
    ).toThrowError(
      'Missing schema variant for type="agent_ui_request"; anyOf length=1; ' +
        'available properties.type.const discriminants: [0]="agent_ui_close"',
    );
  });

  it("diagnoses duplicate semantic Agent UI variants with every matched index", () => {
    expect(() =>
      findUniqueVariantByType(
        [
          { properties: { type: { const: "agent_ui_request" } } },
          { properties: { type: { const: "agent_ui_close" } } },
          { properties: { type: { const: "agent_ui_request" } } },
        ],
        "agent_ui_request",
      ),
    ).toThrowError(
      'Duplicate schema variants for type="agent_ui_request"; matched indexes=[0, 2]; ' +
        'anyOf length=3; available properties.type.const discriminants: ' +
        '[0]="agent_ui_request", [1]="agent_ui_close", [2]="agent_ui_request"',
    );
  });

  it("keeps every generated Agent UI ID on one Unicode code-point acceptance set", () => {
    const cases = AGENT_UI_ID_BOUNDARY_CASES;

    for (const file of AGENT_UI_GENERATED_FILES) {
      const artifact = readGeneratedArtifact(file);
      const idSchemas = locateAgentUiIdSchemas(file, artifact);
      const discoveredIdSchemas = findAgentUiIdStringSchemas(artifact);
      const expectedSchemas = new Set(idSchemas.map(({ schema }) => schema));
      expect(
        {
          missing: idSchemas
            .filter(({ schema }) => !discoveredIdSchemas.some((found) => found.schema === schema))
            .map(({ path }) => path),
          unexpected: discoveredIdSchemas
            .filter(({ schema }) => !expectedSchemas.has(schema))
            .map(({ path }) => path),
        },
        `${file} 应按 properties.type.const 语义唯一定位全部 Agent UI ID 字段，避免顺序绑定或等量替换假绿`,
      ).toEqual({ missing: [], unexpected: [] });
      expect(discoveredIdSchemas).toHaveLength(idSchemas.length);

      for (const { path, schema } of idSchemas) {
        expect(
          schema.maxLength,
          `${file}:${path} 不得恢复与 TypeBox UTF-16 runtime 分叉的 maxLength`,
        ).toBeUndefined();
        expect(
          schema.minLength,
          `${file}:${path} 的 1..=128 code-point 边界应完全由同一个 pattern 表达`,
        ).toBeUndefined();

        const legacyEcma262 = new RegExp(schema.pattern as string);
        for (const testCase of cases) {
          expect(
            checkUnicodeAwareEcma262StringPattern(schema, testCase.value),
            `${file}:${path} 的 Unicode-aware ECMA-262 pattern 对 ${testCase.name} 应为 ${testCase.valid}`,
          ).toBe(testCase.valid);
          expect(
            legacyEcma262.test(testCase.value),
            `${file}:${path} 的无 flag ECMA-262 兼容语义对 ${testCase.name} 应为 ${testCase.valid}`,
          ).toBe(testCase.valid);
        }
      }
    }
  });

  // workspace Ajv 对 client-request / server-data 全量 anyOf 根 schema 编译 + 边界矩阵较慢，
  // 默认 5s 会假红；单独放宽到 60s，并打印 assertionCount 供 Finish Evidence。
  it(
    "compiles generated Agent UI schemas with workspace Ajv and locks the full acceptance matrix",
    () => {
      expect(
        AJV_PACKAGE_VERSION,
        `Ajv package metadata 必须精确锁定 8.12.0；resolved=${AJV_PACKAGE_JSON_PATH}`,
      ).toBe("8.12.0");
      const workspaceAjv = loadWorkspaceAjv();
      expect(
        workspaceAjv.version,
        `workspace Ajv 必须精确解析 8.12.0；package=${workspaceAjv.packageJsonPath}, runtime=${workspaceAjv.resolvedPath}`,
      ).toBe("8.12.0");
      // package metadata 与 runtime import 都必须来自可安装 workspace，而不是 Debian host。
      expect(
        [workspaceAjv.packageJsonPath, workspaceAjv.resolvedPath].every(
          (path) => path.includes("node_modules") && path.includes("ajv") &&
            !path.includes("/usr/share/nodejs/"),
        ),
        `Workspace Ajv must resolve under node_modules (not host): ` +
          `${workspaceAjv.packageJsonPath}, ${workspaceAjv.resolvedPath}`,
      ).toBe(true);

      let assertionCount = 0;
      const count = (condition: boolean, message: string): void => {
        expect(condition, message).toBe(true);
        assertionCount += 1;
      };

      for (const file of AGENT_UI_GENERATED_FILES) {
        const artifact = readGeneratedArtifact(file);
        const validateRoot = workspaceAjv.compile(artifact as object);
        const idSchemas = locateAgentUiIdSchemas(file, artifact);

        for (const { path, schema: fieldSchema, target } of idSchemas) {
          count(
            fieldSchema.type === "string" && typeof fieldSchema.pattern === "string",
            `${file}:${path} 必须仍是 pattern-only string schema`,
          );
          count(
            fieldSchema.maxLength === undefined && fieldSchema.minLength === undefined,
            `${file}:${path} 不得恢复 minLength/maxLength`,
          );

          for (const testCase of AGENT_UI_ID_BOUNDARY_CASES) {
            const payload = buildAgentUiIdPayload(file, target, testCase.value);
            const got = validateRoot(payload);
            count(
              got === testCase.valid,
              [
                `workspace Ajv ${workspaceAjv.version} (${workspaceAjv.resolvedPath})`,
                `${file}:${path}`,
                testCase.name,
                `expected valid=${testCase.valid}, got=${got}`,
                got ? "" : `errors=${JSON.stringify(validateRoot.errors ?? null)}`,
              ]
                .filter(Boolean)
                .join(" | "),
            );
          }
        }

        // 根 schema 级契约：额外字段 / 缺 required / optional null 分流。
        if (file === "agent-ui-response-payload-v1.json") {
          count(
            validateRoot({
              request_id: "legacy-ok",
              action: "dismissed",
              params: {},
            }) === true,
            `${file}: legacy 缺 target_player 必须可被 workspace Ajv 接受`,
          );
          count(
            validateRoot({
              request_id: "extra-bad",
              action: "dismissed",
              params: {},
              unexpected_field: true,
            }) === false,
            `${file}: additionalProperties=false 必须拒绝额外字段`,
          );
          count(
            validateRoot({
              action: "dismissed",
              params: {},
            }) === false,
            `${file}: 缺 request_id 必须失败`,
          );
          count(
            validateRoot({
              request_id: "null-target",
              action: "dismissed",
              params: {},
              target_player: null,
            }) === false,
            `${file}: 显式 null target_player 必须失败`,
          );
        }

        if (file === "client-request-v1.json") {
          const base = {
            v: 1,
            type: "agent_ui_response",
            request_id: "client-ok",
            action: "dismissed",
            params: {},
          };
          count(validateRoot(base) === true, `${file}: 最小 agent_ui_response 必须通过`);
          count(
            validateRoot({ ...base, target_player: "offline:ShouldNotExist" }) === false,
            `${file}: C2S agent_ui_response 禁止 target_player 额外字段`,
          );
          count(
            validateRoot({
              v: 1,
              type: "agent_ui_response",
              action: "dismissed",
              params: {},
            }) === false,
            `${file}: 缺 request_id 必须失败`,
          );
        }

        if (file === "server-data-v1.json") {
          const requestBase = {
            v: 1,
            type: "agent_ui_request",
            request_id: "server-request-ok",
            target_player: "offline:Target",
            xml: "<flow-layout />",
            timeout_ticks: 600,
          };
          const closeBase = {
            v: 1,
            type: "agent_ui_close",
            request_id: "server-close-ok",
            reason: "invalid_button_id",
          };
          count(validateRoot(requestBase) === true, `${file}: agent_ui_request 最小样本必须通过`);
          count(validateRoot(closeBase) === true, `${file}: agent_ui_close 最小样本必须通过`);
          count(
            validateRoot({ ...requestBase, unexpected_field: 1 }) === false,
            `${file}: agent_ui_request additionalProperties=false`,
          );
          count(
            validateRoot({
              v: 1,
              type: "agent_ui_request",
              request_id: "missing-target",
              xml: "<flow-layout />",
              timeout_ticks: 600,
            }) === false,
            `${file}: agent_ui_request 缺 target_player 必须失败`,
          );
          count(
            validateRoot({
              v: 1,
              type: "agent_ui_close",
              request_id: "close-null-reason",
              reason: null,
            }) === false,
            `${file}: agent_ui_close 显式 null reason 必须失败`,
          );
        }
      }

      // 6 fields × 22 boundary cases = 132，再加上字段结构 pin 与根契约 pin。
      // 用下界锁住“真 Ajv 矩阵已自动化”，避免退化成少量 smoke。
      count(
        assertionCount >= 132,
        `workspace Ajv gate must keep the full boundary matrix; assertionCount=${assertionCount}`,
      );
      // 显式打印，便于 Finish Evidence 抓取 assertionCount / workspace Ajv 路径。
      // eslint-disable-next-line no-console
      console.log(
        JSON.stringify({
          ajvVersion: workspaceAjv.version,
          ajvResolvedPath: workspaceAjv.resolvedPath,
          ajvPackageJsonPath: workspaceAjv.packageJsonPath,
          assertionCount,
          boundaryCases: AGENT_UI_ID_BOUNDARY_CASES.length,
          idFields: Object.values(AGENT_UI_ID_SCHEMA_TARGETS).flat().length,
        }),
      );
      expect(
        {
          ajvVersion: workspaceAjv.version,
          ajvResolvedPath: workspaceAjv.resolvedPath,
          assertionCount,
          boundaryCases: AGENT_UI_ID_BOUNDARY_CASES.length,
          idFields: Object.values(AGENT_UI_ID_SCHEMA_TARGETS).flat().length,
        },
        "workspace Ajv gate summary",
      ).toMatchObject({
        ajvVersion: workspaceAjv.version,
        assertionCount,
        boundaryCases: 22,
        idFields: 6,
      });
    },
    60_000,
  );
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

  it("fails freshness when the Agent UI response artifact is missing", () => {
    const outputDir = createTempDir();
    writeGeneratedSchemas(outputDir);
    rmSync(join(outputDir, "agent-ui-response-payload-v1.json"));

    expect(getGeneratedSchemaDrift(outputDir).missing).toContain(
      "agent-ui-response-payload-v1.json",
    );
    expect(() => assertGeneratedSchemasFresh(outputDir)).toThrowError(
      /agent-ui-response-payload-v1\.json/,
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
