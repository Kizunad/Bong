package com.bong.client.processing;

import com.bong.client.hud.FreshnessTooltipHook;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.processing.state.FreshnessStore;
import com.bong.client.processing.state.ProcessingSessionStore;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ProcessingClientTest {
    @BeforeEach
    void resetStores() {
        ProcessingSessionStore.resetForTests();
        FreshnessStore.clearForTests();
    }

    @Test
    void client_processing_action_screen_renders_progress_bar() {
        ProcessingSessionStore.replace(new ProcessingSessionStore.Snapshot(
            true, "processing:1", ProcessingSessionStore.Kind.GRINDING,
            "grind_ci_she_hao", 300, 600, "offline:Azure"
        ));

        assertEquals("§a碾粉 50% §7300/600",
            ProcessingActionScreen.formatProgress(ProcessingSessionStore.snapshot()));
    }

    @Test
    void freshness_tooltip_renders_quantitative_value() {
        FreshnessStore.upsert("item:7", 0.42f, "grinding_v1");
        assertEquals("鲜度: 42/100 · Exponential", FreshnessTooltipHook.tooltipLine("item:7"));
    }

    @Test
    void freshness_update_payload_pushes_on_threshold_change() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        var result = router.route("""
            {"v":1,"type":"freshness_update","item_uuid":"item:9","freshness":0.76,"profile_name":"drying_v1"}
            """, 128);

        assertTrue(result.isHandled());
        assertEquals(0.76f, FreshnessStore.get("item:9").freshness(), 0.0001f);
    }

    @Test
    void client_receives_processing_session_data_payload() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        var result = router.route("""
            {"v":1,"type":"processing_session","session_id":"processing:2","kind":"extraction","recipe_id":"extract_ci_she_hao","progress_ticks":120,"duration_ticks":12000,"player_id":"offline:Azure"}
            """, 192);

        assertTrue(result.isHandled());
        assertEquals(ProcessingSessionStore.Kind.EXTRACTION, ProcessingSessionStore.snapshot().kind());
        assertEquals("extract_ci_she_hao", ProcessingSessionStore.snapshot().recipeId());
    }

    @Test
    void processing_session_payload_can_clear_active_snapshot() {
        ProcessingSessionStore.replace(new ProcessingSessionStore.Snapshot(
            true, "processing:old", ProcessingSessionStore.Kind.DRYING,
            "dry_ci_she_hao", 10, 20, "offline:Azure"
        ));

        ServerDataRouter router = ServerDataRouter.createDefault();
        var result = router.route("""
            {"v":1,"type":"processing_session","active":false,"session_id":"","kind":"drying","recipe_id":"","progress_ticks":0,"duration_ticks":0,"player_id":"offline:Azure"}
            """, 192);

        assertTrue(result.isHandled());
        assertEquals(false, ProcessingSessionStore.snapshot().active());
    }
}
