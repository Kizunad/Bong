package com.bong.client.hud;

import com.bong.client.state.ZoneState;
import com.bong.client.tsy.ExtractState;

import java.util.ArrayList;
import java.util.List;

public final class HudEnvironmentVariantPlanner {
    static final int NEGATIVE_TINT = 0x229966CC;
    static final int DEAD_TINT = 0x55666666;
    static final int TSY_TINT = 0x18905CFF;
    static final int COLLAPSE_EDGE = 0x99FF3030;

    private HudEnvironmentVariantPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        HudEnvironmentVariant variant,
        ZoneState zoneState,
        ExtractState extractState,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        if (screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        ZoneState zone = zoneState == null ? ZoneState.empty() : zoneState;
        ExtractState extract = extractState == null ? ExtractState.empty() : extractState;
        HudEnvironmentVariant safeVariant = variant == null ? HudEnvironmentVariant.from(zone, extract) : variant;
        List<HudRenderCommand> out = new ArrayList<>();
        double boundary = HudEnvironmentVariant.boundaryLerp(zone);
        switch (safeVariant) {
            case NEGATIVE_QI -> {
                out.add(HudRenderCommand.screenTint(HudRenderLayer.HUD_VARIANT, withAlpha(NEGATIVE_TINT, 0.35 + boundary * 0.65)));
                out.add(HudRenderCommand.text(HudRenderLayer.HUD_VARIANT, "负灵域", 8, Math.max(18, screenHeight - 104), 0xCC9966CC));
            }
            case DEAD_ZONE -> {
                out.add(HudRenderCommand.screenTint(HudRenderLayer.HUD_VARIANT, withAlpha(DEAD_TINT, 0.45 + boundary * 0.55)));
                out.add(HudRenderCommand.text(HudRenderLayer.HUD_VARIANT, "死域", 8, Math.max(18, screenHeight - 104), 0xCCB8B8B8));
            }
            case TSY -> {
                out.add(HudRenderCommand.screenTint(HudRenderLayer.HUD_VARIANT, TSY_TINT));
                if (extract.collapseActive(nowMillis)) {
                    int alpha = (int) Math.round(96 + 80 * pulse(nowMillis, 600L));
                    out.add(HudRenderCommand.edgeVignette(HudRenderLayer.HUD_VARIANT, (alpha << 24) | (COLLAPSE_EDGE & 0x00FFFFFF)));
                }
            }
            case NORMAL -> {
            }
        }
        return List.copyOf(out);
    }

    static int jitterOffset(HudEnvironmentVariant variant, long nowMillis) {
        if (variant != HudEnvironmentVariant.TSY) {
            return 0;
        }
        long bucket = Math.max(0L, nowMillis) / 120L;
        return switch ((int) (bucket % 4L)) {
            case 0 -> -1;
            case 2 -> 1;
            default -> 0;
        };
    }

    private static int withAlpha(int color, double factor) {
        int alpha = (int) Math.round((color >>> 24) * Math.max(0.0, Math.min(1.0, factor)));
        return (alpha << 24) | (color & 0x00FFFFFF);
    }

    private static double pulse(long nowMillis, long periodMs) {
        double phase = (Math.max(0L, nowMillis) % periodMs) / (double) periodMs;
        return 0.5 * (1.0 - Math.cos(2.0 * Math.PI * phase));
    }
}
