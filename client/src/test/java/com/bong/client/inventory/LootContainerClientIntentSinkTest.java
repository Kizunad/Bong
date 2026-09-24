package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class LootContainerClientIntentSinkTest {
    @Test
    void validatesSessionAndGridLocationsBeforeTransport() {
        RecordingTransport transport = new RecordingTransport();
        LootContainerClientIntentSink sink = new LootContainerClientIntentSink(transport);
        assertEquals("LOCAL_REJECTED", sink.dispatch(new LootContainerIntent.Close(0)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new LootContainerIntent.Move(
            1L, 2L, "", 0, 0, "ext_1", 0, 0)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new LootContainerIntent.Move(
            1L, 0L, "main", 0, 0, "ext_1", 0, 0)).kind().name());
        assertEquals("LOCAL_REJECTED", sink.dispatch(new LootContainerIntent.Move(
            1L, 2L, "main", -1, 0, "ext_1", 0, 0)).kind().name());
    }

    @Test
    void mapsMoveAndCloseToOneLocalTransportBoundary() {
        RecordingTransport transport = new RecordingTransport();
        LootContainerClientIntentSink sink = new LootContainerClientIntentSink(transport);
        assertEquals("LOCAL_ACCEPTED", sink.dispatch(new LootContainerIntent.Move(
            12L, 42L, "main", 1, 2, "ext_12", 0, 3)).kind().name());
        assertEquals("move:12:42:main:1:2:ext_12:0:3", transport.call);
        assertEquals("LOCAL_ACCEPTED", sink.dispatch(new LootContainerIntent.Close(12L)).kind().name());
        assertEquals("close:12", transport.call);
    }

    private static final class RecordingTransport implements LootContainerClientIntentSink.Transport {
        private String call;

        @Override
        public void move(long sessionId, long itemInstanceId, String fromContainer, int fromRow, int fromCol,
                         String toContainer, int toRow, int toCol) {
            call = "move:" + sessionId + ":" + itemInstanceId + ":" + fromContainer + ":" + fromRow + ":"
                + fromCol + ":" + toContainer + ":" + toRow + ":" + toCol;
        }

        @Override
        public void close(long sessionId) { call = "close:" + sessionId; }
    }
}
