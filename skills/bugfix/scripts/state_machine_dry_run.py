#!/usr/bin/env python3
"""BugFix 调度协议纯内存 dry-run；不访问 GitHub、不修改仓库。"""

from __future__ import annotations

import unittest
from dataclasses import dataclass
from enum import Enum
from typing import Optional


class ProtocolError(RuntimeError):
    pass


class TaskPhase(str, Enum):
    DISPATCHED = "DISPATCHED"
    CLAIMED = "CLAIMED"
    PROMOTED = "PROMOTED"
    VERIFYING = "VERIFYING"
    FIXING = "FIXING"
    NOT_BUG = "NOT_BUG"
    VALIDATING = "VALIDATING"
    GATING = "GATING"
    REBASING = "REBASING"
    ARCHIVING = "ARCHIVING"
    PR_OPEN = "PR_OPEN"
    GATES = "GATES"
    RECOVERING = "RECOVERING"
    BLOCKED = "BLOCKED"
    CLOSED = "CLOSED"


BRANCH_PHASES = {TaskPhase.FIXING, TaskPhase.NOT_BUG}
TERMINAL_PHASES = {TaskPhase.BLOCKED, TaskPhase.CLOSED}
ALLOWED_EDGES: dict[TaskPhase, set[TaskPhase]] = {
    TaskPhase.DISPATCHED: {TaskPhase.CLAIMED, TaskPhase.BLOCKED},
    TaskPhase.CLAIMED: {TaskPhase.PROMOTED, TaskPhase.BLOCKED},
    TaskPhase.PROMOTED: {TaskPhase.VERIFYING, TaskPhase.BLOCKED},
    TaskPhase.VERIFYING: {
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.BLOCKED,
    },
    TaskPhase.FIXING: {TaskPhase.VALIDATING, TaskPhase.BLOCKED},
    TaskPhase.NOT_BUG: {TaskPhase.VALIDATING, TaskPhase.BLOCKED},
    TaskPhase.VALIDATING: {
        TaskPhase.GATING,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.PR_OPEN,
        TaskPhase.BLOCKED,
    },
    TaskPhase.GATING: {
        TaskPhase.REBASING,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.BLOCKED,
    },
    TaskPhase.REBASING: {
        TaskPhase.VALIDATING,
        TaskPhase.ARCHIVING,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.BLOCKED,
    },
    TaskPhase.ARCHIVING: {
        TaskPhase.VALIDATING,
        TaskPhase.PR_OPEN,
        TaskPhase.BLOCKED,
    },
    TaskPhase.PR_OPEN: {TaskPhase.GATES, TaskPhase.BLOCKED},
    TaskPhase.GATES: {
        TaskPhase.CLOSED,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.BLOCKED,
    },
    TaskPhase.RECOVERING: set(),
    TaskPhase.BLOCKED: set(),
    TaskPhase.CLOSED: set(),
}


@dataclass
class TaskState:
    phase: TaskPhase = TaskPhase.DISPATCHED
    resolution: Optional[TaskPhase] = None
    recovery_from: Optional[TaskPhase] = None

    def transition(self, target: TaskPhase) -> None:
        if self.phase in TERMINAL_PHASES:
            raise ProtocolError("terminal phase cannot transition")
        if target not in ALLOWED_EDGES[self.phase]:
            raise ProtocolError(f"illegal transition {self.phase}->{target}")
        if self.phase == TaskPhase.VERIFYING and target in BRANCH_PHASES:
            self.resolution = target
        elif target in BRANCH_PHASES and self.resolution != target:
            raise ProtocolError("FIXING and NOT_BUG are mutually exclusive")
        self.phase = target

    def mark_recovering(self) -> None:
        if self.phase in TERMINAL_PHASES:
            raise ProtocolError("terminal task cannot recover")
        if self.phase == TaskPhase.RECOVERING:
            return
        self.recovery_from = self.phase
        self.phase = TaskPhase.RECOVERING

    def recovery_result(self, success: bool) -> None:
        if self.phase != TaskPhase.RECOVERING or self.recovery_from is None:
            raise ProtocolError("task is not recovering")
        self.phase = self.recovery_from if success else TaskPhase.BLOCKED
        self.recovery_from = None


@dataclass
class ManualClock:
    value: float = 0.0

    def now(self) -> float:
        return self.value

    def advance(self, seconds: float) -> None:
        if seconds < 0:
            raise ValueError("clock cannot move backwards")
        self.value += seconds


@dataclass
class PlatformSnapshot:
    total: Optional[int]
    live_agents: int
    validator_reserve: int = 1

    def implementation_limit(self, user_n: int) -> int:
        available = (
            2
            if self.total is None
            else max(0, self.total - 1 - self.validator_reserve)
        )
        return min(user_n, available)

    def validator_slots(self) -> int:
        if self.total is None:
            return 1
        if self.validator_reserve < 1:
            return 0
        return min(3, max(0, self.total - self.live_agents))


@dataclass
class MutableCapacityProvider:
    snapshot: PlatformSnapshot

    def __call__(self) -> PlatformSnapshot:
        return self.snapshot


class RequestStatus(str, Enum):
    QUEUED = "QUEUED"
    GRANTED = "GRANTED"
    ACKED = "ACKED"
    RECOVERING = "RECOVERING"
    RELEASED = "RELEASED"
    CANCELLED = "CANCELLED"
    EXPIRED = "EXPIRED"


@dataclass(frozen=True)
class RequestPayload:
    request_id: str
    token_type: str
    task: str
    agent: str
    phase: TaskPhase
    head: str
    checkpoint: str


@dataclass
class RequestState:
    payload: RequestPayload
    status: RequestStatus = RequestStatus.QUEUED
    token_id: Optional[str] = None
    granted_at: Optional[float] = None
    expires_at: Optional[float] = None
    release_reason: Optional[str] = None
    pre_recovery_status: Optional[RequestStatus] = None
    recovery_deadline: Optional[float] = None


TOKEN_PHASES = {
    "compile": {TaskPhase.GATING, TaskPhase.REBASING},
    "validator": {TaskPhase.VALIDATING},
}
ACTIVE_TOKEN_STATUSES = {
    RequestStatus.GRANTED,
    RequestStatus.ACKED,
    RequestStatus.RECOVERING,
}


class TokenBroker:
    def __init__(
        self,
        capacity_provider: MutableCapacityProvider,
        clock: ManualClock,
        *,
        grant_ttl: float = 30.0,
        recovery_ttl: float = 60.0,
        compile_capacity: int = 2,
        validator_capacity: int = 3,
    ):
        self.capacity_provider = capacity_provider
        self.clock = clock
        self.grant_ttl = grant_ttl
        self.recovery_ttl = recovery_ttl
        self.logical_capacity = {
            "compile": compile_capacity,
            "validator": validator_capacity,
        }
        self.requests: dict[str, RequestState] = {}
        self.queues: dict[str, list[str]] = {"compile": [], "validator": []}
        self.invalid_tokens: set[str] = set()
        self.next_token = 1

    def request(self, payload: RequestPayload, task: TaskState) -> RequestState:
        if payload.token_type not in TOKEN_PHASES:
            raise ProtocolError("unknown token type")
        if payload.phase != task.phase:
            raise ProtocolError("request phase differs from task phase")
        if payload.phase not in TOKEN_PHASES[payload.token_type]:
            raise ProtocolError("token is not allowed in this phase")
        existing = self.requests.get(payload.request_id)
        if existing is not None:
            if existing.payload != payload:
                raise ProtocolError("request_id payload collision")
            return existing
        state = RequestState(payload=payload)
        self.requests[payload.request_id] = state
        self.queues[payload.token_type].append(payload.request_id)
        return state

    def queue_position(self, request_id: str) -> Optional[int]:
        state = self.requests[request_id]
        if state.status != RequestStatus.QUEUED:
            return None
        return self.queues[state.payload.token_type].index(request_id) + 1

    def held(self, token_type: str) -> int:
        return sum(
            state.payload.token_type == token_type
            and state.status in ACTIVE_TOKEN_STATUSES
            for state in self.requests.values()
        )

    def available(self, token_type: str) -> int:
        logical = self.logical_capacity[token_type] - self.held(token_type)
        if token_type == "validator":
            platform = self.capacity_provider().validator_slots()
            return max(0, min(logical, platform))
        return max(0, logical)

    def grant_next(self, token_type: str) -> Optional[RequestState]:
        self.expire_grants()
        self.sweep_recovery()
        if self.available(token_type) <= 0:
            return None
        queue = self.queues[token_type]
        while queue:
            request_id = queue.pop(0)
            state = self.requests[request_id]
            if state.status != RequestStatus.QUEUED:
                continue
            token_id = f"{token_type}-{self.next_token}"
            self.next_token += 1
            now = self.clock.now()
            state.status = RequestStatus.GRANTED
            state.token_id = token_id
            state.granted_at = now
            state.expires_at = now + self.grant_ttl
            return state
        return None

    def ack(
        self, request_id: str, token_id: str, phase: TaskPhase, head: str
    ) -> bool:
        self.expire_grants()
        state = self.requests[request_id]
        if state.status == RequestStatus.ACKED and state.token_id == token_id:
            return True
        if token_id in self.invalid_tokens:
            raise ProtocolError("old token rejected")
        if state.status != RequestStatus.GRANTED or state.token_id != token_id:
            raise ProtocolError("grant is not active")
        if state.payload.phase != phase or state.payload.head != head:
            self._invalidate(state, RequestStatus.CANCELLED, "STALE_GRANT")
            raise ProtocolError("grant phase/head mismatch")
        state.status = RequestStatus.ACKED
        return True

    def release(self, request_id: str, token_id: str, reason: str) -> bool:
        state = self.requests[request_id]
        if state.status == RequestStatus.RELEASED:
            return False
        if token_id in self.invalid_tokens:
            raise ProtocolError("late release rejected")
        if state.status != RequestStatus.ACKED or state.token_id != token_id:
            raise ProtocolError("release requires ACKED active token")
        self._invalidate(state, RequestStatus.RELEASED, reason)
        return True

    def cancel(self, request_id: str, reason: str) -> bool:
        state = self.requests[request_id]
        if state.status in {
            RequestStatus.RELEASED,
            RequestStatus.CANCELLED,
            RequestStatus.EXPIRED,
        }:
            return False
        self._invalidate(state, RequestStatus.CANCELLED, reason)
        return True

    def expire_grants(self) -> list[str]:
        expired: list[str] = []
        now = self.clock.now()
        for request_id, state in self.requests.items():
            if (
                state.status == RequestStatus.GRANTED
                and state.expires_at is not None
                and now >= state.expires_at
            ):
                self._invalidate(state, RequestStatus.EXPIRED, "GRANT_TTL")
                expired.append(request_id)
        return expired

    def mark_lost(self, request_id: str) -> bool:
        state = self.requests[request_id]
        if state.status == RequestStatus.RECOVERING:
            return False
        if state.status not in {RequestStatus.GRANTED, RequestStatus.ACKED}:
            raise ProtocolError("only active holder can recover")
        state.pre_recovery_status = state.status
        state.status = RequestStatus.RECOVERING
        state.recovery_deadline = self.clock.now() + self.recovery_ttl
        return True

    def recovery_result(self, request_id: str, success: bool) -> None:
        state = self.requests[request_id]
        if state.status != RequestStatus.RECOVERING:
            raise ProtocolError("request is not recovering")
        if success:
            state.status = state.pre_recovery_status or RequestStatus.GRANTED
            state.pre_recovery_status = None
            state.recovery_deadline = None
            return
        self._invalidate(state, RequestStatus.CANCELLED, "RECOVERY_FAILED")

    def sweep_recovery(self) -> list[str]:
        reclaimed: list[str] = []
        now = self.clock.now()
        for request_id, state in self.requests.items():
            if (
                state.status == RequestStatus.RECOVERING
                and state.recovery_deadline is not None
                and now >= state.recovery_deadline
            ):
                self._invalidate(
                    state, RequestStatus.CANCELLED, "RECOVERY_TIMEOUT"
                )
                reclaimed.append(request_id)
        return reclaimed

    def followup_payload(self, request_id: str) -> dict[str, str]:
        state = self.requests[request_id]
        if state.status != RequestStatus.RECOVERING or state.token_id is None:
            raise ProtocolError("request has no recovery checkpoint")
        return {
            "checkpoint": state.payload.checkpoint,
            "request_id": request_id,
            "token_id": state.token_id,
            "phase": state.payload.phase.value,
            "head": state.payload.head,
        }

    def _invalidate(
        self, state: RequestState, status: RequestStatus, reason: str
    ) -> None:
        if state.token_id is not None:
            self.invalid_tokens.add(state.token_id)
        state.status = status
        state.release_reason = reason
        state.pre_recovery_status = None
        state.recovery_deadline = None


def claim_result(
    status: int,
    *,
    reason: Optional[dict[str, str]],
    ref_exists: bool,
    object_sha: Optional[str] = None,
    expected_sha: Optional[str] = None,
) -> str:
    if status == 201:
        return "claimed" if object_sha == expected_sha else "error-sha-mismatch"
    if status != 422:
        return "error-http"
    already_exists = (
        isinstance(reason, dict)
        and reason.get("resource") == "Reference"
        and reason.get("field") == "ref"
        and reason.get("code") == "already_exists"
    )
    return "occupied" if already_exists and ref_exists else "error-422"


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


def make_payload(
    request_id: str,
    token_type: str,
    agent: str,
    phase: TaskPhase,
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


def fixture(
    *,
    total: Optional[int] = 4,
    live: int = 3,
    grant_ttl: float = 30.0,
    recovery_ttl: float = 60.0,
) -> tuple[TokenBroker, MutableCapacityProvider, ManualClock]:
    clock = ManualClock()
    provider = MutableCapacityProvider(PlatformSnapshot(total, live))
    return (
        TokenBroker(
            provider,
            clock,
            grant_ttl=grant_ttl,
            recovery_ttl=recovery_ttl,
        ),
        provider,
        clock,
    )


class StateMachineDryRun(unittest.TestCase):
    def test_all_allowed_phase_edges(self) -> None:
        for source, targets in ALLOWED_EDGES.items():
            for target in targets:
                with self.subTest(source=source, target=target):
                    task = TaskState(phase=source)
                    if source not in {TaskPhase.VERIFYING}:
                        task.resolution = (
                            target if target in BRANCH_PHASES else TaskPhase.FIXING
                        )
                    task.transition(target)
                    self.assertEqual(task.phase, target)

    def test_illegal_mutual_and_terminal_phase_edges(self) -> None:
        with self.assertRaises(ProtocolError):
            TaskState().transition(TaskPhase.CLOSED)
        task = TaskState(phase=TaskPhase.VERIFYING)
        task.transition(TaskPhase.FIXING)
        task.transition(TaskPhase.VALIDATING)
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.NOT_BUG)
        for phase in TERMINAL_PHASES:
            with self.assertRaises(ProtocolError):
                TaskState(phase=phase).transition(TaskPhase.DISPATCHED)

    def test_rework_and_task_recovery(self) -> None:
        task = TaskState(phase=TaskPhase.GATES, resolution=TaskPhase.FIXING)
        task.transition(TaskPhase.FIXING)
        task.mark_recovering()
        self.assertEqual(task.phase, TaskPhase.RECOVERING)
        task.recovery_result(True)
        self.assertEqual(task.phase, TaskPhase.FIXING)
        task.mark_recovering()
        task.recovery_result(False)
        self.assertEqual(task.phase, TaskPhase.BLOCKED)

    def test_platform_reserve_and_dynamic_validator_capacity(self) -> None:
        snapshot = PlatformSnapshot(total=4, live_agents=3)
        self.assertEqual(snapshot.implementation_limit(9), 2)
        self.assertEqual(snapshot.validator_slots(), 1)
        self.assertEqual(
            PlatformSnapshot(4, 3, validator_reserve=0).validator_slots(), 0
        )
        self.assertEqual(
            PlatformSnapshot(None, 99).implementation_limit(9), 2
        )
        broker, provider, _ = fixture(total=4, live=4)
        task = TaskState(phase=TaskPhase.VALIDATING)
        broker.request(
            make_payload("v1", "validator", "agent-a", task.phase), task
        )
        self.assertIsNone(broker.grant_next("validator"))
        provider.snapshot.live_agents = 3
        self.assertIsNotNone(broker.grant_next("validator"))

    def test_validator_capacity_shrinks_and_queue_recovers(self) -> None:
        broker, provider, _ = fixture(total=5, live=3)
        task = TaskState(phase=TaskPhase.VALIDATING)
        for request_id in ("v1", "v2", "v3"):
            broker.request(
                make_payload(request_id, "validator", request_id, task.phase),
                task,
            )
        first = broker.grant_next("validator")
        provider.snapshot.live_agents = 5
        self.assertIsNone(broker.grant_next("validator"))
        broker.ack("v1", first.token_id, task.phase, "abc123")
        broker.release("v1", first.token_id, "PASS")
        provider.snapshot.live_agents = 4
        self.assertEqual(
            broker.grant_next("validator").payload.request_id, "v2"
        )

    def test_fifo_request_ack_release_cancel_and_collision(self) -> None:
        broker, _, _ = fixture()
        task = TaskState(phase=TaskPhase.GATING)
        first_payload = make_payload("r1", "compile", "a", task.phase)
        first = broker.request(first_payload, task)
        broker.request(make_payload("r2", "compile", "b", task.phase), task)
        self.assertEqual(broker.queue_position("r1"), 1)
        self.assertEqual(broker.queue_position("r2"), 2)
        self.assertIs(broker.request(first_payload, task), first)
        grant = broker.grant_next("compile")
        with self.assertRaises(ProtocolError):
            broker.release("r1", grant.token_id, "PASS")
        broker.ack("r1", grant.token_id, task.phase, "abc123")
        self.assertTrue(broker.release("r1", grant.token_id, "PASS"))
        self.assertFalse(broker.release("r1", grant.token_id, "PASS"))
        self.assertTrue(broker.cancel("r2", "CANCELLED"))
        with self.assertRaises(ProtocolError):
            broker.request(
                make_payload("r1", "validator", "a", TaskPhase.VALIDATING),
                TaskState(phase=TaskPhase.VALIDATING),
            )

    def test_token_phase_binding(self) -> None:
        broker, _, _ = fixture()
        with self.assertRaises(ProtocolError):
            broker.request(
                make_payload("x", "compile", "a", TaskPhase.FIXING),
                TaskState(phase=TaskPhase.FIXING),
            )
        with self.assertRaises(ProtocolError):
            broker.request(
                make_payload("y", "validator", "a", TaskPhase.VALIDATING),
                TaskState(phase=TaskPhase.GATING),
            )

    def test_grant_ttl_before_boundary_ack(self) -> None:
        broker, _, clock = fixture(grant_ttl=10)
        task = TaskState(phase=TaskPhase.VALIDATING)
        broker.request(
            make_payload("v1", "validator", "a", task.phase), task
        )
        grant = broker.grant_next("validator")
        clock.advance(9.999)
        self.assertTrue(
            broker.ack("v1", grant.token_id, task.phase, "abc123")
        )

    def test_grant_ttl_boundary_expiry_fifo_and_old_token(self) -> None:
        broker, _, clock = fixture(grant_ttl=10)
        task = TaskState(phase=TaskPhase.VALIDATING)
        for request_id in ("v1", "v2"):
            broker.request(
                make_payload(request_id, "validator", request_id, task.phase),
                task,
            )
        old = broker.grant_next("validator")
        clock.advance(10)
        self.assertEqual(broker.expire_grants(), ["v1"])
        self.assertEqual(broker.expire_grants(), [])
        with self.assertRaises(ProtocolError):
            broker.ack("v1", old.token_id, task.phase, "abc123")
        with self.assertRaises(ProtocolError):
            broker.release("v1", old.token_id, "PASS")
        self.assertEqual(
            broker.grant_next("validator").payload.request_id, "v2"
        )

    def test_validator_all_exit_reasons_release(self) -> None:
        for reason in ("PASS", "FAIL", "TIMEOUT", "ERROR", "CANCEL"):
            with self.subTest(reason=reason):
                broker, _, _ = fixture()
                task = TaskState(phase=TaskPhase.VALIDATING)
                broker.request(
                    make_payload(reason, "validator", "v", task.phase), task
                )
                grant = broker.grant_next("validator")
                broker.ack(reason, grant.token_id, task.phase, "abc123")
                broker.release(reason, grant.token_id, reason)
                self.assertEqual(broker.held("validator"), 0)

    def test_recovery_success_failure_timeout_and_late_messages(self) -> None:
        broker, _, clock = fixture(recovery_ttl=10)
        task = TaskState(phase=TaskPhase.GATING)
        for request_id in ("ok", "fail", "timeout"):
            broker.request(
                make_payload(request_id, "compile", request_id, task.phase),
                task,
            )
            grant = broker.grant_next("compile")
            broker.ack(request_id, grant.token_id, task.phase, "abc123")
            broker.mark_lost(request_id)
            if request_id == "ok":
                self.assertEqual(
                    broker.followup_payload(request_id)["token_id"],
                    grant.token_id,
                )
                broker.recovery_result(request_id, True)
                broker.release(request_id, grant.token_id, "PASS")
            elif request_id == "fail":
                broker.recovery_result(request_id, False)
                with self.assertRaises(ProtocolError):
                    broker.release(request_id, grant.token_id, "LATE")
            else:
                clock.advance(10)
                self.assertEqual(broker.sweep_recovery(), ["timeout"])
                with self.assertRaises(ProtocolError):
                    broker.ack(
                        request_id, grant.token_id, task.phase, "abc123"
                    )

    def test_repeated_recovery_is_idempotent(self) -> None:
        broker, _, _ = fixture()
        task = TaskState(phase=TaskPhase.GATING)
        broker.request(make_payload("r", "compile", "a", task.phase), task)
        grant = broker.grant_next("compile")
        broker.ack("r", grant.token_id, task.phase, "abc123")
        self.assertTrue(broker.mark_lost("r"))
        self.assertFalse(broker.mark_lost("r"))

    def test_head_or_worktree_change_invalidates_verdict(self) -> None:
        self.assertTrue(verdict_valid("a", "a", True, True))
        self.assertFalse(verdict_valid("a", "b", True, True))
        self.assertFalse(verdict_valid("a", "a", False, True))
        self.assertFalse(verdict_valid("a", "a", True, False))

    def test_claim_201_and_structured_422(self) -> None:
        duplicate = {
            "resource": "Reference",
            "field": "ref",
            "code": "already_exists",
        }
        invalid_sha = {
            "resource": "Reference",
            "field": "sha",
            "code": "invalid",
        }
        permission = {
            "resource": "Repository",
            "field": "permission",
            "code": "denied",
        }
        invalid_payload = {
            "resource": "Reference",
            "field": "ref",
            "code": "invalid",
        }
        self.assertEqual(
            claim_result(
                201,
                reason=None,
                ref_exists=False,
                object_sha="a",
                expected_sha="a",
            ),
            "claimed",
        )
        self.assertEqual(
            claim_result(422, reason=duplicate, ref_exists=True), "occupied"
        )
        self.assertEqual(
            claim_result(422, reason=duplicate, ref_exists=False), "error-422"
        )
        self.assertEqual(
            claim_result(422, reason=invalid_sha, ref_exists=True), "error-422"
        )
        self.assertEqual(
            claim_result(422, reason=permission, ref_exists=True), "error-422"
        )
        self.assertEqual(
            claim_result(422, reason=invalid_payload, ref_exists=True),
            "error-422",
        )
        self.assertEqual(
            claim_result(422, reason=None, ref_exists=True), "error-422"
        )

    def test_three_main_sync_paths(self) -> None:
        self.assertEqual(main_sync(True, False), "already-up-to-date")
        self.assertEqual(main_sync(False, True), "fast-forward")
        self.assertEqual(main_sync(False, False), "diverged-no-commit")

    def test_three_no_progress_rounds_alert_but_continue(self) -> None:
        loop = WaitLoop()
        for _ in range(6):
            loop.wake(False)
        self.assertTrue(loop.running)
        self.assertEqual(loop.alerts, 2)
        loop.wake(True)
        self.assertEqual(loop.no_progress_rounds, 0)


if __name__ == "__main__":
    result = unittest.main(verbosity=2, exit=False).result
    if not result.wasSuccessful():
        raise SystemExit(1)
    print("DRY-RUN PASS: BugFix 状态机契约全部通过")
