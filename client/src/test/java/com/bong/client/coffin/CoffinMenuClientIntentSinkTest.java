package com.bong.client.coffin;

import com.bong.client.ui.intent.UiIntentResult;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CoffinMenuClientIntentSinkTest {
    @Test
    void enterAndReclaimUseTheirTypedTransportBranches() {
        int[] enterCalls = {0};
        int[] reclaimCalls = {0};
        BlockPos position = new BlockPos(4, 65, 7);
        CoffinMenuClientIntentSink sink = new CoffinMenuClientIntentSink(new CoffinMenuClientIntentSink.Transport() {
            @Override
            public void enter(BlockPos coffinPos) {
                assertEquals(position, coffinPos);
                enterCalls[0]++;
            }

            @Override
            public void reclaim(BlockPos coffinPos) {
                assertEquals(position, coffinPos);
                reclaimCalls[0]++;
            }
        });

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED,
            sink.dispatch(new CoffinMenuIntent.Enter(position)).kind());
        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED,
            sink.dispatch(new CoffinMenuIntent.Reclaim(position)).kind());
        assertEquals(1, enterCalls[0], "入眠 typed action 应只发送一次");
        assertEquals(1, reclaimCalls[0], "回收 typed action 应只发送一次");
    }

    @Test
    void nullIntentIsRejectedWithoutTransport() {
        int[] calls = {0};
        CoffinMenuClientIntentSink sink = new CoffinMenuClientIntentSink(new CoffinMenuClientIntentSink.Transport() {
            @Override
            public void enter(BlockPos coffinPos) {
                calls[0]++;
            }

            @Override
            public void reclaim(BlockPos coffinPos) {
                calls[0]++;
            }
        });

        UiIntentResult result = sink.dispatch(null);

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, result.kind());
        assertEquals(0, calls[0], "空 action 不应触碰 transport");
    }

    @Test
    void transportFailureIsReportedAsLocalError() {
        CoffinMenuClientIntentSink sink = new CoffinMenuClientIntentSink(new CoffinMenuClientIntentSink.Transport() {
            @Override
            public void enter(BlockPos coffinPos) {
                throw new IllegalStateException("not connected");
            }

            @Override
            public void reclaim(BlockPos coffinPos) {
                throw new IllegalStateException("not connected");
            }
        });

        UiIntentResult result = sink.dispatch(new CoffinMenuIntent.Enter(new BlockPos(0, 64, 0)));

        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, result.kind());
        assertTrue(result.reason().contains("not connected"), "传输异常应保留可修复的原因");
    }
}
