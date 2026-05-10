package com.bong.client.hud;

import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.forge.state.ForgeSessionStore;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class ForgeProgressHudPlanner {
    static final int PANEL_WIDTH = 190;
    static final int BAR_HEIGHT = 6;
    static final int BG = 0xB0101018;
    static final int TRACK = 0xFF202030;
    static final int FILL = 0xFFE8B85C;
    static final int TEXT = 0xFFECE0C0;
    static final int MUTED = 0xFFB0AAA0;
    static final long STEP_FLASH_MS = 100L;

    private static long lastSessionId = -1L;
    private static String lastStep = "";
    private static long stepChangedAt = 0L;

    private ForgeProgressHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(int screenWidth, int screenHeight, long nowMs) {
        List<HudRenderCommand> out = new ArrayList<>();
        ForgeSessionStore.Snapshot session = ForgeSessionStore.snapshot();
        if (screenWidth <= 0 || screenHeight <= 0) {
            return out;
        }
        if (session != null && session.active()) {
            trackStep(session, nowMs);
            appendSession(out, session, screenWidth, screenHeight, nowMs);
        }
        appendOutcomeToast(out, ForgeOutcomeStore.lastOutcome());
        return List.copyOf(out);
    }

    static double progressOf(ForgeSessionStore.Snapshot session) {
        if (session == null) {
            return 0.0;
        }
        JsonObject json = parseJsonObject(session.stepStateJson());
        if (json == null) {
            return 0.0;
        }
        if (json.has("progress")) {
            return clamp01(json.get("progress").getAsDouble());
        }
        if (json.has("progress_ratio")) {
            return clamp01(json.get("progress_ratio").getAsDouble());
        }
        if (json.has("elapsed_ticks") && json.has("target_ticks")) {
            double target = Math.max(1.0, json.get("target_ticks").getAsDouble());
            return clamp01(json.get("elapsed_ticks").getAsDouble() / target);
        }
        return 0.0;
    }

    static String stepLabel(String step) {
        String normalized = step == null ? "" : step.trim().toLowerCase(Locale.ROOT);
        return switch (normalized) {
            case "tempering", "quench" -> "淬火中...";
            case "inscription" -> "铭文刻划...";
            case "consecration" -> "祭炼中...";
            case "assembly", "assemble" -> "合锋中...";
            case "done" -> "锻造完成";
            default -> normalized.isEmpty() ? "锻造中..." : normalized + "...";
        };
    }

    static void resetForTests() {
        lastSessionId = -1L;
        lastStep = "";
        stepChangedAt = 0L;
    }

    private static void appendSession(
        List<HudRenderCommand> out,
        ForgeSessionStore.Snapshot session,
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        int x = Math.max(8, (screenWidth - PANEL_WIDTH) / 2);
        int y = Math.max(24, screenHeight - 112);
        double progress = progressOf(session);
        int flashAlpha = HudAnimation.alphaForFlash(stepChangedAt, STEP_FLASH_MS, nowMs, 80);
        if (flashAlpha > 0) {
            out.add(HudRenderCommand.screenTint(HudRenderLayer.PROCESSING_HUD, HudTextHelper.withAlpha(0xFFFFFF, flashAlpha)));
        }
        out.add(HudRenderCommand.rect(HudRenderLayer.PROCESSING_HUD, x, y, PANEL_WIDTH, 28, BG));
        out.add(HudRenderCommand.text(HudRenderLayer.PROCESSING_HUD, stepLabel(session.currentStep()), x + 8, y + 6, TEXT));
        out.add(HudRenderCommand.text(
            HudRenderLayer.PROCESSING_HUD,
            "Tier " + session.achievedTier(),
            x + PANEL_WIDTH - 42,
            y + 6,
            MUTED
        ));
        int barX = x + 8;
        int barY = y + 18;
        int barW = PANEL_WIDTH - 16;
        out.add(HudRenderCommand.rect(HudRenderLayer.PROCESSING_HUD, barX, barY, barW, BAR_HEIGHT, TRACK));
        int fill = HudAnimation.smoothFillWidth(progress, progress, barW, 1.0);
        if (fill > 0) {
            out.add(HudRenderCommand.rect(HudRenderLayer.PROCESSING_HUD, barX, barY, fill, BAR_HEIGHT, FILL));
        }
    }

    private static void appendOutcomeToast(List<HudRenderCommand> out, ForgeOutcomeStore.Snapshot outcome) {
        if (outcome == null || outcome.sessionId() <= 0 || outcome.weaponItem() == null || outcome.weaponItem().isBlank()) {
            return;
        }
        String text = "炼器完成 · " + outcome.weaponItem() + " · " + outcome.bucket();
        out.add(HudRenderCommand.toast(HudRenderLayer.TOAST, text, 0, 0, rarityColor(outcome.bucket())));
    }

    private static int rarityColor(String bucket) {
        String normalized = bucket == null ? "" : bucket.trim().toLowerCase(Locale.ROOT);
        return switch (normalized) {
            case "legendary", "perfect" -> 0xFFFFD166;
            case "rare", "good" -> 0xFF88C8FF;
            case "flawed" -> 0xFFFFAA60;
            default -> 0xFFE0E0E0;
        };
    }

    private static void trackStep(ForgeSessionStore.Snapshot session, long nowMs) {
        String step = session.currentStep() == null ? "" : session.currentStep();
        if (session.sessionId() != lastSessionId || !step.equals(lastStep)) {
            lastSessionId = session.sessionId();
            lastStep = step;
            stepChangedAt = Math.max(0L, nowMs);
        }
    }

    private static JsonObject parseJsonObject(String json) {
        try {
            if (json == null || json.isBlank()) {
                return null;
            }
            return JsonParser.parseString(json).getAsJsonObject();
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
