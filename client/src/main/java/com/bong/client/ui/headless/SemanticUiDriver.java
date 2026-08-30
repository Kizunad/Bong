package com.bong.client.ui.headless;

import com.bong.client.ui.contract.surface.UiActionSpec;
import com.bong.client.ui.contract.surface.UiSurfaceProjection;
import com.bong.client.ui.intent.UiIntentResult;

import java.util.HashSet;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.function.LongSupplier;

/**
 * 把语义 surface 接到 typed action handler 的无渲染驱动器。
 *
 * <p>它只复用真实 action schema、session/revision 校验和 receipt 语义，
 * 不暴露屏幕 callback、Store 或像素坐标，供 bot/contract roundtrip 使用。</p>
 */
public final class SemanticUiDriver implements UiDriver {
    @FunctionalInterface
    public interface ActionHandler {
        /** handler 必须看到完整请求，才能把 transport request identity 与语义请求对拍。 */
        UiIntentResult dispatch(UiDriver.DispatchRequest request);
    }

    private final FakeUiDriver lifecycle;
    private final Map<String, ActionHandler> handlers;
    private final Set<String> dispatchedRequests = new HashSet<>();

    public SemanticUiDriver(Map<String, ActionHandler> handlers) {
        this(System::currentTimeMillis, handlers);
    }

    public SemanticUiDriver(LongSupplier clock, Map<String, ActionHandler> handlers) {
        lifecycle = new FakeUiDriver(Objects.requireNonNull(clock, "clock must not be null"));
        this.handlers = Map.copyOf(Objects.requireNonNull(handlers, "handlers must not be null"));
    }

    @Override public OpenResult open(UiSurfaceProjection surface) { return lifecycle.open(surface); }
    @Override public SnapshotResult snapshot(String sessionId) { return lifecycle.snapshot(sessionId); }
    @Override public ActionListResult listActions(String sessionId) { return lifecycle.listActions(sessionId); }
    @Override public RevisionResult awaitRevision(String sessionId, long minimumRevision, long timeoutMs) {
        return lifecycle.awaitRevision(sessionId, minimumRevision, timeoutMs);
    }
    @Override public ReceiptResult awaitReceipt(String sessionId, String requestId, long timeoutMs) {
        return lifecycle.awaitReceipt(sessionId, requestId, timeoutMs);
    }
    @Override public CloseResult close(String sessionId) { return lifecycle.close(sessionId); }

    /** 为活动 session 发布新的 authoritative projection，沿用 FakeUiDriver 的 revision 规则。 */
    public FakeUiDriver.PublishResult publish(UiSurfaceProjection surface) {
        return lifecycle.publish(surface);
    }

    @Override
    public synchronized DispatchResult dispatch(DispatchRequest request) {
        Objects.requireNonNull(request, "request must not be null");
        SnapshotResult current = lifecycle.snapshot(request.sessionId());
        if (current.status() != SnapshotResult.Status.ACTIVE) {
            return new DispatchResult(
                DispatchResult.Status.valueOf(current.status().name()), request.requestId(), -1L, current.reason()
            );
        }
        if (request.revision() != current.surface().revision()) {
            return rejected(DispatchResult.Status.STALE, request, "request revision is stale");
        }
        String requestKey = request.sessionId() + "\u0000" + request.requestId();
        if (!dispatchedRequests.add(requestKey)) {
            return rejected(DispatchResult.Status.DUPLICATE, request, "request id was already dispatched");
        }
        UiActionSpec action = current.surface().action(request.actionId());
        if (action == null) {
            dispatchedRequests.remove(requestKey);
            return rejected(DispatchResult.Status.INVALID, request, "unknown action: " + request.actionId());
        }
        UiActionSpec.Validation validation = action.validate(request.args());
        if (!validation.valid()) {
            dispatchedRequests.remove(requestKey);
            return rejected(DispatchResult.Status.INVALID, request, validation.reason());
        }
        ActionHandler handler = handlers.get(request.actionId());
        if (handler == null) {
            dispatchedRequests.remove(requestKey);
            return rejected(DispatchResult.Status.INVALID, request, "no typed handler for action: " + request.actionId());
        }
        UiIntentResult result;
        try {
            result = Objects.requireNonNull(handler.dispatch(request), "action handler result must not be null");
        } catch (RuntimeException failure) {
            dispatchedRequests.remove(requestKey);
            return rejected(DispatchResult.Status.INVALID, request, "action handler failed: " + failure.getMessage());
        }
        if (result.kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) {
            dispatchedRequests.remove(requestKey);
            return rejected(DispatchResult.Status.INVALID, request,
                result.reason() == null ? "typed action was not locally accepted" : result.reason());
        }
        if (!request.requestId().equals(result.requestId())) {
            dispatchedRequests.remove(requestKey);
            return rejected(DispatchResult.Status.INVALID, request,
                "typed action request id does not match dispatch request");
        }
        DispatchResult accepted = lifecycle.dispatch(request);
        if (accepted.status() != DispatchResult.Status.ACCEPTED) {
            dispatchedRequests.remove(requestKey);
        }
        return accepted;
    }

    private static DispatchResult rejected(DispatchResult.Status status, DispatchRequest request, String reason) {
        return new DispatchResult(status, request.requestId(), -1L, reason);
    }
}
