package com.bong.client.block;

import net.minecraft.world.GameMode;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BlockPickerGateTest {

    @AfterEach
    void tearDown() {
        BlockPickerGate.resetForTests();
    }

    @Test
    void pureGateAcceptsOnlyCreative() {
        assertTrue(BlockPickerGate.isDevOrCreative(GameMode.CREATIVE),
            "creative is the dev signal — panel must be allowed");
        assertFalse(BlockPickerGate.isDevOrCreative(GameMode.SURVIVAL),
            "survival players must not see the dev block panel");
        assertFalse(BlockPickerGate.isDevOrCreative(GameMode.ADVENTURE),
            "adventure players must not see the dev block panel");
        assertFalse(BlockPickerGate.isDevOrCreative(GameMode.SPECTATOR),
            "spectator must not see the dev block panel");
        assertFalse(BlockPickerGate.isDevOrCreative((GameMode) null),
            "null gamemode (not connected) must hide the panel — no NPE");
    }

    @Test
    void supplierSeamDrivesIsDevOrCreative() {
        BlockPickerGate.setGameModeSupplierForTests(() -> GameMode.CREATIVE);
        assertTrue(BlockPickerGate.isDevOrCreative(),
            "creative gamemode from supplier must enable the panel");

        BlockPickerGate.setGameModeSupplierForTests(() -> GameMode.SURVIVAL);
        assertFalse(BlockPickerGate.isDevOrCreative(),
            "survival gamemode from supplier must hide the panel");

        BlockPickerGate.setGameModeSupplierForTests(() -> null);
        assertFalse(BlockPickerGate.isDevOrCreative(),
            "null gamemode (no interaction manager) must hide the panel");
    }

    @Test
    void resetRestoresDefaultSupplierWithoutCrash() {
        BlockPickerGate.setGameModeSupplierForTests(() -> GameMode.CREATIVE);
        BlockPickerGate.resetForTests();
        // Default supplier reads MinecraftClient.getInstance(); headless it is null → false, no crash.
        assertFalse(BlockPickerGate.isDevOrCreative(),
            "headless default supplier (no client) must safely report false");
    }
}
