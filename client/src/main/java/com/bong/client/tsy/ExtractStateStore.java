package com.bong.client.tsy;

import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.math.Vec3d;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class ExtractStateStore {
    public static final double PORTAL_INTERACT_RADIUS = 2.0;
    private static final long STATUS_MESSAGE_MS = 1800L;
    private static final long WHITE_FLASH_MS = 1000L;
    private static final long RED_FLASH_MS = 280L;
    private static final int WHITE_FLASH_COLOR = 0xCCFFFFFF;
    private static final int RED_FLASH_COLOR = 0x66FF0000;
    private static final Map<Long, RiftPortalView> portals = new LinkedHashMap<>();
    private static volatile ExtractState snapshot = ExtractState.empty();
    private static boolean collapseFlashTriggered;

    private ExtractStateStore() {
    }

    public static ExtractState snapshot() {
        return snapshot;
    }

    public static synchronized void upsertPortal(RiftPortalView portal) {
        if (portal == null) {
            return;
        }
        portals.put(portal.entityId(), portal);
        refreshSnapshot(snapshot.activePortalEntityId(), snapshot.activePortalKind(), snapshot.elapsedTicks(),
            snapshot.requiredTicks(), snapshot.extracting(), snapshot.message(), snapshot.messageColor(),
            snapshot.messageUntilMs(), snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(),
            snapshot.collapseRemainingTicksAtStart(), snapshot.screenFlashUntilMs(), snapshot.screenFlashColor());
    }

    public static synchronized void removePortal(long entityId) {
        portals.remove(entityId);
        Long activePortal = snapshot.activePortalEntityId();
        if (activePortal != null && activePortal == entityId) {
            refreshSnapshot(null, "", 0, 0, false,
                "撤离中断：裂口闭合", 0xFFFF7070, System.currentTimeMillis() + STATUS_MESSAGE_MS,
                snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(), snapshot.collapseRemainingTicksAtStart(),
                snapshot.screenFlashUntilMs(), snapshot.screenFlashColor());
            return;
        }
        refreshSnapshot(activePortal, snapshot.activePortalKind(), snapshot.elapsedTicks(),
            snapshot.requiredTicks(), snapshot.extracting(), snapshot.message(), snapshot.messageColor(),
            snapshot.messageUntilMs(), snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(),
            snapshot.collapseRemainingTicksAtStart(), snapshot.screenFlashUntilMs(), snapshot.screenFlashColor());
    }

    public static synchronized RiftPortalView nearestPortal(PlayerEntity player) {
        if (player == null || portals.isEmpty()) {
            return null;
        }
        Vec3d pos = player.getPos();
        RiftPortalView best = null;
        double bestDistanceSq = Double.MAX_VALUE;
        for (RiftPortalView portal : portals.values()) {
            if (!"exit".equals(portal.direction())) {
                continue;
            }
            double radius = portal.triggerRadius() > 0.0 ? portal.triggerRadius() : PORTAL_INTERACT_RADIUS;
            double dx = portal.x() - pos.x;
            double dy = portal.y() - pos.y;
            double dz = portal.z() - pos.z;
            double distanceSq = dx * dx + dy * dy + dz * dz;
            if (distanceSq <= radius * radius && distanceSq <= bestDistanceSq) {
                best = portal;
                bestDistanceSq = distanceSq;
            }
        }
        return best;
    }

    public static synchronized void markStarted(long portalEntityId, String portalKind, int requiredTicks, long nowMs) {
        refreshSnapshot(portalEntityId, portalKind, 0, Math.max(0, requiredTicks), true,
            "撤离仪式开始", 0xFF80D8FF, nowMs + STATUS_MESSAGE_MS,
            snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(), snapshot.collapseRemainingTicksAtStart(),
            snapshot.screenFlashUntilMs(), snapshot.screenFlashColor());
    }

    public static synchronized void markProgress(long portalEntityId, int elapsedTicks, int requiredTicks, long nowMs) {
        refreshSnapshot(portalEntityId, snapshot.activePortalKind(), Math.max(0, elapsedTicks),
            Math.max(0, requiredTicks), true, snapshot.message(), snapshot.messageColor(), snapshot.messageUntilMs(),
            snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(), snapshot.collapseRemainingTicksAtStart(),
            snapshot.screenFlashUntilMs(), snapshot.screenFlashColor());
    }

    public static synchronized void markCompleted(String familyId, long nowMs) {
        refreshSnapshot(null, "", 0, 0, false,
            "已撤出：" + safeText(familyId, "TSY"), 0xFF80FF80, nowMs + STATUS_MESSAGE_MS,
            snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(), snapshot.collapseRemainingTicksAtStart(),
            nowMs + WHITE_FLASH_MS, WHITE_FLASH_COLOR);
    }

    public static synchronized void markAborted(String reason, long nowMs) {
        boolean rejection = isRejectionReason(reason);
        refreshSnapshot(null, "", 0, 0, false,
            (rejection ? "无法撤离：" : "撤离中断：") + reasonLabel(reason), 0xFFFF7070, nowMs + STATUS_MESSAGE_MS,
            snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(), snapshot.collapseRemainingTicksAtStart(),
            rejection ? snapshot.screenFlashUntilMs() : nowMs + RED_FLASH_MS,
            rejection ? snapshot.screenFlashColor() : RED_FLASH_COLOR);
    }

    public static synchronized void markFailed(String reason, long nowMs) {
        refreshSnapshot(null, "", 0, 0, false,
            "撤离失败：" + reasonLabel(reason), 0xFFFF4040, nowMs + STATUS_MESSAGE_MS,
            snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(), snapshot.collapseRemainingTicksAtStart(),
            nowMs + RED_FLASH_MS, RED_FLASH_COLOR);
    }

    public static synchronized void markCollapseStarted(String familyId, int remainingTicks, long nowMs) {
        refreshSnapshot(snapshot.activePortalEntityId(), snapshot.activePortalKind(), snapshot.elapsedTicks(),
            snapshot.requiredTicks(), snapshot.extracting(), "坍缩开始，撤离点压缩", 0xFFFF7070,
            nowMs + STATUS_MESSAGE_MS, safeText(familyId, "TSY"), nowMs, Math.max(0, remainingTicks),
            nowMs + RED_FLASH_MS, RED_FLASH_COLOR);
        collapseFlashTriggered = false;
    }

    public static synchronized void tick(long nowMs) {
        if (!collapseFlashTriggered
            && snapshot.collapseStartedAtMs() > 0
            && snapshot.collapseRemainingTicksAtStart() > 0
            && snapshot.collapseRemainingTicks(nowMs) <= 0) {
            collapseFlashTriggered = true;
            refreshSnapshot(snapshot.activePortalEntityId(), snapshot.activePortalKind(), snapshot.elapsedTicks(),
                snapshot.requiredTicks(), snapshot.extracting(), snapshot.message(), snapshot.messageColor(),
                snapshot.messageUntilMs(), snapshot.collapsingFamilyId(), snapshot.collapseStartedAtMs(),
                snapshot.collapseRemainingTicksAtStart(), nowMs + WHITE_FLASH_MS, WHITE_FLASH_COLOR);
        }
        if (snapshot.hasTimedMessage(nowMs) || snapshot.collapseActive(nowMs) || snapshot.screenFlashActive(nowMs)) {
            return;
        }
        if (snapshot.messageUntilMs() > 0 || snapshot.collapseStartedAtMs() > 0 || snapshot.screenFlashUntilMs() > 0) {
            refreshSnapshot(snapshot.activePortalEntityId(), snapshot.activePortalKind(), snapshot.elapsedTicks(),
                snapshot.requiredTicks(), snapshot.extracting(), "", 0xFFFFFFFF, 0L,
                "", 0L, 0, 0L, 0);
        }
    }

    public static synchronized void resetForTests() {
        portals.clear();
        snapshot = ExtractState.empty();
        collapseFlashTriggered = false;
    }

    private static void refreshSnapshot(
        Long activePortalEntityId,
        String activePortalKind,
        int elapsedTicks,
        int requiredTicks,
        boolean extracting,
        String message,
        int messageColor,
        long messageUntilMs,
        String collapsingFamilyId,
        long collapseStartedAtMs,
        int collapseRemainingTicksAtStart,
        long screenFlashUntilMs,
        int screenFlashColor
    ) {
        snapshot = new ExtractState(
            List.copyOf(portals.values()),
            activePortalEntityId,
            activePortalKind == null ? "" : activePortalKind,
            elapsedTicks,
            requiredTicks,
            extracting,
            message == null ? "" : message,
            messageColor,
            messageUntilMs,
            collapsingFamilyId == null ? "" : collapsingFamilyId,
            collapseStartedAtMs,
            collapseRemainingTicksAtStart,
            screenFlashUntilMs,
            screenFlashColor,
            System.currentTimeMillis()
        );
    }

    private static String reasonLabel(String reason) {
        return switch (reason == null ? "" : reason) {
            case "moved" -> "移动";
            case "combat" -> "战斗";
            case "damaged" -> "受击";
            case "portal_expired" -> "裂口闭合";
            case "out_of_range" -> "距离过远";
            case "not_in_tsy" -> "不在坍缩渊";
            case "already_busy" -> "你已在撤离中";
            case "portal_occupied" -> "裂口被占，换下一个";
            case "cannot_exit" -> "不可从此裂口撤离";
            case "spirit_qi_drained" -> "真元耗尽";
            case "cancelled" -> "取消";
            default -> "未明";
        };
    }

    private static boolean isRejectionReason(String reason) {
        return switch (reason == null ? "" : reason) {
            case "out_of_range", "not_in_tsy", "already_busy", "portal_occupied", "cannot_exit" -> true;
            default -> false;
        };
    }

    private static String safeText(String value, String fallback) {
        return value == null || value.isBlank() ? fallback : value;
    }
}
