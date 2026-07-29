"""P4 生产系统：真锄开垦 + 野生灵草采集进度的黑盒链路。

黑盒契约面：
- `inventory_snapshot` 给出 `/give hoe_iron` 的真实 instance_id 与权威来源位置；
  `inventory_move_intent` 把同一实例移到 `main_hand/held`。
- fallback/raster 地表由服务器权威读取；场景在玩家附近尝试候选脚下格，只有收到
  `lingtian_session{active:true,kind:till,target_ticks:40}` 才算开垦受理，不能把
  占位 `hoe_instance_id=0` 或 inactive 心跳记为 P4 证据。
- `/bong gather spirit_grass` 必须进入真实 botany harvest session，并收到深解码的
  `gathering_session` / `botany_harvest_progress` 字段；raw server_data oneof 不算证据。
"""

import math

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import (
    equip_location,
    find_item,
    require_item,
    send_move,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)

DESCRIPTION = "灵田/采集：真锄装备→开垦 active session；/bong gather→深解码采集进度"
MODULES = ["lingtian", "gathering", "inventory"]

HOE_ID = "hoe_iron"
HERB_ID = "spirit_grass"


def _surface_candidates(bot) -> list[tuple[int, int, int]]:
    if bot.position is None:
        raise BotAssertionError("lingtian 场景需要 pos_look 后才能派生目标格")
    px = math.floor(bot.position[0])
    feet_y = math.floor(bot.position[1])
    pz = math.floor(bot.position[2])
    # 玩家脚下优先；spawn/recover 的权威脚点可能在 support 上方 1-2 格，故每个
    # 水平候选向下探三层。terrain 仍由 production handler 从 ChunkLayer 权威分类，
    # Bot 不读取或伪造 block kind。
    offsets = [
        (0, 0),
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
        (2, 0),
        (-2, 0),
        (0, 2),
        (0, -2),
    ]
    return [
        (px + dx, max(1, feet_y - depth), pz + dz)
        for dx, dz in offsets
        for depth in (1, 2, 3)
    ]


def _start_real_till(bot, hoe_iid: int) -> dict:
    for target in _surface_candidates(bot):
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "lingtian_start_till",
                "v": 1,
                "x": target[0],
                "y": target[1],
                "z": target[2],
                "hoe_instance_id": hoe_iid,
                "mode": "manual",
            }
        )
        try:
            event = bot.wait_for(
                lambda e: e.kind == "server_data"
                and e.data["payload_type"] == "lingtian_session"
                and e.t > anchor
                and e.data["payload"]["active"] is True
                and e.data["payload"]["kind"] == "till"
                and e.data["payload"]["pos"] == list(target),
                timeout=1.5,
                description=f"真实锄实例在候选地表 {target} 开启 till session",
            )
        except BotAssertionError:
            continue
        payload = event.data["payload"]
        if payload["kind"] != "till" or payload["pos"] != list(target):
            raise BotAssertionError(
                "lingtian_start_till 受理快照必须锁定 till 类型与请求坐标；"
                f"target={target} actual={payload}"
            )
        if payload["target_ticks"] != 40 or payload["elapsed_ticks"] > 40:
            raise BotAssertionError(
                "manual till 的权威进度应满足 target_ticks=40 且 elapsed<=target；"
                f"actual={payload}"
            )
        return payload
    raise BotAssertionError(
        "玩家附近候选地表均未开启 lingtian till session；"
        "真实 instance_id/主手装备/服务器 terrain 分类任一断链都会在此失败"
    )


def _wait_gather_progress(bot, after: float) -> dict:
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.t > after
        and (
            (
                e.data["payload_type"] == "gathering_session"
                and e.data["payload"]["target_type"] == "herb"
                and e.data["payload"].get("target_name") == HERB_ID
            )
            or (
                e.data["payload_type"] == "botany_harvest_progress"
                and e.data["payload"]["plant_kind"] == HERB_ID
                and e.data["payload"]["target_name"] == HERB_ID
            )
        ),
        timeout=15.0,
        description="/bong gather 后深解码 gathering/botany 进度",
    )
    payload = event.data["payload"]
    if payload["type"] == "gathering_session":
        if payload["target_type"] != "herb":
            raise BotAssertionError(f"spirit_grass 采集 target_type 应为 herb，实际 {payload}")
        if payload["total_ticks"] <= 0 or payload["progress_ticks"] > payload["total_ticks"]:
            raise BotAssertionError(f"采集 tick 区间必须有效，实际 {payload}")
    else:
        if payload["plant_kind"] != HERB_ID or payload["target_name"] != HERB_ID:
            raise BotAssertionError(f"采集进度必须绑定 spirit_grass，实际 {payload}")
        if not 0.0 <= payload["progress"] <= 1.0:
            raise BotAssertionError(f"botany progress 必须位于 [0,1]，实际 {payload}")
    return payload


def run(env) -> None:
    with env.new_bot("ProdLG") as bot:
        snapshot = wait_join_and_inventory(bot)

        # clearinv naked 才清装备；再 all 清掉卸入背包的出生物品。两条命令都以
        # 精确 revision +1 为前置门，避免第二条 clear 与后续 give 交错执行。
        bot.cmd("clearinv naked")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda candidate: all(
                not value
                for value in candidate.get("equipped", {}).values()
            ),
            "clearinv naked 后装备为空",
        )
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda candidate: not candidate.get("placed_items")
            and not any(candidate.get("hotbar", []))
            and not candidate.get("equipped", {}).get("main_hand_held"),
            "灵田准备阶段 carried surfaces 与 main_hand held 为空",
        )

        give_anchor = last_event_time(bot)
        bot.cmd(f"give {HOE_ID} 1")
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda candidate: find_item(candidate, HOE_ID) is not None,
            f"/give 后出现 {HOE_ID}",
        )
        hoe = require_item(snapshot, HOE_ID)
        hoe_iid = int(hoe["item"]["instance_id"])
        if hoe_iid <= 0:
            raise BotAssertionError(f"/give 的锄头必须有正 runtime instance_id，实际 {hoe}")
        if not any(
            e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > give_anchor
            and find_item(e.data["payload"], HOE_ID) is not None
            for e in bot.events
        ):
            raise BotAssertionError("锄头 instance 必须来自 give 动作之后的权威 inventory_snapshot")

        equip_anchor = last_event_time(bot)
        send_move(bot, hoe_iid, hoe["location"], equip_location("main_hand", "held"))
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > equip_anchor
            and (e.data["payload"].get("equipped", {}).get("main_hand_held") or {}).get(
                "instance_id"
            )
            == hoe_iid,
            timeout=10.0,
            description=f"真实锄头 {hoe_iid} 装备到 main_hand held",
        )
        _start_real_till(bot, hoe_iid)

        gather_anchor = last_event_time(bot)
        bot.cmd(f"bong gather {HERB_ID}")
        bot.expect_chat("Gameplay action queued.", timeout=10.0)
        _wait_gather_progress(bot, gather_anchor)
        bot.assert_alive("真锄开垦与采集深断言之后")
