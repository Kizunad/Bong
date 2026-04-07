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
});
