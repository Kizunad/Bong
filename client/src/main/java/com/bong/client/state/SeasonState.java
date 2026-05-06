package com.bong.client.state;

import java.util.Locale;
import java.util.Optional;

public record SeasonState(
    Phase phase,
    long tickIntoPhase,
    long phaseTotalTicks,
    long yearIndex
) {
    public static final long SUMMER_TICKS = 1_382_400L;

    public SeasonState {
        phase = phase == null ? Phase.SUMMER : phase;
        tickIntoPhase = Math.max(0L, tickIntoPhase);
        phaseTotalTicks = Math.max(1L, phaseTotalTicks);
        yearIndex = Math.max(0L, yearIndex);
    }

    public static SeasonState summerAt(long tick) {
        return new SeasonState(Phase.SUMMER, Math.max(0L, tick), SUMMER_TICKS, 0L);
    }

    public enum Phase {
        SUMMER,
        SUMMER_TO_WINTER,
        WINTER,
        WINTER_TO_SUMMER;

        public boolean tideTurn() {
            return this == SUMMER_TO_WINTER || this == WINTER_TO_SUMMER;
        }

        public static Optional<Phase> fromWire(String raw) {
            if (raw == null || raw.isBlank()) {
                return Optional.empty();
            }
            return switch (raw.trim().toLowerCase(Locale.ROOT)) {
                case "summer" -> Optional.of(SUMMER);
                case "summer_to_winter" -> Optional.of(SUMMER_TO_WINTER);
                case "winter" -> Optional.of(WINTER);
                case "winter_to_summer" -> Optional.of(WINTER_TO_SUMMER);
                default -> Optional.empty();
            };
        }
    }
}
