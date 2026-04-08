package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongHudTest {
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
    public void emptyStateStillRendersBaselineWithoutToast() {
        BongHud.HudSnapshot snapshot = BongHud.snapshot(1_000L);
        RecordingHudSurface surface = new RecordingHudSurface(320, 180);

        assertEquals("Bong Client Connected", snapshot.baselineText());
        assertNull(snapshot.toast());
        assertDoesNotThrow(() -> BongHud.renderSurface(surface, snapshot));

        assertEquals(1, surface.shadowTexts.size());
        assertEquals("Bong Client Connected", surface.shadowTexts.get(0).text());
        assertEquals(10, surface.shadowTexts.get(0).x());
        assertEquals(10, surface.shadowTexts.get(0).y());
        assertTrue(surface.fillRects.isEmpty());
        assertTrue(surface.drawTexts.isEmpty());
    }

    @Test
    public void toastStateRendersCenteredOverlay() {
        NarrationState.recordNarration(
                new BongServerPayload.Narration("broadcast", "雷劫将至，速避高处。", "system_warning"),
                1_000L,
                ignored -> {
                }
        );

        BongHud.HudSnapshot snapshot = BongHud.snapshot(2_000L);
        RecordingHudSurface surface = new RecordingHudSurface(200, 100);

        assertDoesNotThrow(() -> BongHud.renderSurface(surface, snapshot));

        assertEquals(1, surface.shadowTexts.size());
        assertEquals(1, surface.fillRects.size());
        assertEquals(1, surface.drawTexts.size());
        assertEquals(snapshot.toast().text(), surface.drawTexts.get(0).text());
        assertEquals(0xFF5555, surface.drawTexts.get(0).color());
        assertTrue(surface.drawTexts.get(0).shadow());

        int expectedWidth = surface.measureText(snapshot.toast().text());
        assertEquals((200 - expectedWidth) / 2, surface.drawTexts.get(0).x());
        assertEquals(25, surface.drawTexts.get(0).y());
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
