package com.bong.client.botany;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class BotanyPlantRenderBootstrapTest {
    @Test
    void botanyPlantV2EntityUsesExpectedRegistryId() {
        assertEquals("bong", BotanyPlantV2Entities.BOTANY_PLANT_V2_ID.getNamespace());
        assertEquals("botany_plant_v2", BotanyPlantV2Entities.BOTANY_PLANT_V2_ID.getPath());
    }

    @Test
    void fallbackProfileUsesRenderableBaseMesh() {
        BotanyPlantRenderProfile fallback = BotanyPlantRenderProfile.fallback("missing");
        assertEquals("missing", fallback.plantId());
        assertEquals("grass", fallback.baseMeshRef());
        assertEquals(0x88AA55, fallback.tintRgb());
    }
}
