#!/usr/bin/env python3
"""BugFix 调度协议纯内存 dry-run；不访问 GitHub、不修改仓库。"""

from __future__ import annotations

import unittest
from dataclasses import dataclass, replace
from enum import Enum
import json
from pathlib import Path
import re
from typing import Optional


class ProtocolError(RuntimeError):
    pass


CONTRACT_PATTERN = re.compile(
    r"```harness-contract\s*(\{.*?\})\s*```", re.DOTALL
)
ADAPTER_OPERATIONS = {"spawn", "message", "wait", "resume", "status"}
ADAPTER_SCRIPT_ROOT = Path(__file__).resolve().parents[3]


def load_harness_contract(skill_path: Path) -> dict[str, object]:
    match = CONTRACT_PATTERN.search(skill_path.read_text(encoding="utf-8"))
    if match is None:
        raise ProtocolError("SKILL.md harness contract block missing")
    contract = json.loads(match.group(1))
    if contract.get("version") != 1:
        raise ProtocolError("unsupported harness contract version")
    adapters = contract.get("adapters")
    if not isinstance(adapters, dict) or not adapters:
        raise ProtocolError("harness adapters missing")
    for name, adapter in adapters.items():
        if not isinstance(adapter, dict):
            raise ProtocolError(f"adapter {name} is not an object")
        required = adapter.get("required_tools")
        if (
            not isinstance(required, list)
            or not required
            or any(not isinstance(tool, str) or not tool for tool in required)
            or len(required) != len(set(required))
        ):
            raise ProtocolError(f"adapter {name} required_tools invalid")
        missing_operations = ADAPTER_OPERATIONS - adapter.keys()
        if missing_operations:
            raise ProtocolError(
                f"adapter {name} operations missing: {missing_operations}"
            )
        script = adapter.get("adapter")
        if script is not None:
            if not isinstance(script, str) or not script:
                raise ProtocolError(f"adapter {name} script invalid")
            script_path = ADAPTER_SCRIPT_ROOT / script
            if not script_path.is_file():
                raise ProtocolError(f"adapter {name} script missing: {script}")
    return contract


def select_harness_adapter(
    contract: dict[str, object], exposed_tools: set[str]
) -> Optional[str]:
    adapters = contract["adapters"]
    matches = [
        name
        for name, adapter in adapters.items()
        if set(adapter["required_tools"]).issubset(exposed_tools)
    ]
    if len(matches) > 1:
        raise ProtocolError("ambiguous harness capability set")
    return matches[0] if matches else None


class TaskPhase(str, Enum):
    DISPATCHED = "DISPATCHED"
    CLAIMED = "CLAIMED"
    PROMOTED = "PROMOTED"
    VERIFYING = "VERIFYING"
    FIXING = "FIXING"
    NOT_BUG = "NOT_BUG"
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
    TaskPhase.FIXING: {TaskPhase.GATING, TaskPhase.BLOCKED},
    TaskPhase.NOT_BUG: {TaskPhase.GATING, TaskPhase.BLOCKED},
    TaskPhase.GATING: {
        TaskPhase.REBASING,
        TaskPhase.FIXING,
        TaskPhase.NOT_BUG,
        TaskPhase.BLOCKED,
    },
    TaskPhase.REBASING: {TaskPhase.ARCHIVING, TaskPhase.BLOCKED},
    TaskPhase.ARCHIVING: {TaskPhase.PR_OPEN, TaskPhase.BLOCKED},
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
    reviewer_model: str


@dataclass
class TaskState:
    task_id: str = "task"
    phase: TaskPhase = TaskPhase.DISPATCHED
    head: str = "abc123"
    generation: int = 0
    resolution: Optional[TaskPhase] = None
    recovery_from: Optional[TaskPhase] = None
    gated: bool = False
    rebased: bool = False
    archived: bool = False
    gate_evidence: dict[TaskPhase, GateEvidence] = None
    sync_evidence: Optional[SyncEvidence] = None
    closure_evidence: Optional[ClosureEvidence] = None
    repository: str = "Kizunad/Bong"
    required_checks: tuple[str, ...] = ("review", "e2e")
    expected_models: tuple[str, str] = (
        "gpt-5",
        "gpt-5.5",
    )

    def __post_init__(self) -> None:
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
        if self.phase == TaskPhase.GATING and target == TaskPhase.REBASING:
            self._require_gate_pass(TaskPhase.GATING)
            self.gated = True
        if self.phase == TaskPhase.REBASING and target == TaskPhase.ARCHIVING:
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
        if self.phase == TaskPhase.ARCHIVING and target == TaskPhase.PR_OPEN:
            if not self.archived:
                raise ProtocolError("archive commit evidence missing")
            self.archived = True
        if self.phase == TaskPhase.GATES and target == TaskPhase.CLOSED:
            evidence = self.closure_evidence
            checks = dict(evidence.checks) if evidence is not None else {}
            models = (
                evidence.implementation_model,
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
                or not all((self.gated, self.rebased, self.archived))
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
                or len(models) != 2
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
        self.phase = target

    def _reset_closure_milestones(self) -> None:
        self.gated = False
        self.rebased = False
        self.archived = False
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
        self.gate_evidence.clear()
        self.sync_evidence = None
        self.closure_evidence = None
        self.gated = False
        self.rebased = False
        self.archived = False

    def record_archive(self, archived_head: str) -> None:
        if self.phase != TaskPhase.ARCHIVING or archived_head == self.head:
            raise ProtocolError("archive must create a new HEAD")
        self.head = archived_head
        self.generation += 1
        self.gate_evidence.clear()
        self.sync_evidence = None
        self.closure_evidence = None
        self.archived = True

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
    main_in_snapshot: bool = True

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
        )
        return min(task_slots, available)


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
    ):
        self.capacity_provider = capacity_provider
        self.clock = clock
        self.authority = authority
        self.grant_ttl = grant_ttl
        self.recovery_ttl = recovery_ttl
        self.logical_capacity = {
            "compile": compile_capacity,
        }
        self.requests: dict[str, RequestState] = {}
        self.queues: dict[str, list[str]] = {"compile": []}
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
        return self.capacity_provider().implementation_limit(
            user_n,
            active_implementations=active_implementations,
        )

    def available(self, token_type: str) -> int:
        logical = self.logical_capacity[token_type] - self.held(token_type)
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
            return state
        return None

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
        state.status = status
        state.release_reason = reason
        state.pre_recovery_status = None
        state.recovery_deadline = None

    def _authority_matches(self, state: RequestState) -> bool:
        task = self.authority.get(state.payload.task)
        phase_matches = task is not None and (
            task.phase == state.payload.phase
            or (
                state.status == RequestStatus.RECOVERING
                and task.phase == TaskPhase.RECOVERING
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
    provider = MutableCapacityProvider(PlatformSnapshot(total, live))
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
            ),
            implementation_model="gpt-5",
            reviewer_model="gpt-5.5",
        )
    )


def drive_closure_to_pr(
    task: TaskState,
    branch: TaskPhase = TaskPhase.FIXING,
    archive_suffix: str = "archive",
) -> None:
    task.transition(branch)
    task.transition(TaskPhase.GATING)
    record_gate_pass(task)
    task.transition(TaskPhase.REBASING)
    record_sync_pass(task)
    record_gate_pass(task)
    task.transition(TaskPhase.ARCHIVING)
    task.record_archive(f"{task.head}-{archive_suffix}")
    task.transition(TaskPhase.PR_OPEN)


class StateMachineDryRun(unittest.TestCase):
    def test_skill_harness_contract_is_executable_and_fail_closed(self) -> None:
        skill_path = Path(__file__).resolve().parents[1] / "SKILL.md"
        contract = load_harness_contract(skill_path)
        adapters = contract["adapters"]
        for name, adapter in adapters.items():
            with self.subTest(adapter=name):
                required = set(adapter["required_tools"])
                self.assertEqual(
                    select_harness_adapter(contract, required), name
                )
                for missing in required:
                    self.assertIsNone(
                        select_harness_adapter(contract, required - {missing})
                    )
        self.assertEqual(
            select_harness_adapter(contract, {"Bash", "Monitor"}),
            "claude_code_cli",
        )
        self.assertIsNone(select_harness_adapter(contract, {"Bash"}))
        self.assertIsNone(select_harness_adapter(contract, {"Monitor"}))
        self.assertIsNone(
            select_harness_adapter(
                contract,
                {"claude", "agents", "--background", "--resume"},
            )
        )
        self.assertIsNone(
            select_harness_adapter(contract, {"Agent", "SendMessage"})
        )
        self.assertIsNone(
            select_harness_adapter(
                contract, {"Agent", "SendMessage", "TaskList"}
            )
        )

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
            TaskPhase.FIXING: {TaskPhase.GATING, TaskPhase.BLOCKED},
            TaskPhase.NOT_BUG: {TaskPhase.GATING, TaskPhase.BLOCKED},
            TaskPhase.GATING: {
                TaskPhase.REBASING,
                TaskPhase.FIXING,
                TaskPhase.NOT_BUG,
                TaskPhase.BLOCKED,
            },
            TaskPhase.REBASING: {TaskPhase.ARCHIVING, TaskPhase.BLOCKED},
            TaskPhase.ARCHIVING: {TaskPhase.PR_OPEN, TaskPhase.BLOCKED},
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

    def test_task_recovering_cannot_bypass_nonrecovering_request(self) -> None:
        for request_status in ("queued", "granted", "acked"):
            with self.subTest(request_status=request_status):
                broker, _, _ = fixture()
                task = TaskState(
                    task_id=f"task-{request_status}",
                    phase=TaskPhase.GATING,
                )
                request_id = f"request-{request_status}"
                submit(
                    broker,
                    make_payload(request_id, "compile", "agent", task),
                    task,
                )
                grant = None
                if request_status != "queued":
                    grant = broker.grant_next("compile")
                if request_status == "acked":
                    broker.ack(
                        request_id,
                        grant.token_id,
                        task.phase,
                        task.head,
                        task.generation,
                    )
                task.mark_recovering()
                if request_status == "queued":
                    self.assertIsNone(broker.grant_next("compile"))
                elif request_status == "granted":
                    with self.assertRaises(ProtocolError):
                        broker.ack(
                            request_id,
                            grant.token_id,
                            TaskPhase.GATING,
                            task.head,
                            task.generation,
                        )
                else:
                    with self.assertRaises(ProtocolError):
                        release_request(
                            broker, request_id, grant.token_id, "PASS"
                        )
                self.assertEqual(broker.held("compile"), 0)

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


    def test_direct_path_requires_gate_and_archive_evidence(self) -> None:
        task = TaskState(phase=TaskPhase.VERIFYING)
        task.transition(TaskPhase.FIXING)
        task.transition(TaskPhase.GATING)
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.REBASING)
        record_gate_pass(task)
        task.transition(TaskPhase.REBASING)
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.ARCHIVING)
        record_sync_pass(task)
        record_gate_pass(task)
        task.transition(TaskPhase.ARCHIVING)
        with self.assertRaises(ProtocolError):
            task.transition(TaskPhase.PR_OPEN)
        task.record_archive("archive-head")
        task.transition(TaskPhase.PR_OPEN)
        self.assertEqual(task.phase, TaskPhase.PR_OPEN)

    def test_complete_path_requires_current_pr_gates(self) -> None:
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
            ),
            implementation_model="gpt-5",
            reviewer_model="gpt-5.5",
        )
        invalid_closures = (
            replace(
                valid_closure,
                pr_url="https://github.com/other/repo/pull/1163",
            ),
            replace(valid_closure, checks=(("review", "SUCCESS"),)),
            replace(valid_closure, implementation_model=" gpt-5 "),
            replace(valid_closure, reviewer_model="Pending"),
            replace(valid_closure, implementation_model="model-id"),
            replace(
                valid_closure,
                checks=(
                    ("review", "FAIL"),
                    ("review", "SUCCESS"),
                    ("e2e", "SUCCESS"),
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
        forged = TaskState(
            phase=TaskPhase.GATES,
            head="forged-head",
            gated=True,
            rebased=True,
            archived=False,
        )
        record_closure_pass(forged)
        with self.assertRaises(ProtocolError):
            forged.transition(TaskPhase.CLOSED)

    def test_rework_resets_current_head_evidence(self) -> None:
        task = TaskState(phase=TaskPhase.VERIFYING)
        drive_closure_to_pr(task, archive_suffix="archive-1")
        task.transition(TaskPhase.GATES)
        task.transition(TaskPhase.FIXING)
        self.assertFalse(task.gated)
        self.assertFalse(task.rebased)
        self.assertFalse(task.archived)
        task.transition(TaskPhase.GATING)
        record_gate_pass(task)
        task.transition(TaskPhase.REBASING)
        record_sync_pass(task)
        record_gate_pass(task)
        task.transition(TaskPhase.ARCHIVING)
        task.record_archive("archive-2")
        task.transition(TaskPhase.PR_OPEN)
        self.assertEqual(task.phase, TaskPhase.PR_OPEN)

    def test_phase_set_contains_only_operational_phases(self) -> None:
        self.assertEqual(set(TOKEN_PHASES), {"compile"})
        self.assertEqual(len(TaskPhase), 14)

    def test_platform_capacity_uses_only_live_agents(self) -> None:
        snapshot = PlatformSnapshot(total=4, live_agents=1)
        self.assertEqual(snapshot.implementation_limit(9), 3)
        self.assertEqual(PlatformSnapshot(6, 1).implementation_limit(4), 4)
        self.assertEqual(PlatformSnapshot(4, 3).implementation_limit(9), 1)
        self.assertEqual(
            PlatformSnapshot(4, 1).implementation_limit(
                9, outstanding_reservations=1
            ),
            2,
        )
        self.assertEqual(
            PlatformSnapshot(4, 0, main_in_snapshot=False).implementation_limit(9),
            3,
        )
        self.assertEqual(PlatformSnapshot(None, 99).implementation_limit(9), 9)
        broker, _, _ = fixture(total=4, live=4)
        self.assertEqual(broker.implementation_limit(9), 0)

    def test_token_phase_binding_and_fifo(self) -> None:
        broker, _, _ = fixture()
        task = TaskState(phase=TaskPhase.GATING)
        for phase in (
            TaskPhase.VERIFYING,
            TaskPhase.FIXING,
            TaskPhase.NOT_BUG,
            TaskPhase.GATING,
            TaskPhase.REBASING,
        ):
            with self.subTest(compile_phase=phase):
                local_broker, _, _ = fixture()
                local_task = TaskState(
                    task_id=f"compile-{phase.value}", phase=phase
                )
                if phase in BRANCH_PHASES:
                    local_task.resolution = phase
                state = submit(
                    local_broker,
                    make_payload(
                        f"compile-{phase.value}", "compile", "agent", local_task
                    ),
                    local_task,
                )
                self.assertEqual(state.status, RequestStatus.QUEUED)
        with self.assertRaises(ProtocolError):
            invalid_task = TaskState(task_id="invalid", phase=TaskPhase.PR_OPEN)
            submit(
                broker,
                make_payload("invalid", "compile", "a", invalid_task),
                invalid_task,
            )
        first_payload = make_payload("r1", "compile", "a", task)
        first = submit(broker, first_payload, task)
        submit(broker, make_payload("r2", "compile", "b", task), task)
        self.assertEqual(broker.queue_position("r1"), 1)
        self.assertEqual(broker.queue_position("r2"), 2)
        grant = broker.grant_next("compile")
        with self.assertRaises(ProtocolError):
            release_request(broker, "r1", grant.token_id, "PASS")
        broker.ack(
            "r1", grant.token_id, task.phase, task.head, task.generation
        )
        self.assertTrue(release_request(broker, "r1", grant.token_id, "PASS"))
        self.assertTrue(cancel_request(broker, "r2", "CANCELLED"))

    def test_compile_grant_ttl_and_old_token_rejection(self) -> None:
        broker, _, clock = fixture(grant_ttl=10)
        task = TaskState(phase=TaskPhase.GATING)
        submit(broker, make_payload("r1", "compile", "a", task), task)
        submit(broker, make_payload("r2", "compile", "b", task), task)
        old = broker.grant_next("compile")
        clock.advance(9.999)
        self.assertTrue(
            broker.ack("r1", old.token_id, task.phase, task.head, task.generation)
        )
        release_request(broker, "r1", old.token_id, "PASS")
        next_grant = broker.grant_next("compile")
        self.assertEqual(next_grant.payload.request_id, "r2")

if __name__ == "__main__":
    result = unittest.main(verbosity=2, exit=False).result
    if not result.wasSuccessful():
        raise SystemExit(1)
    print("DRY-RUN PASS: BugFix 状态机契约全部通过")
