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
    FIX_VALIDATING = "FIX_VALIDATING"
    GATING = "GATING"
    REBASING = "REBASING"
    REBASE_VALIDATING = "REBASE_VALIDATING"
    ARCHIVING = "ARCHIVING"
    FINAL_VALIDATING = "FINAL_VALIDATING"
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
    TaskPhase.FIXING: {TaskPhase.FIX_VALIDATING, TaskPhase.BLOCKED},
    TaskPhase.NOT_BUG: {TaskPhase.FIX_VALIDATING, TaskPhase.BLOCKED},
    TaskPhase.FIX_VALIDATING: {
        TaskPhase.GATING,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.BLOCKED,
    },
    TaskPhase.GATING: {
        TaskPhase.REBASING,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.BLOCKED,
    },
    TaskPhase.REBASING: {
        TaskPhase.REBASE_VALIDATING,
        TaskPhase.BLOCKED,
    },
    TaskPhase.REBASE_VALIDATING: {
        TaskPhase.ARCHIVING,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.BLOCKED,
    },
    TaskPhase.ARCHIVING: {
        TaskPhase.FINAL_VALIDATING,
        TaskPhase.BLOCKED,
    },
    TaskPhase.FINAL_VALIDATING: {
        TaskPhase.PR_OPEN,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
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
class ValidatorEvidence:
    validator_id: str
    phase: TaskPhase
    target_sha: str
    generation: int
    verdict: str
    completed_at: float


@dataclass
class GateEvidence:
    phase: TaskPhase
    target_sha: str
    generation: int
    success: bool


@dataclass
class SyncEvidence:
    target_sha: str
    generation: int
    success: bool


@dataclass
class TaskState:
    task_id: str = "task"
    phase: TaskPhase = TaskPhase.DISPATCHED
    head: str = "abc123"
    generation: int = 0
    resolution: Optional[TaskPhase] = None
    recovery_from: Optional[TaskPhase] = None
    fix_validated: bool = False
    gated: bool = False
    rebased: bool = False
    rebase_validated: bool = False
    archived: bool = False
    final_validated: bool = False
    validator_evidence: dict[TaskPhase, ValidatorEvidence] = None
    gate_evidence: dict[TaskPhase, GateEvidence] = None
    sync_evidence: Optional[SyncEvidence] = None

    def __post_init__(self) -> None:
        if self.validator_evidence is None:
            self.validator_evidence = {}
        if self.gate_evidence is None:
            self.gate_evidence = {}

    def transition(self, target: TaskPhase) -> None:
        if self.phase in TERMINAL_PHASES:
            raise ProtocolError("terminal phase cannot transition")
        if target not in ALLOWED_EDGES[self.phase]:
            raise ProtocolError(f"illegal transition {self.phase}->{target}")
        if self.phase == TaskPhase.VERIFYING and target in BRANCH_PHASES:
            self.resolution = target
        elif target in BRANCH_PHASES and self.resolution != target:
            raise ProtocolError("FIXING and NOT_BUG are mutually exclusive")
        if target in BRANCH_PHASES:
            self._reset_closure_milestones()
        if self.phase == TaskPhase.FIX_VALIDATING and target == TaskPhase.GATING:
            self._require_validator_pass(TaskPhase.FIX_VALIDATING)
            self.fix_validated = True
        if self.phase == TaskPhase.GATING and target == TaskPhase.REBASING:
            self._require_gate_pass(TaskPhase.GATING)
            self.gated = True
        if (
            self.phase == TaskPhase.REBASING
            and target == TaskPhase.REBASE_VALIDATING
        ):
            if (
                self.sync_evidence is None
                or not self.sync_evidence.success
                or self.sync_evidence.target_sha != self.head
                or self.sync_evidence.generation != self.generation
            ):
                raise ProtocolError("main sync evidence missing for current HEAD")
            self._require_gate_pass(TaskPhase.REBASING)
            self.rebased = True
            self.gated = True
        if (
            self.phase == TaskPhase.REBASE_VALIDATING
            and target == TaskPhase.ARCHIVING
        ):
            self._require_validator_pass(TaskPhase.REBASE_VALIDATING)
            self.rebase_validated = True
        if (
            self.phase == TaskPhase.ARCHIVING
            and target == TaskPhase.FINAL_VALIDATING
        ):
            if not self.archived:
                raise ProtocolError("archive commit evidence missing")
            self.archived = True
        if (
            self.phase == TaskPhase.FINAL_VALIDATING
            and target == TaskPhase.PR_OPEN
        ):
            self._require_validator_pass(TaskPhase.FINAL_VALIDATING)
            if not all(
                (
                    self.fix_validated,
                    self.gated,
                    self.rebased,
                    self.rebase_validated,
                    self.archived,
                )
            ):
                raise ProtocolError("PR_OPEN closure prerequisites missing")
            self.final_validated = True
        self.phase = target

    def _reset_closure_milestones(self) -> None:
        self.fix_validated = False
        self.gated = False
        self.rebased = False
        self.rebase_validated = False
        self.archived = False
        self.final_validated = False
        self.validator_evidence.clear()
        self.gate_evidence.clear()
        self.sync_evidence = None

    def update_head(self, new_head: str) -> None:
        if new_head == self.head:
            return
        self.head = new_head
        self.generation += 1
        self._reset_closure_milestones()

    def record_validator(self, evidence: ValidatorEvidence) -> None:
        if evidence.phase != self.phase:
            raise ProtocolError("validator evidence phase mismatch")
        if evidence.target_sha != self.head:
            raise ProtocolError("validator evidence HEAD mismatch")
        if evidence.generation != self.generation:
            raise ProtocolError("validator evidence generation mismatch")
        if evidence.verdict != "PASS":
            raise ProtocolError("validator did not PASS")
        self.validator_evidence[self.phase] = evidence

    def record_gate(self, evidence: GateEvidence) -> None:
        if evidence.phase != self.phase or evidence.target_sha != self.head:
            raise ProtocolError("gate evidence phase/HEAD mismatch")
        if evidence.generation != self.generation:
            raise ProtocolError("gate evidence generation mismatch")
        if not evidence.success:
            raise ProtocolError("gate failed")
        self.gate_evidence[self.phase] = evidence

    def record_sync(self, evidence: SyncEvidence) -> None:
        if (
            self.phase != TaskPhase.REBASING
            or evidence.target_sha != self.head
            or evidence.generation != self.generation
            or not evidence.success
        ):
            raise ProtocolError("sync evidence phase/HEAD mismatch")
        self.sync_evidence = evidence

    def record_rebase_head(self, rebased_head: str) -> None:
        if self.phase != TaskPhase.REBASING or rebased_head == self.head:
            raise ProtocolError("rebase HEAD update is not applicable")
        self.head = rebased_head
        self.generation += 1
        self.validator_evidence.clear()
        self.gate_evidence.clear()
        self.sync_evidence = None
        self.gated = False
        self.rebased = False
        self.rebase_validated = False
        self.archived = False
        self.final_validated = False

    def record_archive(self, archived_head: str) -> None:
        if self.phase != TaskPhase.ARCHIVING or archived_head == self.head:
            raise ProtocolError("archive must create a new HEAD")
        self.head = archived_head
        self.generation += 1
        self.validator_evidence.clear()
        self.gate_evidence.clear()
        self.sync_evidence = None
        self.archived = True

    def _require_validator_pass(self, phase: TaskPhase) -> None:
        evidence = self.validator_evidence.get(phase)
        if evidence is None or evidence.target_sha != self.head:
            raise ProtocolError("current HEAD validator PASS missing")
        if evidence.generation != self.generation:
            raise ProtocolError("current generation validator PASS missing")

    def _require_gate_pass(self, phase: TaskPhase) -> None:
        evidence = self.gate_evidence.get(phase)
        if evidence is None or evidence.target_sha != self.head:
            raise ProtocolError("current HEAD gate PASS missing")
        if evidence.generation != self.generation:
            raise ProtocolError("current generation gate PASS missing")

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
    main_in_snapshot: bool = True
    live_agent_ids: frozenset[str] = frozenset()

    def implementation_limit(
        self,
        user_n: int,
        active_implementations: int = 0,
        outstanding_reservations: int = 0,
    ) -> int:
        task_slots = max(0, user_n - active_implementations)
        if self.total is None:
            return task_slots
        occupied = self.live_agents + (0 if self.main_in_snapshot else 1)
        available = max(
            0,
            self.total
            - occupied
            - outstanding_reservations
            - self.validator_reserve,
        )
        return min(task_slots, available)

    def validator_slots(self, outstanding_reservations: int = 0) -> int:
        if self.total is None:
            return max(0, 1 - outstanding_reservations)
        if self.validator_reserve < 1:
            return 0
        occupied = self.live_agents + (0 if self.main_in_snapshot else 1)
        return min(
            3,
            max(0, self.total - occupied - outstanding_reservations),
        )


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
    generation: int
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
    platform_accounted: bool = False
    validator_agent_id: Optional[str] = None


TOKEN_PHASES = {
    "compile": {
        TaskPhase.VERIFYING,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.GATING,
        TaskPhase.REBASING,
    },
    "validator": {
        TaskPhase.FIX_VALIDATING,
        TaskPhase.REBASE_VALIDATING,
        TaskPhase.FINAL_VALIDATING,
    },
}
ACTIVE_TOKEN_STATUSES = {
    RequestStatus.GRANTED,
    RequestStatus.ACKED,
    RequestStatus.RECOVERING,
}


class TaskAuthority:
    """控制面持有的权威状态；请求方只有只读查询权。"""

    def __init__(self) -> None:
        self._tasks: dict[str, TaskState] = {}

    def register(self, task: TaskState) -> None:
        existing = self._tasks.get(task.task_id)
        if existing is not None and existing is not task:
            raise ProtocolError("authoritative task cannot be replaced")
        self._tasks[task.task_id] = task

    def get(self, task_id: str) -> Optional[TaskState]:
        return self._tasks.get(task_id)


class TokenBroker:
    def __init__(
        self,
        capacity_provider: MutableCapacityProvider,
        clock: ManualClock,
        authority: TaskAuthority,
        *,
        grant_ttl: float = 30.0,
        recovery_ttl: float = 60.0,
        compile_capacity: int = 2,
        validator_capacity: int = 3,
    ):
        self.capacity_provider = capacity_provider
        self.clock = clock
        self.authority = authority
        self.grant_ttl = grant_ttl
        self.recovery_ttl = recovery_ttl
        self.logical_capacity = {
            "compile": compile_capacity,
            "validator": validator_capacity,
        }
        self.requests: dict[str, RequestState] = {}
        self.queues: dict[str, list[str]] = {"compile": [], "validator": []}
        self.validator_agents: dict[str, str] = {}
        self.invalid_tokens: set[str] = set()
        self.next_token = 1

    def request(self, payload: RequestPayload) -> RequestState:
        if payload.token_type not in TOKEN_PHASES:
            raise ProtocolError("unknown token type")
        task = self.authority.get(payload.task)
        if task is None or (
            payload.phase != task.phase
            or payload.head != task.head
            or payload.generation != task.generation
        ):
            raise ProtocolError("request differs from authoritative task state")
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

    def implementation_limit(
        self, user_n: int, active_implementations: int = 0
    ) -> int:
        outstanding = sum(
            state.payload.token_type == "validator"
            and state.status in ACTIVE_TOKEN_STATUSES
            and not state.platform_accounted
            for state in self.requests.values()
        )
        return self.capacity_provider().implementation_limit(
            user_n,
            active_implementations=active_implementations,
            outstanding_reservations=outstanding,
        )

    def available(self, token_type: str) -> int:
        logical = self.logical_capacity[token_type] - self.held(token_type)
        if token_type == "validator":
            outstanding_reservations = sum(
                state.payload.token_type == "validator"
                and state.status in ACTIVE_TOKEN_STATUSES
                and not state.platform_accounted
                for state in self.requests.values()
            )
            platform = self.capacity_provider().validator_slots(
                outstanding_reservations
            )
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
            if not self._authority_matches(state):
                self._invalidate(
                    state, RequestStatus.CANCELLED, "STALE_AUTHORITY"
                )
                continue
            token_id = f"{token_type}-{self.next_token}"
            self.next_token += 1
            now = self.clock.now()
            state.status = RequestStatus.GRANTED
            state.token_id = token_id
            state.granted_at = now
            state.expires_at = now + self.grant_ttl
            state.platform_accounted = False
            return state
        return None

    def bind_validator_spawn(
        self,
        request_id: str,
        token_id: str,
        validator_agent_id: str,
        task_id: str,
        phase: TaskPhase,
        target_sha: str,
        generation: int,
    ) -> None:
        state = self.requests[request_id]
        if (
            state.payload.token_type != "validator"
            or state.status != RequestStatus.ACKED
        ):
            raise ProtocolError("validator spawn requires ACKED token")
        if (
            state.token_id != token_id
            or state.payload.task != task_id
            or state.payload.phase != phase
            or state.payload.head != target_sha
            or state.payload.generation != generation
            or not self._authority_matches(state)
        ):
            raise ProtocolError("validator spawn identity mismatch")
        owner = self.validator_agents.get(validator_agent_id)
        if owner is not None and owner != request_id:
            raise ProtocolError("validator agent already consumed a reservation")
        if (
            state.validator_agent_id is not None
            and state.validator_agent_id != validator_agent_id
        ):
            raise ProtocolError("validator reservation already bound")
        state.validator_agent_id = validator_agent_id
        self.validator_agents[validator_agent_id] = request_id

    def mark_platform_accounted(
        self, request_id: str, validator_agent_id: str
    ) -> None:
        state = self.requests[request_id]
        if state.validator_agent_id != validator_agent_id:
            raise ProtocolError("validator canonical id mismatch")
        if self.validator_agents.get(validator_agent_id) != request_id:
            raise ProtocolError("validator reservation ownership mismatch")
        if validator_agent_id not in self.capacity_provider().live_agent_ids:
            raise ProtocolError("corresponding validator is not live")
        if not self._authority_matches(state):
            self._invalidate(state, RequestStatus.CANCELLED, "STALE_AUTHORITY")
            raise ProtocolError("validator authority became stale")
        state.platform_accounted = True

    def ack(
        self,
        request_id: str,
        token_id: str,
        phase: TaskPhase,
        head: str,
        generation: int,
    ) -> bool:
        self.expire_grants()
        state = self.requests[request_id]
        if not self._authority_matches(state):
            self._invalidate(state, RequestStatus.CANCELLED, "STALE_AUTHORITY")
            raise ProtocolError("ACK authority became stale")
        if (
            phase != state.payload.phase
            or head != state.payload.head
            or generation != state.payload.generation
        ):
            self._invalidate(state, RequestStatus.CANCELLED, "STALE_GRANT")
            raise ProtocolError("grant phase/head/generation mismatch")
        if state.status == RequestStatus.ACKED and state.token_id == token_id:
            return True
        if token_id in self.invalid_tokens:
            raise ProtocolError("old token rejected")
        if state.status != RequestStatus.GRANTED or state.token_id != token_id:
            raise ProtocolError("grant is not active")
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
            if not self._authority_matches(state):
                self._invalidate(
                    state, RequestStatus.CANCELLED, "STALE_AUTHORITY"
                )
                raise ProtocolError("recovery authority became stale")
            state.status = RequestStatus.GRANTED
            state.pre_recovery_status = None
            state.recovery_deadline = None
            state.granted_at = self.clock.now()
            state.expires_at = state.granted_at + self.grant_ttl
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
        if state.validator_agent_id is not None:
            self.validator_agents.pop(state.validator_agent_id, None)
        state.status = status
        state.release_reason = reason
        state.pre_recovery_status = None
        state.recovery_deadline = None
        state.platform_accounted = False
        state.validator_agent_id = None

    def _authority_matches(self, state: RequestState) -> bool:
        task = self.authority.get(state.payload.task)
        return (
            task is not None
            and task.task_id == state.payload.task
            and task.phase == state.payload.phase
            and task.head == state.payload.head
            and task.generation == state.payload.generation
        )


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
    task: TaskState,
) -> RequestPayload:
    return RequestPayload(
        request_id=request_id,
        token_type=token_type,
        task=task.task_id,
        agent=agent,
        phase=task.phase,
        head=task.head,
        generation=task.generation,
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
    live_ids = frozenset(f"agent-{index}" for index in range(live))
    provider = MutableCapacityProvider(
        PlatformSnapshot(total, live, live_agent_ids=live_ids)
    )
    authority = TaskAuthority()
    return (
        TokenBroker(
            provider,
            clock,
            authority,
            grant_ttl=grant_ttl,
            recovery_ttl=recovery_ttl,
        ),
        provider,
        clock,
    )


def submit(
    broker: TokenBroker, payload: RequestPayload, task: TaskState
) -> RequestState:
    if broker.authority.get(task.task_id) is None:
        broker.authority.register(task)
    return broker.request(payload)


def record_pass(task: TaskState, validator_id: str) -> None:
    task.record_validator(
        ValidatorEvidence(
            validator_id=validator_id,
            phase=task.phase,
            target_sha=task.head,
            generation=task.generation,
            verdict="PASS",
            completed_at=1.0,
        )
    )


def record_gate_pass(task: TaskState) -> None:
    task.record_gate(
        GateEvidence(task.phase, task.head, task.generation, True)
    )


def record_sync_pass(task: TaskState) -> None:
    task.record_sync(SyncEvidence(task.head, task.generation, True))


def drive_closure_to_pr(
    task: TaskState,
    branch: TaskPhase = TaskPhase.FIXING,
    archive_suffix: str = "archive",
) -> None:
    task.transition(branch)
    task.transition(TaskPhase.FIX_VALIDATING)
    record_pass(task, "validator-fix")
    task.transition(TaskPhase.GATING)
    record_gate_pass(task)
    task.transition(TaskPhase.REBASING)
    record_sync_pass(task)
    record_gate_pass(task)
    task.transition(TaskPhase.REBASE_VALIDATING)
    record_pass(task, "validator-rebase")
    task.transition(TaskPhase.ARCHIVING)
    task.record_archive(f"{task.head}-{archive_suffix}")
    task.transition(TaskPhase.FINAL_VALIDATING)
    record_pass(task, "validator-final")
    task.transition(TaskPhase.PR_OPEN)


class StateMachineDryRun(unittest.TestCase):
    def test_all_allowed_phase_edges(self) -> None:
        expected = {
            TaskPhase.DISPATCHED: {TaskPhase.CLAIMED, TaskPhase.BLOCKED},
            TaskPhase.CLAIMED: {TaskPhase.PROMOTED, TaskPhase.BLOCKED},
            TaskPhase.PROMOTED: {TaskPhase.VERIFYING, TaskPhase.BLOCKED},
            TaskPhase.VERIFYING: {
                TaskPhase.FIXING,
                TaskPhase.NOT_BUG,
                TaskPhase.BLOCKED,
            },
            TaskPhase.FIXING: {
                TaskPhase.FIX_VALIDATING,
                TaskPhase.BLOCKED,
            },
            TaskPhase.NOT_BUG: {
                TaskPhase.FIX_VALIDATING,
                TaskPhase.BLOCKED,
            },
            TaskPhase.FIX_VALIDATING: {
                TaskPhase.GATING,
                TaskPhase.FIXING,
                TaskPhase.NOT_BUG,
                TaskPhase.BLOCKED,
            },
            TaskPhase.GATING: {
                TaskPhase.REBASING,
                TaskPhase.FIXING,
                TaskPhase.NOT_BUG,
                TaskPhase.BLOCKED,
            },
            TaskPhase.REBASING: {
                TaskPhase.REBASE_VALIDATING,
                TaskPhase.BLOCKED,
            },
            TaskPhase.REBASE_VALIDATING: {
                TaskPhase.ARCHIVING,
                TaskPhase.FIXING,
                TaskPhase.NOT_BUG,
                TaskPhase.BLOCKED,
            },
            TaskPhase.ARCHIVING: {
                TaskPhase.FINAL_VALIDATING,
                TaskPhase.BLOCKED,
            },
            TaskPhase.FINAL_VALIDATING: {
                TaskPhase.PR_OPEN,
                TaskPhase.FIXING,
                TaskPhase.NOT_BUG,
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
        self.assertEqual(ALLOWED_EDGES, expected)
        for source in TaskPhase:
            for target in TaskPhase:
                if target in expected[source]:
                    continue
                with self.subTest(illegal_source=source, illegal_target=target):
                    with self.assertRaises(ProtocolError):
                        TaskState(phase=source).transition(target)

    def test_illegal_mutual_and_terminal_phase_edges(self) -> None:
        with self.assertRaises(ProtocolError):
            TaskState().transition(TaskPhase.CLOSED)
        task = TaskState(phase=TaskPhase.VERIFYING)
        task.transition(TaskPhase.FIXING)
        task.transition(TaskPhase.FIX_VALIDATING)
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

    def test_complete_path_requires_every_closure_stage(self) -> None:
        task = TaskState(phase=TaskPhase.VERIFYING)
        drive_closure_to_pr(task)
        task.transition(TaskPhase.GATES)
        task.transition(TaskPhase.CLOSED)
        self.assertEqual(task.phase, TaskPhase.CLOSED)
        self.assertTrue(task.final_validated)

    def test_pr_open_bypasses_are_rejected(self) -> None:
        for phase in (TaskPhase.FIX_VALIDATING, TaskPhase.ARCHIVING):
            with self.subTest(phase=phase):
                with self.assertRaises(ProtocolError):
                    TaskState(
                        phase=phase, resolution=TaskPhase.FIXING
                    ).transition(TaskPhase.PR_OPEN)
        with self.assertRaises(ProtocolError):
            TaskState(
                phase=TaskPhase.FINAL_VALIDATING,
                resolution=TaskPhase.FIXING,
            ).transition(TaskPhase.PR_OPEN)

    def test_validator_and_gate_evidence_are_required_and_sha_bound(self) -> None:
        task = TaskState(phase=TaskPhase.FIX_VALIDATING)
        task.resolution = TaskPhase.FIXING
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.GATING)
        with self.assertRaises(ProtocolError):
            task.record_validator(
                ValidatorEvidence(
                    "v",
                    task.phase,
                    "old-sha",
                    task.generation,
                    "PASS",
                    1.0,
                )
            )
        with self.assertRaises(ProtocolError):
            task.record_validator(
                ValidatorEvidence(
                    "v",
                    task.phase,
                    task.head,
                    task.generation,
                    "FAIL",
                    1.0,
                )
            )
        record_pass(task, "v-pass")
        task.update_head("new-head")
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.GATING)

        gate_task = TaskState(
            phase=TaskPhase.GATING, resolution=TaskPhase.FIXING
        )
        with self.assertRaises(ProtocolError):
            gate_task.record_gate(
                GateEvidence(
                    TaskPhase.GATING,
                    gate_task.head,
                    gate_task.generation,
                    False,
                )
            )
        with self.assertRaises(ProtocolError):
            gate_task.transition(TaskPhase.REBASING)

    def test_rebase_head_change_clears_old_gate_and_sync_evidence(self) -> None:
        task = TaskState(phase=TaskPhase.VERIFYING)
        task.transition(TaskPhase.FIXING)
        task.transition(TaskPhase.FIX_VALIDATING)
        record_pass(task, "fix-pass")
        task.transition(TaskPhase.GATING)
        record_gate_pass(task)
        task.transition(TaskPhase.REBASING)
        record_sync_pass(task)
        record_gate_pass(task)
        task.record_rebase_head("merged-head")
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.REBASE_VALIDATING)
        record_sync_pass(task)
        record_gate_pass(task)
        task.transition(TaskPhase.REBASE_VALIDATING)

    def test_a_b_a_generation_rejects_old_evidence_replay(self) -> None:
        validator_task = TaskState(
            phase=TaskPhase.FIX_VALIDATING, head="A"
        )
        old_validator = ValidatorEvidence(
            "old-validator",
            validator_task.phase,
            "A",
            validator_task.generation,
            "PASS",
            1.0,
        )
        validator_task.update_head("B")
        validator_task.update_head("A")
        with self.assertRaises(ProtocolError):
            validator_task.record_validator(old_validator)
        record_pass(validator_task, "current-validator")

        gate_task = TaskState(phase=TaskPhase.GATING, head="A")
        old_gate = GateEvidence(
            gate_task.phase, "A", gate_task.generation, True
        )
        gate_task.update_head("B")
        gate_task.update_head("A")
        with self.assertRaises(ProtocolError):
            gate_task.record_gate(old_gate)
        record_gate_pass(gate_task)

        sync_task = TaskState(phase=TaskPhase.REBASING, head="A")
        old_sync = SyncEvidence("A", sync_task.generation, True)
        sync_task.record_rebase_head("B")
        sync_task.record_rebase_head("A")
        with self.assertRaises(ProtocolError):
            sync_task.record_sync(old_sync)
        record_sync_pass(sync_task)

    def test_review_rework_resets_and_repeats_full_closure(self) -> None:
        task = TaskState(phase=TaskPhase.VERIFYING)
        drive_closure_to_pr(task, archive_suffix="archive-1")
        task.transition(TaskPhase.GATES)
        task.transition(TaskPhase.FIXING)
        self.assertFalse(task.final_validated)
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.PR_OPEN)
        task.transition(TaskPhase.FIX_VALIDATING)
        record_pass(task, "validator-fix-2")
        task.transition(TaskPhase.GATING)
        record_gate_pass(task)
        task.transition(TaskPhase.REBASING)
        record_sync_pass(task)
        record_gate_pass(task)
        task.transition(TaskPhase.REBASE_VALIDATING)
        record_pass(task, "validator-rebase-2")
        task.transition(TaskPhase.ARCHIVING)
        task.record_archive(f"{task.head}-archive-2")
        task.transition(TaskPhase.FINAL_VALIDATING)
        record_pass(task, "validator-final-2")
        task.transition(TaskPhase.PR_OPEN)
        self.assertTrue(task.final_validated)

    def test_platform_reserve_and_dynamic_validator_capacity(self) -> None:
        snapshot = PlatformSnapshot(total=4, live_agents=1)
        self.assertEqual(snapshot.implementation_limit(9), 2)
        self.assertEqual(snapshot.validator_slots(), 3)
        self.assertEqual(PlatformSnapshot(6, 1).implementation_limit(4), 4)
        self.assertEqual(PlatformSnapshot(4, 2).implementation_limit(9), 1)
        self.assertEqual(PlatformSnapshot(4, 3).implementation_limit(9), 0)
        self.assertEqual(PlatformSnapshot(4, 4).implementation_limit(9), 0)
        self.assertEqual(
            PlatformSnapshot(4, 1).implementation_limit(
                9, outstanding_reservations=1
            ),
            1,
        )
        self.assertEqual(
            PlatformSnapshot(
                4, 0, main_in_snapshot=False
            ).implementation_limit(9),
            2,
        )
        self.assertEqual(
            PlatformSnapshot(4, 3, validator_reserve=0).validator_slots(), 0
        )
        self.assertEqual(
            PlatformSnapshot(None, 99).implementation_limit(9), 9
        )
        self.assertEqual(
            PlatformSnapshot(None, 99).implementation_limit(
                9, active_implementations=4
            ),
            5,
        )
        self.assertEqual(
            PlatformSnapshot(10, 4).implementation_limit(
                4, active_implementations=3
            ),
            1,
        )
        self.assertEqual(
            PlatformSnapshot(
                2, 1, main_in_snapshot=False
            ).validator_slots(),
            0,
        )
        self.assertEqual(
            PlatformSnapshot(
                4, 2, main_in_snapshot=False
            ).validator_slots(),
            1,
        )
        self.assertEqual(
            PlatformSnapshot(
                4, 2, main_in_snapshot=False
            ).validator_slots(outstanding_reservations=1),
            0,
        )
        broker, provider, _ = fixture(total=4, live=4)
        self.assertEqual(broker.implementation_limit(9), 0)
        task = TaskState(phase=TaskPhase.FIX_VALIDATING)
        submit(
            broker,
            make_payload("v1", "validator", "agent-a", task), task
        )
        self.assertIsNone(broker.grant_next("validator"))
        provider.snapshot.live_agents = 3
        self.assertIsNotNone(broker.grant_next("validator"))
        self.assertEqual(broker.implementation_limit(9), 0)

    def test_implementation_n_is_independent_from_compile_capacity(self) -> None:
        broker, _, _ = fixture(total=8, live=1)
        self.assertEqual(broker.implementation_limit(4), 4)
        tasks = [
            TaskState(task_id=f"compile-{index}", phase=TaskPhase.FIXING)
            for index in range(3)
        ]
        for index, task in enumerate(tasks):
            submit(
                broker,
                make_payload(
                    f"compile-request-{index}", "compile", "agent", task
                ),
                task,
            )
        self.assertIsNotNone(broker.grant_next("compile"))
        self.assertIsNotNone(broker.grant_next("compile"))
        self.assertIsNone(broker.grant_next("compile"))

    def test_validator_capacity_shrinks_and_queue_recovers(self) -> None:
        broker, provider, _ = fixture(total=5, live=3)
        task = TaskState(phase=TaskPhase.FIX_VALIDATING)
        for request_id in ("v1", "v2", "v3"):
            submit(
                broker,
                make_payload(request_id, "validator", request_id, task),
                task,
            )
        first = broker.grant_next("validator")
        provider.snapshot.live_agents = 5
        self.assertIsNone(broker.grant_next("validator"))
        broker.ack(
            "v1", first.token_id, task.phase, task.head, task.generation
        )
        broker.release("v1", first.token_id, "PASS")
        provider.snapshot.live_agents = 4
        self.assertEqual(
            broker.grant_next("validator").payload.request_id, "v2"
        )

    def test_validator_reservation_blocks_duplicate_real_slot(self) -> None:
        broker, provider, _ = fixture(total=4, live=3)
        task = TaskState(phase=TaskPhase.FIX_VALIDATING)
        for request_id in ("v1", "v2"):
            submit(
                broker,
                make_payload(request_id, "validator", request_id, task),
                task,
            )
        first = broker.grant_next("validator")
        self.assertIsNotNone(first)
        self.assertIsNone(broker.grant_next("validator"))
        broker.ack(
            "v1", first.token_id, task.phase, task.head, task.generation
        )
        self.assertIsNone(broker.grant_next("validator"))
        broker.mark_lost("v1")
        self.assertIsNone(broker.grant_next("validator"))
        broker.recovery_result("v1", True)
        broker.ack(
            "v1", first.token_id, task.phase, task.head, task.generation
        )
        broker.bind_validator_spawn(
            "v1",
            first.token_id,
            "validator-v1",
            task.task_id,
            task.phase,
            task.head,
            task.generation,
        )
        with self.assertRaises(ProtocolError):
            broker.mark_platform_accounted("v1", "validator-v1")
        provider.snapshot.live_agents = 4
        provider.snapshot.live_agent_ids = (
            provider.snapshot.live_agent_ids | {"validator-v1"}
        )
        broker.mark_platform_accounted("v1", "validator-v1")
        self.assertIsNone(broker.grant_next("validator"))
        broker.release("v1", first.token_id, "PASS")
        provider.snapshot.live_agents = 3
        self.assertEqual(
            broker.grant_next("validator").payload.request_id, "v2"
        )

    def test_validator_spawn_identity_is_exact_and_one_to_one(self) -> None:
        broker, provider, _ = fixture(total=5, live=3)
        task = TaskState(
            task_id="trade-fix", phase=TaskPhase.FIX_VALIDATING
        )
        for request_id in ("v1", "v2"):
            submit(
                broker,
                make_payload(request_id, "validator", request_id, task), task
            )
        first = broker.grant_next("validator")
        second = broker.grant_next("validator")
        broker.ack(
            "v1", first.token_id, task.phase, task.head, task.generation
        )
        broker.ack(
            "v2", second.token_id, task.phase, task.head, task.generation
        )
        broker.bind_validator_spawn(
            "v1",
            first.token_id,
            "validator-one",
            task.task_id,
            task.phase,
            task.head,
            task.generation,
        )
        with self.assertRaises(ProtocolError):
            broker.bind_validator_spawn(
                "v2",
                second.token_id,
                "validator-one",
                task.task_id,
                task.phase,
                task.head,
                task.generation,
            )
        broker.bind_validator_spawn(
            "v2",
            second.token_id,
            "validator-two",
            task.task_id,
            task.phase,
            task.head,
            task.generation,
        )
        provider.snapshot.live_agents = 4
        provider.snapshot.live_agent_ids = (
            provider.snapshot.live_agent_ids | {"unrelated-agent"}
        )
        with self.assertRaises(ProtocolError):
            broker.mark_platform_accounted("v1", "validator-one")
        provider.snapshot.live_agents = 5
        provider.snapshot.live_agent_ids = (
            provider.snapshot.live_agent_ids | {"validator-two"}
        )
        broker.mark_platform_accounted("v2", "validator-two")
        self.assertFalse(broker.requests["v1"].platform_accounted)
        self.assertTrue(broker.requests["v2"].platform_accounted)
        with self.assertRaises(ProtocolError):
            broker.mark_platform_accounted("v1", "validator-two")
        provider.snapshot.live_agent_ids = (
            provider.snapshot.live_agent_ids
            - {"unrelated-agent"}
            | {"validator-one"}
        )
        broker.mark_platform_accounted("v1", "validator-one")

    def test_authority_provider_rejects_forged_task_replacement(self) -> None:
        broker, _, _ = fixture()
        authoritative = TaskState(
            task_id="authority-store", phase=TaskPhase.GATING, head="real"
        )
        submit(
            broker,
            make_payload("real", "compile", "a", authoritative),
            authoritative,
        )
        forged = TaskState(
            task_id="authority-store",
            phase=TaskPhase.FIXING,
            head="forged",
            generation=9,
        )
        with self.assertRaises(ProtocolError):
            broker.authority.register(forged)
        with self.assertRaises(ProtocolError):
            broker.request(make_payload("forged", "compile", "a", forged))
        self.assertIs(broker.authority.get("authority-store"), authoritative)

    def test_validator_bind_requires_acked_phase_and_generation(self) -> None:
        broker, _, _ = fixture(total=6, live=2)
        task = TaskState(
            task_id="bind-strict", phase=TaskPhase.FIX_VALIDATING
        )
        submit(
            broker, make_payload("bind", "validator", "a", task), task
        )
        grant = broker.grant_next("validator")
        with self.assertRaises(ProtocolError):
            broker.bind_validator_spawn(
                "bind",
                grant.token_id,
                "validator-bind",
                task.task_id,
                task.phase,
                task.head,
                task.generation,
            )
        broker.ack(
            "bind", grant.token_id, task.phase, task.head, task.generation
        )
        with self.assertRaises(ProtocolError):
            broker.bind_validator_spawn(
                "bind",
                grant.token_id,
                "validator-bind",
                task.task_id,
                TaskPhase.REBASE_VALIDATING,
                task.head,
                task.generation,
            )
        with self.assertRaises(ProtocolError):
            broker.bind_validator_spawn(
                "bind",
                grant.token_id,
                "validator-bind",
                task.task_id,
                task.phase,
                task.head,
                task.generation + 1,
            )
        broker.mark_lost("bind")
        with self.assertRaises(ProtocolError):
            broker.bind_validator_spawn(
                "bind",
                grant.token_id,
                "validator-bind",
                task.task_id,
                task.phase,
                task.head,
                task.generation,
            )
        broker.recovery_result("bind", True)
        with self.assertRaises(ProtocolError):
            broker.bind_validator_spawn(
                "bind",
                grant.token_id,
                "validator-bind",
                task.task_id,
                task.phase,
                task.head,
                task.generation,
            )
        broker.ack(
            "bind", grant.token_id, task.phase, task.head, task.generation
        )
        broker.bind_validator_spawn(
            "bind",
            grant.token_id,
            "validator-bind",
            task.task_id,
            task.phase,
            task.head,
            task.generation,
        )
        with self.assertRaises(ProtocolError):
            broker.bind_validator_spawn(
                "bind",
                grant.token_id,
                "validator-bind-second",
                task.task_id,
                task.phase,
                task.head,
                task.generation,
            )

    def test_grant_ack_and_recovery_recheck_authoritative_generation(self) -> None:
        broker, _, _ = fixture()
        task = TaskState(
            task_id="authority", phase=TaskPhase.FIX_VALIDATING
        )
        submit(
            broker, make_payload("queued", "validator", "a", task), task
        )
        task.update_head("head-after-queue")
        self.assertIsNone(broker.grant_next("validator"))
        self.assertEqual(
            broker.requests["queued"].release_reason, "STALE_AUTHORITY"
        )

        phase_task = TaskState(
            task_id="phase-queue", phase=TaskPhase.FIX_VALIDATING
        )
        submit(
            broker,
            make_payload("phase-queued", "validator", "a", phase_task),
            phase_task,
        )
        phase_task.phase = TaskPhase.FIXING
        self.assertIsNone(broker.grant_next("validator"))
        self.assertEqual(
            broker.requests["phase-queued"].release_reason,
            "STALE_AUTHORITY",
        )

        task2 = TaskState(
            task_id="ack-authority", phase=TaskPhase.FIX_VALIDATING
        )
        submit(
            broker, make_payload("ack", "validator", "a", task2), task2
        )
        grant = broker.grant_next("validator")
        task2.update_head("head-before-ack")
        with self.assertRaises(ProtocolError):
            broker.ack(
                "ack",
                grant.token_id,
                TaskPhase.FIX_VALIDATING,
                "abc123",
                0,
            )

        generation_task = TaskState(
            task_id="generation-ack", phase=TaskPhase.FIX_VALIDATING
        )
        submit(
            broker,
            make_payload(
                "generation", "validator", "a", generation_task
            ),
            generation_task,
        )
        generation_grant = broker.grant_next("validator")
        with self.assertRaises(ProtocolError):
            broker.ack(
                "generation",
                generation_grant.token_id,
                generation_task.phase,
                generation_task.head,
                generation_task.generation + 1,
            )

        task3 = TaskState(
            task_id="repeat-ack", phase=TaskPhase.FIX_VALIDATING
        )
        submit(
            broker,
            make_payload("repeat", "validator", "a", task3), task3
        )
        grant3 = broker.grant_next("validator")
        broker.ack(
            "repeat",
            grant3.token_id,
            task3.phase,
            task3.head,
            task3.generation,
        )
        task3.update_head("head-after-ack")
        with self.assertRaises(ProtocolError):
            broker.ack(
                "repeat",
                grant3.token_id,
                TaskPhase.FIX_VALIDATING,
                "abc123",
                0,
            )

        task4 = TaskState(
            task_id="recovery-authority", phase=TaskPhase.GATING
        )
        submit(
            broker, make_payload("recover", "compile", "a", task4), task4
        )
        grant4 = broker.grant_next("compile")
        broker.ack(
            "recover",
            grant4.token_id,
            task4.phase,
            task4.head,
            task4.generation,
        )
        broker.mark_lost("recover")
        task4.update_head("head-during-recovery")
        with self.assertRaises(ProtocolError):
            broker.recovery_result("recover", True)

    def test_fifo_request_ack_release_cancel_and_collision(self) -> None:
        broker, _, _ = fixture()
        task = TaskState(phase=TaskPhase.GATING)
        first_payload = make_payload("r1", "compile", "a", task)
        first = submit(broker, first_payload, task)
        submit(broker, make_payload("r2", "compile", "b", task), task)
        self.assertEqual(broker.queue_position("r1"), 1)
        self.assertEqual(broker.queue_position("r2"), 2)
        self.assertIs(broker.request(first_payload), first)
        grant = broker.grant_next("compile")
        with self.assertRaises(ProtocolError):
            broker.release("r1", grant.token_id, "PASS")
        broker.ack(
            "r1", grant.token_id, task.phase, task.head, task.generation
        )
        self.assertTrue(broker.release("r1", grant.token_id, "PASS"))
        self.assertFalse(broker.release("r1", grant.token_id, "PASS"))
        self.assertTrue(broker.cancel("r2", "CANCELLED"))
        with self.assertRaises(ProtocolError):
            submit(
                broker,
                make_payload(
                    "r1",
                    "validator",
                    "a",
                    TaskState(phase=TaskPhase.FIX_VALIDATING),
                ),
                TaskState(phase=TaskPhase.FIX_VALIDATING),
            )

    def test_token_phase_binding(self) -> None:
        broker, _, _ = fixture()
        for phase in (
            TaskPhase.VERIFYING,
            TaskPhase.FIXING,
            TaskPhase.NOT_BUG,
            TaskPhase.GATING,
            TaskPhase.REBASING,
        ):
            with self.subTest(compile_phase=phase):
                local_broker, _, _ = fixture()
                task = TaskState(phase=phase)
                task.resolution = (
                    phase if phase in BRANCH_PHASES else TaskPhase.FIXING
                )
                state = submit(
                    local_broker,
                    make_payload(
                        f"compile-{phase.value}", "compile", "a", task
                    ),
                    task,
                )
                self.assertEqual(state.status, RequestStatus.QUEUED)
        with self.assertRaises(ProtocolError):
            submit(
                broker,
                make_payload(
                    "x",
                    "compile",
                    "a",
                    TaskState(phase=TaskPhase.FIX_VALIDATING),
                ),
                TaskState(phase=TaskPhase.FIX_VALIDATING),
            )
        with self.assertRaises(ProtocolError):
            invalid_validator_task = TaskState(
                task_id="validator-gating-reject",
                phase=TaskPhase.GATING,
            )
            submit(
                broker,
                make_payload(
                    "y",
                    "validator",
                    "a",
                    invalid_validator_task,
                ),
                invalid_validator_task,
            )

    def test_grant_ttl_before_boundary_ack(self) -> None:
        broker, _, clock = fixture(grant_ttl=10)
        task = TaskState(phase=TaskPhase.FIX_VALIDATING)
        submit(
            broker,
            make_payload("v1", "validator", "a", task), task
        )
        grant = broker.grant_next("validator")
        clock.advance(9.999)
        self.assertTrue(
            broker.ack(
                "v1", grant.token_id, task.phase, task.head, task.generation
            )
        )

    def test_grant_ttl_boundary_expiry_fifo_and_old_token(self) -> None:
        broker, _, clock = fixture(grant_ttl=10)
        task = TaskState(phase=TaskPhase.FIX_VALIDATING)
        for request_id in ("v1", "v2"):
            submit(
                broker,
                make_payload(request_id, "validator", request_id, task),
                task,
            )
        old = broker.grant_next("validator")
        clock.advance(10)
        self.assertEqual(broker.expire_grants(), ["v1"])
        self.assertEqual(broker.expire_grants(), [])
        with self.assertRaises(ProtocolError):
            broker.ack(
                "v1", old.token_id, task.phase, task.head, task.generation
            )
        with self.assertRaises(ProtocolError):
            broker.release("v1", old.token_id, "PASS")
        self.assertEqual(
            broker.grant_next("validator").payload.request_id, "v2"
        )

    def test_validator_all_exit_reasons_release(self) -> None:
        for reason in ("PASS", "FAIL", "TIMEOUT", "ERROR", "CANCEL"):
            with self.subTest(reason=reason):
                broker, _, _ = fixture()
                task = TaskState(phase=TaskPhase.FIX_VALIDATING)
                submit(
                    broker,
                    make_payload(reason, "validator", "v", task), task
                )
                grant = broker.grant_next("validator")
                broker.ack(
                    reason,
                    grant.token_id,
                    task.phase,
                    task.head,
                    task.generation,
                )
                broker.release(reason, grant.token_id, reason)
                self.assertEqual(broker.held("validator"), 0)

    def test_recovery_success_failure_timeout_and_late_messages(self) -> None:
        broker, _, clock = fixture(recovery_ttl=10)
        task = TaskState(phase=TaskPhase.GATING)
        for request_id in ("ok", "fail", "timeout"):
            submit(
                broker,
                make_payload(request_id, "compile", request_id, task),
                task,
            )
            grant = broker.grant_next("compile")
            broker.ack(
                request_id,
                grant.token_id,
                task.phase,
                task.head,
                task.generation,
            )
            broker.mark_lost(request_id)
            if request_id == "ok":
                self.assertEqual(
                    broker.followup_payload(request_id)["token_id"],
                    grant.token_id,
                )
                broker.recovery_result(request_id, True)
                broker.ack(
                    request_id,
                    grant.token_id,
                    task.phase,
                    task.head,
                    task.generation,
                )
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
                        request_id,
                        grant.token_id,
                        task.phase,
                        task.head,
                        task.generation,
                    )

    def test_recovery_renews_grant_ttl_before_mandatory_reack(self) -> None:
        broker, _, clock = fixture(grant_ttl=5, recovery_ttl=10)
        task = TaskState(phase=TaskPhase.GATING)
        submit(
            broker,
            make_payload("renew", "compile", "agent", task),
            task,
        )
        grant = broker.grant_next("compile")
        broker.ack(
            "renew", grant.token_id, task.phase, task.head, task.generation
        )
        broker.mark_lost("renew")
        clock.advance(6)
        broker.recovery_result("renew", True)
        self.assertTrue(
            broker.ack(
                "renew",
                grant.token_id,
                task.phase,
                task.head,
                task.generation,
            )
        )
        self.assertTrue(broker.release("renew", grant.token_id, "PASS"))

    def test_repeated_recovery_is_idempotent(self) -> None:
        broker, _, _ = fixture()
        task = TaskState(phase=TaskPhase.GATING)
        submit(broker, make_payload("r", "compile", "a", task), task)
        grant = broker.grant_next("compile")
        broker.ack(
            "r", grant.token_id, task.phase, task.head, task.generation
        )
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
