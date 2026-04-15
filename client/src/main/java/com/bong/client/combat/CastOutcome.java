package com.bong.client.combat;

/**
 * Reason a cast terminated (§4.4 — drives item refund policy).
 */
public enum CastOutcome {
    NONE,
    COMPLETED,
    INTERRUPT_MOVEMENT,
    INTERRUPT_CONTAM,
    INTERRUPT_CONTROL,
    USER_CANCEL,
    DEATH;

    public boolean consumesItem() {
        return this == COMPLETED;
    }

    public boolean isInterrupt() {
        return switch (this) {
            case INTERRUPT_MOVEMENT, INTERRUPT_CONTAM, INTERRUPT_CONTROL, USER_CANCEL, DEATH -> true;
            default -> false;
        };
    }
}
