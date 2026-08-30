package com.bong.client.combat;

import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TerminateClientIntentSinkTest {
    @Test
    void createNewCharacterUsesTypedTransportOnce() {
        int[] calls = {0};
        TerminateClientIntentSink sink = new TerminateClientIntentSink(() -> calls[0]++);

        UiIntentResult result = sink.dispatch(new TerminateIntent.CreateNewCharacter());

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, result.kind());
        assertEquals(1, calls[0], "创建新角色 action 应只穿过 typed transport 一次");
    }

    @Test
    void nullIntentIsRejectedWithoutTransport() {
        int[] calls = {0};
        TerminateClientIntentSink sink = new TerminateClientIntentSink(() -> calls[0]++);

        UiIntentResult result = sink.dispatch(null);

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, result.kind());
        assertEquals(0, calls[0], "空 action 不应触碰 transport");
    }

    @Test
    void transportFailureIsReportedAsLocalError() {
        TerminateClientIntentSink sink = new TerminateClientIntentSink(() -> {
            throw new IllegalStateException("not connected");
        });

        UiIntentResult result = sink.dispatch(new TerminateIntent.CreateNewCharacter());

        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, result.kind());
        assertTrue(result.reason().contains("not connected"), "传输异常应保留可修复的原因");
    }
}
