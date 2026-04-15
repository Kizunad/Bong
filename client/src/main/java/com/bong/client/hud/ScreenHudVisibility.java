package com.bong.client.hud;

import net.minecraft.client.gui.screen.DeathScreen;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.ingame.HandledScreen;

/**
 * Per-Screen HUD visibility policy (§8.2). Pure function → trivially testable.
 */
public enum ScreenHudVisibility {
    /** No MC Screen open — render the full HUD. */
    FULL,
    /** Native inventory / E — dim everything but keep quick-bar + event stream. */
    INVENTORY_DIMMED,
    /** InspectScreen / CultivationScreen / Dynamic XML — HUD hidden, cast-bar kept. */
    CAST_BAR_ONLY,
    /** DeathScreen, pause menu — hide everything. */
    HIDDEN;

    public static ScreenHudVisibility forScreen(Screen screen) {
        if (screen == null) return FULL;
        if (screen instanceof DeathScreen) return HIDDEN;
        if (screen instanceof GameMenuScreen) return HIDDEN;
        String clsName = screen.getClass().getName();
        if (clsName.equals("com.bong.client.inventory.InspectScreen")
            || clsName.equals("com.bong.client.ui.CultivationScreen")
            || clsName.equals("com.bong.client.ui.DynamicXmlScreen")
            || clsName.equals("com.bong.client.insight.InsightOfferScreen")) {
            return CAST_BAR_ONLY;
        }
        if (screen instanceof HandledScreen<?>) {
            return INVENTORY_DIMMED;
        }
        return HIDDEN;
    }
}
