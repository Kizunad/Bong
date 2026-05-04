package com.bong.client.social;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NicheGuardianStoreTest {
    @AfterEach
    void tearDown() {
        NicheGuardianStore.resetForTests();
    }

    @Test
    void panelListsGuardianStatusAndRecentIntrusion() {
        NicheGuardianStore.recordFatigue("puppet", 4);
        NicheGuardianStore.recordIntrusion(new NicheGuardianStore.NicheIntrusionAlert(
            List.of(42L),
            "char:raider",
            0.2,
            1000L
        ));

        List<String> lines = NicheGuardianPanel.buildLines();

        assertEquals("puppet x4", lines.get(0));
        assertTrue(lines.get(1).contains("char:raider"));
    }
}
