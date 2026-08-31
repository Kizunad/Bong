package com.bong.client.hud;

import com.bong.client.agentui.AgentUiScreen;
import net.minecraft.client.gui.screen.DeathScreen;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.ingame.HandledScreen;

import java.util.Objects;

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
    /** AgentUiScreen — only render the panel's dedicated VFX overlay. */
    AGENT_UI_ONLY,
    /** DeathScreen, pause menu — hide everything. */
    HIDDEN;

    public static ScreenHudVisibility forScreen(Screen screen) {
        if (screen == null) return FULL;
        return forScreenClass(screen.getClass());
    }

    static ScreenHudVisibility forScreenClass(Class<? extends Screen> screenClass) {
        Objects.requireNonNull(screenClass, "screenClass");
        if (DeathScreen.class.isAssignableFrom(screenClass)
            || com.bong.client.combat.screen.DeathScreen.class.isAssignableFrom(screenClass)) return HIDDEN;
        if (GameMenuScreen.class.isAssignableFrom(screenClass)) return HIDDEN;
        if (AgentUiScreen.class.isAssignableFrom(screenClass)) return AGENT_UI_ONLY;
        String clsName = screenClass.getName();
        if (clsName.equals("com.bong.client.inventory.InspectScreen")
            || clsName.equals("com.bong.client.ui.CultivationScreen")
            || clsName.equals("com.bong.client.ui.DynamicXmlScreen")
            || clsName.equals("com.bong.client.insight.InsightOfferScreen")) {
            return CAST_BAR_ONLY;
        }
        if (HandledScreen.class.isAssignableFrom(screenClass)) {
            return INVENTORY_DIMMED;
        }
        return HIDDEN;
    }
}
