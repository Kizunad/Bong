"""bot smoke 断言用的零依赖 protobuf wire 小工具。

这里刻意不做生成式 protobuf binding。Bot 场景只用它做两件浅层检查：
- 识别 `bong:server_data` payload 的 oneof 类型。
- 从 inventory_snapshot 里取 item instance_id，用来驱动真实 client_request intent。

数值级、全 schema 的深断言留给后续 P6，在 Python binding/codegen 策略定下来后再做。
"""

from __future__ import annotations

from dataclasses import dataclass

WIRE_VARINT = 0
WIRE_64BIT = 1
WIRE_LEN = 2
WIRE_32BIT = 5

SERVER_DATA_PAYLOAD_NAMES = {
    1: "welcome",
    2: "heartbeat",
    3: "narration",
    4: "zone_info",
    5: "player_state",
    6: "cultivation_detail",
    7: "skill_xp_gain",
    8: "inventory_snapshot",
    11: "alchemy_furnace",
    12: "alchemy_session",
    15: "alchemy_recipe_book",
    17: "forge_station",
    18: "forge_session",
    19: "forge_outcome",
    20: "forge_blueprint_book",
    25: "botany_harvest_progress",
    30: "gathering_session",
    31: "lingtian_session",
}

EQUIPPED_ITEM_FIELDS = {
    1: ("head", "worn"),
    2: ("head", "held"),
    3: ("chest", "worn"),
    4: ("chest", "held"),
    5: ("legs", "worn"),
    6: ("legs", "held"),
    7: ("feet", "worn"),
    8: ("feet", "held"),
    9: ("main_hand", "worn"),
    10: ("main_hand", "held"),
    11: ("off_hand", "worn"),
    12: ("off_hand", "held"),
    13: ("extra_hand_0", "worn"),
    14: ("extra_hand_0", "held"),
    15: ("extra_hand_1", "worn"),
    16: ("extra_hand_1", "held"),
}


@dataclass(frozen=True)
class ProtoField:
    number: int
    wire_type: int
    value: int | bytes


@dataclass(frozen=True)
class InventoryItemRef:
    instance_id: int
    item_id: str
    location: dict | None = None


def read_varint(data: bytes, pos: int = 0) -> tuple[int, int]:
    value = 0
    shift = 0
    start = pos
    while pos < len(data):
        byte = data[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, pos
        shift += 7
        if shift >= 70:
            raise ValueError(f"protobuf varint too long at offset {start}")
    raise ValueError(f"truncated protobuf varint at offset {start}")


def iter_fields(data: bytes) -> list[ProtoField]:
    fields: list[ProtoField] = []
    pos = 0
    while pos < len(data):
        key, pos = read_varint(data, pos)
        number = key >> 3
        wire_type = key & 0x07
        if number <= 0:
            raise ValueError(f"invalid protobuf field number {number}")

        if wire_type == WIRE_VARINT:
            value, pos = read_varint(data, pos)
        elif wire_type == WIRE_64BIT:
            end = pos + 8
            if end > len(data):
                raise ValueError("truncated protobuf 64-bit field")
            value = data[pos:end]
            pos = end
        elif wire_type == WIRE_LEN:
            size, pos = read_varint(data, pos)
            end = pos + size
            if end > len(data):
                raise ValueError("truncated protobuf length-delimited field")
            value = data[pos:end]
            pos = end
        elif wire_type == WIRE_32BIT:
            end = pos + 4
            if end > len(data):
                raise ValueError("truncated protobuf 32-bit field")
            value = data[pos:end]
            pos = end
        else:
            raise ValueError(f"unsupported protobuf wire type {wire_type}")

        fields.append(ProtoField(number, wire_type, value))
    return fields


def _try_fields(data: bytes) -> list[ProtoField] | None:
    try:
        return iter_fields(data)
    except ValueError:
        return None


def server_data_payload_field(data: bytes) -> int | None:
    fields = _try_fields(data)
    if not fields:
        return None
    return fields[0].number


def server_data_payload_name(data: bytes) -> str | None:
    field = server_data_payload_field(data)
    if field is None:
        return None
    return SERVER_DATA_PAYLOAD_NAMES.get(field, f"field_{field}")


def inventory_item_refs(data: bytes) -> list[InventoryItemRef]:
    """从 ServerDataEnvelope inventory_snapshot 中提取 InventoryItemView 引用。

    非 inventory payload 或畸形字节返回空列表。本解析器只服务 bot 场景铺垫，
    因此有意忽略绝大多数字段。
    """
    envelope = _try_fields(data)
    if not envelope:
        return []
    inventory_fields = [
        field for field in envelope if field.number == 8 and field.wire_type == WIRE_LEN
    ]
    if not inventory_fields:
        return []

    refs: list[InventoryItemRef] = []
    for inventory in inventory_fields:
        assert isinstance(inventory.value, bytes)
        refs.extend(_inventory_snapshot_refs(inventory.value))
    return refs


def _inventory_snapshot_refs(data: bytes) -> list[InventoryItemRef]:
    fields = _try_fields(data)
    if fields is None:
        return []

    refs: list[InventoryItemRef] = []
    hotbar_index = 0
    for field in fields:
        if field.wire_type != WIRE_LEN:
            continue
        assert isinstance(field.value, bytes)
        if field.number == 3:
            ref = _placed_item_ref(field.value)
            if ref is not None:
                refs.append(ref)
        elif field.number == 4:
            refs.extend(_equipped_refs(field.value))
        elif field.number == 5:
            ref = _hotbar_ref(field.value, hotbar_index)
            hotbar_index += 1
            if ref is not None:
                refs.append(ref)
    return refs


def _placed_item_ref(data: bytes) -> InventoryItemRef | None:
    fields = _try_fields(data)
    if fields is None:
        return None
    container_id: str | None = None
    row: int | None = None
    col: int | None = None
    item: InventoryItemRef | None = None
    for field in fields:
        if field.number == 1 and field.wire_type == WIRE_LEN:
            container_id = _decode_utf8(field.value)
        elif field.number == 2 and field.wire_type == WIRE_VARINT:
            assert isinstance(field.value, int)
            row = field.value
        elif field.number == 3 and field.wire_type == WIRE_VARINT:
            assert isinstance(field.value, int)
            col = field.value
        elif field.number == 4 and field.wire_type == WIRE_LEN:
            assert isinstance(field.value, bytes)
            item = _item_view_ref(field.value)
    if item is None:
        return None
    if container_id is None or row is None or col is None:
        return item
    return InventoryItemRef(
        item.instance_id,
        item.item_id,
        {"kind": "container", "container_id": container_id, "row": row, "col": col},
    )


def _hotbar_ref(data: bytes, index: int) -> InventoryItemRef | None:
    fields = _try_fields(data)
    if fields is None:
        return None
    for field in fields:
        if field.number == 1 and field.wire_type == WIRE_LEN:
            assert isinstance(field.value, bytes)
            item = _item_view_ref(field.value)
            if item is not None:
                return InventoryItemRef(
                    item.instance_id, item.item_id, {"kind": "hotbar", "index": index}
                )
    return None


def _equipped_refs(data: bytes) -> list[InventoryItemRef]:
    fields = _try_fields(data)
    if fields is None:
        return []
    refs: list[InventoryItemRef] = []
    for field in fields:
        if field.wire_type != WIRE_LEN or field.number not in EQUIPPED_ITEM_FIELDS:
            continue
        assert isinstance(field.value, bytes)
        item = _item_view_ref(field.value)
        if item is None:
            continue
        slot, state = EQUIPPED_ITEM_FIELDS[field.number]
        refs.append(
            InventoryItemRef(
                item.instance_id,
                item.item_id,
                {"kind": "equip", "slot": slot, "state": state},
            )
        )
    return refs


def _item_view_ref(data: bytes) -> InventoryItemRef | None:
    fields = _try_fields(data)
    if fields is None:
        return None
    instance_id: int | None = None
    item_id: str | None = None
    for field in fields:
        if field.number == 1 and field.wire_type == WIRE_VARINT:
            assert isinstance(field.value, int)
            instance_id = field.value
        elif field.number == 2 and field.wire_type == WIRE_LEN:
            item_id = _decode_utf8(field.value)
    if instance_id is None or item_id is None:
        return None
    return InventoryItemRef(instance_id, item_id)


def _decode_utf8(value: int | bytes) -> str | None:
    if not isinstance(value, bytes):
        return None
    try:
        return value.decode("utf-8")
    except UnicodeDecodeError:
        return None
