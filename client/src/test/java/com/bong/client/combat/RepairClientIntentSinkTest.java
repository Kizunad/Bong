package com.bong.client.combat;

import com.bong.client.ui.intent.UiIntentResult;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class RepairClientIntentSinkTest {
    @Test
    void structuredWeaponCommitUsesInstanceAndStationCoordinates() {
        long[] instance = {0L};
        int[] coordinates = {0, 0, 0};
        int[] legacyCalls = {0};
        RepairClientIntentSink sink = new RepairClientIntentSink(new RepairClientIntentSink.Transport() {
            @Override
            public void sendWeapon(long value, int x, int y, int z) {
                instance[0] = value;
                coordinates[0] = x;
                coordinates[1] = y;
                coordinates[2] = z;
            }

            @Override
            public void sendLegacy(String material) {
                legacyCalls[0]++;
            }
        });

        UiIntentResult result = sink.dispatch(new RepairIntent.Commit("refined_steel", 4242L, -3, 64, 8));

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, result.kind());
        assertEquals(4242L, instance[0]);
        assertArrayEquals(new int[] {-3, 64, 8}, coordinates);
        assertEquals(0, legacyCalls[0], "结构化 repair 不应落回旧 material payload");
    }

    @Test
    void nonPositiveWeaponIdPreservesLegacyMaterialTransport() {
        String[] material = {null};
        int[] structuredCalls = {0};
        RepairClientIntentSink sink = new RepairClientIntentSink(new RepairClientIntentSink.Transport() {
            @Override
            public void sendWeapon(long instanceId, int stationX, int stationY, int stationZ) {
                structuredCalls[0]++;
            }

            @Override
            public void sendLegacy(String value) {
                material[0] = value;
            }
        });

        UiIntentResult result = sink.dispatch(new RepairIntent.Commit("pill", 0L, 1, 2, 3));

        assertEquals(UiIntentResult.Kind.LOCAL_ACCEPTED, result.kind());
        assertEquals("pill", material[0]);
        assertEquals(0, structuredCalls[0], "旧 repair 入口不应伪造 instance_id");
    }

    @Test
    void nullIntentIsRejectedWithoutTransport() {
        int[] calls = {0};
        RepairClientIntentSink sink = new RepairClientIntentSink(new RepairClientIntentSink.Transport() {
            @Override
            public void sendWeapon(long instanceId, int stationX, int stationY, int stationZ) {
                calls[0]++;
            }

            @Override
            public void sendLegacy(String material) {
                calls[0]++;
            }
        });

        UiIntentResult result = sink.dispatch(null);

        assertEquals(UiIntentResult.Kind.LOCAL_REJECTED, result.kind());
        assertEquals(0, calls[0], "空 repair action 不应触碰 transport");
    }

    @Test
    void transportFailureIsReportedAsLocalError() {
        RepairClientIntentSink sink = new RepairClientIntentSink(new RepairClientIntentSink.Transport() {
            @Override
            public void sendWeapon(long instanceId, int stationX, int stationY, int stationZ) {
                throw new IllegalStateException("not connected");
            }

            @Override
            public void sendLegacy(String material) {
                throw new IllegalStateException("not connected");
            }
        });

        UiIntentResult result = sink.dispatch(new RepairIntent.Commit("refined_steel", 42L, 1, 64, 2));

        assertEquals(UiIntentResult.Kind.LOCAL_ERROR, result.kind());
        assertTrue(result.reason().contains("not connected"), "传输异常应保留可修复原因");
    }
}
