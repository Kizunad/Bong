package com.bong.client.hud;

import java.util.concurrent.atomic.AtomicReference;

public final class PoisonTraitHudStateStore {
    public record State(
        boolean active,
        float toxicity,
        float digestionCurrent,
        float digestionCapacity,
        long lifespanWarningUntilMillis,
        float lifespanYearsLost
    ) {
        public static final State NONE = new State(false, 0.0f, 0.0f, 100.0f, 0L, 0.0f);

        public State {
            toxicity = clamp(toxicity, 0.0f, 100.0f);
            digestionCapacity = Float.isFinite(digestionCapacity) && digestionCapacity > 0.0f
                ? digestionCapacity
                : 1.0f;
            digestionCurrent = clamp(digestionCurrent, 0.0f, digestionCapacity);
            lifespanYearsLost = Float.isFinite(lifespanYearsLost)
                ? Math.max(0.0f, lifespanYearsLost)
                : 0.0f;
        }

        public float toxicityRatio() {
            return toxicity / 100.0f;
        }

        public float digestionRatio() {
            return digestionCurrent / digestionCapacity;
        }

        public String toxicityTierLabel() {
            if (toxicity < 30.0f) return "轻毒";
            if (toxicity <= 70.0f) return "中毒";
            return "重毒";
        }

        private static float clamp(float value, float min, float max) {
            if (!Float.isFinite(value)) return min;
            return Math.max(min, Math.min(max, value));
        }
    }

    private static final AtomicReference<State> STATE = new AtomicReference<>(State.NONE);

    private PoisonTraitHudStateStore() {
    }

    public static State snapshot() {
        return STATE.get();
    }

    public static void update(State state) {
        STATE.set(state == null ? State.NONE : state);
    }

    public static void clear() {
        STATE.set(State.NONE);
    }
}
