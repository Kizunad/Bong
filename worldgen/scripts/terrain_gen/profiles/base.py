from __future__ import annotations

import os
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from functools import lru_cache
from types import MappingProxyType
from typing import Any, Mapping

from ..blueprint import BlueprintZone, TerrainProfileSpec
from ..fields import DEFAULT_FIELD_LAYERS, ZoneFieldPlan


# worldgen-v4 P6 §8.1 — anchor enum for NBT-driven decorations. Mirrors the Rust
# `DecorationAnchor` variants in `server/src/world/terrain/nbt_registry.rs`; any
# change here must change there (and the serde rename pin test) in lockstep.
#   "ground"        — stamp origin sits on the ground surface (top_y + 1).
#   "embedded"      — stamp sinks one block into the surface (grave_mound dome).
#   "hanging"       — stamp hangs below a sky-isle underside (bottom_y − 1).
DECORATION_ANCHORS: tuple[str, ...] = ("ground", "embedded", "hanging")


# worldgen-v4 P6 §8.1 #9 / §6.1 — which decoration kinds retire their procedural
# geometry for authored NBT stamping, and the `server/structures/decorations/<dir>`
# they stamp from + the anchor they use. Kinds not listed here (flower / carver /
# tutorial / coral) keep their procedural path (single-block flora, ground cover,
# carvers, authored compounds) — §8.1 #9's explicit retain list.
#
# `tian_mai_crystal` is the one by-name override: it is `kind="crystal"` but hangs
# from a sky-isle underside, so it maps to the `hanging_crystal/` assets with the
# `hanging` anchor (the Rust `stamp_anchor_for` mirrors this special case).
_KIND_NBT_DIR: dict[str, str] = {
    "tree": "small_tree",
    "shrub": "bush",
    "boulder": "boulder",
    "crystal": "crystal",
    "mushroom": "big_mushroom",
    "fallen_log": "fallen_log",
    "grave_mound": "grave",
}
_KIND_NBT_ANCHOR: dict[str, str] = {
    # Most stamped kinds sit on the surface; grave mounds sink one block.
    "grave_mound": "embedded",
}
# By-name overrides (template dir, anchor) that beat the kind mapping.
_NAME_NBT_OVERRIDE: dict[str, tuple[str, str]] = {
    "tian_mai_crystal": ("hanging_crystal", "hanging"),
}

# Repo-root-relative authored asset dir, discovered once so the template lists
# stay authoritative against the actual files (no hardcoded filename drift).
_DECORATIONS_ROOT = os.path.normpath(
    os.path.join(
        os.path.dirname(__file__),
        "..",
        "..",
        "..",
        "..",
        "server",
        "structures",
        "decorations",
    )
)


@lru_cache(maxsize=None)
def _variants_for_dir(template_dir: str) -> tuple[str, ...]:
    """Sorted ``decorations/<dir>/<variant>.nbt`` ids authored under ``<dir>``.

    Sorted so the manifest order is deterministic across machines (the Rust
    `variants_for_kind` sorts identically before its `% len` pick, so the two
    sides agree on which variant a given hash selects). Empty when the dir does
    not exist yet — the spec then stays on its procedural path, exactly as if no
    NBT mapping had been declared.
    """
    kind_path = os.path.join(_DECORATIONS_ROOT, template_dir)
    if not os.path.isdir(kind_path):
        return ()
    names = sorted(
        f[: -len(".nbt")] for f in os.listdir(kind_path) if f.endswith(".nbt")
    )
    return tuple(f"decorations/{template_dir}/{name}.nbt" for name in names)


def nbt_placement_for(name: str, kind: str) -> tuple[tuple[str, ...], str]:
    """Resolve ``(nbt_templates, anchor)`` for a decoration by name + kind.

    Returns the authored variant template-id list and the anchor the server
    stamps them with. ``((), "ground")`` when the kind/name retires no procedural
    geometry (flowers, carvers, tutorial structures) or no assets are authored —
    the spec then stays on its procedural placement path (§8.1 #9 fallback).
    """
    override = _NAME_NBT_OVERRIDE.get(name)
    if override is not None:
        template_dir, anchor = override
        return _variants_for_dir(template_dir), anchor
    template_dir = _KIND_NBT_DIR.get(kind)
    if template_dir is None:
        return (), "ground"
    anchor = _KIND_NBT_ANCHOR.get(kind, "ground")
    return _variants_for_dir(template_dir), anchor


@dataclass(frozen=True)
class DecorationSpec:
    """A single piece of large-scale terrain flora/decor.

    The worldgen pipeline does NOT place these — it exports where they may go
    via the flora_density / flora_variant_id rasters. Rust (or a datapack)
    reads this spec to know HOW to build each variant: which block palette,
    what size, how rare. That keeps the Python pipeline clean (2D rasters
    only) while still carrying complete "what grows here" information.

    kind examples:
        "tree"      — classic trunk+canopy shape (or abstract equivalent)
        "shrub"     — low-lying vegetation, 1-3 blocks tall
        "boulder"   — large standalone rock mass
        "crystal"   — vertical crystalline structure (stalagmite-like)
        "mushroom"  — wide-capped growth (overworld / nether hybrid)
        "flower"    — small single-block flora
        "coral"     — branching underwater / hanging structure

    worldgen-v4 P6 §8.1 NBT pipeline:
        ``nbt_templates`` — relative paths (under ``server/structures/``) of the
        authored NBT variants for this decoration; the server picks one
        deterministically per placement and stamps it (memcpy-level, no runtime
        gzip). Empty tuple ⇒ this spec stays on the §8.1 #9 procedural path
        (mega_tree / decoration.rs / ground cover / aquatic / single-block flower).
        ``anchor`` — how the stamped template is positioned relative to the
        column surface; one of :data:`DECORATION_ANCHORS`. ``size_range`` is the
        legacy procedural knob and is ignored once ``nbt_templates`` is set.
    """

    name: str
    kind: str
    blocks: tuple[str, ...]
    size_range: tuple[int, int] = (3, 6)
    rarity: float = 0.5
    notes: str = ""
    # P6 §8.1 — NBT-driven placement. Default empty/ground keeps every existing
    # spec on the procedural path, so adding these fields is backward-compatible.
    nbt_templates: tuple[str, ...] = ()
    anchor: str = "ground"

    def __post_init__(self) -> None:
        if self.anchor not in DECORATION_ANCHORS:
            raise ValueError(
                f"DecorationSpec '{self.name}' has anchor={self.anchor!r}; "
                f"must be one of {DECORATION_ANCHORS}. Mismatch with the Rust "
                f"DecorationAnchor enum would silently mis-place stamps."
            )


def decoration_payload(deco: DecorationSpec) -> dict[str, object]:
    """The spec → manifest dict fields shared by every palette serializer.

    Both the global palette builder (``profiles/__init__.py``) and the
    per-profile ecology view (``bakers/raster_export.py``) route through this so
    a new ``DecorationSpec`` field shows up in *both* manifest paths with no
    chance of one going stale. Callers add their own ``global_id`` / ``local_id``
    keys on top.

    P6 §8.1: when a spec does not pin its own ``nbt_templates`` explicitly, the
    NBT placement is auto-resolved from its name/kind via
    :func:`nbt_placement_for` (so a profile author does not have to enumerate the
    authored variant filenames on every spec). An explicit ``nbt_templates`` on
    the spec wins — that is the escape hatch for a future bespoke variant set.
    """
    if deco.nbt_templates:
        nbt_templates: tuple[str, ...] = deco.nbt_templates
        anchor = deco.anchor
    else:
        nbt_templates, anchor = nbt_placement_for(deco.name, deco.kind)
    return {
        "name": deco.name,
        "kind": deco.kind,
        "blocks": list(deco.blocks),
        "size_range": list(deco.size_range),
        "rarity": deco.rarity,
        "notes": deco.notes,
        # P6 §8.1 — NBT-driven placement fields (auto-resolved by kind unless the
        # spec pinned its own templates).
        "nbt_templates": list(nbt_templates),
        "anchor": anchor,
    }


@dataclass(frozen=True)
class EcologySpec:
    """Per-profile ecology: flora/decor palette + ambient hints.

    Attached to each TerrainProfileGenerator. Serialized verbatim into the
    raster manifest so Rust / datapack consumers can place decorations
    without reading Python source.
    """

    decorations: tuple[DecorationSpec, ...] = ()
    ambient_effects: tuple[str, ...] = ()          # e.g. "qi_particles", "ash", "mist"
    notes: str = ""


@dataclass(frozen=True)
class CarverSpec:
    """A declarative span-carver entry on a terrain profile (worldgen-v4 P3 §8.1 #1).

    ``kind`` names a carver in ``carvers.CARVER_REGISTRY`` (canyon /
    floating_island / arch / cave_network); ``params`` are its constructor
    kwargs.  A profile declares an ordered tuple of these as its ``carvers``
    class attribute; the export hook (``raster_export._zone_carver_chain``)
    folds them into a real ``Carver`` chain that runs **after** the 2.5D span
    fold and **before** raster export — turning the flat span columns into 3D
    overhang / floating-isle / arch / cave-network geometry without adding any
    new raster layer (§8.1 #1/#12).

    Declared on the profile (not the blueprint) so the carve chain lives next to
    the surface logic it sculpts and is versioned with it.

    ``params`` is wrapped in a read-only ``MappingProxyType`` at construction so a
    spec declared as a **class-level constant** (the common case — profiles list
    their carvers in a class attribute) cannot have its params dict mutated at
    runtime by a downstream consumer (``build_carver`` reads them per export).
    Without this freeze, one export run popping/editing the shared dict would
    silently corrupt the chain for every subsequent run in the same process.
    """

    kind: str
    params: Mapping[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        # frozen=True ⇒ must bypass the field assignment guard to re-wrap.
        object.__setattr__(
            self, "params", MappingProxyType(dict(self.params))
        )


@dataclass(frozen=True)
class ProfileContext:
    zone: BlueprintZone
    profile_spec: TerrainProfileSpec


_DEFAULT_ECOLOGY = EcologySpec()


class TerrainProfileGenerator(ABC):
    profile_name: str = ""
    extra_layers: tuple[str, ...] = ()
    ecology: EcologySpec = _DEFAULT_ECOLOGY
    # worldgen-v4 P3 §8.1 #1 — declarative carver chain. Empty for flat profiles;
    # the landscape profiles (rift_valley → canyon, sky_isle → floating_island,
    # cave_network → cave_network) declare a chain that the export hook applies to
    # the folded span columns. Order matters: later carvers see earlier output.
    carvers: tuple[CarverSpec, ...] = ()

    def plan(self, context: ProfileContext) -> ZoneFieldPlan:
        required_layers = tuple(dict.fromkeys(DEFAULT_FIELD_LAYERS + self.extra_layers))
        return ZoneFieldPlan(
            zone_name=context.zone.name,
            display_name=context.zone.display_name,
            profile_name=self.profile_name,
            generator_name=self.__class__.__name__,
            shape=context.zone.worldgen.shape,
            bounds_xz=context.zone.bounds_xz,
            boundary_mode=context.zone.worldgen.boundary.mode,
            boundary_width=context.zone.worldgen.boundary.width,
            required_layers=required_layers,
            extra_layers=self.extra_layers,
            landmarks=context.zone.worldgen.landmarks,
            notes=self.build_notes(context),
            # plan-terrain-wiring-v1 P0 M2 — passthrough from BlueprintZone.
            architectural_layout=context.zone.architectural_layout,
            compound_flatten_radius=context.zone.compound_flatten_radius,
        )

    @abstractmethod
    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        raise NotImplementedError
