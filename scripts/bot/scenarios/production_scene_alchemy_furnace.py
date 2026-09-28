"""验收 /scene 的实际注册、真实丹炉创建、材料发放与交互开窗协议。"""

import re

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import (
    wait_inventory_contains,
    wait_join_and_inventory,
)

DESCRIPTION = "炼丹测试场景必须备齐材料和区域灵气，允许真实交互开窗并起炉"
MODULES = ["alchemy", "scene"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BONG_TEST_ENV"


def run(env):
    with env.new_bot("AlScene") as bot:
        wait_join_and_inventory(bot)
        bot.cmd("scene list")
        bot.expect_chat("test_alchemy_furnace_1", timeout=15)

        anchor = last_event_time(bot)
        bot.cmd("scene test_alchemy_furnace_1")
        ready = bot.wait_for(
            lambda event: event.kind == "chat"
            and event.t > anchor
            and "[scene]" in event.data["text"],
            timeout=15,
            description="/scene 必须响应炼丹场景创建请求",
        )
        assert "炼丹炉已就绪" in ready.data["text"], ready.data["text"]
        match = re.search(r"\[(-?\d+), (-?\d+), (-?\d+)\]", ready.data["text"])
        assert match, "场景必须报告真实丹炉坐标，不能只有成功提示"
        pos = [int(value) for value in match.groups()]
        wait_inventory_contains(bot, "spirit_grass")
        wait_inventory_contains(bot, "incense_plain")

        # G 键最终发送同一请求；必须收到该炉位的权威快照，不能只看成功 chat。
        anchor = last_event_time(bot)
        bot.intent({"type": "alchemy_open_furnace", "v": 1, "furnace_pos": pos})
        furnace = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.t > anchor
            and event.data["payload_type"] == "alchemy_furnace"
            and event.data["payload"]["pos"] == pos,
            timeout=15,
            description="生成的真实丹炉必须响应工位交互请求",
        ).data["payload"]
        assert furnace["owner_name"] == bot.username, "场景丹炉必须归当前验收玩家使用"

        anchor = last_event_time(bot)
        bot.intent({
            "type": "alchemy_ignite", "v": 1,
            "furnace_pos": pos, "recipe_id": "ling_xi_wan_v1",
        })
        bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.t > anchor
            and event.data["payload_type"] == "alchemy_session"
            and event.data["payload"]["active"],
            timeout=15,
            description="测试场景必须补齐区域灵气，不能在真实起炉时被灵气门槛拒绝",
        )
        anchor = last_event_time(bot)
        bot.intent({
            "type": "alchemy_take_back", "v": 1,
            "furnace_pos": pos, "slot_idx": 0,
        })
        bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.t > anchor
            and event.data["payload_type"] == "alchemy_outcome_resolved",
            timeout=15,
            description="清理测试炉前先结束本次验收炉次",
        )
        bot.cmd("scene clear")
        bot.expect_chat("已清空状态效果", timeout=15)
