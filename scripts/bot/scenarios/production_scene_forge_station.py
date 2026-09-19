"""真实测试砧的创建、开窗请求、锻造中清理保护与空闲清理。"""

import re

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import (
    find_item,
    wait_inventory_contains,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)
from bot.scenarios._craft_helpers import stage_material
from bot.scenarios.production_forge_station_real_place import _wait_forge_payload_after

DESCRIPTION = "测试场景放置真实砧，开窗与锻造走生产协议，禁止清掉活动炉次"
MODULES = ["forge", "scene"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BONG_TEST_ENV"


def run(env):
    with env.new_bot("FgScene") as bot:
        wait_join_and_inventory(bot)
        assert bot.position is not None, "场景需要服务端同步的玩家位置"
        bot.cmd("spawn")
        bot.expect_chat("Teleported to spawn.", timeout=15)
        # /top 在 raster 世界传送到地表上方三格；无地形数据时只向上移动
        # 24 格。机器人没有重力，须逐层下降，由 scene 的真实地面校验确认落地。
        # teleport confirm 与移动同批到达时 Valence 可能忽略移动；逐帧下降到
        # 地表上方一格，不能用只修改本地 position 的单次 set_position 当作落地。
        anchor = last_event_time(bot)
        bot.cmd("top")
        top = bot.wait_for(
            lambda e: e.kind == "chat" and e.t > anchor
            and "Teleported to top at Y=" in e.data["text"],
            timeout=15, description="测试角色移至地表上方",
        )
        top_y = int(re.search(r"Y=(-?\d+)", top.data["text"]).group(1))
        position = bot.wait_for(
            lambda e: e.kind == "pos_look" and e.t > anchor and e.data["y"] == top_y,
            timeout=15, description="等待 /top 权威位置与传送确认",
        ).data
        for depth in range(2, 35):
            bot.move_to(position["x"], top_y - depth, position["z"])
            anchor = last_event_time(bot)
            bot.cmd("scene test_forge_station_1")
            ready = bot.wait_for(
                lambda e: e.kind == "chat" and e.t > anchor and "[scene]" in e.data["text"],
                timeout=30, description="scene 必须返回场地准备结果",
            )
            if "身边没有安全的空位" not in ready.data["text"]:
                break
        assert "炼器砧已就绪" in ready.data["text"], ready.data["text"]
        match = re.search(r"\[(-?\d+), (-?\d+), (-?\d+)\]", ready.data["text"])
        assert match, "场景必须报告真实工位坐标"
        pos = [int(value) for value in match.groups()]
        anchor = last_event_time(bot)
        bot.intent({"type": "forge_station_open", "v": 1, "station_pos": pos})
        _wait_forge_payload_after(bot, anchor, "forge_station", lambda p: p["open_screen"],
                                 30, "测试砧必须响应交互开窗请求")
        _wait_forge_payload_after(bot, anchor, "forge_blueprint_book",
                                 lambda p: any(entry["id"] == "iron_sword_v0" for entry in p["learned"]),
                                 30, "新角色也应拿到场景测试图谱")
        bot.cmd("give fan_tie 3")
        wait_inventory_contains(bot, "fan_tie")
        prepared = stage_material(bot, "iron_sword_v0", "fan_tie", pos)
        original_id = prepared["material_preparation"]["materials"][0]["instance_id"]
        bot.intent({"type": "material_move", "v": 1, "recipe_id": "iron_sword_v0",
                    "instance_id": None, "station_pos": pos, "returning": True,
                    "expected_revision": prepared["revision"]})
        returned = wait_inventory_revision_after_matching(
            bot, prepared["revision"],
            lambda state: not state["material_preparation"]["materials"] and find_item(state, "fan_tie") is not None,
            "开炉前关闭准备窗口必须把材料返还背包",
        )
        assert find_item(returned, "fan_tie")["item"]["instance_id"] == original_id, "返还不能重造实例"
        stage_material(bot, "iron_sword_v0", "fan_tie", pos)
        anchor = last_event_time(bot)
        bot.intent({"type": "forge_start_session", "v": 1, "station_pos": pos,
                    "blueprint_id": "iron_sword_v0", "materials": [["fan_tie", 3]]})
        session = _wait_forge_payload_after(bot, anchor, "forge_session", lambda p: p["active"],
                                           30, "真实投料必须成功开炉").data["payload"]["session_id"]
        bot.intent({"type": "material_move", "v": 1, "recipe_id": "iron_sword_v0",
                    "instance_id": None, "station_pos": pos, "returning": True,
                    "expected_revision": 0})
        bot.expect_chat("已经投入炉次", timeout=30)
        bot.cmd("scene clear")
        bot.expect_chat("测试砧仍在锻造", timeout=30)
        anchor = last_event_time(bot)
        bot.intent({"type": "forge_station_open", "v": 1, "station_pos": pos})
        _wait_forge_payload_after(bot, anchor, "forge_session",
                                 lambda p: p["active"] and p["session_id"] == session,
                                 30, "清理被拒后原炉次必须可以恢复")
        anchor = last_event_time(bot)
        bot.intent({"type": "forge_step_advance", "v": 1, "session_id": session})
        _wait_forge_payload_after(bot, anchor, "forge_outcome",
                                 lambda p: p["session_id"] == session and p["weapon_item"] == "iron_sword",
                                 30, "测试砧必须完成真实结算")
        bot.cmd("scene clear")
        bot.expect_chat("已清空状态效果", timeout=30)
        bot.intent({"type": "forge_station_open", "v": 1, "station_pos": pos})
        bot.expect_chat("工位不存在或无权使用", timeout=30)
