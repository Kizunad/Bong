package com.bong.client.network;

import com.bong.client.botany.BotanySkillStore;
import com.bong.client.botany.BotanySkillViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BotanySkillHandlerTest {
    @AfterEach
    void tearDown() {
        BotanySkillStore.resetForTests();
    }

    @Test
    void validPayloadUpdatesCompatibilitySkillStore() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-botany-skill.json");
        ServerDataDispatch dispatch = new BotanySkillHandler().handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        BotanySkillViewModel snapshot = BotanySkillStore.snapshot();
        assertEquals(2, snapshot.level());
        assertEquals(90L, snapshot.xp());
        assertEquals(3, snapshot.autoUnlockLevel());
    }

    @Test
    void invalidPayloadBecomesSafeNoOp() {
        ServerDataDispatch dispatch = new BotanySkillHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"botany_skill\",\"level\":2,\"xp\":90}"
        ));

        assertFalse(dispatch.handled());
        assertEquals(BotanySkillViewModel.defaultView(), BotanySkillStore.snapshot());
        assertTrue(dispatch.logMessage().contains("xp_to_next_level"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
