// Headless pins for the LIVE param panel: the textarea edit law (parseOverrides),
// the editable-subset projection, the swatch coloring, and the postRegen body
// shape. These lock the contract that an edited parameter actually reaches the
// server as `overrides` (not a zombie write-only textarea).

import { describe, it, expect, vi, afterEach } from "vitest";
import { editableSubset, parseOverrides, EDITABLE_ZONE_FIELDS } from "../src/params";
import { hashSwatchColor } from "../src/palette";
import { postRegen } from "../src/api";
import type { ManifestZone } from "../src/types";

const ZONE: ManifestZone = {
  name: "spawn",
  display_name: "初醒原",
  terrain_profile: "spawn_plain",
  spirit_qi: 0.3,
  danger_level: 1,
  worldgen: { terrain_profile: "spawn_plain", shape: "ellipse" },
  // a field the panel must NOT surface (not in EDITABLE_ZONE_FIELDS):
  secret_internal: "do not edit",
};

// ---------------------------------------------------------------------------
// editableSubset — only the overridable fields, nothing else
// ---------------------------------------------------------------------------

describe("editableSubset", () => {
  it("projects exactly the overridable fields, dropping non-editable ones", () => {
    const sub = editableSubset(ZONE);
    expect(Object.keys(sub).sort()).toEqual([...EDITABLE_ZONE_FIELDS].sort());
    expect(sub.spirit_qi).toBe(0.3);
    expect(sub.worldgen).toEqual({ terrain_profile: "spawn_plain", shape: "ellipse" });
    expect("secret_internal" in sub).toBe(false);
    expect("name" in sub).toBe(false);
  });

  it("EDITABLE_ZONE_FIELDS mirrors the server's _OVERRIDABLE_ZONE_FIELDS", () => {
    // If the server set changes, this list must change in lockstep (a drift here
    // means the panel edits a field the server 400s on, or hides an editable one).
    expect([...EDITABLE_ZONE_FIELDS].sort()).toEqual(
      ["danger_level", "display_name", "spirit_qi", "worldgen"].sort(),
    );
  });
});

// ---------------------------------------------------------------------------
// parseOverrides — the textarea edit law
// ---------------------------------------------------------------------------

describe("parseOverrides", () => {
  it("parses an edited JSON object into overrides", () => {
    const out = parseOverrides('{ "spirit_qi": 0.8 }');
    expect(out).toEqual({ spirit_qi: 0.8 });
  });

  it("treats empty / whitespace text as a no-op {} (not an error)", () => {
    expect(parseOverrides("")).toEqual({});
    expect(parseOverrides("   \n  ")).toEqual({});
  });

  it("throws on invalid JSON (so onRegen refuses to send, never silent-default)", () => {
    expect(() => parseOverrides("{ spirit_qi: }")).toThrow(/解析失败/);
  });

  it("throws when the JSON is not an object (array / number / null)", () => {
    expect(() => parseOverrides("[1,2,3]")).toThrow(/JSON 对象/);
    expect(() => parseOverrides("42")).toThrow(/JSON 对象/);
    expect(() => parseOverrides("null")).toThrow(/JSON 对象/);
  });

  it("round-trips editableSubset -> JSON -> parseOverrides (the panel's own flow)", () => {
    const text = JSON.stringify(editableSubset(ZONE), null, 2);
    const parsed = parseOverrides(text);
    expect(parsed.spirit_qi).toBe(0.3);
    expect(parsed.worldgen).toEqual({ terrain_profile: "spawn_plain", shape: "ellipse" });
  });
});

// ---------------------------------------------------------------------------
// hashSwatchColor — distinct zones get distinct colors (no all-magenta bug)
// ---------------------------------------------------------------------------

describe("hashSwatchColor", () => {
  it("is deterministic per name", () => {
    expect(hashSwatchColor("rift_valley")).toEqual(hashSwatchColor("rift_valley"));
  });

  it("gives distinct zones distinct colors (the all-magenta fallback bug fix)", () => {
    const names = ["spawn", "peaks", "rift", "marsh", "wastes", "ash"];
    const seen = new Set(names.map((n) => hashSwatchColor(n).join(",")));
    // Hash collisions are possible but vanishingly unlikely for this small set;
    // require at least most to be distinct so the list is readable.
    expect(seen.size).toBeGreaterThanOrEqual(names.length - 1);
  });

  it("returns RGB in 0..255", () => {
    for (const v of hashSwatchColor("any_zone")) {
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThanOrEqual(255);
    }
  });
});

// ---------------------------------------------------------------------------
// postRegen — the request body carries overrides end-to-end
// ---------------------------------------------------------------------------

describe("postRegen request body", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  function mockFetchOk() {
    const fetchMock = vi.fn(async () =>
      new Response(
        JSON.stringify({ zone_name: "spawn", rewritten_tiles: ["tile_0_0"], tile_count: 1 }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);
    return fetchMock;
  }

  function sentBody(fetchMock: ReturnType<typeof vi.fn>): Record<string, unknown> {
    const init = fetchMock.mock.calls[0][1] as RequestInit;
    return JSON.parse(init.body as string);
  }

  it("includes overrides in the POST body when non-empty (live edit reaches server)", async () => {
    const fetchMock = mockFetchOk();
    await postRegen("spawn", { spirit_qi: 0.8 });
    expect(fetchMock).toHaveBeenCalledWith("/api/regen", expect.objectContaining({ method: "POST" }));
    expect(sentBody(fetchMock)).toEqual({ zone_name: "spawn", overrides: { spirit_qi: 0.8 } });
  });

  it("omits overrides when the object is empty (a plain regen)", async () => {
    const fetchMock = mockFetchOk();
    await postRegen("spawn", {});
    expect(sentBody(fetchMock)).toEqual({ zone_name: "spawn" });
  });

  it("omits overrides when none is passed at all", async () => {
    const fetchMock = mockFetchOk();
    await postRegen("spawn");
    expect(sentBody(fetchMock)).toEqual({ zone_name: "spawn" });
  });

  it("surfaces a server 400 (bad override) as a thrown error with the detail", async () => {
    const fetchMock = vi.fn(async () =>
      new Response(JSON.stringify({ detail: "unknown override field(s) ['nope']" }), {
        status: 400,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);
    await expect(postRegen("spawn", { nope: 1 })).rejects.toThrow(/unknown override field/);
  });
});
