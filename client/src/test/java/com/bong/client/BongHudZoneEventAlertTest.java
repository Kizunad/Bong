package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongHudZoneEventAlertTest {
    @BeforeEach
    void setUp() {
        NarrationState.clear();
        ZoneState.clear();
        EventAlertState.clear();
    }

    @AfterEach
    void tearDown() {
        NarrationState.clear();
        ZoneState.clear();
        EventAlertState.clear();
    }

    @Test
    public void hudCanComposeBaselineZoneBannerAndNarrationToast() {
        ZoneState.recordZoneInfo(new BongServerPayload.ZoneInfo("blood_valley", 0.42d, 3, null), 1_000L);
        EventAlertState.recordAlert(
                new BongServerPayload.EventAlert(
                        "thunder_tribulation",
                        "雷劫将至",
                        "血谷上空劫云汇聚，三十息内可能落雷。",
                        "critical",
                        "blood_valley"
                ),
                1_000L
        );
        NarrationState.recordNarration(
                new BongServerPayload.Narration("broadcast", "雷劫将至，速避高处。", "system_warning"),
                1_000L,
                ignored -> {
                }
        );

        BongHud.HudSnapshot snapshot = BongHud.snapshot(2_000L);
        RecordingHudSurface surface = new RecordingHudSurface(320, 180);

        assertDoesNotThrow(() -> BongHud.renderSurface(surface, snapshot));
        assertTrue(surface.shadowTexts.size() >= 2, "baseline and zone title should render");
        assertTrue(surface.fillRects.size() >= 5, "zone panel, qi bar, banner, toast backgrounds should render");
        assertTrue(surface.drawTexts.size() >= 6, "zone panel text, event banner, and toast should render");
    }

    private static final class RecordingHudSurface implements BongHud.HudSurface {
        private final int width;
        private final int height;
        private final List<ShadowTextCall> shadowTexts = new ArrayList<>();
        private final List<FillRectCall> fillRects = new ArrayList<>();
        private final List<DrawTextCall> drawTexts = new ArrayList<>();

        private RecordingHudSurface(int width, int height) {
            this.width = width;
            this.height = height;
        }

        @Override
        public int windowWidth() {
            return width;
        }

        @Override
        public int windowHeight() {
            return height;
        }

        @Override
        public int measureText(String text) {
            return text.length() * 6;
        }

        @Override
        public void fill(int x1, int y1, int x2, int y2, int color) {
            fillRects.add(new FillRectCall(x1, y1, x2, y2, color));
        }

        @Override
        public void drawTextWithShadow(String text, int x, int y, int color) {
            shadowTexts.add(new ShadowTextCall(text, x, y, color));
        }

        @Override
        public void drawText(String text, int x, int y, int color, boolean shadow) {
            drawTexts.add(new DrawTextCall(text, x, y, color, shadow));
        }
    }

    private record ShadowTextCall(String text, int x, int y, int color) {
    }

    private record FillRectCall(int x1, int y1, int x2, int y2, int color) {
    }

    private record DrawTextCall(String text, int x, int y, int color, boolean shadow) {
    }
}
