"""freshness_probe 保鲜探针路径（实体/空间探知流 M4a，plan-exploration-probe-return-v1）。

resolve_one_probe（shelflife/probe.rs:101）检查顺序：
1. 修为 < 凝脉（MIN_PROBE_REALM_RANK=2）→ Denied(RealmTooLow) → EventAlert
   「神识未及，凝脉方可感知保鲜」；
2. item 无 freshness → Denied(NoFreshness) → 不发探针响应（freshness_probe_emit 对
   NoFreshness 一律 continue 不发 S2C）；前置同步先经同连接 ping fence 排到请求窗口外；
3. 通过 → Precise → `FreshnessUpdateV1 { item_uuid, freshness, profile_name }`
   （freshness = current_qi/initial_qi；**创建瞬间**为 1.0，但探针响应反映的是
   give→probe 已衰减后的比值，本场景断言其严格 < 1.0）。

dispatch 前置：instance_id 不在玩家背包 → 静默丢弃（client_request_handler
belongs_to_player 检查）。本场景用 `[dev] give` 构造合法背包 item：

1. Awaken 探煮熟肉（food.mundane.cooked_meat，shelflife_profile=
   food_spoil_mundane_meat_v1）→ event_alert 神识未及；
2. 凝脉后**两次探针自校准** → freshness_update（item_uuid=instance_id、
   profile_name=food_spoil_mundane_meat_v1；freshness=current_qi/initial_qi，
   give→probe1 已跨过 Awaken 拒绝与 realm set 的多次同步 fence，任意正常 tick 率下恒
   **< 1.0**）。两次探针的 (1-f2)/(1-f1) 必须等于墙钟比例 r=(t2-give)/(t1-give)
   ——(1-f)∝已过 tick 数，decay_per_tick/storage/season/initial_qi 全部消掉，对
   任意**稳定** tick 率成立，不依赖固定 20 TPS（review finding 2：慢 tick/加速
   tick 都会让旧固定 TPS 墙钟换算误判正确实现）；
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
from ._rejection_helpers import (
    ProtocolFence,
    server_data_protocol_fence,
    wait_for_event_after_cursor,
)

DESCRIPTION = "freshness_probe：Awaken→神识未及告警、凝脉→FreshnessUpdate、无保鲜/坏实例→静默"
MODULES = ["shelflife", "network"]

PROBE_REQUEST = {"type": "freshness_probe", "v": 1}
MEAT_ITEM = "food.mundane.cooked_meat"
MEAT_PROFILE = "food_spoil_mundane_meat_v1"
PLAIN_ITEM = "trade_crate"
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client
# （network/carrier_state_emit.rs，ticks % TICKS_PER_SECOND==0 周期）。请求前用同连接
# /ping fence 确认已到达的前置同步已经排到 Bot；请求窗口内不维护 payload type
# 排除集，所有 server_data 一律判红（central-review 2029 #2）。
# 探针路径 freshness = current_qi/initial_qi（shelflife/probe.rs，Linear：
# current = initial - decay_per_tick × storage×season × (now_tick-created_at_tick)）。
# 服务器主循环是 `app.update() + 5ms sleep`（main.rs:186），tick 率无上限也低于
# 20/s——client 拿不到 game tick，无法用固定 20 TPS 的墙钟换算去套绝对衰减量
# （review finding 2）。修法：两次探针自校准——(1-f) ∝ 已过 tick 数，两次探针的
# (1-f2)/(1-f1) = tick 数之比 = 墙钟比（tick 率稳定时），decay_per_tick / storage /
# season / initial_qi 全部在比值里消掉，对任意**稳定** tick 率（含慢于/快于 20）都成立。
PROBE_INTERVAL_S = 2.5
# 两次测量窗之间的 tick 率漂移容差：墙体比例 r 用 give→probe1 的墙钟算出，期望
# (1-f2) = (1-f1)×r 的前提是两次窗内 tick 率一致；真实 fixture 同场景内 tick 率稳定，
# 慢 tick / 启动 catch-up 只造成 ±50% 内的窗间漂移。统一倍率错（2× per-tick 衰减）
# 与 2× tick 率在 client 侧不可区分，比值法不锁它——这是无 game tick 观测下的
# 最大可区分度。
TICK_RATE_DRIFT = 0.5
# 比值法的绝对容差：有效 dt 的 round()（compute.rs:249）、give 处理延迟对 r 的偏移、
# freshness 的 f32 序列化噪声。留 0.0005 与旧断言同量级。
FRESHNESS_TOLERANCE = 0.0005


def _probe_payload_freshness(bot, update, meat_instance: int) -> float:
    """校验 freshness_update 的 item_uuid/profile_name 并返回 freshness 值。"""
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
    return payload.get("freshness")


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
        # Denied(RealmTooLow) 契约：同请求不得同时产出精确保鲜结果。请求前 fence
        # 先建立 lower watermark，避免把前置同步误归因于本次请求；收到预期告警后
        # 再用 upper fence 收口，两个水位之间的所有 server_data 都必须逐条核验。
        start_fence = server_data_protocol_fence(bot)
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        alert = wait_for_event_after_cursor(
            bot,
            start_fence.cursor,
            lambda event: (
                event.kind == "server_data"
                and event.data["payload_type"] == "event_alert"
            ),
            timeout=10.0,
            description="Awaken freshness_probe 的 event_alert 拒绝告警",
        )
        message = alert.data["payload"].get("message", "")
        if "神识未及" not in message:
            raise BotAssertionError(
                f"[{bot.username}] 期望 EventAlert 含「神识未及」，实际 {message!r}"
            )
        end_fence = server_data_protocol_fence(bot)
        # 该请求的契约 = 唯一 server_data 响应是这条 event_alert；任何其它类型、
        # 解码失败事件或额外聊天都判红（central-review 2029 #2）。
        _assert_no_freshness_update(
            bot,
            start_fence.cursor,
            end_fence.cursor,
            "Awaken 保鲜探针被拒（RealmTooLow）后，同请求不得再产出 freshness_update",
            allowed_server_data_events=(alert,),
            allowed_chat_events=end_fence.markers,
        )
        bot.assert_alive("Awaken 保鲜探针后")

        # 2. 凝脉 → FreshnessUpdate 精确结果
        #    realm set 恒触发 Changed<Cultivation> → player_state 回推给自己（gap10
        #    _realm_set_and_settle 同款）；必须先等它落定再取水位，否则回推会落入
        #    成功路径的响应基数窗口、被判额外 payload 假红（central-review
        #    31437496353 #5）。
        bot.cmd("realm set condense")
        confirm = bot.expect_chat("[dev] realm set ", timeout=10.0)
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data["payload_type"] == "player_state"
                and e.t >= confirm.t
            ),
            timeout=5.0,
            description="realm set condense 的 player_state 回推应已到达",
        )
        # realm set 的同步流先由 upper fence 收口；成功探针从独立的 lower fence
        # 开始，避免把 realm set 的滞后回推带入本次响应窗口。
        _settle_realm_change(bot)
        probe1_start = server_data_protocol_fence(bot)
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        update1 = wait_for_event_after_cursor(
            bot,
            probe1_start.cursor,
            lambda event: (
                event.kind == "server_data"
                and event.data["payload_type"] == "freshness_update"
            ),
            timeout=10.0,
            description="第一次 freshness_probe 的 freshness_update",
        )
        f1 = _probe_payload_freshness(bot, update1, meat_instance)
        probe1_wall = time.monotonic()
        probe1_end = server_data_protocol_fence(bot)
        _assert_no_freshness_update(
            bot,
            probe1_start.cursor,
            probe1_end.cursor,
            "第一次凝脉保鲜探针成功后，同请求不得再产出额外 server_data 或聊天",
            allowed_server_data_events=(update1,),
            allowed_chat_events=probe1_end.markers,
        )
        # 两次探针自校准（review finding 2）：等 PROBE_INTERVAL_S 让 decay 有足够 tick
        # 推进，第二次探针验证衰减**延续**在 (1-f1) 与墙钟比例定的衰减线上——对任意
        # 稳定 tick 率成立（见 TICK_RATE_DRIFT 注释）。第二次探针须按独立水位锚定，
        # 让两次探针之间的 ambient 流落在请求窗口之外。
        time.sleep(PROBE_INTERVAL_S)
        # 两次探针之间的 ambient 流属于两次请求之外；第二次请求前再建 lower fence
        # 把它们排到窗口之外，而不是在断言侧维护类型排除集。
        probe2_start = server_data_protocol_fence(bot)
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        update2 = wait_for_event_after_cursor(
            bot,
            probe2_start.cursor,
            lambda e: (
                e.kind == "server_data"
                and e.data["payload_type"] == "freshness_update"
            ),
            timeout=10.0,
            description="等第二次 freshness_update（时间隔离后的衰减样本）",
        )
        f2 = _probe_payload_freshness(bot, update2, meat_instance)
        probe2_wall = time.monotonic()
        probe2_end = server_data_protocol_fence(bot)
        _assert_no_freshness_update(
            bot,
            probe2_start.cursor,
            probe2_end.cursor,
            "第二次凝脉保鲜探针成功后，同请求不得再产出额外 server_data 或聊天",
            allowed_server_data_events=(update2,),
            allowed_chat_events=probe2_end.markers,
        )
        # f1 必须严格 <1.0：give→probe1 已跨过 Awaken 拒绝、realm set 和多次同步
        # fence，任意正常 tick 率都推进了 ≥1 tick；恒发 freshness=1.0（永不应用衰减）
        # 的坏实现在此必红（central-review 2029 #7 的判别面原样保留）。
        if f1 is None or not (0.0 < float(f1) < 1.0):
            raise BotAssertionError(
                f"[{bot.username}] 期望首次 freshness 严格 <1.0（已衰减，非恒发 1.0）"
                f"且 >0，实际 {f1}"
            )
        if f2 is None or not (0.0 < float(f2) < 1.0):
            raise BotAssertionError(
                f"[{bot.username}] 期望第二次 freshness 严格 <1.0 且 >0，实际 {f2}"
            )
        # 衰减必须继续：第二次探针 freshness 严格低于第一次（期间必推进 ≥1 tick）。
        if not (float(f2) < float(f1)):
            raise BotAssertionError(
                f"[{bot.username}] 期望 freshness 随时间递减（decay 延续），"
                f"实际 {float(f1)} → {float(f2)}"
            )
        # 比值校准：(1-f) ∝ 已过 tick 数（Linear、storage×season 乘子在两次探针间不变），
        # 稳定 tick 率下 (1-f2)/(1-f1) = (t2-created)/(t1-created) = 墙钟比例 r。
        # decay_per_tick/storage/season/initial_qi 全部消掉，不依赖任何固定 TPS。
        r = (probe2_wall - give_anchor) / (probe1_wall - give_anchor)
        expected_remaining2 = (1.0 - float(f1)) * r
        remaining2 = 1.0 - float(f2)
        lo = expected_remaining2 * (1.0 - TICK_RATE_DRIFT) - FRESHNESS_TOLERANCE
        hi = expected_remaining2 * (1.0 + TICK_RATE_DRIFT) + FRESHNESS_TOLERANCE
        if not (lo <= remaining2 <= hi):
            raise BotAssertionError(
                f"[{bot.username}] 期望第二次探针衰减量落在按墙钟比例自校准的衰减线上"
                f"（give→probe1 {probe1_wall - give_anchor:.1f}s，r={r:.3f}，"
                f"期望 (1-f2)∈[{lo:.5f}, {hi:.5f}]，按漂移±{TICK_RATE_DRIFT:.0%}），"
                f"实际 (1-f1)={(1.0 - float(f1)):.5f} → (1-f2)={remaining2:.5f}，"
                f"f1={float(f1)} f2={float(f2)}"
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
        start_fence = server_data_protocol_fence(bot)
        bot.intent({**PROBE_REQUEST, "instance_id": plain["item"]["instance_id"]})
        # 无响应请求用两次 ping 往返：第一条可能与 client-request ingress 落在同一
        # Update，第二条确保请求已被处理并 flush；两条 pong 都只是 fence 自身的允许
        # 标记，窗口内其它 server_data/chat 一律判红。
        end_fence = server_data_protocol_fence(bot, round_trips=2)
        _assert_no_freshness_update(
            bot,
            start_fence.cursor,
            end_fence.cursor,
            "无保鲜 item 的探针应静默（NoFreshness 不发 S2C）",
            allowed_chat_events=end_fence.markers,
        )
        bot.assert_alive("无保鲜 freshness_probe 后")

        # 4. 不存在的 instance_id → dispatch belongs_to_player 前置静默丢弃
        #    （client_request_handler.rs belongs_to_player 检查；warn log 无 S2C）。
        #    此前全部请求都用当前背包快照拿到的实例，从不在生产路径送非法实例——
        #    跳过 belongs_to_player、去探他人/任意 item 的坏实现能通过全部旧断言
        #    （central-review 2029 #6）。999999 是合法 wire 值但不在任何背包。
        start_fence = server_data_protocol_fence(bot)
        bot.intent({**PROBE_REQUEST, "instance_id": 999999})
        end_fence = server_data_protocol_fence(bot, round_trips=2)
        _assert_no_freshness_update(
            bot,
            start_fence.cursor,
            end_fence.cursor,
            "不存在的 instance_id 探针应被 dispatch 静默丢弃（belongs_to_player 拒绝）",
            allowed_chat_events=end_fence.markers,
        )
        bot.assert_alive("freshness_probe 拒绝面全程")


def _settle_realm_change(bot) -> ProtocolFence:
    """用同连接 ping fence 收口 realm set 的异步同步流。"""
    return server_data_protocol_fence(bot)


def _assert_no_freshness_update(
    bot,
    start_cursor: int,
    end_cursor: int,
    description: str,
    allowed_server_data_events: tuple = (),
    allowed_chat_events: tuple = (),
) -> None:
    _scan_silent_violations(
        bot,
        start_cursor,
        end_cursor,
        description,
        allowed_server_data_events,
        allowed_chat_events,
    )


def _scan_silent_violations(
    bot,
    start_cursor: int,
    end_cursor: int,
    description: str,
    allowed_server_data_events: tuple,
    allowed_chat_events: tuple,
) -> None:
    # central-review 2029 #2：fence 区间契约 = 「除明确预期 server_data 外无任何
    # server_data 响应 + 除 fence pong 外无聊天」。不按 payload type 维护豁免集；无法
    # 解码的 server_data 也直接判红，避免未知类型再次静默消失。
    allowed_server_data_ids = {id(event) for event in allowed_server_data_events}
    allowed_chat_ids = {id(event) for event in allowed_chat_events}
    window = bot.events[start_cursor:end_cursor]
    for index, e in enumerate(window):
        if e.kind == "server_data":
            if id(e) not in allowed_server_data_ids:
                raise BotAssertionError(
                    f"[{bot.username}] {description}，"
                    f"实际窗口内收到 server_data/{e.data['payload_type']}（t={e.t:.3f}）"
                )
        elif e.kind == "server_data_raw":
            next_event = window[index + 1] if index + 1 < len(window) else None
            if next_event is None or next_event.kind not in (
                "server_data",
                "server_data_decode_error",
            ):
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际窗口内收到未解码的 server_data"
                )
        elif e.kind == "server_data_decode_error":
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际窗口内收到无法解码的 server_data"
                f"（{e.data.get('error')}）"
            )
        elif e.kind == "chat" and id(e) not in allowed_chat_ids:
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
            )
