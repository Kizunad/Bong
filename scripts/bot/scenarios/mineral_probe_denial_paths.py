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
    """批次间屏障：排空在途 mineral_probe_result，直到静默窗口内无新结果。

    探针结果异步到达，批间必须排空在途响应——否则上一扫的迟到响应会落入下一批次
    sent_at 之后的窗口，被当成当前批次结果（central-review 2029 #3）。静默窗口
    必须显著长于服务端实际响应延迟（同 tick 处理，亚秒~1s）；**到上限时若仍有响应
    在途就报错**——静默放行一个还在来响应的批次，会把污染交给下一批/下一 realm
    阶段，等于没有屏障（central-review 2029 #6）。
    """
    deadline = time.monotonic() + max_wait
    while True:
        anchor = bot.events[-1].t if bot.events else 0.0
        time.sleep(quiet)
        stray = [
            e
            for e in bot.events_of("server_data")
            if e.data["payload_type"] == "mineral_probe_result" and e.t > anchor
        ]
        if not stray:
            return
        if time.monotonic() > deadline:
            raise BotAssertionError(
                f"[{bot.username}] mineral_probe_result 在 {max_wait:.0f}s 内持续到达，"
                f"批次屏障无法收敛——拒绝静默放行可能污染下一批"
            )


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
        try:
            # 结果必须**同时**满足期望的 denial_reason，而非只按到达时间归属批次：
            # 若上一 realm 阶段的迟到响应（realm_too_low）跨屏障落入本批窗口，它会被
            # 谓词跳过，本批的权威结果（not_mineral_ore）仍会被等到——按到达时间认领
            # 会把这个迟到响应误判成本批结果而误报（central-review 2029 #6）。
            result = bot.wait_for(
                lambda e: e.kind == "server_data"
                and e.data["payload_type"] == "mineral_probe_result"
                and e.t > sent_at
                and e.data["payload"].get("denial_reason") == reason,
                timeout=COLUMN_TIMEOUT,
                description=f"{label}: 竖直列 mineral_probe_result/{reason} (t>{sent_at:.3f})",
            )
        except BotAssertionError:
            # 整列都落在权威位置 6m 外（y 带已移动）：排空本批可能迟到的结果后
            # 重读位置换一列重试。
            _drain_probe_results(bot)
            continue
        payload = result.data["payload"]
        if payload.get("kind") != "denied" or payload.get("denial_reason") != reason:
            raise BotAssertionError(
                f"[{bot.username}] {label}：期望 denied/{reason}，实际 {payload}"
            )
        _drain_probe_results(bot)  # 本批其余候选的迟到响应排空，防落入下一批次窗口
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
        for e in bot.events_of("server_data"):
            if e.t > sent_at and e.data["payload_type"] == "mineral_probe_result":
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际收到 mineral_probe_result（t={e.t:.3f}）"
                )
        for e in bot.events_of("chat"):
            if e.t > sent_at:
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
                )
        if time.monotonic() >= deadline:
            return
        bot.assert_alive(f"{description} 窗口内连接保持")
        time.sleep(0.1)
