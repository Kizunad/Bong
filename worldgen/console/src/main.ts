// App entry: fetch manifest -> load tiles around spawn -> decode -> mesh ->
// scene; wire layer toggles, the zone list, the blueprint-param textarea, and
// the regen button. This is the real glue — every UI control drives a live
// fetch/decode/mesh path (no placeholder handlers).

import { fetchManifest, postRegen } from "./api";
import { loadTile, tilesByDistanceToSpawn } from "./tile-loader";
import { Viewer, type ViewerLayers } from "./viewer";
import type { Manifest, ManifestTile } from "./types";
import { ZONE_COLORS, ZONE_FALLBACK, type RGB } from "./palette";

const statusEl = document.getElementById("status")!;
const zoneListEl = document.getElementById("zone-list") as HTMLUListElement;
const layerTogglesEl = document.getElementById("layer-toggles")!;
const paramEdit = document.getElementById("param-edit") as HTMLTextAreaElement;
const regenBtn = document.getElementById("regen-btn") as HTMLButtonElement;

function setStatus(msg: string): void {
  statusEl.textContent = msg;
}

const layers: ViewerLayers = { terrain: true, water: false, qi: false, decorations: false };

interface ZoneInfo {
  name: string;
  profile: string;
  raw: unknown; // the blueprint zone object, for the param editor
}

let manifest: Manifest;
let viewer: Viewer;
let blueprintZones: ZoneInfo[] = [];
let selectedZone: string | null = null;

function rgbCss([r, g, b]: RGB): string {
  return `rgb(${r}, ${g}, ${b})`;
}

function buildLayerToggles(): void {
  const defs: { key: keyof ViewerLayers; label: string }[] = [
    { key: "terrain", label: "地形" },
    { key: "water", label: "水体" },
    { key: "qi", label: "灵气热力 (qi_density)" },
    { key: "decorations", label: "装饰点位" },
  ];
  for (const def of defs) {
    const wrap = document.createElement("label");
    wrap.className = "layer-toggle";
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.checked = layers[def.key];
    cb.addEventListener("change", () => {
      layers[def.key] = cb.checked;
      viewer.setLayers({ ...layers });
    });
    wrap.appendChild(cb);
    wrap.appendChild(document.createTextNode(def.label));
    layerTogglesEl.appendChild(wrap);
  }
}

async function loadBlueprintZones(): Promise<void> {
  // The FastAPI console_server exposes the blueprint only indirectly: tile.zones
  // tells us which zones touch which tiles, and POST /api/regen re-bakes a zone
  // from the server's own blueprint copy (it takes only `zone_name`). So the
  // param panel shows the manifest facts the console actually has about a zone —
  // the tiles it spans, its POIs, the pois' qi affinities — as an editable JSON
  // view. Editing is local-only context (the server is the blueprint source of
  // truth on regen); we never pretend an edited field round-trips to the bake.
  const zoneTiles = new Map<string, string[]>();
  for (const tile of manifest.tiles) {
    for (const z of tile.zones) {
      const list = zoneTiles.get(z) ?? [];
      list.push(tile.dir);
      zoneTiles.set(z, list);
    }
  }
  const zonePois = new Map<string, typeof manifest.pois>();
  for (const poi of manifest.pois) {
    const list = zonePois.get(poi.zone) ?? [];
    list.push(poi);
    zonePois.set(poi.zone, list);
  }
  blueprintZones = [...zoneTiles.keys()].sort().map((name) => ({
    name,
    profile: "", // profile not surfaced per-zone by the manifest; swatch falls back
    raw: {
      name,
      tiles: zoneTiles.get(name) ?? [],
      pois: (zonePois.get(name) ?? []).map((p) => ({
        kind: p.kind,
        name: p.name,
        pos_xyz: p.pos_xyz,
        qi_affinity: p.qi_affinity,
      })),
    },
  }));
}

function zoneSwatch(profile: string): RGB {
  return ZONE_COLORS[profile] ?? ZONE_FALLBACK;
}

function buildZoneList(): void {
  zoneListEl.innerHTML = "";
  for (const zone of blueprintZones) {
    const li = document.createElement("li");
    li.dataset.zone = zone.name;
    const sw = document.createElement("span");
    sw.className = "zone-swatch";
    sw.style.background = rgbCss(zoneSwatch(zone.profile));
    li.appendChild(sw);
    li.appendChild(document.createTextNode(zone.name));
    li.addEventListener("click", () => selectZone(zone.name));
    zoneListEl.appendChild(li);
  }
}

function selectZone(name: string): void {
  selectedZone = name;
  for (const li of Array.from(zoneListEl.children) as HTMLElement[]) {
    li.classList.toggle("selected", li.dataset.zone === name);
  }
  const zone = blueprintZones.find((z) => z.name === name);
  paramEdit.value = JSON.stringify(zone?.raw ?? { name }, null, 2);
  regenBtn.disabled = false;
}

async function loadTilesAroundSpawn(): Promise<void> {
  const ordered = tilesByDistanceToSpawn(manifest);
  let loaded = 0;
  for (const tile of ordered) {
    try {
      const decoded = await loadTile(manifest, tile);
      viewer.setTile(decoded, manifest.surface_palette);
      loaded++;
      setStatus(`已加载 ${loaded}/${ordered.length} tiles…`);
    } catch (err) {
      console.error(`tile ${tile.dir} 加载失败`, err);
    }
  }
  setStatus(`就绪 — ${loaded} tiles · ${manifest.pois.length} POI · 世界 ${manifest.world_name}`);
}

async function reloadTiles(tileDirs: string[]): Promise<void> {
  const byDir = new Map<string, ManifestTile>();
  for (const t of manifest.tiles) byDir.set(t.dir, t);
  for (const dir of tileDirs) {
    const tile = byDir.get(dir);
    if (!tile) continue;
    try {
      const decoded = await loadTile(manifest, tile);
      viewer.setTile(decoded, manifest.surface_palette);
    } catch (err) {
      console.error(`regen 后重载 tile ${dir} 失败`, err);
    }
  }
}

async function onRegen(): Promise<void> {
  if (!selectedZone) return;
  regenBtn.disabled = true;
  setStatus(`重生成 zone '${selectedZone}'…`);
  try {
    const result = await postRegen(selectedZone);
    // Refresh the manifest (tile zone membership / palette may shift) then
    // re-fetch + re-mesh exactly the rewritten tiles.
    manifest = await fetchManifest();
    await reloadTiles(result.rewritten_tiles);
    setStatus(`zone '${result.zone_name}' 重生成完成 — 刷新 ${result.tile_count} tiles`);
  } catch (err) {
    setStatus(`重生成失败: ${(err as Error).message}`);
    console.error(err);
  } finally {
    regenBtn.disabled = false;
  }
}

async function main(): Promise<void> {
  const viewport = document.getElementById("viewport")!;
  viewer = new Viewer(viewport);
  buildLayerToggles();
  regenBtn.addEventListener("click", onRegen);

  setStatus("加载 manifest…");
  try {
    manifest = await fetchManifest();
  } catch (err) {
    setStatus(`manifest 加载失败: ${(err as Error).message} — 先跑 console_server`);
    return;
  }

  await loadBlueprintZones();
  buildZoneList();
  viewer.setPoiMarkers(manifest);
  viewer.setLayers({ ...layers });

  await loadTilesAroundSpawn();
}

main().catch((err) => {
  console.error(err);
  setStatus(`启动失败: ${(err as Error).message}`);
});
