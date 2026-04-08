import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { GENERATED_SCHEMA_FILES } from "./schema-registry.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

export const GENERATED_DIR = join(__dirname, "..", "generated");

export interface GeneratedSchemaDrift {
  missing: string[];
  changed: string[];
  unexpected: string[];
}

export interface WriteGeneratedSchemasResult {
  outputDir: string;
  written: string[];
  removed: string[];
}

type GeneratedSchemaContents = Record<string, string>;

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
        `${JSON.stringify(schema, null, 2)}\n`,
      ]),
    ) as GeneratedSchemaContents,
  );
}

const SNAPSHOTTED_GENERATED_SCHEMA_CONTENTS = captureGeneratedSchemaContents();

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

  return {
    missing,
    changed,
    unexpected,
  };
}

export function assertGeneratedSchemasFresh(outputDir = GENERATED_DIR): void {
  const drift = getGeneratedSchemaDrift(outputDir);
  const problems = [
    drift.missing.length > 0 ? `missing: ${drift.missing.join(", ")}` : null,
    drift.changed.length > 0 ? `changed: ${drift.changed.join(", ")}` : null,
    drift.unexpected.length > 0 ? `unexpected: ${drift.unexpected.join(", ")}` : null,
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
