"""渡虚劫心魔抉择 `heart_demon_decision` 全链路（双 bot，专用场景）。

黑盒断言面（全部走真实 wire 观察，不读 server 内部状态）：
- C2S `heart_demon_decision{v, choice_idx}`（`server/src/schema/client_request.rs`），
  dispatch 见 `client_request_handler.rs` → `HeartDemonChoiceSubmitted`。
- offer：`bong:server_data` oneof=69 `HeartDemonOffer`（`tribulation_heart_demon_offer_emit.rs`），
  心魔相开启时下发 3 个 choice（Composure/守本心、Breakthrough/斩执念、Perception/无解）。
- 决策语义（`tribulation.rs::heart_demon_outcome_for_choice`）：
  Some(0)=守本心（回少量 qi）、Some(2)=无解（无奖惩）、其余/None=心魔（损 30% 当前真元 +
  下一道开天雷 ×1.2）。断言锁「无解 → 开天雷 90 → 存活登仙」与「斩执念 → 30% 真元惩罚 →
  第 5 波开天雷满资源检查（effective qi_max = qi_max − 20% 冻锁）必然不过 → 渡劫失败结算」。
  注：惩罚后真元 ≤ 0.7×qi_max 恒小于 effective 0.8×qi_max，故心魔路径的 wire 结果是
  failed 结算而非 108 致死——×1.2 开天雷在满资源检查之后，实际不可达。
- 进度门槛：`du_xu_full_progress_ticks`（灵台突破/全经脉之后 36000 ticks=30 分钟）决定
  waves_total=5（含心魔相所在第 4 波），announce payload 的 wave_total 即此门槛的 wire 证据。

前置（dev 铺垫，与 cultivation_breakthrough.py 同风格）：
- realm set awaken → meridian open_all → qi max/set 500，
  随后**单 tick 六连发** breakthrough_request 逐级突破（醒灵→引气→凝液→固元→灵台）。
  机制（breakthrough.rs breakthrough_system）：roll 资源每 tick 以同一常量种子重建
  （XorshiftRoll(0x9e3779b97f4a7c15)，r1..r6=0.8598/0.3943/0.4806/0.1890/…）；4 步链只有
  ≥4 条请求落在同一 tick、且首条从 Awaken 起步时才全成（r1 只过 醒灵→引气/引气→凝液，
  r2/r4 过 凝液→固元/固元→灵台）。**release 上网络读批会随机拆批**（实测 2/4 分片），
  拆坏时该批首条取 r1 失败并耗 qi、跌 composure——单发必败、重试更塌，故本场景用
  「回 qi + 等 composure 回升（0.001/tick≈50s 回满）」的重试循环收敛，直至 SystemWarning
  播报「通灵至化虚必须先走渡虚劫」证明链抵灵台。环境前置（breakthrough_environment_error）：
  固元要求 zone 灵气 ≥ 0.8，须先 `zone_qi set spawn 1.00`。成功信号：RequiresTribulation
  播报（Only 在 Spirit 请求通灵→化虚时触发，是链抵灵台的 wire 证据）+ pillar 事件。
  **release 走火入魔死亡**：failure severity≥0.7（breakthrough.rs:1017）直发死亡触发，
  death_screen 最长 ~100s 后落下；每次死亡打回一阶 + 关闭经脉 + 写 MeridianClosed 生平。
  成功突破后 `_final_rearm`（realm set spirit 直改境界不写生平卷 + open_all 补脉 + 回满
  qi/health）把状态钉回稳定基线，gate baseline 在该函数之后读取——见 _final_rearm docstring。
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
    "双 bot 渡虚劫心魔相：NoSolution 无解登仙 + Breakthrough 斩执念触发满资源检查失败结算，"
    "断言 heart_demon_offer 形状与 wave_total=5（30 分钟满进度门槛）"
)
MODULES = ["cultivation", "tribulation", "network", "cmd", "multibot"]

DEFAULT_ENABLED = False  # 专用场景：30 分钟进度门槛，常规 --all 不执行（需显式 --scenario）

BREAKTHROUGH_REQUEST = {"type": "breakthrough_request", "v": 1}
START_DU_XU = {"type": "start_du_xu", "v": 1}
HEART_DEMON_DECISION = {"type": "heart_demon_decision", "v": 1}

BREAKTHROUGH_BURST = 6  # 单 tick 连发：突破共享 roll 序列 r1..r6，4 次突破 + 2 条 Spirit 冗余
BREAKTHROUGH_PILLAR_BASE_COUNT = 12  # gameplay_vfx::BREAKTHROUGH_PILLAR 基础粒子数（coalesce 后 count 相加）
BREAKTHROUGH_RETRIES = 6  # release 读批会随机拆批，拆坏的首条取 r1 失败——重试上限
BREAKTHROUGH_RETRY_COOLDOWN = 60.0  # composure 恢复 0.001/tick≈0.02/s，60s 从塌陷回满
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


def _count_burst_signals(bot: Bot, after: float) -> tuple[int, int, bool]:
    """burst 后 5s 统计：coalesced pillar 粒子总量、fail 事件数、RequiresTribulation 播报。

    同 tick 同 origin 的 pillar 会被 vfx_event_emit coalesce 成单事件并把 count 相加
    （4 级连成 = 1 个 count=48 的事件）；fail 每条失败一个事件；链达灵台后第 5-6 条
    请求触发 RequiresTribulation 的 SystemWarning 播报（"通灵至化虚必须先走渡虚劫"）。
    """
    time.sleep(5.0)
    pillar_particles = 0
    fails = 0
    saw_spirit_gate = False
    with bot._lock:
        for e in bot.events:
            if e.t <= after:
                continue
            if e.kind == "vfx_event":
                event_id = e.data.get("event_id")
                if event_id == "bong:breakthrough_pillar":
                    pillar_particles += int(e.data.get("count", 0))
                elif event_id == "bong:breakthrough_fail":
                    fails += 1
            elif e.kind == "server_data" and e.data.get("payload_type") == "narration":
                for n in (e.data.get("payload") or {}).get("narrations", []):
                    if "通灵至化虚必须先走渡虚劫" in n.get("text", ""):
                        saw_spirit_gate = True
    return pillar_particles, fails, saw_spirit_gate


def _breakthrough_to_spirit(bot: Bot) -> None:
    """dev 铺垫 + 六连发重试循环真实突破到灵台（见模块 docstring 的 roll 机制）。

    release 读批随机拆批（拆坏时该批首条取 r1 失败并耗 qi），故以 RequiresTribulation
    播报（仅在 Spirit 请求通灵→化虚时触发）为链抵灵台的判定，未抵则等 composure 回升后
    重试。每轮全量 re-arm（realm/经脉/qi/zone 灵气）保证从干净的 Awaken 起步；burst 耗
    qi 后立即回满，防 qi_zero_decay（qi≤1%×qi_max 持续 600 ticks 触发降境 + 闭脉）在
    统计/冷却窗口内把境界打回。
    """
    for attempt in range(1, BREAKTHROUGH_RETRIES + 1):
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
        # 所在 zone 灵气 ≥ 0.8；spawn 灵气默认不足且会随 zone drain 回落，须每轮抬回。
        bot.cmd("zone_qi set spawn 1.00")
        bot.expect_chat("[dev] zone_qi `spawn`", timeout=10.0)

        burst_at = last_event_time(bot)
        for _ in range(BREAKTHROUGH_BURST):
            bot.intent(BREAKTHROUGH_REQUEST)

        pillar_particles, fails, saw_spirit_gate = _count_burst_signals(bot, burst_at)

        # burst 耗 qi 到低位：统计窗（5s）内远未到 600 ticks，但冷却窗口会超，须在此回满。
        bot.cmd("qi set 500")
        bot.expect_chat("[dev] qi set", timeout=10.0)

        if saw_spirit_gate:
            if pillar_particles < BREAKTHROUGH_PILLAR_BASE_COUNT:
                raise AssertionError(
                    f"[{bot.username}] RequiresTribulation 已触发但 pillar 异常"
                    f"（期望 ≥ {BREAKTHROUGH_PILLAR_BASE_COUNT} 粒子，实际 {pillar_particles}）"
                )
            # 等未落地的走火入魔死亡全部结算（含本 burst 自身 + 先前失败 burst 的延迟触发），
            # 再统一重装。首次 burst 零失败（全链一次成）时无任何 RolledFailure，不存在
            # backfire 死亡，可跳过等待。gate baseline 必须在 _final_rearm 之后读取：此刻的
            # 境界/经脉才是稳定基线（death 会写 MeridianClosed 生平条目，把 full_meridians_tick
            # 归零，只有重装后再读基线才能保证 wave_total=5 门槛成立）。
            if fails > 0 or attempt > 1:
                time.sleep(BREAKTHROUGH_BACKFIRE_DRAIN_SECONDS)
            _final_rearm(bot)
            return
        if attempt < BREAKTHROUGH_RETRIES:
            time.sleep(BREAKTHROUGH_RETRY_COOLDOWN)

    raise AssertionError(
        f"[{bot.username}] 突破到灵台失败：{BREAKTHROUGH_RETRIES} 次六连发均未收到"
        " RequiresTribulation 播报（通灵至化虚必须先走渡虚劫）——读批连续拆坏或 roll/rate 异常"
    )


def _expect_chat_new(bot: Bot, substring: str, *, timeout: float = 10.0):
    """等一条 t 严格大于探测前最后事件的 chat（expect_chat 每次从头扫，会命中缓存旧事件）。

    与 _read_cult_tick 的 `e.t > after` 过滤同源；用于最终重装等必须验证"这次命令真生效"
    的场景，避免旧回显误判。
    """
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


def _final_rearm(bot: Bot) -> None:
    """突破成功、backfire 死亡结算完后的统一重装（读 gate baseline 前必须完成）。

    release 走火入魔死亡会把境界打回一阶并关闭经脉；realm set spirit 直改境界、不写
    生平卷（cmd/dev/realm.rs 测试断言 biography 保持空），故 latest_spirit_breakthrough_tick
    仍是原突破 tick；meridian open_all 把死亡关闭的经脉补开（此刻写 MeridianOpened 生平
    条目，full_meridians_tick 落在重装当下）——gate 从重装后读取的 baseline 起算，恒满足。
    """
    bot.cmd("realm set spirit")
    realm_ev = _expect_chat_new(bot, "realm set", timeout=15.0)
    assert "-> Spirit" in realm_ev.data.get("text", ""), (
        f"[{bot.username}] 最终重装 realm set spirit 未生效（回显 {realm_ev.data.get('text')!r}）"
    )
    bot.cmd("meridian open_all")
    _expect_chat_new(bot, "open_all does not auto-breakthrough", timeout=15.0)
    bot.cmd("qi max 500")
    _expect_chat_new(bot, "[dev] qi max", timeout=15.0)
    bot.cmd("qi set 500")
    _expect_chat_new(bot, "[dev] qi set", timeout=15.0)
    bot.cmd("health set 100")
    _expect_chat_new(bot, "Queued /health set", timeout=15.0)


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


def _run_duxu_common(bot: Bot, choice_idx: int | None) -> float:
    """登仙/身死共同段：预兆→锁→1-3 波→心魔相(第 4 波槽)→抉择。返回流程锚点。

    入口即截取流程锚点并裁剪旧事件（见 _trim_events），之后所有 phase/wave/offer
    等待都按 t>锚点 过滤，只认本 bot 自己的 du_xu 广播。
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
        _heal(bot)

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
    bot.intent({**HEART_DEMON_DECISION, "choice_idx": choice_idx})
    # 第 5 波（开天雷）不会广播 phase="wave" wave_current=5：begin_tribulation_wave(5)
    # 在 phase_tick_system 置 phase=Wave(5) 的同一帧里，wave_system 处理 cleared{wave:5}
    # 时 wave_current(5)>=waves_total → 满资源检查通过立即 settle(ascended)、失败则
    # failure_system 已把 phase 转成 settle —— 客户端看到的第 5 波载荷恒为 phase="settle"。
    # 登仙/失败结算由 run() 的 _expect_settle_result（含 wave_current 断言）负责。
    return after


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


def run(env) -> None:
    with env.new_bot("Ascend") as ascender:
        wait_for_ready(ascender)
        _breakthrough_to_spirit(ascender)
        # 门槛按"灵台突破 / 全经脉之后再走 36000 ticks"计（CombatClock 真实流逝），
        # 用突破完成当下读到的 CultClock 作基线，之后真实等 ticks 走满。
        ascender_gate_base = _read_cult_tick(ascender)

        with env.new_bot("HdJue") as obsessed:
            wait_for_ready(obsessed)
            # 第二 bot 的突破链在 ascender 的等待窗口内完成，等待时段重叠。
            _breakthrough_to_spirit(obsessed)
            obsessed_gate_base = _read_cult_tick(obsessed)

            _idle_until_gate(ascender, ascender_gate_base)

            # bot A：无解（Perception）→ 开天雷 90 → 存活登仙。
            ascender_anchor = _run_duxu_common(ascender, choice_idx=2)
            _expect_settle_result(
                ascender, ("ascended",), wave_current=5, after=ascender_anchor
            )
            ascender.assert_alive("无解登仙结算后连接应保持")

            # bot B：斩执念（Breakthrough）→ 心魔 → 30% 真元惩罚 →
            # 第 5 波开天雷满资源检查必失败 → failed 结算（不死，止步第 5 波、存活 4 波）。
            _idle_until_gate(obsessed, obsessed_gate_base)
            obsessed_anchor = _run_duxu_common(obsessed, choice_idx=1)
            _expect_settle_result(
                obsessed, ("failed",), wave_current=4, after=obsessed_anchor
            )
            obsessed.assert_alive("斩执念失败结算后玩家应存活（满资源检查失败不致死）")
