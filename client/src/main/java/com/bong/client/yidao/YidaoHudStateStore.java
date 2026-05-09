package com.bong.client.yidao;

import java.util.List;

/** Client-side cache for {@code yidao_hud_state}. */
public final class YidaoHudStateStore {
    public record Snapshot(
        String healerId,
        int reputation,
        float peaceMastery,
        double karma,
        String activeSkill,
        List<String> patientIds,
        Float patientHpPercent,
        Double patientContamTotal,
        int severedMeridianCount,
        int contractCount,
        int massPreviewCount
    ) {
        public static final Snapshot EMPTY = new Snapshot("", 0, 0f, 0d, "", List.of(), null, null, 0, 0, 0);

        public Snapshot {
            healerId = healerId == null ? "" : healerId;
            activeSkill = activeSkill == null ? "" : activeSkill;
            patientIds = List.copyOf(patientIds == null ? List.of() : patientIds);
            peaceMastery = Math.max(0f, Math.min(100f, peaceMastery));
            karma = Math.max(0d, karma);
            severedMeridianCount = Math.max(0, severedMeridianCount);
            contractCount = Math.max(0, contractCount);
            massPreviewCount = Math.max(0, massPreviewCount);
        }

        public boolean active() {
            return !activeSkill.isBlank()
                || !patientIds.isEmpty()
                || reputation != 0
                || peaceMastery > 0f
                || karma > 0d
                || patientHpPercent != null
                || patientContamTotal != null
                || severedMeridianCount > 0
                || contractCount > 0
                || massPreviewCount > 0;
        }
    }

    private static volatile Snapshot snapshot = Snapshot.EMPTY;

    private YidaoHudStateStore() {
    }

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.EMPTY : next;
    }

    public static void resetForTests() {
        snapshot = Snapshot.EMPTY;
    }
}
