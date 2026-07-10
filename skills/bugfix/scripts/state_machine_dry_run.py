#!/usr/bin/env python3
"""BugFix 调度协议的纯内存 dry-run；不访问 GitHub、不修改仓库状态。"""

from __future__ import annotations

import unittest
from dataclasses import dataclass
from typing import Optional


def implementation_limit(user_n: int, platform_total: Optional[int]) -> int:
    """主干占一槽，并永久预留一槽给 validator。"""
    if user_n < 0:
        raise ValueError("user_n must be non-negative")
    available = 2 if platform_total is None else max(0, platform_total - 2)
    return min(user_n, available)


def implementation_admission(
    user_n: int, platform_total: Optional[int]
) -> tuple[int, int]:
    active = implementation_limit(user_n, platform_total)
    return active, user_n - active


def validator_capacity(platform_total: Optional[int], live_agents: int) -> int:
    """容量未知时串行；已知时不超过真实剩余槽和逻辑上限三。"""
    if live_agents < 0:
        raise ValueError("live_agents must be non-negative")
    if platform_total is None:
        return 1
    return min(3, max(0, platform_total - live_agents))


@dataclass(frozen=True)
class RequestPayload:
    request_id: str
    token_type: str
    task: str
    agent: str
    phase: str
    head: str
    checkpoint: str


@dataclass
class RequestState:
    payload: RequestPayload
    state: str = "queued"
    token_id: Optional[str] = None
    ack: bool = False
    release_reason: Optional[str] = None


class TokenBroker:
    """模拟主干唯一 FIFO、grant/ACK/release 和悬挂回收。"""

    def __init__(self, compile_capacity: int = 2, validator_capacity_: int = 3):
        self.capacity = {
            "compile": compile_capacity,
            "validator": validator_capacity_,
        }
        self.requests: dict[str, RequestState] = {}
        self.queues: dict[str, list[str]] = {"compile": [], "validator": []}
        self.next_token = 1

    def request(self, payload: RequestPayload) -> RequestState:
        if payload.token_type not in self.capacity:
            raise ValueError("unknown token type")
        existing = self.requests.get(payload.request_id)
        if existing is not None:
            if existing.payload != payload:
                raise ValueError("request_id reused with different payload")
            return existing
        state = RequestState(payload=payload)
        self.requests[payload.request_id] = state
        self.queues[payload.token_type].append(payload.request_id)
        return state

    def queue_position(self, request_id: str) -> Optional[int]:
        state = self.requests[request_id]
        if state.state != "queued":
            return None
        return self.queues[state.payload.token_type].index(request_id) + 1

    def held(self, token_type: str) -> int:
        return sum(
            state.state in {"granted", "acked"}
            and state.payload.token_type == token_type
            for state in self.requests.values()
        )

    def grant_next(self, token_type: str) -> Optional[RequestState]:
        if self.held(token_type) >= self.capacity[token_type]:
            return None
        queue = self.queues[token_type]
        while queue:
            request_id = queue.pop(0)
            state = self.requests[request_id]
            if state.state != "queued":
                continue
            state.state = "granted"
            state.token_id = f"{token_type}-{self.next_token}"
            self.next_token += 1
            return state
        return None

    def ack(
        self, request_id: str, token_id: str, phase: str, head: str
    ) -> bool:
        state = self.requests[request_id]
        if state.state == "acked" and state.token_id == token_id:
            return True
        if (
            state.state != "granted"
            or state.token_id != token_id
            or state.payload.phase != phase
            or state.payload.head != head
        ):
            self.cancel(request_id, "STALE_GRANT")
            return False
        state.state = "acked"
        state.ack = True
        return True

    def release(self, request_id: str, token_id: str, reason: str) -> bool:
        state = self.requests[request_id]
        if state.state in {"released", "cancelled"}:
            return False
        if state.token_id != token_id:
            raise ValueError("token mismatch")
        state.state = "released"
        state.release_reason = reason
        return True

    def cancel(self, request_id: str, reason: str) -> bool:
        state = self.requests[request_id]
        if state.state in {"released", "cancelled"}:
            return False
        state.state = "cancelled"
        state.release_reason = reason
        return True

    def reclaim_dead(self, live_agents: set[str]) -> list[str]:
        reclaimed: list[str] = []
        for request_id, state in self.requests.items():
            if (
                state.state in {"granted", "acked"}
                and state.payload.agent not in live_agents
            ):
                self.cancel(request_id, "AGENT_LOST")
                reclaimed.append(request_id)
        return reclaimed

    def followup_payload(self, request_id: str) -> dict[str, str]:
        state = self.requests[request_id]
        if state.state not in {"granted", "acked"} or state.token_id is None:
            raise ValueError("request has no resumable grant")
        return {
            "checkpoint": state.payload.checkpoint,
            "request_id": request_id,
            "token_id": state.token_id,
            "phase": state.payload.phase,
            "head": state.payload.head,
        }


def claim_result(
    status: int,
    *,
    ref_exists: bool,
    object_sha: Optional[str] = None,
    expected_sha: Optional[str] = None,
) -> str:
    if status == 201:
        if object_sha != expected_sha:
            return "error-sha-mismatch"
        return "claimed"
    if status == 422:
        return "occupied" if ref_exists else "error-422"
    return "error-http"


def main_sync(origin_is_ancestor: bool, head_is_ancestor: bool) -> str:
    if origin_is_ancestor:
        return "already-up-to-date"
    if head_is_ancestor:
        return "fast-forward"
    return "diverged-no-commit"


def verdict_valid(
    target_head: str, actual_head: str, clean_before: bool, clean_after: bool
) -> bool:
    return target_head == actual_head and clean_before and clean_after


class WaitLoop:
    def __init__(self) -> None:
        self.no_progress_rounds = 0
        self.alerts = 0
        self.running = True

    def wake(self, progress: bool) -> None:
        if progress:
            self.no_progress_rounds = 0
            return
        self.no_progress_rounds += 1
        if self.no_progress_rounds % 3 == 0:
            self.alerts += 1
        self.running = True


def payload(
    request_id: str,
    token_type: str,
    agent: str,
    *,
    phase: str = "GATING",
    head: str = "abc123",
) -> RequestPayload:
    return RequestPayload(
        request_id=request_id,
        token_type=token_type,
        task=f"task-{request_id}",
        agent=agent,
        phase=phase,
        head=head,
        checkpoint=f"checkpoint-{request_id}",
    )


class StateMachineDryRun(unittest.TestCase):
    def test_platform_reserve_and_n_queue(self) -> None:
        self.assertEqual(implementation_limit(9, 4), 2)
        self.assertEqual(implementation_limit(9, 8), 6)
        self.assertEqual(implementation_limit(9, None), 2)
        self.assertEqual(implementation_admission(5, 4), (2, 3))
        self.assertEqual(implementation_admission(1, 4), (1, 0))
        self.assertEqual(validator_capacity(4, 3), 1)
        self.assertEqual(validator_capacity(8, 3), 3)
        self.assertEqual(validator_capacity(None, 99), 1)

    def test_fifo_capacity_request_grant_ack_release(self) -> None:
        broker = TokenBroker(compile_capacity=1)
        first = broker.request(payload("r1", "compile", "agent-a"))
        second = broker.request(payload("r2", "compile", "agent-b"))
        self.assertEqual(broker.queue_position("r1"), 1)
        self.assertEqual(broker.queue_position("r2"), 2)
        self.assertIs(broker.request(first.payload), first)
        grant = broker.grant_next("compile")
        self.assertEqual(grant.payload.request_id, "r1")
        self.assertIsNone(broker.grant_next("compile"))
        self.assertTrue(broker.ack("r1", grant.token_id, "GATING", "abc123"))
        self.assertTrue(broker.release("r1", grant.token_id, "PASS"))
        self.assertFalse(broker.release("r1", grant.token_id, "PASS"))
        self.assertEqual(broker.grant_next("compile").payload.request_id, "r2")

    def test_cancel_and_payload_collision(self) -> None:
        broker = TokenBroker()
        broker.request(payload("r1", "compile", "agent-a"))
        self.assertTrue(broker.cancel("r1", "CANCELLED"))
        self.assertFalse(broker.cancel("r1", "CANCELLED"))
        with self.assertRaises(ValueError):
            broker.request(payload("r1", "validator", "agent-a"))

    def test_stale_grant_is_cancelled(self) -> None:
        broker = TokenBroker()
        broker.request(payload("r1", "validator", "agent-a", phase="VALIDATING"))
        grant = broker.grant_next("validator")
        self.assertFalse(
            broker.ack("r1", grant.token_id, "VALIDATING", "new-head")
        )
        self.assertEqual(broker.requests["r1"].release_reason, "STALE_GRANT")

    def test_validator_pass_fail_timeout_all_release(self) -> None:
        for reason in ("PASS", "FAIL", "TIMEOUT", "ERROR"):
            with self.subTest(reason=reason):
                broker = TokenBroker(validator_capacity_=1)
                broker.request(
                    payload(
                        reason,
                        "validator",
                        "agent-v",
                        phase="VALIDATING",
                    )
                )
                grant = broker.grant_next("validator")
                self.assertTrue(
                    broker.ack(reason, grant.token_id, "VALIDATING", "abc123")
                )
                self.assertTrue(broker.release(reason, grant.token_id, reason))
                self.assertEqual(broker.held("validator"), 0)

    def test_agent_crash_recovery_and_reclaim(self) -> None:
        broker = TokenBroker()
        broker.request(payload("r1", "compile", "agent-a"))
        grant = broker.grant_next("compile")
        recovery = broker.followup_payload("r1")
        self.assertEqual(recovery["token_id"], grant.token_id)
        self.assertEqual(recovery["checkpoint"], "checkpoint-r1")
        self.assertEqual(broker.reclaim_dead({"agent-b"}), ["r1"])
        self.assertEqual(broker.held("compile"), 0)

    def test_head_or_worktree_change_invalidates_verdict(self) -> None:
        self.assertTrue(verdict_valid("a", "a", True, True))
        self.assertFalse(verdict_valid("a", "b", True, True))
        self.assertFalse(verdict_valid("a", "a", False, True))
        self.assertFalse(verdict_valid("a", "a", True, False))

    def test_claim_201_and_422(self) -> None:
        self.assertEqual(
            claim_result(
                201, ref_exists=False, object_sha="a", expected_sha="a"
            ),
            "claimed",
        )
        self.assertEqual(
            claim_result(
                201, ref_exists=False, object_sha="b", expected_sha="a"
            ),
            "error-sha-mismatch",
        )
        self.assertEqual(claim_result(422, ref_exists=True), "occupied")
        self.assertEqual(claim_result(422, ref_exists=False), "error-422")

    def test_three_main_sync_paths(self) -> None:
        self.assertEqual(main_sync(True, False), "already-up-to-date")
        self.assertEqual(main_sync(False, True), "fast-forward")
        self.assertEqual(main_sync(False, False), "diverged-no-commit")

    def test_three_no_progress_rounds_alert_but_continue(self) -> None:
        loop = WaitLoop()
        for _ in range(6):
            loop.wake(progress=False)
        self.assertTrue(loop.running)
        self.assertEqual(loop.alerts, 2)
        loop.wake(progress=True)
        self.assertEqual(loop.no_progress_rounds, 0)


if __name__ == "__main__":
    result = unittest.main(verbosity=2, exit=False).result
    if not result.wasSuccessful():
        raise SystemExit(1)
    print("DRY-RUN PASS: BugFix 状态机契约全部通过")
