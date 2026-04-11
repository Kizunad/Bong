import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";

import { resolveComposeConfig } from "./compose-path.mjs";

test("prefers configured compose file when it exists", () => {
  const result = resolveComposeConfig(
    {
      COMPOSE_FILE: "/workspace/app/docker-compose.yml",
      PROJECT_ROOT: "/workspace/app",
    },
    {
      exists: (filePath) => filePath === "/workspace/app/docker-compose.yml",
      bundledProjectRoot: "/app/project",
    }
  );

  assert.deepEqual(result, {
    composeFile: "/workspace/app/docker-compose.yml",
    projectRoot: "/workspace/app",
  });
});

test("falls back to bundled compose file when mounted path is unavailable", () => {
  const bundledProjectRoot = "/app/project";
  const bundledComposeFile = path.join(bundledProjectRoot, "docker-compose.yml");
  const result = resolveComposeConfig(
    {
      COMPOSE_FILE: "/workspace/app/docker-compose.yml",
      PROJECT_ROOT: "/workspace/app",
    },
    {
      exists: (filePath) => filePath === bundledComposeFile,
      bundledProjectRoot,
    }
  );

  assert.deepEqual(result, {
    composeFile: bundledComposeFile,
    projectRoot: bundledProjectRoot,
  });
});

test("throws a clear error when no compose file is available", () => {
  assert.throws(
    () => resolveComposeConfig({}, { exists: () => false, bundledProjectRoot: "/app/project" }),
    /No docker-compose\.yml found/
  );
});
