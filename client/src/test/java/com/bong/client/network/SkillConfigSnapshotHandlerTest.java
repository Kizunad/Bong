package com.bong.client.network;

import com.bong.client.combat.SkillConfigStore;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SkillConfigSnapshotHandlerTest {
    @BeforeEach void setUp() { SkillConfigStore.resetForTests(); }
    @AfterEach void tearDown() { SkillConfigStore.resetForTests(); }

    @Test
    void appliesSkillConfigSnapshot() {
        ServerDataDispatch dispatch = new SkillConfigSnapshotHandler().handle(parseEnvelope("""
            {"v":1,"type":"skill_config_snapshot","configs":{
              "zhenmai.sever_chain":{"meridian_id":"Pericardium","backfire_kind":"tainted_yuan"}
            }}"""));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(1, SkillConfigStore.snapshot().size());
        assertEquals(
            "Pericardium",
            SkillConfigStore.configFor("zhenmai.sever_chain").get("meridian_id").getAsString()
        );
    }

    @Test
    void rejectsMalformedConfigObject() {
        ServerDataDispatch dispatch = new SkillConfigSnapshotHandler().handle(parseEnvelope("""
            {"v":1,"type":"skill_config_snapshot","configs":{"zhenmai.sever_chain":true}}"""));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("not an object"));
        assertTrue(SkillConfigStore.snapshot().isEmpty());
    }

    @Test
    void snapshotReturnsImmutableDeepCopy() {
        new SkillConfigSnapshotHandler().handle(parseEnvelope("""
            {"v":1,"type":"skill_config_snapshot","configs":{
              "zhenmai.sever_chain":{"meridian_id":"Pericardium","backfire_kind":"tainted_yuan"}
            }}"""));

        Map<String, JsonObject> snapshot = SkillConfigStore.snapshot();
        assertThrows(UnsupportedOperationException.class, snapshot::clear);

        snapshot.get("zhenmai.sever_chain").addProperty("backfire_kind", "array");

        assertEquals(
            "tainted_yuan",
            SkillConfigStore.configFor("zhenmai.sever_chain").get("backfire_kind").getAsString()
        );
    }

    @Test
    void defaultRouterRegistersSkillConfigSnapshot() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        assertTrue(router.registeredTypes().contains("skill_config_snapshot"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
