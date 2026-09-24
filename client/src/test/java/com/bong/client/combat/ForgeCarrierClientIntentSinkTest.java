package com.bong.client.combat;

import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ForgeCarrierClientIntentSinkTest {
    @Test
    void beginActionPreservesItemAndQiRatio() {
        String[] item = {null};
        double[] ratio = {-1.0};
        ForgeCarrierClientIntentSink sink = new ForgeCarrierClientIntentSink((value, qiInvest) -> {
            item[0] = value;
            ratio[0] = qiInvest;
        });

        UiIntentResult result = sink.dispatch(new ForgeCarrierIntent.Begin("needle", 0.75));

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, result.kind());
        assertEquals("needle", item[0]);
        assertEquals(0.75, ratio[0]);
    }

    @Test
    void nullIntentIsRejectedWithoutTransport() {
        int[] calls = {0};
        ForgeCarrierClientIntentSink sink = new ForgeCarrierClientIntentSink((item, qiInvest) -> calls[0]++);

        UiIntentResult result = sink.dispatch(null);

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, result.kind());
        assertEquals(0, calls[0], "空 action 不应触碰 transport");
    }

    @Test
    void transportFailureIsReportedAsLocalError() {
        ForgeCarrierClientIntentSink sink = new ForgeCarrierClientIntentSink((item, qiInvest) -> {
            throw new IllegalStateException("not connected");
        });

        UiIntentResult result = sink.dispatch(new ForgeCarrierIntent.Begin("dagger", 0.5));

        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, result.kind());
        assertTrue(result.reason().contains("not connected"), "传输异常应保留可修复原因");
    }

    @Test
    void intentRejectsBlankOrOutOfRangeValues() {
        org.junit.jupiter.api.Assertions.assertThrows(
            IllegalArgumentException.class,
            () -> new ForgeCarrierIntent.Begin(" ", 0.5));
        org.junit.jupiter.api.Assertions.assertThrows(
            IllegalArgumentException.class,
            () -> new ForgeCarrierIntent.Begin("dagger", -0.1));
        org.junit.jupiter.api.Assertions.assertThrows(
            IllegalArgumentException.class,
            () -> new ForgeCarrierIntent.Begin("dagger", 1.1));
    }
}
