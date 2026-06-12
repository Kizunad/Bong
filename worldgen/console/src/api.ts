// HTTP client for the dev console_server (scripts/terrain_gen/console_server.py).
// Endpoints:
//   GET  /api/manifest            -> manifest v2 JSON
//   GET  /api/tile/{x}/{z}/{layer}-> raw octet-stream (.bin); 404 = absent layer
//   POST /api/regen {zone_name, overrides?} -> { zone_name, rewritten_tiles[], tile_count }
//
// All requests go through the vite dev proxy (/api -> :8765), so paths are relative.

import { MAX_SPANS, SPAN_BYTES_PER_COLUMN, SPAN_SENTINEL } from "./decode";
import type { Manifest } from "./types";

export interface RegenResult {
  zone_name: string;
  rewritten_tiles: string[];
  tile_count: number;
}

/**
 * Assert the manifest's spans_encoding matches the constants the decoder is
 * compiled against. The decoder reads spans.bin at a hard-coded stride
 * (`col_idx * SPAN_BYTES_PER_COLUMN`) with a hard-coded MAX_SPANS / sentinel; if
 * a future bake bumps `max_spans` the manifest would say one thing while the
 * decoder silently mis-reads every column. Fail loud at load time instead.
 */
export function assertSpansEncoding(m: Manifest): void {
  const enc = m.spans_encoding;
  if (!enc) {
    throw new Error(
      "manifest missing spans_encoding — the v2 span contract is mandatory " +
        "(the decoder needs max_spans/stride/sentinel to read spans.bin)",
    );
  }
  const mismatches: string[] = [];
  if (enc.max_spans !== MAX_SPANS) {
    mismatches.push(`max_spans ${enc.max_spans} != decoder MAX_SPANS ${MAX_SPANS}`);
  }
  if (enc.bytes_per_column !== SPAN_BYTES_PER_COLUMN) {
    mismatches.push(
      `bytes_per_column ${enc.bytes_per_column} != decoder stride ${SPAN_BYTES_PER_COLUMN}`,
    );
  }
  if (enc.sentinel !== SPAN_SENTINEL) {
    mismatches.push(`sentinel ${enc.sentinel} != decoder SPAN_SENTINEL ${SPAN_SENTINEL}`);
  }
  if (mismatches.length > 0) {
    throw new Error(
      `manifest spans_encoding disagrees with the compiled decoder — every ` +
        `column would be mis-decoded: ${mismatches.join("; ")}. Rebuild the ` +
        `console against this bake's span layout.`,
    );
  }
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
  assertSpansEncoding(m);
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
