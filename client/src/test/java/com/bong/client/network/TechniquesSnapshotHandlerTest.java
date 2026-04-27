package com.bong.client.network;

import com.bong.client.combat.inspect.TechniquesListPanel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TechniquesSnapshotHandlerTest {
    @BeforeEach void setUp() { TechniquesListPanel.resetForTests(); }
    @AfterEach void tearDown() { TechniquesListPanel.resetForTests(); }

    @Test
    void appliesTechniqueMetadata() {
        ServerDataDispatch dispatch = new TechniquesSnapshotHandler().handle(parseEnvelope("""
            {"v":1,"type":"techniques_snapshot","entries":[{
              "id":"burst_meridian.beng_quan",
              "display_name":"崩拳",
              "grade":"yellow",
              "proficiency":0.62,
              "active":true,
              "description":"凝劲贯臂，短距爆发。",
              "required_realm":"凝脉一层",
              "required_meridians":[{"channel":"LargeIntestine","min_health":0.5}],
              "qi_cost":30,
              "cast_ticks":8,
              "cooldown_ticks":60,
              "range":1.8
            }]}"""));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(1, TechniquesListPanel.snapshot().size());
        var technique = TechniquesListPanel.snapshot().get(0);
        assertEquals(TechniquesListPanel.Grade.YELLOW, technique.grade());
        assertEquals(30, technique.qiCost());
        assertEquals("LargeIntestine", technique.requiredMeridians().get(0).channel());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
