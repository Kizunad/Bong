"""`bong:client_request` 越界字段值 —— schema / version 门禁干净拒绝。

三档拒绝都在 handler 产生任何玩法副作用**之前**拦截：
- **serde 守卫**（`client_request.rs` 内 `deserialize_slot_index` /
  `deserialize_block_picker_count`）：hotbar/quick slot >8 或负数、block_picker
  count=0 或 >64、v 超出 u8、非法 enum 变体 —— 反序列化期拒绝；
- **version 门禁**（`client_request_handler.rs` SUPPORTED_VERSION=1）：v≠1 ——
  handler 入口拒绝。

本场景锁的是：每个越界值都被**干净**拒绝 —— 不崩、不踢、连接状态完好；
探针窗口内**无任何玩法副作用**（server_data / chat / vfx 均未出现），且探针前后
背包快照指纹（revision + 内容）完全一致 —— 证明越界请求在产生任何玩法副作用
之前就被拦截，没有被 clamp 后继续执行；之后合法请求仍被正常处理。

**合法边界也要证明被接受**（review finding 1）：只探坏值不探好值，一个 off-by-one
实现（只接受 slot 0..7 / count 1..63）会拒绝掉全部坏探针、通过全部干净拒绝断言，
却错误拒绝了上边界合法请求。故反向补两组**恰好落在契约边界内**的正向探针：
- slot 0 与 slot 8（`deserialize_slot_index` 契约 0..=8）：`quick_slot_bind` 清空
  该槽，server 回推 `quickslot_config` ack（`ack_request_id` 回显本次 request_id +
  `bind_accepted=true`）—— 证明 serde 接受边界槽位；
- count 1 与 count 64（`deserialize_block_picker_count` 契约 1..=64）：
  `block_picker_give` 进入 handler 并给出 [dev] 回应（fixture 是 Survival，得到
  "requires Creative mode" 聊天；若 Creative 则 "gave ..." 聊天）—— 任一聊天都
  证明 serde 接受了边界 count。

正向探针同样用相对时钟锚点（`time.monotonic() - bot.t0`，与 `event.t` 同一
时钟），响应必须严格晚于发送时刻。
"""

import time

from bot.bot import BotAssertionError  # noqa: F401
from bot.scenarios._inventory_helpers import wait_inventory_contains

DESCRIPTION = "bong:client_request 越界字段值(slot/count/v/变体) 被 schema+版本门禁干净拒绝"
MODULES = ["network"]

OUT_OF_RANGE_PROBES = [
    ("hotbar slot=9 越界", {"type": "use_quick_slot", "v": 1, "slot": 9}),
    ("hotbar slot=-1 负数", {"type": "use_quick_slot", "v": 1, "slot": -1}),
    (
        "block_picker_give count=0",
        {"type": "block_picker_give", "v": 1, "block_id": "stone_bricks", "count": 0},
    ),
    (
        "block_picker_give count=65",
        {"type": "block_picker_give", "v": 1, "block_id": "stone_bricks", "count": 65},
    ),
    ("版本 v=0 低于支持版本", {"type": "breakthrough_request", "v": 0}),
    ("版本 v=255 高于支持版本", {"type": "breakthrough_request", "v": 255}),
    ("v 超出 u8 范围", {"type": "breakthrough_request", "v": 999999999}),
    (
        "botany_harvest mode 非法变体",
        {"type": "botany_harvest_request", "v": 1, "session_id": "x", "mode": "garbage"},
    ),
]

# 契约边界内**合法** slot 值（deserialize_slot_index 契约 0..=8）。用 quick_slot_bind
# 清空槽位（item_id=None）作正向探针：其 ack（quickslot_config，回显 request_id +
# bind_accepted）只有请求真正走完 handler 才出现。
_VALID_SLOTS = (0, 8)

# review finding 2：use_quick_slot slot=9/-1 探针必须打在**已绑定可用物品**的槽位上，
# 否则空槽 no-op 路径会让「错误接受越界 slot、clamp 到边界槽、再走空槽 no-op」的实现
# 无任何可观测副作用地通过全部干净拒绝断言。先 give 回元丹并绑定到边界槽 0/8 —— 若
# 实现错误 clamp 9→8 / -1→0，会命中已绑定且仍在背包的物品并启动施法 → cast_sync 被
# 探针窗口标记（cast_sync 不在 ambient 集合，任何出现即视为副作用）。
_PILL_TEMPLATE = "huiyuan_pill"
_PILL_GIVE_COUNT = 2
_BOUND_SLOTS = (0, 8)


def _bind_slot_with_item(bot, slot: int, label: str) -> None:
    """quick_slot_bind 绑定 item_id 到槽位并等 ack。

    与 ``_assert_slot_boundary_accepted`` 相对：那里用 item_id=None **清空**槽位作
    正向边界探针；这里用真实物品**填满**槽位，让后续越界 use_quick_slot 探针打在
    非空、可用、会触发施法的状态上（review finding 2 的探测前提）。
    """
    request_id = f"rng-bind-slot{slot}"
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {
            "v": 1,
            "type": "quick_slot_bind",
            "slot": slot,
            "item_id": _PILL_TEMPLATE,
            "request_id": request_id,
        }
    )
    bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data.get("payload_type") == "quickslot_config"
        and e.t > sent_at
        and e.data["payload"].get("ack_request_id") == request_id
        and e.data["payload"].get("bind_accepted") is True,
        timeout=10.0,
        description=(
            f"{label}：quick_slot_bind 绑定 {_PILL_TEMPLATE} 到 slot={slot}"
            f"（ack_request_id={request_id} 回显 + bind_accepted）"
        ),
    )


def _bind_pill_to_slots(bot) -> None:
    """give 回元丹并绑定到边界槽 0/8（review finding 2 的探测前提）。"""
    bot.cmd(f"give {_PILL_TEMPLATE} {_PILL_GIVE_COUNT}")
    bot.expect_chat(f"[dev] gave {_PILL_TEMPLATE} x{_PILL_GIVE_COUNT}", timeout=10.0)
    wait_inventory_contains(bot, _PILL_TEMPLATE, timeout=10.0)
    for slot in _BOUND_SLOTS:
        _bind_slot_with_item(bot, slot, f"绑定回元丹 slot={slot}")

# 契约边界内**合法** count 值（deserialize_block_picker_count 契约 1..=64）。
# block_picker_give 的 [dev] 聊天回应证明 serde 接受了该 count（fixture Survival 得到
# "requires Creative mode"，Creative 得到 "gave ..."）。
_VALID_COUNTS = (1, 64)


def _assert_slot_boundary_accepted(bot, slot: int, label: str) -> None:
    request_id = f"rng-slot{slot}"
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {
            "v": 1,
            "type": "quick_slot_bind",
            "slot": slot,
            "item_id": None,
            "request_id": request_id,
        }
    )
    bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data.get("payload_type") == "quickslot_config"
        and e.t > sent_at
        and e.data["payload"].get("ack_request_id") == request_id
        and e.data["payload"].get("bind_accepted") is True,
        timeout=10.0,
        description=(
            f"{label}：server 回推 quickslot_config ack"
            f"（ack_request_id={request_id} 回显 + bind_accepted）"
        ),
    )


def _assert_count_boundary_accepted(bot, count: int, label: str) -> None:
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {
            "v": 1,
            "type": "block_picker_give",
            "block_id": "stone_bricks",
            "count": count,
        }
    )
    bot.wait_for(
        lambda e: e.kind == "chat"
        and e.t > sent_at
        and (
            "requires Creative mode" in e.data["text"]
            or "gave " in e.data["text"]
        ),
        timeout=10.0,
        description=(
            f"{label}：block_picker_give 进入 handler"
            f"（[dev] 聊天回应证明 serde 接受 count={count}）"
        ),
    )


def run(env) -> None:
    from ._inventory_helpers import latest_inventory_snapshot, wait_join_and_inventory
    from ._rejection_helpers import (
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
        inventory_fingerprint,
    )

    with env.new_bot("Rng") as bot:
        wait_join_and_inventory(bot)

        # review finding 2：先建立「越界 use_quick_slot 命中已绑定物品会施法」的可观测
        # 状态，再取指纹基线 —— give 会 bump revision，基线若取在 give 前会把 give 的
        # revision 变化误判成探针副作用。绑定本身不改变背包指纹（QuickSlotBindings 是
        # 独立组件），故绑定后取基线仍与「绑定前背包内容」一致。
        _bind_pill_to_slots(bot)
        pre = latest_inventory_snapshot(bot)
        pre_fingerprint = inventory_fingerprint(pre)

        probes = [
            (label, lambda req=req: bot.intent(req))
            for label, req in OUT_OF_RANGE_PROBES
        ]
        fire_probes_and_keep_connection(
            bot, "越界字段值", probes, baseline_snapshot=pre
        )

        # 背包状态零变化：探针后最新快照指纹（revision + 内容）必须与探针前一致。
        # 任何 slot/count 被 clamp 后继续执行（哪怕只改动一处状态）都会 bump revision。
        post = latest_inventory_snapshot(bot)
        post_fingerprint = inventory_fingerprint(post)
        if post_fingerprint != pre_fingerprint:
            raise BotAssertionError(
                "越界字段值探针后背包快照指纹变化：某个越界请求被 clamp/部分处理了，"
                f"探针前={pre_fingerprint} 探针后={post_fingerprint}"
            )

        # ---- 合法边界正向探针：slot 0/8、count 1/64 必须被 schema 接受（review
        # finding 1）。坏探针全被拒不足以证明好边界被接受 —— off-by-one 实现会拒掉
        # 全部坏探针却拒绝上边界合法请求。
        for slot in _VALID_SLOTS:
            _assert_slot_boundary_accepted(bot, slot, f"合法边界 slot={slot}")
        for count in _VALID_COUNTS:
            _assert_count_boundary_accepted(bot, count, f"合法边界 count={count}")

        assert_valid_request_still_works(bot)
