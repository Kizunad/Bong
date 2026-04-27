package com.bong.client.combat;

/** Client-side mirror for the 1-9 combat skill bar. */
public final class SkillBarStore {
    private static volatile SkillBarConfig snapshot = SkillBarConfig.empty();

    private SkillBarStore() {
    }

    public static SkillBarConfig snapshot() {
        return snapshot;
    }

    public static void replace(SkillBarConfig next) {
        snapshot = next == null ? SkillBarConfig.empty() : next;
    }

    public static void updateSlot(int index, SkillBarEntry entry) {
        snapshot = snapshot.withSlot(index, entry);
    }

    public static int findSkill(String skillId) {
        return snapshot.findSkill(skillId);
    }

    public static void resetForTests() {
        snapshot = SkillBarConfig.empty();
    }
}
