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
        assertTrue(state.voidDistortionActive(),
                "voidDistortionActive should be true for stage >= 3");
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
        assertFalse(state.voidDistortionActive(),
                "voidDistortionActive should be false for stage < 3");
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
        assertFalse(s2.voidDistortionActive(),
                "stage 2 should NOT have sound distortion active");

        VoidErosionVisualStore.reset();

        // Stage 3 → sound_distortion_active=true (EchoBody threshold)
        VoidErosionVisualHandler.handle("""
                {"entity_id":"x","stage":3,"cumulative_erosion":200.0,"ambient_active":true,
                "model_alpha":0.55,"sound_distortion_active":true,"server_tick":0}""");
        VoidErosionVisualStore.State s3 = VoidErosionVisualStore.snapshot();
        assertNotNull(s3);
        assertTrue(s3.voidDistortionActive(),
                "stage 3 SHOULD have sound distortion active — render回路入口保障");
    }

    // ── fix1: store.modelAlpha → render 路径契约 pin ─────────────────────────

    /**
     * 锁住"store.modelAlpha < 1.0 → VoidErosionModelAlphaRenderer 应激活渲染"契约。
     *
     * <p>本测试不依赖 GL 上下文（FeatureRenderer.render 需 MC 主线程），
     * 仅验证 store 数据面：handler 写入正确的 modelAlpha，供 renderer 每帧读取。
     * 这是 e2e 向测试的数据端锚定：若 store 被破坏则 renderer 永不激活。
     */
    @Test
    void modelAlphaStoreToRenderRouteForStage4() {
        // stage 4 → modelAlpha = 0.4（server 计算公式：1.0 - 4 * 0.15）
        String json = """
                {
                  "entity_id": "offline:kiz",
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
        assertNotNull(state, "store 应在 handle() 后有状态");
        // VoidErosionModelAlphaRenderer.ALPHA_THRESHOLD = 0.999f
        // 当 modelAlpha < ALPHA_THRESHOLD 时 renderer 会渲染半透明层
        assertTrue(state.modelAlpha() < 0.999f,
                "stage 4 modelAlpha=" + state.modelAlpha()
                        + " 应 < 0.999f — VoidErosionModelAlphaRenderer 应激活渲染，否则玩家永不半透明");
    }

    @Test
    void modelAlphaStoreNoRenderForStage0() {
        // stage 0 → modelAlpha = 1.0（完全不透明，renderer 应跳过）
        String json = """
                {
                  "entity_id": "offline:kiz",
                  "stage": 0,
                  "cumulative_erosion": 0.0,
                  "ambient_active": false,
                  "model_alpha": 1.0,
                  "sound_distortion_active": false,
                  "server_tick": 0
                }
                """;
        VoidErosionVisualHandler.handle(json);
        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        assertNotNull(state);
        // >= ALPHA_THRESHOLD → renderer 跳过，不额外绘制
        assertTrue(state.modelAlpha() >= 0.999f,
                "stage 0 modelAlpha=" + state.modelAlpha()
                        + " 应 >= 0.999f — VoidErosionModelAlphaRenderer 应跳过渲染");
    }

    @Test
    void modelAlphaProgressesAcrossAllStages() {
        // 验证 stage 1-4 的 modelAlpha 依次递减，确认 server 公式正确传递
        float[] expectedAlphas = {1.0f, 0.85f, 0.70f, 0.55f, 0.40f};
        for (int stage = 0; stage <= 4; stage++) {
            VoidErosionVisualStore.reset();
            float expectedAlpha = expectedAlphas[stage];
            String json = String.format(
                    """
                    {"entity_id":"kiz","stage":%d,"cumulative_erosion":%f,
                     "ambient_active":%b,"model_alpha":%f,
                     "sound_distortion_active":%b,"server_tick":0}
                    """,
                    stage, stage * 100.0, stage >= 1, expectedAlpha, stage >= 3
            );
            VoidErosionVisualHandler.handle(json);
            VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
            assertNotNull(state, "stage " + stage + " 应有状态");
            assertEquals(expectedAlpha, state.modelAlpha(), 0.005f,
                    "stage " + stage + " modelAlpha 应为 " + expectedAlpha
                            + "（公式 1.0 - stage*0.15），实际=" + state.modelAlpha());
        }
    }
}
