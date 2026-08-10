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
    "冷却静默、合法槽 0/8 两端边界绑定+使用、128 合法 request_id、非法槽静默"
)
MODULES = ["inventory", "combat"]

PILL = "guyuan_pill"
BIND_SLOT = 1
NEGATIVE_WINDOW = 2.0
# use_quick_slot 的冷却契约（server DEFAULT_COOLDOWN_MS=1500，guyuan_pill 未覆写）。
# 边界探针在 claimed 值两侧各 ±COOLDOWN_TOLERANCE_MS 处打点（见 6b 注释）。
COOLDOWN_MS = 1500
COOLDOWN_TOLERANCE_MS = 400


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


def _sleep_until_event_time(bot, target_t: float) -> None:
    """睡到事件时间轴上的 target_t（bot.t0 + e.t 与 time.monotonic 对齐）。"""
    delay = (bot.t0 + target_t) - time.monotonic()
    if delay > 0:
        time.sleep(delay)


def _assert_no_cast_sync_until(bot, anchor_t: float, until_t: float) -> None:
    """(anchor_t, until_t] 内不得出现任何新启动的 cast_sync（phase=casting）。

    与 `_assert_no_cast_sync`（固定 NEGATIVE_WINDOW 窗口）同语义，但窗口终点由调用
    方给出——冷却边界探针要观察 1500ms 边界两侧的**小段**窗口，而不是一次盖满 2s。"""
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


def _slot_entries(slots: list) -> list:
    """9 槽快照的逐槽投影：None 或 (item_id, count)，用于全槽逐槽对比。

    review finding [3]：静默拒绝后的权威状态断言必须比较**全部 9 槽**，抽样会放过
    把越界槽 clamp 到邻近槽（尤其 slot 8）的错误实现。"""
    return [
        None if entry is None else (entry.get("item_id"), entry.get("count", 1))
        for entry in slots
    ]


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
        #    先取权威基线：逐槽投影**全部 9 槽**（review finding [3]）——旧断言只抽查
        #    slot 1/2/3，放过了「把 slot 9 clamp 到 8、绑上药丸、再抑制回执」的
        #    off-by-one 实现（slot 8 是最可能的越界落点，且后续 slot-8 正路径会掩盖
        #    那次变异）。基线必须取在请求之前。
        baseline_slots = _authoritative_slots(bot, "gap10-probe-3-base")
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
        # ── 3b. slot=9 静默后权威 9 槽状态逐槽不变（review finding [1]/[3]）──
        slots = _authoritative_slots(bot, "gap10-probe-3")
        assert _slot_entries(slots) == _slot_entries(baseline_slots), (
            f"slot=9 请求后权威 9 槽必须逐槽不变，实际 {_slot_entries(slots)}"
        )

        # ── 4. bind 静默：非法 request_id（空串 + 超长>128）→ 无回执 ──
        #    review finding [6]：旧场景只测空串，放过了「接受任意超长非空 id 且
        #    变异绑定」的错误实现。schema maxLength=128、handler len()>128 双拒。
        #    review finding [6]（round 10）：旧 4b 断言只在请求后抽查 slot 1/2/3/6，
        #    放过了「非法 id 被写入未抽查槽（如 slot 8）再静默」的错误实现（后续
        #    slot-8 正路径会掩盖那次变异）。基线必须捕获在请求**之前**，请求后逐槽
        #    对比**全部 9 槽**（与 3b 的 slot=9 静默同法）。
        baseline_slots = _authoritative_slots(bot, "gap10-probe-4-base")
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

        # ── 4b. 静默拒绝后权威 9 槽状态逐槽不变（review finding [1]/[6]）──
        slots = _authoritative_slots(bot, "gap10-probe-4")
        assert _slot_entries(slots) == _slot_entries(baseline_slots), (
            f"非法 request_id 请求后权威 9 槽必须逐槽不变，实际 {_slot_entries(slots)}"
        )
        # ── 4c. 功能后置：空 request_id 请求带 slot=3，若被错误写入绑定，use slot 3
        #     会错误启动 cast（review finding [1] 举的具体例子）——直接钉死。──
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 3})
        _assert_no_cast_sync(bot, anchor)

        # ── 4d. 最大合法 request_id 长度 128 必须接受（review finding [6]）──
        #     schema maxLength=128、handler len()>128 双拒；旧场景只测空串与 129
        #     超长，一个把 len>=128 当非法的实现会拒绝恰好 128 的合法 id——等价
        #     边界缺失。合法 128 必须绑定成功（accepted + 槽含物品）。
        rid128 = "r" * 128
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": 4,
                "item_id": PILL,
                "request_id": rid128,
            }
        )
        _expect_bind_response(bot, rid128, True, 4)

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
        _assert_no_cast_sync(bot, active_anchor)
        # 等本条 cast 走完 complete：6a 的 2s 负窗口已覆盖 1500ms cast 全程，
        # complete 已缓冲，wait_for 立即返回；同步 Casting→Idle 让 6b 的冷却拒绝
        # 落在干净起点。complete_t 是冷却起点的 bot 侧观测（cast_emit 先
        # set_cast_cooldown 再 push_cast_sync(Complete)，同一 tick）。
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

        # ── 6b. use 冷却分支 + 1500ms 边界钉死（review finding [5] + central-review
        #     31438252846 finding [7]）。冷却从 cast 完成 tick 起算
        #     （set_cast_cooldown 先于 push_cast_sync(Complete)，is_on_cooldown 判定
        #     cooldown_until_tick > now_tick，DEFAULT_COOLDOWN_MS=1500 → 30 tick），
        #     bot 观测的 complete_t 即冷却起点。旧测试只在 complete 后立刻按一次 +
        #     固定 2s 负窗口 + 窗口后按一次，等价断言「0 < 冷却 ≤ 2s」——把 1500ms
        #     误写成 1000ms 的错误实现也全过。现在把观察点移到 claimed 边界两侧
        #     （±COOLDOWN_TOLERANCE_MS=400ms）：
        #       (b) complete_t+1100ms 处 use → 必须仍静默（冷却 ≤1100ms 的实现此时
        #           已过期、开火即被抓）；
        #       (c) complete_t+1900ms 处 use → 必须新开 cast（冷却 >1900ms 的实现
        #           仍冷却、无 casting 即被抓）。
        #     两探针把冷却钉在 (1100, 1900]ms，而不是旧实现的 (0, 2000]ms。
        #     transport 容差：本地 e2e bot 观测 complete_t 与 server 冷却起点差
        #     <100ms，±400ms 裕量充足；20tps 下 30 tick 冷却恰好 1500ms。
        cooldown_anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": BIND_SLOT})
        # (a) 立刻重按必须静默（无冷却的实现会立即开火）。
        _assert_no_cast_sync_until(
            bot, cooldown_anchor, complete_t + COOLDOWN_TOLERANCE_MS / 1000.0
        )
        # (b) 边界前探针：仍须静默（冷却尚未过期）。
        before_probe_t = complete_t + (COOLDOWN_MS - COOLDOWN_TOLERANCE_MS) / 1000.0
        _sleep_until_event_time(bot, before_probe_t)
        before_anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": BIND_SLOT})
        _assert_no_cast_sync_until(
            bot, before_anchor, before_probe_t + COOLDOWN_TOLERANCE_MS / 1000.0
        )
        # (c) 边界后探针：必须新开 cast（冷却已过期）。
        after_probe_t = complete_t + (COOLDOWN_MS + COOLDOWN_TOLERANCE_MS) / 1000.0
        _sleep_until_event_time(bot, after_probe_t)
        after_anchor = last_event_time(bot)
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
                f"冷却边界（complete_t+{COOLDOWN_MS + COOLDOWN_TOLERANCE_MS}ms）后 "
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
        # ── 7a. 等 slot 8 cast 完成（review finding [2]）──
        # 生产 quick-slot 路径以玩家 Casting 态为闸门：cast 进行中再 use（异槽）会
        # 触发 UserCancel + 重启而非干净的新 cast，同槽则静默忽略
        # （client_request_handler.rs handle_use_quick_slot）。cast_sync(casting)
        # 只证明 cast 已启动（duration 1500ms），不等 complete 就发 slot 0 use，
        # slot 0 请求会在 slot 8 仍在 cast 时被 Cancel/Casting 逻辑吞掉，后续
        # cast_sync{phase=casting, slot=0} 超时。6b 的 slot 1 路径已先等 complete
        # 再发下一条 use；7b 必须同样同步 Casting→Idle 状态转换，两条成功 use 之间
        # 缺这道完成同步（review 根因：把收到前一个 cast 的 casting 事件当成玩家
        # 已可再施放）。
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "cast_sync"
                and e.data["payload"].get("phase") == "complete"
                and e.data["payload"].get("slot") == 8
            ),
            timeout=10.0,
            description="slot=8 第一次 cast 的 cast_sync(complete)",
        )

        # ── 7b. 最小合法槽 0 的绑定 + 使用（review finding [4]）──
        #    契约定义 0..=8 合法；旧场景只 bind slot 1、slot 8、拒 slot 9，下边界 0
        #    从未触达——把 0 当非法的 `1..=8` 实现（或把 0 静默丢弃）会通过全部现有
        #    断言。必须 bind 且 use slot 0（use 推 cast_sync{phase=casting, slot=0}）。
        bot.intent(
            {
                "type": "quick_slot_bind",
                "v": 1,
                "slot": 0,
                "item_id": PILL,
                "request_id": "gap10-bind-0",
            }
        )
        _expect_bind_response(bot, "gap10-bind-0", True, 0)
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
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 5})
        _assert_no_cast_sync(bot, anchor)

        # ── 8b. 未绑定槽请求不得打断进行中的 slot 0 cast（review finding [1]）──
        #    7b 刚启动 slot 0 的 1500ms cast，8 的未绑定槽请求此时到达。只断言
        #    「无新 casting」放过了「先走 cast 闸门取消 slot 0、再发现槽 5 未绑定」
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

        # ── 9. use_quick_slot 静默：slot>=9 越界 → 无新 cast_sync ──
        anchor = last_event_time(bot)
        bot.intent({"type": "use_quick_slot", "v": 1, "slot": 9})
        _assert_no_cast_sync(bot, anchor)

        bot.assert_alive("技能栏 9 步正负路径后")
