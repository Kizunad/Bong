// Pure param-panel logic — NO DOM, NO three.js. The textarea editing law lives
// here so it is unit-testable headlessly (text in -> overrides out / throw),
// instead of being buried in main.ts behind getElementById.
//
// Contract mirror: EDITABLE_ZONE_FIELDS must match the server's
// `_OVERRIDABLE_ZONE_FIELDS` (scripts/terrain_gen/console_server.py). Editing
// anything outside it 400s server-side, so we only ever surface / send these.

import type { ManifestZone } from "./types";

/** Zone blueprint fields the param panel may edit and POST as overrides. */
export const EDITABLE_ZONE_FIELDS = [
  "spirit_qi",
  "danger_level",
  "display_name",
  "worldgen",
] as const;

/** Pull the server-declared overridable subset out of a manifest zone entry. */
export function editableSubset(zone: ManifestZone): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const key of EDITABLE_ZONE_FIELDS) {
    if (key in zone) out[key] = (zone as Record<string, unknown>)[key];
  }
  return out;
}

/**
 * Parse the param-textarea text into an overrides object.
 *
 *  - empty / whitespace-only -> {} (a no-op regen, NOT an error).
 *  - valid JSON object       -> the parsed object (sent as overrides).
 *  - invalid JSON            -> throws (caller surfaces it, refuses to send).
 *  - JSON that isn't an object (array / number / null) -> throws.
 *
 * Kept strict on purpose: we never silently drop a bad edit and fall back to a
 * default bake, because that is exactly the "zombie param panel" failure where
 * the user thinks they changed a parameter but the request ignored it.
 */
export function parseOverrides(text: string): Record<string, unknown> {
  const trimmed = text.trim();
  if (trimmed === "") return {};
  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (err) {
    throw new Error(`参数 JSON 解析失败: ${(err as Error).message}`);
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("参数必须是一个 JSON 对象（{ ... }）");
  }
  return parsed as Record<string, unknown>;
}
