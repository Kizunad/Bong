import { defineConfig } from "vite";

// Dev-only viewer. The FastAPI console_server (scripts/terrain_gen/console_server.py)
// listens on :8765; we proxy /api there so the browser fetches stay same-origin in
// dev (CORS on the server already allows :5173 directly, but the proxy keeps the
// frontend code free of an absolute backend host).
const API_TARGET = process.env.BONG_CONSOLE_API ?? "http://127.0.0.1:8765";

export default defineConfig({
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: API_TARGET,
        changeOrigin: true,
      },
    },
  },
  test: {
    globals: true,
    environment: "node",
    include: ["test/**/*.test.ts"],
  },
});
