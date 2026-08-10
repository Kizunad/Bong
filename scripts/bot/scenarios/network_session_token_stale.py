"""`bong:client_request` 过期 / 重放 session token —— session 权威门禁干净拒绝。

`external_container_move` / `external_container_close` 的 `session_id` 指向服务端
`ExternalContainerRegistry::sessions`（`client_request_handler.rs`
`handle_external_container_move`）。本场景覆盖两类坏 token：

1. **真实 session 的 closed token（重放）**：先用 container_open 打开一个真实外部
   容器 session（server 发放 session_id），并把一个**真实背包物品**移入容器（合法
   is_to_ext move，server 回推 `loot_container_update` 证明物品已入容器），然后
   close 成功（server 回推 `loot_container_close` 证明该 token 曾有效），最后
   **重放这个已关闭的 session_id** 发 move / close。重放的 move 请求本身是**完全
   合法的物品移动**（真实 instance_id、真实 from 位置、真实空闲 to 位置），唯一
   无效前提是 session token 已关闭 —— server 必须干净拒绝：move 回推背包 resync
   快照且**零 mutation**（快照指纹 revision+内容与请求前一致），且**没有**
   `loot_container_update`（成功路径响应不能出现）；close 干净 no-op（不再回推
   close 事件）。这锁定的是"曾有效、现已关闭"的 token 不再被接受 —— 若 close
   未正确使 session 失效，重放 move 会成功并产生 loot_container_update / 指纹
   变化，断言立即失败。
2. **forged token（从未发放）**：高位全 1 的 session_id 不可能被分配过 —— 复用
   real move 模板只换 token，**from 仍是 ext_{real_session}@(0,0)（物品真实在其中）**：
   请求形状完全合法，唯一无效前提是 token 从未发放，server 只能在权威 session 注册表
   门禁上拒绝它。若把 from 派生为 ext_{FORGED}@(0,0)，那个容器从未存在（container_id
   由 session 派生、伪造 session 无实体），「声称的来源容器不含物品」成为第二无效
   前提，跳过注册表查询、按声称容器/物品存在性拒绝的回归实现会假通过（本轮 review
   finding 2）。同样干净拒绝：move resync + 零 mutation + 无 loot_container_update，
   close no-op。

拒绝响应的可观测契约：resync 快照的指纹（revision + 内容）与请求前**完全一致**
（server 只在背包内容突变时 bump revision，单纯重发快照不改 revision；故指纹
相同 ⇒ 零 mutation）。注意 `reason=external_container_resync` 只是 server 日志
标注 —— InventorySnapshotV1 的 wire 上没有 reason 字段，零 mutation 必须靠
revision + 内容指纹来断言。

连接状态：整个场景连接保持，心跳连续，最后合法请求仍被正常处理。
"""

import math
import time

from bot.bot import BotAssertionError  # noqa: F401

DESCRIPTION = "bong:client_request 过期/重放 external-container session token 干净拒绝并 resync 背包"
MODULES = ["network"]

# 伪造 session token：u64 高位全 1，不可能是服务端分配的真实 session id。
FORGED_SESSION_ID = 0xFFFF_FFFF_FFFF_FF00


def _move_request(
    session_id: int, instance_id: int, from_loc: dict, to_loc: dict
) -> dict:
    return {
        "v": 1,
        "type": "external_container_move",
        "session_id": session_id,
        "instance_id": instance_id,
        "from": from_loc,
        "to": to_loc,
    }


def _close_request(session_id: int) -> dict:
    return {"v": 1, "type": "external_container_close", "session_id": session_id}


def _placement_candidates(bot) -> list[tuple[int, int, int]]:
    """返回玩家射程内的可放置候选（container_open 射程上限 4.5 格，取 ≤4.0 留余量）。

    TSY 蓝图区地形非平坦：东侧固定偏移可能整面落进实体墙（实跑实证
    `rejected target block stone is not replaceable` 连升 7 格，且每次 run 的
    spawn 坐标/地面高度不同，seed 选点可落在石质结构顶部/沟槽内）。候选铺满
    玩家脚下层起连续 4 层（fy..fy+4）、水平偏移 1-3 的整圈，全部落在 open 射程内
    （marker 落在 (cx+0.5, cy, cz+0.5)，距离按 marker 到玩家计），按距离升序 ——
    就近候选最可能是空气格，少试几次即中。只搜 fy/fy+1 会把「立在结构顶、周围是
    1-2 格高石墙」的 spawn 判成无可放置格（实跑：86/86 全 stone 拒，fy+2/fy+3 的
    空气格从未尝试）；高格 marker 距玩家仍 ≤4.0，open 射程（4.5）内可开。
    """
    if bot.position is None:
        raise BotAssertionError("stale session 场景需要 pos_look 后的位置，实际 position=None")
    px, py, pz = bot.position
    fx, fy, fz = math.floor(px), math.floor(py), math.floor(pz)
    candidates: list[tuple[int, int, int]] = []
    for cy in range(fy, fy + 5):
        for dx in range(-3, 4):
            for dz in range(-3, 4):
                if dx == 0 and dz == 0:
                    continue
                # marker 位置到玩家直线距离 ≤4.0（4.5 射程上限留 0.5 余量）。
                if math.dist((fx + dx + 0.5, cy, fz + dz + 0.5), (px, py, pz)) > 4.0:
                    continue
                candidates.append((fx + dx, cy, fz + dz))
    candidates.sort(
        key=lambda c: math.dist((c[0] + 0.5, c[1], c[2] + 0.5), (px, py, pz))
    )
    return candidates


def _open_real_session(bot) -> dict:
    """创建并打开一个真实外部容器 session，并把一个真实背包物品移入容器。

    返回供 stale / replay move 复用的**真实 move 模板**：
      {
        "session_id": int,   # server 发放的真实 token
        "instance_id": int,  # 已移入 ext 容器的真实物品实例（1x1 grass_fiber）
        "from": {...},       # 该物品在 ext_{session} 中的真实位置 (0,0)
        "to": {...},         # 物品移入前的背包格（现为空，合法空闲 to 位置）
      }
    重放该 move 时只换 ``session_id``（已关闭 / 伪造），物品与位置维度全部真实 →
    只有 token 是无效前提（review finding 2/3）。

    任何环境缺口（无 chunk / 无 marker / 移入失败）都**直接抛错**：issued-to-closed
    重放是场景核心覆盖，不得静默跳过（review finding 3）。
    """
    from ._inventory_helpers import (
        container_location,
        find_item,
        latest_inventory_snapshot,
        require_item,
        wait_inventory_revision_after,
        wait_inventory_revision_after_matching,
    )

    snapshot = latest_inventory_snapshot(bot)
    bot.cmd("clearinv all")
    bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
    snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)

    # bong.db 会跨 run 持久化该用户名上一轮已保存的背包（restored_inventory=true）：
    # join 快照里可能带着上轮遗留的 trade_crate/grass_fiber（实跑实证 Stl 上轮 forged
    # 阶段的 fiber instance=489 被 restore 出来，`wait_inventory_contains` 从 cursor=0
    # 扫历史，会命中 clearinv 之前的 join 快照，拿到已被清掉的遗留实例 → 合法 move 被
    # server 以 "instance not in player inventory" 拒绝。故两次 give 都用 revision 门槛
    # 等新快照：clearinv 之后、give 之后的快照 revision 严格大于清空时点的 revision，
    # 遗留物品所在的历史快照（rev ≤ 门槛）被排除，锁定的实例保证真实存在。
    bot.cmd("give trade_crate 1")
    bot.expect_chat("[dev] gave trade_crate x1", timeout=10.0)
    snapshot = wait_inventory_revision_after_matching(
        bot,
        snapshot["revision"],
        lambda payload: find_item(payload, "trade_crate") is not None,
        "give 后包含 trade_crate 的新快照（revision 门槛排除 restore/遗留 crate 快照）",
        timeout=10.0,
    )
    crate = require_item(snapshot, "trade_crate")

    # spawn 地形非平坦（TSY 蓝图区），东侧 2 格可能是整面实体墙（实跑实证连升 7 格
    # 全被 stone 拒）。改为在玩家射程内（container_open 上限 4.5 格）逐候选格放置：
    # marker 精确落在 (cx+0.5, cy, cz+0.5)，用**紧** predicate（±0.25 只认本格中心）
    # 唯一锁定 marker —— 宽松 box（±1.5/±2.0）会匹配到候选格附近的其它实体，把
    # container_open 送到错误 entity 上（实跑观察：循环被候选格旁的既有实体提前
    # break）。失败放置是 no-op（trade_crate 不消耗，可复用同一 item），全部候选失败
    # 才抛错（fail-closed，issued-to-closed 重放是核心覆盖，环境缺口不得静默跳过，
    # review finding 3）。
    #
    # **迟到 spawn 调和（review finding 1）**：等待超时只证明「0.8s 内该候选格没出现
    # marker」，不证明放置失败 —— server 正常吞掉 crate、但 entity_spawn 事件因调度/
    # 网络延迟晚于 0.8s 到达，且后序候选复用了已被消费的 instance_id 被拒，旧实现把
    # 已成功的放置判成整体失败。predicate 改为匹配**任意已尝试候选格**（attempted 快照
    # 每轮追加）：每轮 wait_for 都从事件历史头部重扫（cursor=0），前一候选格迟到的
    # spawn 在后一轮扫描里命中，成功放置不再被丢弃。候选格间距 ≥1，±0.25 的紧 box
    # 只命中本格 marker，attempted 的 any() 不会把邻近格的既有实体误收。
    spawn = None
    sent_at = time.monotonic() - bot.t0
    attempted: list[tuple[int, int, int]] = []
    for cx, cy, cz in _placement_candidates(bot):
        bot.intent(
            {
                "type": "block_place",
                "v": 1,
                "x": cx,
                "y": cy,
                "z": cz,
                "item_instance_id": crate["item"]["instance_id"],
                "target_face": "north",
            }
        )
        attempted.append((cx, cy, cz))

        def _spawn_at_attempted(e, attempted=tuple(attempted)) -> bool:
            if e.kind != "entity_spawn" or e.t <= sent_at:
                return False
            return any(
                abs(e.data["x"] - (p[0] + 0.5)) <= 0.25
                and abs(e.data["y"] - p[1]) <= 0.25
                and abs(e.data["z"] - (p[2] + 0.5)) <= 0.25
                for p in attempted
            )

        try:
            spawn = bot.wait_for(
                _spawn_at_attempted,
                timeout=0.8,
                description="trade_crate 放置后在该候选格出现容器 Marker entity_spawn",
            )
            break
        except BotAssertionError:
            continue
    if spawn is None:
        raise BotAssertionError(
            "stale-session 场景在玩家射程内所有候选格都无法放置 trade_crate Marker："
            "issued-to-closed 重放是核心覆盖，环境缺口不得静默跳过（review finding 3）"
        )

    bot.intent({"type": "container_open", "v": 1, "entity_id": spawn.data["entity_id"]})
    opened = bot.expect_server_data("loot_container_open", timeout=10.0)
    session_id = opened.data["payload"]["session_id"]
    bot.assert_alive("container_open 打开真实 session 后")

    # 移入一个真实物品：give grass_fiber（1x1）→ 背包 → ext_{session}@(0,0)。
    # revision 门槛（container_open 之后的 revision 之前先读）：bong.db restore 可能
    # 在上轮遗留一个 grass_fiber（实跑实证 instance=489），`wait_inventory_contains`
    # 会命中 join 快照、把已清掉的遗留实例发进 move。门槛锁定 give 后新快照（严格大于
    # 门槛 revision），from 指向本次 give 真实实例的位置。
    fiber_give_before_rev = latest_inventory_snapshot(bot)["revision"]
    bot.cmd("give grass_fiber 1")
    bot.expect_chat("[dev] gave grass_fiber x1", timeout=10.0)
    fiber_snapshot = wait_inventory_revision_after_matching(
        bot,
        fiber_give_before_rev,
        lambda payload: find_item(payload, "grass_fiber") is not None,
        "give 后包含 grass_fiber 的新快照（revision 门槛排除 restore/遗留 fiber 快照）",
        timeout=10.0,
    )
    fiber = find_item(fiber_snapshot, "grass_fiber")
    assert fiber is not None
    # move 前的 revision 门槛：等 move 的 resync 快照（revision > 该值且 fiber 已移出
    # 背包）落定后才返回。move 成功路径的 resync_inventory_only 与 Changed 驱动的
    # inventory_changed 重发都在 loot_container_update 之后到达，run() 的 pre_close
    # 若在它们消费前读 latest，会拿到 give 后的旧快照（fiber 还在背包、revision 未
    # bump），与 close 的 resync（fiber 已出）指纹不匹配，误报零 mutation 违约
    # （实跑实证 pre=rev16/fiber-in-pack vs close resync=rev17/fiber-out）。
    move_before_rev = latest_inventory_snapshot(bot)["revision"]
    move_in_at = time.monotonic() - bot.t0
    bot.intent(
        _move_request(
            session_id,
            fiber["item"]["instance_id"],
            fiber["location"],
            container_location(f"ext_{session_id}", 0, 0),
        )
    )
    update = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "loot_container_update"
        and e.data["payload"]["session_id"] == session_id
        and e.t > move_in_at,
        timeout=10.0,
        description="合法 pack→ext move 回推 loot_container_update",
    )
    placed = [
        placed_item
        for placed_item in update.data["payload"]["placed_items"]
        if placed_item["item"]["instance_id"] == fiber["item"]["instance_id"]
    ]
    if not placed:
        raise BotAssertionError(
            "合法移入后 ext 容器必须包含该 grass_fiber 实例，"
            f"实际 placed_items={update.data['payload']['placed_items']}"
        )
    bot.assert_alive("合法物品移入 ext 容器后")
    wait_inventory_revision_after_matching(
        bot,
        move_before_rev,
        lambda payload: find_item(payload, "grass_fiber") is None,
        "move 后 fiber 移出背包的 resync 快照（revision 门槛排除 move 前旧快照）",
        timeout=10.0,
    )

    return {
        "session_id": session_id,
        "instance_id": fiber["item"]["instance_id"],
        "from": container_location(f"ext_{session_id}", 0, 0),
        "to": fiber["location"],
    }


def _assert_stale_move_rejected_zero_mutation(
    bot, session_id: int, label: str, real_move: dict
) -> None:
    """stale / 重放 external_container_move 必须被干净拒绝：零 mutation + 无成功路径响应。

    ``real_move`` 是 move 模板（instance_id / from / to）—— 探针只换 session token，
    **from 复用模板里的真实容器位置**（``ext_{real_session}@(0,0)``，物品真实在其中）：
    - **closed token（重放）**：session_id=真实 session，from=ext_{real}@(0,0)（物品
      真实在其中）、to=真实空闲背包格。token 已关闭，唯一无效前提是「token 已关闭」。
    - **forged token（从未发放）**：session_id=FORGED，from 仍用 ext_{real}@(0,0)
      （物品真实在其中）。请求形状完全合法：物品在声称的来源容器里、to 是空闲背包格，
      唯一无效前提是「token 从未发放」—— 普通容器/物品校验无从拒绝，server 只能在
      **权威 session 注册表门禁**（``ext_reg.sessions.get``，client_request_handler.rs
      入口首查）上拒绝它。旧设计把 from 派生为 ext_{FORGED}@(0,0) 使位置与顶层 token
      一致，但该容器**从未存在**（container_id 由 session 派生、伪造 session 无实体），
      「声称的来源容器不含物品」成了第二个无效前提：跳过注册表查询、按声称容器/物品
      存在性校验拒绝的回归实现会干净拒绝、探针假通过（本轮 review finding 2）。

    若 server 错误接受坏 token：
    - closed 的源物品真实在 ext_{real}，move 会成功回推 loot_container_update +
      bump revision —— 本断言的零 mutation 指纹与
      ``assert_no_server_data_payload_since(loot_container_update)`` 立即失败；
    - forged 的请求形状完全合法，跳过注册表门禁的实现只有两条出路：(a) 继续执行 move
      成功 → loot_container_update + mutation → 失败；(b) 在 is_from_ext 端点一致性
      检查（``ext_{real} != ext_{FORGED}``，client_request_handler.rs 19288）被拒 →
      该分支回推 ``resync_ext_and_inventory``，**同样发出 loot_container_update** →
      断言失败。任何跳过门禁的实现都逃不出这两种可观测结果；「位置与顶层 token 矛盾」
      不再能当掩盖点（旧 review finding 3 的顾虑正是该拒绝分支，其可观测签名已被
      无 loot_container_update 断言覆盖）。

    **同步点 + resync 契约（本轮 review finding 1/3）**：不靠「等任意 resync 快照」
    判定拒绝 —— 周期 shelflife / Changed 驱动的无变更重发与拒绝 resync 同类型同时钟，
    会提前满足等待，把「请求尚未处理」误判成「已干净拒绝」。server 按连接**串行**
    处理请求，故本函数发完 move 后调 ``assert_valid_request_still_works``（发一个合法
    请求并等它的聊天确认）：确认到达即证明本 move 已处理完毕，任何迟到的成功响应 /
    副作用已全部入队。被拒 move 的 resync（``resync_inventory_only``，各拒绝分支均
    如此）在 move 处理时同步回推、**先于**同步点确认到达 —— 故 resync 快照必须落在
    **t ∈ (before, 同步点确认时刻]** 区间（把同步屏障收进快照谓词），而不是 10s 窗口
    里任意一张快照：同步点确认之后才到达的快照与 move 处理无因果关系，属周期重发，
    不能冒充拒绝 resync（本轮 review finding 3）。且 resync 内容必须零变化 —— 只比对
    latest 缓存快照会放走「直接把 move 丢进黑洞、一张 resync 都不发」的实现（latest
    仍是请求前旧快照，指纹相等照样通过，旧 review finding 4）。pre 指纹在发请求
    **之前**读取；锚点同样在发请求前取发送时刻（与 ``event.t`` 同一相对时钟）。
    """
    from ._inventory_helpers import (
        drain_inventory_snapshots,
        latest_inventory_snapshot,
        wait_inventory_snapshot_after,
    )
    from ._rejection_helpers import (
        assert_no_server_data_payload_since,
        assert_valid_request_still_works,
        inventory_fingerprint,
    )

    # central-review 1993 #2：发请求前排干在途快照 —— event.t 是客户端解码时刻，一张
    # 之前已入队、之后才解码的快照会以 t > before 冒充本 move 的拒绝 resync（请求未
    # 处理就满足断言）。排干后窗口内唯一的快照来源就是本 move 自己的 resync。
    drain_inventory_snapshots(bot)
    pre = latest_inventory_snapshot(bot)
    before = time.monotonic() - bot.t0
    bot.intent(
        _move_request(
            session_id,
            real_move["instance_id"],
            real_move["from"],
            real_move["to"],
        )
    )
    # review finding 1/3：同步点证明本 move 已处理完毕（server 串行处理请求，之后合法
    # 请求的聊天确认到达 ⇒ 本 move 的全部响应已入队），迟到的 loot_container_update
    # 必然落入扫描窗口。确认事件的时间戳同时是 resync 快照窗口的上界（resync 先于它）。
    confirm_t = assert_valid_request_still_works(bot)
    assert_no_server_data_payload_since(bot, before, "loot_container_update", label)
    # resync 契约：被拒 move 必须回推一张 **t ∈ (before, confirm_t]** 的新背包快照且
    # 内容零变化 —— 同步屏障收进快照谓词（review finding 3：同步点确认前的快照才与
    # move 处理有因果关系；确认后才到的属周期重发，不能冒充 resync）。server 若直接把
    # move 丢进黑洞（连 resync 都不发），等待必然超时，不会因周期快照假通过。
    resync = wait_inventory_snapshot_after(
        bot, before, until_t=confirm_t, timeout=10.0
    )
    if inventory_fingerprint(resync) != inventory_fingerprint(pre):
        raise BotAssertionError(
            f"{label}：期望拒绝后 resync 背包零 mutation，"
            f"实际 pre={inventory_fingerprint(pre)} "
            f"resync={inventory_fingerprint(resync)}"
        )


def _assert_stale_close_rejected(bot, session_id: int, label: str) -> None:
    """stale / 重放 external_container_close 必须被干净拒绝：无 close 响应 + 零 mutation。

    review finding 3/6：旧实现 `time.sleep(1.0)` 后只扫一次存量事件 —— server 若在
    扫描之后（调度/网络延迟下，如 1.1s）才处理非法 close，迟到的 `loot_container_close`
    不会被捕获；且 close 断言从不比对前后背包指纹，接受未知/已关闭 token 后只发
    inventory_snapshot（无 loot_container_close 响应）的 mutation 也会通过。与
    ``_assert_stale_move_rejected_zero_mutation`` 同一套**同步点**：server 串行处理
    请求，之后的合法请求确认到达即证明本 close 已处理完毕，任何响应 / 副作用已全部
    入队 —— 再扫 `loot_container_close` 缺失 + 同步点后最新快照指纹 vs 请求前指纹
    相等（零 mutation）。
    """
    from ._inventory_helpers import latest_inventory_snapshot
    from ._rejection_helpers import (
        assert_no_server_data_payload_since,
        assert_valid_request_still_works,
        inventory_fingerprint,
    )

    pre = latest_inventory_snapshot(bot)
    before = time.monotonic() - bot.t0
    bot.intent(_close_request(session_id))
    assert_valid_request_still_works(bot)
    assert_no_server_data_payload_since(bot, before, "loot_container_close", label)
    post = latest_inventory_snapshot(bot)
    if inventory_fingerprint(post) != inventory_fingerprint(pre):
        raise BotAssertionError(
            f"{label}：期望 close 干净 no-op 且背包零 mutation，"
            f"实际 pre={inventory_fingerprint(pre)} post={inventory_fingerprint(post)}"
        )
    bot.assert_alive(f"{label} 后")


def run(env) -> None:
    from ._inventory_helpers import (
        drain_inventory_snapshots,
        latest_inventory_snapshot,
        wait_inventory_snapshot_after,
    )
    from ._rejection_helpers import (
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
        inventory_fingerprint,
    )

    with env.new_bot("Stl") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)
        # 等 join 时的初始快照突发放完，避免把 join snapshot 误判成拒绝响应。
        time.sleep(1.0)

        # ---- 1. 真实 session 生命周期：open → 移入真实物品 → close → replay
        real_move = _open_real_session(bot)
        session_id = real_move["session_id"]

        # 证明 token 曾有效：close 被接受，server 回推 loot_container_close。
        # 同时断言 close 后的 resync 快照零 mutation（指纹与 close 前一致）。
        # central-review 1993 #2：close 前排干在途快照 —— 之后 close resync 断言的
        # 因果链成立（窗口内唯一快照来源就是本 close 的 resync，不再有「之前已入队、
        # 之后才解码」的快照冒充）。
        drain_inventory_snapshots(bot)
        pre_close = latest_inventory_snapshot(bot)
        close_before = time.monotonic() - bot.t0
        bot.intent(_close_request(session_id))
        bot.expect_server_data("loot_container_close", timeout=10.0)
        resync = wait_inventory_snapshot_after(bot, close_before, timeout=10.0)
        if inventory_fingerprint(resync) != inventory_fingerprint(pre_close):
            raise BotAssertionError(
                "真实 session close：期望 close 后 resync 背包零 mutation，"
                f"实际 pre={inventory_fingerprint(pre_close)} "
                f"resync={inventory_fingerprint(resync)}"
            )
        bot.assert_alive("真实 session close 后")

        # replay 已关闭 session 的 move —— move 本身合法（真实物品/位置），唯一
        # 无效前提是 token 已关闭：干净拒绝 = resync + 零 mutation + 无成功响应。
        _assert_stale_move_rejected_zero_mutation(
            bot, session_id, "replay 已关闭 session 的 move", real_move
        )
        bot.assert_alive("replay 已关闭 session 的 move 拒绝后")

        # replay 已关闭 session 的 close —— 干净 no-op：不再回推 close 事件，且背包
        # 零 mutation（review finding 3/6：同步点 + 指纹，见 _assert_stale_close_rejected）。
        _assert_stale_close_rejected(bot, session_id, "replay 已关闭 session 的 close")

        # ---- 2. forged（从未发放的 token）—— 复用真实 session 的 move 模板，只换
        # token（本轮 review finding 2）。helper 复用 real_move 的 from
        # （ext_{real}@(0,0)，物品真实在其中）、to=真实空闲背包格 —— 请求形状完全
        # 合法，唯一无效前提是 token 从未发放。server 只能在权威 session 注册表门禁
        # 上拒绝它：跳过注册表查询的实现只有两条出路 —— 继续执行 move 成功（
        # loot_container_update + mutation → 断言失败），或在 is_from_ext 端点一致性
        # 检查上拒绝（该分支回推 resync_ext_and_inventory，同样发 loot_container_update
        # → 断言失败），均不会被「声称容器不含物品」/「位置与 token 矛盾」掩盖。
        _assert_stale_move_rejected_zero_mutation(
            bot, FORGED_SESSION_ID, "stale move #1（forged token）", real_move
        )
        bot.assert_alive("stale move #1 拒绝响应后")

        # stale move #2（重放同一 forged token）—— 同样干净拒绝，连接不坏。
        _assert_stale_move_rejected_zero_mutation(
            bot, FORGED_SESSION_ID, "stale move #2（重放 forged token）", real_move
        )
        bot.assert_alive("stale move #2 重放拒绝后")

        # stale close（forged token）—— 未知 token 干净 no-op，不再回推 close 事件
        # 且背包零 mutation（review finding 3/6）。
        _assert_stale_close_rejected(bot, FORGED_SESSION_ID, "stale close（forged token）")

        fire_probes_and_keep_connection(
            bot,
            "stale session",
            [("重放 close", lambda: bot.intent(_close_request(FORGED_SESSION_ID)))],
            baseline_snapshot=latest_inventory_snapshot(bot),
        )
        assert_valid_request_still_works(bot)
