package com.bong.client.hud;

import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;

import java.util.List;
import java.util.Locale;

public final class OverweightHudPlanner {
    private static final int X = 10;
    private static final int Y = 34;
    private static final int COLOR = 0xFFFF6060;

    private OverweightHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(HudTextHelper.WidthMeasurer widthMeasurer, int maxWidth) {
        InventoryModel snapshot = InventoryStateStore.snapshot();
        if (snapshot == null || snapshot.maxWeight() <= 0.0 || snapshot.currentWeight() <= snapshot.maxWeight()) {
            return List.of();
        }

        String text = String.format(
            Locale.ROOT,
            "超载 %.1f/%.1f",
            snapshot.currentWeight(),
            snapshot.maxWeight()
        );
        String clipped = HudTextHelper.clipToWidth(text, maxWidth, widthMeasurer);
        if (clipped.isEmpty()) {
            return List.of();
        }

        return List.of(HudRenderCommand.text(HudRenderLayer.BASELINE, clipped, X, Y, COLOR));
    }
}
