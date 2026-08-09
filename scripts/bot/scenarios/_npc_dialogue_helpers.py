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
- BONG_ROGUE_SEED_COUNT>0 播种的散修走 villager fallback（EntityKind::VILLAGER=108），
  display_name "散修·{醒灵|引气|凝脉|固元|通灵|化虚}"，greeting "道友，可有灵草出让？"，
  can_trade=true（fresh 玩家 rep=0 ≥ -30、FactionReputationTier::Normal）。
- 交易判定顺序：offered_items → 目录查找（npc_trade_catalog_entry 按 archetype
  键控，仅 Commoner|Rogue 有条目）→ can_trade → 库存命中 → 定价(rep) → 骨币余额
  → 入包。zombie 无目录条目，"不做买卖"（can_trade=false）分支经 zombie 不可达
  （目录查找先于 can_trade），需 Rogue/Commoner 且 rep < -30——fixture 不可构造。
- 交易定价：fresh 玩家 rep_f32=0.5 → RepTier::Mid → 1.0x，base 价
  spirit_grass=10 / ling_xi_wan_flawed=8 / ju_ling_dan_flawed=15。
- 散修交易库存随机 1-3/3（entity.index() 播种）：请求任意目录条目，
  反馈要么 "当前没有这件货"，要么（命中库存）"骨币不足，需要 N 枚。"
  —— fresh 玩家出生仅 7 骨币（assets/inventory/loadouts/default.toml），
  而 Awaken 档最低价 8，命中必报骨币不足；三件里必有一件命中（库存非空）。
"""

from __future__ import annotations

import math
import time

from bot.bot import Bot, BotAssertionError, Event

VILLAGER_ENTITY_TYPE = 108
ZOMBIE_ENTITY_TYPE = 118

CATALOG_ITEMS = [
    ("spirit_grass", 10),
    ("ling_xi_wan_flawed", 8),
    ("ju_ling_dan_flawed", 15),
]

OUT_OF_RANGE_INSPECT = "[NPC] 目标已不在附近，无法查看。"
OUT_OF_RANGE_CHOICE = "[NPC] 目标已不在附近，无法交谈。"
OUT_OF_RANGE_TRADE = "[NPC] 目标已不在附近，无法交易。"


def last_event_time(bot: Bot) -> float:
    with bot._lock:
        return bot.events[-1].t if bot.events else 0.0


def _distance(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    return math.dist(a, b)


def queue_scenario_zombie(bot: Bot) -> Event:
    """`/npc_scenario chase` 生成 zombie 并等 entity_spawn（12m 圆周，chase thinker 只追不咬）。"""
    if bot.position is None:
        raise BotAssertionError("期望已有 bot.position 后再生成场景 NPC，实际 position=None")
    anchor = last_event_time(bot)
    bot.cmd("npc_scenario chase")
    bot.expect_chat("Scenario queued.", timeout=10.0)
    return bot.wait_for(
        lambda e: e.kind == "entity_spawn"
        and e.t > anchor
        and e.data.get("entity_id") != bot.entity_id
        and e.data.get("type") == ZOMBIE_ENTITY_TYPE
        and _distance(bot.position, (e.data["x"], e.data["y"], e.data["z"])) <= 40.0,
        timeout=15.0,
        description="/npc_scenario chase 后 40 格内出现 zombie(118) entity_spawn",
    )


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
    """发请求并断言逐字反馈；目标走远（"不在附近"）时重新逼近后重试。"""
    for attempt in range(retries):
        anchor = last_event_time(bot)
        bot.intent(request)
        try:
            event = bot.wait_for(
                lambda e: e.kind == "chat" and e.t > anchor and expected in e.data["text"],
                timeout=8.0,
                description=description,
            )
            _assert_feedback_exact(event, expected, description)
            return
        except BotAssertionError:
            with bot._lock:
                stray = [
                    e
                    for e in bot.events
                    if e.kind == "chat" and e.t > anchor and out_of_range in e.data["text"]
                ]
            if not stray:
                raise
        if not approach_entity(bot, entity_id, range_m=3.0):
            raise BotAssertionError(
                f"{description}：实体 {entity_id} 丢失（destroy/despawn），无法重试逼近"
            )
    raise BotAssertionError(
        f"{description} 重试 {retries} 次（每次重新逼近）仍未命中期望反馈 {expected!r}"
    )


def wait_for_rogue_within(
    bot: Bot, timeout: float, description: str
) -> tuple[int, tuple[float, float, float]]:
    """等视野内出现散修 villager（type=108）且拿到最近已知位置。"""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        best_id: int | None = None
        best_pos: tuple[float, float, float] | None = None
        with bot._lock:
            spawns = [
                e
                for e in bot.events
                if e.kind == "entity_spawn"
                and e.data.get("type") == VILLAGER_ENTITY_TYPE
                and e.data.get("entity_id") != bot.entity_id
            ]
        for spawn in spawns:
            entity_id = spawn.data["entity_id"]
            pos = bot.entity_pos(entity_id)
            if pos is None:
                continue
            if best_pos is None or _distance(bot.position, pos) < _distance(
                bot.position, best_pos
            ):
                best_id, best_pos = entity_id, pos
        if best_id is not None:
            return best_id, best_pos
        time.sleep(0.5)
    raise BotAssertionError(
        f"{description}：{timeout:.0f}s 内未出现 villager(108) entity_spawn；"
        "请以 BONG_ROGUE_SEED_COUNT>0 启动 server（BOT_E2E_ROGUE_TRADE=1 契约）"
    )


def rogue_display_prefix() -> str:
    return "§7[NPC] 散修·"


def assert_rogue_display_chat(event: Event, suffix: str, description: str) -> None:
    text = event.data["text"]
    if not (text.startswith(rogue_display_prefix()) and text.endswith(suffix)):
        raise BotAssertionError(
            f"期望 {description} 反馈为「散修·<境界>…{suffix}」（境界随 realm 分布不定），"
            f"实际 {text!r}"
        )


def request_and_assert_rogue(
    bot: Bot,
    request: dict,
    entity_id: int,
    suffix: str,
    description: str,
    retries: int = 5,
) -> None:
    """发请求并断言「散修·<境界>…suffix」形反馈（display_name 带 realm，前缀断言）。"""
    for attempt in range(retries):
        anchor = last_event_time(bot)
        bot.intent(request)
        try:
            event = bot.wait_for(
                lambda e: e.kind == "chat"
                and e.t > anchor
                and e.data["text"].startswith(rogue_display_prefix())
                and suffix in e.data["text"],
                timeout=8.0,
                description=description,
            )
            assert_rogue_display_chat(event, suffix, description)
            return
        except BotAssertionError:
            with bot._lock:
                stray = [
                    e
                    for e in bot.events
                    if e.kind == "chat"
                    and e.t > anchor
                    and (
                        OUT_OF_RANGE_INSPECT in e.data["text"]
                        or OUT_OF_RANGE_CHOICE in e.data["text"]
                        or OUT_OF_RANGE_TRADE in e.data["text"]
                    )
                ]
            if not stray:
                raise
        if not approach_entity(bot, entity_id, range_m=3.0):
            raise BotAssertionError(
                f"{description}：实体 {entity_id} 丢失（destroy/despawn），无法重试逼近"
            )
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


def nearest_villager_id(bot: Bot) -> int | None:
    """当前视野内最近的 villager(108) entity_id（无则 None）。"""
    best_id: int | None = None
    best_dist = float("inf")
    with bot._lock:
        spawns = [
            e
            for e in bot.events
            if e.kind == "entity_spawn"
            and e.data.get("type") == VILLAGER_ENTITY_TYPE
            and e.data.get("entity_id") != bot.entity_id
        ]
    for spawn in spawns:
        entity_id = spawn.data["entity_id"]
        pos = bot.entity_pos(entity_id)
        if pos is None or bot.position is None:
            continue
        dist = _distance(bot.position, pos)
        if dist < best_dist:
            best_dist, best_id = dist, entity_id
    return best_id


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
