package com.bong.client.ui.headless;

import com.bong.client.ui.contract.surface.UiActionSpec;
import com.bong.client.ui.contract.surface.UiSurfaceProjection;
import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SemanticUiDriverTest {
    @Test
    void usesTheSameActionSchemaAndProducesOneAuthoritativeReceipt() {
        AtomicInteger calls = new AtomicInteger();
        SemanticUiDriver driver = new SemanticUiDriver(Map.of(
            "craft.start", request -> {
                calls.incrementAndGet();
                return UiIntentResult.accepted(request.requestId());
            }
        ));
        UiSurfaceProjection surface = new UiSurfaceProjection(
            "craft", "craft", "session-1", 1L, UiSurfaceProjection.NO_EXPIRY, null,
            Map.of("recipe", "pill"), Map.of(),
            Map.of("craft.start", new UiActionSpec(
                "craft.start", Map.of("recipe", UiActionSpec.ArgumentType.STRING), true, null
            ))
        );
        assertEquals(UiDriver.OpenResult.Status.OPENED, driver.open(surface).status());
        UiDriver.DispatchRequest request = new UiDriver.DispatchRequest(
            "session-1", 1L, "craft.start", "request-1", Map.of("recipe", "pill")
        );
        assertEquals(UiDriver.DispatchResult.Status.ACCEPTED, driver.dispatch(request).status());
        assertEquals(1, calls.get(), "同一 request 只应调用一次 typed handler");
        assertEquals(UiDriver.DispatchResult.Status.DUPLICATE, driver.dispatch(request).status());
        assertEquals(1, calls.get(), "duplicate request 不得再次触发 gameplay action");
        assertEquals(UiDriver.ReceiptResult.Status.AVAILABLE,
            driver.awaitReceipt("session-1", "request-1", 0L).status());
    }

    @Test
    void rejectsSchemaErrorsAndLocallyRejectedIntentsWithoutReceipt() {
        AtomicInteger calls = new AtomicInteger();
        SemanticUiDriver driver = new SemanticUiDriver(Map.of(
            "craft.start", request -> {
                calls.incrementAndGet();
                return UiIntentResult.rejected("recipe is locked");
            }
        ));
        UiSurfaceProjection surface = new UiSurfaceProjection(
            "craft", "craft", "session-1", 2L, UiSurfaceProjection.NO_EXPIRY, null,
            Map.of(), Map.of(),
            Map.of("craft.start", new UiActionSpec(
                "craft.start", Map.of("recipe", UiActionSpec.ArgumentType.STRING), true, null
            ))
        );
        driver.open(surface);
        UiDriver.DispatchResult invalid = driver.dispatch(new UiDriver.DispatchRequest(
            "session-1", 2L, "craft.start", "bad", Map.of("recipe", 2)
        ));
        assertEquals(UiDriver.DispatchResult.Status.INVALID, invalid.status());
        assertEquals(0, calls.get(), "schema 错误必须在 typed handler 前拒绝");
        UiDriver.DispatchResult rejected = driver.dispatch(new UiDriver.DispatchRequest(
            "session-1", 2L, "craft.start", "locked", Map.of("recipe", "pill")
        ));
        assertEquals(UiDriver.DispatchResult.Status.INVALID, rejected.status());
        assertEquals(1, calls.get());
        assertEquals(UiDriver.ReceiptResult.Status.TIMEOUT,
            driver.awaitReceipt("session-1", "locked", 0L).status());
        assertTrue(rejected.reason().contains("locked"));
    }

    @Test
    void rejectsAHandlerThatReturnsAnotherRequestIdentity() {
        SemanticUiDriver driver = new SemanticUiDriver(Map.of(
            "craft.start", request -> UiIntentResult.accepted("different-request")
        ));
        UiSurfaceProjection surface = new UiSurfaceProjection(
            "craft", "craft", "session-identity", 1L, UiSurfaceProjection.NO_EXPIRY, null,
            Map.of(), Map.of(),
            Map.of("craft.start", new UiActionSpec(
                "craft.start", Map.of(), true, null
            ))
        );
        driver.open(surface);

        UiDriver.DispatchResult result = driver.dispatch(new UiDriver.DispatchRequest(
            "session-identity", 1L, "craft.start", "request-a", Map.of()
        ));

        assertEquals(UiDriver.DispatchResult.Status.INVALID, result.status(),
            "handler 回传不同 requestId 时不能生成错误的 authoritative receipt");
        assertEquals(UiDriver.ReceiptResult.Status.TIMEOUT,
            driver.awaitReceipt("session-identity", "request-a", 0L).status());
    }
}
