package com.bong.client.hud;

import com.bong.client.gathering.GatheringPresentation;
import com.bong.client.gathering.GatheringSessionViewModel;

import java.util.ArrayList;
import java.util.List;

/** 采集碎弧：中心留空，材质图标与文字位于环下，终态只由服务端触发。 */
public final class GatheringProgressHud {
    static final List<String> SEGMENT_KEYS = List.of(
        "segment-a", "segment-b", "segment-c", "segment-d",
        "segment-e", "segment-f", "segment-g", "segment-h"
    );
    private static final int SIZE = 64;
    private static final String TEXTURE_ROOT = "bong-client:textures/hud/gathering/";

    private GatheringProgressHud() {}

    static List<HudRenderCommand> buildCommands(
        GatheringSessionViewModel session, HudTextHelper.WidthMeasurer measurer, int width, int height, long nowMs
    ) {
        return session == null ? List.of() : buildCommands(GatheringPresentation.of(session), measurer, width, height, nowMs);
    }

    public static List<HudRenderCommand> buildCommands(
        GatheringPresentation frame, HudTextHelper.WidthMeasurer measurer, int width, int height, long nowMs
    ) {
        if (measurer == null || width <= 0 || height <= 0 || !frame.visible(nowMs)) return List.of();
        GatheringSessionViewModel session = frame.session();
        boolean interrupted = session.interrupted();
        boolean completed = session.completed() && !interrupted;
        double end = Math.min(1.0, (double) frame.age(nowMs) / GatheringPresentation.EXIT_MS);
        double opacity = session.active() ? 1.0 : 1.0 - end * end;
        double progress = frame.progress(nowMs);
        String type = switch (session.targetType()) {
            case "ore" -> "ore";
            case "wood" -> "wood";
            default -> "herb";
        };
        int tint = interrupted ? 0xC98E7C : switch (type) {
            case "ore" -> 0xC3D3D9;
            case "wood" -> 0xDDB580;
            default -> 0xB9D4A7;
        };
        int cx = width / 2;
        int cy = height / 2;
        List<HudRenderCommand> out = new ArrayList<>();
        svg(out, "track", cx, cy, SIZE, argb(0xC7BDA3, .48 * opacity));
        for (int i = 0; i < SEGMENT_KEYS.size(); i++) {
            double lit = Math.max(0.0, Math.min(1.0, progress * SEGMENT_KEYS.size() - i));
            double spread = interrupted ? end * 13.0 : (1.0 - lit) * 3.0;
            double angle = Math.toRadians(-67.5 + i * 45.0);
            int x = cx + (int) Math.round(Math.cos(angle) * spread);
            int y = cy + (int) Math.round(Math.sin(angle) * spread);
            // 深色细衬保证草地与天空上都能读出弧段，不铺背景面板。
            svg(out, SEGMENT_KEYS.get(i), x, y + 1, SIZE, argb(0x111916, .7 * opacity));
            svg(out, SEGMENT_KEYS.get(i), x, y, SIZE, argb(tint, (.15 + .85 * lit) * opacity));
            if (completed && end < .3) {
                svg(out, SEGMENT_KEYS.get(i), x, y, SIZE, argb(0xFFF1CE, (1.0 - end / .3) * .8));
            }
        }
        if (completed) {
            svg(out, "track", cx, cy, SIZE + (int) Math.round(end * 22), argb(tint, (1.0 - end) * .6));
        }

        String target = HudTextHelper.clipToWidth(session.displayTargetName(), Math.min(96, width - 34), measurer);
        String detail = interrupted ? "已中断" : completed ? "采集完成" : "";
        String quality = session.qualityLabel();
        if (!interrupted && !quality.isEmpty() && (completed || progress >= .75)) {
            detail = detail.isEmpty() ? quality : detail + " · " + quality;
        }
        int labelWidth = Math.max(measurer.measure(target), measurer.measure(detail));
        int left = cx - (labelWidth + 28) / 2;
        out.add(HudRenderCommand.texture(HudRenderLayer.GATHERING, TEXTURE_ROOT + type + ".png",
            left, cy + 33, 24, 24, argb(0xFFFFFF, opacity)));
        out.add(HudRenderCommand.text(HudRenderLayer.GATHERING, target, left + 28,
            cy + (detail.isEmpty() ? 40 : 35), argb(0xEEE8D7, opacity)));
        if (!detail.isEmpty()) {
            out.add(HudRenderCommand.text(HudRenderLayer.GATHERING, detail, left + 28, cy + 47, argb(tint, .9 * opacity)));
        }
        return List.copyOf(out);
    }

    private static void svg(List<HudRenderCommand> out, String key, int cx, int cy, int size, int color) {
        out.add(HudRenderCommand.svg(HudRenderLayer.GATHERING, key, cx - size / 2, cy - size / 2, size, size, color));
    }

    private static int argb(int rgb, double alpha) {
        return (int) Math.round(Math.max(0.0, Math.min(1.0, alpha)) * 255) << 24 | rgb;
    }
}
