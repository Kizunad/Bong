package com.bong.client.hud;

import com.bong.client.tsy.ExtractState;
import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.tsy.RiftPortalView;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.math.Vec3d;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;

public final class ExtractProgressHudPlanner {
    private static final int PANEL_WIDTH = 240;
    private static final int PANEL_HEIGHT = 44;
    private static final int RIFT_LIST_WIDTH = 150;
    private static final int TRACK_HEIGHT = 5;
    private static final int BG = 0xD0111118;
    private static final int BORDER = 0xFF60A8FF;
    private static final int DANGER = 0xFFFF5050;
    private static final int DANGER_DIM = 0xCC3A0A10;
    private static final int TEXT = 0xFFE6F3FF;
    private static final int MUTED = 0xFF8EA5B8;
    private static final int ACTIVE_PORTAL = 0xFF717982;
    private static final int FILL = 0xFF60D8FF;
    private static final String COLLAPSE_HINT = "→ 冲入塌缩裂口（已占即换下一个）";

    private ExtractProgressHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        if (screenWidth <= 0 || screenHeight <= 0 || widthMeasurer == null) {
            return out;
        }
        ExtractState safe = state == null ? ExtractState.empty() : state;
        if (safe.screenFlashActive(nowMs)) {
            out.add(HudRenderCommand.screenTint(HudRenderLayer.TSY_EXTRACT, safe.screenFlashColor()));
        }
        appendCollapse(out, safe, widthMeasurer, screenWidth, screenHeight, nowMs);
        if (safe.extracting()) {
            appendExtractBar(out, safe, widthMeasurer, screenWidth, screenHeight);
        } else {
            appendNearestPortalHint(out, safe, widthMeasurer, screenWidth, screenHeight);
        }
        if (safe.hasTimedMessage(nowMs)) {
            appendMessage(out, safe, widthMeasurer, screenWidth, screenHeight);
        }
        return List.copyOf(out);
    }

    private static void appendExtractBar(
        List<HudRenderCommand> out,
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        int x = (screenWidth - PANEL_WIDTH) / 2;
        int y = screenHeight - 92;
        appendPanel(out, x, y, BORDER);
        double progress = state.requiredTicks() <= 0 ? 0.0 : (double) state.elapsedTicks() / (double) state.requiredTicks();
        progress = Math.max(0.0, Math.min(1.0, progress));
        int remainingTicks = Math.max(0, state.requiredTicks() - state.elapsedTicks());
        String label = "撤离中 " + secondsLabel(remainingTicks) + " · " + kindLabel(state.activePortalKind());
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, clip(label, PANEL_WIDTH - 16, widthMeasurer), x + 8, y + 8, TEXT));
        int trackX = x + 8;
        int trackY = y + 28;
        int trackW = PANEL_WIDTH - 16;
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, trackX, trackY, trackW, TRACK_HEIGHT, 0xFF101820));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, trackX, trackY, (int) Math.round(trackW * progress), TRACK_HEIGHT, FILL));
    }

    private static void appendNearestPortalHint(
        List<HudRenderCommand> out,
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        MinecraftClient client = MinecraftClient.getInstance();
        RiftPortalView portal = ExtractStateStore.nearestPortal(client == null ? null : client.player);
        if (portal == null) {
            return;
        }
        int x = (screenWidth - PANEL_WIDTH) / 2;
        int y = screenHeight - 78;
        appendPanel(out, x, y, BORDER);
        String label = kindLabel(portal.kind()) + " · 按 Y 开始撤离 [" + secondsLabel(portal.currentExtractTicks()) + "]";
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, clip(label, PANEL_WIDTH - 16, widthMeasurer), x + 8, y + 12, TEXT));
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, "移动 / 战斗 / 受击会归零", x + 8, y + 26, MUTED));
    }

    private static void appendCollapse(
        List<HudRenderCommand> out,
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        if (!state.collapseActive(nowMs)) {
            return;
        }
        int remaining = state.collapseRemainingTicks(nowMs);
        out.add(HudRenderCommand.screenTint(HudRenderLayer.TSY_EXTRACT, 0x22FF0000));

        int bannerHeight = Math.max(28, screenHeight / 8);
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, 0, 0, screenWidth, bannerHeight, DANGER_DIM));
        String banner = "塌缩 RACE-OUT";
        int bannerX = (screenWidth - widthMeasurer.measure(banner)) / 2;
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, banner, Math.max(8, bannerX), 10, DANGER));

        String countdown = Integer.toString(collapseCountdownSeconds(remaining));
        double countdownScale = screenHeight >= 360 ? 4.0 : 3.0;
        int countdownWidth = (int) Math.round(widthMeasurer.measure(countdown) * countdownScale);
        int countdownX = Math.max(8, (screenWidth - countdownWidth) / 2);
        int countdownY = Math.max(bannerHeight + 12, screenHeight / 4);
        out.add(HudRenderCommand.scaledText(
            HudRenderLayer.TSY_EXTRACT,
            countdown,
            countdownX,
            countdownY,
            0xFFFF3030,
            countdownScale
        ));

        String label = "化死域 " + secondsLabel(remaining);
        int labelX = Math.max(8, (screenWidth - widthMeasurer.measure(label)) / 2);
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, label, labelX, countdownY + 42, DANGER));
        out.add(HudRenderCommand.text(
            HudRenderLayer.TSY_EXTRACT,
            COLLAPSE_HINT,
            Math.max(8, (screenWidth - widthMeasurer.measure(COLLAPSE_HINT)) / 2),
            countdownY + 54,
            MUTED
        ));
        appendCollapseRiftListWithPlayerPos(out, state, widthMeasurer, screenWidth, bannerHeight, playerPos());
    }

    private static void appendCollapseRiftListWithPlayerPos(
        List<HudRenderCommand> out,
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int bannerHeight,
        Vec3d playerPos
    ) {
        List<RiftDistance> nearest = state.portals().stream()
            .filter(portal -> "collapse_tear".equals(portal.kind()))
            .filter(portal -> "exit".equals(portal.direction()))
            .filter(portal -> state.collapsingFamilyId().equals(portal.familyId()))
            .map(portal -> new RiftDistance(portal, distanceSq(portal, playerPos)))
            .sorted(Comparator.comparingDouble(RiftDistance::distanceSq).thenComparingLong(rift -> rift.portal().entityId()))
            .limit(5)
            .toList();
        if (nearest.isEmpty()) {
            return;
        }

        int height = 18 + nearest.size() * 12;
        int x = Math.max(8, screenWidth - RIFT_LIST_WIDTH - 12);
        int y = bannerHeight + 8;
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y, RIFT_LIST_WIDTH, height, 0xAA0B0F16));
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, "本族裂口", x + 8, y + 6, TEXT));
        for (int i = 0; i < nearest.size(); i++) {
            RiftPortalView portal = nearest.get(i).portal();
            boolean currentPlayerPortal = state.activePortalEntityId() != null && state.activePortalEntityId() == portal.entityId();
            String line = riftListLine(portal, nearest.get(i).distanceSq(), currentPlayerPortal);
            out.add(HudRenderCommand.text(
                HudRenderLayer.TSY_EXTRACT,
                clip(line, RIFT_LIST_WIDTH - 16, widthMeasurer),
                x + 8,
                y + 20 + i * 12,
                currentPlayerPortal ? ACTIVE_PORTAL : DANGER
            ));
        }
    }

    private static Vec3d playerPos() {
        MinecraftClient client = MinecraftClient.getInstance();
        PlayerEntity player = client == null ? null : client.player;
        return player == null ? Vec3d.ZERO : player.getPos();
    }

    private static double distanceSq(RiftPortalView portal, Vec3d pos) {
        double dx = portal.x() - pos.x;
        double dy = portal.y() - pos.y;
        double dz = portal.z() - pos.z;
        return dx * dx + dy * dy + dz * dz;
    }

    private static String riftListLine(RiftPortalView portal, double distanceSq, boolean currentPlayerPortal) {
        long blocks = Math.round(Math.sqrt(Math.max(0.0, distanceSq)));
        String marker = currentPlayerPortal ? "我" : "›";
        return String.format(Locale.ROOT, "%s %s 距 %d 格", marker, kindIcon(portal.kind()), blocks);
    }

    private static int collapseCountdownSeconds(int ticks) {
        return Math.max(1, (int) Math.ceil(Math.max(0, ticks) / 20.0));
    }

    private static void appendMessage(
        List<HudRenderCommand> out,
        ExtractState state,
        HudTextHelper.WidthMeasurer widthMeasurer,
        int screenWidth,
        int screenHeight
    ) {
        String text = clip(state.message(), PANEL_WIDTH, widthMeasurer);
        int x = (screenWidth - widthMeasurer.measure(text)) / 2;
        int y = screenHeight - 124;
        out.add(HudRenderCommand.text(HudRenderLayer.TSY_EXTRACT, text, x, y, state.messageColor()));
    }

    private static void appendPanel(List<HudRenderCommand> out, int x, int y, int border) {
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x + 2, y + 2, PANEL_WIDTH, PANEL_HEIGHT, 0x88000000));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y, PANEL_WIDTH, PANEL_HEIGHT, BG));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y, PANEL_WIDTH, 1, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y + PANEL_HEIGHT - 1, PANEL_WIDTH, 1, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x, y, 1, PANEL_HEIGHT, border));
        out.add(HudRenderCommand.rect(HudRenderLayer.TSY_EXTRACT, x + PANEL_WIDTH - 1, y, 1, PANEL_HEIGHT, border));
    }

    private static String clip(String value, int maxWidth, HudTextHelper.WidthMeasurer widthMeasurer) {
        return HudTextHelper.clipToWidth(value == null ? "" : value, maxWidth, widthMeasurer);
    }

    private static String secondsLabel(int ticks) {
        double seconds = Math.max(0, ticks) / 20.0;
        if (seconds >= 10.0 || Math.abs(seconds - Math.rint(seconds)) < 0.05) {
            return Math.round(seconds) + "s";
        }
        return String.format(java.util.Locale.ROOT, "%.1fs", seconds);
    }

    private static String kindLabel(String kind) {
        return switch (kind == null ? "" : kind) {
            case "main_rift" -> "主裂缝";
            case "deep_rift" -> "深层缝";
            case "collapse_tear" -> "塌缩裂口";
            default -> "撤离点";
        };
    }

    private static String kindIcon(String kind) {
        return switch (kind == null ? "" : kind) {
            case "collapse_tear" -> "裂";
            case "deep_rift" -> "深";
            case "main_rift" -> "主";
            default -> "门";
        };
    }

    private record RiftDistance(RiftPortalView portal, double distanceSq) {
    }
}
