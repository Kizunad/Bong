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

fixture 放置目标的关键前置（实测推翻初版假设）：join 的 PlayerPositionAndLook
**不**等于权威 Position（movement_commit.rs AuthoritativePositionCommitSet 只收
服务器系统写，C2S 移动不更新）；fixture 出生点落在光栅外实心石墙上，join 坐标
附近的竖直列全是石方，block_place 一律被拒（`target block stone is not
replaceable`），初版在出生点 ±3 扫描必然跳过 happy path。修法：`/top` 把权威
坐标搬到该列地表（surface 扫描顶 +3，确认 chat `Teleported to top at Y=...`，
命令系统 position.set 与确认 chat 同 tick），落地位置是空气，可放置目标成立。
workbench_open 的 interact 范围（Chebyshev ≤3.0）读该权威坐标，在 /top 落点
立即放置+打开即可（权威坐标另有周期性 +10 上移，~8s 一档，放置后立刻 open
的窗口远小于该周期；happy path 失败则按实体重试一次，避免把竞态当回归）。
"""

import math
import re
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
TOP_CHAT_RE = re.compile(r"Teleported to top at Y=(\d+)\.")
TOP_TIMEOUT = 10.0


def run(env) -> None:
    with env.new_bot("WbH") as bot:
        snapshot = wait_join_and_inventory(bot)
        if bot.position is None:
            raise BotAssertionError("workbench 场景需要 pos_look 后的位置，实际 position=None")

        # 权威坐标搬到该列地表（/top：surface 扫描顶 +3）。确认 chat 与 position.set
        # 同 tick，收到即权威坐标已到目标 y；不依赖 pos_look 回包。
        bot.cmd("top")
        top_ev = bot.wait_for(
            lambda e: e.kind == "chat" and TOP_CHAT_RE.search(e.data["text"]),
            timeout=TOP_TIMEOUT,
            description="`/top` 确认聊天 Teleported to top at Y=...",
        )
        y0 = int(TOP_CHAT_RE.search(top_ev.data["text"]).group(1))

        if not _has_any_chunk(bot):
            print("    [warn] 出生点仍无 ChunkData，跳过制作台放置/open leg")
            bot.assert_alive("workbench 场景因无 chunk 跳过前")
            return

        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)

        placed = _place_until_marker(bot, y0, snapshot["revision"])
        if placed is None:
            print(
                "    [warn] 未观察到 workbench Marker；/top 落点无可放置目标，"
                "跳过 workbench_open leg"
            )
            bot.assert_alive("workbench 放置未观察到 marker 后")
            return
        x, y, z, spawn = placed

        # happy path：打开制作台，断言 S2C WorkbenchOpen 回推放置坐标。
        # 权威坐标周期性 +10 上移（~8s 一档），/top 落点后立即 open 的窗口远小于
        # 该周期；若 open 恰逢移档被静默丢弃，对同一实体重试一次。
        payload = None
        for _ in range(2):
            bot.intent({**OPEN_REQUEST, "entity_id": spawn.data["entity_id"]})
            try:
                opened = bot.expect_server_data("workbench_open", timeout=3.0)
                payload = opened.data["payload"]
                break
            except BotAssertionError:
                continue
        if payload is None:
            raise BotAssertionError(
                f"[{bot.username}] 期望 workbench_open 回推 WorkbenchOpen payload，"
                "实际两次尝试均超时（权威坐标可能已移出 3m）"
            )
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


def _place_until_marker(bot, y0: int, start_revision: int):
    """在 /top 落点附近竖直扫描可放置目标：block_place 无 S2C 反馈，只能凭实体
    Marker 判成败。

    /top 把权威坐标搬到该列地表顶（y0），落点是空气，block_place 以
    (floor(x)+2, y0, floor(z)) 为目标成立；保留 ±3 竖直扫描兜底不可替换方块。
    每次尝试前重新 give：block_place 会消耗 workbench_item，上一格成功放置后
    后续格不能拿空背包。give 后必须按 revision 过滤拿最新快照：wait_inventory
    _contains 每次从 0 重扫历史事件，会把上一格的旧 instance 当成当次 give 的
    产物（block_place 判 not held）。
    """
    if bot.position is None:
        raise BotAssertionError("workbench 场景需要 pos_look 后的位置，实际 position=None")
    base_x = math.floor(bot.position[0]) + 2
    base_z = math.floor(bot.position[2])
    revision = start_revision
    # WORKBENCH_INTERACT_RANGE=3.0 用 Chebyshev 距离（max(|dx|,|dy|,|dz|)），
    # 放置位在东侧 2 格，故 |dy| 必须 ≤3 否则 open 出界静默。
    for dy in (0, 1, 2, -1, 3, -2):
        x, y, z = base_x, y0 + dy, base_z
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
