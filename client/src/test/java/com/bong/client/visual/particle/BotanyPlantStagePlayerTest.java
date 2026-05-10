package com.bong.client.visual.particle;

import com.bong.client.botany.PlantGrowthStage;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyPlantStagePlayerTest {
    @Test
    void parseStageEventIdExtractsPlantAndStage() {
        var parsed = BotanyPlantStagePlayer.parseEventId(
            new Identifier("bong", "botany_plant_stage__ning_mai_cao__seedling")
        );

        assertTrue(parsed.isPresent());
        assertEquals("ning_mai_cao", parsed.get().plantId());
        assertEquals(PlantGrowthStage.SEEDLING, parsed.get().stage());
    }

    @Test
    void routeRejectsUnrelatedEventId() {
        assertTrue(BotanyPlantStagePlayer.parseEventId(
            new Identifier("bong", "botany_aura")
        ).isEmpty());
    }
}
