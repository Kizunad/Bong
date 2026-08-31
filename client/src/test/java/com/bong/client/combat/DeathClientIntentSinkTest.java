package com.bong.client.combat;

import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class DeathClientIntentSinkTest {
    @Test
    void typedActionsReachTheirMatchingTransportExactlyOnce() {
        int[] reincarnate = {0};
        int[] terminate = {0};
        DeathClientIntentSink sink = new DeathClientIntentSink(new DeathClientIntentSink.Transport() {
            @Override
            public void reincarnate() {
                reincarnate[0]++;
            }

            @Override
            public void terminate() {
                terminate[0]++;
            }
        });

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, sink.dispatch(new DeathIntent.Reincarnate()).kind());
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, sink.dispatch(new DeathIntent.Terminate()).kind());
        assertEquals(1, reincarnate[0], "重生 action 必须只发送一次");
        assertEquals(1, terminate[0], "终结 action 必须只发送一次");
    }

    @Test
    void nullIntentIsRejectedWithoutTransport() {
        int[] calls = {0};
        DeathClientIntentSink sink = new DeathClientIntentSink(new DeathClientIntentSink.Transport() {
            @Override
            public void reincarnate() {
                calls[0]++;
            }

            @Override
            public void terminate() {
                calls[0]++;
            }
        });

        UiIntentResult result = sink.dispatch(null);

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, result.kind());
        assertEquals(0, calls[0], "空 action 不应触碰 transport");
    }

    @Test
    void transportFailureIsReportedAsLocalError() {
        DeathClientIntentSink sink = new DeathClientIntentSink(new DeathClientIntentSink.Transport() {
            @Override
            public void reincarnate() {
                throw new IllegalStateException("not connected");
            }

            @Override
            public void terminate() {
            }
        });

        UiIntentResult result = sink.dispatch(new DeathIntent.Reincarnate());

        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, result.kind());
        assertTrue(result.reason().contains("not connected"), "传输异常应保留可修复的原因");
    }
}
