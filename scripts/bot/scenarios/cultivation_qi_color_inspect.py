"""真元色探察 qi_color_inspect（实体/空间探知流，plan 感知 §真元色）。

C2S：`qi_color_inspect { observed: "entity:<protocol_id>" }`——protocol id 来自
S2C PlayerSpawn 包（game_join 的 entity_id 恒为 0，只能经 player_spawn 拿同服
玩家 id；bot.py 已补该包解码）。

服务端链路：
- dispatch（client_request_handler.rs:2128）：`entity:<id>` → EntityManager
  get_by_id → 同维度 + ≤QI_COLOR_INSPECT_MAX_DISTANCE(6.0) 才放行，否则静默丢弃；
- emit（qi_color_observed_emit.rs）：observer/observed 均需 Cultivation，observed
  需 QiColor；`realm_diff = rank(observer) - rank(observed)`，≤0 静默 continue；
  ≥2 全量 payload（main/secondary/is_chaotic/is_hunyuan/realm_diff），==1 脱敏
  （secondary **省略**（field 4 缺失，非显式 null）、is_chaotic=false、
  is_hunyuan=false——**main 保留**）。

断言面（两 bot 同出生点，相距 <6m；realm_rank: Awaken=0/Induce=1/Condense=2/
Solidify=3/Spirit=4/Void=5，technique_scroll.rs:211）：
1. host 固元（rank 3）+ victim 引气（rank 1）→ diff=2 全量 payload。
   victim 以真实功法施放积累修行
   （崩拳空挥×4 → Heavy，吸灵口空挥×2 → Intricate），QiColor 演化为非默认
   main=Heavy、secondary=Intricate。全量断言校验**非默认敏感状态**：
   main="Heavy"、secondary="Intricate"——脱敏分支**省略** secondary 字段而
   main 保留，只断言 main 会放走「对一切正境界差都套脱敏」的坏实现；另含
   is_chaotic/is_hunyuan=false、realm_diff=2、observer/observed 为
   offline:<username> canonical id（observer **全等**，不是前缀）；
2. host 通灵（rank 4）→ diff=3 全量——realm_diff>=2 契约的更大差值边界：只测
   diff=2 会放走「diff==2 才全量、diff>2 仍脱敏/静默」的 off-by-one 坏实现
   （review finding 1）；
3. host 凝脉（rank 2）→ diff=1 脱敏 payload（main 保留、secondary **键省略**、
   is_chaotic/is_hunyuan=false）——全量披露与静默之间唯一可见的脱敏边界；
4. host 降回引气（同境界）→ 静默（realm_diff=0 continue，无 S2C）；
5. host 引气、victim 升凝脉 → diff=-1 **负境界差**静默——用共位真实实体隔离，
   同维 <6m 下静默只能来自 realm_diff<=0 continue（review finding 1）；
6. 不存在协议 id 的 observed → dispatch 静默丢弃（无 S2C、无聊天）；
7. victim tpdim 到 TSY（同裸 XYZ，距离前提仍满足）→ host 再探真实 victim
   → 静默（same_dimension 失败，唯一失败的空间前提是维度）；
8. victim 回 overworld 后 tpzone 到远端 zone（同维、距 host >6m）→ host 再探
   真实 victim → 静默（distance 失败，唯一失败的空间前提是距离）。
   7/8 都以**真实可见实体**（victim 的 protocol entity id）放在空间门另一侧——
   只发不存在 id 会放走「resolve 正常但漏掉距离/维度检查」的坏实现
   （central-review 2029 #5）。
"""

import json
import time
from functools import lru_cache
from pathlib import Path

from bot.bot import BotAssertionError, Event
from bot.mc_protocol import offline_uuid
from bot import proto_min
from bot.scenarios._combat_helpers import (
    last_event_time,
    payload_text,
    wait_for_skill_binding,
)
from bot.scenarios._inventory_helpers import wait_join_and_inventory
from bot.scenarios._rejection_helpers import AMBIENT_SERVER_DATA_TYPES

DESCRIPTION = (
    "qi_color_inspect：跨境界全量 payload（victim 非默认真元色 main=Heavy/"
    "secondary=Intricate）、observer 全等 canonical id、同境界静默、坏 target 静默"
)
MODULES = ["cultivation", "network", "combat", "skill", "cmd"]

INSPECT_REQUEST = {"type": "qi_color_inspect", "v": 1}
SILENT_WINDOW = 4.0
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client
# （network/carrier_state_emit.rs，ticks % TICKS_PER_SECOND==0 周期）。
# player_state / zone_info 等只随 Changed 组件 / 区间迁移发射——静默窗口前已把
# realm set 的 player_state 回推排空（_realm_set_and_settle），窗口内除 carrier_state
# 无合法非白名单 payload；白名单外一律判红（central-review 2029 #4）。carrier_state
# 不在 proto_min 白名单，通常不解码成 server_data 事件；保留它只为显式豁免未来
# proto_min 收录后的周期流。
AMBIENT_PERIODIC_PAYLOAD_TYPES = AMBIENT_SERVER_DATA_TYPES

BENG_QUAN = "burst_meridian.beng_quan"
BENG_QUAN_WIRE = "beng_quan"
WOLIU_MOUTH = "woliu.mouth"
SLOT_BENG = 0
SLOT_WOLIU = 1
HEAVY_CASTS = 4
INTRICATE_CASTS = 2
# 10s 沿用原单次接受反馈的 liveness 上限；冷却本身按 server 下发的权威绝对截止
# 时间等待，不用固定 wall-clock 间隔猜测服务端 tick 进度。
CAST_FEEDBACK_TIMEOUT = 10.0
CAST_READY_POLL_INTERVAL = 0.25


def _practice_nondefault_qi_color(bot) -> None:
    """让 victim 以真实功法施放积累双色修行，QiColor 演化为 main=Heavy、
    secondary=Intricate —— 非默认敏感状态，令全量 payload 与脱敏 payload 可区分。

    - 崩拳（burst_meridian.beng_quan）：引气境 + 经脉已开 + 真元足够 → 空挥在
      spend_qi 后无条件 record_style_practice(Heavy)（burst_meridian.rs:320）。
    - 吸灵口（woliu.mouth）：已学功法 + 肺经 + 真元 → 空吸发 VortexCastEvent
      → track_woliu_proficiency_from_casts 记 Intricate（technique_proficiency.rs）。

    比例 4:2 → Heavy 67%>60%（主色）、Intricate 33%>25%（副色）。每色随 tick
    等速衰减 0.001，比例向 50% 收敛，副色不会先于主色跌破阈值；本函数结束后
    run() 立刻 inspect，衰减窗口极小。"""
    bot.cmd("realm set induce")
    bot.expect_chat("[dev] realm set ", timeout=10.0)
    bot.cmd("meridian open_all")
    bot.expect_chat("[dev] opened", timeout=10.0)
    bot.cmd("qi max 100")
    bot.expect_chat("[dev] qi max", timeout=10.0)
    bot.cmd("qi set 100")
    bot.expect_chat("[dev] qi set", timeout=10.0)
    bot.cmd(f"technique give {BENG_QUAN}")
    bot.expect_chat(f"[dev] technique give `{BENG_QUAN}`", timeout=10.0)
    bot.cmd(f"technique give {WOLIU_MOUTH}")
    bot.expect_chat(f"[dev] technique give `{WOLIU_MOUTH}`", timeout=10.0)

    binding_anchor = last_event_time(bot)
    bot.intent(
        {
            "type": "skill_bar_bind",
            "v": 1,
            "slot": SLOT_BENG,
            "binding": {"kind": "skill", "skill_id": BENG_QUAN},
        }
    )
    bot.intent(
        {
            "type": "skill_bar_bind",
            "v": 1,
            "slot": SLOT_WOLIU,
            "binding": {"kind": "skill", "skill_id": WOLIU_MOUTH},
        }
    )
    wait_for_skill_binding(bot, binding_anchor, SLOT_BENG, BENG_QUAN)
    wait_for_skill_binding(bot, binding_anchor, SLOT_WOLIU, WOLIU_MOUTH)

    for index in range(HEAVY_CASTS):
        cast_sent_at = last_event_time(bot)
        _cast_empty_and_confirm(
            bot,
            SLOT_BENG,
            # central-review 31437496353 #4：confirm 回调会收到 wait_for 锚点之后的**一切**
            # 事件（含 position-look 等非 server_data），必须先判 kind 再读
            # payload_type——否则无关事件先到就 KeyError 崩场景（Woliu 回调用 .get
            # 侥幸安全，此回调必须显式拦 kind）。
            _burst_event_observed,
            "空挥崩拳应收到 burst_meridian_event（施放被接受，Heavy 修行落账）",
        )
        cooldown_until_ms = _wait_for_skillbar_cooldown(
            bot,
            cast_sent_at,
            SLOT_BENG,
            BENG_QUAN,
            "崩拳",
        )
        if index + 1 < HEAVY_CASTS:
            _wait_until_cooldown_due(
                bot, cooldown_until_ms, SLOT_BENG, BENG_QUAN, "崩拳"
            )

    # 崩拳按当前真元 40% 扣费，4 次后余量不足吸灵口 12+3*2=18 的启动费，补足。
    bot.cmd("qi set 100")
    bot.expect_chat("[dev] qi set", timeout=10.0)

    for index in range(INTRICATE_CASTS):
        cast_sent_at = last_event_time(bot)
        _cast_empty_and_confirm(
            bot,
            SLOT_WOLIU,
            lambda e: e.data.get("channel") == "bong:vfx_event"
            and "bong:woliu_mouth_funnel" in payload_text(e),
            "空挥吸灵口应收到 bong:woliu_mouth_funnel vfx（施放被接受，Intricate 修行落账）",
        )
        cooldown_until_ms = _wait_for_skillbar_cooldown(
            bot,
            cast_sent_at,
            SLOT_WOLIU,
            WOLIU_MOUTH,
            "吸灵口",
        )
        if index + 1 < INTRICATE_CASTS:
            _wait_until_cooldown_due(
                bot, cooldown_until_ms, SLOT_WOLIU, WOLIU_MOUTH, "吸灵口"
            )


def _tpzone_and_settle(bot, zone: str) -> None:
    """等待 `/tpzone` 的位置提交，而不是把命令 chat 当作状态完成信号。

    dev command 的 chat 是 command result 的确认；`tpzone` 随后才在
    `AuthoritativePositionCommitSet` 写入 `Position`，Valence 再为该变化发出
    `pos_look`。qi_color_inspect 的空间门必须在这个既有 vanilla 状态事件之后才测，
    否则 host 可能在 victim 仍处于旧位置时发出请求，漏掉 distance 负向断言。
    """
    target_position = _zone_teleport_position(zone)
    already_at_target = _position_matches(bot.position, target_position)
    anchor = last_event_time(bot)
    bot.cmd(f"tpzone {zone}")
    bot.wait_for(
        lambda e: e.kind == "chat"
        and e.t > anchor
        and e.data.get("text") == f"Teleported to zone `{zone}`.",
        timeout=10.0,
        description=f"/tpzone {zone} 的新 chat 确认",
    )
    # `handle_tpzone` writes the authoritative Position even when the target is
    # identical to the current Position. Valence does not emit a second
    # PlayerPosLook for that no-op write, so the prior authoritative pos_look
    # (captured by Bot.position) is the settled state in this branch. If the
    # bot was not already at the target, a new pos_look remains mandatory: a
    # chat acknowledgement alone is only command completion, not position
    # completion.
    if already_at_target:
        return
    bot.wait_for(
        lambda e: (
            e.kind == "pos_look"
            and e.t > anchor
            and _position_matches(
                (e.data.get("x"), e.data.get("y"), e.data.get("z")),
                target_position,
            )
        ),
        timeout=10.0,
        description=f"/tpzone {zone} 后目标坐标提交对应的 pos_look",
    )


@lru_cache(maxsize=None)
def _zone_teleport_position(zone: str) -> tuple[float, float, float]:
    """从当前 checkout 的公开 zone 配置推导 `/tpzone` 的写入坐标。"""
    zones_path = Path(__file__).resolve().parents[3] / "server" / "zones.json"
    try:
        with zones_path.open(encoding="utf-8") as stream:
            zones = json.load(stream)["zones"]
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        raise BotAssertionError(f"无法读取 tpzone `{zone}` 的公开 zone 配置：{error}") from error

    for zone_definition in zones:
        if zone_definition.get("name") != zone:
            continue
        aabb = zone_definition.get("aabb")
        if not isinstance(aabb, dict):
            break
        minimum = aabb.get("min")
        maximum = aabb.get("max")
        if (
            isinstance(minimum, list)
            and len(minimum) == 3
            and isinstance(maximum, list)
            and len(maximum) == 3
        ):
            center = tuple((low + high) * 0.5 for low, high in zip(minimum, maximum))
            return (center[0], center[1] + 24.0, center[2])
        break
    raise BotAssertionError(f"公开 zone 配置中没有可用的 tpzone `{zone}` 坐标")


def _position_matches(
    actual: tuple[object, object, object] | None,
    expected: tuple[float, float, float],
) -> bool:
    if actual is None or len(actual) != len(expected):
        return False
    try:
        return all(
            abs(float(observed) - target) <= 1e-6
            for observed, target in zip(actual, expected)
        )
    except (TypeError, ValueError):
        return False


def _cast_empty_and_confirm(bot, slot: int, confirm, description: str) -> Event:
    """空挥 skill_bar_cast（无 target = 空挥/空吸），等施放被接受的 server 反馈。

    冷却/经脉/真元门被拒时不会有反馈 → wait_for 超时，场景正确失败而非静默空过。
    下一次施放由 `_wait_until_cooldown_due` 刷新权威状态到期后再发送；这里必须只
    发送一次请求，不能用重复请求碰运气，也不能把拒绝伪装成成功。"""
    anchor = last_event_time(bot)
    bot.intent({"type": "skill_bar_cast", "v": 1, "slot": slot})
    return bot.wait_for(
        lambda e: e.t > anchor and confirm(e),
        timeout=CAST_FEEDBACK_TIMEOUT,
        description=description,
    )


def _wait_for_skillbar_cooldown(
    bot, cast_sent_at: float, slot: int, skill_id: str, label: str
) -> int:
    """以本次施放请求后的 `skillbar_config` 锁定服务端权威冷却状态。

    resolver 的接受反馈与冷却状态不是同一个事件：崩拳的冷却在通用 `Casting` 完成
    时写入，吸灵口则在 resolver 接受时直接写入；两者都由现有
    `skillbar_config.cooldown_until_ms` 对外呈现。两条路径的反馈事件顺序也不同，
    因此必须以本次请求发送时刻为下界；只用反馈事件作下界会漏掉已先到达的合法
    `skillbar_config`。这里按同一施放请求之后、同一技能槽位的权威状态等待，不新增
    server seam，也不把拒绝改成成功。
    """
    config = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.t > cast_sent_at
            and e.data.get("payload_type") == "skillbar_config"
            and _skillbar_config_matches(e.data.get("payload"), slot, skill_id)
            and _skillbar_config_has_active_cooldown(e.data["payload"], slot)
        ),
        timeout=CAST_FEEDBACK_TIMEOUT,
        description=f"{label} 施放请求(t>{cast_sent_at:.3f}s)后的 skillbar_config 权威冷却状态",
    )
    payload = config.data["payload"]
    return payload["cooldown_until_ms"][slot]


def _skillbar_config_matches(payload, slot: int, skill_id: str) -> bool:
    if not isinstance(payload, dict):
        return False
    slots = payload.get("slots")
    cooldowns = payload.get("cooldown_until_ms")
    if not isinstance(slots, list) or not isinstance(cooldowns, list):
        return False
    if slot < 0 or len(slots) <= slot or len(cooldowns) <= slot:
        return False
    entry = slots[slot]
    return (
        isinstance(entry, dict)
        and entry.get("kind") == "skill"
        and entry.get("skill_id") == skill_id
    )


def _skillbar_config_has_active_cooldown(payload: dict, slot: int) -> bool:
    cooldowns = payload.get("cooldown_until_ms")
    return (
        isinstance(cooldowns, list)
        and len(cooldowns) > slot
        and isinstance(cooldowns[slot], int)
        # The emitter writes zero only after the authoritative server tick has
        # reached the stored deadline.  A non-zero value can already be past in
        # wall-clock time when TPS is low, so comparing it with local time here
        # would discard the very state needed to diagnose that drift.
        and cooldowns[slot] > 0
    )


def _wait_until_cooldown_due(
    bot, cooldown_until_ms: int, slot: int, skill_id: str, label: str
) -> None:
    """等待 server 按实际 tick 清除冷却，再尝试下一次施放。

    `cooldown_until_ms` 是 server 用名义 20TPS 折算出的时间提示，不是实际冷却
    状态：低 TPS 时它可能先到。时间戳只用来减少不必要的查询，最终判定必须来自
    server 在一次无语义变化的同值 `skill_bar_bind` 后重新发出的
    `skillbar_config.cooldown_until_ms == 0`。绑定接口不会清理 SkillBarBindings 的
    skill_id 冷却（server 的既有回归契约），因此这是现有协议的只读状态刷新，不会
    重置冷却或重复施放；每次查询仍检查连接，避免断线被伪装成等待。
    """
    _sleep_until_cooldown_hint(bot, cooldown_until_ms, label)
    while True:
        current_until_ms = _refresh_skillbar_cooldown(
            bot,
            slot=slot,
            skill_id=skill_id,
            label=label,
        )
        if current_until_ms == 0:
            return
        _sleep_until_cooldown_hint(bot, current_until_ms, label)


def _sleep_until_cooldown_hint(bot, cooldown_until_ms: int, label: str) -> None:
    """按最新协议提示休眠；提示过期后由调用方重新取得 server 权威状态。"""
    while True:
        remaining = cooldown_until_ms - time.time_ns() // 1_000_000
        if remaining <= 0:
            return
        bot.assert_alive(f"{label} 权威冷却等待")
        time.sleep(min(remaining / 1000.0, CAST_READY_POLL_INTERVAL))


def _refresh_skillbar_cooldown(bot, slot: int, skill_id: str, label: str) -> int:
    """用同值绑定请求触发一次既有 `skillbar_config` 权威状态刷新。

    `SkillBarBindings::set` 只改槽内容、不触碰按 skill_id 记账的 cooldown；请求完成
    后既有 `Changed<SkillBarBindings>` emitter 会按当前 CombatClock.tick 重新折算
    配置。只有该配置明确报告槽位为 0，才允许下一次 cast，避免把低 TPS 下已经过期
    的预测 Unix 时间当作实际 tick 冷却已到期。
    """
    anchor = last_event_time(bot)
    bot.intent(
        {
            "type": "skill_bar_bind",
            "v": 1,
            "slot": slot,
            "binding": {"kind": "skill", "skill_id": skill_id},
        }
    )
    config = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.t > anchor
            and e.data.get("payload_type") == "skillbar_config"
            and _skillbar_config_matches(e.data.get("payload"), slot, skill_id)
        ),
        timeout=CAST_FEEDBACK_TIMEOUT,
        description=f"{label} 同值绑定后的 skillbar_config 权威冷却刷新",
    )
    payload = config.data["payload"]
    cooldowns = payload["cooldown_until_ms"]
    return cooldowns[slot]


def _burst_event_observed(event) -> bool:
    if event.kind == "server_data":
        return (
            event.data.get("payload_type") == "burst_meridian_event"
            and event.data.get("payload", {}).get("skill") == BENG_QUAN_WIRE
        )
    if event.kind != "server_data_raw":
        return False
    try:
        payload = proto_min.decode_server_data_envelope(event.data["data"])
    except (TypeError, ValueError):
        return False
    return (
        isinstance(payload, dict)
        and payload.get("type") == "burst_meridian_event"
        and payload.get("skill") == BENG_QUAN_WIRE
    )


def run(env) -> None:
    with env.new_bot("QiH") as host:
        wait_join_and_inventory(host)
        # host 固元（rank 3）+ victim 引气（rank 1）→ diff=2 全量分支。
        host.cmd("realm set solidify")
        host.expect_chat("[dev] realm set ", timeout=10.0)

        # Bot.__init__ performs login and starts the reader synchronously. Capture the
        # host watermark immediately before construction, otherwise the victim's one-shot
        # PlayerSpawn can arrive before new_bot returns and be excluded by the predicate.
        spawn_watermark = host.events[-1].t if host.events else 0.0
        with env.new_bot("QiV") as victim:
            # 水位 + canonical username 双锚定到 victim 本人；username 来自 host 的
            # PlayerList（标准顺序在 PlayerSpawn 前到达）。
            wait_join_and_inventory(victim)

            # Spawn selector 为不同用户名分配的出生点可能相距很远，host 因视距
            # 不会收到 victim 的 PlayerSpawn。先把两端放到同一固定 zone，再等待
            # 真实 PlayerSpawn；后续跨维/远距步骤仍使用同一 protocol entity id。
            _tpzone_and_settle(host, "jiuzong_taichu_ruin")
            _tpzone_and_settle(victim, "jiuzong_taichu_ruin")

            spawn = host.wait_for(
                lambda e: (
                    e.kind == "player_spawn"
                    and e.t > spawn_watermark
                    and (
                        e.data.get("username") == victim.username
                        or e.data.get("uuid") == offline_uuid(victim.username)
                    )
                ),
                timeout=15.0,
                description="host 收到 victim 的 PlayerSpawn（protocol entity id）",
            )
            victim_protocol_id = spawn.data["entity_id"]
            if not isinstance(victim_protocol_id, int) or victim_protocol_id <= 0:
                raise BotAssertionError(
                    f"[{host.username}] 期望 player_spawn 携带正数 protocol entity_id，"
                    f"实际 {victim_protocol_id!r}"
                )
            host.assert_alive("victim 就绪后")

            # victim 积累非默认真元色（Heavy/Intricate），为全量 payload 建立
            # 可被脱敏实现的「敏感状态」判别面。
            _practice_nondefault_qi_color(victim)

            # 1. 固元 host 探引气 victim → diff=2 全量 payload（非默认真元色）
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            observed = host.expect_server_data("qi_color_observed", timeout=10.0)
            payload = observed.data["payload"]
            _assert_full_payload(host, payload, victim.username, expected_diff=2)
            host.assert_alive("跨境界 qi_color_inspect 后")

            # 1b. host 升通灵（rank 4）、victim 仍引气（rank 1）→ diff=3 全量 payload。
            #    realm_diff>=2 契约的**更大差值**边界：只测 diff=2 会放走「diff==2 才全量、
            #    diff>2 仍脱敏/静默」的 off-by-one 坏实现（review finding 1）。qi color
            #    仍是 Heavy/Intricate（等速绝对衰减 0.001/tick 不破主色 60%），全量断言
            #    必须带非默认副色。注意 qi_color_observed 已在历史中，不能复用
            #    expect_server_data（只匹配第一条会拿错 payload），按水位锚定。
            sent_at = _realm_set_and_settle(host, "spirit")
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            observed3 = host.wait_for(
                lambda e: e.kind == "server_data"
                and e.data["payload_type"] == "qi_color_observed"
                and e.t > sent_at,
                timeout=10.0,
                description="spirit host 探引气 victim 的 qi_color_observed（diff=3 全量）",
            )
            _assert_full_payload(host, observed3.data["payload"], victim.username, expected_diff=3)
            host.assert_alive("三重境界差 qi_color_inspect 后")

            # 2. host 降为凝脉（rank 2）、victim 仍引气（rank 1）→ diff=1 脱敏 payload：
            #    main 保留、secondary/is_chaotic/is_hunyuan 一律隐藏（qi_color_observed_
            #    emit.rs:52-60）。这是「全量披露 ↔ 静默」之间唯一可见的脱敏边界，必须
            #    演练——只测 diff 2/0 会放走「对一切正境界差都套脱敏」或「diff=1 全量
            #    透出」的坏实现（review finding 1）。
            host.cmd("realm set condense")
            host.expect_chat("[dev] realm set ", timeout=10.0)
            condense_anchor = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            # 不能复用 expect_server_data：diff=2 的全量 payload 已在历史中，它只匹配
            # 第一条历史事件会拿错 payload（review round 4 自身的一个坑）；按水位锚定。
            redacted = host.wait_for(
                lambda e: e.kind == "server_data"
                and e.data["payload_type"] == "qi_color_observed"
                and e.t > condense_anchor,
                timeout=10.0,
                description="condense host 探引气 victim 的 qi_color_observed（diff=1 脱敏）",
            )
            _assert_redacted_payload(host, redacted.data["payload"], victim.username)
            host.assert_alive("单重境界差 qi_color_inspect 后")

            # 3. host 降回引气（同境界）→ realm_diff=0 静默。realm set 的 player_state
            #    回推必须排空在 sent_at 之前（_realm_set_and_settle），否则 setup 期
            #    回推会落入窗口，被白名单外的判红误伤。
            sent_at = _realm_set_and_settle(host, "induce")
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            _assert_silent(host, sent_at, "同境界 qi_color_inspect 应静默（realm_diff=0 continue）")

            # 3b. host 仍引气（rank 1）、victim 升凝脉（rank 2）→ diff=-1 负境界差静默。
            #    realm_diff<=0 分支在空间门**之后**执行——用共位真实实体隔离：同维、
            #    <6m，静默只能来自负境界差 continue（review finding 1）。victim realm
            #    set 只推自己的 player_state（per-client Changed<Cultivation> 查询），
            #    不向 host 发任何 server_data/聊天；锚点取 host 自己的事件水位即可。
            victim.cmd("realm set condense")
            victim.expect_chat("[dev] realm set ", timeout=10.0)
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            _assert_silent(host, sent_at, "负境界差 qi_color_inspect 应静默（realm_diff<=0 continue）")
            # 恢复 victim 引气：后续空间门步骤的静默必须由 dimension/distance 门产生，
            # 不能退化为负境界差静默（保持空间门可隔离性）。
            victim.cmd("realm set induce")
            victim.expect_chat("[dev] realm set ", timeout=10.0)

            # 4. 不存在的协议 id → dispatch 静默丢弃
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": "entity:999999999"})
            _assert_silent(host, sent_at, "坏 observed 协议 id 应被 dispatch 静默丢弃")

            # 5. victim tpdim 到 TSY（同裸 XYZ，距离前提仍满足）→ host 凝脉再探真实
            #    victim → 静默（same_dimension 失败，唯一失败的空间前提是维度）。
            #    空间门必须**可隔离**：初始 setup 已把双 bot 精确共位到同一 zone 中心
            #    （tpzone 定点落位，distance=0），这里的 tpdim 只移 victim（X+0.25）
            #    → distance=0.25 ≤ 6m。若直接依赖出生点相对距离，tpdim 的 +0.25 位移
            #    可能把近 6m 的一对推过 6m，distance 与 dimension 同时失败——漏 dimension
            #    检查的坏实现照样静默，隔离性破（central-review 2029 #5）。不重复发送
            #    已落在同一坐标的 `/tpzone`：no-op teleport 不产生新的 `pos_look`，等待
            #    它会把测试变成无依据的超时。跨维 transfer 只改
            #    EntityLayerId/Position，ECS Entity 与 protocol entity id 保持不变
            #    （dimension_transfer.rs），step 0 捕获的旧 id 仍解析到 victim。
            host.cmd("realm set condense")
            host.expect_chat("[dev] realm set ", timeout=10.0)
            _transfer_dimension(victim, "tsy", victim.events[-1].t if victim.events else 0.0)
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            _assert_silent(host, sent_at, "跨维度 qi_color_inspect 应静默（same_dimension 失败）")

            # 6. victim 回 overworld 后 tpzone 到远端 zone（同维、距 host >6m）→ host
            #    凝脉再探真实 victim → 静默（distance 失败，唯一失败的空间前提是距离）。
            #    tpdim overworld 把 victim 的 X+0.25 精确还原，与 host 再次同坐标共位；
            #    tpzone wangyintai 再到 (4000,144,-1650)——同 overworld 维、距 host
            #    （jiuzong 中心 (0,109,-10000)）~9257m。若距离前提不满足（victim 仍近），
            #    漏 distance 检查的坏实现也能静默，隔离性破。
            _transfer_dimension(victim, "overworld", victim.events[-1].t if victim.events else 0.0)
            _tpzone_and_settle(victim, "wangyintai")
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            _assert_silent(host, sent_at, "远端 zone 的 qi_color_inspect 应静默（distance 失败）")

            victim.assert_alive("qi_color_inspect 全程 victim")
            host.assert_alive("qi_color_inspect 拒绝面全程")


def _assert_full_payload(host, payload: dict, victim_username: str, expected_diff: int) -> None:
    if payload.get("main") != "Heavy":
        raise BotAssertionError(
            f"[{host.username}] 期望 QiColor.main=Heavy（victim 崩拳修行演化），"
            f"实际 {payload.get('main')}"
        )
    if payload.get("realm_diff") != expected_diff:
        raise BotAssertionError(
            f"[{host.username}] 期望 realm_diff={expected_diff}（全量分支），"
            f"实际 {payload.get('realm_diff')}"
        )
    if payload.get("is_chaotic") is not False or payload.get("is_hunyuan") is not False:
        raise BotAssertionError(
            f"[{host.username}] 期望 is_chaotic/is_hunyuan=false，实际 {payload}"
        )
    # 副色是全量/脱敏的判别键：脱敏分支只把 secondary 打成 None 而 main 保留，
    # 只断言 main 会放走「对一切正境界差都套脱敏」的坏实现。
    if payload.get("secondary") != "Intricate":
        raise BotAssertionError(
            f"[{host.username}] 期望 secondary=Intricate（victim 吸灵口修行演化），"
            f"实际 {payload.get('secondary')!r}"
        )
    if payload.get("observed") != f"offline:{victim_username}":
        raise BotAssertionError(
            f"[{host.username}] 期望 observed=offline:{victim_username}，"
            f"实际 {payload.get('observed')}"
        )
    # observer 必须全等 canonical id：生产端把 observer/observed 对调、或发出
    # 任意其他 offline:<user> 都不该通过（startswith 前缀检查拦不住）。
    if payload.get("observer") != f"offline:{host.username}":
        raise BotAssertionError(
            f"[{host.username}] 期望 observer=offline:{host.username}（全等 canonical id），"
            f"实际 {payload.get('observer')}"
        )


def _assert_redacted_payload(host, payload: dict, victim_username: str) -> None:
    """diff=1 脱敏契约：只保留 main 主色，副色与混沌/混元旗标一律隐藏。

    server 端把脱敏后的 `secondary` 序列化为**缺失字段**（None Option 省略 field
    4）；proto_min 解码器只在 field 4 携带时产出 `secondary` 键。故此处断言「键
    缺失」而非 `dict.get("secondary") is None`——dict.get 对「键缺失」与「显式
    null」返回同一个 None，放走显式发 null 的坏实现（central-review 31437496353
    #3）。"""
    if payload.get("main") != "Heavy":
        raise BotAssertionError(
            f"[{host.username}] 期望脱敏 payload.main=Heavy 保留（victim 崩拳修行演化），"
            f"实际 {payload.get('main')}"
        )
    if payload.get("realm_diff") != 1:
        raise BotAssertionError(
            f"[{host.username}] 期望 realm_diff=1（凝脉-引气），实际 {payload.get('realm_diff')}"
        )
    if "secondary" in payload:
        raise BotAssertionError(
            f"[{host.username}] 期望脱敏 payload 省略 secondary 键（field 4 缺失，"
            f"非显式 null），实际 {payload.get('secondary')!r}"
        )
    if payload.get("is_chaotic") is not False or payload.get("is_hunyuan") is not False:
        raise BotAssertionError(
            f"[{host.username}] 期望脱敏 payload is_chaotic/is_hunyuan=false，实际 {payload}"
        )
    if payload.get("observed") != f"offline:{victim_username}":
        raise BotAssertionError(
            f"[{host.username}] 期望脱敏 payload observed=offline:{victim_username}，"
            f"实际 {payload.get('observed')}"
        )
    if payload.get("observer") != f"offline:{host.username}":
        raise BotAssertionError(
            f"[{host.username}] 期望脱敏 payload observer=offline:{host.username}（全等 canonical id），"
            f"实际 {payload.get('observer')}"
        )


def _realm_set_and_settle(bot, realm: str) -> float:
    """realm set 后排空本 bot 的 player_state 回推，返回可作静默水位的时刻。

    realm set 恒触发 Changed<Cultivation> → player_state 无条件推给自己（realm.rs
    同值写仍触发 Changed）。若不等它落定就锚定 sent_at，setup 期 player_state 会落入
    静默窗口，被白名单外的判红误伤（它并非 qi_color_inspect 的响应，central-review
    2029 #4）。以拒信 chat 时刻为锚：player_state 随同批或紧随其后到达，wait_for
    立即命中或等它到。"""
    bot.cmd(f"realm set {realm}")
    confirm = bot.expect_chat("[dev] realm set ", timeout=10.0)
    bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data["payload_type"] == "player_state"
            and e.t >= confirm.t
        ),
        timeout=5.0,
        description=f"realm set {realm} 的 player_state 回推应已到达",
    )
    return bot.events[-1].t if bot.events else 0.0


def _transfer_dimension(bot, target: str, after: float) -> None:
    """把 bot 经正式 transfer consumer 切到 target 维度（保持同 XYZ 邻域）。

    等 server 权威 transfer 排队的聊天确认 + 真实跨维 Respawn 及其后的同 XYZ
    Position pulse/restore 提交，并断言 Respawn 携带目标维度
    （dimension_type_name/dimension_name 双键任一命中即可）。Respawn 先到不代表
    transfer 的 Position 提交已经完成：`/tpdim` 的既有确认协议在后续 tick 先发
    0.001 格 pulse、再恢复最终坐标；若此时立即发下一次 `/tpzone`，后到的旧位置帧
    可能覆盖新传送。维度未完成切换前不 dispatch 探针——否则正确实现会因同维发
    payload，静默断言在 setup 阶段就假红。跨维 transfer 不换 ECS Entity，protocol
    entity id 保持有效。"""
    bot.cmd(f"tpdim {target}")
    bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and e.t > after
            and f"Queued /tpdim {target} within current XYZ gate." in e.data.get("text", "")
        ),
        timeout=10.0,
        description=f"/tpdim {target} 应收到 server 权威 transfer 排队确认",
    )
    respawn = bot.wait_for(
        lambda e: e.kind == "respawn" and e.t > after,
        timeout=10.0,
        description=f"/tpdim {target} 应触发真实跨维 Respawn",
    )
    expected = {"tsy": "bong:tsy", "overworld": "minecraft:overworld"}[target]
    actual = {
        key: respawn.data.get(key)
        for key in ("dimension_type_name", "dimension_name")
    }
    # 契约是「任一字段命中目标即可」：两字段含义不同（dimension_type 是类型注册表
    # 键、dimension_name 是命名空间名），合法 Respawn 可能只带其一。用 any(... != ...)
    # 会要求两字段都等于 target，一个字段缺失/异名即误红——必须在任一字段匹配时才
    # 通过，即失败条件是「无任何字段匹配」（central-review 2029 #1）。
    if expected not in actual.values():
        raise BotAssertionError(
            f"[{bot.username}] /tpdim {target} 的 Respawn 必须携带目标维度 {expected}，"
            f"实际 {actual}"
        )
    pulse = bot.wait_for(
        lambda e: e.kind == "pos_look" and e.t > respawn.t,
        timeout=10.0,
        description=f"/tpdim {target} 的 Respawn 后 Position pulse",
    )
    bot.wait_for(
        lambda e: e.kind == "pos_look" and e.t > pulse.t,
        timeout=10.0,
        description=f"/tpdim {target} 的 Position restore 完成",
    )


def _assert_silent(bot, sent_at: float, description: str) -> None:
    # 截止时刻用单调钟（time.monotonic），不用事件时间戳 bot.events[-1].t：
    # 静默断言正是"之后无事件到达"，事件时间不会推进，以事件时间做 deadline 会
    # 永远等不到 now >= end_at 而死循环（review finding 1/5）。
    deadline = time.monotonic() + SILENT_WINDOW
    while True:
        _scan_silent_violations(bot, sent_at, description)
        if time.monotonic() >= deadline:
            # 终末复扫：事件扫描与 deadline 判定非原子（central-review 2029 #3），
            # deadline 判定成立后、返回前再扫一次，收口最后一段未观测窗口——否则
            # 该段内到达的 qi_color_observed/聊天会被漏掉。
            _scan_silent_violations(bot, sent_at, description)
            return
        bot.assert_alive(f"{description} 窗口内连接保持")
        time.sleep(0.1)


def _scan_silent_violations(bot, sent_at: float, description: str) -> None:
    # 静默契约 = 「无任何非周期 S2C 响应 + 无聊天」。只盯 qi_color_observed 会放走
    # 拒收却发 event_alert / 库存更新等任何其他 payload 的坏实现（central-review
    # 2029 #4）；白名单外一律判红。realm set 的 player_state 回推已由调用方排空
    # （_realm_set_and_settle），tpzone/tpdim 的 zone_info 在 setup 阶段（sent_at 前）
    # 落定——窗口内除 carrier_state 无合法非白名单 payload。
    for e in bot.events_of("server_data"):
        if (
            e.t > sent_at
            and e.data["payload_type"] not in AMBIENT_PERIODIC_PAYLOAD_TYPES
        ):
            raise BotAssertionError(
                f"[{bot.username}] {description}，"
                f"实际窗口内收到 server_data/{e.data['payload_type']}（t={e.t:.3f}）"
            )
    for e in bot.events_of("chat"):
        # dev 修为切换的确认回显可能晚于 sent_at 到达（实测出现在窗口内），
        # 属场景 setup 噪音；其余真实新聊天一律判红。
        if e.t > sent_at and "realm set" not in e.data.get("text", ""):
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
            )
