"""死亡必须收掉低血心跳 loop，裁决与重生期间不能重新播放（实机 bug 回归锁）。

黑盒契约面（全走真实协议，不读 server 内部状态）：
- 存活时血量跌破 20%（`/health set 1`）→ server 发 `bong:audio/play`，
  recipe `heartbeat_low_hp`。这是 **loop recipe**（`interval_ticks: 20`，第二层是
  `minecraft:entity.player.hurt`），client 侧 `SoundRecipePlayer` 自己每秒重放。
- 该 play 必须带**非 0 的稳定 instance_id**：`instance_id: 0` 会让 server 现分配，
  事后无从指认，也就永远收不掉这条 loop。
- 死亡（`/kill self` 走真实死亡链）→ server 必须发
  `bong:audio/stop`，instance_id 与上面那条 play **同一个**；之后不允许再出现
  heartbeat_low_hp 的 play；之后 `/revive self` 也不能重新触发。

原 bug：server 只在上沿发 play，从不发 stop；client 侧带 flag 的 loop 又把 flag
自注册成 sticky（while_flag 判定永真）→ 重生后 `entity.player.hurt` 每秒一响。
"""

import json
import time

from bot.bot import BotAssertionError

DESCRIPTION = "死亡停止同实例低血心跳，复活裁决与重生后不再播放受伤心跳音"
MODULES = ["audio", "combat", "cultivation"]

HEARTBEAT_RECIPE = "heartbeat_low_hp"


def _audio_events(bot, channel: str, after: float = 0.0):
    """取出某条 audio 通道上的所有 payload（已解 JSON）。"""
    out = []
    with bot._lock:
        events = list(bot.events)
    for event in events:
        if event.kind != "payload" or event.t <= after:
            continue
        if event.data.get("channel") != channel:
            continue
        raw = event.data.get("data", b"")
        try:
            decoded = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            continue
        out.append((event.t, decoded.get("payload", decoded)))
    return out


def _heartbeat_plays(bot, after: float = 0.0):
    return [
        (t, payload)
        for t, payload in _audio_events(bot, "bong:audio/play", after)
        if payload.get("recipe_id") == HEARTBEAT_RECIPE
    ]


def run(env) -> None:
    with env.new_bot("RespawnSfx") as bot:
        bot.expect_event("game_join", timeout=20.0)
        bot.expect_event("pos_look", timeout=20.0)
        time.sleep(1.0)

        # 先处于真实的存活低血状态，不能依靠已被修复的“死亡后起心跳”。
        bot.cmd("health set 1")
        bot.expect_chat("Queued /health set", timeout=15.0)
        deadline = time.time() + 15.0
        plays = []
        while time.time() < deadline:
            plays = _heartbeat_plays(bot)
            if plays:
                break
            time.sleep(0.5)
        if not plays:
            raise BotAssertionError(
                f"期望存活低血时收到 {HEARTBEAT_RECIPE} 的 bong:audio/play"
                "（血量跌破 20% 的低血心跳），实际一条都没收到 —— "
                "低血心跳触发链断了，重生残留受伤音的回归锁失去意义"
            )

        heartbeat_instance = plays[0][1].get("instance_id")
        recipe = plays[0][1].get("recipe") or {}
        if not recipe.get("loop"):
            raise BotAssertionError(
                f"期望 {HEARTBEAT_RECIPE} 是 loop recipe（client 侧按 interval 自行重放），"
                f"实际 payload 里没有 loop 段：{recipe}"
            )
        if not heartbeat_instance:
            raise BotAssertionError(
                f"期望 {HEARTBEAT_RECIPE} 的 play 带非 0 稳定 instance_id，"
                f"因为 instance_id=0 由 server 现分配、事后无法用同一 id 发 stop"
                f"（loop 永生 → 重生后仍每秒响 entity.player.hurt）；实际 {heartbeat_instance!r}"
            )

        # 死亡当下就应按同一 instance 收掉 loop，不得等到复活才停止。
        death_anchor = bot.events[-1].t
        bot.cmd("kill self")
        bot.expect_chat("[dev] kill self", timeout=15.0)

        deadline = time.time() + 15.0
        stops = []
        while time.time() < deadline:
            stops = [
                payload
                for _, payload in _audio_events(bot, "bong:audio/stop", death_anchor)
                if payload.get("instance_id") == heartbeat_instance
            ]
            if stops:
                break
            time.sleep(0.5)
        if not stops:
            observed = [
                payload.get("instance_id")
                for _, payload in _audio_events(bot, "bong:audio/stop", death_anchor)
            ]
            raise BotAssertionError(
                f"期望死亡时收到 instance_id={heartbeat_instance} 的 bong:audio/stop，"
                f"因为 {HEARTBEAT_RECIPE} 是 client 侧自行重放的 loop（第二层 = "
                "minecraft:entity.player.hurt），server 不显式 stop 就会重生后一直响受伤音；"
                f"实际死亡后 stop 的 instance 列表 = {observed}"
            )

        # 零血等待裁决期间不能重启，不能只看恢复满血后的状态。
        time.sleep(3.0)
        late = _heartbeat_plays(bot, death_anchor)
        if late:
            raise BotAssertionError(f"死亡裁决期间不应重新播放低血心跳，实际 {late}")

        bot.cmd("revive self")
        bot.expect_chat("[dev] revive self", timeout=15.0)
        time.sleep(3.0)
        late = _heartbeat_plays(bot, death_anchor)
        if late:
            raise BotAssertionError(
                f"期望重生后血量回满、不再有 {HEARTBEAT_RECIPE} 的 play，"
                f"实际又发了 {len(late)} 条：{[payload.get('instance_id') for _, payload in late]}"
            )
