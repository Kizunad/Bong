package com.bong.client.network.handlers;

import com.bong.client.PlayerStateCache;
import com.bong.client.ui.CultivationScreenModel;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class UiOpenHandlerTest {
    private final UiOpenHandler handler = new UiOpenHandler();
    private final PlayerStateHandler playerStateHandler = new PlayerStateHandler();

    @Test
    void validUiOpenPayloadIsIgnoredSafely() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"ui_open\"," +
            "\"ui\":\"cultivation\"," +
            "\"xml\":\"<flow-layout><label>ignored</label></flow-layout>\"" +
            "}";

        assertDoesNotThrow(() -> handler.handle(null, "ui_open", json));
    }

    @Test
    void ignoredUiOpenDoesNotBlockStaticCultivationModelFromCachedPlayerState() {
        String playerStateJson = "{" +
            "\"v\":1," +
            "\"type\":\"player_state\"," +
            "\"realm\":\"qi_refining_3\"," +
            "\"spirit_qi\":78," +
            "\"karma\":0.2," +
            "\"composite_power\":0.35," +
            "\"breakdown\":{" +
            "\"combat\":0.2," +
            "\"wealth\":0.4," +
            "\"social\":0.65," +
            "\"karma\":0.2," +
            "\"territory\":0.1}," +
            "\"zone\":\"qingyun_peak\"" +
            "}";
        String uiOpenJson = "{" +
            "\"v\":1," +
            "\"type\":\"ui_open\"," +
            "\"ui\":\"cultivation\"," +
            "\"xml\":\"<flow-layout><label>ignored</label></flow-layout>\"" +
            "}";

        playerStateHandler.handle(null, "player_state", playerStateJson);
        handler.handle(null, "ui_open", uiOpenJson);

        CultivationScreenModel model = CultivationScreenModel.from(PlayerStateCache.peek());
        assertTrue(model.synced());
        assertEquals("练气三层", model.realmLabel());
        assertEquals("78 / 100", model.spiritQiText());
        assertEquals("+0.20", model.karmaText());
        assertEquals("0.35", model.compositePowerText());
        assertEquals("Qingyun Peak", model.zoneText());
        assertEquals("战斗", model.breakdownEntries().get(0).label());
        assertEquals("0.20", model.breakdownEntries().get(0).valueText());
    }

    @Test
    void malformedUiOpenPayloadIsIgnoredSafely() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"ui_open\"," +
            "\"xml\":\"\"" +
            "}";

        assertDoesNotThrow(() -> handler.handle(null, "ui_open", json));
    }
}
