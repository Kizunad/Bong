import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BUNDLED_PROJECT_ROOT = path.resolve(__dirname, "..");

function uniq(values) {
  return [...new Set(values.filter(Boolean))];
}

function isFile(filePath) {
  try {
    return fs.statSync(filePath).isFile();
  } catch {
    return false;
  }
}

export function resolveComposeConfig(env = process.env, options = {}) {
  const exists = options.exists ?? isFile;
  const bundledProjectRoot = options.bundledProjectRoot ?? BUNDLED_PROJECT_ROOT;
  const projectRootCandidates = uniq([
    env.PROJECT_ROOT,
    "/workspace/app",
    bundledProjectRoot,
  ]);
  const fileCandidates = uniq([
    env.COMPOSE_FILE,
    ...projectRootCandidates.map((projectRoot) => path.join(projectRoot, "docker-compose.yml")),
  ]);

  for (const composeFile of fileCandidates) {
    if (exists(composeFile)) {
      return {
        composeFile,
        projectRoot: path.dirname(composeFile),
      };
    }
  }

  throw new Error(`No docker-compose.yml found. Checked: ${fileCandidates.join(", ")}`);
}

export const bundledProjectRoot = BUNDLED_PROJECT_ROOT;
