"""NPC 对话链路（inspect / dialogue_choice / trade）场景 helper。

下划线前缀让 run_scenarios 跳过本模块。

协议层事实（server/src/network/client_request_handler.rs，plan-npc-engagement-v1）：
- 三类请求经 `bong:client_request` ClientRequestV1 下发，服务端校验三件事：
  同维度、≤6.0m（NPC_INTERACTION_MAX_DISTANCE）、未 Terminated。
  任一不满足 → 无 § 前缀的 "[NPC] 目标已不在附近，无法…。" 兜底。
- 反馈一律走 chat（§ 码原样保留），无 server_data 通道。
- /npc_scenario chase 生成 zombie（EntityKind::ZOMBIE=118，Archetype=Zombie、Realm=Awaken）
  → display_name "游尸·醒灵"，inspect greeting "游尸没有回应。"，can_trade=false；
  chase thinker 只追不咬（无 MeleeRangeScorer/MeleeAttackAction），对话场景零战斗风险。
- BONG_ROGUE_SEED_COUNT>0 播种的散修由 `bong:npc_metadata` 明确标注 archetype=rogue，
  metadata 同时携带真实 `trade_offers`，避免把环境 villager 或高境界库存误认成商贩；
  display_name "散修·{醒灵|引气|凝脉|固元|通灵|化虚}"，greeting "道友，可有灵草出让？"，
  can_trade=true（fresh 玩家 rep=0 ≥ -30、FactionReputationTier::Normal）。
- 交易判定顺序：offered_items → 目录查找（npc_trade_catalog_entry 按 archetype
  键控，仅 Commoner|Rogue 有条目）→ can_trade → 库存命中 → 定价(rep) → 骨币余额
  → 入包。zombie 无目录条目，"不做买卖"（can_trade=false）分支经 zombie 不可达
  （目录查找先于 can_trade），需 Rogue/Commoner 且 rep < -30——fixture 不可构造。
- 交易定价：fresh 玩家 rep_f32=0.5 → RepTier::Mid → 1.0x，base 价
  spirit_grass=10 / ling_xi_wan_flawed=8 / ju_ling_dan_flawed=15。
- 散修交易库存随机 1-3/3（entity.index() 播种）：场景从 metadata 读取实际目录条目，
  反馈要么 "当前没有这件货"，要么（命中库存）"骨币不足，需要 N 枚。"
  —— fresh 玩家出生仅 7 骨币（assets/inventory/loadouts/default.toml），
  而 Awaken 档最低价 8，命中必报骨币不足；三件里必有一件命中（库存非空）。
"""

from __future__ import annotations

import json
import math
import time

from bot.bot import Bot, BotAssertionError, Event

ZOMBIE_ENTITY_TYPE = 118

OUT_OF_RANGE_INSPECT = "[NPC] 目标已不在附近，无法查看。"
OUT_OF_RANGE_CHOICE = "[NPC] 目标已不在附近，无法交谈。"
OUT_OF_RANGE_TRADE = "[NPC] 目标已不在附近，无法交易。"

# 散修 display_name 的正典境界段（server/src/network/npc_metadata.rs realm_label，
# Realm::Awaken..Void 六个枚举值；display_name = "{archetype}·{realm}"）。
ROGUE_REALMS = ("醒灵", "引气", "凝脉", "固元", "通灵", "化虚")

ROGUE_TRADE_PREFIX = "§c[NPC] 散修·"
ROGUE_TRADE_STOCK_MISS_SUFFIX = " 当前没有这件货。"
SCENARIO_SPAWN_RADIUS = 12.0
SCENARIO_SPAWN_TOLERANCE = 1.5


def last_event_time(bot: Bot) -> float:
    with bot._lock:
        return bot.events[-1].t if bot.events else 0.0


def _distance(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    return math.dist(a, b)


def _scenario_spawn_matches(
    event: Event, origin: tuple[float, float, float]
) -> bool:
    """只接受 chase 单体固定的 +X 方向 12 格出生点，排除环境僵尸。"""
    if event.kind != "entity_spawn" or event.data.get("type") != ZOMBIE_ENTITY_TYPE:
        return False
    try:
        spawn = (float(event.data["x"]), float(event.data["y"]), float(event.data["z"]))
    except (KeyError, TypeError, ValueError):
        return False
    expected = (origin[0] + SCENARIO_SPAWN_RADIUS, origin[1], origin[2])
    return _distance(spawn, expected) <= SCENARIO_SPAWN_TOLERANCE


def queue_scenario_zombie(bot: Bot) -> Event:
    """`/npc_scenario chase` 生成 zombie 并等 entity_spawn（12m 圆周，chase thinker 只追不咬）。"""
    if bot.position is None:
        raise BotAssertionError("期望已有 bot.position 后再生成场景 NPC，实际 position=None")
    origin = bot.position
    anchor = last_event_time(bot)
    bot.cmd("npc_scenario chase")
    bot.expect_chat("Scenario queued.", timeout=10.0)
    spawn = bot.wait_for(
        lambda e: e.t > anchor
        and e.data.get("entity_id") != bot.entity_id
        and _scenario_spawn_matches(e, origin),
        timeout=15.0,
        description="/npc_scenario chase 后固定 +X 12 格出现 zombie(118) entity_spawn",
    )
    # 同一窗口内若有第二个候选，说明坐标绑定仍有歧义，不能把错误实体交给后续请求。
    deadline = time.monotonic() + 0.25
    while time.monotonic() < deadline:
        with bot._lock:
            candidates = [
                e
                for e in bot.events
                if e.t > anchor
                and e.data.get("entity_id") != bot.entity_id
                and _scenario_spawn_matches(e, origin)
            ]
        if len(candidates) > 1:
            raise BotAssertionError(
                " /npc_scenario chase 出生窗口出现多个同坐标 zombie 候选，拒绝歧义绑定"
            )
        time.sleep(0.02)
    return spawn


def approach_entity(
    bot: Bot, entity_id: int, range_m: float = 3.0, max_iter: int = 30
) -> bool:
    """朝实体的最近已知位置移动，直到 bot 与它的欧氏距离 ≤ range_m。"""
    for _ in range(max_iter):
        pos = bot.entity_pos(entity_id)
        if pos is None:
            return False
        if bot.position is not None and _distance(bot.position, pos) <= range_m:
            return True
        bot.move_to(pos[0], pos[1], pos[2], speed=5.5)
        time.sleep(0.1)
    return False


def _assert_feedback_exact(event: Event, expected: str, description: str) -> None:
    if event.data["text"] != expected:
        raise BotAssertionError(
            f"期望 {description} 反馈与协议逐字一致：{expected!r}，实际 {event.data['text']!r}"
        )


def request_and_assert(
    bot: Bot,
    request: dict,
    entity_id: int,
    expected: str,
    description: str,
    out_of_range: str,
    retries: int = 5,
) -> None:
    """发请求并断言逐字反馈；目标走远（"不在附近"）时立即识别拒绝并重新逼近后重试。

    wait 谓词同时接收成功与越距两类反馈，越距在 server 端是即时回显（NPC 超出 6m
    判定），不必等满 8s 成功超时才从事件历史里翻出——恢复延迟从每次 ~8s 降到响应即回。
    """
    for attempt in range(retries):
        anchor = last_event_time(bot)
        bot.intent(request)
        try:
            event = bot.wait_for(
                lambda e: (
                    e.kind == "chat"
                    and e.t > anchor
                    and (e.data["text"] == expected or e.data["text"] == out_of_range)
                ),
                timeout=8.0,
                description=description,
            )
        except BotAssertionError:
            with bot._lock:
                stray = [
                    e
                    for e in bot.events
                    if e.kind == "chat" and e.t > anchor and e.data["text"] == out_of_range
                ]
            if not stray:
                raise
            event = stray[-1]
        if event.data["text"] == out_of_range:
            if not approach_entity(bot, entity_id, range_m=3.0):
                raise BotAssertionError(
                    f"{description}：实体 {entity_id} 丢失（destroy/despawn），无法重试逼近"
                )
            continue
        _assert_feedback_exact(event, expected, description)
        return
    raise BotAssertionError(
        f"{description} 重试 {retries} 次（每次重新逼近）仍未命中期望反馈 {expected!r}"
    )


def _trade_metadata(event: Event) -> tuple[int, list[tuple[str, int]]] | None:
    """解析 NPC metadata 中可成交的 Rogue 及其真实库存。"""
    if event.kind != "payload" or event.data.get("channel") != "bong:npc_metadata":
        return None
    try:
        payload = json.loads(event.data["data"].decode("utf-8"))
        if payload.get("type") != "npc_metadata" or payload.get("archetype") != "rogue":
            return None
        entity_id = int(payload["entity_id"])
        offers = [
            (str(offer["template_id"]), int(offer["price_bone_coins"]))
            for offer in payload.get("trade_offers", [])
        ]
    except (AttributeError, KeyError, TypeError, ValueError, json.JSONDecodeError):
        return None
    return (entity_id, offers) if offers else None


def wait_for_rogue_with_inventory(
    bot: Bot, timeout: float, description: str
) -> tuple[int, list[tuple[str, int]]]:
    """只选择 metadata 明确声明为 Rogue 且库存非空的 NPC。"""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        best: tuple[int, list[tuple[str, int]], float] | None = None
        with bot._lock:
            metadata_events = list(bot.events)
        for event in metadata_events:
            parsed = _trade_metadata(event)
            if parsed is None:
                continue
            entity_id, offers = parsed
            pos = bot.entity_pos(entity_id)
            if pos is None or bot.position is None:
                continue
            candidate = (entity_id, offers, _distance(bot.position, pos))
            if best is None or candidate[2] < best[2]:
                best = candidate
        if best is not None:
            return best[0], best[1]
        time.sleep(0.25)
    raise BotAssertionError(
        f"{description}：{timeout:.0f}s 内未收到 archetype=rogue 且 trade_offers 非空的 npc_metadata；"
        "请以 BONG_ROGUE_SEED_COUNT>0 启动 server"
    )


def rogue_display_prefix() -> str:
    return "§7[NPC] 散修·"


def _canonical_realm_between(text: str, prefix: str, suffix: str) -> str | None:
    """文本若为 prefix + 正典境界 + suffix 的逐字形态，返回该境界段，否则 None。"""
    if not (text.startswith(prefix) and text.endswith(suffix)):
        return None
    realm = text[len(prefix) : len(text) - len(suffix)]
    return realm if realm in ROGUE_REALMS else None


def assert_rogue_display_chat(event: Event, suffix: str, description: str) -> None:
    """散修反馈逐字契约：§7[NPC] 散修·{六正典境界}{suffix}。

    display_name 由 server 的 `format!("{}·{}", archetype_label, realm_label)` 生成，
    境界段必须是 npc_metadata.rs 六正典 realm_label 之一；前缀/后缀锚定 + 境界段
    枚举校验 = 全串逐字等价，legacy 或 misdecoded 的 realm 值都会在这里显式失败。
    """
    text = event.data["text"]
    realm = _canonical_realm_between(text, rogue_display_prefix(), suffix)
    if realm is None:
        raise BotAssertionError(
            f"期望 {description} 反馈为「散修·<境界>{suffix}」逐字契约（境界 ∈ "
            f"{'/'.join(ROGUE_REALMS)}），实际 {text!r}"
        )


def is_rogue_stock_miss(text: str) -> bool:
    """散修目录品库存未命中反馈的逐字形态：§c[NPC] 散修·<境界> 当前没有这件货。"""
    return _canonical_realm_between(
        text, ROGUE_TRADE_PREFIX, ROGUE_TRADE_STOCK_MISS_SUFFIX
    ) is not None


def assert_rogue_stock_miss(event: Event, description: str) -> None:
    """目录品库存未命中反馈逐字契约：§c[NPC] 散修·{六正典境界} 当前没有这件货。"""
    realm = _canonical_realm_between(
        event.data["text"], ROGUE_TRADE_PREFIX, ROGUE_TRADE_STOCK_MISS_SUFFIX
    )
    if realm is None:
        raise BotAssertionError(
            f"期望 {description} 反馈为「散修·<境界> 当前没有这件货。」逐字契约（境界 ∈ "
            f"{'/'.join(ROGUE_REALMS)}），实际 {event.data['text']!r}"
        )


def _is_rogue_out_of_range(text: str) -> bool:
    return text in (OUT_OF_RANGE_INSPECT, OUT_OF_RANGE_CHOICE, OUT_OF_RANGE_TRADE)


def request_and_assert_rogue(
    bot: Bot,
    request: dict,
    entity_id: int,
    suffix: str,
    description: str,
    retries: int = 5,
) -> None:
    """发请求并断言「散修·<境界>…suffix」逐字反馈（display_name 境界段为正典枚举）。

    与 request_and_assert 相同：wait 谓词同时接收成功（逐字形态）与三类越距拒绝，
    越距即时回显不消耗 8s 成功超时；断言走 assert_rogue_display_chat 的境界段校验。
    """
    for attempt in range(retries):
        anchor = last_event_time(bot)
        bot.intent(request)
        try:
            event = bot.wait_for(
                lambda e: (
                    e.kind == "chat"
                    and e.t > anchor
                    and (
                        _canonical_realm_between(
                            e.data["text"], rogue_display_prefix(), suffix
                        )
                        is not None
                        or _is_rogue_out_of_range(e.data["text"])
                    )
                ),
                timeout=8.0,
                description=description,
            )
        except BotAssertionError:
            with bot._lock:
                stray = [
                    e
                    for e in bot.events
                    if e.kind == "chat"
                    and e.t > anchor
                    and _is_rogue_out_of_range(e.data["text"])
                ]
            if not stray:
                raise
            event = stray[-1]
        if _is_rogue_out_of_range(event.data["text"]):
            if not approach_entity(bot, entity_id, range_m=3.0):
                raise BotAssertionError(
                    f"{description}：实体 {entity_id} 丢失（destroy/despawn），无法重试逼近"
                )
            continue
        assert_rogue_display_chat(event, suffix, description)
        return
    raise BotAssertionError(
        f"{description} 重试 {retries} 次（每次重新逼近）仍未命中「散修·…{suffix}」反馈"
    )


def expect_no_npc_chat_after(bot: Bot, anchor: float, window: float, description: str) -> None:
    """锚点后 window 秒内不得出现任何 [NPC] 反馈 chat（"leave" 分支无回显的协议断言）。"""
    time.sleep(window)
    with bot._lock:
        stray = [
            e
            for e in bot.events
            if e.kind == "chat" and e.t > anchor and "[NPC]" in e.data["text"]
        ]
    if stray:
        raise BotAssertionError(f"期望 {description}，实际收到 {len(stray)} 条 [NPC] chat：{stray!r}")


def rogue_village_pos_from_tppoi(bot: Bot) -> tuple[float, float, float]:
    import re

    bot.cmd("tppoi novice")
    bot.expect_chat("[dev] novice_poi registry count=", timeout=10.0)
    detail = bot.expect_chat("[dev] novice_poi rogue_village ", timeout=10.0)
    match = re.search(r"pos=(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)", detail.data["text"])
    if match is None:
        raise BotAssertionError(
            f"期望 /tppoi novice 输出 rogue_village 坐标，实际 chat={detail.data['text']!r}"
        )
    return (float(match.group(1)), float(match.group(2)), float(match.group(3)))
