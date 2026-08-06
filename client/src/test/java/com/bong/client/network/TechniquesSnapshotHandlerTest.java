package com.bong.client.network;

import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.hud.BongToast;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TechniquesSnapshotHandlerTest {
    @BeforeEach void setUp() {
        TechniquesListPanel.resetForTests();
        BongToast.resetForTests();
    }

    @AfterEach void tearDown() {
        TechniquesListPanel.resetForTests();
        BongToast.resetForTests();
    }

    @Test
    void appliesTechniqueMetadata() {
        ServerDataDispatch dispatch = new TechniquesSnapshotHandler().handle(parseEnvelope("""
            {"v":1,"type":"techniques_snapshot","entries":[{
              "id":"burst_meridian.beng_quan",
              "display_name":"崩拳",
              "grade":"yellow",
              "proficiency":0.62,
              "proficiency_label":"熟练",
              "active":true,
              "description":"凝劲贯臂，短距爆发。",
              "required_realm":"凝脉一层",
              "required_meridians":[{"channel":"LargeIntestine","min_health":0.5}],
              "qi_cost":0.4,
              "cast_ticks":8,
              "cooldown_ticks":60,
              "range":1.8
            }]}"""));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(1, TechniquesListPanel.snapshot().size());
        var technique = TechniquesListPanel.snapshot().get(0);
        assertEquals(TechniquesListPanel.Grade.YELLOW, technique.grade());
        assertEquals("熟练", technique.proficiencyLabel());
        assertEquals(0.4, technique.qiCost(), 0.0001);
        assertEquals("LargeIntestine", technique.requiredMeridians().get(0).channel());
    }

    @Test
    void preservesLargeQiCostWithoutFloatRounding() {
        ServerDataDispatch dispatch = new TechniquesSnapshotHandler().handle(parseEnvelope("""
            {"v":1,"type":"techniques_snapshot","entries":[{
              "id":"sword.cleave","display_name":"劈","grade":"common",
              "proficiency":0.5,"active":true,"description":"","required_realm":"Awaken",
              "required_meridians":[],"qi_cost":16777217.0,"cast_ticks":1,
              "cooldown_ticks":1,"range":1.0
            }]}"""));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(16_777_217.0, TechniquesListPanel.snapshot().get(0).qiCost());
    }

    @Test
    void newTechniqueSnapshotShowsLearnedToastAfterInitialSync() {
        TechniquesSnapshotHandler handler = new TechniquesSnapshotHandler();
        handler.handle(parseEnvelope("""
            {"v":1,"type":"techniques_snapshot","entries":[{
              "id":"woliu.vortex","display_name":"绝灵涡流","grade":"yellow","proficiency":0.0,
              "active":true,"description":"","required_realm":"Condense","required_meridians":[],
              "qi_cost":0.4,"cast_ticks":8,"cooldown_ticks":60,"range":4.0
            }]}"""));
        BongToast.resetForTests();

        handler.handle(parseEnvelope("""
            {"v":1,"type":"techniques_snapshot","entries":[{
              "id":"woliu.vortex","display_name":"绝灵涡流","grade":"yellow","proficiency":0.0,
              "active":true,"description":"","required_realm":"Condense","required_meridians":[],
              "qi_cost":0.4,"cast_ticks":8,"cooldown_ticks":60,"range":4.0
            },{
              "id":"woliu.hold","display_name":"涡流牵制","grade":"yellow","proficiency":0.0,
              "active":true,"description":"","required_realm":"Condense","required_meridians":[],
              "qi_cost":0.4,"cast_ticks":8,"cooldown_ticks":60,"range":4.0
            }]}"""));

        assertTrue(BongToast.current(System.currentTimeMillis()).text().getString().contains("习得·涡流牵制"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
