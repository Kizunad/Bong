package com.bong.client.network;

import com.bong.client.gathering.GatheringSessionStore;
import com.bong.client.hud.GatheringProgressHud;
import com.bong.client.hud.HudRenderCommand;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class GatheringProgressHandlerTest {
    private static final com.bong.client.hud.HudTextHelper.WidthMeasurer WIDTH = text ->
        text == null ? 0 : text.length() * 6;

    @AfterEach
    void tearDown() {
        GatheringSessionStore.resetForTests();
    }

    @Test
    void miningProgressUpdatesGatheringStoreAndHud() {
        ServerDataRouter.RouteResult result = route("""
            {"v":1,"type":"mining_progress","session_id":"mine-1","ore_pos":[1,64,2],
             "progress":0.42,"interrupted":false,"completed":false}
            """);

        assertTrue(result.isHandled(), result.logMessage());
        assertEquals("mine-1", GatheringSessionStore.snapshot().sessionId());
        assertEquals("矿脉", GatheringSessionStore.snapshot().displayTargetName());
        assertEquals(0.42, GatheringSessionStore.snapshot().progressRatio(), 0.0001);
        List<HudRenderCommand> commands = GatheringProgressHud.buildCommands(WIDTH, 320, 240, System.currentTimeMillis());
        assertFalse(commands.isEmpty());
        assertTrue(commands.stream().anyMatch(command -> command.isText() && command.text().contains("矿脉")));
    }

    @Test
    void lumberProgressUsesDisplayNameAndUpdatesHud() {
        ServerDataRouter.RouteResult result = route("""
            {"v":1,"type":"lumber_progress","session_id":"wood-1","log_pos":[3,70,4],
             "progress":0.25,"interrupted":false,"completed":false,"detail":"青纹灵木"}
            """);

        assertTrue(result.isHandled(), result.logMessage());
        assertEquals("wood-1", GatheringSessionStore.snapshot().sessionId());
        assertEquals("青纹灵木", GatheringSessionStore.snapshot().displayTargetName());
        assertEquals(0.25, GatheringSessionStore.snapshot().progressRatio(), 0.0001);
        List<HudRenderCommand> commands = GatheringProgressHud.buildCommands(WIDTH, 320, 240, System.currentTimeMillis());
        assertTrue(commands.stream().anyMatch(command -> command.isText() && command.text().contains("青纹灵木")));
    }

    @Test
    void completedMiningProgressClearsOnlyMatchingSession() {
        route("""
            {"v":1,"type":"mining_progress","session_id":"mine-1","ore_pos":[1,64,2],
             "progress":0.4,"interrupted":false,"completed":false}
            """);
        route("""
            {"v":1,"type":"lumber_progress","session_id":"wood-1","log_pos":[3,70,4],
             "progress":0.2,"interrupted":false,"completed":false,"detail":"青纹灵木"}
            """);
        route("""
            {"v":1,"type":"mining_progress","session_id":"mine-1","ore_pos":[1,64,2],
             "progress":1.0,"interrupted":false,"completed":true}
            """);

        assertEquals("wood-1", GatheringSessionStore.snapshot().sessionId());

        route("""
            {"v":1,"type":"lumber_progress","session_id":"wood-1","log_pos":[3,70,4],
             "progress":1.0,"interrupted":false,"completed":true,"detail":"青纹灵木"}
            """);

        assertTrue(GatheringSessionStore.snapshot().isEmpty());
        assertTrue(GatheringProgressHud.buildCommands(WIDTH, 320, 240, System.currentTimeMillis()).isEmpty());
    }

    @Test
    void invalidProgressPayloadIsSafeNoOp() {
        ServerDataRouter.RouteResult result = route("""
            {"v":1,"type":"mining_progress","session_id":"mine-1","ore_pos":[1,64,2],
             "progress":"bad","interrupted":false,"completed":false}
            """);

        assertFalse(result.isHandled());
        assertTrue(result.isNoOp());
        assertTrue(GatheringSessionStore.snapshot().isEmpty());
    }

    @Test
    void clearOnDisconnectDropsActiveProgress() {
        route("""
            {"v":1,"type":"lumber_progress","session_id":"wood-1","log_pos":[3,70,4],
             "progress":0.2,"interrupted":false,"completed":false,"detail":"青纹灵木"}
            """);

        GatheringSessionStore.clearOnDisconnect();

        assertTrue(GatheringSessionStore.snapshot().isEmpty());
        assertTrue(GatheringProgressHud.buildCommands(WIDTH, 320, 240, System.currentTimeMillis()).isEmpty());
    }

    private static ServerDataRouter.RouteResult route(String json) {
        return ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);
    }
}
