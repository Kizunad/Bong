package com.bong.client.coffin;

import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F9 跨层修复 — {@link TutorialCoffinPosStore} 单值 store 存取饱和测试。
 */
class TutorialCoffinPosStoreTest {
    @AfterEach
    void tearDown() {
        TutorialCoffinPosStore.resetForTests();
    }

    @Test
    void snapshotIsEmptyBeforeAnyBroadcastIsReceived() {
        assertTrue(TutorialCoffinPosStore.snapshot().isEmpty(),
            "fresh store (no server_data received yet) must report empty, not a stale default pos");
    }

    @Test
    void setStoresTheBroadcastPosAndSnapshotReturnsIt() {
        BlockPos pos = new BlockPos(12, 71, -33);
        TutorialCoffinPosStore.set(pos);

        Optional<BlockPos> snapshot = TutorialCoffinPosStore.snapshot();
        assertTrue(snapshot.isPresent());
        assertEquals(pos, snapshot.get());
    }

    @Test
    void setOverwritesThePreviousValue() {
        TutorialCoffinPosStore.set(new BlockPos(0, 69, 0));
        TutorialCoffinPosStore.set(new BlockPos(400, 70, -400));

        assertEquals(new BlockPos(400, 70, -400), TutorialCoffinPosStore.snapshot().orElseThrow(),
            "latest set() call should win; store holds a single authoritative value, not history");
    }

    @Test
    void clearOnDisconnectResetsToEmpty() {
        TutorialCoffinPosStore.set(new BlockPos(8, 68, 8));
        assertTrue(TutorialCoffinPosStore.snapshot().isPresent());

        TutorialCoffinPosStore.clearOnDisconnect();

        assertFalse(TutorialCoffinPosStore.snapshot().isPresent(),
            "disconnect must drop the cached pos so a reconnect to a different server "
                + "(different spawn coffin location) can't leak the old coordinate");
    }

    @Test
    void resetForTestsResetsToEmpty() {
        TutorialCoffinPosStore.set(new BlockPos(1, 2, 3));
        TutorialCoffinPosStore.resetForTests();
        assertTrue(TutorialCoffinPosStore.snapshot().isEmpty());
    }
}
