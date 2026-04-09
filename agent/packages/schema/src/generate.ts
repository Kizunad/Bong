import { GENERATED_SCHEMA_FILES } from "./schema-registry.js";
import {
  GENERATED_DIR,
  assertGeneratedSchemasFresh,
  writeGeneratedSchemas,
} from "./generated-artifacts.js";

const args = new Set(process.argv.slice(2));

if (args.has("--check")) {
  assertGeneratedSchemasFresh();
  console.log(
    `generated schema artifacts are fresh (${Object.keys(GENERATED_SCHEMA_FILES).length} files) in ${GENERATED_DIR}`,
  );
} else {
  const result = writeGeneratedSchemas();

  for (const filePath of result.written) {
    console.log(`wrote ${filePath}`);
  }

  for (const filePath of result.removed) {
    console.log(`removed ${filePath}`);
  }

  console.log(`\n${result.written.length} schemas exported to ${result.outputDir}`);
}
