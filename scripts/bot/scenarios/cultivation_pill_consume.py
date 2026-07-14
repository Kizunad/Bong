"""使用丹药：huiyuan_pill 双入口（template / instance）吃丹 → qi_current 回升 + 扣存。

黑盒契约面：
- 两条 C2S 都要锁（client_request.rs）：
  ① `alchemy_take_pill{pill_item_id}`（按 template_id）
  ② `apply_pill{instance_id, target:{kind:"self"}}`（按 inventory instance）
  两者汇入 handle_alchemy_take_pill，效果一致。
- huiyuan_pill effect=qi_recovery magnitude=60（assets/items/pills.toml）：
  吃后 Cultivation.qi_current 上升且 **clamp 到 qi_max**（qi=95 吃 60 → 100
  专属边界用例），经快照可观察；inventory 中丹 -1。
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


def _qi_current_after(
    bot, anchor: float, baseline: float, minimum: float, timeout: float = 10.0
) -> float:
    # 自然回气也会让 qi_current 轻微偏离 baseline；必须等到丹药产生显著恢复，
    # 不能把 95 → 95.00x 的心跳快照误判成 clamp 已完成。
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.t > anchor
        and (lambda q: q is not None and q >= minimum)(
            _extract_qi(e.data["payload"])
        ),
        timeout=timeout,
        description=(
            f"吃丹后应收到 qi_current 从基线 {baseline} 恢复到至少 {minimum} 的状态快照"
        ),
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
        bot.cmd(f"give {PILL_ID} 3")
        wait_inventory_contains(bot, PILL_ID)

        # ── 入口①：alchemy_take_pill（template_id 路径）──────────
        anchor = last_event_time(bot)
        bot.intent({"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID})
        qi_after_first = _qi_current_after(bot, anchor, baseline=5.0, minimum=60.0)
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
        qi_after_second = _qi_current_after(bot, anchor, baseline=5.0, minimum=60.0)
        assert qi_after_second > 5.0, (
            f"apply_pill(instance) 路径同样应回真元，实际 {qi_after_second}——"
            f"双入口只修一条是半截修复"
        )

        # ── 边界：qi 接近上限时吃丹 clamp 到 qi_max，不得溢出 ────
        bot.cmd("qi set 95")
        time.sleep(0.5)
        anchor = last_event_time(bot)
        bot.intent({"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID})
        qi_after_clamp = _qi_current_after(bot, anchor, baseline=95.0, minimum=100.0)
        assert abs(qi_after_clamp - 100.0) < 1e-6, (
            f"qi=95 时吃回元丹（magnitude=60）应 clamp 到 qi_max=100，"
            f"实际 {qi_after_clamp}——溢出说明 recover_current_qi 边界回归"
        )

        # 三丹吃完：inventory 不应再有 huiyuan_pill
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and find_item(e.data["payload"], PILL_ID) is None,
            timeout=10.0,
            description=(
                "三枚回元丹吃完后 inventory 应扣存为 0——不扣存 = 无限白嫖丹药"
            ),
        )

        # ── 负分支：背包无丹再吃（宽容不踢）──────────────────────
        bot.intent({"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID})
        time.sleep(1.0)
        bot.assert_alive("空丹重复吃之后（宽容红线：不得踢线/panic）")
