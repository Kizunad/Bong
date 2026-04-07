import { loadEnv, resolveRuntimeConfig, runRuntime } from "./runtime.js";

async function main(): Promise<void> {
  loadEnv();
  const config = resolveRuntimeConfig(process.argv, process.env);
  await runRuntime(config);
}

main().catch((err) => {
  console.error("[tiandao] fatal:", err);
  process.exit(1);
});
