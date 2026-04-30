package com.bong.client.hud;

import com.bong.client.combat.store.TribulationBroadcastStore;
import com.bong.client.combat.store.TribulationStateStore;
import com.bong.client.state.PlayerStateStore;
import net.minecraft.client.MinecraftClient;

import java.util.ArrayList;
import java.util.List;

/**
 * Top-of-screen red broadcast + spectate tip (plan §U6).
 */
public final class TribulationBroadcastHudPlanner {
    public static final int TOP_MARGIN = 28;
    public static final int BAR_HEIGHT = 18;
    public static final int TEXT_COLOR = 0xFFFF4040;
    public static final int BG_COLOR = 0xC0200000;
    public static final int SPECTATE_COLOR = 0xFFFFE080;
    public static final double SPECTATE_HINT_DISTANCE = 50.0;

    private TribulationBroadcastHudPlanner() {}

    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        return buildCommands(screenWidth, screenHeight, nowMs, currentViewerPosition());
    }

    static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight,
        long nowMs,
        ViewerPosition viewerPosition
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        TribulationBroadcastStore.State state = TribulationBroadcastStore.snapshot();
        if (!state.active() || state.expired(nowMs)) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        String stageLabel = switch (state.stage()) {
            case "warn" -> "\u5929\u52ab\u5c06\u81f3";
            case "locked" -> "\u52ab\u9501\u5df2\u6210";
            case "striking" -> "\u5929\u96f7\u964d\u4e34";
            case "done" -> "\u5929\u52ab\u5df2\u8fc7";
            default -> "\u5929\u52ab\u5f02\u52a8";
        };
        String line = "\u26a1 " + stageLabel
            + " \u00b7 " + (state.actorName().isEmpty() ? "\u65e0\u540d\u4fee\u58eb" : state.actorName())
            + " \u00b7 \u5750\u6807 (" + Math.round(state.worldX()) + ", " + Math.round(state.worldZ()) + ")";
        TribulationStateStore.State tribulationState = TribulationStateStore.snapshot();
        String progress = progressLabel(tribulationState);
        if (!progress.isEmpty()) {
            line += " \u00b7 " + progress;
        }
        String viewerRole = viewerRoleLabel(tribulationState, PlayerStateStore.snapshot().playerId());
        if (!viewerRole.isEmpty()) {
            line += " \u00b7 " + viewerRole;
        }
        String direction = directionLabel(viewerPosition, state.worldX(), state.worldZ());
        if (!direction.isEmpty()) {
            line += " \u00b7 \u65b9\u4f4d " + direction;
        }
        if (state.spectateDistance() >= 0d) {
            line += " \u00b7 \u8ddd\u79bb " + Math.round(state.spectateDistance()) + " \u683c";
        }

        // Background bar
        out.add(HudRenderCommand.rect(HudRenderLayer.TRIBULATION, 0, TOP_MARGIN, screenWidth, BAR_HEIGHT, BG_COLOR));
        // Approximate centering: text width ~= line.length() * 6 (ASCII fallback).
        int approxWidth = Math.max(line.length() * 6, 120);
        int x = Math.max(4, (screenWidth - approxWidth) / 2);
        out.add(HudRenderCommand.text(HudRenderLayer.TRIBULATION, line, x, TOP_MARGIN + 5, TEXT_COLOR));

        // Spectate hint
        if (state.spectateInvite() && state.spectateDistance() >= 0d
            && state.spectateDistance() <= SPECTATE_HINT_DISTANCE) {
            String hint = "(" + Math.round(state.spectateDistance()) + " \u683c\u5185\uff0c\u53ef\u524d\u5f80\u89c2\u6218\uff0c100 \u683c\u5185\u4f1a\u627f\u96f7)";
            out.add(HudRenderCommand.text(
                HudRenderLayer.TRIBULATION, hint, x, TOP_MARGIN + BAR_HEIGHT + 2, SPECTATE_COLOR
            ));
        }
        return out;
    }

    static String progressLabel(TribulationStateStore.State state) {
        if (state == null || !state.active()) return "";
        String phase = switch (state.phase()) {
            case "omen" -> "\u9884\u5146\u671f";
            case "lock" -> "\u9501\u5b9a\u671f";
            case "heart_demon" -> "\u5fc3\u9b54\u52ab";
            case "settle" -> "\u7ed3\u7b97";
            case "wave" -> "\u52ab\u6ce2";
            default -> "";
        };
        if (state.waveTotal() > 0 && ("wave".equals(state.phase()) || "heart_demon".equals(state.phase()))) {
            phase += " " + state.waveCurrent() + "/" + state.waveTotal();
        }
        if (state.halfStepOnSuccess()) {
            phase += phase.isEmpty() ? "\u540d\u989d\u5df2\u6ee1" : " \u00b7 \u540d\u989d\u5df2\u6ee1";
        }
        return phase;
    }

    static String viewerRoleLabel(TribulationStateStore.State state, String localPlayerId) {
        if (state == null || !state.active()) return "";
        String normalizedLocal = localPlayerId == null ? "" : localPlayerId.trim();
        if (normalizedLocal.isEmpty()) return "\u89c2\u6218\u8005";
        if (normalizedLocal.equals(state.charId())) return "\u6e21\u52ab\u8005\u672c\u4eba";
        if (state.participants().contains(normalizedLocal)) return "\u622a\u80e1\u8005";
        return "\u89c2\u6218\u8005";
    }

    static String directionLabel(ViewerPosition viewerPosition, double targetX, double targetZ) {
        if (viewerPosition == null || !viewerPosition.finite()
            || !Double.isFinite(targetX) || !Double.isFinite(targetZ)) {
            return "";
        }
        double dx = targetX - viewerPosition.worldX();
        double dz = targetZ - viewerPosition.worldZ();
        if ((dx * dx + dz * dz) < 0.0001d) return "\u811a\u4e0b";

        double degrees = Math.toDegrees(Math.atan2(dz, dx));
        if (degrees < 0d) degrees += 360d;
        String[] labels = {
            "\u4e1c", "\u4e1c\u5357", "\u5357", "\u897f\u5357",
            "\u897f", "\u897f\u5317", "\u5317", "\u4e1c\u5317"
        };
        int index = (int) Math.floor((degrees + 22.5d) / 45d) % labels.length;
        return labels[index];
    }

    private static ViewerPosition currentViewerPosition() {
        try {
            MinecraftClient client = MinecraftClient.getInstance();
            if (client == null || client.player == null) return null;
            return new ViewerPosition(client.player.getX(), client.player.getZ());
        } catch (RuntimeException | LinkageError ignored) {
            return null;
        }
    }

    static record ViewerPosition(double worldX, double worldZ) {
        boolean finite() {
            return Double.isFinite(worldX) && Double.isFinite(worldZ);
        }
    }
}
