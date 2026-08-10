"""延寿棺 CoffinPlace 全生命周期黑盒：放置消费 → 重复/非空/过远/异维拒绝 → 破坏清场后可重放。

plan-coffin-v1 放置链路（client_request_handler.rs → handle_coffin_place_requests）黑盒契约面：

- `coffin_place{v,x,y,z,item_instance_id}` 必须用真实 instance_id（inventory_item_by_instance
  按 id 校验）。放置成功消费**恰好一个实例**（consume_item_instance_once）；失败路径分文不动。
- 重复放置（already registered）在 registry 双键（lower/upper）碰撞处拒绝，**先于消费**——
  第二实例必须保留在背包（防双注册双扣料 / double-grant，本家族最重的重复/双发检查）。
- 非空气目标（not empty）、距离 > 6.0（coffin_target_is_close 36.0 平方）均**静默拒绝**
  （warn 无回执）——只能靠实例计数不变断言。
- 异维（coffin_requires_overworld fail-closed，先于近距/实例校验）拒绝并逐字 chat：
  `§c[棺] 你不在主世界，无法操作延寿棺。`
- `coffin_break` 破坏单发（remove_by_pos → None 后静默 continue），破坏后 registry 清空——
  同一坐标再放置必须成功（"destroy path leaves nothing behind"）。

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

DESCRIPTION = "CoffinPlace 生命周期：放置消费→重复/非空/过远/异维拒绝（计数不变）→破坏清场后可重放"
MODULES = ["coffin", "inventory", "dimension"]

COFFIN_ID = "mundane_coffin"
PLACE_OFFSET = -2  # 横向偏移，与 forge 放砧一致
# 放置目标必须在 y+1（fixture 全平面 surface_y=72，y=72 是实心地面、y=73+ 才是空气）：
# bot.position 的 y 是玩家脚下方块的顶（≈72.x，int 后=72），直接用它当 target 会命中
# 实心地表 → not empty 拒。所有「应为空气」的 target 统一用 py+1。
DISTANCE_REJECT_Z_OFFSET = 10  # > 6.0 的过远目标（coffin_target_is_close 36.0 平方）
DIMENSION_REJECT_TEXT = "你不在主世界，无法操作延寿棺"

_STEP_TIMEOUT = 45.0


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


def _give_barrier(bot, after_t, expected_count, timeout=_STEP_TIMEOUT, description=None):
    """give 一棺并以该 give 的 inventory_snapshot 作 server 权威水位线 + 断言载体。

    不用 give 的 chat ack 当水位线：实测（run4 证据）ack 在 client_request 处理**之前**
    发出（chat 命令 pass 先于 client_request pass），且在紧邻 give→give 交错下 ack 会丢
    （revision=14/16 收到、barrier 的 r17 未到）；而 give 的 inventory_snapshot 落在同 tick
    的 client_request 处理**之后**——收到 count==expected 的新快照，既证明此前每个 C2S
    （含静默拒绝）已处理完毕，又以该计数作「拒绝未扣料」的断言。give 同时推进实例载体。
    匹配用 e.t > after_t 时间锚定，防止复读先前 give 的同类快照。

    静默拒绝路径无回执、无「处理完成」信号：若 give 与前一 C2S intent
    （coffin_place / coffin_break）落入同一 tick 窗口，执行/快照可能与前一个请求的
    处理交错（调试构建实测过的窗口；release 20tps 下同样可能同批到达）。此处固定
    sleep 1.0s（≈20 tick）让 give 落在后续独立 tick，保证 give 命令**执行**且其快照
    可观测。该 sleep 是防御性措辞，不放松任何计数断言。
    """
    time.sleep(1.0)
    bot.cmd(f"give {COFFIN_ID} 1")
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > after_t
        and _coffin_count(e.data["payload"]) == expected_count,
        timeout=timeout,
        description=(
            description
            or f"give {COFFIN_ID} 后 count=={expected_count} 的水位线快照"
        )
        + _snapshot_debug(bot, after_t),
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
            anchor,
            2,
            description=(
                "双放被拒后 +1 give 应恰好 count==2（原保留 1 + 新增 1）——"
                "若 count==1 说明双放偷扣了第二实例（double-grant）"
            ),
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
            anchor,
            3,
            description="非空气+过远双拒后 +1 give 应恰好 count==3，实例分文未扣",
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
            and DIMENSION_REJECT_TEXT in e.data["text"],
            timeout=_STEP_TIMEOUT,
            description=f"异维放置必须逐字拒绝：{DIMENSION_REJECT_TEXT}",
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
        _give_barrier(
            bot,
            ret_anchor,
            4,
            description="异维拒后 +1 give 应恰好 count==4，实例未被异维请求偷扣",
        )

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
        pre_replace = _give_barrier(
            bot,
            break_anchor,
            5,
            description="破坏后 +1 give 应恰好 count==5（破坏只清世界实体，不碰背包实例）",
        )
        replace_anchor = last_event_time(bot)
        _send_coffin_place(
            bot, place_pos, _first_coffin_instance_id(pre_replace)
        )
        replaced = _wait_coffin_count(
            bot,
            4,
            replace_anchor,
            description=(
                "破坏清场后同一坐标重放必须成功（5→4）——"
                "证明 destroy path 未在 registry 留 residue"
            ),
        )
        assert _coffin_count(replaced) == 4, "破坏后重放应恰好消费 1 个实例"
        # 世界侧：重放同样必须在世界重新 spawn marker 实体——旧实体已被 despawn、
        # registry 已清空后，新放置能产生新的世界实体（destroy path 无残留的完整闭环）。
        _wait_coffin_marker_spawn(
            bot,
            place_pos,
            replace_anchor,
            description="破坏清场后重放应再次 spawn mundane_coffin marker 实体（kind=160 @ 放置位）",
        )

        bot.assert_alive("CoffinPlace 全生命周期场景完成后")
