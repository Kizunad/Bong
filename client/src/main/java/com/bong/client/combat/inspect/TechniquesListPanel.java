package com.bong.client.combat.inspect;

import com.bong.client.inventory.model.MeridianChannel;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Optional;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

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
        List<String> aliases,
        Grade grade,
        float proficiency,     // 0..1
        boolean active,         // maintainable toggle
        String castKey,         // which quick slot, or ""
        String description,
        String requiredRealm,
        List<RequiredMeridian> requiredMeridians,
        float qiCost,
        int castTicks,
        int cooldownTicks,
        float range
    ) {
        public Technique(
            String id,
            String displayName,
            Grade grade,
            float proficiency,
            boolean active,
            String castKey,
            String description,
            String requiredRealm,
            List<RequiredMeridian> requiredMeridians,
            float qiCost,
            int castTicks,
            int cooldownTicks,
            float range
        ) {
            this(
                id,
                displayName,
                List.of(),
                grade,
                proficiency,
                active,
                castKey,
                description,
                requiredRealm,
                requiredMeridians,
                qiCost,
                castTicks,
                cooldownTicks,
                range
            );
        }

        public Technique {
            id = id == null ? "" : id;
            displayName = displayName == null ? "" : displayName;
            aliases = aliases == null ? List.of() : List.copyOf(aliases);
            grade = grade == null ? Grade.MORTAL : grade;
            if (proficiency < 0f) proficiency = 0f;
            if (proficiency > 1f) proficiency = 1f;
            castKey = castKey == null ? "" : castKey;
            description = description == null ? "" : description;
            requiredRealm = requiredRealm == null ? "" : requiredRealm;
            requiredMeridians = requiredMeridians == null
                ? List.of()
                : List.copyOf(requiredMeridians);
            if (!Float.isFinite(qiCost) || qiCost < 0f) qiCost = 0f;
            castTicks = Math.max(0, castTicks);
            cooldownTicks = Math.max(0, cooldownTicks);
            if (!Float.isFinite(range) || range < 0f) range = 0f;
        }
    }

    public record RequiredMeridian(String channel, float minHealth) {
        public RequiredMeridian {
            channel = channel == null ? "" : channel;
            if (!Float.isFinite(minHealth) || minHealth < 0f) minHealth = 0f;
            if (minHealth > 1f) minHealth = 1f;
        }
    }

    private static volatile List<Technique> snapshot = Collections.emptyList();
    private static final List<Consumer<List<Technique>>> listeners = new CopyOnWriteArrayList<>();

    private TechniquesListPanel() {}

    public static List<Technique> snapshot() { return snapshot; }

    public static void replace(List<Technique> next) {
        snapshot = (next == null || next.isEmpty())
            ? Collections.emptyList()
            : Collections.unmodifiableList(new java.util.ArrayList<>(next));
        for (Consumer<List<Technique>> listener : listeners) listener.accept(snapshot);
    }

    public static void addListener(Consumer<List<Technique>> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<List<Technique>> listener) {
        listeners.remove(listener);
    }

    public static List<Technique> filter(List<Technique> source, String query) {
        if (source == null || source.isEmpty()) return List.of();
        if (query == null || query.isBlank()) return List.copyOf(source);
        List<Technique> out = new ArrayList<>();
        for (Technique technique : source) {
            if (matchesQuery(technique, query)) out.add(technique);
        }
        return out;
    }

    public static boolean matchesQuery(Technique technique, String query) {
        if (technique == null) return false;
        if (query == null || query.isBlank()) return true;
        String needle = normalizeSearch(query);
        if (containsNormalized(technique.id(), needle) || containsNormalized(technique.displayName(), needle)) {
            return true;
        }
        for (String alias : technique.aliases()) {
            if (containsNormalized(alias, needle)) return true;
        }
        return false;
    }

    public static List<MeridianChannel> requiredChannels(Technique technique) {
        if (technique == null || technique.requiredMeridians().isEmpty()) return List.of();
        List<MeridianChannel> channels = new ArrayList<>();
        for (RequiredMeridian required : technique.requiredMeridians()) {
            channelFromWire(required.channel()).ifPresent(channels::add);
        }
        return channels;
    }

    public static Optional<MeridianChannel> channelFromWire(String wire) {
        if (wire == null || wire.isBlank()) return Optional.empty();
        String key = normalizeMeridian(wire);
        for (MeridianChannel channel : MeridianChannel.values()) {
            if (normalizeMeridian(channel.name()).equals(key)
                || normalizeMeridian(channel.displayName()).equals(key)) {
                return Optional.of(channel);
            }
        }
        return Optional.ofNullable(switch (key) {
            case "lung", "lu" -> MeridianChannel.LU;
            case "largeintestine", "li" -> MeridianChannel.LI;
            case "stomach", "st" -> MeridianChannel.ST;
            case "spleen", "sp" -> MeridianChannel.SP;
            case "heart", "ht" -> MeridianChannel.HT;
            case "smallintestine", "si" -> MeridianChannel.SI;
            case "bladder", "bl" -> MeridianChannel.BL;
            case "kidney", "ki" -> MeridianChannel.KI;
            case "pericardium", "pc" -> MeridianChannel.PC;
            case "tripleenergizer", "te" -> MeridianChannel.TE;
            case "gallbladder", "gb" -> MeridianChannel.GB;
            case "liver", "lr" -> MeridianChannel.LR;
            case "ren" -> MeridianChannel.REN;
            case "du" -> MeridianChannel.DU;
            case "chong" -> MeridianChannel.CHONG;
            case "dai" -> MeridianChannel.DAI;
            case "yinqiao" -> MeridianChannel.YIN_QIAO;
            case "yangqiao" -> MeridianChannel.YANG_QIAO;
            case "yinwei" -> MeridianChannel.YIN_WEI;
            case "yangwei" -> MeridianChannel.YANG_WEI;
            default -> null;
        });
    }

    public static void clearListenersForTests() {
        listeners.clear();
    }

    public static void resetForTests() {
        snapshot = Collections.emptyList();
        listeners.clear();
    }

    private static boolean containsNormalized(String value, String needle) {
        return value != null && normalizeSearch(value).contains(needle);
    }

    private static String normalizeSearch(String value) {
        return value == null ? "" : value.trim().toLowerCase(Locale.ROOT);
    }

    private static String normalizeMeridian(String value) {
        return normalizeSearch(value).replace("_", "").replace("-", "").replace(" ", "");
    }
}
