package com.bong.client.hud;

import com.bong.client.combat.store.TribulationStateStore;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.visual.realm_vision.PerceptionEdgeState;
import com.bong.client.visual.realm_vision.SenseKind;

import java.util.ArrayList;
import java.util.List;

public final class ThreatIndicatorHudPlanner {
    static final int LOW = 0x1A60FF80;
    static final int MEDIUM = 0x33FFD166;
    static final int HIGH = 0x55FF4040;
    static final int EXTREME = 0x88FF2020;
    static final int TRIBULATION = 0x889966FF;
    static final int ATTENTION_BG = 0x66101820;
    static final int ATTENTION_FILL = 0xCCFF4040;
    static final int THICKNESS = 2;

    private ThreatIndicatorHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        PlayerStateViewModel playerState,
        PerceptionEdgeState perceptionState,
        TribulationStateStore.State tribulationState,
        HudRuntimeContext runtimeContext,
        long nowMillis,
        int screenWidth,
        int screenHeight
    ) {
        PlayerStateViewModel player = playerState == null ? PlayerStateViewModel.empty() : playerState;
        if (!HudRealmGate.atLeastSpirit(player.realm()) || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        HudRuntimeContext runtime = runtimeContext == null ? HudRuntimeContext.empty() : runtimeContext;
        PerceptionEdgeState perception = perceptionState == null ? PerceptionEdgeState.empty() : perceptionState;
        ThreatByEdge threat = aggregate(perception, runtime);
        List<HudRenderCommand> out = new ArrayList<>();
        appendEdge(out, Edge.TOP, threat.top, nowMillis, screenWidth, screenHeight);
        appendEdge(out, Edge.RIGHT, threat.right, nowMillis, screenWidth, screenHeight);
        appendEdge(out, Edge.BOTTOM, threat.bottom, nowMillis, screenWidth, screenHeight);
        appendEdge(out, Edge.LEFT, threat.left, nowMillis, screenWidth, screenHeight);

        if (threat.lockWarning) {
            appendFullEdgeFlash(out, screenWidth, screenHeight, pulseColor(EXTREME, nowMillis, 300L, 0.65, 1.0));
        }
        if (tribulationState != null && tribulationState.active()) {
            out.add(HudRenderCommand.edgeVignette(HudRenderLayer.THREAT_INDICATOR, TRIBULATION));
        }
        if (HudRealmGate.atLeastVoid(player.realm())) {
            appendAttention(out, Math.max(threat.max(), tribulationState != null && tribulationState.active() ? 0.65 : 0.0), screenWidth, screenHeight);
        }
        return List.copyOf(out);
    }

    static long pulsePeriodMs(double distanceBlocks) {
        double clamped = Math.max(0.0, Math.min(64.0, Double.isFinite(distanceBlocks) ? distanceBlocks : 64.0));
        return Math.round(300.0 + (clamped / 64.0) * 700.0);
    }

    private static ThreatByEdge aggregate(PerceptionEdgeState state, HudRuntimeContext runtime) {
        ThreatByEdge threat = new ThreatByEdge();
        for (PerceptionEdgeState.SenseEntry entry : state.entries()) {
            if (!isThreat(entry.kind())) {
                continue;
            }
            double dx = entry.x() - runtime.playerX();
            double dz = entry.z() - runtime.playerZ();
            double distance = Math.sqrt(dx * dx + dz * dz);
            double intensity = Math.max(entry.intensity(), Math.max(0.0, 1.0 - distance / 64.0));
            Edge edge = edgeFor(dx, dz, runtime.yawDegrees());
            threat.add(edge, intensity, distance);
            if (entry.intensity() >= 0.9 && distance <= 8.0) {
                threat.lockWarning = true;
            }
        }
        return threat;
    }

    private static boolean isThreat(SenseKind kind) {
        return switch (kind == null ? SenseKind.LIVING_QI : kind) {
            case CRISIS_PREMONITION, HEAVENLY_GAZE, CULTIVATOR_REALM, ZHENFA_WARD_ALERT, NICHE_INTRUSION_TRACE -> true;
            default -> false;
        };
    }

    private static Edge edgeFor(double dx, double dz, double yawDegrees) {
        double bearing = DirectionalCompassHudPlanner.bearingDegrees(0.0, 0.0, dx, dz);
        double delta = DirectionalCompassHudPlanner.signedDelta(bearing, yawDegrees);
        if (delta >= -45.0 && delta <= 45.0) return Edge.TOP;
        if (delta > 45.0 && delta < 135.0) return Edge.RIGHT;
        if (delta < -45.0 && delta > -135.0) return Edge.LEFT;
        return Edge.BOTTOM;
    }

    private static void appendEdge(List<HudRenderCommand> out, Edge edge, ThreatLevel level, long nowMillis, int w, int h) {
        if (level.intensity <= 0.0) {
            return;
        }
        int color = colorFor(level.intensity, nowMillis, level.distance);
        switch (edge) {
            case TOP -> out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, 0, 0, w, THICKNESS, color));
            case RIGHT -> out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, w - THICKNESS, 0, THICKNESS, h, color));
            case BOTTOM -> out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, 0, h - THICKNESS, w, THICKNESS, color));
            case LEFT -> out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, 0, 0, THICKNESS, h, color));
        }
    }

    private static int colorFor(double intensity, long nowMillis, double distance) {
        int base;
        if (intensity >= 0.85) {
            base = EXTREME;
        } else if (intensity >= 0.55) {
            base = HIGH;
        } else if (intensity >= 0.25) {
            base = MEDIUM;
        } else {
            base = LOW;
        }
        return pulseColor(base, nowMillis, pulsePeriodMs(distance), 0.65, 1.0);
    }

    private static int pulseColor(int color, long nowMillis, long periodMs, double min, double max) {
        double phase = (Math.max(0L, nowMillis) % Math.max(1L, periodMs)) / (double) Math.max(1L, periodMs);
        double wave = 0.5 * (1.0 - Math.cos(2.0 * Math.PI * phase));
        int alpha = (int) Math.round((color >>> 24) * (min + (max - min) * wave));
        return (alpha << 24) | (color & 0x00FFFFFF);
    }

    private static void appendFullEdgeFlash(List<HudRenderCommand> out, int w, int h, int color) {
        out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, 0, 0, w, 4, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, 0, h - 4, w, 4, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, 0, 0, 4, h, color));
        out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, w - 4, 0, 4, h, color));
    }

    private static void appendAttention(List<HudRenderCommand> out, double attention, int screenWidth, int screenHeight) {
        int x = Math.max(8, screenWidth - 12);
        int y = Math.max(8, screenHeight - 28);
        out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, x, y, 3, 20, ATTENTION_BG));
        int fill = (int) Math.round(20 * Math.max(0.0, Math.min(1.0, attention)));
        out.add(HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, x, y + 20 - fill, 3, fill, ATTENTION_FILL));
    }

    private enum Edge {
        TOP,
        RIGHT,
        BOTTOM,
        LEFT
    }

    private static final class ThreatByEdge {
        private ThreatLevel top = ThreatLevel.empty();
        private ThreatLevel right = ThreatLevel.empty();
        private ThreatLevel bottom = ThreatLevel.empty();
        private ThreatLevel left = ThreatLevel.empty();
        private boolean lockWarning;

        private void add(Edge edge, double intensity, double distance) {
            ThreatLevel level = new ThreatLevel(intensity, distance);
            switch (edge) {
                case TOP -> top = top.max(level);
                case RIGHT -> right = right.max(level);
                case BOTTOM -> bottom = bottom.max(level);
                case LEFT -> left = left.max(level);
            }
        }

        private double max() {
            return Math.max(Math.max(top.intensity, right.intensity), Math.max(bottom.intensity, left.intensity));
        }
    }

    private record ThreatLevel(double intensity, double distance) {
        private ThreatLevel {
            intensity = Math.max(0.0, Math.min(1.0, Double.isFinite(intensity) ? intensity : 0.0));
            distance = Math.max(0.0, Double.isFinite(distance) ? distance : 64.0);
        }

        static ThreatLevel empty() {
            return new ThreatLevel(0.0, 64.0);
        }

        ThreatLevel max(ThreatLevel other) {
            return other.intensity > intensity ? other : this;
        }
    }
}
