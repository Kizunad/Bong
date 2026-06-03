package com.bong.client.visual;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 — VoidErosionVisualHandler + VoidErosionVisualStore
 * 端对端解析测试。
 *
 * <p>测契约不测实现：断言 handler 解析 JSON → store 状态更新的外部可观察行为。
 */
class VoidErosionVisualHandlerTest {

    @AfterEach
    void cleanup() {
        VoidErosionVisualStore.reset();
    }

    // ── happy path ───────────────────────────────────────────────────────────

    @Test
    void handleStage3SoundDistortionActive() {
        String json = """
                {
                  "entity_id": "offline:kiz",
                  "stage": 3,
                  "cumulative_erosion": 250.0,
                  "ambient_active": true,
                  "model_alpha": 0.55,
                  "sound_distortion_active": true,
                  "server_tick": 5000
                }
                """;
        VoidErosionVisualHandler.handle(json);

        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        assertNotNull(state, "store should have a state after handle()");
        assertEquals("offline:kiz", state.entityId(),
                "entityId should match JSON entity_id");
        assertEquals(3, state.stage(),
                "stage should be 3 (EchoBody)");
        assertEquals(250.0, state.cumulativeErosion(), 1e-6,
                "cumulativeErosion should match JSON cumulative_erosion");
        assertTrue(state.ambientActive(),
                "ambientActive should be true");
        assertEquals(0.55f, state.modelAlpha(), 0.001f,
                "modelAlpha should match JSON model_alpha");
        assertTrue(state.soundDistortionActive(),
                "soundDistortionActive should be true for stage >= 3");
    }

    @Test
    void handleStage1SoundDistortionInactive() {
        String json = """
                {
                  "entity_id": "offline:player2",
                  "stage": 1,
                  "cumulative_erosion": 25.0,
                  "ambient_active": false,
                  "model_alpha": 0.85,
                  "sound_distortion_active": false,
                  "server_tick": 1200
                }
                """;
        VoidErosionVisualHandler.handle(json);

        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        assertNotNull(state);
        assertEquals(1, state.stage(),
                "stage should be 1 (LowPressure)");
        assertFalse(state.soundDistortionActive(),
                "soundDistortionActive should be false for stage < 3");
    }

    @Test
    void handleStage4FullErosion() {
        String json = """
                {
                  "entity_id": "offline:void_player",
                  "stage": 4,
                  "cumulative_erosion": 420.0,
                  "ambient_active": true,
                  "model_alpha": 0.4,
                  "sound_distortion_active": true,
                  "server_tick": 99999
                }
                """;
        VoidErosionVisualHandler.handle(json);

        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        assertNotNull(state);
        assertEquals(4, state.stage(),
                "stage should be 4 (VoidEroded)");
        assertEquals(0.4f, state.modelAlpha(), 0.001f,
                "modelAlpha should be 0.4 at stage 4");
    }

    // ── replace / overwrite ──────────────────────────────────────────────────

    @Test
    void secondHandleReplacesFirstState() {
        VoidErosionVisualHandler.handle("""
                {"entity_id":"a","stage":1,"cumulative_erosion":25.0,"ambient_active":false,
                "model_alpha":0.85,"sound_distortion_active":false,"server_tick":100}""");
        VoidErosionVisualHandler.handle("""
                {"entity_id":"b","stage":3,"cumulative_erosion":250.0,"ambient_active":true,
                "model_alpha":0.55,"sound_distortion_active":true,"server_tick":200}""");

        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        assertNotNull(state);
        assertEquals("b", state.entityId(),
                "store should reflect the latest handle() call");
        assertEquals(3, state.stage());
    }

    // ── edge / error cases ───────────────────────────────────────────────────

    @Test
    void handleSilentlyIgnoresMalformedJson() {
        // Should not throw, store stays null
        VoidErosionVisualHandler.handle("{not valid json");
        assertNull(VoidErosionVisualStore.snapshot(),
                "malformed JSON should not update store");
    }

    @Test
    void handleStageClampedToZeroFourRange() {
        // stage=-1 should be clamped to 0
        String jsonNeg = """
                {"entity_id":"x","stage":-1,"cumulative_erosion":0,"ambient_active":false,
                "model_alpha":1.0,"sound_distortion_active":false,"server_tick":0}""";
        VoidErosionVisualHandler.handle(jsonNeg);
        VoidErosionVisualStore.State stateNeg = VoidErosionVisualStore.snapshot();
        assertNotNull(stateNeg);
        assertEquals(0, stateNeg.stage(),
                "stage=-1 should be clamped to 0");

        VoidErosionVisualStore.reset();

        // stage=10 should be clamped to 4
        String jsonHigh = """
                {"entity_id":"x","stage":10,"cumulative_erosion":0,"ambient_active":false,
                "model_alpha":1.0,"sound_distortion_active":false,"server_tick":0}""";
        VoidErosionVisualHandler.handle(jsonHigh);
        VoidErosionVisualStore.State stateHigh = VoidErosionVisualStore.snapshot();
        assertNotNull(stateHigh);
        assertEquals(4, stateHigh.stage(),
                "stage=10 should be clamped to 4");
    }

    @Test
    void handleModelAlphaClampedToOneRange() {
        // model_alpha > 1.0 should be clamped to 1.0
        String jsonOver = """
                {"entity_id":"x","stage":0,"cumulative_erosion":0,"ambient_active":false,
                "model_alpha":2.5,"sound_distortion_active":false,"server_tick":0}""";
        VoidErosionVisualHandler.handle(jsonOver);
        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        assertNotNull(state);
        assertEquals(1.0f, state.modelAlpha(), 0.001f,
                "model_alpha=2.5 should be clamped to 1.0");
    }

    @Test
    void resetClearsState() {
        VoidErosionVisualHandler.handle("""
                {"entity_id":"x","stage":2,"cumulative_erosion":90.0,"ambient_active":false,
                "model_alpha":0.7,"sound_distortion_active":false,"server_tick":0}""");
        assertNotNull(VoidErosionVisualStore.snapshot(), "state should be set");
        VoidErosionVisualStore.reset();
        assertNull(VoidErosionVisualStore.snapshot(),
                "reset() should clear the store");
    }

    // ── store round-trip (sound distortion rendering gate) ───────────────────

    @Test
    void soundDistortionOverlayGateForStage3And4() {
        // Stage 2 → sound_distortion_active=false (below threshold)
        VoidErosionVisualHandler.handle("""
                {"entity_id":"x","stage":2,"cumulative_erosion":80.0,"ambient_active":false,
                "model_alpha":0.7,"sound_distortion_active":false,"server_tick":0}""");
        VoidErosionVisualStore.State s2 = VoidErosionVisualStore.snapshot();
        assertNotNull(s2);
        assertFalse(s2.soundDistortionActive(),
                "stage 2 should NOT have sound distortion active");

        VoidErosionVisualStore.reset();

        // Stage 3 → sound_distortion_active=true (EchoBody threshold)
        VoidErosionVisualHandler.handle("""
                {"entity_id":"x","stage":3,"cumulative_erosion":200.0,"ambient_active":true,
                "model_alpha":0.55,"sound_distortion_active":true,"server_tick":0}""");
        VoidErosionVisualStore.State s3 = VoidErosionVisualStore.snapshot();
        assertNotNull(s3);
        assertTrue(s3.soundDistortionActive(),
                "stage 3 SHOULD have sound distortion active — render回路入口保障");
    }
}
