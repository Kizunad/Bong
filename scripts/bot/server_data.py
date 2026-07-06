"""Helpers for decoding Bong S2C gameplay payloads observed by protocol bots.

Production `bong:server_data` uses protobuf envelopes. The bot keeps this
decoder local to scripts/bot so gameplay scenarios can assert user-visible
payloads without depending on server internals or generated checked-in Python.
"""

from __future__ import annotations

import json
from typing import Any

from .proto_min import ProtoDecodeError, decode_server_data_envelope


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

    try:
        return decode_server_data_envelope(data)
    except ProtoDecodeError:
        return None
