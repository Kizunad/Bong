package com.bong.client.combat.juice;

import java.util.List;
import java.util.Locale;

public enum CombatSchool {
    GENERIC("generic", "Generic", 0xFFB8A080, false),
    BAOMAI("baomai", "Baomai", 0xFFB87333, false),
    ANQI("anqi", "Shizhen", 0xFFC0C0C0, false),
    TUIKE("tuike", "Tuike", 0xFFA08030, false),
    WOLIU("woliu", "Woliu", 0xFF9966CC, true),
    ZHENFA("zhenfa", "Zhenfa", 0xFFC4A000, false),
    ZHENMAI("zhenmai", "Zhenmai", 0xFF4682B4, false),
    DUGU("dugu", "Dugu", 0xFF2E4E2E, false);

    private static final List<CombatSchool> PLAYABLE = List.of(
        BAOMAI,
        ANQI,
        TUIKE,
        WOLIU,
        ZHENFA,
        ZHENMAI,
        DUGU
    );

    private final String id;
    private final String displayName;
    private final int qiColorArgb;
    private final boolean reverseShake;

    CombatSchool(String id, String displayName, int qiColorArgb, boolean reverseShake) {
        this.id = id;
        this.displayName = displayName;
        this.qiColorArgb = qiColorArgb;
        this.reverseShake = reverseShake;
    }

    public String id() {
        return id;
    }

    public String displayName() {
        return displayName;
    }

    public int qiColorArgb() {
        return qiColorArgb;
    }

    public boolean reverseShake() {
        return reverseShake;
    }

    public static List<CombatSchool> playableSchools() {
        return PLAYABLE;
    }

    public static CombatSchool fromWire(String wire) {
        if (wire == null || wire.isBlank()) {
            return GENERIC;
        }
        String normalized = wire.trim().toLowerCase(Locale.ROOT).replace('-', '_').replace(':', '_');
        return switch (normalized) {
            case "baomai", "baomai_v3", "body", "body_refining" -> BAOMAI;
            case "anqi", "shizhen", "hidden_weapon", "needle", "dugu_needle" -> ANQI;
            case "tuike", "shed_skin", "false_skin" -> TUIKE;
            case "woliu", "vortex" -> WOLIU;
            case "zhenfa", "formation" -> ZHENFA;
            case "zhenmai", "meridian_cut", "jiemai" -> ZHENMAI;
            case "dugu", "dugu_poison", "poison", "gu" -> DUGU;
            default -> GENERIC;
        };
    }
}
