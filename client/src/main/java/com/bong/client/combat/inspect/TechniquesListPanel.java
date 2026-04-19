package com.bong.client.combat.inspect;

import java.util.Collections;
import java.util.List;

/**
 * Data provider for the "已学功法" inspect list (plan §U-parallel / §2.2).
 * Entries are pushed from a future {@code techniques_snapshot} handler; for
 * now this class just holds the store with a neutral empty default so other
 * UI can already bind.
 */
public final class TechniquesListPanel {

    public enum Grade {
        MORTAL("凡阶", 0xFFB0B0B0),
        YELLOW("黄阶", 0xFFFFE080),
        PROFOUND("玄阶", 0xFF80C0FF),
        EARTH("地阶", 0xFF60FFA0),
        HEAVEN("天阶", 0xFFFF80E0),
        IMMORTAL("仙阶", 0xFFFFFFFF);

        private final String label;
        private final int color;

        Grade(String label, int color) {
            this.label = label;
            this.color = color;
        }
        public String label() { return label; }
        public int color() { return color; }

        public static Grade fromWire(String wire) {
            if (wire == null) return MORTAL;
            return switch (wire.trim().toLowerCase(java.util.Locale.ROOT)) {
                case "yellow" -> YELLOW;
                case "profound" -> PROFOUND;
                case "earth" -> EARTH;
                case "heaven" -> HEAVEN;
                case "immortal" -> IMMORTAL;
                default -> MORTAL;
            };
        }
    }

    public record Technique(
        String id,
        String displayName,
        Grade grade,
        float proficiency,     // 0..1
        boolean active,         // maintainable toggle
        String castKey          // which quick slot, or ""
    ) {
        public Technique {
            id = id == null ? "" : id;
            displayName = displayName == null ? "" : displayName;
            grade = grade == null ? Grade.MORTAL : grade;
            if (proficiency < 0f) proficiency = 0f;
            if (proficiency > 1f) proficiency = 1f;
            castKey = castKey == null ? "" : castKey;
        }
    }

    private static volatile List<Technique> snapshot = Collections.emptyList();

    private TechniquesListPanel() {}

    public static List<Technique> snapshot() { return snapshot; }

    public static void replace(List<Technique> next) {
        snapshot = (next == null || next.isEmpty())
            ? Collections.emptyList()
            : Collections.unmodifiableList(new java.util.ArrayList<>(next));
    }

    public static void resetForTests() {
        snapshot = Collections.emptyList();
    }
}
