package com.bong.client.network;

import com.bong.client.hud.PillBuffHudPlanner;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PillBuffStatusHandlerTest {
    @AfterEach
    void cleanup() {
        PillBuffHudPlanner.clear();
    }

    @Test
    void routerAppliesPillBuffStatusToHudPlanner() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"pill_buff_status\"," +
            "\"buff_id\":\"huo_xue_dan\"," +
            "\"remaining_ticks\":3000," +
            "\"effect_multiplier\":1.25" +
            "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled(), result.logMessage());
        var buffs = PillBuffHudPlanner.activeBuffs();
        assertEquals(1, buffs.size());
        assertEquals("huo_xue_dan", buffs.get(0).buffId());
        assertEquals(3000, buffs.get(0).remainingTicks());
        assertEquals(1.25, buffs.get(0).effectMultiplier(), 1e-9);
    }

    @Test
    void invalidPillBuffStatusIsSafeNoOp() {
        String json = "{\"v\":1,\"type\":\"pill_buff_status\",\"remaining_ticks\":3000}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isNoOp(), result.logMessage());
        assertTrue(PillBuffHudPlanner.activeBuffs().isEmpty());
    }
}
