package com.bong.client.hud;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class PoisonTraitHudPlanner {
    static final int WIDTH = 142;
    static final int BG = 0xB0121715;
    static final int TRACK = 0xFF1B2923;
    static final int TEXT = 0xFFE4F1E8;
    static final int TOXICITY_FILL = 0xFF3EAD68;
    static final int DIGESTION_FILL = 0xFF8E6DD5;
    static final int WARNING = 0xFFE8C15C;

    private PoisonTraitHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        PoisonTraitHudStateStore.State state,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        PoisonTraitHudStateStore.State safe = state == null ? PoisonTraitHudStateStore.State.NONE : state;
        if (!safe.active() || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }

        int x = Math.max(8, screenWidth - WIDTH - 12);
        int y = Math.max(38, screenHeight / 2 - 54);
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.POISON_TRAIT, x, y, WIDTH, 42, BG));
        out.add(HudRenderCommand.text(
            HudRenderLayer.POISON_TRAIT,
            String.format(Locale.ROOT, "毒性 %.0f%% · %s", safe.toxicity(), safe.toxicityTierLabel()),
            x + 7,
            y + 6,
            TEXT
        ));
        appendBar(out, x + 7, y + 18, WIDTH - 14, safe.toxicityRatio(), TOXICITY_FILL);
        out.add(HudRenderCommand.text(
            HudRenderLayer.POISON_TRAIT,
            String.format(Locale.ROOT, "消化 %.0f/%.0f", safe.digestionCurrent(), safe.digestionCapacity()),
            x + 7,
            y + 26,
            digestionColor(safe)
        ));
        appendBar(out, x + 76, y + 30, WIDTH - 84, safe.digestionRatio(), DIGESTION_FILL);

        if (nowMillis < safe.lifespanWarningUntilMillis() && safe.lifespanYearsLost() > 0.0f) {
            out.add(HudRenderCommand.toast(
                HudRenderLayer.POISON_TRAIT,
                String.format(Locale.ROOT, "-%.1f 年寿元", safe.lifespanYearsLost()),
                Math.max(8, screenWidth / 2 - 42),
                Math.max(28, screenHeight / 2 - 36),
                WARNING
            ));
        }
        return List.copyOf(out);
    }

    private static int digestionColor(PoisonTraitHudStateStore.State state) {
        if (state.digestionRatio() >= 0.8f) {
            return WARNING;
        }
        return 0xFFC9D5CC;
    }

    private static void appendBar(List<HudRenderCommand> out, int x, int y, int width, double ratio, int fillColor) {
        out.add(HudRenderCommand.rect(HudRenderLayer.POISON_TRAIT, x, y, width, 4, TRACK));
        int fill = Math.max(0, Math.min(width, (int) Math.round(width * clamp01(ratio))));
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.POISON_TRAIT, x, y, fill, 4, fillColor));
        }
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) return 0.0;
        return Math.max(0.0, Math.min(1.0, value));
    }
}
