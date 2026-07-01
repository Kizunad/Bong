package com.bong.client.network;

import com.bong.client.coffin.TutorialCoffinPosStore;
import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F9 跨层修复 — {@code tutorial_coffin_pos} server_data handler 饱和测试。
 */
class TutorialCoffinPosHandlerTest {
    @AfterEach
    void resetStore() {
        TutorialCoffinPosStore.resetForTests();
    }

    @Test
    void acceptsValidPosAndWritesStore() {
        String json = """
            {"type":"tutorial_coffin_pos","v":1,"x":12,"y":71,"z":-33}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "valid tutorial_coffin_pos should be handled: " + result.logMessage());
        assertEquals(new BlockPos(12, 71, -33), TutorialCoffinPosStore.snapshot().orElseThrow());
    }

    @Test
    void acceptsOriginCoordinatesWithoutTreatingThemAsMissing() {
        // x=0/y=0/z=0 is a legitimate coordinate (world origin), must not be confused with
        // "field absent" — this pins the includingDefaultValueFields contract end to end.
        String json = """
            {"type":"tutorial_coffin_pos","v":1,"x":0,"y":0,"z":0}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled());
        assertEquals(new BlockPos(0, 0, 0), TutorialCoffinPosStore.snapshot().orElseThrow());
    }

    @Test
    void acceptsNegativeCoordinates() {
        String json = """
            {"type":"tutorial_coffin_pos","v":1,"x":-4200,"y":96,"z":-1800}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled());
        assertEquals(new BlockPos(-4200, 96, -1800), TutorialCoffinPosStore.snapshot().orElseThrow());
    }

    @Test
    void rejectsPayloadMissingX() {
        String json = """
            {"type":"tutorial_coffin_pos","v":1,"y":71,"z":-33}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp(), "missing x must be rejected as a no-op, not default to 0");
        assertFalse(TutorialCoffinPosStore.snapshot().isPresent(),
            "store must remain untouched when the payload is rejected");
    }

    @Test
    void rejectsPayloadMissingYAndZ() {
        String json = """
            {"type":"tutorial_coffin_pos","v":1,"x":12}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp());
        assertFalse(TutorialCoffinPosStore.snapshot().isPresent());
    }

    @Test
    void rejectsNonIntegerCoordinate() {
        String json = """
            {"type":"tutorial_coffin_pos","v":1,"x":12.5,"y":71,"z":-33}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp(), "fractional x is not a valid block coordinate, must be rejected");
        assertFalse(TutorialCoffinPosStore.snapshot().isPresent());
    }

    @Test
    void doesNotOverwriteAPreviouslyGoodValueWithARejectedPayload() {
        TutorialCoffinPosStore.set(new BlockPos(1, 2, 3));

        String badJson = """
            {"type":"tutorial_coffin_pos","v":1,"y":71,"z":-33}
            """;
        ServerDataRouter.createDefault().route(badJson, badJson.getBytes(StandardCharsets.UTF_8).length);

        assertEquals(new BlockPos(1, 2, 3), TutorialCoffinPosStore.snapshot().orElseThrow(),
            "a malformed follow-up payload must not clobber the last known-good broadcast pos");
    }
}
