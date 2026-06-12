// HTTP client for the dev console_server (scripts/terrain_gen/console_server.py).
// Endpoints:
//   GET  /api/manifest            -> manifest v2 JSON
//   GET  /api/tile/{x}/{z}/{layer}-> raw octet-stream (.bin); 404 = absent layer
//   POST /api/regen {zone_name, overrides?} -> { zone_name, rewritten_tiles[], tile_count }
//
// All requests go through the vite dev proxy (/api -> :8765), so paths are relative.

import type { Manifest } from "./types";

export interface RegenResult {
  zone_name: string;
  rewritten_tiles: string[];
  tile_count: number;
}

export async function fetchManifest(): Promise<Manifest> {
  const resp = await fetch("/api/manifest");
  if (!resp.ok) {
    throw new Error(`manifest fetch failed: ${resp.status} ${resp.statusText}`);
  }
  const m = (await resp.json()) as Manifest;
  if (m.version !== 2) {
    throw new Error(
      `console only speaks manifest v2 (P0 spans); server returned v${m.version}`,
    );
  }
  return m;
}

/**
 * Fetch one tile binary. Returns the ArrayBuffer, or `null` for an absent layer
 * (HTTP 404) so callers fall back to the layer's safe_default. Other non-2xx
 * statuses throw (400 unknown layer, 503 no manifest, …).
 */
export async function fetchTileLayer(
  tileX: number,
  tileZ: number,
  layer: string,
): Promise<ArrayBuffer | null> {
  const resp = await fetch(`/api/tile/${tileX}/${tileZ}/${encodeURIComponent(layer)}`);
  if (resp.status === 404) return null;
  if (!resp.ok) {
    throw new Error(
      `tile ${tileX},${tileZ} layer '${layer}' failed: ${resp.status} ${resp.statusText}`,
    );
  }
  return resp.arrayBuffer();
}

/**
 * Re-bake a zone. When `overrides` is a non-empty object it is POSTed as a
 * partial blueprint patch (e.g. `{ spirit_qi: 0.8 }` or
 * `{ worldgen: { terrain_profile: "..." } }`) so the server re-bakes the zone
 * with those parameters — the param panel is live, not a read-only view. An
 * unknown/invalid override field comes back as a 400 and surfaces as the thrown
 * error's message.
 */
export async function postRegen(
  zoneName: string,
  overrides?: Record<string, unknown>,
): Promise<RegenResult> {
  const body: { zone_name: string; overrides?: Record<string, unknown> } = {
    zone_name: zoneName,
  };
  if (overrides && Object.keys(overrides).length > 0) {
    body.overrides = overrides;
  }
  const resp = await fetch("/api/regen", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!resp.ok) {
    let detail = `${resp.status} ${resp.statusText}`;
    try {
      const body = await resp.json();
      if (body?.detail) detail = String(body.detail);
    } catch {
      /* non-JSON error body */
    }
    throw new Error(`regen '${zoneName}' failed: ${detail}`);
  }
  return (await resp.json()) as RegenResult;
}
