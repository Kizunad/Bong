package com.bong.client.network;

import com.bong.client.botany.BotanyPlantRenderProfile;
import com.bong.client.botany.BotanyPlantRenderProfileStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyPlantRenderProfileHandlerTest {
    @AfterEach
    void tearDown() {
        BotanyPlantRenderProfileStore.clearOnDisconnect();
    }

    @Test
    void validPayloadUpdatesProfileStore() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-botany-plant-render-profiles.json");
        ServerDataDispatch dispatch = new BotanyPlantRenderProfileHandler().handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        assertEquals(2, BotanyPlantRenderProfileStore.snapshot().size());

        BotanyPlantRenderProfile yingYuanGu = BotanyPlantRenderProfileStore.get("ying_yuan_gu").orElseThrow();
        assertEquals("red_mushroom", yingYuanGu.baseMeshRef());
        assertEquals(0xFFA040, yingYuanGu.tintRgb());
        assertEquals(BotanyPlantRenderProfile.ModelOverlay.EMISSIVE, yingYuanGu.overlay());

        BotanyPlantRenderProfile dualPhase = BotanyPlantRenderProfileStore.get("yuan_ni_hong_yu").orElseThrow();
        assertEquals(0xE6321E, dualPhase.tintAt(0));
        assertEquals(0x5B735D, dualPhase.tintAt(18000));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
