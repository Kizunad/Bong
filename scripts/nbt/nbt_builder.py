"""
nbt_builder.py — Minecraft Structure NBT generator (pure Python, no dependencies).

Generates valid Minecraft 1.20.1 Structure Block .nbt files using only
stdlib `struct` + `gzip`.

NBT binary spec: https://wiki.vg/NBT
Structure format: https://minecraft.wiki/w/Structure_block_file_format

Usage:
    from nbt_builder import StructureBuilder
    sb = StructureBuilder(size_x=5, size_y=3, size_z=5)
    sb.set_block(0, 0, 0, "minecraft:stone_bricks")
    sb.set_block(1, 0, 0, "minecraft:mossy_stone_bricks")
    sb.save("output.nbt")
"""

from __future__ import annotations

import gzip
import io
import struct
from dataclasses import dataclass
from typing import Any

# ── NBT tag type IDs ──
TAG_END = 0
TAG_BYTE = 1
TAG_SHORT = 2
TAG_INT = 3
TAG_LONG = 4
TAG_FLOAT = 5
TAG_DOUBLE = 6
TAG_BYTE_ARRAY = 7
TAG_STRING = 8
TAG_LIST = 9
TAG_COMPOUND = 10
TAG_INT_ARRAY = 11
TAG_LONG_ARRAY = 12


class _NBTWriter:
    """Low-level NBT binary writer."""

    def __init__(self) -> None:
        self._buf = io.BytesIO()

    # ── primitives ──

    def _write_byte(self, v: int) -> None:
        self._buf.write(struct.pack(">b", v))

    def _write_ubyte(self, v: int) -> None:
        self._buf.write(struct.pack(">B", v))

    def _write_short(self, v: int) -> None:
        self._buf.write(struct.pack(">h", v))

    def _write_int(self, v: int) -> None:
        self._buf.write(struct.pack(">i", v))

    def _write_long(self, v: int) -> None:
        self._buf.write(struct.pack(">q", v))

    def _write_float(self, v: float) -> None:
        self._buf.write(struct.pack(">f", v))

    def _write_double(self, v: float) -> None:
        self._buf.write(struct.pack(">d", v))

    def _write_string(self, s: str) -> None:
        encoded = s.encode("utf-8")
        self._write_short(len(encoded))
        self._buf.write(encoded)

    # ── named tag header ──

    def _write_tag_header(self, tag_type: int, name: str) -> None:
        self._write_ubyte(tag_type)
        self._write_string(name)

    # ── payload writers (no header) ──

    def _write_compound_payload(self, data: dict[str, Any]) -> None:
        for key, value in data.items():
            self._write_named_tag(key, value)
        self._write_ubyte(TAG_END)

    def _write_list_payload(self, items: list, element_type: int) -> None:
        self._write_ubyte(element_type)
        self._write_int(len(items))
        for item in items:
            self._write_payload(element_type, item)

    def _write_payload(self, tag_type: int, value: Any) -> None:
        if tag_type == TAG_BYTE:
            self._write_byte(value)
        elif tag_type == TAG_SHORT:
            self._write_short(value)
        elif tag_type == TAG_INT:
            self._write_int(value)
        elif tag_type == TAG_LONG:
            self._write_long(value)
        elif tag_type == TAG_FLOAT:
            self._write_float(value)
        elif tag_type == TAG_DOUBLE:
            self._write_double(value)
        elif tag_type == TAG_STRING:
            self._write_string(value)
        elif tag_type == TAG_COMPOUND:
            self._write_compound_payload(value)
        elif tag_type == TAG_LIST:
            elem_type, items = value
            self._write_list_payload(items, elem_type)
        elif tag_type == TAG_INT_ARRAY:
            self._write_int(len(value))
            for v in value:
                self._write_int(v)
        elif tag_type == TAG_BYTE_ARRAY:
            self._write_int(len(value))
            for v in value:
                self._write_byte(v)
        else:
            raise ValueError(f"Unknown tag type: {tag_type}")

    def _infer_tag_type(self, value: Any) -> int:
        if isinstance(value, NBTByte):
            return TAG_BYTE
        if isinstance(value, NBTShort):
            return TAG_SHORT
        if isinstance(value, NBTInt):
            return TAG_INT
        if isinstance(value, NBTLong):
            return TAG_LONG
        if isinstance(value, NBTFloat):
            return TAG_FLOAT
        if isinstance(value, NBTDouble):
            return TAG_DOUBLE
        if isinstance(value, str):
            return TAG_STRING
        if isinstance(value, dict):
            return TAG_COMPOUND
        if isinstance(value, NBTList):
            return TAG_LIST
        if isinstance(value, NBTIntArray):
            return TAG_INT_ARRAY
        if isinstance(value, NBTByteArray):
            return TAG_BYTE_ARRAY
        raise TypeError(f"Cannot infer NBT type for {type(value)}: {value!r}")

    def _write_named_tag(self, name: str, value: Any) -> None:
        tag_type = self._infer_tag_type(value)
        self._write_tag_header(tag_type, name)
        if tag_type == TAG_LIST:
            elem_type, items = value.element_type, value.items
            self._write_list_payload(items, elem_type)
        elif tag_type in (TAG_BYTE, TAG_SHORT, TAG_INT, TAG_LONG):
            self._write_payload(tag_type, int(value))
        elif tag_type in (TAG_FLOAT, TAG_DOUBLE):
            self._write_payload(tag_type, float(value))
        elif tag_type == TAG_INT_ARRAY:
            self._write_payload(tag_type, list(value))
        elif tag_type == TAG_BYTE_ARRAY:
            self._write_payload(tag_type, list(value))
        else:
            self._write_payload(tag_type, value)

    def write_root(self, root_name: str, root_compound: dict[str, Any]) -> bytes:
        """Write a root TAG_Compound and return raw NBT bytes."""
        self._write_tag_header(TAG_COMPOUND, root_name)
        self._write_compound_payload(root_compound)
        return self._buf.getvalue()


# ── typed wrappers so we can distinguish int sizes ──


class NBTByte(int):
    """NBT Byte (-128..127)."""
    pass


class NBTShort(int):
    """NBT Short (-32768..32767)."""
    pass


class NBTInt(int):
    """NBT Int."""
    pass


class NBTLong(int):
    """NBT Long."""
    pass


class NBTFloat(float):
    """NBT Float."""
    pass


class NBTDouble(float):
    """NBT Double."""
    pass


class NBTList:
    """NBT List with explicit element type."""

    def __init__(self, element_type: int, items: list) -> None:
        self.element_type = element_type
        self.items = items


class NBTIntArray(list):
    """NBT IntArray."""
    pass


class NBTByteArray(list):
    """NBT ByteArray."""
    pass


# ── Structure Builder ──


class StructureBuilder:
    """
    High-level builder for Minecraft 1.20.1 Structure Block .nbt files.

    Coordinate system: (x, y, z) where y is up.
    """

    DATA_VERSION = 3465  # MC 1.20.1

    def __init__(self, size_x: int, size_y: int, size_z: int) -> None:
        self.size = (size_x, size_y, size_z)
        self._palette: list[dict[str, Any]] = []
        self._palette_index: dict[str, int] = {}
        self._blocks: list[dict[str, Any]] = []

        # Pre-register air as palette 0 (implicit in MC but explicit helps)
        # Actually, Minecraft structures do NOT include air blocks by default;
        # only non-air blocks go in the blocks list. We skip air.

    def _get_palette_id(self, block_name: str, properties: dict[str, str] | None = None) -> int:
        """Return palette index for given block, registering if new."""
        # Build a hashable key
        prop_key = tuple(sorted(properties.items())) if properties else ()
        cache_key = (block_name, prop_key)
        key_str = str(cache_key)

        if key_str in self._palette_index:
            return self._palette_index[key_str]

        entry: dict[str, Any] = {"Name": block_name}
        if properties:
            entry["Properties"] = {k: v for k, v in properties.items()}
        idx = len(self._palette)
        self._palette.append(entry)
        self._palette_index[key_str] = idx
        return idx

    def set_block(
        self,
        x: int, y: int, z: int,
        block_name: str,
        properties: dict[str, str] | None = None,
        nbt: dict[str, Any] | None = None,
    ) -> None:
        """Place a block at (x, y, z). block_name should be like 'minecraft:stone'."""
        if not block_name.startswith("minecraft:"):
            block_name = f"minecraft:{block_name}"
        state_id = self._get_palette_id(block_name, properties)
        block_entry: dict[str, Any] = {
            "pos": [x, y, z],
            "state": state_id,
        }
        if nbt:
            block_entry["nbt"] = nbt
        self._blocks.append(block_entry)

    def fill(
        self,
        x1: int, y1: int, z1: int,
        x2: int, y2: int, z2: int,
        block_name: str,
        properties: dict[str, str] | None = None,
    ) -> None:
        """Fill a rectangular volume with the given block (inclusive on both ends)."""
        for x in range(min(x1, x2), max(x1, x2) + 1):
            for y in range(min(y1, y2), max(y1, y2) + 1):
                for z in range(min(z1, z2), max(z1, z2) + 1):
                    self.set_block(x, y, z, block_name, properties)

    def fill_hollow(
        self,
        x1: int, y1: int, z1: int,
        x2: int, y2: int, z2: int,
        block_name: str,
        properties: dict[str, str] | None = None,
    ) -> None:
        """Fill only the shell of a rectangular volume."""
        xmin, xmax = min(x1, x2), max(x1, x2)
        ymin, ymax = min(y1, y2), max(y1, y2)
        zmin, zmax = min(z1, z2), max(z1, z2)
        for x in range(xmin, xmax + 1):
            for y in range(ymin, ymax + 1):
                for z in range(zmin, zmax + 1):
                    if (x in (xmin, xmax) or y in (ymin, ymax) or z in (zmin, zmax)):
                        self.set_block(x, y, z, block_name, properties)

    def fill_walls(
        self,
        x1: int, y1: int, z1: int,
        x2: int, y2: int, z2: int,
        block_name: str,
        properties: dict[str, str] | None = None,
    ) -> None:
        """Fill only the 4 vertical walls (no floor/ceiling)."""
        xmin, xmax = min(x1, x2), max(x1, x2)
        ymin, ymax = min(y1, y2), max(y1, y2)
        zmin, zmax = min(z1, z2), max(z1, z2)
        for x in range(xmin, xmax + 1):
            for y in range(ymin, ymax + 1):
                for z in range(zmin, zmax + 1):
                    if x in (xmin, xmax) or z in (zmin, zmax):
                        self.set_block(x, y, z, block_name, properties)

    def fill_floor(
        self,
        x1: int, z1: int,
        x2: int, z2: int,
        y: int,
        block_name: str,
        properties: dict[str, str] | None = None,
    ) -> None:
        """Fill a horizontal plane at height y."""
        for x in range(min(x1, x2), max(x1, x2) + 1):
            for z in range(min(z1, z2), max(z1, z2) + 1):
                self.set_block(x, y, z, block_name, properties)

    def _build_nbt_compound(self) -> dict[str, Any]:
        """Build the root compound for the structure."""
        # palette: List of compounds
        palette_compounds = []
        for entry in self._palette:
            comp: dict[str, Any] = {"Name": entry["Name"]}
            if "Properties" in entry:
                comp["Properties"] = {k: v for k, v in entry["Properties"].items()}
            palette_compounds.append(comp)

        # blocks: List of compounds
        block_compounds = []
        for blk in self._blocks:
            comp: dict[str, Any] = {
                "pos": NBTList(TAG_INT, [NBTInt(blk["pos"][0]), NBTInt(blk["pos"][1]), NBTInt(blk["pos"][2])]),
                "state": NBTInt(blk["state"]),
            }
            if "nbt" in blk:
                comp["nbt"] = blk["nbt"]
            block_compounds.append(comp)

        root: dict[str, Any] = {
            "DataVersion": NBTInt(self.DATA_VERSION),
            "size": NBTList(TAG_INT, [NBTInt(self.size[0]), NBTInt(self.size[1]), NBTInt(self.size[2])]),
            "palette": NBTList(TAG_COMPOUND, palette_compounds),
            "blocks": NBTList(TAG_COMPOUND, block_compounds),
            "entities": NBTList(TAG_COMPOUND, []),
        }
        return root

    def save(self, path: str) -> None:
        """Write gzip-compressed NBT to file."""
        writer = _NBTWriter()
        root = self._build_nbt_compound()
        raw = writer.write_root("", root)
        with gzip.open(path, "wb") as f:
            f.write(raw)

    def get_stats(self) -> dict[str, Any]:
        """Return block count stats for review."""
        from collections import Counter
        counts: Counter[str] = Counter()
        for blk in self._blocks:
            palette_entry = self._palette[blk["state"]]
            counts[palette_entry["Name"]] += 1
        return {
            "size": self.size,
            "total_blocks": len(self._blocks),
            "palette_size": len(self._palette),
            "block_counts": dict(counts.most_common()),
        }

    def ascii_top_view(self, y_level: int | None = None) -> str:
        """Generate ASCII top-down view at given Y level (or all Y averaged)."""
        sx, sy, sz = self.size
        if y_level is not None:
            grid = [["." for _ in range(sz)] for _ in range(sx)]
            for blk in self._blocks:
                bx, by, bz = blk["pos"]
                if by == y_level and 0 <= bx < sx and 0 <= bz < sz:
                    name = self._palette[blk["state"]]["Name"]
                    ch = _block_char(name)
                    grid[bx][bz] = ch
        else:
            # Show highest block at each x,z
            highest: dict[tuple[int, int], tuple[int, str]] = {}
            for blk in self._blocks:
                bx, by, bz = blk["pos"]
                key = (bx, bz)
                if key not in highest or by > highest[key][0]:
                    name = self._palette[blk["state"]]["Name"]
                    highest[key] = (by, _block_char(name))
            grid = [["." for _ in range(sz)] for _ in range(sx)]
            for (bx, bz), (_, ch) in highest.items():
                if 0 <= bx < sx and 0 <= bz < sz:
                    grid[bx][bz] = ch

        lines = []
        lines.append(f"Top view (y={y_level if y_level is not None else 'max'}) — {sx}x{sz}")
        lines.append("  " + "".join(str(z % 10) for z in range(sz)))
        for x in range(sx):
            lines.append(f"{x:2}" + "".join(grid[x]))
        return "\n".join(lines)


def _block_char(name: str) -> str:
    """Map block name to a single ASCII character for visualization."""
    mapping = {
        "minecraft:stone_bricks": "B",
        "minecraft:mossy_stone_bricks": "M",
        "minecraft:cracked_stone_bricks": "C",
        "minecraft:chiseled_stone_bricks": "H",
        "minecraft:mossy_cobblestone": "m",
        "minecraft:cobblestone": "c",
        "minecraft:stone": "s",
        "minecraft:polished_blackstone": "P",
        "minecraft:chiseled_polished_blackstone": "X",
        "minecraft:polished_blackstone_bricks": "p",
        "minecraft:polished_blackstone_slab": "=",
        "minecraft:polished_blackstone_stairs": "/",
        "minecraft:purple_terracotta": "T",
        "minecraft:podzol": "~",
        "minecraft:soul_sand": "S",
        "minecraft:soul_soil": "O",
        "minecraft:purple_stained_glass": "G",
        "minecraft:magenta_stained_glass": "g",
        "minecraft:cauldron": "U",
        "minecraft:campfire": "F",
        "minecraft:soul_campfire": "f",
        "minecraft:bone_block": "b",
        "minecraft:skeleton_skull": "k",
        "minecraft:water": "w",
        "minecraft:bubble_column": "W",
        "minecraft:oak_log": "L",
        "minecraft:dark_oak_log": "D",
        "minecraft:spruce_log": "I",
        "minecraft:oak_planks": "l",
        "minecraft:dark_oak_planks": "d",
        "minecraft:spruce_planks": "i",
        "minecraft:oak_stairs": "<",
        "minecraft:oak_slab": "-",
        "minecraft:stone_slab": "_",
        "minecraft:stone_brick_slab": "=",
        "minecraft:air": " ",
        "minecraft:grass_block": ",",
        "minecraft:dirt": ".",
        "minecraft:gravel": ":",
        "minecraft:clay": ";",
        "minecraft:iron_bars": "|",
        "minecraft:chain": "!",
        "minecraft:lantern": "*",
        "minecraft:soul_lantern": "o",
        "minecraft:deepslate_bricks": "N",
        "minecraft:deepslate_tiles": "n",
        "minecraft:flower_pot": "v",
        "minecraft:dead_bush": "t",
        "minecraft:cobweb": "%",
    }
    return mapping.get(name, "#")


def read_nbt_stats(path: str) -> dict[str, Any]:
    """Read an NBT file and return basic stats (for review/verification)."""
    import gzip as _gzip
    with _gzip.open(path, "rb") as f:
        data = f.read()
    # Very minimal parser — just verify it's valid NBT
    if len(data) < 3:
        return {"error": "File too small"}
    tag_type = data[0]
    if tag_type != TAG_COMPOUND:
        return {"error": f"Root tag is {tag_type}, expected TAG_Compound (10)"}
    return {
        "file_size_bytes": len(data),
        "valid_nbt_root": True,
        "raw_size": len(data),
    }


class _NBTReader:
    """Low-level NBT binary reader."""

    def __init__(self, data: bytes) -> None:
        self._data = data
        self._pos = 0

    def _read_byte(self) -> int:
        v = struct.unpack_from(">b", self._data, self._pos)[0]
        self._pos += 1
        return v

    def _read_ubyte(self) -> int:
        v = struct.unpack_from(">B", self._data, self._pos)[0]
        self._pos += 1
        return v

    def _read_short(self) -> int:
        v = struct.unpack_from(">h", self._data, self._pos)[0]
        self._pos += 2
        return v

    def _read_int(self) -> int:
        v = struct.unpack_from(">i", self._data, self._pos)[0]
        self._pos += 4
        return v

    def _read_long(self) -> int:
        v = struct.unpack_from(">q", self._data, self._pos)[0]
        self._pos += 8
        return v

    def _read_float(self) -> float:
        v = struct.unpack_from(">f", self._data, self._pos)[0]
        self._pos += 4
        return v

    def _read_double(self) -> float:
        v = struct.unpack_from(">d", self._data, self._pos)[0]
        self._pos += 8
        return v

    def _read_string(self) -> str:
        length = self._read_short()
        s = self._data[self._pos : self._pos + length].decode("utf-8")
        self._pos += length
        return s

    def _read_payload(self, tag_type: int) -> Any:
        if tag_type == TAG_BYTE:
            return self._read_byte()
        if tag_type == TAG_SHORT:
            return self._read_short()
        if tag_type == TAG_INT:
            return self._read_int()
        if tag_type == TAG_LONG:
            return self._read_long()
        if tag_type == TAG_FLOAT:
            return self._read_float()
        if tag_type == TAG_DOUBLE:
            return self._read_double()
        if tag_type == TAG_STRING:
            return self._read_string()
        if tag_type == TAG_BYTE_ARRAY:
            length = self._read_int()
            arr = [self._read_byte() for _ in range(length)]
            return arr
        if tag_type == TAG_INT_ARRAY:
            length = self._read_int()
            arr = [self._read_int() for _ in range(length)]
            return arr
        if tag_type == TAG_LONG_ARRAY:
            length = self._read_int()
            arr = [self._read_long() for _ in range(length)]
            return arr
        if tag_type == TAG_LIST:
            elem_type = self._read_ubyte()
            length = self._read_int()
            return [self._read_payload(elem_type) for _ in range(length)]
        if tag_type == TAG_COMPOUND:
            return self._read_compound()
        raise ValueError(f"Unknown tag type: {tag_type}")

    def _read_compound(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        while True:
            tag_type = self._read_ubyte()
            if tag_type == TAG_END:
                break
            name = self._read_string()
            result[name] = self._read_payload(tag_type)
        return result

    def read_root(self) -> tuple[str, dict[str, Any]]:
        """Read root TAG_Compound. Returns (root_name, compound_dict)."""
        tag_type = self._read_ubyte()
        if tag_type != TAG_COMPOUND:
            raise ValueError(f"Root tag must be TAG_Compound (10), got {tag_type}")
        name = self._read_string()
        compound = self._read_compound()
        return name, compound


@dataclass(frozen=True)
class StructureBlock:
    """A single block from a loaded NBT structure."""

    pos: tuple[int, int, int]
    block_name: str
    properties: dict[str, str]


def load_structure(path: str) -> list[StructureBlock]:
    """Load an NBT structure file and return a list of (pos, block_name, properties).

    This is the read counterpart to StructureBuilder.save().
    Parses the MC 1.20.1 Structure Block .nbt format and extracts all
    non-air blocks with their positions and block state properties.
    """
    with gzip.open(path, "rb") as f:
        data = f.read()

    reader = _NBTReader(data)
    _name, root = reader.read_root()

    palette: list[dict[str, Any]] = root.get("palette", [])
    blocks: list[dict[str, Any]] = root.get("blocks", [])

    result: list[StructureBlock] = []
    for block in blocks:
        pos_list = block.get("pos", [0, 0, 0])
        state_idx = block.get("state", 0)
        if state_idx >= len(palette):
            continue

        palette_entry = palette[state_idx]
        block_name = palette_entry.get("Name", "minecraft:air")

        # Skip air blocks
        if block_name == "minecraft:air":
            continue

        properties = palette_entry.get("Properties", {})
        result.append(
            StructureBlock(
                pos=(int(pos_list[0]), int(pos_list[1]), int(pos_list[2])),
                block_name=block_name,
                properties=dict(properties) if properties else {},
            )
        )

    return result


if __name__ == "__main__":
    # Quick self-test
    sb = StructureBuilder(3, 3, 3)
    sb.set_block(0, 0, 0, "minecraft:stone_bricks")
    sb.set_block(1, 0, 0, "minecraft:mossy_stone_bricks")
    sb.set_block(2, 0, 2, "minecraft:cobblestone")
    sb.save("/tmp/test_structure.nbt")
    stats = sb.get_stats()
    print(f"Stats: {stats}")
    print(sb.ascii_top_view(y_level=0))
    info = read_nbt_stats("/tmp/test_structure.nbt")
    print(f"File info: {info}")

    # Test load_structure round-trip
    blocks = load_structure("/tmp/test_structure.nbt")
    print(f"Loaded {len(blocks)} blocks:")
    for b in blocks:
        print(f"  {b.pos} -> {b.block_name} {b.properties}")
    assert len(blocks) == 3, f"Expected 3 blocks, got {len(blocks)}"
    print("Self-test passed!")
