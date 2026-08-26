package com.bong.client.ui.headless;

import com.bong.client.ui.contract.surface.UiActionSpec;
import com.bong.client.ui.contract.surface.UiSurfaceProjection;

import java.util.List;
import java.util.Map;
import java.util.Objects;

/** 机器人与渲染适配器共用的无头消费契约。 */
public interface UiDriver {
    OpenResult open(UiSurfaceProjection surface);

    SnapshotResult snapshot(String sessionId);

    ActionListResult listActions(String sessionId);

    DispatchResult dispatch(DispatchRequest request);

    RevisionResult awaitRevision(String sessionId, long minimumRevision, long timeoutMs);

    ReceiptResult awaitReceipt(String sessionId, String requestId, long timeoutMs);

    CloseResult close(String sessionId);

    record DispatchRequest(
        String sessionId,
        long revision,
        String actionId,
        String requestId,
        Map<String, ?> args
    ) {
        public DispatchRequest {
            sessionId = requireId(sessionId, "sessionId");
            actionId = requireId(actionId, "actionId");
            requestId = requireId(requestId, "requestId");
            if (revision < 0L) {
                throw new IllegalArgumentException("revision must be non-negative");
            }
            args = args == null ? null : Map.copyOf(args);
        }
    }

    record OpenResult(Status status, String sessionId, String reason) {
        public OpenResult {
            Objects.requireNonNull(status, "status must not be null");
            sessionId = sessionId == null ? null : requireId(sessionId, "sessionId");
        }

        public enum Status {
            OPENED,
            DUPLICATE,
            EXPIRED,
            CLOSED,
            INVALID
        }
    }

    record SnapshotResult(Status status, UiSurfaceProjection surface, String reason) {
        public SnapshotResult {
            Objects.requireNonNull(status, "status must not be null");
            if (status == Status.ACTIVE && surface == null) {
                throw new IllegalArgumentException("active snapshot requires a surface");
            }
            if (status != Status.ACTIVE && surface != null) {
                throw new IllegalArgumentException("inactive snapshot cannot carry a surface");
            }
        }

        public enum Status {
            ACTIVE,
            MISSING,
            EXPIRED,
            CLOSED
        }
    }

    record ActionListResult(Status status, List<UiActionSpec> actions, String reason) {
        public ActionListResult {
            Objects.requireNonNull(status, "status must not be null");
            actions = List.copyOf(Objects.requireNonNull(actions, "actions must not be null"));
            if (status != Status.ACTIVE && !actions.isEmpty()) {
                throw new IllegalArgumentException("inactive action list cannot carry actions");
            }
        }

        public enum Status {
            ACTIVE,
            MISSING,
            EXPIRED,
            CLOSED
        }
    }

    record DispatchResult(Status status, String requestId, long revision, String reason) {
        public DispatchResult {
            Objects.requireNonNull(status, "status must not be null");
            if (status == Status.ACCEPTED && (requestId == null || revision < 0L)) {
                throw new IllegalArgumentException("accepted dispatch requires request identity and revision");
            }
        }

        public enum Status {
            ACCEPTED,
            MISSING,
            CLOSED,
            EXPIRED,
            STALE,
            INVALID,
            DUPLICATE
        }
    }

    record RevisionResult(Status status, UiSurfaceProjection surface, String reason) {
        public RevisionResult {
            Objects.requireNonNull(status, "status must not be null");
            if (status == Status.AVAILABLE && surface == null) {
                throw new IllegalArgumentException("available revision requires a surface");
            }
            if (status != Status.AVAILABLE && surface != null) {
                throw new IllegalArgumentException("unavailable revision cannot carry a surface");
            }
        }

        public enum Status {
            AVAILABLE,
            TIMEOUT,
            MISSING,
            EXPIRED,
            CLOSED
        }
    }

    record UiReceipt(String sessionId, String requestId, long revision, String actionId, Status status) {
        public UiReceipt {
            sessionId = requireId(sessionId, "sessionId");
            requestId = requireId(requestId, "requestId");
            actionId = requireId(actionId, "actionId");
            if (revision < 0L) {
                throw new IllegalArgumentException("revision must be non-negative");
            }
            Objects.requireNonNull(status, "status must not be null");
        }

        public enum Status {
            AUTHORITATIVE_ACCEPTED
        }
    }

    record ReceiptResult(Status status, UiReceipt receipt, String reason) {
        public ReceiptResult {
            Objects.requireNonNull(status, "status must not be null");
            if (status == Status.AVAILABLE && receipt == null) {
                throw new IllegalArgumentException("available receipt requires a receipt");
            }
            if (status != Status.AVAILABLE && receipt != null) {
                throw new IllegalArgumentException("unavailable receipt cannot carry a receipt");
            }
        }

        public enum Status {
            AVAILABLE,
            TIMEOUT,
            MISSING,
            CLOSED
        }
    }

    record CloseResult(Status status, String sessionId, String reason) {
        public CloseResult {
            Objects.requireNonNull(status, "status must not be null");
            sessionId = sessionId == null ? null : requireId(sessionId, "sessionId");
        }

        public enum Status {
            CLOSED,
            ALREADY_CLOSED,
            MISSING
        }
    }

    private static String requireId(String value, String name) {
        Objects.requireNonNull(value, name + " must not be null");
        if (value.isBlank()) {
            throw new IllegalArgumentException(name + " must not be blank");
        }
        return value;
    }
}
