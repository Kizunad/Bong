package com.bong.client.network;

import com.bong.client.hud.SearchHudState;
import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.tsy.TsyContainerStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ContainerInteractionHandlerTest {
    @AfterEach
    void tearDown() {
        TsyContainerStateStore.resetForTests();
        SearchHudStateStore.resetForTests();
    }

    @Test
    void routerRegistersContainerInteractionPayloads() {
        ServerDataRouter router = ServerDataRouter.createDefault();

        assertTrue(router.registeredTypes().contains("container_state"));
        assertTrue(router.registeredTypes().contains("search_started"));
        assertTrue(router.registeredTypes().contains("search_progress"));
        assertTrue(router.registeredTypes().contains("search_completed"));
        assertTrue(router.registeredTypes().contains("search_aborted"));
    }

    @Test
    void containerStateFeedsStoreAndSearchHud() {
        route("""
            {"type":"container_state","v":1,"entity_id":42,"kind":"storage_pouch","family_id":"tsy","world_pos":[1.0,2.0,3.0],"depleted":false}
            """);

        assertEquals(42L, TsyContainerStateStore.get(42L).entityId());
        assertEquals("储物袋残骸", TsyContainerStateStore.get(42L).kindLabelZh());

        route("""
            {"type":"search_started","v":1,"player_id":"offline:Kiz","container_entity_id":42,"required_ticks":200,"at_tick":10}
            """);
        assertEquals(SearchHudState.Phase.SEARCHING, SearchHudStateStore.snapshot().phase());
        assertEquals("储物袋残骸", SearchHudStateStore.snapshot().containerKindZh());

        route("""
            {"type":"search_progress","v":1,"player_id":"offline:Kiz","container_entity_id":42,"elapsed_ticks":20,"required_ticks":200}
            """);
        assertEquals(20, SearchHudStateStore.snapshot().elapsedTicks());

        route("""
            {"type":"search_aborted","v":1,"player_id":"offline:Kiz","container_entity_id":42,"reason":"cancelled","at_tick":30}
            """);
        assertEquals(SearchHudState.Phase.ABORTED_FLASH, SearchHudStateStore.snapshot().phase());
        assertEquals(SearchHudState.AbortReason.CANCELLED, SearchHudStateStore.snapshot().abortReason());
    }

    private static void route(String json) {
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json.strip(), json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(result.dispatch().handled(), result.dispatch().logMessage());
    }
}
