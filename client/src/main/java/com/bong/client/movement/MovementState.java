package com.bong.client.movement;

import java.util.Locale;

public record MovementState(
    double currentSpeedMultiplier,
    boolean staminaCostActive,
    Action action,
    ZoneKind zoneKind,
    long dashCooldownRemainingTicks,
    double hitboxHeightBlocks,
    double staminaCurrent,
    double staminaMax,
    boolean lowStamina,
    Long lastActionTick,
    String rejectedAction,
    long receivedAtMs,
    long hudActivityAtMs,
    long rejectedAtMs
) {
    public MovementState {
        currentSpeedMultiplier = finiteNonNegative(currentSpeedMultiplier);
        action = action == null ? Action.NONE : action;
        zoneKind = zoneKind == null ? ZoneKind.NORMAL : zoneKind;
        dashCooldownRemainingTicks = Math.max(0L, dashCooldownRemainingTicks);
        hitboxHeightBlocks = finiteNonNegative(hitboxHeightBlocks);
        staminaCurrent = finiteNonNegative(staminaCurrent);
        staminaMax = Math.max(1.0, finiteNonNegative(staminaMax));
        if (staminaCurrent > staminaMax) {
            staminaCurrent = staminaMax;
        }
        if (lastActionTick != null && lastActionTick < 0L) {
            lastActionTick = null;
        }
        rejectedAction = rejectedAction == null ? "" : rejectedAction.trim();
        receivedAtMs = Math.max(0L, receivedAtMs);
        hudActivityAtMs = Math.max(0L, hudActivityAtMs);
        rejectedAtMs = Math.max(0L, rejectedAtMs);
    }

    public static MovementState empty() {
        return new MovementState(
            0.0,
            false,
            Action.NONE,
            ZoneKind.NORMAL,
            0L,
            1.8,
            0.0,
            1.0,
            false,
            null,
            "",
            0L,
            0L,
            0L
        );
    }

    public MovementState withTiming(long receivedAtMs, long hudActivityAtMs, long rejectedAtMs) {
        return new MovementState(
            currentSpeedMultiplier,
            staminaCostActive,
            action,
            zoneKind,
            dashCooldownRemainingTicks,
            hitboxHeightBlocks,
            staminaCurrent,
            staminaMax,
            lowStamina,
            lastActionTick,
            rejectedAction,
            receivedAtMs,
            hudActivityAtMs,
            rejectedAtMs
        );
    }

    public boolean isEmpty() {
        return action == Action.NONE
            && zoneKind == ZoneKind.NORMAL
            && currentSpeedMultiplier == 0.0
            && !staminaCostActive
            && dashCooldownRemainingTicks == 0L
            && staminaCurrent == 0.0
            && staminaMax == 1.0
            && !lowStamina
            && lastActionTick == null
            && rejectedAction.isEmpty();
    }

    public double staminaRatio() {
        return staminaMax <= 0.0 ? 0.0 : Math.max(0.0, Math.min(1.0, staminaCurrent / staminaMax));
    }

    public boolean rejectedRecently(long nowMs, long windowMs) {
        return rejectedAtMs > 0L && nowMs >= rejectedAtMs && nowMs - rejectedAtMs <= windowMs;
    }

    private static double finiteNonNegative(double value) {
        if (!Double.isFinite(value) || value < 0.0) {
            return 0.0;
        }
        return value;
    }

    public enum Action {
        NONE("none"),
        DASHING("dashing");

        private final String wireName;

        Action(String wireName) {
            this.wireName = wireName;
        }

        public static Action fromWireName(String raw) {
            String normalized = raw == null ? "" : raw.trim().toLowerCase(Locale.ROOT);
            for (Action action : values()) {
                if (action.wireName.equals(normalized)) {
                    return action;
                }
            }
            return null;
        }

        public String wireName() {
            return wireName;
        }
    }

    public enum ZoneKind {
        NORMAL("normal"),
        DEAD("dead"),
        NEGATIVE("negative"),
        RESIDUE_ASH("residue_ash");

        private final String wireName;

        ZoneKind(String wireName) {
            this.wireName = wireName;
        }

        public static ZoneKind fromWireName(String raw) {
            String normalized = raw == null ? "" : raw.trim().toLowerCase(Locale.ROOT);
            for (ZoneKind kind : values()) {
                if (kind.wireName.equals(normalized)) {
                    return kind;
                }
            }
            return null;
        }

        public String wireName() {
            return wireName;
        }
    }
}
