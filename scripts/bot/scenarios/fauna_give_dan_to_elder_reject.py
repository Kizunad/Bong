"""give_dan_to_elder 拒收链（plan-dying-elder-v1 P1 垂死大能馈赠）。

handle_give_dan_to_elder（client_request_handler.rs:19791）前置检查顺序：
1. pill_instance_id 不在背包 → 聊天「§c[垂死大能] 背包中未找到该回元丹。」
2. 物品非 huiyuan_pill → 聊天「§c[垂死大能] 只接受回元丹。」
3. elder_entity_id 无对应实体 → 聊天「§c[垂死大能] 找不到目标大能。」

happy path（大能收丹 → Plea→Recovering）不可黑盒构造：垂死大能 spawn 要求
tsy zone spirit_qi < -0.4 且间隔 30 游戏日（dying_elder.rs:62 DYING_ELDER_
SPAWN_INTERVAL_TICKS = 30 * 24000 ≈ 10h 实墙钟），fixture 运行不可达；dev
命令层也无 elder/大能 spawn 命令。故本场景按拒收链逐条断言聊天反馈，并验证
请求失败不中断连接（大能未收丹、玩家无损失）。

拒收无损契约：每条拒收后，被拒物品的**同实例**必须仍在背包（拒绝分支只回 chat、
不消费物品）。第一拒（非回元丹）断在下一张快照；第二拒（找不到目标大能）用一次
无害 give 触发新快照后验证——防「先吞物品再回拒收文案」的单边损耗实现。

顺序断言同时锁定检查顺序：先背包后模板、模板先于目标实体。chat-only 契约由
_assert_chat_only_response 逐条锁死：每条拒收只回聊天、绝不发任何非周期 S2C 响应
（central-review 2029 #5）。
"""

import math
import time

from bot.bot import BotAssertionError

from ._combat_helpers import last_event_time
from ._inventory_helpers import (
    require_item,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_join_and_inventory,
)

DESCRIPTION = "give_dan_to_elder 拒收链：背包缺失→非回元丹→有效 pill 的目标门禁，逐条拒绝"
MODULES = ["fauna", "network"]

DAN_REQUEST = {"type": "give_dan_to_elder", "v": 1}
MEAT_ITEM = "food.mundane.cooked_meat"
NO_SUCH_ELDER_ID = 987654321
SILENT_WINDOW = 4.0
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client。
# 本场景无 cultivation/meridian/zone 变化，窗口内除 carrier_state 无合法非白名单
# payload；白名单外一律判红（chat-only 契约的 S2C 半）。carrier_state 不在 proto_min
# 白名单，通常不解码成 server_data 事件；保留它只为显式豁免未来 proto_min 收录后的
# 周期流。
AMBIENT_PERIODIC_PAYLOAD_TYPES = frozenset({"carrier_state"})


def run(env) -> None:
    with env.new_bot("DhH") as bot:
        snapshot = wait_join_and_inventory(bot)
        revision = snapshot["revision"]

        # 1. instance_id 不在背包 → 背包中未找到该回元丹。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {**DAN_REQUEST, "pill_instance_id": 999999999999, "elder_entity_id": NO_SUCH_ELDER_ID}
        )
        reject = bot.expect_chat("背包中未找到该回元丹。", timeout=10.0)
        _assert_chat_only_response(
            bot, sent_at, "背包缺失拒收应只回 chat（无 S2C 响应）", allowed_chat_ts=(reject.t,)
        )
        bot.assert_alive("背包缺失拒收后")

        # 2. 背包内有非回元丹物品 → 只接受回元丹。
        bot.cmd(f"give {MEAT_ITEM} 1")
        bot.expect_chat(f"[dev] gave {MEAT_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_contains(bot, MEAT_ITEM, timeout=10.0)
        meat = require_item(snapshot, MEAT_ITEM)
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {**DAN_REQUEST, "pill_instance_id": meat["item"]["instance_id"], "elder_entity_id": NO_SUCH_ELDER_ID}
        )
        reject = bot.expect_chat("只接受回元丹。", timeout=10.0)
        _assert_chat_only_response(
            bot, sent_at, "非回元丹拒收应只回 chat（无 S2C 响应）", allowed_chat_ts=(reject.t,)
        )
        bot.assert_alive("非回元丹拒收后")

        # 3. 回元丹在背包、目标协议实体不存在 → 找不到目标大能。
        revision = snapshot["revision"]
        bot.cmd("give huiyuan_pill 1")
        bot.expect_chat("[dev] gave huiyuan_pill x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, revision, timeout=10.0)
        pill = require_item(snapshot, "huiyuan_pill")
        # 拒收契约：拒绝分支只回 chat、绝不消费被拒物品。上一拒（非回元丹）后，
        # cooked_meat 的**同实例**必须仍在背包——断在下一张快照（give 回元丹触发）；
        # 若实现「先吞物品再回拒收文案」，这里实例 id 会消失或被替换。
        if require_item(snapshot, MEAT_ITEM)["item"]["instance_id"] != meat["item"]["instance_id"]:
            raise BotAssertionError(
                f"[{bot.username}] 拒收「只接受回元丹」后 cooked_meat 应保留原实例 "
                f"{meat['item']['instance_id']}，实际丢失或替换"
            )
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {**DAN_REQUEST, "pill_instance_id": pill["item"]["instance_id"], "elder_entity_id": NO_SUCH_ELDER_ID}
        )
        reject = bot.expect_chat("找不到目标大能。", timeout=10.0)
        _assert_chat_only_response(
            bot, sent_at, "找不到目标大能拒收应只回 chat（无 S2C 响应）", allowed_chat_ts=(reject.t,)
        )
        # 拒收分支不推新快照；用一次无害 give 触发 revision，验证上一拒（找不到
        # 目标大能）后 huiyuan_pill 的**同实例**仍在背包（未被吞掉再补发/替换）。
        revision = snapshot["revision"]
        bot.cmd(f"give {MEAT_ITEM} 1")
        bot.expect_chat(f"[dev] gave {MEAT_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, revision, timeout=10.0)
        if require_item(snapshot, "huiyuan_pill")["item"]["instance_id"] != pill["item"]["instance_id"]:
            raise BotAssertionError(
                f"[{bot.username}] 拒收「找不到目标大能」后 huiyuan_pill 应保留原实例 "
                f"{pill['item']['instance_id']}，实际丢失或替换"
            )

        # 4. Exercise the resolved-target authorization path with a real NPC. The
        # scenario command creates a valid non-elder target beyond the give-dan
        # gate; a valid pill must remain in the same instance after rejection.
        anchor = last_event_time(bot)
        bot.cmd("npc_scenario fight")
        bot.expect_chat("Scenario queued.", timeout=10.0)
        target = bot.wait_for(
            lambda event: (
                event.kind == "entity_spawn"
                and event.t > anchor
                and event.data.get("entity_id") != bot.entity_id
            ),
            timeout=15.0,
            description="npc_scenario fight 后出现可解析的非大能 entity",
        )
        sent_at = last_event_time(bot)
        bot.intent(
            {**DAN_REQUEST, "pill_instance_id": pill["item"]["instance_id"], "elder_entity_id": target.data["entity_id"]}
        )
        reject = bot.expect_chat("目标不是可交互的大能。", timeout=10.0)
        _assert_chat_only_response(
            bot, sent_at, "解析到非大能目标时应在消费前拒绝", allowed_chat_ts=(reject.t,)
        )
        bot.cmd(f"give {MEAT_ITEM} 1")
        bot.expect_chat(f"[dev] gave {MEAT_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)
        if require_item(snapshot, "huiyuan_pill")["item"]["instance_id"] != pill["item"]["instance_id"]:
            raise BotAssertionError(
                f"[{bot.username}] 解析到非大能目标后 huiyuan_pill 应保留原实例 "
                f"{pill['item']['instance_id']}，实际丢失或替换"
            )
        # 5. Spawn a real production DyingElder, then exercise the resolved-target
        # range gate and the cross-dimension gate with the same valid pill instance.
        elder_id = _spawn_real_elder(bot)
        sent_at = last_event_time(bot)
        bot.intent(
            {
                **DAN_REQUEST,
                "pill_instance_id": pill["item"]["instance_id"],
                "elder_entity_id": elder_id,
            }
        )
        reject = bot.expect_chat("目标不在当前位面或交互范围内。", timeout=10.0)
        _assert_chat_only_response(
            bot,
            sent_at,
            "真实 Plea 大能超出 6 格时应在消费前拒绝",
            allowed_chat_ts=(reject.t,),
        )
        bot.cmd(f"give {MEAT_ITEM} 1")
        bot.expect_chat(f"[dev] gave {MEAT_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)
        if require_item(snapshot, "huiyuan_pill")["item"]["instance_id"] != pill["item"]["instance_id"]:
            raise BotAssertionError(
                f"[{bot.username}] 超距真实大能拒绝后 huiyuan_pill 应保留原实例 "
                f"{pill['item']['instance_id']}，实际丢失或替换"
            )

        _transfer_dimension(bot, "overworld")
        sent_at = last_event_time(bot)
        bot.intent(
            {
                **DAN_REQUEST,
                "pill_instance_id": pill["item"]["instance_id"],
                "elder_entity_id": elder_id,
            }
        )
        reject = bot.expect_chat("目标不在当前位面或交互范围内。", timeout=10.0)
        _assert_chat_only_response(
            bot,
            sent_at,
            "跨维真实大能请求应在消费前拒绝",
            allowed_chat_ts=(reject.t,),
        )
        bot.cmd(f"give {MEAT_ITEM} 1")
        bot.expect_chat(f"[dev] gave {MEAT_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)
        if require_item(snapshot, "huiyuan_pill")["item"]["instance_id"] != pill["item"]["instance_id"]:
            raise BotAssertionError(
                f"[{bot.username}] 跨维真实大能拒绝后 huiyuan_pill 应保留原实例 "
                f"{pill['item']['instance_id']}，实际丢失或替换"
            )
        bot.assert_alive("give_dan_to_elder 拒收链全程")


def _spawn_real_elder(bot) -> int:
    """Spawn one production DyingElder and return its MC protocol entity id."""
    _transfer_dimension(bot, "tsy")
    bot.cmd("tpzone tsy_lingxu_01_mid")
    bot.expect_chat("Teleported to zone `tsy_lingxu_01_mid`.", timeout=10.0)
    anchor = last_event_time(bot)
    bot.cmd("time advance 720000")
    bot.expect_chat("[dev] time advanced 720000", timeout=10.0)
    elder = bot.wait_for(
        lambda event: (
            event.kind == "entity_spawn"
            and event.t > anchor
            and event.data.get("type") == 120
        ),
        timeout=15.0,
        description="time advance 后出现 production DyingElder entity_spawn",
    )
    entity_id = elder.data.get("entity_id")
    if not isinstance(entity_id, int) or entity_id <= 0:
        raise BotAssertionError(
            f"DyingElder entity_spawn 必须携带正数 protocol id，实际 {entity_id!r}"
        )
    return entity_id


def _transfer_dimension(bot, target: str) -> None:
    anchor = last_event_time(bot)
    bot.cmd(f"tpdim {target}")
    bot.wait_for(
        lambda event: (
            event.kind == "chat"
            and event.t > anchor
            and f"Queued /tpdim {target} within current XYZ gate." in event.data.get("text", "")
        ),
        timeout=10.0,
        description=f"/tpdim {target} 权威 transfer 排队确认",
    )
    bot.wait_for(
        lambda event: event.kind == "respawn" and event.t > anchor,
        timeout=10.0,
        description=f"/tpdim {target} 真实 Respawn",
    )


def _assert_chat_only_response(
    bot, sent_at: float, description: str, allowed_chat_ts: tuple = ()
) -> None:
    """断言拒收分支只回聊天：窗口内无任何非白名单 server_data、无预期拒信外的聊天。

    只等预期 chat 会放走「照发拒收文案 + 额外发 event_alert / 库存更新 / 拒绝型
    server_data」的坏实现（central-review 2029 #5）——chat-only 契约的 S2C 半必须
    由窗口扫描锁死，白名单外一律判红。已消费的拒信 chat 按事件时刻豁免
    （allowed_chat_ts）。截止时刻用单调钟（time.monotonic），不用事件时间戳
    bot.events[-1].t：静默断言正是"之后无事件到达"，事件时间不会推进，以事件时间
    做 deadline 会永远等不到 now >= end_at 而死循环（review finding 1/5）。"""
    deadline = time.monotonic() + SILENT_WINDOW
    while True:
        _scan_chat_only_violations(bot, sent_at, description, allowed_chat_ts)
        if time.monotonic() >= deadline:
            # 终末复扫：事件扫描与 deadline 判定非原子（central-review 2029 #3），
            # deadline 判定成立后、返回前再扫一次，收口最后一段未观测窗口——否则
            # 该段内到达的 server_data/聊天会被漏掉。
            _scan_chat_only_violations(bot, sent_at, description, allowed_chat_ts)
            return
        bot.assert_alive(f"{description} 窗口内连接保持")
        time.sleep(0.1)


def _scan_chat_only_violations(
    bot, sent_at: float, description: str, allowed_chat_ts: tuple
) -> None:
    for e in bot.events_of("server_data"):
        if e.t > sent_at and e.data["payload_type"] not in AMBIENT_PERIODIC_PAYLOAD_TYPES:
            raise BotAssertionError(
                f"[{bot.username}] {description}，"
                f"实际窗口内收到 server_data/{e.data['payload_type']}（t={e.t:.3f}）"
            )
    for e in bot.events_of("chat"):
        # 预期拒信本身按事件时刻豁免；其余真实新聊天一律判红。
        if e.t > sent_at and e.t not in allowed_chat_ts:
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
            )
