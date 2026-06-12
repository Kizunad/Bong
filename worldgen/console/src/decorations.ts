// Decoration overlay: POI pins from the manifest (real x/y/z) + a flora point
// cloud per tile (flora_variant_id != 0). three.js-light: returns Object3D the
// viewer adds to its decoration group.

import * as THREE from "three";
import type { Manifest } from "./types";

/** Build one billboard-ish marker per manifest POI, colored by qi_affinity. */
export function buildPoiMarkers(manifest: Manifest): THREE.Object3D {
  const group = new THREE.Group();
  const geo = new THREE.ConeGeometry(2.5, 7, 6);
  for (const poi of manifest.pois) {
    const [x, y, z] = poi.pos_xyz;
    // Warmer = higher qi affinity.
    const hue = 0.6 - Math.max(0, Math.min(1, poi.qi_affinity)) * 0.55;
    const mat = new THREE.MeshBasicMaterial({ color: new THREE.Color().setHSL(hue, 0.7, 0.55) });
    const mesh = new THREE.Mesh(geo, mat);
    mesh.position.set(x, y + 5, z); // float the cone tip above the surface
    mesh.userData = { poi };
    group.add(mesh);
  }
  return group;
}

/**
 * Build a flora point cloud for a tile from its flora_variant_id buffer (uint8,
 * 0 = no flora). Points are placed at the column surface Y so they sit on the
 * terrain. Returns null when the tile carries no flora layer or no flora.
 */
export function buildFloraPoints(
  tileX: number,
  tileZ: number,
  tileSize: number,
  floraVariantId: Uint8Array | undefined,
  surfaceYByCol: (colIdx: number) => number | null,
  stride: number,
): THREE.Object3D | null {
  if (!floraVariantId) return null;
  const baseX = tileX * tileSize;
  const baseZ = tileZ * tileSize;
  const positions: number[] = [];
  for (let lz = 0; lz < tileSize; lz += stride) {
    for (let lx = 0; lx < tileSize; lx += stride) {
      const colIdx = lz * tileSize + lx;
      if (floraVariantId[colIdx] === 0) continue;
      const sy = surfaceYByCol(colIdx);
      if (sy === null) continue;
      positions.push(baseX + lx + 0.5, sy + 1.5, baseZ + lz + 0.5);
    }
  }
  if (positions.length === 0) return null;
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  const mat = new THREE.PointsMaterial({ color: 0x6fd08a, size: 2.5, sizeAttenuation: true });
  return new THREE.Points(geo, mat);
}
