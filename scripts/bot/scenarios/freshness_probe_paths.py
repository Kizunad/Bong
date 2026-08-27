"""freshness_probe 保鲜探针路径（实体/空间探知流 M4a，plan-exploration-probe-return-v1）。

resolve_one_probe（shelflife/probe.rs:101）检查顺序：
1. 修为 < 凝脉（MIN_PROBE_REALM_RANK=2）→ Denied(RealmTooLow) → EventAlert
   「神识未及，凝脉方可感知保鲜」；
2. item 无 freshness → Denied(NoFreshness) → 静默（freshness_probe_emit 对
   NoFreshness 一律 continue 不发 S2C）；
3. 通过 → Precise → `FreshnessUpdateV1 { item_uuid, freshness, profile_name }`
   （freshness = current_qi/initial_qi；**创建瞬间**为 1.0，但探针响应反映的是
   give→probe 已衰减后的比值，本场景断言其严格 < 1.0）。

dispatch 前置：instance_id 不在玩家背包 → 静默丢弃（client_request_handler
belongs_to_player 检查）。本场景用 `[dev] give` 构造合法背包 item：

1. Awaken 探煮熟肉（food.mundane.cooked_meat，shelflife_profile=
   food_spoil_mundane_meat_v1）→ event_alert 神识未及；
2. 凝脉后**两次探针自校准** → freshness_update（item_uuid=instance_id、
   profile_name=food_spoil_mundane_meat_v1；freshness=current_qi/initial_qi，
   give→probe1 已过 Awaken 拒绝的 4s 静默窗 + realm set，任意正常 tick 率下恒
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
    AMBIENT_SERVER_DATA_TYPES,
    _relative_now,
    drain_event_stream,
)

DESCRIPTION = "freshness_probe：Awaken→神识未及告警、凝脉→FreshnessUpdate、无保鲜/坏实例→静默"
MODULES = ["shelflife", "network"]

PROBE_REQUEST = {"type": "freshness_probe", "v": 1}
MEAT_ITEM = "food.mundane.cooked_meat"
MEAT_PROFILE = "food_spoil_mundane_meat_v1"
PLAIN_ITEM = "trade_crate"
SILENT_WINDOW = 4.0
REALM_SYNC_DRAIN_MAX = SILENT_WINDOW * 3.0
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client
# （network/carrier_state_emit.rs，ticks % TICKS_PER_SECOND==0 周期）。
# player_state / inventory_snapshot 在本场景只随 Changed 组件发射（gap9 无
# 周期性无变化 flush），窗口内无合法非白名单 payload——白名单外一律判红
# （central-review 2029 #2）。carrier_state 不在 proto_min 白名单，通常不
# 解码成 server_data 事件；保留它只为显式豁免未来 proto_min 收录后的周期流。
AMBIENT_PERIODIC_PAYLOAD_TYPES = AMBIENT_SERVER_DATA_TYPES
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
        # Denied(RealmTooLow) 契约：同请求不得同时产出精确保鲜结果。水位必须在 intent
        # 之前截取——若在拒信（event_alert）消费后才锚定，先于拒信到达的
        # freshness_update 会被排除在静默窗口外，「先发精确保鲜、再发神识未及」的坏
        # 实现就撞不红（review finding 3/5）。
        sent_at = _relative_now(bot)
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
        # realm set 会异步触发 narration 等连接同步；服务端日志中的发送完成不等于
        # Bot reader 已收到（CI 高负载时实测可滞后约 1.5s）。排空上限必须覆盖完整的
        # 静默观察窗，并在排空完成后重新取水位，避免上一条命令的滞后事件被误归因于
        # freshness_probe。成功探针自身的副作用仍会在后面的静默扫描中撞红。
        sent_at = _settle_realm_change(bot)
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        update1 = bot.expect_server_data("freshness_update", timeout=10.0)
        f1 = _probe_payload_freshness(bot, update1, meat_instance)
        probe1_wall = time.monotonic()
        # 两次探针自校准（review finding 2）：等 PROBE_INTERVAL_S 让 decay 有足够 tick
        # 推进，第二次探针验证衰减**延续**在 (1-f1) 与墙钟比例定的衰减线上——对任意
        # 稳定 tick 率成立（见 TICK_RATE_DRIFT 注释）。第二次探针须按水位锚定：update1
        # 已在历史中，expect_server_data 只匹配第一条会拿错 payload。
        time.sleep(PROBE_INTERVAL_S)
        bot.intent({**PROBE_REQUEST, "instance_id": meat_instance})
        update2 = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data["payload_type"] == "freshness_update"
                and e.t > update1.t
            ),
            timeout=10.0,
            description="等第二次 freshness_update（时间隔离后的衰减样本）",
        )
        f2 = _probe_payload_freshness(bot, update2, meat_instance)
        probe2_wall = time.monotonic()
        # f1 必须严格 <1.0：give→probe1 已过 ≥ ~4s（Awaken 静默窗 + realm set），任意
        # 正常 tick 率都推进了 ≥1 tick，恒发 freshness=1.0（永不应用衰减）的坏实现在此
        # 必红（central-review 2029 #7 的判别面原样保留）。
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
        # central-review 31437496353 #5：成功路径也必须断言响应基数——拒绝路径都有
        # 静默窗口，唯独成功路径只等 freshness_update，放走「正确结果之外再发
        # event_alert / 库存更新 / 重复 freshness_update / 聊天」的坏实现。水位在
        # 首个 intent 前，两条已消费的 update 按 t 豁免，窗口内其余 server_data/聊天
        # 一律判红。
        _assert_no_freshness_update(
            bot,
            sent_at,
            "凝脉保鲜探针成功后，同请求不得再产出额外 server_data 或聊天",
            allowed_payload_ts=(update1.t, update2.t),
        )

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
        sent_at = _relative_now(bot)
        bot.intent({**PROBE_REQUEST, "instance_id": plain["item"]["instance_id"]})
        _assert_no_freshness_update(bot, sent_at, "无保鲜 item 的探针应静默（NoFreshness 不发 S2C）")
        bot.assert_alive("无保鲜 freshness_probe 后")

        # 4. 不存在的 instance_id → dispatch belongs_to_player 前置静默丢弃
        #    （client_request_handler.rs belongs_to_player 检查；warn log 无 S2C）。
        #    此前全部请求都用当前背包快照拿到的实例，从不在生产路径送非法实例——
        #    跳过 belongs_to_player、去探他人/任意 item 的坏实现能通过全部旧断言
        #    （central-review 2029 #6）。999999 是合法 wire 值但不在任何背包。
        sent_at = _relative_now(bot)
        bot.intent({**PROBE_REQUEST, "instance_id": 999999})
        _assert_no_freshness_update(
            bot,
            sent_at,
            "不存在的 instance_id 探针应被 dispatch 静默丢弃（belongs_to_player 拒绝）",
        )
        bot.assert_alive("freshness_probe 拒绝面全程")


def _settle_realm_change(bot) -> float:
    """排干 realm set 的异步同步流，并返回与 ``event.t`` 同钟的请求锚点。"""
    drain_event_stream(
        bot,
        quiet_s=SILENT_WINDOW,
        max_s=REALM_SYNC_DRAIN_MAX,
    )
    return _relative_now(bot)


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
