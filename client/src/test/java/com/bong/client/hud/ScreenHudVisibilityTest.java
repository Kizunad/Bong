package com.bong.client.hud;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.insight.InsightCategory;
import com.bong.client.insight.InsightChoice;
import com.bong.client.insight.InsightOfferScreen;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.insight.InsightOfferViewModel;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.UiOpenState;
import com.bong.client.ui.CultivationScreen;
import com.bong.client.ui.UiOpenScreens;
import net.minecraft.client.gui.screen.DeathScreen;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.ingame.HandledScreen;
import net.minecraft.text.Text;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

class ScreenHudVisibilityTest {
    @Test
    void pinsEveryConcreteScreenPolicy() {
        Screen dynamicXmlScreen = UiOpenScreens.createScreen(UiOpenState.dynamicXml(
            "dynamic-test",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            true
        ));
        assertNotNull(dynamicXmlScreen);

        assertEquals(ScreenHudVisibility.FULL, ScreenHudVisibility.forScreen(null));
        assertEquals(
            ScreenHudVisibility.AGENT_UI_ONLY,
            ScreenHudVisibility.forScreen(AgentUiScreen.create(
                "req-classification",
                "<owo-ui><components><flow-layout/></components></owo-ui>",
                200,
                1_000L
            ))
        );
        assertEquals(
            ScreenHudVisibility.HIDDEN,
            ScreenHudVisibility.forScreen(new DeathScreen(Text.literal("death"), false))
        );
        assertEquals(
            ScreenHudVisibility.HIDDEN,
            ScreenHudVisibility.forScreen(new com.bong.client.combat.screen.DeathScreen(
                new DeathStateStore.State(true, "pk", 0.5f, List.of(), 0L, true, true),
                intent -> com.bong.client.ui.intent.UiIntentResult.accepted()
            ))
        );
        assertEquals(
            ScreenHudVisibility.HIDDEN,
            ScreenHudVisibility.forScreen(new GameMenuScreen(false))
        );
        assertEquals(
            ScreenHudVisibility.CAST_BAR_ONLY,
            ScreenHudVisibility.forScreen(new InspectScreen(InventoryModel.empty()))
        );
        assertEquals(
            ScreenHudVisibility.CAST_BAR_ONLY,
            ScreenHudVisibility.forScreen(new CultivationScreen(PlayerStateViewModel.empty()))
        );
        assertEquals(ScreenHudVisibility.CAST_BAR_ONLY, ScreenHudVisibility.forScreen(dynamicXmlScreen));
        InsightOfferViewModel offer = insightOffer();
        InsightOfferStore.replace(offer);
        assertEquals(
            ScreenHudVisibility.CAST_BAR_ONLY,
            ScreenHudVisibility.forScreen(new InsightOfferScreen(offer))
        );
        assertEquals(
            ScreenHudVisibility.HIDDEN,
            ScreenHudVisibility.forScreen(new UnknownScreen())
        );
    }

    @Test
    void handledScreenHierarchyKeepsInventoryDimmedPolicy() {
        assertEquals(
            ScreenHudVisibility.INVENTORY_DIMMED,
            ScreenHudVisibility.forScreenClass(HandledScreen.class)
        );
    }

    @Test
    void classClassifierRejectsNull() {
        assertThrows(NullPointerException.class, () -> ScreenHudVisibility.forScreenClass(null));
    }

    private static InsightOfferViewModel insightOffer() {
        return new InsightOfferViewModel(
            "insight:1:trigger",
            "trigger",
            "触发",
            "醒灵",
            0.5,
            1,
            1,
            2_000L,
            List.of(new InsightChoice(
                "choice",
                InsightCategory.QI,
                "引气",
                "真元运转更稳",
                "气机回环。",
                "稳固"
            ))
        );
    }

    private static final class UnknownScreen extends Screen {
        private UnknownScreen() {
            super(Text.literal("unknown"));
        }
    }
}
