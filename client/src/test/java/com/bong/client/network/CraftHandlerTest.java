package com.bong.client.network;

import com.bong.client.craft.CraftCategory;
import com.bong.client.craft.CraftRecipe;
import com.bong.client.craft.CraftSessionStateView;
import com.bong.client.craft.CraftStore;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CraftHandlerTest {

    @BeforeEach
    void reset() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
    }

    @AfterEach
    void teardown() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
    }

    private static ServerDataEnvelope envelope(String json) {
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        if (!parsed.isSuccess()) {
            throw new AssertionError("test envelope parse failed: " + parsed.errorMessage());
        }
        return parsed.envelope();
    }

    @Test
    void recipeListHandlerLoadsAllRecipesIntoStore() {
        String json = """
            {
              "v":1,"type":"craft_recipe_list","player_id":"offline:Alice","ts":1,
              "recipes":[
                {"id":"craft.example.eclipse_needle.iron",
                 "category":"anqi_carrier",
                 "display_name":"蚀针（凡铁档）",
                 "materials":[["iron_needle",3],["chi_xui_cao",1]],
                 "qi_cost":8.0,
                 "time_ticks":3600,
                 "output":["eclipse_needle_iron",3],
                 "requirements":{"qi_color_min":["Insidious",0.05]},
                 "unlocked":false},
                {"id":"craft.example.herb_knife.iron",
                 "category":"tool",
                 "display_name":"采药刀（凡铁）",
                 "materials":[["iron_ingot",1],["wood_handle",1]],
                 "qi_cost":0.0,
                 "time_ticks":600,
                 "output":["herb_knife_iron",1],
                 "requirements":{},
                 "unlocked":true}
              ]
            }
            """;
        ServerDataDispatch dispatch = new CraftRecipeListHandler().handle(envelope(json));
        assertTrue(dispatch.handled());
        assertEquals(2, CraftStore.recipes().size());
        CraftRecipe needle = CraftStore.recipe("craft.example.eclipse_needle.iron").orElseThrow();
        assertEquals(CraftCategory.ANQI_CARRIER, needle.category());
        assertFalse(needle.unlocked());
        assertEquals("Insidious", needle.requirements().qiColorKind());
        assertNotNull(needle.requirements().qiColorMinShare());
        CraftRecipe knife = CraftStore.recipe("craft.example.herb_knife.iron").orElseThrow();
        assertTrue(knife.unlocked());
        assertEquals(0.0, knife.qiCost());
        // requirements 缺省时不应有任何字段被填
        assertNull(knife.requirements().realmMin());
        assertNull(knife.requirements().qiColorKind());
    }

    @Test
    void recipeListHandlerNoOpsOnMissingRecipesArray() {
        String json = "{\"v\":1,\"type\":\"craft_recipe_list\",\"player_id\":\"offline:A\",\"ts\":0}";
        ServerDataDispatch dispatch = new CraftRecipeListHandler().handle(envelope(json));
        assertFalse(dispatch.handled());
    }

    @Test
    void sessionStateHandlerActivePopulatesView() {
        String json = """
            {"v":1,"type":"craft_session_state","player_id":"offline:A","active":true,
             "recipe_id":"craft.example.eclipse_needle.iron","elapsed_ticks":600,
             "total_ticks":3600,"completed_count":1,"total_count":3,"ts":2}
            """;
        ServerDataDispatch dispatch = new CraftSessionStateHandler().handle(envelope(json));
        assertTrue(dispatch.handled());
        CraftSessionStateView view = CraftStore.sessionState();
        assertTrue(view.active());
        assertEquals("craft.example.eclipse_needle.iron", view.recipeId().orElseThrow());
        assertEquals(600, view.elapsedTicks());
        assertEquals(3600, view.totalTicks());
        assertEquals(1, view.completedCount());
        assertEquals(3, view.totalCount());
    }

    @Test
    void sessionStateHandlerInactiveResetsView() {
        // 先推 active
        new CraftSessionStateHandler().handle(envelope(
            "{\"v\":1,\"type\":\"craft_session_state\",\"player_id\":\"offline:A\",\"active\":true,\"recipe_id\":\"a\",\"elapsed_ticks\":1,\"total_ticks\":2,\"ts\":3}"));
        assertTrue(CraftStore.sessionState().active());
        // 再推 inactive
        ServerDataDispatch dispatch = new CraftSessionStateHandler().handle(envelope(
            "{\"v\":1,\"type\":\"craft_session_state\",\"player_id\":\"offline:A\",\"active\":false,\"elapsed_ticks\":0,\"total_ticks\":0,\"ts\":4}"));
        assertTrue(dispatch.handled());
        assertFalse(CraftStore.sessionState().active());
        assertTrue(CraftStore.sessionState().recipeId().isEmpty());
    }

    @Test
    void outcomeHandlerCompletedRecordsOutcome() {
        String json = """
            {"v":1,"type":"craft_outcome","kind":"completed","player_id":"offline:A",
             "recipe_id":"craft.example.eclipse_needle.iron","output_template":"eclipse_needle_iron",
             "output_count":3,"completed_at_tick":5000,"ts":1}
            """;
        ServerDataDispatch dispatch = new CraftOutcomeHandler().handle(envelope(json));
        assertTrue(dispatch.handled());
        CraftStore.CraftOutcomeEvent outcome = CraftStore.lastOutcome().orElseThrow();
        assertEquals(CraftStore.CraftOutcomeEvent.Kind.COMPLETED, outcome.kind());
        assertEquals("eclipse_needle_iron", outcome.outputTemplate());
        assertEquals(3, outcome.outputCount());
    }

    @Test
    void outcomeHandlerFailedRecordsCancellation() {
        String json = """
            {"v":1,"type":"craft_outcome","kind":"failed","player_id":"offline:A",
             "recipe_id":"x","reason":"player_cancelled","material_returned":2,
             "qi_refunded":0.0,"ts":1}
            """;
        ServerDataDispatch dispatch = new CraftOutcomeHandler().handle(envelope(json));
        assertTrue(dispatch.handled());
        CraftStore.CraftOutcomeEvent outcome = CraftStore.lastOutcome().orElseThrow();
        assertEquals(CraftStore.CraftOutcomeEvent.Kind.FAILED, outcome.kind());
        assertEquals("player_cancelled", outcome.failureReason());
        assertEquals(2, outcome.materialReturned());
    }

    @Test
    void outcomeHandlerNoOpsOnUnknownKind() {
        String json = """
            {"v":1,"type":"craft_outcome","kind":"surprise","player_id":"offline:A","recipe_id":"x","ts":1}
            """;
        ServerDataDispatch dispatch = new CraftOutcomeHandler().handle(envelope(json));
        assertFalse(dispatch.handled());
    }

    @Test
    void recipeUnlockedHandlerAllThreeChannelsRecordSource() {
        // scroll
        ServerDataDispatch s = new RecipeUnlockedHandler().handle(envelope("""
            {"v":1,"type":"recipe_unlocked","player_id":"offline:A","recipe_id":"x",
             "source":{"kind":"scroll","item_template":"scroll_x"},"unlocked_at_tick":1,"ts":1}
            """));
        assertTrue(s.handled());
        CraftStore.RecipeUnlockedEvent ev = CraftStore.lastUnlocked().orElseThrow();
        assertTrue(ev.source() instanceof CraftStore.RecipeUnlockedEvent.Scroll);
        // mentor
        ServerDataDispatch m = new RecipeUnlockedHandler().handle(envelope("""
            {"v":1,"type":"recipe_unlocked","player_id":"offline:A","recipe_id":"y",
             "source":{"kind":"mentor","npc_archetype":"poison_master"},"unlocked_at_tick":2,"ts":1}
            """));
        assertTrue(m.handled());
        assertTrue(CraftStore.lastUnlocked().orElseThrow().source()
            instanceof CraftStore.RecipeUnlockedEvent.Mentor);
        // insight
        ServerDataDispatch i = new RecipeUnlockedHandler().handle(envelope("""
            {"v":1,"type":"recipe_unlocked","player_id":"offline:A","recipe_id":"z",
             "source":{"kind":"insight","trigger":"breakthrough"},"unlocked_at_tick":3,"ts":1}
            """));
        assertTrue(i.handled());
        assertTrue(CraftStore.lastUnlocked().orElseThrow().source()
            instanceof CraftStore.RecipeUnlockedEvent.Insight);
    }

    @Test
    void recipeUnlockedHandlerMarksRecipeAsUnlockedInStore() {
        // 先把 recipe 注册成未解锁
        CraftStore.replaceRecipes(java.util.List.of(new CraftRecipe(
            "craft.example.fake_skin.light",
            CraftCategory.TUIKE_SKIN,
            "伪灵皮（轻档）",
            java.util.List.of(),
            2.0,
            120L,
            "fake_skin_light",
            1,
            CraftRecipe.Requirements.NONE,
            false
        )));
        new RecipeUnlockedHandler().handle(envelope("""
            {"v":1,"type":"recipe_unlocked","player_id":"offline:A",
             "recipe_id":"craft.example.fake_skin.light",
             "source":{"kind":"insight","trigger":"near_death"},
             "unlocked_at_tick":99,"ts":1}
            """));
        assertTrue(CraftStore.recipe("craft.example.fake_skin.light").orElseThrow().unlocked());
    }

    @Test
    void recipeUnlockedHandlerNoOpsOnUnknownSourceKind() {
        ServerDataDispatch dispatch = new RecipeUnlockedHandler().handle(envelope("""
            {"v":1,"type":"recipe_unlocked","player_id":"offline:A","recipe_id":"x",
             "source":{"kind":"miracle"},"unlocked_at_tick":1,"ts":1}
            """));
        assertFalse(dispatch.handled());
    }
}
