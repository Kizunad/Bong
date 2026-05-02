package com.bong.client.input;

import java.util.Objects;

public record InteractCandidate(
    InteractIntent intent,
    int priority,
    double distanceSq,
    String debugLabel
) {
    public InteractCandidate {
        Objects.requireNonNull(intent, "intent");
        if (priority < 0) {
            throw new IllegalArgumentException("priority must be >= 0");
        }
        if (!Double.isFinite(distanceSq) || distanceSq < 0.0) {
            throw new IllegalArgumentException("distanceSq must be finite and >= 0");
        }
        debugLabel = debugLabel == null ? "" : debugLabel;
    }

    public static InteractCandidate of(
        InteractIntent intent,
        int priority,
        double distanceSq,
        String debugLabel
    ) {
        return new InteractCandidate(intent, priority, distanceSq, debugLabel);
    }
}
