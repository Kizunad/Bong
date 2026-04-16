package com.bong.client.combat.inspect;

import java.util.Collections;
import java.util.List;

/**
 * Data provider for inspect "武器/法宝 tooltip" (plan §U-parallel / §2.4).
 * Kept as a simple snapshot store so tooltip rendering can be hooked without
 * touching the live weapon engine.
 */
public final class WeaponTreasurePanel {

    public record Weapon(
        String kind,
        String material,
        String quality,
        float durabilityNorm // 0..1
    ) {
        public Weapon {
            kind = kind == null ? "" : kind;
            material = material == null ? "" : material;
            quality = quality == null ? "" : quality;
            if (durabilityNorm < 0f) durabilityNorm = 0f;
            if (durabilityNorm > 1f) durabilityNorm = 1f;
        }
    }

    public record Treasure(
        String id,
        String displayName,
        String grade,
        float bondStrength,  // 0..1
        float qiPoolNorm,    // 0..1
        List<String> abilities,
        List<String> prevOwners
    ) {
        public Treasure {
            id = id == null ? "" : id;
            displayName = displayName == null ? "" : displayName;
            grade = grade == null ? "" : grade;
            abilities = abilities == null ? List.of() : List.copyOf(abilities);
            prevOwners = prevOwners == null ? List.of() : List.copyOf(prevOwners);
            if (bondStrength < 0f) bondStrength = 0f;
            if (bondStrength > 1f) bondStrength = 1f;
            if (qiPoolNorm < 0f) qiPoolNorm = 0f;
            if (qiPoolNorm > 1f) qiPoolNorm = 1f;
        }
    }

    private static volatile Weapon weapon;
    private static volatile List<Treasure> treasures = Collections.emptyList();

    private WeaponTreasurePanel() {}

    public static Weapon weapon() { return weapon; }
    public static List<Treasure> treasures() { return treasures; }

    public static void replaceWeapon(Weapon next) {
        weapon = next;
    }

    public static void replaceTreasures(List<Treasure> next) {
        treasures = (next == null || next.isEmpty())
            ? Collections.emptyList()
            : Collections.unmodifiableList(new java.util.ArrayList<>(next));
    }

    public static String tooltip(Weapon w) {
        if (w == null) return "";
        StringBuilder sb = new StringBuilder();
        sb.append("\u6b66\u5668: ").append(w.kind());
        if (!w.material().isEmpty()) sb.append(" \u00b7 ").append(w.material());
        if (!w.quality().isEmpty()) sb.append(" [").append(w.quality()).append(']');
        sb.append('\n').append("\u8010\u4e45: ").append(Math.round(w.durabilityNorm() * 100)).append('%');
        return sb.toString();
    }

    public static String tooltip(Treasure t) {
        if (t == null) return "";
        StringBuilder sb = new StringBuilder();
        sb.append("\u6cd5\u5b9d: ").append(t.displayName());
        if (!t.grade().isEmpty()) sb.append(" [").append(t.grade()).append(']');
        sb.append('\n').append("\u9776\u5408\u5ea6: ").append(Math.round(t.bondStrength() * 100)).append('%');
        sb.append('\n').append("\u771f\u5143\u6c60: ").append(Math.round(t.qiPoolNorm() * 100)).append('%');
        if (!t.abilities().isEmpty()) {
            sb.append('\n').append("\u795e\u901a:");
            for (String a : t.abilities()) sb.append("\n  \u00b7 ").append(a);
        }
        if (!t.prevOwners().isEmpty()) {
            sb.append('\n').append("\u524d\u4e3b:").append(String.join("\u3001", t.prevOwners()));
        }
        return sb.toString();
    }

    public static void resetForTests() {
        weapon = null;
        treasures = Collections.emptyList();
    }
}
