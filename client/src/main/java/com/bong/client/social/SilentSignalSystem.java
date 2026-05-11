package com.bong.client.social;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * plan-pvp-encounter-v1 P1: detects visible non-verbal player signals without adding rule text.
 */
public final class SilentSignalSystem {
    public static final double SIGNAL_RANGE_BLOCKS = 15.0;
    private static final double BACKING_AWAY_SPEED_LIMIT = 0.5;
    private static final int DOUBLE_CROUCH_THRESHOLD = 2;
    private static final long POINTING_THRESHOLD_MS = 2_000L;
    private static final long MEDITATING_THRESHOLD_MS = 3_000L;

    private SilentSignalSystem() {
    }

    public static List<SilentSignal> detect(ActionSnapshot snapshot) {
        if (snapshot == null || snapshot.distanceBlocks() > SIGNAL_RANGE_BLOCKS) {
            return Collections.emptyList();
        }

        List<SilentSignal> signals = new ArrayList<>();
        if (isTorch(snapshot.heldItemId())) {
            signals.add(new SilentSignal(SignalKind.PEACE_TORCH, "torch", snapshot.distanceBlocks(), false));
        }
        if (isBoneCoin(snapshot.droppedItemId())) {
            signals.add(new SilentSignal(SignalKind.BONE_COIN_OFFER, "bone_coin", snapshot.distanceBlocks(), false));
        }
        if (snapshot.movingBackward()
            && snapshot.facingTarget()
            && snapshot.speedMultiplier() <= BACKING_AWAY_SPEED_LIMIT) {
            signals.add(new SilentSignal(SignalKind.SLOW_BACK_AWAY, null, snapshot.distanceBlocks(), false));
        }
        if (snapshot.crouchToggles() >= DOUBLE_CROUCH_THRESHOLD) {
            signals.add(new SilentSignal(SignalKind.DOUBLE_CROUCH_WARNING, null, snapshot.distanceBlocks(), false));
        }
        if (snapshot.pointingDurationMs() >= POINTING_THRESHOLD_MS) {
            signals.add(new SilentSignal(SignalKind.DIRECTION_POINT, null, snapshot.distanceBlocks(), false));
        }
        if (snapshot.meditatingDurationMs() >= MEDITATING_THRESHOLD_MS) {
            signals.add(new SilentSignal(SignalKind.SEATED_NEUTRAL, null, snapshot.distanceBlocks(), false));
        }
        return List.copyOf(signals);
    }

    public static boolean shouldShowRuleExplanation(SilentSignal signal) {
        return false;
    }

    private static boolean isTorch(String itemId) {
        return "minecraft:torch".equals(itemId) || "torch".equals(itemId);
    }

    private static boolean isBoneCoin(String itemId) {
        return "bone_coin".equals(itemId) || "bong:bone_coin".equals(itemId);
    }

    public enum SignalKind {
        PEACE_TORCH,
        BONE_COIN_OFFER,
        SLOW_BACK_AWAY,
        DOUBLE_CROUCH_WARNING,
        DIRECTION_POINT,
        SEATED_NEUTRAL
    }

    public record SilentSignal(
        SignalKind kind,
        String iconItemId,
        double distanceBlocks,
        boolean explicitWarning
    ) {
    }

    public record ActionSnapshot(
        String remotePlayerId,
        double distanceBlocks,
        String heldItemId,
        String droppedItemId,
        boolean movingBackward,
        double speedMultiplier,
        boolean facingTarget,
        int crouchToggles,
        long pointingDurationMs,
        long meditatingDurationMs,
        boolean followUpAttack
    ) {
    }
}
