package com.bong.client.hud;

import com.bong.client.state.ZoneState;
import com.bong.client.tsy.ExtractState;
import com.bong.client.tsy.RiftPortalView;

import java.util.ArrayList;
import java.util.List;

public final class DirectionalCompassHudPlanner {
    static final int WIDTH = 240;
    static final int HEIGHT = 24;
    static final int TRACK = 0xAA101820;
    static final int TICK = 0xCCB8D8E8;
    static final int TEXT = 0x99E6F3FF;
    static final int FLASH_TEXT = 0xDDE6F3FF;
    static final int NICHE_MARKER = 0xFF60A8FF;
    static final int TSY_MARKER = 0xFF9966CC;
    static final int COLLAPSE_EXIT_MARKER = 0xFF70FF80;

    private DirectionalCompassHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        ZoneState zoneState,
        ExtractState extractState,
        HudImmersionMode.Mode mode,
        HudRuntimeContext runtimeContext,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        long nowMillis
    ) {
        if (mode == HudImmersionMode.Mode.CULTIVATION || screenWidth <= 0 || screenHeight <= 0 || widthMeasurer == null) {
            return List.of();
        }
        ZoneState zone = zoneState == null ? ZoneState.empty() : zoneState;
        HudRuntimeContext runtime = runtimeContext == null ? HudRuntimeContext.empty() : runtimeContext;
        int x = Math.max(8, (screenWidth - WIDTH) / 2);
        int y = 8;
        List<HudRenderCommand> out = new ArrayList<>();
        out.add(HudRenderCommand.rect(HudRenderLayer.COMPASS, x, y + 10, WIDTH, 2, TRACK));
        appendTicks(out, x, y, runtime.yawDegrees());
        appendMarkers(out, x, y, runtime, markers(runtime, extractState, nowMillis));
        String label = zoneLabel(zone);
        if (!label.isEmpty()) {
            String clipped = HudTextHelper.clipToWidth(label, WIDTH, widthMeasurer);
            int textX = x + (WIDTH - widthMeasurer.measure(clipped)) / 2;
            out.add(HudRenderCommand.text(
                HudRenderLayer.COMPASS,
                clipped,
                textX,
                y + 16,
                zoneFlashColor(zone, nowMillis)
            ));
        }
        return List.copyOf(out);
    }

    private static void appendTicks(List<HudRenderCommand> out, int x, int y, double yawDegrees) {
        int center = x + WIDTH / 2;
        for (int offset = -90; offset <= 90; offset += 30) {
            int px = center + (int) Math.round(offset / 180.0 * WIDTH);
            int height = offset % 90 == 0 ? 9 : 5;
            out.add(HudRenderCommand.rect(HudRenderLayer.COMPASS, px, y + 7, 1, height, TICK));
            if (offset % 90 == 0) {
                out.add(HudRenderCommand.text(HudRenderLayer.COMPASS, cardinal(yawDegrees + offset), px - 3, y, TEXT));
            }
        }
        out.add(HudRenderCommand.rect(HudRenderLayer.COMPASS, center - 1, y + 5, 3, 12, 0xFFE6F3FF));
    }

    private static void appendMarkers(
        List<HudRenderCommand> out,
        int x,
        int y,
        HudRuntimeContext runtime,
        List<HudRuntimeContext.CompassMarker> markers
    ) {
        int center = x + WIDTH / 2;
        for (HudRuntimeContext.CompassMarker marker : markers) {
            double bearing = bearingDegrees(runtime.playerX(), runtime.playerZ(), marker.worldX(), marker.worldZ());
            double delta = signedDelta(bearing, runtime.yawDegrees());
            if (Math.abs(delta) > 90.0) {
                continue;
            }
            int px = center + (int) Math.round(delta / 180.0 * WIDTH);
            int color = markerColor(marker.kind(), marker.intensity());
            if (marker.kind() == HudRuntimeContext.CompassMarker.Kind.COLLAPSE_EXIT) {
                out.add(HudRenderCommand.rect(HudRenderLayer.COMPASS, px - 4, y + 4, 8, 4, color));
                out.add(HudRenderCommand.rect(HudRenderLayer.COMPASS, px - 1, y + 1, 2, 9, color));
            } else {
                out.add(HudRenderCommand.rect(HudRenderLayer.COMPASS, px - 3, y + 4, 6, 6, color));
            }
        }
    }

    static List<HudRuntimeContext.CompassMarker> markers(HudRuntimeContext runtimeContext, ExtractState extractState, long nowMillis) {
        List<HudRuntimeContext.CompassMarker> markers = new ArrayList<>(
            runtimeContext == null ? List.of() : runtimeContext.compassMarkers()
        );
        ExtractState extract = extractState == null ? ExtractState.empty() : extractState;
        boolean collapseActive = extract.collapseActive(Math.max(0L, nowMillis));
        for (RiftPortalView portal : extract.portals()) {
            if (!"exit".equals(portal.direction())) {
                continue;
            }
            HudRuntimeContext.CompassMarker.Kind kind = collapseActive && "collapse_tear".equals(portal.kind())
                ? HudRuntimeContext.CompassMarker.Kind.COLLAPSE_EXIT
                : HudRuntimeContext.CompassMarker.Kind.TSY_PORTAL;
            markers.add(new HudRuntimeContext.CompassMarker(kind, portal.x(), portal.z(), 1.0));
            break;
        }
        return List.copyOf(markers);
    }

    private static int markerColor(HudRuntimeContext.CompassMarker.Kind kind, double intensity) {
        int base = switch (kind) {
            case SPIRIT_NICHE -> NICHE_MARKER;
            case COLLAPSE_EXIT -> COLLAPSE_EXIT_MARKER;
            case TSY_PORTAL -> TSY_MARKER;
        };
        int alpha = (int) Math.round(96 + Math.max(0.0, Math.min(1.0, intensity)) * 159.0);
        return (alpha << 24) | (base & 0x00FFFFFF);
    }

    private static String zoneLabel(ZoneState zone) {
        if (zone == null || zone.isEmpty()) {
            return "";
        }
        if (zone.collapsed()) {
            return "死域·" + zone.zoneLabel();
        }
        return zone.zoneLabel();
    }

    private static int zoneFlashColor(ZoneState zone, long nowMillis) {
        if (zone == null || zone.changedAtMillis() <= 0L) {
            return TEXT;
        }
        long elapsed = Math.max(0L, nowMillis - zone.changedAtMillis());
        return elapsed <= 1_000L ? FLASH_TEXT : TEXT;
    }

    private static String cardinal(double degrees) {
        double normalized = HudRuntimeContext.normalizeDegrees(degrees);
        if (normalized >= 315.0 || normalized < 45.0) return "N";
        if (normalized < 135.0) return "E";
        if (normalized < 225.0) return "S";
        return "W";
    }

    static double bearingDegrees(double fromX, double fromZ, double toX, double toZ) {
        double degrees = Math.toDegrees(Math.atan2(toX - fromX, toZ - fromZ));
        return HudRuntimeContext.normalizeDegrees(degrees);
    }

    static double signedDelta(double targetDegrees, double yawDegrees) {
        double delta = HudRuntimeContext.normalizeDegrees(targetDegrees) - HudRuntimeContext.normalizeDegrees(yawDegrees);
        if (delta > 180.0) delta -= 360.0;
        if (delta < -180.0) delta += 360.0;
        return delta;
    }
}
