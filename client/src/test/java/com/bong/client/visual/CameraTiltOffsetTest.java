package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 覆盖 {@link CameraTiltOffset} 钟形曲线 pitch 偏移：
 * 非仰视 state 返回 0、起止端为 0、中点峰值、始终为负（MC 约定：pitch 负 = 看天空）。
 */
public class CameraTiltOffsetTest {

    @Test
    void nullStateReturnsZero() {
        assertEquals(0f, CameraTiltOffset.computePitchDegrees(null, 0L));
    }

    @Test
    void emptyStateReturnsZero() {
        assertEquals(0f, CameraTiltOffset.computePitchDegrees(VisualEffectState.none(), 0L));
    }

    @Test
    void nonTiltStatesReturnZero() {
        assertEquals(0f, CameraTiltOffset.computePitchDegrees(
            VisualEffectState.create("screen_shake", 1.0, 5_000L, 0L), 100L));
        assertEquals(0f, CameraTiltOffset.computePitchDegrees(
            VisualEffectState.create("fov_zoom_in", 1.0, 5_000L, 0L), 100L));
        assertEquals(0f, CameraTiltOffset.computePitchDegrees(
            VisualEffectState.create("blood_moon", 1.0, 5_000L, 0L), 100L));
    }

    @Test
    void startAndEndProduceZeroTilt() {
        VisualEffectState state = VisualEffectState.create("tribulation_look_up", 1.0, 4_000L, 0L);
        // t=0: sin(0) = 0 → 偏移 0°，不会突然抬头
        assertEquals(0f, CameraTiltOffset.computePitchDegrees(state, 0L), 1e-5);
        // t=duration: 已过期，返回 0
        assertEquals(0f, CameraTiltOffset.computePitchDegrees(state, 4_000L), 1e-5);
        // t=duration+1: 仍然 0
        assertEquals(0f, CameraTiltOffset.computePitchDegrees(state, 4_001L), 1e-5);
    }

    @Test
    void midpointHitsPeakTilt() {
        VisualEffectState state = VisualEffectState.create("tribulation_look_up", 1.0, 4_000L, 0L);
        // t=2000（progress=0.5）: sin(π/2) = 1 → 满强度峰值 -25°
        float midpointPitch = CameraTiltOffset.computePitchDegrees(state, 2_000L);
        assertEquals(-CameraTiltOffset.MAX_TILT_DEGREES, midpointPitch, 1e-4);
    }

    @Test
    void tiltIsAlwaysNegativeMeaningLookUp() {
        VisualEffectState state = VisualEffectState.create("tribulation_look_up", 1.0, 4_000L, 0L);
        // 整个生命周期内采样，应全部 ≤ 0
        for (long t = 1L; t < 4_000L; t++) {
            float pitch = CameraTiltOffset.computePitchDegrees(state, t);
            assertTrue(pitch <= 0f, "t=" + t + " 时 pitch=" + pitch + " 应 ≤ 0（看天/平视）");
        }
    }

    @Test
    void intensityScalesAmplitudeLinearly() {
        VisualEffectState half = VisualEffectState.create("tribulation_look_up", 0.5, 4_000L, 0L);
        VisualEffectState full = VisualEffectState.create("tribulation_look_up", 1.0, 4_000L, 0L);
        float halfPeak = CameraTiltOffset.computePitchDegrees(half, 2_000L);
        float fullPeak = CameraTiltOffset.computePitchDegrees(full, 2_000L);
        assertEquals(fullPeak / 2f, halfPeak, 1e-4);
    }

    @Test
    void symmetricAroundMidpoint() {
        VisualEffectState state = VisualEffectState.create("tribulation_look_up", 1.0, 4_000L, 0L);
        // progress=0.25 对称于 progress=0.75（sin(π/4)=sin(3π/4)）
        float p025 = CameraTiltOffset.computePitchDegrees(state, 1_000L);
        float p075 = CameraTiltOffset.computePitchDegrees(state, 3_000L);
        assertEquals(p025, p075, 1e-4);
    }

    @Test
    void tribulationLookUpDoesNotEmitHudCommands() {
        VisualEffectState state = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("tribulation_look_up", 1.0, 4_000L, 0L),
            0L,
            true
        );
        assertTrue(VisualEffectPlanner.buildCommands(
            state, 2_000L, text -> text.length() * 6, 220, 320, 180, true
        ).isEmpty(), "TRIBULATION_LOOK_UP 纯相机效果不应发 HUD 命令");
    }
}
