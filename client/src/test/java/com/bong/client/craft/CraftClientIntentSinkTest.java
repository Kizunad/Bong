package com.bong.client.craft;

import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CraftClientIntentSinkTest {
    @Test
    void startAndCancelMapToTypedTransportExactlyOnce() {
        List<String> calls = new ArrayList<>();
        CraftClientIntentSink sink = new CraftClientIntentSink(transport(calls));

        UiIntentResult start = sink.dispatch(new CraftIntent.Start(" rough_handle ", 3));
        UiIntentResult cancel = sink.dispatch(new CraftIntent.Cancel());

        assertEquals(List.of("start:rough_handle:3", "cancel"), calls,
            "typed intent 必须按顺序映射到既有 start/cancel transport");
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, start.kind());
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, cancel.kind());
        assertNull(start.requestId(), "当前 craft sender 没有 tracked request id，不能伪造 identity");
        assertNull(start.reason(), "本地 accepted 不代表 server success，也不应携带成功理由");
    }

    @Test
    void invalidArgumentsRejectBeforeTransport() {
        List<String> calls = new ArrayList<>();
        CraftClientIntentSink sink = new CraftClientIntentSink(transport(calls));

        List<UiIntentResult> results = List.of(
            sink.dispatch(null),
            sink.dispatch(new CraftIntent.Start(null, 1)),
            sink.dispatch(new CraftIntent.Start("  ", 1)),
            sink.dispatch(new CraftIntent.Start("valid", 0)),
            sink.dispatch(new CraftIntent.Start("valid", -1))
        );

        assertTrue(results.stream().allMatch(
            result -> result.kind() == UiIntentResult.Kind.LOCAL_REJECTED));
        assertTrue(results.stream().allMatch(result -> result.reason() != null));
        assertTrue(calls.isEmpty(), "任何非法参数都必须在 sender 前 fail closed");
    }

    @Test
    void transportFailureIsLocalErrorRatherThanServerRejection() {
        CraftClientIntentSink sink = new CraftClientIntentSink(new CraftClientIntentSink.Transport() {
            @Override
            public void start(String recipeId, int quantity) {
                throw new IllegalStateException("channel unavailable");
            }

            @Override
            public void cancel() {
                throw new IllegalStateException();
            }
        });

        UiIntentResult start = sink.dispatch(new CraftIntent.Start("rough_handle", 1));
        UiIntentResult cancel = sink.dispatch(new CraftIntent.Cancel());

        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, start.kind());
        assertEquals("craft transport failed: channel unavailable", start.reason());
        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, cancel.kind());
        assertEquals("craft transport failed: IllegalStateException", cancel.reason());
    }

    private static CraftClientIntentSink.Transport transport(List<String> calls) {
        return new CraftClientIntentSink.Transport() {
            @Override
            public void start(String recipeId, int quantity) {
                calls.add("start:" + recipeId + ":" + quantity);
            }

            @Override
            public void cancel() {
                calls.add("cancel");
            }
        };
    }
}
