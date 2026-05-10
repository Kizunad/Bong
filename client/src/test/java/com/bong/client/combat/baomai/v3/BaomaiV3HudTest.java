package com.bong.client.combat.baomai.v3;

import com.bong.client.BongHud;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

public class BaomaiV3HudTest {
    @AfterEach
    void tearDown() {
        BaomaiV3HudStateStore.clear();
    }

    @Test
    void rendersBloodBurnTranscendenceAndScarHudRows() {
        BaomaiV3HudStateStore.recordBloodBurn(200, 1_000L);
        BaomaiV3HudStateStore.recordBodyTranscendence(100, 10.0, 1_000L);
        BaomaiV3HudStateStore.recordMeridianRippleScar(0.45, 1_000L);
        RecordingHudSurface surface = new RecordingHudSurface(320, 180);

        BaomaiV3Hud.render(surface, 1_500L);

        assertTrue(surface.texts.stream().anyMatch(text -> text.contains("焚血")));
        assertTrue(surface.texts.stream().anyMatch(text -> text.contains("凡躯重铸 x10")));
        assertTrue(surface.texts.stream().anyMatch(text -> text.contains("经脉龟裂 45%")));
        assertTrue(surface.fillCount >= 6);
    }

    @Test
    void expiredRowsDoNotRender() {
        BaomaiV3HudStateStore.recordBloodBurn(1, 1_000L);
        RecordingHudSurface surface = new RecordingHudSurface(320, 180);

        BaomaiV3Hud.render(surface, 2_000L);

        assertTrue(surface.texts.isEmpty());
    }

    private static final class RecordingHudSurface implements BongHud.HudSurface {
        private final int width;
        private final int height;
        private final List<String> texts = new ArrayList<>();
        private int fillCount;

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
            fillCount++;
        }

        @Override
        public void drawTextWithShadow(String text, int x, int y, int color) {
            texts.add(text);
        }

        @Override
        public void drawText(String text, int x, int y, int color, boolean shadow) {
            texts.add(text);
        }
    }
}
