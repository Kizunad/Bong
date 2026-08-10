"""`bong:client_request` 越界字段值 —— schema / version 门禁干净拒绝。

三档拒绝都在 handler 产生任何玩法副作用**之前**拦截：
- **serde 守卫**（`client_request.rs` 内 `deserialize_slot_index` /
  `deserialize_block_picker_count`）：hotbar/quick slot >8 或负数、block_picker
  count=0 或 >64、v 超出 u8、非法 enum 变体 —— 反序列化期拒绝；
- **version 门禁**（`client_request_handler.rs` SUPPORTED_VERSION=1）：v≠1 ——
  handler 入口拒绝。

本场景锁的是：每个越界值都被**干净**拒绝 —— 不崩、不踢、连接状态完好；
探针窗口内**无任何玩法副作用**（server_data / chat / vfx 均未出现），且探针前后
背包快照指纹（revision + 内容）完全一致 —— 证明越界请求在产生任何玩法副作用
之前就被拦截，没有被 clamp 后继续执行；之后合法请求仍被正常处理。

**botany_harvest mode 非法变体探针**（review finding 4）单独在 run() 里探：它需要
**真实活跃的 botany session** 才能把「schema 拒绝」和「无 session 的下游 no-op」
区分开 —— 若打在 session_id="x" 这种不存在 session 上，serde 错误接受 garbage 后
请求进 handler 也会被 "missing harvest session" 拒绝，探针假通过。run() 先
`/bong gather` 建真实 session，正向对照每个合法 mode（manual/auto）都有可观测响应
（progress 回推 mode 切换），再对 garbage 断言 mode 不变 + 进度未被重置。

**合法边界也要证明被接受**（review finding 1）：只探坏值不探好值，一个 off-by-one
实现（只接受 slot 0..7 / count 1..63）会拒绝掉全部坏探针、通过全部干净拒绝断言，
却错误拒绝了上边界合法请求。故反向补两组**恰好落在契约边界内**的正向探针：
- slot 0 与 slot 8（`deserialize_slot_index` 契约 0..=8）：`quick_slot_bind` 清空
  该槽，server 回推 `quickslot_config` ack（`ack_request_id` 回显本次 request_id +
  `bind_accepted=true`）—— 证明 serde 接受边界槽位；
- count 1 与 count 64（`deserialize_block_picker_count` 契约 1..=64）：
  `block_picker_give` 进入 handler 并给出 [dev] 回应（fixture 是 Survival，得到
  "requires Creative mode" 聊天；若 Creative 则 "gave ..." 聊天）—— 任一聊天都
  证明 serde 接受了边界 count。

正向探针同样用相对时钟锚点（`time.monotonic() - bot.t0`，与 `event.t` 同一
时钟），响应必须严格晚于发送时刻。
"""

import time

from bot.bot import BotAssertionError  # noqa: F401
from bot.scenarios._inventory_helpers import wait_inventory_contains

DESCRIPTION = "bong:client_request 越界字段值(slot/count/v/变体) 被 schema+版本门禁干净拒绝"
MODULES = ["network"]

OUT_OF_RANGE_PROBES = [
    ("hotbar slot=9 越界", {"type": "use_quick_slot", "v": 1, "slot": 9}),
    ("hotbar slot=-1 负数", {"type": "use_quick_slot", "v": 1, "slot": -1}),
    (
        "block_picker_give count=0",
        {"type": "block_picker_give", "v": 1, "block_id": "stone_bricks", "count": 0},
    ),
    (
        "block_picker_give count=65",
        {"type": "block_picker_give", "v": 1, "block_id": "stone_bricks", "count": 65},
    ),
    ("版本 v=0 低于支持版本", {"type": "breakthrough_request", "v": 0}),
    ("版本 v=255 高于支持版本", {"type": "breakthrough_request", "v": 255}),
    ("v 超出 u8 范围", {"type": "breakthrough_request", "v": 999999999}),
    # botany_harvest mode 非法变体探针**不**在批量里：它需要真实活跃 botany session
    # 才不被「无匹配 session」的 handler 拒绝掩盖（见 _assert_invalid_harvest_mode_rejected），
    # 单独在 run() 里建 session 后探。
]

# 契约边界内**合法** slot 值（deserialize_slot_index 契约 0..=8）。用 quick_slot_bind
# 清空槽位（item_id=None）作正向探针：其 ack（quickslot_config，回显 request_id +
# bind_accepted）只有请求真正走完 handler 才出现。
_VALID_SLOTS = (0, 8)

# review finding 2：use_quick_slot slot=9/-1 探针必须打在**已绑定可用物品**的槽位上，
# 否则空槽 no-op 路径会让「错误接受越界 slot、clamp 到边界槽、再走空槽 no-op」的实现
# 无任何可观测副作用地通过全部干净拒绝断言。先 give 回元丹并绑定到边界槽 0/8 —— 若
# 实现错误 clamp 9→8 / -1→0，会命中已绑定且仍在背包的物品并启动施法 → cast_sync 被
# 探针窗口标记（cast_sync 不在 ambient 集合，任何出现即视为副作用）。
_PILL_TEMPLATE = "huiyuan_pill"
_PILL_GIVE_COUNT = 2
_BOUND_SLOTS = (0, 8)
# botany 进度基线下限：auto 收割时长 AUTO_DURATION_TICKS=120（6s@20t/s），sync 节拍
# 10 tick（0.5s）→ 每拍进度 +0.083。基线必须 ≥0.25（上次 mode 翻转重置后已积累
# ≥1.5s 进度），否则「默认成 resting_mode 并重置」的实现（progress 回落到 ~0 再
# 续增）在首个后置样本就反超基线，单调比较失灵（central-review 1993 #3）。
_PROGRESS_BASELINE_FLOOR = 0.25
# 处理屏障的 sentinel 槽位：不在 _BOUND_SLOTS/_VALID_SLOTS 用到的 0/8 上，避免与
# 背包绑定 / 边界探针的 quick_slot_bind 互踩。
_BARRIER_SLOT = 1


def _bind_slot_with_item(bot, slot: int, label: str) -> None:
    """quick_slot_bind 绑定 item_id 到槽位并等 ack。

    与 ``_assert_slot_boundary_accepted`` 相对：那里用 item_id=None **清空**槽位作
    正向边界探针；这里用真实物品**填满**槽位，让后续越界 use_quick_slot 探针打在
    非空、可用、会触发施法的状态上（review finding 2 的探测前提）。
    """
    request_id = f"rng-bind-slot{slot}"
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {
            "v": 1,
            "type": "quick_slot_bind",
            "slot": slot,
            "item_id": _PILL_TEMPLATE,
            "request_id": request_id,
        }
    )
    bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data.get("payload_type") == "quickslot_config"
        and e.t > sent_at
        and e.data["payload"].get("ack_request_id") == request_id
        and e.data["payload"].get("bind_accepted") is True,
        timeout=10.0,
        description=(
            f"{label}：quick_slot_bind 绑定 {_PILL_TEMPLATE} 到 slot={slot}"
            f"（ack_request_id={request_id} 回显 + bind_accepted）"
        ),
    )


def _bind_pill_to_slots(bot) -> None:
    """give 回元丹并绑定到边界槽 0/8（review finding 2 的探测前提）。"""
    bot.cmd(f"give {_PILL_TEMPLATE} {_PILL_GIVE_COUNT}")
    bot.expect_chat(f"[dev] gave {_PILL_TEMPLATE} x{_PILL_GIVE_COUNT}", timeout=10.0)
    wait_inventory_contains(bot, _PILL_TEMPLATE, timeout=10.0)
    for slot in _BOUND_SLOTS:
        _bind_slot_with_item(bot, slot, f"绑定回元丹 slot={slot}")

# 契约边界内**合法** count 值（deserialize_block_picker_count 契约 1..=64）。
# block_picker_give 的 [dev] 聊天回应证明 serde 接受了该 count（fixture Survival 得到
# "requires Creative mode"，Creative 得到 "gave ..."）。
_VALID_COUNTS = (1, 64)


def _assert_slot_boundary_accepted(bot, slot: int, label: str) -> None:
    request_id = f"rng-slot{slot}"
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {
            "v": 1,
            "type": "quick_slot_bind",
            "slot": slot,
            "item_id": None,
            "request_id": request_id,
        }
    )
    bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data.get("payload_type") == "quickslot_config"
        and e.t > sent_at
        and e.data["payload"].get("ack_request_id") == request_id
        and e.data["payload"].get("bind_accepted") is True,
        timeout=10.0,
        description=(
            f"{label}：server 回推 quickslot_config ack"
            f"（ack_request_id={request_id} 回显 + bind_accepted）"
        ),
    )


def _assert_count_boundary_accepted(bot, count: int, label: str) -> None:
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {
            "v": 1,
            "type": "block_picker_give",
            "block_id": "stone_bricks",
            "count": count,
        }
    )
    bot.wait_for(
        lambda e: e.kind == "chat"
        and e.t > sent_at
        and (
            "requires Creative mode" in e.data["text"]
            or "gave " in e.data["text"]
        ),
        timeout=10.0,
        description=(
            f"{label}：block_picker_give 进入 handler"
            f"（[dev] 聊天回应证明 serde 接受 count={count}）"
        ),
    )


def _open_botany_harvest_session(bot) -> str:
    """建立真实活跃 botany harvest session（/bong gather），返回其 session_id。

    ``botany_harvest_request`` 只在 session 已存在时才有可观测成功路径（
    ``request_harvest_mode`` 更新 session → 下个 sync 节拍回推 botany_harvest_progress）。
    探针若打在 ``session_id="x"`` 这种不存在 session 上，serde 错误接受未知 mode 变体
    后请求进 handler 也会被 "missing harvest session" 拒绝，探针假通过（本轮 review
    finding 4）。先用 ``/bong gather spirit_grass`` 建立真实 session，再读回推的
    botany_harvest_progress 拿 session_id（server 以 canonical_player_id 为 session_id，
    client 无法凭空构造）。
    """
    bot.cmd("bong gather spirit_grass")
    bot.expect_chat("Gameplay action queued.", timeout=10.0)
    progress = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "botany_harvest_progress",
        timeout=15.0,
        description="botany 采集 session 建立后的 botany_harvest_progress 回推",
    )
    return progress.data["payload"]["session_id"]


def _assert_harvest_mode_flip(bot, session_id: str, mode: str, label: str) -> None:
    """正向对照：合法 mode 请求必须产生可观测响应（botany_harvest_progress mode 切换）。

    ``botany_harvest_request`` 走通 handler（``request_harvest_mode``）会把 session 的
    mode 切成请求值，server 在 sync 节拍（10 tick ≈ 0.5s）回推 botany_harvest_progress
    mode 即新值 —— 这是「合法 mode 被 schema 接受」的黑盒证据。只探坏值不探好值，
    默认化/off-by-one 实现会通过全部坏值断言却错误处理合法值（review finding 1 同款
    论证）；覆盖两个合法变体 manual/auto，保证「每个合法 mode 都有可观测响应」。
    """
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {"v": 1, "type": "botany_harvest_request", "session_id": session_id, "mode": mode}
    )
    bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "botany_harvest_progress"
        and e.t > sent_at
        and e.data["payload"].get("mode") == mode,
        timeout=10.0,
        description=(
            f"{label}：botany_harvest_request mode={mode} 被接受"
            f"（progress 回推 mode={mode}，session 状态被合法请求推进）"
        ),
    )


def _harvest_processing_barrier(bot, label: str) -> float:
    """同连接处理屏障：quick_slot_bind sentinel 的 ack 证明此前请求已被处理。

    mode="garbage" 的包被 schema 拒绝时**没有任何响应** —— 只靠固定 sleep 等周期
    progress，会放走「请求仍在排队、尚未处理」的窗口：周期事件照常发射满足断言，
    请求随后才被接受并突变 session（central-review 1993 #1）。quick_slot_bind 在
    handler 执行时回推 quickslot_config ack（ack_request_id 回显 + bind_accepted）；
    server 按连接串行处理请求，sentinel 的 ack 到达 ⇒ 排在它前面的包（含 garbage）
    已处理完毕。绑定 item_id=None（清空 slot 1；QuickSlotBindings 是独立组件，不改
    背包指纹）。返回 ack 事件时刻作为 post-processing 水位。
    """
    request_id = f"rng-barrier-{label}"
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {
            "v": 1,
            "type": "quick_slot_bind",
            "slot": _BARRIER_SLOT,
            "item_id": None,
            "request_id": request_id,
        }
    )
    ack = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data.get("payload_type") == "quickslot_config"
        and e.t > sent_at
        and e.data["payload"].get("ack_request_id") == request_id
        and e.data["payload"].get("bind_accepted") is True,
        timeout=10.0,
        description=f"{label}：quick_slot_bind sentinel ack（同连接处理屏障）",
    )
    return ack.t


def _assert_invalid_harvest_mode_rejected(bot, session_id: str, resting_mode: str) -> None:
    """坏 mode 探针：打在真实 session 上，且不翻转 mode / 不重置进度。

    mode="garbage" 若被 serde 错误接受并默认成某个合法变体，``request_harvest_mode``
    会：(a) 把 session.mode 切到默认值 → 下一次回推的 progress 显示 mode != resting_mode；
    或 (b) 默认成 resting_mode 本身 → mode 字段不变，但 started_at_tick 被重置 →
    progress 回落到 ~0 再续增。正确 serde 在反序列化期整包丢弃该请求，session 状态
    不变 → progress 仍显示 resting_mode、进度单调续增。两种可观测差异任一出现即判定
    serde 接受了 garbage，坏实现不再被「session 不存在」掩盖（本轮 review finding 4）。

    基线必须满足三个前提才可信（central-review 1993 #3）：绑定当前 session 的
    session_id、取最新观察之后的新鲜样本（watermark 后首次）、progress ≥
    _PROGRESS_BASELINE_FLOOR（上次 mode 翻转重置后已积累 ≥1.5s 进度）。基线近零时，
    「默认成 resting_mode 并重置」的实现（progress 回落到 ~0 再以 1/6s 速率续增）在
    首个后置样本就反超基线，单调比较失灵。
    """
    watermark = bot.events[-1].t if bot.events else 0.0
    pre = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data["payload_type"] == "botany_harvest_progress"
            and e.data["payload"].get("session_id") == session_id
            and e.data["payload"].get("mode") == resting_mode
            and e.data["payload"].get("progress", 0.0) >= _PROGRESS_BASELINE_FLOOR
            and e.t > watermark
        ),
        timeout=10.0,
        description=(
            f"garbage 前 session={session_id} resting_mode={resting_mode} 的新鲜进度基线"
            f"（watermark 后 progress≥{_PROGRESS_BASELINE_FLOOR}）"
        ),
    )
    pre_progress = pre.data["payload"].get("progress", 0.0)
    sent_at = time.monotonic() - bot.t0
    bot.intent(
        {
            "v": 1,
            "type": "botany_harvest_request",
            "session_id": session_id,
            "mode": "garbage",
        }
    )
    # central-review 1993 #1：固定 sleep 不是处理屏障 —— 周期 progress 在请求仍排队时
    # 也照常发射，未处理前的样本会满足旧断言、随后被接受的请求在无扫描处突变 session。
    # sentinel 的 ack 证明 garbage 包已被处理；ack 之后的 progress 采样才是处理后状态。
    barrier_t = _harvest_processing_barrier(bot, "garbage 后的处理屏障")
    # barrier 后给 server ≥1 个 sync 节拍（PROGRESS_SYNC_INTERVAL_TICKS=10 ≈ 0.5s/拍）
    # 回推处理后的 progress，再一次性扫处理屏障后的全部 progress —— 迟到的成功响应
    # 也计入拒绝判定。
    time.sleep(0.6)
    offenders = [
        e
        for e in bot.events
        if e.kind == "server_data"
        and e.data["payload_type"] == "botany_harvest_progress"
        and e.t > barrier_t
    ]
    if not offenders:
        raise BotAssertionError(
            "garbage 探针后（处理屏障之后）没有 botany_harvest_progress 回推，"
            "无法判定 session 状态（progress 应每 ~0.5s 回推一次）"
        )
    for e in offenders:
        payload = e.data["payload"]
        if payload.get("mode") != resting_mode:
            raise BotAssertionError(
                "botany_harvest mode 非法变体探针：期望 mode 保持"
                f" {resting_mode}，实际 progress 回推 mode={payload.get('mode')}"
                " —— serde 把 garbage 默认成了别的合法变体"
            )
        if payload.get("progress", 0.0) < pre_progress - 1e-9:
            raise BotAssertionError(
                "botany_harvest mode 非法变体探针：期望进度单调续增（未被请求重置），"
                f"实际 progress {pre_progress:.3f} → {payload.get('progress', 0.0):.3f}"
                " —— serde 接受了 garbage 并重置了 session"
            )


def run(env) -> None:
    from ._inventory_helpers import latest_inventory_snapshot, wait_join_and_inventory
    from ._rejection_helpers import (
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
        inventory_fingerprint,
    )

    with env.new_bot("Rng") as bot:
        wait_join_and_inventory(bot)

        # review finding 2：先建立「越界 use_quick_slot 命中已绑定物品会施法」的可观测
        # 状态，再取指纹基线 —— give 会 bump revision，基线若取在 give 前会把 give 的
        # revision 变化误判成探针副作用。绑定本身不改变背包指纹（QuickSlotBindings 是
        # 独立组件），故绑定后取基线仍与「绑定前背包内容」一致。
        _bind_pill_to_slots(bot)
        pre = latest_inventory_snapshot(bot)
        pre_fingerprint = inventory_fingerprint(pre)

        probes = [
            (label, lambda req=req: bot.intent(req))
            for label, req in OUT_OF_RANGE_PROBES
        ]
        fire_probes_and_keep_connection(
            bot, "越界字段值", probes, baseline_snapshot=pre
        )

        # 背包状态零变化：探针后最新快照指纹（revision + 内容）必须与探针前一致。
        # 任何 slot/count 被 clamp 后继续执行（哪怕只改动一处状态）都会 bump revision。
        post = latest_inventory_snapshot(bot)
        post_fingerprint = inventory_fingerprint(post)
        if post_fingerprint != pre_fingerprint:
            raise BotAssertionError(
                "越界字段值探针后背包快照指纹变化：某个越界请求被 clamp/部分处理了，"
                f"探针前={pre_fingerprint} 探针后={post_fingerprint}"
            )

        # ---- botany_harvest mode 非法变体探针（review finding 4）：必须打在**真实活跃
        # botany session** 上 —— 否则 serde 错误接受 garbage 后请求进 handler 也会被
        # "missing harvest session" 拒绝，探针假通过。先用 /bong gather 建 session，
        # 正向对照证明每个合法 mode（manual/auto）都有可观测响应（progress 回推 mode
        # 切换），再对 garbage 断言 mode 不变 + 进度未被重置（默认化实现两种可观测差异
        # 之一必现）。botany session 的周期回推与背包/聊天断言互不干扰。
        session_id = _open_botany_harvest_session(bot)
        _assert_harvest_mode_flip(bot, session_id, "auto", "合法 mode=auto 正向对照")
        _assert_harvest_mode_flip(bot, session_id, "manual", "合法 mode=manual 正向对照")
        _assert_harvest_mode_flip(
            bot, session_id, "auto", "合法 mode=auto 再翻转(resting)"
        )
        _assert_invalid_harvest_mode_rejected(bot, session_id, resting_mode="auto")
        bot.assert_alive("botany mode 非法变体探针后连接仍存活")

        # ---- 合法边界正向探针：slot 0/8、count 1/64 必须被 schema 接受（review
        # finding 1）。坏探针全被拒不足以证明好边界被接受 —— off-by-one 实现会拒掉
        # 全部坏探针却拒绝上边界合法请求。
        for slot in _VALID_SLOTS:
            _assert_slot_boundary_accepted(bot, slot, f"合法边界 slot={slot}")
        for count in _VALID_COUNTS:
            _assert_count_boundary_accepted(bot, count, f"合法边界 count={count}")

        assert_valid_request_still_works(bot)
