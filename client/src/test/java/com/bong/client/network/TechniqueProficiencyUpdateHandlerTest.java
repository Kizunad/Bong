package com.bong.client.network;

import com.bong.client.combat.inspect.TechniquesListPanel;
import com.bong.client.hud.BongToast;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TechniqueProficiencyUpdateHandlerTest {
    @BeforeEach
    void setUp() {
        TechniquesListPanel.resetForTests();
        BongToast.resetForTests();
    }

    @AfterEach
    void tearDown() {
        TechniquesListPanel.resetForTests();
        BongToast.resetForTests();
    }

    @Test
    void routerAppliesProficiencyUpdateToExistingTechnique() {
        TechniquesListPanel.replace(List.of(newTechnique("woliu.vortex", "绝灵涡流", 0.10f)));
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"technique_proficiency_update\"," +
            "\"technique_id\":\"woliu.vortex\"," +
            "\"proficiency\":0.42," +
            "\"gain\":0.02" +
            "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(), "valid JSON technique_proficiency_update should parse");
        assertTrue(result.isHandled(), result.logMessage());
        assertEquals(1, TechniquesListPanel.snapshot().size());
        var technique = TechniquesListPanel.snapshot().get(0);
        assertEquals("woliu.vortex", technique.id());
        assertEquals(0.42f, technique.proficiency(), 1e-6f);
        assertEquals("入门", technique.proficiencyLabel());
        assertTrue(BongToast.current(System.currentTimeMillis()).text().getString().contains("绝灵涡流 熟练度 42%"),
            "positive gain should show proficiency toast");
    }

    @Test
    void unknownTechniqueUpdateIsSafeNoOp() {
        TechniquesListPanel.replace(List.of(newTechnique("woliu.vortex", "绝灵涡流", 0.10f)));
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"technique_proficiency_update\"," +
            "\"technique_id\":\"woliu.hold\"," +
            "\"proficiency\":0.42," +
            "\"gain\":0.02" +
            "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(), "unknown technique update should still parse as JSON");
        assertTrue(result.isNoOp(), result.logMessage());
        assertEquals(0.10f, TechniquesListPanel.snapshot().get(0).proficiency(), 1e-6f);
    }

    @Test
    void outOfRangeProficiencyIsSafeNoOp() {
        TechniquesListPanel.replace(List.of(newTechnique("woliu.vortex", "绝灵涡流", 0.10f)));
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"technique_proficiency_update\"," +
            "\"technique_id\":\"woliu.vortex\"," +
            "\"proficiency\":1.5," +
            "\"gain\":0.02" +
            "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(), "out-of-range proficiency payload should parse before validation");
        assertTrue(result.isNoOp(), result.logMessage());
        assertEquals(0.10f, TechniquesListPanel.snapshot().get(0).proficiency(), 1e-6f);
    }

    @Test
    void negativeGainIsSafeNoOp() {
        TechniquesListPanel.replace(List.of(newTechnique("woliu.vortex", "绝灵涡流", 0.10f)));
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"technique_proficiency_update\"," +
            "\"technique_id\":\"woliu.vortex\"," +
            "\"proficiency\":0.42," +
            "\"gain\":-0.01" +
            "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(), "negative gain payload should parse before validation");
        assertTrue(result.isNoOp(), result.logMessage());
        assertEquals(0.10f, TechniquesListPanel.snapshot().get(0).proficiency(), 1e-6f);
    }

    @Test
    void zeroGainUpdatesWithoutToast() {
        TechniquesListPanel.replace(List.of(newTechnique("woliu.vortex", "绝灵涡流", 0.10f)));
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"technique_proficiency_update\"," +
            "\"technique_id\":\"woliu.vortex\"," +
            "\"proficiency\":0.42," +
            "\"gain\":0.0" +
            "}";

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(), "zero gain payload should parse");
        assertTrue(result.isHandled(), result.logMessage());
        assertEquals(0.42f, TechniquesListPanel.snapshot().get(0).proficiency(), 1e-6f);
        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty(),
            "zero gain should not show proficiency toast");
    }

    private static TechniquesListPanel.Technique newTechnique(String id, String displayName, float proficiency) {
        return new TechniquesListPanel.Technique(
            id,
            displayName,
            List.of(),
            TechniquesListPanel.Grade.YELLOW,
            proficiency,
            "",
            true,
            "",
            "",
            "凝脉一层",
            List.of(),
            0.4f,
            8,
            60,
            4.0f
        );
    }
}
