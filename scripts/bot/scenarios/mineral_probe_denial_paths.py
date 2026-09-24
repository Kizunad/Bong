"""mineral_probe 拒绝面（实体/空间探知流，plan-mineral-v1 M4a）。

resolve_one_probe（mineral/probe.rs:41）的检查顺序：
1. 修为 < 凝脉（MIN_PROBE_REALM_RANK=2）→ Denied(RealmTooLow)
2. 目标超出 MAX_DISTANCE=6.0 → Denied(OutOfRange)
3. MineralOreIndex 查无该块 → Denied(NotMineralOre)

S2C：`ServerDataPayloadV1::MineralProbeResult { kind:"denied", denial_reason }`
（mineral_probe_emit.rs，denial_reason 为 snake_case tag）。

fixture 出生点无矿脉 anchor（mineral_anchors.json 只在具名 zone 落地，新手 raster
无矿），故 Found 路径不可构造——本场景锁三条可黑盒断言路径：

1. Awaken 玩家探附近方块 → denied + realm_too_low；
2. `[dev] realm set condense` 后探同一区域 → denied + not_mineral_ore；
3. 探 >6m 外的坐标 → dispatch 层 is_probe_target_in_range 前置过滤，静默丢弃
   （无 S2C、无聊天、连接保持——与 resolve_one_probe 的 OutOfRange 不同，后者
   只在越过后置范围检查时出现，dispatch 前置过滤使该理由不可达）。

【权威位置获取的实测教训（fixture 下）】：
- 玩家权威 Position 不是稳定出生点：join 后它会周期性 +10（实测 72→82→92，
  ~8s 一步，疑似 spawn 提升/防卡系统），dispatch 前置范围检查读的就是它。
- bot.position 只在服务器推 S2C PlayerPositionLook 时更新（该推送间隔可达 ~8s），
  join 的 PlayerPositionLook 与权威 Position 不一定一致（join 值可能只是缓冲位）。
- 因此从 bot.position 推导的**单点**探针必然周期性超出 6m，被前置过滤静默丢弃。
- 修法：对以 bot.position 为中心的一列竖直候选**盲扫**（y 带 -6..+30，步长 4），
  无论权威位置当前在哪一档，总有一个目标落在其 6m 内 → realm 门照常触发。
  超时（权威位置已移到列外）则重读 bot.position 换一列重试。
"""

import math
import time

from bot.bot import BotAssertionError

from ._inventory_helpers import wait_join_and_inventory
from ._rejection_helpers import AMBIENT_SERVER_DATA_TYPES

DESCRIPTION = "mineral_probe 拒绝面：Awaken→realm_too_low、凝脉→not_mineral_ore、出界→静默"
MODULES = ["mineral", "network"]

PROBE_REQUEST = {"type": "mineral_probe", "v": 1}
SILENT_WINDOW = 4.0
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client。
# cultivation_detail 需 MeridianSystem+Cultivation，本场景只 realm set（不加
# 经脉系统），不应出现——若出现即判红（这正是契约要求的「无 S2C 响应」）。
AMBIENT_PERIODIC_PAYLOAD_TYPES = AMBIENT_SERVER_DATA_TYPES
# 权威 Position 的 y 带：实测 spawn 后周期性 +10（72→82→92…），单点探针必超 6m。
# 列盲扫覆盖 [-6, +30]（步长 4），覆盖整条提升带；列外则重读位置换列。
Y_LO = -6
Y_HI = 30
Y_STEP = 4
COLUMN_TIMEOUT = 8.0
COLUMN_MAX_ATTEMPTS = 5
# 事件驱动的轮询间隔：等首个响应时不睡满 quiet 才去看，结果通常亚秒~1s 内到达，
# 0.1s 轮询让 happy path 从「盲睡 quiet」降到「快发现 + 单次结算窗」（review
# finding 3：每 phase ≥16s 的串行固定等待）。
RESULT_POLL_INTERVAL_S = 0.1
# 等 pos_look 刷新权威位置的短超时：稳定玩家契约上不再收 pos_look（docstring 载），
# 旧 8s 满超时被当正常控制流白白吃掉；1s 内若权威步进必有 pos_look 到达（wait_for
# 从 cursor 0 重扫含历史，上一 attempt 的处理窗口给过它落地时间），错过则由基数校验
# + 换列重试收敛（review finding 3）。
POS_LOOK_REFRESH_WAIT_S = 1.0


def run(env) -> None:
    with env.new_bot("MPH") as bot:
        wait_join_and_inventory(bot)
        if bot.position is None:
            raise BotAssertionError("mineral_probe 场景需要 pos_look 后的位置，实际 position=None")

        # 1. Awaken → realm_too_low
        _column_probe_and_expect(bot, "realm_too_low", "Awaken 列盲扫")

        # 2. 凝脉 → 同一区域仍非矿脉 → not_mineral_ore
        bot.cmd("realm set condense")
        bot.expect_chat("[dev] realm set ", timeout=10.0)
        _column_probe_and_expect(bot, "not_mineral_ore", "凝脉列盲扫")

        # 3. 出界（x/z +40）→ dispatch 前置范围过滤静默丢弃。y 取当前 bot.position
        #    （权威 y 在提升带上移动，但 x/z 稳定，+40 后水平距离必超 6m）。
        c = bot.position
        if c is None:
            raise BotAssertionError("mineral_probe 场景需要 pos_look 后的位置，实际 position=None")
        far_target = (math.floor(c[0]) + 40, math.floor(c[1]), math.floor(c[2]) + 40)
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent({**PROBE_REQUEST, "x": far_target[0], "y": far_target[1], "z": far_target[2]})
        _assert_no_probe_result(bot, sent_at, "出界 mineral_probe 应被前置范围过滤静默丢弃")
        bot.assert_alive("mineral_probe 拒绝面全程")


def _drain_probe_results(bot, quiet: float = 2.0, max_wait: float = 20.0) -> None:
    """批次间屏障：排空在途 mineral_probe_result，直到连续两个静默样本都无新结果。

    探针结果异步到达，批间必须排空在途响应——否则上一扫的迟到响应会落入下一批次
    sent_at 之后的窗口，被当成当前批次结果（central-review 2029 #3）。单一静默样本
    不能当作「管线已排空」的原子证据：前一扫的迟到响应可能恰在空扫描之后、下一批
    请求之前到达，只睡一个 quiet 窗口就返回会放走它，被 _collect_probe_results 误记
    入新批（realm 迁移错误正是这样被污染的，central-review 2029 #6）。修法：末次空
    样本后再做一次确认扫描——重新锚定事件水位再等一个静默样本，两样本连续为空才
    返回。静默窗口必须显著长于服务端实际响应延迟（同 tick 处理，亚秒~1s）；**到上限
    时若仍有响应在途就报错**——静默放行一个还在来响应的批次，会把污染交给下一批/
    下一 realm 阶段，等于没有屏障（central-review 2029 #6）。死线同时约束确认分支：
    确认窗口内继续到响应时回到主循环顶重查死线，不会绕过上限无限重试
    （central-review 2029 #7——旧实现只在主分支有新结果时查死线，确认分支
    `continue` 直接回主循环，延迟/重复产出的响应每轮确认都命中该分支，循环永不
    收敛）。
    """
    deadline = time.monotonic() + max_wait
    while True:
        # 死线检查放在循环顶，让**所有**路径（主分支与确认分支）都受 max_wait
        # 约束：确认分支发现新结果 `continue` 回主循环时同样先过死线，不可能无限
        # 排空。central-review 2029 #7：max_wait 不约束所有路径 = 无约束。
        if time.monotonic() > deadline:
            raise BotAssertionError(
                f"[{bot.username}] mineral_probe_result 在 {max_wait:.0f}s 内持续到达，"
                f"批次屏障无法收敛——拒绝静默放行可能污染下一批"
            )
        anchor = bot.events[-1].t if bot.events else 0.0
        time.sleep(quiet)
        if _has_results_newer_than(bot, anchor):
            continue
        # 末次空样本后、返回前再确认一次：前一扫迟到响应可能恰在空扫描之后到达，
        # 单一空样本把「此刻恰好无新事件」误当「管线已排空」。确认样本同样必须为
        # 空，否则回到主循环继续排空（两个连续静默样本才构成屏障）。
        confirm_anchor = bot.events[-1].t if bot.events else 0.0
        time.sleep(quiet)
        if _has_results_newer_than(bot, confirm_anchor):
            continue
        return


def _has_results_newer_than(bot, anchor: float) -> bool:
    return any(
        e.t > anchor
        for e in bot.events_of("server_data")
        if e.data["payload_type"] == "mineral_probe_result"
    )


def _collect_probe_results(
    bot,
    after_t: float,
    quiet: float = 2.0,
    first_result_timeout: float = COLUMN_TIMEOUT,
    convergence_cap: float = 20.0,
) -> list:
    """收集 after_t 之后到批静默的全部 mineral_probe_result（本批全部响应）。

    探针结果异步到达，批内全部响应必须被收集后再逐条校验——只取第一个匹配响应、
    把其余静默丢弃，会放走「一条正确 denied/<reason> + 额外 denied/其他理由 或
    found」的坏实现（central-review 2029 #2）。静默窗口必须显著长于服务端实际响应
    延迟（同 tick 处理，亚秒~1s）。返回空列表 = 整批无响应（全部被前置范围过滤），
    调用方据此重读位置换列重试。first_result_timeout 约束「等首个结果」的耗时；
    convergence_cap 内始终有新结果（无法出现完整静默窗）则报错——静默放行一个还在
    来响应的批次，会把污染交给下一批/下一 realm 阶段（central-review 2029 #6）。

    等首个结果用**事件驱动短轮询**（RESULT_POLL_INTERVAL_S）而非先盲睡一个 quiet
    窗，首个结果到达后以**单个**结算窗（quiet，随新结果重置）收尾——旧实现发现 +
    确认两段串行固定 sleep（~4s），happy path 也被迫吃满（review finding 3）。
    """
    def fresh_after(last_seen: float) -> list:
        return [
            e
            for e in bot.events_of("server_data")
            if e.data["payload_type"] == "mineral_probe_result"
            and e.t > after_t
            and e.t > last_seen
        ]

    first_deadline = time.monotonic() + first_result_timeout
    converge_deadline = time.monotonic() + convergence_cap
    collected: list = []
    last_seen = after_t
    # 事件驱动等首个结果：结果通常亚秒~1s 内到达，短轮询**发现**而不是先盲睡一个
    # quiet 窗（review finding 3——旧实现 happy path 也要睡满 quiet 才发现结果）。
    # 空返回 = 整批无响应（全部被前置范围过滤），调用方据此重读位置换列重试。
    while not collected:
        fresh = fresh_after(last_seen)
        if fresh:
            collected.extend(fresh)
            last_seen = max(e.t for e in fresh)
            break
        if time.monotonic() >= first_deadline:
            return collected
        time.sleep(RESULT_POLL_INTERVAL_S)
    # 单次结算窗口：quiet 内无新结果即批完成；新结果到达则重置窗口——不能把「正确
    # 结果之外还在陆续来响应」当静默放行，convergence_cap 内持续到响应即报错
    # （central-review 2029 #6 语义原样保留）。
    settle_deadline = time.monotonic() + quiet
    while True:
        fresh = fresh_after(last_seen)
        if fresh:
            collected.extend(fresh)
            last_seen = max(e.t for e in fresh)
            if time.monotonic() >= converge_deadline:
                raise BotAssertionError(
                    f"[{bot.username}] mineral_probe_result 在 {convergence_cap:.0f}s 内持续到达，"
                    f"批次收集无法收敛——拒绝静默放行可能污染下一批"
                )
            settle_deadline = time.monotonic() + quiet
            continue
        if time.monotonic() >= settle_deadline:
            return collected
        time.sleep(RESULT_POLL_INTERVAL_S)


def _wait_fresh_pos_look(bot, timeout: float = 8.0) -> None:
    """尽力等一个全新 pos_look，让 bot.position 拿到权威位置的最近一档（best-effort）。

    wait_for 每次从 cursor 0 重扫含历史事件（bot.py:497「含历史事件」），裸
    predicate 会命中 join 的旧 pos_look；必须用时间锚限定「锚后新到达」。bot.py 的
    pos_look handler 同步把 self.position 更新为事件坐标（bot.py:145），返回后读
    bot.position 即得服务端最近一次推送的权威位置。

    timeout 内无新 pos_look **不是错误**：权威位置每 ~8s 一步（步进即 position
    变更 → valence 自动向 client 同步 PlayerPosLook），停止移档后稳定玩家不再收
    pos_look，此时 bot.position 就是当前权威档，直接沿用即可。移档期间必有 pos_look，
    timeout 取一档余量（8s）足以拿到最近档；若仍错过，后续的基数校验 + 换列重试会
    收敛（下一 attempt 的 drain 窗口又给 pos_look 落地时间）。"""
    anchor = bot.events[-1].t if bot.events else 0.0
    try:
        bot.wait_for(
            lambda e: e.kind == "pos_look" and e.t > anchor,
            timeout=timeout,
            description="等待新 pos_look 刷新权威位置",
        )
    except BotAssertionError:
        pass  # 稳定档：bot.position 即权威位置，无需刷新


def _expected_in_range_count(player_pos, x: int, z: int, base_y: int) -> int:
    """按 dispatch 前置过滤的同一几何判定，算本列在范围内的目标数。

    client_request_handler.rs:2030 用 is_probe_target_in_range（mineral/probe.rs:79）：
    目标中心 (x+0.5, y+0.5, z+0.5) 到权威 Position 的欧氏距离平方 ≤ 36.0
    （MINERAL_PROBE_MAX_DISTANCE=6.0）。超距目标被前置过滤静默 continue（无 S2C），
    故本列实际响应数应恰好等于在范围内目标数——这是批响应完整性的基数断言
    （central-review 2029 #8：MineralProbeResultV1 payload 不带坐标，响应无法按目标
    回映，非空子集与完整批不可区分）。"""
    return sum(
        1
        for y in range(base_y + Y_LO, base_y + Y_HI + 1, Y_STEP)
        if math.dist(player_pos, (x + 0.5, y + 0.5, z + 0.5)) ** 2 <= 36.0
    )


def _column_probe_and_expect(bot, reason: str, label: str) -> None:
    """竖直列盲扫并校验 denial_reason 与批响应基数；超时/基数不符则重读位置换列。

    docstring 记载的修法（central-review 2029 #2）：权威 y 周期性 +10 会让单点探针
    必超 6m 被 dispatch 前置过滤静默丢弃，因此以 bot.position 为中心的列盲扫 + 超时
    后重读位置换列重试，`COLUMN_MAX_ATTEMPTS` 真正被使用；全部超时才报错。

    central-review 2029 #8：只做非空子集校验会放走「在范围内请求被静默丢弃」的坏
    实现。修法：先等全新 pos_look 拿到权威位置最近一档，按同一几何判定算本列期望
    在范围内目标数，收集后断言 len(results)==expected；基数不符（权威位置批间移档
    或实现丢响应）重读位置换列复核，不能把非空子集当完整批放行。

    review finding 3：批次屏障与 pos_look 等待不再无条件吃满固定时延——
    - 批次屏障只在「上一扫可能仍有在途响应」时才排空（`settled`：空收集 = 可能仍在
      途 → 下 attempt 全量排空；非空收集 = 已过结算窗 = 已排空 → 下 attempt 直接跳过，
      首批前无任何请求也跳过）。跳过屏障后本批 sent_at 取当前水位，上一批响应若迟到
      其 t < 本批 sent_at，仍被 _collect_probe_results 的 t>after_t 过滤排除，屏障语义
      不因跳过而弱化；
    - pos_look 等短超时（POS_LOOK_REFRESH_WAIT_S）：稳定玩家契约上不再收 pos_look，
      旧 8s 满超时被当正常控制流吃掉；错过则由本函数的重试循环收敛。
    """
    settled = True
    for attempt in range(1, COLUMN_MAX_ATTEMPTS + 1):
        # 批次屏障（review finding 3）：只在上扫可能仍在途时排空，避免首批/已结算
        # 批也吃满两次静默样本。settled 在收集后更新——非空收集以结算窗收尾即已排空。
        if not settled:
            _drain_probe_results(bot)
        # 期望基数必须对权威位置计算：bot.position 可能仍是陈旧档（join 值只是缓冲位，
        # 权威 Position 周期性 +10），用陈旧位置算期望基数会与实际不符，基数断言形同
        # 虚设（central-review 2029 #8）。先尽力等一个全新 pos_look 刷新到最近档
        # （best-effort：移档期间必有 pos_look，稳定档则 bot.position 已是权威值；
        # 短超时错过的，wait_for 从 cursor 0 重扫含历史，重试收敛）。
        _wait_fresh_pos_look(bot, timeout=POS_LOOK_REFRESH_WAIT_S)
        c = bot.position
        if c is None:
            raise BotAssertionError("mineral_probe 场景需要 pos_look 后的位置，实际 position=None")
        x = math.floor(c[0]) + 1
        z = math.floor(c[2])
        base_y = math.floor(c[1])
        expected = _expected_in_range_count(c, x, z, base_y)
        if expected == 0:
            # 权威 y 带与整列无重叠（异常 fixture 行为）：重读位置换列。
            continue
        sent_at = bot.events[-1].t if bot.events else 0.0
        for y in range(base_y + Y_LO, base_y + Y_HI + 1, Y_STEP):
            bot.intent({**PROBE_REQUEST, "x": x, "y": y, "z": z})
        # 收集本批**全部**响应并逐条校验（central-review 2029 #2）：只等第一个
        # denial_reason 匹配、其余结果静默丢弃，会放走批内额外发出的 denied/其他理由
        # 或 found 响应——realm 优先边界契约就没锁住。
        results = _collect_probe_results(bot, sent_at)
        # Only a complete batch is settled. A non-empty short batch can still have
        # delayed responses in flight; drain before retrying so they cannot be
        # combined with the next request batch (VRFY PR2029 major finding).
        settled = len(results) == expected
        if len(results) != expected:
            # 基数不符（central-review 2029 #8）：既可能是权威位置在批间 +10 移档
            # （y 带整体平移，期望数随之改变），也可能是实现在范围内请求被静默丢弃。
            # 两者都不可判红——移档是合法行为——但都必须重读位置换列复核，不能把
            # 非空子集当完整批放行。
            continue
        for e in results:
            payload = e.data["payload"]
            if payload.get("kind") != "denied" or payload.get("denial_reason") != reason:
                raise BotAssertionError(
                    f"[{bot.username}] {label}：期望批内全部响应为 denied/{reason}，"
                    f"实际 {payload}（t={e.t:.3f}）"
                )
        bot.assert_alive(f"{label} 后")
        return
    raise BotAssertionError(
        f"[{bot.username}] {label}：{COLUMN_MAX_ATTEMPTS} 次列盲扫均超时或基数不符"
        f"（权威位置持续移出 y 带，需人工确认 fixture 行为）"
    )


def _assert_no_probe_result(bot, sent_at: float, description: str) -> None:
    # 截止时刻用单调钟（time.monotonic），不用事件时间戳 bot.events[-1].t：
    # 静默断言正是"之后无事件到达"，事件时间不会推进，以事件时间做 deadline 会
    # 永远等不到 now >= end_at 而死循环（review finding 1/5）。
    deadline = time.monotonic() + SILENT_WINDOW
    while True:
        _scan_silent_violations(bot, sent_at, description)
        if time.monotonic() >= deadline:
            # 终末复扫：事件扫描与 deadline 判定非原子（central-review 2029 #3），
            # deadline 判定成立后、返回前再扫一次，收口最后一段未观测窗口——否则
            # 该段内到达的 server_data/聊天会被漏掉。
            _scan_silent_violations(bot, sent_at, description)
            return
        bot.assert_alive(f"{description} 窗口内连接保持")
        time.sleep(0.1)


def _scan_silent_violations(bot, sent_at: float, description: str) -> None:
    for e in bot.events_of("server_data"):
        # 出界探针契约是「无 S2C 响应」：白名单外的 payload 一律判红。只盯
        # mineral_probe_result 会放走拒收却发 event_alert / 库存更新的坏实现
        # （review finding 5）。
        if e.t > sent_at and e.data["payload_type"] not in AMBIENT_PERIODIC_PAYLOAD_TYPES:
            raise BotAssertionError(
                f"[{bot.username}] {description}，"
                f"实际窗口内收到 server_data/{e.data['payload_type']}（t={e.t:.3f}）"
            )
    for e in bot.events_of("chat"):
        if e.t > sent_at:
            raise BotAssertionError(
                f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
            )
