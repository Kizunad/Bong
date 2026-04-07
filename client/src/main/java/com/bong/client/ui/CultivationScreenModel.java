package com.bong.client.ui;

import com.bong.client.PlayerStateCache;

import java.util.List;
import java.util.Locale;

public record CultivationScreenModel(
    boolean synced,
    String realmLabel,
    String spiritQiText,
    double spiritQiRatio,
    String karmaText,
    double karmaRatio,
    String compositePowerText,
    List<PowerEntry> breakdownEntries,
    String zoneText,
    String footerText
) {
    private static final double DEFAULT_SPIRIT_QI_CAPACITY = 100.0;

    public static CultivationScreenModel from(PlayerStateCache.PlayerStateSnapshot snapshot) {
        if (snapshot == null) {
            return new CultivationScreenModel(
                false,
                "未同步",
                formatSpiritQi(0.0, DEFAULT_SPIRIT_QI_CAPACITY),
                0.0,
                formatSignedDecimal(0.0),
                0.5,
                formatUnitDecimal(0.0),
                zeroBreakdownEntries(),
                "未知区域",
                "等待 server 下发 player_state"
            );
        }

        double spiritQiMax = Math.max(DEFAULT_SPIRIT_QI_CAPACITY, Math.ceil(snapshot.spiritQi()));
        return new CultivationScreenModel(
            true,
            formatRealm(snapshot.realm()),
            formatSpiritQi(snapshot.spiritQi(), spiritQiMax),
            clampUnit(snapshot.spiritQi() / spiritQiMax),
            formatSignedDecimal(snapshot.karma()),
            clampUnit((snapshot.karma() + 1.0) * 0.5),
            formatUnitDecimal(snapshot.compositePower()),
            buildBreakdownEntries(snapshot.breakdown()),
            formatDisplayName(snapshot.zone()),
            "按 K 可随时重新打开此面板"
        );
    }

    private static List<PowerEntry> zeroBreakdownEntries() {
        return List.of(
            new PowerEntry("战斗", 0.0),
            new PowerEntry("财富", 0.0),
            new PowerEntry("社交", 0.0),
            new PowerEntry("因果", 0.0),
            new PowerEntry("领地", 0.0)
        );
    }

    private static List<PowerEntry> buildBreakdownEntries(PlayerStateCache.PowerBreakdown breakdown) {
        return List.of(
            new PowerEntry("战斗", breakdown.combat()),
            new PowerEntry("财富", breakdown.wealth()),
            new PowerEntry("社交", breakdown.social()),
            new PowerEntry("因果", breakdown.karma()),
            new PowerEntry("领地", breakdown.territory())
        );
    }

    static String formatRealm(String realm) {
        if (realm == null || realm.isBlank()) {
            return "未同步";
        }

        String normalized = realm.trim().toLowerCase(Locale.ROOT);
        if (normalized.equals("mortal")) {
            return "凡人";
        }
        if (normalized.equals("golden_core")) {
            return "金丹";
        }
        if (normalized.equals("nascent_soul")) {
            return "元婴";
        }

        Integer qiRefiningStage = parseStage(normalized, "qi_refining_");
        if (qiRefiningStage != null) {
            return "练气" + chineseNumber(qiRefiningStage) + "层";
        }

        Integer foundationStage = parseStage(normalized, "foundation_establishment_");
        if (foundationStage == null) {
            foundationStage = parseStage(normalized, "foundation_");
        }
        if (foundationStage != null) {
            return "筑基" + chineseNumber(foundationStage) + "层";
        }

        return formatDisplayName(realm);
    }

    static String formatDisplayName(String rawValue) {
        if (rawValue == null || rawValue.isBlank()) {
            return "未知区域";
        }

        String normalized = rawValue.trim().replace('_', ' ').replace('-', ' ');
        String[] parts = normalized.split("\\s+");
        StringBuilder builder = new StringBuilder();
        for (String part : parts) {
            if (part.isBlank()) {
                continue;
            }

            if (builder.length() > 0) {
                builder.append(' ');
            }

            if (part.chars().allMatch(character -> character < 128 && Character.isLetterOrDigit(character))) {
                builder.append(Character.toUpperCase(part.charAt(0)));
                if (part.length() > 1) {
                    builder.append(part.substring(1));
                }
            } else {
                builder.append(part);
            }
        }

        return builder.length() == 0 ? "未知区域" : builder.toString();
    }

    private static Integer parseStage(String realm, String prefix) {
        if (!realm.startsWith(prefix)) {
            return null;
        }

        try {
            return Integer.parseInt(realm.substring(prefix.length()));
        } catch (NumberFormatException ignored) {
            return null;
        }
    }

    private static String chineseNumber(int value) {
        return switch (value) {
            case 1 -> "一";
            case 2 -> "二";
            case 3 -> "三";
            case 4 -> "四";
            case 5 -> "五";
            case 6 -> "六";
            case 7 -> "七";
            case 8 -> "八";
            case 9 -> "九";
            case 10 -> "十";
            case 11 -> "十一";
            case 12 -> "十二";
            default -> Integer.toString(value);
        };
    }

    private static String formatSpiritQi(double spiritQi, double spiritQiMax) {
        return formatCompactNumber(spiritQi) + " / " + formatCompactNumber(spiritQiMax);
    }

    private static String formatSignedDecimal(double value) {
        return String.format(Locale.ROOT, "%+.2f", value);
    }

    private static String formatUnitDecimal(double value) {
        return String.format(Locale.ROOT, "%.2f", value);
    }

    private static String formatCompactNumber(double value) {
        long rounded = Math.round(value);
        if (Math.abs(value - rounded) < 1e-9) {
            return Long.toString(rounded);
        }

        return String.format(Locale.ROOT, "%.1f", value);
    }

    private static double clampUnit(double value) {
        return Math.max(0.0, Math.min(1.0, value));
    }

    public record PowerEntry(String label, double value) {
        public String valueText() {
            return String.format(Locale.ROOT, "%.2f", value);
        }

        public double ratio() {
            return Math.max(0.0, Math.min(1.0, value));
        }
    }
}
