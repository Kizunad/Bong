package com.bong.client.network;

import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanyHarvestProgressHandlerTest {
    @AfterEach
    void tearDown() {
        HarvestSessionStore.resetForTests();
    }

    @Test
    void validPayloadUpdatesHarvestSessionStore() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-botany-harvest-progress.json");
        ServerDataDispatch dispatch = new BotanyHarvestProgressHandler().handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        HarvestSessionViewModel snapshot = HarvestSessionStore.snapshot();
        assertEquals("session-botany-01", snapshot.sessionId());
        assertEquals(BotanyHarvestMode.MANUAL, snapshot.mode());
        assertEquals(0.6, snapshot.progress(), 0.0001);
    }

    @Test
    void missingSessionIdBecomesSafeNoOp() {
        ServerDataDispatch dispatch = new BotanyHarvestProgressHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"botany_harvest_progress\",\"target_name\":\"开脉草\"}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(HarvestSessionStore.snapshot().isEmpty());
        assertTrue(dispatch.logMessage().contains("session_id"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
