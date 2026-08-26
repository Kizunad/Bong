package com.bong.client.ui.headless;

import com.bong.client.ui.contract.surface.UiActionSpec;
import com.bong.client.ui.contract.surface.UiSurfaceProjection;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.function.LongSupplier;

/**
 * 契约测试使用的内存语义驱动器。它只有在完成渲染适配器同样的
 * 会话、版本、动作和参数校验后，才接受一次本地派发。
 */
public final class FakeUiDriver implements UiDriver {
    private final LongSupplier clock;
    private final Map<String, UiSurfaceProjection> active = new LinkedHashMap<>();
    private final Map<String, Map<String, UiReceipt>> receipts = new HashMap<>();
    private final Set<String> closedSessions = new LinkedHashSet<>();

    public FakeUiDriver() {
        this(System::currentTimeMillis);
    }

    public FakeUiDriver(LongSupplier clock) {
        this.clock = Objects.requireNonNull(clock, "clock must not be null");
    }

    @Override
    public synchronized OpenResult open(UiSurfaceProjection surface) {
        Objects.requireNonNull(surface, "surface must not be null");
        if (surface.isClosed()) {
            return new OpenResult(OpenResult.Status.INVALID, surface.sessionId(), "surface is closed");
        }
        if (surface.isExpired(clock.getAsLong())) {
            return new OpenResult(OpenResult.Status.EXPIRED, surface.sessionId(), "surface is expired");
        }
        if (closedSessions.contains(surface.sessionId())) {
            return new OpenResult(OpenResult.Status.CLOSED, surface.sessionId(), "session is already closed");
        }
        if (active.containsKey(surface.sessionId())) {
            return new OpenResult(OpenResult.Status.DUPLICATE, surface.sessionId(), "session is already active");
        }
        active.put(surface.sessionId(), surface);
        receipts.putIfAbsent(surface.sessionId(), new LinkedHashMap<>());
        return new OpenResult(OpenResult.Status.OPENED, surface.sessionId(), null);
    }

    /** 为活动会话发布严格递增的语义界面版本。 */
    public synchronized PublishResult publish(UiSurfaceProjection surface) {
        Objects.requireNonNull(surface, "surface must not be null");
        UiSurfaceProjection current = active.get(surface.sessionId());
        if (current == null) {
            return new PublishResult(PublishResult.Status.MISSING, "session is not active");
        }
        if (surface.isClosed()) {
            return new PublishResult(PublishResult.Status.INVALID, "surface is closed");
        }
        if (surface.isExpired(clock.getAsLong())) {
            return new PublishResult(PublishResult.Status.EXPIRED, "surface is expired");
        }
        if (surface.revision() <= current.revision()) {
            return new PublishResult(PublishResult.Status.STALE, "revision must increase monotonically");
        }
        active.put(surface.sessionId(), surface);
        return new PublishResult(PublishResult.Status.PUBLISHED, null);
    }

    @Override
    public synchronized SnapshotResult snapshot(String sessionId) {
        String checked = requireId(sessionId, "sessionId");
        UiSurfaceProjection surface = active.get(checked);
        if (surface == null) {
            return closedSessions.contains(checked)
                ? new SnapshotResult(SnapshotResult.Status.CLOSED, null, "session is closed")
                : new SnapshotResult(SnapshotResult.Status.MISSING, null, "session is unknown");
        }
        if (surface.isExpired(clock.getAsLong())) {
            return new SnapshotResult(SnapshotResult.Status.EXPIRED, null, "surface is expired");
        }
        return new SnapshotResult(SnapshotResult.Status.ACTIVE, surface, null);
    }

    @Override
    public synchronized ActionListResult listActions(String sessionId) {
        SnapshotResult snapshot = snapshot(sessionId);
        if (snapshot.status() != SnapshotResult.Status.ACTIVE) {
            return new ActionListResult(
                ActionListResult.Status.valueOf(snapshot.status().name()),
                List.of(),
                snapshot.reason()
            );
        }
        return new ActionListResult(
            ActionListResult.Status.ACTIVE,
            new ArrayList<>(snapshot.surface().allowedActions().values()),
            null
        );
    }

    @Override
    public synchronized DispatchResult dispatch(DispatchRequest request) {
        Objects.requireNonNull(request, "request must not be null");
        SnapshotResult snapshot = snapshot(request.sessionId());
        if (snapshot.status() == SnapshotResult.Status.MISSING) {
            return rejected(DispatchResult.Status.MISSING, request, snapshot.reason());
        }
        if (snapshot.status() == SnapshotResult.Status.CLOSED) {
            return rejected(DispatchResult.Status.CLOSED, request, snapshot.reason());
        }
        if (snapshot.status() == SnapshotResult.Status.EXPIRED) {
            return rejected(DispatchResult.Status.EXPIRED, request, snapshot.reason());
        }
        if (request.revision() != snapshot.surface().revision()) {
            return rejected(DispatchResult.Status.STALE, request, "request revision is stale");
        }
        Map<String, UiReceipt> sessionReceipts = receipts.computeIfAbsent(
            request.sessionId(), ignored -> new LinkedHashMap<>()
        );
        if (sessionReceipts.containsKey(request.requestId())) {
            return rejected(DispatchResult.Status.DUPLICATE, request, "request id was already dispatched");
        }
        UiActionSpec action = snapshot.surface().action(request.actionId());
        if (action == null) {
            return rejected(DispatchResult.Status.INVALID, request, "unknown action: " + request.actionId());
        }
        UiActionSpec.Validation validation = action.validate(request.args());
        if (!validation.valid()) {
            return rejected(DispatchResult.Status.INVALID, request, validation.reason());
        }
        UiReceipt receipt = new UiReceipt(
            request.sessionId(),
            request.requestId(),
            request.revision(),
            request.actionId(),
            UiReceipt.Status.AUTHORITATIVE_ACCEPTED
        );
        sessionReceipts.put(request.requestId(), receipt);
        return new DispatchResult(
            DispatchResult.Status.ACCEPTED,
            request.requestId(),
            request.revision(),
            null
        );
    }

    @Override
    public synchronized RevisionResult awaitRevision(String sessionId, long minimumRevision, long timeoutMs) {
        requireTimeout(timeoutMs);
        if (minimumRevision < 0L) {
            throw new IllegalArgumentException("minimumRevision must be non-negative");
        }
        SnapshotResult snapshot = snapshot(sessionId);
        if (snapshot.status() == SnapshotResult.Status.MISSING) {
            return new RevisionResult(RevisionResult.Status.MISSING, null, snapshot.reason());
        }
        if (snapshot.status() == SnapshotResult.Status.CLOSED) {
            return new RevisionResult(RevisionResult.Status.CLOSED, null, snapshot.reason());
        }
        if (snapshot.status() == SnapshotResult.Status.EXPIRED) {
            return new RevisionResult(RevisionResult.Status.EXPIRED, null, snapshot.reason());
        }
        if (snapshot.surface().revision() >= minimumRevision) {
            return new RevisionResult(RevisionResult.Status.AVAILABLE, snapshot.surface(), null);
        }
        return new RevisionResult(RevisionResult.Status.TIMEOUT, null, "revision did not arrive before timeout");
    }

    @Override
    public synchronized ReceiptResult awaitReceipt(String sessionId, String requestId, long timeoutMs) {
        requireTimeout(timeoutMs);
        String checkedSession = requireId(sessionId, "sessionId");
        String checkedRequest = requireId(requestId, "requestId");
        if (closedSessions.contains(checkedSession)) {
            return new ReceiptResult(ReceiptResult.Status.CLOSED, null, "session is closed");
        }
        if (!active.containsKey(checkedSession)) {
            return new ReceiptResult(ReceiptResult.Status.MISSING, null, "session is unknown");
        }
        UiReceipt receipt = receipts.getOrDefault(checkedSession, Map.of()).get(checkedRequest);
        return receipt == null
            ? new ReceiptResult(ReceiptResult.Status.TIMEOUT, null, "receipt did not arrive before timeout")
            : new ReceiptResult(ReceiptResult.Status.AVAILABLE, receipt, null);
    }

    @Override
    public synchronized CloseResult close(String sessionId) {
        String checked = requireId(sessionId, "sessionId");
        if (active.remove(checked) != null) {
            closedSessions.add(checked);
            return new CloseResult(CloseResult.Status.CLOSED, checked, null);
        }
        if (closedSessions.contains(checked)) {
            return new CloseResult(CloseResult.Status.ALREADY_CLOSED, checked, "session is already closed");
        }
        return new CloseResult(CloseResult.Status.MISSING, checked, "session is unknown");
    }

    private static DispatchResult rejected(DispatchResult.Status status, DispatchRequest request, String reason) {
        return new DispatchResult(status, request.requestId(), -1L, reason);
    }

    private static void requireTimeout(long timeoutMs) {
        if (timeoutMs < 0L) {
            throw new IllegalArgumentException("timeoutMs must be non-negative");
        }
    }

    private static String requireId(String value, String name) {
        Objects.requireNonNull(value, name + " must not be null");
        if (value.isBlank()) {
            throw new IllegalArgumentException(name + " must not be blank");
        }
        return value;
    }

    public record PublishResult(Status status, String reason) {
        public enum Status {
            PUBLISHED,
            MISSING,
            STALE,
            EXPIRED,
            INVALID
        }
    }
}
