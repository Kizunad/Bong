package com.bong.client.inventory.component;

import com.bong.client.inventory.model.MeridianChannel;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;

class BodyInspectTechniqueHighlightTest {
    @Test
    void storesAndClearsMultiMeridianTechniqueHighlights() {
        BodyInspectComponent component = new BodyInspectComponent();

        component.setTechniqueMeridianHighlights(List.of(
            MeridianChannel.PC,
            MeridianChannel.YIN_WEI,
            MeridianChannel.PC
        ));

        assertEquals(
            Set.of(MeridianChannel.PC, MeridianChannel.YIN_WEI),
            component.techniqueMeridianHighlightsForTests()
        );

        component.clearTechniqueMeridianHighlights();

        assertEquals(Set.of(), component.techniqueMeridianHighlightsForTests());
    }
}
