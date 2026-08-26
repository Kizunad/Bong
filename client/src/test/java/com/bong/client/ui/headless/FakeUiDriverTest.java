package com.bong.client.ui.headless;

import com.bong.client.ui.contract.surface.UiActionSpec;
import com.bong.client.ui.contract.surface.UiSurfaceProjection;
import org.junit.jupiter.api.Test;

import java.util.Map;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

class FakeUiDriverTest {
    @Test
    void openListDispatchReceiptAndDuplicateRequestUseOneSemanticContract() {
        AtomicLong now = new AtomicLong(10L);
        FakeUiDriver driver = new FakeUiDriver(now::get);
        UiSurfaceProjection surface = surface(1L, 100L);
        assertEquals(UiDriver.OpenResult.Status.OPENED, driver.open(surface).status());
        assertEquals(UiDriver.OpenResult.Status.DUPLICATE, driver.open(surface).status());
        assertEquals(UiDriver.SnapshotResult.Status.ACTIVE, driver.snapshot("session-1").status());
        assertEquals(1, driver.listActions("session-1").actions().size());

        UiDriver.DispatchRequest request = new UiDriver.DispatchRequest(
            "session-1", 1L, "craft.start", "request-1", Map.of("recipe", "pill")
        );
        assertEquals(UiDriver.DispatchResult.Status.ACCEPTED, driver.dispatch(request).status());
        assertEquals(UiDriver.DispatchResult.Status.DUPLICATE, driver.dispatch(request).status());
        UiDriver.ReceiptResult receipt = driver.awaitReceipt("session-1", "request-1", 0L);
        assertEquals(UiDriver.ReceiptResult.Status.AVAILABLE, receipt.status());
        assertEquals(UiDriver.UiReceipt.Status.AUTHORITATIVE_ACCEPTED, receipt.receipt().status());
    }

    @Test
    void invalidStaleUnavailableAndUnknownActionsFailClosed() {
        FakeUiDriver driver = new FakeUiDriver(() -> 10L);
        driver.open(surface(2L, 100L));
        assertEquals(UiDriver.DispatchResult.Status.STALE, driver.dispatch(
            new UiDriver.DispatchRequest("session-1", 1L, "craft.start", "stale", Map.of("recipe", "pill"))
        ).status());
        assertEquals(UiDriver.DispatchResult.Status.INVALID, driver.dispatch(
            new UiDriver.DispatchRequest("session-1", 2L, "missing", "missing", Map.of())
        ).status());
        assertEquals(UiDriver.DispatchResult.Status.INVALID, driver.dispatch(
            new UiDriver.DispatchRequest("session-1", 2L, "craft.start", "wrong-type", Map.of("recipe", 2))
        ).status());
        assertEquals(UiDriver.DispatchResult.Status.MISSING, driver.dispatch(
            new UiDriver.DispatchRequest("missing", 0L, "craft.start", "missing-session", Map.of("recipe", "pill"))
        ).status());
    }

    @Test
    void revisionTimeoutPublishExpiryAndCloseAreObservable() {
        AtomicLong now = new AtomicLong(10L);
        FakeUiDriver driver = new FakeUiDriver(now::get);
        driver.open(surface(1L, 20L));
        assertEquals(UiDriver.RevisionResult.Status.TIMEOUT,
            driver.awaitRevision("session-1", 2L, 0L).status());
        assertEquals(FakeUiDriver.PublishResult.Status.PUBLISHED,
            driver.publish(surface(2L, 20L)).status());
        assertEquals(UiDriver.RevisionResult.Status.AVAILABLE,
            driver.awaitRevision("session-1", 2L, 0L).status());
        assertEquals(FakeUiDriver.PublishResult.Status.STALE,
            driver.publish(surface(2L, 20L)).status());

        now.set(20L);
        assertEquals(UiDriver.SnapshotResult.Status.EXPIRED, driver.snapshot("session-1").status());
        assertEquals(UiDriver.DispatchResult.Status.EXPIRED, driver.dispatch(
            new UiDriver.DispatchRequest("session-1", 2L, "craft.start", "expired", Map.of("recipe", "pill"))
        ).status());
        assertEquals(UiDriver.CloseResult.Status.CLOSED, driver.close("session-1").status());
        assertEquals(UiDriver.CloseResult.Status.ALREADY_CLOSED, driver.close("session-1").status());
        assertEquals(UiDriver.ReceiptResult.Status.CLOSED,
            driver.awaitReceipt("session-1", "expired", 0L).status());
        assertEquals(UiDriver.DispatchResult.Status.CLOSED, driver.dispatch(
            new UiDriver.DispatchRequest("session-1", 2L, "craft.start", "late", Map.of("recipe", "pill"))
        ).status());
    }

    @Test
    void invalidTimeoutAndSurfaceOpenInputsAreRejected() {
        FakeUiDriver driver = new FakeUiDriver(() -> 10L);
        assertThrows(IllegalArgumentException.class,
            () -> driver.awaitRevision("missing", 0L, -1L));
        assertEquals(UiDriver.OpenResult.Status.EXPIRED,
            driver.open(surface(1L, 10L)).status());
        assertEquals(UiDriver.CloseResult.Status.MISSING, driver.close("missing").status());
    }

    private static UiSurfaceProjection surface(long revision, long expiresAtMs) {
        UiActionSpec action = new UiActionSpec(
            "craft.start", Map.of("recipe", UiActionSpec.ArgumentType.STRING), true, null);
        return new UiSurfaceProjection(
            "surface-1", "craft", "session-1", revision, expiresAtMs, null,
            Map.of("title", "炼器"), Map.of("row-1", "instance-1"), Map.of(action.actionId(), action)
        );
    }
}
