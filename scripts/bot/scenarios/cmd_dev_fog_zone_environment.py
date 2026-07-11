"""dev 命令链路 —— `/fog` 动态雾堤（brigadier → EnvironmentOverlays → sync 组装 → 广播）。

黑盒断言面：
- `/fog spawn` 后 `bong:zone_environment` 广播里出现对应 density 的 `fog_veil`
  （锁 `server/src/world/environment_overlay.rs` 的 overlay 并进
  `sync_zone_environment_effects` 组装、不被每 tick `replace_for_dimension` 冲掉）
- `/fog clear_all` 后同 zone 的下一条广播不再含该 `fog_veil`（锁摘除路径 + dirty 重播）
- chat 反馈契约 `[dev] fog ...`（`server/src/cmd/dev/fog.rs`）

DENSITY 取 0.93：区别于所有静态/天气 FogVeil 常量（scorch 0.34 / tribulation 0.42 /
tsy 0.58 / HeavyHaze 0.85 等），断言不会误中别的雾。
"""

import json

DESCRIPTION = "/fog spawn/clear_all 后 bong:zone_environment 广播含/不含对应 fog_veil（动态雾堤链路）"
MODULES = ["cmd", "world"]

CHANNEL = "bong:zone_environment"
DENSITY = 0.93


def _decode_state(event) -> dict | None:
    try:
        return json.loads(bytes(event.data["data"]))
    except (ValueError, TypeError, KeyError):
        return None


def _has_bank(state: dict | None) -> bool:
    if not state:
        return False
    return any(
        effect.get("kind") == "fog_veil"
        and abs(float(effect.get("density", 0.0)) - DENSITY) < 1e-3
        for effect in state.get("effects", [])
    )


def run(env) -> None:
    with env.new_bot("Fog") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        bot.cmd(f"fog spawn 48 {DENSITY} 6000")
        bot.expect_chat("[dev] fog", timeout=10.0)

        spawned = bot.wait_for(
            lambda e: e.kind == "payload"
            and e.data["channel"] == CHANNEL
            and _has_bank(_decode_state(e)),
            15.0,
            f"含 density={DENSITY} fog_veil 的 {CHANNEL} 广播（overlay 应并进 sync 组装并触发 dirty 重播）",
        )
        zone_id = _decode_state(spawned)["zone_id"]

        bot.cmd("fog clear_all")
        bot.expect_chat("cleared", timeout=10.0)

        bot.wait_for(
            lambda e: e.kind == "payload"
            and e.data["channel"] == CHANNEL
            and e.t > spawned.t
            and (lambda state: state is not None
                 and state.get("zone_id") == zone_id
                 and not _has_bank(state))(_decode_state(e)),
            15.0,
            f"clear_all 后 zone `{zone_id}` 的 {CHANNEL} 重播且不再含该 fog_veil（摘除应触发重播）",
        )

        bot.assert_alive("fog spawn/clear_all 全链路后")
