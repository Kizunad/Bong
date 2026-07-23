"""真实 Bot + Redis + Tiandao runtime：境界门拒绝提示仅投递给目标玩家。

两名 Minecraft 协议 Bot 真实连接同一 server。测试进程启动生产
``AgentUiRuntime``（真实 ``UiRenderer`` + ``UiResponseConsumer``）并故意用过期的
高境界快照触发清晰版面板；server 依据目标 Bot 的实际醒灵境权威拒绝。最终必须只有
目标 Bot 收到 typed ``bong:server_data/narration``，旁观 Bot 的后续命令响应栅栏之后
仍无该提示，也不能出现旧式 chat mirror。
"""

from __future__ import annotations

import json
import os
import pathlib
import subprocess

from bot.bot import BotAssertionError


DESCRIPTION = "真实 AgentUiRuntime 境界门拒绝：双 Bot 中仅目标收到 system_warning"
MODULES = ["agent", "network", "multibot"]

REALM_GATE_TEXT = (
    "天道的注意力掠过，境界未至，缘分尚浅——此时感知到的，只是一片模糊的余韵。"
)
ROOT = pathlib.Path(__file__).resolve().parents[3]
RUNNER = ROOT / "agent/packages/tiandao/tests/agent-ui-realm-gate-bot-runner.ts"
TSX = ROOT / "agent/node_modules/.bin/tsx"


def _is_target_narration(event, *, after: float) -> bool:
    if event.kind != "server_data" or event.t <= after:
        return False
    payload = event.data.get("payload", {})
    if payload.get("type") != "narration":
        return False
    return any(
        narration.get("text") == REALM_GATE_TEXT
        and narration.get("style") == "system_warning"
        for narration in payload.get("narrations", [])
    )


def _run_agent_runtime(target_name: str) -> dict:
    if not TSX.is_file():
        raise BotAssertionError(
            f"真实 Tiandao runtime 需要 {TSX}；先在 agent/ 执行 npm ci，实际文件不存在"
        )

    child_env = os.environ.copy()
    child_env.setdefault("REDIS_URL", "redis://127.0.0.1:6379")
    child_env["TARGET_NAME"] = target_name
    child_env["TARGET_PLAYER"] = f"offline:{target_name}"

    try:
        completed = subprocess.run(
            [str(TSX), str(RUNNER)],
            cwd=ROOT / "agent/packages/tiandao",
            env=child_env,
            text=True,
            capture_output=True,
            timeout=35,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise BotAssertionError(
            "真实 AgentUiRuntime 35s 内未完成 server→agent→server 往返；"
            f"stdout={error.stdout!r} stderr={error.stderr!r}"
        ) from error

    if completed.returncode != 0:
        raise BotAssertionError(
            "真实 AgentUiRuntime 子进程失败；"
            f"exit={completed.returncode} stdout={completed.stdout!r} stderr={completed.stderr!r}"
        )

    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    try:
        evidence = json.loads(lines[-1])
    except (IndexError, json.JSONDecodeError) as error:
        raise BotAssertionError(
            f"AgentUiRuntime 未输出可解析证据 JSON：stdout={completed.stdout!r}"
        ) from error

    expected_target = f"offline:{target_name}"
    if (
        evidence.get("target_player") != expected_target
        or evidence.get("realm_gate") != 5
        or evidence.get("sent_blur_version") is not False
        or evidence.get("stats", {}).get("realmGateRejected") != 1
        or evidence.get("stats", {}).get("narrationPublished") != 1
    ):
        raise BotAssertionError(
            "生产 UiRenderer/UiResponseConsumer 证据不满足 clear gate→reject→narration 契约；"
            f"期望 target={expected_target}, gate=5, blur=false, stats=1/1，实际={evidence}"
        )
    return evidence


def run(env) -> None:
    with env.new_bot("RGA") as alice:
        alice.expect_event("game_join", timeout=15.0)
        alice.expect_event("pos_look", timeout=15.0)
        alice.cmd("realm set awaken")
        alice.expect_chat("[dev] realm set", timeout=10.0)

        with env.new_bot("RGB") as bob:
            bob.expect_event("game_join", timeout=15.0)
            bob.expect_event("pos_look", timeout=15.0)

            alice_after = alice.events[-1].t if alice.events else 0.0
            bob_after = bob.events[-1].t if bob.events else 0.0
            evidence = _run_agent_runtime(alice.username)

            received = alice.wait_for(
                lambda event: _is_target_narration(event, after=alice_after),
                timeout=20.0,
                description=(
                    "真实 realm_gate_rejected 经 AgentUiRuntime 回流后的 system_warning narration"
                ),
            )
            narrations = received.data["payload"]["narrations"]
            matching = [entry for entry in narrations if entry.get("text") == REALM_GATE_TEXT]
            if len(matching) != 1:
                raise BotAssertionError(
                    f"目标玩家应恰好收到 1 条境界门提示，实际 matching={matching} evidence={evidence}"
                )
            expected_target = f"offline:{alice.username}"
            if (
                matching[0].get("scope") != "player"
                or matching[0].get("style") != "system_warning"
                or matching[0].get("target") != expected_target
            ):
                raise BotAssertionError(
                    "目标 Bot 收到的 production protobuf narration 必须保留 player scope、"
                    f"system_warning style 与 canonical target={expected_target}，实际={matching[0]}"
                )

            # 用 Bob 的后续命令响应作网络栅栏：若旧 broadcast 路由仍存在，同一批次的
            # narration 必已在这条更晚的 server→Bot 响应之前进入 reader 事件流。
            fence = bob.events[-1].t if bob.events else bob_after
            bob.cmd("realm set awaken")
            bob.wait_for(
                lambda event: (
                    event.kind == "chat"
                    and event.t > fence
                    and "[dev] realm set" in event.data.get("text", "")
                ),
                timeout=10.0,
                description="旁观玩家的后续命令响应网络栅栏",
            )

            leaked = [
                event
                for event in bob.events
                if _is_target_narration(event, after=bob_after)
            ]
            if leaked:
                raise BotAssertionError(
                    "境界门私人提示泄漏给旁观玩家；"
                    f"target={alice.username} bystander={bob.username} leaked={leaked}"
                )

            for bot, after in ((alice, alice_after), (bob, bob_after)):
                mirrors = [
                    event
                    for event in bot.events_of("chat")
                    if event.t > after and REALM_GATE_TEXT in event.data.get("text", "")
                ]
                if mirrors:
                    raise BotAssertionError(
                        f"[{bot.username}] typed narration 不应退化为 GameMessage chat mirror，实际={mirrors}"
                    )

            alice.assert_alive("境界门私人 narration 完整往返后")
            bob.assert_alive("旁观玩家隔离断言后")
