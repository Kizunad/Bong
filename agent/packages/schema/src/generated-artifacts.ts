/**
 * 管理 TypeBox registry 到已提交 JSON Schema 的生成快照、来源 pin 与
 * manifest freshness gate；不负责把这些 declared/unwired 产物接入生产传输。
 */
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  SCHEMA_GENERATION_MANIFEST_FILE_NAME,
  SCHEMA_GENERATION_MANIFEST_VERSION,
  SCHEMA_GENERATION_STAGES,
  SCHEMA_GENERATION_VERSION,
} from "./generation-manifest.js";
import { GENERATED_SCHEMA_FILES, SCHEMA_REGISTRY } from "./schema-registry.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

export const GENERATED_DIR = join(__dirname, "..", "generated");
/** 已提交 JSON Schema 中记录 TypeBox 来源内容的 SHA-256 字段名。 */
export const GENERATED_TYPEBOX_SOURCE_HASH_FIELD = "x-bong-typebox-source-sha256";
/** 已提交 JSON Schema 中记录生成器版本的字段名。 */
export const GENERATED_SCHEMA_GENERATION_VERSION_FIELD =
  "x-bong-schema-generation-version";
/** 生成 manifest 在当前 schema 包中的提交路径。 */
export const GENERATION_MANIFEST_PATH = join(
  GENERATED_DIR,
  SCHEMA_GENERATION_MANIFEST_FILE_NAME,
);

export interface GeneratedSchemaDrift {
  missing: string[];
  changed: string[];
  unexpected: string[];
  pinMismatches: string[];
  /** 生成 manifest 缺失或内容与当前 registry/生成器不一致的条目。 */
  manifestMismatches: string[];
}

export interface WriteGeneratedSchemasResult {
  outputDir: string;
  written: string[];
  removed: string[];
}

type GeneratedSchemaContents = Record<string, string>;
type GeneratedSchemaSourceHashes = Record<string, string>;

function sourceHashForSchema(schema: unknown): string {
  return createHash("sha256")
    .update(JSON.stringify(schema))
    .digest("hex");
}

function sortedGeneratedSchemaEntries(): [string, unknown][] {
  return Object.entries(GENERATED_SCHEMA_FILES).sort(([left], [right]) =>
    left.localeCompare(right),
  );
}

function renderGeneratedSchema(schema: unknown): string {
  return `${JSON.stringify(
    {
      ...(schema as Record<string, unknown>),
      [GENERATED_TYPEBOX_SOURCE_HASH_FIELD]: sourceHashForSchema(schema),
      [GENERATED_SCHEMA_GENERATION_VERSION_FIELD]: SCHEMA_GENERATION_VERSION,
    },
    null,
    2,
  )}\n`;
}

function listGeneratedJsonFiles(outputDir: string): string[] {
  if (!existsSync(outputDir)) {
    return [];
  }

  return readdirSync(outputDir)
    .filter(
      (fileName) =>
        fileName.endsWith(".json") &&
        fileName !== SCHEMA_GENERATION_MANIFEST_FILE_NAME,
    )
    .sort();
}

function captureGeneratedSchemaContents(): GeneratedSchemaContents {
  return Object.freeze(
    Object.fromEntries(
      sortedGeneratedSchemaEntries().map(([fileName, schema]) => [
        fileName,
        renderGeneratedSchema(schema),
      ]),
    ) as GeneratedSchemaContents,
  );
}

function captureGeneratedSchemaSourceHashes(): GeneratedSchemaSourceHashes {
  return Object.freeze(
    Object.fromEntries(
      sortedGeneratedSchemaEntries().map(([fileName, schema]) => [
        fileName,
        sourceHashForSchema(schema),
      ]),
    ) as GeneratedSchemaSourceHashes,
  );
}

const SNAPSHOTTED_GENERATED_SCHEMA_CONTENTS = captureGeneratedSchemaContents();
const SNAPSHOTTED_GENERATED_SCHEMA_SOURCE_HASHES = captureGeneratedSchemaSourceHashes();

interface GeneratedSchemaPins {
  sourceHash?: string;
  generationVersion?: string;
}

function readGeneratedSchemaPins(content: string): GeneratedSchemaPins {
  const parsed: unknown = JSON.parse(content);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return {};
  }
  const record = parsed as Record<string, unknown>;
  return {
    sourceHash:
      typeof record[GENERATED_TYPEBOX_SOURCE_HASH_FIELD] === "string"
        ? record[GENERATED_TYPEBOX_SOURCE_HASH_FIELD]
        : undefined,
    generationVersion:
      typeof record[GENERATED_SCHEMA_GENERATION_VERSION_FIELD] === "string"
        ? record[GENERATED_SCHEMA_GENERATION_VERSION_FIELD]
        : undefined,
  };
}

export function getGeneratedSchemaSourceHashes(): GeneratedSchemaSourceHashes {
  return { ...SNAPSHOTTED_GENERATED_SCHEMA_SOURCE_HASHES };
}

function sourceHashMismatches(
  outputDir: string,
  expectedFiles: GeneratedSchemaContents,
): string[] {
  return Object.entries(expectedFiles).flatMap(([fileName]) => {
    const filePath = join(outputDir, fileName);
    if (!existsSync(filePath)) return [];
    let pins: GeneratedSchemaPins;
    try {
      pins = readGeneratedSchemaPins(readFileSync(filePath, "utf8"));
    } catch {
      return [`${fileName}:invalid_json`];
    }
    const mismatches: string[] = [];
    if (pins.sourceHash !== SNAPSHOTTED_GENERATED_SCHEMA_SOURCE_HASHES[fileName]) {
      mismatches.push(`${fileName}:source_sha256`);
    }
    if (pins.generationVersion !== SCHEMA_GENERATION_VERSION) {
      mismatches.push(`${fileName}:generation_version`);
    }
    return mismatches;
  });
}

function registryKeysForSchema(schema: unknown): string[] {
  return Object.entries(SCHEMA_REGISTRY)
    .filter(([, registeredSchema]) => registeredSchema === schema)
    .map(([registryKey]) => registryKey)
    .sort();
}

function contractVersionForFile(fileName: string): string {
  const match = /-v(\d+)\.json$/.exec(fileName);
  return match === null ? "unversioned" : `v${match[1]}`;
}

function readSchemaPackageVersion(): string {
  const packagePath = join(__dirname, "..", "package.json");
  const packageJson: unknown = JSON.parse(readFileSync(packagePath, "utf8"));
  if (
    typeof packageJson !== "object" ||
    packageJson === null ||
    Array.isArray(packageJson) ||
    typeof (packageJson as Record<string, unknown>).version !== "string"
  ) {
    throw new Error(`schema package version is missing from ${packagePath}`);
  }
  return (packageJson as Record<string, string>).version;
}

/** 返回与当前 TypeBox registry 对拍的确定性 manifest。 */
export function renderGenerationManifest(): string {
  const artifacts = sortedGeneratedSchemaEntries().map(([fileName, schema]) => ({
    file: fileName,
    registry_keys: registryKeysForSchema(schema),
    source_sha256: SNAPSHOTTED_GENERATED_SCHEMA_SOURCE_HASHES[fileName],
    generation_version: SCHEMA_GENERATION_VERSION,
    contract_version: contractVersionForFile(fileName),
    status: "declared" as const,
    production_reachable: false as const,
  }));
  return `${JSON.stringify(
    {
      manifest_version: SCHEMA_GENERATION_MANIFEST_VERSION,
      generation_version: SCHEMA_GENERATION_VERSION,
      generator: {
        package: "@bong/schema",
        package_version: readSchemaPackageVersion(),
        entrypoint: "agent/packages/schema/src/generate.ts",
        registry: "agent/packages/schema/src/schema-registry.ts",
        output_directory: "agent/packages/schema/generated",
      },
      stages: SCHEMA_GENERATION_STAGES,
      artifact_count: artifacts.length,
      artifacts,
    },
    null,
    2,
  )}\n`;
}

function manifestMismatches(outputDir: string): string[] {
  const manifestPath = join(outputDir, SCHEMA_GENERATION_MANIFEST_FILE_NAME);
  if (!existsSync(manifestPath)) {
    return [`${SCHEMA_GENERATION_MANIFEST_FILE_NAME}:missing`];
  }
  try {
    if (readFileSync(manifestPath, "utf8") !== renderGenerationManifest()) {
      return [`${SCHEMA_GENERATION_MANIFEST_FILE_NAME}:stale`];
    }
  } catch {
    return [`${SCHEMA_GENERATION_MANIFEST_FILE_NAME}:invalid_json`];
  }
  return [];
}

export function renderGeneratedSchemas(): GeneratedSchemaContents {
  return { ...SNAPSHOTTED_GENERATED_SCHEMA_CONTENTS };
}

/**
 * 确认同一份 TypeBox registry 在一次进程内重复渲染仍得到完全相同的字节。
 * 这让生成器的 deterministic 约束在写盘前也有独立的契约入口。
 */
export function assertGeneratedSchemasDeterministic(): void {
  const firstSchemas = renderGeneratedSchemas();
  const secondSchemas = renderGeneratedSchemas();
  const firstManifest = renderGenerationManifest();
  const secondManifest = renderGenerationManifest();
  if (
    JSON.stringify(firstSchemas) !== JSON.stringify(secondSchemas) ||
    firstManifest !== secondManifest
  ) {
    throw new Error(
      "Schema generation is not deterministic; repeated renders differ byte-for-byte.",
    );
  }
}

export function getGeneratedSchemaDrift(outputDir = GENERATED_DIR): GeneratedSchemaDrift {
  const expectedFiles = SNAPSHOTTED_GENERATED_SCHEMA_CONTENTS;
  const missing: string[] = [];
  const changed: string[] = [];

  for (const [fileName, expectedContent] of Object.entries(expectedFiles)) {
    const filePath = join(outputDir, fileName);
    if (!existsSync(filePath)) {
      missing.push(fileName);
      continue;
    }

    let actualContent: string;
    try {
      actualContent = readFileSync(filePath, "utf8");
    } catch {
      changed.push(fileName);
      continue;
    }
    if (actualContent !== expectedContent) {
      changed.push(fileName);
    }
  }

  const unexpected = listGeneratedJsonFiles(outputDir).filter(
    (fileName) => !(fileName in expectedFiles),
  );
  const pinMismatches = sourceHashMismatches(outputDir, expectedFiles);
  const manifestDrift = manifestMismatches(outputDir);

  return {
    missing,
    changed,
    unexpected,
    pinMismatches,
    manifestMismatches: manifestDrift,
  };
}

export function assertGeneratedSchemasFresh(outputDir = GENERATED_DIR): void {
  assertGeneratedSchemasDeterministic();
  const drift = getGeneratedSchemaDrift(outputDir);
  const problems = [
    drift.missing.length > 0 ? `missing: ${drift.missing.join(", ")}` : null,
    drift.changed.length > 0 ? `changed: ${drift.changed.join(", ")}` : null,
    drift.unexpected.length > 0 ? `unexpected: ${drift.unexpected.join(", ")}` : null,
    drift.pinMismatches.length > 0
      ? `source hash/version pin mismatch: ${drift.pinMismatches.join(", ")}`
      : null,
    drift.manifestMismatches.length > 0
      ? `generation manifest mismatch: ${drift.manifestMismatches.join(", ")}`
      : null,
  ].filter((value): value is string => value !== null);

  if (problems.length === 0) {
    return;
  }

  throw new Error(
    `Generated schema artifacts are out of date (${problems.join("; ")}). Run "npm run generate".`,
  );
}

export function writeGeneratedSchemas(outputDir = GENERATED_DIR): WriteGeneratedSchemasResult {
  mkdirSync(outputDir, { recursive: true });

  const expectedFiles = SNAPSHOTTED_GENERATED_SCHEMA_CONTENTS;
  const written: string[] = [];
  const removed: string[] = [];

  for (const [fileName, content] of Object.entries(expectedFiles)) {
    const filePath = join(outputDir, fileName);
    writeFileSync(filePath, content);
    written.push(filePath);
  }

  for (const fileName of listGeneratedJsonFiles(outputDir)) {
    if (fileName in expectedFiles) {
      continue;
    }

    const filePath = join(outputDir, fileName);
    rmSync(filePath);
    removed.push(filePath);
  }

  const manifestPath = join(outputDir, SCHEMA_GENERATION_MANIFEST_FILE_NAME);
  writeFileSync(manifestPath, renderGenerationManifest());
  written.push(manifestPath);

  return {
    outputDir,
    written,
    removed,
  };
}
