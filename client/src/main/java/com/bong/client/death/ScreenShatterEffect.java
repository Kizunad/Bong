package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.hud.HudTextHelper;

import java.util.ArrayList;
import java.util.List;

public final class ScreenShatterEffect {
    private ScreenShatterEffect() {}

    public record Fragment(int x, int y, int width, int height, double rotation, double velocityX, double velocityY) {}

    public static List<Fragment> fragments(int width, int height, long seed) {
        if (width <= 0 || height <= 0) return List.of();
        List<Fragment> out = new ArrayList<>();
        int cols = 4;
        int rows = 4;
        int cellW = Math.max(1, width / cols);
        int cellH = Math.max(1, height / rows);
        for (int row = 0; row < rows; row++) {
            for (int col = 0; col < cols; col++) {
                long mixed = seed ^ (row * 31L + col * 131L);
                int jitterX = (int) Math.floorMod(mixed, Math.max(1, cellW / 5 + 1));
                int jitterY = (int) Math.floorMod(mixed >> 8, Math.max(1, cellH / 5 + 1));
                int x = Math.min(width - 1, col * cellW + jitterX / 2);
                int y = Math.min(height - 1, row * cellH + jitterY / 2);
                int w = Math.max(1, Math.min(cellW + jitterX, width - x));
                int h = Math.max(1, Math.min(cellH + jitterY, height - y));
                double dx = (col - 1.5) * 0.75;
                double dy = -0.3 + row * 0.35;
                out.add(new Fragment(x, y, w, h, (mixed & 31) - 15, dx, dy));
            }
        }
        return out;
    }

    public static List<HudRenderCommand> buildCommands(DeathCinematicState state, int width, int height) {
        if (state == null || !state.active() || width <= 0 || height <= 0) return List.of();
        double progress = state.phaseProgress();
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.screenTint(
            HudRenderLayer.VISUAL,
            HudTextHelper.withAlpha(0x000000, (int) (progress * 180))
        ));
        for (Fragment fragment : fragments(width, height, state.deathNumber())) {
            int x = (int) Math.round(fragment.x() + fragment.velocityX() * progress * 24.0);
            int y = (int) Math.round(fragment.y() + fragment.velocityY() * progress * 24.0);
            out.add(HudRenderCommand.rect(
                HudRenderLayer.VISUAL,
                x,
                y,
                fragment.width(),
                fragment.height(),
                HudTextHelper.withAlpha(0x0F0F10, 190 - (int) (progress * 150))
            ));
        }
        return out;
    }
}
