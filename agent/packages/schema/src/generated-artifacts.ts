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

import { GENERATED_SCHEMA_FILES } from "./schema-registry.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

export const GENERATED_DIR = join(__dirname, "..", "generated");
export const GENERATED_TYPEBOX_SOURCE_HASH_FIELD = "x-bong-typebox-source-sha256";

export interface GeneratedSchemaDrift {
  missing: string[];
  changed: string[];
  unexpected: string[];
  pinMismatches: string[];
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

function renderGeneratedSchema(schema: unknown): string {
  return `${JSON.stringify(
    {
      ...(schema as Record<string, unknown>),
      [GENERATED_TYPEBOX_SOURCE_HASH_FIELD]: sourceHashForSchema(schema),
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
    .filter((fileName) => fileName.endsWith(".json"))
    .sort();
}

function captureGeneratedSchemaContents(): GeneratedSchemaContents {
  return Object.freeze(
    Object.fromEntries(
      Object.entries(GENERATED_SCHEMA_FILES).map(([fileName, schema]) => [
        fileName,
        renderGeneratedSchema(schema),
      ]),
    ) as GeneratedSchemaContents,
  );
}

function captureGeneratedSchemaSourceHashes(): GeneratedSchemaSourceHashes {
  return Object.freeze(
    Object.fromEntries(
      Object.entries(GENERATED_SCHEMA_FILES).map(([fileName, schema]) => [
        fileName,
        sourceHashForSchema(schema),
      ]),
    ) as GeneratedSchemaSourceHashes,
  );
}

const SNAPSHOTTED_GENERATED_SCHEMA_CONTENTS = captureGeneratedSchemaContents();
const SNAPSHOTTED_GENERATED_SCHEMA_SOURCE_HASHES = captureGeneratedSchemaSourceHashes();

function readSourceHash(content: string): string | undefined {
  const parsed: unknown = JSON.parse(content);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return undefined;
  }
  const value = (parsed as Record<string, unknown>)[GENERATED_TYPEBOX_SOURCE_HASH_FIELD];
  return typeof value === "string" ? value : undefined;
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
    let actualHash: string | undefined;
    try {
      actualHash = readSourceHash(readFileSync(filePath, "utf8"));
    } catch {
      return [fileName];
    }
    return actualHash === SNAPSHOTTED_GENERATED_SCHEMA_SOURCE_HASHES[fileName]
      ? []
      : [fileName];
  });
}

export function renderGeneratedSchemas(): GeneratedSchemaContents {
  return { ...SNAPSHOTTED_GENERATED_SCHEMA_CONTENTS };
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

    const actualContent = readFileSync(filePath, "utf8");
    if (actualContent !== expectedContent) {
      changed.push(fileName);
    }
  }

  const unexpected = listGeneratedJsonFiles(outputDir).filter(
    (fileName) => !(fileName in expectedFiles),
  );
  const pinMismatches = sourceHashMismatches(outputDir, expectedFiles);

  return {
    missing,
    changed,
    unexpected,
    pinMismatches,
  };
}

export function assertGeneratedSchemasFresh(outputDir = GENERATED_DIR): void {
  const drift = getGeneratedSchemaDrift(outputDir);
  const problems = [
    drift.missing.length > 0 ? `missing: ${drift.missing.join(", ")}` : null,
    drift.changed.length > 0 ? `changed: ${drift.changed.join(", ")}` : null,
    drift.unexpected.length > 0 ? `unexpected: ${drift.unexpected.join(", ")}` : null,
    drift.pinMismatches.length > 0
      ? `source hash mismatch: ${drift.pinMismatches.join(", ")}`
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

  return {
    outputDir,
    written,
    removed,
  };
}
