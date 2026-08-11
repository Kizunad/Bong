"""渡虚劫心魔抉择 `heart_demon_decision` 全链路（四 bot，专用场景）。

黑盒断言面（全部走真实 wire 观察，不读 server 内部状态）：
- C2S `heart_demon_decision{v, choice_idx}`（`server/src/schema/client_request.rs`，
  choice_idx 为 Option<u32>，省略键即 None），dispatch 见 `client_request_handler.rs`
  → `HeartDemonChoiceSubmitted`。
- offer：`bong:server_data` oneof=69 `HeartDemonOffer`（`tribulation_heart_demon_offer_emit.rs`），
  心魔相开启时下发 3 个 choice（Composure/守本心、Breakthrough/斩执念、Perception/无解）。
- 决策语义（`tribulation.rs::heart_demon_outcome_for_choice`）：
  Some(0)=守本心（回 min(10%×有效上限, 空余) qi）、Some(2)=无解（无奖惩）、其余/None=心魔
  （损 30% 当前真元 + 下一道开天雷 ×1.2）。四 bot 各锁一分支：
  - bot A 无解 → 开天雷 90 → 存活登仙（wave_current=5）；
  - bot B 斩执念 → 30% 真元惩罚（500→350）→ 满资源检查不过 → failed 结算（wave_current=4）；
  - bot C 守本心 → 真元 360（<有效上限 400）进抉择回补 40（10%×有效上限）至 400=有效上限
    → 第 5 波满资源检查通过 → 登仙。登仙结算即授予铁证：360→400 只有 Steadfast 授予能完成，
    实现把 choice_idx=0 当无解/忽略/拒不回真元 → 真元停 360<400 → 检查失败 → failed 结算必被
    拦截（失败结算释放全部真元不可观测，故必须登仙留证）。不直接断 wire 真元——登仙后 zone
    灵气同化把真元拉向 local_zone×50 平衡点（实测 061108 登仙后稳在 246.86，不稳）；
  - bot D **省略 choice_idx**（活跃心魔相的缺失选择）→ Obsession 30% 惩罚（500→350）
    → failed 结算（out_of_phase 场景只证无 TribulationState 时的容忍，此处证活跃相的
    缺失选择分支；实现若把缺失 choice 当无解/忽略，真元不降，断言超时）。
  注：心魔路径惩罚后真元 ≤ 0.7×qi_max 恒小于 effective 0.8×qi_max，故 wire 结果是
  failed 结算而非 108 致死——×1.2 开天雷在满资源检查之后，实际不可达。
- 进度门槛：`du_xu_full_progress_ticks`（灵台突破/全经脉之后 36000 ticks=30 分钟）决定
  waves_total=5（含心魔相所在第 4 波），announce payload 的 wave_total 即此门槛的 wire 证据。

前置（dev 铺垫，与 cultivation_breakthrough.py 同风格）：
- realm set awaken → meridian open_all → qi max/set 500 → zone_qi set spawn 1.00，
  随后**逐级双连发** breakthrough_request 突破（醒灵→引气→凝液→固元→灵台）。
  机制（breakthrough.rs breakthrough_system）：roll 资源每 tick 以同一常量种子重建
  （XorshiftRoll(0x9e3779b97f4a7c15)，r1..r6=0.8598/0.3943/0.4806/0.1890/…），每 tick
  首条请求取 r1。r1 只过 醒灵→引气/引气→凝液（低阶成功率 ≥0.86），r2 过 凝液→固元/
  固元→灵台。**release 上网络读批会随机拆批**（实测 2/4 分片），故每级只发 2 条：
  同 tick 落地则 r1/r2 连过、拆成 1/1 则首条 r1 失败（耗 qi/跌 composure/积冻结）但次条
  r2 仍必过——任意拆批都只需"2 条落同一 tick"，且每级用 player_state.realm 帧确认
  （Changed<Cultivation> 广播，wire 解码境界名），失败则「qi max 清冻结 + qi set 回满 +
  meridian open_all 补脉 + 等 composure 回升」后重试，逐级独立收敛，不依赖单 tick 六连发
  的 ≥4-in-tick 批运（原 review finding #8）。环境前置（breakthrough_environment_error）：
  固元要求 zone 灵气 ≥ 0.8，须先 `zone_qi set spawn 1.00`。每级双连发后立即回满 qi，
  防 qi_zero_decay（qi≤1%×qi_max 持续 600 ticks 触发降境+闭脉）。
  **release 走火入魔死亡**：failure severity≥0.7（breakthrough.rs:1017）直发死亡触发，
  death_screen 最长 ~100s 后落下；每次死亡打回一阶 + 关闭经脉 + 写 MeridianClosed 生平。
  链抵灵台且有过失败重试时，`_final_rearm`（realm set spirit 直改境界不写生平卷 +
  open_all 补脉 + 回满 qi/health）前先等 backfire 死亡全部结算——见 _final_rearm docstring。
- 灵台达成后原地等待 30 分钟（keepalive 自动应答保持连接），再 start_du_xu：
  预兆 60s → 锁 30s → 第 1-3 波（每波 15s，伤害 18×波数、耗 35×波数）→ **心魔相占据
  第 4 波槽位**（wave 3 冷却结束即 begin_heart_demon_phase，cleared{wave:4}，
  tribulation_state 为 phase=heart_demon / wave_current=4，offer 30s 抉择窗）→
  决策后第 5 波开天雷（满资源检查 + 90/108 伤害 + 175 耗）→ 结算。
  注：**第 5 波不广播 phase="wave"**——begin_tribulation_wave(5) 与 wave_system 结算
  （满资源通过→登仙 / 失败→failure_system 先转 phase=settle）同帧完成，客户端只见
  phase="settle" 的 wave_current=5 载荷；波次证据靠结算 wave_current（登仙=5、失败=4）。
- 波间回血：每波 cleared 事件后 `health set 100` + `qi set 500`（下波 15s 后才落伤害）。
"""

from __future__ import annotations

import time

from bot.bot import Bot
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready

DESCRIPTION = (
    "四 bot 渡虚劫心魔相：NoSolution 无解登仙 + Breakthrough 斩执念 + Composure 守本心回补真元"
    "登仙 + 省略 choice_idx 的缺失选择（失败结算），断言 offer 形状与 wave_total=5"
    "（30 分钟满进度门槛）"
)
MODULES = ["cultivation", "tribulation", "network", "cmd", "multibot"]

DEFAULT_ENABLED = False  # 专用场景：30 分钟进度门槛，常规 --all 不执行（需显式 --scenario）

BREAKTHROUGH_REQUEST = {"type": "breakthrough_request", "v": 1}
START_DU_XU = {"type": "start_du_xu", "v": 1}
HEART_DEMON_DECISION = {"type": "heart_demon_decision", "v": 1}

# 逐级双连发：release 读批把 burst 拆到多个 tick 时，每个新 tick 的**首条**请求取 r1
# （XorshiftRoll(0x9e3779b97f4a7c15)，r1..r6=0.8598/0.3943/…，breakthrough.rs:700 每 tick 重建）。
# 成功率 = base×integrity×composure×completeness×…（breakthrough.rs:251）。completeness=
# 1.0+0.05×(全开−need) 钳 [0.8,1.3]（breakthrough.rs:530），**全脉开 →1.3**：
# - 引气→凝液：0.80×1.3×(composure≥0.9)=0.936 ≥ r1 → 首发必过；
# - 凝液→固元：req1 0.70×1.3×0.8=0.73<r1 失败、req2 0.70×1.3×0.5=0.455≥r2=0.3943 过；
# - 固元→灵台：需要 pair 起始 composure≥0.85（req2=0.55×1.3×(C−0.3)×integrity≥r2），
#   而链上前一步把 composure 扣到 ~0.4，故灵台步首发前先 _rearm_breakthrough 恢复满。
# 所以每级只发 2 条：同 tick 落地 r1/r2 连过，拆成 1/1 则首条（r1）失败消耗代价、次条（r2）
# 仍过——任意拆批都只需"2 条落同一 tick"，且每级独立收敛。相比单 tick 六连发一次冲顶
# （≥4 条须落同一 tick，run 实测 2/4 拆批必坏），本设计不依赖 socket 批的运气（finding #8）。
BREAKTHROUGH_PAIR = 2
BREAKTHROUGH_STEP_RETRIES = 10  # 每级双连发重试上限（拆成 1/1 连续 10 轮才放弃）
BREAKTHROUGH_STEP_CONFIRM_TIMEOUT = 12.0  # player_state.realm 前进确认窗（< 30s 的 qi_zero_decay 触发线）
# composure 恢复 0.001/tick（components.rs:672 玩家 0.001、NPC 0.01），本机共享重载 TPS 实测可
# 低至 ~3-6。一次失败双连发跌 0.6（req1 −0.3 + req2 −0.3），冷却须让 composure 从 0.0 回到
# ≥0.85（Spirit pair 起始门槛）才够：0.85/0.001=850 ticks，TPS 4 时需 212s。取 240s 覆盖
# TPS≥4 的满恢复；若仍失败，下一轮 rearm 再叠加恢复（单调收敛）。
BREAKTHROUGH_RETRY_COOLDOWN = 240.0
# release 构建走火入魔：breakthrough failure severity>=0.7（breakthrough.rs:1017）直发
# CultivationDeathTrigger，death_screen 最长在 burst 后 ~100s 内落下（run10 实测 32s/101s）。
# 每次死亡 apply_revive_penalty（death_hooks.rs:110）把境界打回一阶、关闭经脉并写
# MeridianClosed 生平条目。成功突破后必须等所有 backfire 死亡结算完再统一重装，否则
# 延迟死亡会在 gate wait 期间把境界打回，start_du_xu 以 realm!=Spirit 拒收。
BREAKTHROUGH_BACKFIRE_DRAIN_SECONDS = 300.0
# 渡虚劫满进度门槛：DUXU_FULL_PROGRESS_MIN_TICKS=30*60*20=36000，读的是 CombatClock（每 tick +1）。
# 实测本机并非稳定 20tps（约 19.1），不能按墙钟 1800s 硬等——靠 `/dev time now`
# 读 CultivationClock（与 CombatClock 同节奏每 tick +1）自校准，等真正走过的 ticks 达标。
# GATE_TARGET_TICKS 多留 200 ticks 余量，避免读数和 start_du_xu 之间落后几 tick 卡边界。
GATE_MIN_TICKS = 36000
GATE_TARGET_TICKS = GATE_MIN_TICKS + 200
# 兜底上限：本机是共享重载环境，服务器实测 TPS 可低至 ~3（systems overrun），
# 36000 ticks 需 ~3.3h；4h 上限只防死循环，不催结果（真实进度由 tick 差判定）。
GATE_WALL_TIMEOUT = 60 * 60 * 4
TICK_PROBE_INTERVAL = 20.0


def _trib_state(bot: Bot, predicate, after: float, timeout: float, description: str):
    """等待 t>after 的匹配 tribulation_state 解码 payload（防跨 bot 广播污染）。

    双 bot 时 tribulation_state 广播给所有在线客户端，另一 bot 的 omen/lock/wave/settle
    事件会进本 bot 列表；不按 t>after 过滤，wait_for 会误匹配历史广播（run11 实测
    offer 等待因误匹配历史 phase/wave 而空转超时）。
    """
    return bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "tribulation_state"
            and e.data.get("payload") is not None
            and e.t > after
            and predicate(e.data["payload"])
        ),
        timeout=timeout,
        description=description,
    )


def _expect_phase(bot: Bot, phase: str, *, after: float, timeout: float = 90.0) -> dict:
    ev = _trib_state(
        bot,
        lambda p: p.get("phase") == phase,
        after,
        timeout,
        f"tribulation_state phase={phase}",
    )
    return ev.data["payload"]


def _expect_wave_cleared(
    bot: Bot, wave: int, *, after: float, timeout: float = 90.0
) -> dict:
    ev = _trib_state(
        bot,
        lambda p: p.get("phase") == "wave" and p.get("wave_current") == wave,
        after,
        timeout,
        f"tribulation_state wave_current={wave}",
    )
    return ev.data["payload"]


def _trim_events(bot: Bot, keep_from: float) -> None:
    """丢弃 e.t <= keep_from 的旧事件，把 wait_for 扫描收敛到当前流程。

    双 bot 时双方事件列表都收满另一 bot 的广播与自身铺垫期事件（run11 达 81 万条）。
    wait_for 每次从索引 0 全表扫描且在 _new_event 条件变量上持锁，会把 reader 线程
    饿死（帧来不及读、socket 接收缓冲积压）——run11 实测 90s offer 超时扫描期间双
    连接同时掉线即此机制。流程锚点之后的事件才是本 bot 自己的，可安全清掉旧的。
    """
    with bot._new_event:
        bot.events[:] = [e for e in bot.events if e.t > keep_from]


def _realm_reaches(bot: Bot, after: float, target: str, within: float) -> bool:
    """在 within 秒内看到 t>after 的 player_state.realm == target（突破成功的 wire 证据）。

    player_state 在 Changed<Cultivation> 时广播（突破改境界即触发），取值是 wire 解码的
    境界名（PLAYER_STATE_REALM_NAMES：3=Condense/4=Solidify/5=Spirit）。读列表最新一条
    而不是断言中间帧，因为同 tick 双连发会连发 Induce+Condense 两次广播，只有最新帧是
    目标境界。t>after 过滤排除突破前的旧广播。
    """
    deadline = time.monotonic() + within
    while time.monotonic() < deadline:
        with bot._lock:
            realm = None
            for e in bot.events:
                if e.t <= after or e.kind != "server_data":
                    continue
                if e.data.get("payload_type") == "player_state":
                    realm = e.data.get("payload", {}).get("realm")
        if realm == target:
            return True
        time.sleep(0.2)
    return False


def _breakthrough_setup(bot: Bot) -> None:
    """从干净的 Awaken 起步：境界/经脉/真元/zone 灵气一次铺好。"""
    bot.cmd("realm set awaken")
    bot.expect_chat("[dev] realm set", timeout=10.0)
    bot.expect_chat("Awaken", timeout=10.0)

    bot.cmd("meridian open_all")
    bot.expect_chat("open_all does not auto-breakthrough", timeout=10.0)

    bot.cmd("qi max 500")
    bot.expect_chat("[dev] qi max", timeout=10.0)
    bot.cmd("qi set 500")
    bot.expect_chat("[dev] qi set", timeout=10.0)

    # 环境前置（breakthrough_environment_error）：固元（Condense→Solidify）要求
    # 所在 zone 灵气 ≥ 0.8；spawn 灵气默认不足且会随 zone drain 回落。
    bot.cmd("zone_qi set spawn 1.00")
    bot.expect_chat("[dev] zone_qi `spawn`", timeout=10.0)


def _rearm_breakthrough(bot: Bot) -> None:
    """失败/灵台步前恢复：补脉、清冻结、回满真元、zone 灵气重铺、等 composure 回到满。

    - meridian open_all 保持全开 → completeness=1.0+0.05×(open−need) 钳 [0.8,1.3] 到 1.3，
      是 Spirit 双连发能过的前提（0.55×1.3=0.715）；失败/backfire 死亡会关脉，重试前必须补回。
    - qi max 即清真元冻结（qi.rs 本次修复：设新上限即重置 qi_max_frozen）。
    - zone_qi set spawn 1.00 重铺：spawn zone 灵气会随 zone drain 回落，不重铺则凝液→固元
      二次尝试必然 EnvInsufficient（实测 ~5min 后已 <0.8）。
    - 冷却窗口让失败跌的 composure 回升（0.001/tick，TPS 感知 240s，见常量注释）。
    """
    bot.cmd("meridian open_all")
    bot.expect_chat("open_all does not auto-breakthrough", timeout=10.0)
    bot.cmd("qi max 500")
    bot.expect_chat("[dev] qi max", timeout=10.0)
    bot.cmd("qi set 500")
    bot.expect_chat("[dev] qi set", timeout=10.0)
    bot.cmd("zone_qi set spawn 1.00")
    bot.expect_chat("[dev] zone_qi `spawn`", timeout=10.0)
    time.sleep(BREAKTHROUGH_RETRY_COOLDOWN)


def _breakthrough_to_spirit(bot: Bot) -> None:
    """dev 铺垫 + 逐级双连发真实突破到灵台，每级用 player_state.realm 确认（见 docstring）。

    链：Awaken --(双连发)--> Condense（r1 过引气→凝液，r2 过凝液→固元）--> Solidify
    （r2 过固元→灵台）--> Spirit（r2 过灵台→通灵）。任意读批拆法下每级都只需 2-in-tick，
    逐级独立收敛；确认用 realm wire 帧而非 RequiresTribulation 播报。completeness=1.3
    （全脉开）把引气→凝液/凝液→固元的成功率顶到 r1/r2 之上，前两级首发即过；固元→灵台
    的 req2 只在 pair 起始 composure≥0.85 时过（0.55×1.3×(C−0.3)×integrity≥r2），而链上前
    一步成功把 composure 扣到 ~0.4，故灵台步首个 attempt 前先 _rearm_breakthrough 做完整
    恢复。失败的双连发会耗 qi、跌 composure、积冻结与经脉裂纹，重试前同样 rearm；**每级
    双连发后立即回满 qi**，防 qi_zero_decay（qi≤1%×qi_max 持续 600 ticks 触发降境+闭脉）。
    """
    _breakthrough_setup(bot)

    # (步骤序号, 目标 realm wire 名)：目标即本级双连发推进到的境界。
    break_used_retries = False
    for step, target in ((3, "Condense"), (4, "Solidify"), (5, "Spirit")):
        for attempt in range(BREAKTHROUGH_STEP_RETRIES):
            # 灵台步首个 attempt（或任何步的重试）都先完整恢复：Solidify 成功把 composure
            # 留在 ~0.4，直接低 composure 试 Spirit 只会双败 + 两次 severity≥0.7 的 backfire
            # 死亡；恢复满后再试则 req1 失败（severity 0.32，不触发死亡）+ req2 必过。
            if target == "Spirit" or attempt > 0:
                _rearm_breakthrough(bot)
            burst_at = last_event_time(bot)
            bot.intent(BREAKTHROUGH_REQUEST)
            bot.intent(BREAKTHROUGH_REQUEST)
            reached = _realm_reaches(bot, burst_at, target, BREAKTHROUGH_STEP_CONFIRM_TIMEOUT)
            # 双连发把 qi 打到低位（Solidify 级失败可到 0）：确认窗 12s < 30s 的
            # qi_zero_decay 触发线，仍立即回满，给任何后续等待留足安全余量。
            bot.cmd("qi set 500")
            bot.expect_chat("[dev] qi set", timeout=10.0)
            if reached:
                break
            break_used_retries = True
            # 下一次迭代顶部会再 _rearm_breakthrough（attempt>0）恢复后再试。
        if not reached:
            raise AssertionError(
                f"[{bot.username}] 突破到 {target} 失败：{BREAKTHROUGH_STEP_RETRIES} 次双连发"
                " 均未在确认窗内看到 player_state.realm 前进（读批连续拆成单条、r2 也未过？）"
            )

    # 链抵灵台后等未落地的走火入魔死亡全部结算（失败双连发的延迟触发），再统一重装。
    # 全程零失败时无 RolledFailure，不存在 backfire 死亡，可跳过等待。gate baseline 必须
    # 在 _final_rearm 之后读取：此刻的境界/经脉才是稳定基线（death 会写 MeridianClosed
    # 生平条目，把 full_meridians_tick 归零，只有重装后再读基线才能保证 wave_total=5 门槛）。
    if break_used_retries:
        time.sleep(BREAKTHROUGH_BACKFIRE_DRAIN_SECONDS)
    _final_rearm(bot)


def _expect_chat_new(
    bot: Bot, substring: str, *, after: float | None = None, timeout: float = 10.0
):
    """等一条 t 严格大于 `after` 的 chat（默认探测前最后事件；expect_chat 每次从头扫会命中旧事件）。

    调用方必须在 `bot.cmd` **之前**截取 `after` 传入：reader 异步，命令下发到本函数返回之间
    的快速回显若被当作锚点自身，会被 `e.t > after` 永久拒绝（run11 心魔相抉择同理）。直接
    调用本函数默认锚点取函数入口——这只对尚未下发的等待成立。
    """
    if after is None:
        after = last_event_time(bot)
    return bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and substring in e.data.get("text", "")
            and e.t > after
        ),
        timeout,
        f"新的包含「{substring}」的聊天消息",
    )


def _cmd_new(bot: Bot, command: str, substring: str, *, timeout: float = 15.0):
    """在命令下发**前**捕获锚点，再等严格新的回显（见 _expect_chat_new 的竞争说明）。"""
    after = last_event_time(bot)
    bot.cmd(command)
    return _expect_chat_new(bot, substring, after=after, timeout=timeout)


def _final_rearm(bot: Bot) -> None:
    """突破成功、backfire 死亡结算完后的统一重装（读 gate baseline 前必须完成）。

    release 走火入魔死亡会把境界打回一阶并关闭经脉；realm set spirit 直改境界、不写
    生平卷（cmd/dev/realm.rs 测试断言 biography 保持空），故 latest_spirit_breakthrough_tick
    仍是原突破 tick；meridian open_all 把死亡关闭的经脉补开（此刻写 MeridianOpened 生平
    条目，full_meridians_tick 落在重装当下）——gate 从重装后读取的 baseline 起算，恒满足。
    """
    realm_ev = _cmd_new(bot, "realm set spirit", "realm set")
    assert "-> Spirit" in realm_ev.data.get("text", ""), (
        f"[{bot.username}] 最终重装 realm set spirit 未生效（回显 {realm_ev.data.get('text')!r}）"
    )
    _cmd_new(bot, "meridian open_all", "open_all does not auto-breakthrough")
    _cmd_new(bot, "qi max 500", "[dev] qi max")
    _cmd_new(bot, "qi set 500", "[dev] qi set")
    _cmd_new(bot, "health set 100", "Queued /health set")


def _heal(bot: Bot) -> None:
    """回满气血/真元（波间 15s 窗口与开天雷前满资源检查用）。

    命令回显按 t>after 过滤，避免匹配同 bot 上一次 heal 的旧回显而提前返回。
    """
    after = last_event_time(bot)
    bot.cmd("health set 100")
    bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and "Queued /health set" in e.data.get("text", "")
            and e.t > after
        ),
        timeout=10.0,
        description="新的 Queued /health set",
    )
    bot.cmd("qi set 500")
    bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and "[dev] qi set" in e.data.get("text", "")
            and e.t > after
        ),
        timeout=10.0,
        description="新的 [dev] qi set",
    )
    # 渡劫期间对全部 bot（含旁观者）逐波 heal；旁观者事件列表无界增长，wait_for 每次
    # 从头扫描会 O(n) 饿死 reader（_trim_events docstring 的 run11 机制）。每次 heal 后
    # 丢弃锚点前旧事件，把旁观者的扫描窗口收敛到最近一次 heal。
    _trim_events(bot, after)


def _fleet_rearm(fleet: list[Bot]) -> None:
    """每个 bot 渡劫开始前，对**全部** bot 清冻结 + 回满真元/气血。

    四 bot 同处出生点（[8,150,8]），渡劫波击无差别命中 100 半径内所有目标：旁观
    bot 会被波击耗真元、wave-3 冻结 qi_max、开天雷打伤甚至"观劫而亡"（死亡打回一阶
    境界）。若不清理，前序 bot 的渡劫会把后序 bot 打崩（降境 → start_du_xu 拒收、
    effective_qi_max 缩水 → 守本心回补/满资源检查失真）。

    `qi max 500` 即清冻结（qi.rs 本次修复：设新上限重置 qi_max_frozen），只动真元
    账本、不写生平卷——不重写 BreakthroughSucceeded/MeridianOpened，故 wave_total=5
    的满进度门槛（max(spirit_tick, full_meridians_tick) 距今 ≥36000 ticks）不受影响。
    境界/经脉保持渡劫前状态（旁观不致死时恒为灵台/全开），无需 realm/meridian 干预。
    """
    for b in fleet:
        _cmd_new(b, "qi max 500", "[dev] qi max")
        _cmd_new(b, "qi set 500", "[dev] qi set")
        _cmd_new(b, "health set 100", "Queued /health set")


def _read_cult_tick(bot: Bot) -> int:
    """通过 `/dev time now` 读 CultivationClock（每 tick +1，与 CombatClock 同节奏）。

    `/dev time advance` 只改 CultivationClock/GameTick、不动 CombatClock，而渡虚劫门槛
    （DUXU_FULL_PROGRESS_MIN_TICKS=36000）读 CombatClock 的实数差，无法用 dev 快进；
    故这里只能真实等 ticks 走够，用 CultClock 自校准（Server 非 20tps，不能按墙钟硬等）。

    注意不能用 `expect_chat` 直接匹配：`wait_for` 每次从头扫历史事件，多次探测会反复
    命中第一条缓存的 `[dev] time now:` 响应（tick 值永远是第一次读数），导致门限差恒为 0
    ——必须用 `e.t > after` 过滤出严格新的响应（gap6 run7 已实测此坑：等满 88 分钟不返回）。
    """
    after = last_event_time(bot)
    bot.cmd("time now")
    ev = bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and "[dev] time now:" in e.data.get("text", "")
            and e.t > after
        ),
        timeout=10.0,
        description="新的 [dev] time now 响应（t 严格大于探测前最后事件）",
    )
    # 每轮探测后丢弃锚点前的旧事件：gate 等待可长达 4h（GATE_WALL_TIMEOUT），事件列表
    # 无界增长，而 wait_for 每次从索引 0 全表扫描且在 _new_event 条件变量上持锁——长轮询
    # 会让扫描变 O(n) 且饿死 reader 线程（_trim_events docstring 的 run11 机制）。trim 后
    # 列表收敛到本轮探测体量，扫描成本恒定，突破期事件在后续 phase 等待前也会被重建。
    _trim_events(bot, after)
    text = ev.data.get("text", "")
    return int(text.split(":", 1)[-1].strip())


def _idle_until_gate(bot: Bot, gate_tick_from: int) -> None:
    """原地挂机直到真实 ticks 走过渡虚劫满进度门槛（keepalive 自动应答保持连接）。

    每轮 `/dev time now` 既当 tick 探针也当存活证明（命令往返本身成保活）；达标后
    卡在 >=36000 但 <36000+ 的边界也没关系，读数为 poll+命令往返，天然多走若干 tick。
    """
    wait_started = time.monotonic()
    deadline = wait_started + GATE_WALL_TIMEOUT
    while time.monotonic() < deadline:
        now_tick = _read_cult_tick(bot)
        if now_tick - gate_tick_from >= GATE_TARGET_TICKS:
            return
        time.sleep(TICK_PROBE_INTERVAL)
    raise AssertionError(
        f"[{bot.username}] 满进度门槛等待超时（{GATE_WALL_TIMEOUT / 3600:.1f}h 墙钟内"
        f" ticks 未从 {gate_tick_from} 前进满 {GATE_TARGET_TICKS}，"
        f"平均 {(now_tick - gate_tick_from) / (time.monotonic() - wait_started):.2f} tps）"
    )


def _assert_offer_shape(offer: dict) -> None:
    assert offer.get("offer_id", "").startswith("heart_demon:"), (
        f"heart_demon_offer.offer_id 应为 heart_demon: 前缀，实际 {offer.get('offer_id')!r}"
    )
    assert offer.get("trigger_label") == "心魔劫临身", (
        f"trigger_label 应为 心魔劫临身，实际 {offer.get('trigger_label')!r}"
    )
    assert offer.get("realm_label") == "渡虚劫 · 心魔", (
        f"realm_label 应为 渡虚劫 · 心魔，实际 {offer.get('realm_label')!r}"
    )
    assert offer.get("expires_at_ms", 0) > time.time() * 1000 - 1000, (
        "expires_at_ms 应为未来时刻（30s 抉择窗）"
    )
    choices = offer.get("choices", [])
    assert len(choices) == 3, f"offer 应含 3 个 choice，实际 {len(choices)}"
    assert [(c.get("choice_id"), c.get("category")) for c in choices] == [
        ("heart_demon_choice_0", "Composure"),
        ("heart_demon_choice_1", "Breakthrough"),
        ("heart_demon_choice_2", "Perception"),
    ], f"choice_id/category 三元组不符，实际 {[(c.get('choice_id'), c.get('category')) for c in choices]}"
    assert [c.get("title") for c in choices] == ["守本心", "斩执念", "无解"], (
        f"choice 标题不符，实际 {[c.get('title') for c in choices]}"
    )


def _run_duxu_common(
    bot: Bot,
    choice_idx: int | None,
    fleet: list[Bot],
    *,
    pre_decision_qi: float | None = None,
) -> tuple[float, float]:
    """登仙/身死共同段：预兆→锁→1-3 波→心魔相(第 4 波槽)→抉择。

    choice_idx=None 时省略 wire 字段（serde Option<u32> 默认 None，等价于"错过抉择"，
    走 Obsession 惩罚——out_of_phase 场景只证了无 TribulationState 时的容忍，这里证
    活跃心魔相的缺失选择分支）。pre_decision_qi 非 None 时在抉择前把真元设到该值
    （守本心用例需要真元低于有效上限以观察回补）。

    波间对 `fleet` 全部 bot 补血回蓝：四 bot 同处出生点，波击会命中旁观 bot
    （"观劫而亡"死亡惩罚会把旁观者境界打回一阶），只回活跃 bot 的话后序 bot 会被
    前序渡劫打崩。每波 cleared 后 15s 窗口内 heal 全部（4 bot × 2 命令 ≈ 2-3s，够用）。

    返回 (入口锚点, 抉择锚点)。抉择锚点在 intent **之前**截取，供后续真元观测
    （t>锚点 的 player_state 才可能是抉择的副作用）。
    """
    after = last_event_time(bot)
    _trim_events(bot, after)
    _heal(bot)
    bot.intent(START_DU_XU)

    announce = _expect_phase(bot, "omen", after=after, timeout=120.0)
    assert announce.get("kind") == "du_xu", f"announce kind 应为 du_xu，实际 {announce.get('kind')!r}"
    assert announce.get("wave_total") == 5, (
        "wave_total 应为 5（灵台突破+全经脉后 36000 ticks 满进度门槛生效），"
        f"实际 {announce.get('wave_total')!r}"
    )

    _expect_phase(bot, "lock", after=after, timeout=120.0)

    # 5 波渡虚劫没有 phase="wave" 的第 4 波：wave 3 冷却结束即 begin_heart_demon_phase
    # （cleared{wave:4}），心魔相占据第 4 波槽位（phase=heart_demon、wave_current=4）。
    for wave in (1, 2, 3):
        _expect_wave_cleared(bot, wave, after=after, timeout=90.0)
        for b in fleet:
            _heal(b)

    offer_ev = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "heart_demon_offer"
            and e.t > after
        ),
        timeout=90.0,
        description="bong:server_data/heart_demon_offer payload（server 应推送该玩法数据）",
    )
    _assert_offer_shape(offer_ev.data["payload"])

    heart_demon = _expect_phase(bot, "heart_demon", after=after, timeout=60.0)
    assert heart_demon.get("wave_current") == 4, (
        "心魔相应占据第 4 波槽位（wave_current=4），"
        f"实际 {heart_demon.get('wave_current')!r}"
    )

    _heal(bot)
    if pre_decision_qi is not None:
        # 守本心抉择前把真元抬到目标值：grant 需要 zone 灵气（release_qi_from_zone），
        # zone 灵气默认不足/被 drain 回落，须先抬回（与 _breakthrough_setup 同因）。
        # 用 _cmd_new（t>after 过滤）等**严格新**回显：qi set 在效果落地之后才回显
        # （qi.rs Set 先改 qi_current 再发 echo），不带时间过滤的 expect_chat 会匹配
        # _heal 的旧 `qi set 500` 回显而提前返回——决策包抢在 qi=360 生效前到达（实测
        # 061108 失败：decision .085 早于 qi 500->360 .096），满资源检查看到 500 而非 360，
        # 登仙便不再是"授予"的证据。
        _cmd_new(bot, "zone_qi set spawn 1.00", "[dev] zone_qi `spawn`")
        _cmd_new(bot, f"qi set {pre_decision_qi:.0f}", "[dev] qi set")
    decision_after = last_event_time(bot)
    decision = {**HEART_DEMON_DECISION}
    if choice_idx is not None:
        decision["choice_idx"] = choice_idx
    bot.intent(decision)
    # 第 5 波（开天雷）不会广播 phase="wave" wave_current=5：begin_tribulation_wave(5)
    # 在 phase_tick_system 置 phase=Wave(5) 的同一帧里，wave_system 处理 cleared{wave:5}
    # 时 wave_current(5)>=waves_total → 满资源检查通过立即 settle(ascended)、失败则
    # failure_system 已把 phase 转成 settle —— 客户端看到的第 5 波载荷恒为 phase="settle"。
    # 登仙/失败结算由 run() 的 _expect_settle_result（含 wave_current 断言）负责。
    return after, decision_after


def _expect_settle_result(
    bot: Bot,
    allowed: tuple[str, ...],
    *,
    wave_current: int,
    after: float,
    timeout: float = 90.0,
) -> dict:
    ev = _trib_state(
        bot,
        lambda p: p.get("result") is not None,
        after,
        timeout,
        "tribulation_state 结算（result 非空）",
    )
    payload = ev.data["payload"]
    result = payload.get("result")
    assert result in allowed, f"结算 result 应为 {allowed} 之一，实际 {result!r}"
    assert payload.get("wave_current") == wave_current, (
        f"结算 wave_current 应为 {wave_current}（登仙=5 波全过 / 失败=第 5 波止步存活 4），"
        f"实际 {payload.get('wave_current')!r}"
    )
    return payload


def _spirit_qi_after(bot: Bot, after: float) -> float:
    """读 t>after 的最新 player_state.spirit_qi（wire 字段 3=qi_current，抉择副作用观测）。"""
    qi = None
    with bot._lock:
        for e in bot.events:
            if e.t <= after or e.kind != "server_data":
                continue
            if e.data.get("payload_type") == "player_state":
                qi = e.data.get("payload", {}).get("spirit_qi")
    return qi


def _expect_qi_drain(bot: Bot, after: float, full: float, timeout: float) -> None:
    """心魔（Obsession，缺失 choice_idx）：抉择后真元必须显著低于 full（30% 当前真元惩罚）。

    full=500 时扣 150 → 350 ≤ 500×0.75=375。若实现把缺失 choice 当无解/忽略，真元保持
    满值，断言超时。低于有效上限（0.8×500=400）也决定了第 5 波满资源检查必失败。
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        qi = _spirit_qi_after(bot, after)
        if qi is not None and qi <= full * 0.75:
            return
        time.sleep(0.2)
    raise AssertionError(
        f"[{bot.username}] 缺失 choice 抉择后真元未扣减：期望 ≤ {full}×0.75={full * 0.75:.0f}"
        f"（Obsession 30% 惩罚），实际最新 spirit_qi={_spirit_qi_after(bot, after)!r}"
    )


def run(env) -> None:
    with env.new_bot("Ascend") as ascender:
        wait_for_ready(ascender)
        _breakthrough_to_spirit(ascender)
        # 门槛按"灵台突破 / 全经脉之后再走 36000 ticks"计（CombatClock 真实流逝），
        # 用突破完成当下读到的 CultClock 作基线，之后真实等 ticks 走满。
        ascender_gate_base = _read_cult_tick(ascender)

        with env.new_bot("HdJue") as obsessed:
            wait_for_ready(obsessed)
            # 后续 bot 的突破链在 ascender 的等待窗口内完成，等待时段重叠。
            _breakthrough_to_spirit(obsessed)
            obsessed_gate_base = _read_cult_tick(obsessed)

            with env.new_bot("HdKeep") as steadfast:
                wait_for_ready(steadfast)
                _breakthrough_to_spirit(steadfast)
                steadfast_gate_base = _read_cult_tick(steadfast)

                with env.new_bot("HdSkip") as omit:
                    wait_for_ready(omit)
                    _breakthrough_to_spirit(omit)
                    omit_gate_base = _read_cult_tick(omit)

                    # 四个 gate baseline 都读到后再统一等：末位 bot 的 baseline 最晚，
                    # 其等待驱动整段空闲，前三个在 _idle_until_gate 各自收敛。
                    _idle_until_gate(ascender, ascender_gate_base)
                    _idle_until_gate(obsessed, obsessed_gate_base)
                    _idle_until_gate(steadfast, steadfast_gate_base)
                    _idle_until_gate(omit, omit_gate_base)

                    # 每个 bot 渡劫前给全部 bot 清冻结+回满（四 bot 同处出生点，前序渡劫的
                    # 波击会冻结/打伤旁观者；见 _fleet_rearm docstring）。
                    fleet = [ascender, obsessed, steadfast, omit]

                    # bot A：无解（Perception）→ 开天雷 90 → 存活登仙。
                    _fleet_rearm(fleet)
                    ascender_anchor, _ = _run_duxu_common(ascender, choice_idx=2, fleet=fleet)
                    _expect_settle_result(
                        ascender, ("ascended",), wave_current=5, after=ascender_anchor
                    )
                    ascender.assert_alive("无解登仙结算后连接应保持")

                    # bot B：斩执念（Breakthrough）→ 心魔 → 30% 真元惩罚 →
                    # 第 5 波开天雷满资源检查必失败 → failed 结算（不死，止步第 5 波、存活 4 波）。
                    _fleet_rearm(fleet)
                    obsessed_anchor, _ = _run_duxu_common(obsessed, choice_idx=1, fleet=fleet)
                    _expect_settle_result(
                        obsessed, ("failed",), wave_current=4, after=obsessed_anchor
                    )
                    obsessed.assert_alive("斩执念失败结算后玩家应存活（满资源检查失败不致死）")

                    # bot C：守本心（Composure, choice_idx=0）→ 真元 360（<有效上限 400）进抉择，
                    # 授予 10%×有效上限 = min(40, 400−360) = 40 → 400=有效上限 → 第 5 波满资源
                    # 检查通过 → 登仙。有效上限 400 = qi_max 500 − 第 3 波冻结 20%（wave 3
                    # freeze ratio 0.20）。登仙结算即授予的铁证：第 5 波检查要求真元 ≥ 有效上限，
                    # 而 360→400 只有 Steadfast 授予能完成——实现若把 choice_idx=0 当无解/忽略/
                    # 拒不回真元，真元停 360<400 → 检查失败 → failed 结算，断言必然拦截。不再直接
                    # 断 wire 真元：登仙后 zone 灵气同化会把真元拉向 local_zone×50 的平衡点
                    # （实测 061108 登仙后稳在 246.86），wire 回补断言不可靠。
                    _fleet_rearm(fleet)
                    steadfast_anchor, _ = _run_duxu_common(
                        steadfast, choice_idx=0, fleet=fleet, pre_decision_qi=360.0
                    )
                    _expect_settle_result(
                        steadfast, ("ascended",), wave_current=5, after=steadfast_anchor
                    )
                    steadfast.assert_alive("守本心登仙结算后玩家应存活")

                    # bot D：省略 choice_idx（活跃心魔相的缺失选择）→ Obsession 30% 真元惩罚
                    # → 低于有效上限 → 第 5 波满资源检查失败 → failed 结算。
                    _fleet_rearm(fleet)
                    omit_anchor, omit_decision_after = _run_duxu_common(omit, choice_idx=None, fleet=fleet)
                    _expect_qi_drain(omit, omit_decision_after, full=500.0, timeout=30.0)
                    _expect_settle_result(
                        omit, ("failed",), wave_current=4, after=omit_anchor
                    )
                    omit.assert_alive("省略 choice 失败结算后玩家应存活")
