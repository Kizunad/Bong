package com.bong.client.hud;

import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemyOutcomeForecastStore;
import com.bong.client.alchemy.state.AlchemySessionStore;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class AlchemyProgressHudPlanner {
    static final int PANEL_WIDTH = 196;
    static final int BG = 0xB00E1418;
    static final int TRACK = 0xFF1C2630;
    static final int TEXT = 0xFFDCE8E0;
    static final int STEAM = 0x99D8F0FF;
    static final long OUTCOME_TOAST_MS = 4_000L;

    private AlchemyProgressHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight) {
        return buildCommands(screenWidth, screenHeight, System.currentTimeMillis());
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight, long nowMillis) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }
        AlchemySessionStore.Snapshot session = AlchemySessionStore.snapshot();
        if (AlchemyFurnaceStore.snapshot().hasSession() && session != null && session.isActive()) {
            appendSession(out, session, AlchemyOutcomeForecastStore.snapshot(), screenWidth, screenHeight);
        }
        appendOutcomeToast(out, AlchemyAttemptHistoryStore.snapshot(), AlchemyAttemptHistoryStore.lastAppendMillis(), nowMillis);
        return List.copyOf(out);
    }

    static double progressOf(AlchemySessionStore.Snapshot session) {
        if (session == null || session.targetTicks() <= 0) {
            return 0.0;
        }
        return clamp01(session.elapsedTicks() / (double) session.targetTicks());
    }

    static int tempColor(AlchemySessionStore.Snapshot session) {
        if (session == null) {
            return 0xFF7AA8FF;
        }
        float diff = Math.abs(session.tempCurrent() - session.tempTarget());
        if (diff <= session.tempBand() * 0.5f) {
            return 0xFF66D890;
        }
        if (session.tempCurrent() > session.tempTarget()) {
            return 0xFFE06040;
        }
        return 0xFF62A8FF;
    }

    private static void appendSession(
        List<HudRenderCommand> out,
        AlchemySessionStore.Snapshot session,
        AlchemyOutcomeForecastStore.Snapshot forecast,
        int screenWidth,
        int screenHeight
    ) {
        int x = Math.max(8, (screenWidth - PANEL_WIDTH) / 2);
        int y = Math.max(24, screenHeight - 146);
        out.add(HudRenderCommand.rect(HudRenderLayer.PROCESSING_HUD, x, y, PANEL_WIDTH, 42, BG));
        out.add(HudRenderCommand.text(
            HudRenderLayer.PROCESSING_HUD,
            "炼制 " + Math.round(progressOf(session) * 100.0) + "% · " + session.statusLabel(),
            x + 8,
            y + 6,
            TEXT
        ));
        appendBar(out, x + 8, y + 18, PANEL_WIDTH - 16, progressOf(session), 0xFF88E090);
        double tempRatio = clamp01(session.tempCurrent());
        appendBar(out, x + 8, y + 28, PANEL_WIDTH - 16, tempRatio, tempColor(session));
        String steam = "~ ~";
        out.add(HudRenderCommand.text(HudRenderLayer.PROCESSING_HUD, steam, x + PANEL_WIDTH - 30, y - 4, STEAM));
        if (forecast != null) {
            out.add(HudRenderCommand.text(
                HudRenderLayer.PROCESSING_HUD,
                String.format(Locale.ROOT, "良 %.0f%% 废 %.0f%%", forecast.goodPct(), forecast.wastePct()),
                x + 8,
                y + 32,
                0xFFB8C8C0
            ));
        }
    }

    private static void appendOutcomeToast(
        List<HudRenderCommand> out,
        List<AlchemyAttemptHistoryStore.Entry> history,
        long updatedAtMillis,
        long nowMillis
    ) {
        if (history == null || history.isEmpty() || !isRecentOutcome(updatedAtMillis, nowMillis)) {
            return;
        }
        AlchemyAttemptHistoryStore.Entry last = history.get(history.size() - 1);
        String label = switch (last.bucket()) {
            case "perfect" -> "丹成上品";
            case "good" -> "丹成";
            case "flawed" -> "丹成有瑕";
            case "explode" -> "炸炉";
            default -> "炼废";
        };
        String pill = last.pill().isBlank() ? last.recipeId() : last.pill();
        out.add(HudRenderCommand.toast(HudRenderLayer.TOAST, label + " · " + pill, 0, 0, outcomeColor(last.bucket())));
    }

    static boolean isRecentOutcome(long updatedAtMillis, long nowMillis) {
        long safeUpdatedAt = Math.max(0L, updatedAtMillis);
        long safeNow = Math.max(0L, nowMillis);
        if (safeUpdatedAt == 0L) {
            return false;
        }
        if (safeNow < safeUpdatedAt) {
            return true;
        }
        return safeNow - safeUpdatedAt <= OUTCOME_TOAST_MS;
    }

    private static void appendBar(List<HudRenderCommand> out, int x, int y, int width, double ratio, int fillColor) {
        out.add(HudRenderCommand.rect(HudRenderLayer.PROCESSING_HUD, x, y, width, 4, TRACK));
        int fill = Math.max(0, Math.min(width, (int) Math.round(width * clamp01(ratio))));
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.PROCESSING_HUD, x, y, fill, 4, fillColor));
        }
    }

    private static int outcomeColor(String bucket) {
        return switch (bucket == null ? "" : bucket) {
            case "perfect" -> 0xFFFFD166;
            case "good" -> 0xFF80E090;
            case "flawed" -> 0xFFFFB060;
            case "explode" -> 0xFFFF6060;
            default -> 0xFFB8B8B8;
        };
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
