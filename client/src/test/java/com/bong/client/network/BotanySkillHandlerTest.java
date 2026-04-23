package com.bong.client.network;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetStore;
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
        SkillSetStore.resetForTests();
    }

    @Test
    void validPayloadUpdatesHerbalismSkillEntry() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-botany-skill.json");
        ServerDataDispatch dispatch = new BotanySkillHandler().handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        var snapshot = SkillSetStore.snapshot().get(SkillId.HERBALISM);
        assertEquals(2, snapshot.lv());
        assertEquals(90L, snapshot.xp());
    }

    @Test
    void invalidPayloadBecomesSafeNoOp() {
        ServerDataDispatch dispatch = new BotanySkillHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"botany_skill\",\"level\":2,\"xp\":90}"
        ));

        assertFalse(dispatch.handled());
        assertEquals(0, SkillSetStore.snapshot().get(SkillId.HERBALISM).lv());
        assertTrue(dispatch.logMessage().contains("xp_to_next_level"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
