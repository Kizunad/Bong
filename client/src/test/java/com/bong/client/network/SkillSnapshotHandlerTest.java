package com.bong.client.network;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class SkillSnapshotHandlerTest {
    @AfterEach
    void tearDown() {
        SkillSetStore.resetForTests();
    }

    @Test
    void validSnapshotReplacesAllEntries() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"skill_snapshot\"," +
            "\"char_id\":1001," +
            "\"consumed_scrolls\":[\"skill_scroll_herbalism_baicao_can\"]," +
            "\"skills\":{" +
                "\"herbalism\":{\"lv\":3,\"xp\":120,\"xp_to_next\":1600,\"total_xp\":1520,\"cap\":5,\"recent_gain_xp\":5}," +
                "\"alchemy\":{\"lv\":1,\"xp\":80,\"xp_to_next\":400,\"total_xp\":180,\"cap\":5,\"recent_gain_xp\":0}," +
                "\"forging\":{\"lv\":0,\"xp\":0,\"xp_to_next\":100,\"total_xp\":0,\"cap\":5,\"recent_gain_xp\":0}," +
                "\"combat\":{\"lv\":0,\"xp\":0,\"xp_to_next\":100,\"total_xp\":0,\"cap\":5,\"recent_gain_xp\":0}," +
                "\"mineral\":{\"lv\":0,\"xp\":0,\"xp_to_next\":100,\"total_xp\":0,\"cap\":5,\"recent_gain_xp\":0}," +
                "\"cultivation\":{\"lv\":0,\"xp\":0,\"xp_to_next\":100,\"total_xp\":0,\"cap\":5,\"recent_gain_xp\":0}" +
            "}" +
        "}";

        ServerDataDispatch dispatch = new SkillSnapshotHandler().handle(parseEnvelope(json));
        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(3, SkillSetStore.snapshot().get(SkillId.HERBALISM).lv());
        assertEquals(5, SkillSetStore.snapshot().get(SkillId.ALCHEMY).cap());
        assertEquals(100L, SkillSetStore.snapshot().get(SkillId.FORGING).xpToNext());
        assertEquals(5, SkillSetStore.snapshot().get(SkillId.COMBAT).cap());
        assertEquals(100L, SkillSetStore.snapshot().get(SkillId.MINERAL).xpToNext());
        assertEquals(0, SkillSetStore.snapshot().get(SkillId.CULTIVATION).lv());
        assertTrue(SkillSetStore.snapshot().hasConsumedScroll("skill_scroll_herbalism_baicao_can"));
    }

    @Test
    void invalidSnapshotBecomesSafeNoOp() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"skill_snapshot\"," +
            "\"char_id\":1001," +
            "\"consumed_scrolls\":[]," +
            "\"skills\":{" +
                "\"herbalism\":{\"lv\":3,\"xp\":120,\"xp_to_next\":1600,\"total_xp\":1520,\"cap\":5,\"recent_gain_xp\":5}" +
            "}" +
        "}";

        ServerDataDispatch dispatch = new SkillSnapshotHandler().handle(parseEnvelope(json));
        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("missing skill entry"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
