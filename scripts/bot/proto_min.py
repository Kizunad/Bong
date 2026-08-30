"""bot smoke 断言用的零依赖 protobuf wire 小工具。

这里刻意不做生成式 protobuf binding。Bot 场景用它做三类检查：
- 识别 `bong:server_data` payload 的 oneof 类型。
- 从 inventory_snapshot 里取 item instance_id，用来驱动真实 client_request intent。
- 解码 bot 场景需要的 production `bong:server_data` payload（含 zone_info 与玩法状态）。

这里仍不追求全 schema binding；只把真实场景使用的观察面按权威 proto 精确 pin 住。
"""

from __future__ import annotations

import struct
import json
from dataclasses import dataclass
from typing import Any

SERVER_DATA_ZONE_INFO_FIELD = 4
ZONE_INFO_ZONE_FIELD = 1
ZONE_INFO_SPIRIT_QI_FIELD = 2
ZONE_INFO_DANGER_LEVEL_FIELD = 3
ZONE_INFO_STATUS_FIELD = 4
ZONE_INFO_ACTIVE_EVENTS_FIELD = 5
ZONE_INFO_PERCEPTION_TEXT_FIELD = 6

SERVER_DATA_PLAYER_STATE_FIELD = 5
PLAYER_STATE_REALM_FIELD = 2
PLAYER_STATE_SPIRIT_QI_FIELD = 3
PLAYER_STATE_SPIRIT_QI_MAX_FIELD = 11

PLAYER_STATE_REALM_NAMES = {
    0: "Unspecified",
    1: "Awaken",
    2: "Induce",
    3: "Condense",
    4: "Solidify",
    5: "Spirit",
    6: "Void",
}

SERVER_DATA_BREAKTHROUGH_CINEMATIC_FIELD = 71
# proto/bong/envelope.proto ServerDataPayload oneof（与 server/src/schema/server_data.rs 对应）
SERVER_DATA_SPARRING_INVITE_FIELD = 64
SERVER_DATA_TRADE_OFFER_FIELD = 65
SERVER_DATA_QUICKSLOT_CONFIG_FIELD = 35

# QuickSlotConfigV1 内部字段（proto/bong/envelope.proto QuickSlotConfig）——
# 命名常量供 decoder 与 wire 契约测试共同引用，避免两端各写一遍魔法数字漂移。
QUICKSLOT_CONFIG_SLOTS_FIELD = 1
QUICKSLOT_CONFIG_COOLDOWN_UNTIL_MS_FIELD = 2
QUICKSLOT_CONFIG_ACK_REQUEST_ID_FIELD = 3
QUICKSLOT_CONFIG_BIND_ACCEPTED_FIELD = 4

# QuickSlotEntryV1 / OptionalQuickSlotEntry 内部字段。
QUICKSLOT_ENTRY_ITEM_ID_FIELD = 1
QUICKSLOT_ENTRY_DISPLAY_NAME_FIELD = 2
QUICKSLOT_ENTRY_CAST_DURATION_MS_FIELD = 3
QUICKSLOT_ENTRY_COOLDOWN_MS_FIELD = 4
QUICKSLOT_ENTRY_ICON_TEXTURE_FIELD = 5
OPTIONAL_QUICKSLOT_ENTRY_ENTRY_FIELD = 1


class ProtoDecodeError(ValueError):
    pass


# Authoritative oneof names live in proto/bong/envelope.proto. This table is the
# Bot harness's supported shallow observation surface; deep decoders below are a
# strict subset keyed by the same field numbers.
SERVER_DATA_PAYLOAD_NAMES = {
    1: "welcome",
    2: "heartbeat",
    3: "narration",
    4: "zone_info",
    5: "player_state",
    6: "cultivation_detail",
    7: "skill_xp_gain",
    8: "inventory_snapshot",
    9: "combat_hud_state",
    32: "wounds_snapshot",
    104: "movement_state",
    11: "alchemy_furnace",
    12: "alchemy_session",
    14: "alchemy_outcome_resolved",
    15: "alchemy_recipe_book",
    17: "forge_station",
    18: "forge_session",
    19: "forge_outcome",
    20: "forge_blueprint_book",
    22: "craft_session_state",
    23: "craft_outcome",
    25: "botany_harvest_progress",
    29: "lumber_progress",
    30: "gathering_session",
    31: "lingtian_session",
    34: "cast_sync",
    35: "quickslot_config",
    36: "skillbar_config",
    37: "techniques_snapshot",
    38: "unlocks_sync",
    39: "derived_attrs_sync",
    43: "treasure_equipped",
    44: "vortex_state",
    45: "dugu_poison_state",
    48: "poison_trait_state",
    49: "carrier_state",
    50: "false_skin_state",
    51: "combat_event",
    54: "skill_config_snapshot",
    64: "sparring_invite",
    65: "trade_offer",
    66: "tribulation_state",
    69: "heart_demon_offer",
    70: "burst_meridian_event",
    71: "breakthrough_cinematic",
    72: "death_screen",
    73: "terminate_screen",
    78: "coffin_state",
    80: "inventory_event",
    81: "dropped_loot_sync",
    90: "container_state",
    97: "skill_scroll_used",
    98: "skill_snapshot",
    119: "loot_container_open",
    120: "loot_container_update",
    121: "loot_container_close",
    128: "sword_bond_hud_state",
    131: "insight_offer",
    137: "inventory_move_rejected",
    49: "carrier_state",
    74: "qi_color_observed",
    76: "spiritual_sense_targets",
    139: "remains_sync",
    77: "event_alert",
    129: "mineral_probe_result",
    130: "freshness_update",
    132: "workbench_open",
    142: "morph_state",
}


def _narration_batch(data: bytes) -> dict[str, Any]:
    """Decode production ``NarrationBatch`` (server_data oneof field 3).

    The realm-gate privacy e2e must assert the actual user-visible narration on
    two protocol clients. Merely identifying oneof field 3 as ``narration`` is
    insufficient because it cannot distinguish the target warning from an
    unrelated narration emitted during the same server run.
    """
    fields = _fields(data)
    narrations = []
    for raw in _messages(fields, 1):
        entry = _fields(raw)
        narration = {
            "text": _string(entry, 1),
            "scope": _string(entry, 2),
            "style": _string(entry, 3),
        }
        if _has(entry, 4):
            narration["target"] = _string(entry, 4)
        if _has(entry, 5):
            narration["kind"] = _string(entry, 5)
        narrations.append(narration)
    return {"v": 1, "type": "narration", "narrations": narrations}


def _zone_info(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "zone_info",
        "zone": _string(fields, ZONE_INFO_ZONE_FIELD),
        "spirit_qi": _double(fields, ZONE_INFO_SPIRIT_QI_FIELD),
        "danger_level": _varint(fields, ZONE_INFO_DANGER_LEVEL_FIELD),
        "status": _string(fields, ZONE_INFO_STATUS_FIELD),
        "active_events": _strings(fields, ZONE_INFO_ACTIVE_EVENTS_FIELD),
        "perception_text": _optional_string(fields, ZONE_INFO_PERCEPTION_TEXT_FIELD),
    }


def _player_state(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    realm = _varint(fields, PLAYER_STATE_REALM_FIELD)
    return {
        "v": 1,
        "type": "player_state",
        "realm": PLAYER_STATE_REALM_NAMES.get(realm, f"unknown_{realm}"),
        "spirit_qi": _double(fields, PLAYER_STATE_SPIRIT_QI_FIELD),
        "spirit_qi_max": _double(fields, PLAYER_STATE_SPIRIT_QI_MAX_FIELD),
    }


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


def _combat_hud_state(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    derived = _message(fields, 4)
    return {
        "v": 1,
        "type": "combat_hud_state",
        "hp_percent": _float32(fields, 1),
        "qi_percent": _float32(fields, 2),
        "stamina_percent": _float32(fields, 3),
        "combat_active": bool(_varint(fields, 5)),
        "derived": {
            "flying": bool(_varint(derived, 1)),
            "phasing": bool(_varint(derived, 2)),
            "tribulation_locked": bool(_varint(derived, 3)),
        },
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


def _dropped_loot_sync(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    drops = []
    for raw in _messages(fields, 1):
        entry = _fields(raw)
        drops.append(
            {
                "instance_id": _varint(entry, 1),
                "source_container_id": _string(entry, 2),
                "source_row": _varint(entry, 3),
                "source_col": _varint(entry, 4),
                "world_pos": [
                    _double(entry, 5),
                    _double(entry, 6),
                    _double(entry, 7),
                ],
                "item": _item_view(_message(entry, 8)),
            }
        )
    return {"v": 1, "type": "dropped_loot_sync", "drops": drops}


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


def _loot_container_update(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "loot_container_update",
        "session_id": _varint(fields, 1),
        "placed_items": [_placed_inventory_item(raw) for raw in _messages(fields, 2)],
    }


def _loot_container_close(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "loot_container_close",
        "session_id": _varint(fields, 1),
        "reason": _string(fields, 2),
    }


def _morph_state_entry(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "entity_id": _varint(fields, 1),
        "model_kind": _varint(fields, 2),
        "form_race_id": _string(fields, 3),
        "form_body_plan_id": _string(fields, 4),
        "active": bool(_varint(fields, 5)),
    }


def _morph_state(data: bytes) -> dict[str, Any]:
    """plan-race-system-v1 P4 —— 易形状态快照（field 142）。`mode` "full" 或
    "delta"；`entries[].active=false` 表示该 entity 应从本地缓存删除（解除易形）。
    """
    fields = _fields(data)
    return {
        "v": _varint(fields, 1, default=1),
        "type": "morph_state",
        "mode": _string(fields, 2),
        "entries": [_morph_state_entry(_fields(raw)) for raw in _messages(fields, 3)],
    }


def _coffin_state(data: bytes) -> dict[str, Any]:
    """plan-coffin-v1 —— 延寿棺状态（field 78）。enter 推 grade=Some、
    multiplier<1.0；leave 推 grade 缺席、multiplier=1.0。"""
    fields = _fields(data)
    return {
        "v": 1,
        "type": "coffin_state",
        "in_coffin": bool(_varint(fields, 1)),
        "lifespan_rate_multiplier": _double(fields, 2, default=1.0),
        "coffin_grade": _optional_string(fields, 3),
    }


def _container_state(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "container_state",
        "entity_id": _varint(fields, 1),
        "visual_entity_id": _optional_varint(fields, 10),
    }


# proto/bong/envelope.proto:1890-1893 reserves zero for UNSPECIFIED.
# Keep the generated protobuf enum values here; do not compress away the
# unspecified slot because live CarrierState payloads use IDLE=1,
# CHARGING=2, and CHARGED=3.
CARRIER_CHARGE_PHASE_NAMES = {
    0: "unspecified",
    1: "idle",
    2: "charging",
    3: "charged",
}


def _carrier_state(data: bytes) -> dict[str, Any]:
    """CarrierStateV1（server_data oneof field 49，envelope.proto:1896）。

    carrier 是持有者的线缆 wire id（`player:{uuid}`），server 每 tick 周期向
    各客户端推送自身快照；bot 场景用它与 redis 事件流上的 carrier 字段互证，
    把 bong:anqi/container_swap 事件归属到本 bot。
    """
    fields = _fields(data)
    return {
        "v": 1,
        "type": "carrier_state",
        "carrier": _string(fields, 1),
        "phase": CARRIER_CHARGE_PHASE_NAMES.get(_varint(fields, 2), "unspecified"),
        "progress": _float32(fields, 3),
        "sealed_qi": _float32(fields, 4),
        "sealed_qi_initial": _float32(fields, 5),
        "half_life_remaining_ticks": _varint(fields, 6),
        "item_instance_id": _optional_varint(fields, 7),
    }


def _death_screen(data: bytes) -> dict[str, Any]:
    """死亡屏（field 72，envelope.proto `DeathScreen`）。

    濒死判定出决策（Fortune/Tribulation）后 server 推送，visible=true；复活/终结
    后 visible=false 收屏。stage/zone_kind 为 proto 枚举值：stage 1=Fortune、
    2=Tribulation；zone_kind 1=ordinary、2=death、3=negative。
    """
    fields = _fields(data)
    out: dict[str, Any] = {
        "v": 1,
        "type": "death_screen",
        "visible": bool(_varint(fields, 1)),
        "cause": _string(fields, 2),
        "luck_remaining": _double(fields, 3),
        "final_words": _strings(fields, 4),
        "countdown_until_ms": _varint(fields, 5),
        "can_reincarnate": bool(_varint(fields, 6)),
        "can_terminate": bool(_varint(fields, 7)),
    }
    if _has(fields, 8):
        out["stage"] = _varint(fields, 8)
    if _has(fields, 9):
        out["death_number"] = _varint(fields, 9)
    if _has(fields, 10):
        out["zone_kind"] = _varint(fields, 10)
    return out


def _terminate_screen(data: bytes) -> dict[str, Any]:
    """终结屏（field 73，envelope.proto `TerminateScreen`）。

    终结（主动归隐或劫数失败）后推送，visible=true；新建角色/复生后收屏
    visible=false。
    """
    fields = _fields(data)
    return {
        "v": 1,
        "type": "terminate_screen",
        "visible": bool(_varint(fields, 1)),
        "final_words": _string(fields, 2),
        "epilogue": _string(fields, 3),
        "archetype_suggestion": _string(fields, 4),
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
        "mineral_id": _optional_string(fields, 12),
        "scroll_kind": _optional_string(fields, 13),
        "scroll_skill_id": _optional_string(fields, 14),
        "scroll_xp_grant": _optional_varint(fields, 15),
        "charges": _optional_varint(fields, 16),
        "forge_quality": _optional_float32(fields, 17),
        "forge_color": _optional_varint(fields, 18),
        "forge_side_effects": _strings(fields, 19),
        "forge_achieved_tier": _optional_varint(fields, 20),
        "alchemy": (
            _alchemy_item_data(_message(fields, 21)) if _has(fields, 21) else None
        ),
        "freshness": _inventory_freshness(_message(fields, 22)) if _has(fields, 22) else None,
    }


def _inventory_freshness(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "created_at_tick": _varint(fields, 1),
        "initial_qi": _float32(fields, 2),
        "track": _string(fields, 3),
        "profile": _string(fields, 4),
        "frozen_accumulated": _varint(fields, 5),
        "frozen_since_tick": _optional_varint(fields, 6),
    }


def _alchemy_item_data(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "kind": _string(fields, 1),
        "recipe_id": _optional_string(fields, 2),
        "quality_tier": _optional_varint(fields, 3),
        "effect_multiplier": _optional_double(fields, 4),
        "consecrated": (
            bool(_optional_varint(fields, 5)) if _has(fields, 5) else None
        ),
        "side_effect": (
            _alchemy_side_effect(_message(fields, 6)) if _has(fields, 6) else None
        ),
        "fragment": (
            _alchemy_fragment(_message(fields, 7)) if _has(fields, 7) else None
        ),
        "hint": _alchemy_hint(_message(fields, 8)) if _has(fields, 8) else None,
        "residue_kind": _optional_string(fields, 9),
        "produced_at_tick": _optional_varint(fields, 10),
        "expires_at_tick": _optional_varint(fields, 11),
    }


def _alchemy_side_effect(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "tag": _string(fields, 1),
        "duration_s": _optional_varint(fields, 2),
        "weight": _optional_varint(fields, 3),
        "perm": bool(_optional_varint(fields, 4)) if _has(fields, 4) else None,
        "color": _optional_varint(fields, 5),
        "amount": _optional_double(fields, 6),
    }


def _alchemy_fragment(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "recipe_id": _string(fields, 1),
        "known_stages": _uint32s(fields, 2),
        "max_quality_tier": _varint(fields, 3),
    }


def _alchemy_hint(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "source_pill": _string(fields, 1),
        "recipe_id": _optional_string(fields, 2),
        "accuracy": _double(fields, 3),
        "ingredients": _strings(fields, 4),
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
    for existing, wire, value in reversed(fields):
        if existing == field and wire == WIRE_VARINT:
            return int(value)
    return None


def _string(fields: list[tuple[int, int, Any]], field: int, default: str = "") -> str:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 2:
            return value.decode("utf-8", errors="replace")
    return default


def _optional_string(fields: list[tuple[int, int, Any]], field: int) -> str | None:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 2:
            return value.decode("utf-8", errors="replace")
    return None


def _strings(fields: list[tuple[int, int, Any]], field: int) -> list[str]:
    return [
        value.decode("utf-8", errors="replace")
        for existing, wire, value in fields
        if existing == field and wire == 2
    ]


def _uint32s(fields: list[tuple[int, int, Any]], field: int) -> list[int]:
    """Decode repeated uint32 in either packed or unpacked protobuf form."""
    values: list[int] = []
    for existing, wire, value in fields:
        if existing != field:
            continue
        if wire == WIRE_VARINT:
            values.append(int(value))
            continue
        if wire != WIRE_LEN or not isinstance(value, bytes):
            continue
        pos = 0
        while pos < len(value):
            decoded, pos = _read_varint(value, pos)
            values.append(decoded)
    return values


def _double(fields: list[tuple[int, int, Any]], field: int, default: float = 0.0) -> float:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 1:
            return struct.unpack("<d", value)[0]
    return default


def _optional_double(fields: list[tuple[int, int, Any]], field: int) -> float | None:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 1:
            return struct.unpack("<d", value)[0]
    return None


def _message(fields: list[tuple[int, int, Any]], field: int) -> list[tuple[int, int, Any]]:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 2:
            return _fields(value)
    return []


def _messages(fields: list[tuple[int, int, Any]], field: int) -> list[bytes]:
    """重复 message/string 字段：只收 wire type 2（length-delimited）。

    review finding minor：此前 `isinstance(value, bytes)` 会把 fixed64(wire 1)/fixed32
    (wire 5) 也当成消息字节——TribulationState.participants 会把误编码的 fixed64 field 15
    解成参与者文本；HeartDemonOffer.choices 则把 fixed64 field 9 喂给嵌套解析器抛
    ProtoDecodeError、丢掉 offer 观测。与单值 `_message`（显式要求 wire==2）对齐。
    """
    return [value for existing, wire, value in fields if existing == field and wire == 2]


def _float32(fields: list[tuple[int, int, Any]], field: int, default: float = 0.0) -> float:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 5:
            return struct.unpack("<f", value)[0]
    return default


def _optional_float32(fields: list[tuple[int, int, Any]], field: int) -> float | None:
    for existing, wire, value in reversed(fields):
        if existing == field and wire == 5:
            return struct.unpack("<f", value)[0]
    return None


# ── 生产 / 消费玩法 payload（envelope.proto oneof tag 见 proto/bong/envelope.proto）──


def _botany_harvest_progress(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "botany_harvest_progress",
        "session_id": _string(fields, 1),
        "target_id": _string(fields, 2),
        "target_name": _string(fields, 3),
        "plant_kind": _string(fields, 4),
        "mode": _string(fields, 5),
        "progress": _double(fields, 6),
        "auto_selectable": bool(_varint(fields, 7)),
        "request_pending": bool(_varint(fields, 8)),
        "interrupted": bool(_varint(fields, 9)),
        "completed": bool(_varint(fields, 10)),
        "detail": _string(fields, 11),
        "hazard_hints": _strings(fields, 12),
        "target_pos": [
            _optional_double(fields, 13),
            _optional_double(fields, 14),
            _optional_double(fields, 15),
        ],
    }


def _enum_name(names: dict[int, str], value: int) -> str:
    return names.get(value, f"unknown_{value}")


GATHERING_TARGET_TYPE_NAMES = {
    0: "unspecified",
    1: "herb",
    2: "ore",
    3: "wood",
}

GATHERING_QUALITY_HINT_NAMES = {
    0: "unspecified",
    1: "normal",
    2: "fine_likely",
    3: "perfect_possible",
    4: "fine",
    5: "perfect",
}


def _gathering_session(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "gathering_session",
        "session_id": _string(fields, 1),
        "progress_ticks": _varint(fields, 2),
        "total_ticks": _varint(fields, 3),
        "target_name": _string(fields, 4),
        "target_type": _enum_name(GATHERING_TARGET_TYPE_NAMES, _varint(fields, 5)),
        "quality_hint": _enum_name(GATHERING_QUALITY_HINT_NAMES, _varint(fields, 6)),
        "tool_used": _optional_string(fields, 7),
        "interrupted": bool(_varint(fields, 8)),
        "completed": bool(_varint(fields, 9)),
    }


LINGTIAN_SESSION_KIND_NAMES = {
    0: "unspecified",
    1: "till",
    2: "renew",
    3: "planting",
    4: "harvest",
    5: "replenish",
    6: "drain_qi",
}


def _lingtian_session(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "lingtian_session",
        "active": bool(_varint(fields, 1)),
        "kind": _enum_name(LINGTIAN_SESSION_KIND_NAMES, _varint(fields, 2)),
        "pos": [_int32(fields, 3), _int32(fields, 4), _int32(fields, 5)],
        "elapsed_ticks": _varint(fields, 6),
        "target_ticks": _varint(fields, 7),
        "plant_id": _optional_string(fields, 8),
        "source": _optional_string(fields, 9),
        "dye_contamination": _optional_float32(fields, 10),
        "dye_contamination_warning": bool(_varint(fields, 11)),
    }


def _lumber_progress(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "lumber_progress",
        "session_id": _string(fields, 1),
        "log_pos": [_int32(fields, 2), _int32(fields, 3), _int32(fields, 4)],
        "progress": _double(fields, 5),
        "interrupted": bool(_varint(fields, 6)),
        "completed": bool(_varint(fields, 7)),
        "detail": _string(fields, 8),
    }

CAST_OUTCOME_NAMES = {
    0: "unspecified",
    1: "none",
    2: "completed",
    3: "interrupt_movement",
    4: "interrupt_contam",
    5: "interrupt_control",
    6: "user_cancel",
    7: "death",
    8: "meridian_gated",
    9: "reject_qi_insufficient",
    10: "reject_on_cooldown",
    11: "reject_invalid_target",
    12: "reject_in_recovery",
    13: "reject_realm_too_low",
    14: "reject_no_weapon",
    15: "reject_technique_inactive",
    16: "reject_race_mismatch",
}

CAST_PHASE_NAMES = {0: "unspecified", 1: "idle", 2: "casting", 3: "complete", 4: "interrupt"}

SKILL_ID_NAMES = {
    0: "unspecified",
    1: "herbalism",
    2: "alchemy",
    3: "forging",
    4: "combat",
    5: "mineral",
    6: "cultivation",
}


def _skill_id_name(value: int) -> str:
    return SKILL_ID_NAMES.get(value, f"unknown_{value}")


def _skill_xp_gain(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    if _has(fields, 5):
        scroll = _message(fields, 5)
        source = {
            "kind": "scroll",
            "scroll_id": _string(scroll, 1),
            "xp_grant": _varint(scroll, 2),
        }
    elif _has(fields, 7):
        mentor = _message(fields, 7)
        source = {"kind": "mentor", "mentor_char": _varint(mentor, 1)}
    elif _has(fields, 4):
        action = _message(fields, 4)
        source = {"kind": "action", "plan_id": _string(action, 1), "action": _string(action, 2)}
    elif _has(fields, 6):
        source = {"kind": "realm_breakthrough"}
    else:
        source = None
    return {
        "v": 1,
        "type": "skill_xp_gain",
        "char_id": _varint(fields, 1),
        "skill": _skill_id_name(_varint(fields, 2)),
        "amount": _varint(fields, 3),
        "source": source,
    }


def _quick_slot_entry(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "item_id": _string(fields, 1),
        "display_name": _string(fields, 2),
        "cast_duration_ms": _varint(fields, 3),
        "cooldown_ms": _varint(fields, 4),
        "icon_texture": _string(fields, 5),
    }


def _optional_quick_slot_entry(data: bytes) -> dict[str, Any] | None:
    if not data:
        return None
    fields = _fields(data)
    # `data` 是 repeated `OptionalQuickSlotEntry` 的一个元素（schema bong.rs）：
    #   OptionalQuickSlotEntry { entry(1): Option<QuickSlotEntry> }
    # 而 QuickSlotEntry { item_id(1), display_name(2), cast_duration_ms(3),
    # cooldown_ms(4), icon_texture(5) }。proto3 repeated 不支持 optional element，
    # 所以服务器用 wrapper 包一层——field 1 是**又一层嵌套 message**，必须先
    # `_message(fields, 1)` 解出 QuickSlotEntry 再交给 `_quick_slot_entry`。
    # central-review 2012 #3 的证据称调用方已传入 unwrapped 载荷、field 1 即
    # item_id——与 prost 生成的 OptionalQuickSlotEntry 包装矛盾；此处以 schema 为
    # 准，test_proto_quick_slot_config_payload_decodes 的 bound round-trip 断言
    # 钉死该解码（跳过这层会把 QuickSlotEntry 原始字节误当 item_id）。
    if not _has(fields, 1):
        return None
    return _quick_slot_entry(_message(fields, 1))


def _packed_varints(data: bytes) -> list[int]:
    """解码 proto3 packed repeated 标量：length-delimited blob 内连续 varint。"""
    pos = 0
    values = []
    while pos < len(data):
        value, pos = _read_varint(data, pos)
        values.append(int(value))
    return values


def _quick_slot_config(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    # review finding [5]：`repeated uint64 cooldown_until_ms` 在 proto3 里默认
    # **packed**（wire type 2：length-delimited blob 内连续 varint）。旧实现只读
    # 独立 wire-0 varint（w==0），真实服务器生产的 packed 载荷被解码成空列表。
    # 两种编码都收：packed 逐 blob 展开，unpacked 逐 varint 追加。
    cooldowns: list[int] = []
    for f, w, v in fields:
        if f != 2:
            continue
        if w == 0:
            cooldowns.append(int(v))
        elif w == 2:
            cooldowns.extend(_packed_varints(v))
    return {
        "v": 1,
        "type": "quickslot_config",
        "slots": [_optional_quick_slot_entry(raw) for raw in _messages(fields, 1)],
        "cooldown_until_ms": cooldowns,
        "ack_request_id": _optional_string(fields, 3),
        "bind_accepted": (
            bool(_varint(fields, 4)) if _has(fields, 4) else None
        ),
    }


def _technique_required_meridian(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {"channel": _string(fields, 1), "min_health": _float32(fields, 2)}


def _technique_entry(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "id": _string(fields, 1),
        "display_name": _string(fields, 2),
        "grade": _string(fields, 3),
        "proficiency": _float32(fields, 4),
        "proficiency_label": _string(fields, 5),
        "active": bool(_varint(fields, 6)),
        "description": _string(fields, 7),
        "required_realm": _string(fields, 8),
        "required_meridians": [
            _technique_required_meridian(raw) for raw in _messages(fields, 9)
        ],
        "qi_cost": _float32(fields, 10),
        "stamina_cost": _float32(fields, 11),
        "cast_ticks": _varint(fields, 12),
        "cooldown_ticks": _varint(fields, 13),
        "range": _float32(fields, 14),
    }


def _techniques_snapshot(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "techniques_snapshot",
        "entries": [_technique_entry(raw) for raw in _messages(fields, 1)],
    }


def _skill_config_entry(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {"skill_id": _string(fields, 1), "json_config": _string(fields, 2)}


def _skill_config_snapshot(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "skill_config_snapshot",
        "configs": [_skill_config_entry(raw) for raw in _messages(fields, 1)],
    }


def _skill_scroll_used(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "skill_scroll_used",
        "char_id": _varint(fields, 1),
        "scroll_id": _string(fields, 2),
        "skill": _skill_id_name(_varint(fields, 3)),
        "xp_granted": _varint(fields, 4),
        "was_duplicate": bool(_varint(fields, 5)),
    }


def _skill_entry_snapshot(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "lv": _varint(fields, 1),
        "xp": _varint(fields, 2),
        "xp_to_next": _varint(fields, 3),
        "total_xp": _varint(fields, 4),
        "cap": _varint(fields, 5),
        "recent_gain_xp": _varint(fields, 6),
    }


def _skill_snapshot_entry(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "skill_name": _string(fields, 1),
        "entry": _skill_entry_snapshot(_message(fields, 2)),
    }


def _skill_snapshot(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "skill_snapshot",
        "char_id": _varint(fields, 1),
        "skills": [_skill_snapshot_entry(raw) for raw in _messages(fields, 2)],
        "consumed_scrolls": _strings(fields, 3),
    }


def _craft_session_state(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "craft_session_state",
        "active": bool(_varint(fields, 3)),
        "recipe_id": _string(fields, 4),
        "elapsed_ticks": _varint(fields, 5),
        "total_ticks": _varint(fields, 6),
        "completed_count": _varint(fields, 7),
        "total_count": _varint(fields, 8),
    }


def _craft_outcome(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    for field, wire, value in fields:
        if wire != 2:
            continue
        inner = _fields(value)
        if field == 1:
            return {
                "v": 1,
                "type": "craft_outcome",
                "outcome": "completed",
                "recipe_id": _string(inner, 3),
                "output_template": _string(inner, 4),
                "output_count": _varint(inner, 5),
            }
        if field == 2:
            return {
                "v": 1,
                "type": "craft_outcome",
                "outcome": "failed",
                "recipe_id": _string(inner, 3),
                "reason": _varint(inner, 4),
                "material_returned": _varint(inner, 5),
            }
    return {"v": 1, "type": "craft_outcome", "outcome": "unknown"}


def _alchemy_furnace(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "alchemy_furnace",
        "pos": [
            _optional_varint(fields, 1),
            _optional_varint(fields, 2),
            _optional_varint(fields, 3),
        ],
        "tier": _varint(fields, 4),
        "integrity": _double(fields, 5),
        "integrity_max": _double(fields, 6),
        "owner_name": _string(fields, 7),
        "has_session": bool(_varint(fields, 8)),
    }


def _alchemy_stage_hint(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "at_tick": _varint(fields, 1),
        "window": _varint(fields, 2),
        "summary": _string(fields, 3),
        "completed": bool(_varint(fields, 4)),
        "missed": bool(_varint(fields, 5)),
    }


def _alchemy_session(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "alchemy_session",
        "recipe_id": _string(fields, 1),
        "active": bool(_varint(fields, 2)),
        "elapsed_ticks": _varint(fields, 3),
        "target_ticks": _varint(fields, 4),
        "temp_current": _double(fields, 5),
        "temp_target": _double(fields, 6),
        "temp_band": _double(fields, 7),
        "qi_injected": _double(fields, 8),
        "qi_target": _double(fields, 9),
        "status_label": _string(fields, 10),
        "stages": [_alchemy_stage_hint(raw) for raw in _messages(fields, 11)],
        "interventions_recent": [
            raw.decode("utf-8", errors="replace") for raw in _messages(fields, 12)
        ],
    }


ALCHEMY_OUTCOME_BUCKET_NAMES = {
    0: "unspecified",
    1: "perfect",
    2: "good",
    3: "flawed",
    4: "waste",
    5: "explode",
}

COLOR_KIND_NAMES = {
    0: "unspecified",
    1: "sharp",
    2: "heavy",
    3: "mellow",
    4: "solid",
    5: "light",
    6: "intricate",
    7: "gentle",
    8: "insidious",
    9: "violent",
    10: "turbid",
}


def _alchemy_outcome_resolved(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    toxin_color = _optional_varint(fields, 6)
    return {
        "v": 1,
        "type": "alchemy_outcome_resolved",
        "bucket": _enum_name(ALCHEMY_OUTCOME_BUCKET_NAMES, _varint(fields, 1)),
        "recipe_id": _optional_string(fields, 2),
        "pill": _optional_string(fields, 3),
        "quality": _optional_double(fields, 4),
        "toxin_amount": _optional_double(fields, 5),
        "toxin_color": (
            _enum_name(COLOR_KIND_NAMES, toxin_color)
            if toxin_color is not None
            else None
        ),
        "qi_gain": _optional_double(fields, 7),
        "side_effect_tag": _optional_string(fields, 8),
        "flawed_path": bool(_varint(fields, 9)),
        "damage": _optional_double(fields, 10),
        "meridian_crack": _optional_double(fields, 11),
    }


def _inventory_move_rejected(data: bytes) -> dict[str, Any]:
    """plan-race-system-v1 P3b —— `InventoryMoveRejected`（field 137）解码。此前该
    payload_type 未接入本最小解码器（field 137 不在 dispatch 白名单里），任何 bot
    场景想断言 `inventory_move_rejected`（含新增的 race_mismatch 拒绝原因）都收不到
    解码结果，`expect_server_data("inventory_move_rejected", ...)` 会静默超时。
    `reason` 恒有值（proto 非 optional）；`required_realm`/`slot`/`cap` 仅对应拒绝
    原因才携带，缺省时保持 None（不伪造占位值）。
    """
    fields = _fields(data)
    return {
        "v": 1,
        "type": "inventory_move_rejected",
        "reason": _string(fields, 1),
        "required_realm": _string(fields, 2) if _has(fields, 2) else None,
        "slot": _string(fields, 3) if _has(fields, 3) else None,
        "cap": _optional_varint(fields, 4),
    }


COLOR_KIND_PASCAL_NAMES = {
    0: "unspecified",
    1: "Sharp",
    2: "Heavy",
    3: "Mellow",
    4: "Solid",
    5: "Light",
    6: "Intricate",
    7: "Gentle",
    8: "Insidious",
    9: "Violent",
    10: "Turbid",
}

EVENT_KIND_NAMES = {
    0: "unspecified",
    1: "thunder_tribulation",
    2: "beast_tide",
    3: "realm_collapse",
    4: "karma_backlash",
    5: "poison_miasma",
    6: "meridian_seal",
    7: "daoxiang_wave",
    8: "heavenly_fire",
    9: "pressure_invert",
    10: "all_wither",
    11: "generic",
}


def _sint32(fields: list[tuple[int, int, Any]], field: int, default: int = 0) -> int:
    """protobuf `sint32`（zigzag varint）。WorkbenchOpen 的坐标可为负。"""
    if not _has(fields, field):
        return default
    raw = _varint(fields, field)
    return (raw >> 1) ^ -(raw & 1)


def _qi_color_observed(data: bytes) -> dict[str, Any]:
    """plan-exploration-probe-return-v1 —— 神识观色 S2C（field 74）。

    与 Rust ServerDataPayloadV1::QiColorObserved 精确对应；`main`/`secondary`
    是 ColorKind 枚举（varint），映射为 PascalCase 名（与 Rust 变体一致）。
    `secondary` 只在该字段**实际携带**时才进 dict——脱敏路径（diff=1）服务端省略
    field 4，键即缺失（presence 契约，区分「省略」与「显式 null」；central-review
    31437496353 #3 要求场景断言键缺失而非 `dict.get is None`）。ColorKind=0 映射
    为 `unspecified`；未知非零 wire 值保留为 `unknown_N`，避免与合法默认值混淆。
    """
    fields = _fields(data)
    main = _optional_varint(fields, 3)
    decoded = {
        "v": 1,
        "type": "qi_color_observed",
        "observer": _string(fields, 1),
        "observed": _string(fields, 2),
        "main": _enum_name(COLOR_KIND_PASCAL_NAMES, main if main is not None else 0),
        "is_chaotic": bool(_varint(fields, 5)),
        "is_hunyuan": bool(_varint(fields, 6)),
        "realm_diff": _int32(fields, 7),
    }
    secondary = _optional_varint(fields, 4)
    if secondary is not None:
        decoded["secondary"] = _enum_name(COLOR_KIND_PASCAL_NAMES, secondary)
    return decoded


def _event_alert(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    event = _optional_varint(fields, 1)
    return {
        "v": 1,
        "type": "event_alert",
        "event": _enum_name(EVENT_KIND_NAMES, event if event is not None else 0),
        "message": _string(fields, 2),
        "zone": _optional_string(fields, 3),
        "duration_ticks": _optional_varint(fields, 4),
    }


def _mineral_probe_result(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "mineral_probe_result",
        "kind": _string(fields, 1),
        "mineral_id": _optional_string(fields, 2),
        "remaining_units": _optional_varint(fields, 3),
        "display_name_zh": _optional_string(fields, 4),
        "denial_reason": _optional_string(fields, 5),
    }


def _freshness_update(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "freshness_update",
        "item_uuid": _string(fields, 1),
        "freshness": _float32(fields, 2),
        "profile_name": _string(fields, 3),
    }


def _workbench_open(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "workbench_open",
        "entity_id": _varint(fields, 1),
        "position": [
            _sint32(fields, 2),
            _sint32(fields, 3),
            _sint32(fields, 4),
        ],
    }


def _cast_sync(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "cast_sync",
        "phase": CAST_PHASE_NAMES.get(_varint(fields, 1), "unspecified"),
        "slot": _varint(fields, 2),
        "duration_ms": _varint(fields, 3),
        "outcome": CAST_OUTCOME_NAMES.get(_varint(fields, 5), "unspecified"),
    }


def _skill_bar_config(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "skillbar_config",
        "slots": [_optional_skill_bar_entry(raw) for raw in _messages(fields, 1)],
        "cooldown_until_ms": _repeated_uint64(fields, 2),
    }


def _optional_skill_bar_entry(data: bytes) -> dict[str, Any] | None:
    # OptionalSkillBarEntry.entry → SkillBarEntry → item/skill.
    fields = _fields(data)
    entries = _messages(fields, 1)
    if not entries:
        return None
    entry = _fields(entries[-1])
    items = _messages(entry, 1)
    if items:
        item = _fields(items[-1])
        return {
            "kind": "item",
            "template_id": _string(item, 1),
            "display_name": _string(item, 2),
            "cast_duration_ms": _varint(item, 3),
            "cooldown_ms": _varint(item, 4),
            "icon_texture": _string(item, 5),
        }
    skills = _messages(entry, 2)
    if skills:
        skill = _fields(skills[-1])
        return {
            "kind": "skill",
            "skill_id": _string(skill, 1),
            "display_name": _string(skill, 2),
            "cast_duration_ms": _varint(skill, 3),
            "cooldown_ms": _varint(skill, 4),
            "icon_texture": _string(skill, 5),
        }
    return None


def _repeated_uint64(fields: list[tuple[int, int, Any]], field: int) -> list[int]:
    values: list[int] = []
    for number, wire, value in fields:
        if number != field:
            continue
        if wire == 0:
            values.append(int(value))
            continue
        if wire == 2:
            pos = 0
            while pos < len(value):
                decoded, pos = _read_varint(value, pos)
                values.append(decoded)
    return values


def _quick_slot_config(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "quickslot_config",
        "slots": [
            _quick_slot_entry(raw) for raw in _messages(fields, QUICKSLOT_CONFIG_SLOTS_FIELD)
        ],
        "cooldown_until_ms": _repeated_varints(fields, QUICKSLOT_CONFIG_COOLDOWN_UNTIL_MS_FIELD),
        "ack_request_id": _optional_string(fields, QUICKSLOT_CONFIG_ACK_REQUEST_ID_FIELD),
        "bind_accepted": (
            bool(_varint(fields, QUICKSLOT_CONFIG_BIND_ACCEPTED_FIELD))
            if _has(fields, QUICKSLOT_CONFIG_BIND_ACCEPTED_FIELD)
            else None
        ),
    }


def _quick_slot_entry(data: bytes) -> dict[str, Any] | None:
    fields = _fields(data)
    if not _has(fields, OPTIONAL_QUICKSLOT_ENTRY_ENTRY_FIELD):
        return None
    entry = _message(fields, OPTIONAL_QUICKSLOT_ENTRY_ENTRY_FIELD)
    return {
        "item_id": _string(entry, QUICKSLOT_ENTRY_ITEM_ID_FIELD),
        "display_name": _string(entry, QUICKSLOT_ENTRY_DISPLAY_NAME_FIELD),
        "cast_duration_ms": _varint(entry, QUICKSLOT_ENTRY_CAST_DURATION_MS_FIELD),
        "cooldown_ms": _varint(entry, QUICKSLOT_ENTRY_COOLDOWN_MS_FIELD),
        "icon_texture": _string(entry, QUICKSLOT_ENTRY_ICON_TEXTURE_FIELD),
    }


def _repeated_varints(fields: list[tuple[int, int, Any]], field: int) -> list[int]:
    values: list[int] = []
    for existing, wire, value in fields:
        if existing != field:
            continue
        if wire == 0:
            values.append(int(value))
        elif wire == 2:
            pos = 0
            while pos < len(value):
                v, pos = _read_varint(value, pos)
                values.append(v)
    return values


def _combat_event_floater(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    events = []
    for raw in _messages(fields, 1):
        entry = _fields(raw)
        events.append(
            {
                "kind": _string(entry, 1),
                "amount": _float32(entry, 2),
                "text": _string(entry, 3),
                "outgoing": bool(_varint(entry, 7)),
            }
        )
    return {"v": 1, "type": "combat_event", "events": events}


def _false_skin_state(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "false_skin_state",
        "target_id": _string(fields, 1),
        "kind": _optional_varint(fields, 2),
        "layers_remaining": _varint(fields, 3),
        "contam_capacity_per_layer": _double(fields, 4),
        "absorbed_contam": _double(fields, 5),
        "equipped_at_tick": _varint(fields, 6),
    }


def _treasure_equipped(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    treasure = _message(fields, 2)
    return {
        "v": 1,
        "type": "treasure_equipped",
        "slot": _string(fields, 1),
        "treasure": {
            "instance_id": _varint(treasure, 1),
            "template_id": _string(treasure, 2),
            "display_name": _string(treasure, 3),
        }
        if treasure
        else None,
    }


def _burst_meridian_event(data: bytes) -> dict[str, Any]:
    """Decode the single BurstMeridianEvent payload (envelope field 70)."""
    fields = _fields(data)
    decoded = {
        "v": 1,
        "type": "burst_meridian_event",
        "skill": _string(fields, 1),
        "caster": _string(fields, 2),
        "tick": _varint(fields, 4),
        "overload_ratio": _double(fields, 5),
        "integrity_snapshot": _double(fields, 6),
    }
    target = _optional_string(fields, 3)
    if target is not None:
        decoded["target"] = target
    return decoded


def _breakthrough_cinematic(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "breakthrough_cinematic",
        "actor_id": _string(fields, 1),
        "phase": _string(fields, 2),
        "phase_tick": _varint(fields, 3),
        "phase_duration_ticks": _varint(fields, 4),
        "realm_from": _string(fields, 5),
        "realm_to": _string(fields, 6),
        "result": _string(fields, 7),
        "interrupted": bool(_varint(fields, 8)),
        "world_pos": [
            _double(fields, 9),
            _double(fields, 10),
            _double(fields, 11),
        ],
        "visible_radius_blocks": _double(fields, 12),
        "global": bool(_varint(fields, 13)),
        "distant_billboard": bool(_varint(fields, 14)),
        "particle_density": _float32(fields, 15),
        "intensity": _float32(fields, 16),
        "season_overlay": _string(fields, 17),
        "style": _string(fields, 18),
        "at_tick": _varint(fields, 19),
    }


FORGE_STEP_NAMES = {
    0: "unspecified",
    1: "billet",
    2: "tempering",
    3: "inscription",
    4: "consecration",
    5: "done",
}

FORGE_OUTCOME_BUCKET_NAMES = {
    0: "unspecified",
    1: "perfect",
    2: "good",
    3: "flawed",
    4: "waste",
    5: "explode",
}


def _int32(fields: list[tuple[int, int, Any]], field: int, default: int = 0) -> int:
    """protobuf `int32` 字段：负值在 wire 上按 64-bit 补码编码（并非 32-bit 掩码——
    这与本文件里给实际 MC 协议 varint 用的 `mc.write_varint` 32-bit 掩码不同）。
    读回后只取低 32 位再按补码转回带符号整数，供 station_pos_x/y/z 使用。"""
    if not _has(fields, field):
        return default
    raw = _varint(fields, field) & 0xFFFFFFFF
    if raw & 0x80000000:
        raw -= 0x100000000
    return raw


def _forge_station(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "forge_station",
        "station_id": _string(fields, 1),
        "tier": _varint(fields, 2),
        "integrity": _float32(fields, 3),
        "owner_name": _string(fields, 4),
        "has_session": bool(_varint(fields, 5)),
        "pos": [_int32(fields, 6), _int32(fields, 7), _int32(fields, 8)],
    }


def _forge_step_state(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    if _has(fields, 2):
        tempering = _message(fields, 2)
        return {
            "kind": "tempering",
            "beat_cursor": _varint(tempering, 2),
            "hits": _varint(tempering, 3),
            "misses": _varint(tempering, 4),
            "deviation": _varint(tempering, 5),
            "qi_spent": _double(tempering, 6),
        }
    if _has(fields, 1):
        billet = _message(fields, 1)
        return {
            "kind": "billet",
            "resolved_tier_cap": _varint(billet, 3),
        }
    if _has(fields, 3):
        inscription = _message(fields, 3)
        return {
            "kind": "inscription",
            "filled_slots": _varint(inscription, 1),
            "max_slots": _varint(inscription, 2),
            "failed": bool(_varint(inscription, 3)),
        }
    if _has(fields, 4):
        consecration = _message(fields, 4)
        return {
            "kind": "consecration",
            "qi_injected": _double(consecration, 1),
            "qi_required": _double(consecration, 2),
        }
    return {"kind": "none"}


def _forge_session(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "forge_session",
        "session_id": _varint(fields, 1),
        "blueprint_id": _string(fields, 2),
        "blueprint_name": _string(fields, 3),
        "active": bool(_varint(fields, 4)),
        "current_step": FORGE_STEP_NAMES.get(_varint(fields, 5), "unspecified"),
        "step_index": _varint(fields, 6),
        "achieved_tier": _varint(fields, 7),
        "step_state": _forge_step_state(_message(fields, 8)),
    }


def _forge_outcome(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "forge_outcome",
        "session_id": _varint(fields, 1),
        "blueprint_id": _string(fields, 2),
        "bucket": FORGE_OUTCOME_BUCKET_NAMES.get(_varint(fields, 3), "unspecified"),
        "weapon_item": _string(fields, 4) if _has(fields, 4) else None,
        "quality": _float32(fields, 5),
        "side_effects": [
            value.decode("utf-8", errors="replace") for value in _messages(fields, 7)
        ],
        "achieved_tier": _varint(fields, 8),
        "flawed_path": bool(_varint(fields, 9)),
    }


def _trade_item_summary(fields: list[tuple[int, int, Any]]) -> dict[str, Any]:
    return {
        "instance_id": _varint(fields, 1),
        "item_id": _string(fields, 2),
        "display_name": _string(fields, 3),
        "stack_count": _varint(fields, 4),
    }


def _trade_offer(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "trade_offer",
        "offer_id": _string(fields, 1),
        "initiator": _string(fields, 2),
        "target": _string(fields, 3),
        "offered_item": _trade_item_summary(_message(fields, 4)),
        "requested_items": [_trade_item_summary(_fields(raw)) for raw in _messages(fields, 5)],
        "expires_at_ms": _varint(fields, 6),
    }


def _sparring_invite(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    return {
        "v": 1,
        "type": "sparring_invite",
        "invite_id": _string(fields, 1),
        "initiator": _string(fields, 2),
        "target": _string(fields, 3),
        "realm_band": _string(fields, 4),
        "breath_hint": _string(fields, 5),
        "terms": _string(fields, 6),
        "expires_at_ms": _varint(fields, 7),
    }


def _forge_blueprint_book(data: bytes) -> dict[str, Any]:
    fields = _fields(data)
    entries = []
    for raw in _messages(fields, 1):
        entry = _fields(raw)
        entries.append(
            {
                "id": _string(entry, 1),
                "display_name": _string(entry, 2),
                "tier_cap": _varint(entry, 3),
                "step_count": _varint(entry, 4),
            }
        )
    return {
        "v": 1,
        "type": "forge_blueprint_book",
        "learned": entries,
        "current_index": _varint(fields, 2),
    }


def _tribulation_state(data: bytes) -> dict[str, Any]:
    """渡虚劫/绝壁劫状态（server_data oneof field 66）。

    kind/phase 在 wire 上是 **string** 字段，不是 varint enum（envelope.proto:2374-2375：
    `string kind = 4; // du_xu / zone_collapse / targeted / jue_bi / ascension_quota_open`、
    `string phase = 5; // omen / lock / wave / heart_demon / settle`），用 `_string` 解。
    heart_demon_decision e2e 必须解出 phase/wave_current/wave_total/result，才能黑盒
    断言「waves_total=5（30 分钟满进度门槛生效）→ 心魔相 → 结算」全链路。
    """
    fields = _fields(data)
    return {
        "v": 1,
        "type": "tribulation_state",
        "active": bool(_varint(fields, 1)),
        "char_id": _string(fields, 2),
        "actor_name": _string(fields, 3),
        "kind": _string(fields, 4),
        "phase": _string(fields, 5),
        "wave_current": _varint(fields, 8),
        "wave_total": _varint(fields, 9),
        "world_x": _double(fields, 6),
        "world_z": _double(fields, 7),
        "started_tick": _varint(fields, 10),
        "phase_started_tick": _varint(fields, 11),
        "next_wave_tick": _varint(fields, 12),
        "failed": bool(_varint(fields, 13)),
        "half_step_on_success": bool(_varint(fields, 14)),
        "participants": [
            value.decode("utf-8", errors="replace")
            for value in _messages(fields, 15)
        ],
        "result": _optional_string(fields, 16),
    }


def _insight_offer(data: bytes) -> dict[str, Any]:
    """DONE-W6-HEADLESSAUDIT §5 P0-4：顿悟邀约（envelope.proto:131）。

    只解 offer 标识字段；choices 保持原始 message 字节（场景按需浅扫描）。
    """
    fields = _fields(data)
    return {
        "v": 1,
        "type": "insight_offer",
        "offer_id": _string(fields, 1),
        "trigger_id": _string(fields, 2),
        "character_id": _string(fields, 3),
        "choices": _messages(fields, 4),
    }


def _heart_demon_offer(data: bytes) -> dict[str, Any]:
    """生产 ``HeartDemonOffer``（server_data oneof field 69，心魔劫抉择面板）。

    heart_demon_decision e2e 断言 offer 形状（choice_id/category/title 三元组），
    并据 choice 面板给出对应的 decision 输入。
    """
    fields = _fields(data)
    choices = []
    for raw in _messages(fields, 9):
        entry = _fields(raw)
        choices.append(
            {
                "choice_id": _string(entry, 1),
                "category": _string(entry, 2),
                "title": _string(entry, 3),
                "effect_summary": _string(entry, 4),
                "flavor": _string(entry, 5),
                "style_hint": _string(entry, 6),
            }
        )
    return {
        "v": 1,
        "type": "heart_demon_offer",
        "offer_id": _string(fields, 1),
        "trigger_id": _string(fields, 2),
        "trigger_label": _string(fields, 3),
        "realm_label": _string(fields, 4),
        "composure": _double(fields, 5),
        "quota_remaining": _varint(fields, 6),
        "quota_total": _varint(fields, 7),
        "expires_at_ms": _varint(fields, 8),
        "choices": choices,
    }


SERVER_DATA_PAYLOAD_DECODERS = {
    3: _narration_batch,
    SERVER_DATA_ZONE_INFO_FIELD: _zone_info,
    SERVER_DATA_PLAYER_STATE_FIELD: _player_state,
    7: _skill_xp_gain,
    8: _inventory_snapshot,
    9: _combat_hud_state,
    11: _alchemy_furnace,
    12: _alchemy_session,
    14: _alchemy_outcome_resolved,
    17: _forge_station,
    18: _forge_session,
    19: _forge_outcome,
    20: _forge_blueprint_book,
    22: _craft_session_state,
    23: _craft_outcome,
    25: _botany_harvest_progress,
    29: _lumber_progress,
    30: _gathering_session,
    31: _lingtian_session,
    34: _cast_sync,
    SERVER_DATA_QUICKSLOT_CONFIG_FIELD: _quick_slot_config,
    36: _skill_bar_config,
    37: _techniques_snapshot,
    43: _treasure_equipped,
    44: lambda data: {"v": 1, "type": "vortex_state"},
    49: _carrier_state,
    50: _false_skin_state,
    51: _combat_event_floater,
    70: _burst_meridian_event,
    54: _skill_config_snapshot,
    SERVER_DATA_SPARRING_INVITE_FIELD: _sparring_invite,
    SERVER_DATA_TRADE_OFFER_FIELD: _trade_offer,
    66: _tribulation_state,
    69: _heart_demon_offer,
    SERVER_DATA_BREAKTHROUGH_CINEMATIC_FIELD: _breakthrough_cinematic,
    72: _death_screen,
    73: _terminate_screen,
    80: _inventory_event,
    81: _dropped_loot_sync,
    90: _container_state,
    97: _skill_scroll_used,
    98: _skill_snapshot,
    119: _loot_container_open,
    120: _loot_container_update,
    121: _loot_container_close,
    131: _insight_offer,
    137: _inventory_move_rejected,
    74: _qi_color_observed,
    77: _event_alert,
    129: _mineral_probe_result,
    130: _freshness_update,
    132: _workbench_open,
    142: _morph_state,
    78: _coffin_state,
}


if not set(SERVER_DATA_PAYLOAD_DECODERS) <= set(SERVER_DATA_PAYLOAD_NAMES):
    raise RuntimeError("deep server_data decoders must use named oneof fields")


def decode_server_data_envelope(data: bytes) -> dict[str, Any] | None:
    for field, wire, value in _fields(data):
        if wire != 2:
            continue
        decoder = SERVER_DATA_PAYLOAD_DECODERS.get(field)
        if decoder is not None:
            return decoder(value)
    return None


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


WIRE_VARINT = 0
WIRE_64BIT = 1
WIRE_LEN = 2
WIRE_32BIT = 5


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
    stripped = data.lstrip()
    if stripped.startswith(b"{"):
        try:
            decoded = json.loads(stripped.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            return None
        if isinstance(decoded, dict) and isinstance(decoded.get("type"), str):
            return decoded["type"]
        return None
    field = server_data_payload_field(data)
    if field is None:
        return None
    known = SERVER_DATA_PAYLOAD_NAMES.get(field)
    if known is not None:
        return known
    # dispatch 分支与 name 注册表独立：注册表漏登记但 decoder 认识的 oneof 字段，
    # 由 decoded payload 的 type 回退，保证 bridge 与 decoder 一致（防注册表漂移）。
    decoded = decode_server_data_envelope(data)
    if isinstance(decoded, dict) and isinstance(decoded.get("type"), str):
        return decoded["type"]
    return f"field_{field}"


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
