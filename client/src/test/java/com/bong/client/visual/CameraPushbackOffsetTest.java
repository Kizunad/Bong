package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 覆盖 {@link CameraPushbackOffset} 的纯数学行为：
 * 非 HIT_PUSHBACK 返回 0、满强度达 MAX_PUSHBACK_BLOCKS、线性衰减、intensity 缩放。
 */
public class CameraPushbackOffsetTest {

    @Test
    void nullStateReturnsZero() {
        assertEquals(0.0, CameraPushbackOffset.computeBackwardDistance(null, 0L));
    }

    @Test
    void emptyStateReturnsZero() {
        assertEquals(0.0, CameraPushbackOffset.computeBackwardDistance(VisualEffectState.none(), 0L));
    }

    @Test
    void nonPushbackStatesReturnZero() {
        assertEquals(0.0, CameraPushbackOffset.computeBackwardDistance(
            VisualEffectState.create("screen_shake", 1.0, 5_000L, 0L), 100L));
        assertEquals(0.0, CameraPushbackOffset.computeBackwardDistance(
            VisualEffectState.create("tribulation_look_up", 1.0, 5_000L, 0L), 100L));
        assertEquals(0.0, CameraPushbackOffset.computeBackwardDistance(
            VisualEffectState.create("blood_moon", 1.0, 5_000L, 0L), 100L));
    }

    @Test
    void expiredStateReturnsZero() {
        VisualEffectState state = VisualEffectState.create("hit_pushback", 1.0, 200L, 0L);
        assertEquals(0.0, CameraPushbackOffset.computeBackwardDistance(state, 500L));
    }

    @Test
    void fullIntensityAtStartReachesMaxDistance() {
        VisualEffectState state = VisualEffectState.create("hit_pushback", 1.0, 350L, 0L);
        // t=0: scaled = 1.0 → 距离 = MAX_PUSHBACK_BLOCKS
        assertEquals(CameraPushbackOffset.MAX_PUSHBACK_BLOCKS,
            CameraPushbackOffset.computeBackwardDistance(state, 0L), 1e-6);
    }

    @Test
    void distanceDecaysLinearly() {
        VisualEffectState state = VisualEffectState.create("hit_pushback", 1.0, 1_000L, 0L);
        double start = CameraPushbackOffset.computeBackwardDistance(state, 0L);
        double mid = CameraPushbackOffset.computeBackwardDistance(state, 500L);
        double late = CameraPushbackOffset.computeBackwardDistance(state, 900L);

        assertTrue(start > mid, "开始 > 中途");
        assertTrue(mid > late, "中途 > 末尾");
        // linear decay: 500ms 对应约一半
        assertEquals(start / 2.0, mid, 1e-3);
    }

    @Test
    void intensityScalesDistanceLinearly() {
        VisualEffectState half = VisualEffectState.create("hit_pushback", 0.5, 350L, 0L);
        VisualEffectState full = VisualEffectState.create("hit_pushback", 1.0, 350L, 0L);
        double halfDist = CameraPushbackOffset.computeBackwardDistance(half, 0L);
        double fullDist = CameraPushbackOffset.computeBackwardDistance(full, 0L);
        assertEquals(fullDist / 2.0, halfDist, 1e-6);
    }

    @Test
    void distanceAlwaysNonNegative() {
        VisualEffectState state = VisualEffectState.create("hit_pushback", 1.0, 350L, 0L);
        for (long t = 0L; t <= 350L; t++) {
            double d = CameraPushbackOffset.computeBackwardDistance(state, t);
            assertTrue(d >= 0.0, "距离应始终非负，t=" + t + " d=" + d);
        }
    }

    @Test
    void hitPushbackDoesNotEmitHudCommands() {
        VisualEffectState state = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("hit_pushback", 1.0, 350L, 0L),
            0L,
            true
        );
        assertTrue(VisualEffectPlanner.buildCommands(
            state, 100L, text -> text.length() * 6, 220, 320, 180, true
        ).isEmpty(), "HIT_PUSHBACK 纯相机效果不应发 HUD 命令");
    }
}
