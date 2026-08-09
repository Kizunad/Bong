"""C2S 拒绝路径黑盒断言工具 —— `network_*_reject` 场景共用（AGENTS.md §15）。

「干净拒绝」的可观察契约（对齐 `client_request_handler.rs` 与 valence
`custom_payload` 的坏输入处理）：
1. 坏请求被 server 拒绝后连接**不被踢**、**不被单方面遗忘**（无 disconnect /
   connection_lost 事件）；
2. server 在拒绝之后**继续心跳**（新的 keepalive 到达）；
3. **拒绝发生在本请求产生任何玩法副作用之前** —— 探针窗口内没有响应式
   server_data / chat / vfx 反馈（server 要么根本没进 handler，要么在入口被拦）。
   窗口起点在 `drain_event_stream` 把 join 突发排干之后取锚，连接同步流量（join
   突发 / Changed 驱动的 status_snapshot）不算副作用。这是区分「拒绝」与
   「成功/部分处理」的黑盒证据；
4. 拒绝之后一个**合法**请求仍能产生它的预期响应 —— 证明连接不是"没崩但已坏"，
   而是完整可用。这是"连接状态定义良好"的最强黑盒证据。

下划线前缀：runner（`run_scenarios.py`）按 `pkgutil.iter_modules` 发现场景，
跳过下划线开头的文件，故本模块只做共享工具不被当作场景。
"""

from __future__ import annotations

import json
import time

from bot.bot import BotAssertionError  # noqa: F401  # 断言失败类型由场景抛出

# 连接同步类 server_data payload type 的显式兜底集合：这些类型由 Changed/周期驱动
# 的同步系统推送（无论 client 发什么都会出现），且首次出现可能晚于窗口起点之前、
# 落在探针窗口内（实跑观察到 zone_info 在 join 后 6-10s 才因 spirit_qi 波动触发）。
# 其余 server_data / chat / vfx_event 均视为响应式反馈。完整的连接同步集合由
# ``_ambient_types_in_window`` 从窗口起点前的已有事件里自校准得出。
_AMBIENT_SERVER_DATA_TYPES = frozenset(
    {
        "status_snapshot",  # Changed<StatusEffects> 驱动的 HUD 同步（status_snapshot_emit.rs）
        "zone_info",        # 区域内 spirit_qi 等状态波动时重发（连接同步，非请求响应）
        "player_state",     # 玩家灵气等状态变化时重发（连接同步，非请求响应）
    }
)


def is_gameplay_side_effect(event, ambient_types: frozenset = frozenset()) -> bool:
    """判断事件是否属于玩法副作用（拒绝路径必须保证探针窗口内不出现）。

    只把「响应式反馈」算副作用：chat / vfx_event 恒算；server_data 按 payload_type
    判定 —— 不在 ``ambient_types``（连接同步类型集合）里的才算。裸 ``payload`` 事件
    不算副作用：它要么是已解码 server_data 事件的字节重复（同一次推送同时发 raw
    payload + 解码后 server_data 两个事件），要么是 join 突发里解码器读不动的字节
    （如玩家 spawn），都不独立构成玩法反馈。
    """
    if event.kind in ("chat", "vfx_event"):
        return True
    if event.kind == "server_data":
        return event.data.get("payload_type") not in ambient_types
    return False


def _ambient_types_in_window(bot, since_t: float) -> frozenset:
    """探针窗口的连接同步类型集合：``t <= since_t`` 已出现过的 payload type，并上
    显式兜底集合。

    窗口起点之前的事件与探针无关（探针还没发出），其 payload type 属于连接同步 ——
    这是自校准基线：任何在探针发出前就出现过的类型，之后再次出现都不算探针副作用。
    显式兜底覆盖 Changed/周期驱动、可能首次出现晚于窗口起点、落在窗口内的类型。
    """
    observed = {
        event.data.get("payload_type")
        for event in bot.events
        if event.kind == "server_data" and event.t <= since_t
    }
    return frozenset(observed | _AMBIENT_SERVER_DATA_TYPES)


def assert_no_gameplay_side_effect_since(bot, since_t: float, label: str) -> None:
    """断言 t > since_t 的已有事件中没有玩法副作用（server_data / chat / vfx）。

    干净拒绝契约第 3 条：坏请求在产生任何玩法副作用**之前**被拦截。若探针窗口内
    出现了任意副作用事件，说明某个坏请求被成功/部分处理了，直接抛带修复线索的
    BotAssertionError。调用时机是探针全部发出并 settle 之后 —— 该窗口内的副作用
    此时必然已在 events 里，扫存量即可，不需要再等。连接同步流量由
    ``_ambient_types_in_window`` 排除（窗口起点前已出现的类型 + 显式兜底集合）。
    """
    ambient_types = _ambient_types_in_window(bot, since_t)
    offenders = [
        event
        for event in bot.events
        if event.t > since_t and is_gameplay_side_effect(event, ambient_types)
    ]
    if offenders:
        raise BotAssertionError(
            f"{label}：期望探针窗口内无玩法副作用（请求应在产生副作用前被拒绝），"
            f"实际观察到 {offenders[0]!r}"
        )


def assert_no_server_data_payload_since(
    bot, since_t: float, payload_type: str, label: str
) -> None:
    """断言 t > since_t 的已有事件中没有指定类型的 server_data payload。

    用于锁定「该请求没有产生它的成功响应」。例如 replay 一个已关闭 session 的
    external_container_move 后，必须**没有** loot_container_update（成功路径的
    响应），只有零 mutation 的 resync 快照。
    """
    offenders = [
        event
        for event in bot.events
        if event.kind == "server_data"
        and event.t > since_t
        and event.data.get("payload_type") == payload_type
    ]
    if offenders:
        raise BotAssertionError(
            f"{label}：期望没有 {payload_type}（成功路径响应不应出现），"
            f"实际观察到 {offenders[0]!r}"
        )


def inventory_fingerprint(snapshot: dict) -> str:
    """背包快照的可比较指纹：revision + 全部内容字段。

    server 仅在背包内容突变时 bump revision，单纯重发快照（如 resync）不改变它。
    故 fingerprint 相等 ⇒ 该请求没有造成任何背包 mutation（零 mutation 的可观测
    证据，绕开 InventorySnapshotV1 无 reason 字段、resync 只靠日志区分的限制）。
    """
    keys = ("revision", "containers", "placed_items", "equipped", "hotbar", "bone_coins")
    return json.dumps({key: snapshot.get(key) for key in keys}, sort_keys=True)


def wait_keepalive_after(bot, after: float, timeout: float = 25.0):
    """等 t > after 的新 keepalive（server 拒绝坏请求后仍主动维持连接）。

    拿不到就抛带修复线索的 BotAssertionError —— 那意味着 server 要么把这条连接
    遗忘（不再心跳）、要么已经断掉，两者都不是"干净拒绝"。
    """
    return bot.wait_for(
        lambda e: e.kind == "keepalive" and e.t > after,
        timeout=timeout,
        description="server 在拒绝坏请求后仍继续心跳（连接没被踢、也没被单方面遗忘）",
    )


def drain_event_stream(bot, *, quiet_s: float = 1.5, max_s: float = 6.0) -> None:
    """等事件流安静下来（连续 ``quiet_s`` 秒无新事件），最多 ``max_s`` 秒兜底。

    join 会一次性突发放出大量连接同步 payload（welcome / remains_sync /
    dropped_loot_sync / spawn / inventory_snapshot / status_snapshot 等），且部分
    滞后于 inventory_snapshot 到达。探针窗口必须在这波突发放完后再取锚，否则连接
    同步流量会被误判成探针的玩法副作用。若事件流一直有周期流量（如心跳）到不了
    安静，``max_s`` 兜底 —— 此时 sent_at 也已落在突发之后，窗口仍然正确。
    """
    start = time.monotonic()
    last_change_at = start
    last_len = len(bot.events)
    while time.monotonic() - last_change_at < quiet_s:
        if time.monotonic() - start >= max_s:
            break
        time.sleep(0.25)
        n = len(bot.events)
        if n != last_len:
            last_len = n
            last_change_at = time.monotonic()


def fire_probes_and_keep_connection(
    bot,
    label: str,
    probes: list[tuple[str, callable]],
    *,
    settle_s: float = 2.0,
) -> None:
    """连发一组坏请求探针，断言整体干净拒绝：无副作用 + 无断连 + 心跳继续。

    ``probes`` 是 ``(探针名, 发送函数)`` 列表 —— 发送函数执行一次坏请求（直接
    socket 写帧或 bot.send_payload / bot.intent）。先等 join 突发排干再取窗口锚，
    然后统一断言：
    - **探针窗口内无玩法副作用**（响应式 server_data / chat / vfx 均未出现 ——
      坏请求在产生任何玩法副作用之前被拦截，这是「拒绝」区别于「成功/部分处理」
      的证据；连接同步流量由 ``drain_event_stream`` 排干、由 ``is_gameplay_side_effect``
      排除，不误判）；
    - settle 窗口内 ``assert_alive``（"踢人/panic/断流"这类坏响应在此窗口显形）；
    - 探针之后的新 keepalive 到达（server 仍主动维护这条连接）。

    分模块的"合法请求仍可用"强断言由各场景在调用本函数后自己做（需要不同请求）。
    """
    drain_event_stream(bot)
    sent_at = bot.events[-1].t if bot.events else 0.0
    for probe_name, send in probes:
        send()
    time.sleep(settle_s)
    bot.assert_alive(f"{label} 探针发出后 {settle_s:.1f}s 窗口内无断连")
    assert_no_gameplay_side_effect_since(bot, sent_at, f"{label} 探针窗口")
    wait_keepalive_after(bot, sent_at)
    bot.assert_alive(f"{label} 心跳往返后仍存活")


def assert_valid_request_still_works(bot, *, meridian: str = "lung") -> None:
    """合法请求必须仍被正常处理 —— 连接在拒绝后处于完好可用状态。

    用 `set_meridian_target` 当探针：其预期响应是 server 广播「已收到经脉目标：」
    聊天确认，只有请求真正走完 handler 才会出现。先坏后好同一个连接，
    好请求成功 = 拒绝没有毒化连接（server 没崩、没卡死、没把连接标记为可疑）。

    **时序锚定**：先记录发送时刻 ``sent_at``，再要求响应 ``t > sent_at`` 到达。
    一个更早的匹配广播（例如来自之前被错误接受的坏请求探针）t ≤ sent_at，
    不能冒充本请求的响应 —— 成功断言与所发的合法请求严格对应。
    """
    sent_at = bot.events[-1].t if bot.events else 0.0
    bot.intent({"v": 1, "type": "set_meridian_target", "meridian": meridian})
    bot.wait_for(
        lambda e: e.kind == "chat" and "已收到经脉目标" in e.data["text"] and e.t > sent_at,
        timeout=10.0,
        description=f"t>{sent_at:.3f}s 后（合法请求发出后）的「已收到经脉目标」聊天确认",
    )
