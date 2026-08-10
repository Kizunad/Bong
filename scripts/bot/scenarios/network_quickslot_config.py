"""技能栏配置组：quick_slot_bind / use_quick_slot。

黑盒契约面（server/src/network/client_request_handler.rs + schema/client_request.rs）：
- `quick_slot_bind` → handle_quick_slot_bind → send_quick_slot_bind_response：每次必回
  `quickslot_config` payload，携带 ack_request_id 回显 + bind_accepted + 9 槽快照。
  成功 → 绑定物品（slots[i].entry.item_id）；物品不存在 → accepted=false 且槽为空；
  slot 越界（>=9）→ **schema 层拒绝**（deserialize_slot_index 抛 "slot must be
  between 0 and 8"，请求在反序列化处 drop，无回执）；request_id 非法（空串/超长
  >128）→ handle_quick_slot_bind 静默返回（无回执）。
  item_id=null → 解绑（accepted=true，槽清空）。
- `use_quick_slot` → handle_use_quick_slot：slot>=9 / 无绑定 / 冷却（cast 完成后
  1500ms，DEFAULT_COOLDOWN_MS）/ 同槽 cast 中 → 静默忽略；
  命中绑定 → insert Casting + 推 `cast_sync`{phase=casting, slot, duration_ms}。
  guyuan_pill cast_duration_ms=1500（DEFAULT_CAST_DURATION_MS，pills.toml 未覆写）。
"""

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = (
    "技能栏：bind 回执/槽快照、非法 request_id 静默后权威状态守恒、use 推 cast_sync、"
    "冷却静默、最大合法槽 8 绑定+使用、非法槽静默"
)
MODULES = ["inventory", "combat"]

PILL = "guyuan_pill"
BIND_SLOT = 1
NEGATIVE_WINDOW = 2.0


def _expect_bind_response(
    bot, request_id: str, accepted: bool, slot: int, timeout: float = 10.0
) -> dict:
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "quickslot_config"
            and e.data["payload"].get("ack_request_id") == request_id
        ),
        timeout=timeout,
        description=f"quickslot_config 回执 ack_request_id={request_id}",
    )
    payload = event.data["payload"]
    got = payload.get("bind_accepted")
    assert got is accepted, f"bind 回执 bind_accepted 应为 {accepted}，实际 {got!r}"
    slots = payload.get("slots")
    assert isinstance(slots, list) and len(slots) == 9, (
        f"quickslot_config.slots 应为固定 9 槽，实际 {slots!r}"
    )
    entry = slots[slot] if slot < len(slots) else None
    if accepted:
        assert entry is not None and entry.get("item_id") == PILL, (
            f"绑定成功槽 {slot} 应含 {PILL}，实际 {entry!r}"
        )
    else:
        assert entry is None, f"绑定失败槽 {slot} 应为空，实际 {entry!r}"
    return payload


def _assert_no_cast_sync(bot, anchor_t: float) -> None:
    """窗口内不得出现任何新启动的 cast_sync（phase=casting，任意 slot）。

    场景内已启动的 cast 在本窗口内只推 phase=complete，phase=casting 过滤天然排除
    它；按 slot 过滤会放过「请求非法槽却回落到已绑定槽 1 开火」的错误实现
    （central-review 2012 #7）——新 cast 的 phase=casting + slot=1 会被漏掉。"""
    time.sleep(NEGATIVE_WINDOW)
    stray = [
        e
        for e in bot.events_of("server_data")
        if e.data.get("payload_type") == "cast_sync"
        and e.data["payload"].get("phase") == "casting"
        and e.t > anchor_t
    ]
    if stray:
        raise BotAssertionError(
            f"[{bot.username}] 期望 {NEGATIVE_WINDOW}s 内无新启动 cast_sync，"
            f"实际收到 {len(stray)} 条"
            f"（slots={[e.data['payload'].get('slot') for e in stray]}）"
        )


def _authoritative_slots(bot, probe_request_id: str) -> list:
    """发合法 bind 回执取权威 9 槽快照。

    静默拒绝只断言「无回执」不足以证明请求未产生副作用——错误实现可先写入绑定、
    再抑制回执（review finding [1]）。合法 bind 的 `quickslot_config` 回执携带当前
    全量 9 槽快照（server build_quickslot_config 实时读 bindings，见
    send_quick_slot_bind_response）。probe 用重绑 slot 1（item 恒在包），不改任何
    槽内容即拿到权威状态。"""
    bot.intent(
        {
            "type": "quick_slot_bind",
            "v": 1,
            "slot": BIND_SLOT,
            "item_id": PILL,
            "request_id": probe_request_id,
        }
    )
    return _expect_bind_response(bot, probe_request_id, True, BIND_SLOT)["slots"]


def run(env) -> None:
    with env.new_bot("Quickslot") as bot:
        wait_for_ready(bot)
        # 不清包：guyuan_pill 是起手包常驻（assets/inventory/loadouts/default.toml）；
        # clearinv all 会把它清掉，bind 反被拒「not in inventory」。
        snapshot = wait_inventory_contains(bot, PILL, timeout=10.0)
        require_item(snapshot, PILL)

        # ── 1. bind 正路径：回执 ack 回显 + accepted=true + 槽含物品 ──
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": BIND_SLOT,
                "item_id": PILL,
                "request_id": "gap10-bind-1",
            }
        )
        _expect_bind_response(bot, "gap10-bind-1", True, BIND_SLOT)

        # ── 2. bind 拒绝：物品不在背包 → accepted=false + 槽为空 ──
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": 2,
                "item_id": "no_such_item_xyz",
                "request_id": "gap10-bind-2",
            }
        )
        _expect_bind_response(bot, "gap10-bind-2", False, 2)

        # ── 3. bind 静默：slot>=9 越界 → schema 层拒绝（"slot must be between
        #    0 and 8"），无回执（实测 server.log，不满足 0..=8 直接 drop）──
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": 9,
                "item_id": PILL,
                "request_id": "gap10-bind-3",
            }
        )
        time.sleep(NEGATIVE_WINDOW)
        stray = [
            e
            for e in bot.events_of("server_data")
            if e.data.get("payload_type") == "quickslot_config" and e.t > anchor
        ]
        assert not stray, (
            f"[{bot.username}] slot=9 bind 应在 schema 层静默拒绝，实际收到 {len(stray)} 条 quickslot_config"
        )
        # ── 3b. slot=9 静默后权威状态不变（review finding [1]）──
        slots = _authoritative_slots(bot, "gap10-probe-3")
        assert slots[BIND_SLOT] is not None and slots[BIND_SLOT]["item_id"] == PILL, (
            f"slot=9 请求后已绑定槽 {BIND_SLOT} 应保持 {PILL}，实际 {slots[BIND_SLOT]!r}"
        )
        for empty_slot in (2, 3):
            assert slots[empty_slot] is None, (
                f"slot=9 请求不得污染其他槽，实际 slots[{empty_slot}]={slots[empty_slot]!r}"
            )

        # ── 4. bind 静默：非法 request_id（空串 + 超长>128）→ 无回执 ──
        #    review finding [6]：旧场景只测空串，放过了「接受任意超长非空 id 且
        #    变异绑定」的错误实现。schema maxLength=128、handler len()>128 双拒。
        for slot, bad_request_id in ((3, ""), (6, "x" * 129)):
            anchor = last_event_time(bot)
            bot.intent(
                {
                    "type": "quick_slot_bind",
                    "v": 1,
                    "slot": slot,
                    "item_id": PILL,
                    "request_id": bad_request_id,
                }
            )
            time.sleep(NEGATIVE_WINDOW)
            stray = [
                e
                for e in bot.events_of("server_data")
                if e.data.get("payload_type") == "quickslot_config" and e.t > anchor
            ]
            assert not stray, (
                f"[{bot.username}] 非法 request_id（len={len(bad_request_id)}）应静默，"
                f"实际收到 {len(stray)} 条 quickslot_config"
            )

        # ── 4b. 静默拒绝后权威状态不变（review finding [1]/[6]）──
        slots = _authoritative_slots(bot, "gap10-probe-4")
        assert slots[BIND_SLOT] is not None and slots[BIND_SLOT]["item_id"] == PILL, (
            f"非法 request_id 请求后已绑定槽 {BIND_SLOT} 应保持 {PILL}，实际 {slots[BIND_SLOT]!r}"
        )
        for empty_slot in (2, 3, 6):
            assert slots[empty_slot] is None, (
                f"非法 request_id 请求不得把槽 {empty_slot} 绑定上，实际 {slots[empty_slot]!r}"
            )
        # ── 4c. 功能后置：空 request_id 请求带 slot=3，若被错误写入绑定，use slot 3
        #     会错误启动 cast（review finding [1] 举的具体例子）——直接钉死。──
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 3})
        _assert_no_cast_sync(bot, anchor)

        # ── 5. bind 解绑：item_id=null → accepted=true + 槽清空 ──
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": BIND_SLOT,
                "item_id": None,
                "request_id": "gap10-bind-5",
            }
        )
        event = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "quickslot_config"
                and e.data["payload"].get("ack_request_id") == "gap10-bind-5"
            ),
            timeout=10.0,
            description="解绑回执 ack_request_id=gap10-bind-5",
        )
        payload = event.data["payload"]
        assert payload.get("bind_accepted") is True, (
            f"解绑 bind_accepted 应为 True，实际 {payload.get('bind_accepted')!r}"
        )
        assert payload["slots"][BIND_SLOT] is None, (
            f"解绑后槽 {BIND_SLOT} 应为空，实际 {payload['slots'][BIND_SLOT]!r}"
        )

        # ── 6. use_quick_slot 正路径：重绑后 use → cast_sync{phase=casting, slot} ──
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": BIND_SLOT,
                "item_id": PILL,
                "request_id": "gap10-bind-6",
            }
        )
        _expect_bind_response(bot, "gap10-bind-6", True, BIND_SLOT)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": BIND_SLOT})
        cast = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "casting"
                and e.data["payload"].get("slot") == BIND_SLOT
            ),
            timeout=10.0,
            description=f"use_quick_slot slot={BIND_SLOT} 的 cast_sync(casting)",
        ).data["payload"]
        assert int(cast.get("duration_ms", 0)) == 1500, (
            f"guyuan_pill cast_duration_ms 应为 1500，实际 {cast.get('duration_ms')!r}"
        )
        # ── 6b. use 冷却分支（review finding [5]）：cast 完成后槽 1 进入 1500ms
        #     冷却（DEFAULT_COOLDOWN_MS，cast 完成 tick 起算），期间再次 use 必须
        #     静默。旧场景只在解绑槽 5 / 越界槽 9 上测静默，从未在冷却中的已绑定
        #     槽上测——忽略冷却、命中即再开 cast 的错误实现会通过。收到 complete
        #     时冷却已随同 tick 写入（cast_emit set_cast_cooldown 先于 push_cast_
        #     sync(Complete)），立刻重按必落在冷却窗口内。──
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "complete"
                and e.data["payload"].get("slot") == BIND_SLOT
            ),
            timeout=10.0,
            description=f"slot={BIND_SLOT} 第一次 cast 的 cast_sync(complete)",
        )
        cooldown_anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": BIND_SLOT})
        _assert_no_cast_sync(bot, cooldown_anchor)

        # ── 7. 最大合法槽 8 的绑定 + 使用（review finding [4]）──
        #    契约定义 0..=8 合法；旧场景只 bind slot 1、拒 slot 9，从未触达最大
        #    合法值——`slot < 8` 的 off-by-one 实现会通过。必须 bind 且 use slot 8。
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": 8,
                "item_id": PILL,
                "request_id": "gap10-bind-8",
            }
        )
        _expect_bind_response(bot, "gap10-bind-8", True, 8)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 8})
        cast8 = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "casting"
                and e.data["payload"].get("slot") == 8
            ),
            timeout=10.0,
            description="use_quick_slot slot=8 的 cast_sync(casting)",
        ).data["payload"]
        assert int(cast8.get("duration_ms", 0)) == 1500, (
            f"guyuan_pill cast_duration_ms 应为 1500，实际 {cast8.get('duration_ms')!r}"
        )

        # ── 8. use_quick_slot 静默：未绑定槽 → 无新 cast_sync ──
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 5})
        _assert_no_cast_sync(bot, anchor)

        # ── 9. use_quick_slot 静默：slot>=9 越界 → 无新 cast_sync ──
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 9})
        _assert_no_cast_sync(bot, anchor)

        bot.assert_alive("技能栏 9 步正负路径后")
