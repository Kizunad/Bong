package com.bong.client.hud;

import com.bong.client.gathering.GatheringSessionStore;
import com.bong.client.gathering.GatheringSessionViewModel;

import java.util.ArrayList;
import java.util.List;

public final class GatheringProgressHud {
    private static final long COMPLETE_FADE_MS = 1000L;
    private static final int RADIUS = 14;
    private static final int THICKNESS = 2;
    private static final int SIDE = RADIUS * 2;
    private static final int TRACK = 0xAA101820;
    private static final int ACTIVE = 0xFFEAF4FF;
    private static final int NEAR_DONE = 0xFF62E67A;
    private static final int FINE = 0xFF62E67A;
    private static final int PERFECT = 0xFFFFD35A;
    private static final int MUTED = 0xCCB8C5D6;

    private GatheringProgressHud() {
    }

    public static List<HudRenderCommand> buildCommands(
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        return buildCommands(GatheringSessionStore.snapshot(), widthMeasurer, screenWidth, screenHeight, nowMs);
    }

    static List<HudRenderCommand> buildCommands(
        GatheringSessionViewModel session,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (session == null || session.isEmpty() || widthMeasurer == null || screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }
        long age = Math.max(0L, nowMs - session.updatedAtMillis());
        if ((session.completed() || session.interrupted()) && age > COMPLETE_FADE_MS) {
            return out;
        }

        int cx = screenWidth / 2;
        int cy = screenHeight / 2;
        appendTrack(out, cx, cy);

        double progress = session.completed() ? 1.0 : session.progressRatio();
        int color = session.completed()
            ? (session.hasPerfectQualityHint() ? PERFECT : NEAR_DONE)
            : progress >= 0.85 ? NEAR_DONE : ACTIVE;
        appendProgress(out, cx, cy, progress, color);

        String label = HudTextHelper.clipToWidth(session.displayTargetName(), 96, widthMeasurer);
        if (!label.isEmpty()) {
            int x = cx - widthMeasurer.measure(label) / 2;
            out.add(HudRenderCommand.text(HudRenderLayer.GATHERING, label, x, cy - RADIUS - 14, MUTED));
        }

        String quality = session.qualityLabel();
        if (!quality.isEmpty() && (session.completed() || progress >= 0.75)) {
            int qColor = session.hasPerfectQualityHint() ? PERFECT : FINE;
            int x = cx - widthMeasurer.measure(quality) / 2;
            out.add(HudRenderCommand.text(HudRenderLayer.GATHERING, quality, x, cy + RADIUS + 6, qColor));
        }
        return List.copyOf(out);
    }

    private static void appendTrack(List<HudRenderCommand> out, int cx, int cy) {
        int x = cx - RADIUS;
        int y = cy - RADIUS;
        out.add(HudRenderCommand.rect(HudRenderLayer.GATHERING, x, y, SIDE, THICKNESS, TRACK));
        out.add(HudRenderCommand.rect(HudRenderLayer.GATHERING, x + SIDE - THICKNESS, y, THICKNESS, SIDE, TRACK));
        out.add(HudRenderCommand.rect(HudRenderLayer.GATHERING, x, y + SIDE - THICKNESS, SIDE, THICKNESS, TRACK));
        out.add(HudRenderCommand.rect(HudRenderLayer.GATHERING, x, y, THICKNESS, SIDE, TRACK));
    }

    private static void appendProgress(List<HudRenderCommand> out, int cx, int cy, double progress, int color) {
        int remaining = (int) Math.round(Math.max(0.0, Math.min(1.0, progress)) * SIDE * 4.0);
        int x = cx - RADIUS;
        int y = cy - RADIUS;
        remaining = appendSegment(out, remaining, x, y, SIDE, THICKNESS, color);
        remaining = appendSegment(out, remaining, x + SIDE - THICKNESS, y, THICKNESS, SIDE, color);
        remaining = appendReverseHorizontal(out, remaining, x, y + SIDE - THICKNESS, SIDE, THICKNESS, color);
        appendReverseVertical(out, remaining, x, y, THICKNESS, SIDE, color);
    }

    private static int appendSegment(List<HudRenderCommand> out, int remaining, int x, int y, int w, int h, int color) {
        if (remaining <= 0) {
            return 0;
        }
        int length = Math.min(remaining, Math.max(w, h));
        out.add(HudRenderCommand.rect(HudRenderLayer.GATHERING, x, y, w >= h ? length : w, h > w ? length : h, color));
        return remaining - length;
    }

    private static int appendReverseHorizontal(List<HudRenderCommand> out, int remaining, int x, int y, int w, int h, int color) {
        if (remaining <= 0) {
            return 0;
        }
        int length = Math.min(remaining, w);
        out.add(HudRenderCommand.rect(HudRenderLayer.GATHERING, x + w - length, y, length, h, color));
        return remaining - length;
    }

    private static int appendReverseVertical(List<HudRenderCommand> out, int remaining, int x, int y, int w, int h, int color) {
        if (remaining <= 0) {
            return 0;
        }
        int length = Math.min(remaining, h);
        out.add(HudRenderCommand.rect(HudRenderLayer.GATHERING, x, y + h - length, w, length, color));
        return remaining - length;
    }
}
