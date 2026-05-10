package com.bong.client.hud;

import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.ZoneState;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.SenseKind;

import java.util.ArrayList;
import java.util.List;

public final class QiDensityRadarHudPlanner {
    static final int RADIUS = 24;
    static final int PANEL = RADIUS * 2 + 10;
    static final int GOLD_QI = 0xFFE0C060;
    static final int MID_QI = 0xFFC8F4FF;
    static final int LOW_QI = 0xFF888888;
    static final int NEGATIVE_QI = 0xFF9966CC;
    static final int CULTIVATOR_DOT = 0xFFFFFFFF;

    private static final int[][] DIRECTIONS = {
        {0, -1}, {1, -1}, {1, 0}, {1, 1}, {0, 1}, {-1, 1}, {-1, 0}, {-1, -1}
    };

    private QiDensityRadarHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        PlayerStateViewModel playerState,
        ZoneState zoneState,
        PerceptionEdgeState perceptionState,
        HudImmersionMode.Mode mode,
        HudEnvironmentVariant variant,
        HudRuntimeContext runtimeContext,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        PlayerStateViewModel player = playerState == null ? PlayerStateViewModel.empty() : playerState;
        ZoneState zone = zoneState == null ? ZoneState.empty() : zoneState;
        HudRuntimeContext runtime = runtimeContext == null ? HudRuntimeContext.empty() : runtimeContext;
        if (!HudRealmGate.atLeastCondense(player.realm()) || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        int anchorX = MiniBodyHudPlanner.MARGIN_X + MiniBodyHudPlanner.PANEL_W + 8;
        int anchorY = screenHeight - PANEL - MiniBodyHudPlanner.MARGIN_Y;
        int jitter = HudEnvironmentVariantPlanner.jitterOffset(variant, nowMillis);
        anchorX += jitter;
        anchorY -= jitter;

        double baseQi = zone.negativeSpiritQi() ? -Math.abs(zone.spiritQiRaw()) : zone.spiritQiNormalized();
        double alphaFactor = mode == HudImmersionMode.Mode.PEACE ? 0.3 : 1.0;
        if (mode == HudImmersionMode.Mode.CULTIVATION) {
            alphaFactor = 1.0;
        }

        List<HudRenderCommand> out = new ArrayList<>();
        appendFrame(out, anchorX, anchorY, alphaFactor);
        int centerX = anchorX + PANEL / 2;
        int centerY = anchorY + PANEL / 2;
        for (int i = 0; i < DIRECTIONS.length; i++) {
            appendQiMarker(out, centerX, centerY, i, baseQi, alphaFactor);
        }
        appendCultivatorDots(out, perceptionState, runtime, centerX, centerY, alphaFactor);
        if (variant == HudEnvironmentVariant.TSY) {
            appendTsyFalseMarker(out, centerX, centerY, nowMillis, alphaFactor);
        }
        return List.copyOf(out);
    }

    private static void appendFrame(List<HudRenderCommand> out, int x, int y, double alpha) {
        int frame = scaleAlpha(0x88405058, alpha);
        out.add(HudRenderCommand.rect(HudRenderLayer.QI_RADAR, x, y, PANEL, 1, frame));
        out.add(HudRenderCommand.rect(HudRenderLayer.QI_RADAR, x, y + PANEL - 1, PANEL, 1, frame));
        out.add(HudRenderCommand.rect(HudRenderLayer.QI_RADAR, x, y, 1, PANEL, frame));
        out.add(HudRenderCommand.rect(HudRenderLayer.QI_RADAR, x + PANEL - 1, y, 1, PANEL, frame));
        out.add(HudRenderCommand.rect(HudRenderLayer.QI_RADAR, x + PANEL / 2 - 1, y + PANEL / 2 - 1, 3, 3, scaleAlpha(0xCCB8D8E8, alpha)));
    }

    private static void appendQiMarker(List<HudRenderCommand> out, int centerX, int centerY, int directionIndex, double baseQi, double alpha) {
        int[] dir = DIRECTIONS[directionIndex];
        boolean negative = baseQi < 0.0;
        double qi = Math.max(0.0, Math.min(1.0, Math.abs(baseQi)));
        int len = negative ? 9 : (int) Math.round(6 + qi * 12);
        int color = colorFor(baseQi);
        int sign = negative ? -1 : 1;
        int x = centerX + dir[0] * sign * (RADIUS - len);
        int y = centerY + dir[1] * sign * (RADIUS - len);
        int w = Math.max(2, Math.abs(dir[0]) == 1 ? len : 2);
        int h = Math.max(2, Math.abs(dir[1]) == 1 ? len : 2);
        if (dir[0] < 0 && !negative || dir[0] > 0 && negative) {
            x -= w;
        }
        if (dir[1] < 0 && !negative || dir[1] > 0 && negative) {
            y -= h;
        }
        out.add(HudRenderCommand.rect(HudRenderLayer.QI_RADAR, x, y, w, h, scaleAlpha(color, alpha)));
    }

    static int colorFor(double baseQi) {
        if (baseQi < 0.0) {
            return NEGATIVE_QI;
        }
        if (baseQi >= 0.66) {
            return GOLD_QI;
        }
        if (baseQi <= 0.15) {
            return LOW_QI;
        }
        return MID_QI;
    }

    private static void appendCultivatorDots(
        List<HudRenderCommand> out,
        PerceptionEdgeState perceptionState,
        HudRuntimeContext runtime,
        int centerX,
        int centerY,
        double alpha
    ) {
        PerceptionEdgeState state = perceptionState == null ? PerceptionEdgeState.empty() : perceptionState;
        int rendered = 0;
        for (PerceptionEdgeState.SenseEntry entry : state.entries()) {
            if (rendered >= 4) break;
            if (entry.kind() != SenseKind.CULTIVATOR_REALM && entry.kind() != SenseKind.LIVING_QI) {
                continue;
            }
            double dx = entry.x() - runtime.playerX();
            double dz = entry.z() - runtime.playerZ();
            double distanceSq = dx * dx + dz * dz;
            if (distanceSq > 64.0 || distanceSq <= 0.0001) {
                continue;
            }
            double bearing = DirectionalCompassHudPlanner.bearingDegrees(
                runtime.playerX(), runtime.playerZ(), entry.x(), entry.z()
            );
            double delta = DirectionalCompassHudPlanner.signedDelta(bearing, runtime.yawDegrees());
            double radians = Math.toRadians(delta);
            int x = centerX + (int) Math.round(Math.sin(radians) * (RADIUS - 6));
            int y = centerY - (int) Math.round(Math.cos(radians) * (RADIUS - 6));
            out.add(HudRenderCommand.rect(HudRenderLayer.QI_RADAR, x - 1, y - 1, 2, 2, scaleAlpha(CULTIVATOR_DOT, alpha)));
            rendered++;
        }
    }

    private static void appendTsyFalseMarker(List<HudRenderCommand> out, int centerX, int centerY, long nowMillis, double alpha) {
        long bucket = Math.max(0L, nowMillis) / 200L;
        int dirIndex = (int) (bucket % DIRECTIONS.length);
        int[] dir = DIRECTIONS[dirIndex];
        int x = centerX + dir[0] * (RADIUS - 4);
        int y = centerY + dir[1] * (RADIUS - 4);
        out.add(HudRenderCommand.rect(HudRenderLayer.QI_RADAR, x - 2, y - 2, 4, 4, scaleAlpha(0x88F8F6FF, alpha)));
    }

    private static int scaleAlpha(int color, double factor) {
        int baseAlpha = (color >>> 24) == 0 ? 0xFF : (color >>> 24);
        int alpha = (int) Math.round(baseAlpha * Math.max(0.0, Math.min(1.0, factor)));
        return (alpha << 24) | (color & 0x00FFFFFF);
    }
}
