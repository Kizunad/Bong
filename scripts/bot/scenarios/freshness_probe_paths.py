"""freshness_probe 保鲜探针路径（实体/空间探知流 M4a，plan-exploration-probe-return-v1）。

resolve_one_probe（shelflife/probe.rs:101）检查顺序：
1. 修为 < 凝脉（MIN_PROBE_REALM_RANK=2）→ Denied(RealmTooLow) → EventAlert
   「神识未及，凝脉方可感知保鲜」；
2. item 无 freshness → Denied(NoFreshness) → 静默（freshness_probe_emit 对
   NoFreshness 一律 continue 不发 S2C）；
3. 通过 → Precise → `FreshnessUpdateV1 { item_uuid, freshness, profile_name }`
   （freshness = current_qi/initial_qi，新物品 = 1.0）。

dispatch 前置：instance_id 不在玩家背包 → 静默丢弃（client_request_handler
belongs_to_player 检查）。本场景用 `[dev] give` 构造合法背包 item：

1. Awaken 探煮熟肉（food.mundane.cooked_meat，shelflife_profile=
   food_spoil_mundane_meat_v1）→ event_alert 神识未及；
2. 凝脉后再探 → freshness_update（item_uuid=instance_id、freshness=1.0、
   profile_name=food_spoil_mundane_meat_v1）；
3. 凝脉探无保鲜 item（trade_crate）→ NoFreshness 静默（无 S2C、无聊天）；
4. 凝脉探不存在的 instance_id（不在背包）→ dispatch belongs_to_player 前置
   静默丢弃（无 S2C、无聊天）。
"""

import time

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    require_item,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_join_and_inventory,
)

DESCRIPTION = "freshness_probe：Awaken→神识未及告警、凝脉→FreshnessUpdate、无保鲜/坏实例→静默"
MODULES = ["shelflife", "network"]

PROBE_REQUEST = {"type": "freshness_probe", "v": 1}
MEAT_ITEM = "food.mundane.cooked_meat"
MEAT_PROFILE = "food_spoil_mundane_meat_v1"
PLAIN_ITEM = "trade_crate"
SILENT_WINDOW = 4.0
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client
# （network/carrier_state_emit.rs，ticks % TICKS_PER_SECOND==0 周期）。
# player_state / inventory_snapshot 在本场景只随 Changed 组件发射（gap9 无
# 周期性无变化 flush），窗口内无合法非白名单 payload——白名单外一律判红
# （central-review 2029 #2）。carrier_state 不在 proto_min 白名单，通常不
# 解码成 server_data 事件；保留它只为显式豁免未来 proto_min 收录后的周期流。
AMBIENT_PERIODIC_PAYLOAD_TYPES = frozenset({"carrier_state"})
# food_spoil_mundane_meat_v1：Linear 衰减 decay_per_tick = 1/(GAME_DAY_TICKS×3)，
# GAME_DAY_TICKS=24000、TICKS_PER_SECOND=20 → 2.78e-4/s（server/src/shelflife/registry.rs）。
MEAT_DECAY_PER_SECOND = 1.0 / (24000 * 3) * 20.0
# 探针路径 multiplier = container_storage_multiplier(Normal) × season_decay_modifier
# （shelflife/probe.rs）。fixture 恒为 Summer（×1.3，YEAR_TICKS 前 40%），但服务器慢
# tick / 季节变更时取宽括号 [0.7, 1.3]（Winter=0.7，过渡期 0.8..1.2）避免误报——
# 只要 elapsed 足够大，最小倍率下界也能把 1.0 压出上界之外。
SEASON_DECAY_MIN = 0.7
SEASON_DECAY_MAX = 1.3
FRESHNESS_TOLERANCE = 0.0005


def run(env) -> None:
    with env.new_bot("FpH") as bot:
        snapshot = wait_join_and_inventory(bot)
        revision = snapshot["revision"]

        bot.cmd(f"give {MEAT_ITEM} 1")
        bot.expect_chat(f"[dev] gave {MEAT_ITEM} x1", timeout=10.0)
        give_anchor = time.monotonic()
        snapshot = wait_inventory_contains(bot, MEAT_ITEM, timeout=10.0)
        meat = require_item(snapshot, MEAT_ITEM)
        meat_instance = meat["item"]["instance_id"]

        # 1. Awaken → RealmTooLow → EventAlert 神识未及
        # Denied(RealmTooLow) 契约：同请求不得同时产出精确保鲜结果。水位必须在 intent
        # 之前截取——若在拒信（event_alert）消费后才锚定，先于拒信到达的
        # freshness_update 会被排除在静默窗口外，「先发精确保鲜、再发神识未及」的坏
        # 实现就撞不红（review finding 3/5）。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        alert = bot.expect_server_data("event_alert", timeout=10.0)
        message = alert.data["payload"].get("message", "")
        if "神识未及" not in message:
            raise BotAssertionError(
                f"[{bot.username}] 期望 EventAlert 含「神识未及」，实际 {message!r}"
            )
        # 该请求的契约 = 唯一响应是这条 event_alert；水位须在 intent 前（否则
        # 「先发 freshness_update、再发神识未及」的坏实现被排除）。已消费的 alert
        # 按 t 豁免，其余任何 server_data 一律判红（central-review 2029 #2）。
        _assert_no_freshness_update(
            bot,
            sent_at,
            "Awaken 保鲜探针被拒（RealmTooLow）后，同请求不得再产出 freshness_update",
            allowed_payload_ts=(alert.t,),
        )
        bot.assert_alive("Awaken 保鲜探针后")

        # 2. 凝脉 → FreshnessUpdate 精确结果
        bot.cmd("realm set condense")
        bot.expect_chat("[dev] realm set ", timeout=10.0)
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        update = bot.expect_server_data("freshness_update", timeout=10.0)
        payload = update.data["payload"]
        if str(payload.get("item_uuid")) != str(meat_instance):
            raise BotAssertionError(
                f"[{bot.username}] 期望 FreshnessUpdate.item_uuid={meat_instance}，"
                f"实际 {payload.get('item_uuid')}"
            )
        if payload.get("profile_name") != MEAT_PROFILE:
            raise BotAssertionError(
                f"[{bot.username}] 期望 FreshnessUpdate.profile_name={MEAT_PROFILE}，"
                f"实际 {payload.get('profile_name')}"
            )
        freshness = payload.get("freshness")
        # 保鲜是线性衰减（decay_per_tick=1/(24000×3)/tick，20 ticks/s → 2.78e-4/s），
        # 且必须随正 elapsed 真的下降——原断言上限 1.001 + 余量 0.005 覆盖了 ~18s 的
        # canonical 衰减，恒发 freshness=1.0（永不应用衰减）的坏实现任何窗口都通过
        # （central-review 2029 #7）。give→probe 窗口必然 ≥ ~4s：give 后先走 Awaken
        # RealmTooLow 的 4s 静默窗（SILENT_WINDOW）再 realm set 凝脉才探，故
        # elapsed≥SILENT_WINDOW 时上界 = 1 - 最小倍率×衰减×elapsed + tol 严格 < 1.0，
        # 1.0 必被拒；下限 = 1 - 最大倍率×衰减×elapsed - tol 同时防过度衰减/双倍扣减。
        elapsed = time.monotonic() - give_anchor
        max_decay = MEAT_DECAY_PER_SECOND * SEASON_DECAY_MAX * elapsed
        min_decay = MEAT_DECAY_PER_SECOND * SEASON_DECAY_MIN * elapsed
        lower = 1.0 - max_decay - FRESHNESS_TOLERANCE
        upper = 1.0 - min_decay + FRESHNESS_TOLERANCE
        if freshness is None or not (lower <= float(freshness) <= upper):
            raise BotAssertionError(
                f"[{bot.username}] 期望新物品 freshness 随正 elapsed 按 canonical 衰减"
                f"（give→probe {elapsed:.1f}s，季节×[{SEASON_DECAY_MIN},"
                f"{SEASON_DECAY_MAX}] → [{lower:.4f}, {upper:.4f}]），实际 {freshness}"
            )
        bot.assert_alive("凝脉保鲜探针后")

        # 3. 凝脉探无保鲜 item（trade_crate）→ NoFreshness 静默
        #    先清空背包：此前 give 的 meat + 出生物品已占满包，trade_crate 直接
        #    give 会被拒（回显 `give trade_crate failed: inventory full` 而非
        #    `gave ... x1`，expect_chat 超时）。clearinv 腾位后再 give。
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)
        bot.cmd(f"give {PLAIN_ITEM} 1")
        bot.expect_chat(f"[dev] gave {PLAIN_ITEM} x1", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)
        plain = require_item(snapshot, PLAIN_ITEM)
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent({**PROBE_REQUEST, "instance_id": plain["item"]["instance_id"]})
        _assert_no_freshness_update(bot, sent_at, "无保鲜 item 的探针应静默（NoFreshness 不发 S2C）")
        bot.assert_alive("无保鲜 freshness_probe 后")

        # 4. 不存在的 instance_id → dispatch belongs_to_player 前置静默丢弃
        #    （client_request_handler.rs belongs_to_player 检查；warn log 无 S2C）。
        #    此前全部请求都用当前背包快照拿到的实例，从不在生产路径送非法实例——
        #    跳过 belongs_to_player、去探他人/任意 item 的坏实现能通过全部旧断言
        #    （central-review 2029 #6）。999999 是合法 wire 值但不在任何背包。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent({**PROBE_REQUEST, "instance_id": 999999})
        _assert_no_freshness_update(
            bot,
            sent_at,
            "不存在的 instance_id 探针应被 dispatch 静默丢弃（belongs_to_player 拒绝）",
        )
        bot.assert_alive("freshness_probe 拒绝面全程")


def _assert_no_freshness_update(
    bot, sent_at: float, description: str, allowed_payload_ts: tuple = ()
) -> None:
    # 截止时刻用单调钟（time.monotonic），不用事件时间戳 bot.events[-1].t：
    # 静默断言正是"之后无事件到达"，事件时间不会推进，以事件时间做 deadline 会
    # 永远等不到 now >= end_at 而死循环（review finding 1/5）。
    deadline = time.monotonic() + SILENT_WINDOW
    while True:
        _scan_silent_violations(bot, sent_at, description, allowed_payload_ts)
        if time.monotonic() >= deadline:
            # 终末复扫：事件扫描与 deadline 判定非原子（review finding 3），deadline
            # 判定成立后、返回前再扫一次，收口最后一段未观测窗口。
            _scan_silent_violations(bot, sent_at, description, allowed_payload_ts)
            return
        bot.assert_alive(f"{description} 窗口内连接保持")
        time.sleep(0.1)


def _scan_silent_violations(bot, sent_at: float, description: str, allowed_payload_ts: tuple) -> None:
    # central-review 2029 #2：静默契约 = 「无任何非周期 S2C 响应 + 无聊天」。只盯
    # freshness_update 会放走拒收却发 event_alert / mineral_probe_result / 库存
    # 更新等任何其他 payload 的坏实现；白名单外 payload 一律判红。
    for e in bot.events_of("server_data"):
        if (
            e.t > sent_at
            and e.t not in allowed_payload_ts
            and e.data["payload_type"] not in AMBIENT_PERIODIC_PAYLOAD_TYPES
        ):
            raise BotAssertionError(
                f"[{bot.username}] {description}，"
                f"实际窗口内收到 server_data/{e.data['payload_type']}（t={e.t:.3f}）"
            )
    for e in bot.events_of("chat"):
        if e.t > sent_at:
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
            )
