package com.bong.client.combat;

import java.util.Arrays;

/** Immutable snapshot of the 1-9 combat skill bar bindings. */
public final class SkillBarConfig {
    public static final int SLOT_COUNT = 9;
    private static final SkillBarConfig EMPTY = new SkillBarConfig(new SkillBarEntry[SLOT_COUNT], new long[SLOT_COUNT]);

    private final SkillBarEntry[] slots;
    private final long[] cooldownUntilMs;

    private SkillBarConfig(SkillBarEntry[] slots, long[] cooldownUntilMs) {
        this.slots = slots;
        this.cooldownUntilMs = cooldownUntilMs;
    }

    public static SkillBarConfig empty() {
        return EMPTY;
    }

    public static SkillBarConfig of(SkillBarEntry[] slots, long[] cooldownUntilMs) {
        SkillBarEntry[] slotCopy = new SkillBarEntry[SLOT_COUNT];
        long[] cooldownCopy = new long[SLOT_COUNT];
        if (slots != null) {
            System.arraycopy(slots, 0, slotCopy, 0, Math.min(SLOT_COUNT, slots.length));
        }
        if (cooldownUntilMs != null) {
            System.arraycopy(cooldownUntilMs, 0, cooldownCopy, 0, Math.min(SLOT_COUNT, cooldownUntilMs.length));
        }
        return new SkillBarConfig(slotCopy, cooldownCopy);
    }

    public SkillBarEntry slot(int index) {
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

    public SkillBarConfig withSlot(int index, SkillBarEntry entry) {
        if (index < 0 || index >= SLOT_COUNT) return this;
        SkillBarEntry[] slotCopy = Arrays.copyOf(slots, SLOT_COUNT);
        slotCopy[index] = entry;
        return new SkillBarConfig(slotCopy, Arrays.copyOf(cooldownUntilMs, SLOT_COUNT));
    }

    public int findSkill(String skillId) {
        if (skillId == null || skillId.isBlank()) return -1;
        for (int i = 0; i < SLOT_COUNT; i++) {
            SkillBarEntry entry = slots[i];
            if (entry != null && entry.kind() == SkillBarEntry.Kind.SKILL && skillId.equals(entry.id())) {
                return i;
            }
        }
        return -1;
    }
}
