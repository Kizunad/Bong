package com.bong.client.cultivation;

import java.util.Locale;

/** 十种真元色（与 server/agent ColorKind wire 字面量对齐）。 */
public enum ColorKind {
    Sharp(0xFFE8F2FF, "锐"),
    Heavy(0xFF8F7A55, "厚"),
    Mellow(0xFFC89B5D, "醇"),
    Solid(0xFFB5C48A, "实"),
    Light(0xFFD8F8FF, "轻"),
    Intricate(0xFFC79CFF, "巧"),
    Gentle(0xFFA7E8C4, "柔"),
    Insidious(0xFF72629D, "阴"),
    Violent(0xFFFF6A4C, "烈"),
    Turbid(0xFF8A8872, "浊");

    private final int argb;
    private final String label;

    ColorKind(int argb, String label) {
        this.argb = argb;
        this.label = label;
    }

    public int argb() {
        return argb;
    }

    public String label() {
        return label;
    }

    public static ColorKind fromWire(String wire) {
        if (wire == null || wire.isBlank()) return null;
        String normalized = wire.trim().toLowerCase(Locale.ROOT);
        for (ColorKind kind : values()) {
            if (kind.name().toLowerCase(Locale.ROOT).equals(normalized)) return kind;
        }
        return null;
    }
}
