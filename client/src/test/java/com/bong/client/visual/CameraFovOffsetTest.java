package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 覆盖 {@link CameraFovOffset} 的纯数学行为：
 * 非 FOV 类 state 返回 0、FOV_ZOOM_IN 产负值、FOV_STRETCH 产正值、振幅随 scaledIntensityAt 衰减。
 */
public class CameraFovOffsetTest {

    @Test
    void nullStateReturnsZero() {
        assertEquals(0.0, CameraFovOffset.compute(null, 0L));
    }

    @Test
    void emptyStateReturnsZero() {
        assertEquals(0.0, CameraFovOffset.compute(VisualEffectState.none(), 0L));
    }

    @Test
    void nonFovStatesReturnZero() {
        assertEquals(0.0, CameraFovOffset.compute(
            VisualEffectState.create("screen_shake", 1.0, 5_000L, 0L), 100L));
        assertEquals(0.0, CameraFovOffset.compute(
            VisualEffectState.create("blood_moon", 1.0, 5_000L, 0L), 100L));
        assertEquals(0.0, CameraFovOffset.compute(
            VisualEffectState.create("title_flash", 1.0, 5_000L, 0L), 100L));
    }

    @Test
    void expiredFovStateReturnsZero() {
        VisualEffectState state = VisualEffectState.create("fov_zoom_in", 1.0, 1_000L, 0L);
        assertEquals(0.0, CameraFovOffset.compute(state, 2_000L));
    }

    @Test
    void fovZoomInReturnsNegativeOffset() {
        VisualEffectState state = VisualEffectState.create("fov_zoom_in", 1.0, 10_000L, 0L);
        double offset = CameraFovOffset.compute(state, 0L);
        // 满强度刚起步：scaled = 1.0 → -MAX_ZOOM_DEGREES
        assertEquals(-CameraFovOffset.MAX_ZOOM_DEGREES, offset, 1e-6);
    }

    @Test
    void fovStretchReturnsPositiveOffset() {
        VisualEffectState state = VisualEffectState.create("fov_stretch", 1.0, 500L, 0L);
        double offset = CameraFovOffset.compute(state, 0L);
        // 满强度刚起步：scaled = 1.0 → +MAX_STRETCH_DEGREES
        assertEquals(CameraFovOffset.MAX_STRETCH_DEGREES, offset, 1e-6);
    }

    @Test
    void zoomAmplitudeBoundedByIntensity() {
        VisualEffectState state = VisualEffectState.create("fov_zoom_in", 0.5, 10_000L, 0L);
        double offset = CameraFovOffset.compute(state, 0L);
        double expectedCap = CameraFovOffset.MAX_ZOOM_DEGREES * 0.5;
        // intensity 0.5 → 峰值只有 -MAX_ZOOM * 0.5
        assertEquals(-expectedCap, offset, 1e-6);
    }

    @Test
    void offsetDecaysLinearlyOverDuration() {
        VisualEffectState state = VisualEffectState.create("fov_stretch", 1.0, 1_000L, 0L);
        double startOffset = CameraFovOffset.compute(state, 0L);
        double midOffset = CameraFovOffset.compute(state, 500L);
        double endOffset = CameraFovOffset.compute(state, 999L);

        assertTrue(startOffset > midOffset, "开始时偏移应大于中途");
        assertTrue(midOffset > endOffset, "中途偏移应大于临近结束");
        // 中点应接近峰值的一半（linear decay）
        assertEquals(startOffset / 2.0, midOffset, 0.5);
    }

    @Test
    void zoomAndStretchDirectionsAreOpposite() {
        VisualEffectState zoom = VisualEffectState.create("fov_zoom_in", 1.0, 5_000L, 0L);
        VisualEffectState stretch = VisualEffectState.create("fov_stretch", 1.0, 5_000L, 0L);
        double zoomOffset = CameraFovOffset.compute(zoom, 0L);
        double stretchOffset = CameraFovOffset.compute(stretch, 0L);
        assertTrue(zoomOffset < 0, "zoom_in 应收窄 FOV (负值)");
        assertTrue(stretchOffset > 0, "stretch 应扩大 FOV (正值)");
    }

    @Test
    void fovProfilesDoNotEmitHudCommands() {
        // FOV 效果通过 Mixin 改相机 FOV，HUD 层不应画任何东西
        VisualEffectState zoomState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("fov_zoom_in", 1.0, 5_000L, 0L),
            0L,
            true
        );
        VisualEffectState stretchState = VisualEffectController.acceptIncoming(
            VisualEffectState.none(),
            VisualEffectState.create("fov_stretch", 1.0, 500L, 0L),
            0L,
            true
        );
        assertTrue(VisualEffectPlanner.buildCommands(
            zoomState, 100L, text -> text.length() * 6, 220, 320, 180, true
        ).isEmpty(), "FOV_ZOOM_IN 不应发 HUD 命令");
        assertTrue(VisualEffectPlanner.buildCommands(
            stretchState, 100L, text -> text.length() * 6, 220, 320, 180, true
        ).isEmpty(), "FOV_STRETCH 不应发 HUD 命令");
    }
}
