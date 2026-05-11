package com.bong.client.hud;

import com.bong.client.omen.OmenStateStore;

import java.util.ArrayList;
import java.util.List;

public final class OmenHudPlanner {
    private OmenHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        OmenStateStore.Snapshot snapshot,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        if (snapshot == null || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        List<HudRenderCommand> out = new ArrayList<>();
        for (OmenStateStore.Entry entry : snapshot.entries()) {
            appendEntry(out, entry, nowMillis, screenWidth, screenHeight);
        }
        return out;
    }

    private static void appendEntry(
        List<HudRenderCommand> out,
        OmenStateStore.Entry entry,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        double pulse = 0.55 + 0.45 * Math.sin(nowMillis / pulsePeriod(entry.kind()));
        double strength = Math.max(0.0, Math.min(1.0, entry.strength() * pulse));
        switch (entry.kind()) {
            case PSEUDO_VEIN -> out.add(HudRenderCommand.edgeVignette(
                HudRenderLayer.VISUAL,
                withAlpha(0x66D8C8, 45 + (int) Math.round(80.0 * strength))
            ));
            case BEAST_TIDE -> {
                int alpha = 30 + (int) Math.round(70.0 * strength);
                out.add(HudRenderCommand.rect(
                    HudRenderLayer.VISUAL,
                    0,
                    Math.max(0, screenHeight - 18),
                    screenWidth,
                    18,
                    withAlpha(0xB8864A, alpha)
                ));
            }
            case REALM_COLLAPSE -> {
                out.add(HudRenderCommand.edgeVignette(
                    HudRenderLayer.VISUAL,
                    withAlpha(0x7A1E24, 70 + (int) Math.round(110.0 * strength))
                ));
                out.add(HudRenderCommand.screenTint(
                    HudRenderLayer.VISUAL,
                    withAlpha(0x100608, 18 + (int) Math.round(38.0 * strength))
                ));
            }
            case KARMA_BACKLASH -> out.add(HudRenderCommand.edgeVignette(
                HudRenderLayer.VISUAL,
                withAlpha(0xA01830, 80 + (int) Math.round(120.0 * strength))
            ));
        }
    }

    private static double pulsePeriod(OmenStateStore.Kind kind) {
        return switch (kind) {
            case PSEUDO_VEIN -> 900.0;
            case BEAST_TIDE -> 600.0;
            case REALM_COLLAPSE -> 450.0;
            case KARMA_BACKLASH -> 240.0;
        };
    }

    private static int withAlpha(int rgb, int alpha) {
        int clamped = Math.max(0, Math.min(255, alpha));
        return (clamped << 24) | (rgb & 0x00FFFFFF);
    }
}
