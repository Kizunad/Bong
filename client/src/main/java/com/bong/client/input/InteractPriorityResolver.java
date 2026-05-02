package com.bong.client.input;

import java.util.List;
import java.util.Optional;

public final class InteractPriorityResolver {
    private InteractPriorityResolver() {
    }

    public static Optional<InteractCandidate> choose(List<InteractCandidate> candidates) {
        if (candidates == null || candidates.isEmpty()) {
            return Optional.empty();
        }

        InteractCandidate best = null;
        for (InteractCandidate candidate : candidates) {
            if (candidate == null || candidate.intent() == InteractIntent.None) {
                continue;
            }
            if (best == null || isBetter(candidate, best)) {
                best = candidate;
            }
        }
        return Optional.ofNullable(best);
    }

    private static boolean isBetter(InteractCandidate candidate, InteractCandidate best) {
        if (candidate.priority() != best.priority()) {
            return candidate.priority() > best.priority();
        }
        return candidate.distanceSq() < best.distanceSq();
    }
}
