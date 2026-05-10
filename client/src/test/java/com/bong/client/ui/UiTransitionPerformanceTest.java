package com.bong.client.ui;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UiTransitionPerformanceTest {
    @AfterEach
    void resetSettings() {
        UiTransitionSettings.resetForTests();
    }

    @Test
    void disabled_setting_forces_instant_transition() {
        UiTransitionSettings.setEnabledForTests(false);

        assertEquals(0, UiTransitionSettings.durationFor(300));
    }

    @Test
    void low_fps_suggests_fallback_once() {
        UiTransitionSettings.FpsDecision first = UiTransitionSettings.observeFrameRate(24.0);
        UiTransitionSettings.FpsDecision second = UiTransitionSettings.observeFrameRate(25.0);

        assertTrue(first.showToast());
        assertTrue(first.lowSpecFallback());
        assertFalse(second.showToast());
        assertTrue(second.lowSpecFallback());
    }

    @Test
    void slide_up_offset_scales_with_resolution() {
        ScreenTransition.Frame small = ScreenTransition.sample(
            ScreenTransition.Type.SLIDE_UP,
            300,
            ScreenTransition.Easing.LINEAR,
            0L,
            150L,
            320,
            180
        );
        ScreenTransition.Frame large = ScreenTransition.sample(
            ScreenTransition.Type.SLIDE_UP,
            300,
            ScreenTransition.Easing.LINEAR,
            0L,
            150L,
            640,
            360
        );

        assertEquals(90, small.offsetY());
        assertEquals(180, large.offsetY());
    }

    @Test
    void scale_up_uses_screen_center_contract() {
        ScreenTransition.Frame frame = ScreenTransition.sample(
            ScreenTransition.Type.SCALE_UP,
            400,
            ScreenTransition.Easing.LINEAR,
            0L,
            200L,
            800,
            600
        );

        assertEquals(0.9, frame.scale(), 0.001);
    }
}
