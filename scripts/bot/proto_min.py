"""Tiny protobuf decoder for bot e2e server_data assertions.

This intentionally covers only the production `bong:server_data` payloads that
bot inventory/container scenarios need. It avoids a Python `protobuf` package
dependency in CI while still decoding real proto3 wire bytes from the server.
"""

from __future__ import annotations

import struct
from typing import Any


class ProtoDecodeError(ValueError):
    pass


def decode_server_data_envelope(data: bytes) -> dict[str, Any] | None:
    fields = _fields(data)
    for field, wire, value in fields:
        if wire != 2:
            continue
        if field == 8:
            return _inventory_snapshot(value)
        if field == 80:
            return _inventory_event(value)
        if field == 90:
            return _container_state(value)
        if field == 119:
            return _loot_container_open(value)
    return None


def _inventory_snapshot(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    equipped = _message(fields, 4)
    return {
        "v": 1,
        "type": "inventory_snapshot",
        "revision": _varint(fields, 1),
        "containers": [_container_snapshot(raw) for raw in _messages(fields, 2)],
        "placed_items": [_placed_inventory_item(raw) for raw in _messages(fields, 3)],
        "equipped": _equipped_inventory_snapshot(equipped),
        "hotbar": [_hotbar_slot(raw) for raw in _messages(fields, 5)],
        "bone_coins": _varint(fields, 6),
        "weight": _inventory_weight(_message(fields, 7)),
        "realm": _string(fields, 8),
        "qi_current": _double(fields, 9),
        "qi_max": _double(fields, 10),
        "body_level": _double(fields, 11),
    }


def _container_snapshot(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "id": _string(fields, 1),
        "name": _string(fields, 2),
        "rows": _varint(fields, 3),
        "cols": _varint(fields, 4),
        "owner_instance_id": _optional_varint(fields, 5),
        "quick_access": bool(_varint(fields, 6)),
    }


def _placed_inventory_item(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "container_id": _string(fields, 1),
        "row": _varint(fields, 2),
        "col": _varint(fields, 3),
        "item": _item_view(_message(fields, 4)),
    }


def _equipped_inventory_snapshot(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "head_worn": [_item_view(_fields(raw)) for raw in _messages(fields, 1)],
        "head_held": _optional_item(fields, 2),
        "chest_worn": [_item_view(_fields(raw)) for raw in _messages(fields, 3)],
        "chest_held": _optional_item(fields, 4),
        "legs_worn": [_item_view(_fields(raw)) for raw in _messages(fields, 5)],
        "legs_held": _optional_item(fields, 6),
        "feet_worn": [_item_view(_fields(raw)) for raw in _messages(fields, 7)],
        "feet_held": _optional_item(fields, 8),
        "main_hand_worn": [_item_view(_fields(raw)) for raw in _messages(fields, 9)],
        "main_hand_held": _optional_item(fields, 10),
        "off_hand_worn": [_item_view(_fields(raw)) for raw in _messages(fields, 11)],
        "off_hand_held": _optional_item(fields, 12),
        "extra_hand_0_worn": [_item_view(_fields(raw)) for raw in _messages(fields, 13)],
        "extra_hand_0_held": _optional_item(fields, 14),
        "extra_hand_1_worn": [_item_view(_fields(raw)) for raw in _messages(fields, 15)],
        "extra_hand_1_held": _optional_item(fields, 16),
    }


def _hotbar_slot(data: bytes) -> dict[str, Any] | None:
    fields = _fields(data)
    if not _has(fields, 1):
        return None
    return _item_view(_message(fields, 1))


def _inventory_weight(fields: list[tuple[int, int, Any]]) -> dict[str, float]:
    return {"current": _double(fields, 1), "max": _double(fields, 2)}


def _inventory_event(data: bytes) -> dict[str, Any] | None:
    fields = _fields(data)
    for field, wire, value in fields:
        if wire != 2:
            continue
        if field == 1:
            moved = _fields(value)
            return {
                "v": 1,
                "type": "inventory_event",
                "kind": "moved",
                "revision": _varint(moved, 1),
                "instance_id": _varint(moved, 2),
                "from": _location(_message(moved, 3)),
                "to": _location(_message(moved, 4)),
            }
        if field == 2:
            dropped = _fields(value)
            return {
                "v": 1,
                "type": "inventory_event",
                "kind": "dropped",
                "revision": _varint(dropped, 1),
                "instance_id": _varint(dropped, 2),
                "from": _location(_message(dropped, 3)),
                "world_pos": [
                    _double(dropped, 4),
                    _double(dropped, 5),
                    _double(dropped, 6),
                ],
                "item": _item_view(_message(dropped, 7)),
            }
        if field == 3:
            stack = _fields(value)
            return {
                "v": 1,
                "type": "inventory_event",
                "kind": "stack_changed",
                "revision": _varint(stack, 1),
                "instance_id": _varint(stack, 2),
                "stack_count": _varint(stack, 3),
            }
        if field == 4:
            durability = _fields(value)
            return {
                "v": 1,
                "type": "inventory_event",
                "kind": "durability_changed",
                "revision": _varint(durability, 1),
                "instance_id": _varint(durability, 2),
                "durability": _double(durability, 3),
            }
    return None


def _loot_container_open(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "loot_container_open",
        "session_id": _varint(fields, 1),
        "source_kind": _string(fields, 2),
        "rows": _varint(fields, 3),
        "cols": _varint(fields, 4),
        "placed_items": [_placed_inventory_item(raw) for raw in _messages(fields, 5)],
        "timeout_wall_secs": _varint(fields, 6),
    }


def _container_state(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "container_state",
        "entity_id": _varint(fields, 1),
        "visual_entity_id": _optional_varint(fields, 10),
    }


def _item_view(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "instance_id": _varint(fields, 1),
        "item_id": _string(fields, 2),
        "display_name": _string(fields, 3),
        "grid_width": _varint(fields, 4),
        "grid_height": _varint(fields, 5),
        "weight": _double(fields, 6),
        "rarity": _string(fields, 7),
        "description": _string(fields, 8),
        "stack_count": _varint(fields, 9),
        "spirit_quality": _double(fields, 10),
        "durability": _double(fields, 11),
    }


def _optional_item(fields: list[tuple[int, int, Any]], field: int) -> dict[str, Any] | None:
    if not _has(fields, field):
        return None
    return _item_view(_message(fields, field))


def _location(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    if _has(fields, 1):
        container = _message(fields, 1)
        return {
            "kind": "container",
            "container_id": _string(container, 1),
            "row": _varint(container, 2),
            "col": _varint(container, 3),
        }
    if _has(fields, 2):
        equip = _message(fields, 2)
        return {
            "kind": "equip",
            "slot": _equip_slot(_varint(equip, 1)),
            "state": _equip_state(_varint(equip, 2)),
        }
    if _has(fields, 3):
        hotbar = _message(fields, 3)
        return {"kind": "hotbar", "index": _varint(hotbar, 1)}
    return {"kind": "unknown"}


def _fields(data: bytes) -> list[tuple[int, int, Any]]:
    pos = 0
    out = []
    while pos < len(data):
        key, pos = _read_varint(data, pos)
        field = key >> 3
        wire = key & 0x07
        if field <= 0:
            raise ProtoDecodeError(f"bad field number {field}")
        if wire == 0:
            value, pos = _read_varint(data, pos)
        elif wire == 1:
            if pos + 8 > len(data):
                raise ProtoDecodeError("truncated fixed64")
            value = data[pos : pos + 8]
            pos += 8
        elif wire == 2:
            size, pos = _read_varint(data, pos)
            if pos + size > len(data):
                raise ProtoDecodeError("truncated length-delimited field")
            value = data[pos : pos + size]
            pos += size
        elif wire == 5:
            if pos + 4 > len(data):
                raise ProtoDecodeError("truncated fixed32")
            value = data[pos : pos + 4]
            pos += 4
        else:
            raise ProtoDecodeError(f"unsupported wire type {wire}")
        out.append((field, wire, value))
    return out


def _read_varint(data: bytes, pos: int) -> tuple[int, int]:
    result = 0
    shift = 0
    while pos < len(data):
        byte = data[pos]
        pos += 1
        result |= (byte & 0x7F) << shift
        if byte < 0x80:
            return result, pos
        shift += 7
        if shift >= 70:
            raise ProtoDecodeError("varint too long")
    raise ProtoDecodeError("truncated varint")


def _has(fields: list[tuple[int, int, Any]], field: int) -> bool:
    return any(existing == field for existing, _wire, _value in fields)


def _values(fields: list[tuple[int, int, Any]], field: int) -> list[Any]:
    return [value for existing, _wire, value in fields if existing == field]


def _varint(fields: list[tuple[int, int, Any]], field: int, default: int = 0) -> int:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 0:
            return int(value)
    return default


def _optional_varint(fields: list[tuple[int, int, Any]], field: int) -> int | None:
    return _varint(fields, field) if _has(fields, field) else None


def _string(fields: list[tuple[int, int, Any]], field: int, default: str = "") -> str:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 2:
            return value.decode("utf-8", errors="replace")
    return default


def _double(fields: list[tuple[int, int, Any]], field: int, default: float = 0.0) -> float:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 1:
            return struct.unpack("<d", value)[0]
    return default


def _message(fields: list[tuple[int, int, Any]], field: int) -> list[tuple[int, int, Any]]:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 2:
            return _fields(value)
    return []


def _messages(fields: list[tuple[int, int, Any]], field: int) -> list[bytes]:
    return [value for value in _values(fields, field) if isinstance(value, bytes)]


def _equip_slot(value: int) -> str:
    return {
        1: "head",
        2: "chest",
        3: "legs",
        4: "feet",
        6: "main_hand",
        7: "off_hand",
        16: "extra_hand_0",
        17: "extra_hand_1",
    }.get(value, "unspecified")


def _equip_state(value: int) -> str:
    return {1: "worn", 2: "held"}.get(value, "unspecified")
