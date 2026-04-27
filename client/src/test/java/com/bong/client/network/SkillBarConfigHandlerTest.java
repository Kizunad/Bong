package com.bong.client.network;

import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SkillBarConfigHandlerTest {
    @BeforeEach void setUp() { SkillBarStore.resetForTests(); }
    @AfterEach void tearDown() { SkillBarStore.resetForTests(); }

    @Test
    void appliesSkillAndItemEntries() {
        ServerDataDispatch dispatch = new SkillBarConfigHandler().handle(parseEnvelope("""
            {"v":1,"type":"skillbar_config","slots":[
              {"kind":"skill","skill_id":"burst_meridian.beng_quan","display_name":"崩拳","cast_duration_ms":400,"cooldown_ms":3000,"icon_texture":""},
              {"kind":"item","template_id":"kai_mai_pill_v0","display_name":"开脉丹","cast_duration_ms":1500,"cooldown_ms":500,"icon_texture":""},
              null,null,null,null,null,null,null
            ],"cooldown_until_ms":[1700000000000,0,0,0,0,0,0,0,0]}"""));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(SkillBarEntry.Kind.SKILL, SkillBarStore.snapshot().slot(0).kind());
        assertEquals("burst_meridian.beng_quan", SkillBarStore.snapshot().slot(0).id());
        assertEquals(SkillBarEntry.Kind.ITEM, SkillBarStore.snapshot().slot(1).kind());
        assertEquals(1700000000000L, SkillBarStore.snapshot().cooldownUntilMs(0));
    }

    @Test
    void rejectsLengthMismatch() {
        ServerDataDispatch dispatch = new SkillBarConfigHandler().handle(parseEnvelope("""
            {"v":1,"type":"skillbar_config","slots":[],"cooldown_until_ms":[]}"""));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("array length mismatch"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
