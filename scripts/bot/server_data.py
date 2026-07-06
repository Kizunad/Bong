"""Helpers for decoding Bong S2C gameplay payloads observed by protocol bots.

Production `bong:server_data` uses protobuf envelopes. The bot keeps this
decoder local to scripts/bot so gameplay scenarios can assert user-visible
payloads without depending on server internals or generated checked-in Python.
"""

from __future__ import annotations

import importlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile
from typing import Any

_ENVELOPE_PB2 = None


def decode_server_data_payload(data: bytes) -> dict[str, Any] | None:
    """Decode `bong:server_data` bytes into a small JSON-like dict.

    Returns None when the bytes are not a server_data envelope. Test builds can
    still emit JSON, so JSON is accepted first; production falls through to
    protobuf.
    """

    stripped = data.lstrip()
    if stripped.startswith(b"{"):
        try:
            decoded = json.loads(data.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            return None
        return decoded if isinstance(decoded, dict) and decoded.get("type") else None

    envelope_pb2 = _load_envelope_pb2()
    envelope = envelope_pb2.ServerDataEnvelope()
    try:
        envelope.ParseFromString(data)
    except Exception:
        return None

    payload_type = envelope.WhichOneof("payload")
    if payload_type is None:
        return None

    payload = getattr(envelope, payload_type)
    if payload_type == "inventory_snapshot":
        return _inventory_snapshot(payload)
    if payload_type == "inventory_event":
        return _inventory_event(payload)
    if payload_type == "loot_container_open":
        return _loot_container_open(payload)
    if payload_type == "container_state":
        return {
            "v": 1,
            "type": "container_state",
            "entity_id": int(payload.entity_id),
            "visual_entity_id": (
                int(payload.visual_entity_id) if payload.HasField("visual_entity_id") else None
            ),
        }
    return {"v": 1, "type": payload_type}


def _load_envelope_pb2():
    global _ENVELOPE_PB2
    if _ENVELOPE_PB2 is not None:
        return _ENVELOPE_PB2

    repo_root = pathlib.Path(__file__).resolve().parents[2]
    proto_root = repo_root / "proto"
    out_dir = pathlib.Path(tempfile.mkdtemp(prefix="bong_bot_proto_"))
    cmd = [
        "protoc",
        f"--proto_path={proto_root}",
        f"--python_out={out_dir}",
        str(proto_root / "bong" / "common.proto"),
        str(proto_root / "bong" / "envelope.proto"),
    ]
    try:
        subprocess.run(cmd, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    except (OSError, subprocess.CalledProcessError) as error:
        raise RuntimeError(
            "无法生成 Bong protobuf Python bindings；需要可用的 protoc 和 google.protobuf"
        ) from error

    sys.path.insert(0, os.fspath(out_dir))
    _ENVELOPE_PB2 = importlib.import_module("bong.envelope_pb2")
    return _ENVELOPE_PB2


def _inventory_snapshot(snapshot) -> dict[str, Any]:
    equipped = snapshot.equipped
    return {
        "v": 1,
        "type": "inventory_snapshot",
        "revision": int(snapshot.revision),
        "containers": [
            {
                "id": c.id,
                "name": c.name,
                "rows": int(c.rows),
                "cols": int(c.cols),
                "owner_instance_id": (
                    int(c.owner_instance_id) if c.HasField("owner_instance_id") else None
                ),
                "quick_access": bool(c.quick_access),
            }
            for c in snapshot.containers
        ],
        "placed_items": [
            {
                "container_id": p.container_id,
                "row": int(p.row),
                "col": int(p.col),
                "item": _item_view(p.item),
            }
            for p in snapshot.placed_items
        ],
        "equipped": {
            "head_worn": [_item_view(item) for item in equipped.head_worn],
            "head_held": _optional_item(equipped, "head_held"),
            "chest_worn": [_item_view(item) for item in equipped.chest_worn],
            "chest_held": _optional_item(equipped, "chest_held"),
            "legs_worn": [_item_view(item) for item in equipped.legs_worn],
            "legs_held": _optional_item(equipped, "legs_held"),
            "feet_worn": [_item_view(item) for item in equipped.feet_worn],
            "feet_held": _optional_item(equipped, "feet_held"),
            "main_hand_worn": [_item_view(item) for item in equipped.main_hand_worn],
            "main_hand_held": _optional_item(equipped, "main_hand_held"),
            "off_hand_worn": [_item_view(item) for item in equipped.off_hand_worn],
            "off_hand_held": _optional_item(equipped, "off_hand_held"),
            "extra_hand_0_worn": [_item_view(item) for item in equipped.extra_hand_0_worn],
            "extra_hand_0_held": _optional_item(equipped, "extra_hand_0_held"),
            "extra_hand_1_worn": [_item_view(item) for item in equipped.extra_hand_1_worn],
            "extra_hand_1_held": _optional_item(equipped, "extra_hand_1_held"),
        },
        "hotbar": [_optional_item(slot, "item") for slot in snapshot.hotbar],
        "bone_coins": int(snapshot.bone_coins),
        "weight": {
            "current": float(snapshot.weight.current),
            "max": float(snapshot.weight.max),
        },
        "realm": snapshot.realm,
        "qi_current": float(snapshot.qi_current),
        "qi_max": float(snapshot.qi_max),
        "body_level": float(snapshot.body_level),
    }


def _inventory_event(event) -> dict[str, Any] | None:
    event_kind = event.WhichOneof("event")
    if event_kind is None:
        return None
    payload = getattr(event, event_kind)
    base = {
        "v": 1,
        "type": "inventory_event",
        "kind": event_kind,
        "revision": int(payload.revision),
        "instance_id": int(payload.instance_id),
    }
    if event_kind == "moved":
        base.update({"from": _location(getattr(payload, "from")), "to": _location(payload.to)})
    elif event_kind == "dropped":
        base.update(
            {
                "from": _location(getattr(payload, "from")),
                "world_pos": [
                    float(payload.world_pos_x),
                    float(payload.world_pos_y),
                    float(payload.world_pos_z),
                ],
                "item": _item_view(payload.item),
            }
        )
    elif event_kind == "stack_changed":
        base["stack_count"] = int(payload.stack_count)
    elif event_kind == "durability_changed":
        base["durability"] = float(payload.durability)
    return base


def _loot_container_open(open_payload) -> dict[str, Any]:
    return {
        "v": 1,
        "type": "loot_container_open",
        "session_id": int(open_payload.session_id),
        "source_kind": open_payload.source_kind,
        "rows": int(open_payload.rows),
        "cols": int(open_payload.cols),
        "placed_items": [
            {
                "container_id": p.container_id,
                "row": int(p.row),
                "col": int(p.col),
                "item": _item_view(p.item),
            }
            for p in open_payload.placed_items
        ],
        "timeout_wall_secs": int(open_payload.timeout_wall_secs),
    }


def _item_view(item) -> dict[str, Any]:
    return {
        "instance_id": int(item.instance_id),
        "item_id": item.item_id,
        "display_name": item.display_name,
        "grid_width": int(item.grid_width),
        "grid_height": int(item.grid_height),
        "weight": float(item.weight),
        "rarity": item.rarity,
        "description": item.description,
        "stack_count": int(item.stack_count),
        "spirit_quality": float(item.spirit_quality),
        "durability": float(item.durability),
    }


def _optional_item(message, field_name: str) -> dict[str, Any] | None:
    return _item_view(getattr(message, field_name)) if message.HasField(field_name) else None


def _location(location) -> dict[str, Any]:
    kind = location.WhichOneof("location")
    if kind == "container":
        c = location.container
        return {
            "kind": "container",
            "container_id": c.container_id,
            "row": int(c.row),
            "col": int(c.col),
        }
    if kind == "equip":
        e = location.equip
        return {
            "kind": "equip",
            "slot": _enum_suffix(e.DESCRIPTOR.fields_by_name["slot"].enum_type, e.slot, "EQUIP_SLOT_"),
            "state": _enum_suffix(
                e.DESCRIPTOR.fields_by_name["state"].enum_type, e.state, "EQUIP_STATE_"
            ),
        }
    if kind == "hotbar":
        return {"kind": "hotbar", "index": int(location.hotbar.index)}
    return {"kind": "unknown"}


def _enum_suffix(enum_type, value: int, prefix: str) -> str:
    enum_value = enum_type.values_by_number.get(value)
    if enum_value is None:
        return "unspecified"
    name = enum_value.name
    if name.endswith("_UNSPECIFIED"):
        return "unspecified"
    return name.removeprefix(prefix).lower()
