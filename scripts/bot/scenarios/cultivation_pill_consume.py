"""使用丹药：huiyuan_pill 双入口（template / instance）吃丹 → qi_current 回升 + 扣存。

黑盒契约面：
- 两条 C2S 都要锁（client_request.rs）：
  ① `alchemy_take_pill{pill_item_id}`（按 template_id）
  ② `apply_pill{instance_id, target:{kind:"self"}}`（按 inventory instance）
  两者汇入 handle_alchemy_take_pill，效果一致。
- huiyuan_pill effect=qi_recovery magnitude=60（assets/items/pills.toml）：
  吃后 Cultivation.qi_current 上升（clamp 到 qi_max），经 cultivation 快照
  （server_data）可观察；inventory 中丹 -1。
- 负分支：吃不存在的丹（背包无此 template）不得踢线/panic（宽容红线）。
"""

import json
import time

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = "回元丹双入口吃丹：qi_current 回升可观察 + 丹扣存 + 空丹宽容"
MODULES = ["alchemy", "cultivation", "inventory"]

PILL_ID = "huiyuan_pill"


def _qi_current_after(bot, anchor: float, timeout: float = 10.0) -> float:
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.t > anchor
        and _extract_qi(e.data["payload"]) is not None,
        timeout=timeout,
        description="吃丹后应收到携带 qi_current 的状态快照（真元回升可观察）",
    )
    return _extract_qi(event.data["payload"])


def _extract_qi(node):
    if isinstance(node, dict):
        for key, value in node.items():
            if key == "qi_current" and isinstance(value, (int, float)):
                return float(value)
            got = _extract_qi(value)
            if got is not None:
                return got
    elif isinstance(node, list):
        for value in node:
            got = _extract_qi(value)
            if got is not None:
                return got
    return None


def run(env) -> None:
    with env.new_bot("Pill") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd("qi max 100")
        bot.cmd("qi set 5")
        bot.cmd(f"give {PILL_ID} 2")
        wait_inventory_contains(bot, PILL_ID)

        # ── 入口①：alchemy_take_pill（template_id 路径）──────────
        anchor = last_event_time(bot)
        bot.intent({"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID})
        qi_after_first = _qi_current_after(bot, anchor)
        assert qi_after_first > 5.0, (
            f"回元丹（qi_recovery magnitude=60）吃下后 qi_current 应从 5 显著回升，"
            f"实际 {qi_after_first}——效果链断或快照未 resync"
        )

        # ── 入口②：apply_pill（instance_id 路径）─────────────────
        bot.cmd("qi set 5")
        time.sleep(0.5)
        snapshot = latest_inventory_snapshot(bot)
        pill = require_item(snapshot, PILL_ID)
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "apply_pill",
                "v": 1,
                "instance_id": int(pill["item"]["instance_id"]),
                "target": {"kind": "self"},
            }
        )
        qi_after_second = _qi_current_after(bot, anchor)
        assert qi_after_second > 5.0, (
            f"apply_pill(instance) 路径同样应回真元，实际 {qi_after_second}——"
            f"双入口只修一条是半截修复"
        )

        # 两丹吃完：inventory 不应再有 huiyuan_pill
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and find_item(e.data["payload"], PILL_ID) is None,
            timeout=10.0,
            description=(
                "两枚回元丹吃完后 inventory 应扣存为 0——不扣存 = 无限白嫖丹药"
            ),
        )

        # ── 负分支：背包无丹再吃（宽容不踢）──────────────────────
        bot.intent({"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID})
        time.sleep(1.0)
        bot.assert_alive("空丹重复吃之后（宽容红线：不得踢线/panic）")
