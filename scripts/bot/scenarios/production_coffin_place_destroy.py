"""延寿棺 CoffinPlace 全生命周期黑盒：放置消费 → 重复/非空/过远/异维拒绝 → 破坏清场后可重放。

plan-coffin-v1 放置链路（client_request_handler.rs → handle_coffin_place_requests）黑盒契约面：

- `coffin_place{v,x,y,z,item_instance_id}` 必须用真实 instance_id（inventory_item_by_instance
  按 id 校验）。放置成功消费**恰好一个实例**（consume_item_instance_once）；失败路径分文不动。
- 重复放置（already registered）在 registry 双键（lower/upper）碰撞处拒绝，**先于消费**——
  第二实例必须保留在背包（防双注册双扣料 / double-grant，本家族最重的重复/双发检查）。
  双键碰撞的两条分支都被演练：新 lower==既有 lower 的精确重复、以及新 lower==既有
  upper（=lower+1x）的 overlap 探针——只查 lower 键、漏查 upper 键的错误实现（接受
  overlap 放置并吞掉实例）在第二探针处红（review finding [major]）。
- 非空气目标（not empty）、距离 > 6.0（coffin_target_is_close 36.0 平方）均**静默拒绝**
  （warn 无回执）——只能靠实例计数不变断言。
- 异维（coffin_requires_overworld fail-closed，先于近距/实例校验）拒绝并逐字 chat：
  `§c[棺] 你不在主世界，无法操作延寿棺。`——对**完整文本**（含 §c 色码、[棺] 前缀、
  句末句号）整体相等断言，不接受任意前缀/后缀（review finding [major]：子串匹配
  放行「错误的前缀：你不在主世界…（错误后缀）」式的错误回复）。
- 拒绝路径（重复/非空气/过远/异维/无效实例 id）**不得 spawn 或注册任何 coffin marker**——
  有界窗口内**任何**新建 kind=160 实体即红，**不按位置过滤**——拒绝请求却把 marker
  错放到偏移/默认坐标的错误实现同样红（review finding [major]：旧实现按目标位置过滤，
  「目标附近无 spawn」≠「拒绝即无 spawn」；位置匹配只用于正向放置断言）。
  负向断言只见证「无 client-visible spawn」：先插入 registry 双键再早退（不消费、不
  spawn）的错误实现能全过——故 stale 拒绝段后必须对**被拒坐标 stale_pos** 做
  state-applying 复查（合法实例成功放置 + marker spawn）：拒绝时错误登记双键会使复查
  以 already-registered 拒绝而超时红，暴露 registry-only 残留（review finding [major]：
  场景此前只在原 place_pos 做 destroy/re-place 复查，被拒坐标从未被重新放置/破坏）。
- `coffin_place` 必须用真实 instance_id（inventory_item_by_instance 按 id 校验）；
  已消费的 stale id 同样拒绝、分文不动（review finding [major]：全场景只用合法 id，
  无视 item_instance_id 吞掉可用棺材的实现能全绿）。
- `coffin_break` 破坏单发（remove_by_pos → None 后静默 continue），破坏后 registry 清空——
  同一坐标再放置必须成功（"destroy path leaves nothing behind"）；且**空→空重复破坏**
  必须是无副作用 no-op：不 panic / 不断连 / 不再 despawn / 不扣料 / 不加料
  （review finding [major]：只演练了 populated→empty 一次，empty→empty 从未被断言）。
  首次 break 的全量库存基线必须锚在 break **之前**（review finding [major]：旧基线取在
  break 完成之后，despawn 时 grant 非棺回收料的错误实现把料并入基线、重复 break 的
  total_baseline+1 断言照过）——break 后 +1 give 快照的全量必须恰好 = break 前全量 + 1。

**世界侧观察（本场景对实体生命周期的直接断言，非代理）**：
- 放置成功必须在世界 spawn mundane_coffin marker 渲染实体（entity kind=160，
  `coffin_marker_position(lower)=(x+1, y, z+0.5)`）——「世界确实获得棺实体」；
  只消费实例而无实体 spawn 在此红。
- `coffin_break` 必须把该 marker 实体 despawn（`entities_destroy` 含首放 entity_id）——
  marker 无方块无 block update，entity 层移除（valence `Despawned` → S2C_ENTITIES_DESTROY）
  是唯一清场信号；只移 registry 条目而留 stale 实体在此红。
- 破坏后同一坐标重放必须再次 spawn 新 marker 实体——旧实体已 despawn + registry 清空的
  完整闭环。

所有断言走实例计数（stack_count 求和，跨容器/装备/快捷栏），不是 presence——count 才是
复制增殖的暴露面。拒绝路径服务端静默（无回执快照），用「give 一棺 + 等其
inventory_snapshot 计数达到期望值」作 server-authoritative 水位线：give 快照落在同 tick
的 client_request 处理之后，收到它既证明此前每个 C2S（含静默拒绝）已处理完毕，又以该
计数断言「拒绝未扣料」（tutorial_coffin_open/gap12 同款约定）。

超时口径：45s——CI e2e 串行连跑长 server 后 TPS 退化（对齐 forge 场景先例），断言不放宽。
"""

from __future__ import annotations

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import latest_inventory_snapshot

DESCRIPTION = "CoffinPlace 生命周期：放置消费→重复/非空/过远/异维/stale实例拒绝（计数不变+无实体spawn）→单发破坏+空→空重破no-op→清场重放"
MODULES = ["coffin", "inventory", "dimension"]

COFFIN_ID = "mundane_coffin"
PLACE_OFFSET = -2  # 横向偏移，与 forge 放砧一致
# 放置目标必须在 y+1（fixture 全平面 surface_y=72，y=72 是实心地面、y=73+ 才是空气）：
# bot.position 的 y 是玩家脚下方块的顶（≈72.x，int 后=72），直接用它当 target 会命中
# 实心地表 → not empty 拒。所有「应为空气」的 target 统一用 py+1。
DISTANCE_REJECT_Z_OFFSET = 10  # > 6.0 的过远目标（coffin_target_is_close 36.0 平方）
# 完整、逐字的异维拒绝文本（服务端 COFFIN_DIMENSION_REJECTION_MESSAGE，coffin/mod.rs:1313）。
# 断言用整体相等（e.data["text"] == DIMENSION_REJECT_TEXT），含 §c 色码、[棺] 前缀与句号——
# 子串匹配会让「任意前缀/后缀包裹同一核心句」的错误回复漏网（review finding [major]）。
DIMENSION_REJECT_TEXT = "§c[棺] 你不在主世界，无法操作延寿棺。"

_STEP_TIMEOUT = 45.0
# 拒绝路径世界侧断言的有界观察窗（秒，事件时间）：inventory_snapshot 与实体
# spawn/destroy 走不同时序路径（reader 线程后台 append），「先发快照、后排队实体
# 事件」的实现会让晚到事件在持锁扫完当前前缀后漏检（review finding [major]）。
# 等待必须**有界**——负向断言不无限等，deadline = after_t + window 是确定性终点。
# 异维段必须**等满整窗仍在异维**（返回主世界后既有 marker 会重流，故返回锚点取在
# 观察结束之后）——以 sleep 提前闭合观察窗会让晚发（>0.5s）的错误 marker 漏检
# （review finding [major]）。
_REJECT_OBS_WINDOW = 2.0


def _stack_count(item) -> int:
    return int(round(float(item.get("stack_count", 1))))


def _coffin_count(snapshot) -> int:
    """统计 snapshot 中 mundane_coffin 的总持有数（跨容器/装备/快捷栏求和）。"""
    total = 0
    for placed in snapshot.get("placed_items", []):
        item = placed["item"]
        if item["item_id"] == COFFIN_ID:
            total += _stack_count(item)
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item and item["item_id"] == COFFIN_ID:
                total += _stack_count(item)
    for item in snapshot.get("hotbar", []):
        if item and item["item_id"] == COFFIN_ID:
            total += _stack_count(item)
    return total


def _total_items(snapshot) -> int:
    """全量持有数（跨容器/装备/快捷栏求和）——空→空重复 break 的「不得加料/扣料」判据。

    _coffin_count 只看棺材；重复破坏若错误地再 grant 回收材料，棺材数不变而全量数
    超基线，此函数暴露它（review finding [major]：无副作用 no-op 断言）。
    """
    total = 0
    for placed in snapshot.get("placed_items", []):
        total += _stack_count(placed["item"])
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item:
                total += _stack_count(item)
    for item in snapshot.get("hotbar", []):
        if item:
            total += _stack_count(item)
    return total


def _first_coffin_instance_id(snapshot) -> int:
    for placed in snapshot.get("placed_items", []):
        item = placed["item"]
        if item["item_id"] == COFFIN_ID:
            return int(item["instance_id"])
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item and item["item_id"] == COFFIN_ID:
                return int(item["instance_id"])
    for item in snapshot.get("hotbar", []):
        if item and item["item_id"] == COFFIN_ID:
            return int(item["instance_id"])
    raise BotAssertionError(f"期望 inventory_snapshot 中有 {COFFIN_ID} 实例可取，实际没有")


def _wait_coffin_count(bot, expected, after_t, timeout=_STEP_TIMEOUT, description=None):
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > after_t
        and _coffin_count(e.data["payload"]) == expected,
        timeout=timeout,
        description=description or f"mundane_coffin count=={expected} 的 inventory_snapshot",
    )
    return event.data["payload"]


def _coffin_locations(snapshot) -> str:
    """诊断用：列出 coffin 实例分布（location → 数量），暴露计数器漏扫的载体。"""
    parts = []
    for placed in snapshot.get("placed_items", []):
        item = placed["item"]
        if item["item_id"] == COFFIN_ID:
            parts.append(
                f"container:{placed.get('container_id', '?')}/r{placed.get('row')}c{placed.get('col')}"
                f"#{item.get('stack_count', 1)}"
            )
    for slot, values in snapshot.get("equipped", {}).items():
        items = values if isinstance(values, list) else [values]
        for item in items:
            if item and item["item_id"] == COFFIN_ID:
                parts.append(f"equip:{slot}#{item.get('stack_count', 1)}")
    for i, item in enumerate(snapshot.get("hotbar", [])):
        if item and item["item_id"] == COFFIN_ID:
            parts.append(f"hotbar:{i}#{item.get('stack_count', 1)}")
    return ", ".join(parts) or "∅"


def _give_barrier(bot, expected_count, timeout=_STEP_TIMEOUT, description=None):
    """give 一棺并以该 give 的 inventory_snapshot 作 server 权威水位线 + 断言载体。

    不用 give 的 chat ack 当水位线：实测（run4 证据）ack 在 client_request 处理**之前**
    发出（chat 命令 pass 先于 client_request pass），且在紧邻 give→give 交错下 ack 会丢
    （revision=14/16 收到、barrier 的 r17 未到）；而 give 的 inventory_snapshot 落在同 tick
    的 client_request 处理**之后**——收到 count==expected 的新快照，既证明此前每个 C2S
    （含静默拒绝）已处理完毕，又以该计数作「拒绝未扣料」的断言。give 同时推进实例载体。

    **时间锚定在 give 之前瞬间、不用调用方锚点**（review finding [major]）：调用方传入
    的 after_t 在「操作 + 1.0s sleep」**之前**截取。若前一 C2S 被错误实现额外 grant，
    其 inventory_snapshot（count 恰为期望值）在 sleep 期间就到——e.t > after_t 会让
    wait_for 扫历史事件时把它误认作 barrier 的 give 快照，barrier 提前返回、真 give 的
    更高计数快照未被观察 → 假绿（重复 break 侧的 _total_items 断言同款被绕过）。因此
    sleep 之后、cmd(give) **之前**取 give_anchor = last_event_time(bot)，排除 give 之前
    一切快照（含错误 grant），wait_for 只能命中 give 自己的快照。

    静默拒绝路径无回执、无「处理完成」信号：若 give 与前一 C2S intent
    （coffin_place / coffin_break）落入同一 tick 窗口，执行/快照可能与前一个请求的
    处理交错（调试构建实测过的窗口；release 20tps 下同样可能同批到达）。此处固定
    sleep 1.0s（≈20 tick）让 give 落在后续独立 tick，保证 give 命令**执行**且其快照
    可观测。该 sleep 是防御性措辞，不放松任何计数断言。
    """
    time.sleep(1.0)
    give_anchor = last_event_time(bot)
    bot.cmd(f"give {COFFIN_ID} 1")
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > give_anchor
        and _coffin_count(e.data["payload"]) == expected_count,
        timeout=timeout,
        description=(
            description
            or f"give {COFFIN_ID} 后 count=={expected_count} 的水位线快照"
        )
        + _snapshot_debug(bot, give_anchor),
    )
    return event.data["payload"]


def _snapshot_debug(bot, after_t):
    from bot.server_data import decode_server_data_payload

    snaps = [
        e
        for e in bot.events
        if e.kind == "server_data" and e.data["payload_type"] == "inventory_snapshot"
    ]
    raw = [
        e
        for e in bot.events
        if e.kind == "payload"
        and e.data.get("channel") == "bong:server_data"
    ]
    raw_desc = []
    for e in raw[-10:]:
        try:
            dec = decode_server_data_payload(e.data["data"])
        except Exception as exc:  # noqa: BLE001
            raw_desc.append(f"t={e.t:.2f} {len(e.data['data'])}B EXC={exc!r}")
        else:
            kind = "NULL" if dec is None else dec.get("type")
            raw_desc.append(f"t={e.t:.2f} {len(e.data['data'])}B {kind}")
    chat_dev = [
        e for e in bot.events if e.kind == "chat" and "[dev]" in e.data.get("text", "")
    ]
    return (
        f"（诊断: after_t={after_t:.3f}, 最近 {len(snaps[-8:])} 条快照 "
        + "; ".join(
            f"t={e.t:.2f} rev={e.data['payload'].get('revision')} "
            f"cnt={_coffin_count(e.data['payload'])} [{_coffin_locations(e.data['payload'])}]"
            for e in snaps[-8:]
        )
        + f"；最近 {len(raw[-10:])} 条 raw bong:server_data: "
        + "; ".join(raw_desc)
        + f"；最近 {len(chat_dev[-4:])} 条 [dev] chat: "
        + "; ".join(f"t={e.t:.2f} {e.data['text']!r}" for e in chat_dev[-4:])
        + "）"
    )


def _give_coffin_instance(bot, after_t, timeout=_STEP_TIMEOUT) -> int:
    """give 一棺并返回锚定 after_t 之后新快照中的实例 id。

    不能用 wait_inventory_contains + latest_inventory_snapshot：wait_for 扫历史事件，
    第二次 give 会瞬间命中先前 give 的旧快照，latest 取到的仍是旧计数快照（无实例）。
    必须 e.t > after_t 锚定「本次 give 之后」的新快照。
    """
    bot.cmd(f"give {COFFIN_ID} 1")
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > after_t
        and _coffin_count(e.data["payload"]) >= 1,
        timeout=timeout,
        description=f"give {COFFIN_ID} 之后含实例的新 inventory_snapshot",
    )
    return _first_coffin_instance_id(event.data["payload"])


def _send_coffin_place(bot, pos, instance_id) -> None:
    bot.intent(
        {
            "type": "coffin_place",
            "v": 1,
            "x": pos[0],
            "y": pos[1],
            "z": pos[2],
            "item_instance_id": instance_id,
        }
    )


# server 侧 mundane coffin marker 实体的 entity kind（entity_model.rs COFFIN_MUNDANE_ENTITY_KIND）
# 与放置位置映射（coffin/mod.rs coffin_marker_position）：marker = (lower.x+1, lower.y, lower.z+0.5)。
COFFIN_MARKER_ENTITY_KIND = 160


def _coffin_marker_pos(place_pos) -> tuple[float, float, float]:
    return (place_pos[0] + 1.0, float(place_pos[1]), place_pos[2] + 0.5)


def _near(pos, x, y, z, tol: float = 0.01) -> bool:
    return (
        abs(pos[0] - x) < tol and abs(pos[1] - y) < tol and abs(pos[2] - z) < tol
    )


def _wait_coffin_marker_spawn(
    bot, place_pos, after_t, timeout=_STEP_TIMEOUT, description=None
) -> int:
    """等 mundane coffin 的 marker 渲染实体 spawn（kind==160 @ coffin_marker_position）。

    证明「世界确实获得了棺实体」：若实现只消费实例却不 spawn 世界实体（或类型/位置不符），
    此处超时红。返回 entity_id 供破坏清场断言复用。
    """
    marker_pos = _coffin_marker_pos(place_pos)

    def matches(event) -> bool:
        return (
            event.kind == "entity_spawn"
            and event.t > after_t
            and event.data["type"] == COFFIN_MARKER_ENTITY_KIND
            and _near(
                (event.data["x"], event.data["y"], event.data["z"]),
                marker_pos[0],
                marker_pos[1],
                marker_pos[2],
            )
        )

    event = bot.wait_for(
        matches,
        timeout,
        description
        or f"mundane_coffin marker 实体 spawn（kind={COFFIN_MARKER_ENTITY_KIND}）于 {marker_pos}",
    )
    return event.data["entity_id"]


def _wait_coffin_marker_destroy(bot, entity_id, after_t, timeout=_STEP_TIMEOUT) -> None:
    """等 coffin_break 后 marker 实体被 despawn（entities_destroy 含 entity_id）。

    证明 destroy path 清掉了世界实体：若实现只移 registry 条目却留下 stale 的
    client-visible 实体（review finding 的 concrete broken implementation），
    此处超时红。coffin 用左键破坏，marker 无方块无 block update，entity 层移除
    （valence Despawned → S2C_ENTITIES_DESTROY）是唯一的清场信号。
    """
    bot.wait_for(
        lambda e: e.kind == "entities_destroy"
        and e.t > after_t
        and entity_id in e.data["entity_ids"],
        timeout=timeout,
        description=f"coffin_break 后 marker 实体 #{entity_id} 应被 despawn（entities_destroy）",
    )


def _assert_no_coffin_marker_spawn(
    bot,
    after_t,
    description,
    window=_REJECT_OBS_WINDOW,
) -> None:
    """拒绝路径不变量：有界窗口内不得出现**任何**新建的 kind=160 marker spawn。

    review finding [major]：旧实现先按放置目标坐标过滤（_near(...) 只收集目标附近
    的 spawn），把「没有在目标附近 spawn」误当成「拒绝即不得 spawn 世界 marker」——
    一个拒绝请求但把 marker 错放到偏移/默认坐标的错误实现能全绿。位置匹配只适用于
    **正向**放置断言；拒绝断言必须拒绝窗口内任何新建 kind=160 实体。本场景每个拒绝
    段的 anchor 都取在首放 marker spawn **之后**（且观察期内 bot 不移动、不跨维，
    无重流来源），因此 e.t > after_t 天然排除既有的首放 marker，窗口内出现的任何
    kind=160 spawn 都只能是拒绝路径错误留下的孤儿实体，无需位置过滤。

    review finding [major]（观察窗）：同 _assert_no_coffin_marker_destroy——旧实现持锁
    扫完**当前**事件前缀就返回——reader 线程后台 append，inventory_snapshot 与实体
    spawn 走不同时序路径，拒绝实现先发快照、后排队 marker spawn 时晚到的 spawn
    从未被检查。因此必须等满一个有界窗口（deadline = after_t + window，事件时间
    单调可比较）再下结论：窗口内逐帧重扫，出现匹配即红，窗口结束仍无才通过。
    扫描持 bot._lock（last_event_time 同款约定）。
    """
    deadline = after_t + window
    spawned = []
    while True:
        with bot._lock:
            spawned = [
                e
                for e in bot.events
                if e.kind == "entity_spawn"
                and e.t > after_t
                and e.data["type"] == COFFIN_MARKER_ENTITY_KIND
            ]
        if spawned:
            break
        if time.monotonic() - bot.t0 >= deadline:
            break
        time.sleep(0.1)
    assert not spawned, (
        f"{description}：拒绝路径不得 spawn mundane_coffin marker 实体，"
        f"实际 {len(spawned)} 个 @ "
        + "; ".join(
            f"#{s.data['entity_id']} ({s.data['x']:.1f},{s.data['y']:.1f},{s.data['z']:.1f})"
            f" t={s.t:.2f}"
            for s in spawned
        )
    )


def _assert_no_coffin_marker_destroy(
    bot, entity_id, after_t, description, window=_REJECT_OBS_WINDOW
) -> None:
    """空→空重复破坏不变量：窗口内不得再次 despawn 指定 marker 实体。

    review finding [major]：单发声称的 empty→empty 转移从未被演练。首破坏的
    despawn 在 after_t 之前；第二次 coffin_break 若再发 entities_destroy 含同一
    entity_id，即实现把「已清场」状态又错误清了一遍（removes unrelated state）。

    review finding [major]（观察窗）：同 spawn 侧——entities_destroy 与
    inventory_snapshot 走不同时序路径，重复 despawn 可能晚于 give 水位线快照到达。
    先等满有界窗口（deadline = after_t + window）再断言，晚到的重复 despawn 不漏检。
    """
    deadline = after_t + window
    redestroyed = []
    while True:
        with bot._lock:
            redestroyed = [
                e
                for e in bot.events
                if e.kind == "entities_destroy"
                and e.t > after_t
                and entity_id in e.data["entity_ids"]
            ]
        if redestroyed:
            break
        if time.monotonic() - bot.t0 >= deadline:
            break
        time.sleep(0.1)
    assert not redestroyed, (
        f"{description}：窗口内发现 {len(redestroyed)} 次对实体 #{entity_id} 的重复 despawn"
    )


def run(env) -> None:
    with env.new_bot("CoPl") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=_STEP_TIMEOUT)

        assert bot.position is not None, "需要 pos_look 后的 bot.position 来定棺位"
        px, py, pz = (int(v) for v in bot.position)
        # y+1：target 必须落在空气层（见模块顶部注释）
        place_pos = (px + PLACE_OFFSET, py + 1, pz)

        # ── 正向：放置消费恰好一个实例 ──────────────────────────────────
        give_anchor = last_event_time(bot)
        first_instance = _give_coffin_instance(bot, give_anchor)
        anchor = last_event_time(bot)
        _send_coffin_place(bot, place_pos, first_instance)
        consumed = _wait_coffin_count(
            bot, 0, anchor, description="首放成功后 coffin 应彻底消费（1→0），且不产生复本"
        )
        assert _coffin_count(consumed) == 0, "首放必须恰好消费 1 个实例"
        # 世界侧：等 mundane coffin 的 marker 渲染实体 spawn——证明世界确实获得了棺实体，
        # 而不只是背包实例被消费。
        marker_entity_id = _wait_coffin_marker_spawn(
            bot,
            place_pos,
            anchor,
            description="首放成功后应出现 mundane_coffin marker 实体（kind=160 @ 放置位）",
        )

        # ── 负向：重复放置 → already registered，第二实例保留 ───────────
        give_anchor = last_event_time(bot)
        second_instance = _give_coffin_instance(bot, give_anchor)
        anchor = last_event_time(bot)
        _send_coffin_place(bot, place_pos, second_instance)
        _give_barrier(
            bot,
            2,
            description=(
                "双放被拒后 +1 give 应恰好 count==2（原保留 1 + 新增 1）——"
                "若 count==1 说明双放偷扣了第二实例（double-grant）"
            ),
        )
        # 世界侧（review finding [major]）：重复放置被拒不得 spawn 第二个 marker。
        # 首放的 marker 在 anchor 之前已 spawn（被时间锚定排除），窗口内再出现
        # **任何** kind=160 spawn（不按位置过滤）即重复路径错误地留下世界孤儿实体。
        _assert_no_coffin_marker_spawn(
            bot,
            anchor,
            description="重复放置拒绝路径不得 spawn 第二个 coffin marker",
        )

        # ── 负向：registry 双键 upper-key 碰撞（review finding [major]）──
        # 已注册棺占 (lower.x, y, z)..(lower.x+1, y, z) 两格，registry 同时以 lower 与
        # upper(=lower+1x) 为键登记、insert 同时查两键（coffin/mod.rs:133-134）。
        # 上面精确重复只把新 lower 打在既有 lower 键上；若实现只查新 lower 对已登记
        # **lower** 键的碰撞、漏查已登记 upper 键，把新放置打在 first 棺的 upper 坐标
        # （upper_pos = place_pos + 1x）会被**接受**并吞掉一个实例（且 spawn 错误 marker）。
        # 正确实现必须以 already-registered 拒绝、实例保留、不得 spawn。
        upper_pos = (place_pos[0] + 1, place_pos[1], place_pos[2])
        upper_give_anchor = last_event_time(bot)
        upper_instance = _give_coffin_instance(bot, upper_give_anchor)
        upper_anchor = last_event_time(bot)
        _send_coffin_place(bot, upper_pos, upper_instance)
        _give_barrier(
            bot,
            4,
            description=(
                "upper-key 重叠放置被拒后 +1 give 应恰好 count==4（原保留 3 + 新增 1）——"
                "若 count==3 说明实现吞掉了 upper-key 碰撞路径的实例"
            ),
        )
        _assert_no_coffin_marker_spawn(
            bot,
            upper_anchor,
            description="upper-key 重叠拒绝路径不得 spawn coffin marker",
        )

        # ── 负向：非空气目标（脚下方块）+ 过远目标，各静默拒绝 ──────────
        anchor = last_event_time(bot)
        probe_instance = _first_coffin_instance_id(latest_inventory_snapshot(bot))
        # 非空气 probe 用 py-1（脚下实心地表，fixture 全平面 y=71/72 均为实心）；
        # 过远 probe 用 py+1（空气层），保证唯一拒绝理由是距离而非 not empty
        _send_coffin_place(bot, (px, py - 1, pz), probe_instance)
        _send_coffin_place(
            bot, (px, py + 1, pz + DISTANCE_REJECT_Z_OFFSET), probe_instance
        )
        _give_barrier(
            bot,
            5,
            description="非空气+过远双拒后 +1 give 应恰好 count==5，实例分文未扣",
        )
        # 世界侧（review finding [major]）：非空气 / 过远拒绝不得 spawn marker——
        # 窗口内任何新建 kind=160 实体都不应出现。
        _assert_no_coffin_marker_spawn(
            bot,
            anchor,
            description="非空气/过远拒绝路径不得 spawn coffin marker",
        )

        # ── 负向（review finding [major]）：无效（已消费 stale）item_instance_id ──
        # first_instance 已被首放消费（consume_item_instance_once 移除实例行，
        # inventory/mod.rs:4171），inventory_item_by_instance 找不到 → 拒绝。位置选
        # 空气层、距玩家 ≈1.4 格（≤6.0 近距）且未注册处，唯一拒绝理由是实例缺失而非
        # not empty / 距离 / already-registered。若实现无视 item_instance_id 而吞掉
        # 任一可用棺材，count 会少 1 → 红。
        stale_pos = (px, py + 1, pz + 1)
        stale_anchor = last_event_time(bot)
        _send_coffin_place(bot, stale_pos, first_instance)
        _give_barrier(
            bot,
            6,
            description=(
                "stale（已消费）instance id 拒绝后 +1 give 应恰好 count==6——"
                "实现不得无视 item_instance_id 吞掉可用棺材"
            ),
        )
        _assert_no_coffin_marker_spawn(
            bot,
            stale_anchor,
            description="stale instance id 拒绝路径不得 spawn coffin marker",
        )

        # review finding [2]（round 5）：拒绝路径只断言「无 spawn + 计数不变」——一个
        # 在 stale_pos 拒绝时先插入 registry 双键（lower/upper）再早退（不消费实例、
        # 不 spawn marker）的错误实现两个负向断言全过；场景此后从未在 stale_pos 放置
        # /破坏，最终 destroy/re-place 只复查原 place_pos，invisible registry 残留
        # 永不暴露。拒绝段后必须对**被拒坐标**做 state-applying 复查：用合法实例在
        # stale_pos 成功放置——若拒绝时错误登记了双键，此放置会以 already-registered
        # 拒绝、marker 不 spawn、实例不消费 → _wait_coffin_marker_spawn 超时红。
        stale_recheck_give = last_event_time(bot)
        stale_recheck_instance = _give_coffin_instance(bot, stale_recheck_give)
        stale_recheck_anchor = last_event_time(bot)
        _send_coffin_place(bot, stale_pos, stale_recheck_instance)
        _wait_coffin_marker_spawn(
            bot,
            stale_pos,
            stale_recheck_anchor,
            description=(
                "stale 拒绝段后在被拒坐标 stale_pos 成功放置并 spawn marker"
                "（证明拒绝未在 registry 留残留，否则此处 already-registered 超时红）"
            ),
        )

        # ── 负向：异维 → 逐字 chat 拒绝，实例保留 ───────────────────────
        tp_anchor = last_event_time(bot)
        bot.cmd("tpdim tsy")
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > tp_anchor
            and "Queued /tpdim tsy within current XYZ gate." in e.data["text"],
            timeout=_STEP_TIMEOUT,
            description="tpdim tsy 排队反馈",
        )
        bot.wait_for(
            lambda e: e.kind == "respawn" and e.t > tp_anchor,
            timeout=_STEP_TIMEOUT,
            description="tpdim tsy 跨维 Respawn",
        )
        bot.wait_for(
            lambda e: e.kind == "pos_look" and e.t > tp_anchor,
            timeout=_STEP_TIMEOUT,
            description="跨维后的坐标确认脉冲",
        )
        cross_anchor = last_event_time(bot)
        _send_coffin_place(
            bot, place_pos, _first_coffin_instance_id(latest_inventory_snapshot(bot))
        )
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > cross_anchor
            and e.data["text"] == DIMENSION_REJECT_TEXT,
            timeout=_STEP_TIMEOUT,
            description=f"异维放置必须逐字拒绝（整体相等）：{DIMENSION_REJECT_TEXT}",
        )
        # 世界侧（review finding [major]）：异维拒绝不得 spawn marker。主世界 marker
        # 早已 spawn（cross_anchor 前）；bot 此刻仍在 tsy，主世界实体不可见、不会重流，
        # 因此整窗 (cross_anchor, cross_anchor+window) 内出现的任何 kind=160 spawn 都
        # 必然是拒绝路径错误留下的孤儿实体——旧实现以 0.5s sleep 提前闭合观察窗
        # （until_t=ret_anchor），拒绝 0.6s 后才发出的错误 marker 会被滤掉而漏检；
        # 必须**先等满整个有界观察窗（仍在异维）再返回**，返回锚点取在观察结束之后。
        _assert_no_coffin_marker_spawn(
            bot,
            cross_anchor,
            description="异维拒绝路径不得 spawn coffin marker",
        )
        ret_anchor = last_event_time(bot)
        bot.cmd("tpdim overworld")
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > ret_anchor
            and "Queued /tpdim overworld within current XYZ gate." in e.data["text"],
            timeout=_STEP_TIMEOUT,
            description="tpdim overworld 排队反馈",
        )
        bot.wait_for(
            lambda e: e.kind == "respawn" and e.t > ret_anchor,
            timeout=_STEP_TIMEOUT,
            description="返回主世界 Respawn",
        )
        pre_break_snapshot = _give_barrier(
            bot,
            7,
            description="异维拒后 +1 give 应恰好 count==7，实例未被异维请求偷扣",
        )
        # review finding [1]（round 5）：首次成功 break 的库存副作用基线必须锚在
        # break **之前**。旧代码 total_baseline 取在 break 完成之后——错误实现
        # despawn marker 时 grant 非棺回收料，该料被并入基线，重复 break 的
        # total_baseline+1 断言照样过。此 count==7 快照（异维拒后权威状态、break
        # intent 之前）即 break 前全量基线，供 break 后 +1 give 快照与之比对。
        total_pre_break = _total_items(pre_break_snapshot)

        # ── 破坏：CoffinBreak 单发清场，同一坐标可重放 ──────────────────
        break_anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_break",
                "v": 1,
                "x": place_pos[0],
                "y": place_pos[1],
                "z": place_pos[2],
            }
        )
        # 世界侧：coffin_break 必须把首放的 marker 实体 despawn（entities_destroy 含其 id）。
        # 若实现只移 registry 条目却留下 stale client-visible 实体，此断言超时红——
        # 这是本场景对「destroy path 清掉世界实体」的直接观察，不是 registry 重用的代理。
        _wait_coffin_marker_destroy(bot, marker_entity_id, break_anchor)

        # review finding [major]：单发清场声称的 empty→empty 重复破坏从未被演练。
        # 第二次对同一坐标 coffin_break：server 端 remove_by_pos → None 后静默 continue
        # （coffin/mod.rs:811-813），不得 panic / 断连 / 再 despawn / 扣料 / 加料。
        # 先以 +1 give 快照（count==8）作全量库存基线——含首次破坏的回收料。
        pre_second = _give_barrier(
            bot,
            8,
            description="首次破坏后 +1 give 应恰好 count==8（破坏只清世界实体，不碰背包实例）",
        )
        # review finding [1]（round 5）：break 后 +1 give 的 pre_second 全量必须恰好
        # = break 前基线 + 1（仅 give 的这口棺）。错误实现若在首次 break despawn
        # marker 时 grant 非棺回收料，_total_items 会多出 1 → 红——这就是「break 前
        # vs break 后立即比较」缺口补上的直接断言，且不被任何后续 give 掩盖。
        assert _total_items(pre_second) == total_pre_break + 1, (
            f"首次 coffin_break 不得产生库存副作用：break 前全量 {total_pre_break}，"
            f"break 后 +1 give 应恰好 {total_pre_break + 1}（仅 give 的这口棺），"
            f"实际 {_total_items(pre_second)}——despawn 时 grant 回收料的错误实现在此红"
        )
        total_baseline = _total_items(pre_second)
        noop_anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_break",
                "v": 1,
                "x": place_pos[0],
                "y": place_pos[1],
                "z": place_pos[2],
            }
        )
        # 第二次 break 处理完毕的水位线：+1 give 后 count 仍==9（无消费）——
        # 若重复破坏再 grant 回收料，_total_items 会超过基线+1（double-grant 红）。
        post_second = _give_barrier(
            bot,
            9,
            description=(
                "重复破坏（空→空）后 +1 give 应恰好 count==9——"
                "第二次 break 不得消费任何实例"
            ),
        )
        assert _total_items(post_second) == total_baseline + 1, (
            f"重复 coffin_break（空→空）必须是无副作用 no-op：全量库存仅应多出 give 的 1 个棺"
            f"（{total_baseline} -> {_total_items(post_second)}），不得扣料/加料/重复 grant"
        )
        _assert_no_coffin_marker_destroy(
            bot,
            marker_entity_id,
            noop_anchor,
            description="重复 coffin_break 不得再次 despawn 已清场的 marker 实体",
        )

        replace_anchor = last_event_time(bot)
        _send_coffin_place(
            bot, place_pos, _first_coffin_instance_id(post_second)
        )
        replaced = _wait_coffin_count(
            bot,
            8,
            replace_anchor,
            description=(
                "破坏清场后同一坐标重放必须成功（9→8）——"
                "证明 destroy path 未在 registry 留 residue"
            ),
        )
        assert _coffin_count(replaced) == 8, "破坏后重放应恰好消费 1 个实例"
        # 世界侧：重放同样必须在世界重新 spawn marker 实体——旧实体已被 despawn、
        # registry 已清空后，新放置能产生新的世界实体（destroy path 无残留的完整闭环）。
        _wait_coffin_marker_spawn(
            bot,
            place_pos,
            replace_anchor,
            description="破坏清场后重放应再次 spawn mundane_coffin marker 实体（kind=160 @ 放置位）",
        )

        bot.assert_alive("CoffinPlace 全生命周期场景完成后")
