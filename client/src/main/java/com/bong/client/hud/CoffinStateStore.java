package com.bong.client.hud;

public final class CoffinStateStore {
    public static final State OUT = new State(false, 1.0);

    private static volatile State snapshot = OUT;

    private CoffinStateStore() {
    }

    public static synchronized State snapshot() {
        return snapshot;
    }

    public static synchronized void replace(State next) {
        snapshot = next == null ? OUT : next.normalized();
    }

    public static synchronized void clear() {
        snapshot = OUT;
    }

    public static void resetForTests() {
        clear();
    }

    public record State(boolean inCoffin, double lifespanRateMultiplier) {
        State normalized() {
            double multiplier = Double.isFinite(lifespanRateMultiplier) && lifespanRateMultiplier > 0.0
                ? lifespanRateMultiplier
                : 1.0;
            return new State(inCoffin, multiplier);
        }
    }
}
