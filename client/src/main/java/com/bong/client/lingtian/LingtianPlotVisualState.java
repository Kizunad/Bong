package com.bong.client.lingtian;

import com.bong.client.lingtian.state.LingtianSessionStore;

public record LingtianPlotVisualState(
    String icon,
    String title,
    String detail,
    int runeColor,
    int fillColor,
    float progress
) {
    public static final int CYAN_RUNE = 0xFF44CCCC;
    public static final int PLANTED_RUNE = 0xFF55EE88;
    public static final int MATURE_RUNE = 0xFF88FF66;
    public static final int DEPLETED_RUNE = 0xFF888888;

    private static final LingtianPlotVisualState EMPTY = new LingtianPlotVisualState(
        "",
        "",
        "",
        CYAN_RUNE,
        0xFF66CC66,
        0.0f
    );

    public static LingtianPlotVisualState empty() {
        return EMPTY;
    }

    public static LingtianPlotVisualState fromSnapshot(LingtianSessionStore.Snapshot snapshot) {
        if (snapshot == null || !snapshot.active()) {
            return EMPTY;
        }

        LingtianSessionStore.Kind kind = snapshot.kind() == null
            ? LingtianSessionStore.Kind.TILL
            : snapshot.kind();
        int runeColor = runeColorFor(kind);
        String plant = snapshot.plantId() == null || snapshot.plantId().isBlank()
            ? "空置地块"
            : snapshot.plantId().trim();
        String detail = String.format(
            java.util.Locale.ROOT,
            "%.0f%% · 染污 %.0f%%",
            snapshot.progress() * 100.0f,
            Math.max(0.0f, Math.min(1.0f, snapshot.dyeContamination())) * 100.0f
        );
        if (snapshot.dyeContaminationWarning()) {
            detail += "!";
        }
        return new LingtianPlotVisualState(
            iconFor(kind),
            kind.label() + " · " + plant,
            detail,
            runeColor,
            fillColorFor(kind),
            snapshot.progress()
        );
    }

    private static String iconFor(LingtianSessionStore.Kind kind) {
        return switch (kind) {
            case TILL -> "耕";
            case RENEW -> "新";
            case PLANTING -> "种";
            case HARVEST -> "收";
            case REPLENISH -> "补";
            case DRAIN_QI -> "吸";
        };
    }

    private static int runeColorFor(LingtianSessionStore.Kind kind) {
        return switch (kind) {
            case TILL, RENEW -> CYAN_RUNE;
            case PLANTING -> PLANTED_RUNE;
            case HARVEST -> MATURE_RUNE;
            case REPLENISH -> CYAN_RUNE;
            case DRAIN_QI -> DEPLETED_RUNE;
        };
    }

    private static int fillColorFor(LingtianSessionStore.Kind kind) {
        return switch (kind) {
            case TILL, RENEW -> 0xFF44CCCC;
            case PLANTING -> 0xFF55EE88;
            case HARVEST -> 0xFF88FF66;
            case REPLENISH -> 0xFF66DDCC;
            case DRAIN_QI -> 0xFF9A9A9A;
        };
    }
}
