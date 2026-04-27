package com.bong.client;

import java.util.List;
import java.util.Locale;
import java.util.Objects;

public record PlayerStateViewModel(
        boolean hasState,
        String statusText,
        String realmLabel,
        String spiritQiLabel,
        String spiritQiBar,
        double spiritQiRatio,
        String karmaLabel,
        String karmaAxis,
        String compositePowerLabel,
        String zoneLabel,
        List<PowerBreakdownRow> powerBreakdown,
        boolean dynamicXmlUiEnabled
) {
    static final int BAR_SEGMENTS = 10;
    static final int KARMA_AXIS_SEGMENTS = 9;

    public PlayerStateViewModel {
        Objects.requireNonNull(statusText, "statusText");
        Objects.requireNonNull(realmLabel, "realmLabel");
        Objects.requireNonNull(spiritQiLabel, "spiritQiLabel");
        Objects.requireNonNull(spiritQiBar, "spiritQiBar");
        Objects.requireNonNull(karmaLabel, "karmaLabel");
        Objects.requireNonNull(karmaAxis, "karmaAxis");
        Objects.requireNonNull(compositePowerLabel, "compositePowerLabel");
        Objects.requireNonNull(zoneLabel, "zoneLabel");
        powerBreakdown = List.copyOf(Objects.requireNonNull(powerBreakdown, "powerBreakdown"));
    }

    public static PlayerStateViewModel fromCurrentState() {
        return from(PlayerStateState.getCurrentPlayerState());
    }

    static PlayerStateViewModel from(PlayerStateState.PlayerStateSnapshot snapshot) {
        if (snapshot == null) {
            return empty();
        }

        double spiritQiRatio = PlayerStateState.spiritQiRatio(snapshot.spiritQi(), snapshot.spiritQiMax());
        PowerBreakdown breakdown = derivePowerBreakdown(snapshot.realmKey(), spiritQiRatio, snapshot.karma(), snapshot.compositePower());

        return new PlayerStateViewModel(
                true,
                "本地只读状态",
                humanizeRealm(snapshot.realmKey()),
                formatAmount(snapshot.spiritQi()) + " / " + formatAmount(snapshot.spiritQiMax()),
                blockBar(spiritQiRatio),
                spiritQiRatio,
                formatSigned(snapshot.karma()),
                karmaAxis(snapshot.karma()),
                formatUnit(snapshot.compositePower()),
                ZoneState.clipLabel(ZoneState.humanizeZoneName(snapshot.zoneKey()), ZoneState.MAX_ZONE_LABEL_LENGTH),
                List.of(
                        breakdownRow("战斗", breakdown.combat()),
                        breakdownRow("财富", breakdown.wealth()),
                        breakdownRow("社交", breakdown.social()),
                        breakdownRow("领地", breakdown.territory())
                ),
                CultivationUiFeatures.isDynamicXmlUiEnabled()
        );
    }

    static PlayerStateViewModel empty() {
        return new PlayerStateViewModel(
                false,
                "尚未收到 player_state 载荷",
                "未感应",
                "0 / 0",
                blockBar(0.0d),
                0.0d,
                formatSigned(0.0d),
                karmaAxis(0.0d),
                formatUnit(0.0d),
                "未知区域",
                List.of(
                        breakdownRow("战斗", 0.0d),
                        breakdownRow("财富", 0.0d),
                        breakdownRow("社交", 0.0d),
                        breakdownRow("领地", 0.0d)
                ),
                CultivationUiFeatures.isDynamicXmlUiEnabled()
        );
    }

    public String dynamicXmlUiLabel() {
        return dynamicXmlUiEnabled ? "ON" : "OFF";
    }

    static String humanizeRealm(String realmKey) {
        Objects.requireNonNull(realmKey, "realmKey");

        String trimmed = realmKey.trim();
        if (trimmed.isEmpty()) {
            return "凡体";
        }

        String normalized = trimmed.toLowerCase(Locale.ROOT);
        return switch (normalized) {
            case "awaken" -> "醒灵";
            case "induce" -> "引气";
            case "condense" -> "凝脉";
            case "solidify" -> "固元";
            case "spirit" -> "通灵";
            case "void" -> "化虚";
            default -> trimmed;
        };
    }

    static String blockBar(double ratio) {
        int filledSegments = (int) Math.round(PlayerStateState.clampUnit(ratio) * BAR_SEGMENTS);
        StringBuilder builder = new StringBuilder(BAR_SEGMENTS);
        for (int index = 0; index < BAR_SEGMENTS; index++) {
            builder.append(index < filledSegments ? '█' : '░');
        }
        return builder.toString();
    }

    static String karmaAxis(double karma) {
        int position = (int) Math.round(((PlayerStateState.clampKarma(karma) + 1.0d) * 0.5d) * (KARMA_AXIS_SEGMENTS - 1));
        StringBuilder builder = new StringBuilder("善 ");
        for (int index = 0; index < KARMA_AXIS_SEGMENTS; index++) {
            builder.append(index == position ? '●' : '═');
        }
        builder.append(" 恶");
        return builder.toString();
    }

    static String formatSigned(double value) {
        return String.format(Locale.ROOT, "%+.2f", value);
    }

    static String formatUnit(double value) {
        return String.format(Locale.ROOT, "%.2f", PlayerStateState.clampUnit(value));
    }

    static String formatAmount(double value) {
        double normalized = Double.isFinite(value) ? value : 0.0d;
        if (Math.abs(normalized - Math.rint(normalized)) < 0.0001d) {
            return String.format(Locale.ROOT, "%.0f", normalized);
        }

        return String.format(Locale.ROOT, "%.1f", normalized);
    }

    private static PowerBreakdown derivePowerBreakdown(String realmKey, double spiritQiRatio, double karma, double compositePower) {
        double realmScore = realmProgressScore(realmKey);
        double karmaAlignment = (PlayerStateState.clampKarma(karma) + 1.0d) * 0.5d;
        double anchoredPower = PlayerStateState.clampUnit(compositePower);

        return new PowerBreakdown(
                PlayerStateState.clampUnit((realmScore * 0.6d) + (spiritQiRatio * 0.4d)),
                PlayerStateState.clampUnit((anchoredPower * 0.7d) + (spiritQiRatio * 0.3d)),
                PlayerStateState.clampUnit((anchoredPower * 0.5d) + (karmaAlignment * 0.5d)),
                PlayerStateState.clampUnit((anchoredPower * 0.6d) + (realmScore * 0.4d))
        );
    }

    private static double realmProgressScore(String realmKey) {
        String normalized = PlayerStateState.normalizeRealmKey(realmKey).trim().toLowerCase(Locale.ROOT);
        if (normalized.isEmpty()) {
            return 0.05d;
        }

        return switch (normalized) {
            case "awaken" -> 0.15d;
            case "induce" -> 0.25d;
            case "condense" -> 0.45d;
            case "solidify" -> 0.65d;
            case "spirit" -> 0.85d;
            case "void" -> 1.0d;
            default -> 0.0d;
        };
    }

    private static PowerBreakdownRow breakdownRow(String label, double value) {
        return new PowerBreakdownRow(label, value, formatUnit(value), blockBar(value));
    }

    private record PowerBreakdown(double combat, double wealth, double social, double territory) {
    }

    public record PowerBreakdownRow(String label, double value, String valueLabel, String barText) {
        public PowerBreakdownRow {
            Objects.requireNonNull(label, "label");
            Objects.requireNonNull(valueLabel, "valueLabel");
            Objects.requireNonNull(barText, "barText");
        }
    }
}
