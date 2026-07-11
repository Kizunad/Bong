#!/usr/bin/env python3
"""BugFix 调度协议纯内存 dry-run；不访问 GitHub、不修改仓库。"""

from __future__ import annotations

import unittest
from dataclasses import dataclass, field, replace
from enum import Enum
import re
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
    request_id: str
    token_id: str
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
class ClosureEvidence:
    target_sha: str
    generation: int
    pr_number: int
    pr_url: str
    remote_sha: str
    checks: tuple[tuple[str, str], ...]
    implementation_model: str
    validator_model: str
    reviewer_model: str


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
    closure_evidence: Optional[ClosureEvidence] = None
    _validator_capability: object = field(
        default_factory=object, init=False, repr=False
    )
    repository: str = "Kizunad/Bong"
    required_checks: tuple[str, ...] = ("review", "e2e", "CodeRabbit")
    expected_models: tuple[str, str, str] = (
        "gpt-5",
        "gpt-5.6-sol-xhigh",
        "gpt-5.5",
    )

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
            if self.phase != TaskPhase.VERIFYING:
                self.generation += 1
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
        if self.phase == TaskPhase.GATES and target == TaskPhase.CLOSED:
            evidence = self.closure_evidence
            checks = dict(evidence.checks) if evidence is not None else {}
            models = (
                evidence.implementation_model,
                evidence.validator_model,
                evidence.reviewer_model,
            ) if evidence is not None else ()
            invalid_models = {"", "AI", "agent", "unknown", "pending"}
            expected_pr_url = (
                f"https://github.com/{self.repository}/pull/"
                f"{evidence.pr_number}"
                if evidence is not None
                else ""
            )
            if (
                evidence is None
                or not self.head
                or not self.final_validated
                or evidence.target_sha != self.head
                or evidence.generation != self.generation
                or evidence.pr_number <= 0
                or evidence.pr_url != expected_pr_url
                or evidence.remote_sha != self.head
                or any(
                    checks.get(name) != "SUCCESS"
                    for name in self.required_checks
                )
                or len(checks) != len(evidence.checks)
                or any(result != "SUCCESS" for result in checks.values())
                or len(models) != 3
                or models != self.expected_models
                or any(
                    model != model.strip()
                    or model.lower() in {value.lower() for value in invalid_models}
                    or re.fullmatch(r"[a-z0-9]+(?:[.-][a-z0-9]+)+", model)
                    is None
                    for model in models
                )
            ):
                raise ProtocolError("CLOSED gate evidence incomplete")
            self._require_validator_pass(TaskPhase.FINAL_VALIDATING)
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
        self.closure_evidence = None

    def update_head(self, new_head: str) -> None:
        effective_phase = (
            self.recovery_from
            if self.phase == TaskPhase.RECOVERING
            else self.phase
        )
        if effective_phase in {TaskPhase.PR_OPEN, TaskPhase.GATES}:
            raise ProtocolError("PR phases must enter rework before HEAD changes")
        if new_head == self.head:
            return
        self.head = new_head
        self.generation += 1
        self._reset_closure_milestones()

    def _record_validator(
        self, evidence: ValidatorEvidence, capability: object
    ) -> None:
        if capability is not self._validator_capability:
            raise ProtocolError("validator evidence requires broker capability")
        if evidence.phase != self.phase:
            raise ProtocolError("validator evidence phase mismatch")
        if evidence.target_sha != self.head:
            raise ProtocolError("validator evidence HEAD mismatch")
        if evidence.generation != self.generation:
            raise ProtocolError("validator evidence generation mismatch")
        if evidence.verdict != "PASS":
            raise ProtocolError("validator did not PASS")
        self.validator_evidence[self.phase] = evidence

    def record_closure(self, evidence: ClosureEvidence) -> None:
        if self.phase != TaskPhase.GATES:
            raise ProtocolError("closure evidence only applies in GATES")
        if (
            evidence.target_sha != self.head
            or evidence.generation != self.generation
        ):
            raise ProtocolError("closure evidence authority mismatch")
        self.closure_evidence = evidence

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
        self.closure_evidence = None
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
        self.closure_evidence = None
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
    terminal_message: Optional[tuple[object, ...]] = None
    terminal_kind: Optional[str] = None


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
        self.consumed_validator_agents: set[str] = set()
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
            snapshot = self.capacity_provider()
            if snapshot.total is None:
                return max(0, min(logical, 1 - self.held("validator")))
            outstanding_reservations = sum(
                state.payload.token_type == "validator"
                and state.status in ACTIVE_TOKEN_STATUSES
                and not state.platform_accounted
                for state in self.requests.values()
            )
            platform = snapshot.validator_slots(
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
        if not validator_agent_id.strip():
            raise ProtocolError("canonical validator id is empty")
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
            validator_agent_id in self.consumed_validator_agents
            and state.validator_agent_id != validator_agent_id
        ):
            raise ProtocolError("validator agent cannot cross reservations")
        if (
            state.validator_agent_id is not None
            and state.validator_agent_id != validator_agent_id
        ):
            raise ProtocolError("validator reservation already bound")
        state.validator_agent_id = validator_agent_id
        self.validator_agents[validator_agent_id] = request_id
        self.consumed_validator_agents.add(validator_agent_id)

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

    def consume_validator_evidence(
        self,
        request_id: str,
        token_id: str,
        evidence: ValidatorEvidence,
    ) -> None:
        state = self.requests[request_id]
        task = self.authority.get(state.payload.task)
        validator_id = state.validator_agent_id
        if (
            state.status != RequestStatus.ACKED
            or state.payload.token_type != "validator"
            or state.token_id != token_id
            or evidence.request_id != request_id
            or evidence.token_id != token_id
            or validator_id is None
            or evidence.validator_id != validator_id
            or not state.platform_accounted
            or validator_id not in self.capacity_provider().live_agent_ids
            or task is None
            or not self._authority_matches(state)
            or evidence.phase != state.payload.phase
            or evidence.target_sha != state.payload.head
            or evidence.generation != state.payload.generation
        ):
            raise ProtocolError("validator evidence is not bound to live grant")
        task._record_validator(evidence, task._validator_capability)

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

    def release(
        self,
        request_id: str,
        token_id: str,
        agent: str,
        phase: TaskPhase,
        head: str,
        generation: int,
        reason: str,
    ) -> bool:
        state = self.requests[request_id]
        message = (token_id, agent, phase, head, generation, reason)
        if state.status == RequestStatus.RELEASED:
            if state.terminal_message != message:
                raise ProtocolError("duplicate release payload collision")
            return False
        if not self._authority_matches(state):
            self._invalidate(state, RequestStatus.CANCELLED, "STALE_AUTHORITY")
            raise ProtocolError("release authority became stale")
        if token_id in self.invalid_tokens:
            raise ProtocolError("late release rejected")
        if (
            state.status != RequestStatus.ACKED
            or state.token_id != token_id
            or state.payload.agent != agent
            or state.payload.phase != phase
            or state.payload.head != head
            or state.payload.generation != generation
        ):
            raise ProtocolError("release requires ACKED active token")
        state.terminal_message = message
        state.terminal_kind = "RELEASED"
        self._invalidate(state, RequestStatus.RELEASED, reason)
        return True

    def cancel(
        self,
        request_id: str,
        agent: str,
        phase: TaskPhase,
        head: str,
        generation: int,
        reason: str,
    ) -> bool:
        state = self.requests[request_id]
        message = (agent, phase, head, generation, reason)
        if (
            state.payload.agent != agent
            or state.payload.phase != phase
            or state.payload.head != head
            or state.payload.generation != generation
        ):
            raise ProtocolError("cancel payload differs from request")
        if state.status in {
            RequestStatus.RELEASED,
            RequestStatus.CANCELLED,
            RequestStatus.EXPIRED,
        }:
            if state.terminal_kind == "RELEASED":
                raise ProtocolError("cancel collides with prior release")
            if state.status == RequestStatus.EXPIRED:
                raise ProtocolError("cancel collides with prior expiry")
            if state.status == RequestStatus.CANCELLED and (
                state.terminal_message != message
            ):
                raise ProtocolError("duplicate cancel payload collision")
            return False
        state.terminal_message = message
        state.terminal_kind = "CANCELLED"
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
        if not self._authority_matches(state):
            self._invalidate(state, RequestStatus.CANCELLED, "STALE_AUTHORITY")
            raise ProtocolError("recovery entry authority became stale")
        if state.status == RequestStatus.RECOVERING:
            return False
        if state.status not in {RequestStatus.GRANTED, RequestStatus.ACKED}:
            raise ProtocolError("only active holder can recover")
        state.pre_recovery_status = state.status
        state.status = RequestStatus.RECOVERING
        state.recovery_deadline = self.clock.now() + self.recovery_ttl
        return True

    def recovery_result(
        self,
        request_id: str,
        token_id: str,
        agent: str,
        checkpoint: str,
        phase: TaskPhase,
        head: str,
        generation: int,
        success: bool,
    ) -> None:
        state = self.requests[request_id]
        if state.status != RequestStatus.RECOVERING:
            raise ProtocolError("request is not recovering")
        if (
            state.token_id != token_id
            or state.payload.agent != agent
            or state.payload.checkpoint != checkpoint
            or state.payload.phase != phase
            or state.payload.head != head
            or state.payload.generation != generation
        ):
            self._invalidate(state, RequestStatus.CANCELLED, "STALE_RECOVERY")
            raise ProtocolError("recovery payload mismatch")
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
                and not self._authority_matches(state)
            ):
                self._invalidate(
                    state, RequestStatus.CANCELLED, "STALE_AUTHORITY"
                )
                reclaimed.append(request_id)
                continue
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

    def followup_payload(self, request_id: str) -> dict[str, object]:
        state = self.requests[request_id]
        if not self._authority_matches(state):
            self._invalidate(state, RequestStatus.CANCELLED, "STALE_AUTHORITY")
            raise ProtocolError("followup authority became stale")
        if state.status != RequestStatus.RECOVERING or state.token_id is None:
            raise ProtocolError("request has no recovery checkpoint")
        return {
            "checkpoint": state.payload.checkpoint,
            "request_id": request_id,
            "token_id": state.token_id,
            "agent": state.payload.agent,
            "phase": state.payload.phase.value,
            "head": state.payload.head,
            "generation": state.payload.generation,
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
        phase_matches = task is not None and (
            task.phase == state.payload.phase
            or (
                task.phase == TaskPhase.RECOVERING
                and task.recovery_from == state.payload.phase
            )
        )
        return (
            task is not None
            and task.task_id == state.payload.task
            and phase_matches
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
    broker, provider, _ = fixture(total=4, live=1)
    request_id = f"{validator_id}-{task.phase.value}-{task.generation}"
    payload = make_payload(request_id, "validator", "implementer", task)
    submit(broker, payload, task)
    grant = broker.grant_next("validator")
    broker.ack(
        request_id,
        grant.token_id,
        task.phase,
        task.head,
        task.generation,
    )
    broker.bind_validator_spawn(
        request_id,
        grant.token_id,
        validator_id,
        task.task_id,
        task.phase,
        task.head,
        task.generation,
    )
    provider.snapshot.live_agents += 1
    provider.snapshot.live_agent_ids = (
        provider.snapshot.live_agent_ids | {validator_id}
    )
    broker.mark_platform_accounted(request_id, validator_id)
    broker.consume_validator_evidence(
        request_id,
        grant.token_id,
        ValidatorEvidence(
            request_id=request_id,
            token_id=grant.token_id,
            validator_id=validator_id,
            phase=task.phase,
            target_sha=task.head,
            generation=task.generation,
            verdict="PASS",
            completed_at=1.0,
        ),
    )
    broker.release(
        request_id,
        grant.token_id,
        payload.agent,
        payload.phase,
        payload.head,
        payload.generation,
        "PASS",
    )


def release_request(
    broker: TokenBroker,
    request_id: str,
    token_id: str,
    reason: str,
) -> bool:
    payload = broker.requests[request_id].payload
    return broker.release(
        request_id,
        token_id,
        payload.agent,
        payload.phase,
        payload.head,
        payload.generation,
        reason,
    )


def cancel_request(
    broker: TokenBroker, request_id: str, reason: str
) -> bool:
    payload = broker.requests[request_id].payload
    return broker.cancel(
        request_id,
        payload.agent,
        payload.phase,
        payload.head,
        payload.generation,
        reason,
    )


def recover_request(
    broker: TokenBroker, request_id: str, success: bool
) -> None:
    state = broker.requests[request_id]
    payload = state.payload
    broker.recovery_result(
        request_id,
        state.token_id,
        payload.agent,
        payload.checkpoint,
        payload.phase,
        payload.head,
        payload.generation,
        success,
    )


def record_gate_pass(task: TaskState) -> None:
    task.record_gate(
        GateEvidence(task.phase, task.head, task.generation, True)
    )


def record_sync_pass(task: TaskState) -> None:
    task.record_sync(SyncEvidence(task.head, task.generation, True))


def record_closure_pass(task: TaskState) -> None:
    task.record_closure(
        ClosureEvidence(
            target_sha=task.head,
            generation=task.generation,
            pr_number=1163,
            pr_url="https://github.com/Kizunad/Bong/pull/1163",
            remote_sha=task.head,
            checks=(
                ("review", "SUCCESS"),
                ("e2e", "SUCCESS"),
                ("CodeRabbit", "SUCCESS"),
            ),
            implementation_model="gpt-5",
            validator_model="gpt-5.6-sol-xhigh",
            reviewer_model="gpt-5.5",
        )
    )


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

        for pr_phase in (TaskPhase.PR_OPEN, TaskPhase.GATES):
            with self.subTest(pr_recovery_phase=pr_phase):
                pr_task = TaskState(phase=pr_phase)
                pr_task.mark_recovering()
                with self.assertRaises(ProtocolError):
                    pr_task.update_head("new-head")

    def test_complete_path_requires_every_closure_stage(self) -> None:
        task = TaskState(phase=TaskPhase.VERIFYING)
        drive_closure_to_pr(task)
        task.transition(TaskPhase.GATES)
        with self.assertRaises(ProtocolError):
            task.update_head("")
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.CLOSED)
        task.record_closure(
            ClosureEvidence(
                target_sha=task.head,
                generation=task.generation,
                pr_number=0,
                pr_url="",
                remote_sha=task.head,
                checks=(),
                implementation_model="pending",
                validator_model="pending",
                reviewer_model="pending",
            )
        )
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.CLOSED)
        valid_closure = ClosureEvidence(
            target_sha=task.head,
            generation=task.generation,
            pr_number=1163,
            pr_url="https://github.com/Kizunad/Bong/pull/1163",
            remote_sha=task.head,
            checks=(
                ("review", "SUCCESS"),
                ("e2e", "SUCCESS"),
                ("CodeRabbit", "SUCCESS"),
            ),
            implementation_model="gpt-5",
            validator_model="gpt-5.6-sol-xhigh",
            reviewer_model="gpt-5.5",
        )
        invalid_closures = (
            replace(
                valid_closure,
                pr_url="https://github.com/other/repo/pull/1163",
            ),
            replace(
                valid_closure,
                checks=(("review", "SUCCESS"), ("e2e", "SUCCESS")),
            ),
            replace(valid_closure, implementation_model=" gpt-5 "),
            replace(valid_closure, reviewer_model="Pending"),
            replace(valid_closure, implementation_model="model-id"),
            replace(
                valid_closure,
                checks=(
                    ("review", "FAIL"),
                    ("review", "SUCCESS"),
                    ("e2e", "SUCCESS"),
                    ("CodeRabbit", "SUCCESS"),
                ),
            ),
        )
        for evidence in invalid_closures:
            task.record_closure(evidence)
            with self.assertRaises(ProtocolError):
                task.transition(TaskPhase.CLOSED)
        record_closure_pass(task)
        task.transition(TaskPhase.CLOSED)
        self.assertEqual(task.phase, TaskPhase.CLOSED)
        self.assertTrue(task.final_validated)

        forged = TaskState(
            phase=TaskPhase.GATES,
            head="forged-head",
            final_validated=True,
        )
        record_closure_pass(forged)
        with self.assertRaises(ProtocolError):
            forged.transition(TaskPhase.CLOSED)

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
            task._record_validator(
                ValidatorEvidence(
                    "request-old",
                    "token-old",
                    "v",
                    task.phase,
                    "old-sha",
                    task.generation,
                    "PASS",
                    1.0,
                ),
                task._validator_capability,
            )
        with self.assertRaises(ProtocolError):
            task._record_validator(
                ValidatorEvidence(
                    "request-fail",
                    "token-fail",
                    "v",
                    task.phase,
                    task.head,
                    task.generation,
                    "FAIL",
                    1.0,
                ),
                task._validator_capability,
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
            "request-old",
            "token-old",
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
            validator_task._record_validator(
                old_validator, validator_task._validator_capability
            )
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

    def test_rework_generation_invalidates_queued_validator(self) -> None:
        broker, _, _ = fixture(total=6, live=2)
        task = TaskState(phase=TaskPhase.FIX_VALIDATING)
        task.resolution = TaskPhase.FIXING
        submit(
            broker,
            make_payload("old-validator", "validator", "agent", task),
            task,
        )
        old_generation = task.generation
        task.transition(TaskPhase.FIXING)
        self.assertEqual(task.generation, old_generation + 1)
        task.transition(TaskPhase.FIX_VALIDATING)
        self.assertIsNone(broker.grant_next("validator"))
        self.assertEqual(
            broker.requests["old-validator"].release_reason,
            "STALE_AUTHORITY",
        )

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
        release_request(broker, "v1", first.token_id, "PASS")
        provider.snapshot.live_agents = 4
        self.assertEqual(
            broker.grant_next("validator").payload.request_id, "v2"
        )

        unknown_broker, unknown_provider, _ = fixture(total=None, live=0)
        unknown_task = TaskState(phase=TaskPhase.FIX_VALIDATING)
        for request_id in ("unknown-1", "unknown-2"):
            submit(
                unknown_broker,
                make_payload(
                    request_id, "validator", request_id, unknown_task
                ),
                unknown_task,
            )
        unknown_first = unknown_broker.grant_next("validator")
        unknown_broker.ack(
            "unknown-1",
            unknown_first.token_id,
            unknown_task.phase,
            unknown_task.head,
            unknown_task.generation,
        )
        unknown_broker.bind_validator_spawn(
            "unknown-1",
            unknown_first.token_id,
            "unknown-validator",
            unknown_task.task_id,
            unknown_task.phase,
            unknown_task.head,
            unknown_task.generation,
        )
        unknown_provider.snapshot.live_agent_ids = frozenset(
            {"unknown-validator"}
        )
        unknown_broker.mark_platform_accounted(
            "unknown-1", "unknown-validator"
        )
        self.assertIsNone(unknown_broker.grant_next("validator"))

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
        recover_request(broker, "v1", True)
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
        release_request(broker, "v1", first.token_id, "PASS")
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
        with self.assertRaises(ProtocolError):
            broker.consume_validator_evidence(
                "v1",
                first.token_id,
                ValidatorEvidence(
                    "v1",
                    first.token_id,
                    "validator-two",
                    task.phase,
                    task.head,
                    task.generation,
                    "PASS",
                    1.0,
                ),
            )
        broker.consume_validator_evidence(
            "v1",
            first.token_id,
            ValidatorEvidence(
                "v1",
                first.token_id,
                "validator-one",
                task.phase,
                task.head,
                task.generation,
                "PASS",
                1.0,
            ),
        )
        release_request(broker, "v1", first.token_id, "PASS")
        provider.snapshot.live_agents -= 1
        provider.snapshot.live_agent_ids = (
            provider.snapshot.live_agent_ids - {"validator-one"}
        )
        reuse_task = TaskState(
            task_id="reuse-validator", phase=TaskPhase.FIX_VALIDATING
        )
        submit(
            broker,
            make_payload("v3", "validator", "third", reuse_task),
            reuse_task,
        )
        third = broker.grant_next("validator")
        broker.ack(
            "v3",
            third.token_id,
            reuse_task.phase,
            reuse_task.head,
            reuse_task.generation,
        )
        with self.assertRaises(ProtocolError):
            broker.bind_validator_spawn(
                "v3",
                third.token_id,
                "validator-one",
                reuse_task.task_id,
                reuse_task.phase,
                reuse_task.head,
                reuse_task.generation,
            )
        task.resolution = TaskPhase.FIXING
        task.transition(TaskPhase.GATING)

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
                "   ",
                task.task_id,
                task.phase,
                task.head,
                task.generation,
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
        recover_request(broker, "bind", True)
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
            recover_request(broker, "recover", True)

        task5 = TaskState(
            task_id="release-authority", phase=TaskPhase.GATING
        )
        submit(
            broker, make_payload("release-stale", "compile", "a", task5), task5
        )
        grant5 = broker.grant_next("compile")
        broker.ack(
            "release-stale",
            grant5.token_id,
            task5.phase,
            task5.head,
            task5.generation,
        )
        task5.phase = TaskPhase.BLOCKED
        with self.assertRaises(ProtocolError):
            release_request(broker, "release-stale", grant5.token_id, "PASS")
        self.assertEqual(
            broker.requests["release-stale"].release_reason,
            "STALE_AUTHORITY",
        )

        task6 = TaskState(task_id="lost-entry", phase=TaskPhase.GATING)
        submit(
            broker, make_payload("lost-entry", "compile", "a", task6), task6
        )
        grant6 = broker.grant_next("compile")
        broker.ack(
            "lost-entry",
            grant6.token_id,
            task6.phase,
            task6.head,
            task6.generation,
        )
        task6.update_head("changed-before-lost")
        with self.assertRaises(ProtocolError):
            broker.mark_lost("lost-entry")
        self.assertEqual(broker.held("compile"), 0)

        task7 = TaskState(task_id="followup-entry", phase=TaskPhase.GATING)
        submit(
            broker,
            make_payload("followup-entry", "compile", "a", task7),
            task7,
        )
        grant7 = broker.grant_next("compile")
        broker.ack(
            "followup-entry",
            grant7.token_id,
            task7.phase,
            task7.head,
            task7.generation,
        )
        broker.mark_lost("followup-entry")
        task7.update_head("changed-before-followup")
        with self.assertRaises(ProtocolError):
            broker.followup_payload("followup-entry")
        self.assertEqual(broker.held("compile"), 0)

        task8 = TaskState(task_id="sweep-entry", phase=TaskPhase.GATING)
        submit(
            broker,
            make_payload("sweep-entry", "compile", "a", task8),
            task8,
        )
        grant8 = broker.grant_next("compile")
        broker.ack(
            "sweep-entry",
            grant8.token_id,
            task8.phase,
            task8.head,
            task8.generation,
        )
        broker.mark_lost("sweep-entry")
        task8.update_head("changed-before-sweep")
        self.assertEqual(broker.sweep_recovery(), ["sweep-entry"])
        self.assertEqual(
            broker.requests["sweep-entry"].release_reason,
            "STALE_AUTHORITY",
        )
        self.assertEqual(broker.held("compile"), 0)

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
            release_request(broker, "r1", grant.token_id, "PASS")
        broker.ack(
            "r1", grant.token_id, task.phase, task.head, task.generation
        )
        self.assertTrue(release_request(broker, "r1", grant.token_id, "PASS"))
        self.assertFalse(release_request(broker, "r1", grant.token_id, "PASS"))
        with self.assertRaises(ProtocolError):
            release_request(broker, "r1", grant.token_id, "DIFFERENT")
        with self.assertRaises(ProtocolError):
            cancel_request(broker, "r1", "CANCELLED")
        self.assertTrue(cancel_request(broker, "r2", "CANCELLED"))
        self.assertFalse(cancel_request(broker, "r2", "CANCELLED"))
        with self.assertRaises(ProtocolError):
            cancel_request(broker, "r2", "DIFFERENT")
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
            release_request(broker, "v1", old.token_id, "PASS")
        with self.assertRaises(ProtocolError):
            cancel_request(broker, "v1", "CANCELLED")
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
                release_request(broker, reason, grant.token_id, reason)
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
                    broker.followup_payload(request_id),
                    {
                        "checkpoint": "checkpoint-ok",
                        "request_id": "ok",
                        "token_id": grant.token_id,
                        "agent": "ok",
                        "phase": task.phase.value,
                        "head": task.head,
                        "generation": task.generation,
                    },
                )
                recover_request(broker, request_id, True)
                broker.ack(
                    request_id,
                    grant.token_id,
                    task.phase,
                    task.head,
                    task.generation,
                )
                release_request(broker, request_id, grant.token_id, "PASS")
            elif request_id == "fail":
                recover_request(broker, request_id, False)
                with self.assertRaises(ProtocolError):
                    release_request(broker, request_id, grant.token_id, "LATE")
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
        recover_request(broker, "renew", True)
        self.assertTrue(
            broker.ack(
                "renew",
                grant.token_id,
                task.phase,
                task.head,
                task.generation,
            )
        )
        self.assertTrue(release_request(broker, "renew", grant.token_id, "PASS"))

    def test_recovery_rejects_token_agent_and_checkpoint_drift(self) -> None:
        for field_name in ("token", "agent", "checkpoint"):
            with self.subTest(field=field_name):
                broker, _, _ = fixture()
                task = TaskState(phase=TaskPhase.GATING)
                payload = make_payload("drift", "compile", "agent", task)
                submit(broker, payload, task)
                grant = broker.grant_next("compile")
                broker.ack(
                    "drift",
                    grant.token_id,
                    task.phase,
                    task.head,
                    task.generation,
                )
                broker.mark_lost("drift")
                values = {
                    "token": grant.token_id,
                    "agent": payload.agent,
                    "checkpoint": payload.checkpoint,
                }
                values[field_name] = f"wrong-{field_name}"
                with self.assertRaises(ProtocolError):
                    broker.recovery_result(
                        "drift",
                        values["token"],
                        values["agent"],
                        values["checkpoint"],
                        task.phase,
                        task.head,
                        task.generation,
                        True,
                    )
                self.assertEqual(
                    broker.requests["drift"].release_reason,
                    "STALE_RECOVERY",
                )

    def test_task_and_broker_recovery_states_are_linked(self) -> None:
        broker, _, _ = fixture()
        task = TaskState(phase=TaskPhase.GATING)
        submit(
            broker,
            make_payload("linked", "compile", "agent", task),
            task,
        )
        grant = broker.grant_next("compile")
        broker.ack(
            "linked", grant.token_id, task.phase, task.head, task.generation
        )
        broker.mark_lost("linked")
        task.mark_recovering()
        self.assertEqual(broker.sweep_recovery(), [])
        self.assertEqual(
            broker.followup_payload("linked")["phase"],
            TaskPhase.GATING.value,
        )
        task.recovery_result(True)
        recover_request(broker, "linked", True)
        broker.ack(
            "linked", grant.token_id, task.phase, task.head, task.generation
        )
        self.assertTrue(
            release_request(broker, "linked", grant.token_id, "PASS")
        )

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
