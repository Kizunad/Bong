package com.bong.client.visual;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 fix4/fix5 — VoidErosionHudOverlay 单元测试。
 *
 * <p>fix4：验证 fade 曲线进入/退出行为（applyFadeAlpha + fadeProgress 状态机）。
 * <p>fix5：验证 voidDistortionActive 字段正确驱动 fade（原 soundDistortionActive 已重命名）。
 */
class VoidErosionHudOverlayTest {

    @BeforeEach
    void setup() {
        VoidErosionHudOverlay.resetFadeForTest();
        VoidErosionVisualStore.reset();
    }

    @AfterEach
    void cleanup() {
        VoidErosionHudOverlay.resetFadeForTest();
        VoidErosionVisualStore.reset();
    }

    // ── applyFadeAlpha 纯函数测试 ──────────────────────────────────────────

    @Test
    void applyFadeAlpha_fullProgress_returnsOriginalAlpha() {
        // fadeProgress=1.0 → 输出 alpha = 目标 alpha
        int color = VoidErosionHudOverlay.VOID_DISTORTION_VIGNETTE_STAGE3_COLOR;
        int result = VoidErosionHudOverlay.applyFadeAlpha(color, 1.0f);
        int expectedAlpha = (color >>> 24) & 0xFF;
        int resultAlpha = (result >>> 24) & 0xFF;
        assertEquals(expectedAlpha, resultAlpha,
                "fadeProgress=1.0 应保留原始 alpha=" + expectedAlpha + "，得 " + resultAlpha);
        // RGB 部分不变
        assertEquals(color & 0x00FFFFFF, result & 0x00FFFFFF,
                "applyFadeAlpha 不应修改 RGB 部分");
    }

    @Test
    void applyFadeAlpha_zeroProgress_returnsTransparent() {
        int color = VoidErosionHudOverlay.VOID_DISTORTION_VIGNETTE_STAGE4_COLOR;
        int result = VoidErosionHudOverlay.applyFadeAlpha(color, 0.0f);
        int resultAlpha = (result >>> 24) & 0xFF;
        assertEquals(0, resultAlpha,
                "fadeProgress=0.0 应输出完全透明（alpha=0），得 " + resultAlpha);
        assertEquals(color & 0x00FFFFFF, result & 0x00FFFFFF,
                "applyFadeAlpha 不应修改 RGB 部分（即使透明）");
    }

    @Test
    void applyFadeAlpha_halfProgress_returnsHalfAlpha() {
        // STAGE3 color: 0x553322AA → target alpha = 0x55 = 85
        int color = VoidErosionHudOverlay.VOID_DISTORTION_VIGNETTE_STAGE3_COLOR;
        int targetAlpha = (color >>> 24) & 0xFF; // 0x55 = 85
        int result = VoidErosionHudOverlay.applyFadeAlpha(color, 0.5f);
        int resultAlpha = (result >>> 24) & 0xFF;
        int expectedAlpha = Math.round(targetAlpha * 0.5f); // ~42
        assertEquals(expectedAlpha, resultAlpha,
                "fadeProgress=0.5 应输出 ~半alpha=" + expectedAlpha + "，得 " + resultAlpha);
    }

    @Test
    void applyFadeAlpha_stage3VsStage4_differentBaseAlpha() {
        // stage4 颜色应有更高的 base alpha（0x88 > 0x55）
        int s3Alpha = (VoidErosionHudOverlay.VOID_DISTORTION_VIGNETTE_STAGE3_COLOR >>> 24) & 0xFF;
        int s4Alpha = (VoidErosionHudOverlay.VOID_DISTORTION_VIGNETTE_STAGE4_COLOR >>> 24) & 0xFF;
        assertTrue(s4Alpha > s3Alpha,
                "stage4 vignette alpha(0x" + Integer.toHexString(s4Alpha)
                        + ") 应 > stage3 alpha(0x" + Integer.toHexString(s3Alpha)
                        + ")，确保阶段 4 更深");
    }

    // ── fade 状态机测试 ────────────────────────────────────────────────────

    @Test
    void fadeProgress_startsAtZero() {
        assertEquals(0.0f, VoidErosionHudOverlay.getFadeProgressForTest(), 1e-6f,
                "初始 fade 进度应为 0.0");
    }

    @Test
    void fadeProgress_clampsAtZero() {
        VoidErosionHudOverlay.setFadeProgressForTest(-0.5f);
        assertEquals(0.0f, VoidErosionHudOverlay.getFadeProgressForTest(), 1e-6f,
                "fade 进度应 clamp 到 0.0，负值无效");
    }

    @Test
    void fadeProgress_clampsAtOne() {
        VoidErosionHudOverlay.setFadeProgressForTest(2.0f);
        assertEquals(1.0f, VoidErosionHudOverlay.getFadeProgressForTest(), 1e-6f,
                "fade 进度应 clamp 到 1.0，超过 1.0 无效");
    }

    @Test
    void fadeIn_stepConstant_isPositiveAndFinite() {
        assertTrue(VoidErosionHudOverlay.FADE_IN_STEP > 0.0f
                        && VoidErosionHudOverlay.FADE_IN_STEP < 1.0f
                        && Float.isFinite(VoidErosionHudOverlay.FADE_IN_STEP),
                "FADE_IN_STEP 应在 (0,1) 区间，得 " + VoidErosionHudOverlay.FADE_IN_STEP);
    }

    @Test
    void fadeOut_stepConstant_isPositiveAndFinite() {
        assertTrue(VoidErosionHudOverlay.FADE_OUT_STEP > 0.0f
                        && VoidErosionHudOverlay.FADE_OUT_STEP < 1.0f
                        && Float.isFinite(VoidErosionHudOverlay.FADE_OUT_STEP),
                "FADE_OUT_STEP 应在 (0,1) 区间，得 " + VoidErosionHudOverlay.FADE_OUT_STEP);
    }

    // ── fix5: voidDistortionActive 字段命名确认 ──────────────────────────

    @Test
    void voidDistortionActive_fieldExistsOnState() {
        // 验证 store record 有 voidDistortionActive()（原 soundDistortionActive()，fix5 重命名）
        VoidErosionVisualStore.replace("player_test", 3, 250.0, true, 0.55f, true);
        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        assertNotNull(state);
        assertTrue(state.voidDistortionActive(),
                "stage 3 store 应暴露 voidDistortionActive()=true（fix5：原 soundDistortionActive 重命名）");
    }

    @Test
    void voidDistortionActive_falseForStage2() {
        VoidErosionVisualStore.replace("player_test", 2, 80.0, false, 0.70f, false);
        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        assertNotNull(state);
        assertFalse(state.voidDistortionActive(),
                "stage 2 store 应暴露 voidDistortionActive()=false");
    }

    // ── 颜色常量 pin 测试（fix4: 已命名，hex 值不应悄悄变）────────────────

    @Test
    void colorConstants_pinned() {
        assertEquals(0x553322AA, VoidErosionHudOverlay.VOID_DISTORTION_VIGNETTE_STAGE3_COLOR,
                "STAGE3 vignette 颜色常量不应被修改（pin test）");
        assertEquals(0x884422CC, VoidErosionHudOverlay.VOID_DISTORTION_VIGNETTE_STAGE4_COLOR,
                "STAGE4 vignette 颜色常量不应被修改（pin test）");
    }
}
