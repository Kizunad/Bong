"""锻造可达面：真砧放置（真实 instance_id）+ 未接线 intent 宽容。

黑盒契约面与已知红旗：
- `forge_station_place{x,y,z,item_instance_id,station_tier}` 必须用真实
  instance_id——station.rs forge_station_item_by_instance 按 id 校验，
  旧场景传 0 是假放置（请求被静默丢弃却报 PASS）。
- **已知孤岛红旗（2026-07-07 库存实证）**：`forge_start_session` /
  `forge_blueprint_turn_page` 在 client_request_handler.rs 只打 debug log
  丢弃（"plan-forge-v1 client_request not yet wired"）——武器锻造全链路
  对协议黑盒是死路。本场景以宽容断言钉住现状：发未接线 intent 不得
  踢线/panic；等接线后此场景应升级为全链路（tempering→outcome→入包）。
- **观察面弱点（一并记录）**：放砧成功玩家侧无专属 S2C 回执，只能从
  inventory 扣除推断——违背「重要动作必有回执」，接线锻造时应补。
"""

import time

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = "真砧 instance_id 放置出 forge_station 快照 + 未接线 forge intent 宽容"
MODULES = ["forge", "inventory"]

ANVIL_ID = "fan_iron_anvil"


def run(env) -> None:
    with env.new_bot("Forge") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)

        bot.cmd(f"give {ANVIL_ID} 1")
        wait_inventory_contains(bot, ANVIL_ID)
        snapshot = latest_inventory_snapshot(bot)
        anvil = require_item(snapshot, ANVIL_ID)

        assert bot.position is not None
        px, py, pz = (int(v) for v in bot.position)

        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "forge_station_place",
                "v": 1,
                "x": px - 2,
                "y": py,
                "z": pz,
                "item_instance_id": int(anvil["item"]["instance_id"]),
                "station_tier": 1,
            }
        )
        # 放砧成功的可观察面 = 砧从背包被消耗（server 不给放置者发
        # forge_station 专属回执——观察面弱点，实测 2026-07-07 place_station ok
        # 只落 INFO log + inventory_changed）
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and find_item(e.data["payload"], ANVIL_ID) is None,
            timeout=10.0,
            description=(
                "真实 instance_id 放砧后砧应从背包消耗（forge_station_place_consumed）"
                "——不消耗说明 instance 校验没走通（旧场景传 0 即假放置）"
            ),
        )

        # 未接线 intent 宽容钉现状（接线后应升级为全链路场景）
        bot.intent(
            {
                "type": "forge_start_session",
                "v": 1,
                "station_id": 1,
                "blueprint_id": "iron_sword_v0",
                "materials": [["fan_tie", 3]],
            }
        )
        time.sleep(1.0)
        bot.assert_alive("发送未接线 forge_start_session 之后（宽容红线）")
