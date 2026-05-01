package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class InventoryItemTest {

    @Test
    void legacyFactoryDefaultsAuthoritativeFields() {
        InventoryItem item = InventoryItem.create(
            "starter_talisman",
            "Starter Talisman",
            1,
            1,
            0.5,
            "common",
            ""
        );

        assertEquals(0L, item.instanceId());
        assertEquals(1, item.stackCount());
        assertEquals(1.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }

    @Test
    void fullFactoryClampsNewInventoryFields() {
        InventoryItem item = InventoryItem.createFull(
            42L,
            "weathered_pill",
            "Weathered Pill",
            1,
            1,
            0.2,
            "rare",
            "",
            0,
            -0.25,
            1.75
        );

        assertEquals(42L, item.instanceId());
        assertEquals(1, item.stackCount());
        assertEquals(0.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }

    @Test
    void nanQualityAndDurabilityDefaultToFullValue() {
        InventoryItem item = InventoryItem.createFull(
            43L,
            "unstable_pill",
            "Unstable Pill",
            1,
            1,
            0.2,
            "rare",
            "",
            2,
            Double.NaN,
            Double.NaN
        );

        assertEquals(1.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }
}
