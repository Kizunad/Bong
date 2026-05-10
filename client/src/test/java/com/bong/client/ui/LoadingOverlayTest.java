package com.bong.client.ui;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class LoadingOverlayTest {
    @AfterEach
    void resetSettings() {
        UiTransitionSettings.resetForTests();
    }

    @Test
    void loading_overlay_shows_on_async() {
        LoadingOverlay.Snapshot snapshot = LoadingOverlay.snapshot(1_000L, 1_500L, false, false);

        assertEquals(LoadingOverlay.Phase.LOADING, snapshot.phase());
        assertEquals("凝神中...", snapshot.message());
        assertEquals(5, snapshot.particles().size());
    }

    @Test
    void loading_timeout_3s_retry() {
        LoadingOverlay.Snapshot snapshot = LoadingOverlay.snapshot(1_000L, 4_000L, false, false);

        assertEquals(LoadingOverlay.Phase.RETRY, snapshot.phase());
        assertEquals("灵脉堵塞，稍后再试", snapshot.message());
        assertEquals("重试", snapshot.buttonLabels().get(0));
    }

    @Test
    void loading_timeout_10s_lost() {
        LoadingOverlay.Snapshot snapshot = LoadingOverlay.snapshot(1_000L, 11_000L, false, false);

        assertEquals(LoadingOverlay.Phase.LOST, snapshot.phase());
        assertTrue(snapshot.buttonLabels().contains("返回主世界"));
    }

    @Test
    void preload_parallel_with_transition() {
        LoadingOverlay.PreloadFrame duringTransition = LoadingOverlay.preloadFrame(
            1_000L,
            1_000L,
            400,
            1_200L,
            false
        );
        LoadingOverlay.PreloadFrame afterTransition = LoadingOverlay.preloadFrame(
            1_000L,
            1_000L,
            400,
            1_450L,
            false
        );
        LoadingOverlay.PreloadFrame ready = LoadingOverlay.preloadFrame(
            1_000L,
            1_000L,
            400,
            1_450L,
            true
        );

        assertTrue(duringTransition.transitionRunning());
        assertFalse(duringTransition.loadingVisible());
        assertTrue(afterTransition.loadingVisible());
        assertTrue(ready.readyToOpen());
    }

    @Test
    void low_spec_fallback_removes_ink_particles() {
        UiTransitionSettings.setLowSpecFallbackForTests(true);

        LoadingOverlay.Snapshot snapshot = LoadingOverlay.snapshot(1_000L, 1_500L, false, UiTransitionSettings.lowSpecFallback());

        assertTrue(snapshot.particles().isEmpty());
    }
}
