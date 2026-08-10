"""freshness_probe 保鲜探针路径（实体/空间探知流 M4a，plan-exploration-probe-return-v1）。

resolve_one_probe（shelflife/probe.rs:101）检查顺序：
1. 修为 < 凝脉（MIN_PROBE_REALM_RANK=2）→ Denied(RealmTooLow) → EventAlert
   「神识未及，凝脉方可感知保鲜」；
2. item 无 freshness → Denied(NoFreshness) → 静默（freshness_probe_emit 对
   NoFreshness 一律 continue 不发 S2C）；
3. 通过 → Precise → `FreshnessUpdateV1 { item_uuid, freshness, profile_name }`
   （freshness = current_qi/initial_qi，新物品 = 1.0）。

dispatch 前置：instance_id 不在玩家背包 → 静默丢弃（client_request_handler
belongs_to_player 检查）。本场景用 `[dev] give` 构造合法背包 item：

1. Awaken 探煮熟肉（food.mundane.cooked_meat，shelflife_profile=
   food_spoil_mundane_meat_v1）→ event_alert 神识未及；
2. 凝脉后再探 → freshness_update（item_uuid=instance_id、freshness=1.0、
   profile_name=food_spoil_mundane_meat_v1）；
3. 凝脉探无保鲜 item（trade_crate）→ NoFreshness 静默（无 S2C、无聊天）。
"""

import time

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    require_item,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_join_and_inventory,
)

DESCRIPTION = "freshness_probe：Awaken→神识未及告警、凝脉→FreshnessUpdate、无保鲜→静默"
MODULES = ["shelflife", "network"]

PROBE_REQUEST = {"type": "freshness_probe", "v": 1}
MEAT_ITEM = "food.mundane.cooked_meat"
MEAT_PROFILE = "food_spoil_mundane_meat_v1"
PLAIN_ITEM = "trade_crate"
SILENT_WINDOW = 4.0
# food_spoil_mundane_meat_v1：Linear 衰减 decay_per_tick = 1/(GAME_DAY_TICKS×3)，
# GAME_DAY_TICKS=24000、TICKS_PER_SECOND=20 → 2.78e-4/s（server/src/shelflife/registry.rs）。
MEAT_DECAY_PER_SECOND = 1.0 / (24000 * 3) * 20.0
FRESHNESS_MARGIN = 0.005


def run(env) -> None:
    with env.new_bot("FpH") as bot:
        snapshot = wait_join_and_inventory(bot)
        revision = snapshot["revision"]

        bot.cmd(f"give {MEAT_ITEM} 1")
        bot.expect_chat(f"[dev] gave {MEAT_ITEM} x1", timeout=10.0)
        give_anchor = time.monotonic()
        snapshot = wait_inventory_contains(bot, MEAT_ITEM, timeout=10.0)
        meat = require_item(snapshot, MEAT_ITEM)
        meat_instance = meat["item"]["instance_id"]

        # 1. Awaken → RealmTooLow → EventAlert 神识未及
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        alert = bot.expect_server_data("event_alert", timeout=10.0)
        message = alert.data["payload"].get("message", "")
        if "神识未及" not in message:
            raise BotAssertionError(
                f"[{bot.username}] 期望 EventAlert 含「神识未及」，实际 {message!r}"
            )
        bot.assert_alive("Awaken 保鲜探针后")

        # 2. 凝脉 → FreshnessUpdate 精确结果
        bot.cmd("realm set condense")
        bot.expect_chat("[dev] realm set ", timeout=10.0)
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        update = bot.expect_server_data("freshness_update", timeout=10.0)
        payload = update.data["payload"]
        if str(payload.get("item_uuid")) != str(meat_instance):
            raise BotAssertionError(
                f"[{bot.username}] 期望 FreshnessUpdate.item_uuid={meat_instance}，"
                f"实际 {payload.get('item_uuid')}"
            )
        if payload.get("profile_name") != MEAT_PROFILE:
            raise BotAssertionError(
                f"[{bot.username}] 期望 FreshnessUpdate.profile_name={MEAT_PROFILE}，"
                f"实际 {payload.get('profile_name')}"
            )
        freshness = payload.get("freshness")
        # 保鲜是线性衰减（decay_per_tick=1/(24000×3)/tick，20 ticks/s → 2.78e-4/s）。
        # give→probe 窗口由墙钟实测，下限 = 1 - 窗口×衰减率 - 浮点余量。原断言
        # 放宽到 0.5 会把硬编码 0.5 / 立即 50% 衰减的坏实现放过去——下限必须由
        # canonical 衰减计算界定。
        elapsed = time.monotonic() - give_anchor
        max_decay = MEAT_DECAY_PER_SECOND * elapsed
        lower = 1.0 - max_decay - FRESHNESS_MARGIN
        if freshness is None or not (lower <= float(freshness) <= 1.001):
            raise BotAssertionError(
                f"[{bot.username}] 期望新物品 freshness≈1.0（give→probe "
                f"{elapsed:.1f}s，canonical 衰减下限 {lower:.4f}），实际 {freshness}"
            )
        bot.assert_alive("凝脉保鲜探针后")

        # 3. 凝脉探无保鲜 item（trade_crate）→ NoFreshness 静默
        #    先清空背包：此前 give 的 meat + 出生物品已占满包，trade_crate 直接
        #    give 会被拒（回显 `give trade_crate failed: inventory full` 而非
        #    `gave ... x1`，expect_chat 超时）。clearinv 腾位后再 give。
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)
        bot.cmd(f"give {PLAIN_ITEM} 1")
        bot.expect_chat(f"[dev] gave {PLAIN_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)
        plain = require_item(snapshot, PLAIN_ITEM)
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent({**PROBE_REQUEST, "instance_id": plain["item"]["instance_id"]})
        _assert_no_freshness_update(bot, sent_at, "无保鲜 item 的探针应静默（NoFreshness 不发 S2C）")
        bot.assert_alive("freshness_probe 拒绝面全程")


def _assert_no_freshness_update(bot, sent_at: float, description: str) -> None:
    # 截止时刻用单调钟（time.monotonic），不用事件时间戳 bot.events[-1].t：
    # 静默断言正是"之后无事件到达"，事件时间不会推进，以事件时间做 deadline 会
    # 永远等不到 now >= end_at 而死循环（review finding 1/5）。
    deadline = time.monotonic() + SILENT_WINDOW
    while True:
        for e in bot.events_of("server_data"):
            if e.t > sent_at and e.data["payload_type"] == "freshness_update":
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际收到 freshness_update（t={e.t:.3f}）"
                )
        for e in bot.events_of("chat"):
            if e.t > sent_at:
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
                )
        if time.monotonic() >= deadline:
            return
        bot.assert_alive(f"{description} 窗口内连接保持")
        time.sleep(0.1)
