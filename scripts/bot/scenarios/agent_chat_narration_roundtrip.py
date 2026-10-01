"""P5c 完整聊天回流：Bot → Redis 列表 → Tiandao 标注 → Redis 叙事 → Bot。

场景启动一个前台、限时的 TypeScript 适配器。适配器调用生产 RedisIpc 与
processChatBatch，只有 LLM 标注替换为确定性实现；最终仍由 server 消费
``bong:agent_narrate`` 并经真实 ``bong:server_data/narration`` 发给目标 Bot。
"""

from __future__ import annotations

import json
import os
import pathlib
import secrets
import subprocess

from bot.bot import BotAssertionError


DESCRIPTION = "聊天经 bong:player_chat→Tiandao/mock→bong:agent_narrate 回流为目标玩家 narration"
MODULES = ["agent", "network", "multibot"]

ROOT = pathlib.Path(__file__).resolve().parents[3]
RUNNER = ROOT / "agent/packages/tiandao/tests/chat-narration-bot-runner.ts"
TSX = ROOT / "agent/node_modules/.bin/tsx"
AGENT_TIMEOUT_SECONDS = 30
NARRATION_TIMEOUT_SECONDS = 20


def _is_marker_narration(event, *, after: float, marker: str, target: str) -> bool:
    if event.kind != "server_data" or event.t <= after:
        return False
    payload = event.data.get("payload", {})
    if payload.get("type") != "narration":
        return False
    return any(
        entry.get("scope") == "player"
        and entry.get("target") == target
        and marker in entry.get("text", "")
        for entry in payload.get("narrations", [])
    )


def _run_agent_roundtrip(target_name: str, marker: str) -> dict:
    if not TSX.is_file():
        raise BotAssertionError(
            f"完整 Agent 回流需要 {TSX}；先在 agent/ 执行 npm ci，实际文件不存在"
        )

    child_env = os.environ.copy()
    child_env.setdefault("REDIS_URL", "redis://127.0.0.1:6379")
    child_env["TARGET_NAME"] = target_name
    child_env["CHAT_TOKEN"] = marker
    try:
        completed = subprocess.run(
            [str(TSX), str(RUNNER)],
            cwd=ROOT / "agent/packages/tiandao",
            env=child_env,
            text=True,
            capture_output=True,
            timeout=AGENT_TIMEOUT_SECONDS,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise BotAssertionError(
            f"Tiandao 回流适配器 {AGENT_TIMEOUT_SECONDS}s 内未完成；"
            f"stdout={error.stdout!r} stderr={error.stderr!r}"
        ) from error

    if completed.returncode != 0:
        raise BotAssertionError(
            "Tiandao 回流适配器失败；"
            f"exit={completed.returncode} stdout={completed.stdout!r} stderr={completed.stderr!r}"
        )

    evidence = None
    for line in reversed([line for line in completed.stdout.splitlines() if line.strip()]):
        try:
            candidate = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(candidate, dict) and candidate.get("chat_channel"):
            evidence = candidate
            break
    if evidence is None:
        raise BotAssertionError(
            f"Tiandao 回流适配器未输出可解析证据 JSON：stdout={completed.stdout!r}"
        )

    expected_player = f"offline:{target_name}"
    narration = evidence.get("narration", {})
    if (
        evidence.get("chat_channel") != "bong:player_chat"
        or evidence.get("narration_channel") != "bong:agent_narrate"
        or evidence.get("player") != expected_player
        or evidence.get("raw") != marker
        or evidence.get("observed_ts") != evidence.get("signal_ts")
        or narration.get("scope") != "player"
        or narration.get("target") != expected_player
        or marker not in narration.get("text", "")
        or f"server_ts={evidence.get('observed_ts')}" not in narration.get("text", "")
    ):
        raise BotAssertionError(
            "Redis→Tiandao→narration 适配器证据未保留消息身份、时间戳和 player target；"
            f"实际={evidence}"
        )
    return evidence


def run(env) -> None:
    with env.new_bot("ChatA") as sender:
        sender.expect_event("game_join", timeout=15.0)
        sender.expect_event("pos_look", timeout=15.0)

        with env.new_bot("ChatB") as bystander:
            bystander.expect_event("game_join", timeout=15.0)
            bystander.expect_event("pos_look", timeout=15.0)

            marker = f"rf37-chat-{env.run_tag}-{secrets.token_hex(8)}"
            sender_after = sender.events[-1].t if sender.events else 0.0
            bystander_after = bystander.events[-1].t if bystander.events else 0.0
            sender.chat(marker)
            evidence = _run_agent_roundtrip(sender.username, marker)

            received = sender.wait_for(
                lambda event: _is_marker_narration(
                    event,
                    after=sender_after,
                    marker=marker,
                    target=f"offline:{sender.username}",
                ),
                timeout=NARRATION_TIMEOUT_SECONDS,
                description="目标 Bot 收到带原始 marker 与 server_ts 的 player narration",
            )
            matching = [
                entry
                for entry in received.data["payload"]["narrations"]
                if marker in entry.get("text", "")
            ]
            if len(matching) != 1:
                raise BotAssertionError(
                    f"目标 Bot 应恰好收到 1 条 marker narration，实际 matching={matching} evidence={evidence}"
                )
            if matching[0].get("target") != f"offline:{sender.username}":
                raise BotAssertionError(
                    "回流 narration 必须保持 player target 为发送者 canonical id，"
                    f"实际={matching[0]}"
                )

            # 用旁观者命令回执作有序网络栅栏，随后检查 player scope 没有泄漏。
            fence = bystander.events[-1].t if bystander.events else bystander_after
            bystander.cmd("realm set awaken")
            bystander.wait_for(
                lambda event: (
                    event.kind == "chat"
                    and event.t > fence
                    and "[dev] realm set" in event.data.get("text", "")
                ),
                timeout=10.0,
                description="旁观 Bot 后续命令响应网络栅栏",
            )
            leaked = [
                event
                for event in bystander.events
                if _is_marker_narration(
                    event,
                    after=bystander_after,
                    marker=marker,
                    target=f"offline:{sender.username}",
                )
            ]
            if leaked:
                raise BotAssertionError(
                    "player scope narration 不应泄漏给旁观 Bot；"
                    f"sender={sender.username} bystander={bystander.username} leaked={leaked}"
                )

            sender.assert_alive("完整 Agent 聊天回流后")
            bystander.assert_alive("player scope 隔离断言后")
