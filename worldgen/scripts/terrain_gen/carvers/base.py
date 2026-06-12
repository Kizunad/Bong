"""worldgen-v4 P3 §8.1 #1 — span carver framework.

A *carver* takes a tile's folded span columns (the output of
``spans_fold.spans_for_tile``) plus per-column world coordinates and a seed,
samples **3D noise** by height, and rewrites the spans into a richer vertical
shape — overhang, floating-isle underside, arch void, multi-layer cave.  The
whole point of P3 (§8.1 #1 决议: 多段 span 列 + Python 端局部 3D 噪声雕刻) is to
turn the 2.5D span columns into奇幻 3D 地貌 **without introducing any new raster
layer** — a carver only ever mutates ``ColumnSpans``.

Why a solid/air *run* representation under the hood
---------------------------------------------------
``ColumnSpans`` is frozen and validates non-overlap + world range + span[0]=
surface at construction.  Carving by editing the immutable tuple directly is
error-prone (every edit risks an overlap or a y-range bust).  Instead each
carver works on a boolean **solid mask over world Y** (a ``SolidColumn``):

  * rasterize the input spans → a set of solid block-Ys,
  * the carver flips blocks solid↔air by a 3D-noise rule,
  * re-extract contiguous solid runs → spans,
  * enforce the §8.1 hard invariants on the way out
    (MAX_SPANS≤4 / non-overlap / floor<=ceiling / world-Y range / span[0] is the
    surface so ``query_surface`` keeps reading the original ground).

This makes illegal output structurally impossible: the runs come out sorted and
non-overlapping by construction, and ``_runs_to_spans`` does the slot budgeting
+ surface-anchor bookkeeping in one place that every carver shares.

Determinism
-----------
The only randomness is ``noise.*_3d`` keyed by ``seed`` (+ a per-carver salt).
The permutation table is seeded and cached, so two carve passes with the same
seed and coordinates produce byte-identical spans — the determinism the P3 plan
and tests require.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol, runtime_checkable

import numpy as np

from ..fields import (
    MAX_SPANS,
    SPAN_MAX_Y,
    SPAN_MIN_Y,
    ColumnSpans,
)

# Hashing salt mixed into a carver's seed so two carvers in the same chain that
# share the caller seed still draw independent noise fields.
SEED_SALT = {
    "canyon": 0x0C0A,
    "floating_island": 0x1571,
    "arch": 0x0A3C,
    "cave_network": 0xCA7E,
}


def clamp_y(value: int) -> int:
    """Clamp a world Y to the legal span range ``[SPAN_MIN_Y, SPAN_MAX_Y]``."""
    return max(SPAN_MIN_Y, min(SPAN_MAX_Y, int(value)))


@dataclass
class SolidColumn:
    """A column's solid blocks as a boolean mask over world Y.

    Index ``i`` of :attr:`mask` is world Y ``SPAN_MIN_Y + i`` (so the mask spans
    the full legal world range ``[SPAN_MIN_Y, SPAN_MAX_Y]`` inclusive).  Carvers
    flip entries; :meth:`to_spans` re-extracts legal spans.

    :attr:`surface_y` records the original walkable surface (span[0].ceiling)
    so :meth:`to_spans` can re-anchor span[0] on it even after the carve digs a
    void below or floats an isle above.
    """

    mask: np.ndarray  # bool, length = SPAN_MAX_Y - SPAN_MIN_Y + 1
    surface_y: int | None

    HEIGHT = SPAN_MAX_Y - SPAN_MIN_Y + 1  # number of world-Y slots

    @classmethod
    def from_spans(cls, column: ColumnSpans) -> "SolidColumn":
        mask = np.zeros(cls.HEIGHT, dtype=bool)
        for floor_y, ceiling_y in column.spans:
            lo = clamp_y(floor_y) - SPAN_MIN_Y
            hi = clamp_y(ceiling_y) - SPAN_MIN_Y
            mask[lo : hi + 1] = True
        return cls(mask=mask, surface_y=column.surface_ceiling_y)

    def is_solid(self, world_y: int) -> bool:
        idx = world_y - SPAN_MIN_Y
        if idx < 0 or idx >= self.HEIGHT:
            return False
        return bool(self.mask[idx])

    def set_solid(self, world_y: int, solid: bool) -> None:
        idx = clamp_y(world_y) - SPAN_MIN_Y
        self.mask[idx] = solid

    def fill_range(self, floor_y: int, ceiling_y: int, solid: bool) -> None:
        """Set ``[floor_y, ceiling_y]`` inclusive to *solid* (clamped to range)."""
        lo = clamp_y(min(floor_y, ceiling_y)) - SPAN_MIN_Y
        hi = clamp_y(max(floor_y, ceiling_y)) - SPAN_MIN_Y
        self.mask[lo : hi + 1] = solid

    def runs(self) -> list[tuple[int, int]]:
        """Contiguous solid ``(floor_y, ceiling_y)`` runs, world-Y, ascending."""
        return _mask_to_runs(self.mask)

    def to_spans(self) -> ColumnSpans:
        """Re-fold to a legal ColumnSpans (budgeted to MAX_SPANS, surface[0])."""
        return _runs_to_spans(self.runs(), self.surface_y)


def _mask_to_runs(mask: np.ndarray) -> list[tuple[int, int]]:
    """Extract contiguous True runs as world-Y ``(floor, ceiling)`` pairs.

    Vectorized edge-detect: a run starts where a False→True transition happens
    and ends where True→False does.  Padding the mask with a False on both ends
    turns the head/tail runs into ordinary interior transitions, so the starts
    and ends come straight out of ``np.diff`` — byte-identical to the scalar
    scan but ~one ``np.diff`` instead of a 495-iteration Python loop per column
    (``_mask_to_runs`` is the dominant cost of the post-carve refold).
    """
    if not mask.any():
        return []
    # diff of the int view: +1 at each run start, -1 just past each run end.
    edges = np.diff(np.concatenate(([0], mask.view(np.int8), [0])))
    starts = np.flatnonzero(edges == 1)
    ends = np.flatnonzero(edges == -1) - 1  # inclusive ceiling index
    base = int(SPAN_MIN_Y)
    return [(int(s) + base, int(e) + base) for s, e in zip(starts, ends)]


def _runs_to_spans(
    runs: list[tuple[int, int]], surface_y: int | None
) -> ColumnSpans:
    """Fold ascending solid runs into a legal, budgeted ColumnSpans.

    Guarantees the §8.1 hard invariants:
      * runs come in sorted + non-overlapping from ``_mask_to_runs`` — but we may
        have more than ``MAX_SPANS`` of them, so we budget down by **merging the
        smallest adjacent gaps** until ``count <= MAX_SPANS`` (merging keeps the
        column legal — two solid runs plus the gap between them become one solid
        run, never an overlap);
      * **span[0] is the surface span**: the run whose ceiling matches the
        recorded ``surface_y`` (or, if the carve removed it, the highest run at
        or below ``surface_y``) is moved to slot 0, matching the
        ``query_surface`` contract (§8.1 #2).  A fully-void column stays empty.
    """
    if not runs:
        return ColumnSpans(())

    budgeted = _budget_runs(list(runs), MAX_SPANS)

    # Pick the surface run: the run whose ceiling is closest to (and not above)
    # the original surface_y.  Fall back to the topmost run if every run floats
    # above the old surface (e.g. a column that became a pure floating isle).
    surface_idx = _select_surface_run(budgeted, surface_y)
    ordered = [budgeted[surface_idx]] + [
        run for i, run in enumerate(budgeted) if i != surface_idx
    ]
    return ColumnSpans(tuple(ordered))


def _budget_runs(
    runs: list[tuple[int, int]], max_spans: int
) -> list[tuple[int, int]]:
    """Reduce *runs* to at most *max_spans* by merging the narrowest gaps.

    Merging two solid runs across the gap between them keeps every block in
    ``[lower.floor, upper.ceiling]`` solid, so the result is always legal — we
    only ever *lose* a thin air gap, never create an overlap.  We greedily
    merge the smallest gap first so the visually dominant voids survive.
    """
    while len(runs) > max_spans:
        # Find the adjacent pair with the smallest air gap and merge it.
        best_i = 0
        best_gap = None
        for i in range(len(runs) - 1):
            gap = runs[i + 1][0] - runs[i][1] - 1
            if best_gap is None or gap < best_gap:
                best_gap = gap
                best_i = i
        merged = (runs[best_i][0], runs[best_i + 1][1])
        runs = runs[:best_i] + [merged] + runs[best_i + 2 :]
    return runs


def _select_surface_run(
    runs: list[tuple[int, int]], surface_y: int | None
) -> int:
    """Index of the run that should be span[0] (the walkable surface)."""
    if surface_y is None:
        # No recorded surface (was a void) — anchor on the topmost solid run.
        return max(range(len(runs)), key=lambda i: runs[i][1])
    # Prefer the highest run whose ceiling does not exceed the original surface.
    candidates = [i for i, run in enumerate(runs) if run[1] <= surface_y]
    if candidates:
        return max(candidates, key=lambda i: runs[i][1])
    # Every remaining run floats above the old surface; take the lowest of them
    # so span[0] is still the closest thing to the ground.
    return min(range(len(runs)), key=lambda i: runs[i][1])


def validate_spans(column: ColumnSpans) -> ColumnSpans:
    """Assert the §8.1 carve invariants and return the column unchanged.

    ``ColumnSpans.__post_init__`` already enforces non-overlap / floor<=ceiling /
    world range / count<=MAX_SPANS at construction; this wrapper exists so a
    carver chain can re-assert after each step with a carve-specific message and
    so the contract is documented at the carver boundary (defensive double check
    — illegal spans should be impossible to even construct).
    """
    if column.count > MAX_SPANS:
        raise AssertionError(
            f"carved column has {column.count} spans > MAX_SPANS={MAX_SPANS}"
        )
    ordered = sorted(column.spans)
    prev_ceiling: int | None = None
    for floor_y, ceiling_y in ordered:
        if not (SPAN_MIN_Y <= floor_y <= ceiling_y <= SPAN_MAX_Y):
            raise AssertionError(
                f"carved span ({floor_y},{ceiling_y}) is out of range or "
                f"inverted (need {SPAN_MIN_Y} <= floor <= ceiling <= {SPAN_MAX_Y})"
            )
        if prev_ceiling is not None and floor_y <= prev_ceiling + 1:
            raise AssertionError(
                f"carved spans overlap/touch: floor {floor_y} <= prev ceiling "
                f"{prev_ceiling} + 1 (carve must leave a real air gap)"
            )
        prev_ceiling = ceiling_y
    return column


@runtime_checkable
class Carver(Protocol):
    """A span carver: ``(spans, wx, wz, seed) -> spans``.

    Implementations must be **pure + deterministic**: identical inputs (same
    spans, coordinates, seed) yield identical outputs, and the output is always
    a legal ColumnSpans.  Carvers are applied per-column over a tile after the
    base profile folds its 2.5D spans and before raster export (the pipeline
    hook in :func:`apply_carver_chain`).
    """

    name: str

    def carve_column(
        self, column: ColumnSpans, wx: int, wz: int, seed: int
    ) -> ColumnSpans:
        """Return a carved copy of *column* at world ``(wx, wz)``."""
        ...


class BaseCarver:
    """Convenience base: handles seed salting + post-carve legality assertion.

    Subclasses implement :meth:`_carve` on a :class:`SolidColumn`; the base
    wrapper rasterizes spans → SolidColumn, runs the subclass edit, re-folds to
    a legal ColumnSpans, and asserts the invariants.  A subclass therefore only
    has to express "which blocks become air / solid" — slot budgeting, surface
    anchoring, and non-overlap are guaranteed by the framework.
    """

    name: str = "base"
    salt: int = 0

    def _seed_for(self, seed: int) -> int:
        return (seed ^ self.salt) & 0x7FFFFFFF

    def _carve(self, col: SolidColumn, wx: int, wz: int, seed: int) -> None:
        """Mutate *col* in place. Subclasses override."""
        raise NotImplementedError

    def carve_column(
        self, column: ColumnSpans, wx: int, wz: int, seed: int
    ) -> ColumnSpans:
        col = SolidColumn.from_spans(column)
        self._carve(col, wx, wz, self._seed_for(seed))
        return validate_spans(col.to_spans())

    def _gate_mask(
        self,
        wx: np.ndarray,
        surface_y: np.ndarray,
        wz: np.ndarray,
        seed: int,
        active: np.ndarray,
    ) -> np.ndarray:
        """Vectorized "this column *might* carve" mask over a whole tile.

        Default: every active (non-void, surface-bearing) column is a candidate,
        so :meth:`carve_tile` falls back to running the scalar ``_carve`` on all
        of them — correct but slow.  A subclass overrides this to vectorize its
        cheap gating noise (silhouette / wall-strength / depth band) so the
        scalar ``_carve`` only runs on the minority of columns that can actually
        carve.  The mask MUST be a *superset* of the columns the scalar
        ``_carve`` would change — skipping a False column must be exactly
        equivalent to running ``_carve`` and having it no-op, or the vectorized
        path would diverge from ``carve_column`` and break determinism.
        """
        return active

    def carve_tile(
        self,
        columns: list[ColumnSpans],
        wx: np.ndarray,
        wz: np.ndarray,
        seed: int,
    ) -> list[ColumnSpans]:
        """Carve a whole tile, vectorizing the gate so most columns short-circuit.

        ``wx`` / ``wz`` are flat per-column world-coordinate arrays in the same
        row-major order as ``columns``.  Produces output **identical** to calling
        ``carve_column`` on each column (the determinism contract): the gate is a
        superset of carve-able columns, and only gated columns run the scalar
        ``_carve``; the rest are passed through untouched, exactly as a no-op
        ``_carve`` would leave them.
        """
        salted = self._seed_for(seed)
        n = len(columns)
        # Per-column surface (None → -1 sentinel) + "active" (surface-bearing).
        surface = np.full(n, -1, dtype=np.int64)
        active = np.zeros(n, dtype=bool)
        for i, col in enumerate(columns):
            s = col.surface_ceiling_y
            if s is not None:
                surface[i] = s
                active[i] = True
        gate = self._gate_mask(
            np.asarray(wx, dtype=np.float64),
            surface.astype(np.float64),
            np.asarray(wz, dtype=np.float64),
            salted,
            active,
        )
        out: list[ColumnSpans] = []
        for i, column in enumerate(columns):
            if not gate[i]:
                out.append(column)
                continue
            col = SolidColumn.from_spans(column)
            self._carve(col, int(wx[i]), int(wz[i]), salted)
            out.append(validate_spans(col.to_spans()))
        return out


def apply_carver_chain(
    columns: list[ColumnSpans],
    carvers: list[Carver],
    *,
    origin_x: int,
    origin_z: int,
    tile_size: int,
    seed: int,
) -> list[ColumnSpans]:
    """Pipeline hook: apply a carver chain over a tile's folded span columns.

    ``columns`` is row-major (``index = local_z * tile_size + local_x``) — the
    same order ``spans_fold.spans_for_tile`` returns and ``encode_spans_arrays``
    consumes — so the carve slots straight between fold and export.  Each carver
    is applied to every column in chain order; later carvers see the previous
    carver's output (chains compose, e.g. canyon then cave below the floor).

    Returns a new list (does not mutate the input).  Deterministic for a given
    ``seed``: world coordinates + seed fully determine every column.

    Uses each carver's vectorized :meth:`BaseCarver.carve_tile` (gate noise
    computed once over the whole tile, scalar ``_carve`` only on gated columns)
    — orders of magnitude faster than 40k scalar ``carve_column`` calls, and
    produces byte-identical output (carve_tile is a faithful vectorization of
    the scalar loop).
    """
    if not carvers:
        return list(columns)
    n = len(columns)
    idx = np.arange(n)
    wx = (origin_x + (idx % tile_size)).astype(np.float64)
    wz = (origin_z + (idx // tile_size)).astype(np.float64)
    carved = list(columns)
    for carver in carvers:
        carve_tile = getattr(carver, "carve_tile", None)
        if callable(carve_tile):
            carved = carve_tile(carved, wx, wz, seed)
        else:  # pragma: no cover — a non-BaseCarver Carver protocol impl
            carved = [
                carver.carve_column(col, int(wx[i]), int(wz[i]), seed)
                for i, col in enumerate(carved)
            ]
    return carved
