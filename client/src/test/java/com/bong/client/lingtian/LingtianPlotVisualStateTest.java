package com.bong.client.lingtian;

import com.bong.client.BongHud;
import com.bong.client.lingtian.state.LingtianSessionStore;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class LingtianPlotVisualStateTest {
    @Test
    void plot_rune_decal_updates() {
        LingtianSessionStore.Snapshot planted = snapshot(
            LingtianSessionStore.Kind.PLANTING,
            20,
            40,
            "ci_she_hao",
            0.25f,
            false
        );
        LingtianSessionStore.Snapshot drained = snapshot(
            LingtianSessionStore.Kind.DRAIN_QI,
            32,
            40,
            "ci_she_hao",
            0.62f,
            true
        );

        LingtianPlotVisualState plantedState = LingtianPlotVisualState.fromSnapshot(planted);
        LingtianPlotVisualState drainedState = LingtianPlotVisualState.fromSnapshot(drained);

        assertEquals("种", plantedState.icon());
        assertEquals(LingtianPlotVisualState.PLANTED_RUNE, plantedState.runeColor());
        assertEquals(0.5f, plantedState.progress(), 1e-6);
        assertEquals("吸", drainedState.icon());
        assertEquals(LingtianPlotVisualState.DEPLETED_RUNE, drainedState.runeColor());
        assertTrue(drainedState.detail().contains("染污 62%"));
        assertTrue(drainedState.detail().endsWith("!"));
    }

    @Test
    void proximity_overlay_shows() {
        RecordingSurface surface = new RecordingSurface(320, 180);
        LingtianPlotVisualState state = LingtianPlotVisualState.fromSnapshot(snapshot(
            LingtianSessionStore.Kind.HARVEST,
            30,
            40,
            "ning_mai_cao",
            0.0f,
            false
        ));

        LingtianSessionHud.renderPlotOverlay(surface, state);

        assertTrue(surface.fillRects.size() >= 8, "mini panel should draw background, border, icon, and progress bar");
        assertTrue(surface.drawTexts.stream().anyMatch(call -> call.text().contains("收")));
        assertTrue(surface.drawTexts.stream().anyMatch(call -> call.text().contains("收获")));
        assertTrue(surface.drawTexts.stream().anyMatch(call -> call.text().contains("75%")));
    }

    private static LingtianSessionStore.Snapshot snapshot(
        LingtianSessionStore.Kind kind,
        int elapsed,
        int target,
        String plantId,
        float dye,
        boolean warning
    ) {
        return new LingtianSessionStore.Snapshot(
            true,
            kind,
            1,
            64,
            2,
            elapsed,
            target,
            plantId,
            "manual",
            dye,
            warning
        );
    }

    private static final class RecordingSurface implements BongHud.HudSurface {
        private final int width;
        private final int height;
        private final List<FillRectCall> fillRects = new ArrayList<>();
        private final List<DrawTextCall> drawTexts = new ArrayList<>();

        private RecordingSurface(int width, int height) {
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
            drawTexts.add(new DrawTextCall(text, x, y, color, true));
        }

        @Override
        public void drawText(String text, int x, int y, int color, boolean shadow) {
            drawTexts.add(new DrawTextCall(text, x, y, color, shadow));
        }
    }

    private record FillRectCall(int x1, int y1, int x2, int y2, int color) {}

    private record DrawTextCall(String text, int x, int y, int color, boolean shadow) {}
}
