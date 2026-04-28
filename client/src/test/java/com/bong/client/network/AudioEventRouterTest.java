package com.bong.client.network;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class AudioEventRouterTest {
    @Test
    void dispatchesPlayToBridge() throws IOException {
        RecordingBridge bridge = new RecordingBridge(true);
        AudioEventRouter router = new AudioEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("valid-audio-play.json");

        AudioEventRouter.RouteResult result = router.routePlay(json, jsonLen(json));

        assertTrue(result.isHandled(), result.logMessage());
        assertEquals(1, bridge.playCalls.size());
        assertEquals("pill_consume", bridge.playCalls.get(0).recipeId());
    }

    @Test
    void dispatchesStopToBridge() throws IOException {
        RecordingBridge bridge = new RecordingBridge(true);
        AudioEventRouter router = new AudioEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("valid-audio-stop.json");

        AudioEventRouter.RouteResult result = router.routeStop(json, jsonLen(json));

        assertTrue(result.isHandled(), result.logMessage());
        assertEquals(1, bridge.stopCalls.size());
        assertEquals(42L, bridge.stopCalls.get(0).instanceId());
    }

    @Test
    void bridgeDeclineBecomesBridgeMiss() throws IOException {
        RecordingBridge bridge = new RecordingBridge(false);
        AudioEventRouter router = new AudioEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("valid-audio-play.json");

        AudioEventRouter.RouteResult result = router.routePlay(json, jsonLen(json));

        assertTrue(result.isBridgeMiss());
        assertEquals(1, bridge.playCalls.size(), "bridge should still receive valid payload");
    }

    @Test
    void parseErrorShortCircuits() throws IOException {
        RecordingBridge bridge = new RecordingBridge(true);
        AudioEventRouter router = new AudioEventRouter(bridge);
        String json = PayloadFixtureLoader.readText("invalid-audio-bad-layer-pitch.json");

        AudioEventRouter.RouteResult result = router.routePlay(json, jsonLen(json));

        assertTrue(result.isParseError());
        assertEquals(0, bridge.playCalls.size());
    }

    private static int jsonLen(String json) {
        return json.getBytes(StandardCharsets.UTF_8).length;
    }

    private static final class RecordingBridge implements AudioPlaybackBridge {
        final List<AudioEventPayload.PlaySoundRecipe> playCalls = new ArrayList<>();
        final List<AudioEventPayload.StopSoundRecipe> stopCalls = new ArrayList<>();
        final boolean returnValue;

        RecordingBridge(boolean returnValue) {
            this.returnValue = returnValue;
        }

        @Override
        public boolean play(AudioEventPayload.PlaySoundRecipe payload) {
            playCalls.add(payload);
            return returnValue;
        }

        @Override
        public boolean stop(AudioEventPayload.StopSoundRecipe payload) {
            stopCalls.add(payload);
            return returnValue;
        }
    }
}
