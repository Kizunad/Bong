package com.bong.client.combat;

import java.util.Arrays;

/**
 * Immutable snapshot of the 9 F-key quick-use slot bindings (§11.1
 * {@code QuickUseSlotStore}). Slot 0 ↔ F1, slot 8 ↔ F9.
 */
public final class QuickSlotConfig {
    public static final int SLOT_COUNT = 9;
    private static final QuickSlotConfig EMPTY = new QuickSlotConfig(new QuickSlotEntry[SLOT_COUNT], new long[SLOT_COUNT]);

    private final QuickSlotEntry[] slots;
    private final long[] cooldownUntilMs;

    private QuickSlotConfig(QuickSlotEntry[] slots, long[] cooldownUntilMs) {
        this.slots = slots;
        this.cooldownUntilMs = cooldownUntilMs;
    }

    public static QuickSlotConfig empty() {
        return EMPTY;
    }

    public static QuickSlotConfig of(QuickSlotEntry[] slots, long[] cooldownUntilMs) {
        QuickSlotEntry[] copy = new QuickSlotEntry[SLOT_COUNT];
        long[] cds = new long[SLOT_COUNT];
        if (slots != null) {
            int n = Math.min(SLOT_COUNT, slots.length);
            System.arraycopy(slots, 0, copy, 0, n);
        }
        if (cooldownUntilMs != null) {
            int n = Math.min(SLOT_COUNT, cooldownUntilMs.length);
            System.arraycopy(cooldownUntilMs, 0, cds, 0, n);
        }
        return new QuickSlotConfig(copy, cds);
    }

    public QuickSlotEntry slot(int index) {
        if (index < 0 || index >= SLOT_COUNT) return null;
        return slots[index];
    }

    public long cooldownUntilMs(int index) {
        if (index < 0 || index >= SLOT_COUNT) return 0L;
        return cooldownUntilMs[index];
    }

    public boolean isOnCooldown(int index, long nowMs) {
        return cooldownUntilMs(index) > nowMs;
    }

    public float cooldownProgress(int index, long nowMs, long startMs) {
        long end = cooldownUntilMs(index);
        if (end <= nowMs || end <= startMs) return 1.0f;
        long span = end - startMs;
        if (span <= 0L) return 1.0f;
        long elapsed = nowMs - startMs;
        if (elapsed <= 0L) return 0.0f;
        return Math.min(1.0f, (float) elapsed / (float) span);
    }

    public QuickSlotConfig withSlot(int index, QuickSlotEntry entry) {
        if (index < 0 || index >= SLOT_COUNT) return this;
        QuickSlotEntry[] copy = Arrays.copyOf(slots, SLOT_COUNT);
        copy[index] = entry;
        return new QuickSlotConfig(copy, Arrays.copyOf(cooldownUntilMs, SLOT_COUNT));
    }

    public QuickSlotConfig withCooldownUntil(int index, long untilMs) {
        if (index < 0 || index >= SLOT_COUNT) return this;
        long[] copy = Arrays.copyOf(cooldownUntilMs, SLOT_COUNT);
        copy[index] = Math.max(copy[index], untilMs);
        return new QuickSlotConfig(Arrays.copyOf(slots, SLOT_COUNT), copy);
    }
}
