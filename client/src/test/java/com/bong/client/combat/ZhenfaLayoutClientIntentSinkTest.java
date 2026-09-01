package com.bong.client.combat;

import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ZhenfaLayoutClientIntentSinkTest {
    @Test
    void placePreservesTheSemanticIntent() {
        ZhenfaLayoutIntent.Place intent = new ZhenfaLayoutIntent.Place(
            11, 64, -3, "blast_trap", "common_stone", 0.25, null, 9001L, "north");
        ZhenfaLayoutIntent.Place[] captured = {null};
        ZhenfaLayoutClientIntentSink sink = new ZhenfaLayoutClientIntentSink(value -> captured[0] = value);

        UiIntentResult result = sink.dispatch(intent);

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, result.kind());
        assertEquals(intent, captured[0], "typed action 应完整到达 transport");
    }

    @Test
    void nullIntentIsRejectedWithoutTransport() {
        int[] calls = {0};
        ZhenfaLayoutClientIntentSink sink = new ZhenfaLayoutClientIntentSink(value -> calls[0]++);

        UiIntentResult result = sink.dispatch(null);

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, result.kind());
        assertEquals(0, calls[0], "空 action 不应触碰 transport");
    }

    @Test
    void transportFailureIsReportedAsLocalError() {
        ZhenfaLayoutClientIntentSink sink = new ZhenfaLayoutClientIntentSink(value -> {
            throw new IllegalStateException("not connected");
        });

        UiIntentResult result = sink.dispatch(new ZhenfaLayoutIntent.Place(
            0, 64, 0, "trap", "common_stone", 0.1, "proximity", null, null));

        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, result.kind());
        assertTrue(result.reason().contains("not connected"), "传输异常应保留可修复原因");
    }

    @Test
    void semanticIntentRejectsInvalidValues() {
        org.junit.jupiter.api.Assertions.assertThrows(IllegalArgumentException.class,
            () -> new ZhenfaLayoutIntent.Place(0, 64, 0, " ", "common_stone", 0.1, null, null, null));
        org.junit.jupiter.api.Assertions.assertThrows(IllegalArgumentException.class,
            () -> new ZhenfaLayoutIntent.Place(0, 64, 0, "trap", "common_stone", 1.1, null, null, null));
        org.junit.jupiter.api.Assertions.assertThrows(IllegalArgumentException.class,
            () -> new ZhenfaLayoutIntent.Place(0, 64, 0, "trap", "common_stone", 0.1, null, -1L, null));
    }
}
