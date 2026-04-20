package com.bong.client.hud;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class OverweightHudPlannerTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    @Test
    void noCommandWhenWeightIsWithinLimit() {
        InventoryStateStore.applyAuthoritativeSnapshot(
            InventoryModel.builder()
                .containers(InventoryModel.DEFAULT_CONTAINERS)
                .weight(12.0, 50.0)
                .build(),
            1L
        );

        assertTrue(OverweightHudPlanner.buildCommands(FIXED_WIDTH, 220).isEmpty());
    }

    @Test
    void emitsRedBaselineTextWhenOverweight() {
        InventoryStateStore.applyAuthoritativeSnapshot(
            InventoryModel.builder()
                .containers(InventoryModel.DEFAULT_CONTAINERS)
                .weight(60.0, 50.0)
                .gridItem(
                    InventoryItem.simple("starter_talisman", "启程护符"),
                    InventoryModel.PRIMARY_CONTAINER_ID,
                    0,
                    0
                )
                .build(),
            2L
        );

        List<HudRenderCommand> commands = OverweightHudPlanner.buildCommands(FIXED_WIDTH, 220);
        assertEquals(1, commands.size());
        assertEquals(HudRenderLayer.BASELINE, commands.get(0).layer());
        assertTrue(commands.get(0).text().contains("超载"));
    }
}
