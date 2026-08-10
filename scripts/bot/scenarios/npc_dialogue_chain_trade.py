"""`npc_trade_request` 的协议级回归（对话链路 P1 覆盖）。

两阶段，均锚定 chat 逐字回显（server/src/network/client_request_handler.rs
NpcTradeRequest 分支，判定顺序：offered_items → catalog → can_trade →
库存命中 → 定价(rep) → 骨币余额 → 入包）：

Phase 1（无环境依赖，任意 fixture server 可跑）：`/npc_scenario chase` zombie
（display_name="游尸·醒灵"，can_trade=false）驱动两条确定性拒绝分支 + 目录键控锚点：
- 非空 offered_items → "§c[NPC] 当前交易只支持骨币结算。"
- 任意 requested_item_id → "§c[NPC] {display} 没有这件货。"——目录按 archetype
  键控（npc_trade_catalog_entry 仅 Commoner|Rogue 有条目），zombie 无目录条目，
  且目录查找先于 can_trade 检查，"不做买卖"（can_trade=false）分支经 zombie
  不可达；该分支需 Rogue/Commoner 且 rep < -30（或 Wanted）——fixture 玩家
  rep=0 → Normal → can_trade=true，不可构造，留待后续 dev 命令覆盖。

Phase 2（BOT_E2E_ROGUE_TRADE=1 才执行）：BONG_ROGUE_SEED_COUNT>0 播种的散修
（villager fallback=108，can_trade=true；fresh 玩家 rep=0 → RepTier::Mid 1.0x 定价）。
bot 从 spawn 走向 rogue_village POI（/tppoi novice 读坐标），追最近 villager 至
≤3m 后：inspect greeting、"trade" 摊开货物、三件目录品逐件请求——库存随机 1-3/3，
每件反馈 ∈ {"当前没有这件货", "骨币不足，需要 N 枚。"}（fresh 玩家 7 骨币 < 最低价 8，
命中必报骨币不足），且三件至少一次骨币不足（库存非空必然命中一件）。

fixture 模式（BOT_E2E_AMBIENT_FIXTURE_MODE=1 或 CI 通用自起）不设 BOT_E2E_ROGUE_TRADE：
播种受 NpcRegistry 预算封顶（实测 300 → 50，"seeded 50 rogue NPCs"），散落 zone
bounds ±750 且 80m 激活半径，RogueVillage POI 重生成需 ~24h server ticks——
fixture 世界无法确定性构造散修相遇，故该阶段是真实世界契约而非 CI 场景。
"""

DESCRIPTION = "npc_trade_request：zombie 拒绝分支（骨币结算/目录键控）+ 散修商贩链路（BOT_E2E_ROGUE_TRADE=1 可选）"
MODULES = ["npc", "dialogue", "trade"]

import os

from bot.bot import BotAssertionError

from ._npc_dialogue_helpers import (
    CATALOG_ITEMS,
    OUT_OF_RANGE_TRADE,
    approach_entity,
    assert_rogue_stock_miss,
    is_rogue_stock_miss,
    last_event_time,
    nearest_villager_id,
    queue_scenario_zombie,
    request_and_assert,
    request_and_assert_rogue,
    rogue_village_pos_from_tppoi,
)

ZOMBIE_DISPLAY = "游尸·醒灵"
TRADE_ONLY_BONE_COINS = "§c[NPC] 当前交易只支持骨币结算。"
ZOMBIE_NO_STOCK = f"§c[NPC] {ZOMBIE_DISPLAY} 没有这件货。"
# 服务端形态 `§7[NPC] {display} 摊开了随身货物。`——display 以境界段结尾，后缀含前导空格。
ROGUE_TRADE_OPEN = " 摊开了随身货物。"
ROGUE_GREETING_SUFFIX = "：道友，可有灵草出让？"
COINS_SHORT = "骨币不足，需要 "


def _request_trade(npc_id, requested_item_id, offered_items):
    return {
        "type": "npc_trade_request",
        "v": 1,
        "npc_entity_id": npc_id,
        "offered_items": offered_items,
        "requested_item_id": requested_item_id,
    }


def run_phase_1(bot) -> None:
    spawn = queue_scenario_zombie(bot)
    zombie_id = spawn.data["entity_id"]

    request_and_assert(
        bot,
        _request_trade(zombie_id, "spirit_grass", [1]),
        zombie_id,
        TRADE_ONLY_BONE_COINS,
        "非空 offered_items 的逐字拒绝（只支持骨币结算）",
        OUT_OF_RANGE_TRADE,
    )
    request_and_assert(
        bot,
        _request_trade(zombie_id, "nonexistent_curio", []),
        zombie_id,
        ZOMBIE_NO_STOCK,
        "目录外 requested_item_id 的逐字拒绝（没有这件货）",
        OUT_OF_RANGE_TRADE,
    )
    request_and_assert(
        bot,
        _request_trade(zombie_id, "spirit_grass", []),
        zombie_id,
        ZOMBIE_NO_STOCK,
        "目录内商品从 zombie 购买也落「没有这件货」（目录按 archetype 键控）",
        OUT_OF_RANGE_TRADE,
    )


def run_phase_2(bot) -> None:
    import time

    village = rogue_village_pos_from_tppoi(bot)

    # 播种是 Poisson 场：出生点附近期望 ~3 个 villager（80m 激活半径）。
    # 先原地等 25s 捕获最近者；没有才向村庄走（路上顺路扫场 + 村庄 POI 聚拢）。
    npc_id = nearest_villager_id(bot)
    deadline = time.monotonic() + 25.0
    while npc_id is None and time.monotonic() < deadline:
        time.sleep(1.0)
        npc_id = nearest_villager_id(bot)
    if npc_id is not None:
        if not approach_entity(bot, npc_id, range_m=3.0):
            raise BotAssertionError(f"追近 villager entity_id={npc_id} 失败（实体丢失）")
        _run_rogue_chain(bot, npc_id)
        return

    bot.move_to(village[0], village[1], village[2], speed=5.5)
    npc_id = nearest_villager_id(bot)
    if npc_id is None:
        # 走到村庄后仍未见：再站 30s——玩家 80m 内激活的散修按日程
        # Trade/Patrol/Socialize 聚向 RogueVillage POI，视野内迟早出现。
        deadline = time.monotonic() + 30.0
        while npc_id is None and time.monotonic() < deadline:
            time.sleep(1.0)
            npc_id = nearest_villager_id(bot)
    if npc_id is None:
        raise BotAssertionError(
            "出生点等待 25s、走向 rogue_village 途中及村庄等待 30s 均未捕获 "
            "villager(108) entity_spawn；BOT_E2E_ROGUE_TRADE=1 契约要求 "
            "BONG_ROGUE_SEED_COUNT>0 启动 server"
        )
    if not approach_entity(bot, npc_id, range_m=3.0):
        raise BotAssertionError(f"追近 villager entity_id={npc_id} 失败（实体丢失）")
    _run_rogue_chain(bot, npc_id)


def _run_rogue_chain(bot, npc_id) -> None:

    request_and_assert_rogue(
        bot,
        {"type": "npc_inspect_request", "v": 1, "npc_entity_id": npc_id},
        npc_id,
        ROGUE_GREETING_SUFFIX,
        "散修 inspect greeting（道友，可有灵草出让？）",
    )
    request_and_assert_rogue(
        bot,
        {
            "type": "npc_dialogue_choice",
            "v": 1,
            "npc_entity_id": npc_id,
            "option_id": "trade",
        },
        npc_id,
        ROGUE_TRADE_OPEN,
        '散修商贩 choice option="trade" 摊开货物',
    )

    coins_short_hits = 0
    for item_id, price in CATALOG_ITEMS:
        expected_short = f"§c[NPC] {COINS_SHORT}{price} 枚。"
        event = None
        for _attempt in range(5):
            anchor = last_event_time(bot)
            bot.intent(_request_trade(npc_id, item_id, []))
            try:
                # 谓词只接受三类逐字形态：余额不足全文、库存未命中逐字形态、越距拒绝。
                # 越距即时回显不消耗 8s 成功超时；成功后仍按逐字契约断言，不放过畸形响应。
                event = bot.wait_for(
                    lambda e: e.kind == "chat"
                    and e.t > anchor
                    and (
                        e.data["text"] == expected_short
                        or e.data["text"] == OUT_OF_RANGE_TRADE
                        or is_rogue_stock_miss(e.data["text"])
                    ),
                    timeout=8.0,
                    description=f"目录品 {item_id} 的库存/余额二分回显",
                )
                break
            except BotAssertionError:
                with bot._lock:
                    out_of_range = [
                        e
                        for e in bot.events
                        if e.kind == "chat"
                        and e.t > anchor
                        and e.data["text"] == OUT_OF_RANGE_TRADE
                    ]
                if not out_of_range:
                    raise
                event = out_of_range[-1]
                break
        if event is None:
            raise BotAssertionError(f"目录品 {item_id} 五次重试仍未命中二分回显")
        text = event.data["text"]
        if text == OUT_OF_RANGE_TRADE:
            if not approach_entity(bot, npc_id, range_m=3.0):
                raise BotAssertionError(
                    f"目录品 {item_id} 重试期间 villager {npc_id} 丢失"
                )
            continue
        if text == expected_short:
            coins_short_hits += 1
        elif is_rogue_stock_miss(text):
            assert_rogue_stock_miss(event, f"目录品 {item_id} 的库存未命中回显")
        else:
            raise BotAssertionError(
                f"期望 {item_id} 反馈为「散修·<境界> 当前没有这件货。」或「骨币不足，需要 "
                f"{price} 枚。」逐字契约，实际 {text!r}"
            )
    if coins_short_hits == 0:
        raise BotAssertionError(
            f"三件目录品均报「当前没有这件货」——散修库存必为 1-3/3 非空，"
            "至少一件命中并触发骨币不足；实际一次未命中"
        )


def run(env) -> None:
    with env.new_bot("NpcTr") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        run_phase_1(bot)

        if os.environ.get("BOT_E2E_ROGUE_TRADE") == "1":
            run_phase_2(bot)
        else:
            print(
                "    [npc_dialogue_chain_trade] BOT_E2E_ROGUE_TRADE 未置 1，跳过散修商贩阶段"
                "（fixture/CI 模式播种预算封顶 50 且散落 ±750，无法确定性相遇；"
                "该阶段面向真实世界 BONG_ROGUE_SEED_COUNT>0 server）"
            )
        bot.assert_alive("trade 对话链路检查后")
