"""give_dan_to_elder 拒收链（plan-dying-elder-v1 P1 垂死大能馈赠）。

handle_give_dan_to_elder（client_request_handler.rs:19791）前置检查顺序：
1. pill_instance_id 不在背包 → 聊天「§c[垂死大能] 背包中未找到该回元丹。」
2. 物品非 huiyuan_pill → 聊天「§c[垂死大能] 只接受回元丹。」
3. elder_entity_id 无对应实体 → 聊天「§c[垂死大能] 找不到目标大能。」
4. 已解析实体不是 DyingElder / 状态不可接丹 → 目标门禁拒绝；
5. 已解析 Plea 大能超距或跨维 → 空间门禁拒绝。

本场景先覆盖前三条拒收链，再用 production passive target 构造真实非 elder
protocol target，最后以 `/time advance 720000` 触发 production DyingElder，在真实
protocol entity id 上分别验证距离与维度门。每条拒收都断言聊天-only、连接保持和
回元丹同实例未被消费。

拒收无损契约：每条拒收后，被拒物品的**同实例**必须仍在背包（拒绝分支只回 chat、
不消费物品）。每次拒收后用无害 give 触发新快照验证——防「先吞物品再回拒收文案」的
单边损耗实现。

顺序断言同时锁定检查顺序：先背包后模板、模板先于目标实体。chat-only 契约由
_assert_chat_only_response 逐条锁死：每条拒收只回聊天、绝不发任何非周期 S2C 响应
（central-review 2029 #5）。
"""

import json
import time

from bot.bot import BotAssertionError

from ._combat_helpers import last_event_time, queue_passive_target
from ._inventory_helpers import (
    require_item,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_join_and_inventory,
)
from ._rejection_helpers import AMBIENT_SERVER_DATA_TYPES

DESCRIPTION = "give_dan_to_elder 拒收链：背包缺失→非回元丹→有效 pill 的目标门禁，逐条拒绝"
MODULES = ["fauna", "network"]

DAN_REQUEST = {"type": "give_dan_to_elder", "v": 1}
MEAT_ITEM = "food.mundane.cooked_meat"
NO_SUCH_ELDER_ID = 987654321
SILENT_WINDOW = 4.0
TSY_ZONE_CENTERS = {
    "tsy_lingxu_01_shallow": (50.0, 80.0, 50.0),
    "tsy_lingxu_01_mid": (50.0, 20.0, 50.0),
    "tsy_lingxu_01_deep": (50.0, -20.0, 50.0),
    "tsy_zongmen_01_shallow": (250.0, 80.0, 250.0),
    "tsy_zongmen_01_mid": (250.0, 20.0, 250.0),
    "tsy_zongmen_01_deep": (250.0, -20.0, 250.0),
    "tsy_daneng_01_shallow": (550.0, 80.0, 550.0),
    "tsy_daneng_01_mid": (550.0, 20.0, 550.0),
    "tsy_daneng_01_deep": (550.0, -20.0, 550.0),
    "tsy_gaoshou_01_shallow": (1050.0, 80.0, 550.0),
    "tsy_gaoshou_01_mid": (1050.0, 20.0, 550.0),
    "tsy_gaoshou_01_deep": (1050.0, -20.0, 550.0),
}
TSY_ZONE_ORDER = tuple(TSY_ZONE_CENTERS)
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client。
# 本场景无 cultivation/meridian/zone 变化，窗口内除 carrier_state 无合法非白名单
# payload；白名单外一律判红（chat-only 契约的 S2C 半）。carrier_state 不在 proto_min
# 白名单，通常不解码成 server_data 事件；保留它只为显式豁免未来 proto_min 收录后的
# 周期流。
AMBIENT_PERIODIC_PAYLOAD_TYPES = AMBIENT_SERVER_DATA_TYPES


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

        # 4. Exercise the resolved-target authorization path with a production NPC that
        # is stationary and spawns one block away. The server must resolve it, pass the
        # dimension/range gates, then reject it as non-elder.
        target = queue_passive_target(bot)
        target_id = target.data.get("entity_id")
        if not isinstance(target_id, int) or target_id <= 0:
            raise BotAssertionError(
                f"[{bot.username}] passive_target entity_id 应为正整数，实际 {target_id!r}"
            )
        sent_at = last_event_time(bot)
        bot.intent(
            {**DAN_REQUEST, "pill_instance_id": pill["item"]["instance_id"], "elder_entity_id": target_id}
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
        reject = _expect_chat_after(
            bot, "目标不在当前位面或交互范围内。", sent_at, timeout=10.0
        )
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
        reject = _expect_chat_after(
            bot, "目标不在当前位面或交互范围内。", sent_at, timeout=10.0
        )
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
    # The selected TSY spawn zone is deliberately negative. Fresh bot characters
    # may have little remaining qi after the earlier rejection chain, so replenish
    # the dev-only setup resource before entering the zone; otherwise the negative
    # pressure can move the requester out of Alive while the elder is spawning.
    bot.cmd("qi set 10")
    bot.expect_chat("[dev] qi set", timeout=10.0)
    # 生产 spawn selector 取 TSY registry 中第一个低于阈值的 zone。先用 dev-only
    # zone_qi 把候选面钉成唯一的 daneng shallow，再把 bot 放到该层中心；使用略低于
    # spawn threshold 的值，给拒绝链留下足够观察时间，不把 elder 提前抽干。
    spawn_zone = "tsy_daneng_01_shallow"
    for zone in TSY_ZONE_ORDER:
        bot.cmd(f"zone_qi set {zone} 0")
        bot.expect_chat(f"[dev] zone_qi `{zone}`", timeout=10.0)
    bot.cmd(f"zone_qi set {spawn_zone} -0.41")
    bot.expect_chat(f"[dev] zone_qi `{spawn_zone}`", timeout=10.0)
    bot.cmd(f"tpzone {spawn_zone}")
    bot.expect_chat(f"Teleported to zone `{spawn_zone}`.", timeout=10.0)
    bot.set_position(*TSY_ZONE_CENTERS[spawn_zone])
    anchor = last_event_time(bot)
    bot.cmd("time advance 720000")
    bot.expect_chat("[dev] time advanced 720000", timeout=10.0)
    elder = bot.wait_for(
        lambda event: (
            event.kind == "entity_spawn"
            and event.t > anchor
            and event.data.get("type") == 120
        )
        or (
            event.kind == "payload"
            and event.t > anchor
            and event.data.get("channel") == "bong:elder_encounter"
            and _elder_appeared_payload(event) is not None
        ),
        timeout=10.0,
        description="time advance 后出现 production DyingElder entity_spawn/elder_encounter",
    )
    if elder is None:
        raise BotAssertionError(
            f"[{bot.username}] time advance 后未在 TSY 层收到 DyingElder entity_spawn/elder_encounter"
        )
    if elder.kind == "entity_spawn":
        entity_id = elder.data.get("entity_id")
    else:
        entity_id = _elder_appeared_payload(elder)["elder_entity_id"]
    if not isinstance(entity_id, int) or entity_id <= 0:
        raise BotAssertionError(
            f"DyingElder entity_spawn 必须携带正数 protocol id，实际 {entity_id!r}"
        )
    # Move to a separate TSY family through the authoritative zone command. The same
    # entity id remains resolvable server-side, but the large horizontal gap exercises
    # the range gate before the later cross-dimension check.
    range_zone = "tsy_zongmen_01_shallow"
    bot.cmd(f"tpzone {range_zone}")
    bot.expect_chat(f"Teleported to zone `{range_zone}`.", timeout=10.0)
    return entity_id


def _elder_appeared_payload(event):
    if event.kind != "payload" or event.data.get("channel") != "bong:elder_encounter":
        return None
    try:
        payload = json.loads(bytes(event.data["data"]).decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) and payload.get("event_kind") == "appeared" else None


def _expect_chat_after(bot, substring: str, after: float, timeout: float = 10.0):
    """等待当前请求产生的聊天，避免匹配前一条同文案拒绝。"""
    return bot.wait_for(
        lambda event: event.kind == "chat"
        and event.t > after
        and substring in event.data.get("text", ""),
        timeout=timeout,
        description=f"t>{after:.3f}s 后包含「{substring}」的聊天消息",
    )


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
