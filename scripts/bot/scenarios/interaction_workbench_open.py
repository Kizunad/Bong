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
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client
# （network/carrier_state_emit.rs）。本场景 bot 为 Awaken 无经脉，cultivation_detail
# 不出现；player_state/inventory_snapshot 只随 Changed 组件发射，/tpzone 改位置不在
# PlayerStateEmitQueryFilter 的 Changed 名单内（network/mod.rs），不触发 flush。
# 静默契约 = 白名单外任何 server_data 一律判红（central-review 2029 #8：未知实体与
# 出界 open 都被文档化为不产 S2C，只盯 workbench_open 会放走拒收却发 event_alert /
# 库存更新等副作用的坏实现）。
AMBIENT_PERIODIC_PAYLOAD_TYPES = frozenset({"carrier_state"})


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
            # 无 ChunkData 属于 fixture 前置失败而非合法跳过：直接 return 会让场景
            # 以成功通过、却未执行任何 workbench_open 断言，丧失回归保护（review
            # finding 2/5）。
            raise BotAssertionError(
                f"[{bot.username}] 期望出生点收到 ChunkData，实际窗口内无任何 chunk——"
                "制作台放置/open 链无法构造，场景契约未被执行"
            )

        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)

        placed = _place_until_marker(bot, y0, snapshot["revision"])
        if placed is None:
            # 放置失败（含 block_place 损坏 / Marker 不再产出）不是可选跳过：
            # 场景声明的放置→open 生产链恰恰要防这种回归，直接 return 会把
            # 失败报告成成功（review finding 2/5）。
            raise BotAssertionError(
                f"[{bot.username}] 期望 block_place 在 /top 落点附近产出 workbench Marker，"
                "实际竖直扫描全部落空——放置链路或 Marker 产出已断，场景契约未被执行"
            )
        x, y, z, spawn = placed

        # happy path：打开制作台，断言 S2C WorkbenchOpen 回推放置坐标。
        # 权威坐标周期性 +10 上移（~8s 一档），/top 落点后立即 open 的窗口远小于
        # 该周期；若 open 恰逢移档被静默丢弃，重试前必须先恢复位置前提——直接对
        # 同一实体重试只会被同样丢弃（review finding 6）。重试先 /top 把权威坐标
        # 搬回该列地表顶（x/z 不变，故回到同一落点），等确认聊天锚定在重试发起
        # 之后，避免误匹配首轮 confirm 造成「未真正恢复就开始重试」。
        payload = None
        for attempt in range(2):
            if attempt > 0:
                retry_anchor = bot.events[-1].t if bot.events else 0.0
                bot.cmd("top")
                bot.wait_for(
                    lambda e: e.kind == "chat"
                    and e.t > retry_anchor
                    and TOP_CHAT_RE.search(e.data["text"]),
                    timeout=TOP_TIMEOUT,
                    description="重试前 `/top` 恢复权威坐标确认",
                )
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
        # 必须与请求的实体 id 相等（request→lookup→emit→consumer 一条事务闭环），
        # 只断言非空会让"恒返回 1 或别的制作台 id"的坏生产者照样通过（review
        # finding 3/5）。
        expected_entity_id = spawn.data["entity_id"]
        if payload.get("entity_id") != expected_entity_id:
            raise BotAssertionError(
                f"[{bot.username}] 期望 WorkbenchOpen.entity_id={expected_entity_id}"
                f"（回推所打开的实体），实际 {payload.get('entity_id')}"
            )
        bot.assert_alive("workbench_open happy path 后")

        # 拒绝 1：实体 id 不存在 → 聊天「目标不存在。」（dispatch 层 get_by_id 失败）。
        # 拒收契约 = 该请求只产拒信、不得同时产出 WorkbenchOpen payload。水位必须在
        # intent 之前截取——若在拒信消费后才锚定，先于拒信到达的成功 payload 会被
        # 排除在静默窗口外，「照发 WorkbenchOpen 又回拒收文案」的坏实现就撞不红
        # （review finding 1/5）。拒信 chat 本身已被 expect_chat 消费，静默窗口按 t
        # 豁免该条 chat；其余 workbench_open payload 与新聊天仍判红。
        reject_sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent({**OPEN_REQUEST, "entity_id": 987654321})
        reject_chat = bot.expect_chat("目标不存在。", timeout=10.0)
        _assert_silent_window(
            bot,
            reject_sent_at,
            "不存在实体的 workbench_open 应只回「目标不存在。」，不得同时回推 WorkbenchOpen payload",
            window=SILENT_WINDOW,
            allowed_chat_ts=(reject_chat.t,),
        )

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
            "出界 workbench_open 应被静默丢弃（interact 出界 continue，无 S2C 无聊天）",
            window=SILENT_WINDOW,
        )


def _assert_silent_window(
    bot,
    sent_at: float,
    description: str,
    window: float,
    allowed_chat_ts: tuple = (),
) -> None:
    """断言窗口内无任何非周期 server_data 与任何新聊天（静默契约）。

    服务器有周期 payload（carrier_state ~1s 一次），静默按白名单豁免周期流、白名单
    外一律判红——不能断言"无任何 server_data"（central-review 2029 #8）。
    allowed_chat_ts 用于豁免已被 expect_chat 消费掉的预期拒信（水位前移后拒信
    t > sent_at，按 t 放行该条）。
    """
    deadline = time.monotonic() + window
    while True:
        _scan_silent_window(bot, sent_at, description, allowed_chat_ts)
        if time.monotonic() >= deadline:
            # 终末复扫：事件扫描与 deadline 判定非原子（central-review 2029 #3），
            # deadline 判定成立后、返回前再扫一次，收口最后一段未观测窗口——否则
            # 该段内到达的目标 payload/聊天会被漏掉。
            _scan_silent_window(bot, sent_at, description, allowed_chat_ts)
            return
        bot.assert_alive(f"{description} 窗口内连接保持")
        time.sleep(0.1)


def _scan_silent_window(
    bot,
    sent_at: float,
    description: str,
    allowed_chat_ts: tuple,
) -> None:
    for e in bot.events_of("server_data"):
        # 静默契约 = 「无任何非周期 S2C 响应 + 无聊天」。只盯 workbench_open 会放走
        # 拒收却发 event_alert / inventory_update / 其他 payload 的坏实现——未知实体
        # 与出界 open 都被文档化为不产 S2C，白名单外一律判红，与 mineral/freshness
        # 场景共用同一 observable 契约（central-review 2029 #8）。
        if e.t > sent_at and e.data["payload_type"] not in AMBIENT_PERIODIC_PAYLOAD_TYPES:
            raise BotAssertionError(
                f"[{bot.username}] {description}，"
                f"实际窗口内收到 server_data/{e.data['payload_type']}（t={e.t:.3f}）"
            )
    for e in bot.events_of("chat"):
        if e.t > sent_at and e.t not in allowed_chat_ts:
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际窗口内出现聊天 {e.data['text']!r}"
            )


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
