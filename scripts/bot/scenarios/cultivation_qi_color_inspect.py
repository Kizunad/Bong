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
  （secondary=None、is_chaotic=false、is_hunyuan=false——**main 保留**）。

断言面（两 bot 同出生点，相距 <6m）：
1. host 凝实 + victim 引气 → diff=2 全量 payload。victim 以真实功法施放积累修行
   （崩拳空挥×4 → Heavy，吸灵口空挥×2 → Intricate），QiColor 演化为非默认
   main=Heavy、secondary=Intricate。全量断言校验**非默认敏感状态**：
   main="Heavy"、secondary="Intricate"——脱敏分支把 secondary 打成 None 而
   main 保留，只断言 main 会放走「对一切正境界差都套脱敏」的坏实现；另含
   is_chaotic/is_hunyuan=false、realm_diff=2、observer/observed 为
   offline:<username> canonical id（observer **全等**，不是前缀）；
2. host 降回引气（同境界）→ 静默（realm_diff=0 continue，无 S2C）；
3. 不存在协议 id 的 observed → dispatch 静默丢弃（无 S2C、无聊天）；
4. victim tpdim 到 TSY（同裸 XYZ，距离前提仍满足）→ host 凝实再探真实 victim
   → 静默（same_dimension 失败，唯一失败的空间前提是维度）；
5. victim 回 overworld 后 tpzone 到远端 zone（同维、距 host >6m）→ host 凝实
   再探真实 victim → 静默（distance 失败，唯一失败的空间前提是距离）。
   4/5 都以**真实可见实体**（victim 的 protocol entity id）放在空间门另一侧——
   只发不存在 id 会放走「resolve 正常但漏掉距离/维度检查」的坏实现
   （central-review 2029 #5）。
"""

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, payload_text
from bot.scenarios._inventory_helpers import wait_join_and_inventory

DESCRIPTION = (
    "qi_color_inspect：跨境界全量 payload（victim 非默认真元色 main=Heavy/"
    "secondary=Intricate）、observer 全等 canonical id、同境界静默、坏 target 静默"
)
MODULES = ["cultivation", "network", "combat", "skill", "cmd"]

INSPECT_REQUEST = {"type": "qi_color_inspect", "v": 1}
SILENT_WINDOW = 4.0

BENG_QUAN = "burst_meridian.beng_quan"
WOLIU_MOUTH = "woliu.mouth"
SLOT_BENG = 0
SLOT_WOLIU = 1
HEAVY_CASTS = 4
INTRICATE_CASTS = 2
# BENG_QUAN_COOLDOWN_TICKS=60（3s）；woliu mouth 引气境 cooldown 8s（8*TICKS_PER_SECOND）。
# 每 20 ticks/s，留 margin。
BENG_GAP = 3.5
WOLIU_GAP = 8.5


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
    time.sleep(0.3)

    for _ in range(HEAVY_CASTS):
        _cast_empty_and_confirm(
            bot,
            SLOT_BENG,
            lambda e: e.data["payload_type"] == "burst_meridian_event"
            and e.data["payload"].get("skill") == BENG_QUAN,
            "空挥崩拳应收到 burst_meridian_event（施放被接受，Heavy 修行落账）",
        )
        time.sleep(BENG_GAP)

    # 崩拳按当前真元 40% 扣费，4 次后余量不足吸灵口 12+3*2=18 的启动费，补足。
    bot.cmd("qi set 100")
    bot.expect_chat("[dev] qi set", timeout=10.0)

    for _ in range(INTRICATE_CASTS):
        _cast_empty_and_confirm(
            bot,
            SLOT_WOLIU,
            lambda e: e.data.get("channel") == "bong:vfx_event"
            and "bong:woliu_mouth_funnel" in payload_text(e),
            "空挥吸灵口应收到 bong:woliu_mouth_funnel vfx（施放被接受，Intricate 修行落账）",
        )
        time.sleep(WOLIU_GAP)


def _cast_empty_and_confirm(bot, slot: int, confirm, description: str) -> None:
    """空挥 skill_bar_cast（无 target = 空挥/空吸），等施放被接受的 server 反馈。

    冷却/经脉/真元门被拒时不会有反馈 → wait_for 超时，场景正确失败而非静默空过。"""
    anchor = last_event_time(bot)
    bot.intent({"type": "skill_bar_cast", "v": 1, "slot": slot})
    bot.wait_for(
        lambda e: e.t > anchor and confirm(e),
        timeout=10.0,
        description=description,
    )


def run(env) -> None:
    with env.new_bot("QiH") as host:
        wait_join_and_inventory(host)
        # host 凝实（rank 3）+ victim 引气（rank 1）→ diff=2 全量分支。
        host.cmd("realm set solidify")
        host.expect_chat("[dev] realm set ", timeout=10.0)

        with env.new_bot("QiV") as victim:
            # 事件水位必须取在 victim join **之前**：PlayerSpawn 可在 join 期间的
            # 任意时刻到达 host。只匹配 e.kind=="player_spawn" 会选中历史或并发
            # PlayerSpawn（central-review 2029 #4）——水位 + canonical username 双
            # 锚定到 victim 本人。username 来自 host 的 player_names（S2C_PLAYER_LIST
            # 在 PlayerSpawn 同包之前推送，Valence 标准顺序）。
            spawn_watermark = host.events[-1].t if host.events else 0.0
            wait_join_and_inventory(victim)

            spawn = host.wait_for(
                lambda e: (
                    e.kind == "player_spawn"
                    and e.t > spawn_watermark
                    and e.data.get("username") == victim.username
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

            # 1. 凝实 host 探引气 victim → diff=2 全量 payload（非默认真元色）
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            observed = host.expect_server_data("qi_color_observed", timeout=10.0)
            payload = observed.data["payload"]
            _assert_full_payload(host, payload, victim.username)
            host.assert_alive("跨境界 qi_color_inspect 后")

            # 2. host 降为凝实（rank 3）、victim 仍引气（rank 2）→ diff=1 脱敏 payload：
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

            # 3. host 降回引气（同境界）→ realm_diff=0 静默
            host.cmd("realm set induce")
            host.expect_chat("[dev] realm set ", timeout=10.0)
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            _assert_silent(host, sent_at, "同境界 qi_color_inspect 应静默（realm_diff=0 continue）")

            # 4. 不存在的协议 id → dispatch 静默丢弃
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": "entity:999999999"})
            _assert_silent(host, sent_at, "坏 observed 协议 id 应被 dispatch 静默丢弃")

            # 5. victim tpdim 到 TSY（同裸 XYZ，距离前提仍满足）→ host 凝实再探真实
            #    victim → 静默（same_dimension 失败，唯一失败的空间前提是维度）。
            #    空间门必须**可隔离**：把双 bot 精确共位到同一 zone 中心（tpzone 定点
            #    落位，distance=0）再 tpdim 只移 victim（X+0.25）→ distance=0.25 ≤ 6m。
            #    若直接依赖出生点相对距离，tpdim 的 +0.25 位移可能把近 6m 的一对推过
            #    6m，distance 与 dimension 同时失败——漏 dimension 检查的坏实现照样
            #    静默，隔离性破（central-review 2029 #5）。跨维 transfer 只改
            #    EntityLayerId/Position，ECS Entity 与 protocol entity id 保持不变
            #    （dimension_transfer.rs），step 0 捕获的旧 id 仍解析到 victim。
            host.cmd("tpzone jiuzong_taichu_ruin")
            host.expect_chat("Teleported to zone `jiuzong_taichu_ruin`.", timeout=10.0)
            victim.cmd("tpzone jiuzong_taichu_ruin")
            victim.expect_chat("Teleported to zone `jiuzong_taichu_ruin`.", timeout=10.0)
            host.cmd("realm set condense")
            host.expect_chat("[dev] realm set ", timeout=10.0)
            _transfer_dimension(victim, "tsy", victim.events[-1].t if victim.events else 0.0)
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            _assert_silent(host, sent_at, "跨维度 qi_color_inspect 应静默（same_dimension 失败）")

            # 6. victim 回 overworld 后 tpzone 到远端 zone（同维、距 host >6m）→ host
            #    凝实再探真实 victim → 静默（distance 失败，唯一失败的空间前提是距离）。
            #    tpdim overworld 把 victim 的 X+0.25 精确还原，与 host 再次同坐标共位；
            #    tpzone wangyintai 再到 (4000,144,-1650)——同 overworld 维、距 host
            #    （jiuzong 中心 (0,109,-10000)）~9257m。若距离前提不满足（victim 仍近），
            #    漏 distance 检查的坏实现也能静默，隔离性破。
            _transfer_dimension(victim, "overworld", victim.events[-1].t if victim.events else 0.0)
            victim.cmd("tpzone wangyintai")
            victim.expect_chat("Teleported to zone `wangyintai`.", timeout=10.0)
            sent_at = host.events[-1].t if host.events else 0.0
            host.intent({**INSPECT_REQUEST, "observed": f"entity:{victim_protocol_id}"})
            _assert_silent(host, sent_at, "远端 zone 的 qi_color_inspect 应静默（distance 失败）")

            victim.assert_alive("qi_color_inspect 全程 victim")
            host.assert_alive("qi_color_inspect 拒绝面全程")


def _assert_full_payload(host, payload: dict, victim_username: str) -> None:
    if payload.get("main") != "Heavy":
        raise BotAssertionError(
            f"[{host.username}] 期望 QiColor.main=Heavy（victim 崩拳修行演化），"
            f"实际 {payload.get('main')}"
        )
    if payload.get("realm_diff") != 2:
        raise BotAssertionError(
            f"[{host.username}] 期望 realm_diff=2（凝实-引气），实际 {payload.get('realm_diff')}"
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

    server 端把脱敏后的 `secondary` 序列化为缺失键（None Option 省略，单测断言
    get("secondary").is_none()），故 `payload.get("secondary") is None` 对「键
    缺失」与「显式 null」两种坏实现都判红。"""
    if payload.get("main") != "Heavy":
        raise BotAssertionError(
            f"[{host.username}] 期望脱敏 payload.main=Heavy 保留（victim 崩拳修行演化），"
            f"实际 {payload.get('main')}"
        )
    if payload.get("realm_diff") != 1:
        raise BotAssertionError(
            f"[{host.username}] 期望 realm_diff=1（凝实-引气），实际 {payload.get('realm_diff')}"
        )
    if payload.get("secondary") is not None:
        raise BotAssertionError(
            f"[{host.username}] 期望脱敏 payload 隐藏 secondary，实际 {payload.get('secondary')!r}"
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


def _transfer_dimension(bot, target: str, after: float) -> None:
    """把 bot 经正式 transfer consumer 切到 target 维度（保持同 XYZ 邻域）。

    等 server 权威 transfer 排队的聊天确认 + 真实跨维 Respawn，并断言 Respawn 携带
    目标维度（dimension_type_name/dimension_name 双键任一命中即可）。维度未完成切换
    前不 dispatch 探针——否则正确实现会因同维发 payload，静默断言在 setup 阶段就
    假红。跨维 transfer 不换 ECS Entity，protocol entity id 保持有效。"""
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
    if any(value != expected for value in actual.values()):
        raise BotAssertionError(
            f"[{bot.username}] /tpdim {target} 的 Respawn 必须携带目标维度 {expected}，"
            f"实际 {actual}"
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
    for e in bot.events_of("server_data"):
        if e.t > sent_at and e.data["payload_type"] == "qi_color_observed":
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际收到 qi_color_observed（t={e.t:.3f}）"
            )
    for e in bot.events_of("chat"):
        # dev 修为切换的确认回显可能晚于 sent_at 到达（实测出现在窗口内），
        # 属场景 setup 噪音；静默断言只看 qi_color_observed 与真实新聊天。
        if e.t > sent_at and "realm set" not in e.data.get("text", ""):
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
            )
