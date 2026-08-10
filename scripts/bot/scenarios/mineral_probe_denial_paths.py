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

DESCRIPTION = "mineral_probe 拒绝面：Awaken→realm_too_low、凝脉→not_mineral_ore、出界→静默"
MODULES = ["mineral", "network"]

PROBE_REQUEST = {"type": "mineral_probe", "v": 1}
SILENT_WINDOW = 4.0
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client。
# cultivation_detail 需 MeridianSystem+Cultivation，本场景只 realm set（不加
# 经脉系统），不应出现——若出现即判红（这正是契约要求的「无 S2C 响应」）。
AMBIENT_PERIODIC_PAYLOAD_TYPES = frozenset({"carrier_state"})
# 权威 Position 的 y 带：实测 spawn 后周期性 +10（72→82→92…），单点探针必超 6m。
# 列盲扫覆盖 [-6, +30]（步长 4），覆盖整条提升带；列外则重读位置换列。
Y_LO = -6
Y_HI = 30
Y_STEP = 4
COLUMN_TIMEOUT = 8.0
COLUMN_MAX_ATTEMPTS = 5


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
    下一 realm 阶段，等于没有屏障（central-review 2029 #6）。
    """
    deadline = time.monotonic() + max_wait
    while True:
        anchor = bot.events[-1].t if bot.events else 0.0
        time.sleep(quiet)
        if _has_results_newer_than(bot, anchor):
            if time.monotonic() > deadline:
                raise BotAssertionError(
                    f"[{bot.username}] mineral_probe_result 在 {max_wait:.0f}s 内持续到达，"
                    f"批次屏障无法收敛——拒绝静默放行可能污染下一批"
                )
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
    """
    first_deadline = time.monotonic() + first_result_timeout
    converge_deadline = time.monotonic() + convergence_cap
    collected: list = []
    last_seen = after_t
    while True:
        time.sleep(quiet)
        fresh = [
            e
            for e in bot.events_of("server_data")
            if e.data["payload_type"] == "mineral_probe_result"
            and e.t > after_t
            and e.t > last_seen
        ]
        if fresh:
            collected.extend(fresh)
            last_seen = max(e.t for e in fresh)
            if time.monotonic() >= converge_deadline:
                raise BotAssertionError(
                    f"[{bot.username}] mineral_probe_result 在 {convergence_cap:.0f}s 内持续到达，"
                    f"批次收集无法收敛——拒绝静默放行可能污染下一批"
                )
            continue
        if collected:
            return collected
        if time.monotonic() >= first_deadline:
            return collected


def _column_probe_and_expect(bot, reason: str, label: str) -> None:
    """竖直列盲扫并校验 denial_reason；超时则重读位置换列，最多重试多次。

    docstring 记载的修法（central-review 2029 #2）：权威 y 周期性 +10 会让单点探针
    必超 6m 被 dispatch 前置过滤静默丢弃，因此以 bot.position 为中心的列盲扫 + 超时
    后重读位置换列重试，`COLUMN_MAX_ATTEMPTS` 真正被使用；全部超时才报错。
    """
    for attempt in range(1, COLUMN_MAX_ATTEMPTS + 1):
        # 批次屏障：先排空上一扫在途响应，本批 sent_at 之后只可能是本批结果——
        # 迟到响应不得跨 realm 阶段被误认成另一批（central-review 2029 #3）。
        _drain_probe_results(bot)
        c = bot.position
        if c is None:
            raise BotAssertionError("mineral_probe 场景需要 pos_look 后的位置，实际 position=None")
        x = math.floor(c[0]) + 1
        z = math.floor(c[2])
        base_y = math.floor(c[1])
        sent_at = bot.events[-1].t if bot.events else 0.0
        for y in range(base_y + Y_LO, base_y + Y_HI + 1, Y_STEP):
            bot.intent({**PROBE_REQUEST, "x": x, "y": y, "z": z})
        # 收集本批**全部**响应并逐条校验（central-review 2029 #2）：只等第一个
        # denial_reason 匹配、其余结果静默丢弃，会放走批内额外发出的 denied/其他理由
        # 或 found 响应——realm 优先边界契约就没锁住。
        results = _collect_probe_results(bot, sent_at)
        if not results:
            # 整列都落在权威位置 6m 外（y 带已移动）：排空本批可能迟到的结果后
            # 重读位置换一列重试。
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
        f"[{bot.username}] {label}：{COLUMN_MAX_ATTEMPTS} 次列盲扫均超时"
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
