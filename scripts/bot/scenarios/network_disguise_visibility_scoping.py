"""伪装名单泄漏面契约 —— disguise_enter 只许下发视野内实体 id。

spider/daozhan 的 `*_disguise_enter` 周期 full sync 原先把**全图**伪装实体
id 明文广播给所有玩家——改装客户端可直读「谁是伪装的」，拟态玩法对作弊者
归零。server 侧已改为按 client ViewDistance 半径过滤（network/disguise_sync.rs）。

本场景锁两个契约：

1. **泄漏面**：收到的 disguise entity_ids 必须 ⊆ bot 已通过 entity_spawn
   看到的实体集合。视野外实体 id 出现在名单里 = 全图广播回归。
   （fallback 世界无拟态生物时空表 vacuous 通过；raster dev 世界有远处
   拟态蛛/道伥时会真实撞红。）
2. **空表 keepalive**：client 端 handler 是全量替换语义（clear+addAll），
   周期 sync 必须照发（含空表）来清 stale 条目。断流 = 半径过滤把
   keepalive 一起砍掉的回归。
"""

import json
import time

DESCRIPTION = "disguise_enter 名单 ⊆ 视野内实体（泄漏契约）+ 空表 keepalive 不断流"
MODULES = ["network", "fauna"]

DISGUISE_CHANNELS = (
    "bong:spider_disguise_enter",
    "bong:daozhan_disguise_enter",
)

# 周期 sync 间隔 40 个 CultivationClock tick（每 Update +1）：墙钟时长随
# TPS 浮动——debug build 实测 ~10 TPS → 周期 ~4s。窗口取 30s，
# 即使全套连跑把 TPS 压到 ~3（周期 ~13s）也能覆盖 ≥2 个周期。
SETTLE_SECONDS = 5.0
COLLECT_SECONDS = 30.0
MIN_KEEPALIVES_PER_CHANNEL = 2
# 名单与实体包同 tick flush 的顺序竞态缓冲：名单收集截止后再多收实体包
ENTITY_GRACE_SECONDS = 4.0


def _decode_ids(event) -> list[int]:
    payload = json.loads(event.data["data"].decode("utf-8"))
    ids = payload.get("entity_ids")
    assert isinstance(ids, list), (
        f"disguise payload entity_ids 应为数组（wire 契约 v1），"
        f"实际 {type(ids).__name__}: {payload!r}"
    )
    return ids


def run(env) -> None:
    with env.new_bot("Scope") as bot:
        bot.expect_event("game_join", timeout=15.0)
        time.sleep(SETTLE_SECONDS)  # chunk + 实体流式下发稳定

        start_t = bot.events[-1].t if bot.events else 0.0
        time.sleep(COLLECT_SECONDS)
        cutoff_t = start_t + COLLECT_SECONDS
        time.sleep(ENTITY_GRACE_SECONDS)

        events = list(bot.events)

        disguise_events = [
            e
            for e in events
            if e.kind == "payload"
            and e.data["channel"] in DISGUISE_CHANNELS
            and start_t <= e.t <= cutoff_t
        ]
        known_ids = {e.data["entity_id"] for e in events if e.kind == "entity_spawn"}

        # 契约 2：空表 keepalive 不断流（窗口覆盖 ≥2 个 sync 周期，两通道都该
        # 周期性出现——只出现 1 次可能是 join 残留，锁不住周期性）
        for channel in DISGUISE_CHANNELS:
            hits = [e for e in disguise_events if e.data["channel"] == channel]
            assert len(hits) >= MIN_KEEPALIVES_PER_CHANNEL, (
                f"窗口 {COLLECT_SECONDS}s 内只收到 {len(hits)} 条 {channel}"
                f"（期望 ≥{MIN_KEEPALIVES_PER_CHANNEL}，40 clock-tick 周期在 5 TPS 下也应"
                f"命中 ≥2 次）——periodic sync 断流。半径过滤只许过滤名单内容，"
                f"空表 keepalive 必须保留（client 全量替换语义靠它清 stale）"
            )

        # 契约 1：名单 ⊆ 视野内实体（full sync 与 delta 都不许携带没见过的 id）
        for event in disguise_events:
            ids = _decode_ids(event)
            unseen = [i for i in ids if i not in known_ids]
            assert not unseen, (
                f"{event.data['channel']} 名单携带 bot 从未通过 entity_spawn 见过的 "
                f"entity id {unseen}（已见 {len(known_ids)} 个实体）——视野外伪装"
                f"名单泄漏，改装客户端可全图直读「谁是伪装的」。检查 "
                f"disguise_sync 半径过滤是否被绕过/回归为全图广播"
            )

        bot.assert_alive("disguise 泄漏面契约收集窗口之后")
