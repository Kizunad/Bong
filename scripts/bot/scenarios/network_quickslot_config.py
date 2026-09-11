"""技能栏配置组：quick_slot_bind / use_quick_slot。

黑盒契约面（server/src/network/client_request_handler.rs + schema/client_request.rs）：
- `quick_slot_bind` → handle_quick_slot_bind → send_quick_slot_bind_response：每次必回
  `quickslot_config` payload，携带 ack_request_id 回显 + bind_accepted + 2 槽快照。
  成功 → 绑定物品（slots[i].entry.item_id）；物品不存在 → accepted=false 且槽为空；
  slot 越界（>=2）→ schema 层拒绝（请求在反序列化处 drop，无回执）；request_id 非法（空串/超长
  >128）→ handle_quick_slot_bind 静默返回（无回执）。
  item_id=null → 解绑（accepted=true，槽清空）。
- `use_quick_slot` → handle_use_quick_slot：未开放槽（>=2）/ 无绑定 / 冷却（cast 完成后
  1500ms，DEFAULT_COOLDOWN_MS）/ 同槽 cast 中 → 静默忽略；
  命中绑定 → insert Casting + 推 `cast_sync`{phase=casting, slot, duration_ms}。
  guyuan_pill cast_duration_ms=1500（DEFAULT_CAST_DURATION_MS，pills.toml 未覆写）。
"""

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_instance_by_id,
    find_item,
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = (
    "技能栏：bind 回执/槽快照、非法 request_id 静默后权威状态守恒、use 推 cast_sync、"
    "冷却静默、开放槽 0/1 边界绑定+使用、第三格拒绝、128 合法 request_id、非法槽静默"
)
MODULES = ["inventory", "combat"]

PILL = "guyuan_pill"
BIND_SLOT = 1
NEGATIVE_WINDOW = 2.0
# 精确 tick 边界由 cast_emit::cooldown_set_get_round_trip 与
# quickslot_config_emit_test 保护；E2E 验证冷却期拒用和权威到期后的恢复。
COOLDOWN_MS = 1500


def _expect_bind_response(
    bot,
    request_id: str,
    accepted: bool,
    slot: int,
    timeout: float = 10.0,
    expected_entry: dict | None = None,
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
    assert isinstance(slots, list) and len(slots) == 2, (
        f"quickslot_config.slots 应为固定 2 槽，实际 {slots!r}"
    )
    entry = slots[slot] if slot < len(slots) else None
    if accepted:
        assert entry is not None and entry.get("item_id") == PILL, (
            f"绑定成功槽 {slot} 应含 {PILL}，实际 {entry!r}"
        )
    elif expected_entry is None:
        assert entry is None, f"绑定失败槽 {slot} 应为空，实际 {entry!r}"
    else:
        assert entry == expected_entry, (
            f"绑定失败槽 {slot} 应保持既有条目 {expected_entry!r}，实际 {entry!r}"
        )
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


def _sleep_until_event_time(bot, target_t: float) -> None:
    """睡到事件时间轴上的 target_t（bot.t0 + e.t 与 time.monotonic 对齐）。"""
    delay = (bot.t0 + target_t) - time.monotonic()
    if delay > 0:
        time.sleep(delay)


def _assert_no_cast_sync_until(bot, anchor_t: float, until_t: float) -> None:
    """(anchor_t, until_t] 内不得出现任何新启动的 cast_sync（phase=casting）。

    与 `_assert_no_cast_sync` 同语义，用于扫描权威完成/冷却状态之间的事件。"""
    _sleep_until_event_time(bot, until_t)
    stray = [
        e
        for e in bot.events_of("server_data")
        if e.data.get("payload_type") == "cast_sync"
        and e.data["payload"].get("phase") == "casting"
        and anchor_t < e.t <= until_t
    ]
    if stray:
        raise BotAssertionError(
            f"[{bot.username}] 期望 ({anchor_t:.2f}, {until_t:.2f}] 内无新启动 cast_sync，"
            f"实际收到 {len(stray)} 条"
            f"（slots={[e.data['payload'].get('slot') for e in stray]}）"
        )


def _authoritative_slots(bot, probe_request_id: str) -> list:
    """发合法 bind 回执取权威 2 槽快照。

    静默拒绝只断言「无回执」不足以证明请求未产生副作用——错误实现可先写入绑定、
    再抑制回执（review finding [1]）。合法 bind 的 `quickslot_config` 回执携带当前
    全量 2 槽快照（server build_quickslot_config 实时读 bindings，见
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


def _slot_entries(slots: list) -> list:
    """2 槽快照的逐槽投影：None 或 (item_id, count)，用于全槽逐槽对比。

    review finding [3]：静默拒绝后的权威状态断言必须比较**全部 2 槽**，抽样会放过
    把越界槽 clamp 到邻近槽（尤其 slot 8）的错误实现。"""
    return [
        None if entry is None else (entry.get("item_id"), entry.get("count", 1))
        for entry in slots
    ]


def _give_fresh_pill_and_bind(
    bot, previous_instance_id: int, slot: int, request_id: str
) -> tuple[dict, int]:
    """补一枚全新固元丹，证明上一枚已消费，再把新实例绑定到 ``slot``。

    QuickSlot 按 instance_id 绑定；仅重新发送 bind 而不补给会命中服务端的
    ``inventory_has_instance`` 早退。谓词同时要求新实例存在、旧实例消失，故
    「cast 完成只发 complete 但没有消费」或「give 合并/错误复用旧实例」都会红。
    """
    anchor = last_event_time(bot)
    bot.cmd(f"give {PILL} 1")
    time.sleep(0.5)  # chat→command 给物品先落地，再等 Changed<PlayerInventory> 快照
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and e.t > anchor
            and (found := find_item(e.data["payload"], PILL)) is not None
            and found["location"]["kind"] != "equip"
            and int(found["item"]["instance_id"]) != previous_instance_id
            and find_instance_by_id(e.data["payload"], previous_instance_id) is None
        ),
        timeout=10.0,
        description=(
            f"补给 {PILL} 后新实例出现且旧实例 {previous_instance_id} 已消费"
        ),
    )
    snapshot = event.data["payload"]
    fresh = require_item(snapshot, PILL)
    fresh_instance_id = int(fresh["item"]["instance_id"])
    assert fresh_instance_id != previous_instance_id, (
        f"补给后的 {PILL} 必须是新实例，旧={previous_instance_id} 新={fresh_instance_id}"
    )
    assert find_instance_by_id(snapshot, previous_instance_id) is None, (
        f"上一枚 {PILL} 实例 {previous_instance_id} 应在 cast 完成后被消费，"
        f"实际仍存在于 inventory_snapshot"
    )
    bot.intent(
        {
            "type": "quick_slot_bind",
            "v": 1,
            "slot": slot,
            "item_id": PILL,
            "request_id": request_id,
        }
    )
    _expect_bind_response(bot, request_id, True, slot)
    return snapshot, fresh_instance_id


def _wait_quickslot_cooldown_clear(bot) -> None:
    """同值重绑获取权威冷却；到期提示是 wall-clock 估计，不能替代服务器 tick。"""
    deadline = time.monotonic() + 10.0
    attempt = 0
    while time.monotonic() < deadline:
        request_id = f"gap10-cooldown-{attempt}"
        bot.intent({
            "type": "quick_slot_bind", "v": 1, "slot": BIND_SLOT,
            "item_id": PILL, "request_id": request_id,
        })
        config = _expect_bind_response(bot, request_id, True, BIND_SLOT)
        if config["cooldown_until_ms"][BIND_SLOT] == 0:
            return
        attempt += 1
        time.sleep(0.1)
    raise BotAssertionError("快捷槽冷却未在 10s 内由服务器确认到期")


def run(env) -> None:
    with env.new_bot("Quickslot") as bot:
        wait_for_ready(bot)
        # 一份两枚的堆叠让首次消费后绑定仍有效，冷却探针不再夹杂补给/重绑耗时。
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar", timeout=10.0)
        give_anchor = last_event_time(bot)
        bot.cmd(f"give {PILL} 2")
        bot.expect_chat(f"[dev] gave {PILL} x2", timeout=10.0)
        snapshot = wait_inventory_contains(bot, PILL, timeout=10.0, after_t=give_anchor)
        initial_pill = require_item(snapshot, PILL)
        initial_pill_instance = int(initial_pill["item"]["instance_id"])
        assert initial_pill["item"]["stack_count"] == 2, "冷却测试需要同实例的两枚丹药"

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
                "slot": 0,
                "item_id": "no_such_item_xyz",
                "request_id": "gap10-bind-2",
            }
        )
        _expect_bind_response(bot, "gap10-bind-2", False, 0)

        # 越界绑定无回执；前后对比两格权威快照，防止错误 clamp 后静默改绑。
        baseline_slots = _authoritative_slots(bot, "gap10-probe-3-base")
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": 2,
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
            f"[{bot.username}] slot=2 bind 应在 schema 层静默拒绝，实际收到 {len(stray)} 条 quickslot_config"
        )
        # ── 3b. slot=2 静默后权威 2 槽状态逐槽不变（review finding [1]/[3]）──
        slots = _authoritative_slots(bot, "gap10-probe-3")
        assert _slot_entries(slots) == _slot_entries(baseline_slots), (
            f"slot=2 请求后权威 2 槽必须逐槽不变，实际 {_slot_entries(slots)}"
        )

        # ── 4. bind 静默：非法 request_id（空串 + 超长>128）→ 无回执 ──
        #    review finding [6]：旧场景只测空串，放过了「接受任意超长非空 id 且
        #    变异绑定」的错误实现。schema maxLength=128、handler len()>128 双拒。
        #    请求前捕获基线，请求后对比全部两格。
        baseline_slots = _authoritative_slots(bot, "gap10-probe-4-base")
        for slot, bad_request_id in ((0, ""), (0, "x" * 129)):
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

        # ── 4b. 静默拒绝后权威 2 槽状态逐槽不变（review finding [1]/[6]）──
        slots = _authoritative_slots(bot, "gap10-probe-4")
        assert _slot_entries(slots) == _slot_entries(baseline_slots), (
            f"非法 request_id 请求后权威 2 槽必须逐槽不变，实际 {_slot_entries(slots)}"
        )
        # ── 4c. 功能后置：空 request_id 请求带 slot=0，若被错误写入绑定，use slot 0
        #     会错误启动 cast（review finding [1] 举的具体例子）——直接钉死。──
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 0})
        _assert_no_cast_sync(bot, anchor)

        # ── 4d. 最大合法 request_id 长度 128 必须接受（review finding [6] / Unicode 契约对齐）──
        #     schema maxLength=128、handler chars().count()>128 双拒；
        #     合法 128 个非 ASCII 字符（如 '界'，UTF-8 384 字节）必须绑定成功（accepted + 槽含物品）。
        rid128 = "界" * 128
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": 0,
                "item_id": PILL,
                "request_id": rid128,
            }
        )
        bind_0 = _expect_bind_response(bot, rid128, True, 0)

        # ── 4e. 畸形 item_id="" 拒绝且既有绑定不变 ──
        #     schema 契约仅允许 null 或 minLength:1 字符串，item_id="" 为畸形输入。
        #     服务端应回推 bind_accepted=false，不得将其当成 null 解绑清空槽 0。
        empty_item_rid = "gap10-empty-item-4e"
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": 0,
                "item_id": "",
                "request_id": empty_item_rid,
            }
        )
        _expect_bind_response(
            bot,
            empty_item_rid,
            False,
            0,
            expected_entry=bind_0["slots"][0],
        )
        # 槽 0 必须保持槽含物品（未被清空）
        slots_after_empty = _authoritative_slots(bot, "gap10-probe-4e")
        assert slots_after_empty[0] is not None, (
            f"item_id='' 拒绝后槽 0 必须保持绑定，实际被清空为 None: {slots_after_empty[0]!r}"
        )

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
        # ── 6a. use 活动 cast 分支（review finding [2]）：cast 进行中（casting 已推、
        #     complete 未到）再按**同槽**，必须静默忽略。旧场景只在完成后的冷却
        #     分支测静默，从未在活动 cast 中按同槽——「cast 中重启同槽 cast」的
        #     错误实现会通过（重启会再推一条 casting）。──
        active_anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": BIND_SLOT})
        # 等完成后复扫活动阶段，避免固定 2s 等待吞掉随后的冷却测试窗口。
        complete_event = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "complete"
                and e.data["payload"].get("slot") == BIND_SLOT
                and e.t > active_anchor
            ),
            timeout=10.0,
            description=f"slot={BIND_SLOT} 第一次 cast 的 cast_sync(complete)",
        )
        complete_t = complete_event.t
        _assert_no_cast_sync_until(bot, active_anchor, complete_t)
        consumed_snapshot = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
                and e.t > complete_t
                and (remaining := find_instance_by_id(e.data["payload"], initial_pill_instance)) is not None
                and remaining["item"]["stack_count"] == 1
            ),
            timeout=10.0,
            description=(
                f"{PILL} 实例 {initial_pill_instance} 在 cast complete 后剩余一枚"
            ),
        ).data["payload"]
        assert find_instance_by_id(consumed_snapshot, initial_pill_instance)["item"]["stack_count"] == 1

        # ── 6b. 冷却期拒用，然后按服务器确认的到期状态重新施放。 ──
        cooldown = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data.get("payload_type") == "quickslot_config"
            and e.t > complete_t
            and e.data["payload"]["cooldown_until_ms"][BIND_SLOT] > 0,
            timeout=10.0,
            description="首次消费后的非零快捷槽冷却",
        ).data["payload"]
        assert cooldown["slots"][BIND_SLOT]["cooldown_ms"] == COOLDOWN_MS
        cooldown_anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": BIND_SLOT})
        _wait_quickslot_cooldown_clear(bot)
        after_anchor = last_event_time(bot)
        _assert_no_cast_sync_until(bot, cooldown_anchor, after_anchor)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": BIND_SLOT})
        recovered = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "casting"
                and e.data["payload"].get("slot") == BIND_SLOT
                and e.t > after_anchor
            ),
            timeout=10.0,
            description=(
                "服务器确认冷却到期后 "
                f"slot={BIND_SLOT} 重新施放的 cast_sync(casting)"
            ),
        ).data["payload"]
        assert int(recovered.get("duration_ms", 0)) == COOLDOWN_MS, (
            f"冷却恢复 cast 的 duration_ms 应为 {COOLDOWN_MS}，"
            f"实际 {recovered.get('duration_ms')!r}"
        )
        # 恢复 cast 同样要等 complete 收尾：后续 step 7 在异槽 use，玩家 Casting 态
        # 未清会触发 UserCancel+重启而非干净新 cast（见 7a 注释），必须同步回 Idle。
        recover_anchor = last_event_time(bot)
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "complete"
                and e.data["payload"].get("slot") == BIND_SLOT
                and e.t > recover_anchor
            ),
            timeout=10.0,
            description=f"冷却恢复 cast 的 cast_sync(complete)",
        )

        # 补给并绑定第一格，再证明越界使用不会误用现有物品。
        _give_fresh_pill_and_bind(bot, initial_pill_instance, 0, "gap10-bind-0")
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 2})
        _assert_no_cast_sync(bot, anchor)

        # 清空第二格，后续用它验证未绑定请求不会打断第一格。
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": BIND_SLOT,
                "item_id": None,
                "request_id": "unbind-before-cross-slot",
            }
        )
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "quickslot_config"
                and e.data["payload"].get("ack_request_id") == "unbind-before-cross-slot"
                and e.data["payload"].get("bind_accepted") is True
            ),
            timeout=10.0,
            description="第二格解绑完成",
        )

        # 第一格边界；第二格的绑定、使用和冷却已由前面的 BIND_SLOT=1 覆盖。
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 0})
        cast0 = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "casting"
                and e.data["payload"].get("slot") == 0
            ),
            timeout=10.0,
            description="use_quick_slot slot=0 的 cast_sync(casting)",
        ).data["payload"]
        assert int(cast0.get("duration_ms", 0)) == 1500, (
            f"guyuan_pill cast_duration_ms 应为 1500，实际 {cast0.get('duration_ms')!r}"
        )

        # ── 8. use_quick_slot 静默：未绑定槽 → 无新 cast_sync ──
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 1})
        _assert_no_cast_sync(bot, anchor)

        # ── 8b. 未绑定槽请求不得打断进行中的 slot 0 cast（review finding [1]）──
        #    7b 刚启动 slot 0 的 1500ms cast，8 的未绑定槽请求此时到达。只断言
        #    「无新 casting」放过了「先走 cast 闸门取消 slot 0、再发现槽 1 未绑定」
        #    的错误实现——cancel 不发新 casting 事件（发 cast_sync{phase=interrupt,
        #    outcome=user_cancel}），却中断 slot 0 的效果。双向封死：(a) 窗口内
        #    不得出现 slot 0 的 interrupt 事件；(b) slot 0 的 cast 必须仍走完
        #    complete。8 的 2s 负窗口已覆盖 interrupt/complete 的到达窗口。
        interrupted = [
            e
            for e in bot.events_of("server_data")
            if e.data.get("payload_type") == "cast_sync"
            and e.data["payload"].get("slot") == 0
            and e.data["payload"].get("phase") == "interrupt"
            and e.t > anchor
        ]
        assert not interrupted, (
            f"[{bot.username}] 未绑定槽请求不得打断 slot 0 的进行中 cast，"
            f"实际收到 {len(interrupted)} 条 cast_sync(interrupt, slot=0)"
        )
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "complete"
                and e.data["payload"].get("slot") == 0
                and e.t > anchor
            ),
            timeout=10.0,
            description="未绑定槽请求后 slot 0 的 cast 仍应走完 complete",
        )

        # ── 9. use_quick_slot 静默：slot>=2 越界 → 无新 cast_sync ──
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 2})
        _assert_no_cast_sync(bot, anchor)

        bot.assert_alive("技能栏 9 步正负路径后")
