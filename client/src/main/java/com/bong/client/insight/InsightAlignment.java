package com.bong.client.insight;

public enum InsightAlignment {
    CONVERGE("converge", "»«", "深化", 0x33C0C0E0, 0xFFC0C0E0),
    NEUTRAL("neutral", "--", "通用", 0x26808080, 0xFFE0E0E0),
    DIVERGE("diverge", "«»", "转向", 0x33C0B0D0, 0xFFC0B0D0);

    private final String wire;
    private final String icon;
    private final String label;
    private final int cardTintArgb;
    private final int gainArgb;

    InsightAlignment(String wire, String icon, String label, int cardTintArgb, int gainArgb) {
        this.wire = wire;
        this.icon = icon;
        this.label = label;
        this.cardTintArgb = cardTintArgb;
        this.gainArgb = gainArgb;
    }

    public String wire() {
        return wire;
    }

    public String icon() {
        return icon;
    }

    public String label() {
        return label;
    }

    public int cardTintArgb() {
        return cardTintArgb;
    }

    public int gainArgb() {
        return gainArgb;
    }

    public static InsightAlignment parse(String wire) {
        if (wire == null) {
            return NEUTRAL;
        }
        return switch (wire) {
            case "converge" -> CONVERGE;
            case "diverge" -> DIVERGE;
            default -> NEUTRAL;
        };
    }
}
