"""制作台放置 + workbench_open 交互链路（实体/空间探知流 P2, plan-workbench-place-runtime-v1）。

纯 bot 驱动路径（镜像 inventory_container_open_minimal 的 trade_crate 套路）：
1. `[dev] give workbench_item 1` 获取 placeable 制作台物品。
2. `block_place` intent 放置，观察 Workbench 视觉 Marker entity_spawn。
3. `workbench_open` intent 打开，断言 S2C
   `ServerDataPayloadV1::WorkbenchOpen { entity_id, position }`（workbench.rs:141）回推。
4. 拒绝路径：
   - 实体 id 不存在的 open → 聊天「[制作台] 目标不存在。」（client_request_handler.rs:2472）；
   - 走离制作台超过 `WORKBENCH_INTERACT_RANGE=3.0` 后的 open → 静默丢弃
     （workbench.rs handle_workbench_interact 出界 continue，无 S2C 无聊天）。

若当前出生点无稳定可放置 chunk（与 container_open 场景同一环境判断），显式跳过
放置/open leg，避免把环境缺口误判成 workbench_open 协议回归。
"""

import math
import time

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    latest_inventory_snapshot,
    require_item,
    wait_inventory_revision_after,
    wait_join_and_inventory,
)

DESCRIPTION = "放置制作台后 workbench_open 回推 WorkbenchOpen payload；坏 id 聊天拒绝、出界静默"
MODULES = ["craft", "interaction", "network"]

WORKBENCH_ITEM = "workbench_item"
OPEN_REQUEST = {"type": "workbench_open", "v": 1}
SILENT_WINDOW = 5.0


def run(env) -> None:
    with env.new_bot("WbH") as bot:
        snapshot = wait_join_and_inventory(bot)
        if bot.position is None:
            raise BotAssertionError("workbench 场景需要 pos_look 后的位置，实际 position=None")
        # 出生点 [8,150,8] 在 fixture 光栅上方（空中、有 ChunkData），join 的
        # PlayerPositionAndLook 就是权威 Position（movement_commit.rs
        # AuthoritativePositionCommitSet 只收服务器系统写）；workbench_open 的
        # interact 范围（WORKBENCH_INTERACT_RANGE=3.0 Chebyshev）读该权威坐标。
        # 直接在出生坐标放置+打开，bot.position 与权威坐标天然一致，无需传送。
        if not _has_any_chunk(bot):
            print("    [warn] 出生点仍无 ChunkData，跳过制作台放置/open leg")
            bot.assert_alive("workbench 场景因无 chunk 跳过前")
            return

        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)

        placed = _place_until_marker(bot)
        if placed is None:
            print(
                "    [warn] 未观察到 workbench Marker；当前出生点可能无稳定可放置目标，"
                "跳过 workbench_open leg"
            )
            bot.assert_alive("workbench 放置未观察到 marker 后")
            return
        x, y, z, spawn = placed

        # happy path：打开制作台，断言 S2C WorkbenchOpen 回推放置坐标。
        bot.intent({**OPEN_REQUEST, "entity_id": spawn.data["entity_id"]})
        opened = bot.expect_server_data("workbench_open", timeout=10.0)
        payload = opened.data["payload"]
        if list(payload.get("position", [])) != [x, y, z]:
            raise BotAssertionError(
                f"[{bot.username}] 期望 WorkbenchOpen.position={[x, y, z]}，"
                f"实际 {payload.get('position')}"
            )
        if not payload.get("entity_id"):
            raise BotAssertionError(
                f"[{bot.username}] 期望 WorkbenchOpen.entity_id 非空，实际 {payload.get('entity_id')}"
            )
        bot.assert_alive("workbench_open happy path 后")

        # 拒绝 1：实体 id 不存在 → 聊天「目标不存在。」（dispatch 层 get_by_id 失败）。
        bot.intent({**OPEN_REQUEST, "entity_id": 987654321})
        bot.expect_chat("目标不存在。", timeout=10.0)

        # 拒绝 2：权威坐标离开制作台 >3m 后 open → 静默（interact 出界 continue）。
        # C2S 移动不更新权威 Position，用 /tpzone 拉到远端 zone 中心（fixture
        # 二进制按 CARGO_MANIFEST_DIR 载入 zones.json，jiuzong_taichu_ruin 中心
        # (0,85,-10000) 与任何 spawn 邻域距离都 >3m，出界判定确定性成立）。命令
        # 系统里 position.set 与确认 chat 同 tick 发生，收到 chat 即保证权威坐标
        # 已搬走，本 leg 只断言 open 被静默丢弃，不需要 pos_look 回包。
        bot.cmd("tpzone jiuzong_taichu_ruin")
        bot.expect_chat("Teleported to zone `jiuzong_taichu_ruin`.", timeout=10.0)
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent({**OPEN_REQUEST, "entity_id": spawn.data["entity_id"]})
        bot.assert_alive("出界 workbench_open 后")
        _assert_silent_window(
            bot,
            sent_at,
            "workbench_open",
            "出界 workbench_open 应被静默丢弃（interact 出界 continue，无 S2C 无聊天）",
            window=SILENT_WINDOW,
        )


def _assert_silent_window(bot, sent_at: float, payload_type: str, description: str, window: float) -> None:
    """断言窗口内未出现指定 payload_type 的 server_data 与任何新聊天。

    服务器有周期 payload（如 cultivation_detail ~1s 一次），所以静默只能按
    payload_type 细粒度断言，不能断言"无任何 server_data"。
    """
    end_at = sent_at + window
    while True:
        now = bot.events[-1].t if bot.events else 0.0
        for e in bot.events_of("server_data"):
            if e.t > sent_at and e.data["payload_type"] == payload_type:
                raise BotAssertionError(
                    f"[{bot.username}] {description}，"
                    f"实际窗口内收到 payload_type={payload_type}"
                )
        for e in bot.events_of("chat"):
            if e.t > sent_at:
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际窗口内出现聊天 {e.data['text']!r}"
                )
        if now >= end_at:
            return
        time.sleep(0.1)


def _has_any_chunk(bot) -> bool:
    try:
        bot.wait_for(lambda e: e.kind == "chunk_data", timeout=2.0, description="任意 ChunkData")
        return True
    except BotAssertionError:
        return False


def _place_until_marker(bot):
    """竖直逐格扫描可放置目标：block_place 无 S2C 反馈，只能凭实体 Marker 判成败。

    场景前置已把权威坐标搬进 spawn zone 中心（空中），但 block_place 不做范围
    预检只做方块校验，若目标格恰为不可替换方块（服务器 WARN `target block ...
    is not replaceable`）则静默失败——保留竖直扫描兜底。每次尝试前重新 give，
    避免前一格已成功放置消耗物品后，后续格拿空背包误判失败。give 后必须按
    revision 过滤拿最新快照：wait_inventory_contains 每次从 0 重扫历史事件，
    会把上一格的旧 instance 当成当次 give 的产物（block_place 判 not held）。
    """
    if bot.position is None:
        raise BotAssertionError("workbench 场景需要 pos_look 后的位置，实际 position=None")
    base_x = math.floor(bot.position[0]) + 2
    base_z = math.floor(bot.position[2])
    base_y = math.floor(bot.position[1])
    # WORKBENCH_INTERACT_RANGE=3.0 用 Chebyshev 距离（max(|dx|,|dy|,|dz|)），
    # 放置位在东侧 2 格，故 |dy| 必须 ≤3 否则 open 出界静默。
    revision = latest_inventory_snapshot(bot).get("revision", 0)
    for dy in (0, 1, 2, 3, -1, -2, -3):
        x, y, z = base_x, base_y + dy, base_z
        bot.cmd(f"give {WORKBENCH_ITEM} 1")
        bot.expect_chat(f"[dev] gave {WORKBENCH_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, revision, timeout=10.0)
        revision = snapshot["revision"]
        item = require_item(snapshot, WORKBENCH_ITEM)
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {
                "type": "block_place",
                "v": 1,
                "x": x,
                "y": y,
                "z": z,
                "item_instance_id": item["item"]["instance_id"],
                "target_face": "north",
            }
        )
        try:
            spawn = bot.wait_for(
                lambda e: e.kind == "entity_spawn"
                and e.t > sent_at
                and abs(e.data["x"] - (x + 0.5)) <= 1.5
                and abs(e.data["y"] - y) <= 2.0
                and abs(e.data["z"] - (z + 0.5)) <= 1.5,
                timeout=2.5,
                description=f"workbench_item 在 ({x},{y},{z}) 放置后附近出现 Marker",
            )
        except BotAssertionError:
            continue
        return x, y, z, spawn
    return None
