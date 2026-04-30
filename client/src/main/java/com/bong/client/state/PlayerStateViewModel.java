package com.bong.client.state;

import java.util.Objects;

public final class PlayerStateViewModel {
    private static final double DEFAULT_SPIRIT_QI_MAX = 100.0;

    private final String realm;
    private final String playerId;
    private final double spiritQiCurrent;
    private final double spiritQiMax;
    private final double spiritQiFillRatio;
    private final double karma;
    private final double compositePower;
    private final PowerBreakdown breakdown;
    private final String zoneId;
    private final String zoneLabel;
    private final double zoneSpiritQiNormalized;

    private PlayerStateViewModel(
        String realm,
        String playerId,
        double spiritQiCurrent,
        double spiritQiMax,
        double spiritQiFillRatio,
        double karma,
        double compositePower,
        PowerBreakdown breakdown,
        String zoneId,
        String zoneLabel,
        double zoneSpiritQiNormalized
    ) {
        this.realm = Objects.requireNonNull(realm, "realm");
        this.playerId = Objects.requireNonNull(playerId, "playerId");
        this.spiritQiCurrent = spiritQiCurrent;
        this.spiritQiMax = spiritQiMax;
        this.spiritQiFillRatio = spiritQiFillRatio;
        this.karma = karma;
        this.compositePower = compositePower;
        this.breakdown = Objects.requireNonNull(breakdown, "breakdown");
        this.zoneId = Objects.requireNonNull(zoneId, "zoneId");
        this.zoneLabel = Objects.requireNonNull(zoneLabel, "zoneLabel");
        this.zoneSpiritQiNormalized = zoneSpiritQiNormalized;
    }

    public static PlayerStateViewModel empty() {
        return new PlayerStateViewModel(
            "",
            "",
            0.0,
            DEFAULT_SPIRIT_QI_MAX,
            0.0,
            0.0,
            0.0,
            PowerBreakdown.empty(),
            "",
            "",
            0.0
        );
    }

    public static PlayerStateViewModel create(
        String realm,
        String playerId,
        double spiritQiCurrent,
        double spiritQiMax,
        double karma,
        double compositePower,
        PowerBreakdown breakdown,
        String zoneId,
        String zoneLabel,
        double zoneSpiritQiNormalized
    ) {
        String normalizedRealm = normalizeText(realm);
        if (normalizedRealm.isEmpty()) {
            return empty();
        }
        String normalizedPlayerId = normalizeText(playerId);

        double normalizedCurrentBase = clampNonNegative(spiritQiCurrent);
        double normalizedMax = normalizeSpiritQiMax(spiritQiMax, normalizedCurrentBase);
        double normalizedCurrent = clamp(normalizedCurrentBase, 0.0, normalizedMax);
        String normalizedZoneId = normalizeText(zoneId);
        String normalizedZoneLabel = normalizeText(zoneLabel);
        if (normalizedZoneId.isEmpty()) {
            normalizedZoneId = normalizedZoneLabel;
        }
        if (normalizedZoneLabel.isEmpty()) {
            normalizedZoneLabel = normalizedZoneId;
        }

        return new PlayerStateViewModel(
            normalizedRealm,
            normalizedPlayerId,
            normalizedCurrent,
            normalizedMax,
            normalizedCurrent / normalizedMax,
            clamp(karma, -1.0, 1.0),
            clamp(compositePower, 0.0, 1.0),
            breakdown == null ? PowerBreakdown.empty() : breakdown,
            normalizedZoneId,
            normalizedZoneLabel,
            clamp(zoneSpiritQiNormalized, 0.0, 1.0)
        );
    }

    private static String normalizeText(String value) {
        return value == null ? "" : value.trim();
    }

    private static double normalizeSpiritQiMax(double spiritQiMax, double spiritQiCurrent) {
        if (!Double.isFinite(spiritQiMax) || spiritQiMax <= 0.0) {
            return Math.max(DEFAULT_SPIRIT_QI_MAX, spiritQiCurrent);
        }
        return Math.max(1.0, spiritQiMax);
    }

    private static double clampNonNegative(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, value);
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }

    public String realm() {
        return realm;
    }

    public String playerId() {
        return playerId;
    }

    public double spiritQiCurrent() {
        return spiritQiCurrent;
    }

    public double spiritQiMax() {
        return spiritQiMax;
    }

    public double spiritQiFillRatio() {
        return spiritQiFillRatio;
    }

    public double karma() {
        return karma;
    }

    public double compositePower() {
        return compositePower;
    }

    public PowerBreakdown breakdown() {
        return breakdown;
    }

    public String zoneId() {
        return zoneId;
    }

    public String zoneLabel() {
        return zoneLabel;
    }

    public double zoneSpiritQiNormalized() {
        return zoneSpiritQiNormalized;
    }

    public boolean isEmpty() {
        return realm.isEmpty();
    }

    public static final class PowerBreakdown {
        private final double combat;
        private final double wealth;
        private final double social;
        private final double territory;

        private PowerBreakdown(double combat, double wealth, double social, double territory) {
            this.combat = combat;
            this.wealth = wealth;
            this.social = social;
            this.territory = territory;
        }

        public static PowerBreakdown empty() {
            return new PowerBreakdown(0.0, 0.0, 0.0, 0.0);
        }

        public static PowerBreakdown create(double combat, double wealth, double social, double territory) {
            return new PowerBreakdown(
                clamp(combat, 0.0, 1.0),
                clamp(wealth, 0.0, 1.0),
                clamp(social, 0.0, 1.0),
                clamp(territory, 0.0, 1.0)
            );
        }

        public double combat() {
            return combat;
        }

        public double wealth() {
            return wealth;
        }

        public double social() {
            return social;
        }

        public double territory() {
            return territory;
        }
    }
}
