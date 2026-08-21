"""灵木真实采伐：满包落地、freshness wire 保真并可拾回。"""

import re
import time

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import (
    equip_location,
    find_item,
    send_move,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)

DESCRIPTION = "DiggingEvent 采伐灵木：满包落地 freshness 完整，清包后同实例可拾回"
MODULES = ["spiritwood", "inventory", "worldgen"]

TREE_LOG_PATTERN = re.compile(
    r"Teleported to spirit tree log at \((-?\d+), (-?\d+), (-?\d+)\)\."
)
# Bot gate 过载实测可降至 2 TPS：240 production tick 需 120 秒。
# 额外留出 chunk/worldgen stall 余量，但 terminal payload 断言不降级。
LUMBER_TERMINAL_TIMEOUT_SECONDS = 180.0


def _latest_drops(bot, timeout: float = 10.0) -> list[dict]:
    observed = [
        event.data["payload"]
        for event in bot.events_of("server_data")
        if event.data.get("payload_type") == "dropped_loot_sync"
    ]
    if observed:
        return observed[-1]["drops"]
    return bot.expect_server_data("dropped_loot_sync", timeout=timeout).data["payload"][
        "drops"
    ]


def _inventory_count(snapshot: dict, item_id: str) -> int:
    return sum(
        placed["item"]["stack_count"]
        for placed in snapshot.get("placed_items", [])
        if placed["item"]["item_id"] == item_id
    )


def run(env) -> None:
    with env.new_bot("WoodDrop") as bot:
        initial = wait_join_and_inventory(bot)
        baseline_drop_ids = {drop["instance_id"] for drop in _latest_drops(bot)}

        # 新手 loadout 自带出生剑；`clearinv all` 按契约保留装备槽。先清空
        # 携带面，把出生剑移入暗袋，再清一次携带面，既腾出主手又保留有效
        # worn 背包容器。`clearinv naked` 会权威移除装备、动态 pack 拓扑与
        # 容量加成，不适合后续“填满全部随身格”的采伐测试。
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        after_first_clear = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "inventory_snapshot"
            and event.data["payload"].get("placed_items") == []
            and (
                (found := find_item(event.data["payload"], "iron_sword")) is not None
                and found["location"] == equip_location("main_hand", "held")
            ),
            timeout=10.0,
            description="首次 clearinv all 后出生剑仅留在装备槽",
        ).data["payload"]
        starter_sword = find_item(after_first_clear, "iron_sword")
        assert starter_sword is not None
        send_move(
            bot,
            starter_sword["item"]["instance_id"],
            starter_sword["location"],
            {"kind": "container", "container_id": "body_pocket", "row": 0, "col": 0},
        )
        bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "inventory_snapshot"
            and event.data["payload"].get("equipped", {}).get("main_hand_held") is None,
            timeout=10.0,
            description="出生剑移入暗袋后主手为空",
        )
        clear_anchor = last_event_time(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        empty = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "inventory_snapshot"
            and event.t > clear_anchor
            and not event.data["payload"].get("placed_items")
            and find_item(event.data["payload"], "axe_iron") is None,
            timeout=10.0,
            description="clearinv all 后的空 inventory_snapshot",
        ).data["payload"]
        assert empty["revision"] > initial["revision"]

        bot.cmd("give axe_iron 1")
        bot.expect_chat("[dev] gave axe_iron x1", timeout=10.0)
        axe_snapshot = wait_inventory_revision_after_matching(
            bot,
            empty["revision"],
            lambda snapshot: find_item(snapshot, "axe_iron") is not None,
            "出现 axe_iron",
        )
        axe = find_item(axe_snapshot, "axe_iron")
        assert axe is not None
        send_move(
            bot,
            axe["item"]["instance_id"],
            axe["location"],
            equip_location("main_hand", "held"),
        )
        equipped = wait_inventory_revision_after_matching(
            bot,
            axe_snapshot["revision"],
            lambda snapshot: (
                (found := find_item(snapshot, "axe_iron")) is not None
                and found["location"] == equip_location("main_hand", "held")
            ),
            "axe_iron 已装备到 main_hand held",
        )

        bot.cmd("give grass_fiber 144")
        bot.expect_chat("[dev] gave grass_fiber x144", timeout=10.0)
        first_fill = wait_inventory_revision_after_matching(
            bot,
            equipped["revision"],
            lambda snapshot: _inventory_count(snapshot, "grass_fiber") == 144,
            "第一批 grass_fiber 填满主包",
        )
        bot.cmd("give grass_fiber 96")
        bot.expect_chat("[dev] gave grass_fiber x96", timeout=10.0)
        full = wait_inventory_revision_after_matching(
            bot,
            first_fill["revision"],
            lambda snapshot: _inventory_count(snapshot, "grass_fiber") == 240,
            "两批 grass_fiber 填满全部 15 个随身格",
        )
        assert find_item(full, "axe_iron")["location"] == equip_location(
            "main_hand", "held"
        )

        teleport_anchor = last_event_time(bot)
        bot.cmd("tptree spirit")
        teleport_chat = bot.wait_for(
            lambda event: event.kind == "chat"
            and event.t > teleport_anchor
            and TREE_LOG_PATTERN.search(event.data["text"]) is not None,
            timeout=20.0,
            description="/tptree spirit 回报生产 mega_tree 算出的真实树干坐标",
        )
        match = TREE_LOG_PATTERN.search(teleport_chat.data["text"])
        assert match is not None
        log_pos = tuple(int(value) for value in match.groups())
        assert log_pos == (1285, 73, 1509), (
            "bot fixture 的 production SpiritWood 外缘主干坐标漂移："
            f"期望 (1285, 73, 1509)，实际 {log_pos}"
        )
        teleport_position = bot.wait_for(
            lambda event: event.kind == "pos_look" and event.t > teleport_chat.t,
            timeout=20.0,
            description="server 权威传送到真实 SpiritWood 树干外缘",
        ).data
        assert abs(teleport_position["x"] - (log_pos[0] + 0.5)) <= 2.5, (
            f"外缘站位 x 应贴近 log={log_pos}，实际 {teleport_position}"
        )
        assert abs(teleport_position["z"] - (log_pos[2] + 0.5)) <= 2.5, (
            f"外缘站位 z 应贴近 log={log_pos}，实际 {teleport_position}"
        )
        assert abs(teleport_position["y"] - log_pos[1]) <= 1.5, (
            f"外缘站位 y 应落在真实地表，实际 {teleport_position}"
        )
        # 站在树干内部会被碰撞校正持续向上推；外缘站位必须在短窗口内保持稳定。
        time.sleep(0.75)
        recent_positions = [
            event.data
            for event in bot.events_of("pos_look")
            if event.t > teleport_anchor
        ]
        if recent_positions:
            settled = recent_positions[-1]
            assert all(
                abs(settled[key] - teleport_position[key]) < 0.01
                for key in ("x", "y", "z")
            ), f"树干外缘站位不应被物理校正移动，实际 {recent_positions[-3:]}"
        target_chunk = (log_pos[0] // 16, log_pos[2] // 16)
        bot.wait_for(
            lambda event: event.kind == "chunk_data"
            and event.t > teleport_anchor
            and (event.data["x"], event.data["z"]) == target_chunk,
            timeout=30.0,
            description=f"SpiritWood 所在 chunk={target_chunk} 已按生产 worldgen 下发",
        )

        dig_anchor = last_event_time(bot)
        bot.start_digging(*log_pos, sequence=1)
        bot.wait_for(
            lambda event: event.kind == "player_action_response"
            and event.t > dig_anchor
            and event.data["sequence"] == 1,
            timeout=5.0,
            description="server 已解码真实 C2S_PLAYER_ACTION 并回 ACK sequence=1",
        )
        terminal = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "lumber_progress"
            and event.t > dig_anchor
            and event.data["payload"]["log_pos"] == list(log_pos)
            and (
                event.data["payload"]["completed"]
                or event.data["payload"]["interrupted"]
            ),
            timeout=LUMBER_TERMINAL_TIMEOUT_SECONDS,
            description="真实 DiggingEvent 经过 240 tick 后的 lumber_progress terminal",
        ).data["payload"]
        assert terminal["completed"] is True, f"采伐应成功完成，实际 {terminal!r}"
        assert terminal["interrupted"] is False, f"采伐不应被打断，实际 {terminal!r}"
        assert terminal["progress"] == 1.0
        assert "背包已满，灵木原木已落地" in terminal["detail"]

        drops = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "dropped_loot_sync"
            and event.t > dig_anchor
            and len(
                [
                    drop
                    for drop in event.data["payload"]["drops"]
                    if drop["instance_id"] not in baseline_drop_ids
                    and drop["item"]["item_id"] == "ling_mu_gun"
                ]
            )
            == 1,
            timeout=10.0,
            description="满包采伐后的唯一新 ling_mu_gun 地面掉落",
        ).data["payload"]["drops"]
        drop = next(
            drop
            for drop in drops
            if drop["instance_id"] not in baseline_drop_ids
            and drop["item"]["item_id"] == "ling_mu_gun"
        )
        assert drop["world_pos"] == [
            log_pos[0] + 0.5,
            log_pos[1] + 0.5,
            log_pos[2] + 0.5,
        ]
        assert drop["item"]["instance_id"] == drop["instance_id"]
        assert 2 <= drop["item"]["stack_count"] <= 4
        assert abs(drop["item"]["spirit_quality"] - 0.9) < 1e-9
        freshness = drop["item"]["freshness"]
        assert freshness is not None, "ling_mu_gun 地面掉落必须携带 freshness"
        assert set(freshness) == {
            "created_at_tick",
            "initial_qi",
            "track",
            "profile",
            "frozen_accumulated",
            "frozen_since_tick",
        }
        assert freshness["created_at_tick"] > 0, (
            "真实采伐至少经过 240 tick，created_at_tick=0 表示 protobuf tag 1 未过线"
        )
        assert abs(freshness["initial_qi"] - 100.0) < 1e-6
        assert freshness["track"] == "Decay"
        assert freshness["profile"] == "ling_mu_gun_v1"
        assert freshness["frozen_accumulated"] == 0
        assert freshness["frozen_since_tick"] is None

        pickup_id = drop["instance_id"]
        clear_drop_anchor = last_event_time(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "inventory_snapshot"
            and event.t > clear_drop_anchor
            and not event.data["payload"].get("placed_items"),
            timeout=10.0,
            description="拾取前 clearinv all 已腾出随身空间",
        )

        pickup_anchor = last_event_time(bot)
        bot.intent(
            {"type": "pickup_dropped_item", "v": 1, "instance_id": pickup_id}
        )
        bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "dropped_loot_sync"
            and event.t > pickup_anchor
            and pickup_id
            not in {
                candidate["instance_id"]
                for candidate in event.data["payload"]["drops"]
            },
            timeout=10.0,
            description="拾取后 dropped_loot_sync 移除同一 instance_id",
        )
        picked_snapshot = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "inventory_snapshot"
            and event.t > pickup_anchor
            and (
                (found := find_item(event.data["payload"], "ling_mu_gun")) is not None
                and found["item"]["instance_id"] == pickup_id
            ),
            timeout=10.0,
            description="拾取后 inventory_snapshot 出现同一 ling_mu_gun instance_id",
        ).data["payload"]
        picked = find_item(picked_snapshot, "ling_mu_gun")
        assert picked is not None
        assert picked["item"]["stack_count"] == drop["item"]["stack_count"]
        assert picked["item"]["freshness"] == freshness, (
            "地面掉落拾回后 freshness 六字段必须逐项保真"
        )
        bot.assert_alive("灵木满包落地并拾回完成")
